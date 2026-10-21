//! Parcels and right-of-way (ROW) acquisition screening for route alternatives.
//!
//! Parcel geometry and values are user data (GeoJSON or ESRI Shapefile, with an optional CSV of
//! values joined by parcel id). Nothing here is a valuation model or an appraisal: land rates,
//! improvement values and the admin / severance factors all come from the user. A parcel
//! without a usable rate makes the ROW cost of every alternative that touches it unknown
//! (reported as `null` with a known-part lower bound), never zero.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const M2_PER_ACRE: f64 = 4_046.856_422_4;
pub const M2_PER_SF: f64 = 0.092_903_04;
pub const M2_PER_HA: f64 = 10_000.0;
/// intersections smaller than this (m²) are treated as touching only (numerical noise)
pub const AREA_TOL_M2: f64 = 0.01;

type Ring = Vec<[f64; 2]>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parcel {
    pub id: String,
    /// outer rings counter-clockwise, holes clockwise (multipolygons give several outer rings)
    pub rings: Vec<Ring>,
    pub area_m2: f64,
    /// total land value of the parcel (user data)
    pub land_value: Option<f64>,
    /// land rate per m² (user data, converted from the given rate unit)
    pub land_rate_m2: Option<f64>,
    /// value of buildings and other improvements (user data); paid on full takes
    pub improvement_value: Option<f64>,
    pub zoning: Option<String>,
    pub no_go: bool,
    pub no_go_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Building {
    pub id: String,
    pub parcel_id: Option<String>,
    pub rings: Vec<Ring>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParcelSet {
    pub parcels: Vec<Parcel>,
    pub buildings: Vec<Building>,
    /// user fallback land rate per m² by zoning class (never defaulted)
    pub fallback_rate_m2: BTreeMap<String, f64>,
    pub currency: String,
    pub source: String,
    pub warnings: Vec<String>,
}

/// Options for ROW take and cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowOpts {
    /// ROW line = daylight (catch) line + this buffer on each side (m)
    #[serde(default)]
    pub buffer_m: f64,
    /// a remnant smaller than this (m²) makes the take a full take (0 = off)
    #[serde(default)]
    pub min_remnant_m2: f64,
    /// a remnant smaller than this fraction of the parcel makes the take a full take (0 = off)
    #[serde(default)]
    pub min_remnant_fraction: f64,
    /// administration / acquisition overhead as a fraction of land + improvements + severance (0 = not included)
    #[serde(default)]
    pub admin_factor: f64,
    /// severance damages on partial takes as a fraction of the land cost of the take (0 = not included)
    #[serde(default)]
    pub severance_factor: f64,
    /// ranking weights: score = w_construction·cost proxy + w_row·ROW cost + relocation_penalty·relocations
    #[serde(default = "one")]
    pub w_construction: f64,
    #[serde(default = "one")]
    pub w_row: f64,
    #[serde(default)]
    pub relocation_penalty: f64,
    /// "infeasible" (default): an alternative crossing a no-go parcel ranks after all others;
    /// "penalty": add no_go_penalty per no-go parcel hit to the score
    #[serde(default = "infeasible")]
    pub no_go: String,
    #[serde(default)]
    pub no_go_penalty: f64,
}
fn one() -> f64 {
    1.0
}
fn infeasible() -> String {
    "infeasible".into()
}
impl Default for RowOpts {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Take {
    pub parcel_id: String,
    pub zoning: Option<String>,
    pub parcel_area_m2: f64,
    pub take_area_m2: f64,
    pub remnant_area_m2: f64,
    pub full_take: bool,
    pub full_take_reason: Option<String>,
    pub buildings_hit: Vec<String>,
    pub rate_source: String,
    pub land_cost: Option<f64>,
    pub improvement_cost: Option<f64>,
    pub severance_cost: Option<f64>,
    pub admin_cost: Option<f64>,
    pub cost: Option<f64>,
    pub no_go: bool,
    pub no_go_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RowResult {
    pub row_area_m2: f64,
    /// part of the ROW polygon covered by no imported parcel (e.g. existing public road or a data gap)
    pub row_area_outside_parcels_m2: f64,
    pub parcels_affected: usize,
    pub full_takes: usize,
    pub partial_takes: usize,
    pub relocations: usize,
    pub no_go_hits: Vec<String>,
    /// None when any affected parcel has no usable rate or improvement value
    pub cost: Option<f64>,
    /// sum of the takes whose cost is known (a lower bound when cost is None)
    pub cost_known_part: f64,
    pub unknown_cost_parcels: Vec<String>,
    pub takes: Vec<Take>,
    /// ROW polygon used (counter-clockwise ring, world x/y)
    pub polygon: Vec<[f64; 2]>,
}

// ---------------------------------------------------------------- geometry

pub fn signed_area(r: &[[f64; 2]]) -> f64 {
    let mut a = 0.0;
    for i in 0..r.len() {
        let (u, v) = (r[i], r[(i + 1) % r.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a / 2.0
}

fn close_open(mut r: Ring) -> Ring {
    if r.len() > 1 && r.first() == r.last() {
        r.pop();
    }
    r
}

fn orient(mut r: Ring, ccw: bool) -> Ring {
    if (signed_area(&r) > 0.0) != ccw {
        r.reverse();
    }
    r
}

/// Area of a ring set (outer ccw, holes cw).
pub fn rings_area(rs: &[Ring]) -> f64 {
    rs.iter().map(|r| signed_area(r)).sum()
}

/// Even-odd point in ring set.
pub fn inside(p: [f64; 2], rs: &[Ring]) -> bool {
    let mut c = false;
    for r in rs {
        let n = r.len();
        let mut j = n - 1;
        for i in 0..n {
            let (a, b) = (r[i], r[j]);
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                c = !c;
            }
            j = i;
        }
    }
    c
}

const EPS: f64 = 1e-9;

/// Parameters along p→q where it meets a→b (crossings, and the ends of a collinear overlap).
fn seg_ts(p: [f64; 2], q: [f64; 2], a: [f64; 2], b: [f64; 2], out: &mut Vec<f64>) {
    let r = [q[0] - p[0], q[1] - p[1]];
    let s = [b[0] - a[0], b[1] - a[1]];
    let d = r[0] * s[1] - r[1] * s[0];
    let w = [a[0] - p[0], a[1] - p[1]];
    let rr = r[0] * r[0] + r[1] * r[1];
    let scale = rr.sqrt() * (s[0] * s[0] + s[1] * s[1]).sqrt();
    if d.abs() <= 1e-12 * scale.max(1e-300) {
        // parallel: collinear when a is on the line of p→q
        if (w[0] * r[1] - w[1] * r[0]).abs() <= EPS * rr.sqrt().max(1.0) {
            for e in [a, b] {
                let t = ((e[0] - p[0]) * r[0] + (e[1] - p[1]) * r[1]) / rr;
                if (0.0..=1.0).contains(&t) {
                    out.push(t);
                }
            }
        }
        return;
    }
    let t = (w[0] * s[1] - w[1] * s[0]) / d;
    let u = (w[0] * r[1] - w[1] * r[0]) / d;
    if (0.0..=1.0).contains(&t) && (-1e-12..=1.0 + 1e-12).contains(&u) {
        out.push(t);
    }
}

/// Direction of the edge of `rs` that the point lies on: Some(+1) same as `dir`, Some(-1) opposite.
fn on_edge(m: [f64; 2], dir: [f64; 2], rs: &[Ring]) -> Option<i8> {
    for r in rs {
        for k in 0..r.len() {
            let (a, b) = (r[k], r[(k + 1) % r.len()]);
            let s = [b[0] - a[0], b[1] - a[1]];
            let l = (s[0] * s[0] + s[1] * s[1]).sqrt();
            if l == 0.0 {
                continue;
            }
            let w = [m[0] - a[0], m[1] - a[1]];
            let t = (w[0] * s[0] + w[1] * s[1]) / (l * l);
            if (0.0..=1.0).contains(&t) && (w[0] * s[1] - w[1] * s[0]).abs() / l < 1e-7 {
                return Some(if dir[0] * s[0] + dir[1] * s[1] > 0.0 { 1 } else { -1 });
            }
        }
    }
    None
}

/// ∮ of the boundary of `a` that lies inside `b` (x dy − y dx terms). Boundary shared with `b`
/// in the same direction counts once (when `first`); shared in opposite directions not at all.
fn half(a: &[Ring], b: &[Ring], first: bool) -> f64 {
    let mut s = 0.0;
    for r in a {
        for i in 0..r.len() {
            let (p, q) = (r[i], r[(i + 1) % r.len()]);
            let mut ts = vec![0.0, 1.0];
            for rb in b {
                for k in 0..rb.len() {
                    seg_ts(p, q, rb[k], rb[(k + 1) % rb.len()], &mut ts);
                }
            }
            ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
            let dir = [q[0] - p[0], q[1] - p[1]];
            for w in ts.windows(2) {
                if w[1] - w[0] < 1e-12 {
                    continue;
                }
                let at = |t: f64| [p[0] + t * dir[0], p[1] + t * dir[1]];
                let (u, v, m) = (at(w[0]), at(w[1]), at(0.5 * (w[0] + w[1])));
                let take = match on_edge(m, dir, b) {
                    Some(1) => first,
                    Some(_) => false,
                    None => inside(m, b),
                };
                if take {
                    s += u[0] * v[1] - v[0] * u[1];
                }
            }
        }
    }
    s
}

/// Area of the intersection of two ring sets (outer rings ccw, holes cw, no self-intersections),
/// by the boundary integral of the intersection region (Green's theorem).
pub fn intersection_area(a: &[Ring], b: &[Ring]) -> f64 {
    ((half(a, b, true) + half(b, a, false)) / 2.0).max(0.0)
}

/// ROW polygon of a corridor: daylight (catch) point of every section offset outward by
/// `buffer_m`, left side forward and right side back. Counter-clockwise.
pub fn row_polygon(c: &crate::corridor::Corridor, buffer_m: f64) -> Ring {
    let pt = |s: &crate::corridor::Section, left: bool| {
        let sd = if left { &s.left } else { &s.right };
        let o = sd.fg.last().map_or(0.0, |q| q[0]) + buffer_m;
        crate::cad3d::sec_xy(s.x, s.y, s.heading, o, left)
    };
    let mut r: Ring = c.sections.iter().map(|s| pt(s, true)).collect();
    r.extend(c.sections.iter().rev().map(|s| pt(s, false)));
    orient(r, true)
}

/// 45° hatch segments (spacing `sp`) over the intersection of two ring sets, for drawing take areas.
pub fn hatch(a: &[Ring], b: &[Ring], sp: f64) -> Vec<[[f64; 2]; 2]> {
    let bb = |rs: &[Ring]| {
        let mut m = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in rs.iter().flatten() {
            m = [m[0].min(p[0]), m[1].min(p[1]), m[2].max(p[0]), m[3].max(p[1])];
        }
        m
    };
    let (ba, bb2) = (bb(a), bb(b));
    let m = [ba[0].max(bb2[0]), ba[1].max(bb2[1]), ba[2].min(bb2[2]), ba[3].min(bb2[3])];
    let mut out = vec![];
    if m[0] >= m[2] || m[1] >= m[3] || sp <= 0.0 {
        return out;
    }
    // lines x - y = c
    let (c0, c1) = (m[0] - m[3], m[2] - m[1]);
    let mut c = (c0 / sp).ceil() * sp;
    while c <= c1 {
        let p = [m[1] + c, m[1]];
        let q = [m[3] + c, m[3]];
        let mut ts = vec![];
        for rs in [a, b] {
            for r in rs {
                for k in 0..r.len() {
                    seg_ts(p, q, r[k], r[(k + 1) % r.len()], &mut ts);
                }
            }
        }
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let at = |t: f64| [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])];
        for w in ts.windows(2) {
            let mid = at(0.5 * (w[0] + w[1]));
            if w[1] - w[0] > 1e-9 && inside(mid, a) && inside(mid, b) {
                out.push([at(w[0]), at(w[1])]);
            }
        }
        c += sp;
    }
    out
}

// ---------------------------------------------------------------- import

/// Property names read from the input (each can be renamed with `field_map`).
pub const FIELDS: &[&str] = &["id", "land_value", "land_rate", "improvement_value", "zoning", "no_go", "no_go_reason", "feature_type", "parcel_id"];

pub struct ImportOpts {
    pub field_map: BTreeMap<String, String>,
    /// unit of `land_rate` and of the fallback rates: m2, ha, acre, sf
    pub rate_unit: String,
    /// coordinate unit of the geometry: m, ft (international), usft (US survey)
    pub coord_unit: String,
    pub fallback: BTreeMap<String, f64>,
    pub currency: String,
}

pub fn rate_to_m2(r: f64, unit: &str) -> crate::Result<f64> {
    Ok(match unit {
        "m2" | "sqm" => r,
        "ha" => r / M2_PER_HA,
        "acre" | "ac" => r / M2_PER_ACRE,
        "sf" | "sqft" | "ft2" => r / M2_PER_SF,
        _ => return Err(format!("rate_unit '{unit}': use m2, ha, acre or sf")),
    })
}

fn coord_factor(u: &str) -> crate::Result<f64> {
    Ok(match u {
        "m" => 1.0,
        "ft" => 0.3048,
        "usft" | "us_survey_ft" => 1200.0 / 3937.0,
        _ => return Err(format!("coord_unit '{u}': use m, ft or usft")),
    })
}

/// One feature with property values as strings.
struct Feat {
    props: BTreeMap<String, String>,
    polys: Vec<Vec<Ring>>,
}

fn num(s: Option<&String>) -> Option<f64> {
    s.and_then(|v| v.trim().replace(',', "").parse::<f64>().ok()).filter(|x| x.is_finite())
}
fn truthy(s: Option<&String>) -> bool {
    s.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "y" | "t"))
}

fn val_str(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        x => Some(x.to_string()),
    }
}

