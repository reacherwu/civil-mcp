//! Print measured differences between our evaluation of LandXML files and the
//! values the producing application wrote into them.
//! `cargo run --release -p civil-core --example landxml_compare -- file.xml ...`
use civil_core::landxml::{read, to_alignment};
use civil_core::profile::Profile;
use civil_core::tin::Tin;

fn main() {
    for f in std::env::args().skip(1) {
        let d = read(&std::fs::read_to_string(&f).unwrap()).unwrap();
        println!("== {f} ({})", d.application.clone().unwrap_or_default());
        for s in &d.surfaces {
            let (t, st) = Tin::from_faces(&s.points, &s.faces).unwrap();
            let _ = t;
            let pct = |a: f64, b: Option<f64>| {
                b.map(|b| format!("{:.6} (file {:.6}, diff {:+.2e} m², {:+.2e}%)", a, b, a - b, (a - b) / b * 100.0)).unwrap_or(format!("{a:.6} (file: none)"))
            };
            println!("surface {}: {} pts, {} faces ({} invisible skipped)", s.name, st.points, st.triangles, s.invisible_faces);
            println!("  area2d {}", pct(st.area_m2, s.reported_area2d));
            println!("  area3d {}", pct(st.area3d_m2, s.reported_area3d));
            println!("  elev min {:.4} (file {:?}) max {:.4} (file {:?})", st.min_z, s.reported_elev_min, st.max_z, s.reported_elev_max);
        }
        for a in &d.alignments {
            match to_alignment(a) {
                Ok(al) => {
                    println!(
                        "alignment {}: {} elements, length {:.6} (file {:?}, diff {:+.2e} m), max element-end deviation {:.2e} m, closure {:.2e} m",
                        a.name,
                        al.elements.len(),
                        al.length,
                        a.reported_length,
                        al.length - a.reported_length.unwrap_or(f64::NAN),
                        a.max_end_deviation,
                        al.closure_error
                    );
                    for w in a.warnings.iter().take(5) {
                        println!("  warn: {w}");
                    }
                }
                Err(e) => println!("alignment {}: {e}", a.name),
            }
            for p in &a.profiles {
                match Profile::build(p.clone()) {
                    Ok(pr) => println!(
                        "  profile {}: {} VPIs, grades {:?}",
                        p.name,
                        p.vpis.len(),
                        pr.grades_pct.iter().map(|g| (g * 1000.0).round() / 1000.0).collect::<Vec<_>>()
                    ),
                    Err(e) => println!("  profile {}: {e}", p.name),
                }
            }
        }
    }
}
