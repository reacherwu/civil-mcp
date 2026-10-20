//! Pavement internal (subsurface) drainage: FHWA infiltration-ratio / crack-infiltration
//! inflow (Cedergren; FHWA-NHI-05-114 / FHWA HIF DRIP), Darcy capacity of the drainage layer,
//! edge-drain pipe capacity and outlet spacing, and the AASHTO 1993 drainage coefficient m
//! (Guide Table 2.4 - engineer input from the quality-of-drainage assessment).
use serde::{Deserialize, Serialize};

pub const FHWA_SUB: &str = "FHWA-NHI-05-114 Pavement Subsurface Drainage Design (Cedergren crack-infiltration method qi = Ic(Nc/W + Wc/(W·Cs)) + kp); AASHTO 1993 Guide §2.4.1 (drainage coefficient)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubInput {
    /// crack infiltration rate Ic (m³/day per m of crack); FHWA suggests 0.223 (2.4 ft³/day/ft)
    #[serde(default = "d_ic")]
    pub ic_m3_day_m: f64,
    /// transverse crack spacing Cs (m) and length of transverse cracks Wc (m)
    pub crack_spacing_m: f64,
    /// pavement surface permeability kp (m/day)
    #[serde(default)]
    pub kp_m_day: f64,
    /// drainage layer: permeability k (m/day), thickness (m), effective porosity
    pub k_layer_m_day: f64,
    pub layer_thickness_m: f64,
    pub porosity: f64,
    /// edge drain pipe diameter (m), Manning n, grade (m/m) - grade defaults to the profile
    pub pipe_d_m: f64,
    #[serde(default = "d_n")]
    pub pipe_n: f64,
    pub max_outlet_spacing_m: f64,
    /// AASHTO drainage coefficient m (engineer, Guide Table 2.4)
    pub m_coefficient: f64,
    #[serde(default)]
    pub source: String,
}
fn d_ic() -> f64 { 0.223 }
fn d_n() -> f64 { 0.012 }

#[derive(Debug, Clone, Serialize)]
pub struct SubResult {
    pub width_m: f64,
    pub n_cracks: f64,
    pub qi_m3_day_m2: f64,
    pub inflow_m3_day_per_m: f64,
    pub layer_capacity_m3_day_per_m: f64,
    pub resultant_slope: f64,
    pub flow_path_m: f64,
    pub t50_hours: f64,
    pub pipe_capacity_m3s: f64,
    pub outlet_spacing_m: f64,
    pub m_coefficient: f64,
    pub steps: Vec<String>,
}

/// width W (m, one side drained), lanes nl, cross slope sx, longitudinal grade g
pub fn design(inp: &SubInput, width: f64, nl: usize, sx: f64, g: f64) -> SubResult {
    let nc = nl as f64 + 1.0;
    let qi = inp.ic_m3_day_m * (nc / width + width / (width * inp.crack_spacing_m)) + inp.kp_m_day;
    let qin = qi * width; // per m of road length
    let sr = (sx * sx + g * g).sqrt();
    let lr = width * (1.0 + (g / sx.max(1e-6)).powi(2)).sqrt();
    // Darcy: q = k·i·A, i ≈ resultant slope, A = thickness per m
    let cap = inp.k_layer_m_day * sr * inp.layer_thickness_m;
    // time for 50% drainage (Casagrande–Shannon approximation used by FHWA DRIP: t50 = ne·L²/(2k(H + L·S)))
    let t50 = inp.porosity * lr * lr / (2.0 * inp.k_layer_m_day * (inp.layer_thickness_m + lr * sr)) * 24.0;
    let a = std::f64::consts::PI * inp.pipe_d_m.powi(2) / 4.0;
    let qp = a * (inp.pipe_d_m / 4.0).powf(2.0 / 3.0) * g.abs().max(0.002).sqrt() / inp.pipe_n;
    let spacing = (qp * 86400.0 / qin).min(inp.max_outlet_spacing_m);
    SubResult { width_m: width, n_cracks: nc, qi_m3_day_m2: qi, inflow_m3_day_per_m: qin, layer_capacity_m3_day_per_m: cap, resultant_slope: sr, flow_path_m: lr, t50_hours: t50, pipe_capacity_m3s: qp, outlet_spacing_m: spacing, m_coefficient: inp.m_coefficient,
        steps: vec![format!("qi = Ic(Nc/W + Wc/(W·Cs)) + kp = {:.3}({nc:.0}/{width:.2} + {width:.2}/({width:.2}·{:.1})) + {:.4} = {qi:.4} m³/day/m²", inp.ic_m3_day_m, inp.crack_spacing_m, inp.kp_m_day),
            format!("inflow per m = qi·W = {qin:.4} m³/day/m; layer capacity k·S_R·H = {cap:.4} m³/day/m"),
            format!("resultant slope S_R = {sr:.4}, flow path L_R = {lr:.2} m, t50 = ne·L_R²/(2k(H+L_R·S_R)) = {t50:.2} h (approximation)"),
            format!("edge drain D{:.0} mm full capacity {qp:.5} m³/s; outlet spacing = min(Qp/inflow, max) = {spacing:.0} m", inp.pipe_d_m * 1000.0)] }
}
