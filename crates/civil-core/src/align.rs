//! Horizontal alignments made of curvature-linear elements: tangents (k=0),
//! circular arcs (k const) and clothoid spirals (k linear in length).
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentInput {
    pub name: String,
    /// Points of intersection, [easting, northing]. First and last are start/end.
    pub pis: Vec<[f64; 2]>,
    /// Circular curve radius at each interior PI (len = pis.len()-2).
    pub radii: Vec<f64>,
    /// Optional symmetric clothoid length at each interior PI (0 = simple curve).
    #[serde(default)]
    pub spirals: Vec<f64>,
    #[serde(default)]
    pub start_station: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Kind {
    Line,
    Arc,
    Spiral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub kind: Kind,
    pub sta: f64,
    pub len: f64,
    pub start: [f64; 2],
    /// Heading at start, radians from +x (east), counter-clockwise.
    pub az0: f64,
    /// Signed curvature at start/end (+ = left turn).
    pub k0: f64,
    pub k1: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurveInfo {
    pub index: usize,
    pub radius: f64,
    pub arc_len: f64,
    pub ls_in: f64,
    pub ls_out: f64,
    pub delta_deg: f64,
    /// +1 left, -1 right
    pub dir: f64,
    pub sta_start: f64,
    pub sta_cs_start: f64,
    pub sta_cs_end: f64,
    pub sta_end: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Alignment {
    pub name: String,
    pub pis: Vec<[f64; 2]>,
    pub elements: Vec<Element>,
    pub length: f64,
    pub start_station: f64,
    /// Max gap between consecutive elements found while building (m).
    pub closure_error: f64,
}

pub fn norm(a: f64) -> f64 {
    let mut a = a % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI
    }
    if a < -PI {
        a += 2.0 * PI
    }
    a
}

const GL_X: [f64; 5] = [-0.906_179_845_938_664, -0.538_469_310_105_683, 0.0, 0.538_469_310_105_683, 0.906_179_845_938_664];
const GL_W: [f64; 5] = [0.236_926_885_056_189, 0.478_628_670_499_366, 0.568_888_888_888_889, 0.478_628_670_499_366, 0.236_926_885_056_189];

impl Element {
    pub fn heading(&self, u: f64) -> f64 {
        self.az0 + self.k0 * u + (self.k1 - self.k0) * u * u / (2.0 * self.len.max(1e-12))
    }
    pub fn curvature(&self, u: f64) -> f64 {
        self.k0 + (self.k1 - self.k0) * u / self.len.max(1e-12)
    }
    /// Point at distance u from the element start.
    pub fn point(&self, u: f64) -> [f64; 2] {
        match self.kind {
            Kind::Line => [self.start[0] + u * self.az0.cos(), self.start[1] + u * self.az0.sin()],
            Kind::Arc => {
                let k = self.k0;
                let a1 = self.az0 + k * u;
                [self.start[0] + (a1.sin() - self.az0.sin()) / k, self.start[1] - (a1.cos() - self.az0.cos()) / k]
            }
            Kind::Spiral => {
                let n = (u / 5.0).ceil().max(1.0) as usize;
                let h = u / n as f64;
                let (mut x, mut y) = (0.0, 0.0);
                for i in 0..n {
                    let m = (i as f64 + 0.5) * h;
                    for j in 0..5 {
                        let t = m + GL_X[j] * h / 2.0;
                        let th = self.heading(t);
                        x += GL_W[j] * h / 2.0 * th.cos();
                        y += GL_W[j] * h / 2.0 * th.sin();
                    }
                }
                [self.start[0] + x, self.start[1] + y]
            }
        }
    }
}

/// Clothoid from k=0 to k=1/R over Ls in local frame: (X, Y) at its end.
pub fn clothoid_xy(r: f64, ls: f64) -> (f64, f64) {
    let e = Element { kind: Kind::Spiral, sta: 0.0, len: ls, start: [0.0, 0.0], az0: 0.0, k0: 0.0, k1: 1.0 / r };
    let p = e.point(ls);
    (p[0], p[1])
}

impl Alignment {
    /// Tangent – (spiral) – arc – (spiral) – tangent geometry from PIs.
    /// Fails, never silently adjusts, if curves overlap or spirals are too long.
    pub fn build(input: AlignmentInput) -> crate::Result<Alignment> {
        let p = &input.pis;
        if p.len() < 2 {
            return Err("need at least 2 PIs".into());
        }
        let n_int = p.len() - 2;
        if input.radii.len() != n_int {
            return Err(format!("need {} radii, got {}", n_int, input.radii.len()));
        }
        let ls: Vec<f64> = if input.spirals.is_empty() { vec![0.0; n_int] } else { input.spirals.clone() };
        if ls.len() != n_int {
            return Err(format!("need {} spiral lengths, got {}", n_int, ls.len()));
        }
        let az = |a: [f64; 2], b: [f64; 2]| (b[1] - a[1]).atan2(b[0] - a[0]);
        let dist = |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let mut tl = vec![0.0; p.len()];
        let mut defl = vec![0.0; p.len()];
        for i in 1..p.len() - 1 {
            let r = input.radii[i - 1];
            let s = ls[i - 1];
            if !(r > 0.0) || s < 0.0 {
                return Err(format!("PI {i}: radius must be > 0 and spiral length >= 0"));
            }
            let d = norm(az(p[i], p[i + 1]) - az(p[i - 1], p[i]));
            if d.abs() < 1e-9 {
                return Err(format!("PI {i} has zero deflection; remove it"));
            }
            let ths = s / (2.0 * r);
            if 2.0 * ths > d.abs() + 1e-12 {
                return Err(format!("PI {i}: spirals too long for the deflection (2·Ls/(2R) = {:.4} rad > Δ = {:.4} rad)", 2.0 * ths, d.abs()));
            }
            defl[i] = d;
            tl[i] = if s > 0.0 {
                let (xs, ys) = clothoid_xy(r, s);
                let pp = ys - r * (1.0 - ths.cos());
                let kk = xs - r * ths.sin();
                (r + pp) * (d.abs() / 2.0).tan() + kk
            } else {
                r * (d.abs() / 2.0).tan()
            };
        }
        for i in 0..p.len() - 1 {
            let avail = dist(p[i], p[i + 1]);
            if tl[i] + tl[i + 1] > avail + 1e-9 {
                return Err(format!(
                    "curves overlap on segment PI{i}->PI{}: tangents {:.2}+{:.2} m > {:.2} m available; reduce radius/spiral or move PIs",
                    i + 1, tl[i], tl[i + 1], avail
                ));
            }
        }
        let mut el = Vec::new();
        let mut sta = input.start_station;
        let mut closure: f64 = 0.0;
        for i in 0..p.len() - 1 {
            let a = az(p[i], p[i + 1]);
            let (ux, uy) = (a.cos(), a.sin());
            let start = [p[i][0] + tl[i] * ux, p[i][1] + tl[i] * uy];
            if let Some(last) = el.last() {
                let e: &Element = last;
                let q = e.point(e.len);
                closure = closure.max(((q[0] - start[0]).powi(2) + (q[1] - start[1]).powi(2)).sqrt());
            }
            let len = dist(p[i], p[i + 1]) - tl[i] - tl[i + 1];
            if len > 1e-9 {
                el.push(Element { kind: Kind::Line, sta, len, start, az0: a, k0: 0.0, k1: 0.0 });
                sta += len;
            }
            if i + 1 < p.len() - 1 {
                let j = i + 1;
                let r = input.radii[j - 1];
                let s = ls[j - 1];
                let d = defl[j];
                let k = d.signum() / r;
                let ts = [p[j][0] - tl[j] * ux, p[j][1] - tl[j] * uy];
                let mut cur = Element { kind: Kind::Spiral, sta, len: s, start: ts, az0: a, k0: 0.0, k1: k };
                if s > 0.0 {
                    el.push(cur.clone());
                    sta += s;
                }
                let arc_len = r * (d.abs() - s / r);
                let a_start = if s > 0.0 { cur.point(s) } else { ts };
                let h1 = a + if s > 0.0 { k * s / 2.0 } else { 0.0 };
                cur = Element { kind: Kind::Arc, sta, len: arc_len, start: a_start, az0: h1, k0: k, k1: k };
                if arc_len > 1e-9 {
                    el.push(cur.clone());
                    sta += arc_len;
                }
                if s > 0.0 {
                    let st = cur.point(arc_len.max(0.0));
                    el.push(Element { kind: Kind::Spiral, sta, len: s, start: st, az0: h1 + k * arc_len, k0: k, k1: 0.0 });
                    sta += s;
                }
            }
        }
        if let Some(e) = el.last() {
            let q = e.point(e.len);
            let end = p[p.len() - 1];
            closure = closure.max(((q[0] - end[0]).powi(2) + (q[1] - end[1]).powi(2)).sqrt());
        }
        let length = sta - input.start_station;
        Ok(Alignment { name: input.name, pis: input.pis, elements: el, length, start_station: input.start_station, closure_error: closure })
    }

    /// From an explicit element list (e.g. LandXML CoordGeom). Stations are re-chained.
    pub fn from_elements(name: &str, start_station: f64, mut el: Vec<Element>) -> crate::Result<Alignment> {
        if el.is_empty() {
            return Err("alignment has no elements".into());
        }
        let mut sta = start_station;
        let mut closure: f64 = 0.0;
        for i in 0..el.len() {
            if i > 0 {
                let q = el[i - 1].point(el[i - 1].len);
                closure = closure.max(((q[0] - el[i].start[0]).powi(2) + (q[1] - el[i].start[1]).powi(2)).sqrt());
            }
            el[i].sta = sta;
            sta += el[i].len;
        }
        let mut pis = vec![el[0].start];
        for e in &el {
            if e.kind == Kind::Line {
                pis.push(e.point(e.len));
            }
        }
        Ok(Alignment { name: name.into(), pis, length: sta - start_station, start_station, elements: el, closure_error: closure })
    }

    pub fn start_station(&self) -> f64 {
        self.start_station
    }
    pub fn end_station(&self) -> f64 {
        self.start_station + self.length
    }

    fn find(&self, s: f64) -> Option<&Element> {
        if s < self.start_station() - 1e-6 || s > self.end_station() + 1e-6 {
            return None;
        }
        let i = self.elements.partition_point(|e| e.sta + e.len < s - 1e-9).min(self.elements.len() - 1);
        Some(&self.elements[i])
    }

    /// (x, y, heading_radians) at a station.
    pub fn point_at(&self, s: f64) -> Option<(f64, f64, f64)> {
        let e = self.find(s)?;
        let u = (s - e.sta).clamp(0.0, e.len);
        let p = e.point(u);
        Some((p[0], p[1], e.heading(u)))
    }

    pub fn curvature_at(&self, s: f64) -> f64 {
        self.find(s).map_or(0.0, |e| e.curvature((s - e.sta).clamp(0.0, e.len)))
    }

    /// Group arcs with their adjacent spirals into curves.
    pub fn curves(&self) -> Vec<CurveInfo> {
        let mut out = Vec::new();
        let el = &self.elements;
        let mut i = 0;
        while i < el.len() {
            if el[i].kind == Kind::Line {
                i += 1;
                continue;
            }
            let start = i;
            while i < el.len() && el[i].kind != Kind::Line {
                i += 1;
            }
            let grp = &el[start..i];
            let arc = grp.iter().find(|e| e.kind == Kind::Arc);
            let kmax = grp.iter().map(|e| e.k0.abs().max(e.k1.abs())).fold(0.0, f64::max);
            let first = &grp[0];
            let last = &grp[grp.len() - 1];
            let ls_in = if first.kind == Kind::Spiral { first.len } else { 0.0 };
            let ls_out = if last.kind == Kind::Spiral && grp.len() > 1 { last.len } else { 0.0 };
            let delta = norm(last.heading(last.len) - first.az0);
            out.push(CurveInfo {
                index: out.len() + 1,
                radius: if kmax > 0.0 { 1.0 / kmax } else { f64::INFINITY },
                arc_len: arc.map_or(0.0, |a| a.len),
                ls_in,
                ls_out,
                delta_deg: delta.to_degrees(),
                dir: delta.signum(),
                sta_start: first.sta,
                sta_cs_start: first.sta + ls_in,
                sta_cs_end: last.sta + last.len - ls_out,
                sta_end: last.sta + last.len,
            });
        }
        out
    }

    /// Stations of every element boundary (TS/SC/CS/ST/PC/PT).
    pub fn key_stations(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self.elements.iter().map(|e| e.sta).collect();
        v.push(self.end_station());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn right_angle_curve_geometry() {
        let a = Alignment::build(AlignmentInput { name: "t".into(), pis: vec![[0.0, 0.0], [200.0, 0.0], [200.0, 200.0]], radii: vec![100.0], spirals: vec![], start_station: 0.0 }).unwrap();
        assert!((a.length - (200.0 + PI * 50.0)).abs() < 1e-9);
        let (x, y, h) = a.point_at(a.end_station()).unwrap();
        assert!((x - 200.0).abs() < 1e-9 && (y - 200.0).abs() < 1e-9);
        assert!((h - PI / 2.0).abs() < 1e-9);
        let (x, y, _) = a.point_at(100.0 + PI * 25.0).unwrap();
        assert!((((x - 100.0).powi(2) + (y - 100.0).powi(2)).sqrt() - 100.0).abs() < 1e-9);
    }
    #[test]
    fn overlap_is_rejected() {
        let r = Alignment::build(AlignmentInput { name: "t".into(), pis: vec![[0.0, 0.0], [50.0, 0.0], [50.0, 50.0]], radii: vec![100.0], spirals: vec![], start_station: 0.0 });
        assert!(r.unwrap_err().contains("overlap"));
    }
    #[test]
    fn clothoid_matches_series() {
        // Fresnel series: X = L(1 - θ²/10 + θ⁴/216), Y = L(θ/3 - θ³/42 + θ⁵/1320), θ = L/(2R)
        let (r, l) = (200.0, 80.0);
        let t: f64 = l / (2.0 * r);
        let (x, y) = clothoid_xy(r, l);
        let xs = l * (1.0 - t.powi(2) / 10.0 + t.powi(4) / 216.0 - t.powi(6) / 9360.0);
        let ys = l * (t / 3.0 - t.powi(3) / 42.0 + t.powi(5) / 1320.0 - t.powi(7) / 75600.0);
        assert!((x - xs).abs() < 1e-8 && (y - ys).abs() < 1e-8, "{x} {xs} {y} {ys}");
    }
    #[test]
    fn spiral_curve_spiral_closes_on_outgoing_tangent() {
        for &(dx, dy) in &[(300.0, 260.0), (300.0, -150.0)] {
            let a = Alignment::build(AlignmentInput { name: "t".into(), pis: vec![[0.0, 0.0], [400.0, 0.0], [400.0 + dx, dy]], radii: vec![250.0], spirals: vec![60.0], start_station: 1000.0 }).unwrap();
            assert!(a.closure_error < 1e-6, "closure {}", a.closure_error);
            let (x, y, _) = a.point_at(a.end_station()).unwrap();
            assert!((x - 400.0 - dx).abs() < 1e-6 && (y - dy).abs() < 1e-6);
            let c = &a.curves()[0];
            assert!((c.radius - 250.0).abs() < 1e-9 && (c.ls_in - 60.0).abs() < 1e-9);
            // curvature continuous at SC
            assert!((a.curvature_at(c.sta_cs_start + 1e-6).abs() - 1.0 / 250.0).abs() < 1e-6);
        }
    }
}
