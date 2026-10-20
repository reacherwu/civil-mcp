//! Corridor: assemblies applied at sample-line stations, daylighting to the
//! existing ground, end-area and composite volumes, corridor surfaces.
use crate::align::Alignment;
use crate::profile::Profile;
use crate::superelev::Superelevation;
use crate::tin::Tin;
use crate::walls::{WallDesign, WallRule};
use crate::widening::{Vehicle, WideningResult};
use serde::{Deserialize, Serialize};

/// Barrier curb with gutter pan and optional sidewalk (urban section).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curb {
    pub gutter_m: f64,
    /// gutter pan cross slope (rise per m outward, e.g. -0.06 toward the curb face)
    pub gutter_slope: f64,
    pub height_m: f64,
    pub top_m: f64,
    #[serde(default)]
    pub sidewalk_m: f64,
    /// sidewalk cross slope (rise per m outward, e.g. +0.02 draining to the gutter)
    #[serde(default)]
    pub sidewalk_slope: f64,
}

/// Right-side overrides for asymmetric sections (fields left unset copy the left side).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SideOverride {
    #[serde(default)]
    pub lanes_m: Option<Vec<f64>>,
    #[serde(default)]
    pub shoulder_m: Option<f64>,
    #[serde(default)]
    pub shoulder_slope: Option<f64>,
    #[serde(default)]
    pub shoulder_paved: Option<bool>,
    #[serde(default)]
    pub curb: Option<Curb>,
    #[serde(default)]
    pub ditch: Option<Ditch>,
    #[serde(default)]
    pub cut_slope_h: Option<f64>,
    #[serde(default)]
    pub fill_slope_h: Option<f64>,
    #[serde(default)]
    pub wall: Option<WallRule>,
    /// components to drop on this side: "curb", "ditch", "wall"
    #[serde(default)]
    pub remove: Vec<String>,
}

