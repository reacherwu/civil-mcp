//! v5 tools: hydrology and culverts, slope stability, subsurface drainage, mechanistic pavement
//! check, wall stem reinforcement, marking/signing, lighting, temporary traffic management,
//! utility conflicts, land requirements, locality plan, non-US intersection sight distance,
//! UK RRRAP input export, cost plan structure and licensed-specialist hand-off packages.
//! Every engineer-supplied value is recorded with its source in the audit log.
use crate::drawing::Drawing;
use crate::sheet::{base, sta_label, title_block_s, wrap, SheetInfo, SHEET_H, SHEET_W};
use crate::standards::{chk, Check, Standard};
use crate::tools::Session;
use serde::Serialize;
use serde_json::{json, Value};
use std::fmt::Write;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Panel {
    pub title: String,
    pub polys: Vec<(String, Vec<[f64; 2]>)>,
    pub labels: Vec<(String, [f64; 2], String)>,
    pub equal: bool,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExtSheet {
    pub title: String,
    pub panels: Vec<Panel>,
    pub tables: Vec<(Vec<f64>, Vec<Vec<String>>)>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ExtOut {
    pub result: Value,
    pub checks: Vec<Check>,
    pub sheets: Vec<ExtSheet>,
}

pub const TOOLS: &[&str] = &["design_hydrology", "design_culverts", "check_slope_stability", "design_subsurface_drainage", "check_pavement_mechanistic", "design_wall_reinforcement", "design_marking_signing", "design_lighting", "plan_traffic_management", "register_utilities", "land_requirements", "locality_plan", "check_intersection_sight", "export_rrrap_inputs", "cost_plan", "specialist_handoffs", "plan_erosion_control"];

pub fn tool_list() -> Vec<Value> {
    let o = |n: &str, d: &str, req: &[&str]| json!({"name": n, "description": d, "inputSchema": {"type": "object", "properties": {}, "required": req, "additionalProperties": true}});
    vec![
        o("design_hydrology", "Peak flows at drainage crossings. Catchments are delineated on the TIN (priority-flood pit filling + D8 flow accumulation on a cell_m grid; crossings where the accumulated area at the centreline is >= min_area_ha), or given explicitly as catchments[]. Methods: rational (US/CA), NRCS TR-55 graphical peak (Type II, with p24_mm + cn), ARR 2019 rational-type with Bransby-Williams tc (AU/NZ, engineer IFD from BoM), UK: FEH/ReFH2 peak entered by the engineer (feh_peaks_m3s) with a rational sense check. Inputs: idf {label, points [[duration_min, intensity_mm_h]], source}, c, cn, p24_mm, rain_type, method, cell_m, min_area_ha.", &["idf"]),
        o("design_culverts", "FHWA HDS-5 culvert design at every fill crossing from design_hydrology: inlet control (unsubmerged form 1 / submerged) and outlet control (full-flow energy); smallest diameter from sizes_m meeting headwater <= min(hw_d_max*D, embankment - freeboard) and minimum cover. Inputs: inlet (concrete_square_headwall|concrete_groove_headwall|concrete_groove_projecting|cmp_headwall|cmp_mitered|cmp_projecting), n, sizes_m, hw_d_max, freeboard_m, min_cover_m, v_out_max_ms, tailwater_m, source.", &["inlet", "n", "sizes_m", "hw_d_max", "freeboard_m", "min_cover_m"]),
        o("check_slope_stability", "Bishop simplified and Spencer limit equilibrium on circular slip surfaces for the highest cut and fill slopes (or stations[]). Soil parameters from the geotechnical report: soil {c_kpa, phi_deg, gamma_knm3, ru, source}, geotech_report_ref; fs_min is the engineer/agency criterion. Writes slope-sections.csv for the geotechnical designer.", &["soil", "fs_min", "geotech_report_ref"]),
        o("design_subsurface_drainage", "Pavement subsurface drainage (FHWA-NHI-99-028 / AASHTO 1993 2.4): Cedergren crack infiltration, drainage-layer Darcy capacity, t50 and drainage quality, edge-drain capacity and outlet spacing. Inputs: crack_spacing_m, k_layer_m_day, layer_thickness_m, porosity, pipe_d_m, max_outlet_spacing_m, m_coefficient, t50_target_h, source.", &["crack_spacing_m", "k_layer_m_day", "layer_thickness_m", "porosity", "pipe_d_m", "max_outlet_spacing_m", "m_coefficient"]),
        o("check_pavement_mechanistic", "Screening mechanistic check (AGPT02 transfer functions, Odemark equivalent thickness + Boussinesq strains). Verify in CIRCLY / AASHTOWare ME. Inputs: layers [{name,h_m,e_mpa}], subgrade_e_mpa, vb_pct, rf, design_esa, tyre_kpa, wheel_kn, nu.", &["layers", "subgrade_e_mpa", "vb_pct", "rf", "design_esa"]),
        o("design_wall_reinforcement", "Cantilever wall stem flexural reinforcement for every corridor wall. Inputs: fc_mpa, fy_mpa, phi, load_factor, cover_m, bar_d_mm, min_ratio, gamma_knm3, phi_soil_deg, surcharge_kpa, source. Shear, cracking, footing and detailing remain with the structural engineer.", &["fc_mpa", "fy_mpa", "load_factor", "cover_m", "bar_d_mm", "min_ratio", "gamma_knm3", "phi_soil_deg"]),
        o("design_marking_signing", "Concept marking and signing: no-passing zones from passing sight distance on the profile (US MUTCD Table 3B-1; other profiles need psd_m), line colours by manual, curve warning signs below curve_warning_r_m. Inputs: psd_m, eye_h_m, min_zone_m, curve_warning_r_m.", &["eye_h_m", "min_zone_m", "curve_warning_r_m"]),
        o("design_lighting", "Concept lighting layout by the lumen method, spacing capped at max_spacing_to_height*H. Inputs: lumens, cu, llf, e_avg_lux, mounting_h_m, arrangement, max_spacing_to_height, source.", &["lumens", "cu", "llf", "e_avg_lux", "mounting_h_m", "max_spacing_to_height"]),
        o("plan_traffic_management", "Concept temporary traffic management staging with tapers (US MUTCD Part 6 formula; other profiles: taper_m from the engineer). Inputs: existing_speed_kmh, lane_w_m, taper_m.", &["existing_speed_kmh", "lane_w_m"]),
        o("register_utilities", "Utility conflict register against the design and designed pipes. Inputs: utilities [{id, kind, owner, polyline, depth_m, diameter_m, quality_level}], clearance_m, max_cover_m.", &["utilities", "clearance_m", "max_cover_m"]),
        o("land_requirements", "Land requirement plan: footprint (daylight + buffer_m), area, and take per parcel (parcels [{id, owner, polygon}]).", &["buffer_m"]),
        o("locality_plan", "Locality plan with UTM and geographic coordinates (WGS84 inverse TM). Inputs: utm_zone, north, places [{name,x,y}].", &["utm_zone", "north"]),
        o("check_intersection_sight", "Intersection sight distance by profile: US Case B1 / CA TAC time gap; AU/NZ Austroads SISD with engineer RT and d; UK CD 123 y-distance. Available = vertical sight line on the major road. Inputs: time_gap_s, reaction_s, decel_coeff, grade_pct, y_distance_m, eye_h_m, object_h_m, source.", &["eye_h_m", "object_h_m"]),
        o("export_rrrap_inputs", "UK only: rrrap-inputs.csv hazard list for the National Highways RRRAP tool (CD 377). Inputs: aadt, hgv_pct.", &["aadt", "hgv_pct"]),
        o("cost_plan", "Cost plan structure: BoQ quantities with blank rates, contingency_pct, risks [{name,p50,p90}], optimism_bias_pct (UK). No market prices supplied.", &["contingency_pct"]),
        o("plan_erosion_control", "Concept erosion and sediment control plan: perimeter sediment fence along fill toes, check dams in cut ditches at spacing L = h_dam / S (toe of the upstream dam level with the crest of the next), inlet/outlet protection at culverts, stabilised construction entrances at the tie-ins. Inputs: check_dam_h_m, fence_offset_m, source. Agency ESC manual governs (EPA CGP/SWPPP, provincial ESC guides, CIRIA C648/C532, IECA / Blue Book, regional council guidelines).", &["check_dam_h_m"]),
        o("specialist_handoffs", "Hand-off packages (data templates + deliverable checklists) for environmental assessment and bridges/major structures. LICENSED SPECIALIST - NOT PRODUCED BY THIS SOFTWARE.", &[]),
    ]
}

fn f(a: &Value, k: &str) -> Option<f64> { a.get(k).and_then(|x| x.as_f64()) }
fn req(a: &Value, k: &str) -> crate::Result<f64> { f(a, k).ok_or_else(|| format!("{k} required (engineer input)")) }
fn hdr(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
fn csvq(s: &str) -> String { if s.contains(',') || s.contains('"') { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.into() } }

// ---------------------------------------------------------------- D8 on a grid
pub struct Flow { pub nx: usize, pub ny: usize, pub x0: f64, pub y0: f64, pub cell: f64, pub zf: Vec<f64>, pub rec: Vec<Option<usize>>, pub acc: Vec<f64>, pub len: Vec<f64>, pub srcz: Vec<f64> }

fn nb8(k: usize, nx: usize, ny: usize) -> Vec<(usize, f64)> {
    let (i, j) = ((k % nx) as i64, (k / nx) as i64);
    let mut v = vec![];
    for (di, dj) in [(-1i64, -1i64), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
        let (a, b) = (i + di, j + dj);
        if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
            v.push(((b as usize) * nx + a as usize, if di != 0 && dj != 0 { std::f64::consts::SQRT_2 } else { 1.0 }));
        }
    }
    v
}

/// Priority-flood pit filling with epsilon (Barnes et al. 2014), D8 receivers, flow accumulation
/// and longest upstream flow path with its source elevation.
pub fn flow(g: &crate::route::Grid) -> Flow {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let (nx, ny) = (g.nx, g.ny);
    let n = nx * ny;
    let valid: Vec<bool> = g.z.iter().map(|z| z.is_some()).collect();
    let z0: Vec<f64> = g.z.iter().map(|z| z.unwrap_or(f64::NAN)).collect();
    let mut zf = z0.clone();
    let mut done = vec![false; n];
    let key = |v: f64| (v * 1e6).round() as i64;
    let mut pq: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
    for k in 0..n {
        if !valid[k] { continue; }
        let nb = nb8(k, nx, ny);
        if nb.len() < 8 || nb.iter().any(|(m, _)| !valid[*m]) { done[k] = true; pq.push(Reverse((key(zf[k]), k))); }
    }
    while let Some(Reverse((_, k))) = pq.pop() {
        for (m, _) in nb8(k, nx, ny) {
            if !valid[m] || done[m] { continue; }
            done[m] = true;
            if zf[m] <= zf[k] { zf[m] = zf[k] + 1e-5; }
            pq.push(Reverse((key(zf[m]), m)));
        }
    }
    let mut rec = vec![None; n];
    for k in 0..n {
        if !valid[k] { continue; }
        let mut best = 0.0;
        for (m, d) in nb8(k, nx, ny) {
            if !valid[m] { continue; }
            let s = (zf[k] - zf[m]) / (d * g.cell);
            if s > best { best = s; rec[k] = Some(m); }
        }
    }
    let mut order: Vec<usize> = (0..n).filter(|k| valid[*k]).collect();
    order.sort_by(|a, b| zf[*b].partial_cmp(&zf[*a]).unwrap().then(a.cmp(b)));
    let ca = g.cell * g.cell;
    let mut acc = vec![0.0; n];
    let mut len = vec![0.0; n];
    let mut srcz = z0.clone();
    for &k in &order { acc[k] += ca; }
    for &k in &order {
        if let Some(r) = rec[k] {
            acc[r] += acc[k];
            let step = if (r % nx != k % nx) && (r / nx != k / nx) { g.cell * std::f64::consts::SQRT_2 } else { g.cell };
            if len[k] + step > len[r] { len[r] = len[k] + step; srcz[r] = srcz[k]; }
        }
    }
    Flow { nx, ny, x0: g.x0, y0: g.y0, cell: g.cell, zf, rec, acc, len, srcz }
}

impl Flow {
    pub fn idx(&self, p: [f64; 2]) -> usize {
        let i = ((p[0] - self.x0) / self.cell).round().clamp(0.0, (self.nx - 1) as f64) as usize;
        let j = ((p[1] - self.y0) / self.cell).round().clamp(0.0, (self.ny - 1) as f64) as usize;
        j * self.nx + i
    }
    pub fn xy(&self, k: usize) -> [f64; 2] { [self.x0 + (k % self.nx) as f64 * self.cell, self.y0 + (k / self.nx) as f64 * self.cell] }
}

#[derive(Debug, Clone, Serialize)]
pub struct Crossing { pub id: String, pub station: f64, pub x: f64, pub y: f64, pub cell: usize, pub area_ha: f64, pub length_m: f64, pub slope: f64, pub fill_m: f64, pub flow_dir: [f64; 2] }

/// Crossings: maximum of accumulated area within each run of centreline samples above `min_ha`.
pub fn crossings(fl: &Flow, al: &crate::align::Alignment, pr: &crate::profile::Profile, tin: &crate::tin::Tin, min_ha: f64) -> Vec<Crossing> {
    let step = fl.cell / 2.0;
    let mut samp = vec![];
    let mut s = al.start_station();
    while s <= al.end_station() {
        if let Some((x, y, _)) = al.point_at(s) { let k = fl.idx([x, y]); samp.push((s, k, fl.acc[k] / 1e4)); }
        s += step;
    }
    let mut out: Vec<Crossing> = vec![];
    let mut i = 0;
    while i < samp.len() {
        if samp[i].2 < min_ha { i += 1; continue; }
        let mut j = i;
        while j + 1 < samp.len() && samp[j + 1].2 >= min_ha { j += 1; }
        let b = (i..=j).max_by(|a, c| samp[*a].2.partial_cmp(&samp[*c].2).unwrap().then(c.cmp(a))).unwrap();
        let (st, k, a) = samp[b];
        if let (Some((x, y, _)), Some(zfg)) = (al.point_at(st), pr.elevation_at(st)) {
            let eg = tin.elevation_at(x, y).unwrap_or(fl.zf[k]);
            let l = fl.len[k].max(fl.cell);
            let sl = ((fl.srcz[k] - fl.zf[k]) / l).max(0.002);
            let d = fl.rec[k].map_or([0.0, 0.0], |r| { let (p, q) = (fl.xy(k), fl.xy(r)); let n = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt(); [(q[0] - p[0]) / n, (q[1] - p[1]) / n] });
            out.push(Crossing { id: format!("X{}", out.len() + 1), station: st, x, y, cell: k, area_ha: a, length_m: l, slope: sl, fill_m: zfg - eg, flow_dir: d });
        }
        i = j + 1;
    }
    out
}

fn scale_pts(pts: &[[f64; 2]], ext: [f64; 4], win: [f64; 4], equal: bool) -> Vec<[f64; 2]> {
    let (sx, sy) = ((win[2] - win[0]) / (ext[2] - ext[0]).max(1e-9), (win[3] - win[1]) / (ext[3] - ext[1]).max(1e-9));
    let (sx, sy) = if equal { let m = sx.min(sy); (m, m) } else { (sx, sy) };
    pts.iter().map(|p| [win[0] + (p[0] - ext[0]) * sx, win[1] + (p[1] - ext[1]) * sy]).collect()
}

fn table(d: &mut Drawing, x: f64, y: f64, widths: &[f64], h: f64, rows: &[Vec<String>]) -> f64 {
    let rh = h * 2.0;
    let w: f64 = widths.iter().sum();
    let mut yy = y;
    for (ri, r) in rows.iter().enumerate() {
        d.line("TEXT", [x, yy], [x + w, yy]);
        let mut xx = x;
        for (ci, c) in r.iter().enumerate() {
            let em = widths.get(ci).copied().unwrap_or(20.0) / h / 0.62;
            let txt: String = if c.chars().count() as f64 > em { c.chars().take(em.max(3.0) as usize - 1).collect::<String>() + "~" } else { c.clone() };
            d.text(if ri == 0 { "TITLE" } else { "TEXT" }, [xx + 1.0, yy - rh + h * 0.5], h, &txt);
            xx += widths.get(ci).copied().unwrap_or(20.0);
        }
        yy -= rh;
    }
    d.line("TEXT", [x, yy], [x + w, yy]);
    let mut xx = x;
    for wi in std::iter::once(&0.0).chain(widths.iter()) { xx += wi; d.line("TEXT", [xx, y], [xx, yy]); }
    yy
}

pub fn render(sh: &ExtSheet, info: &SheetInfo, page: usize, pages: usize) -> Drawing {
    let mut d = Drawing::default();
    base(&mut d);
    for (l, c) in [("DETAIL", 7), ("HATCH", 8), ("CULVERT", 4), ("SLIP", 1), ("SIGN", 2), ("LIGHT", 2), ("UTILITY", 6), ("ROW", 1), ("PARCEL", 3), ("TTM", 30), ("GRID", 8), ("PAVEMENT", 2), ("SUBGRADE", 8), ("ROUTE-SELECTED", 6)] { d.layer(l, c); }
    let x1 = SHEET_W - 160.0;
    let np = sh.panels.len().max(1) as f64;
    let top = SHEET_H - 20.0;
    let ph = if sh.tables.is_empty() && sh.notes.is_empty() { SHEET_H - 60.0 } else { 300.0 };
    let pw = (x1 - 30.0 - 10.0 * (np - 1.0)) / np;
    for (k, p) in sh.panels.iter().enumerate() {
        let wx = 20.0 + k as f64 * (pw + 10.0);
        let win = [wx + 8.0, top - ph + 10.0, wx + pw - 8.0, top - 14.0];
        d.poly("DETAIL", vec![[wx, top - ph], [wx + pw, top - ph], [wx + pw, top], [wx, top], [wx, top - ph]]);
        d.text("TITLE", [wx + 3.0, top - 8.0], 3.5, &p.title);
        let all: Vec<[f64; 2]> = p.polys.iter().flat_map(|(_, v)| v.iter().copied()).chain(p.labels.iter().map(|l| l.1)).filter(|q| q[0].is_finite() && q[1].is_finite()).collect();
        if all.is_empty() { continue; }
        let mut ext = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for q in &all { ext[0] = ext[0].min(q[0]); ext[1] = ext[1].min(q[1]); ext[2] = ext[2].max(q[0]); ext[3] = ext[3].max(q[1]); }
        let (dx, dy) = ((ext[2] - ext[0]).max(1.0) * 0.03, (ext[3] - ext[1]).max(1.0) * 0.05);
        ext = [ext[0] - dx, ext[1] - dy, ext[2] + dx, ext[3] + dy];
        for (l, v) in &p.polys { if v.len() > 1 { d.poly(l, scale_pts(v, ext, win, p.equal)); } }
        for (l, q, t) in &p.labels { let s = scale_pts(&[*q], ext, win, p.equal)[0]; d.text(l, [s[0] + 1.0, s[1] + 1.0], 2.2, t); }
        if p.equal {
            let sc = ((win[2] - win[0]) / (ext[2] - ext[0])).min((win[3] - win[1]) / (ext[3] - ext[1]));
            let mut bar = 10f64.powf((40.0 / sc).log10().floor());
            if bar * sc < 20.0 { bar *= 2.0; }
            let (bx, by) = (win[0], win[1] - 5.0);
            d.poly("TEXT", vec![[bx, by], [bx + bar * sc, by]]);
            d.text("TEXT", [bx + bar * sc + 2.0, by - 1.0], 2.0, &format!("{bar} m"));
            d.poly("TEXT", vec![[win[2] - 4.0, win[3] - 14.0], [win[2] - 4.0, win[3] - 4.0]]);
            d.text("TEXT", [win[2] - 5.5, win[3] - 3.0], 2.5, "N");
        }
        if !p.note.is_empty() { d.text("TEXT", [wx + 3.0, top - ph + 2.0], 2.0, &p.note); }
    }
    let mut y = top - ph - 8.0;
    for (w, rows) in &sh.tables {
        if y < 40.0 { break; }
        let take = (((y - 30.0) / 4.8) as usize).min(rows.len());
        y = table(&mut d, 20.0, y, w, 2.4, &rows[..take]) - 6.0;
    }
    for n in &sh.notes {
        for line in wrap(n, 280.0) { if y < 18.0 { break; } d.text("TEXT", [20.0, y], 2.2, &line); y -= 3.5; }
    }
    title_block_s(&mut d, info, &sh.title, "as shown", page, pages);
    d
}

fn cl_poly(al: &crate::align::Alignment, step: f64) -> Vec<[f64; 2]> {
    let mut v = vec![];
    let mut s = al.start_station();
    while s < al.end_station() { if let Some((x, y, _)) = al.point_at(s) { v.push([x, y]); } s += step; }
    if let Some((x, y, _)) = al.point_at(al.end_station()) { v.push([x, y]); }
    v
}

fn cl_panel(s: &Session, title: &str) -> Panel {
    let mut p = Panel { title: title.into(), equal: true, ..Default::default() };
    if let Some(al) = &s.alignment {
        p.polys.push(("ALIGNMENT".into(), cl_poly(al, 5.0)));
        let mut st = (al.start_station() / 200.0).ceil() * 200.0;
        while st <= al.end_station() { if let Some((x, y, _)) = al.point_at(st) { p.labels.push(("TEXT".into(), [x, y], sta_label(st))); } st += 200.0; }
    }
    if let Some(c) = &s.corridor {
        for left in [true, false] {
            let v: Vec<[f64; 2]> = c.sections.iter().map(|q| { let o = if left { q.left.daylight } else { -q.right.daylight }; [q.x - o * q.heading.sin(), q.y + o * q.heading.cos()] }).collect();
            p.polys.push(("HATCH".into(), v));
        }
    }
    p
}

fn nearest_section(c: &crate::corridor::Corridor, st: f64) -> &crate::corridor::Section {
    c.sections.iter().min_by(|a, b| (a.station - st).abs().partial_cmp(&(b.station - st).abs()).unwrap()).unwrap()
}

fn zs_at(p: &[[f64; 2]], x: f64) -> f64 {
    for w in p.windows(2) { if x >= w[0][0] && x <= w[1][0] { let t = (x - w[0][0]) / (w[1][0] - w[0][0]).max(1e-9); return w[0][1] + t * (w[1][1] - w[0][1]); } }
    p.last().map_or(0.0, |q| q[1])
}

/// Vertical sight distance along the profile from station st in direction dir.
pub fn sight_dir(pr: &crate::profile::Profile, st: f64, eh: f64, oh: f64, dir: f64) -> f64 {
    let (s0, s1) = (pr.start(), pr.end());
    let ze = pr.elevation_at(st).unwrap_or(0.0) + eh;
    let mut d = 0.0;
    loop {
        let t = st + dir * (d + 1.0);
        if t < s0 || t > s1 || d >= 2000.0 { return d; }
        let zo = pr.elevation_at(t).unwrap_or(f64::NAN) + oh;
        let n = (d + 1.0) as usize;
        let ok = (1..n).all(|k| { let u = st + dir * k as f64; let z = pr.elevation_at(u).unwrap_or(f64::NAN); ze + (zo - ze) * k as f64 / n as f64 > z });
        if !ok { return d; }
        d += 1.0;
    }
}

type Files = Vec<(String, Vec<u8>)>;
type B = crate::standards::DesignBasis;

impl Session {
    pub fn ext_checks(&self) -> Vec<Check> { self.ext.values().flat_map(|e| e.checks.iter().cloned()).collect() }
    pub fn ext_sheets(&self) -> Vec<ExtSheet> {
        let order = ["locality_plan", "check_intersection_sight", "design_hydrology", "design_culverts", "check_slope_stability", "design_subsurface_drainage", "design_wall_reinforcement", "design_marking_signing", "design_lighting", "register_utilities", "land_requirements", "plan_erosion_control", "plan_traffic_management"];
        order.iter().filter_map(|k| self.ext.get(*k)).flat_map(|e| e.sheets.iter().cloned()).collect()
    }

    pub fn call_ext(&mut self, name: &str, a: &Value) -> Option<crate::Result<Value>> {
        if !TOOLS.contains(&name) { return None; }
        Some(self.ext_run(name, a))
    }

    fn ext_run(&mut self, name: &str, a: &Value) -> crate::Result<Value> {
        let b = self.need_basis()?.clone();
        let mut o = ExtOut::default();
        let mut files: Files = vec![];
        match name {
            "design_hydrology" => self.x_hydrology(a, &b, &mut o, &mut files)?,
            "design_culverts" => self.x_culverts(a, &b, &mut o, &mut files)?,
            "check_slope_stability" => self.x_slope(a, &b, &mut o, &mut files)?,
            "design_subsurface_drainage" => self.x_subsurface(a, &b, &mut o)?,
            "check_pavement_mechanistic" => self.x_mech(a, &mut o)?,
            "design_wall_reinforcement" => self.x_rebar(a, &mut o, &mut files)?,
            "design_marking_signing" => self.x_marking(a, &b, &mut o, &mut files)?,
            "design_lighting" => self.x_lighting(a, &mut o, &mut files)?,
            "plan_traffic_management" => self.x_ttm(a, &b, &mut o)?,
            "register_utilities" => self.x_utilities(a, &mut o, &mut files)?,
            "land_requirements" => self.x_land(a, &mut o, &mut files)?,
            "locality_plan" => self.x_locality(a, &mut o)?,
            "check_intersection_sight" => self.x_isd(a, &b, &mut o)?,
            "export_rrrap_inputs" => self.x_rrrap(a, &b, &mut o, &mut files)?,
            "cost_plan" => self.x_cost(a, &mut o, &mut files)?,
            "specialist_handoffs" => self.x_handoffs(&b, &mut o, &mut files)?,
            "plan_erosion_control" => self.x_esc(a, &mut o, &mut files)?,
            _ => return Err(format!("unknown tool {name}")),
        }
        let mut fl = json!({});
        for (n, bytes) in files { fl[&n] = json!(self.write(&n, &bytes)?); }
        let mut res = o.result.clone();
        res["files"] = fl;
        res["checks"] = json!(o.checks.iter().map(|c| json!({"id": c.id, "element": c.element, "value": c.value, "limit": c.limit, "pass": c.pass, "severity": c.severity})).collect::<Vec<_>>());
        res["sheets"] = json!(o.sheets.iter().map(|s| s.title.clone()).collect::<Vec<_>>());
        o.result = res.clone();
        self.ext.insert(name.into(), o);
        let prov = self.log("agent", name, a, &res, &[])?;
        Ok(json!({"result": res, "provenance": prov}))
    }

    fn x_hydrology(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let idf: crate::hydrology::Idf = serde_json::from_value(a["idf"].clone()).map_err(|e| format!("idf: {e}"))?;
        let cats: Vec<crate::hydrology::CatchmentInput>;
        let mut xs: Vec<Crossing> = vec![];
        let mut sheet = ExtSheet { title: "Drainage area map and hydrology data".into(), ..Default::default() };
        if let Some(c) = a.get("catchments").filter(|v| v.is_array()) {
            cats = serde_json::from_value(c.clone()).map_err(|e| format!("catchments: {e}"))?;
        } else {
            let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
            let al = self.alignment.as_ref().ok_or("create_alignment first")?;
            let pr = self.profile.as_ref().ok_or("create_profile first")?;
            let cell = f(a, "cell_m").unwrap_or(10.0);
            let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for p in &t.pts { bb[0] = bb[0].min(p[0]); bb[1] = bb[1].min(p[1]); bb[2] = bb[2].max(p[0]); bb[3] = bb[3].max(p[1]); }
            let g = crate::route::Grid::from_tin(t, cell, bb);
            let fl = flow(&g);
            xs = crossings(&fl, al, pr, t, f(a, "min_area_ha").unwrap_or(2.0));
            let mut gcsv = format!("# priority-flood + D8 grid, cell {cell} m, origin ({:.3}, {:.3})\ni,j,z_raw,z_filled,receiver,acc_m2,flow_len_m\n", g.x0, g.y0);
            for k in 0..fl.zf.len() {
                if fl.zf[k].is_finite() { let _ = writeln!(gcsv, "{},{},{:.4},{:.6},{},{:.1},{:.2}", k % fl.nx, k / fl.nx, g.z[k].unwrap_or(f64::NAN), fl.zf[k], fl.rec[k].map_or(-1, |r| r as i64), fl.acc[k], fl.len[k]); }
            }
            files.push(("flow-grid.csv".into(), gcsv.into_bytes()));
            let feh: Vec<f64> = a["feh_peaks_m3s"].as_array().map(|v| v.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
            cats = xs.iter().enumerate().map(|(i, x)| crate::hydrology::CatchmentInput { id: x.id.clone(), area_ha: x.area_ha, c: f(a, "c"), cn: f(a, "cn"), length_m: x.length_m, slope: x.slope, tc_min: None, feh_peak_m3s: feh.get(i).copied() }).collect();
            let mut p = cl_panel(self, "Drainage area map (D8 flow paths >= 0.5 ha, crossings)");
            for k in 0..fl.zf.len() {
                if fl.acc[k] >= 5000.0 { if let Some(r) = fl.rec[k] { p.polys.push(("CULVERT".into(), vec![fl.xy(k), fl.xy(r)])); } }
            }
            for x in &xs { p.labels.push(("CULVERT".into(), [x.x, x.y], format!("{} {:.1} ha", x.id, x.area_ha))); }
            sheet.panels.push(p);
        }
        let hin = crate::hydrology::HydroInput { catchments: cats, idf, p24_mm: f(a, "p24_mm"), p2yr24_mm: f(a, "p2yr24_mm"), rain_type: a["rain_type"].as_str().unwrap_or("II").into(), method: a["method"].as_str().map(String::from) };
        let (res, cite) = crate::hydrology::design(b.standard, &hin)?;
        let mut rows = vec![hdr(&["ID", "Sta", "A ha", "L m", "S", "tc min", "i mm/h", "Q m3/s", "Method"])];
        let mut hc = String::from("id,station,area_ha,length_m,slope,tc_min,intensity_mm_h,runoff_mm,q_m3s,q_rational_m3s,method\n");
        for (k, r) in res.iter().enumerate() {
            let x = xs.get(k);
            rows.push(vec![r.id.clone(), x.map_or("-".into(), |x| sta_label(x.station)), format!("{:.2}", r.area_ha), x.map_or("-".into(), |x| format!("{:.0}", x.length_m)), x.map_or("-".into(), |x| format!("{:.4}", x.slope)), format!("{:.1}", r.tc_min), r.intensity_mmh.map_or("-".into(), |v| format!("{v:.1}")), format!("{:.3}", r.q_m3s), r.method.clone()]);
            let _ = writeln!(hc, "{},{},{:.4},{:.3},{:.6},{:.4},{:.4},{},{:.6},{},{}", r.id, x.map_or(String::new(), |x| format!("{:.2}", x.station)), r.area_ha, x.map_or(0.0, |x| x.length_m), x.map_or(0.0, |x| x.slope), r.tc_min, r.intensity_mmh.unwrap_or(f64::NAN), r.runoff_mm.map_or(String::new(), |v| format!("{v:.4}")), r.q_m3s, r.q_rational_m3s.map_or(String::new(), |v| format!("{v:.6}")), r.method);
        }
        files.push(("hydrology.csv".into(), hc.into_bytes()));
        sheet.tables.push((vec![14.0, 24.0, 16.0, 16.0, 16.0, 16.0, 18.0, 18.0, 30.0], rows));
        sheet.notes = vec![format!("Method: {cite}. IFD: {} ({}). Runoff coefficient / curve number and IFD are engineer inputs.", hin.idf.label, hin.idf.source), "Catchments from priority-flood + D8 on the survey TIN; confirm by site inspection and mapping. Area outside the survey extent is not captured.".into()];
        o.sheets.push(sheet);
        o.result = json!({"method_citation": cite, "crossings": xs, "catchments": res});
        Ok(())
    }
}

impl Session {
    fn x_culverts(&self, a: &Value, _b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let h = &self.ext.get("design_hydrology").ok_or("design_hydrology first")?.result;
        let xs: Vec<Value> = h["crossings"].as_array().cloned().unwrap_or_default();
        let cs: Vec<Value> = h["catchments"].as_array().cloned().unwrap_or_default();
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
        let inlet: crate::culvert::Inlet = serde_json::from_value(a["inlet"].clone()).map_err(|e| format!("inlet: {e}"))?;
        let n = req(a, "n")?;
        let sizes: Vec<f64> = a["sizes_m"].as_array().ok_or("sizes_m required")?.iter().filter_map(|x| x.as_f64()).collect();
        if sizes.is_empty() { return Err("sizes_m empty".into()); }
        let (hwd, fb, mc) = (req(a, "hw_d_max")?, req(a, "freeboard_m")?, req(a, "min_cover_m")?);
        let vmax = f(a, "v_out_max_ms");
        let tw = f(a, "tailwater_m").unwrap_or(0.0);
        let wall = f(a, "wall_t_m").unwrap_or(0.1);
        let src = a["source"].as_str().unwrap_or("engineer");
        let pav = c.assembly.pavement_thickness();
        let mut rows = vec![hdr(&["ID", "Sta", "Q m3/s", "D m", "L m", "S %", "Inv in", "Inv out", "HWi m", "HWo m", "HW/D", "Ctrl", "V out", "Cover m"])];
        let mut out = vec![];
        let mut pl = cl_panel(self, "Culvert layout");
        let mut prof = Panel { title: "Longitudinal section, largest-flow culvert (V = H)".into(), equal: true, ..Default::default() };
        let mut ccsv = String::from("id,station,q_m3s,d_m,length_m,slope,inv_in,inv_out,hw_inlet_m,hw_outlet_m,hw_m,control,v_out_ms,cover_m,allow_hw_m,x_in,y_in,x_out,y_out,cl_elev,subgrade_elev,inlet\n");
        let mut qgov = -1.0;
        for (x, q) in xs.iter().zip(cs.iter()) {
            let st = x["station"].as_f64().unwrap_or(0.0);
            let id = x["id"].as_str().unwrap_or("").to_string();
            let qd = q["q_m3s"].as_f64().unwrap_or(0.0);
            let sec = nearest_section(c, st);
            if x["fill_m"].as_f64().unwrap_or(0.0) <= 0.3 {
                out.push(json!({"id": id, "station": st, "status": "in cut / at grade: flow intercepted by the roadside ditch - route to the nearest culvert or outfall"}));
                rows.push(vec![id, sta_label(st), format!("{qd:.3}"), "-".into(), "-".into(), "-".into(), "-".into(), "-".into(), "-".into(), "-".into(), "-".into(), "in cut".into(), "-".into(), "-".into()]);
                continue;
            }
            let nv = [-sec.heading.sin(), sec.heading.cos()];
            let fd = [x["flow_dir"][0].as_f64().unwrap_or(0.0), x["flow_dir"][1].as_f64().unwrap_or(0.0)];
            // flow towards the right side (against the left normal) => inlet on the left
            let up_left = fd[0] * nv[0] + fd[1] * nv[1] <= 0.0;
            let (ou, od) = if up_left { (sec.left.daylight, -sec.right.daylight) } else { (-sec.right.daylight, sec.left.daylight) };
            let pin = [sec.x + ou * nv[0], sec.y + ou * nv[1]];
            let pout = [sec.x + od * nv[0], sec.y + od * nv[1]];
            let l = (ou - od).abs();
            let zin = t.elevation_at(pin[0], pin[1]).unwrap_or(sec.ground_cl);
            let zout0 = t.elevation_at(pout[0], pout[1]).unwrap_or(sec.ground_cl);
            let s = ((zin - zout0) / l).max(0.005);
            let zout = zin - s * l;
            let sub = sec.cl_elev - pav;
            let zcl = zin - s * ou.abs();
            let allow_for = |d: f64| (hwd * d).min(sec.cl_elev - fb - zin);
            let cover_for = |d: f64| sub - (zcl + d + wall);
            let mut pick = sizes.len() - 1;
            for (i, &d) in sizes.iter().enumerate() {
                let r = crate::culvert::analyse(inlet, d, qd, n, l, s, tw);
                if r.hw_m <= allow_for(d) && cover_for(d) >= mc { pick = i; break; }
            }
            let d = sizes[pick];
            let r = crate::culvert::analyse(inlet, d, qd, n, l, s, tw);
            let (allow, cover) = (allow_for(d), cover_for(d));
            o.checks.push(chk("C-HW", "Culvert headwater within allowable", format!("culvert {id} at {}", sta_label(st)), r.hw_m, allow, "m", "max", "mandatory", format!("HW = max(HWi {:.3}, HWo {:.3}); allowable = min({hwd}*D, road - {fb} m freeboard - inlet invert)", r.hw_inlet_m, r.hw_outlet_m), crate::culvert::HDS5.into()));
            o.checks.push(chk("C-COVER", "Culvert cover under pavement", format!("culvert {id}"), cover, mc, "m", "min", "mandatory", "subgrade - (invert at CL + D + wall)".into(), format!("engineer minimum cover ({src})")));
            if let Some(vm) = vmax { o.checks.push(chk("C-VOUT", "Culvert outlet velocity (scour / dissipator need)", format!("culvert {id}"), r.v_out_ms, vm, "m/s", "max", "advisory", "outlet velocity at normal depth (HDS-5)".into(), format!("engineer limit; HEC-14 energy dissipator if exceeded ({src})"))); }
            let _ = writeln!(ccsv, "{id},{st:.2},{qd:.6},{:.3},{l:.4},{s:.6},{zin:.4},{zout:.4},{:.5},{:.5},{:.5},{},{:.5},{cover:.4},{allow:.4},{:.3},{:.3},{:.3},{:.3},{:.4},{sub:.4},{}", r.d_m, r.hw_inlet_m, r.hw_outlet_m, r.hw_m, r.control, r.v_out_ms, pin[0], pin[1], pout[0], pout[1], sec.cl_elev, a["inlet"].as_str().unwrap_or(""));
            rows.push(vec![id.clone(), sta_label(st), format!("{qd:.3}"), format!("{:.2}", r.d_m), format!("{l:.1}"), format!("{:.2}", s * 100.0), format!("{zin:.2}"), format!("{zout:.2}"), format!("{:.2}", r.hw_inlet_m), format!("{:.2}", r.hw_outlet_m), format!("{:.2}", r.hw_m / r.d_m), r.control.clone(), format!("{:.2}", r.v_out_ms), format!("{cover:.2}")]);
            pl.polys.push(("CULVERT".into(), vec![pin, pout]));
            pl.labels.push(("CULVERT".into(), pin, format!("{id} {:.0} mm", r.d_m * 1000.0)));
            if qd > qgov {
                qgov = qd;
                let (mut gr, mut rd) = (vec![], vec![]);
                for i in 0..=60 {
                    let o_ = ou + (od - ou) * i as f64 / 60.0;
                    let p = [sec.x + o_ * nv[0], sec.y + o_ * nv[1]];
                    let ch = (o_ - ou).abs();
                    if let Some(z) = t.elevation_at(p[0], p[1]) { gr.push([ch, z]); }
                    let side = if o_ >= 0.0 { &sec.left } else { &sec.right };
                    rd.push([ch, crate::corridor::interp(&side.fg, o_.abs())]);
                }
                prof.polys = vec![("HATCH".into(), gr), ("PAVEMENT".into(), rd), ("CULVERT".into(), vec![[0.0, zin], [l, zout]]), ("CULVERT".into(), vec![[0.0, zin + d], [l, zout + d]]), ("SLIP".into(), vec![[-4.0, zin + r.hw_m], [0.0, zin + r.hw_m]])];
                prof.labels = vec![("CULVERT".into(), [0.0, zin], format!("{id} inv {zin:.2}")), ("SLIP".into(), [-4.0, zin + r.hw_m], format!("HW {:.2} ({} control)", zin + r.hw_m, r.control)), ("CULVERT".into(), [l, zout], format!("inv {zout:.2}"))];
            }
            out.push(json!({"id": id, "station": st, "q_m3s": qd, "length_m": l, "slope": s, "inv_in": zin, "inv_out": zout, "allow_hw_m": allow, "cover_m": cover, "hyd": r}));
        }
        files.push(("culverts.csv".into(), ccsv.into_bytes()));
        o.sheets.push(ExtSheet { title: "Culvert layout, section and schedule".into(), panels: vec![pl, prof], tables: vec![(vec![12.0, 22.0, 16.0, 12.0, 14.0, 12.0, 17.0, 17.0, 14.0, 14.0, 13.0, 14.0, 14.0, 14.0], rows)], notes: vec![format!("FHWA HDS-5 (3rd ed.): inlet control unsubmerged Form 1 for Q/(A D^0.5) <= 3.5 (Ku 1.811 SI), submerged for >= 4.0, linear between; outlet control full-flow energy with ho = max(TW, (dc+D)/2). Inlet {:?}, n {n}, TW {tw} m. Sizes, HW/D, freeboard and cover are engineer inputs ({src}).", inlet), "Structural class (fill height, bedding, end treatment) per agency standard drawings; culverts above the agency structure threshold go to the bridge/structures hand-off.".into()] });
        o.result = json!({"culverts": out});
        Ok(())
    }

    fn x_slope(&self, a: &Value, _b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let soil: crate::slope::Soil = serde_json::from_value(a["soil"].clone()).map_err(|e| format!("soil: {e}"))?;
        let fsmin = req(a, "fs_min")?;
        let rref = a["geotech_report_ref"].as_str().ok_or("geotech_report_ref required")?.to_string();
        let mut cand: Vec<(f64, bool, f64, bool)> = vec![];
        for s in &c.sections {
            for left in [true, false] {
                let sd = if left { &s.left } else { &s.right };
                if sd.fg.len() < 2 || sd.wall.is_some() { continue; }
                let hinge = sd.fg[sd.fg.len() - 2];
                let dl = sd.fg[sd.fg.len() - 1];
                cand.push((s.station, left, (dl[1] - hinge[1]).abs(), sd.cut));
            }
        }
        let mut pick: Vec<(f64, bool, f64, bool)> = vec![];
        if let Some(st) = a["stations"].as_array() {
            for v in st.iter().filter_map(|x| x.as_f64()) {
                for left in [true, false] {
                    if let Some(cc) = cand.iter().filter(|q| q.1 == left).min_by(|p, q| (p.0 - v).abs().partial_cmp(&(q.0 - v).abs()).unwrap()) { pick.push(*cc); }
                }
            }
        } else {
            for cut in [true, false] {
                let mut v: Vec<_> = cand.iter().filter(|q| q.3 == cut).cloned().collect();
                v.sort_by(|p, q| q.2.partial_cmp(&p.2).unwrap());
                for q in v {
                    if q.2 > 1.0 && !pick.iter().any(|p| p.3 == cut && (p.0 - q.0).abs() < 50.0) && pick.iter().filter(|p| p.3 == cut).count() < 3 { pick.push(q); }
                }
            }
        }
        let mut rows = vec![hdr(&["Sta", "Side", "Type", "H m", "FS Bishop", "FS Spencer", "theta deg", "FS min", "Xc", "Zc", "R"])];
        let mut scsv = String::from("# section geometry for the geotechnical designer (x along the slope section, elevation); slip circles from this tool for comparison\nstation,side,kind,point,x_m,elev_m\n");
        let mut res = vec![];
        let mut panels = vec![];
        for (st, left, hgt, cut) in pick {
            let s = c.sections.iter().find(|q| q.station == st).unwrap();
            let sd = if left { &s.left } else { &s.right };
            let dl = sd.daylight;
            let ext = dl + 2.5 * hgt.max(3.0) + 10.0;
            let sign = if left { 1.0 } else { -1.0 };
            let mut prof: Vec<[f64; 2]> = sd.fg.clone();
            let mut gs: Vec<[f64; 2]> = s.ground.iter().filter(|g| g[0] * sign > dl + 0.01 && g[0].abs() <= ext).map(|g| [g[0].abs(), g[1]]).collect();
            gs.sort_by(|p, q| p[0].partial_cmp(&q[0]).unwrap());
            prof.extend(gs);
            let ext = prof.last().map_or(ext, |q| q[0]);
            // x increasing towards the uphill side: cut slopes rise outward; fill slopes are mirrored
            let p2: Vec<[f64; 2]> = if cut { prof.clone() } else { prof.iter().rev().map(|p| [ext - p[0], p[1]]).collect() };
            let hinge = sd.fg[sd.fg.len() - 2][0];
            let (toe, crest) = if cut { (hinge, dl) } else { (ext - dl, ext - hinge) };
            let side = if left { "L" } else { "R" };
            let kind = if cut { "cut" } else { "fill" };
            for (i, p) in p2.iter().enumerate() { let _ = writeln!(scsv, "{st:.2},{side},{kind},{i},{:.4},{:.4}", p[0], p[1]); }
            let Some(cr) = crate::slope::critical(&p2, &soil, toe, crest) else { continue };
            o.checks.push(chk("S-FS", "Slope stability factor of safety (Bishop simplified)", format!("{kind} slope {} {side}", sta_label(st)), cr.fs_bishop, fsmin, "", "min", "mandatory", format!("critical circle xc {:.2}, zc {:.2}, R {:.2}; Spencer FS {:.3}", cr.xc, cr.zc, cr.r, cr.fs_spencer), format!("{}; parameters from {rref}", crate::slope::BISHOP)));
            rows.push(vec![sta_label(st), side.into(), kind.into(), format!("{hgt:.1}"), format!("{:.3}", cr.fs_bishop), format!("{:.3}", cr.fs_spencer), format!("{:.1}", cr.theta_deg), format!("{fsmin:.2}"), format!("{:.1}", cr.xc), format!("{:.1}", cr.zc), format!("{:.1}", cr.r)]);
            if panels.len() < 2 {
                let mut p = Panel { title: format!("{kind} {} {side}: FS {:.2} Bishop / {:.2} Spencer", sta_label(st), cr.fs_bishop, cr.fs_spencer), equal: true, ..Default::default() };
                p.polys.push(("PAVEMENT".into(), p2.clone()));
                let arc: Vec<[f64; 2]> = (0..=60).map(|k| { let x = cr.x_entry + (cr.x_exit - cr.x_entry) * k as f64 / 60.0; [x, cr.zc - (cr.r * cr.r - (x - cr.xc).powi(2)).max(0.0).sqrt()] }).collect();
                p.polys.push(("SLIP".into(), arc));
                p.polys.push(("SLIP".into(), vec![[cr.x_entry, zs_at(&p2, cr.x_entry)], [cr.xc, cr.zc], [cr.x_exit, zs_at(&p2, cr.x_exit)]]));
                p.labels.push(("SLIP".into(), [cr.xc, cr.zc], format!("centre, R {:.1} m", cr.r)));
                panels.push(p);
            }
            res.push(json!({"station": st, "side": side, "kind": kind, "height_m": hgt, "toe_x": toe, "crest_x": crest, "profile": p2, "circle": cr}));
        }
        files.push(("slope-sections.csv".into(), scsv.into_bytes()));
        o.sheets.push(ExtSheet { title: "Slope stability - critical slip circles".into(), panels, tables: vec![(vec![24.0, 12.0, 14.0, 14.0, 20.0, 20.0, 18.0, 16.0, 18.0, 18.0, 16.0], rows)], notes: vec![format!("Soil c' {} kPa, phi' {} deg, gamma {} kN/m3, ru {} from {rref} ({}). Homogeneous soil, circular surfaces, pore pressure by ru; Bishop simplified and Spencer (constant interslice inclination). Layering, water tables, seismic and reinforcement need the geotechnical designer's analysis using slope-sections.csv.", soil.c_kpa, soil.phi_deg, soil.gamma_knm3, soil.ru, soil.source), format!("Criterion FS >= {fsmin} (engineer / agency).")] });
        o.result = json!({"soil": soil, "geotech_report_ref": rref, "fs_min": fsmin, "sections": res, "geotech_interface": {"needed_from_report": ["unit boundaries per section", "c', phi', gamma per unit", "groundwater level or ru", "seismic coefficient", "required FS per agency"], "returned_to_geotech": "slope-sections.csv"}});
        Ok(())
    }
}

impl Session {
    fn x_subsurface(&self, a: &Value, b: &B, o: &mut ExtOut) -> crate::Result<()> {
        let inp: crate::subsurface::SubInput = serde_json::from_value(a.clone()).map_err(|e| format!("input: {e}"))?;
        let asm = self.assembly.as_ref().ok_or("create_assembly first")?;
        let pr = self.profile.as_ref().ok_or("create_profile first")?;
        let w = asm.paved_width() / 2.0;
        let nl = (asm.lanes_m.len() / 2).max(1);
        let g = pr.grades_pct.iter().map(|x| x.abs() / 100.0).fold(0.0, f64::max);
        let r = crate::subsurface::design(&inp, w, nl, b.normal_crown, g);
        o.checks.push(chk("D-SUBCAP", "Drainage layer capacity >= infiltration", "pavement drainage layer".into(), r.layer_capacity_m3_day_per_m, r.inflow_m3_day_per_m, "m3/day/m", "min", "mandatory", "Darcy q = k*S*t vs Cedergren qi*W".into(), crate::subsurface::FHWA_SUB.into()));
        let tq = f(a, "t50_target_h").unwrap_or(24.0);
        o.checks.push(chk("D-T50", "Time to drain 50% (quality of drainage)", "pavement drainage layer".into(), r.t50_hours, tq, "h", "max", "advisory", "t50 approximation (FHWA)".into(), "AASHTO 1993 Guide §2.4.1 quality of drainage; target engineer".into()));
        let mut p = Panel { title: "Edge drain detail (schematic, NTS)".into(), equal: true, ..Default::default() };
        let t_ = inp.layer_thickness_m;
        let cs = b.normal_crown;
        let ze = -w * cs;
        let x0 = (w - 1.5).max(0.0);
        let zx0 = -x0 * cs;
        p.polys.push(("PAVEMENT".into(), vec![[x0, zx0], [w, ze]]));
        p.polys.push(("HATCH".into(), vec![[x0, zx0 - 0.3], [w + 0.3, ze - 0.3 - 0.3 * cs]]));
        p.polys.push(("HATCH".into(), vec![[x0, zx0 - 0.3 - t_], [w + 0.3, ze - 0.3 - t_ - 0.3 * cs]]));
        let (ex, ey) = (w + 0.15, ze - 0.3 - t_ - 0.25 - inp.pipe_d_m / 2.0);
        p.polys.push(("CULVERT".into(), (0..=32).map(|k| { let q = k as f64 / 32.0 * std::f64::consts::TAU; [ex + inp.pipe_d_m / 2.0 * q.cos(), ey + inp.pipe_d_m / 2.0 * q.sin()] }).collect()));
        p.polys.push(("DETAIL".into(), vec![[w, ze - 0.3], [w, ey - inp.pipe_d_m / 2.0 - 0.1], [w + 0.3, ey - inp.pipe_d_m / 2.0 - 0.1], [w + 0.3, ze - 0.3]]));
        p.labels.push(("TEXT".into(), [x0, zx0 - 0.3 - t_ - 0.15], format!("drainage layer {:.0} mm, k {} m/day, ne {}", t_ * 1000.0, inp.k_layer_m_day, inp.porosity)));
        p.labels.push(("TEXT".into(), [x0, ey], format!("edge drain {:.0} mm in filter trench", inp.pipe_d_m * 1000.0)));
        let rows = vec![hdr(&["Item", "Value"]), vec!["qi (Cedergren)".into(), format!("{:.4} m3/day/m2", r.qi_m3_day_m2)], vec!["inflow per m".into(), format!("{:.4} m3/day/m", r.inflow_m3_day_per_m)], vec!["layer capacity".into(), format!("{:.4} m3/day/m", r.layer_capacity_m3_day_per_m)], vec!["resultant slope / path".into(), format!("{:.4} / {:.2} m", r.resultant_slope, r.flow_path_m)], vec!["t50".into(), format!("{:.2} h", r.t50_hours)], vec!["edge drain capacity".into(), format!("{:.4} m3/s", r.pipe_capacity_m3s)], vec!["outlet spacing".into(), format!("{:.0} m", r.outlet_spacing_m)], vec!["AASHTO m (engineer)".into(), format!("{}", r.m_coefficient)]];
        o.sheets.push(ExtSheet { title: "Subsurface (pavement) drainage".into(), panels: vec![p], tables: vec![(vec![50.0, 50.0], rows)], notes: r.steps.clone() });
        o.result = json!({"input": inp, "result": r, "half_width_m": w, "lanes_per_side": nl, "cross_slope": cs, "max_grade": g});
        Ok(())
    }

    fn x_mech(&self, a: &Value, o: &mut ExtOut) -> crate::Result<()> {
        let m: crate::mech::MechInput = serde_json::from_value(a.clone()).map_err(|e| format!("input: {e}"))?;
        let r = crate::mech::analyse(&m);
        o.checks.push(chk("P-MECH-SG", "Subgrade strain life >= design traffic (screening)", "pavement".into(), r.n_sub, m.design_esa, "ESA", "min", "advisory", format!("eps_v {:.0} microstrain", r.eps_v_sub * 1e6), format!("{}; verify with CIRCLY", crate::mech::AGPT02_MECH)));
        if r.eps_t_ac > 0.0 { o.checks.push(chk("P-MECH-AC", "Asphalt fatigue life >= design traffic (screening)", "pavement".into(), r.n_ac, m.design_esa, "ESA", "min", "advisory", format!("eps_t {:.0} microstrain", r.eps_t_ac * 1e6), format!("{}; verify with CIRCLY", crate::mech::AGPT02_MECH))); }
        o.result = json!({"input": m, "result": r});
        Ok(())
    }

    fn x_rebar(&self, a: &Value, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let inp: crate::reinforce::RebarInput = serde_json::from_value(a.clone()).map_err(|e| format!("input: {e}"))?;
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let mut res: Vec<crate::reinforce::RebarResult> = vec![];
        for s in &c.sections {
            for (side, sd) in [("L", &s.left), ("R", &s.right)] {
                if let Some(w) = &sd.wall {
                    if !matches!(w.design.kind, crate::walls::WallKind::Cantilever) { continue; }
                    let hs = w.design.h - w.design.base_t;
                    let (mu, d, asr, asmin, as_, sp, ok) = crate::reinforce::stem(&inp, hs, w.design.stem_bot);
                    res.push(crate::reinforce::RebarResult { station: s.station, side: side.into(), stem_h_m: hs, t_m: w.design.stem_bot, mu_knm: mu, d_m: d, as_req_mm2: asr, as_min_mm2: asmin, as_mm2: as_, spacing_mm: sp, ok, note: String::new() });
                }
            }
        }
        let mut worst: std::collections::BTreeMap<String, &crate::reinforce::RebarResult> = Default::default();
        for r in &res { let k = format!("{}{:05}", r.side, (r.station / 100.0).floor() as i64); if worst.get(&k).map_or(true, |w| r.mu_knm > w.mu_knm) { worst.insert(k, r); } }
        let mut rows = vec![hdr(&["Sta", "Side", "Stem H m", "t m", "Mu kNm/m", "d m", "As req", "As min", "As mm2/m", "Bar mm", "Spacing mm", "OK"])];
        for r in worst.values() {
            o.checks.push(chk("W-REBAR", "Stem flexural steel fits (bar spacing >= 100 mm)", format!("wall {} {}", sta_label(r.station), r.side), r.spacing_mm, 100.0, "mm", "min", "mandatory", format!("Mu {:.1} kNm/m, As {:.0} mm2/m", r.mu_knm, r.as_mm2), format!("{} ({})", crate::reinforce::LRFD_FLEX, inp.source)));
            rows.push(vec![sta_label(r.station), r.side.clone(), format!("{:.2}", r.stem_h_m), format!("{:.2}", r.t_m), format!("{:.1}", r.mu_knm), format!("{:.3}", r.d_m), format!("{:.0}", r.as_req_mm2), format!("{:.0}", r.as_min_mm2), format!("{:.0}", r.as_mm2), format!("{:.0}", inp.bar_d_mm), format!("{:.0}", r.spacing_mm), if r.ok { "yes".into() } else { "NO".into() }]);
        }
        let mut wcsv = String::from("station,side,stem_h_m,t_m,mu_knm_per_m,d_m,as_req_mm2_per_m,as_min_mm2_per_m,as_mm2_per_m,spacing_mm,ok\n");
        for r in &res { let _ = writeln!(wcsv, "{:.2},{},{:.4},{:.4},{:.4},{:.5},{:.2},{:.2},{:.2},{:.2},{}", r.station, r.side, r.stem_h_m, r.t_m, r.mu_knm, r.d_m, r.as_req_mm2, r.as_min_mm2, r.as_mm2, r.spacing_mm, r.ok); }
        files.push(("wall-reinforcement.csv".into(), wcsv.into_bytes()));
        if let Some(r) = worst.values().max_by(|p, q| p.mu_knm.partial_cmp(&q.mu_knm).unwrap()) {
            let mut p = Panel { title: format!("Stem reinforcement, governing wall {} {} (schematic)", sta_label(r.station), r.side), equal: true, ..Default::default() };
            let (h, t_) = (r.stem_h_m, r.t_m);
            let tt = (t_ * 0.6).max(0.2).min(t_);
            p.polys.push(("DETAIL".into(), vec![[0.0, 0.0], [t_, 0.0], [t_, h], [t_ - tt, h], [0.0, 0.0]]));
            p.polys.push(("DETAIL".into(), vec![[-0.6, 0.0], [t_ + 1.2, 0.0], [t_ + 1.2, -0.4], [-0.6, -0.4], [-0.6, 0.0]]));
            let cv = inp.cover_m;
            p.polys.push(("SLIP".into(), vec![[t_ - cv, -0.3], [t_ - cv, h - cv]]));
            p.labels.push(("SLIP".into(), [t_ - cv, h * 0.5], format!("{}{:.0} mm bars @ {:.0} mm, retained face", if inp.bars_per_bundle > 1.0 { format!("{:.0}-bar bundles of ", inp.bars_per_bundle) } else { String::new() }, inp.bar_d_mm, (r.spacing_mm / 25.0).floor() * 25.0)));
            p.labels.push(("TEXT".into(), [-0.6, -0.4], format!("cover {:.0} mm; footing, laps, shear by the structural engineer", cv * 1000.0)));
            o.sheets.push(ExtSheet { title: "Retaining wall stem reinforcement".into(), panels: vec![p], tables: vec![(vec![24.0, 12.0, 18.0, 14.0, 20.0, 14.0, 18.0, 18.0, 18.0, 14.0, 20.0, 10.0], rows)], notes: vec![format!("f'c {} MPa, fy {} MPa, phi {}, load factor {} on the active thrust (phi' {} deg, gamma {} kN/m3, surcharge {} kPa), min steel ratio {} - engineer inputs ({}). Shear, crack width, footing design and detailing remain with the structural engineer.", inp.fc_mpa, inp.fy_mpa, inp.phi, inp.load_factor, inp.phi_soil_deg, inp.gamma_knm3, inp.surcharge_kpa, inp.min_ratio, inp.source)] });
        }
        o.result = json!({"input": inp, "sections": res});
        Ok(())
    }

    fn x_marking(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let al = self.alignment.as_ref().ok_or("create_alignment first")?;
        let pr = self.profile.as_ref().ok_or("create_profile first")?;
        let m = crate::traffic::marking(b.standard, al, pr, b.design_speed_kmh, f(a, "psd_m"), req(a, "eye_h_m")?, req(a, "min_zone_m")?, req(a, "curve_warning_r_m")?, f(a, "lateral_clear_m"))?;
        let mut p = cl_panel(self, "Signing and pavement marking (concept)");
        for z in &m.no_passing {
            let mut v = vec![];
            let mut s = z.from;
            while s <= z.to { if let Some((x, y, h)) = al.point_at(s) { let off = if z.direction == "up-station" { -2.0 } else { 2.0 }; v.push([x - off * h.sin(), y + off * h.cos()]); } s += 2.0; }
            p.polys.push(("SIGN".into(), v));
        }
        for sg in &m.signs {
            if let Some((x, y, h)) = al.point_at(sg.station) {
                let off = if sg.side == "left" { 10.0 } else { -10.0 };
                let q = [x - off * h.sin(), y + off * h.cos()];
                p.labels.push(("SIGN".into(), q, sg.code.clone()));
                p.polys.push(("SIGN".into(), vec![[q[0] - 2.0, q[1] - 2.0], [q[0] + 2.0, q[1] - 2.0], [q[0] + 2.0, q[1] + 2.0], [q[0] - 2.0, q[1] + 2.0], [q[0] - 2.0, q[1] - 2.0]]));
            }
        }
        let mut rows = vec![hdr(&["Code", "Sign", "Station", "Side", "Reason"])];
        for sg in &m.signs { rows.push(vec![sg.code.clone(), sg.name.clone(), sta_label(sg.station), sg.side.clone(), sg.reason.clone()]); }
        let mut zr = vec![hdr(&["No-passing from", "to", "Direction"])];
        for z in &m.no_passing { zr.push(vec![sta_label(z.from), sta_label(z.to), z.direction.clone()]); }
        let mut notes = vec![format!("Manual: {}. PSD {:.0} m ({}). Centre line {}, edge line {}.", crate::traffic::manual(b.standard), m.psd_m, m.psd_source, m.centre_colour, m.edge_colour)];
        notes.extend(m.notes.iter().cloned());
        o.sheets.push(ExtSheet { title: "Signing and pavement marking (concept)".into(), panels: vec![p], tables: vec![(vec![24.0, 60.0, 24.0, 14.0, 110.0], rows), (vec![40.0, 30.0, 40.0], zr)], notes });
        let mut sc = String::from("code,name,station,side,reason\n");
        for sg in &m.signs { let _ = writeln!(sc, "{},{},{:.2},{},{}", csvq(&sg.code), csvq(&sg.name), sg.station, sg.side, csvq(&sg.reason)); }
        let mut zc = String::from("from,to,direction\n");
        for z in &m.no_passing { let _ = writeln!(zc, "{:.2},{:.2},{}", z.from, z.to, csvq(&z.direction)); }
        files.push(("sign-schedule.csv".into(), sc.into_bytes()));
        files.push(("no-passing-zones.csv".into(), zc.into_bytes()));
        o.result = serde_json::to_value(&m).unwrap_or_default();
        Ok(())
    }

    fn x_lighting(&self, a: &Value, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let li: crate::traffic::LightingInput = serde_json::from_value(a.clone()).map_err(|e| format!("input: {e}"))?;
        let al = self.alignment.as_ref().ok_or("create_alignment first")?;
        let asm = self.assembly.as_ref().ok_or("create_assembly first")?;
        let r = crate::traffic::lighting(al, asm.paved_width(), &li);
        o.checks.push(chk("L-SPACING", "Pole spacing within spacing/height limit", "lighting".into(), r.spacing_m, li.max_spacing_to_height * li.mounting_h_m, "m", "max", "advisory", "min(lumen-method spacing, ratio*H)".into(), format!("engineer ratio ({})", li.source)));
        let mut p = cl_panel(self, "Lighting layout (concept)");
        for (i, q) in r.poles.iter().enumerate() {
            p.polys.push(("LIGHT".into(), (0..=12).map(|k| { let t = k as f64 / 12.0 * std::f64::consts::TAU; [q.1 + 2.5 * t.cos(), q.2 + 2.5 * t.sin()] }).collect()));
            if i % 5 == 0 { p.labels.push(("LIGHT".into(), [q.1, q.2], format!("P{}", i + 1))); }
        }
        let mut rows = vec![hdr(&["Pole", "Station", "E", "N", "Side"])];
        let mut lc = String::from("pole,station,easting,northing,side\n");
        for (i, q) in r.poles.iter().enumerate() {
            if i < 20 { rows.push(vec![format!("P{}", i + 1), sta_label(q.0), format!("{:.2}", q.1), format!("{:.2}", q.2), q.3.clone()]); }
            let _ = writeln!(lc, "P{},{:.3},{:.3},{:.3},{}", i + 1, q.0, q.1, q.2, q.3);
        }
        if r.poles.len() > 20 { rows.push(vec!["...".into(), format!("{} poles in lighting-poles.csv", r.poles.len()), String::new(), String::new(), String::new()]); }
        files.push(("lighting-poles.csv".into(), lc.into_bytes()));
        let mut notes = r.steps.clone();
        notes.push("Concept layout by the lumen method. Photometric design (luminance, uniformity, glare) to IES RP-8 / TAC Guide for the Design of Roadway Lighting / BS 5489-1 + BS EN 13201 / AS/NZS 1158 is by the lighting designer.".into());
        o.sheets.push(ExtSheet { title: "Road lighting layout (concept)".into(), panels: vec![p], tables: vec![(vec![16.0, 26.0, 30.0, 30.0, 20.0], rows)], notes });
        o.result = serde_json::to_value(&r).unwrap_or_default();
        Ok(())
    }

    fn x_ttm(&self, a: &Value, b: &B, o: &mut ExtOut) -> crate::Result<()> {
        let al = self.alignment.as_ref().ok_or("create_alignment first")?;
        let (stages, cite) = crate::traffic::ttm(b.standard, al, req(a, "existing_speed_kmh")?, req(a, "lane_w_m")?, f(a, "taper_m"))?;
        let mut rows = vec![hdr(&["Stage", "Name", "From", "To", "Traffic", "Works", "Taper m", "Devices"])];
        let (s0, s1) = (al.start_station(), al.end_station());
        let mut p = Panel { title: "Staging diagram (station vs stage)".into(), equal: false, ..Default::default() };
        for st in &stages {
            rows.push(vec![st.n.to_string(), st.name.clone(), sta_label(st.from), sta_label(st.to), st.traffic.clone(), st.works.clone(), st.taper_m.map_or("-".into(), |t| format!("{t:.0}")), st.devices.join("; ")]);
            let y = -(st.n as f64) * 10.0;
            p.polys.push(("GRID".into(), vec![[s0, y], [s1, y]]));
            p.polys.push(("TTM".into(), vec![[st.from, y + 2.0], [st.to, y + 2.0], [st.to, y - 2.0], [st.from, y - 2.0], [st.from, y + 2.0]]));
            p.labels.push(("TEXT".into(), [s0, y + 3.0], format!("Stage {}: {}", st.n, st.name)));
        }
        p.note = "horizontal: station; bars: work zone extent".into();
        o.sheets.push(ExtSheet { title: "Temporary traffic management staging (concept)".into(), panels: vec![p], tables: vec![(vec![12.0, 45.0, 22.0, 22.0, 60.0, 60.0, 16.0, 120.0], rows)], notes: vec![cite.clone(), "Concept staging only. Detailed TTM / TMP layouts, device spacing and approvals per the agency manual by a qualified TTM designer.".into()] });
        o.result = json!({"stages": stages, "citation": cite});
        Ok(())
    }
}

impl Session {
    fn x_utilities(&self, a: &Value, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let us: Vec<crate::landuse::Utility> = serde_json::from_value(a["utilities"].clone()).map_err(|e| format!("utilities: {e}"))?;
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
        let (cl, mx) = (req(a, "clearance_m")?, req(a, "max_cover_m")?);
        let src = a["source"].as_str().unwrap_or("engineer");
        let cf = crate::landuse::conflicts(c, &self.networks, t, &us, cl, mx);
        let mut rc = String::from("utility,kind,owner,station,x,y,issue,value_m,action,quality_level\n");
        let mut rows = vec![hdr(&["Utility", "Kind", "Owner", "Station", "Issue", "Value m", "Action", "QL"])];
        for k in &cf {
            let sta = if k.station.is_finite() { format!("{:.2}", k.station) } else { String::new() };
            let _ = writeln!(rc, "{},{},{},{sta},{:.3},{:.3},{},{:.4},{},{}", csvq(&k.utility), csvq(&k.kind), csvq(&k.owner), k.x, k.y, csvq(&k.issue), k.value_m, csvq(&k.action), csvq(&k.quality_level));
            rows.push(vec![k.utility.clone(), k.kind.clone(), k.owner.clone(), if k.station.is_finite() { sta_label(k.station) } else { "-".into() }, k.issue.clone(), format!("{:.2}", k.value_m), k.action.clone(), k.quality_level.clone()]);
            if !k.issue.starts_with("crossing - no") { o.checks.push(chk("U-CONFLICT", "Utility conflict to resolve", format!("{} at ({:.0}, {:.0})", k.utility, k.x, k.y), k.value_m, cl, "m", "min", "advisory", k.issue.clone(), "utility conflict register (ASCE 38-22 / PAS 128 / AS 5488 quality levels)".into())); }
        }
        files.push(("utility-register.csv".into(), rc.into_bytes()));
        let mut p = cl_panel(self, "Utilities and conflicts");
        for u in &us { p.polys.push(("UTILITY".into(), u.polyline.clone())); if let Some(q) = u.polyline.first() { p.labels.push(("UTILITY".into(), *q, format!("{} {}", u.id, u.kind))); } }
        for n in &self.networks { for pp in &n.pipes { if let (Some(x), Some(y)) = (n.structures.iter().find(|s| s.0 == pp.from), n.structures.iter().find(|s| s.0 == pp.to)) { p.polys.push(("CULVERT".into(), vec![[x.1, x.2], [y.1, y.2]])); } } }
        for k in &cf { p.polys.push(("SLIP".into(), (0..=12).map(|i| { let t = i as f64 / 12.0 * std::f64::consts::TAU; [k.x + 5.0 * t.cos(), k.y + 5.0 * t.sin()] }).collect())); }
        o.sheets.push(ExtSheet { title: "Utility plan and conflict register".into(), panels: vec![p], tables: vec![(vec![20.0, 20.0, 26.0, 22.0, 80.0, 16.0, 80.0, 28.0], rows)], notes: vec![format!("Utility records are owner / engineer supplied at the stated quality level ({src}). Clearance {cl} m and maximum added cover {mx} m are engineer inputs. Conflicts at QL-C/D need potholing (QL-A) before design is finalised.")] });
        o.result = json!({"conflicts": cf, "n_utilities": us.len()});
        Ok(())
    }

    fn x_land(&self, a: &Value, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let buf = req(a, "buffer_m")?;
        let src = a["source"].as_str().unwrap_or("engineer");
        let parcels: Vec<crate::landuse::Parcel> = match a.get("parcels") { Some(v) if v.is_array() => serde_json::from_value(v.clone()).map_err(|e| format!("parcels: {e}"))?, _ => vec![] };
        let fp = crate::landuse::footprint(c, buf);
        let area = crate::landuse::area(&fp);
        let tk = crate::landuse::takes(&fp, &parcels, f(a, "cell_m").unwrap_or(1.0));
        let mut fc = String::from("vertex,easting,northing\n");
        for (i, q) in fp.iter().enumerate() { let _ = writeln!(fc, "{i},{:.4},{:.4}", q[0], q[1]); }
        files.push(("footprint.csv".into(), fc.into_bytes()));
        let mut lc = String::from("parcel,owner,parcel_area_m2,take_m2,take_pct\n");
        let mut rows = vec![hdr(&["Parcel", "Owner", "Parcel m2", "Take m2", "Take %"])];
        for t_ in &tk { let _ = writeln!(lc, "{},{},{:.2},{:.2},{:.3}", csvq(&t_.parcel), csvq(&t_.owner), t_.parcel_area_m2, t_.take_m2, t_.take_pct); rows.push(vec![t_.parcel.clone(), t_.owner.clone(), format!("{:.0}", t_.parcel_area_m2), format!("{:.0}", t_.take_m2), format!("{:.1}", t_.take_pct)]); }
        files.push(("land-requirements.csv".into(), lc.into_bytes()));
        let mut p = cl_panel(self, "Land requirement plan");
        for pc in &parcels {
            let mut v = pc.polygon.clone();
            if let Some(f0) = v.first().copied() { v.push(f0); }
            p.polys.push(("PARCEL".into(), v));
            let n = pc.polygon.len().max(1) as f64;
            p.labels.push(("PARCEL".into(), [pc.polygon.iter().map(|q| q[0]).sum::<f64>() / n, pc.polygon.iter().map(|q| q[1]).sum::<f64>() / n], pc.id.clone()));
        }
        let mut v = fp.clone();
        v.push(fp[0]);
        p.polys.push(("ROW".into(), v));
        o.sheets.push(ExtSheet { title: "Right-of-way / land requirement plan".into(), panels: vec![p], tables: vec![(vec![30.0, 60.0, 30.0, 30.0, 20.0], rows)], notes: vec![format!("Land requirement = daylight lines + {buf} m working / maintenance buffer (engineer). Area {area:.0} m2. Parcel boundaries from surveyor / cadastre data ({src}); legal descriptions and title by the land surveyor.")] });
        o.result = json!({"footprint_area_m2": area, "vertices": fp.len(), "takes": tk, "buffer_m": buf});
        Ok(())
    }

    fn x_locality(&self, a: &Value, o: &mut ExtOut) -> crate::Result<()> {
        let al = self.alignment.as_ref().ok_or("create_alignment first")?;
        let zone = req(a, "utm_zone")? as u32;
        let north = a["north"].as_bool().unwrap_or(true);
        let (x0, y0, _) = al.point_at(al.start_station()).ok_or("alignment start")?;
        let (x1, y1, _) = al.point_at(al.end_station()).ok_or("alignment end")?;
        let ll0 = crate::landuse::utm_to_ll(x0, y0, zone, north);
        let ll1 = crate::landuse::utm_to_ll(x1, y1, zone, north);
        let cl = cl_poly(al, 10.0);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let mut p = Panel { title: "Locality plan (UTM 1 km grid)".into(), equal: true, ..Default::default() };
        let (gx0, gy0) = (((cx - 5000.0) / 1000.0).floor() * 1000.0, ((cy - 5000.0) / 1000.0).floor() * 1000.0);
        for k in 0..=10 {
            let (gx, gy) = (gx0 + k as f64 * 1000.0, gy0 + k as f64 * 1000.0);
            p.polys.push(("GRID".into(), vec![[gx, gy0], [gx, gy0 + 10000.0]]));
            p.polys.push(("GRID".into(), vec![[gx0, gy], [gx0 + 10000.0, gy]]));
            if k % 2 == 0 { p.labels.push(("TEXT".into(), [gx, gy0 - 300.0], format!("{gx:.0}E"))); p.labels.push(("TEXT".into(), [gx0 - 900.0, gy], format!("{gy:.0}N"))); }
        }
        p.polys.push(("ROUTE-SELECTED".into(), cl.clone()));
        let bx = 1200.0;
        p.polys.push(("ROW".into(), vec![[cx - bx, cy - bx], [cx + bx, cy - bx], [cx + bx, cy + bx], [cx - bx, cy + bx], [cx - bx, cy - bx]]));
        p.labels.push(("ROW".into(), [cx - bx, cy + bx], "PROJECT SITE".into()));
        for pl in a["places"].as_array().cloned().unwrap_or_default() { if let (Some(x), Some(y)) = (f(&pl, "x"), f(&pl, "y")) { p.labels.push(("TEXT".into(), [x, y], pl["name"].as_str().unwrap_or("").into())); } }
        let mut q = Panel { title: "Project extent".into(), equal: true, ..Default::default() };
        q.polys.push(("ROUTE-SELECTED".into(), cl));
        q.labels.push(("TEXT".into(), [x0, y0], format!("START {} ({:.5}, {:.5})", sta_label(al.start_station()), ll0.0, ll0.1)));
        q.labels.push(("TEXT".into(), [x1, y1], format!("END {} ({:.5}, {:.5})", sta_label(al.end_station()), ll1.0, ll1.1)));
        let rows = vec![hdr(&["Point", "Easting", "Northing", "Latitude", "Longitude"]), vec!["Start".into(), format!("{x0:.3}"), format!("{y0:.3}"), format!("{:.6}", ll0.0), format!("{:.6}", ll0.1)], vec!["End".into(), format!("{x1:.3}"), format!("{y1:.3}"), format!("{:.6}", ll1.0), format!("{:.6}", ll1.1)]];
        o.sheets.push(ExtSheet { title: "Locality plan".into(), panels: vec![p, q], tables: vec![(vec![20.0, 34.0, 34.0, 30.0, 30.0], rows)], notes: vec![format!("WGS84 / UTM zone {zone}{} (inverse transverse Mercator series, Snyder 1987, USGS PP 1395). No base mapping is shipped: overlay agency / OS / LINZ / USGS mapping under the user's licence.", if north { "N" } else { "S" })] });
        o.result = json!({"start": {"e": x0, "n": y0, "lat": ll0.0, "lon": ll0.1}, "end": {"e": x1, "n": y1, "lat": ll1.0, "lon": ll1.1}, "zone": zone, "north": north});
        Ok(())
    }

    fn x_isd(&self, a: &Value, b: &B, o: &mut ExtOut) -> crate::Result<()> {
        let pr = self.profile.as_ref().ok_or("create_profile first")?;
        let (eh, oh) = (req(a, "eye_h_m")?, req(a, "object_h_m")?);
        let v = b.design_speed_kmh;
        let src = a["source"].as_str().unwrap_or("engineer");
        let (need, form, cite): (f64, String, String) = match b.standard {
            Standard::Us | Standard::Ca => {
                let tg = req(a, "time_gap_s")?;
                (0.278 * v * tg, format!("ISD = 0.278*V*tg = 0.278*{v}*{tg}"), if b.standard == Standard::Us { format!("AASHTO Green Book 2018 §9.5.3 Case B1, time gap ({src})") } else { format!("TAC GDG 2017 Ch. 9 intersection sight distance, time gap engineer input ({src})") })
            }
            Standard::Au | Standard::Nz => {
                let rt = req(a, "reaction_s")?;
                let d = req(a, "decel_coeff")?;
                let g = f(a, "grade_pct").unwrap_or(0.0);
                (rt * v / 3.6 + v * v / (254.0 * (d + 0.01 * g)), format!("SISD = RT*V/3.6 + V^2/(254(d+0.01a)), RT {rt} s, d {d}, a {g}%"), format!("Austroads AGRD Part 4A §3.2.2 SISD; RT and d engineer inputs ({src})"))
            }
            Standard::Uk => { let y = req(a, "y_distance_m")?; (y, format!("y-distance {y} m at x = 2.4 m"), format!("DMRB CD 123 visibility splay; y-distance engineer input ({src})")) }
        };
        let mut rows = vec![hdr(&["Intersection", "Station", "Required m", "Avail. ahead m", "Avail. back m"])];
        for it in &self.intersections {
            let st = it.main_station;
            let ahead = sight_dir(pr, st, eh, oh, 1.0);
            let back = sight_dir(pr, st, eh, oh, -1.0);
            for (lbl, av) in [("ahead", ahead), ("back", back)] { o.checks.push(chk("I-SISD", "Intersection sight distance on the major road (vertical)", format!("{} {lbl}", it.name), av, need, "m", "min", "mandatory", form.clone(), cite.clone())); }
            rows.push(vec![it.name.clone(), sta_label(st), format!("{need:.1}"), format!("{ahead:.1}"), format!("{back:.1}")]);
        }
        o.sheets.push(ExtSheet { title: "Intersection layout and sight distance".into(), panels: vec![intersection_panel(self)], tables: vec![(vec![40.0, 26.0, 26.0, 30.0, 30.0], rows.clone())], notes: vec![format!("{form}. {cite}. Available distance: vertical sight line along the major-road profile (eye {eh} m, object {oh} m), limited by the profile ends; horizontal obstructions in the sight triangle are cleared on site.")] });
        o.result = json!({"required_m": need, "formula": form, "citation": cite, "rows": rows});
        Ok(())
    }

    fn x_rrrap(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        if b.standard != Standard::Uk { return Err("RRRAP (DMRB CD 377) applies to the UK profile only".into()); }
        let pt = self.protection.as_ref().ok_or("design_protection first")?;
        let mut s = format!("# Inputs for the National Highways Road Restraint Risk Assessment Process (RRRAP) tool, DMRB CD 377. The assessment is run by the designer in RRRAP.\n# design speed {} km/h, AADT {}, HGV {}%\nhazard_type,side,station,offset_from_edge_m,height_or_depth_m\n", b.design_speed_kmh, req(a, "aadt")?, req(a, "hgv_pct")?);
        for h in &pt.hazards { let _ = writeln!(s, "{},{},{:.2},{:.2},{:.2}", csvq(&h.kind), h.side, h.station, h.offset_from_etw_m, h.height_m); }
        files.push(("rrrap-inputs.csv".into(), s.into_bytes()));
        o.result = json!({"hazards": pt.hazards.len()});
        Ok(())
    }

    fn x_cost(&self, a: &Value, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let (rows, method) = crate::deliver::boq(self);
        let cp = req(a, "contingency_pct")?;
        let ob = f(a, "optimism_bias_pct");
        let mut s = format!("# Cost plan structure. {method}\n# Rates are blank: enter agency / market rates. Contingency {cp}% (engineer).{}\nsection,code,item,unit,quantity,rate,amount,p50_amount,p90_amount\n", ob.map_or(String::new(), |o| format!(" Optimism bias {o}% (HM Treasury Green Book supplementary guidance, engineer).")));
        for r in &rows { let _ = writeln!(s, "base,{},{},{},{:.2},,,,", csvq(&r.code), csvq(&r.item), r.unit, r.qty); }
        let _ = writeln!(s, "subtotal,,Base cost (sum of amounts),,,,,,");
        let _ = writeln!(s, "contingency,,Contingency {cp}% of base,,,,,,");
        for rk in a["risks"].as_array().cloned().unwrap_or_default() { let _ = writeln!(s, "risk,,{},,,,,{},{}", csvq(rk["name"].as_str().unwrap_or("")), rk["p50"].as_f64().map_or(String::new(), |v| v.to_string()), rk["p90"].as_f64().map_or(String::new(), |v| v.to_string())); }
        if let Some(x) = ob { let _ = writeln!(s, "optimism_bias,,Optimism bias {x}% (UK),,,,,,"); }
        let _ = writeln!(s, "total,,Estimate (base + contingency + risk{}),,,,,,", if ob.is_some() { " + optimism bias" } else { "" });
        files.push(("cost-plan.csv".into(), s.into_bytes()));
        o.result = json!({"rows": rows.len(), "contingency_pct": cp, "optimism_bias_pct": ob});
        Ok(())
    }

    fn x_handoffs(&self, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let (env, bridge) = handoffs(self, b.standard);
        files.push(("handoff-environmental.json".into(), serde_json::to_vec_pretty(&env).unwrap_or_default()));
        files.push(("handoff-bridge-structures.json".into(), serde_json::to_vec_pretty(&bridge).unwrap_or_default()));
        let mut md = String::from("# LICENSED SPECIALIST HAND-OFF - NOT PRODUCED BY THIS SOFTWARE\n\nThese packages carry the design data this tool can supply. The assessments and designs are produced and signed by licensed specialists.\n");
        for (t, v) in [("Environmental assessment", &env), ("Bridges and major structures", &bridge)] {
            let _ = writeln!(md, "\n## {t}\n\nRegime: {}\n\n### Deliverables checklist (specialist)\n", v["regime"].as_str().unwrap_or(""));
            for c in v["checklist"].as_array().cloned().unwrap_or_default() { let _ = writeln!(md, "- [ ] {}", c.as_str().unwrap_or("")); }
            let _ = writeln!(md, "\n### Data supplied by civil-mcp\n");
            for (k, x) in v["data_from_design"].as_object().cloned().unwrap_or_default() { let _ = writeln!(md, "- {k}: {}", x); }
            let _ = writeln!(md, "\n### Needed back from the specialist\n");
            for c in v["required_back"].as_array().cloned().unwrap_or_default() { let _ = writeln!(md, "- {}", c.as_str().unwrap_or("")); }
        }
        files.push(("specialist-handoffs.md".into(), md.into_bytes()));
        o.result = json!({"status": "LICENSED SPECIALIST - interface only", "environmental": env["regime"], "bridge": bridge["regime"]});
        Ok(())
    }
}

fn intersection_panel(s: &Session) -> Panel {
    let mut p = Panel { title: "Intersection layout".into(), equal: true, ..Default::default() };
    let Some(it) = s.intersections.first() else { return p };
    let c = [it.point[0], it.point[1]];
    if let Some(al) = &s.alignment {
        let v: Vec<[f64; 2]> = cl_poly(al, 1.0).into_iter().filter(|q| (q[0] - c[0]).hypot(q[1] - c[1]) < 80.0).collect();
        p.polys.push(("ALIGNMENT".into(), v));
    }
    if let Some(cor) = &s.corridor {
        for left in [true, false] {
            let v: Vec<[f64; 2]> = cor.sections.iter().filter(|q| (q.x - c[0]).hypot(q.y - c[1]) < 80.0).map(|q| { let o = if left { q.left.paved_width } else { -q.right.paved_width }; [q.x - o * q.heading.sin(), q.y + o * q.heading.cos()] }).collect();
            if v.len() > 1 { p.polys.push(("PAVEMENT".into(), v)); }
        }
    }
    for l in &it.legs {
        p.polys.push(("ALIGNMENT".into(), l.cl.to_vec()));
        p.polys.push(("PAVEMENT".into(), l.edge_left.to_vec()));
        p.polys.push(("PAVEMENT".into(), l.edge_right.to_vec()));
        for r in &l.returns { p.polys.push(("PAVEMENT".into(), r.arc.iter().map(|q| [q[0], q[1]]).collect())); p.labels.push(("TEXT".into(), [r.arc[r.arc.len() / 2][0], r.arc[r.arc.len() / 2][1]], format!("R{:.0}", r.radius))); }
        for sl in &l.sight { let t = sl.triangle; p.polys.push(("SIGN".into(), vec![t[0], t[1], t[2], t[0]])); }
        p.labels.push(("TEXT".into(), l.cl[1], l.name.clone()));
    }
    p.labels.push(("TEXT".into(), c, it.name.clone()));
    p
}

fn handoffs(s: &Session, std: Standard) -> (Value, Value) {
    let (env_regime, env_list): (&str, Vec<&str>) = match std {
        Standard::Us => ("NEPA (23 CFR 771) CE / EA / EIS; CWA Sections 404/401; ESA Section 7; NHPA Section 106; Section 4(f)", vec!["NEPA class of action determination", "wetlands and waters delineation", "threatened / endangered species consultation", "cultural resources (Section 106)", "noise study per 23 CFR 772 (Type I projects)", "air quality conformity", "SWPPP under the NPDES construction general permit", "environmental commitments record"]),
        Standard::Ca => ("Impact Assessment Act / provincial EA (e.g. Ontario Class EA for Provincial Transportation Facilities, BC Environmental Assessment Act); Fisheries Act; SARA", vec!["EA screening / class determination", "fish and fish habitat (DFO) review", "species at risk", "archaeology / heritage", "noise and air", "erosion and sediment control plan", "environmental commitments"]),
        Standard::Uk => ("EIA Regulations (Highways Act 1980 Part VA / Planning Act 2008 DCO); DMRB LA 101-LA 120", vec!["LA 104 screening and scoping", "LA 105 air quality", "LA 106 cultural heritage", "LA 107 landscape and visual", "LA 108 biodiversity (and HRA)", "LA 109 geology and soils", "LA 111 noise and vibration", "LA 113 road drainage and the water environment", "LA 120 environmental management plan"]),
        Standard::Au => ("EPBC Act 1999 + state planning approval (e.g. NSW EP&A Act REF / EIS)", vec!["REF / EIS determination", "matters of national environmental significance (EPBC)", "biodiversity assessment and offsets", "Aboriginal cultural heritage", "road traffic noise assessment", "soil and water management / ESC plan", "CEMP"]),
        Standard::Nz => ("Resource Management Act 1991 consents; NZTA environmental and social responsibility requirements", vec!["Assessment of Environmental Effects (AEE)", "ecology", "archaeological authority (HNZPTA)", "road traffic noise (NZS 6806)", "erosion and sediment control (regional guideline)", "cultural values assessment with iwi", "CEMP"]),
    };
    let fp_area = s.corridor.as_ref().map(|c| crate::landuse::area(&crate::landuse::footprint(c, 0.0)));
    let env_data = json!({"footprint_area_m2": fp_area, "cut_m3": s.corridor.as_ref().map(|c| c.cut_m3), "fill_m3": s.corridor.as_ref().map(|c| c.fill_m3), "culverts": s.ext.get("design_culverts").map(|e| e.result["culverts"].clone()), "drainage_networks": s.networks.iter().map(|n| n.name.clone()).collect::<Vec<_>>(), "files": ["design.xml (LandXML)", "footprint.csv", "hydrology.csv", "culverts.csv", "land-requirements.csv"]});
    let env = json!({"status": "LICENSED SPECIALIST - NOT PRODUCED BY THIS SOFTWARE", "regime": env_regime, "checklist": env_list, "data_from_design": env_data, "required_back": ["constraints map (no-go areas) as polygons for route selection avoid[]", "commitments that change the design (fish passage, noise barriers, water treatment)", "consent conditions"]});
    let (br_regime, br_list): (&str, Vec<&str>) = match std {
        Standard::Us => ("AASHTO LRFD Bridge Design Specifications + state bridge manual; NBIS", vec!["type, size and location (TS&L) report", "bridge hydraulics and scour (HEC-18) for stream crossings", "foundation report", "general plan and elevation", "load rating", "PE-sealed calculations and plans"]),
        Standard::Ca => ("CSA S6 Canadian Highway Bridge Design Code + provincial structural manual", vec!["structural planning report", "hydraulic and scour assessment", "foundation investigation report", "general arrangement drawing", "P.Eng-sealed design"]),
        Standard::Uk => ("DMRB CG 300 technical approval (AIP); Eurocodes with UK National Annexes", vec!["Approval in Principle (CG 300)", "design and check certificates by category", "hydraulic and scour assessment where applicable", "general arrangement drawings", "health and safety file inputs"]),
        Standard::Au => ("AS 5100 Bridge design + state bridge technical requirements", vec!["concept / options report", "design certification by a registered engineer (e.g. RPEQ in QLD)", "hydraulic / flood immunity report", "geotechnical report", "general arrangement drawings", "independent verification"]),
        Standard::Nz => ("NZTA Bridge Manual (SP/M/022) with AS/NZS 5100 and NZS 3101", vec!["structural options report", "design features report", "producer statements (CPEng)", "seismic design to the Bridge Manual", "hydraulic and scour report", "peer review"]),
    };
    let thr = if std == Standard::Us { 6.1 } else { 3.0 };
    let big: Vec<Value> = s.ext.get("design_culverts").and_then(|e| e.result["culverts"].as_array().cloned()).unwrap_or_default().into_iter().filter(|c| c["hyd"]["d_m"].as_f64().map_or(false, |d| d >= thr)).collect();
    let bridge = json!({"status": "LICENSED SPECIALIST - NOT PRODUCED BY THIS SOFTWARE", "regime": br_regime, "checklist": br_list, "structure_threshold_m": thr, "threshold_note": "US: NBIS bridge length > 20 ft (6.1 m); other profiles: 3 m screening value - confirm the agency threshold", "data_from_design": {"alignment_and_profile": "design.xml (LandXML), setting-out.csv", "design_speed_kmh": s.basis.as_ref().map(|b| b.design_speed_kmh), "typical_section": s.assembly.as_ref().map(|a| json!({"lanes_m": a.lanes_m, "shoulder_m": a.shoulder_m})), "design_flows": s.ext.get("design_hydrology").map(|e| e.result["catchments"].clone()), "crossings_above_threshold": big, "walls": "wall-reinforcement.csv (stem flexure only)"}, "required_back": ["deck levels and clearances to tie into the profile", "abutment / approach slab geometry", "structure depth for vertical clearance checks"]});
    (env, bridge)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn d8_on_a_tilted_v_valley_collects_everything_at_the_outlet() {
        // z = |x - 50|·0.1 + y·0.05 on a 101x51 grid of 1 m cells: flow goes to the valley line then south
        let (nx, ny) = (101, 51);
        let z = (0..ny).flat_map(|j| (0..nx).map(move |i| Some((i as f64 - 50.0).abs() * 0.1 + j as f64 * 0.05))).collect();
        let g = crate::route::Grid { x0: 0.0, y0: 0.0, cell: 1.0, nx, ny, z };
        let f = flow(&g);
        let out = 50; // (i 50, j 0)
        // every interior cell drains to the outlet or to another edge cell; the valley outlet gets the valley's catchment
        assert!(f.acc[out] > 0.3 * (nx * ny) as f64, "{}", f.acc[out]);
        // flow length at the outlet >= the valley length
        assert!(f.len[out] >= 49.0);
    }
    #[test]
    fn horizontal_sight_offset() {
        // M = R(1 - cos(S/2R)); R 200, S 160 -> 200(1 - cos 0.4) = 15.7878
        assert!((crate::traffic::hso_m(200.0, 160.0) - 15.7878).abs() < 1e-3);
    }
}

impl Session {
    fn x_esc(&self, a: &Value, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let hd = req(a, "check_dam_h_m")?;
        let fo = f(a, "fence_offset_m").unwrap_or(1.0);
        let src = a["source"].as_str().unwrap_or("engineer");
        let mut p = cl_panel(self, "Erosion and sediment control (concept)");
        let mut fence_len = 0.0;
        let mut rows = vec![hdr(&["Item", "Location", "Quantity", "Basis"])];
        let mut qc = String::from("item,side,from_station,to_station,quantity,unit,basis\n");
        for left in [true, false] {
            // runs of consecutive fill sections on this side -> sediment fence at daylight + offset
            let mut run: Vec<(f64, [f64; 2])> = vec![];
            let flush = |run: &mut Vec<(f64, [f64; 2])>, p: &mut Panel, fence_len: &mut f64, rows: &mut Vec<Vec<String>>, qc: &mut String| {
                if run.len() >= 2 {
                    let l: f64 = run.windows(2).map(|w| (w[1].1[0] - w[0].1[0]).hypot(w[1].1[1] - w[0].1[1])).sum();
                    *fence_len += l;
                    p.polys.push(("ROW".into(), run.iter().map(|q| q.1).collect()));
                    let side = if left { "L" } else { "R" };
                    rows.push(vec!["Sediment fence".into(), format!("{side} {} - {}", sta_label(run[0].0), sta_label(run[run.len() - 1].0)), format!("{l:.0} m"), "fill toe + offset".into()]);
                    let _ = writeln!(qc, "sediment fence,{side},{:.2},{:.2},{l:.2},m,fill toe + {fo} m", run[0].0, run[run.len() - 1].0);
                }
                run.clear();
            };
            for q in &c.sections {
                let sd = if left { &q.left } else { &q.right };
                if !sd.cut && sd.wall.is_none() {
                    let off = (sd.daylight + fo) * if left { 1.0 } else { -1.0 };
                    run.push((q.station, [q.x - off * q.heading.sin(), q.y + off * q.heading.cos()]));
                } else { flush(&mut run, &mut p, &mut fence_len, &mut rows, &mut qc); }
            }
            flush(&mut run, &mut p, &mut fence_len, &mut rows, &mut qc);
        }
        // check dams in cut ditches
        let mut dams = 0usize;
        if let Some(pr) = &self.profile {
            for left in [true, false] {
                let mut sec_run: Vec<f64> = vec![];
                let mut runs = vec![];
                for q in &c.sections { let sd = if left { &q.left } else { &q.right }; if sd.cut && sd.codes.iter().any(|k| k.contains("DITCH")) { sec_run.push(q.station); } else if !sec_run.is_empty() { runs.push(std::mem::take(&mut sec_run)); } }
                if !sec_run.is_empty() { runs.push(sec_run); }
                for r in runs {
                    let (s0, s1) = (r[0], r[r.len() - 1]);
                    if s1 - s0 < 10.0 { continue; }
                    let g = ((pr.elevation_at(s1).unwrap_or(0.0) - pr.elevation_at(s0).unwrap_or(0.0)) / (s1 - s0)).abs().max(0.005);
                    let sp = hd / g;
                    let n = ((s1 - s0) / sp).floor() as usize + 1;
                    dams += n;
                    let side = if left { "L" } else { "R" };
                    rows.push(vec!["Check dams".into(), format!("{side} ditch {} - {}", sta_label(s0), sta_label(s1)), format!("{n} @ {sp:.0} m"), format!("L = h/S = {hd}/{g:.4}")]);
                    let _ = writeln!(qc, "check dam,{side},{s0:.2},{s1:.2},{n},each,spacing {sp:.2} m = {hd}/{g:.5}");
                    let mut st = s0;
                    while st <= s1 { if let Some(q) = c.sections.iter().min_by(|a, b| (a.station - st).abs().partial_cmp(&(b.station - st).abs()).unwrap()) { let sd = if left { &q.left } else { &q.right }; let off = sd.fg.iter().zip(sd.codes.iter()).find(|(_, k)| k.contains("DITCH")).map_or(sd.daylight, |(v, _)| v[0]) * if left { 1.0 } else { -1.0 }; let xy = [q.x - off * q.heading.sin(), q.y + off * q.heading.cos()]; p.polys.push(("SLIP".into(), vec![[xy[0] - 1.5, xy[1] - 1.5], [xy[0] + 1.5, xy[1] + 1.5]])); } st += sp; }
                }
            }
        }
        let mut n_cul = 0;
        if let Some(e) = self.ext.get("design_culverts") {
            for cv in e.result["culverts"].as_array().cloned().unwrap_or_default() { if cv["hyd"].is_object() { n_cul += 1; } }
            if n_cul > 0 { rows.push(vec!["Culvert inlet / outlet protection".into(), format!("{n_cul} culverts"), format!("{} each", 2 * n_cul), "both ends".into()]); let _ = writeln!(qc, "culvert inlet/outlet protection,,,,{},each,both ends of each culvert", 2 * n_cul); }
        }
        rows.push(vec!["Stabilised construction entrance".into(), "start and end tie-ins".into(), "2 each".into(), "site access".into()]);
        let _ = writeln!(qc, "stabilised construction entrance,,,,2,each,tie-ins");
        files.push(("esc-quantities.csv".into(), qc.into_bytes()));
        o.sheets.push(ExtSheet { title: "Erosion and sediment control plan (concept)".into(), panels: vec![p], tables: vec![(vec![60.0, 80.0, 30.0, 60.0], rows)], notes: vec![format!("Concept ESC plan: sediment fence at fill toes + {fo} m, check dams at L = h/S (h = {hd} m), protection at culvert ends, stabilised entrances. Devices, spacing and inspection regime per the agency ESC manual and permit ({src}); the SWPPP / ESC plan is completed by the qualified ESC designer.")] });
        o.result = json!({"sediment_fence_m": fence_len, "check_dams": dams, "culvert_protection_each": 2 * n_cul});
        Ok(())
    }
}
