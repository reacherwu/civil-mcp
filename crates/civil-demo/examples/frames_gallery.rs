//! Renders the demo sheet set on each Five Eyes drafting profile (default frame) plus two
//! extra sizes, into <out>/frames/<tag>/ (sheets.pdf, sheet-NN.svg/.dxf, drawing-standards.json,
//! frame-preview.*). Used by scripts/render_readme_images.py --frames.
//! usage: cargo run --release -p civil-demo --example frames_gallery -- <out>
#[path = "../src/main.rs"]
#[allow(dead_code)]
mod demo;
use serde_json::json;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "demo-out".into());
    let base = format!("{out}/frames-run");
    let mut s = demo::run(&base);
    let cases = [
        (
            "US",
            "ANSI D",
            "landscape",
            json!({"client": "Demo County Public Works (fictitious)", "project_code": "DEMO", "contract_no": "Contract DEMO-001", "date": "2026-10-20", "logo_text": "OWNER LOGO"}),
        ),
        (
            "CA",
            "ISO A1",
            "landscape",
            json!({"client": "Demo Municipality (fictitious)", "project_code": "DEMO", "date": "2026-10-20", "logo_text": "OWNER LOGO"}),
        ),
        (
            "UK",
            "ISO A1",
            "landscape",
            json!({"client": "Demo Highway Authority (fictitious)", "project_code": "DEMO", "originator": "CMC", "date": "2026-10-20", "status_code": "S3", "logo_text": "OWNER LOGO"}),
        ),
        (
            "AU",
            "ISO A1",
            "landscape",
            json!({"client": "Demo Roads Authority (fictitious)", "project_code": "DEMO", "date": "2026-10-20", "logo_text": "OWNER LOGO"}),
        ),
        (
            "NZ",
            "ISO A1",
            "landscape",
            json!({"client": "Demo District Council (fictitious)", "project_code": "DEMO", "date": "2026-10-20", "logo_text": "OWNER LOGO"}),
        ),
        (
            "UK",
            "ISO A3",
            "landscape",
            json!({"client": "Demo Highway Authority (fictitious)", "project_code": "DEMO", "originator": "CMC", "date": "2026-10-20"}),
        ),
        ("AU", "ISO A4", "portrait", json!({"client": "Demo Roads Authority (fictitious)", "project_code": "DEMO", "date": "2026-10-20"})),
    ];
    println!("tag,profile,size,orientation,layout_factor,checks,mandatory_failures,advisory");
    for (pf, size, ori, fields) in cases {
        let tag = format!("{pf}-{}-{ori}", size.replace("ISO ", "").replace(' ', ""));
        s.call("set_sheet_frame", &json!({"frame": {"profile": pf, "size": size, "orientation": ori, "fields": fields}})).unwrap();
        let g = s.call("generate_drawings", &json!({})).unwrap();
        s.call("preview_sheet_frame", &json!({})).unwrap();
        let ds = &g["result"]["drawing_standards"];
        println!(
            "{tag},{pf},{size},{ori},{:.3},{},{},{}",
            ds["layout_factor"].as_f64().unwrap(),
            ds["checks"],
            ds["mandatory_failures"],
            ds["advisory"].as_array().unwrap().len()
        );
        assert_eq!(ds["mandatory_failures"], 0, "{tag}: {ds}");
        let d = format!("{out}/frames/{tag}");
        std::fs::create_dir_all(&d).unwrap();
        for f in std::fs::read_dir(&base).unwrap().flatten() {
            let n = f.file_name().to_string_lossy().to_string();
            if n.starts_with("sheet") || n.starts_with("frame-preview") || n == "drawing-standards.json" {
                std::fs::copy(f.path(), format!("{d}/{n}")).unwrap();
            }
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}
