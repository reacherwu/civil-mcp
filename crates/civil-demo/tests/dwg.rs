//! Optional DWG path: without ODA File Converter the tool says so and still writes the DXF; with it
//! (ODA_FILE_CONVERTER set, as on a developer box) the model and every sheet are written as DWG
//! with the requested version magic.
#[path = "../src/main.rs"]
#[allow(dead_code)]
mod demo;
use serde_json::json;

#[test]
fn dwg_export_optional() {
    let out = std::env::temp_dir().join(format!("civil-dwg-{}", std::process::id())).to_string_lossy().to_string();
    let mut s = demo::run(&out);
    if civil_core::dwg::find_converter().is_none() {
        let e = s.call("export_cad_model", &json!({"dwg_version": "ACAD2018"})).unwrap_err();
        assert!(e.contains("ODA File Converter not found"), "{e}");
        let r = s.call("export_cad_model", &json!({})).unwrap();
        assert!(r["result"]["dwg"].as_str().unwrap().starts_with("not written"));
        return;
    }
    for (ver, magic) in [("ACAD2013", "AC1027"), ("ACAD2018", "AC1032")] {
        let r = s.call("export_cad_model", &json!({"dwg_version": ver, "dwg_include_sheets": true})).unwrap();
        let files = r["result"]["dwg"]["files"].as_array().unwrap();
        assert_eq!(files.len(), 1 + 38 + 14, "{ver}");
        assert!(files.iter().all(|f| f["magic"] == magic), "{ver}");
    }
    assert!(s.call("export_cad_model", &json!({"dwg_version": "R12"})).is_err());
}