fn geojson_feats(s: &str) -> crate::Result<Vec<Feat>> {
    let v: Value = serde_json::from_str(s).map_err(|e| format!("GeoJSON: {e}"))?;
    let feats = match v["type"].as_str() {
        Some("FeatureCollection") => v["features"].as_array().cloned().unwrap_or_default(),
        Some("Feature") => vec![v.clone()],
        _ => return Err("GeoJSON: expected a FeatureCollection or Feature".into()),
    };
    let ring = |r: &Value| -> Ring {
        r.as_array().map(|a| a.iter().map(|p| [p[0].as_f64().unwrap_or(f64::NAN), p[1].as_f64().unwrap_or(f64::NAN)]).collect()).unwrap_or_default()
    };
    let poly = |p: &Value| -> Vec<Ring> { p.as_array().map(|a| a.iter().map(ring).collect()).unwrap_or_default() };
    let mut out = vec![];
    for f in feats {
        let g = &f["geometry"];
        let polys = match g["type"].as_str() {
            Some("Polygon") => vec![poly(&g["coordinates"])],
            Some("MultiPolygon") => g["coordinates"].as_array().map(|a| a.iter().map(poly).collect()).unwrap_or_default(),
            _ => continue,
        };
        let mut props = BTreeMap::new();
        if let Some(m) = f["properties"].as_object() {
            for (k, x) in m {
                if let Some(s) = val_str(x) {
                    props.insert(k.clone(), s);
                }
            }
        }
        if let Some(id) = f.get("id").and_then(val_str) {
            props.entry("id".into()).or_insert(id);
        }
        out.push(Feat { props, polys });
    }
    Ok(out)
}

