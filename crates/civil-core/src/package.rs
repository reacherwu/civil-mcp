//! Design package v1: a folder of JSON files (canonical SI) plus a LandXML 1.2 file,
//! described by `manifest.json`. JSON Schemas live in `schema/design-package/v1/` and are
//! embedded here for `validate_design_package`. Format and intended Civil 3D API mapping:
//! docs/design-package.md.

use crate::align::Kind;
use crate::audit::sha256_hex;
use crate::tools::Session;
use serde_json::{json, Value};
use std::path::Path;

pub const VERSION: &str = "1.0.0";
pub const DIR: &str = "design-package";

/// (role, file name, embedded schema text)
pub const PARTS: &[(&str, &str, &str)] = &[
    ("alignments", "alignments.json", include_str!("../../../schema/design-package/v1/alignments.schema.json")),
    ("profiles", "profiles.json", include_str!("../../../schema/design-package/v1/profiles.schema.json")),
    ("superelevation", "superelevation.json", include_str!("../../../schema/design-package/v1/superelevation.schema.json")),
    ("assemblies", "assemblies.json", include_str!("../../../schema/design-package/v1/assemblies.schema.json")),
    ("corridors", "corridors.json", include_str!("../../../schema/design-package/v1/corridors.schema.json")),
    ("surfaces", "surfaces.json", include_str!("../../../schema/design-package/v1/surfaces.schema.json")),
    ("pipes", "pipes.json", include_str!("../../../schema/design-package/v1/pipes.schema.json")),
    ("parcels", "parcels.json", include_str!("../../../schema/design-package/v1/parcels.schema.json")),
    ("quantities", "quantities.json", include_str!("../../../schema/design-package/v1/quantities.schema.json")),
    ("sheets", "sheets.json", include_str!("../../../schema/design-package/v1/sheets.schema.json")),
];
pub const MANIFEST_SCHEMA: &str = include_str!("../../../schema/design-package/v1/manifest.schema.json");
pub const LANDXML: &str = "design.xml";

fn head(kind: &str) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("schema".into(), json!(format!("civil-mcp/design-package/{kind}")));
    m.insert("schema_version".into(), json!(VERSION));
    m.insert("units".into(), json!("SI"));
    m
}
fn doc(kind: &str, body: Value) -> Value {
    let mut m = head(kind);
    if let Value::Object(b) = body {
        m.extend(b);
    }
    Value::Object(m)
}
/// grid azimuth in degrees clockwise from north, from radians counter-clockwise from east
fn azd(a: f64) -> f64 {
    (90.0 - a.to_degrees()).rem_euclid(360.0)
}
fn rad(k: f64) -> Value {
    if k.abs() < 1e-12 {
        Value::Null
    } else {
        json!(1.0 / k.abs())
    }
}

