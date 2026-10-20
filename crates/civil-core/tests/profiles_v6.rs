//! v6 tools under the non-US profiles: signal warrant method per jurisdiction, UK foundation classes,
//! AU rigid pavement scope, capacity limits.
use civil_core::tools::Session;
use serde_json::{json, Value};

fn session(std: &str) -> Session {
    let d = std::env::temp_dir().join(format!("civil-v6-{std}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut s = Session::default();
    s.call("open_project", &json!({"dir": d})).unwrap();
    s.call("set_design_basis", &json!({"actor": "engineer:E", "basis": {"project": "p", "engineer_of_record": "E", "standard": std, "design_speed_kmh": 60.0, "e_max": 0.07, "f_max": 0.17, "max_grade_pct": 8.0, "road_type": "single", "max_relative_gradient_pct": 0.6, "perception_reaction_s": 2.0, "deceleration_coeff": 0.36, "deceleration_mps2": 3.4, "eye_height_m": 1.1, "object_height_m": 0.38, "sag_k_min": 12.0}})).unwrap();
    s
}

fn signals(local: Value, tac: Value) -> Value {
    json!({"warrants": {"hours": vec![[700.0, 210.0]; 8], "major_lanes": 1, "minor_lanes": 1, "major_speed_kmh": 60.0, "local": local, "tac_points": tac},
           "timing": {"approaches": [
               {"name": "EB", "major": true, "through_vph": 300.0, "left_vph": 30.0, "right_vph": 30.0, "lanes": 1, "speed_kmh": 60.0, "crossing_m": 12.0},
               {"name": "WB", "major": true, "through_vph": 300.0, "left_vph": 30.0, "right_vph": 30.0, "lanes": 1, "speed_kmh": 60.0, "crossing_m": 12.0},
               {"name": "NB", "major": false, "through_vph": 60.0, "left_vph": 60.0, "right_vph": 60.0, "lanes": 1, "speed_kmh": 40.0, "crossing_m": 14.0}],
               "sat_flow_pcuhpl": 1900.0}})
}

#[test]
fn uk_signals_need_local_thresholds_and_foundation_uses_cd225_classes() {
    let mut s = session("UK_DMRB");
    let r = s.call("design_signals", &signals(Value::Null, Value::Null)).unwrap();
    assert!(r["result"]["warrant"]["method"].as_str().unwrap().starts_with("no national numeric warrant"));
    let r =
        s.call("design_signals", &signals(json!({"major_vph": 600.0, "minor_vph": 200.0, "hours_required": 4, "source": "test table"}), Value::Null)).unwrap();
    assert_eq!(r["result"]["warrant"]["met"], true);
    let r = s
        .call(
            "design_foundation",
            &json!({"subgrade_cbr_pct": 3.0, "layers": [{"name": "subbase", "e_mpa": 150.0, "h_m": 0.15}], "target_class": 2, "h_max_m": 0.8}),
        )
        .unwrap();
    assert_eq!(r["result"]["target_e0_mpa"].as_f64().unwrap(), 100.0);
    assert!(r["result"]["e0_mpa"].as_f64().unwrap() >= 100.0);
}

#[test]
fn ca_signals_use_tac_points() {
    let mut s = session("CA_TAC_2017");
    let r = s.call("design_signals", &signals(Value::Null, json!(85.0))).unwrap();
    assert!(r["result"]["warrant"]["method"].as_str().unwrap().contains("TAC"));
    assert_eq!(r["result"]["warrant"]["met"], false);
}

#[test]
fn au_rigid_in_scope_and_dos_limit() {
    let mut s = session("AU_AUSTROADS");
    let r = s
        .call("design_rigid_pavement", &json!({"base_type": "PCP", "groups": [{"name": "SADT", "load_kn": 80.0, "tyres": 4.0, "repetitions": 1e6}], "k_mpa_m": 70.0, "tied_shoulder": true, "fcf_mpa": 4.5, "lsf": 1.2}))
        .unwrap();
    assert!(r["result"]["scope"].as_str().unwrap().starts_with("AU/NZ project"));
    assert!(r["result"]["result"]["pass"].as_bool().unwrap());
    // UK profile: foundation class mapping is UK-only
    let e = s
        .call("design_foundation", &json!({"subgrade_cbr_pct": 3.0, "layers": [{"name": "subbase", "e_mpa": 150.0, "h_m": 0.15}], "target_class": 2}))
        .unwrap_err();
    assert!(e.contains("target_e0_mpa"), "{e}");
}
