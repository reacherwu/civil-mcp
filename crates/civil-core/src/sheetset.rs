//! Additional design-stage sheets: cover with sheet index and general notes, route
//! alternatives, typical sections, superelevation diagram, pavement and subgrade
//! details, roadside protection layout, drainage details and a standard-details atlas.
//! All sheets are A1 landscape in millimetres and carry the common title block.
use crate::align::Alignment;
use crate::corridor::{Assembly, Corridor};
use crate::drawing::Drawing;
use crate::pavement::PavementDesign;
use crate::pipes::Network;
use crate::protection::Protection;
use crate::sheet::{base, sta_label, title_block_s, wrap, SheetInfo, SHEET_H, SHEET_W};
use crate::standards::Standard;
use crate::superelev::Superelevation;
use crate::tin::Tin;
use serde_json::Value;

const X1: f64 = SHEET_W - 160.0; // right edge of the drawing area (title block to the right)

fn circle(d: &mut Drawing, l: &str, c: [f64; 2], r: f64) {
    d.poly(l, (0..=48).map(|k| { let a = k as f64 / 48.0 * std::f64::consts::TAU; [c[0] + r * a.cos(), c[1] + r * a.sin()] }).collect());
}

fn para(d: &mut Drawing, x: f64, y: &mut f64, h: f64, max_em: f64, s: &str) {
    for line in wrap(s, max_em) {
        d.text("TEXT", [x, *y], h, &line);
        *y -= h * 1.6;
    }
}

/// Simple table: header + rows, fixed column widths (mm).
fn table(d: &mut Drawing, x: f64, y: f64, widths: &[f64], h: f64, rows: &[Vec<String>]) -> f64 {
    let rh = h * 2.0;
    let w: f64 = widths.iter().sum();
    let mut yy = y;
    for (ri, r) in rows.iter().enumerate() {
        d.line("TEXT", [x, yy], [x + w, yy]);
        let mut xx = x;
        for (ci, c) in r.iter().enumerate() {
            let em = widths.get(ci).copied().unwrap_or(20.0) / h / 0.62;
            let txt: String = if c.chars().count() as f64 > em { c.chars().take(em.max(3.0) as usize - 1).collect::<String>() + "…" } else { c.clone() };
            d.text(if ri == 0 { "TITLE" } else { "TEXT" }, [xx + 1.0, yy - rh + h * 0.5], h, &txt);
            xx += widths.get(ci).copied().unwrap_or(20.0);
        }
        yy -= rh;
    }
    d.line("TEXT", [x, yy], [x + w, yy]);
    let mut xx = x;
    for wi in std::iter::once(&0.0).chain(widths.iter()) {
        xx += wi;
        d.line("TEXT", [xx, y], [xx, yy]);
    }
    yy
}

fn new_sheet() -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    for (l, c) in [("ROUTE-A", 1), ("ROUTE-B", 3), ("ROUTE-C", 5), ("ROUTE-SELECTED", 6), ("PAVEMENT", 2), ("SUBGRADE", 8), ("BARRIER", 1), ("HAZARD", 6), ("LINING", 4), ("DETAIL", 7), ("HATCH", 8)] {
        d.layer(l, c);
    }
    d
}

pub fn std_label(s: Standard) -> &'static str {
    match s {
        Standard::Us => "US - AASHTO Green Book 2018 / FHWA",
        Standard::Ca => "Canada - TAC Geometric Design Guide 2017",
        Standard::Uk => "UK - Design Manual for Roads and Bridges",
        Standard::Au => "Australia - Austroads Guides",
        Standard::Nz => "New Zealand - Austroads with NZTA supplements",
    }
}

/// Sheet 1: cover, sheet index and general notes.
pub fn cover(info: &SheetInfo, std: Standard, index: &[(String, String)], notes: &[String], pages: usize) -> Drawing {
    let mut d = new_sheet();
    d.text("TITLE", [30.0, SHEET_H - 50.0], 9.0, "ROAD DESIGN - PRELIMINARY DESIGN DRAWINGS");
    let mut y = SHEET_H - 66.0;
    for l in wrap(&info.project, 60.0) {
        d.text("TITLE", [30.0, y], 6.0, &l);
        y -= 9.0;
    }
    d.text("TEXT", [30.0, y - 2.0], 3.5, &format!("Standards profile: {}   Design speed {:.0} km/h", std_label(std), info.design_speed));
    d.text("TITLE", [30.0, y - 16.0], 4.5, "SHEET INDEX");
    let mut rows = vec![vec!["Sheet".to_string(), "Title".to_string(), "File".to_string()]];
    for (i, (t, f)) in index.iter().enumerate() {
        rows.push(vec![format!("{} of {}", i + 1, pages), t.clone(), f.clone()]);
    }
    table(&mut d, 30.0, y - 22.0, &[30.0, 190.0, 40.0], 3.0, &rows);
    let mut ny = SHEET_H - 120.0;
    let nx = 340.0;
    d.text("TITLE", [nx, ny + 14.0], 4.5, "GENERAL NOTES");
    for (i, n) in notes.iter().enumerate() {
        para(&mut d, nx, &mut ny, 2.8, 108.0, &format!("{}. {}", i + 1, n));
        ny -= 2.0;
    }
    title_block_s(&mut d, info, "Cover, sheet index and general notes", "NTS", 1, pages);
    d
}

