//! Mechanistic-empirical flexible pavement design: multilayer linear-elastic analysis (crate::lea,
//! verified against Boussinesq and a published KENLAYER table) under a dual-wheel standard axle half,
//! critical strains at the bottom of asphalt and top of subgrade, and transfer functions
//! (Austroads AGPT02 or Asphalt Institute MS-1). Optional asphalt thickness design by stepping.
//! The Odemark–Boussinesq screening values are kept for comparison. Isotropic linear layers only
//! (no cross-anisotropy, no CIRCLY damage summation over a load spectrum, no AASHTOWare ME climate
//! models); confirm with the agency software.
use serde::{Deserialize, Serialize};

pub const AGPT02_MECH: &str = "Austroads AGPT02 (2017) §8.2 transfer functions: subgrade N = (9300/µε)^7.14, asphalt fatigue N = RF·[6918(0.856Vb+1.08)/(E^0.36·µε)]^5; strains by multilayer linear-elastic analysis";
pub const AI_MECH: &str = "Asphalt Institute MS-1 (1982/1991): Nf = 0.0796·εt^-3.291·E(psi)^-0.854 (C = 1), Nd = 1.365e-9·εc^-4.477";

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
    /// "agpt02" (default) or "asphalt_institute"
    #[serde(default)]
    pub criteria: String,
    /// dual-tyre centre spacing (m); 0 = single wheel. Austroads standard axle: 0.33 m
    #[serde(default = "d_s")]
    pub dual_spacing_m: f64,
    /// subgrade Poisson's ratio
    #[serde(default = "d_nus")]
    pub nu_subgrade: f64,
    /// step the asphalt (first layer) thickness up to this value (m) until both criteria pass
    #[serde(default)]
    pub design_ac_max_m: Option<f64>,
}
fn d_s() -> f64 {
    0.33
}
fn d_nus() -> f64 {
    0.45
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
    pub method: String,
    pub h_ac_m: f64,
    pub odemark_eps_t: f64,
    pub odemark_eps_v: f64,
    pub points: Vec<(String, f64, f64, f64, f64)>,
    pub iterations: Vec<(f64, f64, f64)>,
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

/// strains from the layered analysis: (εt max at bottom of layer 1, εv max at top of subgrade, point table)
pub fn lea_strains(m: &MechInput, h_ac: f64) -> (f64, f64, Vec<(String, f64, f64, f64, f64)>) {
    use crate::lea::{multi, LeaLayer};
    let a = (m.wheel_kn / (std::f64::consts::PI * m.tyre_kpa)).sqrt();
    let mut ls: Vec<LeaLayer> = m.layers.iter().map(|l| LeaLayer { h: l.h_m, e: l.e_mpa * 1000.0, nu: m.nu }).collect();
    ls[0].h = h_ac;
    ls.push(LeaLayer { h: 0.0, e: m.subgrade_e_mpa * 1000.0, nu: m.nu_subgrade });
    let loads: Vec<[f64; 2]> = if m.dual_spacing_m > 0.0 { vec![[-m.dual_spacing_m / 2.0, 0.0], [m.dual_spacing_m / 2.0, 0.0]] } else { vec![[0.0, 0.0]] };
    let xs: Vec<(String, f64)> = if m.dual_spacing_m > 0.0 {
        vec![("under wheel".into(), m.dual_spacing_m / 2.0), ("wheel edge".into(), m.dual_spacing_m / 2.0 - a), ("between wheels".into(), 0.0)]
    } else {
        vec![("under wheel".into(), 0.0)]
    };
    let z_ac = h_ac;
    let z_sg = ls[..ls.len() - 1].iter().map(|l| l.h).sum::<f64>() + 1e-6;
    let (mut et, mut ev) = (0.0f64, 0.0f64);
    let mut pts = vec![];
    for (n, x) in xs {
        let (ex, ey, _, _) = multi(&ls, m.tyre_kpa, a, &loads, [x, 0.0], z_ac);
        let (_, _, ez, w) = multi(&ls, m.tyre_kpa, a, &loads, [x, 0.0], z_sg);
        et = et.max(ex.max(ey));
        ev = ev.max(-ez);
        pts.push((n, x, ex.max(ey) * 1e6, -ez * 1e6, w * 1000.0));
    }
    (et, ev, pts)
}

/// transfer functions: (N subgrade, N asphalt)
pub fn transfer(m: &MechInput, et: f64, ev: f64) -> (f64, f64) {
    let e_ac = m.layers[0].e_mpa;
    if m.criteria == "asphalt_institute" {
        (1.365e-9 * ev.powf(-4.477), 0.0796 * et.max(1e-9).powf(-3.291) * (e_ac * 145.0377).powf(-0.854))
    } else {
        ((9300.0 / (ev * 1e6)).powf(7.14), m.rf * (6918.0 * (0.856 * m.vb_pct + 1.08) / (e_ac.powf(0.36) * (et * 1e6).max(1e-3))).powi(5))
    }
}

pub fn analyse(m: &MechInput) -> MechResult {
    let a = (m.wheel_kn / (std::f64::consts::PI * m.tyre_kpa)).sqrt();
    // Odemark–Boussinesq screening (single wheel), kept for comparison
    let he = m.odemark_f * m.layers.iter().map(|l| l.h_m * (l.e_mpa / m.subgrade_e_mpa).powf(1.0 / 3.0)).sum::<f64>();
    let ev0 = eps_z(m.tyre_kpa, a, he, m.subgrade_e_mpa, m.nu);
    let ac = &m.layers[0];
    let e2 = m.layers.get(1).map_or(m.subgrade_e_mpa, |l| l.e_mpa);
    let he1 = m.odemark_f * ac.h_m * (ac.e_mpa / e2).powf(1.0 / 3.0);
    let et0 = eps_r(m.tyre_kpa, a, he1, e2, m.nu).max(1e-9);
    // layered analysis, optional thickness design
    let mut h = ac.h_m;
    let mut its = vec![];
    let (mut et, mut ev, mut pts);
    loop {
        (et, ev, pts) = lea_strains(m, h);
        let (ns, na) = transfer(m, et, ev);
        its.push((h, ns, na));
        let ok = ns >= m.design_esa && na >= m.design_esa;
        match m.design_ac_max_m {
            Some(hm) if !ok && h + 0.01 <= hm + 1e-9 => h = ((h + 0.01) * 1000.0).round() / 1000.0,
            _ => break,
        }
    }
    let (n_sub, n_ac) = transfer(m, et, ev);
    let pass = n_sub >= m.design_esa && n_ac >= m.design_esa;
    let method = if m.criteria == "asphalt_institute" { AI_MECH } else { AGPT02_MECH };
    MechResult {
        method: method.into(),
        h_ac_m: h,
        odemark_eps_t: et0 * 1e6,
        odemark_eps_v: ev0 * 1e6,
        points: pts,
        iterations: its,
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
            format!("load radius a = √(P/(π·p)) = {a:.4} m; dual spacing {:.3} m", m.dual_spacing_m),
            format!(
                "layered elastic ({}): asphalt h = {h:.3} m, εt = {:.1} µε (bottom of asphalt), εv = {:.1} µε (top of subgrade)",
                crate::lea::LEA_REF,
                et * 1e6,
                ev * 1e6
            ),
            format!("N subgrade = {n_sub:.3e}, N asphalt = {n_ac:.3e} vs design {:.3e} ({method})", m.design_esa),
            format!("Odemark–Boussinesq screening for comparison: εt ≈ {:.0} µε, εv ≈ {:.0} µε", et0 * 1e6, ev0 * 1e6),
        ],
    }
}
