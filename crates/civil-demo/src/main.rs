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
        let fails: Vec<Value> = checks
            .iter()
            .filter(|c| !c.pass && c.severity == "mandatory")
            .map(|c| json!({"id": c.id, "element": c.element, "value": c.value, "limit": c.limit, "unit": c.unit, "citation": c.citation}))
            .collect();
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
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out).unwrap();
    let log = std::fs::File::create(format!("{out}/run-log.jsonl")).unwrap();
    let mut r = Run { s: Session::default(), log };
    r.call("open_project", json!({"dir": out}));
    // US profile. Values the profile cannot ship (f_max, max grade) are entered by the engineer.
    let v = r.call(
        "set_design_basis",
        json!({"actor":"engineer:REVIEWER-PLACEHOLDER","basis":{
        "project":"Demo rural collector (USGS 3DEP terrain, Palo Alto CA)","engineer_of_record":"REVIEWER-PLACEHOLDER",
        "standard":"US_AASHTO_2018","design_speed_kmh":50.0,"e_max":0.08,"f_max":0.18,"max_grade_pct":8.0,"normal_crown":0.02,"lane_width_m":3.5}}),
    );
    println!("design basis: US AASHTO 2018, 50 km/h, SSD {:.1} m; engineer inputs: {}", v["result"]["ssd_m"].as_f64().unwrap(), v["result"]["engineer_inputs"]);
    let v = r.call("load_points", json!({"path": data}));
    println!("points: {}", v["result"]["points"]);
    let v = r.call("build_surface", json!({}));
    println!("TIN: {} triangles", v["result"]["triangles"]);
    // pavement: rigid option first for comparison, then the adopted flexible design
    let traffic = json!({"aadt":1500.0,"hv_frac":0.08,"esa_per_hv":1.2,"growth":0.02,"years":20.0,"dir_factor":0.5,"lane_factor":1.0});
    let v = r.call(
        "design_pavement",
        json!({"kind":"rigid","subgrade_cbr":6.0,"traffic":traffic,"reliability_zr":-1.282,"s0":0.35,"p0":4.5,"pt":2.0,
        "sc_mpa":4.5,"ec_mpa":28000.0,"j":3.2,"cd":1.0,"k_mpa_m":40.0,"layers":[{"name":"Aggregate subbase","min_mm":150.0}]}),
    );
    println!(
        "pavement (rigid option): {}",
        v["result"]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| format!("{} {} mm", l["name"].as_str().unwrap(), l["thickness_mm"]))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let v = r.call("design_pavement", json!({"kind":"flexible","subgrade_cbr":6.0,"traffic":traffic,"reliability_zr":-1.282,"s0":0.45,"p0":4.2,"pt":2.0,
        "layers":[{"name":"Asphalt concrete","a":0.44,"m":1.0,"modulus_mpa":2758.0,"min_mm":75.0},{"name":"Aggregate base","a":0.14,"m":1.0,"modulus_mpa":207.0,"min_mm":150.0},{"name":"Aggregate subbase","a":0.11,"m":1.0,"modulus_mpa":103.0,"min_mm":150.0}]}));
    println!(
        "pavement (adopted flexible): W18 {:.3e}, SN {:.2} required / {:.2} provided; {}",
        v["traffic"]["design_traffic"].as_f64().unwrap(),
        v["result"]["sn_required"].as_f64().unwrap(),
        v["result"]["sn_provided"].as_f64().unwrap(),
        v["result"]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| format!("{} {} mm", l["name"].as_str().unwrap(), l["thickness_mm"]))
            .collect::<Vec<_>>()
            .join(", ")
    );
    r.call(
        "create_assembly",
        json!({"name":"2-lane rural collector","lanes_m":[3.5],"shoulder_m":1.2,"shoulder_slope":-0.04,"shoulder_paved":false,
        "ditch":{"foreslope_h":4.0,"depth_m":0.6,"bottom_width_m":0.6},"cut_slope_h":1.5,"fill_slope_h":2.0,
        "cut_bench":{"every_m":8.0,"width_m":2.0},"fill_bench":{"every_m":8.0,"width_m":2.0},
        // right side (frontage): curb, gutter and sidewalk instead of shoulder and ditch
        "right":{"curb":{"gutter_m":0.5,"gutter_slope":-0.06,"height_m":0.15,"top_m":0.15,"sidewalk_m":2.0,"sidewalk_slope":0.02},"remove":["ditch"]},
        // walls where the earthwork would leave the 30 m right-of-way or slopes exceed 12 m
        "wall":{"kind":"cantilever","max_daylight_offset_m":30.0,"max_slope_height_m":12.0,"offset_from_hinge_m":0.5,"embedment_m":0.8,
                "soil":{"gamma_kn_m3":19.0,"phi_deg":30.0,"base_friction":0.45,"bearing_kpa":250.0,"surcharge_kpa":10.0}}}),
    );
    // route alternatives over the terrain
    let v = r.call("generate_route_alternatives", json!({"start":[573520.0,4139660.0],"end":[574280.0,4140380.0],"cell_m":10.0,"radius_pref_m":200.0,"spiral_m":40.0,"simplify_tol_m":15.0,"vpi_spacing_m":150.0,"interval_m":20.0,"shrink":0.9}));
    for a in v["result"]["alternatives"].as_array().unwrap() {
        let e = &a["evaluation"];
        println!(
            "route {}: {} PIs, length {:.0} m, max grade {:.2}%, cut {:.0} fill {:.0} m3, wall concrete {:.0} m3, failures {}, cost proxy {:.0}",
            a["name"].as_str().unwrap(),
            a["pis"],
            e["length_m"].as_f64().unwrap_or(f64::NAN),
            e["max_design_grade_pct"].as_f64().unwrap_or(f64::NAN),
            e["cut_m3"].as_f64().unwrap_or(f64::NAN),
            e["fill_m3"].as_f64().unwrap_or(f64::NAN),
            e["wall_concrete_m3"].as_f64().unwrap_or(f64::NAN),
            e["geometry_mandatory_failures"],
            e["cost_proxy"].as_f64().unwrap_or(f64::NAN)
        );
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
    println!(
        "corridor: {} sections, cut {:.0} m3, fill {:.0} m3 (composite check cut {:+.2}%, fill {:+.2}%), warnings {}",
        res["sections"],
        res["cut_m3"].as_f64().unwrap(),
        res["fill_m3"].as_f64().unwrap(),
        res["composite_check"]["cut_diff_pct"].as_f64().unwrap(),
        res["composite_check"]["fill_diff_pct"].as_f64().unwrap(),
        res["sections_with_warnings"]
    );
    println!(
        "widening: {:?}; walls: {}",
        res["widening"].as_array().unwrap().iter().map(|w| (w["widening_m"].as_f64().unwrap() * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
        res["walls"]
    );
    // 4-leg crossroad, stop-controlled minor road; the agent tries candidate stations until the checks pass
    let minor = json!({"name":"minor road","lanes_m":[3.0],"shoulder_m":0.5,"shoulder_slope":-0.04,"shoulder_paved":false,"cut_slope_h":1.0,"fill_slope_h":1.5,
        "ditch":{"foreslope_h":3.0,"depth_m":0.4,"bottom_width_m":0.4},"pavement":[{"name":"Asphalt concrete","thickness_m":0.05},{"name":"Aggregate base","thickness_m":0.18}]});
    let xin = |sta: f64| {
        json!({"name":"X1","main_station": sta, "major_speed_kmh":50.0,"minor_speed_kmh":30.0,"min_return_radius_m":12.0,"max_approach_grade_pct":3.0,
        "legs":[{"name":"N leg","side":"left","angle_deg":90.0,"length_m":100.0,"grade_pct":-2.0,"return_radii_m":[15.0,15.0],"assembly":minor},
                {"name":"S leg","side":"right","angle_deg":90.0,"length_m":100.0,"grade_pct":2.0,"return_radii_m":[15.0,15.0],"assembly":minor}]})
    };
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
                Ok(t) if t["result"]["warnings"].as_array().is_some_and(|w| w.is_empty()) => {
                    let v = r.call("grade", a);
                    println!(
                        "pad grading at station {:.0} ({}), z {z} m: cut {:.0} m3, fill {:.0} m3",
                        60.0 * k as f64,
                        if side > 0.0 { "right" } else { "left" },
                        v["result"]["cut_m3"].as_f64().unwrap(),
                        v["result"]["fill_m3"].as_f64().unwrap()
                    );
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
            pipes.push(
                json!({"id": format!("P{}", i), "from": format!("SD{}", i), "to": format!("SD{}", i + 1), "diameter_m": 0.45, "n": 0.013, "material": "RCP"}),
            );
        }
    }
    let crit =
        json!({"standard":"FHWA_HEC22","system":"storm","min_cover_m":0.9,"min_slope":0.005,"structure_drop_m":0.03,"intensity_mm_h":100.0,"freeboard_m":0.3});
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
        let fails: Vec<String> =
            r.s.all_checks().iter().filter(|c| !c.pass && c.severity == "mandatory" && c.id.starts_with("P-")).map(|c| c.element.clone()).collect();
        println!(
            "storm network attempt {attempt}: {} pipes, {} mandatory failures",
            v["result"]["pipes"].as_array().unwrap().len(),
            v["result"]["mandatory_failures"]
        );
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
    let scrit =
        |ms: f64| json!({"standard":"ENGINEER","system":"sanitary","min_cover_m":1.2,"min_slope":ms,"structure_drop_m":0.03,"freeboard_m":0.3,"local":local});
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
    println!(
        "protection: {} hazards, {} barrier runs ({:.0} m), {} ditch reaches ({}), mandatory failures {}",
        res["hazards"],
        res["barriers"].as_array().unwrap().len(),
        res["barriers"].as_array().unwrap().iter().map(|b| b["length_m"].as_f64().unwrap()).sum::<f64>(),
        res["ditches"].as_array().unwrap().len(),
        res["ditches"].as_array().unwrap().iter().map(|d| d["lining"].as_str().unwrap().to_string()).collect::<Vec<_>>().join("; "),
        res["mandatory_failures"]
    );
    let v = r.call("compute_quantities", json!({"cut_factor":0.9,"bulking_factor":1.25,"free_haul_m":150.0,"table_step_m":100.0}));
    println!(
        "mass haul: final ordinate {:.0} m3, balance points {}, borrow {:.0} m3, waste {:.0} m3 (bank), overhaul {:.0} m3·m",
        v["result"]["final_ordinate_m3"].as_f64().unwrap(),
        v["result"]["balance_stations"],
        v["result"]["borrow_compacted_m3"].as_f64().unwrap(),
        v["result"]["waste_bank_m3"].as_f64().unwrap(),
        v["result"]["overhaul_m3m"].as_f64().unwrap()
    );
    // ---- v5: hydrology, culverts, geotechnical, subsurface drainage, traffic, land and utilities
    let demo = "DEMO VALUE - engineer to replace";
    let v = r.call("design_hydrology", json!({"idf": {"label": "10-yr illustrative IDF", "points": [[5.0, 86.0], [10.0, 62.0], [15.0, 51.0], [30.0, 35.0], [60.0, 24.0], [120.0, 16.0], [360.0, 8.8], [1440.0, 3.9]], "source": "DEMO VALUES - not NOAA Atlas 14; engineer to enter the PFDS table for the site"}, "method": "rational", "c": 0.35, "cell_m": 10.0, "min_area_ha": 2.0}));
    println!(
        "hydrology: {} crossings; Q = {}",
        v["result"]["crossings"].as_array().unwrap().len(),
        v["result"]["catchments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| format!("{}:{:.3}", c["id"].as_str().unwrap(), c["q_m3s"].as_f64().unwrap()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let v = r.call("design_culverts", json!({"inlet": "concrete_groove_headwall", "n": 0.012, "sizes_m": [0.45, 0.6, 0.75, 0.9, 1.05, 1.2, 1.5, 1.8, 2.1, 2.4], "hw_d_max": 1.2, "freeboard_m": 0.5, "min_cover_m": 0.3, "v_out_max_ms": 4.5, "tailwater_m": 0.0, "source": demo}));
    println!(
        "culverts: {}",
        v["result"]["culverts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| if c["hyd"].is_object() {
                format!(
                    "{} D{:.2} HW/D {:.2} {}",
                    c["id"].as_str().unwrap(),
                    c["hyd"]["d_m"].as_f64().unwrap(),
                    c["hyd"]["hw_m"].as_f64().unwrap() / c["hyd"]["d_m"].as_f64().unwrap(),
                    c["hyd"]["control"].as_str().unwrap()
                )
            } else {
                format!("{} in cut", c["id"].as_str().unwrap())
            })
            .collect::<Vec<_>>()
            .join("; ")
    );
    let v = r.call("check_slope_stability", json!({"soil": {"c_kpa": 8.0, "phi_deg": 30.0, "gamma_knm3": 19.0, "ru": 0.1, "source": demo,
        "layers": [{"name": "soft clay (DEMO)", "z_top": -0.5, "c_kpa": 6.0, "phi_deg": 20.0, "gamma_knm3": 17.5}]}, "layers_relative_to_toe": true, "fs_min": 1.3, "geotech_report_ref": "GEOTECH REPORT PLACEHOLDER (no site investigation in the demo)"}));
    println!(
        "slope stability: {}",
        v["result"]["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| format!(
                "{} {} {:.2}/{:.2}",
                s["kind"].as_str().unwrap(),
                s["station"].as_f64().unwrap() as i64,
                s["circle"]["fs_bishop"].as_f64().unwrap(),
                s["circle"]["fs_spencer"].as_f64().unwrap()
            ))
            .collect::<Vec<_>>()
            .join("; ")
    );
    let v = r.call("design_subsurface_drainage", json!({"crack_spacing_m": 12.0, "k_layer_m_day": 300.0, "layer_thickness_m": 0.1, "porosity": 0.25, "pipe_d_m": 0.1, "max_outlet_spacing_m": 150.0, "m_coefficient": 1.0, "t50_target_h": 24.0, "source": demo}));
    println!(
        "subsurface drainage: t50 {:.2} h, outlet spacing {:.0} m",
        v["result"]["result"]["t50_hours"].as_f64().unwrap(),
        v["result"]["result"]["outlet_spacing_m"].as_f64().unwrap()
    );
    let v = r.call("check_pavement_mechanistic", json!({"layers": [{"name": "asphalt", "h_m": 0.1, "e_mpa": 3000.0}, {"name": "base", "h_m": 0.3, "e_mpa": 300.0}], "subgrade_e_mpa": 50.0, "vb_pct": 11.0, "rf": 1.0, "design_esa": 1.0e6, "criteria": "agpt02", "dual_spacing_m": 0.33, "design_ac_max_m": 0.3}));
    println!(
        "mechanistic (layered elastic): asphalt {:.3} m, subgrade N {:.3e}, asphalt N {:.3e}",
        v["result"]["result"]["h_ac_m"].as_f64().unwrap(),
        v["result"]["result"]["n_sub"].as_f64().unwrap(),
        v["result"]["result"]["n_ac"].as_f64().unwrap()
    );
    // wall type selection: tall walls leave the cantilever range (FHWA GEC 11 / BS 8006 / AS 4678 / CSA S6)
    let v = r.call("select_wall_types", json!({"cantilever_max_h_m": 8.0, "gravity_max_h_m": 3.0, "reshape_max_dz_m": 1.0, "reinforced_gamma_knm3": 20.0, "reinforced_phi_deg": 34.0, "sv_m": 0.6, "mse_bearing_kpa": 400.0, "source": demo}));
    println!("wall types: {}", v["result"]["summary"]);
    // the agent steps the bar size (then bundles) until every wall fits; each attempt is logged
    for (bar, bundle) in [(16.0, 1.0), (20.0, 1.0), (25.0, 1.0), (32.0, 1.0), (32.0, 2.0)] {
        let v = r.call("design_wall_reinforcement", json!({"fc_mpa": 28.0, "fy_mpa": 420.0, "phi": 0.9, "load_factor": 1.5, "cover_m": 0.05, "bar_d_mm": bar, "bars_per_bundle": bundle, "min_ratio": 0.0018, "gamma_knm3": 19.0, "phi_soil_deg": 30.0, "surcharge_kpa": 10.0, "source": demo}));
        let fails = v["result"]["checks"].as_array().unwrap().iter().filter(|c| !c["pass"].as_bool().unwrap()).count();
        println!(
            "wall stem reinforcement: {} mm bars x{bundle}: {} sections, {fails} walls failing bar spacing",
            bar,
            v["result"]["sections"].as_array().unwrap().len()
        );
        if fails == 0 {
            break;
        }
    }
    let v = r.call("design_marking_signing", json!({"eye_h_m": 1.08, "min_zone_m": 150.0, "curve_warning_r_m": 300.0, "lateral_clear_m": 8.0}));
    println!("marking: {} no-passing zones, {} signs", v["result"]["no_passing"].as_array().unwrap().len(), v["result"]["signs"].as_array().unwrap().len());
    let v = r.call("design_lighting", json!({"lumens": 20000.0, "cu": 0.45, "llf": 0.8, "e_avg_lux": 9.0, "mounting_h_m": 12.0, "arrangement": "single_side", "max_spacing_to_height": 4.0, "source": demo}));
    println!("lighting: spacing {:.1} m, {} poles", v["result"]["spacing_m"].as_f64().unwrap(), v["result"]["poles"].as_array().unwrap().len());
    r.call("plan_traffic_management", json!({"existing_speed_kmh": 72.0, "lane_w_m": 3.6}));
    // synthetic utility records and parcels for the demo (no real records are used)
    let (utils, parcels) = {
        let al = r.s.alignment.as_ref().unwrap();
        let (s0, s1) = (al.start_station(), al.end_station());
        let mut u = vec![];
        for (k, (frac, kind, depth, dia)) in [(0.3, "water main", 1.2, 0.3), (0.55, "gas", 0.9, 0.15), (0.8, "telecom duct", 0.6, 0.1)].iter().enumerate() {
            let (x, y, h) = al.point_at(s0 + (s1 - s0) * frac).unwrap();
            let (nx, ny) = (-h.sin(), h.cos());
            let sk: f64 = 0.3;
            let (dx, dy) = (nx * sk.cos() + h.cos() * sk.sin(), ny * sk.cos() + h.sin() * sk.sin());
            u.push(json!({"id": format!("U{}", k + 1), "kind": kind, "owner": "DEMO UTILITY OWNER", "polyline": [[x - 120.0 * dx, y - 120.0 * dy], [x + 120.0 * dx, y + 120.0 * dy]], "depth_m": depth, "diameter_m": dia, "quality_level": "QL-D (synthetic demo record)"}));
        }
        let pts: Vec<[f64; 2]> = (0..=40).filter_map(|i| al.point_at(s0 + (s1 - s0) * i as f64 / 40.0).map(|p| [p.0, p.1])).collect();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &pts {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let (x0, y0, x1, y1) = (x0 - 80.0, y0 - 80.0, x1 + 80.0, y1 + 80.0);
        let (xm, ym) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let pc: Vec<Value> = [(x0, y0, xm, ym), (xm, y0, x1, ym), (x0, ym, xm, y1), (xm, ym, x1, y1)]
            .iter()
            .enumerate()
            .map(|(i, b)| json!({"id": format!("LOT-{}", i + 1), "owner": "DEMO PARCEL", "polygon": [[b.0, b.1], [b.2, b.1], [b.2, b.3], [b.0, b.3]]}))
            .collect();
        (u, pc)
    };
    let v = r.call("register_utilities", json!({"utilities": utils, "clearance_m": 0.3, "max_cover_m": 3.0, "source": demo}));
    println!("utilities: {} register rows", v["result"]["conflicts"].as_array().unwrap().len());
    let v = r.call("land_requirements", json!({"buffer_m": 3.0, "parcels": parcels, "cell_m": 1.0, "source": demo}));
    println!(
        "land: footprint {:.0} m2; takes {}",
        v["result"]["footprint_area_m2"].as_f64().unwrap(),
        v["result"]["takes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| format!("{} {:.0} m2", t["parcel"].as_str().unwrap(), t["take_m2"].as_f64().unwrap()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let v = r.call("locality_plan", json!({"utm_zone": 10, "north": true, "basemap_geojson": format!("{root}/../../data/basemap/osm-locality.geojson")}));
    println!("locality: start {:.6},{:.6}", v["result"]["start"]["lat"].as_f64().unwrap(), v["result"]["start"]["lon"].as_f64().unwrap());
    let v = r.call(
        "check_intersection_sight",
        json!({"time_gap_s": 7.5, "eye_h_m": 1.08, "object_h_m": 1.08, "source": "AASHTO 2018 Table 9-6 passenger car left turn from stop (verify)"}),
    );
    println!("intersection sight: required {:.1} m", v["result"]["required_m"].as_f64().unwrap());
    let v = r.call("plan_erosion_control", json!({"check_dam_h_m": 0.6, "fence_offset_m": 1.0, "source": demo}));
    println!("ESC: sediment fence {:.0} m, {} check dams", v["result"]["sediment_fence_m"].as_f64().unwrap(), v["result"]["check_dams"]);
    // v6: signals (warrant + concept timing), capacity, landscape, rigid pavement (AU/NZ method shown for comparison), constructability
    let v = r.call("design_signals", json!({
        "warrants": {"hours": [[640.0, 165.0], [720.0, 185.0], [580.0, 155.0], [560.0, 152.0], [600.0, 160.0], [650.0, 175.0], [700.0, 195.0], [660.0, 180.0], [480.0, 120.0], [420.0, 95.0]],
                     "major_lanes": 1, "minor_lanes": 1, "major_speed_kmh": 50.0, "isolated_community": false},
        "timing": {"approaches": [
            {"name": "EB", "major": true, "through_vph": 330.0, "left_vph": 40.0, "right_vph": 30.0, "lanes": 1, "speed_kmh": 50.0, "crossing_m": 12.0},
            {"name": "WB", "major": true, "through_vph": 300.0, "left_vph": 35.0, "right_vph": 35.0, "lanes": 1, "speed_kmh": 50.0, "crossing_m": 12.0},
            {"name": "NB", "major": false, "through_vph": 50.0, "left_vph": 70.0, "right_vph": 60.0, "lanes": 1, "speed_kmh": 30.0, "crossing_m": 14.0},
            {"name": "SB", "major": false, "through_vph": 40.0, "left_vph": 55.0, "right_vph": 50.0, "lanes": 1, "speed_kmh": 30.0, "crossing_m": 14.0}],
            "sat_flow_pcuhpl": 1800.0, "permissive_left_pcu": 2.0, "lost_per_phase_s": 4.0, "source": demo},
        "x_max": 0.9, "los_target": "C"}));
    println!(
        "signals: warrant met {} ({}), cycle {:.0} s, delay {:.1} s/veh, LOS {}",
        v["result"]["warrant"]["met"],
        v["result"]["warrant"]["detail"],
        v["result"]["timing"]["cycle_s"].as_f64().unwrap(),
        v["result"]["timing"]["delay_s"].as_f64().unwrap(),
        v["result"]["timing"]["los"]
    );
    let v = r.call("analyse_capacity", json!({
        "link": {"peak_hour_vph": 760.0, "direction_split": 0.55, "phf": 0.92, "heavy_pct": 6.0, "e_t": 2.0, "capacity_pcphpl": 1700.0, "lanes_per_direction": 1.0, "ffs_kmh": 60.0,
                 "los_vc": [[0.25, "A"], [0.4, "B"], [0.6, "C"], [0.8, "D"], [1.0, "E"]]},
        "priority": {"movements": [
            {"name": "minor left (NB)", "volume_vph": 70.0, "conflicting_vph": 700.0, "tc_s": 7.1, "tf_s": 3.5},
            {"name": "minor right (NB)", "volume_vph": 60.0, "conflicting_vph": 330.0, "tc_s": 6.2, "tf_s": 3.3},
            {"name": "major left (EB)", "volume_vph": 40.0, "conflicting_vph": 335.0, "tc_s": 4.1, "tf_s": 2.2}]},
        "source": demo}));
    println!("capacity: link v/c {:.2}, LOS {}", v["result"]["link"]["v_c"].as_f64().unwrap(), v["result"]["link"]["los"]);
    let v = r.call("landscape_concept", json!({"tree_spacing_m": 12.0, "tree_offset_m": 11.0, "clear_zone_m": 3.0, "side": "R", "intersection_clear_m": 40.0, "pole_clear_m": 3.0, "shrub_density_per_m2": 1.0, "topsoil_mm": 100.0, "source": demo}));
    println!("landscape: {} trees, areas {}", v["result"]["trees"], v["result"]["areas_m2"]);
    let v = r.call(
        "design_rigid_pavement",
        json!({"base_type": "JRCP", "groups": [
            {"name": "SAST 53 kN", "load_kn": 53.0, "tyres": 2.0, "repetitions": 2.0e5},
            {"name": "SADT 80 kN", "load_kn": 80.0, "tyres": 4.0, "repetitions": 1.5e6},
            {"name": "TADT 135 kN", "load_kn": 135.0, "tyres": 8.0, "repetitions": 6.0e5},
            {"name": "TRDT 181 kN", "load_kn": 181.0, "tyres": 12.0, "repetitions": 3.0e5}],
        "k_mpa_m": 70.0, "fcf_mpa": 4.5, "e_mpa": 30000.0, "nu": 0.15, "lsf": 1.2, "tyre_kpa": 750.0, "tied_shoulder": true, "source": demo}),
    );
    println!(
        "rigid (AGPT02 structure, comparison): base {:.0} mm, fatigue {:.1} %, erosion {:.1} %",
        v["result"]["result"]["h_mm"].as_f64().unwrap(),
        v["result"]["result"]["fatigue_pct"].as_f64().unwrap(),
        v["result"]["result"]["erosion_pct"].as_f64().unwrap()
    );
    let v = r.call("design_foundation", json!({"subgrade_cbr_pct": 4.0, "layers": [{"name": "granular subbase", "e_mpa": 150.0, "h_m": 0.15}], "h_max_m": 0.6, "target_e0_mpa": 100.0, "plate_radius_m": 0.15, "plate_kpa": 100.0, "source": demo}));
    println!(
        "foundation: subbase {:.2} m, E0 {:.0} MPa (target {})",
        v["result"]["layers"][0]["h_m"].as_f64().unwrap(),
        v["result"]["e0_mpa"].as_f64().unwrap(),
        v["result"]["target"]
    );
    let v = r.call("constructability_report", json!({"max_fill_h_m": 10.0, "max_grade_paving_pct": 6.0, "mse_hold_h_m": 10.0, "source": demo}));
    println!("constructability: {} stages, {} risks", v["result"]["sequence"].as_array().unwrap().len(), v["result"]["risks"].as_array().unwrap().len());
    r.call("cost_plan", json!({"contingency_pct": 30.0, "risks": [{"name": "unknown utilities (demo)", "p50": null, "p90": null}]}));
    r.call("specialist_handoffs", json!({}));
    let c = r.call("check_standards", json!({}));
    println!("all checks: {} total, {} mandatory failures, {} advisory", c["result"]["total"], c["result"]["mandatory_failures"], c["result"]["advisory"]);
    r.call("export_landxml", json!({"file":"design.xml"}));
    // sheet frame: US drafting profile defaults (ANSI D, NCS layers, UDS sheet ids) + title-block content
    r.call(
        "set_sheet_frame",
        json!({"frame": {"profile": "US", "size": "ANSI D", "fields": {
            "client": "Demo County Public Works (fictitious)", "contract_no": "Contract DEMO-001", "project_code": "DEMO",
            "logo_text": "OWNER LOGO",
            "date": "2026-10-20", "revisions": [{"rev": "A", "date": "2026-10-20", "description": "Preliminary design for review", "by": "AGT"}]}}}),
    );
    let v = r.call("generate_drawings", json!({}));
    println!(
        "drawings: {} sheets, status {}; drawing standards ({} {}): {} checks, {} mandatory failures",
        v["result"]["pages"],
        v["result"]["status"],
        v["result"]["drawing_standards"]["profile"],
        v["result"]["drawing_standards"]["size"],
        v["result"]["drawing_standards"]["checks"],
        v["result"]["drawing_standards"]["mandatory_failures"]
    );
    let v = r.call("generate_deliverables", json!({"stage":"preliminary"}));
    println!("deliverables: {}", v["result"]["files"].as_object().unwrap().keys().cloned().collect::<Vec<_>>().join(", "));
    let v = r.call("generate_review_report", json!({}));
    println!("review report: {}", v["result"]["audit"]);
    println!("outputs in {out}/");
    r.s
}
