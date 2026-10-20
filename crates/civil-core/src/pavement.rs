//! Pavement thickness design, one method per standards profile.
//!
//! - US / Canada: AASHTO Guide for Design of Pavement Structures (1993), flexible (structural number SN,
//!   layered analysis) and rigid (slab thickness D) design equations. TAC's Pavement Asset Design and
//!   Management Guide accepts AASHTO 1993 and AASHTOWare Pavement ME; the ME method is not implemented.
//! - Australia / New Zealand: Austroads AGPT02 empirical design chart for granular pavements with thin
//!   bituminous surfacings (Figure 8.4 equation). The mechanistic-empirical (CIRCLY) method is not implemented.
//! - UK: DMRB CD 226. Equation 2.24 (asphalt over an HBGM base) and Equations 2.43a/b (URC rigid) are shipped.
//!   Asphalt-base thickness and HBGM thickness come from the Figure 2.20 nomograph, which the engineer enters.
use crate::standards::Standard;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSpec {
    pub name: String,
    /// AASHTO layer coefficient a_i (per inch), engineer input
    #[serde(default)]
    pub a: f64,
    /// AASHTO drainage coefficient m_i (1.0 for surface course)
    #[serde(default = "one")]
    pub m: f64,
    /// resilient modulus of the layer (MPa), used to find SN required above it
    #[serde(default)]
    pub modulus_mpa: f64,
    /// minimum thickness (mm), engineer input (e.g. agency minimums)
    #[serde(default)]
    pub min_mm: f64,
}
fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PavementInput {
    /// "flexible" | "rigid"
    pub kind: String,
    /// design subgrade CBR (%)
    pub subgrade_cbr: f64,
    /// subgrade resilient modulus (MPa). US/CA: if absent, Mr(psi) = 1500·CBR (AASHTO 1993 Part I correlation; engineer to confirm)
    #[serde(default)]
    pub subgrade_mr_mpa: Option<f64>,
    /// design traffic in the profile's unit: ESAL (US/CA, W18), DESA (AU/NZ), msa (UK)
    pub design_traffic: f64,
    // --- AASHTO 1993 ---
    #[serde(default)]
    pub reliability_zr: Option<f64>,
    #[serde(default)]
    pub s0: Option<f64>,
    #[serde(default)]
    pub p0: Option<f64>,
    #[serde(default)]
    pub pt: Option<f64>,
    /// flexible: layers top to bottom (surface, base, subbase)
    #[serde(default)]
    pub layers: Vec<LayerSpec>,
    /// rigid: concrete modulus of rupture S'c (MPa), modulus Ec (MPa), load transfer J, drainage Cd, k-value (MPa/m)
    #[serde(default)]
    pub sc_mpa: Option<f64>,
    #[serde(default)]
    pub ec_mpa: Option<f64>,
    #[serde(default)]
    pub j: Option<f64>,
    #[serde(default)]
    pub cd: Option<f64>,
    #[serde(default)]
    pub k_mpa_m: Option<f64>,
    // --- Austroads AGPT02 empirical ---
    /// thickness of asphalt/seal surfacing (mm) and granular base (mm) chosen by the engineer
    #[serde(default)]
    pub surfacing_mm: Option<f64>,
    #[serde(default)]
    pub base_mm: Option<f64>,
    // --- DMRB CD 226 ---
    /// foundation class 1-4 (CD 225)
    #[serde(default)]
    pub foundation_class: Option<u8>,
    /// "asphalt" | "hbgm" | "urc"
    #[serde(default)]
    pub uk_type: Option<String>,
    /// asphalt-base design thickness read from CD 226 Figure 2.20 by the engineer (mm)
    #[serde(default)]
    pub uk_chart_asphalt_mm: Option<f64>,
    /// HBGM base thickness read from Figure 2.20 by the engineer (mm)
    #[serde(default)]
    pub uk_chart_hbgm_mm: Option<f64>,
    /// URC: mean 28-day cube strength Rc (MPa); tied shoulder / 1 m edge strip
    #[serde(default)]
    pub uk_rc_mpa: Option<f64>,
    #[serde(default)]
    pub uk_tied_shoulder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub thickness_mm: f64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PavementDesign {
    pub method: String,
    pub citation: String,
    pub kind: String,
    pub inputs: serde_json::Value,
    pub layers: Vec<Layer>,
    pub total_mm: f64,
    pub sn_required: Option<f64>,
    pub sn_provided: Option<f64>,
    pub steps: Vec<String>,
    pub engineer_inputs: Vec<String>,
    pub warnings: Vec<String>,
}

pub const AASHTO93: &str = "AASHTO Guide for Design of Pavement Structures (1993), Part II Ch. 3";
pub const AGPT02: &str = "Austroads AGPT02 Guide to Pavement Technology Part 2: Pavement Structural Design, empirical design of granular pavements with thin bituminous surfacing (Figure 8.4)";
pub const CD226: &str = "DMRB CD 226 Design for new pavement construction (Version 0.1.0, 2021)";
const PSI_PER_MPA: f64 = 145.0377;
const IN: f64 = 25.4;

/// AASHTO 1993 flexible design equation: log10(W18) for a given SN, Mr in psi.
pub fn aashto_flex_log_w18(sn: f64, zr: f64, s0: f64, dpsi: f64, mr_psi: f64) -> f64 {
    zr * s0 + 9.36 * (sn + 1.0).log10() - 0.20 + (dpsi / (4.2 - 1.5)).log10() / (0.40 + 1094.0 / (sn + 1.0).powf(5.19)) + 2.32 * mr_psi.log10() - 8.07
}
/// SN required to carry W18 (bisection).
pub fn aashto_flex_sn(w18: f64, zr: f64, s0: f64, dpsi: f64, mr_psi: f64) -> f64 {
    let t = w18.log10();
    let (mut lo, mut hi) = (0.01, 30.0);
    for _ in 0..200 {
        let m = (lo + hi) / 2.0;
        if aashto_flex_log_w18(m, zr, s0, dpsi, mr_psi) < t {
            lo = m
        } else {
            hi = m
        }
    }
    (lo + hi) / 2.0
}
/// AASHTO 1993 rigid design equation (US units: D in, Sc psi, Ec psi, k pci).
pub fn aashto_rigid_log_w18(d: f64, zr: f64, s0: f64, dpsi: f64, pt: f64, sc: f64, cd: f64, j: f64, ec: f64, k: f64) -> f64 {
    let num = sc * cd * (d.powf(0.75) - 1.132);
    let den = 215.63 * j * (d.powf(0.75) - 18.42 / (ec / k).powf(0.25));
    zr * s0 + 7.35 * (d + 1.0).log10() - 0.06 + (dpsi / (4.5 - 1.5)).log10() / (1.0 + 1.624e7 / (d + 1.0).powf(8.46)) + (4.22 - 0.32 * pt) * (num / den).log10()
}
pub fn aashto_rigid_d(w18: f64, zr: f64, s0: f64, dpsi: f64, pt: f64, sc: f64, cd: f64, j: f64, ec: f64, k: f64) -> f64 {
    let t = w18.log10();
    let (mut lo, mut hi) = (4.0, 30.0);
    for _ in 0..200 {
        let m = (lo + hi) / 2.0;
        if aashto_rigid_log_w18(m, zr, s0, dpsi, pt, sc, cd, j, ec, k) < t {
            lo = m
        } else {
            hi = m
        }
    }
    (lo + hi) / 2.0
}
/// Austroads AGPT02 Figure 8.4: total granular thickness over subgrade (mm).
pub fn agpt02_granular_mm(cbr: f64, desa: f64) -> f64 {
    let l = cbr.log10();
    (219.0 - 211.0 * l + 58.0 * l * l) * (desa / 120.0).log10()
}
/// CD 226 Eq 2.24: total asphalt thickness over an HBGM base (mm), T in msa (≥ 80 msa → 180 mm).
pub fn cd226_hbgm_asphalt_mm(t_msa: f64) -> f64 {
    if t_msa >= 80.0 {
        return 180.0;
    }
    let l = t_msa.log10();
    -16.05 * l * l + 101.0 * l + 45.8
}
/// CD 226 Eq 2.43a/b: URC slab thickness (mm); E = 200 (FC3) or 400 (FC4) MPa.
pub fn cd226_urc_mm(t_msa: f64, rc: f64, e: f64, tied: bool) -> f64 {
    let h1 = ((t_msa.ln() - 3.466 * rc.ln() - 0.484 * e.ln() + 40.483) / 5.094).exp();
    if tied {
        0.934 * h1 - 12.5
    } else {
        h1
    }
}
fn up(x: f64, step: f64) -> f64 {
    (x / step - 1e-9).ceil() * step
}

pub fn design(std: Standard, p: &PavementInput) -> crate::Result<PavementDesign> {
    let mut steps = Vec::new();
    let mut eng = Vec::new();
    let mut warn = Vec::new();
    let mut layers = Vec::new();
    let req = |v: Option<f64>, n: &str| v.ok_or_else(|| format!("pavement: '{n}' is an engineer input for this method"));
    if !(p.subgrade_cbr > 0.0) || !(p.design_traffic > 0.0) {
        return Err("pavement: subgrade_cbr and design_traffic must be > 0".into());
    }
    let inputs = serde_json::to_value(p).unwrap();
    match std {
        Standard::Us | Standard::Ca => {
            let zr = req(p.reliability_zr, "reliability_zr")?;
            let s0 = req(p.s0, "s0")?;
            let (p0, pt) = (req(p.p0, "p0")?, req(p.pt, "pt")?);
            eng.extend(["reliability ZR", "overall standard deviation S0", "initial/terminal serviceability p0/pt"].map(String::from));
            let dpsi = p0 - pt;
            let w18 = p.design_traffic;
            if p.kind == "rigid" {
                let (sc, ec, j, cd, k) = (req(p.sc_mpa, "sc_mpa")?, req(p.ec_mpa, "ec_mpa")?, req(p.j, "j")?, req(p.cd, "cd")?, req(p.k_mpa_m, "k_mpa_m")?);
                eng.extend(["S'c", "Ec", "J", "Cd", "effective k"].map(String::from));
                let k_pci = k * 3.684;
                let d = aashto_rigid_d(w18, zr, s0, dpsi, pt, sc * PSI_PER_MPA, cd, j, ec * PSI_PER_MPA, k_pci);
                steps.push(format!("rigid: solve log10(W18) = ZR·S0 + 7.35·log10(D+1) − 0.06 + log10(ΔPSI/3)/(1 + 1.624e7/(D+1)^8.46) + (4.22 − 0.32pt)·log10[S'c·Cd·(D^0.75 − 1.132)/(215.63·J·(D^0.75 − 18.42/(Ec/k)^0.25))] → D = {d:.2} in"));
                let dmm = up(d * IN, 10.0);
                layers.push(Layer { name: "PCC slab".into(), thickness_mm: dmm, note: format!("D = {d:.2} in, rounded up to 10 mm") });
                for l in &p.layers {
                    if l.min_mm > 0.0 {
                        layers.push(Layer { name: l.name.clone(), thickness_mm: l.min_mm, note: "engineer-specified (subbase/drainage layer)".into() });
                    }
                }
                let total = layers.iter().map(|l| l.thickness_mm).sum();
                return Ok(PavementDesign {
                    method: "AASHTO 1993 rigid (JPCP) design equation".into(),
                    citation: format!(
                        "{AASHTO93} (rigid design equation){}",
                        if std == Standard::Ca { "; TAC Pavement Asset Design and Management Guide (2013) accepts AASHTO 1993" } else { "" }
                    ),
                    kind: p.kind.clone(),
                    inputs,
                    layers,
                    total_mm: total,
                    sn_required: None,
                    sn_provided: None,
                    steps,
                    engineer_inputs: eng,
                    warnings: warn,
                });
            }
            let mr_sg = p.subgrade_mr_mpa.map(|v| v * PSI_PER_MPA).unwrap_or_else(|| {
                warn.push("subgrade Mr from Mr(psi) = 1500·CBR (AASHTO 1993 correlation); confirm with tests".into());
                1500.0 * p.subgrade_cbr
            });
            if p.layers.len() < 2 {
                return Err("flexible: give layers top to bottom (surface, base[, subbase]) with a, m, modulus_mpa".into());
            }
            eng.push("layer coefficients a_i, drainage m_i, layer moduli".into());
            let sn_total = aashto_flex_sn(w18, zr, s0, dpsi, mr_sg);
            steps.push(format!("SN over subgrade (Mr = {mr_sg:.0} psi) = {sn_total:.2}"));
            // layered analysis: SN required above each layer from that layer's modulus
            let mut provided = 0.0;
            for (i, l) in p.layers.iter().enumerate() {
                let below_mr = if i + 1 < p.layers.len() { p.layers[i + 1].modulus_mpa * PSI_PER_MPA } else { mr_sg };
                let sn_req = if i + 1 < p.layers.len() { aashto_flex_sn(w18, zr, s0, dpsi, below_mr) } else { sn_total };
                let am = l.a * l.m;
                if am <= 0.0 {
                    return Err(format!("layer {}: a·m must be > 0", l.name));
                }
                let need_in = ((sn_req - provided) / am).max(0.0);
                let t = up((need_in * IN).max(l.min_mm), 5.0);
                provided += am * t / IN;
                steps.push(format!("{}: SN required above the layer below = {sn_req:.2}; D = (SN − ΣSN_above)/(a·m) = ({sn_req:.2} − {:.2})/({:.2}·{:.2}) = {need_in:.2} in → {t:.0} mm (min {} mm); SN provided so far {provided:.2}", l.name, provided - am * t / IN, l.a, l.m, l.min_mm));
                layers.push(Layer { name: l.name.clone(), thickness_mm: t, note: format!("a = {}, m = {}", l.a, l.m) });
            }
            let total = layers.iter().map(|l| l.thickness_mm).sum();
            Ok(PavementDesign {
                method: "AASHTO 1993 flexible: structural number with layered analysis".into(),
                citation: format!(
                    "{AASHTO93} (flexible design equation, layered design analysis){}",
                    if std == Standard::Ca { "; TAC Pavement Asset Design and Management Guide (2013) accepts AASHTO 1993" } else { "" }
                ),
                kind: p.kind.clone(),
                inputs,
                layers,
                total_mm: total,
                sn_required: Some(sn_total),
                sn_provided: Some(provided),
                steps,
                engineer_inputs: eng,
                warnings: warn,
            })
        }
        Standard::Au | Standard::Nz => {
            if p.kind == "rigid" {
                return Err("AU/NZ rigid pavement design (AGPT02 Chapter 9) is not implemented: engineer to design and enter layers".into());
            }
            let surf = req(p.surfacing_mm, "surfacing_mm")?;
            let base = req(p.base_mm, "base_mm")?;
            eng.extend(["surfacing thickness", "base thickness (agency minimum)", "design subgrade CBR", "DESA"].map(String::from));
            let t = agpt02_granular_mm(p.subgrade_cbr, p.design_traffic);
            steps.push(format!(
                "t = [219 − 211·log10(CBR) + 58·log10(CBR)²]·log10(DESA/120) = [219 − 211·log10({0}) + 58·log10({0})²]·log10({1:.3e}/120) = {t:.1} mm",
                p.subgrade_cbr, p.design_traffic
            ));
            if !(2.0..=30.0).contains(&p.subgrade_cbr) {
                warn.push("CBR outside the chart range; engineer to confirm".into());
            }
            let tot = up(t, 10.0);
            let sub = (tot - surf - base).max(0.0);
            layers.push(Layer {
                name: "Bituminous surfacing".into(),
                thickness_mm: surf,
                note: "engineer-specified (thin surfacing; not structural in the chart method)".into(),
            });
            layers.push(Layer { name: "Granular base".into(), thickness_mm: base, note: "engineer-specified".into() });
            if sub > 0.0 {
                layers.push(Layer { name: "Granular subbase".into(), thickness_mm: sub, note: format!("total granular {tot:.0} mm − base") });
            }
            steps.push(format!(
                "total granular thickness rounded up to 10 mm: {tot:.0} mm; subbase = {tot:.0} − {surf:.0} − {base:.0} (surfacing counted in the cover)"
            ));
            let total = layers.iter().map(|l| l.thickness_mm).sum();
            Ok(PavementDesign {
                method: "Austroads empirical design chart for granular pavements with thin bituminous surfacing".into(),
                citation: if std == Standard::Nz {
                    format!("{AGPT02}; NZTA Waka Kotahi pavement design supplement applies (engineer to confirm NZ-specific inputs)")
                } else {
                    AGPT02.into()
                },
                kind: p.kind.clone(),
                inputs,
                layers,
                total_mm: total,
                sn_required: None,
                sn_provided: None,
                steps,
                engineer_inputs: eng,
                warnings: warn,
            })
        }
        Standard::Uk => {
            let t = p.design_traffic;
            let fc = p.foundation_class.ok_or("pavement: foundation_class (CD 225) is an engineer input")?;
            eng.push("foundation class (CD 225 design)".into());
            if t < 1.0 {
                warn.push("CD 226 §2.19: minimum design traffic for new roads is 1 msa; 1 msa used".into());
            }
            let t = t.max(1.0);
            if t > 400.0 {
                return Err("CD 226: maximum design traffic is 400 msa".into());
            }
            let ty = p.uk_type.clone().unwrap_or_else(|| "asphalt".into());
            match ty.as_str() {
                "asphalt" => {
                    if (fc == 1 && t > 20.0) || (fc == 2 && t > 80.0) {
                        return Err(format!("CD 226 §2.4/2.5: foundation class {fc} not permitted for {t} msa (class 1 ≤ 20 msa, class 2 ≤ 80 msa)"));
                    }
                    let h = req(p.uk_chart_asphalt_mm, "uk_chart_asphalt_mm (read from CD 226 Figure 2.20 for the traffic and foundation class)")?;
                    eng.push("asphalt thickness from CD 226 Figure 2.20 (nomograph not reproduced)".into());
                    let h = up(h, 5.0);
                    steps.push(format!("Figure 2.20 asphalt-base design thickness (engineer read) = {h:.0} mm (rounded up to 5 mm, Note 3)"));
                    layers.push(Layer { name: "Surface course".into(), thickness_mm: 40.0, note: "typical; included in total (Fig 2.20 Note 4)".into() });
                    layers.push(Layer {
                        name: "Binder course + asphalt base".into(),
                        thickness_mm: h - 40.0,
                        note: "same material for base and binder (CD 226 §2.20.1)".into(),
                    });
                }
                "hbgm" => {
                    let h = up(cd226_hbgm_asphalt_mm(t), 5.0);
                    steps.push(format!("CD 226 Eq 2.24: H = −16.05(log T)² + 101·log T + 45.8 = {:.1} mm → {h:.0} mm", cd226_hbgm_asphalt_mm(t)));
                    let hb = req(p.uk_chart_hbgm_mm, "uk_chart_hbgm_mm (HBGM thickness from CD 226 Figure 2.20, left side)")?;
                    eng.push("HBGM base thickness from CD 226 Figure 2.20".into());
                    layers.push(Layer { name: "Asphalt (surface + binder + base)".into(), thickness_mm: h, note: "CD 226 Eq 2.24".into() });
                    layers.push(Layer { name: "HBGM base".into(), thickness_mm: up(hb, 5.0), note: "engineer read from Figure 2.20".into() });
                }
                "urc" => {
                    if fc < 3 {
                        return Err("CD 226 URC equations apply to foundation class 3 or 4".into());
                    }
                    let rc = req(p.uk_rc_mpa, "uk_rc_mpa")?;
                    let e = if fc == 3 { 200.0 } else { 400.0 };
                    let h1 = cd226_urc_mm(t, rc, e, false);
                    let h = cd226_urc_mm(t, rc, e, p.uk_tied_shoulder).max(150.0);
                    steps.push(format!(
                        "CD 226 Eq 2.43a: Ln(H1) = (Ln T − 3.466·Ln Rc − 0.484·Ln E + 40.483)/5.094 → H1 = {h1:.1} mm{}",
                        if p.uk_tied_shoulder { format!("; Eq 2.43b H2 = 0.934·H1 − 12.5 = {:.1} mm", 0.934 * h1 - 12.5) } else { String::new() }
                    ));
                    layers.push(Layer { name: "URC slab".into(), thickness_mm: up(h, 5.0), note: "min 150 mm, rounded up to 5 mm".into() });
                }
                o => return Err(format!("uk_type {o}: asphalt | hbgm | urc")),
            }
            layers.push(Layer { name: format!("Foundation class {fc}"), thickness_mm: 0.0, note: "designed to CD 225 (thickness by engineer)".into() });
            let total = layers.iter().map(|l| l.thickness_mm).sum();
            Ok(PavementDesign {
                method: format!("DMRB CD 226 standard design ({ty})"),
                citation: CD226.into(),
                kind: p.kind.clone(),
                inputs,
                layers,
                total_mm: total,
                sn_required: None,
                sn_provided: None,
                steps,
                engineer_inputs: eng,
                warnings: warn,
            })
        }
    }
}

/// Design traffic from AADT (generic growth formula): N = 365·AADT·HV·ESA/HV·DF·LF·((1+g)^n − 1)/g.
pub fn cumulative_traffic(aadt: f64, hv_frac: f64, esa_per_hv: f64, growth: f64, years: f64, dir_factor: f64, lane_factor: f64) -> f64 {
    let gf = if growth.abs() < 1e-9 { years } else { ((1.0 + growth).powf(years) - 1.0) / growth };
    365.0 * aadt * hv_frac * esa_per_hv * dir_factor * lane_factor * gf
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aashto93_flexible_worked_example() {
        // Guide Figure 3.1 example: W18 = 5e6, R = 95 % (ZR = −1.645), S0 = 0.35, Mr = 5000 psi, ΔPSI = 1.9 → SN ≈ 5.0
        let sn = aashto_flex_sn(5e6, -1.645, 0.35, 1.9, 5000.0);
        assert!((sn - 5.0).abs() < 0.1, "{sn}");
    }
    #[test]
    fn aashto93_rigid_worked_example() {
        // Guide Figure 3.7 example: k = 72 pci, Ec = 5e6 psi, S'c = 650 psi, J = 3.2, Cd = 1.0, S0 = 0.29, R = 95 %, ΔPSI = 1.7, W18 = 5.1e6 → D ≈ 10.0 in
        let d = aashto_rigid_d(5.1e6, -1.645, 0.29, 1.7, 2.5, 650.0, 1.0, 3.2, 5e6, 72.0);
        assert!((d - 10.0).abs() < 0.3, "{d}");
    }
    #[test]
    fn cd226_eq224_limits() {
        assert_eq!(cd226_hbgm_asphalt_mm(100.0), 180.0);
        let h = cd226_hbgm_asphalt_mm(1.0);
        assert!((h - 45.8).abs() < 1e-9);
    }
}