fn le_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn le_f64(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// Polygon shapes (types 5, 15, 25; x/y only) of an ESRI .shp file. Rings are grouped into
/// polygons by orientation (clockwise = outer ring in the shapefile convention).
pub fn read_shp(b: &[u8]) -> crate::Result<Vec<Vec<Vec<Ring>>>> {
    if b.len() < 100 || i32::from_be_bytes(b[0..4].try_into().unwrap()) != 9994 {
        return Err("not an ESRI shapefile (.shp)".into());
    }
    let mut o = 100;
    let mut out = vec![];
    while o + 8 <= b.len() {
        let len = i32::from_be_bytes(b[o + 4..o + 8].try_into().unwrap()) as usize * 2;
        let r = o + 8;
        if r + len > b.len() || len < 4 {
            return Err("shapefile record runs past the end of the file".into());
        }
        let st = le_i32(b, r);
        let mut polys: Vec<Vec<Ring>> = vec![];
        if matches!(st, 5 | 15 | 25) {
            let (np, npt) = (le_i32(b, r + 36) as usize, le_i32(b, r + 40) as usize);
            let parts: Vec<usize> = (0..np).map(|i| le_i32(b, r + 44 + 4 * i) as usize).collect();
            let p0 = r + 44 + 4 * np;
            if p0 + 16 * npt > r + len {
                return Err("shapefile polygon record too short".into());
            }
            for (k, &s) in parts.iter().enumerate() {
                let e = parts.get(k + 1).copied().unwrap_or(npt);
                let ring: Ring = (s..e).map(|i| [le_f64(b, p0 + 16 * i), le_f64(b, p0 + 16 * i + 8)]).collect();
                if signed_area(&ring) <= 0.0 || polys.is_empty() {
                    polys.push(vec![ring]);
                } else {
                    polys.last_mut().unwrap().push(ring);
                }
            }
        } else if st != 0 {
            return Err(format!("shapefile shape type {st}: only polygons (5, 15, 25) are supported"));
        }
        out.push(polys);
        o = r + len;
    }
    Ok(out)
}

/// dBASE III table (.dbf) as rows of field name → trimmed string.
pub fn read_dbf(b: &[u8]) -> crate::Result<Vec<BTreeMap<String, String>>> {
    if b.len() < 32 {
        return Err("dbf too short".into());
    }
    let n = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    let hl = u16::from_le_bytes(b[8..10].try_into().unwrap()) as usize;
    let rl = u16::from_le_bytes(b[10..12].try_into().unwrap()) as usize;
    let mut fields = vec![];
    let mut o = 32;
    while o + 32 <= hl && b[o] != 0x0d {
        let name: String = b[o..o + 11].iter().take_while(|c| **c != 0).map(|c| *c as char).collect();
        fields.push((name, b[o + 16] as usize));
        o += 32;
    }
    let mut rows = vec![];
    for i in 0..n {
        let s = hl + i * rl;
        if s + rl > b.len() {
            return Err("dbf record runs past the end of the file".into());
        }
        let mut row = BTreeMap::new();
        let mut p = s + 1;
        for (name, w) in &fields {
            let v = String::from_utf8_lossy(&b[p..p + w]).trim().to_string();
            if !v.is_empty() && v != "?" {
                row.insert(name.clone(), v);
            }
            p += w;
        }
        rows.push(row);
    }
    Ok(rows)
}

fn shp_feats(path: &std::path::Path) -> crate::Result<Vec<Feat>> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let shapes = read_shp(&b)?;
    let dbf = path.with_extension("dbf");
    let rows = if dbf.exists() { read_dbf(&std::fs::read(&dbf).map_err(|e| e.to_string())?)? } else { vec![] };
    Ok(shapes.into_iter().enumerate().map(|(i, polys)| Feat { props: rows.get(i).cloned().unwrap_or_default(), polys }).collect())
}

