//! Cantilever wall stem flexural reinforcement at concept depth: factored moment at the stem base
//! from Rankine pressure and surcharge, rectangular stress block As (AASHTO LRFD §5.6.3 / ACI 318
//! form Mu = φ·As·fy·(d − a/2), a = As·fy/(0.85·f'c·b)), minimum steel by engineer ratio.
use serde::{Deserialize, Serialize};

pub const LRFD_FLEX: &str = "AASHTO LRFD BDS §5.6.3 (rectangular stress block); load factor for EH per §3.4.1 (engineer); bar sizes per agency";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebarInput {
    pub fc_mpa: f64,
    pub fy_mpa: f64,
    #[serde(default = "d_phi")]
    pub phi: f64,
    pub load_factor: f64,
    pub cover_m: f64,
    pub bar_d_mm: f64,
    pub min_ratio: f64,
    pub gamma_knm3: f64,
    pub phi_soil_deg: f64,
    #[serde(default)]
    pub surcharge_kpa: f64,
    #[serde(default)]
    pub source: String,
}
fn d_phi() -> f64 { 0.9 }

#[derive(Debug, Clone, Serialize)]
pub struct RebarResult { pub station: f64, pub side: String, pub stem_h_m: f64, pub t_m: f64, pub mu_knm: f64, pub d_m: f64, pub as_req_mm2: f64, pub as_min_mm2: f64, pub as_mm2: f64, pub spacing_mm: f64, pub ok: bool, pub note: String }

pub fn stem(inp: &RebarInput, hs: f64, t: f64) -> (f64, f64, f64, f64, f64, f64, bool) {
    let ka = (std::f64::consts::FRAC_PI_4 - inp.phi_soil_deg.to_radians() / 2.0).tan().powi(2);
    let m = ka * inp.gamma_knm3 * hs.powi(3) / 6.0 + ka * inp.surcharge_kpa * hs * hs / 2.0;
    let mu = inp.load_factor * m; // kN·m per m
    let d = t - inp.cover_m - inp.bar_d_mm / 2000.0;
    let b = 1.0;
    // Mu = φ As fy (d − As fy /(1.7 f'c b)) → quadratic in As (m²)
    let (fy, fc) = (inp.fy_mpa * 1000.0, inp.fc_mpa * 1000.0);
    let qa = inp.phi * fy * fy / (1.7 * fc * b);
    let qb = -inp.phi * fy * d;
    let disc = qb * qb - 4.0 * qa * mu;
    let ok = disc >= 0.0 && d > 0.0;
    let as_req = if ok { (-qb - disc.sqrt()) / (2.0 * qa) } else { f64::NAN };
    let as_min = inp.min_ratio * b * t;
    let as_ = as_req.max(as_min);
    let ab = std::f64::consts::PI * (inp.bar_d_mm / 1000.0).powi(2) / 4.0;
    let sp = (ab / as_ * 1000.0 / 5.0).floor() * 5.0 * 1000.0 / 1000.0;
    (mu, d, as_req * 1e6, as_min * 1e6, as_ * 1e6, sp.min(450.0), ok)
}
