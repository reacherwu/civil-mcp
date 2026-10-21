//! Scene export for visualisation (Blender and other glTF viewers): binary glTF 2.0 (.glb)
//! with existing ground, road pavement, earthwork slopes, walls, pipes and structures, and
//! route alternatives as draped ribbons. Coordinates are local metres about a georeference
//! origin (E0, N0, Z0) written to the scene JSON and to the glTF `extras`; glTF axes are
//! x = east, y = up, z = −north.
//!
//! With a corridor the scene also carries the pavement structure (one closed solid per assembly
//! layer, following crossfall / superelevation) and a copy of the existing ground trimmed exactly
//! to the daylight lines, so ground meets the slope catch points without showing through. The
//! untrimmed ground stays as the first mesh ("Existing ground") for pre-design views.
use crate::corridor::Corridor;
use crate::pipes::Network;
use crate::tin::Tin;
use serde_json::{json, Value};

pub struct Mesh {
    pub name: String,
    pub material: usize,
    pub pos: Vec<[f64; 3]>,
    pub idx: Vec<u32>,
}

pub const MATERIALS: &[(&str, [f32; 4], f32)] = &[
    ("Terrain", [0.36, 0.42, 0.25, 1.0], 0.95),
    ("Asphalt", [0.08, 0.08, 0.09, 1.0], 0.8),
    ("Earthwork", [0.55, 0.45, 0.32, 1.0], 0.95),
    ("Concrete", [0.7, 0.7, 0.68, 1.0], 0.7),
    ("Pipe", [0.2, 0.35, 0.8, 1.0], 0.5),
    ("Structure", [0.45, 0.45, 0.5, 1.0], 0.6),
    ("AltA", [0.9, 0.2, 0.2, 1.0], 0.6),
    ("AltB", [0.2, 0.6, 0.95, 1.0], 0.6),
    ("AltC", [0.95, 0.75, 0.1, 1.0], 0.6),
    ("AltD", [0.8, 0.3, 0.9, 1.0], 0.6),
    ("PavementLayer1", [0.10, 0.10, 0.11, 1.0], 0.8),
    ("PavementLayer2", [0.62, 0.55, 0.42, 1.0], 0.95),
    ("PavementLayer3", [0.80, 0.74, 0.60, 1.0], 0.95),
];

/// Ground / slope-edge differences below this are left as they are (no skirt).
pub const SKIRT_MIN: f64 = 0.01;
/// Name of the skirt / end-face mesh that closes the corridor against the ground.
pub const CLOSURE: &str = "Corridor closure (skirts and end faces)";

/// Vertical faces that close the corridor against the existing ground, which keeps its survey
/// elevations: a skirt along every daylight segment where the slope edge (straight between
/// sections) and the ground differ by more than [`SKIRT_MIN`] (terrain between sections, retaining
/// walls, slopes that did not reach the ground),
/// and an end face at the first and last section between the design line and the ground. End faces
/// run 0.3 m past the ground so no sliver of sky shows along the joint.
pub fn closure(eg: &Tin, c: &Corridor) -> (Vec<[f64; 3]>, Vec<u32>) {
    let mut m = (vec![], vec![]);
    let gz = |p: [f64; 3]| eg.elevation_at(p[0], p[1]);
    let past = |d: f64, g: f64| if g >= d { g + 0.3 } else { g - 0.3 };
    for sg in daylight_lines(c) {
        let (a, b) = (sg[0], sg[1]);
        if let (Some(ga), Some(gb)) = (gz(a), gz(b)) {
            // densified so the ground edge of the skirt follows the TIN along the seam; it runs
            // 0.3 m past the design edge (behind the slope) and 0.05 m past the ground edge
            let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            let n = (l / 0.5).ceil().max(1.0) as usize;
            let (mut d, mut g) = (vec![], vec![]);
            let mut need = false;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let p = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), a[2] + t * (b[2] - a[2])];
                let gg = gz(p).unwrap_or(ga + t * (gb - ga));
                need |= (gg - p[2]).abs() > SKIRT_MIN;
                let s = if gg >= p[2] { 1.0 } else { -1.0 };
                d.push([p[0], p[1], p[2] - s * 0.3]);
                g.push([p[0], p[1], gg + s * 0.05]);
            }
            if need {
                merge(&mut m, strip(&d, &g));
            }
        }
    }
    for s in [c.sections.first(), c.sections.last()].into_iter().flatten() {
        let mut top = vec![];
        for (left, sd) in [(false, &s.right), (true, &s.left)] {
            let mut pts: Vec<[f64; 2]> = sd.fg.clone();
            if !left {
                pts.reverse();
            }
            for q in pts {
                let xy = crate::cad3d::sec_xy(s.x, s.y, s.heading, q[0], left);
                top.push([xy[0], xy[1], q[1]]);
            }
        }
        // densify to 0.5 m so the ground line is followed
        let mut dense = vec![];
        for w in top.windows(2) {
            let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            let n = (l / 0.5).ceil().max(1.0) as usize;
            for k in 0..n {
                let t = k as f64 / n as f64;
                dense.push([w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1]), w[0][2] + t * (w[1][2] - w[0][2])]);
            }
        }
        if let Some(l) = top.last() {
            dense.push(*l);
        }
        let bot: Vec<[f64; 3]> = dense.iter().map(|p| [p[0], p[1], gz(*p).map_or(p[2], |g| past(p[2], g))]).collect();
        merge(&mut m, strip(&dense, &bot));
    }
    m
}

