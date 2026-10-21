//! Construction-scale sheet set: plan-and-profile sheets split at match lines at a standard
//! scale (1:500 metric, 1" = 50' US customary), existing-conditions base with engineer-entered
//! right-of-way and property lines, survey control and curve data tables, gridded cross
//! sections at a standard interval with ROW and construction limits, and a quantity summary
//! by pay item. Content is drawn on the A1 layout (mm) and placed on the frame by
//! `frame::compose`, like the main set.
use crate::align::Alignment;
use crate::corridor::{Corridor, Section};
use crate::drawing::Drawing;
use crate::profile::Profile;
use crate::sheet::{sta_label, SHEET_H, SHEET_W};
use crate::tin::Tin;
use crate::units::{len as ul, lu};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Control {
    pub id: String,
    pub e: f64,
    pub n: f64,
    pub z: f64,
    #[serde(default)]
    pub desc: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Parcel {
    pub id: String,
    pub pts: Vec<[f64; 2]>,
}

/// Engineer inputs for the construction set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inputs {
    /// right-of-way offsets from the centreline (m), left and right
    pub row_left_m: Option<f64>,
    pub row_right_m: Option<f64>,
    #[serde(default)]
    pub property_lines: Vec<Parcel>,
    #[serde(default)]
    pub control: Vec<Control>,
    /// cross-section interval (m); default 20 m metric, 50 ft US customary
    pub xs_interval_m: Option<f64>,
    /// plan scale 1:n; default 500 metric, 600 (1" = 50') US customary
    pub plan_scale: Option<f64>,
}

pub struct Ctx<'a> {
    pub tin: &'a Tin,
    pub al: &'a Alignment,
    pub pr: &'a Profile,
    pub cor: &'a Corridor,
    pub boq: Vec<crate::deliver::BoqRow>,
    pub inp: &'a Inputs,
    /// imported parcels (import_parcels) and the ROW take of the current corridor
    pub parcels: Option<&'a crate::parcels::ParcelSet>,
    pub take: Option<crate::parcels::RowResult>,
}

/// Standard metric plan scales and US engineer's scales used for construction plans.
pub const SI_PLAN: &[f64] = &[200.0, 250.0, 500.0, 1000.0];
pub const US_PLAN: &[f64] = &[240.0, 480.0, 600.0, 1200.0];

#[derive(Debug, Clone, Serialize)]
pub struct Window {
    pub s0: f64,
    pub s1: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub plan_scale: f64,
    pub profile_v_scale: f64,
    pub windows: Vec<Window>,
    pub xs_interval_m: f64,
    pub xs_scale: f64,
    pub xs_stations: Vec<f64>,
    pub missing_inputs: Vec<String>,
}

const PX0: f64 = 20.0;
const PX1: f64 = SHEET_W - 160.0;

fn us() -> bool {
    crate::units::cur().us()
}

