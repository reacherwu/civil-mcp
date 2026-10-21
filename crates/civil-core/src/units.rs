//! Unit systems. civil-mcp designs and checks in SI internally; US customary
//! is applied at the boundaries: MCP tool inputs and outputs, drawing text,
//! scales and stationing, reports, the bill of quantities and CSV/Markdown
//! files.
//!
//! Foot definitions: the international foot is exactly 0.3048 m. The US survey
//! foot is exactly 1200/3937 m; NIST and NGS deprecated it on 2022-12-31, but
//! older state plane coordinate data still use it. The selected foot applies to
//! lengths, coordinates, stations, areas and volumes. Speeds, pressures, forces
//! and inches always use the international definitions.
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cell::Cell;

pub const INTL_FT: f64 = 0.3048;
pub const SURVEY_FT: f64 = 1200.0 / 3937.0;
pub const INCH: f64 = 0.0254;
pub const LBF: f64 = 4.448_221_615_260_5;
pub const PSI: f64 = LBF / (INCH * INCH);
pub const PSF: f64 = LBF / (INTL_FT * INTL_FT);
pub const SHORT_TON_KG: f64 = 907.184_74;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum System {
    #[default]
    #[serde(rename = "SI", alias = "si", alias = "metric")]
    Si,
    #[serde(rename = "US", alias = "us", alias = "US_customary", alias = "imperial")]
    Us,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Foot {
    #[default]
    #[serde(rename = "international", alias = "intl")]
    International,
    #[serde(rename = "survey", alias = "us_survey")]
    Survey,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Units {
    pub system: System,
    pub foot: Foot,
}

impl Units {
    pub const SI: Units = Units { system: System::Si, foot: Foot::International };
    pub const US: Units = Units { system: System::Us, foot: Foot::International };
    pub fn us(&self) -> bool {
        self.system == System::Us
    }
    /// metres per foot for lengths, coordinates and stations
    pub fn ft(&self) -> f64 {
        match self.foot {
            Foot::International => INTL_FT,
            Foot::Survey => SURVEY_FT,
        }
    }
    pub fn label(&self) -> &'static str {
        match (self.system, self.foot) {
            (System::Si, _) => "SI (metres)",
            (System::Us, Foot::International) => "US customary (international foot)",
            (System::Us, Foot::Survey) => "US customary (US survey foot)",
        }
    }
}

/// Unit settings of a session. `system` drives drawings, reports, BoQ and
/// files; `inputs` and `outputs` drive the MCP JSON boundary.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitSpec {
    pub system: System,
    #[serde(default)]
    pub foot: Foot,
    #[serde(default)]
    pub inputs: Option<System>,
    #[serde(default)]
    pub outputs: Option<System>,
}

impl UnitSpec {
    pub fn files(&self) -> Units {
        Units { system: self.system, foot: self.foot }
    }
    pub fn inputs(&self) -> Units {
        Units { system: self.inputs.unwrap_or(self.system), foot: self.foot }
    }
    pub fn outputs(&self) -> Units {
        Units { system: self.outputs.unwrap_or(self.system), foot: self.foot }
    }
}

thread_local! {
    static CUR: Cell<Units> = const { Cell::new(Units::SI) };
}

/// Units used for drawing text, reports and files on this thread.
pub fn cur() -> Units {
    CUR.with(|c| c.get())
}
pub fn set_cur(u: Units) {
    CUR.with(|c| c.set(u))
}
/// Run `f` with `u` as the current units, restoring the previous value.
pub fn with<T>(u: Units, f: impl FnOnce() -> T) -> T {
    let old = cur();
    set_cur(u);
    let r = f();
    set_cur(old);
    r
}

// ---------- quantity kinds ----------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Length,
    Area,
    Volume,
    HaulMoment,
    Flow,
    Small,
    SmallArea,
    Speed,
    Velocity,
    Accel,
    Hectare,
    Kpa,
    Mpa,
    UnitWeight,
    Kn,
    Rain,
}

