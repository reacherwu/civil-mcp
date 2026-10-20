//! Limit-equilibrium slope stability for corridor cut/fill slopes on circular slip surfaces:
//! Bishop simplified (moment equilibrium) and Spencer (force + moment, constant interslice
//! inclination). Mohr–Coulomb soil in horizontal strata (multi-layer) or homogeneous, with pore-pressure ratio ru; parameters are
//! engineer inputs from the geotechnical report. Critical circle by grid search of centres
//! and radii. Not a substitute for the geotechnical designer's analysis (sloping
//! strata, groundwater surfaces, non-circular surfaces, seismic, reinforcement are not modelled).
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
    /// Optional horizontal strata below the upper material (multi-layer analysis). A point at
    /// elevation z takes the layer with the lowest `z_top` that is ≥ z; above every `z_top` the
    /// top-level c/φ/γ apply (e.g. embankment fill or the upper stratum).
    #[serde(default)]
    pub layers: Vec<SoilLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilLayer {
    pub name: String,
    pub z_top: f64,
    pub c_kpa: f64,
    pub phi_deg: f64,
    pub gamma_knm3: f64,
}

impl Soil {
    /// (c, φ°, γ) at elevation z
    pub fn at(&self, z: f64) -> (f64, f64, f64) {
        let mut best: Option<&SoilLayer> = None;
        for l in &self.layers {
            if z <= l.z_top && best.is_none_or(|b| l.z_top < b.z_top) {
                best = Some(l);
            }
        }
        best.map_or((self.c_kpa, self.phi_deg, self.gamma_knm3), |l| (l.c_kpa, l.phi_deg, l.gamma_knm3))
    }
    /// column weight per unit width between zb and zt (kN/m²), integrating γ through the strata
    pub fn column(&self, zb: f64, zt: f64) -> f64 {
        let mut cuts: Vec<f64> = self.layers.iter().map(|l| l.z_top).filter(|&z| z > zb && z < zt).collect();
        cuts.push(zb);
        cuts.push(zt);
        cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        cuts.windows(2).map(|w| self.at(0.5 * (w[0] + w[1])).2 * (w[1] - w[0])).sum()
    }
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
    if x <= p[0][0] {
        return p[0][1];
    }
    for w in p.windows(2) {
        if x <= w[1][0] {
            let t = if w[1][0] > w[0][0] { (x - w[0][0]) / (w[1][0] - w[0][0]) } else { 0.0 };
            return w[0][1] + t * (w[1][1] - w[0][1]);
        }
    }
    p[p.len() - 1][1]
}

/// slice: width, weight (kN/m), base inclination, base pore pressure (kPa), base c and tanφ
struct Sl {
    b: f64,
    w: f64,
    alpha: f64,
    u: f64,
    c: f64,
    tp: f64,
}

fn slices(p: &[[f64; 2]], s: &Soil, xc: f64, zc: f64, r: f64, n: usize) -> Option<(Vec<Sl>, f64, f64)> {
    // circle bottom z = zc - sqrt(r²-(x-xc)²); find where it is below the surface
    let (x0, x1) = (p[0][0], p[p.len() - 1][0]);
    let m = 2000;
    let mut inside = vec![];
    for k in 0..=m {
        let x = x0 + (x1 - x0) * k as f64 / m as f64;
        let dx = x - xc;
        if dx.abs() < r && zc - (r * r - dx * dx).sqrt() < zs(p, x) {
            inside.push(x);
        }
    }
    if inside.len() < 10 {
        return None;
    }
    let (a, b) = (inside[0], *inside.last().unwrap());
    // a contiguous slip mass only, not touching the section ends
    if a <= x0 + 1e-6 || b >= x1 - 1e-6 || (inside.len() as f64) < (b - a) / (x1 - x0) * m as f64 * 0.98 {
        return None;
    }
    let bw = (b - a) / n as f64;
    let mut v = vec![];
    for i in 0..n {
        let xm = a + (i as f64 + 0.5) * bw;
        let dx = xm - xc;
        if dx.abs() >= r {
            return None;
        }
        let zb = zc - (r * r - dx * dx).sqrt();
        let h = zs(p, xm) - zb;
        if h <= 0.0 {
            continue;
        }
        let alpha = (dx / r).asin();
        let w = s.column(zb, zb + h) * bw;
        let (c, phi, _) = s.at(zb);
        v.push(Sl { b: bw, w, alpha, u: s.ru * w / bw, c, tp: phi.to_radians().tan() });
    }
    Some((v, a, b))
}

