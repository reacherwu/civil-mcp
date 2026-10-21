//! US customary units: the full demo renders all sheets with zero mandatory failures in US
//! customary units, the default stays SI, and values round-trip through the MCP boundary.
#[path = "../src/main.rs"]
#[allow(dead_code)]
mod demo;
use serde_json::json;

#[test]
fn us_customary_full_set_and_round_trip() {
    let dir = std::env::temp_dir().join("civil-units-us");
    let out = dir.to_str().unwrap();
    let mut s = demo::run_units(out, true);
    let rd = |f: &str| std::fs::read_to_string(dir.join(f)).unwrap();

    // design checks are unit independent
    let checks = s.all_checks();
    assert_eq!(checks.iter().filter(|c| !c.pass && c.severity == "mandatory").count(), 0);
    let ds = s.call("check_drawing_standards", &json!({})).unwrap();
    let dsf: Vec<_> = ds["result"]["checks"].as_array().unwrap().iter().filter(|c| c["pass"] == false && c["severity"] == "mandatory").collect();
    assert!(dsf.is_empty(), "{dsf:?}");

    // 38 sheets, register with US scales
    let reg = rd("sheet-register.csv");
    assert!(reg.starts_with("page,sheet_id,title,revision,status,size,dxf,svg,scale,units\n"));
    assert_eq!(reg.lines().count(), 39);
    assert!(reg.contains("US customary") && reg.contains("\"\""), "{reg}");
    for i in 1..=38 {
        assert!(dir.join(format!("sheet-{i:02}.svg")).exists());
    }
    let p3 = rd("sheet-03.svg");
    assert!(p3.contains("PLAN  1&quot; = ") || p3.contains("PLAN  1\" = "), "plan scale");
    assert!(p3.contains("contours 2 ft"));
    assert!(p3.contains(" ft</text>"), "scale bar in feet");
    let ft_sta = regex_like_station(&p3);
    assert!(ft_sta, "100 ft stations");
    let dsj: serde_json::Value = serde_json::from_str(&rd("drawing-standards.json")).unwrap();
    assert_eq!(dsj["units"]["system"], "US");
    let fr09 = dsj["checks"].as_array().unwrap().iter().find(|c| c["id"] == "FR-09").unwrap();
    assert_eq!(fr09["unit"], "in");

    // BoQ in CY / SY / LF and equal to the SI corridor volumes
    let boq = rd("boq.csv");
    let (cut_m3, stations): (f64, Vec<f64>) = {
        let c = s.corridor.as_ref().unwrap();
        (c.cut_m3, c.sections.iter().map(|x| x.station).collect())
    };
    let exc: f64 = boq.lines().find(|l| l.contains("Roadway excavation")).unwrap().split(',').nth(3).unwrap().parse().unwrap();
    assert!(boq.contains(",CY,") && boq.contains(",LF,") && boq.contains(",SY,"));
    assert!((exc * 0.764_554_857_984 - cut_m3).abs() < 0.01, "{exc} CY vs {cut_m3} m3");
    let so = rd("setting-out.csv");
    assert!(so.contains("radius_ft") && so.contains("station_ft"));
    assert!(rd("design-report.md").contains(" ft"));

    // MCP boundary: US outputs and inputs (survey foot) round trip
    s.call("set_units", &json!({"system": "US", "foot": "survey"})).unwrap();
    let st_ft = 1000.0;
    let r = s.call("get_section", &json!({"station": st_ft})).unwrap();
    assert_eq!(r["units"], "US customary (US survey foot)");
    let si_m = st_ft * 1200.0 / 3937.0;
    let near = stations.iter().cloned().min_by(|a, b| (a - si_m).abs().partial_cmp(&(b - si_m).abs()).unwrap()).unwrap();
    let got = r.to_string();
    assert!(got.contains(&format!("{}", (near * 3937.0 / 1200.0 * 1e6).round() / 1e6).chars().take(6).collect::<String>()), "{got}");
    let cs = s.call("check_standards", &json!({})).unwrap();
    assert!(cs.to_string().contains("\"unit\":\"ft\""));
    // back to SI: the same call gives metres
    s.call("set_units", &json!({"system": "SI"})).unwrap();
    let r = s.call("get_section", &json!({"station": si_m})).unwrap();
    assert!(r.get("units").is_none());
}

fn regex_like_station(s: &str) -> bool {
    // a label like 15+00.00 (two digits after '+', before the decimal point)
    s.as_bytes().windows(7).any(|w| {
        w[0].is_ascii_digit()
            && w[1] == b'+'
            && w[2].is_ascii_digit()
            && w[3].is_ascii_digit()
            && w[4] == b'.'
            && w[5].is_ascii_digit()
            && w[6].is_ascii_digit()
    })
}

#[test]
fn default_is_si() {
    let s = civil_core::tools::Session::default();
    assert!(!s.unit_spec().files().us());
    let mut s = s;
    let v = s.call("set_units", &json!({})).unwrap();
    assert_eq!(v["result"]["system"], "SI");
    assert_eq!(v["result"]["default"], true);
}