/// Resolved components of one side.
#[derive(Debug, Clone)]
pub struct SideCfg {
    pub lanes_m: Vec<f64>,
    pub shoulder_m: f64,
    pub shoulder_slope: f64,
    pub shoulder_paved: bool,
    pub curb: Option<Curb>,
    pub ditch: Option<Ditch>,
    pub cut_slope_h: f64,
    pub fill_slope_h: f64,
    pub cut_bench: Option<Bench>,
    pub fill_bench: Option<Bench>,
    pub wall: Option<WallRule>,
    pub pavement_t: f64,
    pub subgrade_slope_h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WideningSpec {
    /// "AASHTO" (off-tracking formula; Green Book §3.3.10) or "TABLE" (engineer-entered radius → widening table)
    pub method: String,
    /// engineer table for method TABLE: [[radius_upper_m, widening_m], ...]
    #[serde(default)]
    pub table: Vec<[f64; 2]>,
    #[serde(default)]
    pub table_citation: String,
    #[serde(default)]
    pub vehicle: Option<Vehicle>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WallPlaced {
    pub offset: f64,
    pub top_z: f64,
    pub bottom_z: f64,
    pub cut: bool,
    pub trigger: String,
    pub design: WallDesign,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub thickness_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ditch {
    /// Foreslope, horizontal per 1 vertical
    pub foreslope_h: f64,
    pub depth_m: f64,
    pub bottom_width_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bench {
    /// Vertical height between benches
    pub every_m: f64,
    pub width_m: f64,
}

/// Symmetric assembly (same components left and right).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assembly {
    pub name: String,
    /// Lane widths from the centreline outward (each side).
    pub lanes_m: Vec<f64>,
    pub shoulder_m: f64,
    /// Shoulder cross slope (rise per m outward, normally negative, e.g. -0.04).
    pub shoulder_slope: f64,
    pub shoulder_paved: bool,
    /// Ditch applied in cut only.
    #[serde(default)]
    pub ditch: Option<Ditch>,
    pub cut_slope_h: f64,
    pub fill_slope_h: f64,
    #[serde(default)]
    pub cut_bench: Option<Bench>,
    #[serde(default)]
    pub fill_bench: Option<Bench>,
    /// Pavement layers, top to bottom, under lanes (and shoulders if paved).
    #[serde(default)]
    pub pavement: Vec<Layer>,
    /// Subgrade daylight slope from paved edge back up to finished grade (H per 1V).
    #[serde(default = "d_sg")]
    pub subgrade_slope_h: f64,
    #[serde(default)]
    pub curb: Option<Curb>,
    #[serde(default)]
    pub wall: Option<WallRule>,
    #[serde(default)]
    pub right: Option<SideOverride>,
}
fn d_sg() -> f64 {
    4.0
}

impl Assembly {
    pub fn validate(&self) -> crate::Result<()> {
        let pos = |x: f64| x > 0.0 && x.is_finite();
        if self.lanes_m.is_empty()
            || !self.lanes_m.iter().all(|w| pos(*w))
            || self.shoulder_m < 0.0
            || !pos(self.cut_slope_h)
            || !pos(self.fill_slope_h)
            || !pos(self.subgrade_slope_h)
        {
            return Err("assembly: lanes > 0, shoulder >= 0, slopes > 0 required".into());
        }
        if let Some(d) = &self.ditch {
            if !pos(d.foreslope_h) || !pos(d.depth_m) || d.bottom_width_m < 0.0 {
                return Err("ditch: foreslope, depth > 0, bottom >= 0".into());
            }
        }
        for b in [&self.cut_bench, &self.fill_bench].into_iter().flatten() {
            if !pos(b.every_m) || !pos(b.width_m) {
                return Err("bench: every_m and width_m > 0".into());
            }
        }
        if self.pavement.iter().any(|l| !pos(l.thickness_m)) {
            return Err("pavement layer thickness must be > 0".into());
        }
        Ok(())
    }
    pub fn lane_width(&self) -> f64 {
        self.lanes_m.iter().sum()
    }
    pub fn paved_width(&self) -> f64 {
        self.lane_width() + if self.shoulder_paved { self.shoulder_m } else { 0.0 }
    }
    pub fn pavement_thickness(&self) -> f64 {
        self.pavement.iter().map(|l| l.thickness_m).sum()
    }
    pub fn side_cfg(&self, left: bool) -> SideCfg {
        let mut c = SideCfg {
            lanes_m: self.lanes_m.clone(),
            shoulder_m: self.shoulder_m,
            shoulder_slope: self.shoulder_slope,
            shoulder_paved: self.shoulder_paved,
            curb: self.curb.clone(),
            ditch: self.ditch.clone(),
            cut_slope_h: self.cut_slope_h,
            fill_slope_h: self.fill_slope_h,
            cut_bench: self.cut_bench.clone(),
            fill_bench: self.fill_bench.clone(),
            wall: self.wall.clone(),
            pavement_t: self.pavement_thickness(),
            subgrade_slope_h: self.subgrade_slope_h,
        };
        if !left {
            if let Some(o) = &self.right {
                if let Some(v) = &o.lanes_m {
                    c.lanes_m = v.clone()
                }
                if let Some(v) = o.shoulder_m {
                    c.shoulder_m = v
                }
                if let Some(v) = o.shoulder_slope {
                    c.shoulder_slope = v
                }
                if let Some(v) = o.shoulder_paved {
                    c.shoulder_paved = v
                }
                if o.curb.is_some() {
                    c.curb = o.curb.clone()
                }
                if o.ditch.is_some() {
                    c.ditch = o.ditch.clone()
                }
                if let Some(v) = o.cut_slope_h {
                    c.cut_slope_h = v
                }
                if let Some(v) = o.fill_slope_h {
                    c.fill_slope_h = v
                }
                if o.wall.is_some() {
                    c.wall = o.wall.clone()
                }
                for r in &o.remove {
                    match r.as_str() {
                        "curb" => c.curb = None,
                        "ditch" => c.ditch = None,
                        "wall" => c.wall = None,
                        _ => {}
                    }
                }
            }
        }
        c
    }
}

impl SideCfg {
    pub fn paved_width(&self, extra: f64) -> f64 {
        let lanes: f64 = self.lanes_m.iter().sum::<f64>() + extra;
        match &self.curb {
            Some(c) => lanes + c.gutter_m,
            None => lanes + if self.shoulder_paved { self.shoulder_m } else { 0.0 },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SideSection {
    /// Finished-grade polyline from centreline outward: (offset >= 0, elevation)
    pub fg: Vec<[f64; 2]>,
    /// Earthwork datum (subgrade) polyline, same extent as fg.
    pub datum: Vec<[f64; 2]>,
    pub cut: bool,
    pub daylight: f64,
    pub warning: Option<String>,
    /// Point codes for fg vertices
    pub codes: Vec<String>,
    pub wall: Option<WallPlaced>,
    pub paved_width: f64,
    pub widening: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    pub station: f64,
    pub x: f64,
    pub y: f64,
    pub heading: f64,
    pub cl_elev: f64,
    pub ground_cl: f64,
    pub slope_left: f64,
    pub slope_right: f64,
    pub left: SideSection,
    pub right: SideSection,
    /// Ground samples across the section: (signed offset, left +), elevation
    pub ground: Vec<[f64; 2]>,
    pub cut_area: f64,
    pub fill_area: f64,
    /// Layer areas (m²) in pavement order
    pub layer_areas: Vec<f64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Corridor {
    pub assembly: Assembly,
    pub sample_stations: Vec<f64>,
    pub sections: Vec<Section>,
    pub cut_m3: f64,
    pub fill_m3: f64,
    pub layer_m3: Vec<(String, f64)>,
    pub method: String,
    pub sections_with_warnings: usize,
    pub widening: Vec<WideningResult>,
    /// concrete volume of retaining walls (m³, average end area) and wall length per side (m)
    pub wall_concrete_m3: f64,
    pub wall_length_left_m: f64,
    pub wall_length_right_m: f64,
}

const MAX_OFFSET: f64 = 150.0;

/// Ground along the section line; None outside the surface.
fn ground_at(tin: &Tin, x: f64, y: f64, nx: f64, ny: f64, o: f64) -> Option<f64> {
    tin.elevation_at(x + o * nx, y + o * ny)
}

/// Walk a slope (with optional benches) from `start` outward until it meets the ground.
/// dir = +1 upward (cut) or -1 downward (fill). Returns the added vertices and success flag.
fn daylight(g: &dyn Fn(f64) -> Option<f64>, start: [f64; 2], h: f64, dir: f64, bench: Option<&Bench>) -> (Vec<[f64; 2]>, bool, Option<String>) {
    let mut pts = Vec::new();
    let mut p = start;
    let mut since_bench = 0.0;
    loop {
        // next segment: slope until bench height, or bench
        let seg_v = bench.map_or(1000.0, |b| b.every_m - since_bench);
        let q = [p[0] + seg_v * h, p[1] + dir * seg_v];
        if let Some(hit) = intersect(g, p, q) {
            pts.push(hit);
            return (pts, true, None);
        }
        if q[0] > MAX_OFFSET || g(q[0].min(MAX_OFFSET)).is_none() {
            let o = MAX_OFFSET.min(q[0]);
            // stop at edge of data
            let mut last = p;
            let mut t = p[0];
            while t < o {
                t += 0.5;
                if g(t).is_none() {
                    break;
                }
                last = [t, p[1] + dir * (t - p[0]) / h];
            }
            pts.push(last);
            return (pts, false, Some(format!("no daylight within {:.0} m or ground missing", last[0])));
        }
        pts.push(q);
        p = q;
        since_bench = 0.0;
        if let Some(b) = bench {
            let r = [p[0] + b.width_m, p[1]];
            if let Some(hit) = intersect(g, p, r) {
                pts.push(hit);
                return (pts, true, None);
            }
            pts.push(r);
            p = r;
        }
    }
}

/// First crossing of segment a->b with ground (design - ground changes sign).
fn intersect(g: &dyn Fn(f64) -> Option<f64>, a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let n = ((b[0] - a[0]) / 0.25).ceil().max(1.0) as usize;
    let at = |t: f64| [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    let f = |t: f64| {
        let p = at(t);
        g(p[0]).map(|z| p[1] - z)
    };
    let mut t0 = 0.0;
    let mut f0 = f(0.0)?;
    if f0 == 0.0 {
        return Some(at(0.0));
    }
    for i in 1..=n {
        let t1 = i as f64 / n as f64;
        let f1 = f(t1)?;
        if f1 == 0.0 || (f0 > 0.0) != (f1 > 0.0) {
            let (mut lo, mut hi, mut flo) = (t0, t1, f0);
            for _ in 0..60 {
                let m = (lo + hi) / 2.0;
                let fm = f(m)?;
                if (fm > 0.0) == (flo > 0.0) && fm != 0.0 {
                    lo = m;
                    flo = fm
                } else {
                    hi = m
                }
            }
            return Some(at(hi));
        }
        t0 = t1;
        f0 = f1;
    }
    None
}

fn side(cfg: &SideCfg, slope: f64, cl: f64, g: &dyn Fn(f64) -> Option<f64>, extra_w: f64, wc: Option<&crate::walls::Criteria>) -> SideSection {
    let mut fg = vec![[0.0, cl]];
    let mut codes = vec!["CL".to_string()];
    let mut o = 0.0;
    let mut z = cl;
    let nl = cfg.lanes_m.len();
    for (i, w) in cfg.lanes_m.iter().enumerate() {
        let w = if i + 1 == nl { w + extra_w } else { *w };
        o += w;
        z += slope * w;
        fg.push([o, z]);
        codes.push(format!("ETW{}", i + 1));
    }
    let mut has_ditch = cfg.ditch.is_some();
    if let Some(c) = &cfg.curb {
        o += c.gutter_m;
        z += c.gutter_slope * c.gutter_m;
        fg.push([o, z]);
        codes.push("GUTTER".into());
        o += 0.01;
        z += c.height_m;
        fg.push([o, z]);
        codes.push("TOP-CURB".into());
        o += c.top_m;
        fg.push([o, z]);
        codes.push("BACK-CURB".into());
        if c.sidewalk_m > 0.0 {
            o += c.sidewalk_m;
            z += c.sidewalk_slope * c.sidewalk_m;
            fg.push([o, z]);
            codes.push("BACK-WALK".into());
        }
        has_ditch = false;
    } else if cfg.shoulder_m > 0.0 {
        // shoulder never flatter than a lane falling more steeply (high side keeps shoulder slope)
        let s = cfg.shoulder_slope.min(slope);
        o += cfg.shoulder_m;
        z += s * cfg.shoulder_m;
        fg.push([o, z]);
        codes.push("SHLD".into());
    }
    let hinge = [o, z];
    let gh = g(o);
    let cut = gh.is_some_and(|gz| gz >= z);
    let mut warning = None;
    let mut start = hinge;
    let (tail, ok, w) = if cut {
        if has_ditch {
            let d = cfg.ditch.as_ref().unwrap();
            let p1 = [o + d.depth_m * d.foreslope_h, z - d.depth_m];
            fg.push(p1);
            codes.push("DITCH-IN".into());
            let p2 = [p1[0] + d.bottom_width_m, p1[1]];
            if d.bottom_width_m > 0.0 {
                fg.push(p2);
                codes.push("DITCH-OUT".into());
            }
            start = p2;
        }
        daylight(g, start, cfg.cut_slope_h, 1.0, cfg.cut_bench.as_ref())
    } else if gh.is_some() {
        daylight(g, hinge, cfg.fill_slope_h, -1.0, cfg.fill_bench.as_ref())
    } else {
        (vec![], false, Some("ground missing at hinge".to_string()))
    };
    if !ok {
        warning = w;
    }
    // retaining wall rule
    let mut wall = None;
    let mut tail = tail;
    if let (Some(rule), Some(last)) = (&cfg.wall, tail.last().copied()) {
        let height = (last[1] - start[1]).abs();
        let trig_off = rule.max_daylight_offset_m.is_some_and(|m| last[0] > m);
        let trig_h = rule.max_slope_height_m.is_some_and(|m| height > m);
        if trig_off || trig_h {
            let ow = start[0] + rule.offset_from_hinge_m;
            if let Some(gz) = g(ow + 0.01) {
                let exposed = if cut { gz - start[1] } else { start[1] - gz };
                if exposed > 0.0 {
                    let h = exposed + rule.embedment_m;
                    let cr = wc
                        .cloned()
                        .ok_or("retaining wall needed but no wall criteria for this standards profile (enter design_basis.wall_criteria)")
                        .unwrap_or_else(|e| crate::walls::Criteria {
                            fs_sliding: f64::INFINITY,
                            fs_overturning: f64::INFINITY,
                            e_frac: 0.0,
                            qmax_factor: 0.0,
                            citation: e.to_string(),
                        });
                    let mut d = crate::walls::proportion(&rule.kind, h, &rule.soil, &cr);
                    d.exposed_h = exposed;
                    let trigger = if trig_off {
                        format!("daylight {:.1} m > ROW {:.1} m", last[0], rule.max_daylight_offset_m.unwrap())
                    } else {
                        format!("slope height {:.1} m > {:.1} m", height, rule.max_slope_height_m.unwrap())
                    };
                    tail = vec![[ow, start[1]], [ow + 0.01, gz]];
                    warning = None;
                    wall = Some(WallPlaced {
                        offset: ow,
                        top_z: if cut { gz } else { start[1] },
                        bottom_z: if cut { start[1] } else { gz },
                        cut,
                        trigger,
                        design: d,
                    });
                }
            }
        }
    }
    let n_tail = tail.len();
    for (i, p) in tail.iter().enumerate() {
        fg.push(*p);
        let code = if wall.is_some() {
            if i == 0 {
                if cut {
                    "WALL-BOTTOM"
                } else {
                    "WALL-TOP"
                }
            } else {
                "DAYLIGHT"
            }
        } else if i + 1 == n_tail {
            "DAYLIGHT"
        } else {
            "SLOPE"
        };
        codes.push(code.into());
    }
    let daylight = fg.last().unwrap()[0];
    // datum: lowered by pavement thickness across the paved width, then back up at subgrade slope
    let t = cfg.pavement_t;
    let pw = cfg.paved_width(extra_w);
    let datum = if t > 0.0 {
        let fgz = |x: f64| interp(&fg, x);
        let mut d = vec![[0.0, cl - t]];
        for p in fg.iter().skip(1) {
            if p[0] < pw - 1e-9 {
                d.push([p[0], p[1] - t]);
            }
        }
        d.push([pw, fgz(pw) - t]);
        // walk outward at subgrade slope until it meets fg
        let mut x = pw;
        let mut zz = fgz(pw) - t;
        let mut met = false;
        while x < daylight {
            let nx = (x + 0.05).min(daylight);
            let nz = zz + (nx - x) / cfg.subgrade_slope_h;
            if nz >= fgz(nx) {
                // meet
                d.push([nx, fgz(nx)]);
                x = nx;
                met = true;
                break;
            }
            x = nx;
            zz = nz;
        }
        if !met {
            // finished grade rises faster than the subgrade slope (curb, cut slope): step up at the paved edge
            x = pw;
            d.push([pw + 1e-3, fgz(pw + 1e-3)]);
            x += 1e-3;
        }
        for p in fg.iter() {
            if p[0] > x + 1e-9 {
                d.push(*p);
            }
        }
        d
    } else {
        fg.clone()
    };
    SideSection { fg, datum, cut, daylight, warning, codes, wall, paved_width: pw, widening: extra_w }
}

/// Linear interpolation on a polyline sorted by offset (clamped).
pub fn interp(p: &[[f64; 2]], x: f64) -> f64 {
    if x <= p[0][0] {
        return p[0][1];
    }
    for w in p.windows(2) {
        if x <= w[1][0] {
            let d = w[1][0] - w[0][0];
            return if d.abs() < 1e-12 { w[1][1] } else { w[0][1] + (w[1][1] - w[0][1]) * (x - w[0][0]) / d };
        }
    }
    p[p.len() - 1][1]
}

/// Cut/fill area between datum (design) and ground along one side, 0..daylight.
fn side_areas(datum: &[[f64; 2]], g: &dyn Fn(f64) -> Option<f64>, dl: f64) -> (f64, f64) {
    let mut xs: Vec<f64> = datum.iter().map(|p| p[0]).filter(|x| *x <= dl).collect();
    let mut t = 0.0;
    while t < dl {
        xs.push(t);
        t += 0.25;
    }
    xs.push(dl);
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let (mut cut, mut fill) = (0.0, 0.0);
    let d = |x: f64| g(x).map(|z| interp(datum, x) - z);
    for w in xs.windows(2) {
        let (Some(a), Some(b)) = (d(w[0]), d(w[1])) else { continue };
        let l = w[1] - w[0];
        if a >= 0.0 && b >= 0.0 {
            fill += (a + b) / 2.0 * l
        } else if a <= 0.0 && b <= 0.0 {
            cut -= (a + b) / 2.0 * l
        } else {
            let x = a / (a - b) * l;
            if a > 0.0 {
                fill += a * x / 2.0;
                cut += -b * (l - x) / 2.0
            } else {
                cut += -a * x / 2.0;
                fill += b * (l - x) / 2.0
            }
        }
    }
    (cut, fill)
}

pub fn sample_stations(al: &Alignment, pr: &Profile, se: Option<&Superelevation>, interval: f64) -> Vec<f64> {
    let (s0, s1) = (al.start_station().max(pr.start()), al.end_station().min(pr.end()));
    let mut v = Vec::new();
    let mut s = (s0 / interval).ceil() * interval;
    while s < s1 {
        v.push(s);
        s += interval;
    }
    v.push(s0);
    v.push(s1);
    v.extend(al.key_stations());
    for c in &pr.curves {
        if c.length > 0.0 {
            v.push(c.station - c.length / 2.0);
            v.push(c.station + c.length / 2.0);
        }
    }
    if let Some(se) = se {
        for c in &se.curves {
            if c.e > 0.0 {
                v.extend([c.a_in, c.b_in, c.b_out, c.a_out]);
            }
        }
    }
    v.retain(|x| *x >= s0 - 1e-9 && *x <= s1 + 1e-9);
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    v
}

pub fn section_at(tin: &Tin, al: &Alignment, pr: &Profile, se: Option<&Superelevation>, asm: &Assembly, s: f64) -> crate::Result<Section> {
    let wc = crate::walls::criteria(crate::standards::Standard::Us, None)?;
    section_at_opts(tin, al, pr, se, asm, s, (0.0, 0.0), Some(&wc))
}

#[allow(clippy::too_many_arguments)]
pub fn section_at_opts(
    tin: &Tin,
    al: &Alignment,
    pr: &Profile,
    se: Option<&Superelevation>,
    asm: &Assembly,
    s: f64,
    widen: (f64, f64),
    wc: Option<&crate::walls::Criteria>,
) -> crate::Result<Section> {
    let (x, y, h) = al.point_at(s).ok_or("station off alignment")?;
    let cl = pr.elevation_at(s).ok_or("station off profile")?;
    let (nx, ny) = (-h.sin(), h.cos());
    let gcl = tin.elevation_at(x, y).ok_or_else(|| format!("centreline at station {s:.2} is outside the surface"))?;
    let (sl, sr) = se.map_or((-0.02, -0.02), |e| e.slopes_at(s));
    let gl = |o: f64| ground_at(tin, x, y, nx, ny, o);
    let gr = |o: f64| ground_at(tin, x, y, nx, ny, -o);
    let left = side(&asm.side_cfg(true), sl, cl, &gl, widen.0, wc);
    let right = side(&asm.side_cfg(false), sr, cl, &gr, widen.1, wc);
    let (cl_, fl) = side_areas(&left.datum, &gl, left.daylight);
    let (cr, fr) = side_areas(&right.datum, &gr, right.daylight);
    let pw = left.paved_width + right.paved_width;
    let layer_areas = asm.pavement.iter().map(|l| pw * l.thickness_m).collect();
    let mut ground = Vec::new();
    let mut o = -right.daylight - 5.0;
    while o <= left.daylight + 5.0 {
        if let Some(z) = gl(o) {
            ground.push([o, z]);
        }
        o += 1.0;
    }
    let warnings = [left.warning.clone(), right.warning.clone()].into_iter().flatten().collect();
    Ok(Section {
        station: s,
        x,
        y,
        heading: h,
        cl_elev: cl,
        ground_cl: gcl,
        slope_left: sl,
        slope_right: sr,
        left,
        right,
        ground,
        cut_area: cl_ + cr,
        fill_area: fl + fr,
        layer_areas,
        warnings,
    })
}

pub fn build(tin: &Tin, al: &Alignment, pr: &Profile, se: Option<&Superelevation>, asm: &Assembly, interval: f64) -> crate::Result<Corridor> {
    let wc = crate::walls::criteria(crate::standards::Standard::Us, None)?;
    build_opts(tin, al, pr, se, asm, interval, None, 0.0, Some(&wc))
}

/// Widening at a station: (left, right). Full value on the arc, linear over the
/// spirals (or over 10 m outside PC/PT for simple curves), on the inside of the curve.
pub fn widening_at(al: &Alignment, w: &[f64], s: f64) -> (f64, f64) {
    let (mut l, mut r) = (0.0, 0.0);
    for (c, wc) in al.curves().iter().zip(w) {
        if *wc <= 0.0 || !wc.is_finite() {
            continue;
        }
        let (a0, a1) = if c.ls_in > 0.0 { (c.sta_start, c.sta_cs_start) } else { (c.sta_start - 10.0, c.sta_start) };
        let (b0, b1) = if c.ls_out > 0.0 { (c.sta_cs_end, c.sta_end) } else { (c.sta_end, c.sta_end + 10.0) };
        let f = if s <= a0 || s >= b1 {
            0.0
        } else if s < a1 {
            (s - a0) / (a1 - a0)
        } else if s <= b0 {
            1.0
        } else {
            (b1 - s) / (b1 - b0)
        };
        let v = f * wc;
        if c.dir > 0.0 {
            l = f64::max(l, v)
        } else {
            r = f64::max(r, v)
        }
    }
    (l, r)
}

#[allow(clippy::too_many_arguments)]
pub fn build_opts(
    tin: &Tin,
    al: &Alignment,
    pr: &Profile,
    se: Option<&Superelevation>,
    asm: &Assembly,
    interval: f64,
    widening: Option<&WideningSpec>,
    speed: f64,
    wc: Option<&crate::walls::Criteria>,
) -> crate::Result<Corridor> {
    asm.validate()?;
    if !(interval > 0.0) {
        return Err("interval must be > 0".into());
    }
    let mut wres = Vec::new();
    if let Some(ws) = widening {
        let lanes = asm.lanes_m.len() + asm.side_cfg(false).lanes_m.len();
        let lw = asm.lanes_m.iter().sum::<f64>() / asm.lanes_m.len() as f64;
        for c in al.curves() {
            let r = match ws.method.as_str() {
                "TABLE" => crate::widening::table(c.radius, &ws.table, &ws.table_citation),
                "AASHTO" => crate::widening::aashto(c.radius, speed, lanes, lw, ws.vehicle.as_ref().ok_or("AASHTO widening needs a design vehicle")?),
                m => return Err(format!("unknown widening method {m}")),
            };
            if r.widening_m.is_nan() {
                return Err(format!("radius {:.1} m is below the smallest radius in the widening table", c.radius));
            }
            wres.push(r);
        }
    }
    let wv: Vec<f64> = wres.iter().map(|r| r.widening_m).collect();
    let mut st = sample_stations(al, pr, se, interval);
    for c in al.curves() {
        if c.ls_in == 0.0 {
            st.push(c.sta_start - 10.0)
        }
        if c.ls_out == 0.0 {
            st.push(c.sta_end + 10.0)
        }
    }
    let (s0, s1) = (al.start_station().max(pr.start()), al.end_station().min(pr.end()));
    st.retain(|x| *x >= s0 - 1e-9 && *x <= s1 + 1e-9);
    st.sort_by(|a, b| a.partial_cmp(b).unwrap());
    st.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    let mut secs = Vec::new();
    for &s in &st {
        secs.push(section_at_opts(tin, al, pr, se, asm, s, widening_at(al, &wv, s), wc)?);
    }
    let (mut cut, mut fill) = (0.0, 0.0);
    let mut layers = vec![0.0; asm.pavement.len()];
    let (mut wc, mut wl, mut wr) = (0.0, 0.0, 0.0);
    let wa = |s: &SideSection| s.wall.as_ref().map_or(0.0, |w| w.design.concrete_m2);
    for w in secs.windows(2) {
        let l = w[1].station - w[0].station;
        wc += (wa(&w[0].left) + wa(&w[0].right) + wa(&w[1].left) + wa(&w[1].right)) / 2.0 * l;
        if w[0].left.wall.is_some() && w[1].left.wall.is_some() {
            wl += l
        }
        if w[0].right.wall.is_some() && w[1].right.wall.is_some() {
            wr += l
        }
        cut += (w[0].cut_area + w[1].cut_area) / 2.0 * l;
        fill += (w[0].fill_area + w[1].fill_area) / 2.0 * l;
        for (k, v) in layers.iter_mut().enumerate() {
            *v += (w[0].layer_areas[k] + w[1].layer_areas[k]) / 2.0 * l;
        }
    }
    let nw = secs.iter().filter(|s| !s.warnings.is_empty()).count();
    Ok(Corridor {
        assembly: asm.clone(),
        sample_stations: st,
        layer_m3: asm.pavement.iter().map(|l| l.name.clone()).zip(layers).collect(),
        sections: secs,
        cut_m3: cut,
        fill_m3: fill,
        method: "average end area between sample lines (no curvature correction, bank volumes); earthwork measured to the subgrade datum".into(),
        sections_with_warnings: nw,
        widening: wres,
        wall_concrete_m3: wc,
        wall_length_left_m: wl,
        wall_length_right_m: wr,
    })
}

impl Section {
    /// World 3D points of a polyline across the section, right daylight → left daylight.
    pub fn world(&self, datum: bool) -> Vec<[f64; 3]> {
        let (nx, ny) = (-self.heading.sin(), self.heading.cos());
        let (l, r) = if datum { (&self.left.datum, &self.right.datum) } else { (&self.left.fg, &self.right.fg) };
        let mut v: Vec<[f64; 3]> = r.iter().rev().map(|p| [self.x - p[0] * nx, self.y - p[0] * ny, p[1]]).collect();
        v.pop(); // centreline duplicated
        v.extend(l.iter().map(|p| [self.x + p[0] * nx, self.y + p[0] * ny, p[1]]));
        v
    }
}

/// Zipper-triangulate consecutive section polylines into a TIN.
pub fn surface(c: &Corridor, datum: bool) -> crate::Result<(Tin, crate::tin::TinStats)> {
    let mut pts: Vec<[f64; 3]> = Vec::new();
    let mut faces = Vec::new();
    let mut prev: Option<(usize, usize, Vec<f64>)> = None;
    for s in &c.sections {
        let w = s.world(datum);
        // signed offsets (right negative → left positive), same order as world()
        let (l, r) = if datum { (&s.left.datum, &s.right.datum) } else { (&s.left.fg, &s.right.fg) };
        let mut off: Vec<f64> = r.iter().rev().map(|p| -p[0]).collect();
        off.pop();
        off.extend(l.iter().map(|p| p[0]));
        let base = pts.len();
        pts.extend(&w);
        if let Some((pb, pn, po)) = prev {
            let (mut i, mut j) = (0, 0);
            let (n, m) = (pn, w.len());
            // zipper in offset order: always advance the side whose next vertex has the smaller offset,
            // so triangles never fold across closely spaced vertices (curb faces, walls)
            while i + 1 < n || j + 1 < m {
                let adv_a = if j + 1 >= m {
                    true
                } else if i + 1 >= n {
                    false
                } else {
                    po[i + 1] < off[j + 1]
                };
                if adv_a {
                    faces.push([pb + i, pb + i + 1, base + j]);
                    i += 1;
                } else {
                    faces.push([pb + i, base + j + 1, base + j]);
                    j += 1;
                }
            }
        }
        prev = Some((base, w.len(), off));
    }
    Tin::from_faces(&pts, &faces)
}

/// Composite (grid) volume between two surfaces: (cut, fill, cells) where top < bottom is cut.
pub fn composite_volume(top: &Tin, bottom: &Tin, cell: f64) -> (f64, f64, usize) {
    let (mut mnx, mut mny, mut mxx, mut mxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for t in &top.tris {
        for &i in t {
            let p = top.pts[i];
            mnx = mnx.min(p[0]);
            mny = mny.min(p[1]);
            mxx = mxx.max(p[0]);
            mxy = mxy.max(p[1]);
        }
    }
    let (mut cut, mut fill, mut n) = (0.0, 0.0, 0);
    let mut y = mny + cell / 2.0;
    while y < mxy {
        let mut x = mnx + cell / 2.0;
        while x < mxx {
            if let (Some(a), Some(b)) = (top.elevation_at(x, y), bottom.elevation_at(x, y)) {
                let d = a - b;
                if d > 0.0 {
                    fill += d * cell * cell
                } else {
                    cut -= d * cell * cell
                }
                n += 1;
            }
            x += cell;
        }
        y += cell;
    }
    (cut, fill, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::align::AlignmentInput;
    use crate::profile::ProfileInput;
    fn flat(z: f64) -> Tin {
        let mut pts = vec![];
        for i in -30..=30 {
            for j in -30..=80 {
                pts.push([i as f64 * 10.0 + (j % 2) as f64 * 0.01, j as f64 * 10.0, z]);
            }
        }
        Tin::build(&pts, None).unwrap().0
    }
    fn straight() -> (Alignment, Profile) {
        (
            Alignment::build(AlignmentInput { name: "a".into(), pis: vec![[0.0, 0.0], [0.0, 500.0]], radii: vec![], spirals: vec![], start_station: 0.0 })
                .unwrap(),
            Profile::build(ProfileInput { name: "p".into(), vpis: vec![[0.0, 102.0], [500.0, 102.0]], curve_lengths: vec![] }).unwrap(),
        )
    }
    fn asm() -> Assembly {
        serde_json::from_value(
            serde_json::json!({"name":"t","lanes_m":[5.0],"shoulder_m":0.0,"shoulder_slope":-0.04,"shoulder_paved":false,"cut_slope_h":2.0,"fill_slope_h":2.0}),
        )
        .unwrap()
    }
    #[test]
    fn fill_prism_matches_hand_calc() {
        let (al, pr) = straight();
        let c = build(&flat(100.0), &al, &pr, None, &asm(), 20.0).unwrap();
        // no superelevation passed → slopes -0.02: CL 102, edge 101.9
        // per side: 5·(2+1.9)/2 = 9.75 ; slope height 1.9 → width 3.8 → 0.5·3.8·1.9 = 3.61 ; total 2·13.36 = 26.72
        let s = &c.sections[3];
        assert!((s.fill_area - 26.72).abs() < 1e-6, "{}", s.fill_area);
        assert!((s.left.daylight - 8.8).abs() < 1e-6);
        assert!((c.fill_m3 - 26.72 * 500.0).abs() < 1e-3);
        let (top, _) = surface(&c, true).unwrap();
        let (cut, fill, _) = composite_volume(&top, &flat(100.0), 0.5);
        assert!(cut < 1e-6 && (fill - c.fill_m3).abs() / c.fill_m3 < 0.002, "{fill}");
    }
    #[test]
    fn cut_with_ditch_and_bench() {
        let (al, pr) = straight();
        let mut a = asm();
        a.ditch = Some(Ditch { foreslope_h: 4.0, depth_m: 1.0, bottom_width_m: 1.0 });
        a.cut_bench = Some(Bench { every_m: 3.0, width_m: 2.0 });
        let c = build(&flat(110.0), &al, &pr, None, &a, 50.0).unwrap();
        let s = &c.sections[1];
        // hinge (5, 101.9); ditch to (9, 100.9), (10, 100.9); slope up 2:1: 3 m rise → (16, 103.9); bench → (18, 103.9);
        // 3 m → (24, 106.9); bench (26,106.9); 3 m → (32, 109.9); bench (34, 109.9); 0.1 m → 34.2
        assert!((s.left.daylight - 34.2).abs() < 1e-6, "{}", s.left.daylight);
        assert!(s.left.cut && s.fill_area < 1e-9);
        // hand area: ground 110 minus design polyline integrated
        let pts = &s.left.fg;
        let mut area = 0.0;
        for w in pts.windows(2) {
            area += ((110.0 - w[0][1]) + (110.0 - w[1][1])) / 2.0 * (w[1][0] - w[0][0]);
        }
        assert!((s.cut_area / 2.0 - area).abs() < 1e-6);
    }
    #[test]
    fn pavement_datum_adds_box_cut() {
        let (al, pr) = straight();
        let mut a = asm();
        a.pavement = vec![Layer { name: "AC".into(), thickness_m: 0.1 }, Layer { name: "Base".into(), thickness_m: 0.3 }];
        let c = build(&flat(101.0), &al, &pr, None, &a, 100.0).unwrap();
        assert!((c.layer_m3[1].1 - 0.3 * 10.0 * 500.0).abs() < 1e-6);
        assert!(c.sections[0].fill_area > 0.0);
    }
    #[test]
    fn curb_in_cut_steps_datum_and_matches_composite() {
        // ground 103 (cut), CL 102, curb right only; pavement 0.3 m
        let (al, pr) = straight();
        let a: Assembly = serde_json::from_value(
            serde_json::json!({"name":"t","lanes_m":[3.5],"shoulder_m":0.0,"shoulder_slope":-0.04,"shoulder_paved":false,"cut_slope_h":1.0,"fill_slope_h":2.0,
            "pavement":[{"name":"p","thickness_m":0.3}],
            "right":{"curb":{"gutter_m":0.5,"gutter_slope":-0.06,"height_m":0.15,"top_m":0.15,"sidewalk_m":2.0,"sidewalk_slope":0.02}}}),
        )
        .unwrap();
        let g = flat(103.0);
        let c = build(&g, &al, &pr, None, &a, 20.0).unwrap();
        let s = &c.sections[3];
        // right datum must continue past the paved edge (regression: it used to stop at the gutter)
        assert!(s.right.datum.last().unwrap()[0] > s.right.paved_width + 1.0, "{:?}", s.right.datum);
        assert!((s.right.paved_width - 4.0).abs() < 1e-12);
        let (top, _) = surface(&c, true).unwrap();
        let (cut, _, _) = composite_volume(&top, &g, 0.1);
        assert!((cut - c.cut_m3).abs() / c.cut_m3 < 0.01, "composite {cut} vs end area {}", c.cut_m3);
    }
    #[test]
    fn wall_replaces_slope_beyond_row() {
        let (al, pr) = straight();
        let mut a = asm();
        a.wall = Some(
            serde_json::from_value(serde_json::json!({"kind":"gravity","max_daylight_offset_m":7.0,"offset_from_hinge_m":0.0,"embedment_m":0.5,
            "soil":{"gamma_kn_m3":18.0,"phi_deg":30.0,"base_friction":0.5,"bearing_kpa":200.0}}))
            .unwrap(),
        );
        // ground 99 → fill 3 m below CL 102: slope would daylight at 5 + 2·2.9 = 10.8 > 7 → wall at 5 m, exposed 101.9 − 99 = 2.9
        let c = build(&flat(99.0), &al, &pr, None, &a, 20.0).unwrap();
        let s = &c.sections[2];
        let w = s.left.wall.as_ref().expect("wall");
        assert!((w.offset - 5.0).abs() < 1e-9 && (w.design.exposed_h - 2.9).abs() < 1e-9 && (w.design.h - 3.4).abs() < 1e-9);
        assert!(w.design.pass);
        assert!((s.left.daylight - 5.01).abs() < 1e-9);
        // fill area per side = trapezoid 0..5 (heights 3.0 → 2.9) + sliver of the 0.01 m face
        assert!((s.fill_area / 2.0 - (5.0 * (3.0 + 2.9) / 2.0 + 0.01 * 2.9 / 2.0)).abs() < 1e-6, "{}", s.fill_area);
    }
    #[test]
    fn widening_ramps_over_spiral() {
        let al = Alignment::build(AlignmentInput {
            name: "a".into(),
            pis: vec![[0.0, 0.0], [0.0, 300.0], [300.0, 600.0]],
            radii: vec![150.0],
            spirals: vec![50.0],
            start_station: 0.0,
        })
        .unwrap();
        let c = &al.curves()[0];
        let w = [0.6];
        assert_eq!(widening_at(&al, &w, c.sta_start - 1.0), (0.0, 0.0));
        let mid = (c.sta_cs_start + c.sta_cs_end) / 2.0;
        let (l, r) = widening_at(&al, &w, mid);
        assert!((l + r - 0.6).abs() < 1e-12 && l.min(r) == 0.0);
        let (l, r) = widening_at(&al, &w, c.sta_start + 25.0);
        assert!((l + r - 0.3).abs() < 1e-9);
    }
}
