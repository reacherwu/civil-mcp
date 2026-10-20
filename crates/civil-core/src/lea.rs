#![allow(clippy::excessive_precision, clippy::needless_range_loop)]
//! Multilayer linear-elastic analysis (Burmister layered half-space) under uniform circular loads.
//! Love stress function φ = J0(mr)·f(z), f = (P + Qζ)e^{+m(ζ-h)} + (R + Sζ)e^{-mζ} per layer, fully
//! bonded interfaces (σz, τrz, w, u continuous), traction-free surface outside the load, bottom
//! layer semi-infinite. Hankel inversion R(r,z) = q·a·∫ R*(m) J1(ma) dm by Gauss–Legendre over
//! half-periods of J1(ma). Sign convention: tension positive. Units: any consistent set
//! (e.g. kPa, m, kPa for E). Verified against Boussinesq and the published KENLAYER (KENPAVE)
//! three-layer table distributed with PyMastic (see scripts/independent_check_v6.py).
use serde::{Deserialize, Serialize};

pub const LEA_REF: &str = "Burmister (1945) layered system; Huang, Pavement Analysis and Design (2004) Ch. 2 & App. B (KENLAYER)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaLayer {
    /// thickness (ignored for the last, semi-infinite layer)
    pub h: f64,
    pub e: f64,
    pub nu: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct Resp {
    pub sz: f64,
    pub sr: f64,
    pub st: f64,
    pub w: f64,
    pub ez: f64,
    pub er: f64,
    pub et: f64,
}

/// Bessel J0 (Numerical Recipes rational approximations, |err| < 1e-8)
pub fn j0(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let a1 = 57568490574.0 + y * (-13362590354.0 + y * (651619640.7 + y * (-11214424.18 + y * (77392.33017 + y * (-184.9052456)))));
        let a2 = 57568490411.0 + y * (1029532985.0 + y * (9494680.718 + y * (59272.64853 + y * (267.8532712 + y))));
        a1 / a2
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 0.785398164;
        let a1 = 1.0 + y * (-0.1098628627e-2 + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let a2 = -0.1562499995e-1 + y * (0.1430488765e-3 + y * (-0.6911147651e-5 + y * (0.7621095161e-6 - y * 0.934935152e-7)));
        (std::f64::consts::FRAC_2_PI / ax).sqrt() * (xx.cos() * a1 - z * xx.sin() * a2)
    }
}