/// Route alternatives: contours, the alternative centre lines, the selected route and
/// the comparison table with the recorded reasons.
pub fn route_sheet(tin: &Tin, route: &Value, selected: &str, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = new_sheet();
    let alts = route["alternatives"].as_array().cloned().unwrap_or_default();
    let (px0, py0, px1, py1) = (20.0, 200.0, X1, SHEET_H - 20.0);
    d.rect("BORDER", px0, py0, px1, py1);
    let pts: Vec<[f64; 2]> = alts.iter().flat_map(|a| a["path"].as_array().cloned().unwrap_or_default()).filter_map(|p| Some([p[0].as_f64()?, p[1].as_f64()?])).collect();
    if pts.is_empty() {
        return d;
    }
    let (mut mnx, mut mny, mut mxx, mut mxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &pts {
        mnx = mnx.min(p[0]);
        mny = mny.min(p[1]);
        mxx = mxx.max(p[0]);
        mxy = mxy.max(p[1]);
    }
    let pad = 60.0;
    let (mnx, mny, mxx, mxy) = (mnx - pad, mny - pad, mxx + pad, mxy + pad);
    let need = ((mxx - mnx) / (px1 - px0 - 10.0) * 1000.0).max((mxy - mny) / (py1 - py0 - 10.0) * 1000.0);
    let n = [1000.0, 2000.0, 2500.0, 3000.0, 4000.0, 5000.0, 10000.0, 20000.0].into_iter().find(|&n| n >= need).unwrap_or(need.ceil());
    let sc = 1000.0 / n;
    let (cx, cy, vx, vy) = ((mnx + mxx) / 2.0, (mny + mxy) / 2.0, (px0 + px1) / 2.0, (py0 + py1) / 2.0);
    let tp = |x: f64, y: f64| [vx + (x - cx) * sc, vy + (y - cy) * sc];
    let inside = |p: [f64; 2]| p[0] > px0 && p[0] < px1 && p[1] > py0 && p[1] < py1;
    for (lv, a, b) in tin.contours(5.0) {
        let (pa, pb) = (tp(a[0], a[1]), tp(b[0], b[1]));
        if inside(pa) && inside(pb) {
            let major = (lv / 25.0 - (lv / 25.0).round()).abs() < 1e-6;
            d.line(if major { "CONTOUR-MAJOR" } else { "CONTOUR-MINOR" }, pa, pb);
        }
    }
    for (k, a) in alts.iter().enumerate() {
        let name = a["name"].as_str().unwrap_or("?");
        let l = if name == selected { "ROUTE-SELECTED" } else { ["ROUTE-A", "ROUTE-B", "ROUTE-C"][k % 3] };
        let pis: Vec<[f64; 2]> = a["pis"].as_array().cloned().unwrap_or_default().iter().filter_map(|p| Some(tp(p[0].as_f64()?, p[1].as_f64()?))).collect();
        d.poly(l, pis.clone());
        for (i, p) in pis.iter().enumerate() {
            circle(&mut d, l, *p, 1.2);
            if i > 0 && i + 1 < pis.len() {
                d.text(l, [p[0] + 2.0, p[1] + 2.0], 2.0, &format!("PI{}", i));
            }
        }
        if let Some(m) = pis.get(pis.len() / 2) {
            d.text(l, [m[0] + 3.0, m[1] - 5.0], 3.0, &format!("{}{}", name, if name == selected { " (SELECTED)" } else { "" }));
        }
    }
    // comparison table
    let mut rows = vec![["Alternative", "Length m", "Min R m", "Max grade %", "Cut m3", "Fill m3", "Imbalance m3", "Wall conc. m3", "Mandatory fails", "Cost proxy"].iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for a in &alts {
        let e = &a["evaluation"];
        let f = |k: &str, p: usize| e[k].as_f64().map_or("-".into(), |v| format!("{v:.p$}"));
        rows.push(vec![a["name"].as_str().unwrap_or("?").into(), f("length_m", 0), f("min_radius_m", 0), f("max_design_grade_pct", 2), f("cut_m3", 0), f("fill_m3", 0), f("imbalance_m3", 0), f("wall_concrete_m3", 0), e["geometry_mandatory_failures"].to_string(), f("cost_proxy", 0)]);
    }
    let yb = table(&mut d, 20.0, 190.0, &[48.0, 22.0, 20.0, 24.0, 24.0, 24.0, 28.0, 28.0, 30.0, 28.0], 2.6, &rows);
    let mut y = yb - 8.0;
    let rec = &route["recommendation"];
    para(&mut d, 20.0, &mut y, 2.6, 270.0, &format!("Selection rule: {}", rec["rule"].as_str().unwrap_or("")));
    for r in rec["reasons"].as_array().cloned().unwrap_or_default() {
        para(&mut d, 20.0, &mut y, 2.4, 290.0, &format!("- {}", r.as_str().unwrap_or("")));
    }
    para(&mut d, 20.0, &mut y, 2.4, 290.0, "Cost proxy uses placeholder unit rates unless the engineer enters rates; it ranks alternatives and is not an estimate. Least-cost search on a 10 m grid, 16 headings, grade penalty above the profile maximum.");
    title_block_s(&mut d, info, "Route alternatives and selection", &format!("1:{n:.0}"), page, pages);
    d
}

/// Typical sections at 1:100: lanes, shoulders, curb/sidewalk, ditch in cut, side slopes,
/// pavement layers.
pub fn typical_sections(asm: &Assembly, nc: f64, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = new_sheet();
    let s = 10.0; // 1:100 → 10 mm per m
    for (k, (title, cut)) in [("TYPICAL SECTION IN CUT", true), ("TYPICAL SECTION IN FILL", false)].iter().enumerate() {
        let ox = 20.0 + (X1 - 20.0) / 4.0 + k as f64 * (X1 - 20.0) / 2.0;
        let oy = SHEET_H - 170.0;
        d.text("TITLE", [ox - 60.0, SHEET_H - 40.0], 4.0, title);
        d.line("DETAIL", [ox, oy - 40.0], [ox, oy + 30.0]);
        d.text("TEXT", [ox - 3.0, oy + 32.0], 2.5, "CL");
        for side in [1.0f64, -1.0] {
            let cfg = asm.side_cfg(side > 0.0);
            let mut x = 0.0;
            let mut z = 0.0;
            let mut pts = vec![[ox, oy]];
            for w in &cfg.lanes_m {
                x += w;
                z -= w * nc;
                pts.push([ox + side * x * s, oy + z * s]);
                d.text("TEXT", [ox + side * (x - w / 2.0) * s - 6.0, oy + 8.0], 2.2, &format!("Lane {w:.2} m"));
            }
            d.text("TEXT", [ox + side * (x / 2.0) * s - 5.0, oy + 4.0], 2.0, &format!("{:.1}%", nc * 100.0));
            if cfg.shoulder_m > 0.0 {
                x += cfg.shoulder_m;
                z += cfg.shoulder_m * cfg.shoulder_slope;
                pts.push([ox + side * x * s, oy + z * s]);
                d.text("TEXT", [ox + side * (x - cfg.shoulder_m / 2.0) * s - 6.0, oy + 14.0], 2.0, &format!("Shoulder {:.2} m {:.1}%", cfg.shoulder_m, cfg.shoulder_slope * 100.0));
            }
            if let Some(c) = &cfg.curb {
                x += c.gutter_m;
                z += c.gutter_m * c.gutter_slope;
                pts.push([ox + side * x * s, oy + z * s]);
                z += c.height_m;
                pts.push([ox + side * x * s, oy + z * s]);
                x += c.top_m;
                pts.push([ox + side * x * s, oy + z * s]);
                d.text("TEXT", [ox + side * x * s - 8.0, oy + z * s + 4.0], 2.0, &format!("Curb {:.0} mm", c.height_m * 1000.0));
                if c.sidewalk_m > 0.0 {
                    x += c.sidewalk_m;
                    pts.push([ox + side * x * s, oy + z * s]);
                    d.text("TEXT", [ox + side * (x - c.sidewalk_m / 2.0) * s - 6.0, oy + z * s + 8.0], 2.0, &format!("Sidewalk {:.2} m", c.sidewalk_m));
                }
            }
            if *cut {
                if let (Some(dt), true) = (&cfg.ditch, cfg.curb.is_none()) {
                    x += dt.foreslope_h * dt.depth_m;
                    z -= dt.depth_m;
                    pts.push([ox + side * x * s, oy + z * s]);
                    x += dt.bottom_width_m;
                    pts.push([ox + side * x * s, oy + z * s]);
                    d.text("TEXT", [ox + side * x * s - 8.0, oy + z * s - 5.0], 2.0, &format!("Ditch {:.1}H:1V, {:.2} m deep", dt.foreslope_h, dt.depth_m));
                }
                let h = 3.0;
                x += cfg.cut_slope_h * h;
                z += h + 0.0;
                pts.push([ox + side * x * s, oy + z * s]);
                d.text("TEXT", [ox + side * x * s - 10.0, oy + z * s + 3.0], 2.2, &format!("Cut {:.1}H:1V", cfg.cut_slope_h));
            } else {
                let h = 2.5;
                x += cfg.fill_slope_h * h;
                z -= h;
                pts.push([ox + side * x * s, oy + z * s]);
                d.text("TEXT", [ox + side * x * s - 10.0, oy + z * s - 5.0], 2.2, &format!("Fill {:.1}H:1V", cfg.fill_slope_h));
            }
            d.poly("XS-DESIGN", pts);
            // pavement box under the travelled way
            let pw: f64 = cfg.lanes_m.iter().sum::<f64>() + if asm.shoulder_paved { cfg.shoulder_m } else { 0.0 };
            let mut depth = 0.0;
            for l in &asm.pavement {
                let top = depth;
                depth += l.thickness_m;
                let zt = -pw.min(cfg.lanes_m.iter().sum()) * nc;
                d.poly("PAVEMENT", vec![[ox, oy - top * s * 4.0], [ox + side * pw * s, oy + zt * s - top * s * 4.0]]);
                let _ = top;
            }
            d.poly("SUBGRADE", vec![[ox, oy - depth * s * 4.0], [ox + side * pw * s, oy - depth * s * 4.0 - pw * nc * s]]);
        }
    }
    // pavement layer legend
    let mut rows = vec![vec!["Layer (top to bottom)".to_string(), "Thickness mm".to_string()]];
    for l in &asm.pavement {
        rows.push(vec![l.name.clone(), format!("{:.0}", l.thickness_m * 1000.0)]);
    }
    let yb = table(&mut d, 30.0, 250.0, &[90.0, 35.0], 3.0, &rows);
    let mut y = yb - 8.0;
    para(&mut d, 30.0, &mut y, 2.6, 120.0, "Pavement layer lines are drawn at 4x vertical exaggeration for legibility. Side slopes shown for a nominal 3.0 m cut / 2.5 m fill height; actual daylight per cross sections. Barrier, lining and protection treatments per the protection layout sheet.");
    title_block_s(&mut d, info, "Typical sections", "1:100", page, pages);
    d
}

/// Superelevation diagram: left and right edge cross slope against station.
pub fn superelevation_sheet(se: &Superelevation, al: &Alignment, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = new_sheet();
    let (x0, x1, y0, y1) = (40.0, X1 - 10.0, 200.0, SHEET_H - 60.0);
    let (s0, s1) = (al.start_station(), al.end_station());
    let emax = se.curves.iter().map(|c| c.e).fold(0.04f64, f64::max) + 0.01;
    let tx = |s: f64| x0 + (s - s0) / (s1 - s0) * (x1 - x0);
    let ty = |e: f64| (y0 + y1) / 2.0 + e / emax * (y1 - y0) / 2.0;
    d.rect("GRID", x0, y0, x1, y1);
    let mut e = -((emax * 100.0).floor()) / 100.0;
    while e <= emax + 1e-9 {
        d.line("GRID", [x0, ty(e)], [x1, ty(e)]);
        d.text("TEXT", [x0 - 16.0, ty(e) - 1.0], 2.2, &format!("{:+.0}%", e * 100.0));
        e += 0.02;
    }
    let mut st = (s0 / 100.0).ceil() * 100.0;
    while st <= s1 {
        d.line("GRID", [tx(st), y0], [tx(st), y0 - 3.0]);
        d.text("TEXT", [tx(st) - 8.0, y0 - 8.0], 2.2, &sta_label(st));
        st += 100.0;
    }
    let n = 600;
    let (mut l, mut r) = (vec![], vec![]);
    for k in 0..=n {
        let s = s0 + (s1 - s0) * k as f64 / n as f64;
        let (sl, sr) = se.slopes_at(s);
        l.push([tx(s), ty(sl)]);
        r.push([tx(s), ty(sr)]);
    }
    d.poly("PROFILE-DESIGN", l);
    d.poly("HGL", r);
    d.text("PROFILE-DESIGN", [x0 + 4.0, y1 + 6.0], 3.0, "Left edge cross slope (rise per run outward from CL)");
    d.text("HGL", [x0 + 4.0, y1 + 12.0], 3.0, "Right edge cross slope");
    let mut rows = vec![["Curve", "R m", "e", "Method", "Runoff start", "Full e", "Full e end", "Runoff end", "Rel. gradient %"].iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for c in &se.curves {
        rows.push(vec![c.curve.to_string(), format!("{:.0}", c.radius), format!("{:.3}", c.e), c.method.clone(), sta_label(c.a_in), sta_label(c.b_in), sta_label(c.b_out), sta_label(c.a_out), format!("{:.3}", c.relative_gradient * 100.0)]);
    }
    table(&mut d, 40.0, 180.0, &[18.0, 20.0, 18.0, 120.0, 30.0, 30.0, 30.0, 30.0, 30.0], 2.6, &rows);
    title_block_s(&mut d, info, "Superelevation diagram", "H 1:as shown; V as shown", page, pages);
    d
}

/// Pavement and subgrade details: layer stack at 1:10, design inputs and steps.
pub fn pavement_sheet(p: &PavementDesign, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = new_sheet();
    let (x, top) = (40.0, SHEET_H - 60.0);
    d.text("TITLE", [x, top + 12.0], 4.0, &format!("PAVEMENT STRUCTURE ({})", p.kind.to_uppercase()));
    let mut z = top;
    let w = 160.0;
    for (i, l) in p.layers.iter().enumerate() {
        let h = l.thickness_mm / 10.0; // 1:10
        d.rect("PAVEMENT", x, z - h, x + w, z);
        if i % 2 == 1 {
            let mut k = 0.0;
            while k < w {
                d.line("HATCH", [x + k, z - h], [x + (k + h).min(w), z - (k + h).min(w) + k]);
                k += 6.0;
            }
        }
        d.text("TEXT", [x + w + 6.0, z - h / 2.0 - 1.2], 3.0, &format!("{} - {:.0} mm", l.name, l.thickness_mm));
        if !l.note.is_empty() {
            d.text("TEXT", [x + w + 6.0, z - h / 2.0 - 5.5], 2.2, &l.note);
        }
        z -= h;
    }
    d.line("SUBGRADE", [x - 10.0, z], [x + w + 10.0, z]);
    let mut k = 0.0;
    while k < w + 20.0 {
        d.line("SUBGRADE", [x - 10.0 + k, z], [x - 14.0 + k, z - 4.0]);
        k += 5.0;
    }
    let sg = p.inputs.get("subgrade_cbr").and_then(|v| v.as_f64()).map_or(String::new(), |c| format!(", design CBR {c:.0}%"));
    d.text("TEXT", [x + w + 6.0, z - 4.0], 3.0, &format!("Prepared subgrade{sg}"));
    d.text("TEXT", [x, z - 14.0], 3.0, &format!("Total thickness {:.0} mm", p.total_mm));
    let mut y = z - 30.0;
    d.text("TITLE", [x, y], 3.5, "DESIGN BASIS");
    y -= 7.0;
    para(&mut d, x, &mut y, 2.6, 280.0, &format!("Method: {} ({})", p.method, p.citation));
    if let (Some(r), Some(q)) = (p.sn_required, p.sn_provided) {
        para(&mut d, x, &mut y, 2.6, 280.0, &format!("Structural number required {r:.2}, provided {q:.2}"));
    }
    for s in &p.steps {
        para(&mut d, x, &mut y, 2.4, 300.0, &format!("- {s}"));
    }
    if !p.engineer_inputs.is_empty() {
        para(&mut d, x, &mut y, 2.4, 300.0, &format!("Engineer inputs: {}", p.engineer_inputs.join("; ")));
    }
    for wn in &p.warnings {
        para(&mut d, x, &mut y, 2.4, 300.0, &format!("Note: {wn}"));
    }
    // subgrade preparation detail (generic)
    let (sx, sy) = (440.0, SHEET_H - 80.0);
    d.text("TITLE", [sx, sy + 12.0], 3.5, "SUBGRADE PREPARATION (SCHEMATIC)");
    d.poly("XS-DESIGN", vec![[sx, sy], [sx + 120.0, sy], [sx + 160.0, sy - 20.0]]);
    d.poly("SUBGRADE", vec![[sx, sy - 25.0], [sx + 120.0, sy - 25.0], [sx + 145.0, sy - 37.5]]);
    d.poly("SUBGRADE", vec![[sx, sy - 40.0], [sx + 120.0, sy - 40.0], [sx + 140.0, sy - 50.0]]);
    let mut yy = sy - 60.0;
    for t in ["Strip topsoil and unsuitable material; proof-roll exposed subgrade.", "Scarify and recompact the top 300 mm (dashed) to the specified density.", "Replace soft spots identified by proof-rolling (engineer to direct).", "Subgrade drains where the geotechnical report requires them.", "Density, moisture and CBR testing per the governing specification."] {
        para(&mut d, sx, &mut yy, 2.4, 120.0, t);
    }
    title_block_s(&mut d, info, "Pavement and subgrade details", "1:10 (layers)", page, pages);
    d
}

/// Roadside protection straight-line diagram: barrier runs, hazards and ditch linings
/// left and right of the centre line, with tables.
pub fn protection_sheet(pt: &Protection, c: &Corridor, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = new_sheet();
    let (s0, s1) = (c.sections.first().map_or(0.0, |s| s.station), c.sections.last().map_or(1.0, |s| s.station));
    let (x0, x1) = (40.0, X1 - 10.0);
    let yc = SHEET_H - 120.0;
    let tx = |s: f64| x0 + (s - s0) / (s1 - s0).max(1.0) * (x1 - x0);
    d.line("ALIGNMENT", [x0, yc], [x1, yc]);
    d.text("ALIGNMENT", [x0 - 30.0, yc - 1.0], 2.5, "CL");
    let mut st = (s0 / 100.0).ceil() * 100.0;
    while st <= s1 {
        d.line("ALIGNMENT", [tx(st), yc - 2.0], [tx(st), yc + 2.0]);
        d.text("TEXT", [tx(st) - 8.0, yc - 7.0], 2.2, &sta_label(st));
        st += 100.0;
    }
    let side_y = |side: &str, k: f64| if side == "left" { yc + 10.0 + k } else { yc - 14.0 - k };
    d.text("TEXT", [x0 - 30.0, side_y("left", 10.0)], 2.5, "LEFT");
    d.text("TEXT", [x0 - 30.0, side_y("right", 10.0)], 2.5, "RIGHT");
    for h in &pt.hazards {
        let y = side_y(&h.side, 20.0);
        d.line("HAZARD", [tx(h.station), y - 1.5], [tx(h.station), y + 1.5]);
    }
    for b in &pt.barriers {
        let y = side_y(&b.side, 8.0);
        d.poly("BARRIER", vec![[tx(b.from), y], [tx(b.to), y]]);
        d.poly("BARRIER", vec![[tx(b.from), y - 1.5], [tx(b.from), y + 1.5]]);
        d.poly("BARRIER", vec![[tx(b.to), y - 1.5], [tx(b.to), y + 1.5]]);
        d.text("BARRIER", [tx(b.from), y + 2.5], 2.0, &format!("{:.0} m", b.length_m));
    }
    for r in &pt.ditches {
        let y = side_y(&r.side, 30.0);
        d.poly("LINING", vec![[tx(r.from), y], [tx(r.to), y]]);
        d.text("LINING", [tx(r.from), y + 2.0], 2.0, &r.lining);
    }
    d.text("HAZARD", [x0, yc + 58.0], 2.4, "Ticks: hazards inside the clear zone (embankment above warrant height, steep slopes, walls). Thick lines: barrier runs incl. length of need. Blue: ditch lining reaches.");
    let mut rows = vec![["Side", "From", "To", "Length m", "LON m", "Type", "Reason"].iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for b in &pt.barriers {
        rows.push(vec![b.side.clone(), sta_label(b.from), sta_label(b.to), format!("{:.0}", b.length_m), format!("{:.1}", b.length_of_need_m), b.barrier_type.clone(), b.reason.clone()]);
    }
    let yb = table(&mut d, 20.0, yc - 70.0, &[16.0, 26.0, 26.0, 20.0, 18.0, 45.0, 120.0], 2.4, &rows);
    let mut rows = vec![["Side", "From", "To", "Q m3/s", "Depth m", "V m/s", "Slope %", "tau_d N/m2", "Lining", "tau_p N/m2"].iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for r in &pt.ditches {
        rows.push(vec![r.side.clone(), sta_label(r.from), sta_label(r.to), format!("{:.3}", r.q_max_m3s), format!("{:.2}", r.depth_m), format!("{:.2}", r.velocity_mps), format!("{:.2}", r.slope * 100.0), format!("{:.1}", r.tau_d), r.lining.clone(), if r.tau_p.is_finite() { format!("{:.0}", r.tau_p) } else { "rigid".into() }]);
    }
    let yb = table(&mut d, 20.0, yb - 8.0, &[16.0, 26.0, 26.0, 18.0, 18.0, 16.0, 16.0, 22.0, 60.0, 22.0], 2.4, &rows);
    let mut y = yb - 6.0;
    for n in pt.notes.iter().take(6) {
        para(&mut d, 20.0, &mut y, 2.2, 300.0, n);
    }
    title_block_s(&mut d, info, "Roadside protection layout (straight-line diagram)", &format!("H 1:{:.0}", ((s1 - s0) / (x1 - x0) * 1000.0 / 100.0).ceil() * 100.0), page, pages);
    d
}

/// Drainage details: pipe trench, manhole and grate inlet (schematic), plus a structure
/// schedule for every network.
pub fn drainage_details(nets: &[Network], info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = new_sheet();
    let s = 20.0; // 1:50 → 20 mm per m
    // trench
    let (tx, ty) = (60.0, SHEET_H - 150.0);
    let dmax = nets.iter().flat_map(|n| n.pipes.iter().map(|p| p.diameter_m)).fold(0.3, f64::max);
    let w = dmax + 0.6;
    d.text("TITLE", [tx - 20.0, SHEET_H - 40.0], 3.5, "PIPE TRENCH (SCHEMATIC, 1:50)");
    let (bed, cover) = (0.15, 0.3);
    let depth = bed + dmax + cover + 0.9;
    d.poly("DETAIL", vec![[tx - 1.0 * s, ty + depth * s], [tx, ty + depth * s], [tx, ty], [tx + w * s, ty], [tx + w * s, ty + depth * s], [tx + w * s + 1.0 * s, ty + depth * s]]);
    d.rect("HATCH", tx, ty, tx + w * s, ty + bed * s);
    circle(&mut d, "PIPE", [tx + w * s / 2.0, ty + (bed + dmax / 2.0) * s], dmax / 2.0 * s);
    d.line("HATCH", [tx, ty + (bed + dmax + cover) * s], [tx + w * s, ty + (bed + dmax + cover) * s]);
    for (yv, t) in [(bed / 2.0, format!("Bedding {:.0} mm", bed * 1000.0)), (bed + dmax / 2.0, format!("Pipe D up to {:.0} mm", dmax * 1000.0)), (bed + dmax + cover / 2.0, format!("Haunch/initial backfill to {:.0} mm over crown", cover * 1000.0)), (bed + dmax + cover + 0.45, "Trench backfill / pavement per typical section".into())] {
        d.text("TEXT", [tx + w * s + 26.0, ty + yv * s - 1.0], 2.2, &t);
    }
    d.text("TEXT", [tx, ty - 6.0], 2.2, &format!("Trench width D + 600 mm = {:.2} m", w));
    // manhole
    let (mx, my) = (330.0, SHEET_H - 160.0);
    d.text("TITLE", [mx - 20.0, SHEET_H - 40.0], 3.5, "MANHOLE (SCHEMATIC, 1:50)");
    let (r_b, h_b, h_c) = (0.6, 2.0, 0.6);
    d.poly("STRUCTURE", vec![[mx - r_b * s, my], [mx - r_b * s, my + h_b * s], [mx - 0.3 * s, my + (h_b + h_c) * s], [mx - 0.3 * s, my + (h_b + h_c + 0.2) * s]]);
    d.poly("STRUCTURE", vec![[mx + r_b * s, my], [mx + r_b * s, my + h_b * s], [mx + 0.3 * s, my + (h_b + h_c) * s], [mx + 0.3 * s, my + (h_b + h_c + 0.2) * s]]);
    d.rect("STRUCTURE", mx - (r_b + 0.15) * s, my - 0.2 * s, mx + (r_b + 0.15) * s, my);
    d.poly("STRUCTURE", vec![[mx - r_b * s, my + 0.3 * s], [mx - 0.25 * s, my + 0.15 * s], [mx + 0.25 * s, my + 0.15 * s], [mx + r_b * s, my + 0.3 * s]]);
    d.rect("STRUCTURE", mx - 0.4 * s, my + (h_b + h_c + 0.2) * s, mx + 0.4 * s, my + (h_b + h_c + 0.3) * s);
    for (yv, t) in [(-0.1, "Base slab 200 mm"), (0.22, "Benching to pipe soffit, channel formed"), (h_b / 2.0, "Barrel 1200 mm ID (precast)"), (h_b + h_c / 2.0, "Eccentric cone / reducer"), (h_b + h_c + 0.25, "Frame and cover to finished grade")] {
        d.text("TEXT", [mx + (r_b + 0.4) * s, my + yv * s - 1.0], 2.2, t);
    }
    // inlet
    let (ix, iy) = (600.0, SHEET_H - 150.0);
    d.text("TITLE", [ix - 20.0, SHEET_H - 40.0], 3.5, "GRATE INLET (SCHEMATIC, 1:50)");
    d.rect("STRUCTURE", ix, iy, ix + 0.9 * s, iy + 1.5 * s);
    d.rect("STRUCTURE", ix - 0.15 * s, iy - 0.15 * s, ix + 1.05 * s, iy);
    d.rect("STRUCTURE", ix, iy + 1.5 * s, ix + 0.9 * s, iy + 1.58 * s);
    d.line("CURB", [ix - 1.0 * s, iy + 1.58 * s], [ix + 1.9 * s, iy + 1.58 * s]);
    d.text("TEXT", [ix + 1.1 * s, iy + 1.6 * s], 2.2, "Grate at gutter flow line");
    d.text("TEXT", [ix + 1.1 * s, iy + 0.7 * s], 2.2, "Box 600 x 900 mm, outlet pipe to trunk");
    d.text("TEXT", [ix + 1.1 * s, iy - 0.1 * s], 2.2, "Base 150 mm");
    // schedule
    let mut rows = vec![["Network", "Structure", "Rim", "Sump invert", "Depth m", "Inverts out (pipe, D mm)"].iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for n in nets {
        for st in &n.structures {
            let outs: Vec<String> = n.pipes.iter().filter(|p| p.from == st.0).map(|p| format!("{} {:.3} D{:.0}", p.id, p.invert_up, p.diameter_m * 1000.0)).collect();
            rows.push(vec![n.name.clone(), st.0.clone(), format!("{:.3}", st.3), format!("{:.3}", st.4), format!("{:.2}", st.3 - st.4), outs.join("; ")]);
        }
    }
    let rows: Vec<Vec<String>> = rows.into_iter().take(40).collect();
    table(&mut d, 20.0, 330.0, &[30.0, 26.0, 24.0, 26.0, 20.0, 120.0], 2.4, &rows);
    let mut y = 330.0;
    para(&mut d, 280.0, &mut y, 2.4, 140.0, "Details are schematic. Final dimensions, materials, frames, grates, joint types and bedding class per the agency standard drawings and the governing specification cited on the standard-details sheet. Structure schedule lists every structure designed by the pipe-network tool (first 39 rows).");
    title_block_s(&mut d, info, "Drainage details and structure schedule", "1:50 (details)", page, pages);
    d
}

/// Standard-details atlas: schematic of each standard item with the reference to the
/// agency standard drawing / specification section for the selected profile.
pub fn details_atlas(std: Standard, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = new_sheet();
    let refs: [(&str, &str); 8] = match std {
        Standard::Us => [("Curb and gutter", "State DOT standard plan; FP-14 Section 609"), ("W-beam guardrail", "AASHTO RDG 2011 Ch. 5; FP-14 Section 617; MASH-tested system"), ("Riprap / ditch lining", "HEC-15 (2005); FP-14 Sections 251, 629"), ("Pipe trench and bedding", "FP-14 Section 602; pipe manufacturer / AASHTO LRFD Sec. 12"), ("Manhole", "FP-14 Section 604; state DOT standard plan"), ("Grate inlet", "HEC-22 (2009) inlet capacity; FP-14 Section 604"), ("Pavement edge / shoulder", "AASHTO 93 design; FP-14 Sections 301, 401"), ("Sidewalk", "FP-14 Section 615; PROWAG accessibility")],
        Standard::Ca => [("Curb and gutter", "Provincial standard drawing (engineer to cite)"), ("Roadside barrier", "TAC GDG 2017 Ch. 7; provincial barrier standard"), ("Riprap / ditch lining", "Provincial drainage manual (engineer to cite)"), ("Pipe trench and bedding", "Provincial / OPSD-type standard (engineer to cite)"), ("Manhole", "Provincial standard drawing (engineer to cite)"), ("Catch basin", "Provincial standard drawing (engineer to cite)"), ("Pavement edge / shoulder", "Provincial pavement design manual"), ("Sidewalk", "Municipal standard (engineer to cite)")],
        Standard::Uk => [("Kerb and channel", "MCHW Highway Construction Details (HCD) B series; SHW Series 1100"), ("Vehicle restraint system", "CD 377; SHW Series 400"), ("Lined ditch / erosion protection", "CD 521 / CD 524; SHW Series 500"), ("Pipe bedding", "HCD F series; SHW Series 500"), ("Chamber / manhole", "HCD F series; SHW Series 500"), ("Gully", "CD 526; HCD F series"), ("Pavement edge", "CD 226; SHW Series 700/900"), ("Footway", "SHW Series 1100")],
        Standard::Au | Standard::Nz => [("Kerb and channel", "State road authority standard drawing (engineer to cite)"), ("Safety barrier", "AGRD06 Roadside Design; AS/NZS 3845"), ("Lined drain / scour protection", "AGRD05 Drainage (engineer to cite)"), ("Pipe bedding", "AS/NZS 3725 / AS/NZS 2566 (engineer to cite)"), ("Maintenance hole", "State / council standard drawing"), ("Pit / gully", "AGRD05; council standard drawing"), ("Pavement edge", "AGPT02 design; state specification"), ("Footpath", "Council standard drawing")],
    };
    let (cw, ch) = ((X1 - 20.0) / 4.0, (SHEET_H - 40.0) / 2.0);
    for (k, (name, r)) in refs.iter().enumerate() {
        let (cx, cy) = (20.0 + (k % 4) as f64 * cw, SHEET_H - 20.0 - (k / 4 + 1) as f64 * ch);
        d.rect("GRID", cx, cy, cx + cw, cy + ch);
        d.text("TITLE", [cx + 4.0, cy + ch - 9.0], 3.2, &format!("D{} {}", k + 1, name));
        let mut y = cy + 30.0;
        para(&mut d, cx + 4.0, &mut y, 2.2, (cw - 8.0) / 2.2 / 0.62, &format!("Reference: {r}"));
        let (ox, oy) = (cx + 30.0, cy + 70.0);
        match k {
            0 => d.poly("DETAIL", vec![[ox, oy], [ox + 60.0, oy - 4.0], [ox + 60.0, oy + 15.0], [ox + 75.0, oy + 15.0], [ox + 75.0, oy - 20.0], [ox, oy - 20.0], [ox, oy]]),
            1 => {
                d.rect("DETAIL", ox + 30.0, oy - 40.0, ox + 36.0, oy + 40.0);
                d.poly("BARRIER", vec![[ox + 36.0, oy + 30.0], [ox + 50.0, oy + 32.0], [ox + 46.0, oy + 22.0], [ox + 50.0, oy + 12.0], [ox + 36.0, oy + 14.0]]);
                d.line("XS-GROUND", [ox, oy - 10.0], [ox + 90.0, oy - 10.0]);
            }
            2 => {
                d.poly("DETAIL", vec![[ox, oy + 30.0], [ox + 30.0, oy], [ox + 50.0, oy], [ox + 80.0, oy + 30.0]]);
                for i in 0..12 {
                    circle(&mut d, "LINING", [ox + 12.0 + i as f64 * 5.0, oy + 3.0 + if (12..50).contains(&(i * 5)) { 0.0 } else { 6.0 }], 2.5);
                }
            }
            3 => {
                d.poly("DETAIL", vec![[ox, oy + 40.0], [ox + 20.0, oy + 40.0], [ox + 20.0, oy - 20.0], [ox + 60.0, oy - 20.0], [ox + 60.0, oy + 40.0], [ox + 80.0, oy + 40.0]]);
                circle(&mut d, "PIPE", [ox + 40.0, oy - 2.0], 12.0);
                d.rect("HATCH", ox + 20.0, oy - 20.0, ox + 60.0, oy - 15.0);
            }
            4 => {
                d.poly("STRUCTURE", vec![[ox + 15.0, oy - 40.0], [ox + 15.0, oy + 15.0], [ox + 30.0, oy + 35.0], [ox + 30.0, oy + 42.0]]);
                d.poly("STRUCTURE", vec![[ox + 65.0, oy - 40.0], [ox + 65.0, oy + 15.0], [ox + 50.0, oy + 35.0], [ox + 50.0, oy + 42.0]]);
                d.rect("STRUCTURE", ox + 10.0, oy - 45.0, ox + 70.0, oy - 40.0);
            }
            5 => {
                d.rect("STRUCTURE", ox + 20.0, oy - 30.0, ox + 60.0, oy + 30.0);
                for i in 0..7 {
                    d.line("STRUCTURE", [ox + 22.0 + i as f64 * 6.0, oy + 30.0], [ox + 22.0 + i as f64 * 6.0, oy + 34.0]);
                }
            }
            6 => {
                for (i, t) in [8.0, 12.0, 16.0].iter().enumerate() {
                    let y0 = oy + 20.0 - [0.0, 8.0, 20.0][i];
                    d.poly("PAVEMENT", vec![[ox, y0], [ox + 70.0, y0], [ox + 70.0 + t, y0 - t]]);
                }
            }
            _ => {
                d.poly("DETAIL", vec![[ox, oy], [ox + 70.0, oy + 1.5], [ox + 70.0, oy - 10.0], [ox, oy - 10.0], [ox, oy]]);
                d.rect("HATCH", ox, oy - 18.0, ox + 70.0, oy - 10.0);
            }
        }
        d.text("TEXT", [cx + 4.0, cy + 8.0], 2.0, "Schematic only - not to scale. Use the referenced standard drawing.");
    }
    title_block_s(&mut d, info, "Standard details atlas (references)", "NTS", page, pages);
    d
}
