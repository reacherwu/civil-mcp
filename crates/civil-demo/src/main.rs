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
        let fails: Vec<Value> = checks.iter().filter(|c| !c.pass && c.severity == "mandatory").map(|c| json!({"id": c.id, "element": c.element, "value": c.value, "limit": c.limit, "unit": c.unit, "citation": c.citation})).collect();
        writeln!(self.log, "{}", json!({"tool": name, "ok": ok, "args": a, "result": v.get("result").cloned().unwrap_or(v.clone()), "checks_total": checks.len(), "mandatory_failures": fails})).unwrap();
        if !ok {
            eprintln!("tool {name} failed: {}", v["error"]);
            std::process::exit(1);
        }
        v
    }
}

#[allow(dead_code)]
fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "demo-out".into());
    run(&out);
}

/// Runs the whole demo into `out` and returns the live session.
pub fn run(out: &str) -> Session {
    let root = env!("CARGO_MANIFEST_DIR");
    let data = format!("{root}/../../data/usgs3dep_palo_alto_points.csv");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let log = std::fs::File::create(format!("{out}/run-log.jsonl")).unwrap();
    let mut r = Run { s: Session::default(), log };
    r.call("open_project", json!({"dir": out}));
    // US profile. Values the profile cannot ship (f_max, max grade) are entered by the engineer.
    let v = r.call("set_design_basis", json!({"actor":"engineer:REVIEWER-PLACEHOLDER","basis":{
        "project":"Demo rural collector (USGS 3DEP terrain, Palo Alto CA)","engineer_of_record":"REVIEWER-PLACEHOLDER",
        "standard":"US_AASHTO_2018","design_speed_kmh":50.0,"e_max":0.08,"f_max":0.18,"max_grade_pct":8.0,"normal_crown":0.02,"lane_width_m":3.5}}));
    println!("design basis: US AASHTO 2018, 50 km/h, SSD {:.1} m; engineer inputs: {}", v["result"]["ssd_m"].as_f64().unwrap(), v["result"]["engineer_inputs"]);
    let v = r.call("load_points", json!({"path": data}));
    println!("points: {}", v["result"]["points"]);
    let v = r.call("build_surface", json!({}));
    println!("TIN: {} triangles", v["result"]["triangles"]);
    // pavement: rigid option first for comparison, then the adopted flexible design
    let traffic = json!({"aadt":1500.0,"hv_frac":0.08,"esa_per_hv":1.2,"growth":0.02,"years":20.0,"dir_factor":0.5,"lane_factor":1.0});
    let v = r.call("design_pavement", json!({"kind":"rigid","subgrade_cbr":6.0,"traffic":traffic,"reliability_zr":-1.282,"s0":0.35,"p0":4.5,"pt":2.0,
        "sc_mpa":4.5,"ec_mpa":28000.0,"j":3.2,"cd":1.0,"k_mpa_m":40.0,"layers":[{"name":"Aggregate subbase","min_mm":150.0}]}));
    println!("pavement (rigid option): {}", v["result"]["layers"].as_array().unwrap().iter().map(|l| format!("{} {} mm", l["name"].as_str().unwrap(), l["thickness_mm"])).collect::<Vec<_>>().join(", "));
    let v = r.call("design_pavement", json!({"kind":"flexible","subgrade_cbr":6.0,"traffic":traffic,"reliability_zr":-1.282,"s0":0.45,"p0":4.2,"pt":2.0,
        "layers":[{"name":"Asphalt concrete","a":0.44,"m":1.0,"modulus_mpa":2758.0,"min_mm":75.0},{"name":"Aggregate base","a":0.14,"m":1.0,"modulus_mpa":207.0,"min_mm":150.0},{"name":"Aggregate subbase","a":0.11,"m":1.0,"modulus_mpa":103.0,"min_mm":150.0}]}));
    println!("pavement (adopted flexible): W18 {:.3e}, SN {:.2} required / {:.2} provided; {}", v["traffic"]["design_traffic"].as_f64().unwrap(), v["result"]["sn_required"].as_f64().unwrap(), v["result"]["sn_provided"].as_f64().unwrap(), v["result"]["layers"].as_array().unwrap().iter().map(|l| format!("{} {} mm", l["name"].as_str().unwrap(), l["thickness_mm"])).collect::<Vec<_>>().join(", "));
    r.call("create_assembly", json!({"name":"2-lane rural collector","lanes_m":[3.5],"shoulder_m":1.2,"shoulder_slope":-0.04,"shoulder_paved":false,
        "ditch":{"foreslope_h":4.0,"depth_m":0.6,"bottom_width_m":0.6},"cut_slope_h":1.5,"fill_slope_h":2.0,
        "cut_bench":{"every_m":8.0,"width_m":2.0},"fill_bench":{"every_m":8.0,"width_m":2.0},
        // right side (frontage): curb, gutter and sidewalk instead of shoulder and ditch
        "right":{"curb":{"gutter_m":0.5,"gutter_slope":-0.06,"height_m":0.15,"top_m":0.15,"sidewalk_m":2.0,"sidewalk_slope":0.02},"remove":["ditch"]},
        // walls where the earthwork would leave the 30 m right-of-way or slopes exceed 12 m
        "wall":{"kind":"cantilever","max_daylight_offset_m":30.0,"max_slope_height_m":12.0,"offset_from_hinge_m":0.5,"embedment_m":0.8,
                "soil":{"gamma_kn_m3":19.0,"phi_deg":30.0,"base_friction":0.45,"bearing_kpa":250.0,"surcharge_kpa":10.0}}}));
    // route alternatives over the terrain
    let v = r.call("generate_route_alternatives", json!({"start":[573520.0,4139660.0],"end":[574280.0,4140380.0],"cell_m":10.0,"radius_pref_m":200.0,"spiral_m":40.0,"simplify_tol_m":15.0,"vpi_spacing_m":150.0,"interval_m":20.0,"shrink":0.9}));
    for a in v["result"]["alternatives"].as_array().unwrap() {
        let e = &a["evaluation"];
        println!("route {}: {} PIs, length {:.0} m, max grade {:.2}%, cut {:.0} fill {:.0} m3, wall concrete {:.0} m3, failures {}, cost proxy {:.0}", a["name"].as_str().unwrap(), a["pis"], e["length_m"].as_f64().unwrap_or(f64::NAN), e["max_design_grade_pct"].as_f64().unwrap_or(f64::NAN), e["cut_m3"].as_f64().unwrap_or(f64::NAN), e["fill_m3"].as_f64().unwrap_or(f64::NAN), e["wall_concrete_m3"].as_f64().unwrap_or(f64::NAN), e["geometry_mandatory_failures"], e["cost_proxy"].as_f64().unwrap_or(f64::NAN));
    }
    let rec = v["result"]["recommendation"]["recommended"].as_str().unwrap().to_string();
    println!("recommended route: {rec}");
    let v = r.call("select_route", json!({"alternative": rec, "actor": "agent"}));
    let end = v["result"]["length_m"].as_f64().unwrap();
    r.call("design_superelevation", json!({}));
    let n = (end / 150.0).round().max(1.0) as usize;
    let st: Vec<f64> = (0..=n).map(|i| end * i as f64 / n as f64).collect();
    let p = r.call("suggest_profile", json!({"vpi_stations": st}));
    let prop = p["result"]["proposal"].clone();
    r.call("create_profile", prop);
    let c = r.call("check_standards", json!({}));
    println!("geometry checks: {} total, {} mandatory failures", c["result"]["total"], c["result"]["mandatory_failures"]);
    let v = r.call("optimize_profile", json!({"interval_m":20.0,"shrink":0.9,"w_volume":0.05,"w_wall":5.0,"w_haul":0.002,"steps_m":[2.0,1.0,0.5]}));
    let o = &v["result"];
    println!("profile optimiser: imbalance {:.0} -> {:.0} m3, haul moment {:.0} -> {:.0} m3·m, cut+fill {:.0} -> {:.0} m3, wall concrete {:.0} -> {:.0} m3 ({} evaluations, {} moves)", o["before"]["imbalance_m3"].as_f64().unwrap(), o["after"]["imbalance_m3"].as_f64().unwrap(), o["before"]["haul_moment_m3m"].as_f64().unwrap(), o["after"]["haul_moment_m3m"].as_f64().unwrap(), o["before"]["cut_m3"].as_f64().unwrap() + o["before"]["fill_m3"].as_f64().unwrap(), o["after"]["cut_m3"].as_f64().unwrap() + o["after"]["fill_m3"].as_f64().unwrap(), o["before"]["wall_concrete_m3"].as_f64().unwrap(), o["after"]["wall_concrete_m3"].as_f64().unwrap(), o["evaluations"], o["accepted_moves"]);
    let v = r.call("build_corridor", json!({"interval_m": 20.0, "composite_cell_m": 0.5, "widening": {"method":"AASHTO","vehicle":{"name":"SU-9 (single-unit truck; engineer to verify dimensions)","track_u_m":2.44,"wheelbases_m":[6.1],"front_overhang_m":1.2}}}));
    let res = &v["result"];
    println!("corridor: {} sections, cut {:.0} m3, fill {:.0} m3 (composite check cut {:+.2}%, fill {:+.2}%), warnings {}", res["sections"], res["cut_m3"].as_f64().unwrap(), res["fill_m3"].as_f64().unwrap(), res["composite_check"]["cut_diff_pct"].as_f64().unwrap(), res["composite_check"]["fill_diff_pct"].as_f64().unwrap(), res["sections_with_warnings"]);
    println!("widening: {:?}; walls: {}", res["widening"].as_array().unwrap().iter().map(|w| (w["widening_m"].as_f64().unwrap() * 1000.0).round() / 1000.0).collect::<Vec<_>>(), res["walls"]);
    // 4-leg crossroad, stop-controlled minor road; the agent tries candidate stations until the checks pass
    let minor = json!({"name":"minor road","lanes_m":[3.0],"shoulder_m":0.5,"shoulder_slope":-0.04,"shoulder_paved":false,"cut_slope_h":1.0,"fill_slope_h":1.5,
        "ditch":{"foreslope_h":3.0,"depth_m":0.4,"bottom_width_m":0.4},"pavement":[{"name":"Asphalt concrete","thickness_m":0.05},{"name":"Aggregate base","thickness_m":0.18}]});
    let xin = |sta: f64| json!({"name":"X1","main_station": sta, "major_speed_kmh":50.0,"minor_speed_kmh":30.0,"min_return_radius_m":12.0,"max_approach_grade_pct":3.0,
        "legs":[{"name":"N leg","side":"left","angle_deg":90.0,"length_m":100.0,"grade_pct":-2.0,"return_radii_m":[15.0,15.0],"assembly":minor},
                {"name":"S leg","side":"right","angle_deg":90.0,"length_m":100.0,"grade_pct":2.0,"return_radii_m":[15.0,15.0],"assembly":minor}]});
    let mut xdone = false;
    for k in [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8] {
        let sta = (end * k / 10.0).round() * 10.0;
        let rr = r.s.call("design_intersection", &xin(sta));
        match rr {
            Ok(v) => {
                let f = v["result"]["mandatory_failures"].as_u64().unwrap_or(99);
                println!("intersection at {sta:.0}: {} checks, {f} mandatory failures", v["result"]["checks"]);
                if f == 0 {
                    r.call("design_intersection", xin(sta));
                    xdone = true;
                    break;
                }
            }
            Err(e) => println!("intersection at {sta:.0}: {e}"),
        }
    }
    if !xdone {
        println!("no intersection location passed; last attempt kept for the engineer");
    }
    // maintenance yard pad beside the road: the agent tries stations and sides until the
    // grading daylights everywhere inside the surface
    let al = r.s.alignment.clone().unwrap();
    let mut pad_done = false;
    'pad: for k in 1..((end / 60.0) as usize) {
        for side in [1.0f64, -1.0] {
            let (sx, sy, sh) = al.point_at(60.0 * k as f64).unwrap();
            let (cx, cy) = (sx + side * 45.0 * sh.sin(), sy - side * 45.0 * sh.cos());
            let Some(z) = r.s.tin.as_ref().unwrap().0.elevation_at(cx, cy) else { continue };
            let z = z.round();
            let a = json!({"name":"yard","vertices":[[cx-20.0,cy-12.0,z],[cx+20.0,cy-12.0,z],[cx+20.0,cy+12.0,z],[cx-20.0,cy+12.0,z]],"closed":true,"cut_slope_h":2.0,"fill_slope_h":2.0,"spacing_m":1.0,"cell_m":0.25});
            match r.s.call("grade", &a) {
                Ok(t) if t["result"]["warnings"].as_array().map_or(false, |w| w.is_empty()) => {
                    let v = r.call("grade", a);
                    println!("pad grading at station {:.0} ({}), z {z} m: cut {:.0} m3, fill {:.0} m3", 60.0 * k as f64, if side > 0.0 { "right" } else { "left" }, v["result"]["cut_m3"].as_f64().unwrap(), v["result"]["fill_m3"].as_f64().unwrap());
                    pad_done = true;
                    break 'pad;
                }
                _ => {}
            }
        }
    }
    if !pad_done {
        println!("no pad location daylights inside the surface");
    }
    // storm drain behind the right curb, flowing to the low end
    let pr = r.s.profile.clone().unwrap();
    let falls = pr.elevation_at(0.0).unwrap() > pr.elevation_at(end).unwrap();
    let mut stations: Vec<f64> = (0..).map(|i| 30.0 + i as f64 * 90.0).take_while(|s| *s < end - 10.0).collect();
    if !falls {
        stations.reverse();
    }
    let mut structures = vec![];
    let mut pipes = vec![];
    for (i, s) in stations.iter().enumerate() {
        let (x, y, h) = al.point_at(*s).unwrap();
        structures.push(json!({"id": format!("SD{}", i + 1), "x": x + 5.0 * h.sin(), "y": y - 5.0 * h.cos(), "area_ha": 0.35, "c": 0.7}));
        if i > 0 {
            pipes.push(json!({"id": format!("P{}", i), "from": format!("SD{}", i), "to": format!("SD{}", i + 1), "diameter_m": 0.45, "n": 0.013, "material": "RCP"}));
        }
    }
    let crit = json!({"standard":"FHWA_HEC22","system":"storm","min_cover_m":0.9,"min_slope":0.005,"structure_drop_m":0.03,"intensity_mm_h":100.0,"freeboard_m":0.3});
    let v = r.call("design_pipe_network", json!({"name":"Storm","criteria":crit,"structures":structures,"pipes":pipes}));
    println!("storm network attempt 1 (all 450 mm): {} mandatory failures", v["result"]["mandatory_failures"]);
    // the agent upsizes the pipes at any structure whose HGL check fails (and any pipe short of capacity), one size at a time
    let sizes = [0.45, 0.525, 0.6, 0.675, 0.75, 0.9, 1.05, 1.2];
    for p in pipes.iter_mut() {
        p["max_slope"] = json!(0.08);
    }
    let mut crit = crit;
    let mut last = usize::MAX;
    for attempt in 2..14 {
        let v = r.call("design_pipe_network", json!({"name":"Storm","criteria":crit,"structures":structures,"pipes":pipes}));
        let fails: Vec<String> = r.s.all_checks().iter().filter(|c| !c.pass && c.severity == "mandatory" && c.id.starts_with("P-")).map(|c| c.element.clone()).collect();
        println!("storm network attempt {attempt}: {} pipes, {} mandatory failures", v["result"]["pipes"].as_array().unwrap().len(), v["result"]["mandatory_failures"]);
        if fails.is_empty() {
            break;
        }
        if fails.len() >= last {
            // resizing no longer helps: lower the whole network (more cover keeps the HGL below the rims)
            let c = crit["min_cover_m"].as_f64().unwrap() + 0.3;
            crit["min_cover_m"] = json!(c);
            println!("  resizing stalled; agent lowers the network: min cover {c:.1} m");
            last = usize::MAX;
            continue;
        }
        last = fails.len();
        for e in fails {
            let id = e.rsplit(' ').next().unwrap().to_string();
            for p in pipes.iter_mut() {
                if p["from"] == json!(id) || p["to"] == json!(id) || p["id"] == json!(id) {
                    let d = p["diameter_m"].as_f64().unwrap();
                    let next = sizes.iter().copied().find(|s| *s > d + 1e-9).unwrap_or(d);
                    p["diameter_m"] = json!(next);
                }
            }
        }
    }
    // sanitary sewer on the left side, 150 people per manhole, 380 L/person/day (100 gpcd); Ten States peaking
    let mut ss = vec![];
    let mut sp = vec![];
    let mut st2: Vec<f64> = (0..).map(|i| 20.0 + i as f64 * 80.0).take_while(|s| *s < end - 10.0).collect();
    if !falls {
        st2.reverse();
    }
    for (i, s) in st2.iter().enumerate() {
        let (x, y, h) = al.point_at(*s).unwrap();
        ss.push(json!({"id": format!("SS{}", i + 1), "x": x - 3.0 * h.sin(), "y": y + 3.0 * h.cos(), "population": 150.0, "per_capita_lpd": 380.0}));
        if i > 0 {
            sp.push(json!({"id": format!("S{}", i), "from": format!("SS{}", i), "to": format!("SS{}", i + 1), "diameter_m": 0.2, "n": 0.013, "material": "PVC", "max_slope": 0.08}));
        }
    }
    let local = json!({"min_velocity_mps":0.6,"max_depth_ratio":0.75,"min_diameter_m":0.2,"peaking":"ten_states","citation":"engineer criteria for the demo: Ten States Standards (2014) peaking (Figure 1) and 0.6 m/s minimum velocity; h/D ≤ 0.75 is agency practice (verify locally)"});
    let scrit = |ms: f64| json!({"standard":"ENGINEER","system":"sanitary","min_cover_m":1.2,"min_slope":ms,"structure_drop_m":0.03,"freeboard_m":0.3,"local":local});
    let v = r.call("design_pipe_network", json!({"name":"Sanitary","criteria":scrit(0.004),"structures":ss,"pipes":sp}));
    println!("sanitary attempt 1 (min slope 0.4%): {} mandatory failures", v["result"]["mandatory_failures"]);
    let mut fails = v["result"]["mandatory_failures"].as_u64().unwrap();
    let mut ms = 0.004;
    while fails > 0 && ms < 0.03 {
        ms += 0.004;
        let v = r.call("design_pipe_network", json!({"name":"Sanitary","criteria":scrit(ms),"structures":ss,"pipes":sp}));
        fails = v["result"]["mandatory_failures"].as_u64().unwrap();
        println!("sanitary attempt (min slope {:.1}%): {} pipes, {} mandatory failures", ms * 100.0, v["result"]["pipes"].as_array().unwrap().len(), fails);
    }
    // protection works
    let v = r.call("design_protection", json!({"clear_zone_m":5.0,"clear_zone_source":"engineer input from AASHTO Roadside Design Guide Table 3-1 for the design speed, ADT and slopes (demo value)",
        "recoverable_h":4.0,"traversable_h":3.0,"embankment_warrant_height_m":3.0,"barrier_offset_m":1.5,"runout_length_m":40.0,"barrier_type":"W-beam guardrail",
        "runoff_c":0.5,"intensity_mm_h":100.0,"catchment_width_m":20.0,"soil_tau_p":3.5,"soil_name":"silty sand, PI > 20 (HEC-15 Table 2.3)","riprap_sg":2.65}));
    let res = &v["result"];
    println!("protection: {} hazards, {} barrier runs ({:.0} m), {} ditch reaches ({}), mandatory failures {}", res["hazards"], res["barriers"].as_array().unwrap().len(), res["barriers"].as_array().unwrap().iter().map(|b| b["length_m"].as_f64().unwrap()).sum::<f64>(), res["ditches"].as_array().unwrap().len(), res["ditches"].as_array().unwrap().iter().map(|d| d["lining"].as_str().unwrap().to_string()).collect::<Vec<_>>().join("; "), res["mandatory_failures"]);
    let v = r.call("compute_quantities", json!({"cut_factor":0.9,"bulking_factor":1.25,"free_haul_m":150.0,"table_step_m":100.0}));
    println!("mass haul: final ordinate {:.0} m3, balance points {}, borrow {:.0} m3, waste {:.0} m3 (bank), overhaul {:.0} m3·m", v["result"]["final_ordinate_m3"].as_f64().unwrap(), v["result"]["balance_stations"], v["result"]["borrow_compacted_m3"].as_f64().unwrap(), v["result"]["waste_bank_m3"].as_f64().unwrap(), v["result"]["overhaul_m3m"].as_f64().unwrap());
    let c = r.call("check_standards", json!({}));
    println!("all checks: {} total, {} mandatory failures, {} advisory", c["result"]["total"], c["result"]["mandatory_failures"], c["result"]["advisory"]);
    r.call("export_landxml", json!({"file":"design.xml"}));
    let v = r.call("generate_drawings", json!({}));
    println!("drawings: {} sheets, status {}", v["result"]["pages"], v["result"]["status"]);
    let v = r.call("generate_deliverables", json!({"stage":"preliminary"}));
    println!("deliverables: {}", v["result"]["files"].as_object().unwrap().keys().cloned().collect::<Vec<_>>().join(", "));
    let v = r.call("generate_review_report", json!({}));
    println!("review report: {}", v["result"]["audit"]);
    println!("outputs in {out}/");
    r.s
}
