//! Design-standard checks. Every check carries the formula or table it used and
//! a citation so a reviewing engineer can verify it by hand.
//!
//! IMPORTANT: table values below were transcribed by the authors of this tool
//! from the cited documents. Clause/table numbers and values must be verified
//! by the engineer in responsible charge against the edition they use.
use crate::align::Alignment;
use crate::profile::Profile;
use crate::superelev;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Standard {
    #[serde(rename = "AASHTO_GREEN_BOOK_2018")]
    Aashto2018,
    #[serde(rename = "JTG_D20_2017")]
    JtgD20_2017,
}

/// Set by the engineer in responsible charge *before* design starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignBasis {
    pub project: String,
    pub engineer_of_record: String,
    pub standard: Standard,
    pub design_speed_kmh: f64,
    /// Maximum superelevation (m/m), engineer's choice (e.g. 0.06, 0.08).
    #[serde(default)]
    pub e_max: Option<f64>,
    /// Side friction factor used for superelevation/radius (AASHTO f_max, JTG μ).
    #[serde(default)]
    pub f_max: Option<f64>,
    /// AASHTO only: maximum grade %, from the Green Book chapter for the functional class/terrain.
    #[serde(default)]
    pub max_grade_pct: Option<f64>,
    #[serde(default = "d_t")]
    pub perception_reaction_s: f64,
    #[serde(default = "d_a")]
    pub deceleration_mps2: f64,
    /// Normal crown cross slope (m/m).
    #[serde(default = "d_nc")]
    pub normal_crown: f64,
    #[serde(default = "d_lw")]
    pub lane_width_m: f64,
    #[serde(default = "d_one")]
    pub lanes_rotated_per_side: f64,
}
fn d_t() -> f64 {
    2.5
}
fn d_a() -> f64 {
    3.4
}
fn d_nc() -> f64 {
    0.02
}
fn d_lw() -> f64 {
    3.5
}
fn d_one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: String,
    pub item: String,
    pub element: String,
    pub value: f64,
    pub limit: f64,
    pub unit: String,
    /// "min" or "max"
    pub kind: String,
    pub pass: bool,
    /// "mandatory" or "advisory"
    pub severity: String,
    pub formula: String,
    pub citation: String,
}

#[derive(Debug, Clone, Copy)]
pub struct JtgRow {
    pub speed: f64,
    pub r_general: f64,
    pub r_limit: f64,
    pub max_grade: f64,
    pub min_grade_len: f64,
    pub crest_rv: f64,
    pub sag_rv: f64,
    pub min_vc_len: f64,
    pub ssd: f64,
    pub min_spiral: f64,
    pub no_super_radius: f64,
    /// 超高渐变率 1/p for rotation about the centreline (value = p)
    pub rel_gradient_inv: f64,
}

