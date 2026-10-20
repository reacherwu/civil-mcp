//! AU/NZ rigid (concrete) pavement thickness at concept depth, following the structure of Austroads
//! AGPT02 (2017) Chapter 9: base type, subbase, effective subgrade support, load safety factor (LSF),
//! base thickness by trial so that cumulative flexural fatigue and erosion damage over the axle-group
//! spectrum are ≤ 100 %. AGPT02's equivalent-stress and erosion-factor tables are copyrighted and not
//! shipped: slab stresses are computed here with Westergaard edge loading (Ioannides et al. 1985 form)
//! and corner deflection, fatigue with the stress-ratio relation used in AGPT02 / PCA (1984), erosion
//! with the PCA (1984) power-based relation. Values are indicative; the design must be confirmed with
//! AGPT02 tables (or the agency's procedure) by the pavement engineer.
use serde::{Deserialize, Serialize};

pub const REF: &str = "Austroads AGPT02 (2017) Ch. 9 procedure structure; fatigue Sr relation as AGPT02/PCA (1984); Westergaard edge stress (Ioannides, Thompson & Barenberg 1985); PCA (1984) erosion";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub load_kn: f64,
    pub tyres: f64,
    pub repetitions: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigidInput {
    pub groups: Vec<Group>,
    /// effective modulus of subgrade reaction on top of the subbase (MPa/m), from effective CBR (engineer)
    pub k_mpa_m: f64,
    pub fcf_mpa: f64,
    #[serde(default = "d_e")]
    pub e_mpa: f64,
    #[serde(default = "d_nu")]
    pub nu: f64,
    pub lsf: f64,
    #[serde(default = "d_p")]
    pub tyre_kpa: f64,
    /// tied concrete shoulder / integral kerb (true) reduces erosion: C2 = 0.94 vs 0.06 (PCA)
    #[serde(default)]
    pub tied_shoulder: bool,
    #[serde(default = "d_h0")]
    pub h_min_mm: f64,
    #[serde(default = "d_h1")]
    pub h_max_mm: f64,
    pub base_type: String,
    #[serde(default)]
    pub source: String,
}
fn d_e() -> f64 {
    30000.0
}
fn d_nu() -> f64 {
    0.15
}
fn d_p() -> f64 {
    750.0
}
fn d_h0() -> f64 {
    150.0
}
fn d_h1() -> f64 {
    350.0
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupResult {
    pub name: String,
    pub p_n: f64,
    pub a_mm: f64,
    pub stress_mpa: f64,
    pub sr: f64,
    pub nf: f64,
    pub fatigue_pct: f64,
    pub corner_w_mm: f64,
    pub power: f64,
    pub ne: f64,
    pub erosion_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RigidResult {
    pub h_mm: f64,
    pub fatigue_pct: f64,
    pub erosion_pct: f64,
    pub pass: bool,
    pub groups: Vec<GroupResult>,
    pub trials: Vec<(f64, f64, f64)>,
}

pub fn nf(sr: f64) -> f64 {
    if sr > 0.55 {
        10f64.powf((0.9719 - sr) / 0.0828)
    } else if sr > 0.45 {
        (4.258 / (sr - 0.4325)).powf(3.268)
    } else {
        f64::INFINITY
    }
}

pub fn evaluate(r: &RigidInput, h: f64) -> (f64, f64, Vec<GroupResult>) {
    let k = r.k_mpa_m / 1000.0; // N/mm³
    let l = (r.e_mpa * h.powi(3) / (12.0 * (1.0 - r.nu * r.nu) * k)).powf(0.25);
    let c2 = if r.tied_shoulder { 0.94 } else { 0.06 };
    let (mut fd, mut ed) = (0.0, 0.0);
    let mut out = vec![];
    for g in &r.groups {
        let p = g.load_kn * 1000.0 * r.lsf / g.tyres;
        let a = (p / (std::f64::consts::PI * r.tyre_kpa / 1000.0)).sqrt();
        let nu = r.nu;
        let s = 3.0 * (1.0 + nu) * p / (std::f64::consts::PI * (3.0 + nu) * h * h)
            * ((r.e_mpa * h.powi(3) / (100.0 * k * a.powi(4))).ln() + 1.84 - 4.0 * nu / 3.0 + (1.0 - nu) / 2.0 + 1.18 * (1.0 + 2.0 * nu) * a / l);
        let sr = s / r.fcf_mpa;
        let n_f = nf(sr);
        let w = p / (k * l * l) * (1.1 - 0.88 * (a * std::f64::consts::SQRT_2 / l));
        let pc_psi = k * w * 145.0377;
        let (h_in, k_pci) = (h / 25.4, r.k_mpa_m * 3.684);
        let power = 268.7 * pc_psi * pc_psi / (h_in * k_pci.powf(0.73));
        let c1 = 1.0 - (k_pci / 2000.0 * 4.0 / h_in).powi(2);
        let ne = if c1 * power > 9.0 { 10f64.powf(14.524 - 6.777 * (c1 * power - 9.0).powf(0.103)) } else { f64::INFINITY };
        let fp = 100.0 * g.repetitions / n_f;
        let ep = 100.0 * c2 * g.repetitions / ne;
        fd += fp;
        ed += ep;
        out.push(GroupResult {
            name: g.name.clone(),
            p_n: p,
            a_mm: a,
            stress_mpa: s,
            sr,
            nf: n_f,
            fatigue_pct: fp,
            corner_w_mm: w,
            power,
            ne,
            erosion_pct: ep,
        });
    }
    (fd, ed, out)
}

pub fn design(r: &RigidInput) -> RigidResult {
    let mut h = r.h_min_mm;
    let mut trials = vec![];
    loop {
        let (f, e, g) = evaluate(r, h);
        trials.push((h, f, e));
        let pass = f <= 100.0 && e <= 100.0;
        if pass || h + 5.0 > r.h_max_mm {
            return RigidResult { h_mm: h, fatigue_pct: f, erosion_pct: e, pass, groups: g, trials };
        }
        h += 5.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fatigue_relation_and_monotonic_design() {
        assert!((nf(0.6) - 10f64.powf((0.9719 - 0.6) / 0.0828)).abs() < 1e-6);
        assert!(nf(0.44).is_infinite());
        let r = RigidInput {
            groups: vec![Group { name: "SADT 80 kN".into(), load_kn: 80.0, tyres: 4.0, repetitions: 5e6 }],
            k_mpa_m: 60.0,
            fcf_mpa: 4.5,
            e_mpa: 30000.0,
            nu: 0.15,
            lsf: 1.2,
            tyre_kpa: 750.0,
            tied_shoulder: true,
            h_min_mm: 150.0,
            h_max_mm: 350.0,
            base_type: "JRCP".into(),
            source: String::new(),
        };
        let (f1, _, _) = evaluate(&r, 180.0);
        let (f2, _, _) = evaluate(&r, 220.0);
        assert!(f2 < f1);
        assert!(design(&r).pass);
    }
}
