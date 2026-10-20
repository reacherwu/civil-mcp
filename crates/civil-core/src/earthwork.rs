//! Corridor cut/fill by cross sections and the average-end-area method.
use crate::align::Alignment;
use crate::profile::Profile;
use crate::tin::Tin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Finished-grade half width from centreline to edge (m).
    pub half_width_m: f64,
    /// Crown cross slope, m/m, falling away from centreline (e.g. 0.02).
    pub cross_slope: f64,
    /// Cut side slope, horizontal per 1 vertical (e.g. 2.0 = 2H:1V).
    pub cut_slope_h: f64,
    /// Fill side slope, horizontal per 1 vertical.
    pub fill_slope_h: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    pub station: f64,
    pub x: f64,
    pub y: f64,
    pub heading: f64,
    pub cl_elev: f64,
    pub ground_cl: f64,
    /// Signed offsets of daylight (left negative? no: left positive) — [left, right] as distances >= 0.
    pub daylight_left: f64,
    pub daylight_right: f64,
    pub cut_area: f64,
    pub fill_area: f64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Earthwork {
    pub method: String,
    pub interval_m: f64,
    pub sections: Vec<Section>,
    pub cut_m3: f64,
    pub fill_m3: f64,
    pub net_m3: f64,
    pub sections_with_warnings: usize,
}

const MAX_OFFSET: f64 = 80.0;
const STEP: f64 = 0.25;

/// Template elevation at signed offset `o` (left +). Returns None beyond daylight.
pub fn template_z(t: &Template, cl: f64, o: f64, ground: f64) -> f64 {
    let a = o.abs();
    let edge = cl - t.cross_slope * t.half_width_m;
    if a <= t.half_width_m {
        return cl - t.cross_slope * a;
    }
    let d = a - t.half_width_m;
    if ground >= edge {
        edge + d / t.cut_slope_h
    } else {
        edge - d / t.fill_slope_h
    }
}

fn side(tin: &Tin, t: &Template, cl: f64, x: f64, y: f64, nx: f64, ny: f64, sign: f64) -> (f64, f64, f64, Option<String>) {
    // walk outward from the centreline; returns (daylight, cut_area, fill_area, warning)
    let (mut cut, mut fill) = (0.0, 0.0);
    let mut prev: Option<(f64, f64)> = None; // (offset, diff=design-ground)
    let mut o = 0.0;
    let edge = cl - t.cross_slope * t.half_width_m;
    let mut mode: Option<bool> = None; // Some(true)=cut slope
    while o <= MAX_OFFSET + 1e-9 {
        let g = match tin.elevation_at(x + sign * o * nx, y + sign * o * ny) {
            Some(g) => g,
            None => return (o, cut, fill, Some(format!("ground missing at offset {:.1} m ({})", o, if sign > 0.0 { "left" } else { "right" }))),
        };
        let z = if o <= t.half_width_m {
            cl - t.cross_slope * o
        } else {
            let m = *mode.get_or_insert_with(|| tin.elevation_at(x + sign * t.half_width_m * nx, y + sign * t.half_width_m * ny).map_or(true, |ge| ge >= edge));
            let d = o - t.half_width_m;
            if m { edge + d / t.cut_slope_h } else { edge - d / t.fill_slope_h }
        };
        let diff = z - g;
        if let Some((po, pd)) = prev {
            let beyond = o > t.half_width_m;
            let crossed = beyond && pd != 0.0 && (pd > 0.0) != (diff > 0.0);
            let (o2, d2) = if crossed { (po + pd / (pd - diff) * (o - po), 0.0) } else { (o, diff) };
            // trapezoid with sign split
            let (a1, a2) = (pd, d2);
            let w = o2 - po;
            if a1 >= 0.0 && a2 >= 0.0 {
                fill += (a1 + a2) / 2.0 * w
            } else if a1 <= 0.0 && a2 <= 0.0 {
                cut += -(a1 + a2) / 2.0 * w
            } else {
                let xz = a1 / (a1 - a2) * w;
                if a1 > 0.0 {
                    fill += a1 * xz / 2.0;
                    cut += -a2 * (w - xz) / 2.0
                } else {
                    cut += -a1 * xz / 2.0;
                    fill += a2 * (w - xz) / 2.0
                }
            }
            if crossed || (beyond && diff == 0.0) {
                return (o2, cut, fill, None);
            }
        }
        prev = Some((o, diff));
        o += STEP;
    }
    (MAX_OFFSET, cut, fill, Some(format!("no daylight within {MAX_OFFSET} m ({})", if sign > 0.0 { "left" } else { "right" })))
}

