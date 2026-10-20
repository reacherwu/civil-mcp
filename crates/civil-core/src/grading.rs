//! Grading: project a pad (closed polygon at an elevation) or a 3D feature line
//! to the existing surface at cut/fill slopes, build the grading surface and
//! measure volumes.
use crate::corridor::composite_volume;
use crate::tin::{Tin, TinStats};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradingInput {
    pub name: String,
    /// Vertices [x, y, z]. For a pad give the same z for all; closed = true.
    pub vertices: Vec<[f64; 3]>,
    pub closed: bool,
    /// For open feature lines: "left" or "right" of the direction of travel.
    #[serde(default)]
    pub side: Option<String>,
    pub cut_slope_h: f64,
    pub fill_slope_h: f64,
    /// Resample spacing along edges (m)
    #[serde(default = "d_sp")]
    pub spacing_m: f64,
}
fn d_sp() -> f64 {
    2.0
}

#[derive(Debug, Clone, Serialize)]
pub struct Grading {
    pub name: String,
    pub footprint: Vec<[f64; 3]>,
    pub daylight: Vec<[f64; 3]>,
    pub cut_m3: f64,
    pub fill_m3: f64,
    pub cells: usize,
    pub cell_m: f64,
    pub warnings: Vec<String>,
    pub surface_stats: TinStats,
    #[serde(skip)]
    pub surface: Option<Tin>,
}

fn signed_area(p: &[[f64; 3]]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a / 2.0
}