/// Name of the ground mesh trimmed to the corridor footprint (daylight lines).
pub const GROUND_TRIMMED: &str = "Existing ground (trimmed to daylight)";
/// Prefix of the pavement layer solids.
pub const LAYER_PREFIX: &str = "Pavement layer ";
const LAYER_SAMPLES: usize = 8;

/// Left and right daylight (catch-point) lines as 3D segments between consecutive sections.
pub fn daylight_lines(c: &Corridor) -> Vec<[[f64; 3]; 2]> {
    let dl = |s: &crate::corridor::Section, left: bool| {
        let sd = if left { &s.left } else { &s.right };
        let q = sd.fg.last().copied().unwrap_or([0.0, s.cl_elev]);
        let p = crate::cad3d::sec_xy(s.x, s.y, s.heading, q[0], left);
        [p[0], p[1], q[1]]
    };
    let mut out = vec![];
    for w in c.sections.windows(2) {
        for left in [true, false] {
            out.push([dl(&w[0], left), dl(&w[1], left)]);
        }
    }
    out
}

/// Corridor footprint in plan between the left and right daylight (catch) points, as triangles
/// (each quad between consecutive sections split along an interior diagonal), counter-clockwise.
pub fn footprint(c: &Corridor) -> Vec<[[f64; 2]; 3]> {
    let dl = |s: &crate::corridor::Section, left: bool| {
        let sd = if left { &s.left } else { &s.right };
        let o = sd.fg.last().map_or(0.0, |q| q[0]);
        crate::cad3d::sec_xy(s.x, s.y, s.heading, o, left)
    };
    let mut out = vec![];
    for w in c.sections.windows(2) {
        let (l0, l1, r0, r1) = (dl(&w[0], true), dl(&w[1], true), dl(&w[0], false), dl(&w[1], false));
        // split the quad l0-l1-r1-r0 along a diagonal that lies inside it (through the reflex vertex
        // of a non-convex quad); a self-intersecting quad falls back to both diagonals
        let q = [l0, l1, r1, r0];
        let cr = |i: usize| {
            let (a, b, c) = (q[(i + 3) % 4], q[i], q[(i + 1) % 4]);
            (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])
        };
        let sg: Vec<f64> = (0..4).map(cr).collect();
        let pos = sg.iter().filter(|v| **v > 0.0).count();
        let tris: Vec<[[f64; 2]; 3]> = if pos == 4 || pos == 0 || pos == 1 || pos == 3 {
            // convex (pos 0/4) or one reflex vertex (pos 1/3): split through the odd one out
            let odd = if pos == 1 {
                sg.iter().position(|v| *v > 0.0)
            } else if pos == 3 {
                sg.iter().position(|v| *v <= 0.0)
            } else {
                Some(0)
            };
            let k = odd.unwrap_or(0);
            vec![[q[k], q[(k + 1) % 4], q[(k + 2) % 4]], [q[k], q[(k + 2) % 4], q[(k + 3) % 4]]]
        } else {
            vec![[l0, l1, r1], [l0, r1, r0], [l0, l1, r0], [l1, r1, r0]]
        };
        for t in tris {
            let a = area2(&t);
            if a.abs() > 1e-9 {
                out.push(if a > 0.0 { t } else { [t[0], t[2], t[1]] });
            }
        }
    }
    out
}

fn area2(p: &[[f64; 2]]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a
}

/// Sutherland-Hodgman clip of a convex polygon to the left (keep_left) or right of a->b.
fn clip(poly: &[[f64; 2]], a: [f64; 2], b: [f64; 2], keep_left: bool) -> Vec<[f64; 2]> {
    let side = |p: [f64; 2]| {
        let c = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        if keep_left {
            c
        } else {
            -c
        }
    };
    let mut out = vec![];
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let t = sp / (sp - sq);
            out.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
        }
    }
    out
}

/// Drops repeated and collinear vertices of a convex polygon (they would give zero-area fan
/// triangles and T-junctions).
fn simplify(p: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let mut q = p;
    loop {
        let n = q.len();
        if n < 3 {
            return q;
        }
        let bad = (0..n).find(|&i| {
            let (a, b, c) = (q[(i + n - 1) % n], q[i], q[(i + 1) % n]);
            let (ux, uy, vx, vy) = (b[0] - a[0], b[1] - a[1], c[0] - b[0], c[1] - b[1]);
            let lu = (ux * ux + uy * uy).sqrt();
            let lv = (vx * vx + vy * vy).sqrt();
            lu < 1e-7 || lv < 1e-7 || (ux * vy - uy * vx).abs() < 1e-9 * lu * lv
        });
        match bad {
            Some(i) => {
                q.remove(i);
            }
            None => return q,
        }
    }
}

/// Convex polygon minus a CCW triangle, as convex pieces.
fn subtract(poly: Vec<[f64; 2]>, t: &[[f64; 2]; 3]) -> Vec<Vec<[f64; 2]>> {
    let mut out = vec![];
    let mut cur = poly;
    for k in 0..3 {
        let (a, b) = (t[k], t[(k + 1) % 3]);
        let o = clip(&cur, a, b, false);
        if o.len() >= 3 && area2(&o).abs() > 2e-6 {
            out.push(o);
        }
        cur = clip(&cur, a, b, true);
        if cur.len() < 3 || area2(&cur).abs() <= 2e-6 {
            return out;
        }
    }
    out // remainder `cur` lies inside the triangle and is dropped
}

