//! Design-flood hydrology for small road catchments.
//! - Rational method (US/CA; UK Modified Rational as a sense check): Q = C·i·A/360 (m³/s, mm/h, ha).
//! - NRCS TR-55 (1986): curve-number runoff, segmental time of concentration, graphical peak
//!   discharge (Type II coefficients transcribed from TR-55 Table F-1; verify).
//! - AU/NZ (ARR 2019): rational-method peak with the BoM IFD intensity supplied by the engineer;
//!   ARR 2019 prefers RFFE / flood models for rural catchments, which are outside this tool.
//! - UK: FEH / ReFH2 peak flows come from the licensed FEH software; the engineer enters them and
//!   the tool reports the Modified Rational (Wallingford) value as a sense check only.
use crate::standards::Standard;
use serde::{Deserialize, Serialize};

pub const TR55: &str = "USDA NRCS TR-55 Urban Hydrology for Small Watersheds (1986), Ch. 2-4, Table F-1";
pub const ARR: &str =
    "Australian Rainfall and Runoff 2019 (Ball et al.), Book 2 (IFD from BoM Data Hub), Book 5; rational method where permitted by the local authority";
pub const FEH: &str =
    "Flood Estimation Handbook / ReFH2 (UKCEH) - peak supplied by the engineer; Modified Rational (Wallingford Procedure) shown as sense check";
pub const RATIONAL: &str = "Rational method Q = C·i·A (FHWA HDS-2 / HEC-22 §3.2)";

/// Engineer-supplied intensity-duration table for one design AEP/return period: (duration min, mm/h).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idf {
    pub label: String,
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub source: String,
}

impl Idf {
    /// log-log interpolation; clamps outside the table
    pub fn intensity(&self, dur_min: f64) -> f64 {
        let p = &self.points;
        if dur_min <= p[0][0] {
            return p[0][1];
        }
        for w in p.windows(2) {
            if dur_min <= w[1][0] {
                let t = (dur_min.ln() - w[0][0].ln()) / (w[1][0].ln() - w[0][0].ln());
                return (w[0][1].ln() + t * (w[1][1].ln() - w[0][1].ln())).exp();
            }
        }
        p[p.len() - 1][1]
    }
}

pub fn rational_q(c: f64, i_mmh: f64, a_ha: f64) -> f64 {
    c * i_mmh * a_ha / 360.0
}

/// NRCS runoff depth (mm) for rainfall P (mm) and curve number CN (Ia = 0.2 S).
pub fn scs_runoff_mm(p: f64, cn: f64) -> f64 {
    let s = 25400.0 / cn - 254.0;
    if p <= 0.2 * s {
        0.0
    } else {
        (p - 0.2 * s).powi(2) / (p + 0.8 * s)
    }
}

/// TR-55 travel times (h). Sheet flow (L ≤ 30 m recommended) Eq 3-3 in US units; shallow
/// concentrated flow Fig 3-1 (unpaved V = 16.1345·√s ft/s, paved 20.3282·√s); channel by Manning.
pub fn tr55_tc_h(
    sheet_l_m: f64,
    sheet_n: f64,
    p2_mm: f64,
    sheet_s: f64,
    shallow_l_m: f64,
    shallow_s: f64,
    paved: bool,
    chan_l_m: f64,
    chan_v_ms: f64,
) -> (f64, Vec<String>) {
    let ft = 3.28084;
    let p2_in = p2_mm / 25.4;
    let t1 = if sheet_l_m > 0.0 { 0.007 * (sheet_n * sheet_l_m * ft).powf(0.8) / (p2_in.sqrt() * sheet_s.powf(0.4)) } else { 0.0 };
    let v2 = if paved { 20.3282 } else { 16.1345 } * shallow_s.max(1e-6).sqrt() / ft;
    let t2 = if shallow_l_m > 0.0 { shallow_l_m / v2 / 3600.0 } else { 0.0 };
    let t3 = if chan_l_m > 0.0 { chan_l_m / chan_v_ms.max(0.1) / 3600.0 } else { 0.0 };
    (
        t1 + t2 + t3,
        vec![
            format!("sheet flow Tt = 0.007(nL)^0.8/(P2^0.5 s^0.4) = {t1:.3} h"),
            format!("shallow concentrated V = {v2:.2} m/s, Tt = {t2:.3} h"),
            format!("channel Tt = {t3:.3} h"),
        ],
    )
}

