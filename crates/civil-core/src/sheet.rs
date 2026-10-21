//! Sheets (A1 landscape, millimetres): plan/profile, cross sections, mass haul
//! and quantities; plus a world-coordinate model drawing.
use crate::align::Alignment;
use crate::corridor::{Corridor, Section};
use crate::drawing::Drawing;
use crate::grading::Grading;
use crate::intersect::Intersection;
use crate::pipes::Network;
use crate::profile::Profile;
use crate::quantities::{MassHaul, MaterialRow};
use crate::tin::Tin;

pub const SHEET_W: f64 = 841.0;
pub const SHEET_H: f64 = 594.0;

pub fn sta_label(s: f64) -> String {
    // SI: km + metres (0+123.45); US customary: 100 ft stations (12+34.56)
    if crate::units::cur().us() {
        return crate::units::sta(s);
    }
    let k = (s / 1000.0).floor();
    format!("{}+{:06.2}", k as i64, s - k * 1000.0)
}

use crate::units::len as ul;

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

#[derive(Clone)]
pub struct SheetInfo {
    pub project: String,
    pub engineer: String,
    pub standard: String,
    pub design_speed: f64,
    pub audit_head: String,
    pub checks_failed: usize,
    pub checks_total: usize,
    /// Some(seq) when the engineer recorded an approval in the audit log.
    pub approved_seq: Option<u64>,
}

pub struct Design<'a> {
    pub tin: &'a Tin,
    pub al: &'a Alignment,
    pub pr: &'a Profile,
    pub corridor: Option<&'a Corridor>,
    pub networks: &'a [Network],
    pub gradings: &'a [Grading],
    pub intersections: &'a [Intersection],
}

pub(crate) fn base(d: &mut Drawing) {
    d.units_mm = true;
    for (l, c) in [
        ("BORDER", 7),
        ("GRID", 9),
        ("CONTOUR-MINOR", 8),
        ("CONTOUR-MAJOR", 30),
        ("ALIGNMENT", 1),
        ("DAYLIGHT", 3),
        ("PROFILE-GROUND", 30),
        ("PROFILE-DESIGN", 1),
        ("XS-GROUND", 30),
        ("XS-DESIGN", 1),
        ("XS-DATUM", 3),
        ("PIPE", 5),
        ("STRUCTURE", 5),
        ("GRADING", 6),
        ("WALL", 4),
        ("CURB", 2),
        ("INTERSECTION", 6),
        ("SIGHT", 140),
        ("HGL", 4),
        ("EGL", 1),
        ("TEXT", 7),
        ("TITLE", 7),
        ("STAMP", 1),
    ] {
        d.layer(l, c);
    }
}

/// Greedy wrap by estimated width in ems (CJK = 1 em, other = 0.55 em).
pub(crate) fn wrap(s: &str, max_em: f64) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut w = 0.0;
    for word in s.split_inclusive(' ') {
        let ww: f64 = word.chars().map(|c| if (c as u32) > 0x2E80 { 1.0 } else { 0.62 }).sum();
        if w + ww > max_em && !out.last().unwrap().is_empty() {
            out.push(String::new());
            w = 0.0;
        }
        out.last_mut().unwrap().push_str(word);
        w += ww;
    }
    out.into_iter().map(|l| l.trim_end().to_string()).collect()
}

pub(crate) fn title_block(d: &mut Drawing, info: &SheetInfo, sheet_title: &str, page: usize, pages: usize) {
    title_block_s(d, info, sheet_title, "AS SHOWN", page, pages)
}

/// Records the sheet identity; the frame, title block, stamp and seal box are drawn by
/// `frame::compose` so that the same layout can be put on any sheet size and drafting standard.
pub(crate) fn title_block_s(d: &mut Drawing, _info: &SheetInfo, sheet_title: &str, scale: &str, page: usize, pages: usize) {
    d.meta = Some(crate::drawing::SheetMeta { title: sheet_title.into(), scale: scale.into(), page, pages });
}

