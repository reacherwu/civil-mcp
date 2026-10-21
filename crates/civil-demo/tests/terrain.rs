//! Terrain import through the MCP tool, and infeasibility / earthwork-limit handling of the
//! route search, on synthetic grids written as ESRI ASCII files.
use civil_core::tools::Session;
use serde_json::json;

fn asc(path: &std::path::Path, n: usize, cell: f64, z: impl Fn(usize, usize) -> f64) {
    let mut s = format!("ncols {n}\nnrows {n}\nxllcorner 500000\nyllcorner 4000000\ncellsize {cell}\nNODATA_value -9999\n");
    for r in 0..n {
        let row: Vec<String> = (0..n).map(|c| format!("{:.3}", z(n - 1 - r, c))).collect();
        s.push_str(&row.join(" "));
        s.push('\n');
    }
    std::fs::write(path, s).unwrap();
}

fn session(dir: &std::path::Path) -> Session {
    let mut s = Session::default();
    s.call("open_project", &json!({"dir": dir.to_str().unwrap()})).unwrap();
    s.call("set_design_basis", &json!({"actor":"engineer:TEST","basis":{"project":"terrain test","engineer_of_record":"TEST","standard":"US_AASHTO_2018","design_speed_kmh":50.0,"e_max":0.08,"f_max":0.18,"max_grade_pct":8.0,"normal_crown":0.02,"lane_width_m":3.5}})).unwrap();
    s
}

const ASM: &str = r#"{"name":"t","lanes_m":[3.5],"shoulder_m":1.2,"shoulder_slope":-0.04,"shoulder_paved":false,"cut_slope_h":1.5,"fill_slope_h":2.0}"#;