/// Existing ground with the corridor footprint cut out: triangles outside are kept as they are,
/// triangles crossing a daylight line are clipped exactly to it (new vertices on the original
/// triangle plane), triangles inside are removed.
/// Every vertex keeps the survey surface elevation; the joint with the slope edge (which is straight
/// between sections) is closed by the skirt in [`closure`]. `seams` is kept for API stability.
pub fn trim_ground(eg: &Tin, foot: &[[[f64; 2]; 3]], seams: &[[[f64; 3]; 2]]) -> (Vec<[f64; 3]>, Vec<u32>) {
    let _ = seams;
    let bb: Vec<[f64; 4]> = foot
        .iter()
        .map(|t| {
            let (mut a, mut b) = ([f64::MAX; 2], [f64::MIN; 2]);
            for p in t {
                for k in 0..2 {
                    a[k] = a[k].min(p[k]);
                    b[k] = b[k].max(p[k]);
                }
            }
            [a[0], a[1], b[0], b[1]]
        })
        .collect();
    let mut pos = eg.pts.clone();
    let mut idx = vec![];
    for tr in &eg.tris {
        let v = [eg.pts[tr[0]], eg.pts[tr[1]], eg.pts[tr[2]]];
        let (mut a, mut b) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &v {
            for k in 0..2 {
                a[k] = a[k].min(p[k]);
                b[k] = b[k].max(p[k]);
            }
        }
        let hits: Vec<usize> = (0..foot.len()).filter(|&i| bb[i][0] <= b[0] && bb[i][2] >= a[0] && bb[i][1] <= b[1] && bb[i][3] >= a[1]).collect();
        if hits.is_empty() {
            idx.extend([tr[0] as u32, tr[1] as u32, tr[2] as u32]);
            continue;
        }
        let ccw = area2(&[[v[0][0], v[0][1]], [v[1][0], v[1][1]], [v[2][0], v[2][1]]]) > 0.0;
        let ord = if ccw { [0, 1, 2] } else { [0, 2, 1] };
        let mut pieces = vec![ord.iter().map(|&k| [v[k][0], v[k][1]]).collect::<Vec<_>>()];
        for &h in &hits {
            pieces = pieces.into_iter().flat_map(|p| subtract(p, &foot[h])).collect();
            if pieces.is_empty() {
                break;
            }
        }
        if pieces.len() == 1 && pieces[0].len() == 3 && pieces[0].iter().zip(ord).all(|(p, k)| p[0] == v[k][0] && p[1] == v[k][1]) {
            idx.extend([tr[0] as u32, tr[1] as u32, tr[2] as u32]);
            continue;
        }
        // plane through the original triangle
        let (e1, e2) = ([v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]], [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]]);
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let z = |p: [f64; 2]| if n[2].abs() < 1e-12 { v[0][2] } else { v[0][2] - (n[0] * (p[0] - v[0][0]) + n[1] * (p[1] - v[0][1])) / n[2] };
        for p in pieces {
            let p = simplify(p);
            if p.len() < 3 {
                continue;
            }
            let b0 = pos.len() as u32;
            for q in &p {
                pos.push([q[0], q[1], z(*q)]);
            }
            for k in 1..p.len() as u32 - 1 {
                idx.extend([b0, b0 + k, b0 + k + 1]);
            }
        }
    }
    weld_t_junctions(pos, idx)
}

