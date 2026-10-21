//! Route / corridor selection: least-cost paths over the terrain with grade and
//! curvature constraints, simplified to PIs for a horizontal alignment.
//!
//! The search runs on a regular grid sampled from the TIN. A node is (cell, heading),
//! with 16 headings. A move may change the heading by at most one step (22.5°), which
//! bounds curvature during the search. Edge cost per metre is:
//!   1 + w_grade·(100·max(0, |g| − g_max))² + w_terrain·100·|g| + avoidance
//! plus w_turn per heading step. Here g is the ground grade along the move. The weights
//! are agent choices recorded with each alternative; they are not standards values.
use crate::tin::Tin;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AltSpec {
    pub name: String,
    pub w_grade: f64,
    pub w_terrain: f64,
    pub w_turn: f64,
    /// penalise cells within this distance (m) of previously found paths (0 = off)
    #[serde(default)]
    pub avoid_previous_m: f64,
    /// forbid moves whose ground grade exceeds 2 × the design maximum (switchback / hairpin
    /// search; the factor 2 is an agent choice so that hairpin turns can pass through steeper
    /// headings briefly, with cut/fill absorbing the excess)
    #[serde(default)]
    pub hard_grade: bool,
    /// weight on the imported parcels' ROW cost surface (land rate / median rate per metre of
    /// path; no-go parcels impassable). 0 = off; only used when parcels are imported
    #[serde(default)]
    pub w_row: f64,
}

pub fn default_alts() -> Vec<AltSpec> {
    vec![
        AltSpec { name: "A-direct".into(), w_grade: 1.0, w_terrain: 0.0, w_turn: 2.0, avoid_previous_m: 0.0, hard_grade: false, w_row: 0.0 },
        AltSpec { name: "B-contour-following".into(), w_grade: 10.0, w_terrain: 0.3, w_turn: 2.0, avoid_previous_m: 0.0, hard_grade: false, w_row: 0.0 },
        AltSpec { name: "C-separate-corridor".into(), w_grade: 4.0, w_terrain: 0.1, w_turn: 2.0, avoid_previous_m: 80.0, hard_grade: false, w_row: 0.0 },
        AltSpec { name: "D-switchback".into(), w_grade: 200.0, w_terrain: 0.0, w_turn: 0.5, avoid_previous_m: 0.0, hard_grade: true, w_row: 0.0 },
    ]
}

#[derive(Debug, Clone, Serialize)]
pub struct PathResult {
    pub name: String,
    pub spec: AltSpec,
    pub path: Vec<[f64; 2]>,
    pub pis: Vec<[f64; 2]>,
    pub search_cost: f64,
    pub path_length_m: f64,
    pub max_ground_grade_pct: f64,
    pub nodes_expanded: usize,
}

const DIRS: [(i32, i32); 16] =
    [(1, 0), (2, 1), (1, 1), (1, 2), (0, 1), (-1, 2), (-1, 1), (-2, 1), (-1, 0), (-2, -1), (-1, -1), (-1, -2), (0, -1), (1, -2), (1, -1), (2, -1)];

#[derive(PartialEq)]
struct Node(f64, usize);
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}

pub struct Grid {
    pub x0: f64,
    pub y0: f64,
    pub cell: f64,
    pub nx: usize,
    pub ny: usize,
    pub z: Vec<Option<f64>>,
}

impl Grid {
    pub fn from_tin(t: &Tin, cell: f64, bbox: [f64; 4]) -> Grid {
        let nx = ((bbox[2] - bbox[0]) / cell).floor() as usize + 1;
        let ny = ((bbox[3] - bbox[1]) / cell).floor() as usize + 1;
        let mut z = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                z.push(t.elevation_at(bbox[0] + i as f64 * cell, bbox[1] + j as f64 * cell));
            }
        }
        Grid { x0: bbox[0], y0: bbox[1], cell, nx, ny, z }
    }
    fn xy(&self, i: usize, j: usize) -> [f64; 2] {
        [self.x0 + i as f64 * self.cell, self.y0 + j as f64 * self.cell]
    }
    fn ij(&self, p: [f64; 2]) -> (usize, usize) {
        (
            ((p[0] - self.x0) / self.cell).round().clamp(0.0, (self.nx - 1) as f64) as usize,
            ((p[1] - self.y0) / self.cell).round().clamp(0.0, (self.ny - 1) as f64) as usize,
        )
    }
}