fn fs_bishop(sl: &[Sl]) -> f64 {
    let drive: f64 = sl.iter().map(|q| q.w * q.alpha.sin()).sum();
    if drive <= 1e-9 {
        return f64::INFINITY;
    }
    let mut f = 1.5;
    for _ in 0..200 {
        let res: f64 = sl
            .iter()
            .map(|q| {
                let ma = q.alpha.cos() * (1.0 + q.alpha.tan() * q.tp / f);
                (q.c * q.b + (q.w - q.u * q.b) * q.tp) / ma
            })
            .sum();
        let nf = res / drive;
        if (nf - f).abs() < 1e-10 {
            return nf;
        }
        f = nf;
    }
    f
}

/// Spencer: interslice resultant Q at constant inclination θ; Ff from ΣQ = 0, Fm from
/// Σ Q·cos(α−θ) = 0 (moments about the circle centre); θ found where Ff = Fm.
fn spencer(sl: &[Sl]) -> (f64, f64) {
    let q = |f: f64, th: f64, q_: &Sl| {
        let sec = 1.0 / q_.alpha.cos();
        let num = q_.c * q_.b * sec / f + (q_.w * q_.alpha.cos() - q_.u * q_.b * sec) * q_.tp / f - q_.w * q_.alpha.sin();
        let den = (q_.alpha - th).cos() * (1.0 + (q_.alpha - th).tan() * q_.tp / f);
        num / den
    };
    let solve = |th: f64, moment: bool| {
        let g = |f: f64| sl.iter().map(|x| q(f, th, x) * if moment { (x.alpha - th).cos() } else { 1.0 }).sum::<f64>();
        // scan upward from a low F for the first admissible sign change (all mα > 0.2)
        let ok = |f: f64| sl.iter().all(|x| (x.alpha - th).cos() * (1.0 + (x.alpha - th).tan() * x.tp / f) > 0.2);
        let mut br = None;
        let mut pf = 0.2;
        while pf < 10.0 {
            let nf = pf + 0.02;
            if ok(pf) && ok(nf) && g(pf).signum() != g(nf).signum() {
                br = Some((pf, nf));
                break;
            }
            pf = nf;
        }
        let Some((mut lo, mut hi)) = br else { return f64::NAN };
        for _ in 0..200 {
            let m = 0.5 * (lo + hi);
            if g(m).signum() == g(lo).signum() {
                lo = m
            } else {
                hi = m
            }
        }
        0.5 * (lo + hi)
    };
    let d = |th: f64| solve(th, false) - solve(th, true);
    let (mut lo, mut hi) = (-0.2, 0.8);
    let mut prev = (lo, d(lo));
    let mut found = None;
    for k in 1..=50 {
        let th = lo + (hi - lo) * k as f64 / 50.0;
        let v = d(th);
        if v.is_finite() && prev.1.is_finite() && v.signum() != prev.1.signum() {
            found = Some((prev.0, th));
            break;
        }
        prev = (th, v);
    }
    let Some((a, b)) = found else { return (fs_bishop(sl), 0.0) };
    lo = a;
    hi = b;
    for _ in 0..80 {
        let m = 0.5 * (lo + hi);
        if d(m).signum() == d(lo).signum() {
            lo = m
        } else {
            hi = m
        }
    }
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
                let r = (zc - zt) + (-0.2 + (1.0 * w + 0.2) * k as f64 / 15.0);
                if r <= 0.5 {
                    continue;
                }
                let Some((sl, a, b)) = slices(p, s, xc, zc, r, n) else { continue };
                if sl.len() < n / 2 || (b - a) < 0.5 * w {
                    continue;
                }
                let f = fs_bishop(&sl);
                if f.is_finite() && best.as_ref().is_none_or(|c| f < c.fs_bishop) {
                    best = Some(Circle { xc, zc, r, fs_bishop: f, fs_spencer: f64::NAN, theta_deg: 0.0, x_entry: a, x_exit: b, slices: sl.len() });
                }
            }
        }
    }
    // local refinement: pattern search on (xc, zc, r) from the best grid circle, halving steps
    if let Some(c0) = best.clone() {
        let (mut xc, mut zc, mut r, mut fbest) = (c0.xc, c0.zc, c0.r, c0.fs_bishop);
        let mut step = [w / 6.0, w / 6.0, w / 10.0];
        let eval = |xc: f64, zc: f64, r: f64| -> Option<(f64, f64, f64, usize)> {
            if r <= 0.5 {
                return None;
            }
            let (sl, a, b) = slices(p, s, xc, zc, r, n)?;
            if sl.len() < n / 2 || (b - a) < 0.5 * w {
                return None;
            }
            let f = fs_bishop(&sl);
            if f.is_finite() {
                Some((f, a, b, sl.len()))
            } else {
                None
            }
        };
        for _ in 0..200 {
            let mut improved = false;
            for (dx, dz, dr) in [
                (1.0, 0.0, 0.0),
                (-1.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (0.0, -1.0, 0.0),
                (0.0, 0.0, 1.0),
                (0.0, 0.0, -1.0),
                (0.0, 1.0, 1.0),
                (0.0, -1.0, -1.0),
                (1.0, 0.0, 1.0),
                (-1.0, 0.0, -1.0),
            ] {
                let (x2, z2, r2) = (xc + dx * step[0], zc + dz * step[1], r + dr * step[2]);
                if let Some((f, a, b, ns)) = eval(x2, z2, r2) {
                    if f < fbest - 1e-9 {
                        fbest = f;
                        xc = x2;
                        zc = z2;
                        r = r2;
                        improved = true;
                        best = Some(Circle { xc, zc, r, fs_bishop: f, fs_spencer: f64::NAN, theta_deg: 0.0, x_entry: a, x_exit: b, slices: ns });
                    }
                }
            }
            if !improved {
                for q in step.iter_mut() {
                    *q *= 0.5;
                }
                if step[0] < 0.01 {
                    break;
                }
            }
        }
    }
    if let Some(c) = best.as_mut() {
        let (sl, _, _) = slices(p, s, c.xc, c.zc, c.r, n).unwrap();
        let (fsp, th) = spencer(&sl);
        c.fs_spencer = fsp;
        c.theta_deg = th.to_degrees();
    }
    best
}

