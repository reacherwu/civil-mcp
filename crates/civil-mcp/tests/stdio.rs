use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn mcp_stdio_roundtrip() {
    let dir = std::env::temp_dir().join(format!("civil-mcp-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut child = Command::new(env!("CARGO_BIN_EXE_civil-mcp")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut id = 0;
    let mut rpc = |method: &str, params: Value| -> Value {
        id += 1;
        writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})).unwrap();
        let mut l = String::new();
        out.read_line(&mut l).unwrap();
        serde_json::from_str(&l).unwrap()
    };
    let r = rpc("initialize", json!({}));
    assert_eq!(r["result"]["serverInfo"]["name"], "civil-mcp");
    let r = rpc("tools/list", json!({}));
    assert!(r["result"]["tools"].as_array().unwrap().len() >= 14);
    let call = |rpc: &mut dyn FnMut(&str, Value) -> Value, n: &str, a: Value| rpc("tools/call", json!({"name":n,"arguments":a}))["result"].clone();
    assert_eq!(call(&mut rpc, "open_project", json!({"dir": dir}))["isError"], false);
    // agent may not set the design basis
    let r = call(
        &mut rpc,
        "set_design_basis",
        json!({"actor":"agent","basis":{"project":"p","engineer_of_record":"E","standard":"UK_DMRB","design_speed_kmh":60.0,"e_max":0.07,"road_type":"single","max_relative_gradient_pct":0.6}}),
    );
    assert_eq!(r["isError"], true);
    // design tools refuse to run before the engineer sets a basis
    let r = call(&mut rpc, "create_alignment", json!({"name":"a","pis":[[0,0],[100,0]],"radii":[]}));
    assert_eq!(r["isError"], true);
    let r = call(
        &mut rpc,
        "set_design_basis",
        json!({"actor":"engineer:E","basis":{"project":"p","engineer_of_record":"E","standard":"UK_DMRB","design_speed_kmh":60.0,"e_max":0.07,"road_type":"single","max_relative_gradient_pct":0.6}}),
    );
    assert_eq!(r["isError"], false);
    assert_eq!(r["structuredContent"]["provenance"]["audit_seq"], 1);
    let r = call(&mut rpc, "create_alignment", json!({"name":"a","pis":[[0,0],[300,0],[300,300]],"radii":[100.0]}));
    assert_eq!(r["isError"], false);
    let r = call(&mut rpc, "check_standards", json!({}));
    // R = 100 m < 127 m (CD 109 Table 2.10, 60 kph, two steps below desirable) must fail and cite CD 109
    let checks = r["structuredContent"]["result"]["checks"].as_array().unwrap().clone();
    assert!(checks.iter().any(|c| c["id"] == "H-RMIN" && c["pass"] == false && c["citation"].as_str().unwrap().contains("CD 109")));
    // and below the desirable minimum radius (255 m): recorded as a relaxation (advisory)
    assert!(checks.iter().any(|c| c["id"] == "H-RDES" && c["pass"] == false && c["severity"] == "advisory"));
    let r = call(&mut rpc, "record_review", json!({"actor":"engineer:E","decision":"approve","note":"x"}));
    assert_eq!(r["isError"], true, "approval must be refused while checks fail");
    let r = call(&mut rpc, "verify_audit_log", json!({}));
    assert_eq!(r["structuredContent"]["result"]["ok"], true);
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