/// Bessel J1
pub fn j1(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let a1 = x * (72362614232.0 + y * (-7895059235.0 + y * (242396853.1 + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
        let a2 = 144725228442.0 + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        a1 / a2
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 2.356194491;
        let a1 = 1.0 + y * (0.183105e-2 + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
        let a2 = 0.04687499995 + y * (-0.2002690873e-3 + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        let a = (std::f64::consts::FRAC_2_PI / ax).sqrt() * (xx.cos() * a1 - z * xx.sin() * a2);
        if x < 0.0 {
            -a
        } else {
            a
        }
    }
}

fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().partial_cmp(&a[j][c].abs()).unwrap()).unwrap();
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f != 0.0 {
                for k in c..n {
                    a[r][k] -= f * a[c][k];
                }
                b[r] -= f * b[c];
            }
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    x
}

/// terms of layer i at local depth ζ: (s, E, X, column of P-like unknown)
fn terms(ls: &[LeaLayer], i: usize, m: f64, zeta: f64) -> Vec<(f64, f64, f64, usize)> {
    let n = ls.len();
    let x = m * zeta;
    let mut v = vec![];
    let base = 4 * i;
    if i + 1 < n {
        v.push((1.0, (m * (zeta - ls[i].h)).exp(), x, base));
        v.push((-1.0, (-m * zeta).exp(), x, base + 2));
    } else {
        v.push((-1.0, (-m * zeta).exp(), x, base));
    }
    v
}

/// rows (σz, τ, w, u) contributions of layer i at ζ; returns 4 rows × unknown count
fn rows(ls: &[LeaLayer], i: usize, m: f64, zeta: f64, nu_k: usize) -> [Vec<f64>; 4] {
    let l = &ls[i];
    let k = (1.0 + l.nu) / l.e;
    let mut r = [vec![0.0; nu_k], vec![0.0; nu_k], vec![0.0; nu_k], vec![0.0; nu_k]];
    for (s, e, x, c) in terms(ls, i, m, zeta) {
        // unknowns p (col c) and t (col c+1)
        r[0][c] += -s * e;
        r[0][c + 1] += ((1.0 - 2.0 * l.nu) - s * x) * e;
        r[1][c] += -e;
        r[1][c + 1] += (-x - 2.0 * l.nu * s) * e;
        r[2][c] += -k * e;
        r[2][c + 1] += k * (2.0 * s * (1.0 - 2.0 * l.nu) - x) * e;
        r[3][c] += k * s * e;
        r[3][c + 1] += k * (1.0 + s * x) * e;
    }
    r
}

fn coeffs(ls: &[LeaLayer], m: f64) -> Vec<f64> {
    let n = ls.len();
    let nk = 4 * n - 2;
    let mut a = vec![];
    let mut b = vec![];
    let r0 = rows(ls, 0, m, 0.0, nk);
    a.push(r0[0].clone());
    b.push(-1.0);
    a.push(r0[1].clone());
    b.push(0.0);
    for i in 0..n - 1 {
        let up = rows(ls, i, m, ls[i].h, nk);
        let dn = rows(ls, i + 1, m, 0.0, nk);
        for j in 0..4 {
            a.push(up[j].iter().zip(&dn[j]).map(|(p, q)| p - q).collect());
            b.push(0.0);
        }
    }
    solve(a, b)
}

/// kernel values at (r, z) for one m: (σz, σr, σθ, w) per unit q·a, before ×J1(ma)
fn kernel(ls: &[LeaLayer], m: f64, r: f64, i: usize, zeta: f64) -> [f64; 4] {
    let c = coeffs(ls, m);
    let l = &ls[i];
    let k = (1.0 + l.nu) / l.e;
    let (jr0, jr1m) = if r == 0.0 { (1.0, 0.5) } else { (j0(m * r), j1(m * r) / (m * r)) };
    let mut o = [0.0; 4];
    for (s, e, x, col) in terms(ls, i, m, zeta) {
        let (p, t) = (c[col], c[col + 1]);
        let f = (t + s * (p + t * x)) * e;
        o[0] += ((1.0 - 2.0 * l.nu) * t - s * (p + t * x)) * e * jr0;
        o[1] += (2.0 * l.nu * t * e + f) * jr0 - f * jr1m;
        o[2] += 2.0 * l.nu * t * e * jr0 + f * jr1m;
        o[3] += k / m * (2.0 * s * (1.0 - 2.0 * l.nu) * t - (p + t * x)) * e * jr0;
    }
    o
}

const GL: [(f64, f64); 8] = [
    (0.1834346424956498, 0.3626837833783620),
    (-0.1834346424956498, 0.3626837833783620),
    (0.5255324099163290, 0.3137066458778873),
    (-0.5255324099163290, 0.3137066458778873),
    (0.7966664774136267, 0.2223810344533745),
    (-0.7966664774136267, 0.2223810344533745),
    (0.9602898564975363, 0.1012285362903763),
    (-0.9602898564975363, 0.1012285362903763),
];

/// response at horizontal distance r from the centre of one circular load (pressure q, radius a), depth z.
/// A point exactly on an interface is assigned to the upper layer.
pub fn response(ls: &[LeaLayer], q: f64, a: f64, r: f64, z: f64) -> Resp {
    let (mut i, mut top) = (0, 0.0);
    while i + 1 < ls.len() && z > top + ls[i].h {
        top += ls[i].h;
        i += 1;
    }
    let zeta = z - top;
    // integrate over panels of width π/(a+r)/2, to m_max with tail stop
    let dm = std::f64::consts::PI / (a + r + 1e-12) / 2.0;
    let mmax = 400.0 / a;
    let mut acc = [0.0; 4];
    let mut m0 = 0.0;
    let mut small = 0;
    while m0 < mmax {
        let mut part = [0.0; 4];
        for &(xg, wg) in &GL {
            let m = m0 + 0.5 * dm * (1.0 + xg);
            let kv = kernel(ls, m, r, i, zeta);
            let jm = j1(m * a);
            for j in 0..4 {
                part[j] += 0.5 * dm * wg * kv[j] * jm;
            }
        }
        for j in 0..4 {
            acc[j] += part[j];
        }
        m0 += dm;
        let tiny = (0..4).all(|j| part[j].abs() <= 1e-11 * acc[j].abs().max(1e-30));
        small = if tiny { small + 1 } else { 0 };
        if small > 8 {
            break;
        }
    }
    let l = &ls[i];
    let (sz, sr, st, w) = (q * a * acc[0], q * a * acc[1], q * a * acc[2], q * a * acc[3]);
    Resp { sz, sr, st, w, ez: (sz - l.nu * (sr + st)) / l.e, er: (sr - l.nu * (st + sz)) / l.e, et: (st - l.nu * (sr + sz)) / l.e }
}

/// Cartesian strains at a point from several identical circular loads at (x, y) positions; returns (εx, εy, εz, w)
pub fn multi(ls: &[LeaLayer], q: f64, a: f64, loads: &[[f64; 2]], p: [f64; 2], z: f64) -> (f64, f64, f64, f64) {
    let (mut sx, mut sy, mut szz, mut w) = (0.0, 0.0, 0.0, 0.0);
    for c in loads {
        let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
        let r = (dx * dx + dy * dy).sqrt();
        let o = response(ls, q, a, r, z);
        let (cs, sn) = if r > 1e-9 { (dx / r, dy / r) } else { (1.0, 0.0) };
        sx += o.sr * cs * cs + o.st * sn * sn;
        sy += o.sr * sn * sn + o.st * cs * cs;
        szz += o.sz;
        w += o.w;
    }
    let (mut i, mut top) = (0, 0.0);
    while i + 1 < ls.len() && z > top + ls[i].h {
        top += ls[i].h;
        i += 1;
    }
    let l = &ls[i];
    ((sx - l.nu * (sy + szz)) / l.e, (sy - l.nu * (sx + szz)) / l.e, (szz - l.nu * (sx + sy)) / l.e, w)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boussinesq_half_space() {
        let ls = vec![LeaLayer { h: 0.1, e: 1e5, nu: 0.35 }, LeaLayer { h: 0.0, e: 1e5, nu: 0.35 }];
        let (q, a) = (700.0, 0.15);
        for z in [0.05, 0.1, 0.3] {
            let o = response(&ls, q, a, 0.0, z);
            let r = (a * a + z * z).sqrt();
            let sz = -q * (1.0 - (z / r).powi(3));
            assert!((o.sz - sz).abs() < 1e-3 * q, "{z} {} {sz}", o.sz);
            let w = (1.0 + 0.35) * q * a / 1e5 * (a / r + (1.0 - 2.0 * 0.35) * (r - z) / a);
            assert!((o.w - w).abs() / w < 2e-3, "{z} {} {w}", o.w);
        }
    }
    #[test]
    fn kenlayer_three_layer_published() {
        // KENPAVE results distributed with PyMastic (Validation/Kenpave.csv): q 100 psi, a 5.99 in,
        // h 6, 10 in, E 500/40/10 ksi, ν 0.35/0.40/0.45, bonded, r = 0
        let ls = vec![LeaLayer { h: 6.0, e: 500e3, nu: 0.35 }, LeaLayer { h: 10.0, e: 40e3, nu: 0.4 }, LeaLayer { h: 0.0, e: 10e3, nu: 0.45 }];
        for (z, w, sz, er) in
            [(6.0, 0.02186, 24.051, -0.0002332), (10.0, 0.01991, 13.195, -0.0001872), (16.0, 0.01774, 5.722, -0.0002353), (25.0, 0.01401, 3.419, -0.0001426)]
        {
            let o = response(&ls, 100.0, 5.99, 0.0, z);
            assert!((o.w - w).abs() / w < 0.01, "w {z} {} {w}", o.w);
            assert!((-o.sz - sz).abs() / sz < 0.01, "sz {z} {} {sz}", o.sz);
            // KENPAVE strain sign: compression positive
            assert!((-o.er - er).abs() / er.abs() < 0.01, "er {z} {} {er}", o.er);
        }
    }
}
