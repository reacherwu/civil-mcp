//! Retaining wall type selection at concept depth. For every wall section the corridor placed, the
//! options are screened against engineer-set practical height ranges and checked:
//! - gravity (mass concrete) and cantilever RC: walls::proportion (AASHTO / engineer criteria);
//! - MSE / reinforced soil: external stability (sliding, eccentricity, bearing) by the allowable-stress
//!   method of FHWA-NHI-10-024 (GEC 11) §4 with L ≥ max(0.7H, L_min); internal maximum reinforcement
//!   load T = Kr·σv·Sv with Kr = Ka (extensible reinforcement); per-country reinforced-soil standard
//!   cited (BS 8006-1 UK, AS 4678 AU/NZ, CSA S6 + CFEM CA);
//! - soil nail (cut walls only, top-down): FHWA-NHI-14-007 (GEC 7) concept layout, nail length ≈ 0.7–1.0H,
//!   simplified maximum nail force 0.75·Ka·γ·H·Sh·Sv.
//! A profile reshape that lowers the wall below the cantilever range is recorded as the first option
//! when the required change is within the engineer's limit. Supplier/specialist design governs.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelInput {
    #[serde(default = "d_cant")]
    pub cantilever_max_h_m: f64,
    #[serde(default = "d_grav")]
    pub gravity_max_h_m: f64,
    #[serde(default = "d_lr")]
    pub mse_l_ratio: f64,
    #[serde(default = "d_lmin")]
    pub mse_l_min_m: f64,
    #[serde(default = "d_g")]
    pub reinforced_gamma_knm3: f64,
    #[serde(default = "d_phi")]
    pub reinforced_phi_deg: f64,
    #[serde(default = "d_sv")]
    pub sv_m: f64,
    #[serde(default = "d_fs")]
    pub mse_fs_sliding: f64,
    #[serde(default = "d_e")]
    pub mse_e_frac: f64,
    #[serde(default = "d_nl")]
    pub nail_l_ratio: f64,
    #[serde(default = "d_ns")]
    pub nail_spacing_m: f64,
    /// largest profile change (m) the engineer accepts as a reshape instead of a taller wall type
    #[serde(default = "d_dz")]
    pub reshape_max_dz_m: f64,
    /// allowable bearing for the wide MSE base (kPa); defaults to the wall rule's footing value
    #[serde(default)]
    pub mse_bearing_kpa: Option<f64>,
    #[serde(default)]
    pub source: String,
}
fn d_cant() -> f64 {
    8.0
}
fn d_grav() -> f64 {
    3.0
}
fn d_lr() -> f64 {
    0.7
}
fn d_lmin() -> f64 {
    2.4
}
fn d_g() -> f64 {
    20.0
}
fn d_phi() -> f64 {
    34.0
}
fn d_sv() -> f64 {
    0.6
}
fn d_fs() -> f64 {
    1.5
}
fn d_e() -> f64 {
    1.0 / 6.0
}
fn d_nl() -> f64 {
    0.8
}
fn d_ns() -> f64 {
    1.5
}
fn d_dz() -> f64 {
    1.0
}