fn csv_rows(s: &str) -> Vec<BTreeMap<String, String>> {
    let mut lines = s.lines().filter(|l| !l.trim().is_empty());
    let Some(h) = lines.next() else { return vec![] };
    let split = |l: &str| l.split(',').map(|x| x.trim().trim_matches('"').to_string()).collect::<Vec<_>>();
    let hs = split(h.trim_start_matches('\u{feff}'));
    lines.map(|l| hs.iter().cloned().zip(split(l)).filter(|(_, v)| !v.is_empty()).collect()).collect()
}

/// Read parcels (and buildings) from a GeoJSON (.geojson/.json) or Shapefile (.shp + .dbf),
/// with an optional values CSV joined by parcel id (CSV values override file attributes).
pub fn import(path: &std::path::Path, values_csv: Option<&std::path::Path>, o: &ImportOpts) -> crate::Result<ParcelSet> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let feats = match ext.as_str() {
        "shp" => shp_feats(path)?,
        "geojson" | "json" => geojson_feats(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)?,
        _ => return Err("parcels: use a .geojson/.json or .shp file".into()),
    };
    let key = |k: &str| o.field_map.get(k).cloned().unwrap_or_else(|| k.to_string());
    let cf = coord_factor(&o.coord_unit)?;
    let mut join: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    if let Some(c) = values_csv {
        for r in csv_rows(&std::fs::read_to_string(c).map_err(|e| format!("{}: {e}", c.display()))?) {
            if let Some(id) = r.get(&key("id")).or_else(|| r.get("parcel_id")).cloned() {
                join.insert(id, r);
            }
        }
    }
    let mut set = ParcelSet { currency: o.currency.clone(), source: path.display().to_string(), ..Default::default() };
    for (z, r) in &o.fallback {
        set.fallback_rate_m2.insert(z.clone(), rate_to_m2(*r, &o.rate_unit)?);
    }
    let mut joined = 0;
    for (i, f) in feats.into_iter().enumerate() {
        let mut rings: Vec<Ring> = vec![];
        for poly in &f.polys {
            for (k, r) in poly.iter().enumerate() {
                let r: Ring = close_open(r.iter().map(|p| [p[0] * cf, p[1] * cf]).collect());
                if r.len() >= 3 && r.iter().all(|p| p[0].is_finite() && p[1].is_finite()) {
                    rings.push(orient(r, k == 0));
                }
            }
        }
        if rings.is_empty() {
            set.warnings.push(format!("feature {i}: no polygon geometry, skipped"));
            continue;
        }
        let mut p = f.props.clone();
        let id = p.get(&key("id")).cloned().unwrap_or_else(|| format!("F{}", i + 1));
        if let Some(j) = join.get(&id) {
            joined += 1;
            for (k, v) in j {
                p.insert(k.clone(), v.clone());
            }
        }
        let g = |k: &str| p.get(&key(k));
        if g("feature_type").is_some_and(|t| t.eq_ignore_ascii_case("building")) {
            set.buildings.push(Building { id, parcel_id: g("parcel_id").cloned(), rings });
            continue;
        }
        let area = rings_area(&rings);
        let land_rate_m2 = match num(g("land_rate")) {
            Some(r) => Some(rate_to_m2(r, &o.rate_unit)?),
            None => None,
        };
        set.parcels.push(Parcel {
            id,
            area_m2: area,
            land_value: num(g("land_value")),
            land_rate_m2,
            improvement_value: num(g("improvement_value")),
            zoning: g("zoning").cloned(),
            no_go: truthy(g("no_go")),
            no_go_reason: g("no_go_reason").cloned(),
            rings,
        });
    }
    if values_csv.is_some() && joined < join.len() {
        set.warnings.push(format!("{} CSV rows matched no parcel id", join.len() - joined));
    }
    // buildings without a parcel id belong to the parcel containing their first vertex centroid
    for bd in &mut set.buildings {
        if bd.parcel_id.is_none() {
            let r = &bd.rings[0];
            let c = [r.iter().map(|q| q[0]).sum::<f64>() / r.len() as f64, r.iter().map(|q| q[1]).sum::<f64>() / r.len() as f64];
            bd.parcel_id = set.parcels.iter().find(|p| inside(c, &p.rings)).map(|p| p.id.clone());
            if bd.parcel_id.is_none() {
                set.warnings.push(format!("building {} lies in no imported parcel; it counts as a relocation but has no parcel cost", bd.id));
            }
        }
    }
    let ids: std::collections::BTreeSet<&str> = set.parcels.iter().map(|p| p.id.as_str()).collect();
    if ids.len() < set.parcels.len() {
        set.warnings.push("duplicate parcel ids".into());
    }
    if set.parcels.is_empty() {
        return Err("no parcel polygons found".into());
    }
    Ok(set)
}

