use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentInput {
    pub name: String,
    /// Points of intersection, [easting, northing]. First and last are start/end.
    pub pis: Vec<[f64; 2]>,
    /// Circular curve radius at each interior PI (len = pis.len()-2).
    pub radii: Vec<f64>,
    #[serde(default)]
    pub start_station: f64,
}

#[derive(Debug, Clone, Serialize)]
pub enum Element {
    Tangent { sta: f64, len: f64, start: [f64; 2], az: f64 },
    Arc { sta: f64, len: f64, center: [f64; 2], radius: f64, a0: f64, dir: f64, az0: f64, pi_index: usize, deflection_deg: f64 },
}

#[derive(Debug, Clone, Serialize)]
pub struct Alignment {
    pub input: AlignmentInput,
    pub elements: Vec<Element>,
    pub length: f64,
}

fn norm(a: f64) -> f64 {
    let mut a = a;
    while a > PI {
        a -= 2.0 * PI
    }
    while a < -PI {
        a += 2.0 * PI
    }
    a
}

impl Alignment {
    /// Tangent–circular curve–tangent geometry. Fails (never silently adjusts)
    /// if curve tangents overlap.
    pub fn build(input: AlignmentInput) -> crate::Result<Alignment> {
        let p = &input.pis;
        if p.len() < 2 {
            return Err("need at least 2 PIs".into());
        }
        if input.radii.len() != p.len() - 2 {
            return Err(format!("need {} radii, got {}", p.len() - 2, input.radii.len()));
        }
        let az = |a: [f64; 2], b: [f64; 2]| (b[1] - a[1]).atan2(b[0] - a[0]);
        let dist = |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        // tangent length at each PI
        let mut tl = vec![0.0; p.len()];
        let mut defl = vec![0.0; p.len()];
        for i in 1..p.len() - 1 {
            let r = input.radii[i - 1];
            if !(r > 0.0) {
                return Err(format!("radius at PI {i} must be > 0"));
            }
            let d = norm(az(p[i], p[i + 1]) - az(p[i - 1], p[i]));
            if d.abs() < 1e-9 {
                return Err(format!("PI {i} has zero deflection; remove it"));
            }
            defl[i] = d;
            tl[i] = r * (d.abs() / 2.0).tan();
        }
        for i in 0..p.len() - 1 {
            let avail = dist(p[i], p[i + 1]);
            if tl[i] + tl[i + 1] > avail + 1e-9 {
                return Err(format!(
                    "curves overlap on segment PI{i}->PI{}: tangents {:.2}+{:.2} m > {:.2} m available; reduce radius or move PIs",
                    i + 1, tl[i], tl[i + 1], avail
                ));
            }
        }
        let mut el = Vec::new();
        let mut sta = input.start_station;
        for i in 0..p.len() - 1 {
            let a = az(p[i], p[i + 1]);
            let (ux, uy) = (a.cos(), a.sin());
            let start = [p[i][0] + tl[i] * ux, p[i][1] + tl[i] * uy];
            let len = dist(p[i], p[i + 1]) - tl[i] - tl[i + 1];
            if len > 1e-9 {
                el.push(Element::Tangent { sta, len, start, az: a });
                sta += len;
            }
            if i + 1 < p.len() - 1 {
                let j = i + 1;
                let r = input.radii[j - 1];
                let d = defl[j];
                let dir = d.signum();
                let pc = [p[j][0] - tl[j] * ux, p[j][1] - tl[j] * uy];
                let n = if dir > 0.0 { [-uy, ux] } else { [uy, -ux] };
                let center = [pc[0] + r * n[0], pc[1] + r * n[1]];
                let a0 = (pc[1] - center[1]).atan2(pc[0] - center[0]);
                let len = r * d.abs();
                el.push(Element::Arc { sta, len, center, radius: r, a0, dir, az0: a, pi_index: j, deflection_deg: d.to_degrees() });
                sta += len;
            }
        }
        let length = sta - input.start_station;
        Ok(Alignment { input, elements: el, length })
    }

    pub fn start_station(&self) -> f64 {
        self.input.start_station
    }
    pub fn end_station(&self) -> f64 {
        self.input.start_station + self.length
    }

    /// (x, y, heading_radians) at a station.
    pub fn point_at(&self, s: f64) -> Option<(f64, f64, f64)> {
        if s < self.start_station() - 1e-6 || s > self.end_station() + 1e-6 {
            return None;
        }
        for (k, e) in self.elements.iter().enumerate() {
            let last = k + 1 == self.elements.len();
            match *e {
                Element::Tangent { sta, len, start, az } => {
                    if s <= sta + len + 1e-9 || last {
                        let d = s - sta;
                        return Some((start[0] + d * az.cos(), start[1] + d * az.sin(), az));
                    }
                }
                Element::Arc { sta, len, center, radius, a0, dir, az0, .. } => {
                    if s <= sta + len + 1e-9 || last {
                        let th = (s - sta) / radius * dir;
                        let a = a0 + th;
                        return Some((center[0] + radius * a.cos(), center[1] + radius * a.sin(), az0 + th));
                    }
                }
            }
        }
        None
    }

    pub fn curves(&self) -> Vec<(usize, f64, f64, f64)> {
        self.elements
            .iter()
            .filter_map(|e| match e {
                Element::Arc { pi_index, radius, len, deflection_deg, .. } => Some((*pi_index, *radius, *len, *deflection_deg)),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn right_angle_curve_geometry() {
        let a = Alignment::build(AlignmentInput { name: "t".into(), pis: vec![[0.0, 0.0], [200.0, 0.0], [200.0, 200.0]], radii: vec![100.0], start_station: 0.0 }).unwrap();
        // L = 100 + pi/2*100 + 100
        assert!((a.length - (200.0 + PI * 50.0)).abs() < 1e-9);
        let (x, y, h) = a.point_at(a.end_station()).unwrap();
        assert!((x - 200.0).abs() < 1e-9 && (y - 200.0).abs() < 1e-9);
        assert!((h - PI / 2.0).abs() < 1e-9);
        let (x, y, _) = a.point_at(100.0 + PI * 25.0).unwrap();
        // curve midpoint lies R from center (100,100)
        assert!((((x - 100.0).powi(2) + (y - 100.0).powi(2)).sqrt() - 100.0).abs() < 1e-9);
    }
    #[test]
    fn overlap_is_rejected() {
        let r = Alignment::build(AlignmentInput { name: "t".into(), pis: vec![[0.0, 0.0], [50.0, 0.0], [50.0, 50.0]], radii: vec![100.0], start_station: 0.0 });
        assert!(r.unwrap_err().contains("overlap"));
    }
}
