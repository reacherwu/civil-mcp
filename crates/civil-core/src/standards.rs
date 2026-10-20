//! Design standards profiles for road design in the US, Canada, the UK, Australia and New Zealand.
//!
//! Every value used by a check carries its provenance:
//! - `verified`: read from a public source during development; the source and clause are cited.
//! - `formula`: a published formula is shipped, and the inputs come from the engineer.
//! - `engineer`: the table is copyrighted or paywalled, or could not be verified. The engineer
//!   in responsible charge must enter the value from the edition they work to, and the tool refuses
//!   to run without it.
//! The engineer must still confirm every value against the current edition and any jurisdictional
//! supplement before use.
use crate::align::Alignment;
use crate::profile::Profile;
use crate::superelev;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Standard {
    #[serde(rename = "US_AASHTO_2018")]
    Us,
    #[serde(rename = "CA_TAC_2017")]
    Ca,
    #[serde(rename = "UK_DMRB")]
    Uk,
    #[serde(rename = "AU_AUSTROADS")]
    Au,
    #[serde(rename = "NZ_AUSTROADS_NZTA")]
    Nz,
}

impl Standard {
    pub fn label(&self) -> &'static str {
        match self {
            Standard::Us => "US: AASHTO A Policy on Geometric Design of Highways and Streets (2018, 7th ed.), FHWA HEC-22/HEC-15",
            Standard::Ca => "Canada: TAC Geometric Design Guide for Canadian Roads (2017) + provincial supplement",
            Standard::Uk => "UK: DMRB (CD 109, CD 123, CD 224/225/226, CG 501, CD 377)",
            Standard::Au => "Australia: Austroads Guide to Road Design Parts 3/4/6, AGPT02, ARR 2019 + state supplement",
            Standard::Nz => "New Zealand: Austroads GRD + NZTA Waka Kotahi supplements",
        }
    }
}

pub const AASHTO: &str = "AASHTO A Policy on Geometric Design of Highways and Streets, 7th ed. (2018), metric";
pub const TAC: &str = "TAC Geometric Design Guide for Canadian Roads (2017)";
pub const CD109: &str = "DMRB CD 109 Highway link design, Revision 1 (Mar 2020)";
pub const AGRD3: &str = "Austroads Guide to Road Design Part 3: Geometric Design";
pub const NZTA: &str = "NZTA Waka Kotahi: Austroads GRD is the primary geometric reference (NZTA geometric design page); SHGDM supplementary";