impl Kind {
    /// factor f: value_us = value_si / f
    pub fn factor(self, u: Units) -> f64 {
        let ft = u.ft();
        match self {
            Kind::Length => ft,
            Kind::Area => ft * ft,
            Kind::Volume => 27.0 * ft * ft * ft,
            Kind::HaulMoment => 27.0 * ft * ft * ft * ft,
            Kind::Flow => INTL_FT.powi(3),
            Kind::Small => 25.4,
            Kind::SmallArea => 645.16,
            Kind::Speed => 1.609_344,
            Kind::Velocity => INTL_FT,
            Kind::Accel => INTL_FT,
            Kind::Hectare => 43_560.0 * ft * ft / 10_000.0,
            Kind::Kpa => PSF / 1000.0,
            Kind::Mpa => PSI / 1.0e6,
            Kind::UnitWeight => LBF / INTL_FT.powi(3) / 1000.0,
            Kind::Kn => LBF,
            Kind::Rain => 25.4,
        }
    }
    pub fn us_unit(self) -> &'static str {
        match self {
            Kind::Length => "ft",
            Kind::Area => "ft²",
            Kind::Volume => "CY",
            Kind::HaulMoment => "CY·ft",
            Kind::Flow => "ft³/s",
            Kind::Small => "in",
            Kind::SmallArea => "in²",
            Kind::Speed => "mph",
            Kind::Velocity => "ft/s",
            Kind::Accel => "ft/s²",
            Kind::Hectare => "ac",
            Kind::Kpa => "psf",
            Kind::Mpa => "psi",
            Kind::UnitWeight => "pcf",
            Kind::Kn => "lbf",
            Kind::Rain => "in/h",
        }
    }
}

pub fn to_us(v: f64, k: Kind, u: Units) -> f64 {
    v / k.factor(u)
}
pub fn to_si(v: f64, k: Kind, u: Units) -> f64 {
    v * k.factor(u)
}

/// Length in the current units.
pub fn len(m: f64) -> f64 {
    let u = cur();
    if u.us() {
        m / u.ft()
    } else {
        m
    }
}
/// Length unit label in the current units.
pub fn lu() -> &'static str {
    if cur().us() {
        "ft"
    } else {
        "m"
    }
}
/// Pipe diameter label: DN in mm (SI) or inches (US).
pub fn dia(d_m: f64) -> String {
    if cur().us() {
        format!("{:.0}\"", d_m / INCH)
    } else {
        format!("{:.0}", d_m * 1000.0)
    }
}

/// Station label: SI `k+mmm.mm` (km + metres), US `ss+ff.ff` (100 ft stations).
pub fn sta(s_m: f64) -> String {
    sta_in(s_m, cur())
}
pub fn sta_in(s_m: f64, u: Units) -> String {
    let (v, base, w) = if u.us() { (s_m / u.ft(), 100.0, 5) } else { (s_m, 1000.0, 6) };
    let neg = v < 0.0;
    let a = (v.abs() * 100.0).round() / 100.0;
    let k = (a / base).floor();
    let r = a - k * base;
    format!("{}{}+{:0w$.2}", if neg { "-" } else { "" }, k as i64, r, w = w)
}
/// Parse a station label in either format back to metres.
pub fn parse_sta(s: &str, u: Units) -> Option<f64> {
    let s = s.trim();
    let (neg, s) = s.strip_prefix('-').map_or((false, s), |r| (true, r));
    let (a, b) = s.split_once('+')?;
    let (a, b): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
    let v = if u.us() { (a * 100.0 + b) * u.ft() } else { a * 1000.0 + b };
    Some(if neg { -v } else { v })
}

/// Engineering scales (feet per inch) used for US sheets.
pub const US_PLAN_FT_PER_IN: &[f64] = &[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 100.0, 200.0, 300.0, 400.0, 500.0, 1000.0, 2000.0];
pub const US_VERT_FT_PER_IN: &[f64] = &[1.0, 2.0, 4.0, 5.0, 10.0, 20.0, 40.0, 50.0, 100.0];

/// Ratio 1:n of an engineering scale (international foot: 1" = 50' is 1:600).
pub fn ratio_of_ft_per_in(f: f64) -> f64 {
    f * 12.0
}

/// Smallest preferred layout ratio n >= need (or `need` rounded up when none).
pub fn pick_scale(need: f64, si: &[f64], us_ft_per_in: &[f64]) -> f64 {
    if cur().us() {
        us_ft_per_in.iter().map(|f| ratio_of_ft_per_in(*f)).find(|&n| n >= need - 1e-9).unwrap_or(need.ceil())
    } else {
        si.iter().cloned().find(|&n| n >= need).unwrap_or(need.ceil())
    }
}

/// Scale label: `1:n` (SI) or `1" = X'` (US).
pub fn scale(n: f64) -> String {
    if cur().us() {
        let f = n / 12.0;
        if (f - f.round()).abs() < 0.05 {
            format!("1\" = {:.0}'", f)
        } else {
            format!("1\" = {:.1}'", f)
        }
    } else {
        format!("1:{n:.0}")
    }
}

// ---------- BoQ ----------