pub fn point_in_poly(x: f64, y: f64, p: &[[f64; 3]]) -> bool {
    let mut inside = false;
    let n = p.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (p[i], p[j]);
        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Ear clipping of a CCW simple polygon given by indices into pts.
fn ear_clip(pts: &[[f64; 3]], idx: &[usize]) -> crate::Result<Vec<[usize; 3]>> {
    let mut v = idx.to_vec();
    let mut out = Vec::new();
    let cross = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
    let mut guard = 0;
    while v.len() > 3 {
        let n = v.len();
        let mut clipped = false;
        for i in 0..n {
            let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
            if cross(pts[a], pts[b], pts[c]) <= 1e-12 {
                continue;
            }
            let inside = v.iter().any(|&k| k != a && k != b && k != c && cross(pts[a], pts[b], pts[k]) >= 0.0 && cross(pts[b], pts[c], pts[k]) >= 0.0 && cross(pts[c], pts[a], pts[k]) >= 0.0);
            if !inside {
                out.push([a, b, c]);
                v.remove(i);
                clipped = true;
                break;
            }
        }
        guard += 1;
        if !clipped || guard > 100000 {
            return Err("pad polygon could not be triangulated (self-intersecting?)".into());
        }
    }
    if v.len() == 3 {
        out.push([v[0], v[1], v[2]]);
    }
    Ok(out)
}

pub fn grade(tin: &Tin, g: &GradingInput, cell: f64) -> crate::Result<Grading> {
    let n0 = g.vertices.len();
    if n0 < 2 || (g.closed && n0 < 3) || g.cut_slope_h <= 0.0 || g.fill_slope_h <= 0.0 || g.spacing_m <= 0.0 {
        return Err("need >=3 vertices for a pad (>=2 for a feature line) and positive slopes/spacing".into());
    }
    let mut v = g.vertices.clone();
    // orientation: outward normal is to the right of travel for CCW polygons
    let outward_right = if g.closed {
        if signed_area(&v) < 0.0 {
            v.reverse();
        }
        true
    } else {
        match g.side.as_deref() {
            Some("left") => false,
            Some("right") => true,
            _ => return Err("feature line needs side = left|right".into()),
        }
    };
    let n = v.len();
    let seg = if g.closed { n } else { n - 1 };
    let normal = |i: usize| {
        let (a, b) = (v[i], v[(i + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dy * dy).sqrt();
        if outward_right { [dy / l, -dx / l] } else { [-dy / l, dx / l] }
    };
    // footprint samples with their projection direction (miter at vertices)
    let mut fp: Vec<([f64; 3], [f64; 2])> = Vec::new();
    for i in 0..seg {
        let (a, b) = (v[i], v[(i + 1) % n]);
        let nn = normal(i);
        // vertex direction: miter between previous and this edge
        let prev = if i > 0 { Some(normal(i - 1)) } else if g.closed { Some(normal(seg - 1)) } else { None };
        let dir = match prev {
            Some(p) => {
                let m = [p[0] + nn[0], p[1] + nn[1]];
                let ml = (m[0] * m[0] + m[1] * m[1]).sqrt();
                let cos_half = ml / 2.0;
                if cos_half < 0.2 {
                    return Err(format!("vertex {i}: angle too sharp for a mitred slope"));
                }
                [m[0] / ml / cos_half, m[1] / ml / cos_half]
            }
            None => nn,
        };
        fp.push((a, dir));
        let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let k = (l / g.spacing_m).ceil() as usize;
        for j in 1..k {
            let t = j as f64 / k as f64;
            fp.push(([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), a[2] + t * (b[2] - a[2])], nn));
        }
    }
    if !g.closed {
        fp.push((v[n - 1], normal(n - 2)));
    }
    let mut warnings = Vec::new();
    let mut day = Vec::new();
    for (k, (p, d)) in fp.iter().enumerate() {
        let gz = tin.elevation_at(p[0], p[1]).ok_or_else(|| format!("footprint point {k} is outside the surface"))?;
        let cut = gz > p[2];
        let h = if cut { g.cut_slope_h } else { g.fill_slope_h };
        let s = if cut { 1.0 } else { -1.0 };
        // d is scaled so that one unit of t moves one unit normal to each adjacent edge (mitre): rise per t is 1/h
        let dl = 1.0;
        let f = |t: f64| tin.elevation_at(p[0] + t * d[0], p[1] + t * d[1]).map(|z| p[2] + s * t * dl / h - z);
        let mut t0 = 0.0;
        let mut f0 = p[2] - gz;
        let mut hit = None;
        let mut t = 0.0;
        while t < 200.0 {
            t += 0.25;
            let Some(f1) = f(t) else { break };
            if f1 == 0.0 || (f1 > 0.0) != (f0 > 0.0) {
                let (mut lo, mut hi) = (t0, t);
                for _ in 0..50 {
                    let m = (lo + hi) / 2.0;
                    match f(m) {
                        Some(fm) if (fm > 0.0) == (f0 > 0.0) && fm != 0.0 => lo = m,
                        _ => hi = m,
                    }
                }
                hit = Some(hi);
                break;
            }
            t0 = t;
            f0 = f1;
        }
        let t = hit.unwrap_or_else(|| {
            warnings.push(format!("no daylight for footprint point {k}"));
            t0
        });
        day.push([p[0] + t * d[0], p[1] + t * d[1], p[2] + s * t * dl / h]);
    }
    // grading surface: zipper strip footprint→daylight (+ pad top fan for closed)
    let mut pts: Vec<[f64; 3]> = fp.iter().map(|f| f.0).collect();
    let m = pts.len();
    pts.extend(day.iter().copied());
    let mut faces = Vec::new();
    let last = if g.closed { m } else { m - 1 };
    for i in 0..last {
        let j = (i + 1) % m;
        faces.push([i, j, m + i]);
        faces.push([j, m + j, m + i]);
    }
    if g.closed {
        // pad top: ear clipping of the footprint (collinear samples skipped)
        let idx: Vec<usize> = (0..m).filter(|&i| {
            let (p, c, n) = (pts[(i + m - 1) % m], pts[i], pts[(i + 1) % m]);
            ((c[0] - p[0]) * (n[1] - p[1]) - (n[0] - p[0]) * (c[1] - p[1])).abs() > 1e-9
        }).collect();
        faces.extend(ear_clip(&pts, &idx)?);
    }
    let (surf, stats) = Tin::from_faces(&pts, &faces)?;
    let (cut, fill, cells) = composite_volume(&surf, tin, cell);
    Ok(Grading { name: g.name.clone(), footprint: fp.iter().map(|f| f.0).collect(), daylight: day, cut_m3: cut, fill_m3: fill, cells, cell_m: cell, warnings, surface_stats: stats, surface: Some(surf) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn square_pad_on_flat_ground_matches_prismoid() {
        let mut pts = vec![];
        for i in -20..=20 {
            for j in -20..=20 {
                pts.push([i as f64 * 5.0 + (j % 2) as f64 * 0.001, j as f64 * 5.0, 50.0]);
            }
        }
        let (tin, _) = Tin::build(&pts, None).unwrap();
        let (a, h, s) = (20.0, 2.0, 2.0);
        let g = GradingInput { name: "pad".into(), vertices: vec![[-10.0, -10.0, 52.0], [10.0, -10.0, 52.0], [10.0, 10.0, 52.0], [-10.0, 10.0, 52.0]], closed: true, side: None, cut_slope_h: s, fill_slope_h: s, spacing_m: 1.0 };
        let r = grade(&tin, &g, 0.1).unwrap();
        let exact = a * a * h + 2.0 * a * s * h * h + 4.0 / 3.0 * s * s * h * h * h;
        assert!(r.cut_m3 < 1e-6);
        assert!((r.fill_m3 - exact).abs() / exact < 0.005, "{} vs {exact} area {} faces {}", r.fill_m3, r.surface_stats.area_m2, r.surface_stats.triangles);
        assert!((r.daylight[0][0] + 14.0).abs() < 1e-6 && (r.daylight[0][1] + 14.0).abs() < 1e-6);
    }
}