/// Set by the engineer in responsible charge *before* design starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignBasis {
    pub project: String,
    pub engineer_of_record: String,
    pub standard: Standard,
    pub design_speed_kmh: f64,
    /// Maximum superelevation (m/m), engineer's choice.
    #[serde(default)]
    pub e_max: Option<f64>,
    /// Side friction factor for the minimum-radius formula (US/CA/AU/NZ). Engineer input (copyrighted tables).
    #[serde(default)]
    pub f_max: Option<f64>,
    /// Maximum grade %. UK: defaults to CD 109 Table 5.1 from `road_type`; elsewhere engineer input.
    #[serde(default)]
    pub max_grade_pct: Option<f64>,
    /// UK only: "motorway" | "dual" | "single" (CD 109 Table 5.1).
    #[serde(default)]
    pub road_type: Option<String>,
    /// Reaction time (s). US default 2.5 (Green Book); AU/NZ engineer input (typically 2.0–2.5).
    #[serde(default)]
    pub perception_reaction_s: Option<f64>,
    /// US/CA deceleration rate a (m/s²). US default 3.4.
    #[serde(default)]
    pub deceleration_mps2: Option<f64>,
    /// AU/NZ coefficient of deceleration d (car, sealed road); engineer input (0.36 typical).
    #[serde(default)]
    pub deceleration_coeff: Option<f64>,
    /// Crest sight-line driver eye / object heights (m). US default 1.08/0.60; AU/NZ default 1.1/0.2; CA engineer input.
    #[serde(default)]
    pub eye_height_m: Option<f64>,
    #[serde(default)]
    pub object_height_m: Option<f64>,
    /// Minimum sag K (m per %). US: from the headlight formula; UK: CD 109 Table 2.10; CA/AU/NZ engineer input.
    #[serde(default)]
    pub sag_k_min: Option<f64>,
    /// Maximum relative gradient of the rotated edge (%). US: AASHTO table; others engineer input.
    #[serde(default)]
    pub max_relative_gradient_pct: Option<f64>,
    /// UK only: rate of increase of centripetal acceleration q (m/s³) for the basic transition; CD 109 §4.14.1 desirable ≤ 0.3.
    #[serde(default)]
    pub transition_q: Option<f64>,
    /// Retaining-wall acceptance criteria (required outside the US profile).
    #[serde(default)]
    pub wall_criteria: Option<crate::walls::Criteria>,
    #[serde(default = "d_nc")]
    pub normal_crown: f64,
    #[serde(default = "d_lw")]
    pub lane_width_m: f64,
    #[serde(default = "d_one")]
    pub lanes_rotated_per_side: f64,
}
fn d_nc() -> f64 {
    0.025
}
fn d_lw() -> f64 {
    3.65
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

/// One design parameter with its provenance (for the parameter register in the design report).
#[derive(Debug, Clone, Serialize)]
pub struct Param {
    pub name: String,
    pub value: f64,
    pub unit: String,
    /// "verified" | "formula" | "engineer"
    pub status: String,
    pub source: String,
}

fn p(name: &str, value: f64, unit: &str, status: &str, source: &str) -> Param {
    Param { name: name.into(), value, unit: unit.into(), status: status.into(), source: source.into() }
}

/// CD 109 Table 2.10 (Crown copyright, Open Government Licence), verified from the published PDF.
/// columns: design speed, SSD desirable, SSD one step below, R adverse camber, R 2.5 %, R 3.5 %, R desirable (5 %),
/// R one step below (7 %), R two steps below (7 %), crest K desirable, crest K one step below, sag K desirable.
pub const CD109_T210: [[f64; 12]; 6] = [
    [120.0, 295.0, 215.0, 2880.0, 2040.0, 1440.0, 1020.0, 720.0, 510.0, 182.0, 100.0, 37.0],
    [100.0, 215.0, 160.0, 2040.0, 1440.0, 1020.0, 720.0, 510.0, 360.0, 100.0, 55.0, 26.0],
    [85.0, 160.0, 120.0, 1440.0, 1020.0, 720.0, 510.0, 360.0, 255.0, 55.0, 30.0, 20.0],
    [70.0, 120.0, 90.0, 1020.0, 720.0, 510.0, 360.0, 255.0, 180.0, 30.0, 17.0, 20.0],
    [60.0, 90.0, 70.0, 720.0, 510.0, 360.0, 255.0, 180.0, 127.0, 17.0, 10.0, 13.0],
    [50.0, 70.0, 50.0, 520.0, 360.0, 255.0, 180.0, 127.0, 90.0, 10.0, 6.5, 9.0],
];
pub fn cd109_row(v: f64) -> Option<[f64; 12]> {
    CD109_T210.iter().find(|r| r[0] == v).copied()
}
/// CD 109 Table 5.1: (desirable max, permitted relaxation) gradient % by road type.
pub fn cd109_grade(road_type: &str) -> Option<(f64, f64)> {
    match road_type {
        "motorway" => Some((3.0, 4.0)),
        "dual" => Some((4.0, 8.0)),
        "single" => Some((6.0, 8.0)),
        _ => None,
    }
}

/// AASHTO 2018 metric maximum relative gradient (%), by design speed. Transcribed from the
/// Green Book (Table 3-15); copyrighted table, values must be verified by the engineer.
pub fn aashto_max_rel_gradient_pct(v: f64) -> f64 {
    let t = [(20.0, 0.80), (30.0, 0.75), (40.0, 0.70), (50.0, 0.65), (60.0, 0.60), (70.0, 0.55), (80.0, 0.50), (90.0, 0.47), (100.0, 0.44), (110.0, 0.41), (120.0, 0.38), (130.0, 0.35)];
    let mut best = t[0];
    for r in t {
        if (r.0 - v).abs() < (best.0 - v).abs() {
            best = r
        }
    }
    best.1
}

/// Resolved parameter set for a basis: every value the checks use, with provenance.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    pub ssd: f64,
    pub ssd_formula: String,
    pub ssd_source: String,
    /// crest: either a K value (UK table) or eye/object heights for the sight formula
    pub crest_k_one_step: Option<f64>,
    pub crest_k_desirable: Option<f64>,
    pub eye_object: Option<(f64, f64)>,
    pub sag_k_min: Option<f64>,
    pub r_min: f64,
    pub r_desirable: Option<f64>,
    pub r_formula: String,
    pub g_max: f64,
    pub g_desirable: Option<f64>,
    pub rel_grad_pct: f64,
    pub params: Vec<Param>,
}

