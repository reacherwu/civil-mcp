//! Gravity pipe networks: invert design, Manning hydraulics, rule checks.
use crate::standards::{chk, Check};
use crate::tin::Tin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PipeStandard {
    /// FHWA HEC-22 storm-drain criteria (US; public domain).
    #[serde(rename = "FHWA_HEC22")]
    Hec22,
    /// Engineer-entered criteria from the governing local standard (e.g. Sewers for Adoption / Design and
    /// Construction Guidance, a provincial or state agency sewer standard, Ten States Standards).
    #[serde(rename = "ENGINEER")]
    Engineer,
}

/// Engineer-entered pipe criteria (required for ENGINEER; optional overrides for HEC-22).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalCriteria {
    #[serde(default)]
    pub min_velocity_mps: Option<f64>,
    #[serde(default)]
    pub max_velocity_mps: Option<f64>,
    /// sanitary: maximum design depth ratio h/D at peak flow
    #[serde(default)]
    pub max_depth_ratio: Option<f64>,
    #[serde(default)]
    pub min_diameter_m: Option<f64>,
    /// "harmon" | "ten_states" | "fixed"
    #[serde(default)]
    pub peaking: Option<String>,
    #[serde(default)]
    pub peak_factor: Option<f64>,
    #[serde(default)]
    pub citation: String,
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
    /// Water level at the outfall (m); HGL starts at max(tailwater, invert + (dc + D)/2).
    #[serde(default)]
    pub tailwater_m: Option<f64>,
    /// Required freeboard between structure HGL and rim (m).
    #[serde(default = "d_fb")]
    pub freeboard_m: f64,
    #[serde(default)]
    pub local: LocalCriteria,
}
fn d_fb() -> f64 {
    0.3
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
    /// Sanitary: population served at this structure and per-capita flow (L/person/day).
    #[serde(default)]
    pub population: f64,
    #[serde(default)]
    pub per_capita_lpd: f64,
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
    /// Optional per-pipe minimum slope (overrides the network minimum), e.g. head reaches.
    #[serde(default)]
    pub min_slope: Option<f64>,
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
    /// Sanitary only: average dry-weather flow (m³/s) and peaking factor.
    pub q_avg: f64,
    pub peak_factor: f64,
    pub hgl_up: f64,
    pub hgl_dn: f64,
    pub egl_up: f64,
    pub egl_dn: f64,
    /// "full" (pressurised / surcharged) or "partial" (normal depth governs)
    pub regime: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructHydraulics {
    pub id: String,
    pub rim: f64,
    pub hgl: f64,
    pub egl: f64,
    pub k: f64,
    pub deflection_deg: Option<f64>,
    pub loss_m: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Network {
    pub name: String,
    pub structures: Vec<(String, f64, f64, f64, f64)>, // id, x, y, rim, sump invert
    pub pipes: Vec<PipeResult>,
    pub checks: Vec<Check>,
    pub hydraulics: Vec<StructHydraulics>,
    pub hgl_method: String,
}

pub const G: f64 = 9.80665;

/// Harmon peaking factor, P = population in thousands: PF = 1 + 14/(4 + √P).
pub fn harmon_pf(p_thousands: f64) -> f64 {
    1.0 + 14.0 / (4.0 + p_thousands.sqrt())
}
/// Ten States Standards (Recommended Standards for Wastewater Facilities, 2014) Figure 1:
/// PF = (18 + √P)/(4 + √P), P = population in thousands.
pub fn ten_states_pf(p_thousands: f64) -> f64 {
    (18.0 + p_thousands.sqrt()) / (4.0 + p_thousands.sqrt())
}

/// HEC-22 approximate method, manhole loss coefficient vs deflection angle
/// (straight 0.15, 22.5° 0.45, 45° 0.75, 60° 0.85, 90° 1.00), linear between rows.
pub fn hec22_k(deflection_deg: f64) -> f64 {
    let t = [(0.0, 0.15), (22.5, 0.45), (45.0, 0.75), (60.0, 0.85), (90.0, 1.0)];
    let a = deflection_deg.abs().min(90.0);
    for w in t.windows(2) {
        if a <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (a - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    1.0
}

/// Flow area and top width at depth ratio y.
fn seg(d: f64, y: f64) -> (f64, f64) {
    let th = 2.0 * (1.0 - 2.0 * y).acos();
    (d * d / 8.0 * (th - th.sin()), d * (th / 2.0).sin())
}

/// Critical depth ratio in a circular pipe: Q²/g = A³/T (bisection).
pub fn critical_depth(d: f64, q: f64) -> f64 {
    if q <= 0.0 {
        return 0.0;
    }
    let (mut lo, mut hi) = (1e-6, 0.999);
    for _ in 0..80 {
        let m = (lo + hi) / 2.0;
        let (a, t) = seg(d, m);
        if a * a * a / t < q * q / G {
            lo = m
        } else {
            hi = m
        }
    }
    (lo + hi) / 2.0
}

/// Full-flow friction slope from Manning: Sf = (Q·n / (A·R^(2/3)))².
pub fn friction_slope_full(d: f64, n: f64, q: f64) -> f64 {
    let a = std::f64::consts::PI * d * d / 4.0;
    (q * n / (a * (d / 4.0).powf(2.0 / 3.0))).powi(2)
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
    let mut avg = vec![0.0; ns];
    let mut pop = vec![0.0; ns];
    for i in 0..ns {
        ca[i] = input.structures[i].c * input.structures[i].area_ha;
        base[i] = input.structures[i].inflow_m3s;
        pop[i] = input.structures[i].population;
        avg[i] = input.structures[i].population * input.structures[i].per_capita_lpd / 1000.0 / 86400.0;
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
        let kz = if avg[i] > 0.0 {
            match cr.local.peaking.as_deref() {
                Some("harmon") => harmon_pf(pop[i] / 1000.0),
                Some("ten_states") => ten_states_pf(pop[i] / 1000.0),
                Some("fixed") => cr.local.peak_factor.ok_or("peaking 'fixed' needs peak_factor")?,
                _ => return Err("sanitary flows need criteria.local.peaking = harmon | ten_states | fixed (engineer choice)".into()),
            }
        } else {
            0.0
        };
        let q = ca[i] * cr.intensity_mm_h.unwrap_or(0.0) / 360.0 + base[i] + kz * avg[i];
        let q_avg = avg[i];
        ca[j] += ca[i];
        base[j] += base[i];
        avg[j] += avg[i];
        pop[j] += pop[i];
        let inv_up = (rim[i] - cr.min_cover_m - p.diameter_m).min(lowest_in[i] - cr.structure_drop_m);
        let smin = p.min_slope.unwrap_or(cr.min_slope);
        let inv_dn = (inv_up - smin * len).min(rim[j] - cr.min_cover_m - p.diameter_m);
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
        let r = PipeResult { id: p.id.clone(), from: p.from.clone(), to: p.to.clone(), length: len, diameter_m: p.diameter_m, material: p.material.clone(), invert_up: inv_up, invert_dn: inv_dn, slope, q_design: q, q_full: qf, v_full: vf, depth_ratio: y.unwrap_or(f64::INFINITY), v_design: vd, cover_up: rim[i] - inv_up - p.diameter_m, cover_dn: rim[j] - inv_dn - p.diameter_m, q_avg, peak_factor: kz, hgl_up: f64::NAN, hgl_dn: f64::NAN, egl_up: f64::NAN, egl_dn: f64::NAN, regime: String::new() };
        let el = format!("pipe {}", p.id);
        let mf = format!("Manning (SI): Q = (1/n)·A·R^(2/3)·S^(1/2), n={}, S={:.5}, D={} m", p.n, slope, p.diameter_m);
        checks.push(chk("P-COVER", "Minimum cover", el.clone(), r.cover_up.min(r.cover_dn), cr.min_cover_m, "m", "min", "mandatory", "rim − invert − D at both ends".into(), "design basis (engineer-set)".into()));
        checks.push(chk("P-SLOPE", "Minimum slope", el.clone(), slope, cr.min_slope, "m/m", "min", "mandatory", "(inv_up − inv_dn)/L".into(), "design basis (engineer-set)".into()));
        match (&cr.standard, storm) {
            (PipeStandard::Engineer, _) => {
                let lc = &cr.local;
                let cite = if lc.citation.is_empty() { "engineer-entered criteria (no citation given)".to_string() } else { lc.citation.clone() };
                let (vchk, vname) = if storm { (vf, "full-flow") } else { (vd, "design-flow") };
                if storm {
                    checks.push(chk("P-CAP", "Full-flow capacity ≥ design flow", el.clone(), qf, q, "m³/s", "min", "mandatory", format!("{mf}; Q = C·i·A/360 (SI rational)"), cite.clone()));
                } else {
                    let lim = lc.max_depth_ratio.ok_or("sanitary ENGINEER criteria need local.max_depth_ratio")?;
                    checks.push(chk("P-DEPTH", "Maximum depth ratio at peak flow", el.clone(), r.depth_ratio, lim, "h/D", "max", "mandatory", format!("{mf}; depth solved for Q={q:.4} m³/s"), cite.clone()));
                }
                let vmin = lc.min_velocity_mps.ok_or("ENGINEER criteria need local.min_velocity_mps")?;
                checks.push(chk("P-VMIN", &format!("Minimum {vname} velocity (self-cleansing)"), el.clone(), vchk, vmin, "m/s", "min", "mandatory", mf.clone(), cite.clone()));
                if let Some(vmax) = lc.max_velocity_mps {
                    checks.push(chk("P-VMAX", &format!("Maximum {vname} velocity"), el.clone(), vchk, vmax, "m/s", "max", "mandatory", mf.clone(), cite.clone()));
                }
                if let Some(dm) = lc.min_diameter_m {
                    checks.push(chk("P-DMIN", "Minimum diameter", el.clone(), p.diameter_m, dm, "m", "min", "mandatory", "D".into(), cite.clone()));
                }
            }
            (PipeStandard::Hec22, _) => {
                checks.push(chk("P-CAP", "Full-flow capacity ≥ design flow", el.clone(), qf, q, "m³/s", "min", "mandatory", format!("{mf}; Q = C·i·A/360 (SI rational)"), format!("{HEC}, Ch.6 storm drain hydraulics")));
                checks.push(chk("P-VMIN", "Minimum full-flow velocity", el.clone(), vf, 0.9, "m/s", "min", "mandatory", mf.clone(), format!("{HEC}, minimum velocity 0.9 m/s (3 ft/s)")));
            }
        }
        results[k] = Some(r);
    }
    // ---- hydraulic & energy grade lines (downstream → upstream) ----
    let res: Vec<PipeResult> = results.iter().map(|r| r.clone().unwrap()).collect();
    let mut results: Vec<PipeResult> = res;
    let mut s_hgl = vec![f64::NAN; ns];
    let mut s_egl = vec![f64::NAN; ns];
    let mut s_k = vec![0.0; ns];
    let mut s_def: Vec<Option<f64>> = vec![None; ns];
    let mut s_loss = vec![0.0; ns];
    let heading = |a: usize, b: usize| (input.structures[b].y - input.structures[a].y).atan2(input.structures[b].x - input.structures[a].x);
    // outfalls
    for i in 0..ns {
        if out[i].is_none() {
            let inc: Vec<usize> = (0..input.pipes.len()).filter(|&k| idx(&input.pipes[k].to).ok() == Some(i)).collect();
            let mut h = cr.tailwater_m.unwrap_or(f64::NEG_INFINITY);
            for &k in &inc {
                let r = &results[k];
                let dc = critical_depth(r.diameter_m, r.q_design) * r.diameter_m;
                h = h.max(r.invert_dn + (dc + r.diameter_m) / 2.0);
            }
            s_hgl[i] = h;
            s_egl[i] = h;
        }
    }
    for &i in order.iter().rev() {
        let Some(k) = out[i] else { continue };
        let j = idx(&input.pipes[k].to)?;
        let n = input.pipes[k].n;
        let r = &mut results[k];
        let (d, q) = (r.diameter_m, r.q_design);
        let a_full = std::f64::consts::PI * d * d / 4.0;
        let yn = if r.depth_ratio.is_finite() { r.depth_ratio * d } else { d };
        let hdn = s_hgl[j].max(r.invert_dn + yn);
        let full = hdn >= r.invert_dn + d - 1e-9 || !r.depth_ratio.is_finite();
        let (hup, vup, vdn) = if full {
            let hup = (hdn + friction_slope_full(d, n, q) * r.length).max(r.invert_up + yn);
            (hup, q / a_full, q / a_full)
        } else {
            let a = seg(d, yn / d).0.max(1e-12);
            (r.invert_up + yn, q / a, q / a)
        };
        r.hgl_dn = hdn;
        r.hgl_up = hup;
        r.egl_dn = hdn + vdn * vdn / (2.0 * G);
        r.egl_up = hup + vup * vup / (2.0 * G);
        r.regime = if full { "full".into() } else { "partial".into() };
        // structure i loss (HEC-22 approximate: H = K·Vo²/2g, Vo = outlet pipe velocity)
        let inc: Vec<usize> = (0..input.pipes.len()).filter(|&kk| idx(&input.pipes[kk].to).ok() == Some(i)).collect();
        let main_in = inc.iter().copied().max_by(|a, b| results[*a].q_design.partial_cmp(&results[*b].q_design).unwrap());
        let (kk, defl) = match main_in {
            Some(m) => {
                let a_in = heading(idx(&input.pipes[m].from)?, i);
                let a_out = heading(i, j);
                let mut dd = (a_out - a_in).to_degrees();
                while dd > 180.0 {
                    dd -= 360.0
                }
                while dd < -180.0 {
                    dd += 360.0
                }
                (hec22_k(dd), Some(dd.abs()))
            }
            None => (0.5, None), // inlet / start structure, straight run
        };
        let r = &results[k];
        let loss = kk * vup * vup / (2.0 * G);
        s_k[i] = kk;
        s_def[i] = defl;
        s_loss[i] = loss;
        s_egl[i] = r.egl_up + loss;
        s_hgl[i] = s_egl[i]; // velocity head in the structure neglected
    }
    let mut hyd = Vec::new();
    for i in 0..ns {
        let id = input.structures[i].id.clone();
        checks.push(chk("P-HGL", "Structure HGL below rim minus freeboard", format!("structure {id}"), s_hgl[i], rim[i] - cr.freeboard_m, "m", "max", "mandatory", format!("HGL_s = EGL_out + K·Vo²/2g, K={:.2}; freeboard {} m", s_k[i], cr.freeboard_m), format!("{HEC}, energy grade line evaluation (approximate method)")));
        hyd.push(StructHydraulics { id, rim: rim[i], hgl: s_hgl[i], egl: s_egl[i], k: s_k[i], deflection_deg: s_def[i], loss_m: s_loss[i] });
    }
    for r in &results {
        if r.regime == "full" {
            checks.push(chk("P-SURCH", "Pipe flows full (surcharged)", format!("pipe {}", r.id), r.hgl_dn - r.invert_dn - r.diameter_m, 0.0, "m", "max", "advisory", "HGL at outlet − crown".into(), HEC.into()));
        }
    }
    let results: Vec<Option<PipeResult>> = results.into_iter().map(Some).collect();
    let structures = input.structures.iter().enumerate().map(|(i, s)| (s.id.clone(), s.x, s.y, rim[i], if lowest_in[i].is_finite() { lowest_in[i].min(results.iter().flatten().find(|r| r.from == s.id).map_or(f64::INFINITY, |r| r.invert_up)) } else { results.iter().flatten().find(|r| r.from == s.id).map_or(rim[i] - cr.min_cover_m, |r| r.invert_up) })).collect();
    Ok(Network { name: input.name.clone(), structures, pipes: results.into_iter().flatten().collect(), checks, hydraulics: hyd, hgl_method: "FHWA HEC-22 approximate method: outfall HGL = max(tailwater, invert + (dc+D)/2); full flow: HGL_up = HGL_dn + Sf·L, Sf = (Qn/(A R^(2/3)))²; partial flow: normal depth; structure: E = EGL_out + K·Vo²/2g, K by deflection (0°0.15, 22.5°0.45, 45°0.75, 60°0.85, 90°1.0; start structure 0.5). Coefficients transcribed — verify against the HEC-22 edition in use.".into() })
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
    #[test]
    fn peaking_and_hgl_full_flow() {
        assert!((harmon_pf(1.0) - 3.8).abs() < 1e-12);
        assert!((ten_states_pf(1.0) - 3.8).abs() < 1e-12);
        assert!((ten_states_pf(100.0) - 2.0).abs() < 1e-12);
        assert_eq!(hec22_k(0.0), 0.15);
        assert!((hec22_k(30.0) - (0.45 + 0.30 * 7.5 / 22.5)).abs() < 1e-12);
        // one pipe, tailwater 1 m above the outlet crown → full flow; HGL_up = TW + Sf·L
        let tin = Tin::build(&[[0.0, 0.0, 10.0], [200.0, 0.0, 10.0], [0.0, 200.0, 10.0], [200.0, 200.0, 10.0]], None).unwrap().0;
        let inp: NetworkInput = serde_json::from_value(serde_json::json!({"name":"t","criteria":{"standard":"FHWA_HEC22","system":"storm","min_cover_m":1.0,"min_slope":0.005,"intensity_mm_h":100.0,"tailwater_m":9.0},
            "structures":[{"id":"A","x":10.0,"y":100.0,"area_ha":1.0,"c":0.9},{"id":"B","x":110.0,"y":100.0}],
            "pipes":[{"id":"P","from":"A","to":"B","diameter_m":0.6,"n":0.013}]})).unwrap();
        let n = design(&inp, &tin, None).unwrap();
        let p = &n.pipes[0];
        let q = 0.9 * 100.0 / 360.0;
        let sf = friction_slope_full(0.6, 0.013, q);
        assert_eq!(p.regime, "full");
        assert!((p.hgl_dn - 9.0).abs() < 1e-12 && (p.hgl_up - (9.0 + sf * 100.0)).abs() < 1e-9);
        let v = q / (std::f64::consts::PI * 0.09);
        let a = n.hydraulics.iter().find(|h| h.id == "A").unwrap();
        assert!((a.hgl - (p.hgl_up + v * v / (2.0 * G) * 1.5)).abs() < 1e-9, "{a:?}");
    }
}
