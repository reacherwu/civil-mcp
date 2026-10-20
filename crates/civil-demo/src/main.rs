//! Offline demo: drives the same tool surface an MCP client would, end to end,
//! on public USGS 3DEP terrain. `cargo run --release -p civil-demo [out-dir]`
//!
//! Every call and its result summary is appended to <out>/run-log.jsonl so the
//! animation and the README are generated from what actually ran.
use civil_core::tools::Session;
use serde_json::{json, Value};
use std::io::Write;

struct Run {
    s: Session,
    log: std::fs::File,
}

impl Run {
    fn call(&mut self, name: &str, a: Value) -> Value {
        let r = self.s.call(name, &a);
        let (ok, v) = match r {
            Ok(v) => (true, v),
            Err(e) => (false, json!({"error": e})),
        };
        let checks = self.s.all_checks();
        let fails: Vec<String> = checks.iter().filter(|c| !c.pass && c.severity == "mandatory").map(|c| format!("{} {}", c.id, c.element)).collect();
        writeln!(self.log, "{}", json!({"tool": name, "ok": ok, "args": a, "result": v.get("result").cloned().unwrap_or(v.clone()), "checks_total": checks.len(), "mandatory_failures": fails})).unwrap();
        if !ok {
            eprintln!("tool {name} failed: {}", v["error"]);
            std::process::exit(1);
        }
        v
    }
}