impl ParcelSet {
    /// Land rate per m² and where it came from; None when no value, rate or fallback exists.
    pub fn rate(&self, p: &Parcel) -> (Option<f64>, String) {
        if let Some(r) = p.land_rate_m2 {
            return (Some(r), "parcel land rate".into());
        }
        if let Some(v) = p.land_value {
            if p.area_m2 > 0.0 {
                return (Some(v / p.area_m2), "parcel land value / parcel area".into());
            }
        }
        if let Some(z) = &p.zoning {
            if let Some(r) = self.fallback_rate_m2.get(z) {
                return (Some(*r), format!("user fallback rate for zoning '{z}'"));
            }
        }
        (None, "no land value, rate or fallback rate: unknown".into())
    }

    /// Normalised ROW cost per cell for the route search: rate / median rate of rated parcels;
    /// NaN marks no-go parcels; 0 outside parcels or for parcels without a rate.
    pub fn cost_surface(&self, g: &crate::route::Grid) -> Vec<f64> {
        let mut rates: Vec<f64> = self.parcels.iter().filter_map(|p| self.rate(p).0).filter(|r| *r > 0.0).collect();
        rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = rates.get(rates.len() / 2).copied().unwrap_or(1.0);
        let mut out = vec![0.0; g.nx * g.ny];
        for j in 0..g.ny {
            for i in 0..g.nx {
                let p = [g.x0 + i as f64 * g.cell, g.y0 + j as f64 * g.cell];
                if let Some(pc) = self.parcels.iter().find(|q| inside(p, &q.rings)) {
                    out[j * g.nx + i] = if pc.no_go { f64::NAN } else { self.rate(pc).0.map_or(0.0, |r| r / med) };
                }
            }
        }
        out
    }