/// Convert a BoQ quantity and unit to the current system:
/// m³ → CY, m² → SY, m → LF, t → TON (short ton), ha → AC.
pub fn boq(unit: &str, qty: f64) -> (String, f64) {
    boq_in(unit, qty, cur())
}
pub fn boq_in(unit: &str, qty: f64, u: Units) -> (String, f64) {
    if !u.us() {
        return (unit.into(), qty);
    }
    let ft = u.ft();
    match unit {
        "m³" | "m3" => ("CY".into(), qty / (27.0 * ft.powi(3))),
        "m²" | "m2" => ("SY".into(), qty / (9.0 * ft * ft)),
        "m" => ("LF".into(), qty / ft),
        "t" => ("TON".into(), qty * 1000.0 / SHORT_TON_KG),
        "ha" => ("AC".into(), qty * 10_000.0 / (43_560.0 * ft * ft)),
        "each" | "nr" => ("EA".into(), qty),
        o => (o.into(), qty),
    }
}

// ---------- free-text conversion ----------

struct TU {
    si: &'static str,
    us: &'static str,
    kind: Kind,
    extra: usize,
}

const TEXT_UNITS: &[TU] = &[
    TU { si: "m³/s", us: "ft³/s", kind: Kind::Flow, extra: 1 },
    TU { si: "m³·m", us: "CY·ft", kind: Kind::HaulMoment, extra: 0 },
    TU { si: "m³", us: "CY", kind: Kind::Volume, extra: 0 },
    TU { si: "m²", us: "ft²", kind: Kind::Area, extra: 0 },
    TU { si: "m/s²", us: "ft/s²", kind: Kind::Accel, extra: 0 },
    TU { si: "m/s", us: "ft/s", kind: Kind::Velocity, extra: 0 },
    TU { si: "mm²", us: "in²", kind: Kind::SmallArea, extra: 2 },
    TU { si: "mm/h", us: "in/h", kind: Kind::Rain, extra: 2 },
    TU { si: "mm", us: "in", kind: Kind::Small, extra: 2 },
    TU { si: "km/h", us: "mph", kind: Kind::Speed, extra: 0 },
    TU { si: "kN/m³", us: "pcf", kind: Kind::UnitWeight, extra: 0 },
    TU { si: "kN", us: "lbf", kind: Kind::Kn, extra: 0 },
    TU { si: "kPa", us: "psf", kind: Kind::Kpa, extra: 0 },
    TU { si: "MPa", us: "psi", kind: Kind::Mpa, extra: 0 },
    TU { si: "ha", us: "ac", kind: Kind::Hectare, extra: 1 },
    TU { si: "m", us: "ft", kind: Kind::Length, extra: 0 },
];

fn fmt_dec(v: f64, dec: usize) -> String {
    format!("{v:.dec$}")
}

/// Convert `<number> <SI unit>` tokens and metric station labels in free
/// text to the current units. Unitless numbers are left unchanged.
pub fn text(s: &str) -> String {
    let u = cur();
    if !u.us() || s.is_empty() {
        return s.to_string();
    }
    text_in(s, u)
}

pub fn text_in(s: &str, u: Units) -> String {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    let word = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '.' || ch == '#';
    while i < n {
        let ch = c[i];
        let start_ok = ch.is_ascii_digit() && (i == 0 || !word(c[i - 1]));
        if !start_ok {
            out.push(ch);
            i += 1;
            continue;
        }
        // number
        let mut j = i;
        while j < n && c[j].is_ascii_digit() {
            j += 1;
        }
        let int_end = j;
        let mut dec = 0;
        if j + 1 < n && c[j] == '.' && c[j + 1].is_ascii_digit() {
            j += 1;
            while j < n && c[j].is_ascii_digit() {
                j += 1;
                dec += 1;
            }
        }
        // metric station k+mmm(.mm)
        if dec == 0 && j < n && c[j] == '+' && j + 3 < n && c[j + 1..j + 4].iter().all(|x| x.is_ascii_digit()) && (j + 4 >= n || !c[j + 4].is_ascii_digit()) {
            let mut k = j + 4;
            if k + 1 < n && c[k] == '.' && c[k + 1].is_ascii_digit() {
                k += 1;
                while k < n && c[k].is_ascii_digit() {
                    k += 1;
                }
            }
            if k >= n || !word(c[k]) || c[k] == '.' {
                let lab: String = c[i..k].iter().collect();
                if let Some(m) = parse_sta(&lab, Units::SI) {
                    out.push_str(&sta_in(m, u));
                    i = k;
                    continue;
                }
            }
        }
        let num: String = c[i..j].iter().collect();
        let _ = int_end;
        // optional single space, then unit
        let mut k = j;
        let sp = k < n && c[k] == ' ';
        if sp {
            k += 1;
        }
        let hit = unit_at(&c, k);
        match (hit, num.parse::<f64>()) {
            (Some((t, e)), Ok(v)) => {
                let neg = out.ends_with('-') && (out.len() < 2 || !out[..out.len() - 1].ends_with(|x: char| x.is_alphanumeric()));
                let _ = neg;
                let w = to_us(v, t.kind, u);
                let mut d = dec + t.extra;
                if t.kind == Kind::Speed && dec == 0 && (w - w.round()).abs() > 0.05 {
                    d = 1;
                }
                if t.kind == Kind::Length && dec == 0 {
                    d = 1;
                }
                out.push_str(&fmt_dec(w, d));
                if sp {
                    out.push(' ');
                }
                out.push_str(t.us);
                i = e;
            }
            (None, Ok(v)) if list_unit(&c, j).is_some() => {
                // number in a list or comparison whose unit follows later ("1136 vs 3570 m", "[3.5, 3.6] m")
                let t = list_unit(&c, j).unwrap();
                let w = to_us(v, t.kind, u);
                let d = dec + t.extra + usize::from(t.kind == Kind::Length && dec == 0);
                out.push_str(&fmt_dec(w, d));
                i = j;
                // "...3.6] m": emit the brackets and the converted unit
                let mut k = j;
                while k < n && (c[k] == ']' || c[k] == ')') {
                    k += 1;
                }
                if k > j {
                    let k2 = if k < n && c[k] == ' ' { k + 1 } else { k };
                    if let Some((t2, e)) = unit_at(&c, k2) {
                        out.extend(c[j..k2].iter());
                        out.push_str(t2.us);
                        i = e;
                    }
                }
            }
            _ => {
                out.push_str(&num);
                i = j;
            }
        }
    }
    out
}

