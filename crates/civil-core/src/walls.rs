//! Retaining walls: placement rule, proportioning and external stability
//! (sliding, overturning, eccentricity, bearing) with Rankine active pressure.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soil {
    /// Retained soil unit weight (kN/m³)
    pub gamma_kn_m3: f64,
    /// Retained soil friction angle (degrees)
    pub phi_deg: f64,
    /// Base friction coefficient μ between footing and foundation soil
    pub base_friction: f64,
    /// Allowable bearing pressure f_a (kPa), from the geotechnical report
    pub bearing_kpa: f64,
    /// Uniform surcharge on the retained side (kPa), e.g. traffic
    #[serde(default)]
    pub surcharge_kpa: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WallKind {
    #[serde(rename = "cantilever")]
    Cantilever,
    #[serde(rename = "gravity")]
    Gravity,
}

/// Engineer-set rule: replace the side slope by a wall when it would exceed
/// the right-of-way offset or a slope height.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallRule {
    pub kind: WallKind,
    /// Maximum offset (m) from the centreline the earthwork may reach (ROW line).
    #[serde(default)]
    pub max_daylight_offset_m: Option<f64>,
    /// Maximum side-slope height (m) before a wall is used.
    #[serde(default)]
    pub max_slope_height_m: Option<f64>,
    /// Wall face offset from the slope hinge (m).
    #[serde(default = "d_off")]
    pub offset_from_hinge_m: f64,
    /// Footing embedment below ground at the wall face (m).
    #[serde(default = "d_emb")]
    pub embedment_m: f64,
    pub soil: Soil,
}
fn d_off() -> f64 {
    0.5
}
fn d_emb() -> f64 {
    0.8
}

#[derive(Debug, Clone, Serialize)]
pub struct WallDesign {
    pub kind: WallKind,
    /// Total design height from footing underside to top of retained soil (m)
    pub h: f64,
    pub exposed_h: f64,
    pub base_b: f64,
    pub toe: f64,
    pub heel: f64,
    pub base_t: f64,
    pub stem_top: f64,
    pub stem_bot: f64,
    pub ka: f64,
    pub pa_kn: f64,
    pub sum_v_kn: f64,
    pub m_resist: f64,
    pub m_over: f64,
    pub fs_sliding: f64,
    pub fs_overturning: f64,
    pub eccentricity: f64,
    pub q_max_kpa: f64,
    pub q_avg_kpa: f64,
    pub concrete_m2: f64,
    pub pass: bool,
    pub iterations: usize,
}

pub const GAMMA_C: f64 = 24.0;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Criteria {
    pub fs_sliding: f64,
    pub fs_overturning: f64,
    /// eccentricity limit as a fraction of B (1/6 keeps the whole base in compression)
    pub e_frac: f64,
    /// p_max ≤ k·f_a
    pub qmax_factor: f64,
    pub citation: &'static str,
}

pub fn criteria(std_jtg: bool) -> Criteria {
    if std_jtg {
        Criteria { fs_sliding: 1.3, fs_overturning: 1.6, e_frac: 1.0 / 6.0, qmax_factor: 1.2, citation: "GB 50007-2011 建筑地基基础设计规范 §6.7.5 (抗滑 Ks≥1.3, 抗倾覆 Kt≥1.6), §5.2.1 (pk≤fa, pkmax≤1.2fa)" }
    } else {
        Criteria { fs_sliding: 1.5, fs_overturning: 2.0, e_frac: 1.0 / 6.0, qmax_factor: 1.0, citation: "AASHTO Standard Specifications for Highway Bridges 17th ed. §5.5.5 (service load: FS sliding ≥1.5, overturning ≥2.0, e ≤ B/6 on soil)" }
    }
}