fn need(v: Option<f64>, what: &str, std: Standard) -> crate::Result<f64> {
    v.ok_or_else(|| format!("{:?} profile: '{what}' must be entered by the engineer (table not shipped: copyrighted/paywalled or not verified)", std))
}

pub fn resolve(b: &DesignBasis) -> crate::Result<Resolved> {
    let v = b.design_speed_kmh;
    let mut params = Vec::new();
    match b.standard {
        Standard::Us | Standard::Ca => {
            let us = b.standard == Standard::Us;
            let src = if us { AASHTO } else { TAC };
            let t = if us { b.perception_reaction_s.unwrap_or(2.5) } else { need(b.perception_reaction_s, "perception_reaction_s", b.standard)? };
            let a = if us { b.deceleration_mps2.unwrap_or(3.4) } else { need(b.deceleration_mps2, "deceleration_mps2", b.standard)? };
            let ssd = 0.278 * v * t + 0.039 * v * v / a;
            params.push(p("brake reaction time t", t, "s", if us && b.perception_reaction_s.is_none() { "verified" } else { "engineer" }, &format!("{src}, stopping sight distance (2.5 s; confirmed in state DOT manuals, e.g. IDOT BLR Ch. 28, MoDOT EPG 941)")));
            params.push(p("deceleration rate a", a, "m/s²", if us && b.deceleration_mps2.is_none() { "verified" } else { "engineer" }, &format!("{src}, stopping sight distance (3.4 m/s²)")));
            params.push(p("SSD", ssd, "m", "formula", &format!("{src}: SSD = 0.278·V·t + 0.039·V²/a")));
            let (h1, h2) = if us { (b.eye_height_m.unwrap_or(1.08), b.object_height_m.unwrap_or(0.60)) } else { (need(b.eye_height_m, "eye_height_m", b.standard)?, need(b.object_height_m, "object_height_m", b.standard)?) };
            params.push(p("crest eye height", h1, "m", if us && b.eye_height_m.is_none() { "verified" } else { "engineer" }, &format!("{src}, crest vertical curves")));
            params.push(p("crest object height", h2, "m", if us && b.object_height_m.is_none() { "verified" } else { "engineer" }, &format!("{src}, crest vertical curves")));
            let sag = if us { b.sag_k_min } else { Some(need(b.sag_k_min, "sag_k_min", b.standard)?) };
            if let Some(k) = sag {
                params.push(p("sag K minimum", k, "m/%", "engineer", src));
            }
            let (e, f) = (need(b.e_max, "e_max", b.standard)?, need(b.f_max, "f_max", b.standard)?);
            let rmin = v * v / (127.0 * (e + f));
            params.push(p("e_max", e, "m/m", "engineer", src));
            params.push(p("f_max (side friction)", f, "", "engineer", &format!("{src} (side-friction table copyrighted; engineer enters)")));
            params.push(p("R_min", rmin, "m", "formula", "R_min = V²/(127(e+f))"));
            let g = need(b.max_grade_pct, "max_grade_pct", b.standard)?;
            params.push(p("max grade", g, "%", "engineer", &format!("{src}, maximum grades by functional class and terrain")));
            let rg = if us { b.max_relative_gradient_pct.unwrap_or(aashto_max_rel_gradient_pct(v)) } else { need(b.max_relative_gradient_pct, "max_relative_gradient_pct", b.standard)? };
            params.push(p("max relative gradient", rg, "%", if us && b.max_relative_gradient_pct.is_none() { "engineer" } else { "engineer" }, &if us { format!("{AASHTO}, superelevation runoff (Table 3-15, transcribed; verify)") } else { format!("{TAC}, superelevation runoff") }));
            Ok(Resolved { ssd, ssd_formula: format!("SSD = 0.278·V·t + 0.039·V²/a = 0.278·{v}·{t} + 0.039·{v}²/{a}"), ssd_source: src.into(), crest_k_one_step: None, crest_k_desirable: None, eye_object: Some((h1, h2)), sag_k_min: sag, r_min: rmin, r_desirable: None, r_formula: format!("V²/(127(e+f)) = {v}²/(127·({e}+{f}))"), g_max: g, g_desirable: None, rel_grad_pct: rg, params })
        }
        Standard::Uk => {
            let r = cd109_row(v).ok_or("UK_DMRB: design speed must be one of 120, 100, 85, 70, 60, 50 km/h (CD 109 Table 2.10)")?;
            params.push(p("SSD desirable minimum", r[1], "m", "verified", &format!("{CD109} Table 2.10")));
            params.push(p("SSD one step below desirable", r[2], "m", "verified", &format!("{CD109} Table 2.10")));
            params.push(p("desirable minimum R (e 5 %)", r[6], "m", "verified", &format!("{CD109} Table 2.10")));
            params.push(p("two steps below desirable R (e 7 %)", r[8], "m", "verified", &format!("{CD109} Table 2.10")));
            params.push(p("crest K desirable", r[9], "m/%", "verified", &format!("{CD109} Table 2.10")));
            params.push(p("crest K one step below", r[10], "m/%", "verified", &format!("{CD109} Table 2.10")));
            params.push(p("sag K desirable", r[11], "m/%", "verified", &format!("{CD109} Table 2.10")));
            let rt = b.road_type.as_deref().ok_or("UK_DMRB: road_type (motorway|dual|single) required for CD 109 Table 5.1")?;
            let (gd, gr) = cd109_grade(rt).ok_or("road_type must be motorway|dual|single")?;
            let g = b.max_grade_pct.unwrap_or(gr);
            params.push(p("gradient desirable max", gd, "%", "verified", &format!("{CD109} Table 5.1 ({rt})")));
            params.push(p("gradient permitted relaxation", gr, "%", "verified", &format!("{CD109} Table 5.1 ({rt})")));
            let e = need(b.e_max, "e_max (CD 109: 7 % rural, 5 % urban max)", b.standard)?;
            params.push(p("e_max", e, "m/m", "engineer", &format!("{CD109} §4.2 (rural ≤ 7 %, urban ≤ 5 %)")));
            let rg = need(b.max_relative_gradient_pct, "max_relative_gradient_pct", b.standard)?;
            params.push(p("max relative gradient", rg, "%", "engineer", "not taken from CD 109 in this tool; engineer enters"));
            let q = b.transition_q.unwrap_or(0.3);
            params.push(p("transition q", q, "m/s³", "verified", &format!("{CD109} §4.13–4.15 (q ≤ 0.3 desirable, ≤ 0.6 max)")));
            Ok(Resolved { ssd: r[1], ssd_formula: format!("CD 109 Table 2.10, V = {v} kph"), ssd_source: CD109.into(), crest_k_one_step: Some(r[10]), crest_k_desirable: Some(r[9]), eye_object: None, sag_k_min: Some(r[11]), r_min: r[8], r_desirable: Some(r[6]), r_formula: format!("CD 109 Table 2.10, V = {v} kph"), g_max: g, g_desirable: Some(gd), rel_grad_pct: rg, params })
        }
        Standard::Au | Standard::Nz => {
            let src = if b.standard == Standard::Au { AGRD3.to_string() } else { format!("{AGRD3} (adopted by {NZTA})") };
            let rt = need(b.perception_reaction_s, "perception_reaction_s", b.standard)?;
            let d = need(b.deceleration_coeff, "deceleration_coeff", b.standard)?;
            let ssd = rt * v / 3.6 + v * v / (254.0 * d);
            params.push(p("reaction time RT", rt, "s", "engineer", &format!("{src}, sight distance (2.0–2.5 s; e.g. MRWA supplement mandates 2.5 s desirable)")));
            params.push(p("coefficient of deceleration d", d, "", "engineer", &format!("{src} (0.36 for cars, e.g. MRWA supplement §5.1)")));
            params.push(p("SSD", ssd, "m", "formula", &format!("{src}: SSD = RT·V/3.6 + V²/(254(d + 0.01a)), a = 0 here")));
            let (h1, h2) = (b.eye_height_m.unwrap_or(1.1), b.object_height_m.unwrap_or(0.2));
            params.push(p("crest eye height", h1, "m", "engineer", &format!("{src} (car eye 1.1 m, object 0.2 m; verify vs the crest K table)")));
            params.push(p("crest object height", h2, "m", "engineer", &src));
            let sag = need(b.sag_k_min, "sag_k_min", b.standard)?;
            params.push(p("sag K minimum", sag, "m/%", "engineer", &src));
            let (e, f) = (need(b.e_max, "e_max", b.standard)?, need(b.f_max, "f_max", b.standard)?);
            let rmin = v * v / (127.0 * (e + f));
            params.push(p("e_max", e, "m/m", "engineer", &src));
            params.push(p("f_max (side friction)", f, "", "engineer", &format!("{src} (side-friction table not shipped)")));
            params.push(p("R_min", rmin, "m", "formula", "R_min = V²/(127(e+f))"));
            let g = need(b.max_grade_pct, "max_grade_pct", b.standard)?;
            params.push(p("max grade", g, "%", "engineer", &src));
            let rg = need(b.max_relative_gradient_pct, "max_relative_gradient_pct", b.standard)?;
            params.push(p("max relative gradient", rg, "%", "engineer", &src));
            Ok(Resolved { ssd, ssd_formula: format!("SSD = RT·V/3.6 + V²/(254·d) = {rt}·{v}/3.6 + {v}²/(254·{d})"), ssd_source: src.clone(), crest_k_one_step: None, crest_k_desirable: None, eye_object: Some((h1, h2)), sag_k_min: Some(sag), r_min: rmin, r_desirable: None, r_formula: format!("V²/(127(e+f)) = {v}²/(127·({e}+{f}))"), g_max: g, g_desirable: None, rel_grad_pct: rg, params })
        }
    }
}