pub fn plan_profile(ds: &Design, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    let (tin, al, pr) = (ds.tin, ds.al, ds.pr);
    let (px0, py0, px1, py1) = (20.0, 300.0, SHEET_W - 160.0, SHEET_H - 20.0);
    d.rect("BORDER", px0, py0, px1, py1);
    let samp = sample_al(al, 2.0);
    let (mut mnx, mut mny, mut mxx, mut mxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &samp {
        mnx = mnx.min(p.1);
        mny = mny.min(p.2);
        mxx = mxx.max(p.1);
        mxy = mxy.max(p.2);
    }
    let pad = 40.0;
    let (mnx, mny, mxx, mxy) = (mnx - pad, mny - pad, mxx + pad, mxy + pad);
    let need = ((mxx - mnx) / (px1 - px0 - 10.0) * 1000.0).max((mxy - mny) / (py1 - py0 - 10.0) * 1000.0);
    let n = crate::units::pick_scale(need, &[500.0, 1000.0, 2000.0, 2500.0, 3000.0, 4000.0, 5000.0, 10000.0, 20000.0], crate::units::US_PLAN_FT_PER_IN);
    let sc = 1000.0 / n;
    let (cx, cy) = ((mnx + mxx) / 2.0, (mny + mxy) / 2.0);
    let (vx, vy) = ((px0 + px1) / 2.0, (py0 + py1) / 2.0);
    let tp = |x: f64, y: f64| [vx + (x - cx) * sc, vy + (y - cy) * sc];
    let inside = |p: [f64; 2]| p[0] > px0 && p[0] < px1 && p[1] > py0 && p[1] < py1;
    let us = crate::units::cur().us();
    let (ci, cmaj) = if us { (2.0 * crate::units::lf(), 5.0) } else { (1.0, 5.0) };
    for (lv, a, b) in tin.contours(ci) {
        let (pa, pb) = (tp(a[0], a[1]), tp(b[0], b[1]));
        if inside(pa) && inside(pb) {
            let q = lv / ci / cmaj;
            let major = (q - q.round()).abs() < 1e-6;
            d.line(if major { "CONTOUR-MAJOR" } else { "CONTOUR-MINOR" }, pa, pb);
        }
    }
    if let Some(c) = ds.corridor {
        let edge = |s: &Section, left: bool, off: f64| {
            let (nx, ny) = (-s.heading.sin(), s.heading.cos());
            let o = if left { off } else { -off };
            tp(s.x + o * nx, s.y + o * ny)
        };
        d.poly("DAYLIGHT", c.sections.iter().map(|s| edge(s, true, s.left.daylight)).collect());
        d.poly("DAYLIGHT", c.sections.iter().map(|s| edge(s, false, s.right.daylight)).collect());
        d.poly("CURB", c.sections.iter().map(|s| edge(s, true, s.left.paved_width)).collect());
        d.poly("CURB", c.sections.iter().map(|s| edge(s, false, s.right.paved_width)).collect());
        wall_runs(&mut d, c, &tp);
    }
    for x in ds.intersections {
        plan_intersection(&mut d, x, &tp, true);
    }
    for g in ds.gradings {
        let mut f: Vec<[f64; 2]> = g.footprint.iter().map(|p| tp(p[0], p[1])).collect();
        f.push(f[0]);
        d.poly("GRADING", f);
        let mut dl: Vec<[f64; 2]> = g.daylight.iter().map(|p| tp(p[0], p[1])).collect();
        dl.push(dl[0]);
        d.poly("GRADING", dl);
    }
    for net in ds.networks {
        for p in &net.pipes {
            let a = net.structures.iter().find(|s| s.0 == p.from).unwrap();
            let b = net.structures.iter().find(|s| s.0 == p.to).unwrap();
            d.line("PIPE", tp(a.1, a.2), tp(b.1, b.2));
            let m = tp((a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0);
            d.text("PIPE", [m[0] + 1.5, m[1] + 1.5], 1.8, &format!("{} D{} S{:.2}%", p.id, crate::units::dia(p.diameter_m), p.slope * 100.0));
        }
        for s in &net.structures {
            let c = tp(s.1, s.2);
            d.rect("STRUCTURE", c[0] - 1.0, c[1] - 1.0, c[0] + 1.0, c[1] + 1.0);
            d.text("STRUCTURE", [c[0] + 1.5, c[1] - 3.0], 1.8, &s.0);
        }
    }
    d.poly("ALIGNMENT", samp.iter().map(|p| tp(p.1, p.2)).collect());
    let sstep = crate::units::sta_step();
    let mut st = (al.start_station() / sstep).ceil() * sstep;
    while st <= al.end_station() + 1e-9 {
        let (x, y, h) = al.point_at(st).unwrap();
        let (nx, ny) = (-h.sin(), h.cos());
        d.line("ALIGNMENT", tp(x - 3.0 / sc * nx, y - 3.0 / sc * ny), tp(x + 3.0 / sc * nx, y + 3.0 / sc * ny));
        d.text("TEXT", tp(x + 5.0 / sc * nx, y + 5.0 / sc * ny), 2.2, &sta_label(st));
        st += sstep;
    }
    for c in al.curves() {
        let (x, y, _) = al.point_at((c.sta_start + c.sta_end) / 2.0).unwrap();
        let p = tp(x, y);
        let txt = if c.ls_in > 0.0 {
            format!("JD{} R={:.0} Ls={:.0} α={:.2}°", c.index, ul(c.radius), ul(c.ls_in), c.delta_deg.abs())
        } else {
            format!("JD{} R={:.0} L={:.1} α={:.2}°", c.index, ul(c.radius), ul(c.arc_len), c.delta_deg.abs())
        };
        d.text("TEXT", [p[0] + 3.0, p[1] - 6.0], 2.2, &txt);
    }
    d.text(
        "TEXT",
        [px0 + 4.0, py1 - 7.0],
        4.0,
        &if us {
            format!("PLAN  {}   contours 2 ft (index 10 ft)  N↑", crate::units::scale(n))
        } else {
            format!("PLAN  1:{n:.0}   contours 1 m (index 5 m)  N↑")
        },
    );
    d.poly("TEXT", vec![[px1 - 15.0, py1 - 25.0], [px1 - 12.0, py1 - 10.0], [px1 - 9.0, py1 - 25.0]]);
    // profile
    let (qx0, qy0, qx1, qy1) = (20.0, 20.0, SHEET_W - 160.0, 290.0);
    d.rect("BORDER", qx0, qy0, qx1, qy1);
    let (s0, s1) = (al.start_station(), al.end_station());
    let mut zs = Vec::new();
    let (mut g, mut fg) = (Vec::new(), Vec::new());
    let mut s = s0;
    while s <= s1 + 1e-9 {
        let ss = s.min(s1);
        let (x, y, _) = al.point_at(ss).unwrap();
        if let Some(z) = tin.elevation_at(x, y) {
            g.push((ss, z));
            zs.push(z)
        }
        if let Some(z) = pr.elevation_at(ss) {
            fg.push((ss, z));
            zs.push(z)
        }
        s += 2.0;
    }
    let lf = crate::units::lf();
    let zg = 10.0 * lf;
    let zmin = (zs.iter().cloned().fold(f64::MAX, f64::min) / zg).floor() * zg - zg;
    let zmax = (zs.iter().cloned().fold(f64::MIN, f64::max) / zg).ceil() * zg + zg;
    let mut hs = (qx1 - qx0 - 40.0) / (s1 - s0);
    let mut vs = ((qy1 - qy0 - 40.0) / (zmax - zmin)).min(hs * 10.0);
    if us {
        // snap to engineering scales (1" = 50' H, 1" = 5' V ...)
        hs = 1000.0 / crate::units::pick_scale(1000.0 / hs, &[], crate::units::US_PLAN_FT_PER_IN);
        vs = 1000.0 / crate::units::pick_scale(1000.0 / vs, &[], crate::units::US_VERT_FT_PER_IN);
    }
    let tq = |s: f64, z: f64| [qx0 + 30.0 + (s - s0) * hs, qy0 + 25.0 + (z - zmin) * vs];
    let mut z = zmin;
    while z <= zmax + 1e-9 {
        d.line("GRID", tq(s0, z), tq(s1, z));
        d.text("TEXT", [qx0 + 3.0, tq(s0, z)[1] - 1.0], 2.0, &format!("{:.0}", ul(z)));
        z += zg;
    }
    let mut st = (s0 / sstep).ceil() * sstep;
    while st <= s1 + 1e-9 {
        d.line("GRID", tq(st, zmin), tq(st, zmax));
        d.text("TEXT", [tq(st, zmin)[0] - 8.0, qy0 + 14.0], 2.0, &sta_label(st));
        if let (Some(zd), Some(zg)) = (pr.elevation_at(st), al.point_at(st).and_then(|(x, y, _)| tin.elevation_at(x, y))) {
            d.text("TEXT", [tq(st, zmin)[0] - 8.0, qy0 + 9.0], 1.8, &format!("FG {:.2}", ul(zd)));
            d.text("TEXT", [tq(st, zmin)[0] - 8.0, qy0 + 5.0], 1.8, &format!("EG {:.2}", ul(zg)));
        }
        st += sstep;
    }
    d.poly("PROFILE-GROUND", g.iter().map(|p| tq(p.0, p.1)).collect());
    d.poly("PROFILE-DESIGN", fg.iter().map(|p| tq(p.0, p.1)).collect());
    for (i, gr) in pr.grades_pct.iter().enumerate() {
        let (a, b) = (pr.input.vpis[i], pr.input.vpis[i + 1]);
        let m = tq((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
        d.text("TEXT", [m[0], m[1] + 4.0], 2.2, &format!("i={gr:+.2}% L={:.0}", ul(b[0] - a[0])));
    }
    for c in &pr.curves {
        let v = pr.input.vpis[c.vpi_index];
        let p = tq(v[0], v[1]);
        d.poly("PROFILE-DESIGN", vec![[p[0], p[1]], [p[0], p[1] + 12.0]]);
        d.text("TEXT", [p[0] - 10.0, p[1] + 14.0], 2.0, &format!("VPI {} EL {:.2}", sta_label(v[0]), ul(v[1])));
        d.text(
            "TEXT",
            [p[0] - 10.0, p[1] + 17.0],
            2.0,
            &format!("{} R={:.0} L={:.0}", if c.crest { "CREST" } else { "SAG" }, ul(c.length / (c.a_pct.abs() / 100.0).max(1e-9)), ul(c.length)),
        );
    }
    d.text(
        "TEXT",
        [qx0 + 4.0, qy1 - 7.0],
        4.0,
        &format!("PROFILE  H {}  V {}  (existing ground brown, finished grade red)", crate::units::scale(1000.0 / hs), crate::units::scale(1000.0 / vs)),
    );
    title_block_s(&mut d, info, "Plan and profile", &format!("plan 1:{n:.0}; profile as shown"), page, pages);
    d
}

/// Wall face polylines (continuous runs of sections that carry a wall on one side).
fn wall_runs(d: &mut Drawing, c: &Corridor, tp: &dyn Fn(f64, f64) -> [f64; 2]) {
    for left in [true, false] {
        let mut run: Vec<[f64; 2]> = Vec::new();
        let flush = |d: &mut Drawing, run: &mut Vec<[f64; 2]>| {
            if run.len() == 1 {
                let p = run[0];
                d.rect("WALL", p[0] - 0.6, p[1] - 0.6, p[0] + 0.6, p[1] + 0.6);
            } else if run.len() > 1 {
                d.poly("WALL", run.clone());
            }
            run.clear();
        };
        for s in &c.sections {
            let sd = if left { &s.left } else { &s.right };
            match &sd.wall {
                Some(w) => {
                    let o = if left { w.offset } else { -w.offset };
                    run.push(tp(s.x - s.heading.sin() * o, s.y + s.heading.cos() * o));
                }
                None => flush(d, &mut run),
            }
        }
        flush(d, &mut run);
    }
}

fn plan_intersection(d: &mut Drawing, x: &Intersection, tp: &dyn Fn(f64, f64) -> [f64; 2], label: bool) {
    for l in &x.legs {
        d.line("INTERSECTION", tp(l.edge_left[0][0], l.edge_left[0][1]), tp(l.edge_left[1][0], l.edge_left[1][1]));
        d.line("INTERSECTION", tp(l.edge_right[0][0], l.edge_right[0][1]), tp(l.edge_right[1][0], l.edge_right[1][1]));
        d.line("ALIGNMENT", tp(l.cl[0][0], l.cl[0][1]), tp(l.cl[1][0], l.cl[1][1]));
        for r in &l.returns {
            d.poly("INTERSECTION", r.arc.iter().map(|p| tp(p[0], p[1])).collect());
        }
        for s in &l.sight {
            let t = &s.triangle;
            d.poly("SIGHT", vec![tp(t[0][0], t[0][1]), tp(t[1][0], t[1][1]), tp(t[2][0], t[2][1]), tp(t[0][0], t[0][1])]);
        }
        if label {
            let e = l.cl[1];
            d.text(
                "TEXT",
                tp(e[0], e[1]),
                2.0,
                &format!("{} R={}", l.name, l.returns.iter().map(|r| format!("{:.0}", r.radius)).collect::<Vec<_>>().join("/")),
            );
        }
    }
    if label {
        let p = tp(x.point[0], x.point[1]);
        d.text("TEXT", [p[0] + 3.0, p[1] + 3.0], 2.2, &format!("{} {} at-grade intersection", x.name, sta_label(x.main_station)));
    }
}

/// Pipe profile: longest upstream→outfall path per network, rim, pipe crown/invert, HGL, EGL.
pub fn pipe_profile(net: &Network, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    // longest path by pipe count, following from → to
    let down = |id: &str| net.pipes.iter().find(|p| p.from == id);
    let mut best: Vec<&crate::pipes::PipeResult> = vec![];
    for s in &net.structures {
        if net.pipes.iter().any(|p| p.to == s.0) {
            continue;
        }
        let mut path = vec![];
        let mut cur = s.0.clone();
        while let Some(p) = down(&cur) {
            path.push(p);
            cur = p.to.clone();
            if path.len() > net.pipes.len() {
                break;
            }
        }
        let len = |v: &Vec<&crate::pipes::PipeResult>| v.iter().map(|p| p.length).sum::<f64>();
        if len(&path) > len(&best) {
            best = path;
        }
    }
    let (x0, y0, x1, y1) = (20.0, 60.0, SHEET_W - 160.0, SHEET_H - 20.0);
    d.rect("BORDER", x0, y0, x1, y1);
    let hyd = |id: &str| net.hydraulics.iter().find(|h| h.id == id);
    let mut ch = vec![0.0];
    for p in &best {
        ch.push(ch.last().unwrap() + p.length);
    }
    let total = ch.last().cloned().unwrap_or(1.0).max(1.0);
    let mut zs: Vec<f64> = vec![];
    for p in &best {
        zs.extend([p.invert_up, p.invert_dn, p.hgl_up, p.egl_up, p.hgl_dn]);
        for id in [&p.from, &p.to] {
            if let Some(h) = hyd(id) {
                zs.extend([h.rim, h.egl]);
            }
        }
    }
    let zlo = (zs.iter().cloned().fold(f64::MAX, f64::min) - 1.0).floor();
    let zhi = (zs.iter().cloned().fold(f64::MIN, f64::max) + 1.0).ceil();
    let hs = (x1 - x0 - 50.0) / total;
    let vs = (y1 - y0 - 50.0) / (zhi - zlo);
    let tp = |c: f64, z: f64| [x0 + 35.0 + c * hs, y0 + 30.0 + (z - zlo) * vs];
    let lfp = crate::units::lf();
    let zstep = if crate::units::cur().us() {
        if (zhi - zlo) / lfp > 60.0 {
            10.0 * lfp
        } else {
            2.0 * lfp
        }
    } else if zhi - zlo > 20.0 {
        5.0
    } else {
        1.0
    };
    let mut z = if crate::units::cur().us() { (zlo / zstep).ceil() * zstep } else { zlo };
    while z <= zhi + 1e-9 {
        d.line("GRID", tp(0.0, z), tp(total, z));
        d.text("TEXT", [x0 + 3.0, tp(0.0, z)[1] - 1.0], 2.0, &format!("{:.0}", ul(z)));
        z += zstep;
    }
    let mut rim = vec![];
    for (i, p) in best.iter().enumerate() {
        let (ca, cb) = (ch[i], ch[i + 1]);
        d.poly(
            "PIPE",
            vec![tp(ca, p.invert_up), tp(cb, p.invert_dn), tp(cb, p.invert_dn + p.diameter_m), tp(ca, p.invert_up + p.diameter_m), tp(ca, p.invert_up)],
        );
        d.line("HGL", tp(ca, p.hgl_up), tp(cb, p.hgl_dn));
        d.line("EGL", tp(ca, p.egl_up), tp(cb, p.egl_dn));
        let m = tp((ca + cb) / 2.0, p.invert_up.min(p.invert_dn));
        d.text(
            "PIPE",
            [m[0] - 12.0, m[1] - 6.0],
            1.8,
            &format!(
                "{} {}{} i={:.2}% L={:.1}",
                p.id,
                if crate::units::cur().us() { "D" } else { "DN" },
                crate::units::dia(p.diameter_m),
                p.slope * 100.0,
                ul(p.length)
            ),
        );
        d.text(
            "PIPE",
            [m[0] - 12.0, m[1] - 9.5],
            1.6,
            &if crate::units::cur().us() {
                format!("Q={:.2} cfs v={:.2} ft/s {}", p.q_design / 0.3048f64.powi(3), p.v_design / 0.3048, p.regime)
            } else {
                format!("Q={:.1} L/s v={:.2} m/s {}", p.q_design * 1000.0, p.v_design, p.regime)
            },
        );
        for (c, id) in [(ca, &p.from), (cb, &p.to)] {
            if let Some(h) = hyd(id) {
                rim.push(tp(c, h.rim));
                d.line("STRUCTURE", tp(c, h.rim), tp(c, p.invert_up.min(p.invert_dn) - 0.3));
                d.line("HGL", [tp(c, h.hgl)[0] - 1.5, tp(c, h.hgl)[1]], [tp(c, h.hgl)[0] + 1.5, tp(c, h.hgl)[1]]);
                if c == ca {
                    d.text(
                        "STRUCTURE",
                        [tp(c, h.rim)[0] - 2.0, tp(c, h.rim)[1] + 2.0],
                        1.8,
                        &format!("{} RIM {:.2} HGL {:.2} K={:.2}", id, ul(h.rim), ul(h.hgl), h.k),
                    );
                }
            }
        }
    }
    if let Some(p) = best.last() {
        if let Some(h) = hyd(&p.to) {
            let c = total;
            d.text("STRUCTURE", [tp(c, h.rim)[0] - 32.0, tp(c, h.rim)[1] + 2.0], 1.8, &format!("{} OUTFALL HGL {:.2}", p.to, ul(h.hgl)));
        }
    }
    d.poly("PROFILE-GROUND", rim);
    d.text(
        "TEXT",
        [x0 + 4.0, y1 - 7.0],
        4.0,
        &format!("PIPE PROFILE - {}  H {} V {}", net.name, crate::units::scale(1000.0 / hs), crate::units::scale(1000.0 / vs)),
    );
    d.text("TEXT", [x0 + 4.0, y1 - 13.0], 2.2, &format!("cyan = HGL, red = EGL, brown = rim; method {}", net.hgl_method));
    let mut y = y0 - 8.0;
    for c in net.checks.iter().filter(|c| !c.pass) {
        d.text(
            "TEXT",
            [x0, y],
            2.0,
            &format!(
                "{} {} {} {}: {:.3} vs {} {:.3} {}",
                c.id,
                if c.severity == "mandatory" { "mandatory" } else { "advisory" },
                c.element,
                c.item,
                c.value,
                c.kind,
                c.limit,
                c.unit
            ),
        );
        y -= 4.0;
        if y < 14.0 {
            break;
        }
    }
    title_block(&mut d, info, &format!("Pipe profile {}", net.name), page, pages);
    d
}

/// Cross-section sheets: 4 columns × 3 rows per sheet, uniform scale.
pub fn cross_sections(c: &Corridor, info: &SheetInfo, first_page: usize, pages: usize) -> Vec<Drawing> {
    let per = 12;
    let mut out = Vec::new();
    let (cw, ch) = ((SHEET_W - 180.0) / 4.0, (SHEET_H - 40.0) / 3.0);
    for (pi, chunk) in c.sections.chunks(per).enumerate() {
        // scale chosen per sheet from the sections on it (true 1:1 H:V)
        let (mut wmax, mut hmax): (f64, f64) = (10.0, 5.0);
        for s in chunk {
            let w = s.left.daylight.max(s.right.daylight) + 3.0;
            wmax = wmax.max(w);
            let zs: Vec<f64> =
                s.left.fg.iter().chain(s.right.fg.iter()).map(|p| p[1]).chain(s.ground.iter().filter(|p| p[0].abs() <= w).map(|p| p[1])).collect();
            let (lo, hi) = zs.iter().fold((f64::MAX, f64::MIN), |a, z| (a.0.min(*z), a.1.max(*z)));
            hmax = hmax.max(hi - lo + 3.0);
        }
        let scale_h = ((cw - 10.0) / (2.0 * wmax)).min((ch - 26.0) / hmax);
        let nice = if crate::units::cur().us() {
            crate::units::pick_scale(1000.0 / scale_h, &[], &[1.0, 2.0, 4.0, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0, 100.0, 200.0])
        } else {
            [50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 400.0, 500.0, 750.0, 1000.0, 1500.0, 2000.0].into_iter().find(|n| 1000.0 / n <= scale_h).unwrap_or(2000.0)
        };
        let sc = 1000.0 / nice;
        let mut d = Drawing::default();
        base(&mut d);
        for (i, s) in chunk.iter().enumerate() {
            let (col, row) = (i % 4, i / 4);
            let (x0, y1) = (20.0 + col as f64 * cw, SHEET_H - 20.0 - row as f64 * ch);
            let y0 = y1 - ch;
            d.rect("GRID", x0 + 2.0, y0 + 2.0, x0 + cw - 2.0, y1 - 2.0);
            let zlo = s
                .left
                .fg
                .iter()
                .chain(s.right.fg.iter())
                .map(|p| p[1])
                .chain(s.ground.iter().filter(|p| p[0].abs() <= wmax).map(|p| p[1]))
                .fold(f64::MAX, f64::min)
                - 1.0;
            let xc = x0 + cw / 2.0;
            let tp = |o: f64, z: f64| [xc + o * sc, y0 + 14.0 + (z - zlo) * sc];
            // cut / fill fills between datum and ground (sampled)
            for (left, sd) in [(true, &s.left), (false, &s.right)] {
                let sg = if left { 1.0 } else { -1.0 };
                let mut gs: Vec<[f64; 2]> = s.ground.iter().map(|p| [p[0] * sg, p[1]]).collect();
                gs.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
                let mut o = 0.0;
                while o < sd.daylight {
                    let o2 = (o + 0.5).min(sd.daylight);
                    let (g1, g2) = (crate::corridor::interp(&gs, o), crate::corridor::interp(&gs, o2));
                    let (d1, d2) = (crate::corridor::interp(&sd.datum, o), crate::corridor::interp(&sd.datum, o2));
                    let rgb = if d1 + d2 < g1 + g2 { [1.0, 0.82, 0.82] } else { [0.82, 0.93, 0.82] };
                    d.fill("XS-GROUND", vec![tp(sg * o, g1), tp(sg * o2, g2), tp(sg * o2, d2), tp(sg * o, d1)], rgb);
                    o = o2;
                }
            }
            d.poly("XS-GROUND", s.ground.iter().filter(|p| p[0].abs() <= wmax).map(|p| tp(p[0], p[1])).collect());
            let fg: Vec<[f64; 2]> = s.right.fg.iter().rev().map(|p| tp(-p[0], p[1])).chain(s.left.fg.iter().skip(1).map(|p| tp(p[0], p[1]))).collect();
            d.poly("XS-DESIGN", fg);
            let dt: Vec<[f64; 2]> = s.right.datum.iter().rev().map(|p| tp(-p[0], p[1])).chain(s.left.datum.iter().skip(1).map(|p| tp(p[0], p[1]))).collect();
            d.poly("XS-DATUM", dt);
            for (sg, sd) in [(1.0, &s.left), (-1.0, &s.right)] {
                if let Some(w) = &sd.wall {
                    // stem (face at offset, back toward road for fill / away for cut) and base slab
                    let wd = &w.design;
                    let back = if w.cut { 1.0 } else { -1.0 } * sg;
                    let (o, zt, zb) = (w.offset * sg, w.top_z, w.bottom_z);
                    let zbt = zb + wd.base_t;
                    d.fill("WALL", vec![tp(o, zt), tp(o, zbt), tp(o + back * wd.stem_bot, zbt), tp(o + back * wd.stem_top, zt)], [0.7, 0.7, 0.75]);
                    let (a, b) = (o - back * wd.toe, o - back * wd.toe + back * wd.base_b);
                    d.fill("WALL", vec![tp(a, zb), tp(b, zb), tp(b, zbt), tp(a, zbt)], [0.7, 0.7, 0.75]);
                    d.text("TEXT", [tp(o, zt)[0] - 6.0, tp(o, zt)[1] + 2.0], 1.5, &format!("WALL H={:.1} B={:.2}", ul(wd.h), ul(wd.base_b)));
                }
            }
            d.line("GRID", tp(0.0, zlo), tp(0.0, zlo + hmax - 2.0));
            d.text("TEXT", [x0 + 4.0, y1 - 7.0], 2.5, &format!("STA {}", sta_label(s.station)));
            d.text(
                "TEXT",
                [x0 + 4.0, y0 + 7.0],
                1.8,
                &format!("FG {:.2} EG {:.2}  cut {:.1} m² fill {:.1} m²", ul(s.cl_elev), ul(s.ground_cl), s.cut_area, s.fill_area),
            );
            d.text(
                "TEXT",
                [x0 + 4.0, y0 + 3.5],
                1.6,
                &format!(
                    "cross slope L {:+.2}% R {:+.2}%  daylight L {:.1} R {:.1} m",
                    s.slope_left * 100.0,
                    s.slope_right * 100.0,
                    s.left.daylight,
                    s.right.daylight
                ),
            );
        }
        d.text("TEXT", [20.0, 12.0], 2.5, &format!("CROSS SECTIONS {}  (red = cut, green = fill, green line = subgrade datum)", crate::units::scale(nice)));
        title_block(&mut d, info, "Cross sections", first_page + pi, pages);
        out.push(d);
    }
    out
}

pub fn n_xs_pages(c: &Corridor) -> usize {
    c.sections.len().div_ceil(12)
}

pub fn mass_haul_sheet(
    mh: &MassHaul,
    rows: &[MaterialRow],
    c: &Corridor,
    nets: &[Network],
    grads: &[Grading],
    info: &SheetInfo,
    page: usize,
    pages: usize,
) -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    let (x0, y0, x1, y1) = (20.0, 320.0, SHEET_W - 160.0, SHEET_H - 20.0);
    d.rect("BORDER", x0, y0, x1, y1);
    let (s0, s1) = (mh.ordinates[0][0], mh.ordinates.last().unwrap()[0]);
    let (lo, hi) = mh.ordinates.iter().fold((0.0f64, 0.0f64), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
    let span = (hi - lo).max(1.0);
    let tp = |s: f64, m: f64| [x0 + 30.0 + (s - s0) / (s1 - s0) * (x1 - x0 - 40.0), y0 + 20.0 + (m - lo) / span * (y1 - y0 - 40.0)];
    d.line("GRID", tp(s0, 0.0), tp(s1, 0.0));
    let vf = if crate::units::cur().us() { crate::units::Kind::Volume.factor(crate::units::cur()) } else { 1.0 };
    let step = 10f64.powf((span / vf / 5.0).log10().floor()) * vf;
    let mut m = (lo / step).floor() * step;
    while m <= hi {
        d.line("GRID", tp(s0, m), tp(s1, m));
        d.text("TEXT", [x0 + 2.0, tp(s0, m)[1] - 1.0], 1.8, &format!("{:.0}", m / vf));
        m += step;
    }
    d.poly("PROFILE-DESIGN", mh.ordinates.iter().map(|p| tp(p[0], p[1])).collect());
    for b in &mh.balance_stations {
        let p = tp(*b, 0.0);
        d.line("ALIGNMENT", [p[0], p[1] - 4.0], [p[0], p[1] + 4.0]);
        d.text("TEXT", [p[0] + 1.0, p[1] + 5.0], 1.8, &format!("BAL {}", sta_label(*b)));
    }
    d.text(
        "TEXT",
        [x0 + 4.0, y1 - 7.0],
        4.0,
        &format!(
            "MASS HAUL ({})  cut factor {:.2}  final ordinate {:.0} m³",
            if crate::units::cur().us() { "CY" } else { "m³" },
            mh.cut_factor,
            mh.final_ordinate
        ),
    );
    // material table
    let mut y = y0 - 12.0;
    let row = |d: &mut Drawing, cols: &[String], y: f64, h: f64| {
        let xs = [20.0, 80.0, 140.0, 200.0, 260.0, 320.0, 380.0, 440.0];
        for (i, c) in cols.iter().enumerate() {
            d.text("TEXT", [xs[i.min(7)], y], h, c);
        }
    };
    let vu = if crate::units::cur().us() { "CY" } else { "m³" };
    let mut head = vec!["Station range".to_string(), format!("Cut {vu}"), format!("Fill {vu}")];
    head.extend(c.assembly.pavement.iter().map(|l| format!("{} {vu}", l.name)));
    row(&mut d, &head, y, 2.5);
    y -= 6.0;
    for r in rows {
        let mut cols = vec![format!("{}–{}", sta_label(r.from), sta_label(r.to)), format!("{:.0}", r.cut_m3 / vf), format!("{:.0}", r.fill_m3 / vf)];
        cols.extend(r.layers_m3.iter().map(|v| format!("{:.1}", v / vf)));
        row(&mut d, &cols, y, 2.0);
        y -= 4.5;
        if y < 120.0 {
            break;
        }
    }
    let mut tot = vec!["Total".to_string(), format!("{:.0}", c.cut_m3 / vf), format!("{:.0}", c.fill_m3 / vf)];
    tot.extend(c.layer_m3.iter().map(|v| format!("{:.1}", v.1 / vf)));
    row(&mut d, &tot, y, 2.5);
    y -= 10.0;
    for n in nets {
        d.text("TEXT", [20.0, y], 3.0, "Pipe schedule");
        y -= 6.0;
        let mut by: std::collections::BTreeMap<(String, i64), f64> = Default::default();
        for p in &n.pipes {
            *by.entry((p.material.clone(), (p.diameter_m * 1000.0).round() as i64)).or_default() += p.length;
        }
        for ((m, dmm), l) in by {
            d.text(
                "TEXT",
                [20.0, y],
                2.0,
                &if crate::units::cur().us() { format!("{m} D{:.0}\": {l:.1} m", dmm as f64 / 25.4) } else { format!("{m} DN{dmm}: {l:.1} m") },
            );
            y -= 4.5;
        }
        d.text("TEXT", [20.0, y], 2.0, &format!("Structures: {}", n.structures.len()));
        y -= 8.0;
    }
    for g in grads {
        d.text("TEXT", [20.0, y], 2.0, &format!("Grading {}: cut {:.0} m³  fill {:.0} m³ (grid {} m)", g.name, g.cut_m3, g.fill_m3, g.cell_m));
        y -= 4.5;
    }
    title_block(&mut d, info, "Mass haul and quantities", page, pages);
    d
}

/// World-coordinate model: contours, alignment, daylight, pipes, grading.
pub fn model(ds: &Design, contour_interval: f64) -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    d.ents.clear();
    d.units_mm = false;
    for (lv, a, b) in ds.tin.contours(contour_interval) {
        let major = ((lv / (contour_interval * 5.0)).round() * contour_interval * 5.0 - lv).abs() < 1e-6;
        d.line(if major { "CONTOUR-MAJOR" } else { "CONTOUR-MINOR" }, a, b);
    }
    let s = sample_al(ds.al, 1.0);
    d.poly("ALIGNMENT", s.iter().map(|p| [p.1, p.2]).collect());
    let mut st = (ds.al.start_station() / 100.0).ceil() * 100.0;
    while st <= ds.al.end_station() {
        let (x, y, h) = ds.al.point_at(st).unwrap();
        let (nx, ny) = (-h.sin(), h.cos());
        d.line("TEXT", [x - 5.0 * nx, y - 5.0 * ny], [x + 5.0 * nx, y + 5.0 * ny]);
        d.text("TEXT", [x + 7.0 * nx, y + 7.0 * ny], 4.0, &format!("STA {}", sta_label(st)));
        st += 100.0;
    }
    if let Some(c) = ds.corridor {
        for (left, f) in [(true, 1.0), (false, -1.0)] {
            d.poly(
                "DAYLIGHT",
                c.sections
                    .iter()
                    .map(|s| {
                        let o = if left { s.left.daylight } else { s.right.daylight } * f;
                        [s.x - s.heading.sin() * o, s.y + s.heading.cos() * o]
                    })
                    .collect(),
            );
        }
    }
    if let Some(c) = ds.corridor {
        wall_runs(&mut d, c, &|x, y| [x, y]);
    }
    for x in ds.intersections {
        plan_intersection(&mut d, x, &|x, y| [x, y], false);
    }
    for n in ds.networks {
        for p in &n.pipes {
            let a = n.structures.iter().find(|s| s.0 == p.from).unwrap();
            let b = n.structures.iter().find(|s| s.0 == p.to).unwrap();
            d.line("PIPE", [a.1, a.2], [b.1, b.2]);
        }
    }
    for g in ds.gradings {
        let mut f: Vec<[f64; 2]> = g.daylight.iter().map(|p| [p[0], p[1]]).collect();
        f.push(f[0]);
        d.poly("GRADING", f);
    }
    d
}