pub fn compute(tin: &Tin, al: &Alignment, pr: &Profile, t: &Template, interval: f64) -> crate::Result<Earthwork> {
    if !(interval > 0.0) || t.half_width_m <= 0.0 || t.cut_slope_h <= 0.0 || t.fill_slope_h <= 0.0 {
        return Err("interval, half width and slopes must be > 0".into());
    }
    let (s0, s1) = (al.start_station().max(pr.start()), al.end_station().min(pr.end()));
    let mut stations = Vec::new();
    let mut s = s0;
    while s < s1 - 1e-9 {
        stations.push(s);
        s = ((s / interval).floor() + 1.0) * interval;
    }
    stations.push(s1);
    let mut secs = Vec::new();
    for &s in &stations {
        let (x, y, h) = al.point_at(s).ok_or("station off alignment")?;
        let cl = pr.elevation_at(s).ok_or("station off profile")?;
        let (nx, ny) = (-h.sin(), h.cos()); // left normal
        let gcl = tin.elevation_at(x, y).ok_or_else(|| format!("centreline at station {s:.2} is outside the surface"))?;
        let (dl, cl_, fl, wl) = side(tin, t, cl, x, y, nx, ny, 1.0);
        let (dr, cr, fr, wr) = side(tin, t, cl, x, y, nx, ny, -1.0);
        secs.push(Section { station: s, x, y, heading: h, cl_elev: cl, ground_cl: gcl, daylight_left: dl, daylight_right: dr, cut_area: cl_ + cr, fill_area: fl + fr, warnings: wl.into_iter().chain(wr).collect() });
    }
    let (mut cut, mut fill) = (0.0, 0.0);
    for w in secs.windows(2) {
        let l = w[1].station - w[0].station;
        cut += (w[0].cut_area + w[1].cut_area) / 2.0 * l;
        fill += (w[0].fill_area + w[1].fill_area) / 2.0 * l;
    }
    let nw = secs.iter().filter(|s| !s.warnings.is_empty()).count();
    Ok(Earthwork { method: "average end area, no curvature correction, no shrink/swell".into(), interval_m: interval, sections: secs, cut_m3: cut, fill_m3: fill, net_m3: cut - fill, sections_with_warnings: nw })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::align::{Alignment, AlignmentInput};
    use crate::profile::{Profile, ProfileInput};
    #[test]
    fn prism_in_flat_ground_matches_hand_calc() {
        // flat ground z=100, road 2 m above with no crown, 2:1 fill slopes, half width 5
        let mut pts = vec![];
        for i in -20..=20 {
            for j in -20..=60 {
                pts.push([i as f64 * 10.0 + (j % 2) as f64 * 0.01, j as f64 * 10.0, 100.0]);
            }
        }
        let (tin, _) = Tin::build(&pts, None).unwrap();
        let al = Alignment::build(AlignmentInput { name: "a".into(), pis: vec![[0.0, 0.0], [0.0, 500.0]], radii: vec![], start_station: 0.0 }).unwrap();
        let pr = Profile::build(ProfileInput { name: "p".into(), vpis: vec![[0.0, 102.0], [500.0, 102.0]], curve_lengths: vec![] }).unwrap();
        let t = Template { half_width_m: 5.0, cross_slope: 0.0, cut_slope_h: 2.0, fill_slope_h: 2.0 };
        let e = compute(&tin, &al, &pr, &t, 20.0).unwrap();
        // area = 2*(5*2) + 2*(0.5*4*2) = 20 + 8 = 28 m² ; volume = 28*500
        let s = &e.sections[3];
        assert!((s.fill_area - 28.0).abs() < 1e-6, "{}", s.fill_area);
        assert!((s.daylight_left - 9.0).abs() < 1e-6);
        assert!((e.fill_m3 - 14000.0).abs() < 1e-3);
        assert!(e.cut_m3.abs() < 1e-9);
    }
}