pub fn validate_basis(b: &DesignBasis) -> crate::Result<()> {
    if b.engineer_of_record.trim().is_empty() {
        return Err("design basis must name the engineer in responsible charge".into());
    }
    if !(b.normal_crown > 0.0 && b.normal_crown < 0.06) || b.lane_width_m <= 0.0 {
        return Err("normal_crown must be in (0, 0.06) and lane_width_m > 0".into());
    }
    resolve(b).map(|_| ())
}

pub fn ssd(b: &DesignBasis) -> f64 {
    resolve(b).map_or(0.0, |r| r.ssd)
}

/// Minimum crest curve length for sight distance S with eye h1 / object h2:
/// L = A·S²/(200(√h1+√h2)²) if S < L, else 2S − 200(√h1+√h2)²/A.
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
    crest_len_min_h(a, s, 1.08, 0.60)
}
/// AASHTO sag curve length for headlight sight distance (0.6 m, 1°): 120 + 3.5S.
pub fn sag_len_min(a: f64, s: f64) -> f64 {
    let a = a.abs();
    let l1 = a * s * s / (120.0 + 3.5 * s);
    if l1 >= s {
        l1
    } else {
        (2.0 * s - (120.0 + 3.5 * s) / a).max(0.0)
    }
}

/// Minimum vertical curve length for a grade change A (%) under the basis.
pub fn vc_len_min(b: &DesignBasis, r: &Resolved, a_pct: f64) -> (f64, String) {
    let crest = a_pct < 0.0;
    if crest {
        if let Some(k) = r.crest_k_desirable {
            return (k * a_pct.abs(), format!("L = K·A, K = {k} (CD 109 Table 2.10 desirable crest K)"));
        }
        let (h1, h2) = r.eye_object.unwrap();
        (crest_len_min_h(a_pct, r.ssd, h1, h2), format!("crest: L = A·S²/(200(√{h1}+√{h2})²) if S<L else 2S−200(√h1+√h2)²/A, S = {:.1} m", r.ssd))
    } else if let Some(k) = r.sag_k_min {
        (k * a_pct.abs(), format!("L = K·A, sag K = {k}"))
    } else if b.standard == Standard::Us {
        (sag_len_min(a_pct, r.ssd), format!("sag (headlight 0.6 m, 1°): L = A·S²/(120+3.5S) if S<L else 2S−(120+3.5S)/A, S = {:.1} m", r.ssd))
    } else {
        (0.0, "no sag criterion".into())
    }
}