fn unit_at(c: &[char], k: usize) -> Option<(&'static TU, usize)> {
    let n = c.len();
    for t in TEXT_UNITS {
        let tc: Vec<char> = t.si.chars().collect();
        if k + tc.len() <= n && c[k..k + tc.len()] == tc[..] {
            let e = k + tc.len();
            if e >= n || !(c[e].is_alphanumeric() || c[e] == '²' || c[e] == '³' || c[e] == '_') {
                return Some((t, e));
            }
        }
    }
    None
}

/// Unit of a number list / comparison starting after a unitless number at `j`.
fn list_unit(c: &[char], mut j: usize) -> Option<&'static TU> {
    let n = c.len();
    let seps: [&[char]; 5] = [&[' ', 'v', 's', ' '], &[',', ' '], &[' ', 't', 'o', ' '], &['–'], &[' ', 'a', 'n', 'd', ' ']];
    for _ in 0..12 {
        // closing bracket then unit
        let mut k = j;
        while k < n && (c[k] == ']' || c[k] == ')') {
            k += 1;
        }
        if k > j {
            let k2 = if k < n && c[k] == ' ' { k + 1 } else { k };
            return unit_at(c, k2).map(|x| x.0);
        }
        let sep = seps.iter().find(|s| j + s.len() <= n && c[j..j + s.len()] == s[..])?;
        k = j + sep.len();
        if k < n && (c[k] == '-' || c[k] == '+') {
            k += 1;
        }
        if k >= n || !c[k].is_ascii_digit() {
            return None;
        }
        while k < n && (c[k].is_ascii_digit() || (c[k] == '.' && k + 1 < n && c[k + 1].is_ascii_digit())) {
            k += 1;
        }
        let k2 = if k < n && c[k] == ' ' { k + 1 } else { k };
        if let Some((t, _)) = unit_at(c, k2) {
            return Some(t);
        }
        j = k;
    }
    None
}

// ---------- JSON boundary ----------

/// SI key suffix → (US suffix, kind). Longest suffix first.
const SUFFIX: &[(&str, &str, Kind)] = &[
    ("_mm_h", "_in_h", Kind::Rain),
    ("_mmh", "_inh", Kind::Rain),
    ("_m3m", "_cyft", Kind::HaulMoment),
    ("_m3s", "_cfs", Kind::Flow),
    ("_mps2", "_fps2", Kind::Accel),
    ("_kn_m3", "_pcf", Kind::UnitWeight),
    ("_knm3", "_pcf", Kind::UnitWeight),
    ("_mm2", "_in2", Kind::SmallArea),
    ("_mps", "_fps", Kind::Velocity),
    ("_kmh", "_mph", Kind::Speed),
    ("_kpa", "_psf", Kind::Kpa),
    ("_mpa", "_psi", Kind::Mpa),
    ("_m3", "_cy", Kind::Volume),
    ("_m2", "_sf", Kind::Area),
    ("_mm", "_in", Kind::Small),
    ("_ms", "_fps", Kind::Velocity),
    ("_ha", "_ac", Kind::Hectare),
    ("_kn", "_lbf", Kind::Kn),
    ("_m", "_ft", Kind::Length),
];