pub fn fs_for(p: &[[f64; 2]], s: &Soil, xc: f64, zc: f64, r: f64, n: usize) -> Option<(f64, f64)> {
    let (sl, _, _) = slices(p, s, xc, zc, r, n)?;
    Some((fs_bishop(&sl), spencer(&sl).0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frictional_infinite_slope_limit_and_spencer_close_to_bishop() {
        // 2H:1V slope, 10 m high, c = 10 kPa, φ = 30°, γ = 19
        let p = vec![[-30.0, 0.0], [0.0, 0.0], [20.0, 10.0], [50.0, 10.0]];
        let s = Soil { c_kpa: 10.0, phi_deg: 30.0, gamma_knm3: 19.0, ru: 0.0, source: String::new(), layers: vec![] };
        let c = critical(&p, &s, 0.0, 20.0).unwrap();
        // c=0 lower bound: tanφ/tanβ = 0.577/0.5 = 1.155; with cohesion FS must be higher
        assert!(c.fs_bishop > 1.155 && c.fs_bishop < 2.5, "{}", c.fs_bishop);
        assert!((c.fs_spencer - c.fs_bishop).abs() / c.fs_bishop < 0.05, "{} {}", c.fs_spencer, c.fs_bishop);
    }
    #[test]
    fn layered_equals_homogeneous_when_layers_identical_and_weak_layer_lowers_fs() {
        let p = vec![[-30.0, 0.0], [0.0, 0.0], [20.0, 10.0], [50.0, 10.0]];
        let h = Soil { c_kpa: 10.0, phi_deg: 30.0, gamma_knm3: 19.0, ru: 0.0, source: String::new(), layers: vec![] };
        let mut l = h.clone();
        l.layers.push(SoilLayer { name: "same".into(), z_top: 2.0, c_kpa: 10.0, phi_deg: 30.0, gamma_knm3: 19.0 });
        let a = critical(&p, &h, 0.0, 20.0).unwrap();
        let b = critical(&p, &l, 0.0, 20.0).unwrap();
        assert!((a.fs_bishop - b.fs_bishop).abs() < 1e-9);
        l.layers[0] = SoilLayer { name: "soft clay".into(), z_top: 2.0, c_kpa: 5.0, phi_deg: 15.0, gamma_knm3: 17.0 };
        let c = critical(&p, &l, 0.0, 20.0).unwrap();
        assert!(c.fs_bishop < a.fs_bishop - 0.05, "{} {}", c.fs_bishop, a.fs_bishop);
    }
}
