//! Plan & profile sheet (A1 landscape, millimetres) and a world-coordinate model drawing.
use crate::align::Alignment;
use crate::drawing::Drawing;
use crate::earthwork::Earthwork;
use crate::profile::Profile;
use crate::tin::Tin;

pub const SHEET_W: f64 = 841.0;
pub const SHEET_H: f64 = 594.0;

fn sample_al(al: &Alignment, step: f64) -> Vec<(f64, f64, f64, f64)> {
    let mut v = Vec::new();
    let mut s = al.start_station();
    while s < al.end_station() {
        let (x, y, h) = al.point_at(s).unwrap();
        v.push((s, x, y, h));
        s += step;
    }
    let (x, y, h) = al.point_at(al.end_station()).unwrap();
    v.push((al.end_station(), x, y, h));
    v
}

pub fn sta_label(s: f64) -> String {
    let k = (s / 1000.0).floor();
    format!("K{}+{:06.2}", k as i64, s - k * 1000.0)
}

/// World-coordinate plan model: contours, alignment, station ticks, daylight lines.
pub fn model(tin: &Tin, al: &Alignment, ew: Option<&Earthwork>, contour_interval: f64) -> Drawing {
    let mut d = Drawing::default();
    d.layer("CONTOUR-MINOR", 8);
    d.layer("CONTOUR-MAJOR", 30);
    d.layer("ALIGNMENT", 1);
    d.layer("STATION", 7);
    d.layer("DAYLIGHT", 3);
    for (lv, a, b) in tin.contours(contour_interval) {
        let major = ((lv / (contour_interval * 5.0)).round() * contour_interval * 5.0 - lv).abs() < 1e-6;
        d.line(if major { "CONTOUR-MAJOR" } else { "CONTOUR-MINOR" }, a, b);
    }
    let s = sample_al(al, 2.0);
    d.poly("ALIGNMENT", s.iter().map(|p| [p.1, p.2]).collect());
    let mut st = (al.start_station() / 100.0).ceil() * 100.0;
    while st <= al.end_station() {
        let (x, y, h) = al.point_at(st).unwrap();
        let (nx, ny) = (-h.sin(), h.cos());
        d.line("STATION", [x - 5.0 * nx, y - 5.0 * ny], [x + 5.0 * nx, y + 5.0 * ny]);
        d.text("STATION", [x + 7.0 * nx, y + 7.0 * ny], 4.0, &sta_label(st));
        st += 100.0;
    }
    if let Some(e) = ew {
        let l: Vec<[f64; 2]> = e.sections.iter().map(|s| [s.x - s.heading.sin() * s.daylight_left, s.y + s.heading.cos() * s.daylight_left]).collect();
        let r: Vec<[f64; 2]> = e.sections.iter().map(|s| [s.x + s.heading.sin() * s.daylight_right, s.y - s.heading.cos() * s.daylight_right]).collect();
        d.poly("DAYLIGHT", l);
        d.poly("DAYLIGHT", r);
    }
    d
}

pub struct SheetInfo<'a> {
    pub project: &'a str,
    pub engineer: &'a str,
    pub standard: &'a str,
    pub design_speed: f64,
    pub audit_head: &'a str,
    pub checks_failed: usize,
    pub checks_total: usize,
    pub cut: f64,
    pub fill: f64,
    pub approved: bool,
}