/// Unsuffixed keys whose numbers are lengths (coordinates, stations, elevations, radii).
const LENGTH_KEYS: &[&str] = &[
    "station",
    "stations",
    "main_station",
    "start_station",
    "sample_stations",
    "balance_stations",
    "vpi_stations",
    "pis",
    "vpis",
    "vpis_after",
    "vertices",
    "radii",
    "spirals",
    "curve_lengths",
    "curve_lengths_after",
    "radius",
    "length",
    "x",
    "y",
    "rim",
    "inv_in",
    "inv_out",
    "xc",
    "zc",
    "crest_x",
    "toe_x",
    "x_entry",
    "x_exit",
    "x_max",
    "min_z",
    "max_z",
    "z_top",
    "polyline",
    "polygon",
    "daylight_points",
    "bbox",
    "start",
    "end",
    "hazard_from",
    "hazard_to",
];

fn skip_key(k: &str) -> bool {
    k.contains("_per_") || k.contains("_day") || k.contains("_kn_m") && !k.ends_with("_kn_m3") || k.contains("_mpa_") || k.ends_with("_pct") || k == "frame"
}

thread_local! {
    static PLAIN_LEN: Cell<bool> = const { Cell::new(true) };
}

/// Tools whose unsuffixed `x`/`y` are not coordinates (signal v/c and flow ratios).
pub const RATIO_XY_TOOLS: &[&str] = &["design_signals", "analyse_capacity"];

/// Enable or disable conversion of unsuffixed length keys (per tool call).
pub fn set_plain_lengths(on: bool) {
    PLAIN_LEN.with(|c| c.set(on))
}

fn plain_len(k: &str) -> bool {
    LENGTH_KEYS.contains(&k) && (PLAIN_LEN.with(|c| c.get()) || !matches!(k, "x" | "y"))
}

fn kind_of_si(k: &str) -> Option<(String, Kind)> {
    if skip_key(k) {
        return None;
    }
    for (si, us, kd) in SUFFIX {
        if let Some(b) = k.strip_suffix(si) {
            if !b.is_empty() {
                return Some((format!("{b}{us}"), *kd));
            }
        }
    }
    if plain_len(k) {
        return Some((k.to_string(), Kind::Length));
    }
    None
}

fn kind_of_us(k: &str) -> Option<(String, Kind)> {
    if skip_key(k) {
        return None;
    }
    for (si, us, kd) in SUFFIX {
        if let Some(b) = k.strip_suffix(us) {
            if !b.is_empty() {
                return Some((format!("{b}{si}"), *kd));
            }
        }
    }
    None
}

fn map_nums(v: &Value, f: &dyn Fn(f64) -> f64) -> Value {
    match v {
        Value::Number(n) => n.as_f64().map(|x| json!(f(x))).unwrap_or(v.clone()),
        Value::Array(a) => Value::Array(a.iter().map(|x| map_nums(x, f)).collect()),
        Value::Object(_) => v.clone(),
        _ => v.clone(),
    }
}

/// Convert tool arguments to SI. Keys with US suffixes (`_ft`, `_mph`, `_in`,
/// `_cfs`, `_ac`, `_psf`, `_psi`, `_pcf`, ...) are converted and renamed to the
/// SI key. Keys with SI suffixes are taken as SI (a per-value override). When
/// `u` is US, unsuffixed length keys (stations, coordinates, radii, `pis`,
/// `vpis`, `vertices`) are read in feet.
pub fn args_to_si(v: &Value, u: Units) -> Value {
    match v {
        Value::Object(m) => {
            let mut o = Map::new();
            for (k, x) in m {
                if k == "frame" || k == "units" {
                    o.insert(k.clone(), x.clone());
                    continue;
                }
                if let Some((sk, kd)) = kind_of_us(k) {
                    let f = kd.factor(u);
                    o.insert(sk, map_nums(&args_to_si(x, u), &|y| y * f));
                } else if u.us() && plain_len(k) && !x.is_object() {
                    let f = u.ft();
                    o.insert(k.clone(), map_nums(x, &|y| y * f));
                } else {
                    o.insert(k.clone(), args_to_si(x, u));
                }
            }
            Value::Object(o)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| args_to_si(x, u)).collect()),
        _ => v.clone(),
    }
}

fn round_sig(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let d = 9 - x.abs().log10().floor() as i32;
    if d <= 0 {
        return x.round();
    }
    let p = 10f64.powi(d.min(15));
    (x * p).round() / p
}

