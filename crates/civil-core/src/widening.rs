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

/// Engineer-entered widening table (e.g. a TAC, DMRB or Austroads table, which are not shipped).
/// Rows [radius_upper, widening]: the widening of the first row whose radius_upper ≥ R, sorted ascending;
/// R above every row: no widening. R below the first row is not handled by the table (NaN).
pub fn table(radius: f64, rows: &[[f64; 2]], citation: &str) -> WideningResult {
    let mut r = rows.to_vec();
    r.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
    let w = if r.is_empty() { f64::NAN } else if radius > r.last().unwrap()[0] { 0.0 } else { r.iter().find(|x| radius <= x[0]).map_or(f64::NAN, |x| x[1]) };
    WideningResult { radius, widening_m: w, method: "engineer-entered widening table".into(), citation: if citation.is_empty() { "engineer-entered table (no citation given)".into() } else { citation.into() } }
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
    fn table_lookup() {
        let t = [[100.0, 0.9], [200.0, 0.6], [300.0, 0.3]];
        assert_eq!(table(350.0, &t, "x").widening_m, 0.0);
        assert_eq!(table(250.0, &t, "x").widening_m, 0.3);
        assert_eq!(table(150.0, &t, "x").widening_m, 0.6);
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
