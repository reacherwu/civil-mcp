//! Optional DWG output through ODA File Converter (an external, separately licensed program that
//! is not bundled). The converter is found from the `ODA_FILE_CONVERTER` environment variable or
//! `ODAFileConverter` on PATH; without a display it is run under `xvfb-run` when available.

use std::path::{Path, PathBuf};
use std::process::Command;

/// AutoCAD release names accepted by ODA File Converter, with the DWG version string.
pub const VERSIONS: &[(&str, &str)] = &[("ACAD2004", "AC1018"), ("ACAD2007", "AC1021"), ("ACAD2010", "AC1024"), ("ACAD2013", "AC1027"), ("ACAD2018", "AC1032")];

pub fn find_converter() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ODA_FILE_CONVERTER") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("ODAFileConverter")).find(|p| p.exists())
}

/// Convert `files` (names inside `dir`) to DWG `version` in `dir/out_sub`. Returns the DWG paths.
pub fn convert(dir: &Path, files: &[String], version: &str, out_sub: &str) -> Result<Vec<PathBuf>, String> {
    if !VERSIONS.iter().any(|v| v.0 == version) {
        return Err(format!("unknown DWG version {version}; use one of {:?}", VERSIONS.iter().map(|v| v.0).collect::<Vec<_>>()));
    }
    let oda = find_converter().ok_or("ODA File Converter not found: set ODA_FILE_CONVERTER or put ODAFileConverter on PATH (free download: https://www.opendesign.com/guestfiles/oda_file_converter)")?;
    let stage = dir.join(".dwg-stage");
    let out = dir.join(out_sub);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    for f in files {
        std::fs::copy(dir.join(f), stage.join(f)).map_err(|e| format!("{f}: {e}"))?;
    }
    let args =
        [stage.to_string_lossy().to_string(), out.to_string_lossy().to_string(), version.to_string(), "DWG".into(), "0".into(), "1".into(), "*.DXF".into()];
    let headless = std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none();
    let xvfb = std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join("xvfb-run")).find(|p| p.exists()));
    let st = match (headless, xvfb) {
        (true, Some(x)) => Command::new(x).arg("-a").arg(&oda).args(&args).output(),
        _ => Command::new(&oda).args(&args).output(),
    }
    .map_err(|e| format!("running {}: {e}", oda.display()))?;
    let _ = std::fs::remove_dir_all(&stage);
    if !st.status.success() {
        return Err(format!("ODA File Converter failed: {}", String::from_utf8_lossy(&st.stderr)));
    }
    let mut res = vec![];
    for f in files {
        let stem = Path::new(f).file_stem().unwrap().to_string_lossy().to_string();
        let p = out.join(format!("{stem}.dwg"));
        let p = if p.exists() { p } else { out.join(format!("{stem}.DWG")) };
        if !p.exists() {
            return Err(format!("ODA File Converter wrote no DWG for {f}"));
        }
        res.push(p);
    }
    Ok(res)
}