/// Convert a tool result from SI to US customary: SI-suffixed keys are
/// renamed and converted, unsuffixed length keys are converted, and strings
/// go through [`text_in`].
pub fn result_to_us(v: &Value, u: Units) -> Value {
    match v {
        Value::Object(m) if m.get("unit").and_then(|x| x.as_str()).and_then(unit_kind).is_some() && (m.contains_key("value") || m.contains_key("limit")) => {
            let kd = unit_kind(m["unit"].as_str().unwrap_or("")).unwrap();
            let f = kd.factor(u);
            let mut o = Map::new();
            for (k, x) in m {
                match k.as_str() {
                    "value" | "limit" => {
                        o.insert(k.clone(), map_nums(x, &|y| round_sig(y / f)));
                    }
                    "unit" => {
                        o.insert(k.clone(), json!(kd.us_unit()));
                    }
                    _ => {
                        o.insert(k.clone(), result_to_us(x, u));
                    }
                }
            }
            Value::Object(o)
        }
        Value::Object(m) => {
            let mut o = Map::new();
            for (k, x) in m {
                if k == "units" || k == "frame" || k == "path" || k == "file" || k.ends_with("hash") || k == "head" {
                    o.insert(k.clone(), x.clone());
                    continue;
                }
                match kind_of_si(k) {
                    Some((uk, kd)) if !x.is_object() => {
                        let f = kd.factor(u);
                        o.insert(uk, map_nums(&result_to_us(x, u), &|y| round_sig(y / f)));
                    }
                    _ => {
                        o.insert(k.clone(), result_to_us(x, u));
                    }
                }
            }
            Value::Object(o)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| result_to_us(x, u)).collect()),
        Value::String(s) => Value::String(text_in(s, u)),
        _ => v.clone(),
    }
}

/// Description of the unit system for tool results and files.
pub fn describe(spec: &UnitSpec, defaulted: bool) -> Value {
    json!({
        "system": spec.system,
        "foot": spec.foot,
        "foot_m": spec.files().ft(),
        "inputs": spec.inputs().system,
        "outputs": spec.outputs().system,
        "default": defaulted,
        "label": spec.files().label(),
        "internal": "SI; conversion at the tool, drawing, report and file boundaries",
        "us_units": "ft, ft², CY, ft³/s, in, mph, ac, psf, psi, pcf, lbf, in/h; BoQ CY, SY, LF, TON (short), EA; stations 12+34.56 (100 ft)",
        "notes": "international foot = 0.3048 m exactly; US survey foot = 1200/3937 m (deprecated by NIST/NGS 2022-12-31, kept for legacy state plane data)",
    })
}

/// Unsuffixed CSV columns that hold lengths.
const CSV_LENGTH_COLS: &[&str] = &[
    "station",
    "easting",
    "northing",
    "elevation",
    "fg_elev",
    "eg_elev",
    "TS_or_PC",
    "SC",
    "CS",
    "ST_or_PT",
    "rim",
    "invert",
    "inv_in",
    "inv_out",
    "from_station",
    "to_station",
    "sta_from",
    "sta_to",
];

/// Convert a CSV file (SI) to the given US units: columns with SI suffixes
/// are renamed and converted, unsuffixed length columns are converted, other
/// cells go through [`text_in`]. `#` lines are comments; the line after a
/// comment (or the first line) is a header.
pub fn csv_in(s: &str, u: Units) -> String {
    let old = PLAIN_LEN.with(|c| c.replace(false));
    let r = csv_in_inner(s, u);
    set_plain_lengths(old);
    r
}

fn csv_in_inner(s: &str, u: Units) -> String {
    let mut out = String::with_capacity(s.len() + 64);
    let mut cols: Vec<Option<Kind>> = vec![];
    let mut header = true;
    for line in s.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            out.push_str(&text_in(line, u));
            out.push('\n');
            header = true;
            continue;
        }
        let cells = split_csv(line);
        if header {
            cols.clear();
            let mut h = vec![];
            for c in &cells {
                let k = c.trim();
                if let Some((nk, kd)) = kind_of_si(k).filter(|(nk, _)| nk != k) {
                    cols.push(Some(kd));
                    h.push(nk);
                } else if CSV_LENGTH_COLS.contains(&k) {
                    cols.push(Some(Kind::Length));
                    h.push(format!("{k}_ft"));
                } else {
                    cols.push(None);
                    h.push(c.clone());
                }
            }
            out.push_str(&h.join(","));
            header = false;
        } else {
            let v: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(i, c)| match (cols.get(i).copied().flatten(), c.trim().parse::<f64>()) {
                    (Some(k), Ok(x)) => {
                        let dec = c.trim().split_once('.').map_or(0, |(_, d)| d.len());
                        let d = dec
                            + match k {
                                Kind::Small => 2,
                                Kind::Length | Kind::Area | Kind::Volume => 1,
                                _ => 0,
                            };
                        format!("{:.*}", d, to_us(x, k, u))
                    }
                    _ => text_in(c, u),
                })
                .collect();
            out.push_str(&v.join(","));
        }
        out.push('\n');
    }
    out
}

