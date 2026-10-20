//! Prints the parameter register status counts per standards profile (used for the README coverage matrix).
//! Starts from a minimal basis and adds a placeholder engineer value only for each input the profile refuses
//! to default (copyrighted / unverified tables), so verified defaults are kept wherever the profile ships them.
use civil_core::standards::{resolve, DesignBasis};
fn main() {
    let fill = [
        ("perception_reaction_s", 2.0),
        ("deceleration_coeff", 0.36),
        ("deceleration_mps2", 3.4),
        ("eye_height_m", 1.1),
        ("object_height_m", 0.38),
        ("sag_k_min", 12.0),
        ("max_relative_gradient_pct", 0.6),
    ];
    println!("profile,verified,formula,engineer,parameters,engineer_entered");
    for s in ["US_AASHTO_2018", "CA_TAC_2017", "UK_DMRB", "AU_AUSTROADS", "NZ_AUSTROADS_NZTA"] {
        let mut v = serde_json::json!({"project": "p", "engineer_of_record": "E", "standard": s, "design_speed_kmh": 60.0, "e_max": 0.07, "f_max": 0.17, "max_grade_pct": 8.0, "road_type": "single"});
        let mut added = vec![];
        let r = loop {
            let b: DesignBasis = serde_json::from_value(v.clone()).unwrap();
            match resolve(&b) {
                Ok(r) => break r,
                Err(e) => {
                    let (k, x) = fill.iter().find(|(k, _)| e.contains(k) && !added.contains(k)).unwrap_or_else(|| panic!("{e}"));
                    v[*k] = serde_json::json!(x);
                    added.push(*k);
                }
            }
        };
        let c = |k: &str| r.params.iter().filter(|p| p.status.to_string() == k).count();
        println!("{s},{},{},{},{},{}", c("verified"), c("formula"), c("engineer"), r.params.len(), added.join(" "));
        for p in &r.params {
            eprintln!("{s}|{}|{}|{}", p.name, p.status, p.source);
        }
    }
}
