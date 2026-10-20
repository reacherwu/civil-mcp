//! Gravity pipe networks: invert design, Manning hydraulics, rule checks.
use crate::standards::{chk, Check};
use crate::tin::Tin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PipeStandard {
    #[serde(rename = "GB50014_2021")]
    Gb50014,
    #[serde(rename = "FHWA_HEC22")]
    Hec22,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeCriteria {
    pub standard: PipeStandard,
    /// "storm" or "sanitary"
    pub system: String,
    /// Minimum cover over pipe crown (m), engineer-set.
    pub min_cover_m: f64,
    /// Minimum slope (m/m), engineer-set.
    pub min_slope: f64,
    /// Invert drop across a structure (m).
    #[serde(default = "d_drop")]
    pub structure_drop_m: f64,
    /// Rainfall intensity for the rational method (mm/h), engineer-set; storm only.
    #[serde(default)]
    pub intensity_mm_h: Option<f64>,
}
fn d_drop() -> f64 {
    0.03
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    pub id: String,
    pub x: f64,
    pub y: f64,
    /// Rim elevation; if omitted taken from the design surface (or existing ground).
    #[serde(default)]
    pub rim: Option<f64>,
    /// Tributary area (ha) and runoff coefficient for the rational method.
    #[serde(default)]
    pub area_ha: f64,
    #[serde(default)]
    pub c: f64,
    /// Sanitary/base inflow at this structure (m³/s).
    #[serde(default)]
    pub inflow_m3s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipe {
    pub id: String,
    pub from: String,
    pub to: String,
    pub diameter_m: f64,
    pub n: f64,
    #[serde(default)]
    pub material: String,
    /// Optional engineer-set maximum slope; when the ground is steeper the pipe is
    /// lowered at its upstream structure (drop manhole) instead of following the ground.
    #[serde(default)]
    pub max_slope: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInput {
    pub name: String,
    pub criteria: PipeCriteria,
    pub structures: Vec<Structure>,
    pub pipes: Vec<Pipe>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipeResult {
    pub id: String,
    pub from: String,
    pub to: String,
    pub length: f64,
    pub diameter_m: f64,
    pub material: String,
    pub invert_up: f64,
    pub invert_dn: f64,
    pub slope: f64,
    pub q_design: f64,
    pub q_full: f64,
    pub v_full: f64,
    pub depth_ratio: f64,
    pub v_design: f64,
    pub cover_up: f64,
    pub cover_dn: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Network {
    pub name: String,
    pub structures: Vec<(String, f64, f64, f64, f64)>, // id, x, y, rim, sump invert
    pub pipes: Vec<PipeResult>,
    pub checks: Vec<Check>,
}

/// Full-pipe Manning flow (SI): Q = (1/n)·A·R^(2/3)·S^(1/2).
pub fn manning_full(d: f64, n: f64, s: f64) -> (f64, f64) {
    let a = std::f64::consts::PI * d * d / 4.0;
    let r = d / 4.0;
    let v = r.powf(2.0 / 3.0) * s.sqrt() / n;
    (v * a, v)
}

/// Partial flow in a circular pipe at depth ratio y = h/D: (Q, V).
pub fn manning_partial(d: f64, n: f64, s: f64, y: f64) -> (f64, f64) {
    let th = 2.0 * (1.0 - 2.0 * y).acos();
    let a = d * d / 8.0 * (th - th.sin());
    let p = d * th / 2.0;
    if a <= 0.0 {
        return (0.0, 0.0);
    }
    let v = (a / p).powf(2.0 / 3.0) * s.sqrt() / n;
    (v * a, v)
}

/// Depth ratio carrying Q (bisection on 0..0.938, the depth of maximum capacity).
pub fn depth_for(d: f64, n: f64, s: f64, q: f64) -> Option<f64> {
    if q <= 0.0 {
        return Some(0.0);
    }
    let (qmax, _) = manning_partial(d, n, s, 0.938);
    if q > qmax {
        return None;
    }
    let (mut lo, mut hi) = (0.0, 0.938);
    for _ in 0..80 {
        let m = (lo + hi) / 2.0;
        if manning_partial(d, n, s, m).0 < q {
            lo = m
        } else {
            hi = m
        }
    }
    Some((lo + hi) / 2.0)
}

/// GB 50014-2021 maximum design depth ratio for sanitary sewers by diameter.
pub fn gb_max_depth_ratio(d: f64) -> f64 {
    if d <= 0.3 {
        0.55
    } else if d <= 0.45 {
        0.65
    } else if d <= 0.9 {
        0.70
    } else {
        0.75
    }
}

const GB: &str = "GB 50014-2021 室外排水设计标准";
const HEC: &str = "FHWA HEC-22 Urban Drainage Design Manual (3rd ed.)";

pub fn design(input: &NetworkInput, surface: &Tin, fallback: Option<&Tin>) -> crate::Result<Network> {
    let cr = &input.criteria;
    if cr.min_cover_m < 0.0 || cr.min_slope <= 0.0 {
        return Err("criteria: min_cover >= 0 and min_slope > 0".into());
    }
    let idx = |id: &str| input.structures.iter().position(|s| s.id == id).ok_or(format!("unknown structure {id}"));
    let ns = input.structures.len();
    let mut rim = vec![0.0; ns];
    for (i, s) in input.structures.iter().enumerate() {
        rim[i] = match s.rim {
            Some(r) => r,
            None => surface.elevation_at(s.x, s.y).or_else(|| fallback.and_then(|f| f.elevation_at(s.x, s.y))).ok_or(format!("structure {} outside surfaces; give rim", s.id))?,
        };
    }
    // topology: each structure has at most one outgoing pipe (dendritic network)
    let mut out = vec![None; ns];
    let mut indeg = vec![0usize; ns];
    for (k, p) in input.pipes.iter().enumerate() {
        let (a, b) = (idx(&p.from)?, idx(&p.to)?);
        if out[a].is_some() {
            return Err(format!("structure {} has more than one outgoing pipe (only dendritic networks supported)", p.from));
        }
        if p.diameter_m <= 0.0 || p.n <= 0.0 {
            return Err(format!("pipe {}: diameter and n must be > 0", p.id));
        }
        out[a] = Some(k);
        indeg[b] += 1;
    }
    // topological order
    let mut order = Vec::new();
    let mut deg = indeg.clone();
    let mut stack: Vec<usize> = (0..ns).filter(|i| deg[*i] == 0).collect();
    while let Some(i) = stack.pop() {
        order.push(i);
        if let Some(k) = out[i] {
            let b = idx(&input.pipes[k].to)?;
            deg[b] -= 1;
            if deg[b] == 0 {
                stack.push(b);
            }
        }
    }
    if order.len() != ns {
        return Err("pipe network has a cycle".into());
    }
    // flows (accumulate downstream)
    let mut ca = vec![0.0; ns];
    let mut base = vec![0.0; ns];
    for i in 0..ns {
        ca[i] = input.structures[i].c * input.structures[i].area_ha;
        base[i] = input.structures[i].inflow_m3s;
    }
    let storm = cr.system == "storm";
    if storm && cr.intensity_mm_h.is_none() && ca.iter().any(|v| *v > 0.0) {
        return Err("storm system with tributary areas needs intensity_mm_h".into());
    }
    let mut lowest_in = vec![f64::INFINITY; ns];
    let mut results: Vec<Option<PipeResult>> = vec![None; input.pipes.len()];
    let mut checks = Vec::new();
    for &i in &order {
        let Some(k) = out[i] else { continue };
        let p = &input.pipes[k];
        let j = idx(&p.to)?;
        let (s0, s1) = (&input.structures[i], &input.structures[j]);
        let len = ((s1.x - s0.x).powi(2) + (s1.y - s0.y).powi(2)).sqrt();
        let q = ca[i] * cr.intensity_mm_h.unwrap_or(0.0) / 360.0 + base[i];
        ca[j] += ca[i];
        base[j] += base[i];
        let inv_up = (rim[i] - cr.min_cover_m - p.diameter_m).min(lowest_in[i] - cr.structure_drop_m);
        let inv_dn = (inv_up - cr.min_slope * len).min(rim[j] - cr.min_cover_m - p.diameter_m);
        let mut inv_up = inv_up;
        if let Some(ms) = p.max_slope {
            if (inv_up - inv_dn) / len > ms {
                inv_up = inv_dn + ms * len;
            }
        }
        let slope = (inv_up - inv_dn) / len;
        lowest_in[j] = lowest_in[j].min(inv_dn);
        let (qf, vf) = manning_full(p.diameter_m, p.n, slope);
        let y = depth_for(p.diameter_m, p.n, slope, q);
        let vd = y.map_or(vf, |y| manning_partial(p.diameter_m, p.n, slope, y).1);
        let r = PipeResult { id: p.id.clone(), from: p.from.clone(), to: p.to.clone(), length: len, diameter_m: p.diameter_m, material: p.material.clone(), invert_up: inv_up, invert_dn: inv_dn, slope, q_design: q, q_full: qf, v_full: vf, depth_ratio: y.unwrap_or(f64::INFINITY), v_design: vd, cover_up: rim[i] - inv_up - p.diameter_m, cover_dn: rim[j] - inv_dn - p.diameter_m };
        let el = format!("pipe {}", p.id);
        let mf = format!("Manning (SI): Q = (1/n)·A·R^(2/3)·S^(1/2), n={}, S={:.5}, D={} m", p.n, slope, p.diameter_m);
        checks.push(chk("P-COVER", "Minimum cover", el.clone(), r.cover_up.min(r.cover_dn), cr.min_cover_m, "m", "min", "mandatory", "rim − invert − D at both ends".into(), "design basis (engineer-set)".into()));
        checks.push(chk("P-SLOPE", "Minimum slope", el.clone(), slope, cr.min_slope, "m/m", "min", "mandatory", "(inv_up − inv_dn)/L".into(), "design basis (engineer-set)".into()));
        match (&cr.standard, storm) {
            (PipeStandard::Gb50014, true) => {
                checks.push(chk("P-CAP", "管道满流能力 ≥ 设计流量", el.clone(), qf, q, "m³/s", "min", "mandatory", format!("{mf}; Q_design = Σ(C·A)·i/360 + base"), format!("{GB}, 雨水管道按满流计算")));
                checks.push(chk("P-VMIN", "雨水管满流最小流速", el.clone(), vf, 0.75, "m/s", "min", "mandatory", mf.clone(), format!("{GB}, 最小设计流速")));
                checks.push(chk("P-VMAX", "非金属管最大流速", el.clone(), vf, 5.0, "m/s", "max", "mandatory", mf.clone(), format!("{GB}, 最大设计流速")));
            }
            (PipeStandard::Gb50014, false) => {
                let lim = gb_max_depth_ratio(p.diameter_m);
                checks.push(chk("P-DEPTH", "污水管最大设计充满度", el.clone(), r.depth_ratio, lim, "h/D", "max", "mandatory", format!("{mf}; depth solved for Q={q:.4} m³/s"), format!("{GB}, 最大设计充满度表")));
                checks.push(chk("P-VMIN", "污水管设计充满度下最小流速", el.clone(), vd, 0.6, "m/s", "min", "mandatory", mf.clone(), format!("{GB}, 最小设计流速")));
                checks.push(chk("P-VMAX", "非金属管最大流速", el.clone(), vd, 5.0, "m/s", "max", "mandatory", mf.clone(), format!("{GB}, 最大设计流速")));
            }
            (PipeStandard::Hec22, _) => {
                checks.push(chk("P-CAP", "Full-flow capacity ≥ design flow", el.clone(), qf, q, "m³/s", "min", "mandatory", format!("{mf}; Q = C·i·A/360 (SI rational)"), format!("{HEC}, Ch.6 storm drain hydraulics")));
                checks.push(chk("P-VMIN", "Minimum full-flow velocity", el.clone(), vf, 0.9, "m/s", "min", "mandatory", mf.clone(), format!("{HEC}, minimum velocity 0.9 m/s (3 ft/s)")));
            }
        }
        results[k] = Some(r);
    }
    let structures = input.structures.iter().enumerate().map(|(i, s)| (s.id.clone(), s.x, s.y, rim[i], if lowest_in[i].is_finite() { lowest_in[i].min(results.iter().flatten().find(|r| r.from == s.id).map_or(f64::INFINITY, |r| r.invert_up)) } else { results.iter().flatten().find(|r| r.from == s.id).map_or(rim[i] - cr.min_cover_m, |r| r.invert_up) })).collect();
    Ok(Network { name: input.name.clone(), structures, pipes: results.into_iter().flatten().collect(), checks })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manning_textbook_and_partial_consistency() {
        // D=0.3, n=0.013, S=0.01: R=0.075, R^(2/3)=0.17784, V = 0.17784·0.1/0.013 = 1.3680 m/s, Q = V·0.070686 = 0.09670 m³/s
        let (q, v) = manning_full(0.3, 0.013, 0.01);
        assert!((v - 1.3680).abs() < 1e-3 && (q - 0.09670).abs() < 1e-4, "{q} {v}");
        // half full: same velocity as full
        let (qh, vh) = manning_partial(0.3, 0.013, 0.01, 0.5);
        assert!((vh - v).abs() < 1e-9 && (qh - q / 2.0).abs() < 1e-9);
        // full partial = full
        assert!((manning_partial(0.3, 0.013, 0.01, 1.0).0 - q).abs() < 1e-9);
        let y = depth_for(0.3, 0.013, 0.01, q / 2.0).unwrap();
        assert!((y - 0.5).abs() < 1e-6);
    }
}
