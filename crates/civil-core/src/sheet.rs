//! Sheets (A1 landscape, millimetres): plan/profile, cross sections, mass haul
//! and quantities; plus a world-coordinate model drawing.
use crate::align::Alignment;
use crate::corridor::{Corridor, Section};
use crate::drawing::Drawing;
use crate::grading::Grading;
use crate::pipes::Network;
use crate::profile::Profile;
use crate::quantities::{MassHaul, MaterialRow};
use crate::tin::Tin;

pub const SHEET_W: f64 = 841.0;
pub const SHEET_H: f64 = 594.0;

pub fn sta_label(s: f64) -> String {
    let k = (s / 1000.0).floor();
    format!("K{}+{:06.2}", k as i64, s - k * 1000.0)
}

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
    pub network: Option<&'a Network>,
    pub gradings: &'a [Grading],
}

fn base(d: &mut Drawing) {
    for (l, c) in [("BORDER", 7), ("GRID", 9), ("CONTOUR-MINOR", 8), ("CONTOUR-MAJOR", 30), ("ALIGNMENT", 1), ("DAYLIGHT", 3), ("PROFILE-GROUND", 30), ("PROFILE-DESIGN", 1), ("XS-GROUND", 30), ("XS-DESIGN", 1), ("XS-DATUM", 3), ("PIPE", 5), ("STRUCTURE", 5), ("GRADING", 6), ("TEXT", 7), ("TITLE", 7), ("STAMP", 1)] {
        d.layer(l, c);
    }
    d.rect("BORDER", 10.0, 10.0, SHEET_W - 10.0, SHEET_H - 10.0);
}