fn main() {
    let root = env!("CARGO_MANIFEST_DIR");
    let data = format!("{root}/../../data/usgs3dep_palo_alto_points.csv");
    let out = std::env::args().nth(1).unwrap_or_else(|| "demo-out".into());
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let log = std::fs::File::create(format!("{out}/run-log.jsonl")).unwrap();
    let mut r = Run { s: Session::default(), log };
    r.call("open_project", json!({"dir": out}));
    let v = r.call("set_design_basis", json!({"actor":"engineer:REVIEWER-PLACEHOLDER","basis":{
        "project":"演示道路 Demo access road (USGS 3DEP, Palo Alto CA)","engineer_of_record":"REVIEWER-PLACEHOLDER",
        "standard":"JTG_D20_2017","design_speed_kmh":40.0,"e_max":0.08,"f_max":0.04,"normal_crown":0.02,"lane_width_m":3.5}}));
    println!("design basis: JTG D20-2017, 40 km/h, SSD {} m", v["result"]["ssd_m"]);
    let v = r.call("load_points", json!({"path": data}));
    println!("points: {}", v["result"]["points"]);
    let v = r.call("build_surface", json!({}));
    println!("TIN: {} triangles", v["result"]["triangles"]);
    let pis = json!([[573520.0,4139660.0],[573790.0,4139880.0],[574000.0,4139930.0],[574280.0,4140380.0]]);
    // attempt 1: 40 m spirals
    r.call("create_alignment", json!({"name":"CL","pis":pis,"radii":[150.0,120.0],"spirals":[40.0,40.0]}));
    r.call("design_superelevation", json!({}));
    let c = r.call("check_standards", json!({}));
    println!("alignment with Ls=40 m: {} mandatory failures", c["result"]["mandatory_failures"]);
    // attempt 2: lengthen spirals so the superelevation transition fits inside them
    let v = r.call("create_alignment", json!({"name":"CL","pis":pis,"radii":[150.0,120.0],"spirals":[50.0,50.0]}));
    let end = v["result"]["end_station"].as_f64().unwrap();
    println!("alignment: {:.2} m, closure error {:.2e} m", v["result"]["length_m"].as_f64().unwrap(), v["result"]["closure_error_m"].as_f64().unwrap());
    r.call("design_superelevation", json!({}));
    let p = r.call("suggest_profile", json!({"vpi_stations": [0.0, 180.0, 360.0, 540.0, 720.0, end]}));
    let prop = p["result"]["proposal"].clone();
    r.call("create_profile", prop);
    let c = r.call("check_standards", json!({}));
    println!("geometry checks: {} total, {} mandatory failures", c["result"]["total"], c["result"]["mandatory_failures"]);
    r.call("create_assembly", json!({"name":"二级路 2-lane rural","lanes_m":[3.5],"shoulder_m":0.75,"shoulder_slope":-0.04,"shoulder_paved":false,
        "ditch":{"foreslope_h":4.0,"depth_m":0.6,"bottom_width_m":0.6},"cut_slope_h":1.0,"fill_slope_h":1.5,
        "cut_bench":{"every_m":8.0,"width_m":2.0},"fill_bench":{"every_m":8.0,"width_m":2.0},
        "pavement":[{"name":"沥青面层 AC","thickness_m":0.05},{"name":"水稳基层 Base","thickness_m":0.20},{"name":"底基层 Subbase","thickness_m":0.20}]}));
    let v = r.call("build_corridor", json!({"interval_m": 20.0, "composite_cell_m": 0.5}));
    let res = &v["result"];
    println!("corridor: {} sections, cut {:.0} m3, fill {:.0} m3 (composite check cut {:+.2}%, fill {:+.2}%), warnings {}", res["sections"], res["cut_m3"].as_f64().unwrap(), res["fill_m3"].as_f64().unwrap(), res["composite_check"]["cut_diff_pct"].as_f64().unwrap(), res["composite_check"]["fill_diff_pct"].as_f64().unwrap(), res["sections_with_warnings"]);
    // maintenance yard pad next to the road, at the ground elevation of its centre
    let (cx, cy) = (573700.0, 4139600.0);
    let z = r.s.tin.as_ref().unwrap().0.elevation_at(cx, cy).unwrap().round();
    let v = r.call("grade", json!({"name":"yard","vertices":[[cx-25.0,cy-15.0,z],[cx+25.0,cy-15.0,z],[cx+25.0,cy+15.0,z],[cx-25.0,cy+15.0,z]],"closed":true,"cut_slope_h":2.0,"fill_slope_h":2.0,"spacing_m":1.0,"cell_m":0.25}));
    println!("pad grading at {z} m: cut {:.0} m3, fill {:.0} m3", v["result"]["cut_m3"].as_f64().unwrap(), v["result"]["fill_m3"].as_f64().unwrap());
    // storm drain along the right edge, flowing down-station
    let al = r.s.alignment.clone().unwrap();
    let mut structures = vec![];
    let mut pipes = vec![];
    let stations: Vec<f64> = (0..=10).map(|i| 40.0 + i as f64 * 100.0).filter(|s| *s < end - 10.0).collect();
    for (i, s) in stations.iter().enumerate() {
        let (x, y, h) = al.point_at(*s).unwrap();
        structures.push(json!({"id": format!("YS{}", i + 1), "x": x + 5.0 * h.sin(), "y": y - 5.0 * h.cos(), "area_ha": 0.4, "c": 0.7}));
        if i > 0 {
            pipes.push(json!({"id": format!("P{}", i), "from": format!("YS{}", i), "to": format!("YS{}", i + 1), "diameter_m": if i < 5 { 0.4 } else { 0.6 }, "n": 0.013, "material": "钢筋混凝土管 RCP"}));
        }
    }
    let crit = json!({"standard":"GB50014_2021","system":"storm","min_cover_m":0.7,"min_slope":0.003,"structure_drop_m":0.03,"intensity_mm_h":120.0});
    let v = r.call("design_pipe_network", json!({"name":"雨水 Storm","criteria":crit,"structures":structures,"pipes":pipes}));
    println!("storm network attempt 1: {} mandatory failures (velocity / capacity)", v["result"]["mandatory_failures"]);
    // attempt 2: upsize P4–P5 to 0.6 m and the flat reach to 0.8 m and cap slopes at 3% with drop manholes
    for (i, p) in pipes.iter_mut().enumerate() {
        p["diameter_m"] = json!(if i < 3 { 0.4 } else if i < 5 { 0.6 } else { 0.8 });
        p["max_slope"] = json!(0.03);
    }
    let v = r.call("design_pipe_network", json!({"name":"雨水 Storm","criteria":crit,"structures":structures,"pipes":pipes}));
    println!("storm network attempt 2: {} pipes, {} mandatory failures", v["result"]["pipes"].as_array().unwrap().len(), v["result"]["mandatory_failures"]);
    let v = r.call("compute_quantities", json!({"cut_factor":0.9,"table_step_m":100.0}));
    println!("mass haul: final ordinate {:.0} m3, balance points {}", v["result"]["final_ordinate_m3"].as_f64().unwrap(), v["result"]["balance_stations"]);
    let c = r.call("check_standards", json!({}));
    println!("all checks: {} total, {} mandatory failures, {} advisory", c["result"]["total"], c["result"]["mandatory_failures"], c["result"]["advisory"]);
    r.call("export_landxml", json!({"file":"design.xml"}));
    let v = r.call("generate_drawings", json!({}));
    println!("drawings: {} sheets, status {}", v["result"]["pages"], v["result"]["status"]);
    let v = r.call("generate_review_report", json!({}));
    println!("review report: {}", v["result"]["audit"]);
    println!("outputs in {out}/");
}