/// Builds every JSON part of the package from the session. Missing design objects give empty arrays.
pub fn build(s: &Session, landxml_unit: &str) -> crate::Result<Vec<(&'static str, &'static str, Value)>> {
    let al = s.alignment.as_ref().ok_or("create_alignment first")?;
    let mut parts = Vec::new();
    // alignments
    let elems: Vec<Value> = al
        .elements
        .iter()
        .map(|e| {
            let end = e.point(e.len);
            let k = if e.k0.abs() > 1e-12 { e.k0 } else { e.k1 };
            let turn = if k.abs() < 1e-12 {
                Value::Null
            } else if k > 0.0 {
                json!("left")
            } else {
                json!("right")
            };
            let center = if matches!(e.kind, Kind::Arc) {
                let r = 1.0 / e.k0;
                json!([e.start[0] - r * e.az0.sin(), e.start[1] + r * e.az0.cos()])
            } else {
                Value::Null
            };
            json!({"type": match e.kind { Kind::Line => "line", Kind::Arc => "arc", Kind::Spiral => "spiral" },
                "sta_start": e.sta, "length": e.len, "start": e.start, "end": end,
                "direction_start_deg": azd(e.az0), "direction_end_deg": azd(e.heading(e.len)),
                "radius_start": rad(e.k0), "radius_end": rad(e.k1), "turn": turn, "center": center,
                "spiral_type": if matches!(e.kind, Kind::Spiral) { json!("clothoid") } else { Value::Null }})
        })
        .collect();
    parts.push((
        "alignments",
        "alignments.json",
        doc(
            "alignments",
            json!({"alignments": [{"name": al.name, "sta_start": al.start_station, "length": al.length,
        "pis": al.pis, "station_equations": [], "elements": elems}]}),
        ),
    ));
    // profiles: design (PVIs) and existing ground sampled from the EG TIN
    let mut profs = Vec::new();
    let end = al.start_station + al.length;
    let stations: Vec<f64> = {
        let mut v: Vec<f64> = (0..).map(|i| al.start_station + 10.0 * i as f64).take_while(|x| *x < end - 1e-6).collect();
        v.push(end);
        v
    };
    if let Some(p) = &s.profile {
        let n = p.input.vpis.len();
        let pvis: Vec<Value> = p
            .input
            .vpis
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let lc = if i > 0 && i + 1 < n { p.input.curve_lengths.get(i - 1).copied().unwrap_or(0.0) } else { 0.0 };
                json!({"station": v[0], "elevation": v[1], "curve_length": lc, "curve_type": if lc > 0.0 { "symmetric_parabola" } else { "none" }})
            })
            .collect();
        let samples: Vec<Value> = stations.iter().filter_map(|&st| p.elevation_at(st).map(|z| json!({"station": st, "elevation": z}))).collect();
        profs.push(json!({"name": p.input.name, "alignment": al.name, "kind": "design", "pvis": pvis, "samples": samples, "grades_pct": p.grades_pct}));
    }
    if let Some((t, _)) = &s.tin {
        let samples: Vec<Value> = stations
            .iter()
            .filter_map(|&st| {
                let (x, y, _) = al.point_at(st)?;
                t.elevation_at(x, y).map(|z| json!({"station": st, "elevation": z}))
            })
            .collect();
        profs.push(json!({"name": "EG", "alignment": al.name, "kind": "existing_ground", "pvis": [], "samples": samples, "grades_pct": []}));
    }
    parts.push(("profiles", "profiles.json", doc("profiles", json!({"profiles": profs}))));
    // superelevation
    let se = match &s.superelevation {
        Some(se) => {
            let c: Vec<Value> = se
                .curves
                .iter()
                .filter(|c| c.e > 0.0)
                .map(|c| {
                    let lc = c.a_in + (c.b_in - c.a_in) * se.nc / (c.e + se.nc);
                    let lco = c.a_out - (c.a_out - c.b_out) * se.nc / (c.e + se.nc);
                    json!({"curve": c.curve, "radius": c.radius, "turn": if c.dir > 0.0 { "left" } else { "right" }, "e_design": c.e,
                        "runout_start": c.a_in, "level_crown": lc, "full_super_start": c.b_in, "full_super_end": c.b_out, "level_crown_out": lco,
                        "runout_end": c.a_out, "relative_gradient": c.relative_gradient, "method": c.method})
                })
                .collect();
            json!({"alignment": al.name, "normal_crown": se.nc, "pivot": "centreline", "rotation_width": se.rotation_width, "curves": c})
        }
        None => json!({"alignment": al.name, "normal_crown": 0.0, "pivot": "centreline", "rotation_width": 0.0, "curves": []}),
    };
    parts.push(("superelevation", "superelevation.json", doc("superelevation", se)));
    // assemblies
    let mut asm = Vec::new();
    if let Some(a) = s.corridor.as_ref().map(|c| &c.assembly).or(s.assembly.as_ref()) {
        asm.push(assembly(a));
    }
    parts.push(("assemblies", "assemblies.json", doc("assemblies", json!({"assemblies": asm}))));
    // corridors
    let mut cors = Vec::new();
    if let Some(c) = &s.corridor {
        let st = &c.sample_stations;
        let gaps: Vec<f64> = st.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 1e-6).collect();
        let on_kind = |k: fn(&Kind) -> bool| -> f64 {
            let g: Vec<f64> = st
                .windows(2)
                .filter(|w| al.elements.iter().any(|e| k(&e.kind) && w[0] >= e.sta - 1e-6 && w[1] <= e.sta + e.len + 1e-6))
                .map(|w| w[1] - w[0])
                .filter(|d| *d > 1e-6)
                .collect();
            g.iter().cloned().fold(0.0, f64::max)
        };
        let mut targets = vec![json!({"kind": "surface", "name": "EG", "role": "daylight (side slopes tie to existing ground)"})];
        if !c.widening.is_empty() {
            targets.push(
                json!({"kind": "width", "name": "curve widening", "role": format!("lane widening on {} curve(s), computed by civil-mcp", c.widening.len())}),
            );
        }
        if c.assembly.wall.is_some() {
            targets.push(json!({"kind": "offset", "name": "wall rule", "role": "retaining wall where the wall rule triggers"}));
        }
        cors.push(json!({"name": "Corridor", "alignment": al.name, "profile": s.profile.as_ref().map(|p| p.input.name.clone()).unwrap_or_default(),
            "regions": [{"name": "R1", "sta_start": st.first().copied().unwrap_or(al.start_station), "sta_end": st.last().copied().unwrap_or(end),
                "assembly": c.assembly.name,
                "frequency": {"tangent": on_kind(|k| matches!(k, Kind::Line)), "curve": on_kind(|k| matches!(k, Kind::Arc)), "spiral": on_kind(|k| matches!(k, Kind::Spiral)),
                    "min_spacing": gaps.iter().cloned().fold(f64::INFINITY, f64::min).min(1e9), "max_spacing": gaps.iter().cloned().fold(0.0, f64::max), "sample_stations": st.len()},
                "targets": targets}],
            "results": {"cut_m3": c.cut_m3, "fill_m3": c.fill_m3, "sections": c.sections.len(), "sections_with_warnings": c.sections_with_warnings, "method": c.method}}));
    }
    parts.push(("corridors", "corridors.json", doc("corridors", json!({"corridors": cors}))));
    // surfaces (geometry lives in the LandXML file)
    let mut sf = Vec::new();
    if let Some((t, st)) = &s.tin {
        sf.push(
            json!({"name": "EG", "kind": "existing_ground", "landxml_file": LANDXML, "landxml_surface": "EG", "points": t.pts.len(), "triangles": t.tris.len(),
            "elev_min": st.min_z, "elev_max": st.max_z, "breaklines": [], "boundary": null}),
        );
    }
    if let Some(t) = &s.corridor_fg {
        let (lo, hi) = t.pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, p| (a.0.min(p[2]), a.1.max(p[2])));
        sf.push(json!({"name": crate::landxml::FG_SURFACE, "kind": "finished_grade", "landxml_file": LANDXML, "landxml_surface": crate::landxml::FG_SURFACE,
            "points": t.pts.len(), "triangles": t.tris.len(), "elev_min": lo, "elev_max": hi,
            "breaklines": if s.corridor.is_some() { json!(["CL", "EP-L", "EP-R", "DAYLIGHT-L", "DAYLIGHT-R"]) } else { json!([]) },
            "boundary": if s.corridor.is_some() { json!("daylight (outer)") } else { Value::Null }}));
    }
    let _ = landxml_unit;
    parts.push(("surfaces", "surfaces.json", doc("surfaces", json!({"surfaces": sf}))));
    // pipes
    let nets: Vec<Value> = s
        .networks
        .iter()
        .map(|n| {
            let lname = n.name.to_lowercase();
            json!({"name": n.name, "kind": if lname.contains("storm") { "storm" } else if lname.contains("sanit") || lname.contains("sewer") { "sanitary" } else { "other" },
                "structures": n.structures.iter().map(|(id, x, y, r, sp)| json!({"id": id, "e": x, "n": y, "rim": r, "sump": sp})).collect::<Vec<_>>(),
                "pipes": n.pipes.iter().map(|p| json!({"id": p.id, "from": p.from, "to": p.to, "length": p.length, "diameter": p.diameter_m, "material": p.material,
                    "invert_up": p.invert_up, "invert_dn": p.invert_dn, "slope": p.slope, "design_flow": p.q_design})).collect::<Vec<_>>()})
        })
        .collect();
    parts.push(("pipes", "pipes.json", doc("pipes", json!({"networks": nets}))));
    // parcels and ROW
    let (src, pl) = match &s.parcels {
        Some(ps) => (
            ps.source.clone(),
            ps.parcels
                .iter()
                .map(|p| {
                    let (mut outer, mut holes) = (vec![], vec![]);
                    for r in &p.rings {
                        if signed_area(r) >= 0.0 {
                            outer.push(r.clone())
                        } else {
                            holes.push(r.clone())
                        }
                    }
                    json!({"id": p.id, "area": p.area_m2, "zoning": p.zoning, "no_go": p.no_go, "outer_rings": outer, "holes": holes})
                })
                .collect::<Vec<_>>(),
        ),
        None => (String::new(), vec![]),
    };
    let row = s.corridor.as_ref().map(|c| {
        let b = s.row_opts.as_ref().map_or(0.0, |o| o.buffer_m);
        let r = crate::parcels::row_polygon(c, b);
        json!({"buffer": b, "area": signed_area(&r).abs(), "polygon": r})
    });
    parts.push(("parcels", "parcels.json", doc("parcels", json!({"source": src, "parcels": pl, "row": row}))));
    // quantities
    let (boq, _) = crate::deliver::boq(s);
    let q = json!({"earthwork": {"corridor_cut_m3": s.corridor.as_ref().map_or(0.0, |c| c.cut_m3), "corridor_fill_m3": s.corridor.as_ref().map_or(0.0, |c| c.fill_m3),
        "layers_m3": s.corridor.as_ref().map_or(vec![], |c| c.layer_m3.iter().map(|(n, v)| json!({"name": n, "volume_m3": v})).collect::<Vec<_>>()),
        "mass_haul": s.earthwork.as_ref().map(|e| serde_json::to_value(e).unwrap_or(Value::Null))},
        "boq": boq.iter().map(|r| json!({"code": r.code, "item": r.item, "unit": r.unit, "qty": r.qty, "basis": r.basis})).collect::<Vec<_>>()});
    parts.push(("quantities", "quantities.json", doc("quantities", q)));
    // sheets: the registers written by generate_drawings / generate_construction_set in this run
    let mut sh = Vec::new();
    if let Ok(dir) = s.dir() {
        for (reg, set, file_col, fixed_file) in
            [("sheet-register.csv", "drawings", Some("dxf"), None), ("construction-register.csv", "construction set", None, Some("construction-set.pdf"))]
        {
            let Ok(t) = std::fs::read_to_string(dir.join(reg)) else { continue };
            let mut rows = csv_rows(&t).into_iter();
            let Some(h) = rows.next() else { continue };
            let col = |r: &Vec<String>, k: &str| h.iter().position(|c| c == k).and_then(|i| r.get(i).cloned());
            for r in rows {
                sh.push(json!({"set": set, "file": file_col.and_then(|c| col(&r, c)).or(fixed_file.map(String::from)).unwrap_or_default(),
                    "page": col(&r, "page").and_then(|p| p.parse::<u64>().ok()).unwrap_or(0), "sheet_id": col(&r, "sheet_id").unwrap_or_default(),
                    "title": col(&r, "title").unwrap_or_default(), "scale": col(&r, "scale")}));
            }
        }
    }
    parts.push(("sheets", "sheets.json", doc("sheets", json!({"sheets": sh}))));
    Ok(parts)
}

