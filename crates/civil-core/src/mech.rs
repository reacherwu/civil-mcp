//! Mechanistic check of a flexible pavement at concept depth: Odemark equivalent thickness with
//! Boussinesq centre-line strains under one circular wheel, Austroads AGPT02 transfer functions.
//! This is an approximation of a multilayer linear-elastic analysis (CIRCLY / AASHTOWare ME are
//! not reproduced); results are indicative and must be confirmed with the agency software.
use serde::{Deserialize, Serialize};

pub const AGPT02_MECH: &str = "Austroads AGPT02 (2017) §8.2 transfer functions: subgrade N = (9300/µε)^7.14, asphalt fatigue N = RF·[6918(0.856Vb+1.08)/(E^0.36·µε)]^5; strains by Odemark–Boussinesq approximation";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLayer {
    pub name: String,
    pub h_m: f64,
    pub e_mpa: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechInput {
    pub layers: Vec<MLayer>,
    pub subgrade_e_mpa: f64,
    #[serde(default = "d_p")]
    pub tyre_kpa: f64,
    #[serde(default = "d_w")]
    pub wheel_kn: f64,
    #[serde(default = "d_nu")]
    pub nu: f64,
    /// asphalt binder volume Vb (%) and reliability factor RF (engineer, AGPT02 Table 6.13)
    pub vb_pct: f64,
    pub rf: f64,
    #[serde(default = "d_f")]
    pub odemark_f: f64,
    pub design_esa: f64,
}
fn d_p() -> f64 {
    750.0
}
fn d_w() -> f64 {
    20.0
}
fn d_nu() -> f64 {
    0.35
}
fn d_f() -> f64 {
    0.9
}

#[derive(Debug, Clone, Serialize)]
pub struct MechResult {
    pub a_m: f64,
    pub he_subgrade_m: f64,
    pub eps_v_sub: f64,
    pub n_sub: f64,
    pub he_ac_m: f64,
    pub eps_t_ac: f64,
    pub n_ac: f64,
    pub design_esa: f64,
    pub pass: bool,
    pub steps: Vec<String>,
}

/// Boussinesq centre-line vertical strain at depth z under uniform circular load p (kPa), radius a, modulus E (MPa)
pub fn eps_z(p: f64, a: f64, z: f64, e: f64, nu: f64) -> f64 {
    let r = (a * a + z * z).sqrt();
    (1.0 + nu) * p / (e * 1000.0) * ((1.0 - 2.0 * nu) + 2.0 * nu * z / r - (z / r).powi(3))
}
/// centre-line radial (horizontal) strain, tension positive
pub fn eps_r(p: f64, a: f64, z: f64, e: f64, nu: f64) -> f64 {
    let r = (a * a + z * z).sqrt();
    -(1.0 + nu) * p / (2.0 * e * 1000.0) * ((1.0 - 2.0 * nu) - 2.0 * (1.0 - nu) * z / r + (z / r).powi(3))
}

pub fn analyse(m: &MechInput) -> MechResult {
    let a = (m.wheel_kn / (std::f64::consts::PI * m.tyre_kpa)).sqrt();
    let he = m.odemark_f * m.layers.iter().map(|l| l.h_m * (l.e_mpa / m.subgrade_e_mpa).powf(1.0 / 3.0)).sum::<f64>();
    let ev = eps_z(m.tyre_kpa, a, he, m.subgrade_e_mpa, m.nu) * 1e6;
    let n_sub = (9300.0 / ev).powf(7.14);
    let ac = &m.layers[0];
    // bottom of asphalt: transform asphalt to the modulus of the layer below (2-layer Odemark)
    let e2 = m.layers.get(1).map_or(m.subgrade_e_mpa, |l| l.e_mpa);
    let he1 = m.odemark_f * ac.h_m * (ac.e_mpa / e2).powf(1.0 / 3.0);
    let et = eps_r(m.tyre_kpa, a, he1, e2, m.nu).max(1e-9) * 1e6; // bonded interface: horizontal strain continuous, so εt = strain at depth he1 in the E2 half-space
    let n_ac = m.rf * (6918.0 * (0.856 * m.vb_pct + 1.08) / (ac.e_mpa.powf(0.36) * et)).powi(5);
    let pass = n_sub >= m.design_esa && n_ac >= m.design_esa;
    MechResult {
        a_m: a,
        he_subgrade_m: he,
        eps_v_sub: ev,
        n_sub,
        he_ac_m: he1,
        eps_t_ac: et,
        n_ac,
        design_esa: m.design_esa,
        pass,
        steps: vec![
            format!("load radius a = √(P/(π·p)) = {a:.4} m"),
            format!("subgrade: he = f·Σh(Ei/Esg)^(1/3) = {he:.3} m, εv = {ev:.0} µε, N = (9300/εv)^7.14 = {n_sub:.3e}"),
            format!("asphalt: he1 = {he1:.3} m, εt ≈ {et:.0} µε, N = RF·[6918(0.856Vb+1.08)/(E^0.36·εt)]^5 = {n_ac:.3e}"),
        ],
    }
}