    /// ROW takes for one ROW polygon.
    pub fn assess(&self, poly: &Ring, o: &RowOpts) -> RowResult {
        let fp = vec![poly.clone()];
        let row_area = signed_area(poly).abs();
        let hit_b: Vec<&Building> = self.buildings.iter().filter(|b| intersection_area(&b.rings, &fp) > AREA_TOL_M2).collect();
        let mut takes = vec![];
        let mut covered = 0.0;
        for p in &self.parcels {
            let ta = intersection_area(&p.rings, &fp);
            covered += ta;
            if ta <= AREA_TOL_M2 {
                continue;
            }
            let rem = (p.area_m2 - ta).max(0.0);
            let bh: Vec<String> = hit_b.iter().filter(|b| b.parcel_id.as_deref() == Some(p.id.as_str())).map(|b| b.id.clone()).collect();
            let reason = if !bh.is_empty() {
                Some(format!("building inside the ROW ({})", bh.join(", ")))
            } else if o.min_remnant_m2 > 0.0 && rem < o.min_remnant_m2 {
                Some(format!("remnant {rem:.1} m² below the {:.1} m² threshold", o.min_remnant_m2))
            } else if o.min_remnant_fraction > 0.0 && rem < o.min_remnant_fraction * p.area_m2 {
                Some(format!("remnant {:.1} % of the parcel, below the {:.1} % threshold", 100.0 * rem / p.area_m2, 100.0 * o.min_remnant_fraction))
            } else {
                None
            };
            let full = reason.is_some();
            let (rate, rate_source) = self.rate(p);
            let land = rate.map(|r| r * if full { p.area_m2 } else { ta });
            let impr = if full { p.improvement_value } else { Some(0.0) };
            let sev = land.map(|l| if full { 0.0 } else { o.severance_factor * l });
            let (cost, admin) = match (land, impr, sev) {
                (Some(l), Some(i), Some(s)) => {
                    let ad = o.admin_factor * (l + i + s);
                    (Some(l + i + s + ad), Some(ad))
                }
                _ => (None, None),
            };
            takes.push(Take {
                parcel_id: p.id.clone(),
                zoning: p.zoning.clone(),
                parcel_area_m2: p.area_m2,
                take_area_m2: ta,
                remnant_area_m2: rem,
                full_take: full,
                full_take_reason: reason,
                buildings_hit: bh,
                rate_source: if full && p.improvement_value.is_none() {
                    format!("{rate_source}; improvement value missing on a full take: unknown")
                } else {
                    rate_source
                },
                land_cost: land,
                improvement_cost: impr,
                severance_cost: sev,
                admin_cost: admin,
                cost,
                no_go: p.no_go,
                no_go_reason: p.no_go_reason.clone(),
            });
        }
        let unknown: Vec<String> = takes.iter().filter(|t| t.cost.is_none()).map(|t| t.parcel_id.clone()).collect();
        let known: f64 = takes.iter().filter_map(|t| t.cost).sum();
        RowResult {
            row_area_m2: row_area,
            row_area_outside_parcels_m2: (row_area - covered).max(0.0),
            parcels_affected: takes.len(),
            full_takes: takes.iter().filter(|t| t.full_take).count(),
            partial_takes: takes.iter().filter(|t| !t.full_take).count(),
            relocations: hit_b.len(),
            no_go_hits: takes.iter().filter(|t| t.no_go).map(|t| t.parcel_id.clone()).collect(),
            cost: unknown.is_empty().then_some(known),
            cost_known_part: known,
            unknown_cost_parcels: unknown,
            takes,
            polygon: poly.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x0: f64, y0: f64, x1: f64, y1: f64) -> Ring {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    #[test]
    fn intersection_of_squares_and_holes() {
        let a = vec![sq(0.0, 0.0, 10.0, 10.0)];
        let b = vec![sq(5.0, 5.0, 15.0, 15.0)];
        assert!((intersection_area(&a, &b) - 25.0).abs() < 1e-9);
        assert!((intersection_area(&a, &a) - 100.0).abs() < 1e-6);
        // a hole (clockwise) removes its overlap
        let h = vec![sq(0.0, 0.0, 10.0, 10.0), orient(sq(6.0, 6.0, 8.0, 8.0), false)];
        assert!((rings_area(&h) - 96.0).abs() < 1e-9);
        assert!((intersection_area(&h, &b) - 21.0).abs() < 1e-9);
        // a concave L shape against a strip
        let l = vec![orient(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 2.0], [2.0, 2.0], [2.0, 10.0], [0.0, 10.0]], true)];
        let s = vec![sq(-1.0, 1.0, 11.0, 3.0)];
        assert!((intersection_area(&l, &s) - (10.0 * 1.0 + 2.0 * 1.0)).abs() < 1e-9);
        assert_eq!(intersection_area(&a, &[sq(20.0, 20.0, 30.0, 30.0)]), 0.0);
    }

