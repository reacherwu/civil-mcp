//! MCP stdio server (JSON-RPC 2.0, newline-delimited) exposing civil-core tools.
use civil_core::tools::{tool_list, Session};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn main() {
    let mut s = Session::default();
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--project") {
        if let Some(d) = args.get(i + 1) {
            if let Err(e) = s.call("open_project", &json!({"dir": d})) {
                eprintln!("open_project failed: {e}");
            }
        }
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(out, "{}", json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":e.to_string()}}));
                continue;
            }
        };
        let id = req.get("id").cloned();
        let method = req["method"].as_str().unwrap_or("");
        let result = match method {
            "initialize" => Some(Ok(
                json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"civil-mcp","version":env!("CARGO_PKG_VERSION")}}),
            )),
            "tools/list" => Some(Ok(json!({"tools": tool_list()}))),
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let a = req["params"].get("arguments").cloned().unwrap_or(json!({}));
                Some(Ok(match s.call(name, &a) {
                    Ok(v) => json!({"content":[{"type":"text","text": serde_json::to_string_pretty(&v).unwrap()}],"structuredContent": v,"isError": false}),
                    Err(e) => json!({"content":[{"type":"text","text": e}],"isError": true}),
                }))
            }
            "ping" => Some(Ok(json!({}))),
            m if m.starts_with("notifications/") => None,
            _ => Some(Err(json!({"code":-32601,"message":format!("method not found: {method}")}))),
        };
        if let (Some(id), Some(r)) = (id, result) {
            let msg = match r {
                Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}),
                Err(e) => json!({"jsonrpc":"2.0","id":id,"error":e}),
            };
            let _ = writeln!(out, "{msg}");
            let _ = out.flush();
        }
    }
}