pub fn plan(c: &Ctx) -> Plan {
    let n = c.inp.plan_scale.unwrap_or(if us() { 600.0 } else { 500.0 });
    // usable plan width (mm) less 40 mm for match-line labels, rounded down to a round length
    let w_m = (PX1 - PX0 - 40.0) * n / 1000.0;
    let unit = if us() { 100.0 * 0.3048 } else { 50.0 };
    let win = (w_m / unit).floor() * unit;
    let (a, b) = (c.al.start_station().max(c.pr.start()), c.al.end_station().min(c.pr.end()));
    let mut windows = vec![];
    let mut s = a;
    while s < b - 1e-6 {
        // first match line on a round station
        let e = if windows.is_empty() { ((a + win) / unit).floor() * unit } else { s + win };
        let e = if e <= s + 1e-6 { s + win } else { e };
        windows.push(Window { s0: s, s1: e.min(b) });
        s = e;
    }
    // vertical scale: 10x exaggeration if the profile fits, otherwise the next standard scale
    let vlist: Vec<f64> = if us() { vec![60.0, 120.0, 240.0, 480.0, 600.0, 1200.0] } else { vec![50.0, 100.0, 200.0, 250.0, 500.0, 1000.0] };
    let per_win_range = windows
        .iter()
        .map(|w| {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for s in c.cor.sections.iter().filter(|s| s.station >= w.s0 - 1e-6 && s.station <= w.s1 + 1e-6) {
                lo = lo.min(s.cl_elev.min(s.ground_cl));
                hi = hi.max(s.cl_elev.max(s.ground_cl));
            }
            hi - lo
        })
        .fold(0.0, f64::max);
    let vs = vlist.iter().copied().find(|v| per_win_range * 1000.0 / v + 30.0 <= 230.0).unwrap_or(*vlist.last().unwrap());
    let xi = c.inp.xs_interval_m.unwrap_or(if us() { 50.0 * 0.3048 } else { 20.0 });
    let xs_stations: Vec<f64> = c
        .cor
        .sections
        .iter()
        .map(|s| s.station)
        .filter(|s| {
            let q = (s - a) / xi;
            (q - q.round()).abs() < 1e-6
        })
        .collect();
    let mut missing = vec![];
    if c.inp.row_left_m.is_none() || c.inp.row_right_m.is_none() {
        missing.push("right-of-way offsets (row_left_m / row_right_m)".to_string());
    }
    if c.inp.property_lines.is_empty() {
        missing.push(if c.parcels.is_some() {
            "property lines (boundary survey; the imported parcels are drawn for ROW screening only)".to_string()
        } else {
            "property lines".to_string()
        });
    }
    if c.inp.control.is_empty() {
        missing.push("survey control points".to_string());
    }
    // cross-section scale: fit ±(max daylight / ROW + 3 m) into a 360 mm box
    let half = c
        .cor
        .sections
        .iter()
        .map(|s| s.left.daylight.max(s.right.daylight))
        .fold(0.0, f64::max)
        .max(c.inp.row_left_m.unwrap_or(0.0))
        .max(c.inp.row_right_m.unwrap_or(0.0))
        + 3.0;
    let xl: &[f64] = if us() { &[120.0, 240.0, 480.0, 600.0] } else { &[100.0, 200.0, 250.0, 500.0] };
    let xs_scale = xl.iter().copied().find(|n| 2.0 * half * 1000.0 / n <= 360.0).unwrap_or(*xl.last().unwrap());
    let expected = ((b - a) / xi + 1e-9).floor() as usize + 1;
    if xs_stations.len() < expected {
        missing.push(format!(
            "corridor sections at the {xi:.2} m cross-section interval ({} of {expected} present; rebuild the corridor with interval_m = {xi:.2})",
            xs_stations.len()
        ));
    }
    Plan { plan_scale: n, profile_v_scale: vs, windows, xs_interval_m: xi, xs_scale, xs_stations, missing_inputs: missing }
}

fn sec_at(c: &Corridor, s: f64) -> Option<&Section> {
    c.sections.iter().min_by(|a, b| (a.station - s).abs().partial_cmp(&(b.station - s).abs()).unwrap())
}

fn layers(d: &mut Drawing) {
    crate::sheet::base(d);
    for (l, c) in [("ROW", 6), ("PARCEL", 8), ("ROW-TAKE", 1), ("DETAIL", 7), ("SUBGRADE", 3)] {
        d.layer(l, c);
    }
}