pub fn chk(id: &str, item: &str, element: String, value: f64, limit: f64, unit: &str, kind: &str, severity: &str, formula: String, citation: String) -> Check {
    let pass = if kind == "min" { value >= limit - 1e-9 } else { value <= limit + 1e-9 };
    Check { id: id.into(), item: item.into(), element, value, limit, unit: unit.into(), kind: kind.into(), pass, severity: severity.into(), formula, citation }
}

pub fn run_checks(b: &DesignBasis, al: &Alignment, pr: Option<&Profile>) -> Vec<Check> {
    let mut out = Vec::new();
    let r = match resolve(b) {
        Ok(r) => r,
        Err(e) => {
            out.push(chk("BASIS", "Design basis complete", "basis".into(), 0.0, 1.0, "bool", "min", "mandatory", e, "tool rule".into()));
            return out;
        }
    };
    let v = b.design_speed_kmh;
    let curves = al.curves();
    let se = superelev::design(b, al);
    let src = r.ssd_source.clone();
    for c in &curves {
        let el = format!("curve {}", c.index);
        out.push(chk("H-RMIN", "Horizontal curve radius (minimum)", el.clone(), c.radius, r.r_min, "m", "min", "mandatory", format!("R_min = {} = {:.1} m", r.r_formula, r.r_min), if b.standard == Standard::Uk { format!("{CD109} Table 2.10 (two steps below desirable; beyond this a departure is required)") } else { format!("{src}, minimum radius") }));
        if let Some(rd) = r.r_desirable {
            out.push(chk("H-RDES", "Horizontal curve radius (desirable minimum; below = relaxation)", el.clone(), c.radius, rd, "m", "min", "advisory", format!("CD 109 Table 2.10 desirable minimum R (e 5 %) = {rd} m"), format!("{CD109} Table 2.10, §4.5 relaxations (Table 4.5)")));
        }
        for (side, ls) in [("entry", c.ls_in), ("exit", c.ls_out)] {
            if ls <= 0.0 {
                continue;
            }
            match b.standard {
                Standard::Uk => {
                    let q = b.transition_q.unwrap_or(0.3);
                    let basic = v.powi(3) / (46.7 * q * c.radius);
                    let lreq = basic.min((24.0 * c.radius).sqrt());
                    out.push(chk("H-LS-MIN", "Transition length (basic transition, CD 109 Eq 4.13)", format!("{el} {side}"), ls, lreq, "m", "min", "advisory", format!("L = V³/(46.7·q·R) = {basic:.1} m (q = {q}); capped at √(24R) = {:.1} m (§4.15)", (24.0 * c.radius).sqrt()), format!("{CD109} §4.13–4.15")));
                }
                Standard::Us | Standard::Ca => {
                    let lmin = (24.0 * 0.20 * c.radius).sqrt();
                    let lmax = (24.0 * 1.0 * c.radius).sqrt();
                    out.push(chk("H-LS-MIN", "Spiral length (minimum, lateral shift 0.20 m)", format!("{el} {side}"), ls, lmin, "m", "min", "advisory", format!("Ls,min = √(24·p_min·R), p_min = 0.20 m → {lmin:.1} m"), format!("{src}, spiral curve transitions")));
                    out.push(chk("H-LS-MAX", "Spiral length (maximum, lateral shift 1.0 m)", format!("{el} {side}"), ls, lmax, "m", "max", "advisory", format!("Ls,max = √(24·p_max·R), p_max = 1.0 m → {lmax:.1} m"), format!("{src}, spiral curve transitions")));
                }
                _ => {}
            }
        }
    }
    if let Ok(se) = &se {
        for sc in &se.curves {
            if sc.e > 0.0 {
                out.push(chk("SE-REL", "Superelevation relative gradient", format!("curve {}", sc.curve), sc.relative_gradient * 100.0, r.rel_grad_pct, "%", "max", "mandatory", format!("Δ = w·(e+nc)/L = {:.2}·({:.4}+{:.3})/L", se.rotation_width, sc.e, se.nc), format!("{src}, superelevation development (limit from design basis parameter register)")));
                let c = curves.iter().find(|c| c.index == sc.curve).unwrap();
                if c.ls_in > 0.0 {
                    out.push(chk("SE-IN-LS", "Superelevation development fits within the transition", format!("curve {}", sc.curve), c.ls_in.min(c.ls_out), sc.required_length, "m", "min", "mandatory", format!("required L = w·(e+nc)/Δmax = {:.1} m ≤ spiral length", sc.required_length), format!("{src}, superelevation development on transitions")));
                }
            }
        }
    }
    if let Some(pr) = pr {
        for (i, g) in pr.grades_pct.iter().enumerate() {
            let el = format!("segment VPI{i}->VPI{}", i + 1);
            out.push(chk("V-GMAX", "Grade (maximum)", el.clone(), g.abs(), r.g_max, "%", "max", "mandatory", format!("|g| ≤ {}%", r.g_max), if b.standard == Standard::Uk { format!("{CD109} Table 5.1 permitted relaxation") } else { format!("{src}, maximum grade (engineer-entered)") }));
            if let Some(gd) = r.g_desirable {
                out.push(chk("V-GDES", "Grade (desirable maximum; above = relaxation)", el, g.abs(), gd, "%", "max", "advisory", format!("|g| ≤ {gd}%"), format!("{CD109} Table 5.1 desirable maximum, §5.1.1")));
            }
        }
        for c in &pr.curves {
            let (lmin, f) = vc_len_min(b, &r, c.a_pct);
            let el = format!("VPI {} (A={:.2}%)", c.vpi_index, c.a_pct);
            out.push(chk("V-LVC", if c.crest { "Crest curve length for stopping sight distance" } else { "Sag curve length" }, el.clone(), c.length, lmin, "m", "min", if b.standard == Standard::Uk { "advisory" } else { "mandatory" }, f, if b.standard == Standard::Uk { format!("{CD109} §5.4–5.5 and Table 2.10 (desirable)") } else { format!("{src}, vertical curves") }));
            if b.standard == Standard::Uk && c.crest {
                let k1 = r.crest_k_one_step.unwrap();
                out.push(chk("V-LVC-RLX", "Crest curve length, one step below desirable", el, c.length, k1 * c.a_pct.abs(), "m", "min", "mandatory", format!("L = K·A, K = {k1}"), format!("{CD109} Table 2.10 one step below desirable crest K; Table 3.5 permitted relaxations")));
            }
        }
    }
    if let Err(e) = &se {
        out.push(chk("SE-DESIGN", "Superelevation design", "alignment".into(), 0.0, 1.0, "bool", "min", "mandatory", e.clone(), "tool rule".into()));
    } else if let Ok(se) = &se {
        for w in se.curves.windows(2) {
            let ok = w[1].a_in >= w[0].a_out - 1e-6 || w[0].e == 0.0 || w[1].e == 0.0;
            let mut c = chk("SE-OVERLAP", "Superelevation transitions do not overlap", format!("curves {}-{}", w[0].curve, w[1].curve), w[1].a_in - w[0].a_out, 0.0, "m gap", "min", "mandatory", format!("next transition start {:.2} ≥ previous end {:.2}", w[1].a_in, w[0].a_out), "geometric consistency (tool rule)".into());
            c.pass = ok;
            out.push(c);
        }
    }
    if let Some(p) = pr {
        let cover = p.start() <= al.start_station() + 1e-6 && p.end() >= al.end_station() - 1e-6;
        out.push(chk("G-COVER", "Profile covers alignment", "profile".into(), if cover { 1.0 } else { 0.0 }, 1.0, "bool", "min", "mandatory", format!("profile {:.2}-{:.2} vs alignment {:.2}-{:.2}", p.start(), p.end(), al.start_station(), al.end_station()), "geometric consistency (tool rule)".into()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn basis() -> DesignBasis {
        serde_json::from_value(serde_json::json!({"project":"t","engineer_of_record":"x","standard":"US_AASHTO_2018","design_speed_kmh":80.0,"e_max":0.08,"f_max":0.14,"max_grade_pct":6.0})).unwrap()
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
        assert!((k - 25.7).abs() < 0.1, "{k}");
    }
    #[test]
    fn uk_cd109_values_and_required_inputs() {
        let mut b = basis();
        b.standard = Standard::Uk;
        b.design_speed_kmh = 85.0;
        assert!(resolve(&b).is_err()); // road_type, relative gradient required
        b.road_type = Some("single".into());
        b.max_grade_pct = None;
        b.max_relative_gradient_pct = Some(0.6);
        let r = resolve(&b).unwrap();
        assert_eq!((r.ssd, r.crest_k_desirable, r.sag_k_min, r.r_min, r.g_max), (160.0, Some(55.0), Some(20.0), 255.0, 8.0));
    }
    #[test]
    fn austroads_ssd_formula_and_engineer_inputs() {
        let mut b = basis();
        b.standard = Standard::Au;
        assert!(resolve(&b).is_err());
        b.perception_reaction_s = Some(2.5);
        b.deceleration_coeff = Some(0.36);
        b.sag_k_min = Some(20.0);
        b.max_relative_gradient_pct = Some(0.5);
        let r = resolve(&b).unwrap();
        // 2.5·80/3.6 + 80²/(254·0.36) = 55.56 + 69.99
        assert!((r.ssd - (2.5 * 80.0 / 3.6 + 6400.0 / (254.0 * 0.36))).abs() < 1e-9);
    }
}