fn signed_area(r: &[[f64; 2]]) -> f64 {
    let n = r.len();
    (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).sum::<f64>() / 2.0
}

fn csv_rows(t: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for line in t.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let (mut row, mut cur, mut q) = (Vec::new(), String::new(), false);
        let mut it = line.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                '"' if q && it.peek() == Some(&'"') => {
                    cur.push('"');
                    it.next();
                }
                '"' => q = !q,
                ',' if !q => row.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            }
        }
        row.push(cur);
        out.push(row);
    }
    out
}

fn assembly(a: &crate::corridor::Assembly) -> Value {
    let src = serde_json::to_value(a).unwrap_or(Value::Null);
    let mut subs = Vec::new();
    let mut push = |name: String, kind: &str, side: &str, params: Value| {
        let order = subs.len();
        subs.push(json!({"name": name, "kind": kind, "side": side, "order": order, "params": params}));
    };
    for side in ["left", "right"] {
        let ov = if side == "right" { a.right.as_ref() } else { None };
        let removed = |k: &str| ov.is_some_and(|o| o.remove.iter().any(|r| r == k));
        let lanes = ov.and_then(|o| o.lanes_m.clone()).unwrap_or_else(|| a.lanes_m.clone());
        for (i, w) in lanes.iter().enumerate() {
            push(format!("{side} lane {}", i + 1), "lane", side, json!({"width": w, "slope": "normal crown / superelevation (superelevation.json)"}));
        }
        let curb = ov.and_then(|o| o.curb.clone()).or_else(|| a.curb.clone());
        let shoulder = ov.and_then(|o| o.shoulder_m).unwrap_or(a.shoulder_m);
        if let Some(c) = curb.filter(|_| !removed("curb")) {
            push(
                format!("{side} curb and gutter"),
                "curb_gutter",
                side,
                json!({"gutter_width": c.gutter_m, "gutter_slope": c.gutter_slope, "curb_height": c.height_m, "curb_top_width": c.top_m}),
            );
            if c.sidewalk_m > 0.0 {
                push(format!("{side} sidewalk"), "sidewalk", side, json!({"width": c.sidewalk_m, "slope": c.sidewalk_slope}));
            }
        } else if shoulder > 0.0 && !removed("shoulder") {
            push(
                format!("{side} shoulder"),
                "shoulder",
                side,
                json!({"width": shoulder, "slope": ov.and_then(|o| o.shoulder_slope).unwrap_or(a.shoulder_slope),
                "paved": ov.and_then(|o| o.shoulder_paved).unwrap_or(a.shoulder_paved)}),
            );
        }
        if let Some(d) = ov.and_then(|o| o.ditch.clone()).or_else(|| a.ditch.clone()).filter(|_| !removed("ditch")) {
            push(
                format!("{side} ditch (cut only)"),
                "ditch",
                side,
                json!({"foreslope_h_per_v": d.foreslope_h, "depth": d.depth_m, "bottom_width": d.bottom_width_m}),
            );
        }
        if let Some(w) = ov.and_then(|o| o.wall.clone()).or_else(|| a.wall.clone()) {
            push(format!("{side} retaining wall rule"), "wall", side, serde_json::to_value(w).unwrap_or(Value::Null));
        }
        push(
            format!("{side} daylight"),
            "daylight",
            side,
            json!({"target": "EG", "cut_slope_h_per_v": ov.and_then(|o| o.cut_slope_h).unwrap_or(a.cut_slope_h),
            "fill_slope_h_per_v": ov.and_then(|o| o.fill_slope_h).unwrap_or(a.fill_slope_h), "cut_bench": src["cut_bench"], "fill_bench": src["fill_bench"]}),
        );
    }
    if !a.pavement.is_empty() {
        push(
            "pavement structure".into(),
            "pavement_structure",
            "both",
            json!({"layers": a.pavement.iter().map(|l| json!({"name": l.name, "thickness": l.thickness_m})).collect::<Vec<_>>(),
            "subgrade_slope_h_per_v": a.subgrade_slope_h}),
        );
    }
    json!({"name": a.name, "subassemblies": subs, "source_parameters": src})
}

