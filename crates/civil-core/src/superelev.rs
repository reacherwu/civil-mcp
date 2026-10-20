//! Superelevation design and cross-slope evaluation along an alignment.
//!
//! Rotation about the centreline. The outside edge slope changes linearly from
//! -nc (normal crown) at A to +e at B; the inside lane stays at -nc until the
//! outside reaches +nc and then mirrors it (inside = min(-nc, -outside)).
//! Slopes are signed "rise per metre going away from the centreline".
use crate::align::Alignment;
use crate::standards::{self, DesignBasis, Standard};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SuperCurve {
    pub curve: usize,
    pub radius: f64,
    /// +1 left turn, -1 right turn
    pub dir: f64,
    /// Design superelevation (m/m); 0 when normal crown is retained.
    pub e: f64,
    pub method: String,
    pub required_length: f64,
    pub a_in: f64,
    pub b_in: f64,
    pub b_out: f64,
    pub a_out: f64,
    pub relative_gradient: f64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Superelevation {
    pub nc: f64,
    pub rotation_width: f64,
    pub curves: Vec<SuperCurve>,
}

/// AASHTO 2018 metric maximum relative gradient (%), by design speed (Table 3-15 values).
pub fn aashto_max_rel_gradient_pct(v: f64) -> f64 {
    let t = [(20.0, 0.80), (30.0, 0.75), (40.0, 0.70), (50.0, 0.65), (60.0, 0.60), (70.0, 0.55), (80.0, 0.50), (90.0, 0.47), (100.0, 0.44), (110.0, 0.41), (120.0, 0.38), (130.0, 0.35)];
    let mut best = t[0];
    for r in t {
        if (r.0 - v).abs() < (best.0 - v).abs() {
            best = r
        }
    }
    best.1
}

pub fn design(b: &DesignBasis, al: &Alignment) -> crate::Result<Superelevation> {
    let nc = b.normal_crown;
    let w = b.lane_width_m * b.lanes_rotated_per_side;
    let v = b.design_speed_kmh;
    let emax = b.e_max.ok_or("e_max must be set by the engineer")?;
    let f = b.f_max.ok_or("f_max (side friction / JTG μ) must be set by the engineer")?;
    let mut out = Vec::new();
    for c in al.curves() {
        let r = c.radius;
        let (e, method) = match b.standard {
            Standard::Aashto2018 => {
                let rmin = v * v / (127.0 * (emax + f));
                (emax * rmin / r, format!("AASHTO Method 1: e = emax·Rmin/R = {emax}·{rmin:.1}/{r:.1}"))
            }
            Standard::JtgD20_2017 => {
                let row = standards::jtg_row(v).ok_or("unsupported JTG speed")?;
                if r >= row.no_super_radius {
                    (0.0, format!("R ≥ 不设超高最小半径 {} m", row.no_super_radius))
                } else {
                    (v * v / (127.0 * r) - f, format!("i = V²/(127R) − μ = {v}²/(127·{r:.1}) − {f}"))
                }
            }
        };
        let (e, note) = if e <= nc + 1e-12 {
            if e > 0.0 {
                (nc, "computed e below normal crown; full section rotated to e = nc".to_string())
            } else {
                (0.0, "normal crown retained".to_string())
            }
        } else if e > emax {
            (emax, "capped at e_max".to_string())
        } else {
            (e, String::new())
        };
        if e == 0.0 {
            out.push(SuperCurve { curve: c.index, radius: r, dir: c.dir, e, method, required_length: 0.0, a_in: c.sta_start, b_in: c.sta_start, b_out: c.sta_end, a_out: c.sta_end, relative_gradient: 0.0, note });
            continue;
        }
        // required transition length (edge rise / allowed relative gradient)
        let req = match b.standard {
            Standard::Aashto2018 => w * (e + nc) / (aashto_max_rel_gradient_pct(v) / 100.0),
            Standard::JtgD20_2017 => w * (e + nc) * standards::jtg_row(v).unwrap().rel_gradient_inv,
        };
        let spiral = c.ls_in > 0.0;
        let (lin, lout) = if spiral { (c.ls_in.max(req), c.ls_out.max(req)) } else { (req, req) };
        // where the full superelevation is reached (B) on entry/exit
        let (b_in, b_out) = if spiral {
            (c.sta_cs_start, c.sta_cs_end)
        } else {
            match b.standard {
                // AASHTO: ~2/3 of runoff (here: of the transition) on the tangent
                Standard::Aashto2018 => (c.sta_start + lin / 3.0, c.sta_end - lout / 3.0),
                // JTG: simple curve, transition placed on the tangent, full e from PC
                Standard::JtgD20_2017 => (c.sta_start, c.sta_end),
            }
        };
        let (b_in, b_out) = if b_in > b_out { let m = (c.sta_start + c.sta_end) / 2.0; (m, m) } else { (b_in, b_out) };
        let a_in = b_in - lin;
        let a_out = b_out + lout;
        let rel = w * (e + nc) / lin.min(lout);
        out.push(SuperCurve { curve: c.index, radius: r, dir: c.dir, e, method, required_length: req, a_in, b_in, b_out, a_out, relative_gradient: rel, note });
    }
    Ok(Superelevation { nc, rotation_width: w, curves: out })
}

impl Superelevation {
    /// (left slope, right slope) at station s.
    pub fn slopes_at(&self, s: f64) -> (f64, f64) {
        let nc = self.nc;
        for c in &self.curves {
            if c.e == 0.0 || s <= c.a_in || s >= c.a_out {
                continue;
            }
            let outer = if s < c.b_in {
                -nc + (c.e + nc) * (s - c.a_in) / (c.b_in - c.a_in)
            } else if s <= c.b_out {
                c.e
            } else {
                -nc + (c.e + nc) * (c.a_out - s) / (c.a_out - c.b_out)
            };
            let inner = (-nc).min(-outer);
            // left turn: inside is left
            return if c.dir > 0.0 { (inner, outer) } else { (outer, inner) };
        }
        (-nc, -nc)
    }
}