fn split_csv(line: &str) -> Vec<String> {
    let mut v = vec![];
    let mut cur = String::new();
    let mut q = false;
    for ch in line.chars() {
        match ch {
            '"' => {
                q = !q;
                cur.push(ch)
            }
            ',' if !q => v.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    v.push(cur);
    v
}

/// Machine data files that stay SI in every unit system (raster grids).
pub const KEEP_SI: &[&str] = &["flow-grid.csv"];

/// Convert a text file by extension (csv, md, html, txt, svg); others unchanged.
pub fn file_in(name: &str, bytes: &[u8], u: Units) -> Option<Vec<u8>> {
    if !u.us() {
        return None;
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    let s = std::str::from_utf8(bytes).ok()?;
    if KEEP_SI.contains(&name) {
        return None;
    }
    match ext {
        "csv" => Some(csv_in(s, u).into_bytes()),
        "md" | "html" | "txt" => Some(text_in(s, u).into_bytes()),
        _ => None,
    }
}

/// Metres per display length unit (1 for SI).
pub fn lf() -> f64 {
    let u = cur();
    if u.us() {
        u.ft()
    } else {
        1.0
    }
}
/// Station label interval in metres (100 m SI; 500 ft US).
pub fn sta_step() -> f64 {
    if cur().us() {
        500.0 * cur().ft()
    } else {
        100.0
    }
}
/// Scale label for a computed (non-preferred) ratio, e.g. profile axes.
pub fn scale_free(n: f64) -> String {
    scale(n)
}

fn header_kind(h: &str) -> Option<(Kind, String)> {
    let t = h.trim();
    let names = ["Rim", "Sump invert", "Invert", "EL", "Elevation", "FG", "EG"];
    if names.contains(&t) {
        return Some((Kind::Length, format!("{t} ft")));
    }
    let (head, unit) = if let Some(b) = t.strip_suffix(')') {
        let (a, u) = b.rsplit_once('(')?;
        (a.trim_end().to_string(), u.to_string())
    } else {
        let (a, u) = t.rsplit_once(' ')?;
        (a.to_string(), u.to_string())
    };
    let k = match unit.as_str() {
        "m" => Kind::Length,
        "m3" | "m³" => Kind::Volume,
        "m2" | "m²" => Kind::Area,
        "m3/s" | "m³/s" => Kind::Flow,
        "m/s" => Kind::Velocity,
        "mm" => Kind::Small,
        "km/h" => Kind::Speed,
        "ha" => Kind::Hectare,
        "kPa" => Kind::Kpa,
        "MPa" => Kind::Mpa,
        _ => return None,
    };
    Some((k, format!("{head} {}", k.us_unit())))
}

/// Convert a drawn table (first row = header) to the current units.
pub fn table(rows: &[Vec<String>]) -> Vec<Vec<String>> {
    let u = cur();
    if !u.us() || rows.is_empty() {
        return rows.to_vec();
    }
    let kinds: Vec<Option<(Kind, String)>> = rows[0].iter().map(|h| header_kind(h)).collect();
    rows.iter()
        .enumerate()
        .map(|(ri, r)| {
            r.iter()
                .enumerate()
                .map(|(ci, c)| match (kinds.get(ci).and_then(|k| k.as_ref()), ri) {
                    (Some((_, h)), 0) => h.clone(),
                    (Some((k, _)), _) => match c.trim().parse::<f64>() {
                        Ok(x) => {
                            let dec = c.trim().split_once('.').map_or(0, |(_, d)| d.len());
                            format!("{:.*}", dec + if *k == Kind::Small { 1 } else { 0 }, to_us(x, *k, u))
                        }
                        Err(_) => text_in(c, u),
                    },
                    _ => text_in(c, u),
                })
                .collect()
        })
        .collect()
}

/// Replace `1:N` scale tokens with US engineering scale labels (US only).
pub fn scale_text(s: &str) -> String {
    if !cur().us() {
        return s.to_string();
    }
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '1' && i + 2 < c.len() && c[i + 1] == ':' && c[i + 2].is_ascii_digit() && (i == 0 || !c[i - 1].is_ascii_digit()) {
            let mut j = i + 2;
            while j < c.len() && c[j].is_ascii_digit() {
                j += 1;
            }
            let n: f64 = c[i + 2..j].iter().collect::<String>().parse().unwrap_or(1.0);
            out.push_str(&scale(n));
            i = j;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// SI unit string of a check / value row → kind.
pub fn unit_kind(unit: &str) -> Option<Kind> {
    Some(match unit {
        "m" => Kind::Length,
        "m²" | "m2" => Kind::Area,
        "m³" | "m3" => Kind::Volume,
        "m³/s" | "m3/s" => Kind::Flow,
        "m/s" => Kind::Velocity,
        "m/s²" => Kind::Accel,
        "mm" => Kind::Small,
        "mm/h" => Kind::Rain,
        "km/h" => Kind::Speed,
        "ha" => Kind::Hectare,
        "kPa" => Kind::Kpa,
        "MPa" => Kind::Mpa,
        "kN/m³" => Kind::UnitWeight,
        "kN" => Kind::Kn,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for u in [Units::US, Units { system: System::Us, foot: Foot::Survey }] {
            for k in [
                Kind::Length,
                Kind::Area,
                Kind::Volume,
                Kind::Flow,
                Kind::Small,
                Kind::Speed,
                Kind::Hectare,
                Kind::Kpa,
                Kind::Mpa,
                Kind::UnitWeight,
                Kind::Kn,
                Kind::Rain,
            ] {
                for v in [0.0, 1.0, 123.456, 98765.4321] {
                    assert!((to_si(to_us(v, k, u), k, u) - v).abs() < 1e-9 * v.max(1.0), "{k:?}");
                }
            }
        }
        assert!((to_us(0.3048, Kind::Length, Units::US) - 1.0).abs() < 1e-15);
        assert!((to_us(1.0, Kind::Volume, Units::US) - 1.307_950_6).abs() < 1e-6);
        assert!((to_us(100.0, Kind::Speed, Units::US) - 62.137_119).abs() < 1e-5);
        let sv = Units { system: System::Us, foot: Foot::Survey };
        // 1 survey ft = 1.000002 international ft
        assert!((to_us(1200.0 / 3937.0, Kind::Length, Units::US) - 1.000_002).abs() < 1e-6);
        assert!((to_us(1200.0 / 3937.0, Kind::Length, sv) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn stations() {
        assert_eq!(sta_in(1234.56, Units::SI), "1+234.56");
        assert_eq!(sta_in(1234.56 * 0.3048, Units::US), "12+34.56");
        assert_eq!(sta_in(0.0, Units::US), "0+00.00");
        for s in [0.0, 12.3, 999.99, 1000.0, 3456.78] {
            for u in [Units::SI, Units::US] {
                let back = parse_sta(&sta_in(s, u), u).unwrap();
                assert!((back - s).abs() < 0.01 * u.ft().max(1.0), "{s} {u:?}");
            }
        }
    }

    #[test]
    fn text_and_json() {
        let u = Units::US;
        assert_eq!(text_in("R = 200 m at 0+500.00, V 80 km/h", u), "R = 656.2 ft at 16+40.42, V 49.7 mph");
        assert_eq!(text_in("cut 1000 m³ and 100 mm", u), "cut 1308 CY and 3.94 in");
        assert_eq!(text_in("A1 sheet-01 e=6% 3 min", u), "A1 sheet-01 e=6% 3 min");
        assert_eq!(text_in("0.0600+0.020", u), "0.0600+0.020");
        assert_eq!(text_in("length 100 vs 200 m, lanes [3.5, 3.6] m", u), "length 328.1 vs 656.2 ft, lanes [11.5, 11.8] ft");
        assert_eq!(text_in("imbalance -100 vs -200 m³", u), "imbalance -131 vs -262 CY");
        assert_eq!(text_in("steps 2, 1 and n 3 runs", u), "steps 2, 1 and n 3 runs");
        let a = json!({"radius_ft": 1000.0, "pis": [[0.0, 100.0]], "lanes_m": [3.6], "design_speed_mph": 50.0});
        let s = args_to_si(&a, u);
        assert!((s["radius_m"].as_f64().unwrap() - 304.8).abs() < 1e-9);
        assert!((s["pis"][0][1].as_f64().unwrap() - 30.48).abs() < 1e-9);
        assert_eq!(s["lanes_m"][0].as_f64(), Some(3.6));
        assert!((s["design_speed_kmh"].as_f64().unwrap() - 80.467_2).abs() < 1e-9);
        let r = result_to_us(&json!({"cut_m3": 764.554_857_984, "station": 30.48, "k_mpa_m": 50.0, "msg": "L = 10 m"}), u);
        assert!((r["cut_cy"].as_f64().unwrap() - 1000.0).abs() < 1e-6);
        assert!((r["station"].as_f64().unwrap() - 100.0).abs() < 1e-9);
        assert_eq!(r["k_mpa_m"].as_f64(), Some(50.0));
        assert_eq!(r["msg"], "L = 32.8 ft");
        // US in → SI → US out round trip
        let back = result_to_us(&args_to_si(&json!({"offset_ft": 12.5, "x": 1000.0}), u), u);
        assert!((back["offset_ft"].as_f64().unwrap() - 12.5).abs() < 1e-9);
        assert!((back["x"].as_f64().unwrap() - 1000.0).abs() < 1e-9);
        assert_eq!(boq_in("m³", 764.554_857_984, u).0, "CY");
        assert!((boq_in("m²", 0.836_127_36, u).1 - 1.0).abs() < 1e-9);
        assert!((boq_in("m", 0.3048, u).1 - 1.0).abs() < 1e-12);
        assert!((boq_in("t", 0.907_184_74, u).1 - 1.0).abs() < 1e-12);
    }
}