pub fn sheet(tin: &Tin, al: &Alignment, pr: &Profile, ew: Option<&Earthwork>, info: &SheetInfo) -> Drawing {
    let mut d = Drawing::default();
    for (l, c) in [("BORDER", 7), ("GRID", 9), ("CONTOUR-MINOR", 8), ("CONTOUR-MAJOR", 30), ("ALIGNMENT", 1), ("DAYLIGHT", 3), ("PROFILE-GROUND", 30), ("PROFILE-DESIGN", 1), ("TEXT", 7), ("TITLE", 7)] {
        d.layer(l, c);
    }
    d.rect("BORDER", 10.0, 10.0, SHEET_W - 10.0, SHEET_H - 10.0);
    // ---- plan viewport
    let (px0, py0, px1, py1) = (20.0, 300.0, SHEET_W - 20.0, SHEET_H - 20.0);
    d.rect("BORDER", px0, py0, px1, py1);
    let samp = sample_al(al, 2.0);
    let (mut mnx, mut mny, mut mxx, mut mxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &samp {
        mnx = mnx.min(p.1);
        mny = mny.min(p.2);
        mxx = mxx.max(p.1);
        mxy = mxy.max(p.2);
    }
    let pad = 60.0;
    let (mnx, mny, mxx, mxy) = (mnx - pad, mny - pad, mxx + pad, mxy + pad);
    // round scale to 1:N with N in {500,1000,2000,2500,5000,10000}
    let need = ((mxx - mnx) / (px1 - px0 - 10.0) * 1000.0).max((mxy - mny) / (py1 - py0 - 10.0) * 1000.0);
    let n = [500.0, 1000.0, 2000.0, 2500.0, 5000.0, 10000.0, 20000.0].into_iter().find(|&n| n >= need).unwrap_or(need.ceil());
    let sc = 1000.0 / n;
    let (cx, cy) = ((mnx + mxx) / 2.0, (mny + mxy) / 2.0);
    let (vx, vy) = ((px0 + px1) / 2.0, (py0 + py1) / 2.0);
    let tp = |x: f64, y: f64| [vx + (x - cx) * sc, vy + (y - cy) * sc];
    let inside = |p: [f64; 2]| p[0] > px0 && p[0] < px1 && p[1] > py0 && p[1] < py1;
    for (lv, a, b) in tin.contours(1.0) {
        let (pa, pb) = (tp(a[0], a[1]), tp(b[0], b[1]));
        if inside(pa) && inside(pb) {
            let major = (lv / 5.0 - (lv / 5.0).round()).abs() < 1e-6;
            d.line(if major { "CONTOUR-MAJOR" } else { "CONTOUR-MINOR" }, pa, pb);
        }
    }
    d.poly("ALIGNMENT", samp.iter().map(|p| tp(p.1, p.2)).collect());
    if let Some(e) = ew {
        d.poly("DAYLIGHT", e.sections.iter().map(|s| tp(s.x - s.heading.sin() * s.daylight_left, s.y + s.heading.cos() * s.daylight_left)).collect());
        d.poly("DAYLIGHT", e.sections.iter().map(|s| tp(s.x + s.heading.sin() * s.daylight_right, s.y - s.heading.cos() * s.daylight_right)).collect());
    }
    let mut st = (al.start_station() / 100.0).ceil() * 100.0;
    while st <= al.end_station() + 1e-9 {
        let (x, y, h) = al.point_at(st).unwrap();
        let (nx, ny) = (-h.sin(), h.cos());
        d.line("ALIGNMENT", tp(x - 4.0 / sc * nx, y - 4.0 / sc * ny), tp(x + 4.0 / sc * nx, y + 4.0 / sc * ny));
        d.text("TEXT", tp(x + 6.0 / sc * nx, y + 6.0 / sc * ny), 2.5, &sta_label(st));
        st += 100.0;
    }
    for (pi, r, l, defl) in al.curves() {
        let p = al.input.pis[pi];
        d.text("TEXT", tp(p[0], p[1]), 2.5, &format!("PI{pi} R={r:.0} L={l:.1} D={defl:.2}deg"));
    }
    d.text("TEXT", [px0 + 4.0, py1 - 7.0], 4.0, &format!("PLAN  1:{n:.0}   contours 1 m (major 5 m)   N up"));
    // north arrow
    d.poly("TEXT", vec![[px1 - 15.0, py1 - 25.0], [px1 - 12.0, py1 - 10.0], [px1 - 9.0, py1 - 25.0]]);
    // ---- profile viewport
    let (qx0, qy0, qx1, qy1) = (20.0, 20.0, 690.0, 290.0);
    d.rect("BORDER", qx0, qy0, qx1, qy1);
    let (s0, s1) = (al.start_station(), al.end_station());
    let mut zs = Vec::new();
    let mut g = Vec::new();
    let mut fg = Vec::new();
    let mut s = s0;
    while s <= s1 + 1e-9 {
        let (x, y, _) = al.point_at(s.min(s1)).unwrap();
        if let Some(z) = tin.elevation_at(x, y) {
            g.push((s, z));
            zs.push(z)
        }
        if let Some(z) = pr.elevation_at(s.min(s1)) {
            fg.push((s, z));
            zs.push(z)
        }
        s += 2.0;
    }
    let zmin = (zs.iter().cloned().fold(f64::MAX, f64::min) / 10.0).floor() * 10.0 - 10.0;
    let zmax = (zs.iter().cloned().fold(f64::MIN, f64::max) / 10.0).ceil() * 10.0 + 10.0;
    let hs = (qx1 - qx0 - 40.0) / (s1 - s0);
    let vs = ((qy1 - qy0 - 30.0) / (zmax - zmin)).min(hs * 10.0);
    let tq = |s: f64, z: f64| [qx0 + 30.0 + (s - s0) * hs, qy0 + 15.0 + (z - zmin) * vs];
    let mut z = zmin;
    while z <= zmax + 1e-9 {
        d.line("GRID", tq(s0, z), tq(s1, z));
        d.text("TEXT", [qx0 + 3.0, tq(s0, z)[1] - 1.0], 2.0, &format!("{z:.0}"));
        z += 10.0;
    }
    let mut st = (s0 / 100.0).ceil() * 100.0;
    while st <= s1 + 1e-9 {
        d.line("GRID", tq(st, zmin), tq(st, zmax));
        d.text("TEXT", [tq(st, zmin)[0] - 8.0, qy0 + 5.0], 2.0, &sta_label(st));
        st += 100.0;
    }
    d.poly("PROFILE-GROUND", g.iter().map(|p| tq(p.0, p.1)).collect());
    d.poly("PROFILE-DESIGN", fg.iter().map(|p| tq(p.0, p.1)).collect());
    for (i, gr) in pr.grades_pct.iter().enumerate() {
        let (a, b) = (pr.input.vpis[i], pr.input.vpis[i + 1]);
        let m = tq((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
        d.text("TEXT", [m[0], m[1] + 4.0], 2.2, &format!("{gr:+.2}%"));
    }
    for c in &pr.curves {
        let v = pr.input.vpis[c.vpi_index];
        let p = tq(v[0], v[1]);
        d.text("TEXT", [p[0] - 10.0, p[1] + 8.0], 2.2, &format!("VPI {} EL {:.2} L={:.0} K={:.1}", sta_label(v[0]), v[1], c.length, c.k));
    }
    d.text("TEXT", [qx0 + 4.0, qy1 - 7.0], 4.0, &format!("PROFILE  H 1:{:.0}  V 1:{:.0}  (ground brown, design red)", 1000.0 / hs, 1000.0 / vs));
    // ---- title block
    let (tx0, ty0, tx1, ty1) = (700.0, 20.0, SHEET_W - 20.0, 290.0);
    d.rect("TITLE", tx0, ty0, tx1, ty1);
    let mut y = ty1 - 6.0;
    let mut t = |d: &mut Drawing, s: &str, h: f64| {
        y -= h;
        d.text("TITLE", [tx0 + 4.0, y], h, s);
        y -= h * 0.8;
    };
    t(&mut d, info.project, 4.0);
    t(&mut d, &format!("Std: {}", info.standard), 2.5);
    t(&mut d, &format!("Design speed: {:.0} km/h", info.design_speed), 2.5);
    t(&mut d, &format!("Checks: {}/{} pass", info.checks_total - info.checks_failed, info.checks_total), 2.5);
    t(&mut d, &format!("Cut {:.0} m3  Fill {:.0} m3", info.cut, info.fill), 2.5);
    t(&mut d, "Audit log head:", 2.5);
    t(&mut d, &info.audit_head[..32.min(info.audit_head.len())], 2.0);
    t(&mut d, &info.audit_head[32.min(info.audit_head.len())..], 2.0);
    t(&mut d, "Prepared by: civil-mcp agent tools", 2.5);
    t(&mut d, &format!("Engineer in responsible charge: {}", info.engineer), 2.2);
    if !info.approved {
        t(&mut d, "PRELIMINARY", 6.0);
        t(&mut d, "NOT FOR CONSTRUCTION", 4.0);
        t(&mut d, "Not reviewed or sealed by", 2.5);
        t(&mut d, "a licensed engineer.", 2.5);
    } else {
        t(&mut d, "Design approved in audit log;", 2.5);
        t(&mut d, "seal applied by engineer only.", 2.5);
    }
    d.rect("TITLE", tx0 + 30.0, ty0 + 10.0, tx1 - 10.0, ty0 + 70.0);
    d.text("TITLE", [tx0 + 35.0, ty0 + 12.0], 2.2, "ENGINEER SEAL / SIGNATURE");
    d
}
