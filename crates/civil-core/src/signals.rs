//! Traffic signals and capacity at concept depth.
//! - Warrants: MUTCD (2009/2023) Warrant 1 eight-hour vehicular volume, Conditions A, B and the
//!   80 % combination, 70 % / 56 % factors for speed > 40 mph or isolated communities (Table 4C-1);
//!   TAC (Canada) via the priority-point total from the TAC Traffic Signal Warrant Handbook entered by
//!   the engineer (≥ 100 points); UK and AU/NZ have no national numeric warrant: engineer-entered
//!   threshold table (e.g. state road authority guide) evaluated hour by hour, with capacity (RFC/DOS)
//!   from the priority-junction analysis as the supporting case.
//! - Timing: Webster optimum cycle C0 = (1.5L + 5)/(1 − Y), green split by critical flow ratios,
//!   intergreen by the ITE change/clearance formula, HCM uniform + incremental delay and LOS.
//! - Priority (two-way stop) junction capacity: HCM potential capacity c = vc·e^(−vc·tc/3600)/(1 − e^(−vc·tf/3600)),
//!   control delay and LOS; UK RFC and AU DOS reported from the same v/c.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarrantInput {
    /// hourly volumes: [major road total both approaches, higher-volume minor approach] (veh/h)
    pub hours: Vec<[f64; 2]>,
    pub major_lanes: usize,
    pub minor_lanes: usize,
    pub major_speed_kmh: f64,
    #[serde(default)]
    pub isolated_community: bool,
    /// TAC priority points (Canada), from the TAC handbook spreadsheet
    #[serde(default)]
    pub tac_points: Option<f64>,
    /// UK / AU / NZ: engineer thresholds {major_vph, minor_vph, hours_required, source}
    #[serde(default)]
    pub local: Option<LocalWarrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalWarrant {
    pub major_vph: f64,
    pub minor_vph: f64,
    pub hours_required: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WarrantResult {
    pub method: String,
    pub factor: f64,
    pub a_threshold: [f64; 2],
    pub b_threshold: [f64; 2],
    pub hours_a: usize,
    pub hours_b: usize,
    pub hours_combo: usize,
    pub met: bool,
    pub detail: String,
}

/// MUTCD Table 4C-1 100 % values: (A major, A minor, B major, B minor)
pub fn mutcd_base(major_lanes: usize, minor_lanes: usize) -> (f64, f64, f64, f64) {
    match (major_lanes >= 2, minor_lanes >= 2) {
        (false, false) => (500.0, 150.0, 750.0, 75.0),
        (true, false) => (600.0, 150.0, 900.0, 75.0),
        (true, true) => (600.0, 200.0, 900.0, 100.0),
        (false, true) => (500.0, 200.0, 750.0, 100.0),
    }
}

pub fn warrants(std: crate::standards::Standard, w: &WarrantInput) -> WarrantResult {
    use crate::standards::Standard::*;
    let (a1, a2, b1, b2) = mutcd_base(w.major_lanes, w.minor_lanes);
    let reduced = w.major_speed_kmh > 64.4 || w.isolated_community;
    let f = if reduced { 0.7 } else { 1.0 };
    let r = |x: f64, k: f64| (x * k).round();
    let cnt = |k: f64, t1: f64, t2: f64| w.hours.iter().filter(|h| h[0] >= r(t1, k) && h[1] >= r(t2, k)).count();
    let (ha, hb) = (cnt(f, a1, a2), cnt(f, b1, b2));
    let fc = if reduced { 0.56 } else { 0.8 };
    let hc = w.hours.iter().filter(|h| (h[0] >= r(a1, fc) && h[1] >= r(a2, fc)) && (h[0] >= r(b1, fc) && h[1] >= r(b2, fc))).count();
    let base = WarrantResult {
        method: String::new(),
        factor: f,
        a_threshold: [r(a1, f), r(a2, f)],
        b_threshold: [r(b1, f), r(b2, f)],
        hours_a: ha,
        hours_b: hb,
        hours_combo: hc,
        met: false,
        detail: String::new(),
    };
    match std {
        Ca if w.tac_points.is_some() => {
            let p = w.tac_points.unwrap();
            WarrantResult {
                method: "TAC Traffic Signal Warrant Handbook (2014) priority-point method: signals justified at ≥ 100 points (verify edition)".into(),
                met: p >= 100.0,
                detail: format!("{p:.0} points entered by the engineer; MUTCD Warrant 1 shown for comparison: A {ha} h, B {hb} h"),
                ..base
            }
        }
        Uk | Au | Nz | Ca => match &w.local {
            Some(l) => {
                let h = w.hours.iter().filter(|x| x[0] >= l.major_vph && x[1] >= l.minor_vph).count();
                WarrantResult {
                    method: format!(
                        "engineer threshold table ({}): major ≥ {:.0} veh/h and minor ≥ {:.0} veh/h for ≥ {} h",
                        l.source, l.major_vph, l.minor_vph, l.hours_required
                    ),
                    met: h >= l.hours_required,
                    hours_a: h,
                    detail: format!(
                        "{h} hours meet the engineer thresholds; no national numeric warrant in this jurisdiction (capacity and safety case govern)"
                    ),
                    ..base
                }
            }
            None => WarrantResult {
                method: "no national numeric warrant: enter local thresholds (local) or TAC points (Canada)".into(),
                detail: "not evaluated".into(),
                ..base
            },
        },
        _ => WarrantResult {
            method: format!(
                "MUTCD Warrant 1 (Table 4C-1), {} % volumes{}",
                (f * 100.0) as i32,
                if reduced { " (speed > 40 mph or isolated community)" } else { "" }
            ),
            met: ha >= 8 || hb >= 8 || hc >= 8,
            detail: format!(
                "Condition A {ha} h, Condition B {hb} h, combination ({:.0} %) {hc} h; 8 h required. Combination applies only after a trial of alternatives.",
                fc * 100.0
            ),
            ..base
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approach {
    pub name: String,
    pub major: bool,
    pub through_vph: f64,
    #[serde(default)]
    pub left_vph: f64,
    #[serde(default)]
    pub right_vph: f64,
    pub lanes: usize,
    pub speed_kmh: f64,
    /// distance to clear the conflict area (m), stop line to far side
    pub crossing_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingInput {
    pub approaches: Vec<Approach>,
    /// saturation flow per lane (pcu/h), engineer
    pub sat_flow_pcuhpl: f64,
    /// left-turn equivalent for permissive lefts (pcu/veh)
    #[serde(default = "d_el")]
    pub permissive_left_pcu: f64,
    #[serde(default = "d_lost")]
    pub lost_per_phase_s: f64,
    #[serde(default = "d_minc")]
    pub min_cycle_s: f64,
    #[serde(default = "d_maxc")]
    pub max_cycle_s: f64,
    /// cross-product threshold for a protected major-road left phase (ITE/HCM rule of thumb, verify)
    #[serde(default = "d_xp")]
    pub protected_left_cross_product: f64,
    #[serde(default)]
    pub source: String,
}
fn d_el() -> f64 {
    2.0
}
fn d_lost() -> f64 {
    4.0
}
fn d_minc() -> f64 {
    40.0
}
fn d_maxc() -> f64 {
    120.0
}
fn d_xp() -> f64 {
    50000.0
}

#[derive(Debug, Clone, Serialize)]
pub struct Phase {
    pub name: String,
    pub movements: Vec<String>,
    pub y: f64,
    pub green_s: f64,
    pub yellow_s: f64,
    pub all_red_s: f64,
    pub x: f64,
    pub delay_s: f64,
    pub volume_vph: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Timing {
    pub cycle_webster_s: f64,
    pub cycle_s: f64,
    pub lost_s: f64,
    pub y_total: f64,
    pub phases: Vec<Phase>,
    pub delay_s: f64,
    pub los: String,
    pub protected_left: bool,
    pub oversaturated: bool,
    pub steps: Vec<String>,
}

pub fn los_signal(d: f64) -> &'static str {
    match d {
        d if d <= 10.0 => "A",
        d if d <= 20.0 => "B",
        d if d <= 35.0 => "C",
        d if d <= 55.0 => "D",
        d if d <= 80.0 => "E",
        _ => "F",
    }
}
pub fn los_twsc(d: f64) -> &'static str {
    match d {
        d if d <= 10.0 => "A",
        d if d <= 15.0 => "B",
        d if d <= 25.0 => "C",
        d if d <= 35.0 => "D",
        d if d <= 50.0 => "E",
        _ => "F",
    }
}

/// ITE change interval: yellow = t + v/(2a), all-red = (W + L)/v (level grade)
pub fn intergreen(speed_kmh: f64, w: f64) -> (f64, f64) {
    let v = speed_kmh / 3.6;
    (1.0 + v / (2.0 * 3.05), (w + 6.0) / v)
}

/// HCM uniform + incremental delay (T = 0.25 h, k = 0.5, I = 1, PF = 1)
pub fn hcm_delay(c: f64, g: f64, x: f64, cap: f64) -> f64 {
    let gc = g / c;
    let d1 = 0.5 * c * (1.0 - gc).powi(2) / (1.0 - x.min(1.0) * gc);
    let t = 0.25;
    let d2 = 900.0 * t * ((x - 1.0) + ((x - 1.0).powi(2) + 8.0 * 0.5 * x / (cap * t)).sqrt());
    d1 + d2
}

pub fn timing(t: &TimingInput) -> Timing {
    let maj: Vec<&Approach> = t.approaches.iter().filter(|a| a.major).collect();
    let min: Vec<&Approach> = t.approaches.iter().filter(|a| !a.major).collect();
    // protected left if left × opposing through exceeds the threshold on any major approach
    let opp = |a: &Approach| maj.iter().filter(|b| b.name != a.name).map(|b| b.through_vph + b.right_vph).sum::<f64>();
    let prot = maj.iter().any(|a| a.left_vph * opp(a) > t.protected_left_cross_product);
    let s = t.sat_flow_pcuhpl;
    let lane_y = |a: &Approach, with_left: bool| {
        let lv = if with_left { a.left_vph * if prot { 1.0 } else { t.permissive_left_pcu } } else { 0.0 };
        (a.through_vph + a.right_vph + lv) / (a.lanes as f64 * s)
    };
    let mut ph: Vec<(String, Vec<String>, f64, f64, f64, f64)> = vec![]; // name, movements, y, volume, speed, crossing
    let mx = |v: &[&Approach]| (v.iter().map(|a| a.speed_kmh).fold(0.0, f64::max), v.iter().map(|a| a.crossing_m).fold(0.0, f64::max));
    if prot {
        let y = maj.iter().map(|a| a.left_vph / s).fold(0.0, f64::max);
        let (sp, w) = mx(&maj);
        ph.push(("major protected left".into(), maj.iter().map(|a| format!("{} L", a.name)).collect(), y, maj.iter().map(|a| a.left_vph).sum(), sp, w));
    }
    let (sp, w) = mx(&maj);
    ph.push((
        "major".into(),
        maj.iter().map(|a| format!("{} T/R{}", a.name, if prot { "" } else { "/L perm." })).collect(),
        maj.iter().map(|a| lane_y(a, !prot)).fold(0.0, f64::max),
        maj.iter().map(|a| a.through_vph + a.right_vph + if prot { 0.0 } else { a.left_vph }).sum(),
        sp,
        w,
    ));
    let (sp, w) = mx(&min);
    ph.push((
        "minor".into(),
        min.iter().map(|a| format!("{} all", a.name)).collect(),
        min.iter().map(|a| lane_y(a, true)).fold(0.0, f64::max),
        min.iter().map(|a| a.through_vph + a.left_vph + a.right_vph).sum(),
        sp,
        w,
    ));
    let n = ph.len() as f64;
    let lost = n * t.lost_per_phase_s;
    let yt: f64 = ph.iter().map(|p| p.2).sum();
    let over = yt >= 0.95;
    let c0 = if over { f64::INFINITY } else { (1.5 * lost + 5.0) / (1.0 - yt) };
    let c = c0.clamp(t.min_cycle_s, t.max_cycle_s);
    let mut phases = vec![];
    let (mut dsum, mut vsum) = (0.0, 0.0);
    for (name, mv, y, vol, sp, w) in &ph {
        let g = (c - lost) * y / yt;
        let x = y * c / g;
        let cap = vol / x.max(1e-9);
        let d = hcm_delay(c, g, x, cap.max(1.0));
        let (ye, ar) = intergreen(*sp, *w);
        dsum += d * vol;
        vsum += vol;
        phases.push(Phase { name: name.clone(), movements: mv.clone(), y: *y, green_s: g, yellow_s: ye, all_red_s: ar, x, delay_s: d, volume_vph: *vol });
    }
    let dl = dsum / vsum.max(1e-9);
    Timing {
        cycle_webster_s: c0,
        cycle_s: c,
        lost_s: lost,
        y_total: yt,
        los: los_signal(dl).into(),
        delay_s: dl,
        protected_left: prot,
        oversaturated: over,
        steps: vec![
            format!("phases {} (protected major left: {prot}), lost time L = {lost:.0} s, ΣY = {yt:.3}", ph.len()),
            format!("Webster C0 = (1.5L + 5)/(1 − Y) = {c0:.1} s, used {c:.0} s (limits {:.0}–{:.0} s)", t.min_cycle_s, t.max_cycle_s),
            "green g_i = (C − L)·y_i/ΣY; X = y·C/g; HCM delay d = d1 + d2 (T = 0.25 h, k = 0.5, I = 1, PF = 1); intergreen per ITE (t = 1 s, a = 3.05 m/s², vehicle 6 m)".into(),
        ],
        phases,
    }
}

/// HCM two-way-stop potential capacity (veh/h) for conflicting flow vc, critical headway tc, follow-up tf
pub fn twsc_capacity(vc: f64, tc: f64, tf: f64) -> f64 {
    if vc <= 0.0 {
        return 3600.0 / tf;
    }
    vc * (-vc * tc / 3600.0).exp() / (1.0 - (-vc * tf / 3600.0).exp())
}
/// HCM TWSC control delay (s/veh), T = 0.25 h
pub fn twsc_delay(v: f64, c: f64) -> f64 {
    let x = v / c;
    3600.0 / c + 900.0 * 0.25 * ((x - 1.0) + ((x - 1.0).powi(2) + (3600.0 / c) * x / (450.0 * 0.25)).sqrt()) + 5.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutcd_table_and_webster() {
        assert_eq!(mutcd_base(1, 1), (500.0, 150.0, 750.0, 75.0));
        let w = WarrantInput {
            hours: vec![[520.0, 160.0]; 8],
            major_lanes: 1,
            minor_lanes: 1,
            major_speed_kmh: 50.0,
            isolated_community: false,
            tac_points: None,
            local: None,
        };
        assert!(warrants(crate::standards::Standard::Us, &w).met);
        let w2 = WarrantInput { major_speed_kmh: 80.0, hours: vec![[360.0, 110.0]; 8], ..w };
        let r = warrants(crate::standards::Standard::Us, &w2);
        assert_eq!(r.a_threshold, [350.0, 105.0]);
        assert!(r.met);
        // TWSC capacity: vc = 500, tc 6.5, tf 3.5 → hand value
        let c = twsc_capacity(500.0, 6.5, 3.5);
        let e = 500.0 * (-500.0f64 * 6.5 / 3600.0).exp() / (1.0 - (-500.0f64 * 3.5 / 3600.0).exp());
        assert!((c - e).abs() < 1e-9);
    }
}