#[test]
fn import_terrain_and_infeasible_route_stops_with_reason() {
    let dir = std::env::temp_dir().join(format!("civil-terrain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // 1 km x 1 km, 40 % slope towards north: 400 m rise needs 5 km of road at 8 %
    let f = dir.join("steep.asc");
    asc(&f, 101, 10.0, |r, _| r as f64 * 10.0 * 0.40);
    let mut s = session(&dir);
    let r = s.call("import_terrain", &json!({"path": f.to_str().unwrap()})).unwrap();
    assert_eq!(r["result"]["format"], "ESRI ASCII grid");
    assert_eq!(r["result"]["kept_points"], 101 * 101);
    s.call("create_assembly", &serde_json::from_str(ASM).unwrap()).unwrap();
    let e = s
        .call(
            "generate_route_alternatives",
            &json!({"start":[500100.0,4000100.0],"end":[500100.0,4000900.0],"cell_m":20.0,"radius_pref_m":120.0,"spiral_m":0.0}),
        )
        .unwrap_err();
    assert!(e.starts_with("INFEASIBLE"), "{e}");
    assert!(e.contains("needs at least 4000 m of road at the 8.0 % maximum grade"), "{e}");
    assert!(e.contains("EW-") || e.contains("ground grade within"), "{e}");

    // gentle 3 % slope in feet (US survey ft): feasible, heights within limits
    let g = dir.join("gentle_ft.asc");
    asc(&g, 101, 30.0, |r, _| r as f64 * 30.0 * 0.03);
    let mut s = session(&dir);
    let r = s.call("import_terrain", &json!({"path": g.to_str().unwrap(), "xy_units":"us_ft", "z_units":"us_ft"})).unwrap();
    let b = r["result"]["bbox"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>();
    let ft = 1200.0 / 3937.0;
    assert!((b[0] - (500000.0 + 15.0) * ft).abs() < 1e-6, "{b:?}");
    s.call("create_assembly", &serde_json::from_str(ASM).unwrap()).unwrap();
    let (x0, y0) = (b[0] + 100.0, b[1] + 100.0);
    let v =
        s.call("generate_route_alternatives", &json!({"start":[x0,y0],"end":[x0+600.0,y0+600.0],"cell_m":20.0,"radius_pref_m":120.0,"spiral_m":0.0})).unwrap();
    let alts = v["result"]["alternatives"].as_array().unwrap();
    assert_eq!(alts.len(), 4);
    let rec = v["result"]["recommendation"]["recommended"].as_str().unwrap();
    let e = &alts.iter().find(|a| a["name"] == rec).unwrap()["evaluation"];
    assert!(e["max_fill_height_m"].as_f64().unwrap() <= 15.0 && e["max_cut_height_m"].as_f64().unwrap() <= 15.0, "{e}");
    assert!(!e["failed_checks"].to_string().contains("EW-"), "{e}");
}

#[test]
fn parcels_unknown_rate_fallback_and_row_cost_surface() {
    let dir = std::env::temp_dir().join(format!("civil-parcels-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let g = dir.join("gentle.asc");
    asc(&g, 81, 10.0, |r, c| (r + c) as f64 * 10.0 * 0.02);
    let mut s = session(&dir);
    s.call("import_terrain", &json!({"path": g.to_str().unwrap()})).unwrap();
    s.call("create_assembly", &serde_json::from_str(ASM).unwrap()).unwrap();
    let (x0, y0) = (500100.0, 4000100.0);
    let sq = |a: f64, b: f64, c: f64, d: f64| json!([[[a, b], [c, b], [c, d], [a, d], [a, b]]]);
    // land at rate 1 with a hole for a commercial block on the straight line that has no rate
    let hole = json!([[500250.0, 4000250.0], [500250.0, 4000550.0], [500550.0, 4000550.0], [500550.0, 4000250.0], [500250.0, 4000250.0]]);
    let mut rest = sq(500000.0, 4000000.0, 500800.0, 4000800.0);
    rest.as_array_mut().unwrap().push(hole);
    let gj = json!({"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"id":"REST","land_rate":1.0,"improvement_value":0.0},"geometry":{"type":"Polygon","coordinates":rest}},
        {"type":"Feature","properties":{"id":"COM","zoning":"COM","improvement_value":0.0},"geometry":{"type":"Polygon","coordinates":sq(500250.0,4000250.0,500550.0,4000550.0)}}]});
    let f = dir.join("p.geojson");
    std::fs::write(&f, gj.to_string()).unwrap();
    let alts = json!([{"name":"A","w_grade":1.0,"w_terrain":0.0,"w_turn":2.0},{"name":"R","w_grade":1.0,"w_terrain":0.0,"w_turn":2.0,"w_row":40.0}]);
    let args = json!({"start":[x0,y0],"end":[x0+600.0,y0+600.0],"cell_m":20.0,"radius_pref_m":120.0,"spiral_m":0.0,"alternatives":alts});
    s.call("import_parcels", &json!({"path": f.to_str().unwrap()})).unwrap();
    let v = s.call("generate_route_alternatives", &args).unwrap();
    let rec = &v["result"]["recommendation"];
    assert_eq!(rec["row_in_score"], false, "{rec}");
    let a = &v["result"]["alternatives"][0]["row"];
    assert!(a["cost"].is_null() && a["unknown_cost_parcels"].to_string().contains("COM"), "{a}");
    assert!(a["cost_known_part"].as_f64().unwrap() > 0.0);
    // zoning fallback rate makes the cost known; the w_row alternative goes round the block
    s.call("import_parcels", &json!({"path": f.to_str().unwrap(), "fallback_rates": {"COM": 50.0}})).unwrap();
    let v = s.call("generate_route_alternatives", &args).unwrap();
    let rec = &v["result"]["recommendation"];
    assert_eq!(rec["row_in_score"], true, "{rec}");
    let alts = v["result"]["alternatives"].as_array().unwrap();
    let take = |k: usize| alts[k]["row"]["cost"].as_f64().unwrap();
    assert!(take(1) < take(0), "ROW-aware search should be cheaper in ROW: {} vs {}", take(1), take(0));
    let ra: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("route-alternatives.json")).unwrap()).unwrap();
    let path = ra["alternatives"][1]["path"].as_array().unwrap();
    let inside = path.iter().filter(|p| {
        let (x, y) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
        (500260.0..500540.0).contains(&x) && (4000260.0..4000540.0).contains(&y)
    });
    assert_eq!(inside.count(), 0, "w_row path crosses the expensive block");
    assert!(std::fs::read_to_string(dir.join("row-takes.csv")).unwrap().starts_with("alternative,parcel_id"));
    std::fs::remove_dir_all(&dir).ok();
}
