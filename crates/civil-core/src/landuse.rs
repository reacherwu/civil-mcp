//! Land and utilities at concept depth: corridor footprint and right-of-way (land requirement)
//! polygons with parcel take areas, utility conflict register, UTM → geographic conversion for
//! the locality plan. Survey/title data and utility records are engineer/surveyor inputs.
use crate::corridor::Corridor;
use crate::pipes::Network;
use serde::{Deserialize, Serialize};

fn off(s: &crate::corridor::Section, o: f64) -> [f64; 2] {
    [s.x - o * s.heading.sin(), s.y + o * s.heading.cos()]
}

/// Footprint polygon (left daylight + buffer down the corridor, right back up).
pub fn footprint(c: &Corridor, buffer: f64) -> Vec<[f64; 2]> {
    let mut p: Vec<[f64; 2]> = c.sections.iter().map(|s| off(s, s.left.daylight + buffer)).collect();
    p.extend(c.sections.iter().rev().map(|s| off(s, -(s.right.daylight + buffer))));
    p
}

pub fn area(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

pub fn inside(p: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut c = false;
    let n = p.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (p[i], p[j]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parcel {
    pub id: String,
    pub owner: String,
    pub polygon: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Take {
    pub parcel: String,
    pub owner: String,
    pub parcel_area_m2: f64,
    pub take_m2: f64,
    pub take_pct: f64,
}

/// Take area by point sampling on a `cell` grid (centre points).
pub fn takes(fp: &[[f64; 2]], parcels: &[Parcel], cell: f64) -> Vec<Take> {
    parcels
        .iter()
        .map(|pc| {
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for q in &pc.polygon {
                x0 = x0.min(q[0]);
                y0 = y0.min(q[1]);
                x1 = x1.max(q[0]);
                y1 = y1.max(q[1]);
            }
            let mut n = 0usize;
            let mut y = y0 + cell / 2.0;
            while y < y1 {
                let mut x = x0 + cell / 2.0;
                while x < x1 {
                    if inside(&pc.polygon, [x, y]) && inside(fp, [x, y]) {
                        n += 1;
                    }
                    x += cell;
                }
                y += cell;
            }
            let a = area(&pc.polygon);
            let t = n as f64 * cell * cell;
            Take { parcel: pc.id.clone(), owner: pc.owner.clone(), parcel_area_m2: a, take_m2: t, take_pct: t / a * 100.0 }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Utility {
    pub id: String,
    pub kind: String,
    pub owner: String,
    pub polyline: Vec<[f64; 2]>,
    pub depth_m: f64,
    #[serde(default)]
    pub diameter_m: f64,
    #[serde(default = "d_qual")]
    pub quality_level: String,
}
fn d_qual() -> String {
    "QL-D (records)".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub utility: String,
    pub kind: String,
    pub owner: String,
    pub station: f64,
    pub x: f64,
    pub y: f64,
    pub issue: String,
    pub value_m: f64,
    pub action: String,
    pub quality_level: String,
}

fn seg_x(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<[f64; 2]> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let t = ((c[0] - a[0]) * s[1] - (c[1] - a[1]) * s[0]) / den;
    let u = ((c[0] - a[0]) * r[1] - (c[1] - a[1]) * r[0]) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some([a[0] + t * r[0], a[1] + t * r[1]])
    } else {
        None
    }
}

/// Conflicts: (1) utility crosses the centre line where the design cut lowers ground to within
/// `clear` of the utility (or fill adds cover beyond `max_cover`); (2) crossings with designed pipes
/// with vertical clearance < `clear`.
pub fn conflicts(c: &Corridor, nets: &[Network], tin: &crate::tin::Tin, us: &[Utility], clear: f64, max_cover: f64) -> Vec<Conflict> {
    let mut out = vec![];
    let cl: Vec<[f64; 2]> = c.sections.iter().map(|s| [s.x, s.y]).collect();
    for u in us {
        for w in u.polyline.windows(2) {
            for (k, cw) in cl.windows(2).enumerate() {
                if let Some(p) = seg_x(w[0], w[1], cw[0], cw[1]) {
                    let s = &c.sections[k];
                    let eg = tin.elevation_at(p[0], p[1]).unwrap_or(s.ground_cl);
                    let top = eg - u.depth_m;
                    let datum = s.cl_elev - s.left.datum.first().map_or(0.0, |d| s.cl_elev - d[1]);
                    let gap = datum - top;
                    let cover_new = s.cl_elev - top;
                    if gap < clear {
                        out.push(Conflict {
                            utility: u.id.clone(),
                            kind: u.kind.clone(),
                            owner: u.owner.clone(),
                            station: s.station,
                            x: p[0],
                            y: p[1],
                            issue: "cut/subgrade within clearance of utility".into(),
                            value_m: gap,
                            action: "pothole (QL-A), then lower/relocate or protect".into(),
                            quality_level: u.quality_level.clone(),
                        });
                    } else if cover_new > max_cover {
                        out.push(Conflict {
                            utility: u.id.clone(),
                            kind: u.kind.clone(),
                            owner: u.owner.clone(),
                            station: s.station,
                            x: p[0],
                            y: p[1],
                            issue: "fill increases cover beyond utility limit".into(),
                            value_m: cover_new,
                            action: "owner to confirm loading / provide access".into(),
                            quality_level: u.quality_level.clone(),
                        });
                    } else {
                        out.push(Conflict {
                            utility: u.id.clone(),
                            kind: u.kind.clone(),
                            owner: u.owner.clone(),
                            station: s.station,
                            x: p[0],
                            y: p[1],
                            issue: "crossing - no vertical conflict found".into(),
                            value_m: gap,
                            action: "protect in place; confirm location".into(),
                            quality_level: u.quality_level.clone(),
                        });
                    }
                }
            }
            for n in nets {
                for pp in &n.pipes {
                    let a = n.structures.iter().find(|s| s.0 == pp.from).unwrap();
                    let b = n.structures.iter().find(|s| s.0 == pp.to).unwrap();
                    if let Some(p) = seg_x(w[0], w[1], [a.1, a.2], [b.1, b.2]) {
                        let t = ((p[0] - a.1).hypot(p[1] - a.2) / pp.length.max(1e-9)).clamp(0.0, 1.0);
                        let inv = pp.invert_up + t * (pp.invert_dn - pp.invert_up);
                        let eg = tin.elevation_at(p[0], p[1]).unwrap_or(inv);
                        let top_u = eg - u.depth_m;
                        let bot_u = top_u - u.diameter_m;
                        let crown = inv + pp.diameter_m;
                        let gap = if bot_u >= crown {
                            bot_u - crown
                        } else if top_u <= inv {
                            inv - top_u
                        } else {
                            -0.0 - (crown.min(top_u) - inv.max(bot_u))
                        };
                        if gap < clear {
                            out.push(Conflict {
                                utility: u.id.clone(),
                                kind: u.kind.clone(),
                                owner: u.owner.clone(),
                                station: f64::NAN,
                                x: p[0],
                                y: p[1],
                                issue: format!("crosses {} pipe {} with clearance below minimum", n.name, pp.id),
                                value_m: gap,
                                action: "adjust pipe grade or relocate utility".into(),
                                quality_level: u.quality_level.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

/// UTM (WGS84) to latitude/longitude, Snyder (1987) / USGS PP 1395 series inverse.
pub fn utm_to_ll(e: f64, n: f64, zone: u32, north: bool) -> (f64, f64) {
    let a: f64 = 6378137.0;
    let f: f64 = 1.0 / 298.257223563;
    let e2 = f * (2.0 - f);
    let ep2 = e2 / (1.0 - e2);
    let k0 = 0.9996;
    let x = e - 500000.0;
    let y = if north { n } else { n - 10000000.0 };
    let m = y / k0;
    let mu = m / (a * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2.powi(3) / 256.0));
    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
    let p1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1 * e1 / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();
    let c1 = ep2 * p1.cos().powi(2);
    let t1 = p1.tan().powi(2);
    let n1 = a / (1.0 - e2 * p1.sin().powi(2)).sqrt();
    let r1 = a * (1.0 - e2) / (1.0 - e2 * p1.sin().powi(2)).powf(1.5);
    let d = x / (n1 * k0);
    let lat = p1
        - (n1 * p1.tan() / r1)
            * (d * d / 2.0 - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d.powi(4) / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1) * d.powi(6) / 720.0);
    let lon0 = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();
    let lon = lon0
        + (d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0 + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1) * d.powi(5) / 120.0)
            / p1.cos();
    (lat.to_degrees(), lon.to_degrees())
}

#[cfg(test)]
mod tests {
    #[test]
    fn utm_inverse_known_point() {
        // central meridian, equator → (0, -123) for zone 10
        let (la, lo) = super::utm_to_ll(500000.0, 0.0, 10, true);
        assert!(la.abs() < 1e-9 && (lo + 123.0).abs() < 1e-9);
        // area of a unit square
        assert!((super::area(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]) - 1.0).abs() < 1e-12);
    }
}
