//! Sheet frames and Five Eyes drafting profiles: every country renders the full sheet set on its
//! default frame (and on other sizes / orientations / custom templates) with zero mandatory
//! drawing-standard failures; outputs are checked for size, layer names and sheet ids.
#[path = "../src/main.rs"]
#[allow(dead_code)]
mod demo;
use serde_json::{json, Value};

fn pdf_pages(p: &str) -> Vec<(f64, f64)> {
    let b = std::fs::read(p).unwrap();
    let s = String::from_utf8_lossy(&b);
    s.match_indices("/MediaBox [0 0 ")
        .map(|(i, _)| {
            let t: Vec<f64> = s[i + 15..].split(']').next().unwrap().split_whitespace().map(|x| x.parse().unwrap()).collect();
            (t[0] * 25.4 / 72.0, t[1] * 25.4 / 72.0)
        })
        .collect()
}

fn ds_fails(s: &mut civil_core::tools::Session) -> Vec<String> {
    fails(&s.call("check_drawing_standards", &json!({})).unwrap())
}

fn fails(v: &Value) -> Vec<String> {
    v["result"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["pass"] == false && c["severity"] == "mandatory")
        .map(|c| format!("{} {} {}", c["id"], c["value"], c["element"]))
        .collect()
}

#[test]
fn five_eyes_frames_full_sets() {
    let out = std::env::var("CIVIL_FRAMES_OUT")
        .unwrap_or_else(|_| std::env::temp_dir().join(format!("civil-frames-{}", std::process::id())).to_string_lossy().into());
    let mut s = demo::run(&out);
    let fields = json!({"client": "Test Roads Authority", "project_code": "TST", "originator": "CMC", "checked_by": "", "date": "2026-10-20"});
    // (profile, size, orientation, expected long/short side mm, layer sample, sheet id sample)
    let cases = [
        ("US", "ANSI D", "landscape", (863.6, 558.8), "C-ROAD-CNTR", "G-001"),
        ("CA", "ISO A1", "landscape", (841.0, 594.0), "C-ROAD-CNTR", "G-001"),
        ("UK", "A1", "landscape", (841.0, 594.0), "C-00-M_Alignment", "TST-CMC-ZZ-ZZ-DR-C-0001"),
        ("AU", "A3", "landscape", (420.0, 297.0), "C-00-M_Alignment", "TST-C-0001"),
        ("NZ", "A1", "landscape", (841.0, 594.0), "C-00-M_Alignment", "TST-C-0001"),
        ("US", "ARCH D", "landscape", (914.4, 609.6), "C-ROAD-CNTR", "G-001"),
        ("UK", "A0", "landscape", (1189.0, 841.0), "C-00-M_Alignment", "TST-CMC-ZZ-ZZ-DR-C-0001"),
        ("AU", "A4", "portrait", (210.0, 297.0), "C-00-M_Alignment", "TST-C-0001"),
        ("US", "ANSI B", "landscape", (431.8, 279.4), "C-ROAD-CNTR", "G-001"),
    ];
    for (pf, size, ori, (w, h), layer, id0) in cases {
        let tag = format!("{pf}-{}-{ori}", size.replace(' ', ""));
        let v = s.call("set_sheet_frame", &json!({"frame": {"profile": pf, "size": size, "orientation": ori, "fields": fields}})).unwrap();
        assert_eq!(v["result"]["frame"]["country"], pf);
        let g = s.call("generate_drawings", &json!({})).unwrap();
        assert_eq!(g["result"]["pages"], 38, "{tag}");
        assert_eq!(g["result"]["drawing_standards"]["mandatory_failures"], 0, "{tag}: {:?}", ds_fails(&mut s));
        let c = s.call("check_drawing_standards", &json!({})).unwrap();
        assert!(fails(&c).is_empty(), "{tag}: {:?}", fails(&c));
        let ids = g["result"]["drawing_standards"]["sheet_ids"].as_array().unwrap();
        assert_eq!(ids[0], id0, "{tag}");
        let pages = pdf_pages(&format!("{out}/sheets.pdf"));
        assert_eq!(pages.len(), 38);
        for p in &pages {
            assert!((p.0 - w).abs() < 0.1 && (p.1 - h).abs() < 0.1, "{tag}: page {p:?}");
        }
        for i in [1, 3, 38] {
            let dxf = std::fs::read_to_string(format!("{out}/sheet-{i:02}.dxf")).unwrap();
            assert!(dxf.contains("\n8\n") && dxf.contains("DASHDOT"), "{tag}");
            let svg = std::fs::read_to_string(format!("{out}/sheet-{i:02}.svg")).unwrap();
            assert!(svg.contains(&format!("width=\"{w}mm\"")) || svg.contains(&format!("width=\"{}mm\"", w as i64)), "{tag}: svg size");
            assert!(svg.contains("PRELIMINARY"), "{tag}");
            if i == 3 {
                assert!(dxf.contains(layer), "{tag}: layer {layer}");
                assert!(svg.contains(&format!("id=\"{layer}\"")), "{tag}: svg layer {layer}");
            }
        }
        let reg = std::fs::read_to_string(format!("{out}/sheet-register.csv")).unwrap();
        assert_eq!(reg.lines().count(), 39);
        if std::env::var("CIVIL_FRAMES_OUT").is_ok() {
            let d = format!("{out}/frames/{tag}");
            std::fs::create_dir_all(&d).unwrap();
            for f in ["sheets.pdf", "sheet-03.dxf", "sheet-03.svg", "drawing-standards.json", "sheet-register.csv"] {
                std::fs::copy(format!("{out}/{f}"), format!("{d}/{f}")).unwrap();
            }
        }
    }
    let _ = (s.call("set_sheet_frame", &json!({"frame": {"profile": "US"}})), ());
    if std::env::var("CIVIL_FRAMES_OUT").is_err() {
        let _ = std::fs::remove_dir_all(&out);
    }
}

