//! Culvert hydraulics per FHWA HDS-5 (3rd ed., 2012): inlet control (unsubmerged form 1 /
//! submerged equations, SI Ku = 1.811, transition by linear interpolation between
//! Ku·Q/(A·D^0.5) = 3.5 and 4.0) and outlet control (full-flow energy, tailwater or (dc+D)/2).
//! Coefficients K, M, c, Y transcribed from HDS-5 Appendix A Table A.1 (verify).
use serde::{Deserialize, Serialize};

pub const HDS5: &str = "FHWA HDS-5 Hydraulic Design of Highway Culverts, 3rd ed. (2012), Ch. 3 (inlet control Eq 3.1–3.3, outlet control Eq 3.4–3.8), App. A Table A.1";
const G: f64 = 9.80665;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Inlet {
    #[serde(rename = "concrete_square_headwall")]
    ConcreteSquareHeadwall,
    #[serde(rename = "concrete_groove_headwall")]
    ConcreteGrooveHeadwall,
    #[serde(rename = "concrete_groove_projecting")]
    ConcreteGrooveProjecting,
    #[serde(rename = "cmp_headwall")]
    CmpHeadwall,
    #[serde(rename = "cmp_mitered")]
    CmpMitered,
    #[serde(rename = "cmp_projecting")]
    CmpProjecting,
}

impl Inlet {
    /// (K, M, c, Y, ke)
    pub fn coef(self) -> (f64, f64, f64, f64, f64) {
        match self {
            Inlet::ConcreteSquareHeadwall => (0.0098, 2.0, 0.0398, 0.67, 0.5),
            Inlet::ConcreteGrooveHeadwall => (0.0018, 2.0, 0.0292, 0.74, 0.2),
            Inlet::ConcreteGrooveProjecting => (0.0045, 2.0, 0.0317, 0.69, 0.2),
            Inlet::CmpHeadwall => (0.0078, 2.0, 0.0379, 0.69, 0.5),
            Inlet::CmpMitered => (0.0210, 1.33, 0.0463, 0.75, 0.7),
            Inlet::CmpProjecting => (0.0340, 1.50, 0.0553, 0.54, 0.9),
        }
    }
    pub fn mitered(self) -> bool {
        self == Inlet::CmpMitered
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CulvertHyd {
    pub d_m: f64,
    pub q_m3s: f64,
    pub hw_inlet_m: f64,
    pub hw_outlet_m: f64,
    pub hw_m: f64,
    pub control: String,
    pub v_out_ms: f64,
    pub dc_m: f64,
    pub steps: Vec<String>,
}

fn circ(d: f64, y: f64) -> (f64, f64, f64) {
    let r = (y / d).clamp(1e-9, 1.0 - 1e-12);
    let th = 2.0 * (1.0 - 2.0 * r).acos();
    let a = d * d / 8.0 * (th - th.sin());
    let p = d * th / 2.0;
    (a, d * (th / 2.0).sin(), p)
}

pub fn critical_depth(d: f64, q: f64) -> f64 {
    let (mut lo, mut hi) = (1e-6 * d, 0.999 * d);
    for _ in 0..100 {
        let m = 0.5 * (lo + hi);
        let (a, t, _) = circ(d, m);
        if a * a * a / t < q * q / G { lo = m } else { hi = m }
    }
    0.5 * (lo + hi)
}

/// Inlet-control headwater (m).
pub fn hw_inlet(inlet: Inlet, d: f64, q: f64, s: f64) -> (f64, String) {
    let (k, m, c, y, _) = inlet.coef();
    let a = std::f64::consts::PI * d * d / 4.0;
    let ks = if inlet.mitered() { 0.7 } else { -0.5 };
    let x = 1.811 * q / (a * d.sqrt());
    let dc = critical_depth(d, q);
    let (ac, _, _) = circ(d, dc);
    let hc = dc + (q / ac).powi(2) / (2.0 * G);
    let un = hc / d + k * x.powf(m) + ks * s;
    let sub = c * x * x + y + ks * s;
    let r = if x <= 3.5 { un } else if x >= 4.0 { sub } else { un + (sub - un) * (x - 3.5) / 0.5 };
    (r * d, format!("Ku·Q/(A·D^0.5) = {x:.3}; unsubmerged HW/D = Hc/D + K·x^M + Ks·S = {un:.3}; submerged HW/D = c·x² + Y + Ks·S = {sub:.3}; used {r:.3}"))
}

/// Outlet-control headwater (m), full-barrel energy equation.
pub fn hw_outlet(inlet: Inlet, d: f64, q: f64, n: f64, l: f64, s: f64, tw: f64) -> (f64, String) {
    let (_, _, _, _, ke) = inlet.coef();
    let a = std::f64::consts::PI * d * d / 4.0;
    let r = d / 4.0;
    let v = q / a;
    let h = (1.0 + ke + 2.0 * G * n * n * l / r.powf(4.0 / 3.0)) * v * v / (2.0 * G);
    let dc = critical_depth(d, q).min(d);
    let ho = tw.max((dc + d) / 2.0);
    let hw = h + ho - l * s;
    (hw, format!("H = (1 + ke + 2g·n²·L/R^(4/3))·V²/2g = {h:.3} m (ke {ke}); ho = max(TW, (dc+D)/2) = {ho:.3} m; HWo = H + ho − L·S = {hw:.3} m"))
}

pub fn analyse(inlet: Inlet, d: f64, q: f64, n: f64, l: f64, s: f64, tw: f64) -> CulvertHyd {
    let (hi, si) = hw_inlet(inlet, d, q, s);
    let (ho, so) = hw_outlet(inlet, d, q, n, l, s, tw);
    let dc = critical_depth(d, q);
    // outlet velocity: normal depth (Manning) if not flowing full, else full
    let a = std::f64::consts::PI * d * d / 4.0;
    let qf = a * (d / 4.0f64).powf(2.0 / 3.0) * s.max(1e-5).sqrt() / n;
    let v = if q >= qf { q / a } else {
        let (mut lo, mut hi2) = (1e-6 * d, 0.938 * d);
        for _ in 0..100 { let m = 0.5 * (lo + hi2); let (am, _, pm) = circ(d, m); if am * (am / pm).powf(2.0 / 3.0) * s.max(1e-5).sqrt() / n < q { lo = m } else { hi2 = m } }
        let (am, _, _) = circ(d, 0.5 * (lo + hi2)); q / am
    };
    CulvertHyd { d_m: d, q_m3s: q, hw_inlet_m: hi, hw_outlet_m: ho, hw_m: hi.max(ho), control: if hi >= ho { "inlet".into() } else { "outlet".into() }, v_out_ms: v, dc_m: dc, steps: vec![si, so] }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inlet_control_monotone_and_outlet_energy() {
        let a = hw_inlet(Inlet::ConcreteSquareHeadwall, 0.9, 0.5, 0.01).0;
        let b = hw_inlet(Inlet::ConcreteSquareHeadwall, 0.9, 1.5, 0.01).0;
        assert!(b > a && a > 0.0);
        // outlet: zero-length, TW governs: HW = (1+ke)V²/2g + TW
        let (h, _) = hw_outlet(Inlet::ConcreteSquareHeadwall, 1.0, 1.0, 0.012, 0.0, 0.0, 2.0);
        let v = 1.0 / (std::f64::consts::PI / 4.0);
        assert!((h - (1.5 * v * v / (2.0 * G) + 2.0)).abs() < 1e-9);
    }
}
