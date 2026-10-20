//! Regression test of the full demo pipeline + engineer approval rules.
#[path = "../src/main.rs"]
#[allow(dead_code)]
mod demo;
use serde_json::{json, Value};

#[test]
fn full_pipeline_and_approval_rules() {
    let out = std::env::temp_dir().join(format!("civil-demo-test-{}", std::process::id()));
    let out = out.to_str().unwrap().to_string();
    let mut s = demo::run(&out);
    let log: Vec<Value> = std::fs::read_to_string(format!("{out}/run-log.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    // the 40 m spiral attempt is caught by a superelevation check, the 50 m one is not
    let first_check = log.iter().find(|e| e["tool"] == "check_standards").unwrap();
    assert!(first_check["mandatory_failures"].as_array().unwrap().iter().any(|f| f["id"].as_str().unwrap().starts_with("SE-")), "{first_check}");
    let pipes: Vec<&Value> = log.iter().filter(|e| e["tool"] == "design_pipe_network").collect();
    assert!(pipes[0]["result"]["mandatory_failures"].as_u64().unwrap() > 0);
    assert_eq!(pipes[1]["result"]["mandatory_failures"], 0);
    let last = log.iter().rev().find(|e| e["tool"] == "check_standards").unwrap();
    assert_eq!(last["result"]["mandatory_failures"], 0);
    // regression values (USGS 3DEP data, measured 2026-10-20); 0.1 % tolerance
    let corr = log.iter().rev().find(|e| e["tool"] == "build_corridor").unwrap();
    let near = |a: f64, b: f64| (a - b).abs() / b < 1e-3;
    assert!(near(corr["result"]["cut_m3"].as_f64().unwrap(), 66180.0), "{}", corr["result"]["cut_m3"]);
    assert!(near(corr["result"]["fill_m3"].as_f64().unwrap(), 92474.0), "{}", corr["result"]["fill_m3"]);
    assert!(corr["result"]["composite_check"]["cut_diff_pct"].as_f64().unwrap().abs() < 1.0);
    // approval: agent cannot approve; engineer can once all mandatory checks pass
    assert!(s.call("record_review", &json!({"actor":"agent","decision":"approve","note":"x"})).is_err());
    s.call("record_review", &json!({"actor":"engineer:TEST","decision":"approve","note":"test approval"})).unwrap();
    let st = s.call("get_state", &json!({})).unwrap();
    assert!(st.to_string().contains("approved"), "{st}");
    let v = s.call("generate_drawings", &json!({})).unwrap();
    assert!(!v["result"]["status"].as_str().unwrap().contains("PRELIMINARY"), "{v}");
    // any later design change voids approval → drawings go back to PRELIMINARY
    s.call("build_corridor", &json!({"interval_m": 25.0})).unwrap();
    let v = s.call("generate_drawings", &json!({})).unwrap();
    assert!(v["result"]["status"].as_str().unwrap().contains("PRELIMINARY"), "{v}");
    // the audit chain still verifies after all of this
    let v = s.call("verify_audit_log", &json!({})).unwrap();
    assert_eq!(v["result"]["ok"], true, "{v}");
    let _ = std::fs::remove_dir_all(&out);
}