#[test]
fn frame_rules_templates_and_rejections() {
    let out = std::env::temp_dir().join(format!("civil-frames-b-{}", std::process::id()));
    let out = out.to_string_lossy().to_string();
    let mut s = demo::run(&out);
    // drafting standards: every profile has verified, formula and engineer-input parameters with sources
    for c in ["US", "CA", "UK", "AU", "NZ"] {
        let v = s.call("get_drafting_standard", &json!({"country": c})).unwrap();
        let r = &v["result"];
        for k in ["verified", "formula", "engineer"] {
            assert!(r["counts"][k].as_u64().unwrap() >= 1, "{c} {k}");
        }
        assert!(r["parameters"].as_array().unwrap().iter().all(|p| !p["source"].as_str().unwrap().is_empty()));
    }
    let l = s.call("list_sheet_frames", &json!({})).unwrap();
    assert_eq!(l["result"]["sizes"].as_array().unwrap().len(), 16);
    assert_eq!(l["result"]["profiles"].as_array().unwrap().len(), 5);
    // missing legal owner is a mandatory failure under ISO 7200 (UK profile)
    s.call("set_sheet_frame", &json!({"frame": {"profile": "UK"}})).unwrap();
    let g = s.call("generate_drawings", &json!({})).unwrap();
    assert!(g["result"]["drawing_standards"]["mandatory_failures"].as_u64().unwrap() >= 1);
    let c = s.call("check_drawing_standards", &json!({})).unwrap();
    assert!(fails(&c).iter().any(|f| f.contains("FR-05") && f.contains("legal_owner")), "{:?}", fails(&c));
    // contractual revision C01 while preliminary is refused by FR-07
    s.call("set_sheet_frame", &json!({"frame": {"profile": "UK", "fields": {"client": "X", "revisions": [{"rev": "C01"}]}}})).unwrap();
    s.call("generate_drawings", &json!({})).unwrap();
    let c = s.call("check_drawing_standards", &json!({})).unwrap();
    assert!(fails(&c).iter().any(|f| f.contains("FR-07")), "{:?}", fails(&c));
    // invalid input
    assert!(s.call("set_sheet_frame", &json!({"frame": {"size": "B7"}})).is_err());
    assert!(s.call("set_sheet_frame", &json!({"frame": {"profile": "FR"}})).is_err());
    assert!(s.call("set_sheet_frame", &json!({"frame": {"size": "CUSTOM", "width_mm": 50.0, "height_mm": 50.0}})).is_err());
    assert!(s.call("set_sheet_frame", &json!({"frame": {"unknown_key": 1}})).is_err());
    // custom size + custom layer map
    let lm = json!({"ALIGNMENT": "RD_Centerline", "TEXT": "TX_Notes"});
    s.call(
        "set_sheet_frame",
        &json!({"frame": {"profile": "US", "size": "CUSTOM", "width_mm": 800.0, "height_mm": 500.0, "layer_map": lm, "fields": {"client": "X"}}}),
    )
    .unwrap();
    let g = s.call("generate_drawings", &json!({})).unwrap();
    assert_eq!(g["result"]["drawing_standards"]["mandatory_failures"], 0, "{:?}", ds_fails(&mut s));
    let dxf = std::fs::read_to_string(format!("{out}/sheet-03.dxf")).unwrap();
    assert!(dxf.contains("RD_Centerline"));
    // custom frame templates: SVG, DXF and inline JSON, with the title block replaced by template fields
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="420mm" height="297mm" viewBox="0 0 420 297">
      <rect x="10" y="10" width="400" height="277" fill="none" stroke="black"/>
      <path d="M 10 247 H 410 M 300 247 V 287"/>
      <text x="15" y="25" font-size="5">AGENCY TEMPLATE</text></svg>"#;
    std::fs::write(format!("{out}/frame.svg"), svg).unwrap();
    let mut dxf_t = String::from("0\nSECTION\n2\nENTITIES\n");
    for (a, b, c, d) in
        [(10.0, 10.0, 410.0, 10.0), (410.0, 10.0, 410.0, 287.0), (410.0, 287.0, 10.0, 287.0), (10.0, 287.0, 10.0, 10.0), (10.0, 50.0, 410.0, 50.0)]
    {
        dxf_t += &format!("0\nLINE\n8\n0\n10\n{a}\n20\n{b}\n11\n{c}\n21\n{d}\n");
    }
    dxf_t += "0\nTEXT\n8\n0\n10\n15\n20\n275\n40\n5\n1\nAGENCY DXF TEMPLATE\n0\nENDSEC\n0\nEOF\n";
    std::fs::write(format!("{out}/frame.dxf"), dxf_t).unwrap();
    let fp = json!({"project": [15.0, 40.0, 3.5], "sheet_title": [15.0, 32.0, 3.5], "sheet_id": [310.0, 40.0, 5.0], "sheet_no": [310.0, 32.0, 2.5], "date": [310.0, 26.0, 2.5],
        "revision": [370.0, 26.0, 2.5], "scale": [200.0, 26.0, 2.5], "legal_owner": [15.0, 26.0, 2.5], "drawn_by": [15.0, 20.0, 2.5], "checked_by": [100.0, 20.0, 2.5],
        "approval_person": [15.0, 14.0, 2.5], "document_type": [200.0, 20.0, 2.5], "status": [200.0, 40.0, 3.5], "seal_box": [370.0, 51.0, 30.0], "scale_bar": [200.0, 14.0, 2.5], "north_arrow": [395.0, 60.0, 8.0]});
    for (fmt, path) in [("svg", format!("{out}/frame.svg")), ("dxf", format!("{out}/frame.dxf"))] {
        let t = json!({"path": path, "format": fmt, "drawing_area": [12.0, 52.0, 408.0, 285.0], "replace_title_block": true, "field_positions": fp});
        let v = s.call("set_sheet_frame", &json!({"frame": {"profile": "AU", "size": "A3", "template": t, "fields": {"client": "Agency"}}})).unwrap();
        assert_eq!(v["result"]["frame"]["tb_layout"], "template");
        let g = s.call("generate_drawings", &json!({})).unwrap();
        assert_eq!(g["result"]["drawing_standards"]["mandatory_failures"], 0, "{fmt}: {:?}", ds_fails(&mut s));
        let sv = std::fs::read_to_string(format!("{out}/sheet-03.svg")).unwrap();
        assert!(sv.contains("AGENCY"), "{fmt}");
        let p = s.call("preview_sheet_frame", &json!({})).unwrap();
        assert_eq!(p["result"]["mandatory_failures"], 0, "{fmt}: {}", p["result"]);
        assert!(std::path::Path::new(&format!("{out}/frame-preview.svg")).exists());
    }
    let t = json!({"format": "json", "entities": {"lines": [[10, 10, 410, 10]], "texts": [[15, 280, 5, "INLINE"]]}, "drawing_area": [12.0, 52.0, 408.0, 285.0], "replace_title_block": false});
    s.call("set_sheet_frame", &json!({"frame": {"profile": "NZ", "size": "A3", "template": t, "fields": {"client": "X"}}})).unwrap();
    let g = s.call("generate_drawings", &json!({})).unwrap();
    assert_eq!(g["result"]["drawing_standards"]["mandatory_failures"], 0, "{:?}", ds_fails(&mut s));
    // frame changes do not void an engineer approval, and the approved set shows the approval in the seal box
    s.call("record_review", &json!({"actor": "engineer:TEST", "decision": "approve", "note": "frames test"})).unwrap();
    s.call("set_sheet_frame", &json!({"frame": {"profile": "UK", "fields": {"client": "X", "revisions": [{"rev": "C01"}]}}})).unwrap();
    s.call("generate_drawings", &json!({})).unwrap();
    assert!(ds_fails(&mut s).iter().any(|f| f.contains("checked_by")), "approved sets need the checker's name");
    s.call("set_sheet_frame", &json!({"frame": {"profile": "UK", "fields": {"client": "X", "checked_by": "J. Checker", "revisions": [{"rev": "C01"}]}}}))
        .unwrap();
    let g = s.call("generate_drawings", &json!({})).unwrap();
    assert!(!g["result"]["status"].as_str().unwrap().contains("PRELIMINARY"));
    assert_eq!(g["result"]["drawing_standards"]["mandatory_failures"], 0, "{:?}", ds_fails(&mut s));
    assert!(std::fs::read_to_string(format!("{out}/sheet-01.svg")).unwrap().contains("Approved, audit entry"));
    let _ = std::fs::remove_dir_all(&out);
}
