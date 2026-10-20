//! Lane widening on horizontal curves.
use serde::{Deserialize, Serialize};

/// AASHTO design vehicle for the off-tracking formula.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vehicle {
    pub name: String,
    /// track width u (m)
    pub track_u_m: f64,
    /// wheelbases L_i (m)
    pub wheelbases_m: Vec<f64>,
    /// front overhang A (m)
    pub front_overhang_m: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WideningResult {
    pub radius: f64,
    pub widening_m: f64,
    pub method: String,
    pub citation: String,
}

/// JTG D20-2017 Table 7.6.1, two-lane widening (m) by radius band; vehicle class 1/2/3.
/// Transcribed — verify against the code. Radius > 250 m: no widening.
pub fn jtg(radius: f64, class: u8) -> WideningResult {
    let bands = [250.0, 200.0, 150.0, 100.0, 70.0, 50.0, 30.0, 25.0, 20.0, 15.0];
    let rows: [[f64; 9]; 3] = [[0.4, 0.6, 0.8, 1.0, 1.2, 1.4, 1.8, 2.2, 2.5], [0.6, 0.7, 0.9, 1.2, 1.5, 2.0, f64::NAN, f64::NAN, f64::NAN], [0.8, 1.0, 1.5, 2.0, 2.5, f64::NAN, f64::NAN, f64::NAN, f64::NAN]];
    let row = &rows[(class.clamp(1, 3) - 1) as usize];
    let mut w = 0.0;
    if radius <= bands[0] {
        w = f64::NAN;
        for i in 0..9 {
            if radius <= bands[i] && radius > bands[i + 1] - 1e-9 {
                w = row[i];
                break;
            }
        }
    }
    WideningResult { radius, widening_m: w, method: format!("JTG table, two lanes, vehicle class {class}"), citation: "JTG D20-2017 公路路线设计规范 §7.6 平曲线加宽 (表 7.6.1)".into() }
}

/// AASHTO Green Book 2018 §3.3.10 (metric): w_c = N(U + C) + (N − 1)F_A + Z,
/// U = u + R − √(R² − ΣL²), F_A = √(R² + A(2L + A)) − R, Z = 0.1·V/√R; widening = w_c − w_n.
pub fn aashto(radius: f64, speed_kmh: f64, lanes: usize, lane_width: f64, v: &Vehicle) -> WideningResult {
    let n = lanes as f64;
    let wn = n * lane_width;
    let c = if wn <= 6.0 { 0.6 } else if wn <= 6.6 { 0.75 } else { 0.9 };
    let sl2: f64 = v.wheelbases_m.iter().map(|l| l * l).sum();
    let u = v.track_u_m + radius - (radius * radius - sl2).sqrt();
    let l1 = v.wheelbases_m.first().copied().unwrap_or(0.0);
    let fa = (radius * radius + v.front_overhang_m * (2.0 * l1 + v.front_overhang_m)).sqrt() - radius;
    let z = 0.1 * speed_kmh / radius.sqrt();
    let wc = n * (u + c) + (n - 1.0) * fa + z;
    WideningResult { radius, widening_m: (wc - wn).max(0.0), method: format!("AASHTO off-tracking, {} (u={}, L={:?}, A={}), C={c}, w_n={wn}", v.name, v.track_u_m, v.wheelbases_m, v.front_overhang_m), citation: "AASHTO A Policy on Geometric Design of Highways and Streets 2018, §3.3.10 Traveled-way widening on horizontal curves".into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jtg_bands() {
        assert_eq!(jtg(300.0, 1).widening_m, 0.0);
        assert_eq!(jtg(150.0, 1).widening_m, 0.6);
        assert_eq!(jtg(120.0, 1).widening_m, 0.8);
        assert_eq!(jtg(60.0, 2).widening_m, 1.5);
    }
    #[test]
    fn aashto_hand_calc() {
        // SU: u=2.44, L=6.1, A=1.2; R=100, V=50, 2 lanes × 3.3 (w_n=6.6 → C=0.75)
        let v = Vehicle { name: "SU-9".into(), track_u_m: 2.44, wheelbases_m: vec![6.1], front_overhang_m: 1.2 };
        let r = aashto(100.0, 50.0, 2, 3.3, &v);
        let u = 2.44 + 100.0 - (10000.0f64 - 37.21).sqrt();
        let fa = (10000.0f64 + 1.2 * (12.2 + 1.2)).sqrt() - 100.0;
        let wc = 2.0 * (u + 0.75) + fa + 0.1 * 50.0 / 10.0;
        assert!((r.widening_m - (wc - 6.6)).abs() < 1e-12);
        assert!((u - 2.6263).abs() < 1e-3, "{u}");
    }
}