/// TR-55 Table F-1 coefficients (C0, C1, C2) by Ia/P, rainfall type II and III.
const F1_II: [(f64, f64, f64, f64); 7] = [
    (0.10, 2.55323, -0.61512, -0.16403),
    (0.30, 2.46532, -0.62257, -0.11657),
    (0.35, 2.41896, -0.61594, -0.08820),
    (0.40, 2.36409, -0.59857, -0.05621),
    (0.45, 2.29238, -0.57005, -0.02281),
    (0.50, 2.20282, -0.51599, -0.01259),
    (0.50, 2.20282, -0.51599, -0.01259),
];

/// Unit peak discharge qu (csm/in) from TR-55 Eq 4-1 with Table F-1, Ia/P clamped to 0.1–0.5,
/// tc clamped to 0.1–10 h; coefficients interpolated linearly in Ia/P.
pub fn tr55_qu(tc_h: f64, ia_p: f64, rain_type: &str) -> f64 {
    let _ = rain_type; // only Type II coefficients are coded; other distributions are rejected upstream
    let t = &F1_II;
    let r = ia_p.clamp(0.1, 0.5);
    let mut c = (t[0].1, t[0].2, t[0].3);
    for w in t.windows(2) {
        if r >= w[0].0 && r <= w[1].0 {
            let f = if w[1].0 > w[0].0 { (r - w[0].0) / (w[1].0 - w[0].0) } else { 0.0 };
            c = (w[0].1 + f * (w[1].1 - w[0].1), w[0].2 + f * (w[1].2 - w[0].2), w[0].3 + f * (w[1].3 - w[0].3));
            break;
        }
    }
    let lt = tc_h.clamp(0.1, 10.0).log10();
    10f64.powf(c.0 + c.1 * lt + c.2 * lt * lt)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchmentInput {
    pub id: String,
    pub area_ha: f64,
    /// runoff coefficient (rational) – engineer
    #[serde(default)]
    pub c: Option<f64>,
    /// curve number (TR-55) – engineer
    #[serde(default)]
    pub cn: Option<f64>,
    /// main stream length (m) and equal-area slope (m/m) for tc when not given
    #[serde(default)]
    pub length_m: f64,
    #[serde(default)]
    pub slope: f64,
    #[serde(default)]
    pub tc_min: Option<f64>,
    /// UK: engineer-entered FEH/ReFH2 peak (m³/s)
    #[serde(default)]
    pub feh_peak_m3s: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydroInput {
    pub catchments: Vec<CatchmentInput>,
    /// design-event IFD table (engineer, from NOAA Atlas 14 / ECCC / BoM / FEH DDF)
    pub idf: Idf,
    /// 24-h design rainfall depth (mm) and NRCS distribution (TR-55, US/CA)
    #[serde(default)]
    pub p24_mm: Option<f64>,
    #[serde(default)]
    pub p2yr24_mm: Option<f64>,
    #[serde(default = "d_ii")]
    pub rain_type: String,
    /// "rational", "tr55", "arr_rational", "uk_check" – default by profile
    #[serde(default)]
    pub method: Option<String>,
}
fn d_ii() -> String {
    "II".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct CatchmentResult {
    pub id: String,
    pub area_ha: f64,
    pub tc_min: f64,
    pub method: String,
    pub intensity_mmh: Option<f64>,
    pub runoff_mm: Option<f64>,
    pub q_m3s: f64,
    pub q_rational_m3s: Option<f64>,
    pub steps: Vec<String>,
    pub engineer_inputs: Vec<String>,
}

/// Kirpich tc (min) = 0.0195·L^0.77·S^-0.385 (L m) – used when no tc or TR-55 segments given.
pub fn kirpich_min(l: f64, s: f64) -> f64 {
    0.0195 * l.powf(0.77) * s.max(1e-4).powf(-0.385)
}
/// Bransby-Williams tc (min) = 58·L/(A^0.1·S^0.2), L km, A km², S m/km (used in AU/NZ practice).
pub fn bransby_williams_min(l_m: f64, a_ha: f64, s: f64) -> f64 {
    58.0 * (l_m / 1000.0) / ((a_ha / 100.0).powf(0.1) * (s * 1000.0).powf(0.2))
}

pub fn design(std: Standard, inp: &HydroInput) -> crate::Result<(Vec<CatchmentResult>, String)> {
    let method = inp.method.clone().unwrap_or_else(|| match std {
        Standard::Us | Standard::Ca => {
            if inp.p24_mm.is_some() {
                "tr55".into()
            } else {
                "rational".into()
            }
        }
        Standard::Au | Standard::Nz => "arr_rational".into(),
        Standard::Uk => "uk_check".into(),
    });
    let cite = match method.as_str() {
        "tr55" => TR55,
        "arr_rational" => ARR,
        "uk_check" => FEH,
        _ => RATIONAL,
    };
    let mut out = vec![];
    for c in &inp.catchments {
        let mut steps = vec![];
        let mut eng = vec![format!("IFD '{}' ({})", inp.idf.label, inp.idf.source)];
        let tc = match c.tc_min {
            Some(t) => {
                eng.push("tc".into());
                t
            }
            None if method == "arr_rational" => {
                let t = bransby_williams_min(c.length_m, c.area_ha, c.slope);
                steps.push(format!("Bransby-Williams tc = 58·L/(A^0.1·S^0.2) = {t:.1} min"));
                t
            }
            None => {
                let t = kirpich_min(c.length_m, c.slope);
                steps.push(format!("Kirpich tc = 0.0195·L^0.77·S^-0.385 = {t:.1} min"));
                t
            }
        }
        .max(5.0);
        let i = inp.idf.intensity(tc);
        let qr = c.c.map(|cc| rational_q(cc, i, c.area_ha));
        if let Some(q) = qr {
            steps.push(format!("rational Q = C·i·A/360 = {:.2}·{i:.1}·{:.2}/360 = {q:.4} m³/s", c.c.unwrap(), c.area_ha));
            eng.push("C".into());
        }
        let (q, ro) = match method.as_str() {
            "tr55" => {
                let p = inp.p24_mm.ok_or("tr55 needs p24_mm")?;
                if inp.rain_type != "II" {
                    return Err(
                        "only the NRCS Type II Table F-1 coefficients are coded; use the rational method or enter tc/qu from TR-55 for other distributions"
                            .into(),
                    );
                }
                let cn = c.cn.ok_or(format!("catchment {}: tr55 needs cn", c.id))?;
                eng.push("CN, P24".into());
                let qmm = scs_runoff_mm(p, cn);
                let s = 25400.0 / cn - 254.0;
                let qu = tr55_qu(tc / 60.0, 0.2 * s / p, &inp.rain_type);
                let am = c.area_ha / 258.999;
                let qp = qu * am * (qmm / 25.4) * 0.0283168;
                steps.push(format!("S = 25400/CN − 254 = {s:.1} mm; Q = (P−0.2S)²/(P+0.8S) = {qmm:.1} mm"));
                steps.push(format!(
                    "Ia/P = {:.3}; qu = {qu:.1} csm/in (Type {}, tc {:.2} h); qp = qu·Am·Q = {qp:.4} m³/s",
                    0.2 * s / p,
                    inp.rain_type,
                    tc / 60.0
                ));
                (qp, Some(qmm))
            }
            "uk_check" => {
                let feh = c.feh_peak_m3s.ok_or(format!("catchment {}: UK profile needs feh_peak_m3s (FEH/ReFH2, engineer)", c.id))?;
                eng.push("FEH/ReFH2 peak".into());
                if let Some(q) = qr {
                    steps.push(format!("sense check: FEH {feh:.4} vs rational {q:.4} m³/s (ratio {:.2})", feh / q));
                }
                (feh, None)
            }
            _ => (qr.ok_or(format!("catchment {}: rational needs c", c.id))?, None),
        };
        out.push(CatchmentResult {
            id: c.id.clone(),
            area_ha: c.area_ha,
            tc_min: tc,
            method: method.clone(),
            intensity_mmh: Some(i),
            runoff_mm: ro,
            q_m3s: q,
            q_rational_m3s: qr,
            steps,
            engineer_inputs: eng,
        });
    }
    Ok((out, cite.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scs_runoff_hand_calc() {
        // CN 80, P = 125 mm: S = 63.5, Q = (125-12.7)^2/(125+50.8) = 71.73 mm
        assert!((scs_runoff_mm(125.0, 80.0) - 71.733).abs() < 0.01);
        assert_eq!(scs_runoff_mm(10.0, 60.0), 0.0);
    }
    #[test]
    fn idf_loglog() {
        let f = Idf { label: "t".into(), points: vec![[10.0, 100.0], [60.0, 40.0]], source: String::new() };
        assert!((f.intensity(10.0) - 100.0).abs() < 1e-9);
        let m = (10f64 * 60.0).sqrt();
        assert!((f.intensity(m) - (100.0f64 * 40.0).sqrt()).abs() < 1e-9);
    }
}
