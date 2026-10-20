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
    /// bars per bundle (1 = single bars; bundles up to the agency limit, e.g. AASHTO LRFD 5.10.3.1.5)
    #[serde(default = "d_one")]
    pub bars_per_bundle: f64,
}
fn d_phi() -> f64 {
    0.9
}
fn d_one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize)]
pub struct RebarResult {
    pub station: f64,
    pub side: String,
    pub stem_h_m: f64,
    pub t_m: f64,
    pub mu_knm: f64,
    pub d_m: f64,
    pub as_req_mm2: f64,
    pub as_min_mm2: f64,
    pub as_mm2: f64,
    pub spacing_mm: f64,
    pub ok: bool,
    pub note: String,
    /// stem base shear: factored Vu and φVc (kN/m)
    pub vu_kn: f64,
    pub phi_vc_kn: f64,
    /// crack control: maximum bar spacing (mm) by AASHTO LRFD 5.6.7 at service
    pub s_crack_mm: f64,
    /// footing heel / toe factored moments (kNm/m), required steel (mm²/m) and bar spacing (mm)
    pub mu_heel: f64,
    pub mu_toe: f64,
    pub as_heel_mm2: f64,
    pub as_toe_mm2: f64,
    pub sp_heel_mm: f64,
    pub sp_toe_mm: f64,
}

pub const LRFD_SHEAR: &str = "AASHTO LRFD BDS §5.7.3.3 simplified (β = 2, θ = 45°: Vc = 0.083·β·√f'c·b·dv, φ = 0.9) — verify against the agency edition";
pub const LRFD_CRACK: &str = "AASHTO LRFD BDS §5.6.7: s ≤ 123000·γe/(βs·fss) − 2dc (SI), βs = 1 + dc/(0.7(h − dc)), fss = Ms/(As·j·d), j = 0.875";

/// steel area for factored moment mu on depth d (quadratic of the rectangular stress block); (As m², spacing mm)
fn as_for(inp: &RebarInput, mu: f64, d: f64, t: f64) -> (f64, f64) {
    let (fy, fc) = (inp.fy_mpa * 1000.0, inp.fc_mpa * 1000.0);
    let qa = inp.phi * fy * fy / (1.7 * fc);
    let qb = -inp.phi * fy * d;
    let disc = qb * qb - 4.0 * qa * mu.max(0.0);
    let as_req = if disc >= 0.0 { (-qb - disc.sqrt()) / (2.0 * qa) } else { f64::NAN };
    let as_ = as_req.max(inp.min_ratio * t);
    let ab = inp.bars_per_bundle.max(1.0) * std::f64::consts::PI * (inp.bar_d_mm / 1000.0).powi(2) / 4.0;
    (as_, ((ab / as_ * 1000.0 / 5.0).floor() * 5.0).min(450.0))
}

/// stem shear at the base (kN/m): (Vu, φVc)
pub fn stem_shear(inp: &RebarInput, hs: f64, d: f64) -> (f64, f64) {
    let ka = (std::f64::consts::FRAC_PI_4 - inp.phi_soil_deg.to_radians() / 2.0).tan().powi(2);
    let v = 0.5 * ka * inp.gamma_knm3 * hs * hs + ka * inp.surcharge_kpa * hs;
    let dv = (0.9 * d).max(0.72 * (d + inp.cover_m + inp.bar_d_mm / 2000.0));
    (inp.load_factor * v, 0.9 * 0.083 * 2.0 * inp.fc_mpa.sqrt() * 1000.0 * dv)
}

/// maximum bar spacing for crack control (mm), class 2 exposure γe = 0.75 unless the engineer sets crack_gamma_e
pub fn crack_spacing(inp: &RebarInput, ms_knm: f64, as_m2: f64, d: f64, t: f64, gamma_e: f64) -> f64 {
    let dc = (t - d) * 1000.0;
    let h = t * 1000.0;
    let fss = (ms_knm / (as_m2 * 0.875 * d) / 1000.0).min(0.6 * inp.fy_mpa);
    let bs = 1.0 + dc / (0.7 * (h - dc));
    123000.0 * gamma_e / (bs * fss) - 2.0 * dc
}

/// footing heel and toe flexure from the linear (or triangular) bearing distribution of the wall analysis
pub fn footing(inp: &RebarInput, w: &crate::walls::WallDesign) -> (f64, f64, f64, f64, f64, f64) {
    let (b, e, v, t) = (w.base_b, w.eccentricity, w.sum_v_kn, w.base_t);
    let q = |x: f64| {
        if e <= b / 6.0 {
            v / b + 6.0 * v * e / (b * b) - 12.0 * v * e * x / b.powi(3)
        } else {
            let l = 3.0 * (b / 2.0 - e);
            (2.0 * v / l * (1.0 - x / l)).max(0.0)
        }
    };
    let hs = w.h - t;
    let wd = inp.gamma_knm3 * hs + inp.surcharge_kpa + crate::walls::GAMMA_C * t;
    let n = 400;
    let x0 = b - w.heel.max(0.0);
    let (mut mh, mut mt) = (0.0, 0.0);
    for k in 0..n {
        let x = x0 + (k as f64 + 0.5) * w.heel.max(0.0) / n as f64;
        mh += (wd - q(x)) * (x - x0) * w.heel.max(0.0) / n as f64;
        let xt = (k as f64 + 0.5) * w.toe / n as f64;
        mt += (q(xt) - crate::walls::GAMMA_C * t) * (w.toe - xt) * w.toe / n as f64;
    }
    let (mh, mt) = (inp.load_factor * mh.max(0.0), inp.load_factor * mt.max(0.0));
    let d = t - inp.cover_m - inp.bar_d_mm / 2000.0;
    let (ah, sh) = as_for(inp, mh, d, t);
    let (at, st) = as_for(inp, mt, d, t);
    (mh, mt, ah * 1e6, at * 1e6, sh, st)
}

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
    let ab = inp.bars_per_bundle.max(1.0) * std::f64::consts::PI * (inp.bar_d_mm / 1000.0).powi(2) / 4.0;
    let sp = (ab / as_ * 1000.0 / 5.0).floor() * 5.0 * 1000.0 / 1000.0;
    (mu, d, as_req * 1e6, as_min * 1e6, as_ * 1e6, sp.min(450.0), ok)
}