/// Welds coincident vertices and removes T-junctions (a vertex lying inside another triangle's
/// edge), which otherwise render as hairline cracks: every triangle with vertices on its edges is
/// re-triangulated as a fan around its centroid.
fn weld_t_junctions(pos: Vec<[f64; 3]>, idx: Vec<u32>) -> (Vec<[f64; 3]>, Vec<u32>) {
    use std::collections::HashMap;
    let key = |p: &[f64; 3]| ((p[0] * 1e5).round() as i64, (p[1] * 1e5).round() as i64);
    let mut map: HashMap<(i64, i64), u32> = HashMap::new();
    let mut out: Vec<[f64; 3]> = vec![];
    let mut remap = vec![0u32; pos.len()];
    for (i, p) in pos.iter().enumerate() {
        remap[i] = *map.entry(key(p)).or_insert_with(|| {
            out.push(*p);
            (out.len() - 1) as u32
        });
    }
    let used: std::collections::HashSet<u32> = idx.iter().map(|&i| remap[i as usize]).collect();
    const CELL: f64 = 5.0;
    let cell = |x: f64, y: f64| ((x / CELL).floor() as i64, (y / CELL).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
    for &v in &used {
        let p = out[v as usize];
        grid.entry(cell(p[0], p[1])).or_default().push(v);
    }
    let mut res = vec![];
    for t in idx.chunks(3) {
        let tv = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
        if tv[0] == tv[1] || tv[1] == tv[2] || tv[0] == tv[2] {
            continue;
        }
        {
            let (a, b, c) = (out[tv[0] as usize], out[tv[1] as usize], out[tv[2] as usize]);
            if ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() < 1e-10 {
                continue; // zero-area
            }
        }
        let mut ring: Vec<u32> = vec![];
        let mut split = false;
        for e in 0..3 {
            let (a, b) = (out[tv[e] as usize], out[tv[(e + 1) % 3] as usize]);
            ring.push(tv[e]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dy * dy;
            let (c0, c1) = (cell(a[0].min(b[0]), a[1].min(b[1])), cell(a[0].max(b[0]), a[1].max(b[1])));
            let mut on: Vec<(f64, u32)> = vec![];
            for cx in c0.0..=c1.0 {
                for cy in c0.1..=c1.1 {
                    for &v in grid.get(&(cx, cy)).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if tv.contains(&v) {
                            continue;
                        }
                        let p = out[v as usize];
                        let tt = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2;
                        if tt <= 1e-9 || tt >= 1.0 - 1e-9 {
                            continue;
                        }
                        let d = ((a[0] + tt * dx - p[0]).powi(2) + (a[1] + tt * dy - p[1]).powi(2)).sqrt();
                        if d < 1e-5 {
                            on.push((tt, v));
                        }
                    }
                }
            }
            on.sort_by(|x, y| x.0.total_cmp(&y.0));
            split |= !on.is_empty();
            ring.extend(on.into_iter().map(|x| x.1));
        }
        if !split {
            res.extend(tv);
            continue;
        }
        let c = tv.iter().fold([0.0; 3], |a, &v| {
            let p = out[v as usize];
            [a[0] + p[0] / 3.0, a[1] + p[1] / 3.0, a[2] + p[2] / 3.0]
        });
        out.push(c);
        let ci = (out.len() - 1) as u32;
        for k in 0..ring.len() {
            res.extend([ci, ring[k], ring[(k + 1) % ring.len()]]);
        }
    }
    (out, res)
}

/// Pavement structure as closed solids, one per assembly layer, from the left to the right paved
/// edge. Each layer's top follows the finished surface (crossfall / superelevation) lowered by the
/// thickness of the layers above it; offsets are vertical, as in the typical section.
pub fn layer_solids(c: &Corridor) -> Vec<(String, Vec<[f64; 3]>, Vec<u32>)> {
    let fz = |fg: &[[f64; 2]], o: f64| {
        for w in fg.windows(2) {
            if o <= w[1][0] + 1e-9 {
                let t = if w[1][0] > w[0][0] { ((o - w[0][0]) / (w[1][0] - w[0][0])).clamp(0.0, 1.0) } else { 1.0 };
                return w[0][1] + t * (w[1][1] - w[0][1]);
            }
        }
        fg.last().map_or(0.0, |q| q[1])
    };
    // cross-section top points, left edge -> CL -> right edge
    let tops: Vec<Vec<[f64; 3]>> = c
        .sections
        .iter()
        .map(|s| {
            let mut row = vec![];
            for k in (1..=LAYER_SAMPLES).rev() {
                let o = s.left.paved_width * k as f64 / LAYER_SAMPLES as f64;
                let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, o, true);
                row.push([q[0], q[1], fz(&s.left.fg, o)]);
            }
            row.push([s.x, s.y, s.cl_elev]);
            for k in 1..=LAYER_SAMPLES {
                let o = s.right.paved_width * k as f64 / LAYER_SAMPLES as f64;
                let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, o, false);
                row.push([q[0], q[1], fz(&s.right.fg, o)]);
            }
            row
        })
        .collect();
    let mut out = vec![];
    let mut depth = 0.0;
    for (li, l) in c.assembly.pavement.iter().enumerate() {
        let (d0, d1) = (depth, depth + l.thickness_m);
        depth = d1;
        let lower = |r: &Vec<[f64; 3]>, d: f64| r.iter().map(|p| [p[0], p[1], p[2] - d]).collect::<Vec<_>>();
        let top: Vec<Vec<[f64; 3]>> = tops.iter().map(|r| lower(r, d0)).collect();
        let bot: Vec<Vec<[f64; 3]>> = tops.iter().map(|r| lower(r, d1)).collect();
        let mut m = (vec![], vec![]);
        let col = |g: &Vec<Vec<[f64; 3]>>, j: usize| g.iter().map(|r| r[j]).collect::<Vec<_>>();
        let last = tops.first().map_or(0, |r| r.len() - 1);
        for j in 0..last {
            merge(&mut m, strip(&col(&top, j), &col(&top, j + 1)));
            merge(&mut m, strip(&col(&bot, j + 1), &col(&bot, j)));
        }
        merge(&mut m, strip(&col(&bot, 0), &col(&top, 0)));
        merge(&mut m, strip(&col(&top, last), &col(&bot, last)));
        if let (Some(t0), Some(b0), Some(t1), Some(b1)) = (top.first(), bot.first(), top.last(), bot.last()) {
            merge(&mut m, strip(t0, b0));
            merge(&mut m, strip(b1, t1));
        }
        out.push((format!("{LAYER_PREFIX}{}: {} {:.0} mm", li + 1, l.name, l.thickness_m * 1000.0), m.0, m.1));
    }
    out
}

pub struct Scene {
    pub origin: [f64; 3],
    pub meshes: Vec<Mesh>,
}

impl Scene {
    fn add(&mut self, name: &str, material: usize, pos: Vec<[f64; 3]>, idx: Vec<u32>) {
        if !idx.is_empty() {
            self.meshes.push(Mesh { name: name.into(), material, pos, idx });
        }
    }
}

