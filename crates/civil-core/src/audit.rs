//! Append-only, hash-chained JSONL audit log. Each entry records who/what made
//! a decision, the exact inputs, a summary of outputs and the rules cited.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub fn sha256_hex(b: &[u8]) -> String {
    let d = Sha256::digest(b);
    d.iter().map(|x| format!("{x:02x}")).collect()
}

pub struct AuditLog {
    path: PathBuf,
    pub seq: u64,
    pub head: String,
}

impl AuditLog {
    pub fn open(path: &Path) -> crate::Result<AuditLog> {
        if path.exists() {
            let (n, head) = verify(path)?;
            Ok(AuditLog { path: path.into(), seq: n, head })
        } else {
            Ok(AuditLog { path: path.into(), seq: 0, head: GENESIS.into() })
        }
    }

    /// actor: "agent" or "engineer:<name>".
    pub fn append(&mut self, actor: &str, tool: &str, inputs: &Value, outputs: &Value, rules: &[String]) -> crate::Result<Value> {
        let seq = self.seq + 1;
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut e = json!({
            "seq": seq, "unix_time": ts, "actor": actor, "tool": tool,
            "tool_version": env!("CARGO_PKG_VERSION"),
            "inputs": inputs, "inputs_sha256": sha256_hex(inputs.to_string().as_bytes()),
            "outputs": outputs, "rules": rules, "prev_hash": self.head,
        });
        let h = sha256_hex(e.to_string().as_bytes());
        e["hash"] = Value::String(h.clone());
        let mut f = OpenOptions::new().create(true).append(true).open(&self.path).map_err(|e| e.to_string())?;
        writeln!(f, "{}", e).map_err(|e| e.to_string())?;
        self.seq = seq;
        self.head = h.clone();
        Ok(json!({"audit_seq": seq, "entry_hash": h, "inputs_sha256": e["inputs_sha256"], "tool_version": env!("CARGO_PKG_VERSION"), "rules": rules}))
    }
}

/// Recompute the chain. Returns (entries, head hash) or the first broken line.
pub fn verify(path: &Path) -> crate::Result<(u64, String)> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut prev = GENESIS.to_string();
    let mut n = 0;
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut v: Value = serde_json::from_str(line).map_err(|e| format!("line {}: {e}", i + 1))?;
        let h = v.get("hash").and_then(|h| h.as_str()).ok_or(format!("line {}: no hash", i + 1))?.to_string();
        if v["prev_hash"] != Value::String(prev.clone()) {
            return Err(format!("line {}: prev_hash does not match previous entry", i + 1));
        }
        v.as_object_mut().unwrap().remove("hash");
        if sha256_hex(v.to_string().as_bytes()) != h {
            return Err(format!("line {}: content was modified (hash mismatch)", i + 1));
        }
        if v["seq"].as_u64() != Some(n + 1) {
            return Err(format!("line {}: sequence gap", i + 1));
        }
        prev = h;
        n += 1;
    }
    Ok((n, prev))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tamper_is_detected() {
        let p = std::env::temp_dir().join(format!("audit-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let mut l = AuditLog::open(&p).unwrap();
        l.append("agent", "a", &json!({"x":1}), &json!({"y":2}), &["r".into()]).unwrap();
        l.append("agent", "b", &json!({"x":3}), &json!({}), &[]).unwrap();
        assert_eq!(verify(&p).unwrap().0, 2);
        let t = std::fs::read_to_string(&p).unwrap().replace("\"x\":3", "\"x\":4");
        std::fs::write(&p, t).unwrap();
        assert!(verify(&p).unwrap_err().contains("line 2"));
        let _ = std::fs::remove_file(&p);
    }
}