/// Plan-and-profile sheet for one window, with match lines to neighbouring sheets.
pub fn plan_profile(c: &Ctx, p: &Plan, k: usize, ids: &[String]) -> Drawing {
    let mut d = Drawing::default();
    layers(&mut d);
    let w = &p.windows[k];
    let sc = 1000.0 / p.plan_scale;
    // plan band: rotate so the window chord runs left to right
    let (py0, py1) = (310.0, SHEET_H - 20.0);
    d.rect("BORDER", PX0, py0, PX1, py1);
    let (xa, ya, _) = c.al.point_at(w.s0).unwrap();
    let (xb, yb, _) = c.al.point_at(w.s1).unwrap();
    let rot = (yb - ya).atan2(xb - xa);
    let (cr, sr) = (rot.cos(), rot.sin());
    let (cx, cy) = ((xa + xb) / 2.0, (ya + yb) / 2.0);
    let (vx, vy) = ((PX0 + PX1) / 2.0, (py0 + py1) / 2.0);
    let tp = move |x: f64, y: f64| {
        let (dx, dy) = (x - cx, y - cy);
        [vx + (dx * cr + dy * sr) * sc, vy + (-dx * sr + dy * cr) * sc]
    };
    let inside = |q: [f64; 2]| q[0] > PX0 && q[0] < PX1 && q[1] > py0 && q[1] < py1;
    // existing conditions: contours (1 m / 2 ft, index every 5th), survey points omitted for legibility
    let ci = if us() { 2.0 * 0.3048 } else { 1.0 };
    for (lv, a, b) in c.tin.contours(ci) {
        let (qa, qb) = (tp(a[0], a[1]), tp(b[0], b[1]));
        if inside(qa) && inside(qb) {
            let q = lv / ci / 5.0;
            d.line(if (q - q.round()).abs() < 1e-6 { "CONTOUR-MAJOR" } else { "CONTOUR-MINOR" }, qa, qb);
        }
    }
    for pl in &c.inp.property_lines {
        let v: Vec<[f64; 2]> = pl.pts.iter().map(|q| tp(q[0], q[1])).collect();
        if v.iter().any(|q| inside(*q)) {
            d.poly("PARCEL", v.iter().map(|q| [q[0].clamp(PX0, PX1), q[1].clamp(py0, py1)]).collect());
            if let Some(q) = v.iter().find(|q| inside(**q)) {
                d.text("PARCEL", [q[0] + 1.0, q[1] + 1.0], 2.0, &format!("PROPERTY {}", pl.id));
            }
        }
    }
    // imported parcels (user data) and the ROW take of the current corridor, hatched
    if let Some(ps) = c.parcels {
        let clip = |a: [f64; 2], b: [f64; 2]| inside(a) && inside(b);
        for p in &ps.parcels {
            let mut shown = false;
            for r in &p.rings {
                let v: Vec<[f64; 2]> = r.iter().map(|q| tp(q[0], q[1])).collect();
                for k in 0..v.len() {
                    let (a, b) = (v[k], v[(k + 1) % v.len()]);
                    if clip(a, b) {
                        d.line("PARCEL", a, b);
                        shown = true;
                    }
                }
            }
            if shown {
                let r = &p.rings[0];
                let cx = r.iter().map(|q| q[0]).sum::<f64>() / r.len() as f64;
                let cy = r.iter().map(|q| q[1]).sum::<f64>() / r.len() as f64;
                let q = tp(cx, cy);
                if inside(q) {
                    let z = p.zoning.as_deref().map_or(String::new(), |z| format!(" {z}"));
                    d.text("PARCEL", q, 2.0, &format!("PARCEL {}{}{}", p.id, z, if p.no_go { " NO-GO" } else { "" }));
                }
            }
        }
        if let Some(t) = &c.take {
            let fp = vec![t.polygon.clone()];
            let sp = 2.5 / sc;
            for tk in &t.takes {
                if let Some(p) = ps.parcels.iter().find(|p| p.id == tk.parcel_id) {
                    for s in crate::parcels::hatch(&p.rings, &fp, sp) {
                        let (a, b) = (tp(s[0][0], s[0][1]), tp(s[1][0], s[1][1]));
                        if clip(a, b) {
                            d.line("ROW-TAKE", a, b);
                        }
                    }
                }
            }
            let v: Vec<[f64; 2]> = t.polygon.iter().map(|q| tp(q[0], q[1])).collect();
            for k in 0..v.len() {
                let (a, b) = (v[k], v[(k + 1) % v.len()]);
                if clip(a, b) {
                    d.line("ROW-TAKE", a, b);
                }
            }
        }
    }
    let secs: Vec<&Section> = c.cor.sections.iter().filter(|s| s.station >= w.s0 - 1e-6 && s.station <= w.s1 + 1e-6).collect();
    let off = |s: &Section, left: bool, o: f64| {
        let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, o, left);
        tp(q[0], q[1])
    };
    for left in [true, false] {
        d.poly("DAYLIGHT", secs.iter().map(|s| off(s, left, if left { s.left.daylight } else { s.right.daylight })).collect());
        d.poly("CURB", secs.iter().map(|s| off(s, left, if left { s.left.paved_width } else { s.right.paved_width })).collect());
        if let Some(r) = if left { c.inp.row_left_m } else { c.inp.row_right_m } {
            d.poly("ROW", secs.iter().map(|s| off(s, left, r)).collect());
            if let Some(s) = secs.get(secs.len() / 2) {
                let q = off(s, left, r);
                d.text("ROW", [q[0], q[1] + if left { 1.5 } else { -4.0 }], 2.2, &format!("R/W {:.1} m {}", r, if left { "LT" } else { "RT" }));
            }
        }
    }
    let n = ((w.s1 - w.s0) / 2.0).ceil().max(1.0) as usize;
    let cl: Vec<[f64; 2]> = (0..=n)
        .filter_map(|i| {
            let (x, y, _) = c.al.point_at(w.s0 + (w.s1 - w.s0) * i as f64 / n as f64)?;
            Some(tp(x, y))
        })
        .collect();
    d.poly("ALIGNMENT", cl);
    let step = crate::units::sta_step() / 5.0;
    let mut s = (w.s0 / step).ceil() * step;
    while s <= w.s1 + 1e-9 {
        let (x, y, h) = c.al.point_at(s).unwrap();
        let (nx, ny) = (-h.sin(), h.cos());
        let major = ((s / (step * 5.0)) - (s / (step * 5.0)).round()).abs() < 1e-9;
        let t = if major { 3.0 } else { 1.5 } / sc;
        d.line("ALIGNMENT", tp(x - t * nx, y - t * ny), tp(x + t * nx, y + t * ny));
        if major {
            d.text("TEXT", tp(x + 4.0 / sc * nx, y + 4.0 / sc * ny), 2.5, &sta_label(s));
        }
        s += step;
    }
    // curve labels
    for cv in c.al.curves() {
        let m = (cv.sta_start + cv.sta_end) / 2.0;
        if m >= w.s0 && m <= w.s1 {
            let (x, y, h) = c.al.point_at(m).unwrap();
            let q = tp(x - 12.0 / sc * h.sin() * cv.dir, y + 12.0 / sc * h.cos() * cv.dir);
            d.text("TEXT", q, 2.5, &format!("CURVE {} R={:.1} m (SEE CURVE DATA)", cv.index + 1, cv.radius));
        }
    }
    // match lines
    for (end, s, other) in [(false, w.s0, k.checked_sub(1)), (true, w.s1, if k + 1 < p.windows.len() { Some(k + 1) } else { None })] {
        if let Some(o) = other {
            let (x, y, h) = c.al.point_at(s).unwrap();
            let (nx, ny) = (-h.sin(), h.cos());
            let half = (py1 - py0) / 2.0 / sc * 0.9;
            let cl = |q: [f64; 2]| [q[0].clamp(PX0, PX1), q[1].clamp(py0, py1)];
            let (qa, qb) = (cl(tp(x - half * nx, y - half * ny)), cl(tp(x + half * nx, y + half * ny)));
            d.line("TITLE", qa, qb);
            let lx = if end { qb[0] - 2.0 } else { qb[0] + 2.0 };
            d.text("TITLE", [lx.clamp(PX0 + 2.0, PX1 - 70.0), py1 - 8.0], 3.0, &format!("MATCH LINE STA {}", sta_label(s)));
            d.text("TITLE", [lx.clamp(PX0 + 2.0, PX1 - 70.0), py1 - 13.0], 2.5, &format!("SEE SHEET {}", ids[o]));
        }
    }
    // north arrow is drawn by the frame; plan scale note
    d.text("TEXT", [PX0 + 3.0, py0 + 3.0], 2.5, &format!("PLAN  {}   CONTOURS {} m, INDEX {} m", crate::units::scale(p.plan_scale), ci, 5.0 * ci));
    // profile band
    let (qy0, qy1) = (40.0, 290.0);
    d.rect("BORDER", PX0, qy0, PX1, qy1);
    let hs = sc;
    let vsc = 1000.0 / p.profile_v_scale;
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for s in &secs {
        lo = lo.min(s.cl_elev.min(s.ground_cl));
        hi = hi.max(s.cl_elev.max(s.ground_cl));
    }
    let gz = if us() { 2.0 * 0.3048 } else { 1.0 };
    let gz = if (hi - lo) * vsc > 120.0 { gz * 5.0 } else { gz };
    let z0 = ((lo + hi) / 2.0 - (qy1 - qy0 - 20.0) / 2.0 / vsc).floor();
    let x_of = |s: f64| PX0 + 20.0 + (s - w.s0) * hs;
    let y_of = |z: f64| qy0 + 10.0 + (z - z0) * vsc;
    let mut z = (z0 / gz).ceil() * gz;
    while y_of(z) < qy1 - 2.0 {
        d.line("GRID", [PX0, y_of(z)], [PX1, y_of(z)]);
        d.text("TEXT", [PX0 + 1.0, y_of(z) + 0.5], 2.0, &format!("{:.1}", ul(z)));
        z += gz;
    }
    let mut s = (w.s0 / step).ceil() * step;
    while s <= w.s1 + 1e-9 {
        d.line("GRID", [x_of(s), qy0], [x_of(s), qy1]);
        if ((s / (step * 5.0)) - (s / (step * 5.0)).round()).abs() < 1e-9 {
            d.text("TEXT", [x_of(s) - 6.0, qy0 - 5.0], 2.2, &sta_label(s));
        }
        s += step;
    }
    let n = ((w.s1 - w.s0) / 2.0).ceil().max(1.0) as usize;
    let mut eg = vec![];
    let mut fg = vec![];
    for i in 0..=n {
        let s = w.s0 + (w.s1 - w.s0) * i as f64 / n as f64;
        if let Some((x, y, _)) = c.al.point_at(s) {
            if let Some(z) = c.tin.elevation_at(x, y) {
                eg.push([x_of(s), y_of(z)]);
            }
        }
        if let Some(z) = c.pr.elevation_at(s) {
            fg.push([x_of(s), y_of(z)]);
        }
    }
    d.poly("PROFILE-GROUND", eg);
    d.poly("PROFILE-DESIGN", fg);
    for (i, v) in c.pr.input.vpis.iter().enumerate() {
        if v[0] >= w.s0 - 1e-6 && v[0] <= w.s1 + 1e-6 {
            let q = [x_of(v[0]), y_of(v[1])];
            d.rect("PROFILE-DESIGN", q[0] - 1.0, q[1] - 1.0, q[0] + 1.0, q[1] + 1.0);
            let l = if i > 0 && i + 1 < c.pr.input.vpis.len() { c.pr.input.curve_lengths[i - 1] } else { 0.0 };
            d.text("TEXT", [q[0] - 10.0, q[1] + 4.0], 2.0, &format!("VPI {} EL {:.2} m", sta_label(v[0]), v[1]));
            if l > 0.0 {
                d.text("TEXT", [q[0] - 10.0, q[1] + 7.5], 2.0, &format!("VC L = {:.1} m", l));
            }
        }
    }
    for g in c.pr.grades_pct.iter().enumerate() {
        let (a, b) = (c.pr.input.vpis[g.0], c.pr.input.vpis[g.0 + 1]);
        let m = (a[0] + b[0]) / 2.0;
        if m >= w.s0 && m <= w.s1 {
            d.text("TEXT", [x_of(m), y_of((a[1] + b[1]) / 2.0) + 10.0], 2.2, &format!("{:+.2}%", g.1));
        }
    }
    for (end, s) in [(false, w.s0), (true, w.s1)] {
        if (end && k + 1 < p.windows.len()) || (!end && k > 0) {
            d.line("TITLE", [x_of(s), qy0], [x_of(s), qy1]);
        }
    }
    d.text("TEXT", [PX0 + 3.0, qy1 + 3.0], 2.5, &format!("PROFILE  H {}  V {}", crate::units::scale(p.plan_scale), crate::units::scale(p.profile_v_scale)));
    crate::sheet::title_block_s(
        &mut d,
        &dummy_info(),
        &format!("Plan and profile STA {} to {}", sta_label(w.s0), sta_label(w.s1)),
        &crate::units::scale(p.plan_scale),
        0,
        0,
    );
    d
}