pub fn citation(std: crate::standards::Standard) -> &'static str {
    use crate::standards::Standard::*;
    match std {
        Uk => "BS 8006-1:2010+A1:2016 (reinforced soil), BS 8006-2:2011 (soil nailing), BS EN 1997-1; type selection per DMRB CG 300 family (verify)",
        Au => "AS 4678-2002 Earth-retaining structures (reinforced soil, gravity, cantilever); TfNSW R57 / MRTS06 for supplier walls (verify)",
        Nz => "AS 4678-2002 as adopted via NZTA Bridge Manual §6 (verify); soil nails to FHWA GEC 7",
        Ca => "CSA S6:19 (CHBDC) §6 and Canadian Foundation Engineering Manual (CFEM) for retaining systems; MSE to FHWA-NHI-10-024 as referenced by provincial agencies (verify)",
        _ => "FHWA-NHI-10-024 (GEC 11) MSE walls; FHWA-NHI-14-007 (GEC 7) soil nail walls; FHWA-NHI-07-071 Earth Retaining Structures (type selection)",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Mse {
    pub l_m: f64,
    pub ka_retained: f64,
    pub fs_sliding: f64,
    pub e_m: f64,
    pub sigma_v_kpa: f64,
    pub layers: usize,
    pub t_max_kn_m: f64,
    pub pass: bool,
}

/// MSE external stability (per metre run). h design height, retained soil (γ, φ, surcharge),
/// base sliding coefficient μ, allowable bearing.
pub fn mse(h: f64, soil: &crate::walls::Soil, s: &SelInput) -> Mse {
    let ka = (std::f64::consts::FRAC_PI_4 - soil.phi_deg.to_radians() / 2.0).tan().powi(2);
    let kr = (std::f64::consts::FRAC_PI_4 - s.reinforced_phi_deg.to_radians() / 2.0).tan().powi(2);
    let (gb, q, gr) = (soil.gamma_kn_m3, soil.surcharge_kpa, s.reinforced_gamma_knm3);
    let f1 = 0.5 * ka * gb * h * h;
    let f2 = ka * q * h;
    let mut l = ((s.mse_l_ratio * h).max(s.mse_l_min_m) * 10.0).ceil() / 10.0;
    loop {
        let v1 = gr * h * l;
        let fs = soil.base_friction * v1 / (f1 + f2);
        let mo = f1 * h / 3.0 + f2 * h / 2.0;
        let e = mo / v1;
        let eb = mo / (v1 + q * l);
        let sv = (v1 + q * l) / (l - 2.0 * eb);
        let pass = fs >= s.mse_fs_sliding && e <= s.mse_e_frac * l + 1e-12 && sv <= s.mse_bearing_kpa.unwrap_or(soil.bearing_kpa);
        if pass || l > 3.0 * h {
            let layers = (h / s.sv_m).ceil() as usize;
            let t = kr * (gr * (h - s.sv_m / 2.0) + q) * s.sv_m;
            return Mse { l_m: l, ka_retained: ka, fs_sliding: fs, e_m: e, sigma_v_kpa: sv, layers, t_max_kn_m: t, pass };
        }
        l = ((l + 0.1) * 10.0).round() / 10.0;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub station: f64,
    pub side: String,
    pub cut: bool,
    pub h_m: f64,
    pub exposed_h_m: f64,
    pub trib_m: f64,
    pub selected: String,
    pub reason: String,
    pub reshape_dz_m: f64,
    pub cantilever_b_m: f64,
    pub cantilever_concrete_m2: f64,
    pub gravity_ok: bool,
    pub mse: Mse,
    pub nail_l_m: f64,
    pub nails_per_m: f64,
    pub nail_t_kn: f64,
    pub options: Vec<(String, String)>,
}

pub fn select(std: crate::standards::Standard, c: &crate::corridor::Corridor, cr: &crate::walls::Criteria, s: &SelInput) -> crate::Result<Vec<Choice>> {
    let soil = c.assembly.wall.as_ref().map(|w| w.soil.clone()).ok_or("the assembly has no wall rule")?;
    let mut raw = vec![];
    for (k, sec) in c.sections.iter().enumerate() {
        for (side, sd) in [("L", &sec.left), ("R", &sec.right)] {
            if let Some(w) = &sd.wall {
                raw.push((k, sec.station, side, w.clone()));
            }
        }
    }
    let mut out = vec![];
    for (k, st, side, w) in raw.iter() {
        // tributary length: half of each adjacent interval whose other end also has a wall on this side
        // (equivalent to the corridor's average-end-area wall volume)
        let has = |j: usize| raw.iter().any(|r| r.0 == j && r.2 == *side);
        let mut trib = 0.0;
        if *k > 0 && has(k - 1) {
            trib += (st - c.sections[k - 1].station) / 2.0;
        }
        if has(k + 1) {
            trib += (c.sections[k + 1].station - st) / 2.0;
        }
        let h = w.design.h;
        let g = crate::walls::proportion(&crate::walls::WallKind::Gravity, h, &soil, cr);
        let m = mse(h, &soil, s);
        let ka = m.ka_retained;
        let nail_l = s.nail_l_ratio * h;
        let nails = (h / s.nail_spacing_m).ceil() / s.nail_spacing_m;
        let nail_t = 0.75 * ka * soil.gamma_kn_m3 * h * s.nail_spacing_m * s.nail_spacing_m;
        let dz = (h - s.cantilever_max_h_m).max(0.0);
        let mut opts = vec![];
        opts.push((
            "gravity".to_string(),
            if h <= s.gravity_max_h_m && g.pass {
                "feasible".into()
            } else {
                format!("rejected: H {h:.1} m > {:.1} m practical limit or stability", s.gravity_max_h_m)
            },
        ));
        opts.push((
            "cantilever".to_string(),
            if h <= s.cantilever_max_h_m && w.design.pass {
                "feasible".into()
            } else {
                format!("rejected: H {h:.1} m > {:.1} m practical cantilever range", s.cantilever_max_h_m)
            },
        ));
        opts.push((
            "reshape profile".to_string(),
            if dz == 0.0 {
                "not needed".into()
            } else if dz <= s.reshape_max_dz_m {
                format!("feasible: change FG by {dz:.2} m")
            } else {
                format!("rejected: needs {dz:.2} m FG change > {:.2} m limit", s.reshape_max_dz_m)
            },
        ));
        opts.push((
            "MSE / reinforced soil".to_string(),
            if w.cut {
                "possible only with excavation and temporary support (cut wall)".into()
            } else if m.pass {
                format!("feasible: L {:.1} m, FS sliding {:.2}, e {:.2} m, σv {:.0} kPa", m.l_m, m.fs_sliding, m.e_m, m.sigma_v_kpa)
            } else {
                "fails external stability".into()
            },
        ));
        opts.push((
            "soil nail".to_string(),
            if w.cut {
                format!("feasible: L {nail_l:.1} m at {:.1} m grid, T ≈ {nail_t:.0} kN/nail", s.nail_spacing_m)
            } else {
                "not applicable (fill)".into()
            },
        ));
        let (sel, why) = if h <= s.gravity_max_h_m && g.pass {
            ("gravity", "lowest height range; mass concrete gravity wall passes external stability")
        } else if h <= s.cantilever_max_h_m && w.design.pass {
            ("cantilever", "within the practical cantilever range; proportioned wall passes external stability")
        } else if dz > 0.0 && dz <= s.reshape_max_dz_m {
            ("reshape profile", "small profile change brings the wall into the cantilever range (engineer to apply in the profile)")
        } else if w.cut {
            ("soil nail", "tall cut wall: top-down soil nailing avoids temporary excavation support")
        } else if m.pass {
            ("MSE", "tall fill wall: MSE / reinforced soil is the economical type above the cantilever range")
        } else {
            ("specialist", "no screened option passes: refer to the structures/geotechnical specialist")
        };
        out.push(Choice {
            station: *st,
            side: side.to_string(),
            cut: w.cut,
            h_m: h,
            exposed_h_m: w.design.exposed_h,
            trib_m: trib,
            selected: sel.into(),
            reason: format!("{why} ({})", citation(std)),
            reshape_dz_m: dz,
            cantilever_b_m: w.design.base_b,
            cantilever_concrete_m2: w.design.concrete_m2,
            gravity_ok: g.pass && h <= s.gravity_max_h_m,
            mse: m,
            nail_l_m: nail_l,
            nails_per_m: nails,
            nail_t_kn: nail_t,
            options: opts,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mse_external_hand_calc() {
        let soil = crate::walls::Soil { gamma_kn_m3: 19.0, phi_deg: 30.0, base_friction: 0.45, bearing_kpa: 400.0, surcharge_kpa: 10.0 };
        let s: SelInput = serde_json::from_str("{}").unwrap();
        let m = mse(10.0, &soil, &s);
        // Ka = 1/3; F1 = 0.5/3·19·100 = 316.67, F2 = 10·10/3 = 33.33; L = 7.0 → V = 20·10·7 = 1400; FS = 0.45·1400/350 = 1.8
        assert!((m.l_m - 7.0).abs() < 1e-9 && (m.fs_sliding - 1.8).abs() < 1e-9, "{:?}", m);
        let mo = 316.6667 * 10.0 / 3.0 + 33.3333 * 5.0;
        assert!((m.e_m - mo / 1400.0).abs() < 1e-3);
    }
}
