use delaunator::{triangulate, Point};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Tin {
    pub pts: Vec<[f64; 3]>,
    pub tris: Vec<[usize; 3]>,
    origin: [f64; 2],
    cell: f64,
    nx: usize,
    ny: usize,
    min: [f64; 2],
    buckets: Vec<Vec<u32>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TinStats {
    pub points: usize,
    pub triangles: usize,
    pub dropped_long_edge_triangles: usize,
    pub min_z: f64,
    pub max_z: f64,
    pub area_m2: f64,
    pub bbox: [f64; 4],
}

impl Tin {
    /// Delaunay TIN. Triangles with any edge longer than `max_edge` are removed
    /// (Civil 3D calls this "maximum triangle length"); None keeps the convex hull.
    pub fn build(points: &[[f64; 3]], max_edge: Option<f64>) -> crate::Result<(Tin, TinStats)> {
        if points.len() < 3 {
            return Err("need >= 3 points".into());
        }
        let origin = [points[0][0], points[0][1]];
        let dp: Vec<Point> = points.iter().map(|p| Point { x: p[0] - origin[0], y: p[1] - origin[1] }).collect();
        let t = triangulate(&dp);
        if t.triangles.is_empty() {
            return Err("triangulation failed (collinear points?)".into());
        }
        let mut tris = Vec::new();
        let mut dropped = 0;
        for c in t.triangles.chunks(3) {
            let tri = [c[0], c[1], c[2]];
            let long = max_edge.map_or(false, |m| {
                (0..3).any(|k| {
                    let a = points[tri[k]];
                    let b = points[tri[(k + 1) % 3]];
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() > m
                })
            });
            if long {
                dropped += 1
            } else {
                tris.push(tri)
            }
        }
        let (mut mn, mut mx) = ([f64::MAX; 3], [f64::MIN; 3]);
        for p in points {
            for k in 0..3 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }
        let area: f64 = tris.iter().map(|t| tri_area(points[t[0]], points[t[1]], points[t[2]])).sum();
        let cell = ((mx[0] - mn[0]) * (mx[1] - mn[1]) / tris.len().max(1) as f64).sqrt().max(1e-6) * 2.0;
        let nx = (((mx[0] - mn[0]) / cell).ceil() as usize).max(1);
        let ny = (((mx[1] - mn[1]) / cell).ceil() as usize).max(1);
        let mut buckets = vec![Vec::new(); nx * ny];
        for (ti, t) in tris.iter().enumerate() {
            let xs = [points[t[0]][0], points[t[1]][0], points[t[2]][0]];
            let ys = [points[t[0]][1], points[t[1]][1], points[t[2]][1]];
            let cx0 = ((xs.iter().cloned().fold(f64::MAX, f64::min) - mn[0]) / cell) as usize;
            let cx1 = ((xs.iter().cloned().fold(f64::MIN, f64::max) - mn[0]) / cell) as usize;
            let cy0 = ((ys.iter().cloned().fold(f64::MAX, f64::min) - mn[1]) / cell) as usize;
            let cy1 = ((ys.iter().cloned().fold(f64::MIN, f64::max) - mn[1]) / cell) as usize;
            for cy in cy0..=cy1.min(ny - 1) {
                for cx in cx0..=cx1.min(nx - 1) {
                    buckets[cy * nx + cx].push(ti as u32);
                }
            }
        }
        let stats = TinStats {
            points: points.len(),
            triangles: tris.len(),
            dropped_long_edge_triangles: dropped,
            min_z: mn[2],
            max_z: mx[2],
            area_m2: area,
            bbox: [mn[0], mn[1], mx[0], mx[1]],
        };
        Ok((Tin { pts: points.to_vec(), tris, origin, cell, nx, ny, min: [mn[0], mn[1]], buckets }, stats))
    }

    /// Linear (planar-facet) interpolation; None outside the TIN.
    pub fn elevation_at(&self, x: f64, y: f64) -> Option<f64> {
        let _ = self.origin;
        let cx = ((x - self.min[0]) / self.cell).floor();
        let cy = ((y - self.min[1]) / self.cell).floor();
        if cx < 0.0 || cy < 0.0 || cx as usize >= self.nx || cy as usize >= self.ny {
            return None;
        }
        for &ti in &self.buckets[cy as usize * self.nx + cx as usize] {
            let t = self.tris[ti as usize];
            let (a, b, c) = (self.pts[t[0]], self.pts[t[1]], self.pts[t[2]]);
            let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if d.abs() < 1e-12 {
                continue;
            }
            let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
            let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
            let l3 = 1.0 - l1 - l2;
            let eps = -1e-9;
            if l1 >= eps && l2 >= eps && l3 >= eps {
                return Some(l1 * a[2] + l2 * b[2] + l3 * c[2]);
            }
        }
        None
    }

    /// Contour segments at every multiple of `interval`.
    pub fn contours(&self, interval: f64) -> Vec<(f64, [f64; 2], [f64; 2])> {
        let mut out = Vec::new();
        for t in &self.tris {
            let p = [self.pts[t[0]], self.pts[t[1]], self.pts[t[2]]];
            let zmin = p.iter().map(|q| q[2]).fold(f64::MAX, f64::min);
            let zmax = p.iter().map(|q| q[2]).fold(f64::MIN, f64::max);
            let mut lv = (zmin / interval).ceil() * interval;
            while lv <= zmax {
                let mut hits = Vec::new();
                for k in 0..3 {
                    let (a, b) = (p[k], p[(k + 1) % 3]);
                    if (a[2] - lv) * (b[2] - lv) < 0.0 || (a[2] == lv && b[2] != lv) {
                        let s = (lv - a[2]) / (b[2] - a[2]);
                        hits.push([a[0] + s * (b[0] - a[0]), a[1] + s * (b[1] - a[1])]);
                    }
                }
                if hits.len() >= 2 {
                    out.push((lv, hits[0], hits[1]));
                }
                lv += interval;
            }
        }
        out
    }
}

pub fn tri_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plane_is_reproduced_exactly() {
        let mut pts = vec![];
        for i in 0..10 {
            for j in 0..10 {
                let (x, y) = (i as f64 * 7.0 + (j % 2) as f64 * 0.3, j as f64 * 5.0);
                pts.push([x, y, 100.0 + 0.02 * x - 0.05 * y]);
            }
        }
        let (t, s) = Tin::build(&pts, None).unwrap();
        assert!(s.triangles > 100);
        let z = t.elevation_at(31.1, 22.2).unwrap();
        assert!((z - (100.0 + 0.02 * 31.1 - 0.05 * 22.2)).abs() < 1e-9);
        assert!(t.elevation_at(-50.0, 0.0).is_none());
    }
}