/// Minimal JSON Schema validator for the keywords used by the committed v1 schemas:
/// type, const, enum, required, properties, additionalProperties (bool), items, minItems,
/// maxItems, minimum, $ref (local "#/$defs/..."). Other keywords are ignored; CI also runs
/// the full Python `jsonschema` validator.
pub fn validate(v: &Value, schema: &Value) -> Vec<String> {
    let mut e = Vec::new();
    check(v, schema, schema, "$", &mut e);
    e
}

fn type_ok(v: &Value, t: &str) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "number" => v.is_number(),
        "integer" => v.is_i64() || v.is_u64() || v.as_f64().is_some_and(|x| x.fract() == 0.0),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        _ => false,
    }
}

fn check(v: &Value, s: &Value, root: &Value, path: &str, e: &mut Vec<String>) {
    if e.len() > 50 {
        return;
    }
    if let Some(r) = s["$ref"].as_str() {
        let mut t = root;
        for k in r.trim_start_matches("#/").split('/') {
            t = &t[k];
        }
        return check(v, t, root, path, e);
    }
    match &s["type"] {
        Value::String(t) if !type_ok(v, t) => return e.push(format!("{path}: expected {t}")),
        Value::Array(ts) if !ts.iter().any(|t| type_ok(v, t.as_str().unwrap_or(""))) => {
            return e.push(format!("{path}: expected one of {}", Value::Array(ts.clone())))
        }
        _ => {}
    }
    if let Some(c) = s.get("const") {
        if v != c {
            e.push(format!("{path}: expected {c}"));
        }
    }
    if let Some(en) = s["enum"].as_array() {
        if !en.contains(v) {
            e.push(format!("{path}: {v} not in {}", s["enum"]));
        }
    }
    if let (Some(x), Some(m)) = (v.as_f64(), s["minimum"].as_f64()) {
        if x < m {
            e.push(format!("{path}: {x} < minimum {m}"));
        }
    }
    if let Some(o) = v.as_object() {
        for r in s["required"].as_array().into_iter().flatten().filter_map(|r| r.as_str()) {
            if !o.contains_key(r) {
                e.push(format!("{path}: missing '{r}'"));
            }
        }
        let props = s["properties"].as_object();
        for (k, x) in o {
            match props.and_then(|p| p.get(k)) {
                Some(ps) => check(x, ps, root, &format!("{path}.{k}"), e),
                None if s["additionalProperties"] == json!(false) => e.push(format!("{path}: unexpected '{k}'")),
                None => {}
            }
        }
    }
    if let Some(a) = v.as_array() {
        if let Some(m) = s["minItems"].as_u64() {
            if (a.len() as u64) < m {
                e.push(format!("{path}: {} items < minItems {m}", a.len()));
            }
        }
        if let Some(m) = s["maxItems"].as_u64() {
            if (a.len() as u64) > m {
                e.push(format!("{path}: {} items > maxItems {m}", a.len()));
            }
        }
        if s["items"].is_object() {
            for (i, x) in a.iter().enumerate() {
                check(x, &s["items"], root, &format!("{path}[{i}]"), e);
            }
        }
    }
}