/// JTG D20-2017 values by design speed (km/h), transcribed; verify before use.
const JTG: [JtgRow; 7] = [
    JtgRow { speed: 120.0, r_general: 1000.0, r_limit: 650.0, max_grade: 3.0, min_grade_len: 300.0, crest_rv: 11000.0, sag_rv: 4000.0, min_vc_len: 100.0, ssd: 210.0, min_spiral: 100.0, no_super_radius: 5500.0, rel_gradient_inv: 250.0 },
    JtgRow { speed: 100.0, r_general: 700.0, r_limit: 400.0, max_grade: 4.0, min_grade_len: 250.0, crest_rv: 6500.0, sag_rv: 3000.0, min_vc_len: 85.0, ssd: 160.0, min_spiral: 85.0, no_super_radius: 4000.0, rel_gradient_inv: 225.0 },
    JtgRow { speed: 80.0, r_general: 400.0, r_limit: 250.0, max_grade: 5.0, min_grade_len: 200.0, crest_rv: 3000.0, sag_rv: 2000.0, min_vc_len: 70.0, ssd: 110.0, min_spiral: 70.0, no_super_radius: 2500.0, rel_gradient_inv: 200.0 },
    JtgRow { speed: 60.0, r_general: 200.0, r_limit: 125.0, max_grade: 6.0, min_grade_len: 150.0, crest_rv: 1400.0, sag_rv: 1000.0, min_vc_len: 50.0, ssd: 75.0, min_spiral: 50.0, no_super_radius: 1500.0, rel_gradient_inv: 175.0 },
    JtgRow { speed: 40.0, r_general: 100.0, r_limit: 60.0, max_grade: 7.0, min_grade_len: 120.0, crest_rv: 450.0, sag_rv: 450.0, min_vc_len: 35.0, ssd: 40.0, min_spiral: 35.0, no_super_radius: 600.0, rel_gradient_inv: 150.0 },
    JtgRow { speed: 30.0, r_general: 65.0, r_limit: 30.0, max_grade: 8.0, min_grade_len: 100.0, crest_rv: 250.0, sag_rv: 250.0, min_vc_len: 25.0, ssd: 30.0, min_spiral: 25.0, no_super_radius: 350.0, rel_gradient_inv: 125.0 },
    JtgRow { speed: 20.0, r_general: 30.0, r_limit: 15.0, max_grade: 9.0, min_grade_len: 60.0, crest_rv: 100.0, sag_rv: 100.0, min_vc_len: 20.0, ssd: 20.0, min_spiral: 20.0, no_super_radius: 150.0, rel_gradient_inv: 100.0 },
];
pub fn jtg_row(speed: f64) -> Option<JtgRow> {
    JTG.iter().find(|r| r.speed == speed).copied()
}
pub const JTG_CITE: &str = "JTG D20-2017 公路路线设计规范 (Design Specification for Highway Alignment)";
pub const AASHTO_CITE: &str = "AASHTO A Policy on Geometric Design of Highways and Streets, 7th ed. (2018), metric";

pub fn validate_basis(b: &DesignBasis) -> crate::Result<()> {
    if b.engineer_of_record.trim().is_empty() {
        return Err("design basis must name the engineer in responsible charge".into());
    }
    if b.e_max.is_none() || b.f_max.is_none() {
        return Err("design basis requires e_max and f_max (JTG: μ) chosen by the engineer".into());
    }
    if !(b.normal_crown > 0.0 && b.normal_crown < 0.06) || b.lane_width_m <= 0.0 {
        return Err("normal_crown must be in (0, 0.06) and lane_width_m > 0".into());
    }
    match b.standard {
        Standard::JtgD20_2017 => {
            if jtg_row(b.design_speed_kmh).is_none() {
                return Err("JTG D20-2017 design speed must be one of 120,100,80,60,40,30,20 km/h".into());
            }
        }
        Standard::Aashto2018 => {
            if b.max_grade_pct.is_none() {
                return Err("AASHTO basis requires max_grade_pct chosen by the engineer (depends on functional class and terrain)".into());
            }
        }
    }
    Ok(())
}

/// Stopping sight distance (m). AASHTO: 0.278·V·t + 0.039·V²/a. JTG: table.
pub fn ssd(b: &DesignBasis) -> f64 {
    match b.standard {
        Standard::JtgD20_2017 => jtg_row(b.design_speed_kmh).map_or(0.0, |r| r.ssd),
        Standard::Aashto2018 => {
            let v = b.design_speed_kmh;
            0.278 * v * b.perception_reaction_s + 0.039 * v * v / b.deceleration_mps2
        }
    }
}

/// Minimum crest curve length for sight distance S with eye h1 / object h2.
pub fn crest_len_min_h(a: f64, s: f64, h1: f64, h2: f64) -> f64 {
    let a = a.abs();
    let c = 200.0 * (h1.sqrt() + h2.sqrt()).powi(2);
    let l1 = a * s * s / c;
    if l1 >= s {
        l1
    } else {
        (2.0 * s - c / a).max(0.0)
    }
}
/// AASHTO crest (eye 1.08 m, object 0.60 m) => 658.
pub fn crest_len_min(a: f64, s: f64) -> f64 {
    let a = a.abs();
    let l1 = a * s * s / 658.0;
    if l1 >= s {
        l1
    } else {
        (2.0 * s - 658.0 / a).max(0.0)
    }
}
/// Minimum sag curve length for headlight sight distance (0.6 m, 1°).
pub fn sag_len_min(a: f64, s: f64) -> f64 {
    let a = a.abs();
    let l1 = a * s * s / (120.0 + 3.5 * s);
    if l1 >= s {
        l1
    } else {
        (2.0 * s - (120.0 + 3.5 * s) / a).max(0.0)
    }
}

