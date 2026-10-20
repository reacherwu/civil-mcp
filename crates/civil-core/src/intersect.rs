//! At-grade intersections (T and 4-leg) on a designed main road: leg geometry,
//! curb returns (fillet arcs between edge lines), stop-controlled sight triangles
//! with a line-of-sight obstruction test, leg corridors tied to the main road
//! finished grade, and an intersection paving patch with volumes.
use crate::align::{Alignment, AlignmentInput};
use crate::corridor::{self, Assembly, Corridor};
use crate::profile::{Profile, ProfileInput};
use crate::standards::{chk, Check};
use crate::tin::Tin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegInput {
    pub name: String,
    /// "left" or "right" of the main road (looking up-station)
    pub side: String,
    /// angle between main road forward direction and the leg (deg), 90 = perpendicular
    #[serde(default = "d90")]
    pub angle_deg: f64,
    pub length_m: f64,
    /// grade away from the main road (%, negative = falling)
    pub grade_pct: f64,
    /// curb-return radii [up-station corner, down-station corner] (m)
    pub return_radii_m: [f64; 2],
    pub assembly: Assembly,
}
fn d90() -> f64 {
    90.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntersectionInput {
    pub name: String,
    pub main_station: f64,
    pub legs: Vec<LegInput>,
    pub major_speed_kmh: f64,
    pub minor_speed_kmh: f64,
    /// engineer-set minimum return radius for the design vehicle (m)
    pub min_return_radius_m: f64,
    /// engineer-set maximum approach grade (%)
    pub max_approach_grade_pct: f64,
    /// AASHTO case B1 gap (s); default 7.5 s (passenger car, left turn from stop)
    #[serde(default = "d_tg")]
    pub time_gap_s: f64,
}
fn d_tg() -> f64 {
    7.5
}

#[derive(Debug, Clone, Serialize)]
pub struct Return {
    pub radius: f64,
    pub center: [f64; 2],
    pub arc: Vec<[f64; 3]>,
    pub corner: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SightLine {
    pub direction: String,
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub leg_m: f64,
    pub along_major_m: f64,
    pub max_obstruction_m: f64,
    pub obstruction_at: [f64; 2],
    pub triangle: [[f64; 2]; 3],
    pub clear: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LegResult {
    pub name: String,
    pub cl: [[f64; 2]; 2],
    pub edge_left: [[f64; 2]; 2],
    pub edge_right: [[f64; 2]; 2],
    pub returns: Vec<Return>,
    pub sight: Vec<SightLine>,
    pub corridor: Corridor,
    pub patch: Vec<[f64; 3]>,
    pub patch_faces: Vec<[usize; 3]>,
    pub patch_cut_m3: f64,
    pub patch_fill_m3: f64,
    pub tie_in_elev: f64,
    pub tie_in_station: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Intersection {
    pub name: String,
    pub main_station: f64,
    pub point: [f64; 3],
    pub legs: Vec<LegResult>,
    pub checks: Vec<Check>,
    pub method: String,
}

fn add(a: [f64; 2], b: [f64; 2], k: f64) -> [f64; 2] {
    [a[0] + b[0] * k, a[1] + b[1] * k]
}

/// Intersect line p + t·d with polyline; returns (point, polyline param index, t).
fn line_poly(p: [f64; 2], d: [f64; 2], poly: &[[f64; 2]]) -> Option<([f64; 2], usize, f64)> {
    for (i, w) in poly.windows(2).enumerate() {
        let e = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
        let den = d[0] * e[1] - d[1] * e[0];
        if den.abs() < 1e-12 {
            continue;
        }
        let q = [w[0][0] - p[0], w[0][1] - p[1]];
        let t = (q[0] * e[1] - q[1] * e[0]) / den;
        let u = (q[0] * d[1] - q[1] * d[0]) / den;
        if (0.0..=1.0).contains(&u) {
            return Some((add(p, d, t), i, t));
        }
    }
    None
}

/// Main-road edge polyline at signed offset (left +) around the station.
fn edge(al: &Alignment, s0: f64, off: f64, half: f64) -> Vec<([f64; 2], f64)> {
    let mut v = Vec::new();
    let mut s = (s0 - half).max(al.start_station());
    while s <= (s0 + half).min(al.end_station()) {
        let (x, y, h) = al.point_at(s).unwrap();
        v.push(([x - off * h.sin(), y + off * h.cos()], s));
        s += 0.25;
    }
    v
}

pub fn design(input: &IntersectionInput, main_al: &Alignment, main_pr: &Profile, main_cor: &Corridor, main_fg: &Tin, eg: &Tin) -> crate::Result<Intersection> {
    let s0 = input.main_station;
    let (x0, y0, h0) = main_al.point_at(s0).ok_or("main station off alignment")?;
    let z0 = main_pr.elevation_at(s0).ok_or("main station off profile")?;
    let sec = main_cor.sections.iter().min_by(|a, b| (a.station - s0).abs().partial_cmp(&(b.station - s0).abs()).unwrap()).ok_or("main corridor has no sections")?;
    let ground = |x: f64, y: f64| main_fg.elevation_at(x, y).or_else(|| eg.elevation_at(x, y));
    let mut legs = Vec::new();
    let mut checks = Vec::new();
    let isd = 0.278 * input.major_speed_kmh * input.time_gap_s;
    for leg in &input.legs {
        let sg = match leg.side.as_str() {
            "left" => 1.0,
            "right" => -1.0,
            o => return Err(format!("leg side must be left|right, got {o}")),
        };
        let ang = h0 + sg * leg.angle_deg.to_radians();
        let dir = [ang.cos(), ang.sin()];
        let nrm = [-dir[1], dir[0]]; // left of the leg looking away from the main road
        let p0 = [x0, y0];
        let w1 = if sg > 0.0 { sec.left.paved_width } else { sec.right.paved_width };
        let cfg = leg.assembly.side_cfg(true);
        let w2l = cfg.paved_width(0.0);
        let w2r = leg.assembly.side_cfg(false).paved_width(0.0);
        // main edge on the leg's side
        let me: Vec<([f64; 2], f64)> = edge(main_al, s0, sg * w1, 80.0);
        let me_xy: Vec<[f64; 2]> = me.iter().map(|p| p.0).collect();
        let (pe, _, s_edge) = line_poly(p0, dir, &me_xy).ok_or("leg does not cross the main road edge")?;
        let tie_z = ground(pe[0], pe[1]).ok_or("tie-in point outside surfaces")?;
        // curb returns: corner between main edge and each leg edge
        let mut returns = Vec::new();
        let mut max_t: f64 = s_edge;
        // the leg's left edge lies up-station when the leg is on the right, and vice versa
        for (k, (lsg, w2)) in [(1.0, w2l), (-1.0, w2r)].into_iter().enumerate() {
            let up_station = (lsg > 0.0) == (sg < 0.0);
            let r = if up_station { leg.return_radii_m[0] } else { leg.return_radii_m[1] };
            checks.push(chk("I-RETURN", "Curb return radius ≥ design-vehicle minimum", format!("{} {} corner", leg.name, if up_station { "up-station" } else { "down-station" }), r, input.min_return_radius_m, "m", "min", "mandatory", "fillet tangent to main-road edge and leg edge".into(), "design basis (engineer-set for the design vehicle); AASHTO 2018 §9.6".into()));
            let mo: Vec<[f64; 2]> = edge(main_al, s0, sg * (w1 + r), 80.0 + r).iter().map(|p| p.0).collect();
            let lp = add(p0, nrm, lsg * (w2 + r));
            let (c, _, _) = line_poly(lp, dir, &mo).ok_or("return radius too large for the edges")?;
            // tangent point on the main edge: closest main-edge point to c
            let tm = me_xy.iter().copied().min_by(|a, b| ((a[0] - c[0]).powi(2) + (a[1] - c[1]).powi(2)).partial_cmp(&((b[0] - c[0]).powi(2) + (b[1] - c[1]).powi(2))).unwrap()).unwrap();
            // tangent on the leg edge: foot of perpendicular from c
            let t_leg = (c[0] - p0[0]) * dir[0] + (c[1] - p0[1]) * dir[1];
            let tl = add(add(p0, nrm, lsg * w2), dir, t_leg);
            max_t = max_t.max(t_leg);
            let a0 = (tm[1] - c[1]).atan2(tm[0] - c[0]);
            let mut a1 = (tl[1] - c[1]).atan2(tl[0] - c[0]);
            while a1 - a0 > std::f64::consts::PI {
                a1 -= 2.0 * std::f64::consts::PI
            }
            while a0 - a1 > std::f64::consts::PI {
                a1 += 2.0 * std::f64::consts::PI
            }
            let n = 16;
            let arc: Vec<[f64; 3]> = (0..=n).map(|i| {
                let a = a0 + (a1 - a0) * i as f64 / n as f64;
                let p = [c[0] + r * a.cos(), c[1] + r * a.sin()];
                [p[0], p[1], 0.0]
            }).collect();
            let _ = k;
            returns.push(Return { radius: r, center: c, arc, corner: if up_station { "up-station".into() } else { "down-station".into() } });
        }
        // leg alignment / profile / corridor, starting past the returns
        let s_start = max_t + 1.0;
        let pa = add(p0, dir, s_start);
        let pb = add(p0, dir, leg.length_m);
        if leg.length_m <= s_start + 10.0 {
            return Err(format!("leg {} too short for its returns (needs > {:.1} m)", leg.name, s_start + 10.0));
        }
        let lal = Alignment::build(AlignmentInput { name: leg.name.clone(), pis: vec![pa, pb], radii: vec![], spirals: vec![], start_station: s_start })?;
        let g = leg.grade_pct / 100.0;
        let lpr = Profile::build(ProfileInput { name: format!("{} FG", leg.name), vpis: vec![[s_edge, tie_z], [leg.length_m, tie_z + g * (leg.length_m - s_edge)]], curve_lengths: vec![] })?;
        let lc = corridor::build(eg, &lal, &lpr, None, &leg.assembly, 10.0)?;
        checks.push(chk("I-GRADE", "Approach grade ≤ limit", format!("leg {}", leg.name), leg.grade_pct.abs(), input.max_approach_grade_pct, "%", "max", "mandatory", "|grade| of the leg profile from the tie-in".into(), "design basis (engineer-set); AASHTO 2018 §9.5".into()));
        // returns: elevations interpolated main edge → leg start section edge
        let first = &lc.sections[0];
        let zl = |left: bool| { let sd = if left { &first.left } else { &first.right }; corridor::interp(&sd.fg, sd.paved_width) };
        let mut rets = returns;
        for (ri, rt) in rets.iter_mut().enumerate() {
            let lsg = if ri == 0 { 1.0 } else { -1.0 };
            let zs = ground(rt.arc[0][0], rt.arc[0][1]).unwrap_or(tie_z);
            let ze = zl(lsg > 0.0);
            let n = rt.arc.len() - 1;
            for (i, p) in rt.arc.iter_mut().enumerate() {
                p[2] = zs + (ze - zs) * i as f64 / n as f64;
            }
        }
        // paving patch polygon: main edge between tangents → return 2 → leg end line → return 1 (reversed)
        let mut poly: Vec<[f64; 3]> = Vec::new();
        let r0 = &rets[0];
        let r1 = &rets[1];
        // leg left-edge return (index 0) starts at main edge; walk main edge from r0 start to r1 start
        let i0 = me_xy.iter().position(|p| (p[0] - r0.arc[0][0]).abs() < 1e-9 && (p[1] - r0.arc[0][1]).abs() < 1e-9).unwrap_or(0);
        let i1 = me_xy.iter().position(|p| (p[0] - r1.arc[0][0]).abs() < 1e-9 && (p[1] - r1.arc[0][1]).abs() < 1e-9).unwrap_or(0);
        let (lo, hi) = (i0.min(i1), i0.max(i1));
        let mut main_part: Vec<[f64; 3]> = me_xy[lo..=hi].iter().step_by(4).map(|p| [p[0], p[1], ground(p[0], p[1]).unwrap_or(tie_z)]).collect();
        if i0 > i1 {
            main_part.reverse();
        }
        poly.extend(main_part);
        let mut a1: Vec<[f64; 3]> = r1.arc.clone();
        poly.extend(a1.drain(..).skip(1));
        let el = add(add(p0, nrm, -w2r), dir, s_start);
        let er = add(add(p0, nrm, w2l), dir, s_start);
        poly.push([el[0], el[1], zl(false)]);
        poly.push([er[0], er[1], zl(true)]);
        let mut a0: Vec<[f64; 3]> = r0.arc.clone();
        a0.reverse();
        poly.extend(a0.into_iter().skip(1));
        poly.pop(); // last equals the first main-edge point
        // orientation for ear clipping: counter-clockwise
        let area: f64 = (0..poly.len()).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % poly.len()]); a[0] * b[1] - b[0] * a[1] }).sum();
        if area < 0.0 {
            poly.reverse();
        }
        let idx: Vec<usize> = (0..poly.len()).collect();
        let faces = match crate::grading::ear_clip(&poly, &idx) {
            Ok(f) => f,
            Err(_) => {
                // fallback: Delaunay of the boundary points, keep triangles whose centroid is inside the polygon
                let (t, _) = Tin::build(&poly, None)?;
                let inside = |x: f64, y: f64| {
                    let mut c = false;
                    let n = poly.len();
                    for i in 0..n {
                        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
                        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
                            c = !c;
                        }
                    }
                    c
                };
                t.tris.iter().copied().filter(|f| { let (a, b, c) = (t.pts[f[0]], t.pts[f[1]], t.pts[f[2]]); inside((a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0) }).collect()
            }
        };
        let (ptin, _) = Tin::from_faces(&poly, &faces)?;
        let (pc, pf, _) = corridor::composite_volume(&ptin, eg, 0.25);
        // sight triangles (stop control on the leg)
        let mut sight = Vec::new();
        let eye_t = s_edge + 4.4; // AASHTO: 4.4 m from the edge of the major-road travelled way
        let lane_c = add(add(p0, dir, eye_t), nrm, w2l / 2.0); // approach lane: right-hand side when driving toward the main road (= +nrm)
        let ez = lpr.elevation_at(eye_t).unwrap_or(tie_z) + 1.08;
        for (name, sdir) in [("up-station", 1.0), ("down-station", -1.0)] {
            let st = (s0 + sdir * isd).clamp(main_al.start_station(), main_al.end_station());
            let (tx, ty, th) = main_al.point_at(st).unwrap();
            // target in the near lane for vehicles approaching from the leg's left, far lane otherwise
            let lane = main_cor.assembly.lanes_m[0] / 2.0;
            // right-hand traffic: traffic from the driver's left uses the near lane
            let from_left = (tx - p0[0]) * -nrm[0] + (ty - p0[1]) * -nrm[1] > 0.0;
            let off = if from_left { lane * sg } else { -lane * sg };
            let tp = [tx - off * th.sin(), ty + off * th.cos()];
            let tz = main_pr.elevation_at(st).unwrap_or(z0) + 1.08;
            let mut worst = (f64::MIN, [0.0, 0.0]);
            for i in 1..100 {
                let t = i as f64 / 100.0;
                let q = [lane_c[0] + (tp[0] - lane_c[0]) * t, lane_c[1] + (tp[1] - lane_c[1]) * t];
                if let Some(gz) = ground(q[0], q[1]) {
                    let los = ez + (tz - ez) * t;
                    if gz - los > worst.0 {
                        worst = (gz - los, q);
                    }
                }
            }
            let clear = worst.0 <= 0.0;
            checks.push(chk("I-ISD", "Intersection sight triangle clear (stop control, case B1)", format!("{} looking {}", leg.name, name), worst.0.max(-9.99), 0.0, "m above sight line", "max", "mandatory", format!("ISD = 0.278·V·t_g = 0.278·{}·{} = {:.1} m; eye 1.08 m at 4.4 m from edge, object 1.08 m", input.major_speed_kmh, input.time_gap_s, isd), "AASHTO 2018 §9.5.3 Case B1 (stop control on the minor road)".into()));
            sight.push(SightLine { direction: name.into(), eye: [lane_c[0], lane_c[1], ez], target: [tp[0], tp[1], tz], leg_m: eye_t, along_major_m: (st - s0).abs(), max_obstruction_m: worst.0, obstruction_at: worst.1, triangle: [lane_c, pe, tp], clear });
        }
        legs.push(LegResult { name: leg.name.clone(), cl: [p0, pb], edge_left: [add(p0, nrm, w2l), add(pb, nrm, w2l)], edge_right: [add(p0, nrm, -w2r), add(pb, nrm, -w2r)], returns: rets, sight, corridor: lc, patch: poly, patch_faces: faces, patch_cut_m3: pc, patch_fill_m3: pf, tie_in_elev: tie_z, tie_in_station: s_edge });
    }
    Ok(Intersection { name: input.name.clone(), main_station: s0, point: [x0, y0, z0], legs, checks, method: "Returns: circle tangent to the main-road edge (offset w1) and the leg edge (offset w2), centre at offsets w1+R and w2+R. Leg profile ties to the main-road finished grade at the edge. Patch: ear-clipped polygon (main edge, returns, leg start section) as a TIN, composite volume vs existing ground (no pavement datum). Sight: AASHTO Case B1, ISD = 0.278·V·t_g, line of sight 1.08→1.08 m over corridor FG else existing ground.".into() })
}