    fn set() -> ParcelSet {
        let p = |id: &str, r: Ring, rate: Option<f64>, impr: Option<f64>, z: &str| Parcel {
            id: id.into(),
            area_m2: signed_area(&r),
            rings: vec![r],
            land_value: None,
            land_rate_m2: rate,
            improvement_value: impr,
            zoning: Some(z.into()),
            no_go: false,
            no_go_reason: None,
        };
        ParcelSet {
            parcels: vec![
                p("P1", sq(0.0, -50.0, 100.0, 50.0), Some(10.0), Some(0.0), "AG"),
                p("P2", sq(100.0, -50.0, 200.0, 50.0), Some(20.0), Some(5000.0), "RES"),
                p("P3", sq(200.0, -50.0, 300.0, 50.0), None, Some(0.0), "COM"),
                p("P4", sq(300.0, -15.0, 400.0, 15.0), Some(5.0), Some(0.0), "AG"),
            ],
            buildings: vec![Building { id: "B1".into(), parcel_id: Some("P2".into()), rings: vec![sq(140.0, 5.0, 150.0, 15.0)] }],
            currency: "TEST".into(),
            ..Default::default()
        }
    }

    #[test]
    fn takes_costs_and_unknowns() {
        let s = set();
        let strip = sq(0.0, -10.0, 400.0, 10.0);
        let o = RowOpts { min_remnant_fraction: 0.25, admin_factor: 0.1, severance_factor: 0.2, ..Default::default() };
        let r = s.assess(&strip, &o);
        assert_eq!(r.parcels_affected, 4);
        let t = |id: &str| r.takes.iter().find(|t| t.parcel_id == id).unwrap();
        // P1 partial: 2000 m² × 10, severance 20 %, admin 10 %
        assert!((t("P1").take_area_m2 - 2000.0).abs() < 1e-6 && !t("P1").full_take);
        assert!((t("P1").cost.unwrap() - 2000.0 * 10.0 * 1.2 * 1.1).abs() < 1e-6);
        // P2 full: building hit; whole parcel 10000 m² × 20 + 5000 improvements, +10 %
        assert!(t("P2").full_take && t("P2").buildings_hit == vec!["B1".to_string()]);
        assert!((t("P2").cost.unwrap() - (10000.0 * 20.0 + 5000.0) * 1.1).abs() < 1e-6);
        // P3 has no rate: unknown, never zero
        assert!(t("P3").cost.is_none());
        // P4 remnant 1000 of 3000 m² (33 %) is above 25 %: partial
        assert!(!t("P4").full_take);
        assert_eq!(r.relocations, 1);
        assert!(r.cost.is_none() && r.unknown_cost_parcels == vec!["P3".to_string()]);
        assert!(r.cost_known_part > 0.0);
        // fallback by zoning makes it known
        let mut s2 = s.clone();
        s2.fallback_rate_m2.insert("COM".into(), 30.0);
        assert!(s2.assess(&strip, &o).cost.is_some());
        // a remnant threshold turns P4 into a full take
        let o2 = RowOpts { min_remnant_fraction: 0.5, ..Default::default() };
        assert!(s.assess(&strip, &o2).takes.iter().find(|t| t.parcel_id == "P4").unwrap().full_take);
    }

    #[test]
    fn hatch_stays_inside_the_intersection() {
        let a = vec![sq(0.0, 0.0, 10.0, 10.0)];
        let b = vec![sq(5.0, -5.0, 20.0, 5.0)];
        let h = hatch(&a, &b, 1.0);
        assert!(!h.is_empty());
        for s in &h {
            for p in s {
                assert!(p[0] >= 5.0 - 1e-9 && p[0] <= 10.0 + 1e-9 && p[1] >= -1e-9 && p[1] <= 5.0 + 1e-9, "{p:?}");
            }
        }
    }
    #[test]
    fn rate_units() {
        assert!((rate_to_m2(4046.8564224, "acre").unwrap() - 1.0).abs() < 1e-12);
        assert!((rate_to_m2(1.0, "sf").unwrap() - 10.763910416709722).abs() < 1e-9);
        assert!((rate_to_m2(10000.0, "ha").unwrap() - 1.0).abs() < 1e-12);
        assert!(rate_to_m2(1.0, "furlong").is_err());
    }

