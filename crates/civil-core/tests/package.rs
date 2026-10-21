use civil_core::package::{validate, validate_dir, MANIFEST_SCHEMA, PARTS};
use serde_json::{json, Value};

fn schema(role: &str) -> Value {
    serde_json::from_str(PARTS.iter().find(|p| p.0 == role).unwrap().2).unwrap()
}

#[test]
fn schemas_parse_and_subset_validator_catches_errors() {
    for (_, _, s) in PARTS {
        let _: Value = serde_json::from_str(s).unwrap();
    }
    let _: Value = serde_json::from_str(MANIFEST_SCHEMA).unwrap();
    let s = schema("alignments");
    let el = json!({"type":"line","sta_start":0.0,"length":10.0,"start":[0.0,0.0],"end":[10.0,0.0],"direction_start_deg":90.0,"direction_end_deg":90.0,
        "radius_start":null,"radius_end":null,"turn":null,"center":null,"spiral_type":null});
    let ok = json!({"schema":"civil-mcp/design-package/alignments","schema_version":"1.0.0","units":"SI",
        "alignments":[{"name":"CL","sta_start":0.0,"length":10.0,"pis":[[0.0,0.0],[10.0,0.0]],"station_equations":[],"elements":[el]}]});
    assert!(validate(&ok, &s).is_empty(), "{:?}", validate(&ok, &s));
    let mut bad = ok.clone();
    bad["alignments"][0]["elements"][0]["type"] = json!("clothoid");
    bad["alignments"][0]["elements"][0]["length"] = json!(-1.0);
    bad["alignments"][0]["pis"] = json!([[0.0, 0.0]]);
    bad["units"] = json!("US");
    bad["extra"] = json!(1);
    let e = validate(&bad, &s);
    for k in ["not in", "minimum", "minItems", "expected \"SI\"", "unexpected 'extra'"] {
        assert!(e.iter().any(|x| x.contains(k)), "{k}: {e:?}");
    }
    let mut miss = ok.clone();
    miss["alignments"][0].as_object_mut().unwrap().remove("elements");
    assert!(validate(&miss, &s).iter().any(|x| x.contains("missing 'elements'")));
}

#[test]
fn validate_dir_detects_missing_and_tampered_files() {
    let d = std::env::temp_dir().join(format!("civil-pkg-test-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let sheets = json!({"schema":"civil-mcp/design-package/sheets","schema_version":"1.0.0","units":"SI","sheets":[]});
    let b = serde_json::to_vec(&sheets).unwrap();
    std::fs::write(d.join("sheets.json"), &b).unwrap();
    let h = civil_core::audit::sha256_hex(&b);
    let m = json!({"schema":"civil-mcp/design-package","schema_version":"1.0.0","created":"2026-10-21","status":"PRELIMINARY",
        "generator":{"name":"civil-mcp","version":"0","url":"u"},"project":{"name":"p","engineer_of_record":"e","standard":"s","design_speed_kmh":50.0},
        "units":{"canonical":"SI","display_system":"SI","display_foot":null,"landxml_linear_unit":"meter"},
        "crs":{"epsg":null,"horizontal_unit":"m","vertical_datum":null,"note":""},
        "source_run":{"project_dir":"x","audit_log":"audit.jsonl","audit_seq":1,"audit_head_sha256":"0","files_in_run":1},
        "files":[{"path":"sheets.json","role":"sheets","schema":"sheets.schema.json","media_type":"application/json","bytes":b.len(),"sha256":h}]});
    std::fs::write(d.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    let r = validate_dir(&d).unwrap();
    // only the other nine roles are missing
    let errs: Vec<String> = r["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap().to_string()).collect();
    assert_eq!(errs.len(), 9, "{errs:?}");
    assert!(errs.iter().all(|e| e.starts_with("manifest: no file with role")));
    std::fs::write(d.join("sheets.json"), b"{\"schema\":\"civil-mcp/design-package/sheets\",\"schema_version\":\"1.0.0\",\"units\":\"SI\",\"sheets\":[{}]}")
        .unwrap();
    let r = validate_dir(&d).unwrap();
    let errs: Vec<String> = r["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap().to_string()).collect();
    assert!(errs.iter().any(|e| e.contains("sha256/bytes differ")), "{errs:?}");
    assert!(errs.iter().any(|e| e.contains("missing 'sheet_id'")), "{errs:?}");
    assert_eq!(r["valid"], json!(false));
    std::fs::remove_dir_all(&d).ok();
}
