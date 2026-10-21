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

/// Design superelevation for radius `r` (same rules as `design`): (e, method, note).
pub fn e_for(b: &DesignBasis, r: f64) -> crate::Result<(f64, String, String)> {
    let r0 = standards::resolve(b)?;
    let nc = b.normal_crown;
    let v = b.design_speed_kmh;
    let emax = b.e_max.ok_or("e_max must be set by the engineer")?;
    let (e, method) = match b.standard {
        Standard::Uk => {
            let row = standards::cd109_row(v).unwrap();
            if r >= row[3] {
                (0.0, format!("CD 109 §4.1: R ≥ {} m (adverse camber permitted), normal camber retained", row[3]))
            } else {
                let s = v * v / (2.828 * r) / 100.0;
                (s.max(0.025), format!("CD 109 Eq 4.2: S = V²/(2.828R) = {v}²/(2.828·{r:.1}) % (min 2.5 % once adverse camber is removed)"))
            }
        }
        _ => (
            emax * r0.r_min / r,
            format!(
                "proportional distribution (AASHTO Method 1): e = emax·Rmin/R = {emax}·{:.1}/{r:.1}; engineer to confirm against the jurisdiction's e/R table",
                r0.r_min
            ),
        ),
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
    Ok((e, method, note))
}

/// Required superelevation development length for radius `r`: w·(e + nc)/Δ, from normal
/// crown to full superelevation at the standard's maximum relative gradient (AASHTO maximum
/// relative gradient, CD 109 / Austroads equivalents through `standards::resolve`). 0 when the
/// curve keeps normal crown.
pub fn required_development(b: &DesignBasis, r: f64) -> crate::Result<f64> {
    let r0 = standards::resolve(b)?;
    let (e, _, _) = e_for(b, r)?;
    if e == 0.0 {
        return Ok(0.0);
    }
    Ok(b.lane_width_m * b.lanes_rotated_per_side * (e + b.normal_crown) / (r0.rel_grad_pct / 100.0))
}

pub fn design(b: &DesignBasis, al: &Alignment) -> crate::Result<Superelevation> {
    let r0 = standards::resolve(b)?;
    let nc = b.normal_crown;
    let w = b.lane_width_m * b.lanes_rotated_per_side;
    let mut out = Vec::new();
    for c in al.curves() {
        let r = c.radius;
        let (e, method, note) = e_for(b, r)?;
        if e == 0.0 {
            out.push(SuperCurve {
                curve: c.index,
                radius: r,
                dir: c.dir,
                e,
                method,
                required_length: 0.0,
                a_in: c.sta_start,
                b_in: c.sta_start,
                b_out: c.sta_end,
                a_out: c.sta_end,
                relative_gradient: 0.0,
                note,
            });
            continue;
        }
        // required development length (edge rise / allowed relative gradient)
        let req = w * (e + nc) / (r0.rel_grad_pct / 100.0);
        let spiral = c.ls_in > 0.0;
        let (lin, lout) = if spiral { (c.ls_in.max(req), c.ls_out.max(req)) } else { (req, req) };
        // where full superelevation is reached (B): on a spiral at the SC/CS; on a simple curve
        // two-thirds of the development lies on the tangent (AASHTO practice; engineer may vary).
        let (b_in, b_out) = if spiral { (c.sta_cs_start, c.sta_cs_end) } else { (c.sta_start + lin / 3.0, c.sta_end - lout / 3.0) };
        let (b_in, b_out) = if b_in > b_out {
            let m = (c.sta_start + c.sta_end) / 2.0;
            (m, m)
        } else {
            (b_in, b_out)
        };
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
