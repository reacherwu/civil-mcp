//! Limit-equilibrium slope stability for corridor cut/fill slopes on circular slip surfaces:
//! Bishop simplified (moment equilibrium) and Spencer (force + moment, constant interslice
//! inclination). Homogeneous Mohr–Coulomb soil with pore-pressure ratio ru; parameters are
//! engineer inputs from the geotechnical report. Critical circle by grid search of centres
//! and radii. Not a substitute for the geotechnical designer's analysis (layered soils,
//! groundwater surfaces, non-circular surfaces, seismic, reinforcement are not modelled).
use serde::{Deserialize, Serialize};

pub const BISHOP: &str = "Bishop (1955) simplified method of slices; Spencer (1967) method; e.g. FHWA-NHI-16-072 Soils and Foundations Ref. Manual Ch. 6";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soil {
    pub c_kpa: f64,
    pub phi_deg: f64,
    pub gamma_knm3: f64,
    #[serde(default)]
    pub ru: f64,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Circle {
    pub xc: f64,
    pub zc: f64,
    pub r: f64,
    pub fs_bishop: f64,
    pub fs_spencer: f64,
    pub theta_deg: f64,
    pub x_entry: f64,
    pub x_exit: f64,
    pub slices: usize,
}

/// surface polyline z(x), x increasing
fn zs(p: &[[f64; 2]], x: f64) -> f64 {
    if x <= p[0][0] { return p[0][1]; }
    for w in p.windows(2) {
        if x <= w[1][0] {
            let t = if w[1][0] > w[0][0] { (x - w[0][0]) / (w[1][0] - w[0][0]) } else { 0.0 };
            return w[0][1] + t * (w[1][1] - w[0][1]);
        }
    }
    p[p.len() - 1][1]
}

struct Sl { b: f64, w: f64, alpha: f64, u: f64, xm: f64 }

fn slices(p: &[[f64; 2]], xc: f64, zc: f64, r: f64, n: usize) -> Option<(Vec<Sl>, f64, f64)> {
    // circle bottom z = zc - sqrt(r²-(x-xc)²); find where it is below the surface
    let (x0, x1) = (p[0][0], p[p.len() - 1][0]);
    let m = 2000;
    let mut inside = vec![];
    for k in 0..=m {
        let x = x0 + (x1 - x0) * k as f64 / m as f64;
        let dx = x - xc;
        if dx.abs() < r && zc - (r * r - dx * dx).sqrt() < zs(p, x) { inside.push(x); }
    }
    if inside.len() < 10 { return None; }
    let (a, b) = (inside[0], *inside.last().unwrap());
    // a contiguous slip mass only, not touching the section ends
    if a <= x0 + 1e-6 || b >= x1 - 1e-6 || (inside.len() as f64) < (b - a) / (x1 - x0) * m as f64 * 0.98 { return None; }
    let bw = (b - a) / n as f64;
    let mut v = vec![];
    for i in 0..n {
        let xm = a + (i as f64 + 0.5) * bw;
        let dx = xm - xc;
        if dx.abs() >= r { return None; }
        let zb = zc - (r * r - dx * dx).sqrt();
        let h = zs(p, xm) - zb;
        if h <= 0.0 { continue; }
        let alpha = (dx / r).asin();
        v.push(Sl { b: bw, w: 0.0 * h + h * bw, alpha, u: 0.0, xm });
    }
    Some((v, a, b))
}

fn fs_bishop(sl: &[Sl], s: &Soil) -> f64 {
    let tp = s.phi_deg.to_radians().tan();
    let drive: f64 = sl.iter().map(|q| q.w * s.gamma_knm3 * q.alpha.sin()).sum();
    if drive <= 1e-9 { return f64::INFINITY; }
    let mut f = 1.5;
    for _ in 0..200 {
        let res: f64 = sl.iter().map(|q| {
            let w = q.w * s.gamma_knm3;
            let u = s.ru * w / q.b;
            let ma = q.alpha.cos() * (1.0 + q.alpha.tan() * tp / f);
            (s.c_kpa * q.b + (w - u * q.b) * tp) / ma
        }).sum();
        let nf = res / drive;
        if (nf - f).abs() < 1e-10 { return nf; }
        f = nf;
    }
    f
}

/// Spencer: interslice resultant Q at constant inclination θ; Ff from ΣQ = 0, Fm from
/// Σ Q·cos(α−θ) = 0 (moments about the circle centre); θ found where Ff = Fm.
fn spencer(sl: &[Sl], s: &Soil) -> (f64, f64) {
    let tp = s.phi_deg.to_radians().tan();
    let q = |f: f64, th: f64, q_: &Sl| {
        let w = q_.w * s.gamma_knm3;
        let u = s.ru * w / q_.b;
        let sec = 1.0 / q_.alpha.cos();
        let num = s.c_kpa * q_.b * sec / f + (w * q_.alpha.cos() - u * q_.b * sec) * tp / f - w * q_.alpha.sin();
        let den = (q_.alpha - th).cos() * (1.0 + (q_.alpha - th).tan() * tp / f);
        num / den
    };
    let solve = |th: f64, moment: bool| {
        let g = |f: f64| sl.iter().map(|x| q(f, th, x) * if moment { (x.alpha - th).cos() } else { 1.0 }).sum::<f64>();
        // scan upward from a low F for the first admissible sign change (all mα > 0.2)
        let ok = |f: f64| sl.iter().all(|x| (x.alpha - th).cos() * (1.0 + (x.alpha - th).tan() * tp / f) > 0.2);
        let mut br = None;
        let mut pf = 0.2;
        while pf < 10.0 {
            let nf = pf + 0.02;
            if ok(pf) && ok(nf) && g(pf).signum() != g(nf).signum() { br = Some((pf, nf)); break; }
            pf = nf;
        }
        let Some((mut lo, mut hi)) = br else { return f64::NAN };
        for _ in 0..200 { let m = 0.5 * (lo + hi); if g(m).signum() == g(lo).signum() { lo = m } else { hi = m } }
        0.5 * (lo + hi)
    };
    let d = |th: f64| solve(th, false) - solve(th, true);
    let (mut lo, mut hi) = (-0.2, 0.8);
    // scan for a sign change
    let mut prev = (lo, d(lo));
    let mut found = None;
    for k in 1..=50 {
        let th = lo + (hi - lo) * k as f64 / 50.0;
        let v = d(th);
        if v.is_finite() && prev.1.is_finite() && v.signum() != prev.1.signum() { found = Some((prev.0, th)); break; }
        prev = (th, v);
    }
    let Some((a, b)) = found else { return (fs_bishop(sl, s), 0.0) };
    lo = a; hi = b;
    for _ in 0..80 { let m = 0.5 * (lo + hi); if d(m).signum() == d(lo).signum() { lo = m } else { hi = m } }
    let th = 0.5 * (lo + hi);
    (solve(th, true), th)
}

/// Grid search over centres above the slope and radii; returns the critical circle (min Bishop FS).
pub fn critical(p: &[[f64; 2]], s: &Soil, toe_x: f64, crest_x: f64) -> Option<Circle> {
    let w = (crest_x - toe_x).abs().max(2.0);
    let zt = zs(p, toe_x).min(zs(p, crest_x));
    let zc_top = zs(p, toe_x).max(zs(p, crest_x));
    let mut best: Option<Circle> = None;
    let n = 30;
    for i in 0..=12 {
        for j in 0..=12 {
            let xc = toe_x.min(crest_x) - 0.5 * w + 2.0 * w * i as f64 / 12.0;
            let zc = zc_top + 0.2 * w + 2.0 * w * j as f64 / 12.0;
            for k in 1..=15 {
                // radii so the circle bottom lies between 0.2 m above and 0.5·w below the toe level
                let r = (zc - zt) + (-0.2 + (0.5 * w + 0.2) * k as f64 / 15.0);
                if r <= 0.5 { continue; }
                let Some((sl, a, b)) = slices(p, xc, zc, r, n) else { continue };
                if sl.len() < n / 2 || (b - a) < 0.5 * w { continue; }
                let f = fs_bishop(&sl, s);
                if f.is_finite() && best.as_ref().map_or(true, |c| f < c.fs_bishop) {
                    best = Some(Circle { xc, zc, r, fs_bishop: f, fs_spencer: f64::NAN, theta_deg: 0.0, x_entry: a, x_exit: b, slices: sl.len() });
                }
            }
        }
    }
    if let Some(c) = best.as_mut() {
        let (sl, _, _) = slices(p, c.xc, c.zc, c.r, n).unwrap();
        let (fsp, th) = spencer(&sl, s);
        c.fs_spencer = fsp;
        c.theta_deg = th.to_degrees();
        let _ = sl.iter().map(|q| q.xm + q.u).count();
    }
    best
}

pub fn fs_for(p: &[[f64; 2]], s: &Soil, xc: f64, zc: f64, r: f64, n: usize) -> Option<(f64, f64)> {
    let (sl, _, _) = slices(p, xc, zc, r, n)?;
    Some((fs_bishop(&sl, s), spencer(&sl, s).0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frictional_infinite_slope_limit_and_spencer_close_to_bishop() {
        // 2H:1V slope, 10 m high, c = 10 kPa, φ = 30°, γ = 19
        let p = vec![[-30.0, 0.0], [0.0, 0.0], [20.0, 10.0], [50.0, 10.0]];
        let s = Soil { c_kpa: 10.0, phi_deg: 30.0, gamma_knm3: 19.0, ru: 0.0, source: String::new() };
        let c = critical(&p, &s, 0.0, 20.0).unwrap();
        // c=0 lower bound: tanφ/tanβ = 0.577/0.5 = 1.155; with cohesion FS must be higher
        assert!(c.fs_bishop > 1.155 && c.fs_bishop < 2.5, "{}", c.fs_bishop);
        assert!((c.fs_spencer - c.fs_bishop).abs() / c.fs_bishop < 0.05, "{} {}", c.fs_spencer, c.fs_bishop);
    }
}