fn dist_to_poly(p: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    let mut d = f64::INFINITY;
    for w in poly.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = vx * vx + vy * vy;
        let t = if l2 > 0.0 { (((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        d = d.min(((a[0] + t * vx - p[0]).powi(2) + (a[1] + t * vy - p[1]).powi(2)).sqrt());
    }
    d
}

/// Least-cost path from start to end (heading-constrained Dijkstra).
pub fn least_cost(
    g: &Grid,
    start: [f64; 2],
    end: [f64; 2],
    gmax_pct: f64,
    spec: &AltSpec,
    avoid: &[Vec<[f64; 2]>],
) -> crate::Result<(Vec<[f64; 2]>, f64, usize)> {
    least_cost_row(g, start, end, gmax_pct, spec, avoid, None)
}

/// [`least_cost`] with an optional ROW cost surface per grid cell (normalised land rate, NaN =
/// no-go). It is added to the per-metre cost with weight `spec.w_row`; NaN cells are impassable.
pub fn least_cost_row(
    g: &Grid,
    start: [f64; 2],
    end: [f64; 2],
    gmax_pct: f64,
    spec: &AltSpec,
    avoid: &[Vec<[f64; 2]>],
    row: Option<&[f64]>,
) -> crate::Result<(Vec<[f64; 2]>, f64, usize)> {
    let (si, sj) = g.ij(start);
    let (ei, ej) = g.ij(end);
    let n = g.nx * g.ny * 16;
    let mut dist = vec![f64::INFINITY; n];
    let mut prev = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    let id = |i: usize, j: usize, d: usize| (j * g.nx + i) * 16 + d;
    let mut pen = vec![0.0; g.nx * g.ny];
    if spec.avoid_previous_m > 0.0 {
        for j in 0..g.ny {
            for i in 0..g.nx {
                let p = g.xy(i, j);
                if avoid.iter().any(|a| dist_to_poly(p, a) < spec.avoid_previous_m) {
                    pen[j * g.nx + i] = 3.0;
                }
            }
        }
    }
    let mut blocked = vec![false; g.nx * g.ny];
    if let Some(rc) = row.filter(|_| spec.w_row > 0.0) {
        for k in 0..pen.len() {
            if rc[k].is_nan() {
                blocked[k] = true;
            } else {
                pen[k] += spec.w_row * rc[k];
            }
        }
    }
    for d in 0..16 {
        dist[id(si, sj, d)] = 0.0;
        heap.push(Node(0.0, id(si, sj, d)));
    }
    let gm = gmax_pct / 100.0;
    let mut expanded = 0;
    let mut goal = None;
    while let Some(Node(c, u)) = heap.pop() {
        if c > dist[u] {
            continue;
        }
        expanded += 1;
        let d = u % 16;
        let cellid = u / 16;
        let (i, j) = (cellid % g.nx, cellid / g.nx);
        if i == ei && j == ej {
            goal = Some(u);
            break;
        }
        let z0 = match g.z[cellid] {
            Some(z) => z,
            None => continue,
        };
        for dd in [-1i32, 0, 1] {
            let nd = ((d as i32 + dd).rem_euclid(16)) as usize;
            let (di, dj) = DIRS[nd];
            let (ni, nj) = (i as i32 + di, j as i32 + dj);
            if ni < 0 || nj < 0 || ni >= g.nx as i32 || nj >= g.ny as i32 {
                continue;
            }
            let (ni, nj) = (ni as usize, nj as usize);
            let Some(z1) = g.z[nj * g.nx + ni] else { continue };
            if blocked[nj * g.nx + ni] {
                continue;
            }
            let l = ((di * di + dj * dj) as f64).sqrt() * g.cell;
            // knight moves cross two cells: the midpoint elevation is the mean of those two,
            // and the steeper half governs (otherwise a one-cell ridge could be jumped)
            let gr = if di.abs() == 2 || dj.abs() == 2 {
                let (ai, aj) = (i as i32 + di / 2, j as i32 + dj / 2);
                let (bi, bj) = (i as i32 + (di + di.signum()) / 2, j as i32 + (dj + dj.signum()) / 2);
                let at =
                    |x: i32, y: i32| if x >= 0 && y >= 0 && (x as usize) < g.nx && (y as usize) < g.ny { g.z[y as usize * g.nx + x as usize] } else { None };
                match (at(ai, aj), at(bi, bj)) {
                    (Some(za), Some(zb)) => {
                        let zm = (za + zb) / 2.0;
                        ((zm - z0).abs()).max((z1 - zm).abs()) / (l / 2.0)
                    }
                    _ => continue,
                }
            } else {
                (z1 - z0).abs() / l
            };
            if spec.hard_grade && gr > 2.0 * gm {
                continue;
            }
            let ex = (gr - gm).max(0.0) * 100.0;
            let per_m = 1.0 + spec.w_grade * ex * ex + spec.w_terrain * 100.0 * gr + pen[nj * g.nx + ni];
            let cost = c + per_m * l + spec.w_turn * (dd.abs() as f64) * g.cell;
            let v = id(ni, nj, nd);
            if cost < dist[v] {
                dist[v] = cost;
                prev[v] = u;
                heap.push(Node(cost, v));
            }
        }
    }
    let goal = goal.ok_or(if spec.hard_grade {
        format!(
            "no path between start and end inside the surface keeps the ground grade within {:.1} % (2 × the {gmax_pct:.1} % design maximum, grid {:.1} m)",
            2.0 * gmax_pct,
            g.cell
        )
    } else {
        "no path between start and end inside the surface".to_string()
    })?;
    let mut path = Vec::new();
    let mut u = goal;
    loop {
        let c = u / 16;
        path.push(g.xy(c % g.nx, c / g.nx));
        if prev[u] == usize::MAX {
            break;
        }
        u = prev[u];
    }
    path.reverse();
    path[0] = start;
    *path.last_mut().unwrap() = end;
    Ok((path, dist[goal], expanded))
}

/// Share of path `p` (by vertex count) lying within `d` metres of polyline `q`.
pub fn overlap(p: &[[f64; 2]], q: &[[f64; 2]], d: f64) -> f64 {
    if p.is_empty() {
        return 0.0;
    }
    p.iter().filter(|x| dist_to_poly(**x, q) < d).count() as f64 / p.len() as f64
}

/// Douglas–Peucker simplification.
pub fn simplify(p: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let (mut imax, mut dmax) = (0, 0.0);
    for i in 1..p.len() - 1 {
        let d = dist_to_poly(p[i], &[p[0], p[p.len() - 1]]);
        if d > dmax {
            dmax = d;
            imax = i;
        }
    }
    if dmax > tol {
        let mut a = simplify(&p[..=imax], tol);
        let b = simplify(&p[imax..], tol);
        a.pop();
        a.extend(b);
        a
    } else {
        vec![p[0], p[p.len() - 1]]
    }
}

pub fn deflection(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    defl(a, b, c)
}
fn defl(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    let h1 = (b[1] - a[1]).atan2(b[0] - a[0]);
    let h2 = (c[1] - b[1]).atan2(c[0] - b[0]);
    crate::align::norm(h2 - h1).abs()
}

/// Drop PIs with tiny deflections or very short legs.
pub fn clean_pis(mut pis: Vec<[f64; 2]>, min_defl_deg: f64, min_leg: f64) -> Vec<[f64; 2]> {
    loop {
        let mut drop = None;
        for k in 1..pis.len().saturating_sub(1) {
            let l1 = ((pis[k][0] - pis[k - 1][0]).powi(2) + (pis[k][1] - pis[k - 1][1]).powi(2)).sqrt();
            let l2 = ((pis[k + 1][0] - pis[k][0]).powi(2) + (pis[k + 1][1] - pis[k][1]).powi(2)).sqrt();
            if defl(pis[k - 1], pis[k], pis[k + 1]).to_degrees() < min_defl_deg || l1.min(l2) < min_leg {
                drop = Some(k);
                break;
            }
        }
        match drop {
            Some(k) => {
                pis.remove(k);
            }
            None => return pis,
        }
    }
}

/// Largest radius ≤ r_pref that fits each PI, with tangents limited to `share` of the shorter
/// adjacent leg (whole leg at the ends). Spiral length ls shortens the available tangent by ls/2.
pub fn fit_radii(pis: &[[f64; 2]], r_pref: f64, ls: f64, share: f64) -> Vec<f64> {
    let leg = |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    (1..pis.len() - 1)
        .map(|k| {
            let d = defl(pis[k - 1], pis[k], pis[k + 1]);
            let l1 = leg(pis[k - 1], pis[k]) * if k == 1 { 0.9 } else { share };
            let l2 = leg(pis[k], pis[k + 1]) * if k + 1 == pis.len() - 1 { 0.9 } else { share };
            let t = l1.min(l2) - ls / 2.0;
            let r = (t / (d / 2.0).tan()).min(r_pref);
            (r / 5.0).floor() * 5.0
        })
        .collect()
}

/// Radius and spiral per PI with transitions sized for superelevation: each spiral is at least
/// the required development length `req(R)` rounded up to 5 m (the preferred `ls_pref` is used when
/// the deflection allows it), the
/// curve's tangent length R·tan(Δ/2) + Ls/2 must fit in `share` of the shorter adjacent leg (90 %
/// at the ends), and both spirals must fit in the deflection (2·Ls/(2R) ≤ 0.9·Δ). Radii are tried
/// from `r_pref` downwards in 5 m steps; a PI where no radius ≥ `r_floor` fits gets radius 0 so
/// the caller can drop it or record a design exception.
pub fn fit_curves(pis: &[[f64; 2]], r_pref: f64, ls_pref: f64, share: f64, r_floor: f64, req: &dyn Fn(f64) -> f64) -> (Vec<f64>, Vec<f64>) {
    let leg = |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let mut radii = vec![];
    let mut spirals = vec![];
    for k in 1..pis.len() - 1 {
        let d = defl(pis[k - 1], pis[k], pis[k + 1]);
        let l1 = leg(pis[k - 1], pis[k]) * if k == 1 { 0.9 } else { share };
        let l2 = leg(pis[k], pis[k + 1]) * if k + 1 == pis.len() - 1 { 0.9 } else { share };
        let avail = l1.min(l2);
        // radii from r_pref down to the floor, then (small deflections, where the spiral cannot fit in
        // Δ) flatter radii up to 4·r_pref, which also lower e and the development length
        let top = (r_pref / 5.0).floor() * 5.0;
        let down = (0..).map(|i| top - 5.0 * i as f64).take_while(|r| *r >= r_floor - 1e-9 && *r > 0.0);
        let up = (1..).map(|i| top + 5.0 * i as f64).take_while(|r| *r <= 4.0 * r_pref + 1e-9);
        let mut found = (0.0, 0.0);
        for r in down.chain(up) {
            // preferred spiral, shortened to what the deflection allows, never below the
            // required development length
            // (a deflection too small for any 5 m spiral gives a simple curve, whose development
            // runs 2/3 on the tangent and is checked by SE-OVERLAP)
            let need = (req(r) / 5.0).ceil() * 5.0;
            let fit = (ls_pref.min(0.9 * r * d) / 5.0).floor() * 5.0;
            let ls = if fit <= 0.0 { 0.0 } else { fit.max(need) };
            if r * (d / 2.0).tan() + ls.max(ls_pref) / 2.0 <= avail && ls <= 0.9 * r * d + 1e-9 {
                found = (r, ls);
                break;
            }
        }
        radii.push(found.0);
        spirals.push(found.1);
    }
    (radii, spirals)
}

pub fn path_stats(g: &Tin, path: &[[f64; 2]]) -> (f64, f64) {
    let mut len = 0.0;
    let mut gmax: f64 = 0.0;
    for w in path.windows(2) {
        let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        len += l;
        if let (Some(a), Some(b)) = (g.elevation_at(w[0][0], w[0][1]), g.elevation_at(w[1][0], w[1][1])) {
            if l > 0.0 {
                gmax = gmax.max((b - a).abs() / l * 100.0);
            }
        }
    }
    (len, gmax)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_sized_to_required_development() {
        // 90° PI with 200 m legs; development requirement 60 m for any radius
        let pis = [[0.0, 0.0], [200.0, 0.0], [200.0, 200.0]];
        let (r, ls) = fit_curves(&pis, 300.0, 30.0, 0.45, 50.0, &|_| 60.0);
        assert_eq!(ls, vec![60.0]);
        assert!(r[0] * (std::f64::consts::FRAC_PI_4).tan() + 30.0 <= 180.0 + 1e-9 && r[0] >= 50.0);
        // requirement that cannot fit: radius 0 (caller drops or flags the PI)
        let (r, _) = fit_curves(&pis, 300.0, 30.0, 0.45, 50.0, &|_| 400.0);
        assert_eq!(r, vec![0.0]);
        // small deflection (~10°): 0.9·R·Δ < 40 m at R 120, so a flatter radius is chosen
        let pis = [[0.0, 0.0], [400.0, 0.0], [800.0, 70.0]];
        let (r, ls) = fit_curves(&pis, 120.0, 30.0, 0.45, 50.0, &|_| 40.0);
        assert!(r[0] > 120.0 && ls[0] >= 40.0 && ls[0] <= 0.9 * r[0] * defl(pis[0], pis[1], pis[2]) + 1e-9, "{r:?} {ls:?}");
    }
    #[test]
    fn flat_grid_gives_straight_path() {
        let g = Grid { x0: 0.0, y0: 0.0, cell: 10.0, nx: 30, ny: 30, z: vec![Some(0.0); 900] };
        let (p, c, _) = least_cost(&g, [0.0, 0.0], [290.0, 0.0], 8.0, &default_alts()[0], &[]).unwrap();
        assert!((c - 290.0).abs() < 1e-6, "{c}");
        assert_eq!(simplify(&p, 1.0).len(), 2);
    }
    #[test]
    fn row_cost_surface_detours_around_expensive_and_no_go_parcels() {
        let g = Grid { x0: 0.0, y0: 0.0, cell: 10.0, nx: 30, ny: 30, z: vec![Some(0.0); 900] };
        // an expensive block (10 × median rate) across the straight line, x 100-200, y 0-150
        let mut rc = vec![1.0; 900];
        for j in 0..=15 {
            for i in 10..=20 {
                rc[j * 30 + i] = 10.0;
            }
        }
        let mut spec = default_alts()[0].clone();
        let (p0, _, _) = least_cost_row(&g, [0.0, 0.0], [290.0, 0.0], 8.0, &spec, &[], Some(&rc)).unwrap();
        assert!(p0.iter().all(|q| q[1] < 1.0), "w_row = 0 ignores the surface");
        spec.w_row = 1.0;
        let (p1, _, _) = least_cost_row(&g, [0.0, 0.0], [290.0, 0.0], 8.0, &spec, &[], Some(&rc)).unwrap();
        assert!(p1.iter().any(|q| q[1] > 150.0), "path should go round the expensive block");
        // the same block as no-go is impassable however small the weight
        let nogo: Vec<f64> = rc.iter().map(|v| if *v > 5.0 { f64::NAN } else { 0.0 }).collect();
        spec.w_row = 1e-6;
        let (p2, _, _) = least_cost_row(&g, [0.0, 0.0], [290.0, 0.0], 8.0, &spec, &[], Some(&nogo)).unwrap();
        for q in &p2 {
            assert!(!(q[0] >= 100.0 && q[0] <= 200.0 && q[1] <= 150.0), "{q:?} inside no-go");
        }
    }
    #[test]
    fn steep_ridge_is_avoided_by_grade_weighting() {
        // a 30 m high ridge across x = 150 except a gap at y >= 200
        let mut z = vec![Some(0.0); 900];
        for j in 0..20 {
            z[j * 30 + 15] = Some(30.0);
        }
        let g = Grid { x0: 0.0, y0: 0.0, cell: 10.0, nx: 30, ny: 30, z };
        let (p, _, _) = least_cost(&g, [0.0, 0.0], [290.0, 0.0], 8.0, &default_alts()[1], &[]).unwrap();
        assert!(p.iter().any(|q| q[1] >= 200.0), "path should detour through the gap");
    }
    #[test]
    fn hard_grade_search_switches_back_on_a_slope() {
        // plane rising 15 % in +x: the 10 % alternative must lengthen the path (switchbacks)
        let (nx, ny) = (40, 40);
        let z = (0..nx * ny).map(|k| Some((k % nx) as f64 * 10.0 * 0.15)).collect();
        let g = Grid { x0: 0.0, y0: 0.0, cell: 10.0, nx, ny, z };
        let spec = &default_alts()[3];
        let (p, _, _) = least_cost(&g, [0.0, 200.0], [100.0, 200.0], 10.0, spec, &[]).unwrap();
        let len: f64 = p.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum();
        assert!(len >= 140.0, "path {len} m should approach Δz/gmax = 150 m");
        for w in p.windows(2) {
            let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            assert!(0.15 * (w[1][0] - w[0][0]).abs() / l <= 0.2 + 1e-9);
        }
        // a 60 % plane admits no move within 20 %: clear error
        let z2 = (0..nx * ny).map(|k| Some((k % nx) as f64 * 10.0 * 0.60)).collect();
        let g2 = Grid { x0: 0.0, y0: 0.0, cell: 10.0, nx, ny, z: z2 };
        let e = least_cost(&g2, [0.0, 200.0], [100.0, 200.0], 10.0, spec, &[]).unwrap_err();
        assert!(e.contains("keeps the ground grade within 20.0 %"), "{e}");
    }
}