pub fn chk(id: &str, item: &str, element: String, value: f64, limit: f64, unit: &str, kind: &str, severity: &str, formula: String, citation: String) -> Check {
    let pass = if kind == "min" { value >= limit - 1e-9 } else { value <= limit + 1e-9 };
    Check { id: id.into(), item: item.into(), element, value, limit, unit: unit.into(), kind: kind.into(), pass, severity: severity.into(), formula, citation }
}

pub fn run_checks(b: &DesignBasis, al: &Alignment, pr: Option<&Profile>) -> Vec<Check> {
    let mut out = Vec::new();
    let v = b.design_speed_kmh;
    let s = ssd(b);
    let curves = al.curves();
    let se = superelev::design(b, al);
    match b.standard {
        Standard::Aashto2018 => {
            let (e, f) = (b.e_max.unwrap_or(0.0), b.f_max.unwrap_or(0.0));
            let rmin = v * v / (127.0 * (e + f));
            for c in &curves {
                let el = format!("curve {}", c.index);
                out.push(chk("H-RMIN", "Horizontal curve radius", el.clone(), c.radius, rmin, "m", "min", "mandatory",
                    format!("Rmin = V²/(127(e+f)) = {v}²/(127·({e}+{f})) = {rmin:.1} m"), format!("{AASHTO_CITE}, Ch.3 minimum radius")));
                for (side, ls) in [("entry", c.ls_in), ("exit", c.ls_out)] {
                    if ls > 0.0 {
                        let lmin = (24.0 * 0.20 * c.radius).sqrt();
                        let lmax = (24.0 * 1.0 * c.radius).sqrt();
                        out.push(chk("H-LS-MIN", "Spiral length (minimum, lateral shift 0.20 m)", format!("{el} {side}"), ls, lmin, "m", "min", "advisory",
                            format!("Ls,min = √(24·p_min·R), p_min=0.20 m → {lmin:.1} m"), format!("{AASHTO_CITE}, Ch.3 spiral curve transitions")));
                        out.push(chk("H-LS-MAX", "Spiral length (maximum, lateral shift 1.0 m)", format!("{el} {side}"), ls, lmax, "m", "max", "advisory",
                            format!("Ls,max = √(24·p_max·R), p_max=1.0 m → {lmax:.1} m"), format!("{AASHTO_CITE}, Ch.3 spiral curve transitions")));
                    }
                }
            }
            if let Ok(se) = &se {
                let dmax = superelev::aashto_max_rel_gradient_pct(v);
                for sc in &se.curves {
                    if sc.e > 0.0 {
                        out.push(chk("SE-REL", "Superelevation relative gradient", format!("curve {}", sc.curve), sc.relative_gradient * 100.0, dmax, "%", "max", "mandatory",
                            format!("Δ = w·(e+nc)/L = {:.2}·({:.4}+{:.3})/L; max Δ for {v} km/h", se.rotation_width, sc.e, se.nc), format!("{AASHTO_CITE}, Ch.3 superelevation runoff, max relative gradient table")));
                    }
                }
            }
            if let Some(p) = pr {
                let gmax = b.max_grade_pct.unwrap_or(0.0);
                for (i, g) in p.grades_pct.iter().enumerate() {
                    out.push(chk("V-GMAX", "Grade", format!("segment VPI{i}->VPI{}", i + 1), g.abs(), gmax, "%", "max", "mandatory",
                        format!("|g| ≤ engineer-selected max grade {gmax}%"), format!("{AASHTO_CITE}, maximum grades by functional class (value set in design basis)")));
                }
                for c in &p.curves {
                    let (lmin, f) = if c.crest {
                        (crest_len_min(c.a_pct, s), format!("crest, S={s:.1} m: L = A·S²/658 if S<L else 2S−658/A"))
                    } else {
                        (sag_len_min(c.a_pct, s), format!("sag (headlight), S={s:.1} m: L = A·S²/(120+3.5S) if S<L else 2S−(120+3.5S)/A"))
                    };
                    out.push(chk("V-LVC", "Vertical curve length for SSD", format!("VPI {} (A={:.2}%)", c.vpi_index, c.a_pct), c.length, lmin, "m", "min", "mandatory",
                        format!("{f}; SSD = 0.278·V·t + 0.039·V²/a, t={} s, a={} m/s²", b.perception_reaction_s, b.deceleration_mps2),
                        format!("{AASHTO_CITE}, Ch.3 stopping sight distance and vertical curves")));
                }
            }
        }
        Standard::JtgD20_2017 => {
            let row = jtg_row(v).unwrap_or(JTG[3]);
            for c in &curves {
                let el = format!("curve {}", c.index);
                out.push(chk("H-RMIN-LIM", "圆曲线最小半径（极限值）", el.clone(), c.radius, row.r_limit, "m", "min", "mandatory",
                    format!("table V={v} km/h → {} m", row.r_limit), format!("{JTG_CITE}, 圆曲线最小半径表")));
                out.push(chk("H-RMIN-GEN", "圆曲线最小半径（一般值）", el.clone(), c.radius, row.r_general, "m", "min", "advisory",
                    format!("table V={v} km/h → {} m", row.r_general), format!("{JTG_CITE}, 圆曲线最小半径表")));
                if c.radius < row.no_super_radius {
                    for (side, ls) in [("entry", c.ls_in), ("exit", c.ls_out)] {
                        out.push(chk("H-LS-MIN", "缓和曲线最小长度", format!("{el} {side}"), ls, row.min_spiral, "m", "min", "mandatory",
                            format!("R={:.0} < 不设超高最小半径 {} m → 缓和曲线 required; table → {} m", c.radius, row.no_super_radius, row.min_spiral), format!("{JTG_CITE}, 缓和曲线最小长度表")));
                    }
                }
            }
            if let Ok(se) = &se {
                for sc in &se.curves {
                    if sc.e > 0.0 {
                        let lim = 1.0 / row.rel_gradient_inv * 100.0;
                        out.push(chk("SE-REL", "超高渐变率", format!("curve {}", sc.curve), sc.relative_gradient * 100.0, lim, "%", "max", "mandatory",
                            format!("p = B'·Δi/Lc = {:.2}·({:.4}+{:.3})/Lc ≤ 1/{}", se.rotation_width, sc.e, se.nc, row.rel_gradient_inv), format!("{JTG_CITE}, 超高渐变率表（绕中线旋转）")));
                        let c = curves.iter().find(|c| c.index == sc.curve).unwrap();
                        if c.ls_in > 0.0 {
                            out.push(chk("SE-IN-LS", "超高过渡段应在缓和曲线内", format!("curve {}", sc.curve), c.ls_in.min(c.ls_out), sc.required_length, "m", "min", "mandatory",
                                format!("Lc = B'·Δi/p = {:.1} m ≤ Ls", sc.required_length), format!("{JTG_CITE}, 超高过渡")));
                        }
                    }
                }
            }
            if let Some(p) = pr {
                for (i, g) in p.grades_pct.iter().enumerate() {
                    out.push(chk("V-GMAX", "最大纵坡", format!("segment VPI{i}->VPI{}", i + 1), g.abs(), row.max_grade, "%", "max", "mandatory",
                        format!("table V={v} km/h → {}%", row.max_grade), format!("{JTG_CITE}, 最大纵坡表")));
                    let len = p.input.vpis[i + 1][0] - p.input.vpis[i][0];
                    out.push(chk("V-LMIN", "最小坡长", format!("segment VPI{i}->VPI{}", i + 1), len, row.min_grade_len, "m", "min", "mandatory",
                        format!("VPI spacing; table V={v} km/h → {} m", row.min_grade_len), format!("{JTG_CITE}, 最小坡长表")));
                }
                for c in &p.curves {
                    let rv = if c.a_pct.abs() > 1e-12 { c.length / (c.a_pct.abs() / 100.0) } else { f64::INFINITY };
                    let (lim, name) = if c.crest { (row.crest_rv, "凸形竖曲线最小半径（极限值）") } else { (row.sag_rv, "凹形竖曲线最小半径（极限值）") };
                    out.push(chk("V-RVC", name, format!("VPI {} (A={:.2}%)", c.vpi_index, c.a_pct), rv, lim, "m", "min", "mandatory",
                        format!("R = L/(A/100) = {:.1}/{:.4}", c.length, c.a_pct.abs() / 100.0), format!("{JTG_CITE}, 竖曲线最小半径和最小长度表")));
                    out.push(chk("V-LVC", "竖曲线最小长度", format!("VPI {}", c.vpi_index), c.length, row.min_vc_len, "m", "min", "mandatory",
                        format!("table V={v} km/h → {} m", row.min_vc_len), format!("{JTG_CITE}, 竖曲线最小半径和最小长度表")));
                    if c.crest {
                        let lmin = crest_len_min_h(c.a_pct, row.ssd, 1.2, 0.1);
                        out.push(chk("V-SSD", "凸形竖曲线停车视距（复核）", format!("VPI {}", c.vpi_index), c.length, lmin, "m", "min", "advisory",
                            format!("停车视距 S={} m (table); L = A·S²/(200(√1.2+√0.1)²) if S<L else 2S−…/A", row.ssd), format!("{JTG_CITE}, 停车视距表; eye 1.2 m / object 0.1 m")));
                    }
                }
            }
        }
    }
    if let Err(e) = &se {
        out.push(Check { id: "SE-DESIGN".into(), item: "Superelevation design".into(), element: "alignment".into(), value: 0.0, limit: 1.0, unit: "bool".into(), kind: "min".into(), pass: false, severity: "mandatory".into(), formula: e.clone(), citation: "tool rule".into() });
    } else if let Ok(se) = &se {
        for w in se.curves.windows(2) {
            let ok = w[1].a_in >= w[0].a_out - 1e-6 || w[0].e == 0.0 || w[1].e == 0.0;
            out.push(Check { id: "SE-OVERLAP".into(), item: "Superelevation transitions do not overlap".into(), element: format!("curves {}–{}", w[0].curve, w[1].curve), value: w[1].a_in - w[0].a_out, limit: 0.0, unit: "m gap".into(), kind: "min".into(), pass: ok, severity: "mandatory".into(), formula: format!("next transition start {:.2} ≥ previous end {:.2}", w[1].a_in, w[0].a_out), citation: "geometric consistency (tool rule)".into() });
        }
    }
    if let Some(p) = pr {
        let cover = p.start() <= al.start_station() + 1e-6 && p.end() >= al.end_station() - 1e-6;
        out.push(Check { id: "G-COVER".into(), item: "Profile covers alignment".into(), element: "profile".into(), value: if cover { 1.0 } else { 0.0 }, limit: 1.0, unit: "bool".into(), kind: "min".into(), pass: cover, severity: "mandatory".into(), formula: format!("profile {:.2}–{:.2} vs alignment {:.2}–{:.2}", p.start(), p.end(), al.start_station(), al.end_station()), citation: "geometric consistency (tool rule)".into() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn basis() -> DesignBasis {
        serde_json::from_value(serde_json::json!({"project":"t","engineer_of_record":"x","standard":"AASHTO_GREEN_BOOK_2018","design_speed_kmh":80.0,"e_max":0.08,"f_max":0.14,"max_grade_pct":6.0})).unwrap()
    }
    #[test]
    fn ssd_80kmh_matches_green_book_table() {
        let s = ssd(&basis());
        assert!((s - 129.0).abs() < 0.5, "{s}");
    }
    #[test]
    fn crest_k_80kmh() {
        assert_eq!(crest_len_min(1.0, 130.0), 0.0);
        let k = crest_len_min(10.0, 130.0) / 10.0;
        assert!((k - 25.7).abs() < 0.1);
        // generic form with AASHTO heights reproduces 658
        assert!((crest_len_min_h(10.0, 130.0, 1.08, 0.60) - crest_len_min(10.0, 130.0)).abs() / crest_len_min(10.0, 130.0) < 0.002);
    }
    #[test]
    fn jtg_ssd_table() {
        let mut b = basis();
        b.standard = Standard::JtgD20_2017;
        b.design_speed_kmh = 60.0;
        assert_eq!(ssd(&b), 75.0);
    }
}
