//! Compare pipe geometry recomputed by civil-mcp from a Civil 3D LandXML pipe-network
//! export against the values Civil 3D wrote into the same file.
//! usage: cargo run --release -p civil-core --example c3d_pipes -- <file.xml>...
use civil_core::landxml;

fn stats(v: &mut [f64]) -> String {
    if v.is_empty() {
        return "n=0".into();
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    let mean = v.iter().sum::<f64>() / n as f64;
    format!("n={n} max|d|={:.3e} median={:.3e} mean={:.3e}", v.iter().fold(0.0f64, |a, x| a.max(x.abs())), v[n / 2], mean)
}

fn main() {
    for f in std::env::args().skip(1) {
        let t = std::fs::read_to_string(&f).unwrap();
        let d = landxml::read(&t).unwrap();
        println!("== {f}\n   application: {:?}", d.application);
        for n in &d.pipe_networks {
            let s = |name: &str| n.structs.iter().find(|s| s.name == name);
            let (mut d2, mut d3, mut ds, mut dh, mut hgl_rel) = (vec![], vec![], vec![], vec![], vec![]);
            let mut used = 0;
            let mut curved = 0;
            for p in &n.pipes {
                let (Some(a), Some(b)) = (s(&p.from), s(&p.to)) else { continue };
                let inv = |st: &landxml::StructData, dir: &str| st.inverts.iter().find(|i| i.2 == p.name && i.1 == dir).map(|i| i.0);
                let (Some(iu), Some(id)) = (inv(a, "out"), inv(b, "in")) else { continue };
                used += 1;
                let chord = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                // curved pipe: horizontal length of the circular arc start → <Center> point → end.
                // Interpreted as the arc mid-point (sagitta s = dist(mid, chord midpoint), R = (c²/4 + s²)/2s).
                let l2 = match p.arc_center {
                    Some(m) => {
                        let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                        let s = ((m[0] - mx).powi(2) + (m[1] - my).powi(2)).sqrt();
                        let r = (chord * chord / 4.0 + s * s) / (2.0 * s);
                        let half = (chord / (2.0 * r)).min(1.0).asin();
                        2.0 * r * if s > r { std::f64::consts::PI - half } else { half }
                    }
                    None => chord,
                };
                if p.arc_center.is_some() {
                    curved += 1;
                }
                let l3 = (l2 * l2 + (iu - id).powi(2)).sqrt();
                if let Some(rl) = p.reported_length {
                    d2.push(l2 - rl);
                    d3.push(l3 - rl);
                }
                if let Some(rs) = p.reported_slope {
                    // Civil 3D reports slope as positive downhill or signed by flow; compare magnitudes
                    ds.push((iu - id).abs() / l2 - rs.abs());
                }
                if let (Some(hu), Some(hd), Some(dm)) = (p.hgl_up, p.hgl_down, p.diameter_m) {
                    dh.push((hu - iu) - (hd - id));
                    hgl_rel.push(((hu - iu) / dm * 1000.0).round() / 1000.0);
                }
            }
            let flows: Vec<f64> = n.pipes.iter().filter_map(|p| p.flow_in).collect();
            println!(
                "   network '{}' type {}: {} structures, {} pipes, {} with both end inverts ({} curved)",
                n.name,
                n.net_type,
                n.structs.len(),
                n.pipes.len(),
                used,
                curved
            );
            println!("   length: ours 2D centre-centre − C3D length: {}", stats(&mut d2));
            println!("   length: ours 3D centre-centre − C3D length: {}", stats(&mut d3));
            println!("   slope : ours |Δinv|/L2D − C3D |slope|:      {}", stats(&mut ds));
            println!("   HGL   : C3D (hglUp−invUp) − (hglDn−invDn):   {}", stats(&mut dh));
            hgl_rel.dedup();
            println!(
                "   flowIn values in file: {:?}; distinct (hglUp−invUp)/D (first 8): {:?}",
                {
                    let mut f = flows.clone();
                    f.dedup();
                    f.truncate(5);
                    f
                },
                &hgl_rel[..hgl_rel.len().min(8)]
            );
        }
    }
}