fn dummy_info() -> crate::sheet::SheetInfo {
    crate::sheet::SheetInfo {
        project: String::new(),
        engineer: String::new(),
        standard: String::new(),
        design_speed: 0.0,
        audit_head: String::new(),
        checks_failed: 0,
        checks_total: 0,
        approved_seq: None,
    }
}

/// Simple ruled table; returns the y below the table.
fn table(d: &mut Drawing, x0: f64, y_top: f64, widths: &[f64], rows: &[Vec<String>], h: f64) -> f64 {
    let rh = h * 2.0;
    let x1 = x0 + widths.iter().sum::<f64>();
    let mut y = y_top;
    for (i, r) in rows.iter().enumerate() {
        d.line("DETAIL", [x0, y], [x1, y]);
        let mut x = x0;
        for (j, c) in r.iter().enumerate() {
            d.text(if i == 0 { "TITLE" } else { "TEXT" }, [x + 1.0, y - rh + h * 0.55], h, c);
            x += widths.get(j).copied().unwrap_or(20.0);
        }
        y -= rh;
    }
    d.line("DETAIL", [x0, y], [x1, y]);
    let mut x = x0;
    for wdt in widths.iter().chain(std::iter::once(&0.0)) {
        d.line("DETAIL", [x, y_top], [x, y]);
        x += wdt;
    }
    y
}