    fn shp_bytes(rings: &[Ring]) -> Vec<u8> {
        let npt: usize = rings.iter().map(|r| r.len()).sum();
        let mut rec = vec![];
        rec.extend(5i32.to_le_bytes());
        rec.extend([0u8; 32]);
        rec.extend((rings.len() as i32).to_le_bytes());
        rec.extend((npt as i32).to_le_bytes());
        let mut s = 0;
        for r in rings {
            rec.extend((s as i32).to_le_bytes());
            s += r.len();
        }
        for r in rings {
            for p in r {
                rec.extend(p[0].to_le_bytes());
                rec.extend(p[1].to_le_bytes());
            }
        }
        let mut b = vec![0u8; 100];
        b[0..4].copy_from_slice(&9994i32.to_be_bytes());
        b.extend(1i32.to_be_bytes());
        b.extend(((rec.len() / 2) as i32).to_be_bytes());
        b.extend(rec);
        b
    }

    fn dbf_bytes(fields: &[(&str, usize)], rows: &[Vec<&str>]) -> Vec<u8> {
        let hl = 32 + 32 * fields.len() + 1;
        let rl = 1 + fields.iter().map(|f| f.1).sum::<usize>();
        let mut b = vec![3u8, 126, 1, 1];
        b.extend((rows.len() as u32).to_le_bytes());
        b.extend((hl as u16).to_le_bytes());
        b.extend((rl as u16).to_le_bytes());
        b.extend([0u8; 20]);
        for (n, w) in fields {
            let mut f = [0u8; 32];
            f[..n.len()].copy_from_slice(n.as_bytes());
            f[11] = b'C';
            f[16] = *w as u8;
            b.extend(f);
        }
        b.push(0x0d);
        for r in rows {
            b.push(b' ');
            for ((_, w), v) in fields.iter().zip(r) {
                b.extend(format!("{v:<w$}").as_bytes());
            }
        }
        b
    }

    #[test]
    fn shapefile_and_csv_join() {
        let d = std::env::temp_dir().join(format!("parcels-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        // shapefile outer rings are clockwise; a counter-clockwise hole follows
        let outer = orient(sq(0.0, 0.0, 100.0, 100.0), false);
        let hole = orient(sq(10.0, 10.0, 20.0, 20.0), true);
        std::fs::write(d.join("p.shp"), shp_bytes(&[outer, hole])).unwrap();
        std::fs::write(d.join("p.dbf"), dbf_bytes(&[("APN", 8), ("ZONE", 4)], &[vec!["001-A", "AG"]])).unwrap();
        std::fs::write(d.join("v.csv"), "APN,land_rate,improvement_value\n001-A,2.5,0\n999,1,1\n").unwrap();
        let o = ImportOpts {
            field_map: [("id".to_string(), "APN".to_string()), ("zoning".to_string(), "ZONE".to_string())].into_iter().collect(),
            rate_unit: "acre".into(),
            coord_unit: "m".into(),
            fallback: BTreeMap::new(),
            currency: "USD".into(),
        };
        let s = import(&d.join("p.shp"), Some(&d.join("v.csv")), &o).unwrap();
        assert_eq!(s.parcels.len(), 1);
        let p = &s.parcels[0];
        assert_eq!(p.id, "001-A");
        assert_eq!(p.zoning.as_deref(), Some("AG"));
        assert!((p.area_m2 - 9900.0).abs() < 1e-9);
        assert!((p.land_rate_m2.unwrap() - 2.5 / M2_PER_ACRE).abs() < 1e-15);
        assert!(s.warnings.iter().any(|w| w.contains("1 CSV rows matched no parcel")));
        // GeoJSON with a building and US survey feet coordinates
        let gj = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"id":"X","land_value":1000,"no_go":"yes","no_go_reason":"cemetery"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[0,100],[100,100],[100,0],[0,0]]]}},
          {"type":"Feature","properties":{"id":"BX","feature_type":"building"},"geometry":{"type":"Polygon","coordinates":[[[10,10],[20,10],[20,20],[10,20],[10,10]]]}}]}"#;
        std::fs::write(d.join("g.geojson"), gj).unwrap();
        let o2 = ImportOpts { field_map: BTreeMap::new(), rate_unit: "m2".into(), coord_unit: "usft".into(), fallback: BTreeMap::new(), currency: "".into() };
        let s = import(&d.join("g.geojson"), None, &o2).unwrap();
        let f = 1200.0 / 3937.0;
        assert!((s.parcels[0].area_m2 - 10000.0 * f * f).abs() < 1e-6 && s.parcels[0].no_go);
        assert_eq!(s.buildings[0].parcel_id.as_deref(), Some("X"));
        std::fs::remove_dir_all(&d).ok();
    }
}