/// Quad strip between two polylines of equal length.
fn strip(a: &[[f64; 3]], b: &[[f64; 3]]) -> (Vec<[f64; 3]>, Vec<u32>) {
    let n = a.len().min(b.len());
    let mut pos = Vec::with_capacity(2 * n);
    let mut idx = vec![];
    for i in 0..n {
        pos.push(a[i]);
        pos.push(b[i]);
        if i + 1 < n {
            let k = (2 * i) as u32;
            idx.extend([k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    (pos, idx)
}

fn merge(dst: &mut (Vec<[f64; 3]>, Vec<u32>), src: (Vec<[f64; 3]>, Vec<u32>)) {
    let o = dst.0.len() as u32;
    dst.0.extend(src.0);
    dst.1.extend(src.1.iter().map(|i| i + o));
}

/// Builds the scene. `alts` are (name, plan polyline) route alternatives.
pub fn build(eg: &Tin, cor: Option<&Corridor>, nets: &[Network], alts: &[(String, Vec<[f64; 2]>)]) -> Scene {
    let (mut x0, mut y0, mut z0) = (0.0, 0.0, f64::MAX);
    for p in &eg.pts {
        x0 += p[0];
        y0 += p[1];
        z0 = z0.min(p[2]);
    }
    let n = eg.pts.len().max(1) as f64;
    let origin = [(x0 / n).round(), (y0 / n).round(), z0.floor()];
    let mut sc = Scene { origin, meshes: vec![] };
    sc.add("Existing ground", 0, eg.pts.clone(), eg.tris.iter().flat_map(|t| [t[0] as u32, t[1] as u32, t[2] as u32]).collect());
    if let Some(c) = cor {
        let at = |s: &crate::corridor::Section, left: bool, o: f64, z: f64| {
            let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, o, left);
            [q[0], q[1], z]
        };
        let mut pav = (vec![], vec![]);
        let mut earth = (vec![], vec![]);
        for left in [true, false] {
            fn pick(s: &crate::corridor::Section, left: bool) -> &crate::corridor::SideSection {
                if left {
                    &s.left
                } else {
                    &s.right
                }
            }
            let side = |s| pick(s, left);
            // pavement: centreline to the outermost paved fg vertex
            let cl: Vec<[f64; 3]> = c.sections.iter().map(|s| [s.x, s.y, s.cl_elev]).collect();
            let edge: Vec<[f64; 3]> = c
                .sections
                .iter()
                .map(|s| {
                    let sd = side(s);
                    let k = sd.fg.iter().position(|q| q[0] >= sd.paved_width - 1e-6).unwrap_or(sd.fg.len() - 1);
                    at(s, left, sd.fg[k][0], sd.fg[k][1])
                })
                .collect();
            merge(&mut pav, strip(&cl, &edge));
            // remaining fg vertices to the daylight point, one strip per vertex rank (resampled)
            const RANKS: usize = 6;
            let rows: Vec<Vec<[f64; 3]>> = (0..=RANKS)
                .map(|r| {
                    c.sections
                        .iter()
                        .map(|s| {
                            let sd = side(s);
                            let k0 = sd.fg.iter().position(|q| q[0] >= sd.paved_width - 1e-6).unwrap_or(sd.fg.len() - 1);
                            let k = k0 + (sd.fg.len() - 1 - k0) * r / RANKS;
                            at(s, left, sd.fg[k][0], sd.fg[k][1])
                        })
                        .collect()
                })
                .collect();
            for w in rows.windows(2) {
                merge(&mut earth, strip(&w[0], &w[1]));
            }
        }
        sc.add("Road pavement", 1, pav.0, pav.1);
        sc.add("Earthwork slopes", 2, earth.0, earth.1);
        // walls: consecutive sections with a wall on the same side
        let mut walls = (vec![], vec![]);
        for left in [true, false] {
            let mut run_top = vec![];
            let mut run_bot = vec![];
            for s in &c.sections {
                let w = if left { &s.left.wall } else { &s.right.wall };
                match w {
                    Some(w) => {
                        run_top.push(at(s, left, w.offset, w.top_z));
                        run_bot.push(at(s, left, w.offset, w.bottom_z));
                    }
                    None => {
                        if run_top.len() >= 2 {
                            merge(&mut walls, strip(&run_bot, &run_top));
                        }
                        run_top.clear();
                        run_bot.clear();
                    }
                }
            }
            if run_top.len() >= 2 {
                merge(&mut walls, strip(&run_bot, &run_top));
            }
        }
        sc.add("Retaining walls", 3, walls.0, walls.1);
        for (li, (name, pos, idx)) in layer_solids(c).into_iter().enumerate() {
            sc.add(&name, 10 + li % 3, pos, idx);
        }
        let (pos, idx) = trim_ground(eg, &footprint(c), &daylight_lines(c));
        sc.add(GROUND_TRIMMED, 0, pos, idx);
        let (pos, idx) = closure(eg, c);
        sc.add(CLOSURE, 2, pos, idx);
    }
    // drainage: 8-sided tubes and structure cylinders
    let tube = |a: [f64; 3], b: [f64; 3], r: f64| -> (Vec<[f64; 3]>, Vec<u32>) {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
        let t = [d[0] / l, d[1] / l, d[2] / l];
        let up = if t[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
        let u = [t[1] * up[2] - t[2] * up[1], t[2] * up[0] - t[0] * up[2], t[0] * up[1] - t[1] * up[0]];
        let ul = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        let u = [u[0] / ul, u[1] / ul, u[2] / ul];
        let v = [t[1] * u[2] - t[2] * u[1], t[2] * u[0] - t[0] * u[2], t[0] * u[1] - t[1] * u[0]];
        let mut pos = vec![];
        let mut idx = vec![];
        for (e, c) in [a, b].iter().enumerate() {
            let _ = e;
            for k in 0..8 {
                let ang = k as f64 * std::f64::consts::TAU / 8.0;
                let (cs, sn) = (ang.cos() * r, ang.sin() * r);
                pos.push([c[0] + u[0] * cs + v[0] * sn, c[1] + u[1] * cs + v[1] * sn, c[2] + u[2] * cs + v[2] * sn]);
            }
        }
        for k in 0..8u32 {
            let k1 = (k + 1) % 8;
            idx.extend([k, k1, 8 + k, k1, 8 + k1, 8 + k]);
        }
        (pos, idx)
    };
    let mut pipes = (vec![], vec![]);
    let mut structs = (vec![], vec![]);
    for n in nets {
        let st = |id: &str| n.structures.iter().find(|s| s.0 == id);
        for p in &n.pipes {
            if let (Some(a), Some(b)) = (st(&p.from), st(&p.to)) {
                let r = p.diameter_m / 2.0;
                merge(&mut pipes, tube([a.1, a.2, p.invert_up + r], [b.1, b.2, p.invert_dn + r], r));
            }
        }
        for s in &n.structures {
            merge(&mut structs, tube([s.1, s.2, s.4], [s.1, s.2, s.3], 0.6));
        }
    }
    sc.add("Pipes", 4, pipes.0, pipes.1);
    sc.add("Structures", 5, structs.0, structs.1);
    // route alternatives: 4 m ribbons draped 0.3 m above ground
    for (i, (name, path)) in alts.iter().enumerate() {
        let mut l = vec![];
        let mut r = vec![];
        let dense: Vec<[f64; 2]> = path
            .windows(2)
            .flat_map(|w| {
                let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
                let n = (d / 5.0).ceil().max(1.0) as usize;
                (0..n).map(move |k| {
                    let t = k as f64 / n as f64;
                    [w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1])]
                })
            })
            .chain(path.last().copied())
            .collect();
        for (k, p) in dense.iter().enumerate() {
            let q = dense[(k + 1).min(dense.len() - 1)];
            let pr = dense[k.saturating_sub(1)];
            let h = (q[1] - pr[1]).atan2(q[0] - pr[0]);
            let (nx, ny) = (-h.sin() * 2.0, h.cos() * 2.0);
            let z = |x: f64, y: f64| eg.elevation_at(x, y).unwrap_or(origin[2]) + 0.3;
            l.push([p[0] + nx, p[1] + ny, z(p[0] + nx, p[1] + ny)]);
            r.push([p[0] - nx, p[1] - ny, z(p[0] - nx, p[1] - ny)]);
        }
        let (pos, idx) = strip(&l, &r);
        sc.add(&format!("Alternative {name}"), 6 + i % 4, pos, idx);
    }
    sc
}

/// Binary glTF 2.0.
pub fn to_glb(sc: &Scene, extras: Value) -> Vec<u8> {
    let o = sc.origin;
    let mut bin: Vec<u8> = vec![];
    let (mut views, mut accs, mut meshes, mut nodes) = (vec![], vec![], vec![], vec![]);
    for m in &sc.meshes {
        let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
        let off = bin.len();
        for p in &m.pos {
            let v = [(p[0] - o[0]) as f32, (p[2] - o[2]) as f32, -(p[1] - o[1]) as f32];
            for k in 0..3 {
                mn[k] = mn[k].min(v[k]);
                mx[k] = mx[k].max(v[k]);
                bin.extend_from_slice(&v[k].to_le_bytes());
            }
        }
        views.push(json!({"buffer": 0, "byteOffset": off, "byteLength": bin.len() - off, "target": 34962}));
        accs.push(json!({"bufferView": views.len() - 1, "componentType": 5126, "count": m.pos.len(), "type": "VEC3", "min": mn, "max": mx}));
        let pa = accs.len() - 1;
        let off = bin.len();
        for i in &m.idx {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        views.push(json!({"buffer": 0, "byteOffset": off, "byteLength": bin.len() - off, "target": 34963}));
        accs.push(json!({"bufferView": views.len() - 1, "componentType": 5125, "count": m.idx.len(), "type": "SCALAR"}));
        meshes.push(json!({"name": m.name, "primitives": [{"attributes": {"POSITION": pa}, "indices": accs.len() - 1, "material": m.material}]}));
        nodes.push(json!({"name": m.name, "mesh": meshes.len() - 1}));
    }
    let mats: Vec<Value> = MATERIALS
        .iter()
        .map(|(n, c, r)| json!({"name": n, "doubleSided": true, "pbrMetallicRoughness": {"baseColorFactor": c, "metallicFactor": 0.0, "roughnessFactor": r}}))
        .collect();
    let doc = json!({
        "asset": {"version": "2.0", "generator": concat!("civil-mcp ", env!("CARGO_PKG_VERSION"))},
        "scene": 0,
        "scenes": [{"name": "civil-mcp", "nodes": (0..nodes.len()).collect::<Vec<_>>(), "extras": extras}],
        "nodes": nodes, "meshes": meshes, "materials": mats, "accessors": accs, "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}]
    });
    let mut js = doc.to_string().into_bytes();
    while js.len() & 3 != 0 {
        js.push(b' ');
    }
    while bin.len() & 3 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::align::{Alignment, AlignmentInput};
    use crate::corridor::{build as cbuild, Assembly, Layer};
    use crate::profile::{Profile, ProfileInput};

    fn tilted() -> Tin {
        let mut pts = vec![];
        for i in -30..=30 {
            for j in -10..=60 {
                let (x, y) = (i as f64 * 10.0 + (j % 2) as f64 * 0.01, j as f64 * 10.0);
                pts.push([x, y, 100.0 + 0.05 * x]);
            }
        }
        Tin::build(&pts, None).unwrap().0
    }
    fn corridor() -> Corridor {
        let al = Alignment::build(AlignmentInput {
            name: "a".into(),
            pis: vec![[0.0, 0.0], [0.0, 250.0], [120.0, 450.0]],
            radii: vec![150.0],
            spirals: vec![],
            start_station: 0.0,
        })
        .unwrap();
        let pr = Profile::build(ProfileInput { name: "p".into(), vpis: vec![[0.0, 101.0], [500.0, 101.0]], curve_lengths: vec![] }).unwrap();
        let mut a: Assembly = serde_json::from_value(
            serde_json::json!({"name":"t","lanes_m":[3.5],"shoulder_m":1.0,"shoulder_slope":-0.04,"shoulder_paved":false,"cut_slope_h":1.5,"fill_slope_h":2.0}),
        )
        .unwrap();
        a.pavement = vec![
            Layer { name: "AC".into(), thickness_m: 0.11 },
            Layer { name: "Base".into(), thickness_m: 0.15 },
            Layer { name: "Subbase".into(), thickness_m: 0.15 },
        ];
        cbuild(&tilted(), &al, &pr, None, &a, 10.0).unwrap()
    }
    fn inside(t: &[[f64; 2]; 3], p: [f64; 2], eps: f64) -> bool {
        (0..3).all(|k| {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) / l > eps
        })
    }

    #[test]
    fn trimmed_ground_has_no_area_inside_footprint_and_keeps_the_rest() {
        let (eg, c) = (tilted(), corridor());
        let foot = footprint(&c);
        let (pos, idx) = trim_ground(&eg, &foot, &daylight_lines(&c));
        let a3 = |t: &[[f64; 3]]| ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs() / 2.0;
        let mut kept = 0.0;
        for t in idx.chunks(3) {
            let v = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
            kept += a3(&v);
            // centroid of every kept triangle lies outside every footprint triangle (5 mm tolerance)
            let g = [(v[0][0] + v[1][0] + v[2][0]) / 3.0, (v[0][1] + v[1][1] + v[2][1]) / 3.0];
            assert!(!foot.iter().any(|f| inside(f, g, 0.005)), "ground triangle inside footprint at {g:?}");
            for p in v {
                assert!((p[2] - (100.0 + 0.05 * p[0])).abs() < 1e-9);
            }
        }
        let full: f64 = eg.tris.iter().map(|t| a3(&[eg.pts[t[0]], eg.pts[t[1]], eg.pts[t[2]]])).sum();
        // removed area = footprint area (quads split without overlap on this gentle curve)
        let fa: f64 = foot.iter().map(|t| area2(t) / 2.0).sum::<f64>();
        assert!(((full - kept) - fa).abs() / fa < 1e-3, "removed {} vs footprint {}", full - kept, fa);
    }

    #[test]
    fn daylight_points_lie_on_the_trimmed_ground_boundary() {
        let (eg, c) = (tilted(), corridor());
        let (pos, _) = trim_ground(&eg, &footprint(&c), &daylight_lines(&c));
        for s in c.sections.iter().skip(1).step_by(5) {
            for left in [true, false] {
                let sd = if left { &s.left } else { &s.right };
                let d = sd.fg.last().unwrap();
                let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, d[0], left);
                let near = pos.iter().map(|p| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()).fold(f64::MAX, f64::min);
                assert!(near < 0.01, "no ground vertex at daylight point ({near} m)");
                assert!((d[1] - (100.0 + 0.05 * q[0])).abs() < 0.02, "catch point off ground");
            }
        }
    }

    #[test]
    fn trimmed_ground_has_no_holes_outside_the_footprint() {
        // T-junction repair + clipping leave no gap: every sample point of the original ground that
        // is more than 1 cm outside the footprint is covered by a kept triangle
        let (eg, c) = (tilted(), corridor());
        let foot = footprint(&c);
        let (pos, idx) = trim_ground(&eg, &foot, &daylight_lines(&c));
        let tris: Vec<[[f64; 2]; 3]> = idx
            .chunks(3)
            .map(|t| {
                let p = |k: usize| [pos[t[k] as usize][0], pos[t[k] as usize][1]];
                let tr = [p(0), p(1), p(2)];
                if area2(&tr) > 0.0 {
                    tr
                } else {
                    [tr[0], tr[2], tr[1]]
                }
            })
            .collect();
        let (mut n, mut checked) = (0u64, 0);
        for k in 0..6000u64 {
            n = n.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407 + k);
            let (u, v) = ((n >> 11) as f64 / (1u64 << 53) as f64, ((n >> 3) % 1_000_003) as f64 / 1_000_003.0);
            let q = [-290.0 + 580.0 * u, -90.0 + 680.0 * v];
            if foot.iter().any(|f| inside(f, q, -0.01)) {
                continue;
            }
            checked += 1;
            assert!(tris.iter().any(|t| inside(t, q, -1e-7)), "hole in trimmed ground at {q:?}");
        }
        assert!(checked > 4000);
        // and no T-junctions (a vertex strictly inside another triangle's edge renders as a crack)
        let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
        let used: std::collections::BTreeSet<usize> = idx.iter().map(|&i| i as usize).collect();
        for &i in &used {
            grid.entry(((pos[i][0] / 5.0).floor() as i64, (pos[i][1] / 5.0).floor() as i64)).or_default().push(i);
        }
        for t in idx.chunks(3) {
            for e in 0..3 {
                let (ia, ib) = (t[e] as usize, t[(e + 1) % 3] as usize);
                let (a, b) = (pos[ia], pos[ib]);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l2 = dx * dx + dy * dy;
                for cx in (a[0].min(b[0]) / 5.0).floor() as i64..=(a[0].max(b[0]) / 5.0).floor() as i64 {
                    for cy in (a[1].min(b[1]) / 5.0).floor() as i64..=(a[1].max(b[1]) / 5.0).floor() as i64 {
                        for &v in grid.get(&(cx, cy)).map(|v| v.as_slice()).unwrap_or(&[]) {
                            if v == ia || v == ib {
                                continue;
                            }
                            let p = pos[v];
                            let tt = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2;
                            let d = ((a[0] + tt * dx - p[0]).powi(2) + (a[1] + tt * dy - p[1]).powi(2)).sqrt();
                            assert!(!(tt > 1e-6 && tt < 1.0 - 1e-6 && d < 1e-6), "T-junction at {p:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ground_moves_at_most_the_snap_limit_and_ends_are_closed() {
        let (eg, c) = (tilted(), corridor());
        let (pos, _) = trim_ground(&eg, &footprint(&c), &daylight_lines(&c));
        let dev = pos.iter().map(|p| (p[2] - (100.0 + 0.05 * p[0])).abs()).fold(0.0, f64::max);
        assert!(dev <= 1e-9, "ground moved {dev} m");
        let (cp, ci) = closure(&eg, &c);
        assert!(!ci.is_empty() && ci.len() % 3 == 0);
        // end faces reach the ground at the first section's centreline (fill here: CL 101, ground 100)
        let s0 = &c.sections[0];
        assert!(cp.iter().any(|p| (p[0] - s0.x).abs() < 1e-6 && (p[1] - s0.y).abs() < 1e-6 && p[2] < 100.0 + 0.05 * s0.x - 0.29));
    }

    #[test]
    fn pavement_layers_follow_finished_surface_with_assembly_thicknesses() {
        let c = corridor();
        let ls = layer_solids(&c);
        assert_eq!(ls.len(), 3);
        assert!(ls[0].0.contains("AC 110 mm") && ls[2].0.contains("Subbase 150 mm"));
        let s = &c.sections[7];
        let cum = [0.0, 0.11, 0.26, 0.41];
        for (k, (_, pos, idx)) in ls.iter().enumerate() {
            assert!(!idx.is_empty() && idx.len() % 3 == 0);
            // the top / bottom of the layer at this section's centreline
            let at_cl: Vec<f64> = pos.iter().filter(|p| (p[0] - s.x).abs() < 1e-6 && (p[1] - s.y).abs() < 1e-6).map(|p| p[2]).collect();
            let (lo, hi) = at_cl.iter().fold((f64::MAX, f64::MIN), |a, &z| (a.0.min(z), a.1.max(z)));
            assert!((hi - (s.cl_elev - cum[k])).abs() < 1e-9 && (lo - (s.cl_elev - cum[k + 1])).abs() < 1e-9, "layer {k}");
            // closed: every edge is shared by exactly two triangles
            let mut e = std::collections::HashMap::new();
            let key = |p: [f64; 3]| [(p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64];
            for t in idx.chunks(3) {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let (ka, kb) = (key(pos[a as usize]), key(pos[b as usize]));
                    if ka == kb {
                        continue;
                    }
                    *e.entry(if ka < kb { (ka, kb) } else { (kb, ka) }).or_insert(0) += 1;
                }
            }
            assert!(e.values().all(|&n| n == 2), "layer {k} not closed");
        }
        // crossfall carried: layer top at the left edge = finished grade at that offset minus depth
        let (_, pos, _) = &ls[1];
        let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, s.left.paved_width, true);
        let fgz = s.left.fg.iter().find(|p| (p[0] - s.left.paved_width).abs() < 1e-6).map(|p| p[1]).unwrap();
        assert!(pos.iter().any(|p| (p[0] - q[0]).abs() < 1e-6 && (p[1] - q[1]).abs() < 1e-6 && (p[2] - (fgz - 0.11)).abs() < 1e-9));
    }
}