/// Survey control, horizontal alignment PI table and curve data.
pub fn control_sheet(c: &Ctx) -> Drawing {
    let mut d = Drawing::default();
    layers(&mut d);
    let mut y = SHEET_H - 30.0;
    d.text("TITLE", [PX0, y + 5.0], 4.0, "SURVEY CONTROL");
    let mut rows =
        vec![vec!["POINT".to_string(), format!("NORTHING ({})", lu()), format!("EASTING ({})", lu()), format!("ELEV ({})", lu()), "DESCRIPTION".into()]];
    if c.inp.control.is_empty() {
        rows.push(vec!["-".into(), "NOT ENTERED".into(), "ENGINEER / SURVEYOR".into(), "INPUT REQUIRED".into(), "".into()]);
    }
    for p in &c.inp.control {
        rows.push(vec![p.id.clone(), format!("{:.3}", ul(p.n)), format!("{:.3}", ul(p.e)), format!("{:.3}", ul(p.z)), p.desc.clone()]);
    }
    y = table(&mut d, PX0, y, &[30.0, 45.0, 45.0, 30.0, 90.0], &rows, 2.5) - 15.0;
    d.text("TITLE", [PX0, y + 5.0], 4.0, "HORIZONTAL ALIGNMENT CURVE DATA");
    let mut rows =
        vec![["CURVE", "DIR", "DELTA", "RADIUS", "LS IN", "ARC L", "LS OUT", "TS/PC", "SC", "CS", "ST/PT"].iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for cv in c.al.curves() {
        let dms = {
            let a = cv.delta_deg.abs();
            let dd = a.floor();
            let mm = ((a - dd) * 60.0).floor();
            let ss = ((a - dd) * 60.0 - mm) * 60.0;
            format!("{dd:.0}°{mm:02.0}'{ss:04.1}\"")
        };
        rows.push(vec![
            format!("{}", cv.index + 1),
            if cv.dir > 0.0 { "LT".into() } else { "RT".into() },
            dms,
            format!("{:.3}", ul(cv.radius)),
            format!("{:.3}", ul(cv.ls_in)),
            format!("{:.3}", ul(cv.arc_len)),
            format!("{:.3}", ul(cv.ls_out)),
            sta_label(cv.sta_start),
            sta_label(cv.sta_cs_start),
            sta_label(cv.sta_cs_end),
            sta_label(cv.sta_end),
        ]);
    }
    y = table(&mut d, PX0, y, &[18.0, 12.0, 30.0, 25.0, 22.0, 25.0, 22.0, 30.0, 30.0, 30.0, 30.0], &rows, 2.5) - 15.0;
    d.text("TITLE", [PX0, y + 5.0], 4.0, "VERTICAL ALIGNMENT DATA");
    let mut rows = vec![["VPI", "STATION", "ELEV", "GRADE IN %", "GRADE OUT %", "VC LENGTH"].iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    let v = &c.pr.input.vpis;
    for (i, q) in v.iter().enumerate() {
        let gi = if i > 0 { format!("{:+.3}", c.pr.grades_pct[i - 1]) } else { "-".into() };
        let go = if i + 1 < v.len() { format!("{:+.3}", c.pr.grades_pct[i]) } else { "-".into() };
        let l = if i > 0 && i + 1 < v.len() { c.pr.input.curve_lengths[i - 1] } else { 0.0 };
        rows.push(vec![format!("{}", i + 1), sta_label(q[0]), format!("{:.3}", ul(q[1])), gi, go, format!("{:.3}", ul(l))]);
    }
    table(&mut d, PX0, y, &[15.0, 30.0, 30.0, 28.0, 28.0, 30.0], &rows, 2.5);
    d.text(
        "TEXT",
        [PX0, 30.0],
        2.5,
        &format!("Units: {}. Coordinates and elevations as entered by the engineer / surveyor; the agent does not establish control.", lu()),
    );
    crate::sheet::title_block_s(&mut d, &dummy_info(), "Survey control and alignment data", "NTS", 0, 0);
    d
}

/// Gridded cross sections at the standard interval, with ROW and construction limits.
pub fn cross_sections(c: &Ctx, p: &Plan) -> Vec<Drawing> {
    let per = 8; // 2 columns x 4 rows
    let secs: Vec<&Section> = p.xs_stations.iter().filter_map(|s| sec_at(c.cor, *s)).collect();
    let sc = 1000.0 / p.xs_scale;
    let (bw, bh) = (320.0, 125.0);
    let gs = if us() {
        5.0 * 0.3048
    } else if p.xs_scale <= 100.0 {
        1.0
    } else {
        2.0
    };
    let mut out = vec![];
    for (pg, chunk) in secs.chunks(per).enumerate() {
        let mut d = Drawing::default();
        layers(&mut d);
        for (i, s) in chunk.iter().enumerate() {
            let (col, row) = (i % 2, i / 2);
            let x0 = PX0 + 5.0 + col as f64 * (bw + 10.0);
            let y1 = SHEET_H - 25.0 - row as f64 * (bh + 12.0);
            let y0 = y1 - bh;
            let cx = x0 + bw / 2.0;
            let zmid = (s.cl_elev + s.ground_cl) / 2.0;
            let zb = ((zmid - (bh / 2.0) / sc) / gs).floor() * gs;
            let tx = |o: f64| cx + o * sc;
            let ty = |z: f64| y0 + (z - zb) * sc;
            // grid
            let mut o = -((bw / 2.0 / sc) / gs).floor() * gs;
            while tx(o) <= x0 + bw + 1e-6 {
                d.line("GRID", [tx(o), y0], [tx(o), y1]);
                o += gs;
            }
            let mut z = zb;
            while ty(z) <= y1 + 1e-6 {
                d.line("GRID", [x0, ty(z)], [x0 + bw, ty(z)]);
                if ((z / (5.0 * gs)) - (z / (5.0 * gs)).round()).abs() < 1e-6 {
                    d.text("TEXT", [x0 - 14.0, ty(z) - 1.0], 2.0, &format!("{:.0}", ul(z)));
                }
                z += gs;
            }
            d.rect("BORDER", x0, y0, x0 + bw, y1);
            let clip = |q: [f64; 2]| [q[0].clamp(x0, x0 + bw), q[1].clamp(y0, y1)];
            // ground: signed offset left +; drawn with left on the left side of the box
            d.poly("XS-GROUND", s.ground.iter().map(|q| clip([tx(-q[0]), ty(q[1])])).collect());
            let mut fg: Vec<[f64; 2]> = s.left.fg.iter().rev().map(|q| clip([tx(-q[0]), ty(q[1])])).collect();
            fg.extend(s.right.fg.iter().skip(1).map(|q| clip([tx(q[0]), ty(q[1])])));
            d.poly("XS-DESIGN", fg);
            let mut dt: Vec<[f64; 2]> = s.left.datum.iter().rev().map(|q| clip([tx(-q[0]), ty(q[1])])).collect();
            dt.extend(s.right.datum.iter().skip(1).map(|q| clip([tx(q[0]), ty(q[1])])));
            d.poly("XS-DATUM", dt);
            d.line("ALIGNMENT", [cx, y0], [cx, y1]);
            for (r, sgn, lab) in [(c.inp.row_left_m, -1.0, "R/W"), (c.inp.row_right_m, 1.0, "R/W")] {
                if let Some(r) = r {
                    let x = tx(sgn * r);
                    if x >= x0 && x <= x0 + bw {
                        d.line("ROW", [x, y0], [x, y1]);
                        d.text("ROW", [x + 0.8, y1 - 4.0], 2.0, lab);
                    }
                }
            }
            for (o, sgn) in [(s.left.daylight, -1.0), (s.right.daylight, 1.0)] {
                let x = tx(sgn * o);
                if x >= x0 && x <= x0 + bw {
                    d.line("DAYLIGHT", [x, y0], [x, y0 + 8.0]);
                    d.text("DAYLIGHT", [x + 0.8, y0 + 2.0], 1.8, "LIM");
                }
            }
            d.text("TITLE", [x0, y1 + 2.5], 2.8, &format!("STA {}", sta_label(s.station)));
            d.text(
                "TEXT",
                [x0 + 60.0, y1 + 2.5],
                2.2,
                &format!("FG CL {:.2} m  EG CL {:.2} m  CUT {:.2} m²  FILL {:.2} m²", s.cl_elev, s.ground_cl, s.cut_area, s.fill_area),
            );
        }
        d.text(
            "TEXT",
            [PX0, 15.0],
            2.5,
            &format!("CROSS SECTIONS  {} (H = V)  GRID {:.1} m  INTERVAL {:.2} m", crate::units::scale(p.xs_scale), gs, p.xs_interval_m),
        );
        crate::sheet::title_block_s(
            &mut d,
            &dummy_info(),
            &format!("Cross sections at standard interval ({} of {})", pg + 1, secs.len().div_ceil(per)),
            &crate::units::scale(p.xs_scale),
            0,
            0,
        );
        out.push(d);
    }
    out
}

/// Quantity summary by pay item, with earthwork split by plan sheet.
pub fn quantity_summary(c: &Ctx, p: &Plan, ids: &[String]) -> Drawing {
    let mut d = Drawing::default();
    layers(&mut d);
    let mut y = SHEET_H - 30.0;
    d.text("TITLE", [PX0, y + 5.0], 4.0, "SUMMARY OF QUANTITIES BY PAY ITEM");
    let mut rows = vec![vec!["ITEM".to_string(), "DESCRIPTION".into(), "UNIT".into(), "QUANTITY".into()]];
    for r in &c.boq {
        let (u, q) = crate::units::boq(&r.unit, r.qty);
        rows.push(vec![r.code.chars().take(28).collect(), r.item.chars().take(70).collect(), u, format!("{q:.1}")]);
    }
    y = table(&mut d, PX0, y, &[70.0, 190.0, 20.0, 35.0], &rows, 2.2) - 15.0;
    d.text("TITLE", [PX0, y + 5.0], 4.0, "EARTHWORK BY PLAN SHEET (END AREA)");
    let vu = crate::units::boq("m³", 0.0).0;
    let mut rows = vec![vec!["SHEET".to_string(), "FROM".into(), "TO".into(), format!("CUT ({vu})"), format!("FILL ({vu})")]];
    for (k, w) in p.windows.iter().enumerate() {
        let (mut cut, mut fill) = (0.0, 0.0);
        for pair in c.cor.sections.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let lo = a.station.max(w.s0);
            let hi = b.station.min(w.s1);
            if hi > lo {
                let f = (hi - lo) / (b.station - a.station);
                cut += (a.cut_area + b.cut_area) / 2.0 * (b.station - a.station) * f;
                fill += (a.fill_area + b.fill_area) / 2.0 * (b.station - a.station) * f;
            }
        }
        let (_, cq) = crate::units::boq("m³", cut);
        let (_, fq) = crate::units::boq("m³", fill);
        rows.push(vec![ids[k].clone(), sta_label(w.s0), sta_label(w.s1), format!("{cq:.1}"), format!("{fq:.1}")]);
    }
    table(&mut d, PX0, y, &[30.0, 35.0, 35.0, 35.0, 35.0], &rows, 2.2);
    d.text(
        "TEXT",
        [PX0, 20.0],
        2.5,
        "Pay item codes map the measurement-method references of boq.csv; the estimator confirms item numbers against the agency specification.",
    );
    crate::sheet::title_block_s(&mut d, &dummy_info(), "Summary of quantities", "NTS", 0, 0);
    d
}
