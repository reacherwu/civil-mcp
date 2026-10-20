//! v5 tools under the non-US profiles: engineer-input gates and the method chosen per profile.
use civil_core::tools::Session;
use serde_json::{json, Value};

fn session(std: &str) -> (Session, std::path::PathBuf) {
    let d = std::env::temp_dir().join(format!("civil-v5-{std}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut s = Session::default();
    s.call("open_project", &json!({"dir": d})).unwrap();
    s.call("set_design_basis", &json!({"actor": "engineer:E", "basis": {"project": "p", "engineer_of_record": "E", "standard": std, "design_speed_kmh": 60.0, "e_max": 0.07, "f_max": 0.17, "max_grade_pct": 8.0, "road_type": "single", "max_relative_gradient_pct": 0.6, "perception_reaction_s": 2.0, "deceleration_coeff": 0.36, "deceleration_mps2": 3.4, "eye_height_m": 1.1, "object_height_m": 0.38, "sag_k_min": 12.0}})).unwrap();
    s.call("create_alignment", &json!({"name": "a", "pis": [[0, 0], [400, 0], [700, 300]], "radii": [250.0]})).unwrap();
    s.call("create_profile", &json!({"name": "p", "vpis": [[0, 100], [300, 112], [650, 100]], "curve_lengths": [150]})).unwrap();
    (s, d)
}

fn idf() -> Value { json!({"label": "test", "points": [[5.0, 120.0], [60.0, 30.0]], "source": "test"}) }

#[test]
fn uk_needs_feh_peak_and_engineer_tapers() {
    let (mut s, _) = session("UK_DMRB");
    let e = s.call("design_hydrology", &json!({"idf": idf(), "catchments": [{"id": "C1", "area_ha": 3.0, "c": 0.4, "length_m": 300.0, "slope": 0.02}]})).unwrap_err();
    assert!(e.contains("feh_peak_m3s"), "{e}");
    let r = s.call("design_hydrology", &json!({"idf": idf(), "catchments": [{"id": "C1", "area_ha": 3.0, "c": 0.4, "length_m": 300.0, "slope": 0.02, "feh_peak_m3s": 0.35}]})).unwrap();
    assert_eq!(r["result"]["catchments"][0]["q_m3s"].as_f64().unwrap(), 0.35);
    assert_eq!(r["result"]["catchments"][0]["method"], "uk_check");
    // no taper formula is shipped for the UK: tapers stay blank until the engineer enters them
    let r = s.call("plan_traffic_management", &json!({"existing_speed_kmh": 60.0, "lane_w_m": 3.65})).unwrap();
    assert!(r["result"]["stages"][1]["taper_m"].is_null());
    let r = s.call("plan_traffic_management", &json!({"existing_speed_kmh": 60.0, "lane_w_m": 3.65, "taper_m": 50.0})).unwrap();
    assert_eq!(r["result"]["stages"][1]["taper_m"].as_f64().unwrap(), 50.0);
    assert!(s.call("design_marking_signing", &json!({"eye_h_m": 1.05, "min_zone_m": 100.0, "curve_warning_r_m": 300.0})).is_err());
    let r = s.call("design_marking_signing", &json!({"psd_m": 300.0, "eye_h_m": 1.05, "min_zone_m": 100.0, "curve_warning_r_m": 300.0})).unwrap();
    assert_eq!(r["result"]["centre_colour"], "white");
}

#[test]
fn au_uses_arr_rational_and_sisd_formula() {
    let (mut s, _) = session("AU_AUSTROADS");
    let r = s.call("design_hydrology", &json!({"idf": idf(), "c": 0.5, "catchments": [{"id": "C1", "area_ha": 10.0, "c": 0.5, "length_m": 1000.0, "slope": 0.01}]})).unwrap();
    let c = &r["result"]["catchments"][0];
    assert_eq!(c["method"], "arr_rational");
    // Bransby-Williams tc = 58 L / (A^0.1 S^0.2), L km, A km2, S m/km
    let tc = 58.0 * 1.0 / (0.1f64.powf(0.1) * 10f64.powf(0.2));
    assert!((c["tc_min"].as_f64().unwrap() - tc).abs() < 1e-9);
    assert!(s.call("export_rrrap_inputs", &json!({"aadt": 1000.0, "hgv_pct": 10.0})).is_err());
    let r = s.call("check_intersection_sight", &json!({"reaction_s": 2.0, "decel_coeff": 0.36, "eye_h_m": 1.1, "object_h_m": 1.25})).unwrap();
    let need = 2.0 * 60.0 / 3.6 + 60.0 * 60.0 / (254.0 * 0.36);
    assert!((r["result"]["required_m"].as_f64().unwrap() - need).abs() < 1e-9);
}

#[test]
fn ca_rational_and_time_gap_required() {
    let (mut s, _) = session("CA_TAC_2017");
    let r = s.call("design_hydrology", &json!({"idf": idf(), "catchments": [{"id": "C1", "area_ha": 2.0, "c": 0.6, "tc_min": 10.0}]})).unwrap();
    assert_eq!(r["result"]["catchments"][0]["method"], "rational");
    assert!(s.call("check_intersection_sight", &json!({"eye_h_m": 1.05, "object_h_m": 1.3})).is_err());
    let loc = s.call("locality_plan", &json!({"utm_zone": 17, "north": true})).unwrap();
    assert!((loc["result"]["start"]["lon"].as_f64().unwrap() + 85.4887).abs() < 0.01, "{}", loc["result"]["start"]);
}
