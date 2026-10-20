//! Design workflows that combine several tools: profile suggestion, evaluation of an
//! alignment alternative and the earthwork-balance profile optimiser.
use crate::align::{Alignment, AlignmentInput};
use crate::corridor::{self, Assembly, Corridor};
use crate::profile::{Profile, ProfileInput};
use crate::standards::{self, DesignBasis};
use crate::superelev::{self, Superelevation};
use crate::tin::Tin;
use serde::{Deserialize, Serialize};

/// VPIs on the ground at the given stations, grades clamped to the profile maximum
/// (UK: desirable maximum), curve lengths at the standard minimum rounded up to 5 m.
pub fn suggest_profile(b: &DesignBasis, tin: &Tin, al: &Alignment, st: &[f64]) -> crate::Result<(ProfileInput, Vec<usize>, f64)> {
    let rs = standards::resolve(b)?;
    let gmax = rs.g_desirable.unwrap_or(rs.g_max);
    let mut v = Vec::new();
    for &s in st {
        let (x, y, _) = al.point_at(s).ok_or(format!("station {s} off alignment"))?;
        v.push([s, tin.elevation_at(x, y).ok_or(format!("station {s} outside surface"))?]);
    }
    let mut clamped = vec![];
    for i in 1..v.len() {
        let g = (v[i][1] - v[i - 1][1]) / (v[i][0] - v[i - 1][0]) * 100.0;
        if g.abs() > gmax {
            v[i][1] = v[i - 1][1] + g.signum() * gmax / 100.0 * (v[i][0] - v[i - 1][0]);
            clamped.push(i);
        }
    }
    // smooth VPIs whose minimum vertical-curve length does not fit between neighbours:
    // move the VPI part-way toward the chord of its neighbours (reduces A), keep grades ≤ gmax
    for _ in 0..200 {
        let mut moved = false;
        for i in 1..v.len() - 1 {
            let g1 = (v[i][1] - v[i - 1][1]) / (v[i][0] - v[i - 1][0]) * 100.0;
            let g2 = (v[i + 1][1] - v[i][1]) / (v[i + 1][0] - v[i][0]) * 100.0;
            let need = standards::vc_len_min(b, &rs, g2 - g1).0;
            let room = (v[i][0] - v[i - 1][0]).min(v[i + 1][0] - v[i][0]);
            if need > (room / 5.0).floor() * 5.0 {
                let t = (v[i][0] - v[i - 1][0]) / (v[i + 1][0] - v[i - 1][0]);
                let zc = v[i - 1][1] + t * (v[i + 1][1] - v[i - 1][1]);
                v[i][1] += 0.3 * (zc - v[i][1]);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let lens = curve_lengths(b, &rs, &v);
    Ok((ProfileInput { name: "suggested".into(), vpis: v, curve_lengths: lens }, clamped, gmax))
}

pub fn curve_lengths(b: &DesignBasis, rs: &standards::Resolved, v: &[[f64; 2]]) -> Vec<f64> {
    let mut lens = vec![];
    for i in 1..v.len() - 1 {
        let g1 = (v[i][1] - v[i - 1][1]) / (v[i][0] - v[i - 1][0]) * 100.0;
        let g2 = (v[i + 1][1] - v[i][1]) / (v[i + 1][0] - v[i][0]) * 100.0;
        let need = standards::vc_len_min(b, rs, g2 - g1).0;
        // never longer than the room between neighbouring VPIs
        let room = (v[i][0] - v[i - 1][0]).min(v[i + 1][0] - v[i][0]);
        lens.push(((need / 5.0).ceil() * 5.0).max(0.0).min((room / 5.0).floor() * 5.0));
    }
    lens
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnitRates {
    /// cost-proxy rates in arbitrary currency units: per m³ excavation, per m³ embankment,
    /// per m³ borrow or waste, per m³ wall concrete, per m² pavement
    pub excavation: f64,
    pub embankment: f64,
    pub borrow_waste: f64,
    pub wall_concrete: f64,
    pub pavement_m2: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Evaluation {
    pub length_m: f64,
    pub radii: Vec<f64>,
    pub min_radius_m: f64,
    pub max_design_grade_pct: f64,
    pub cut_m3: f64,
    pub fill_m3: f64,
    pub imbalance_m3: f64,
    pub wall_concrete_m3: f64,
    pub wall_length_m: f64,
    pub pavement_m2: f64,
    pub geometry_mandatory_failures: usize,
    pub failed_checks: Vec<String>,
    pub daylight_warnings: usize,
    pub cost_proxy: f64,
    /// ∫|M| ds of the mass diagram (filled by the optimiser)
    pub haul_moment_m3m: f64,
}

pub struct Built {
    pub al: Alignment,
    pub se: Superelevation,
    pub pr: Profile,
    pub cor: Corridor,
}

pub fn evaluate(b: &DesignBasis, tin: &Tin, asm: &Assembly, ai: AlignmentInput, vpi_spacing: f64, interval: f64, shrink: f64, rates: &UnitRates, wc: Option<&crate::walls::Criteria>) -> crate::Result<(Evaluation, Built)> {
    let al = Alignment::build(ai.clone())?;
    let se = superelev::design(b, &al)?;
    let n = ((al.length / vpi_spacing).round() as usize).max(1);
    let st: Vec<f64> = (0..=n).map(|i| al.start_station() + al.length * i as f64 / n as f64).collect();
    let (pi, _, _) = suggest_profile(b, tin, &al, &st)?;
    let pr = Profile::build(pi)?;
    let cor = corridor::build_opts(tin, &al, &pr, Some(&se), asm, interval, None, b.design_speed_kmh, wc)?;
    let ev = summarize(b, &al, &pr, &cor, shrink, rates, &ai.radii);
    Ok((ev, Built { al, se, pr, cor }))
}

pub fn summarize(b: &DesignBasis, al: &Alignment, pr: &Profile, cor: &Corridor, shrink: f64, rates: &UnitRates, radii: &[f64]) -> Evaluation {
    let checks = standards::run_checks(b, al, Some(pr));
    let fails: Vec<String> = checks.iter().filter(|c| !c.pass && c.severity == "mandatory").map(|c| format!("{} {}", c.id, c.element)).collect();
    let imb = cor.cut_m3 * shrink - cor.fill_m3;
    let pav = al.length * (cor.assembly.paved_width() + cor.assembly.side_cfg(false).paved_width(0.0));
    let cost = rates.excavation * cor.cut_m3 + rates.embankment * cor.fill_m3 + rates.borrow_waste * imb.abs() + rates.wall_concrete * cor.wall_concrete_m3 + rates.pavement_m2 * pav;
    Evaluation {
        length_m: al.length,
        radii: radii.to_vec(),
        min_radius_m: radii.iter().cloned().fold(f64::INFINITY, f64::min),
        max_design_grade_pct: pr.grades_pct.iter().map(|g| g.abs()).fold(0.0, f64::max),
        cut_m3: cor.cut_m3,
        fill_m3: cor.fill_m3,
        imbalance_m3: imb,
        wall_concrete_m3: cor.wall_concrete_m3,
        wall_length_m: cor.wall_length_left_m + cor.wall_length_right_m,
        pavement_m2: pav,
        geometry_mandatory_failures: fails.len(),
        failed_checks: fails,
        daylight_warnings: cor.sections_with_warnings,
        cost_proxy: cost,
        haul_moment_m3m: crate::quantities::mass_haul(cor, shrink, 0.0).haul_moment_m3m,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OptStep {
    pub iteration: usize,
    pub vpi: usize,
    pub delta_m: f64,
    pub objective: f64,
    pub imbalance_m3: f64,
    pub cut_m3: f64,
    pub fill_m3: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptResult {
    pub before: ProfileInput,
    pub after: ProfileInput,
    pub objective_before: f64,
    pub objective_after: f64,
    pub before_eval: Evaluation,
    pub after_eval: Evaluation,
    pub evaluations: usize,
    pub steps: Vec<OptStep>,
    pub method: String,
}

/// Coordinate-descent profile optimiser.
/// Minimises |cut·shrink − fill| + w_volume·(cut + fill) + w_wall·wall_concrete. Each candidate
/// must keep every mandatory geometry check passing (max grade, vertical-curve lengths
/// recomputed at the minimum). End VPIs are held fixed (tie-ins).
pub fn optimize_profile(b: &DesignBasis, tin: &Tin, al: &Alignment, se: Option<&Superelevation>, asm: &Assembly, start: &ProfileInput, interval: f64, shrink: f64, w_volume: f64, w_wall: f64, w_haul: f64, steps_m: &[f64], wc: Option<&crate::walls::Criteria>) -> crate::Result<OptResult> {
    let rs = standards::resolve(b)?;
    let rates = UnitRates::default();
    let radii: Vec<f64> = al.curves().iter().map(|c| c.radius).collect();
    let eval = |v: &[[f64; 2]]| -> Result<(f64, Evaluation, ProfileInput), String> {
        let pi = ProfileInput { name: start.name.clone(), vpis: v.to_vec(), curve_lengths: curve_lengths(b, &rs, v) };
        let pr = Profile::build(pi.clone()).map_err(|e| format!("profile: {e}"))?;
        let checks = standards::run_checks(b, al, Some(&pr));
        if let Some(c) = checks.iter().find(|c| !c.pass && c.severity == "mandatory" && (c.id.starts_with("V-") || c.id == "G-COVER")) {
            return Err(format!("{} {} fails", c.id, c.element));
        }
        let cor = corridor::build_opts(tin, al, &pr, se, asm, interval, None, b.design_speed_kmh, wc).map_err(|e| format!("corridor: {e}"))?;
        let ev = summarize(b, al, &pr, &cor, shrink, &rates, &radii);
        let hm = crate::quantities::mass_haul(&cor, shrink, 0.0).haul_moment_m3m;
        let obj = ev.imbalance_m3.abs() + w_volume * (ev.cut_m3 + ev.fill_m3) + w_wall * ev.wall_concrete_m3 + w_haul * hm;
        let mut ev = ev;
        ev.haul_moment_m3m = hm;
        Ok((obj, ev, pi))
    };
    let mut v = start.vpis.clone();
    let (o0, e0, p0) = eval(&v).map_err(|e| format!("starting profile rejected ({e}); fix it before optimising"))?;
    let (mut best, mut best_ev, mut best_pi) = (o0, e0.clone(), p0.clone());
    let mut n_eval = 1;
    let mut steps = vec![];
    let mut it = 0;
    for &h in steps_m {
        let mut improved = true;
        while improved && it < 200 {
            improved = false;
            for k in 1..v.len() - 1 {
                for d in [h, -h] {
                    let mut c = v.clone();
                    c[k][1] += d;
                    n_eval += 1;
                    if let Ok((o, ev, pi)) = eval(&c) {
                        if o < best - 1.0 {
                            it += 1;
                            best = o;
                            best_ev = ev.clone();
                            best_pi = pi;
                            v = c;
                            improved = true;
                            steps.push(OptStep { iteration: it, vpi: k, delta_m: d, objective: o, imbalance_m3: ev.imbalance_m3, cut_m3: ev.cut_m3, fill_m3: ev.fill_m3 });
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(OptResult { before: p0, after: best_pi, objective_before: o0, objective_after: best, before_eval: e0, after_eval: best_ev, evaluations: n_eval, steps, method: format!("coordinate descent on interior VPI elevations, steps {steps_m:?} m; objective |cut·{shrink} − fill| + {w_volume}·(cut+fill) + {w_wall}·wall concrete + {w_haul}·haul moment ∫|M|ds; candidates must pass all mandatory grade / vertical-curve checks") })
}