/// Validates a package folder: manifest against its schema, every listed file present with the
/// recorded size and SHA-256, every JSON part against its schema, and the LandXML linear unit.
pub fn validate_dir(dir: &Path) -> crate::Result<Value> {
    let mtext = std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| format!("{}: {e}", dir.join("manifest.json").display()))?;
    let m: Value = serde_json::from_str(&mtext).map_err(|e| format!("manifest.json: {e}"))?;
    let ms: Value = serde_json::from_str(MANIFEST_SCHEMA).map_err(|e| e.to_string())?;
    let mut errors: Vec<String> = validate(&m, &ms).into_iter().map(|x| format!("manifest.json {x}")).collect();
    let mut files = Vec::new();
    for f in m["files"].as_array().into_iter().flatten() {
        let p = f["path"].as_str().unwrap_or("");
        let role = f["role"].as_str().unwrap_or("");
        let Ok(b) = std::fs::read(dir.join(p)) else {
            errors.push(format!("{p}: missing"));
            continue;
        };
        let h = sha256_hex(&b);
        let ok_hash = Some(h.as_str()) == f["sha256"].as_str() && Some(b.len() as u64) == f["bytes"].as_u64();
        if !ok_hash {
            errors.push(format!("{p}: sha256/bytes differ from manifest"));
        }
        let mut n_err = 0;
        if let Some((_, _, sch)) = PARTS.iter().find(|(r, _, _)| *r == role) {
            match serde_json::from_slice::<Value>(&b) {
                Ok(v) => {
                    let s: Value = serde_json::from_str(sch).map_err(|e| e.to_string())?;
                    let es = validate(&v, &s);
                    n_err = es.len();
                    errors.extend(es.into_iter().map(|x| format!("{p} {x}")));
                }
                Err(e) => {
                    n_err = 1;
                    errors.push(format!("{p}: {e}"));
                }
            }
        } else if role == "landxml" {
            let t = String::from_utf8_lossy(&b);
            let want = m["units"]["landxml_linear_unit"].as_str().unwrap_or("");
            if !t.contains(&format!("linearUnit=\"{want}\"")) {
                n_err = 1;
                errors.push(format!("{p}: linearUnit is not '{want}' as recorded in the manifest"));
            }
        }
        files.push(json!({"path": p, "role": role, "sha256_ok": ok_hash, "schema_errors": n_err}));
    }
    for (role, _, _) in PARTS {
        if !m["files"].as_array().is_some_and(|a| a.iter().any(|f| f["role"] == *role)) {
            errors.push(format!("manifest: no file with role '{role}'"));
        }
    }
    Ok(json!({"valid": errors.is_empty(), "schema_version": m["schema_version"], "files": files, "errors": errors}))
}