/// Title block at the right; stamp logic: PRELIMINARY unless the engineer approved
/// with zero mandatory failures. Even then the sheet only says "approved in audit
/// log"; sealing is the engineer's act.
fn title_block(d: &mut Drawing, info: &SheetInfo, sheet_title: &str, page: usize, pages: usize) {
    let (tx0, ty0, tx1, ty1) = (SHEET_W - 150.0, 20.0, SHEET_W - 20.0, SHEET_H - 20.0);
    d.rect("TITLE", tx0, ty0, tx1, ty1);
    let mut y = ty1 - 4.0;
    let mut t = |d: &mut Drawing, s: &str, h: f64, layer: &str| {
        y -= h;
        d.text(layer, [tx0 + 4.0, y], h, s);
        y -= h * 0.9;
    };
    t(d, &info.project, 4.0, "TITLE");
    t(d, &format!("图名 Sheet: {sheet_title}"), 3.0, "TITLE");
    t(d, &format!("图号 {page}/{pages}"), 2.5, "TITLE");
    t(d, &format!("规范 Std: {}", info.standard), 2.5, "TITLE");
    t(d, &format!("设计速度 Design speed: {:.0} km/h", info.design_speed), 2.5, "TITLE");
    t(d, &format!("检查 Checks: {}/{} 通过 pass", info.checks_total - info.checks_failed, info.checks_total), 2.5, "TITLE");
    t(d, "审计日志 Audit log head:", 2.5, "TITLE");
    let h = info.audit_head.clone();
    t(d, &h[..32.min(h.len())], 2.0, "TITLE");
    t(d, &h[32.min(h.len())..], 2.0, "TITLE");
    t(d, "编制 Prepared by: civil-mcp agent tools", 2.5, "TITLE");
    t(d, &format!("负责工程师 Engineer in responsible charge:"), 2.5, "TITLE");
    t(d, &info.engineer, 2.5, "TITLE");
    match info.approved_seq {
        Some(seq) if info.checks_failed == 0 => {
            t(d, "工程师已在审计日志中批准", 3.0, "STAMP");
            t(d, &format!("Approved in audit log, entry #{seq}"), 2.5, "STAMP");
            t(d, "盖章由工程师本人完成 Seal applied by engineer only", 2.2, "STAMP");
        }
        _ => {
            t(d, "初步设计 PRELIMINARY", 6.0, "STAMP");
            t(d, "不得用于施工", 5.0, "STAMP");
            t(d, "NOT FOR CONSTRUCTION", 4.0, "STAMP");
            t(d, "未经注册工程师审核盖章", 2.5, "STAMP");
            t(d, "Not reviewed or sealed by a licensed engineer.", 2.2, "STAMP");
        }
    }
    d.rect("TITLE", tx0 + 10.0, ty0 + 10.0, tx1 - 10.0, ty0 + 90.0);
    d.text("TITLE", [tx0 + 14.0, ty0 + 13.0], 2.2, "注册工程师签章 ENGINEER SEAL / SIGNATURE");
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
    let n = [500.0, 1000.0, 2000.0, 2500.0, 3000.0, 4000.0, 5000.0, 10000.0, 20000.0].into_iter().find(|&n| n >= need).unwrap_or(need.ceil());
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
    if let Some(c) = ds.corridor {
        let edge = |s: &Section, left: bool, off: f64| {
            let (nx, ny) = (-s.heading.sin(), s.heading.cos());
            let o = if left { off } else { -off };
            tp(s.x + o * nx, s.y + o * ny)
        };
        d.poly("DAYLIGHT", c.sections.iter().map(|s| edge(s, true, s.left.daylight)).collect());
        d.poly("DAYLIGHT", c.sections.iter().map(|s| edge(s, false, s.right.daylight)).collect());
        let pw = c.assembly.lane_width();
        d.poly("TEXT", c.sections.iter().map(|s| edge(s, true, pw)).collect());
        d.poly("TEXT", c.sections.iter().map(|s| edge(s, false, pw)).collect());
    }
    for g in ds.gradings {
        let mut f: Vec<[f64; 2]> = g.footprint.iter().map(|p| tp(p[0], p[1])).collect();
        f.push(f[0]);
        d.poly("GRADING", f);
        let mut dl: Vec<[f64; 2]> = g.daylight.iter().map(|p| tp(p[0], p[1])).collect();
        dl.push(dl[0]);
        d.poly("GRADING", dl);
    }
    if let Some(net) = ds.network {
        for p in &net.pipes {
            let a = net.structures.iter().find(|s| s.0 == p.from).unwrap();
            let b = net.structures.iter().find(|s| s.0 == p.to).unwrap();
            d.line("PIPE", tp(a.1, a.2), tp(b.1, b.2));
            let m = tp((a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0);
            d.text("PIPE", [m[0] + 1.5, m[1] + 1.5], 1.8, &format!("{} D{:.0} S{:.2}%", p.id, p.diameter_m * 1000.0, p.slope * 100.0));
        }
        for s in &net.structures {
            let c = tp(s.1, s.2);
            d.rect("STRUCTURE", c[0] - 1.0, c[1] - 1.0, c[0] + 1.0, c[1] + 1.0);
            d.text("STRUCTURE", [c[0] + 1.5, c[1] - 3.0], 1.8, &s.0);
        }
    }
    d.poly("ALIGNMENT", samp.iter().map(|p| tp(p.1, p.2)).collect());
    let mut st = (al.start_station() / 100.0).ceil() * 100.0;
    while st <= al.end_station() + 1e-9 {
        let (x, y, h) = al.point_at(st).unwrap();
        let (nx, ny) = (-h.sin(), h.cos());
        d.line("ALIGNMENT", tp(x - 3.0 / sc * nx, y - 3.0 / sc * ny), tp(x + 3.0 / sc * nx, y + 3.0 / sc * ny));
        d.text("TEXT", tp(x + 5.0 / sc * nx, y + 5.0 / sc * ny), 2.2, &sta_label(st));
        st += 100.0;
    }
    for c in al.curves() {
        let (x, y, _) = al.point_at((c.sta_start + c.sta_end) / 2.0).unwrap();
        let p = tp(x, y);
        let txt = if c.ls_in > 0.0 { format!("JD{} R={:.0} Ls={:.0} α={:.2}°", c.index, c.radius, c.ls_in, c.delta_deg.abs()) } else { format!("JD{} R={:.0} L={:.1} α={:.2}°", c.index, c.radius, c.arc_len, c.delta_deg.abs()) };
        d.text("TEXT", [p[0] + 3.0, p[1] - 6.0], 2.2, &txt);
    }
    d.text("TEXT", [px0 + 4.0, py1 - 7.0], 4.0, &format!("平面图 PLAN  1:{n:.0}   等高线 1 m（计曲线 5 m）  N↑"));
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
    let zmin = (zs.iter().cloned().fold(f64::MAX, f64::min) / 10.0).floor() * 10.0 - 10.0;
    let zmax = (zs.iter().cloned().fold(f64::MIN, f64::max) / 10.0).ceil() * 10.0 + 10.0;
    let hs = (qx1 - qx0 - 40.0) / (s1 - s0);
    let vs = ((qy1 - qy0 - 40.0) / (zmax - zmin)).min(hs * 10.0);
    let tq = |s: f64, z: f64| [qx0 + 30.0 + (s - s0) * hs, qy0 + 25.0 + (z - zmin) * vs];
    let mut z = zmin;
    while z <= zmax + 1e-9 {
        d.line("GRID", tq(s0, z), tq(s1, z));
        d.text("TEXT", [qx0 + 3.0, tq(s0, z)[1] - 1.0], 2.0, &format!("{z:.0}"));
        z += 10.0;
    }
    let mut st = (s0 / 100.0).ceil() * 100.0;
    while st <= s1 + 1e-9 {
        d.line("GRID", tq(st, zmin), tq(st, zmax));
        d.text("TEXT", [tq(st, zmin)[0] - 8.0, qy0 + 14.0], 2.0, &sta_label(st));
        if let (Some(zd), Some(zg)) = (pr.elevation_at(st), al.point_at(st).and_then(|(x, y, _)| tin.elevation_at(x, y))) {
            d.text("TEXT", [tq(st, zmin)[0] - 8.0, qy0 + 9.0], 1.8, &format!("设计 {zd:.2}"));
            d.text("TEXT", [tq(st, zmin)[0] - 8.0, qy0 + 5.0], 1.8, &format!("地面 {zg:.2}"));
        }
        st += 100.0;
    }
    d.poly("PROFILE-GROUND", g.iter().map(|p| tq(p.0, p.1)).collect());
    d.poly("PROFILE-DESIGN", fg.iter().map(|p| tq(p.0, p.1)).collect());
    for (i, gr) in pr.grades_pct.iter().enumerate() {
        let (a, b) = (pr.input.vpis[i], pr.input.vpis[i + 1]);
        let m = tq((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
        d.text("TEXT", [m[0], m[1] + 4.0], 2.2, &format!("i={gr:+.2}% L={:.0}", b[0] - a[0]));
    }
    for c in &pr.curves {
        let v = pr.input.vpis[c.vpi_index];
        let p = tq(v[0], v[1]);
        d.poly("PROFILE-DESIGN", vec![[p[0], p[1]], [p[0], p[1] + 12.0]]);
        d.text("TEXT", [p[0] - 10.0, p[1] + 14.0], 2.0, &format!("变坡点 {} 高程 {:.2}", sta_label(v[0]), v[1]));
        d.text("TEXT", [p[0] - 10.0, p[1] + 17.0], 2.0, &format!("{} R={:.0} L={:.0}", if c.crest { "凸" } else { "凹" }, c.length / (c.a_pct.abs() / 100.0).max(1e-9), c.length));
    }
    d.text("TEXT", [qx0 + 4.0, qy1 - 7.0], 4.0, &format!("纵断面图 PROFILE  H 1:{:.0}  V 1:{:.0}（地面线棕色，设计线红色）", 1000.0 / hs, 1000.0 / vs));
    title_block(&mut d, info, "平纵面图 Plan & Profile", page, pages);
    d
}

/// Cross-section sheets: 4 columns × 3 rows per sheet, uniform scale.
pub fn cross_sections(c: &Corridor, info: &SheetInfo, first_page: usize, pages: usize) -> Vec<Drawing> {
    let per = 12;
    let mut out = Vec::new();
    let (mut wmax, mut hmax): (f64, f64) = (10.0, 5.0);
    for s in &c.sections {
        wmax = wmax.max(s.left.daylight + 5.0).max(s.right.daylight + 5.0);
        let zs: Vec<f64> = s.left.fg.iter().chain(s.right.fg.iter()).map(|p| p[1]).chain(s.ground.iter().map(|p| p[1])).collect();
        let (lo, hi) = zs.iter().fold((f64::MAX, f64::MIN), |a, z| (a.0.min(*z), a.1.max(*z)));
        hmax = hmax.max(hi - lo + 4.0);
    }
    let (cw, ch) = ((SHEET_W - 180.0) / 4.0, (SHEET_H - 40.0) / 3.0);
    let scale_h = ((cw - 10.0) / (2.0 * wmax)).min((ch - 20.0) / hmax);
    let nice = [100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0].into_iter().find(|n| 1000.0 / n <= scale_h).unwrap_or(2000.0);
    let sc = 1000.0 / nice;
    for (pi, chunk) in c.sections.chunks(per).enumerate() {
        let mut d = Drawing::default();
        base(&mut d);
        for (i, s) in chunk.iter().enumerate() {
            let (col, row) = (i % 4, i / 4);
            let (x0, y1) = (20.0 + col as f64 * cw, SHEET_H - 20.0 - row as f64 * ch);
            let y0 = y1 - ch;
            d.rect("GRID", x0 + 2.0, y0 + 2.0, x0 + cw - 2.0, y1 - 2.0);
            let zs: Vec<f64> = s.left.fg.iter().chain(s.right.fg.iter()).map(|p| p[1]).chain(s.ground.iter().map(|p| p[1])).collect();
            let zlo = zs.iter().cloned().fold(f64::MAX, f64::min) - 1.0;
            let xc = x0 + cw / 2.0;
            let tp = |o: f64, z: f64| [xc + o * sc, y0 + 12.0 + (z - zlo) * sc];
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
            d.line("GRID", tp(0.0, zlo), tp(0.0, zlo + hmax - 2.0));
            d.text("TEXT", [x0 + 4.0, y1 - 7.0], 2.5, &format!("桩号 {}", sta_label(s.station)));
            d.text("TEXT", [x0 + 4.0, y0 + 7.0], 1.8, &format!("设计 {:.2} 地面 {:.2}  挖 {:.1} m² 填 {:.1} m²", s.cl_elev, s.ground_cl, s.cut_area, s.fill_area));
            d.text("TEXT", [x0 + 4.0, y0 + 3.5], 1.6, &format!("横坡 左 {:+.2}% 右 {:+.2}%  坡脚/坡顶 左 {:.1} 右 {:.1} m", s.slope_left * 100.0, s.slope_right * 100.0, s.left.daylight, s.right.daylight));
        }
        d.text("TEXT", [20.0, 12.0], 2.5, &format!("横断面图 CROSS SECTIONS 1:{nice:.0}（红=挖方 cut，绿=填方 fill，绿线=路基设计线 datum）"));
        title_block(&mut d, info, "横断面图 Cross sections", first_page + pi, pages);
        out.push(d);
    }
    out
}

pub fn n_xs_pages(c: &Corridor) -> usize {
    c.sections.len().div_ceil(12)
}

pub fn mass_haul_sheet(mh: &MassHaul, rows: &[MaterialRow], c: &Corridor, net: Option<&Network>, grads: &[Grading], info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    let (x0, y0, x1, y1) = (20.0, 320.0, SHEET_W - 160.0, SHEET_H - 20.0);
    d.rect("BORDER", x0, y0, x1, y1);
    let (s0, s1) = (mh.ordinates[0][0], mh.ordinates.last().unwrap()[0]);
    let (lo, hi) = mh.ordinates.iter().fold((0.0f64, 0.0f64), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
    let span = (hi - lo).max(1.0);
    let tp = |s: f64, m: f64| [x0 + 30.0 + (s - s0) / (s1 - s0) * (x1 - x0 - 40.0), y0 + 20.0 + (m - lo) / span * (y1 - y0 - 40.0)];
    d.line("GRID", tp(s0, 0.0), tp(s1, 0.0));
    let step = 10f64.powf((span / 5.0).log10().floor());
    let mut m = (lo / step).floor() * step;
    while m <= hi {
        d.line("GRID", tp(s0, m), tp(s1, m));
        d.text("TEXT", [x0 + 2.0, tp(s0, m)[1] - 1.0], 1.8, &format!("{m:.0}"));
        m += step;
    }
    d.poly("PROFILE-DESIGN", mh.ordinates.iter().map(|p| tp(p[0], p[1])).collect());
    for b in &mh.balance_stations {
        let p = tp(*b, 0.0);
        d.line("ALIGNMENT", [p[0], p[1] - 4.0], [p[0], p[1] + 4.0]);
        d.text("TEXT", [p[0] + 1.0, p[1] + 5.0], 1.8, &format!("平衡点 {}", sta_label(*b)));
    }
    d.text("TEXT", [x0 + 4.0, y1 - 7.0], 4.0, &format!("土方调配图 MASS HAUL (m³)  挖方系数 cut factor {:.2}  终点累计 {:.0} m³", mh.cut_factor, mh.final_ordinate));
    // material table
    let mut y = y0 - 12.0;
    let row = |d: &mut Drawing, cols: &[String], y: f64, h: f64| {
        let xs = [20.0, 80.0, 140.0, 200.0, 260.0, 320.0, 380.0, 440.0];
        for (i, c) in cols.iter().enumerate() {
            d.text("TEXT", [xs[i.min(7)], y], h, c);
        }
    };
    let mut head = vec!["桩号范围 Station".to_string(), "挖方 Cut m³".into(), "填方 Fill m³".into()];
    head.extend(c.assembly.pavement.iter().map(|l| format!("{} m³", l.name)));
    row(&mut d, &head, y, 2.5);
    y -= 6.0;
    for r in rows {
        let mut cols = vec![format!("{}–{}", sta_label(r.from), sta_label(r.to)), format!("{:.0}", r.cut_m3), format!("{:.0}", r.fill_m3)];
        cols.extend(r.layers_m3.iter().map(|v| format!("{v:.1}")));
        row(&mut d, &cols, y, 2.0);
        y -= 4.5;
        if y < 120.0 {
            break;
        }
    }
    let mut tot = vec!["合计 Total".to_string(), format!("{:.0}", c.cut_m3), format!("{:.0}", c.fill_m3)];
    tot.extend(c.layer_m3.iter().map(|v| format!("{:.1}", v.1)));
    row(&mut d, &tot, y, 2.5);
    y -= 10.0;
    if let Some(n) = net {
        d.text("TEXT", [20.0, y], 3.0, "管道材料表 Pipe schedule");
        y -= 6.0;
        let mut by: std::collections::BTreeMap<(String, i64), f64> = Default::default();
        for p in &n.pipes {
            *by.entry((p.material.clone(), (p.diameter_m * 1000.0).round() as i64)).or_default() += p.length;
        }
        for ((m, dmm), l) in by {
            d.text("TEXT", [20.0, y], 2.0, &format!("{m} DN{dmm}: {l:.1} m"));
            y -= 4.5;
        }
        d.text("TEXT", [20.0, y], 2.0, &format!("检查井 Structures: {}", n.structures.len()));
        y -= 8.0;
    }
    for g in grads {
        d.text("TEXT", [20.0, y], 2.0, &format!("场平 Grading {}: 挖 {:.0} m³  填 {:.0} m³ (方格网 {} m)", g.name, g.cut_m3, g.fill_m3, g.cell_m));
        y -= 4.5;
    }
    title_block(&mut d, info, "土方调配及工程量 Mass haul & quantities", page, pages);
    d
}

/// World-coordinate model: contours, alignment, daylight, pipes, grading.
pub fn model(ds: &Design, contour_interval: f64) -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    d.ents.clear();
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
        d.text("TEXT", [x + 7.0 * nx, y + 7.0 * ny], 4.0, &format!("桩号 {}", sta_label(st)));
        st += 100.0;
    }
    if let Some(c) = ds.corridor {
        for (left, f) in [(true, 1.0), (false, -1.0)] {
            d.poly("DAYLIGHT", c.sections.iter().map(|s| {
                let o = if left { s.left.daylight } else { s.right.daylight } * f;
                [s.x - s.heading.sin() * o, s.y + s.heading.cos() * o]
            }).collect());
        }
    }
    if let Some(n) = ds.network {
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
