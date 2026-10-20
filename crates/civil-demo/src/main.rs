//! Offline demo: drives the same tool surface an MCP client would, end to end,
//! on public USGS 3DEP terrain. `cargo run --release -p civil-demo`
use civil_core::tools::Session;
use serde_json::{json, Value};

fn call(s: &mut Session, name: &str, a: Value) -> Value {
    match s.call(name, &a) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("tool {name} failed: {e}");
            std::process::exit(1)
        }
    }
}

fn main() {
    let root = env!("CARGO_MANIFEST_DIR");
    let data = format!("{root}/../../data/usgs3dep_palo_alto_points.csv");
    let out = std::env::args().nth(1).unwrap_or_else(|| "demo-out".into());
    let _ = std::fs::remove_dir_all(&out);
    let mut s = Session::default();
    call(&mut s, "open_project", json!({"dir": out}));
    let r = call(&mut s, "set_design_basis", json!({"actor":"engineer:REVIEWER-PLACEHOLDER","basis":{
        "project":"Demo access road (USGS 3DEP terrain, Palo Alto CA)","engineer_of_record":"REVIEWER-PLACEHOLDER",
        "standard":"JTG_D20_2017","design_speed_kmh":40.0}}));
    println!("design basis set, SSD = {:.1} m", r["result"]["ssd_m"]);
    let r = call(&mut s, "load_points", json!({"path": data}));
    println!("points: {}", r["result"]["points"]);
    let r = call(&mut s, "build_surface", json!({}));
    println!("TIN: {} triangles, z {:.1}..{:.1} m", r["result"]["triangles"], r["result"]["min_z"], r["result"]["max_z"]);
    let r = call(&mut s, "create_alignment", json!({"name":"CL","pis":[[573520.0,4139660.0],[573790.0,4139880.0],[574000.0,4139930.0],[574280.0,4140380.0]],"radii":[150.0,120.0]}));
    println!("alignment: {:.2} m", r["result"]["length_m"]);
    let end = r["result"]["end_station"].as_f64().unwrap();
    // 1st attempt: agent drafts a profile with short vertical curves
    let draft: Vec<f64> = vec![0.0, 200.0, 400.0, end];
    let g: Vec<Value> = draft.iter().map(|st| { let v = call(&mut s, "sample_ground_profile", json!({"interval_m": 1.0})); v["result"]["samples"][*st as usize].clone() }).collect();
    let vpis: Vec<[f64; 2]> = draft.iter().zip(&g).map(|(st, v)| [*st, v[1].as_f64().unwrap_or(100.0)]).collect();
    call(&mut s, "create_profile", json!({"name":"draft","vpis":vpis,"curve_lengths":[20.0,20.0]}));
    let c = call(&mut s, "check_standards", json!({}));
    println!("draft profile: {} checks, {} mandatory failures", c["result"]["total"], c["result"]["mandatory_failures"]);
    // 2nd attempt: ask for a standards-driven proposal and adopt it
    let p = call(&mut s, "suggest_profile", json!({"vpi_stations": [0.0, 180.0, 360.0, 540.0, end]}));
    let prop = p["result"]["proposal"].clone();
    println!("suggested profile fits: {}", p["result"]["fits"]);
    call(&mut s, "create_profile", prop);
    let c = call(&mut s, "check_standards", json!({}));
    println!("adopted profile: {} checks, {} mandatory failures, {} advisory", c["result"]["total"], c["result"]["mandatory_failures"], c["result"]["advisory"]);
    let e = call(&mut s, "compute_earthwork", json!({"template":{"half_width_m":4.5,"cross_slope":0.02,"cut_slope_h":1.5,"fill_slope_h":1.5},"interval_m":10.0}));
    println!("earthwork: cut {:.0} m3, fill {:.0} m3, sections with warnings {}", e["result"]["cut_m3"].as_f64().unwrap(), e["result"]["fill_m3"].as_f64().unwrap(), e["result"]["sections_with_warnings"]);
    call(&mut s, "generate_drawings", json!({}));
    let r = call(&mut s, "generate_review_report", json!({}));
    println!("review report: {}", r["result"]["audit"]);
    println!("outputs in {out}/ : audit.jsonl earthwork.json model.dxf sheet.dxf sheet.pdf review.html");
}