/// Stability of a given geometry. Rankine active pressure, level backfill,
/// moments about the toe, passive resistance ignored.
pub fn analyse(kind: &WallKind, h: f64, b: f64, soil: &Soil, cr: &Criteria) -> WallDesign {
    let ka = (std::f64::consts::FRAC_PI_4 - soil.phi_deg.to_radians() / 2.0).tan().powi(2);
    let (g, q) = (soil.gamma_kn_m3, soil.surcharge_kpa);
    let pa_soil = 0.5 * g * h * h * ka;
    let pa_q = q * ka * h;
    let pa = pa_soil + pa_q;
    let m_over = pa_soil * h / 3.0 + pa_q * h / 2.0;
    let (mut v, mut mr, toe, heel, t, st, sb, area);
    match kind {
        WallKind::Cantilever => {
            t = (0.1 * h).max(0.3);
            st = 0.25;
            sb = (0.1 * h).max(0.3);
            toe = (b / 3.0).max(0.0);
            heel = b - toe - sb;
            let hs = h - t;
            let xb = toe + sb; // back face of stem (vertical)
            let w1 = st * hs * GAMMA_C;
            let w2 = 0.5 * (sb - st) * hs * GAMMA_C;
            let w3 = b * t * GAMMA_C;
            let w4 = heel.max(0.0) * hs * g;
            let w5 = heel.max(0.0) * q;
            v = w1 + w2 + w3 + w4 + w5;
            mr = w1 * (xb - st / 2.0) + w2 * (toe + 2.0 / 3.0 * (sb - st)) + w3 * b / 2.0 + (w4 + w5) * (xb + heel.max(0.0) / 2.0);
            area = st * hs + 0.5 * (sb - st) * hs + b * t;
        }
        WallKind::Gravity => {
            st = 0.5;
            sb = b;
            t = 0.0;
            toe = 0.0;
            heel = 0.0;
            let w1 = st * h * GAMMA_C; // vertical back, rectangle at the back
            let w2 = 0.5 * (b - st) * h * GAMMA_C; // battered front triangle
            v = w1 + w2;
            mr = w1 * (b - st / 2.0) + w2 * (2.0 / 3.0 * (b - st));
            area = st * h + 0.5 * (b - st) * h;
        }
    }
    if v <= 0.0 {
        v = 1e-9;
        mr = 0.0;
    }
    let fs_s = soil.base_friction * v / pa;
    let fs_o = mr / m_over;
    let e = b / 2.0 - (mr - m_over) / v;
    let q_max = if e <= b / 6.0 { v / b * (1.0 + 6.0 * e / b) } else if e < b / 2.0 { 2.0 * v / (3.0 * (b / 2.0 - e)) } else { f64::INFINITY };
    let q_avg = v / b;
    let pass = fs_s >= cr.fs_sliding && fs_o >= cr.fs_overturning && e.abs() <= cr.e_frac * b + 1e-12 && q_max <= cr.qmax_factor * soil.bearing_kpa && q_avg <= soil.bearing_kpa && heel >= -1e-9;
    WallDesign { kind: kind.clone(), h, exposed_h: 0.0, base_b: b, toe, heel, base_t: t, stem_top: st, stem_bot: sb, ka, pa_kn: pa, sum_v_kn: v, m_resist: mr, m_over, fs_sliding: fs_s, fs_overturning: fs_o, eccentricity: e, q_max_kpa: q_max, q_avg_kpa: q_avg, concrete_m2: area, pass, iterations: 0 }
}

/// Smallest base width (0.05 m steps from 0.4H / 0.5H up to 2H) passing all checks.
pub fn proportion(kind: &WallKind, h: f64, soil: &Soil, cr: &Criteria) -> WallDesign {
    let start = match kind {
        WallKind::Cantilever => (0.4 * h).max(1.0),
        WallKind::Gravity => (0.5 * h).max(0.8),
    };
    let mut b = (start / 0.05).ceil() * 0.05;
    let mut it = 0;
    loop {
        it += 1;
        let d = analyse(kind, h, b, soil, cr);
        if d.pass || b >= 2.0 * h.max(1.0) {
            return WallDesign { iterations: it, ..d };
        }
        b += 0.05;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gravity_hand_calc() {
        // H=3, B=1.8, top 0.5, φ=30 (Ka=1/3), γ=18, q=0.
        // Pa = 0.5·18·9/3 = 27 kN; Mo = 27·1 = 27 kN·m
        // W1 = 0.5·3·24 = 36 @ 1.55; W2 = 0.5·1.3·3·24 = 46.8 @ 0.8667 → V = 82.8, Mr = 55.8 + 40.56 = 96.36
        let soil = Soil { gamma_kn_m3: 18.0, phi_deg: 30.0, base_friction: 0.5, bearing_kpa: 200.0, surcharge_kpa: 0.0 };
        let d = analyse(&WallKind::Gravity, 3.0, 1.8, &soil, &criteria(true));
        assert!((d.ka - 1.0 / 3.0).abs() < 1e-12);
        assert!((d.pa_kn - 27.0).abs() < 1e-9 && (d.m_over - 27.0).abs() < 1e-9);
        assert!((d.sum_v_kn - 82.8).abs() < 1e-9 && (d.m_resist - 96.36).abs() < 1e-9, "{} {}", d.sum_v_kn, d.m_resist);
        assert!((d.fs_sliding - 0.5 * 82.8 / 27.0).abs() < 1e-12);
        assert!((d.fs_overturning - 96.36 / 27.0).abs() < 1e-12);
        let e = 0.9 - (96.36 - 27.0) / 82.8;
        assert!((d.eccentricity - e).abs() < 1e-12);
        // sliding FS 1.533 ≥ 1.3, overturning 3.57, e = 0.0623 ≤ 0.3, qmax = 46·(1+6·0.0623/1.8) = 55.55 kPa
        assert!(d.pass);
    }
    #[test]
    fn proportioning_meets_criteria() {
        let soil = Soil { gamma_kn_m3: 19.0, phi_deg: 30.0, base_friction: 0.45, bearing_kpa: 250.0, surcharge_kpa: 10.0 };
        for h in [2.0, 4.0, 6.0, 8.0] {
            for k in [WallKind::Cantilever, WallKind::Gravity] {
                let d = proportion(&k, h, &soil, &criteria(true));
                assert!(d.pass, "{k:?} H={h}: {d:?}");
                // one step narrower fails (minimality)
                if d.iterations > 1 {
                    assert!(!analyse(&k, h, d.base_b - 0.05, &soil, &criteria(true)).pass);
                }
            }
        }
    }
}
