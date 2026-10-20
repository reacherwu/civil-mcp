//! Design-standard checks. Every check carries the formula or table it used and
//! a citation so a reviewing engineer can verify it by hand.
//!
//! IMPORTANT: table values below were transcribed by the authors of this tool
//! from the cited documents. Clause/table numbers and values must be verified
//! by the engineer in responsible charge against the edition they use.
use crate::align::Alignment;
use crate::profile::Profile;
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
    /// AASHTO only: maximum superelevation (m/m), engineer's choice (e.g. 0.06, 0.08).
    #[serde(default)]
    pub e_max: Option<f64>,
    /// AASHTO only: side friction factor for the design speed, from Green Book tables.
    #[serde(default)]
    pub f_max: Option<f64>,
    /// AASHTO only: maximum grade %, from the Green Book chapter for the functional class/terrain.
    #[serde(default)]
    pub max_grade_pct: Option<f64>,
    #[serde(default = "d_t")]
    pub perception_reaction_s: f64,
    #[serde(default = "d_a")]
    pub deceleration_mps2: f64,
}
fn d_t() -> f64 {
    2.5
}
fn d_a() -> f64 {
    3.4
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

/// JTG D20-2017 values by design speed (km/h):
/// (speed, R_min general, R_min limit, max grade %, min grade length m,
///  crest VC radius limit, sag VC radius limit, min VC length m)
const JTG: [(f64, f64, f64, f64, f64, f64, f64, f64); 7] = [
    (120.0, 1000.0, 650.0, 3.0, 300.0, 11000.0, 4000.0, 100.0),
    (100.0, 700.0, 400.0, 4.0, 250.0, 6500.0, 3000.0, 85.0),
    (80.0, 400.0, 250.0, 5.0, 200.0, 3000.0, 2000.0, 70.0),
    (60.0, 200.0, 125.0, 6.0, 150.0, 1400.0, 1000.0, 50.0),
    (40.0, 100.0, 60.0, 7.0, 120.0, 450.0, 450.0, 35.0),
    (30.0, 65.0, 30.0, 8.0, 100.0, 250.0, 250.0, 25.0),
    (20.0, 30.0, 15.0, 9.0, 60.0, 100.0, 100.0, 20.0),
];
/// (R_min general, R_min limit, max grade %, min grade length, crest Rv limit, sag Rv limit, min VC length)
pub fn jtg_row(speed: f64) -> Option<(f64, f64, f64, f64, f64, f64, f64)> {
    JTG.iter().find(|r| r.0 == speed).map(|r| (r.1, r.2, r.3, r.4, r.5, r.6, r.7))
}
const JTG_CITE: &str = "JTG D20-2017 公路路线设计规范 (Design Specification for Highway Alignment)";
const AASHTO_CITE: &str = "AASHTO A Policy on Geometric Design of Highways and Streets, 7th ed. (2018), metric";

pub fn validate_basis(b: &DesignBasis) -> crate::Result<()> {
    if b.engineer_of_record.trim().is_empty() {
        return Err("design basis must name the engineer in responsible charge".into());
    }
    match b.standard {
        Standard::JtgD20_2017 => {
            if !JTG.iter().any(|r| r.0 == b.design_speed_kmh) {
                return Err("JTG D20-2017 design speed must be one of 120,100,80,60,40,30,20 km/h".into());
            }
        }
        Standard::Aashto2018 => {
            if b.e_max.is_none() || b.f_max.is_none() || b.max_grade_pct.is_none() {
                return Err("AASHTO basis requires e_max, f_max and max_grade_pct chosen by the engineer (they depend on facility type, terrain and climate)".into());
            }
        }
    }
    Ok(())
}

/// Stopping sight distance (m): SSD = 0.278·V·t + 0.039·V²/a.
pub fn ssd(b: &DesignBasis) -> f64 {
    let v = b.design_speed_kmh;
    0.278 * v * b.perception_reaction_s + 0.039 * v * v / b.deceleration_mps2
}

/// Minimum crest curve length for SSD (eye 1.08 m, object 0.60 m).
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

fn chk(id: &str, item: &str, element: String, value: f64, limit: f64, unit: &str, kind: &str, severity: &str, formula: String, citation: String) -> Check {
    let pass = if kind == "min" { value >= limit - 1e-9 } else { value <= limit + 1e-9 };
    Check { id: id.into(), item: item.into(), element, value, limit, unit: unit.into(), kind: kind.into(), pass, severity: severity.into(), formula, citation }
}

pub fn run_checks(b: &DesignBasis, al: &Alignment, pr: Option<&Profile>) -> Vec<Check> {
    let mut out = Vec::new();
    let v = b.design_speed_kmh;
    let s = ssd(b);
    match b.standard {
        Standard::Aashto2018 => {
            let (e, f) = (b.e_max.unwrap_or(0.0), b.f_max.unwrap_or(0.0));
            let rmin = v * v / (127.0 * (e + f));
            for (pi, r, _, _) in al.curves() {
                out.push(chk("H-RMIN", "Horizontal curve radius", format!("PI {pi}"), r, rmin, "m", "min", "mandatory",
                    format!("Rmin = V²/(127(e+f)) = {v}²/(127·({e}+{f})) = {rmin:.1} m"),
                    format!("{AASHTO_CITE}, Ch.3 minimum radius (simplified curve formula)")));
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
            let row = JTG.iter().find(|r| r.0 == v).copied().unwrap_or(JTG[3]);
            for (pi, r, _, _) in al.curves() {
                out.push(chk("H-RMIN-LIM", "圆曲线最小半径（极限值）", format!("PI {pi}"), r, row.2, "m", "min", "mandatory",
                    format!("table lookup V={v} km/h → {} m", row.2), format!("{JTG_CITE}, 圆曲线最小半径表")));
                out.push(chk("H-RMIN-GEN", "圆曲线最小半径（一般值）", format!("PI {pi}"), r, row.1, "m", "min", "advisory",
                    format!("table lookup V={v} km/h → {} m", row.1), format!("{JTG_CITE}, 圆曲线最小半径表")));
            }
            if let Some(p) = pr {
                for (i, g) in p.grades_pct.iter().enumerate() {
                    out.push(chk("V-GMAX", "最大纵坡", format!("segment VPI{i}->VPI{}", i + 1), g.abs(), row.3, "%", "max", "mandatory",
                        format!("table lookup V={v} km/h → {}%", row.3), format!("{JTG_CITE}, 最大纵坡表")));
                    let len = p.input.vpis[i + 1][0] - p.input.vpis[i][0];
                    out.push(chk("V-LMIN", "最小坡长", format!("segment VPI{i}->VPI{}", i + 1), len, row.4, "m", "min", "mandatory",
                        format!("VPI spacing; table V={v} km/h → {} m", row.4), format!("{JTG_CITE}, 最小坡长表")));
                }
                for c in &p.curves {
                    let rv = if c.a_pct.abs() > 1e-12 { c.length / (c.a_pct.abs() / 100.0) } else { f64::INFINITY };
                    let (lim, name) = if c.crest { (row.5, "凸形竖曲线最小半径（极限值）") } else { (row.6, "凹形竖曲线最小半径（极限值）") };
                    out.push(chk("V-RVC", name, format!("VPI {} (A={:.2}%)", c.vpi_index, c.a_pct), rv, lim, "m", "min", "mandatory",
                        format!("R = L/(A/100) = {:.1}/{:.4}", c.length, c.a_pct.abs() / 100.0), format!("{JTG_CITE}, 竖曲线最小半径和最小长度表")));
                    out.push(chk("V-LVC", "竖曲线最小长度", format!("VPI {}", c.vpi_index), c.length, row.7, "m", "min", "mandatory",
                        format!("table lookup V={v} km/h → {} m", row.7), format!("{JTG_CITE}, 竖曲线最小半径和最小长度表")));
                }
                // SSD also reported for transparency (JTG has its own 停车视距 table; not checked here).
            }
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
        DesignBasis { project: "t".into(), engineer_of_record: "x".into(), standard: Standard::Aashto2018, design_speed_kmh: 80.0, e_max: Some(0.08), f_max: Some(0.14), max_grade_pct: Some(6.0), perception_reaction_s: 2.5, deceleration_mps2: 3.4 }
    }
    #[test]
    fn ssd_80kmh_matches_green_book_table() {
        // Green Book tabulates 130 m (design value, rounded) for 80 km/h.
        let s = ssd(&basis());
        assert!((s - 129.0).abs() < 0.5, "{s}");
    }
    #[test]
    fn crest_k_80kmh() {
        // K = S²/658 ≈ 25 (Green Book design K for 80 km/h is 26 using rounded 130 m SSD)
        let s = 130.0;
        // small A: S > L case, 2S - 658/A is negative -> no curve required
        assert_eq!(crest_len_min(1.0, s), 0.0);
        let k_large_a = crest_len_min(10.0, s) / 10.0;
        assert!((k_large_a - s * s / 658.0).abs() < 1e-9);
        assert!((k_large_a - 25.7).abs() < 0.1);
    }
}
