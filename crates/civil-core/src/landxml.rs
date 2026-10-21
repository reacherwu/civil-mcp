//! LandXML 1.2 reader/writer for CgPoints, TIN surfaces, alignments (line,
//! arc, clothoid) and profiles (PVI, ParaCurve, CircCurve).
//! Coordinates in LandXML are "northing easting [elevation]".
use crate::align::{Alignment, Element, Kind};
use crate::points::SurveyPoint;
use crate::profile::{Profile, ProfileInput};
use crate::tin::Tin;
use roxmltree::{Document, Node};
use serde::Serialize;
use std::fmt::Write;

#[derive(Debug, Clone, Serialize)]
pub struct SurfaceData {
    pub name: String,
    pub points: Vec<[f64; 3]>,
    pub faces: Vec<[usize; 3]>,
    pub invisible_faces: usize,
    /// Attributes written by the producing application, if present.
    pub reported_area2d: Option<f64>,
    pub reported_area3d: Option<f64>,
    pub reported_elev_min: Option<f64>,
    pub reported_elev_max: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlignmentData {
    pub name: String,
    pub sta_start: f64,
    pub reported_length: Option<f64>,
    pub elements: Vec<Element>,
    /// Max distance between our evaluated element end and the file's <End> (m).
    pub max_end_deviation: f64,
    /// Per-element: (kind, file length attr, our length)
    pub warnings: Vec<String>,
    pub profiles: Vec<ProfileInput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructData {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub rim: Option<f64>,
    pub sump: Option<f64>,
    /// (invert elevation, flowDir "in"/"out", refPipe)
    pub inverts: Vec<(f64, String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipeData {
    pub name: String,
    pub from: String,
    pub to: String,
    /// metres (converted from diameterUnit)
    pub diameter_m: Option<f64>,
    pub reported_length: Option<f64>,
    pub reported_slope: Option<f64>,
    pub hgl_up: Option<f64>,
    pub hgl_down: Option<f64>,
    pub flow_in: Option<f64>,
    /// curved pipe: the Pipe's <Center> point (x, y). In Civil 3D 2025 exports this is the
    /// arc MID-POINT (equidistant from both ends), not the arc centre — measured, see docs/VALIDATION.md
    pub arc_center: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipeNetworkData {
    pub name: String,
    pub net_type: String,
    pub structs: Vec<StructData>,
    pub pipes: Vec<PipeData>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct LandXmlData {
    pub pipe_networks: Vec<PipeNetworkData>,
    pub application: Option<String>,
    pub points: Vec<SurveyPoint>,
    pub surfaces: Vec<SurfaceData>,
    pub alignments: Vec<AlignmentData>,
}

fn nums(t: &str) -> Vec<f64> {
    t.split_whitespace().filter_map(|s| s.parse().ok()).collect()
}
fn child<'a>(n: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}
fn ne(n: Option<Node>) -> Option<[f64; 2]> {
    let v = nums(n?.text()?);
    if v.len() >= 2 {
        Some([v[1], v[0]])
    } else {
        None
    }
}
fn attr_f(n: Node, k: &str) -> Option<f64> {
    let v = n.attribute(k)?;
    if v.eq_ignore_ascii_case("INF") {
        return Some(f64::INFINITY);
    }
    v.parse().ok()
}

pub fn read(text: &str) -> crate::Result<LandXmlData> {
    let doc = Document::parse(text).map_err(|e| format!("XML parse error: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "LandXML" {
        return Err("root element is not LandXML".into());
    }
    let mut out = LandXmlData::default();
    out.application = root
        .descendants()
        .find(|n| n.tag_name().name() == "Application")
        .map(|a| format!("{} {}", a.attribute("name").unwrap_or(""), a.attribute("version").unwrap_or("")));
    for p in root.descendants().filter(|n| n.tag_name().name() == "CgPoint") {
        let v = nums(p.text().unwrap_or(""));
        if v.len() >= 3 {
            out.points.push(SurveyPoint {
                id: p.attribute("name").unwrap_or("").into(),
                n: v[0],
                e: v[1],
                z: v[2],
                desc: p.attribute("desc").unwrap_or("").into(),
            });
        }
    }
    let dia_scale =
        match root.descendants().find(|n| n.tag_name().name() == "Metric" || n.tag_name().name() == "Imperial").and_then(|u| u.attribute("diameterUnit")) {
            Some("millimeter") => 0.001,
            Some("centimeter") => 0.01,
            Some("inch") => 0.0254,
            Some("foot") => 0.3048,
            _ => 1.0,
        };
    for pn in root.descendants().filter(|n| n.tag_name().name() == "PipeNetwork") {
        let mut structs = Vec::new();
        let mut pipes = Vec::new();
        for st in pn.descendants().filter(|n| n.tag_name().name() == "Struct") {
            let c = nums(child(st, "Center").and_then(|c| c.text()).unwrap_or(""));
            if c.len() < 2 {
                continue;
            }
            let inverts = st
                .children()
                .filter(|c| c.tag_name().name() == "Invert")
                .filter_map(|i| Some((attr_f(i, "elev")?, i.attribute("flowDir").unwrap_or("").to_string(), i.attribute("refPipe").unwrap_or("").to_string())))
                .collect();
            structs.push(StructData {
                name: st.attribute("name").unwrap_or("").into(),
                x: c[1],
                y: c[0],
                rim: attr_f(st, "elevRim"),
                sump: attr_f(st, "elevSump"),
                inverts,
            });
        }
        for p in pn.descendants().filter(|n| n.tag_name().name() == "Pipe") {
            let circ = child(p, "CircPipe");
            let flow = child(p, "PipeFlow");
            pipes.push(PipeData {
                name: p.attribute("name").unwrap_or("").into(),
                from: p.attribute("refStart").unwrap_or("").into(),
                to: p.attribute("refEnd").unwrap_or("").into(),
                diameter_m: circ.and_then(|c| attr_f(c, "diameter")).map(|d| d * dia_scale),
                reported_length: attr_f(p, "length"),
                reported_slope: attr_f(p, "slope"),
                hgl_up: flow.and_then(|f| attr_f(f, "hglUp")),
                hgl_down: flow.and_then(|f| attr_f(f, "hglDown")),
                flow_in: flow.and_then(|f| attr_f(f, "flowIn")),
                arc_center: ne(child(p, "Center")),
            });
        }
        out.pipe_networks.push(PipeNetworkData {
            name: pn.attribute("name").unwrap_or("").into(),
            net_type: pn.attribute("pipeNetType").unwrap_or("").into(),
            structs,
            pipes,
        });
    }
    for s in root.descendants().filter(|n| n.tag_name().name() == "Surface") {
        let Some(def) = child(s, "Definition") else { continue };
        if def.attribute("surfType").is_some_and(|t| t != "TIN") {
            continue;
        }
        let mut idx = std::collections::HashMap::new();
        let mut pts = Vec::new();
        if let Some(pn) = child(def, "Pnts") {
            for p in pn.children().filter(|c| c.is_element()) {
                let v = nums(p.text().unwrap_or(""));
                if v.len() >= 3 {
                    idx.insert(p.attribute("id").unwrap_or("").to_string(), pts.len());
                    pts.push([v[1], v[0], v[2]]);
                }
            }
        }
        let mut faces = Vec::new();
        let mut inv = 0;
        if let Some(fs) = child(def, "Faces") {
            for f in fs.children().filter(|c| c.is_element() && c.tag_name().name() == "F") {
                if f.attribute("i") == Some("1") {
                    inv += 1;
                    continue;
                }
                let ids: Vec<&str> = f.text().unwrap_or("").split_whitespace().collect();
                if ids.len() == 3 {
                    if let (Some(a), Some(b), Some(c)) = (idx.get(ids[0]), idx.get(ids[1]), idx.get(ids[2])) {
                        faces.push([*a, *b, *c]);
                    }
                }
            }
        }
        out.surfaces.push(SurfaceData {
            name: s.attribute("name").unwrap_or("").into(),
            points: pts,
            faces,
            invisible_faces: inv,
            reported_area2d: attr_f(def, "area2DSurf"),
            reported_area3d: attr_f(def, "area3DSurf"),
            reported_elev_min: attr_f(def, "elevMin"),
            reported_elev_max: attr_f(def, "elevMax"),
        });
    }
    for a in root.descendants().filter(|n| n.tag_name().name() == "Alignment") {
        let mut el = Vec::new();
        let mut dev: f64 = 0.0;
        let mut warn = Vec::new();
        if let Some(cg) = child(a, "CoordGeom") {
            for g in cg.children().filter(|c| c.is_element()) {
                let st = ne(child(g, "Start"));
                let en = ne(child(g, "End"));
                let (Some(st), Some(en)) = (st, en) else {
                    warn.push(format!("{} without Start/End skipped", g.tag_name().name()));
                    continue;
                };
                let sign = if g.attribute("rot") == Some("cw") { -1.0 } else { 1.0 };
                let e = match g.tag_name().name() {
                    "Line" => {
                        let len = ((en[0] - st[0]).powi(2) + (en[1] - st[1]).powi(2)).sqrt();
                        Element { kind: Kind::Line, sta: 0.0, len, start: st, az0: (en[1] - st[1]).atan2(en[0] - st[0]), k0: 0.0, k1: 0.0 }
                    }
                    "Curve" => {
                        let c = match ne(child(g, "Center")) {
                            Some(c) => c,
                            None => {
                                // derive the centre from chord + radius attribute + rotation (minor arc)
                                let r = attr_f(g, "radius").ok_or("Curve without Center or radius")?;
                                let (dx, dy) = (en[0] - st[0], en[1] - st[1]);
                                let ch = (dx * dx + dy * dy).sqrt();
                                if ch > 2.0 * r {
                                    return Err(format!("Curve chord {ch:.3} exceeds diameter {:.3}", 2.0 * r));
                                }
                                let h = (r * r - ch * ch / 4.0).sqrt();
                                let m = [st[0] + dx / 2.0, st[1] + dy / 2.0];
                                warn.push("Curve without Center: centre derived from radius and chord".into());
                                [m[0] - sign * dy / ch * h, m[1] + sign * dx / ch * h]
                            }
                        };
                        let r = ((st[0] - c[0]).powi(2) + (st[1] - c[1]).powi(2)).sqrt();
                        let ra = (st[1] - c[1]).atan2(st[0] - c[0]);
                        let az0 = ra + sign * std::f64::consts::FRAC_PI_2;
                        let re = (en[1] - c[1]).atan2(en[0] - c[0]);
                        let mut d = sign * (re - ra);
                        while d < 0.0 {
                            d += 2.0 * std::f64::consts::PI
                        }
                        let len = r * d;
                        if let Some(l) = attr_f(g, "length") {
                            if (l - len).abs() > 1e-3 {
                                warn.push(format!("Curve length attr {l:.4} vs geometry {len:.4}"));
                            }
                        }
                        Element { kind: Kind::Arc, sta: 0.0, len, start: st, az0, k0: sign / r, k1: sign / r }
                    }
                    "Spiral" => {
                        let pi = ne(child(g, "PI")).ok_or("Spiral without PI")?;
                        let len = attr_f(g, "length").ok_or("Spiral without length")?;
                        let rs = attr_f(g, "radiusStart").unwrap_or(f64::INFINITY);
                        let re = attr_f(g, "radiusEnd").unwrap_or(f64::INFINITY);
                        if let Some(t) = g.attribute("spiType") {
                            if t != "clothoid" {
                                warn.push(format!("spiral type {t} evaluated as clothoid"));
                            }
                        }
                        let k = |r: f64| if r.is_finite() && r > 0.0 { sign / r } else { 0.0 };
                        Element { kind: Kind::Spiral, sta: 0.0, len, start: st, az0: (pi[1] - st[1]).atan2(pi[0] - st[0]), k0: k(rs), k1: k(re) }
                    }
                    other => {
                        warn.push(format!("unsupported element {other} skipped"));
                        continue;
                    }
                };
                let q = e.point(e.len);
                dev = dev.max(((q[0] - en[0]).powi(2) + (q[1] - en[1]).powi(2)).sqrt());
                el.push(e);
            }
        }
        let mut profiles = Vec::new();
        for p in a.descendants().filter(|n| n.tag_name().name() == "ProfAlign") {
            let mut vpis = Vec::new();
            let mut lens = Vec::new();
            for v in p.children().filter(|c| c.is_element()) {
                let xy = nums(v.text().unwrap_or(""));
                if xy.len() < 2 {
                    continue;
                }
                match v.tag_name().name() {
                    "PVI" => {
                        vpis.push([xy[0], xy[1]]);
                        lens.push(0.0)
                    }
                    "ParaCurve" | "CircCurve" => {
                        if v.tag_name().name() == "CircCurve" {
                            warn.push(format!("CircCurve at {} approximated by a parabola of equal length", xy[0]));
                        }
                        vpis.push([xy[0], xy[1]]);
                        lens.push(attr_f(v, "length").unwrap_or(0.0))
                    }
                    other => warn.push(format!("profile element {other} skipped")),
                }
            }
            if vpis.len() >= 2 {
                let n = vpis.len();
                profiles.push(ProfileInput { name: p.attribute("name").unwrap_or("").into(), vpis, curve_lengths: lens[1..n - 1].to_vec() });
            }
        }
        out.alignments.push(AlignmentData {
            name: a.attribute("name").unwrap_or("").into(),
            sta_start: attr_f(a, "staStart").unwrap_or(0.0),
            reported_length: attr_f(a, "length"),
            elements: el,
            max_end_deviation: dev,
            warnings: warn,
            profiles,
        });
    }
    // lengths in the file's linear unit -> metres
    let units = root.descendants().find(|n| n.tag_name().name() == "Metric" || n.tag_name().name() == "Imperial");
    let lf = match units.and_then(|u| u.attribute("linearUnit")) {
        Some("foot") => 0.3048,
        Some("USSurveyFoot") => 1200.0 / 3937.0,
        Some("inch") => 0.0254,
        Some("mile") => 1609.344,
        Some("millimeter") => 0.001,
        Some("centimeter") => 0.01,
        Some("kilometer") => 1000.0,
        _ => 1.0,
    };
    if lf != 1.0 {
        scale(&mut out, lf, if units.and_then(|u| u.attribute("flowUnit")) == Some("cubicFeetSecond") { 0.3048f64.powi(3) } else { 1.0 });
    }
    Ok(out)
}

/// Converts every length of parsed data by `lf` (file unit -> m) and flows by `qf`.
fn scale(d: &mut LandXmlData, lf: f64, qf: f64) {
    for p in &mut d.points {
        p.n *= lf;
        p.e *= lf;
        p.z *= lf;
    }
    for s in &mut d.surfaces {
        for p in &mut s.points {
            for v in p.iter_mut() {
                *v *= lf;
            }
        }
        s.reported_area2d = s.reported_area2d.map(|a| a * lf * lf);
        s.reported_area3d = s.reported_area3d.map(|a| a * lf * lf);
        s.reported_elev_min = s.reported_elev_min.map(|a| a * lf);
        s.reported_elev_max = s.reported_elev_max.map(|a| a * lf);
    }
    for a in &mut d.alignments {
        a.sta_start *= lf;
        a.reported_length = a.reported_length.map(|v| v * lf);
        a.max_end_deviation *= lf;
        for e in &mut a.elements {
            e.start = [e.start[0] * lf, e.start[1] * lf];
            e.len *= lf;
            e.k0 /= lf;
            e.k1 /= lf;
        }
        for p in &mut a.profiles {
            for v in &mut p.vpis {
                v[0] *= lf;
                v[1] *= lf;
            }
            for c in &mut p.curve_lengths {
                *c *= lf;
            }
        }
    }
    for n in &mut d.pipe_networks {
        for s in &mut n.structs {
            s.x *= lf;
            s.y *= lf;
            s.rim = s.rim.map(|v| v * lf);
            s.sump = s.sump.map(|v| v * lf);
            for i in &mut s.inverts {
                i.0 *= lf;
            }
        }
        for p in &mut n.pipes {
            p.reported_length = p.reported_length.map(|v| v * lf);
            p.hgl_up = p.hgl_up.map(|v| v * lf);
            p.hgl_down = p.hgl_down.map(|v| v * lf);
            p.flow_in = p.flow_in.map(|v| v * qf);
            p.arc_center = p.arc_center.map(|c| [c[0] * lf, c[1] * lf]);
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
/// Unit system of a LandXML file. Imperial writes every length in feet (international or US
/// survey), areas in square feet, pipe diameters in inches and flows in ft³/s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LxUnits {
    Metric,
    Imperial { survey_foot: bool },
}

impl LxUnits {
    /// output length units per metre
    pub fn per_m(self) -> f64 {
        match self {
            LxUnits::Metric => 1.0,
            LxUnits::Imperial { survey_foot: false } => 1.0 / 0.3048,
            LxUnits::Imperial { survey_foot: true } => 3937.0 / 1200.0,
        }
    }
    pub fn from_session(u: crate::units::Units) -> LxUnits {
        if u.us() {
            LxUnits::Imperial { survey_foot: (u.ft() - 0.3048).abs() > 1e-12 }
        } else {
            LxUnits::Metric
        }
    }
}

pub struct Export<'a> {
    pub points: &'a [SurveyPoint],
    pub surfaces: Vec<(&'a str, &'a Tin)>,
    pub alignment: Option<&'a Alignment>,
    pub profiles: Vec<&'a Profile>,
    pub networks: Vec<&'a crate::pipes::Network>,
    /// corridor cross sections (EG and FG per station), written as CrossSects in the alignment;
    /// also gives the breaklines and outer boundary of the finished-grade surface
    pub corridor: Option<&'a crate::corridor::Corridor>,
    /// horizontal CRS of the coordinates, when known (from import_terrain)
    pub epsg: Option<u32>,
    /// imported parcels (user data), written as Parcels
    pub parcels: Option<&'a crate::parcels::ParcelSet>,
    pub superelevation: Option<&'a crate::superelev::Superelevation>,
    /// project name and description
    pub project: Option<(String, String)>,
    pub units: LxUnits,
}

impl Default for Export<'_> {
    fn default() -> Self {
        Export {
            points: &[],
            surfaces: vec![],
            alignment: None,
            profiles: vec![],
            networks: vec![],
            corridor: None,
            epsg: None,
            parcels: None,
            superelevation: None,
            project: None,
            units: LxUnits::Metric,
        }
    }
}

/// Name of the finished-grade surface whose breaklines / boundary come from the corridor.
pub const FG_SURFACE: &str = "Corridor FG";

pub fn write(x: &Export) -> String {
    let f = x.units.per_m();
    let l = |v: f64| v * f;
    // LandXML point order is northing easting
    let p2 = |p: [f64; 2]| format!("{:.6} {:.6}", p[1] * f, p[0] * f);
    // metres keep 4 decimals; feet get 6 so the round trip to metres stays below 1e-6 m
    let d3 = if f == 1.0 { 4 } else { 6 };
    let p3 = |p: [f64; 3]| format!("{:.d$} {:.d$} {:.d$}", p[1] * f, p[0] * f, p[2] * f, d = d3);
    let mut o = String::new();
    let _ = writeln!(o, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        o,
        "<LandXML xmlns=\"http://www.landxml.org/schema/LandXML-1.2\" version=\"1.2\" date=\"{}\" time=\"00:00:00\" language=\"English\" readOnly=\"false\">",
        crate::frame::today()
    );
    match x.units {
        LxUnits::Metric => {
            let _ = writeln!(o, "  <Units><Metric areaUnit=\"squareMeter\" linearUnit=\"meter\" volumeUnit=\"cubicMeter\" temperatureUnit=\"celsius\" pressureUnit=\"milliBars\" diameterUnit=\"millimeter\" flowUnit=\"cubicMeterSecond\" angularUnit=\"decimal degrees\" directionUnit=\"decimal degrees\" elevationUnit=\"meter\"/></Units>");
        }
        LxUnits::Imperial { survey_foot } => {
            let _ = writeln!(o, "  <Units><Imperial areaUnit=\"squareFoot\" linearUnit=\"{}\" volumeUnit=\"cubicYard\" temperatureUnit=\"fahrenheit\" pressureUnit=\"inHG\" diameterUnit=\"inch\" flowUnit=\"cubicFeetSecond\" angularUnit=\"decimal degrees\" directionUnit=\"decimal degrees\" elevationUnit=\"feet\"/></Units>", if survey_foot { "USSurveyFoot" } else { "foot" });
        }
    }
    if let Some(e) = x.epsg {
        let _ = writeln!(o, "  <CoordinateSystem name=\"EPSG:{e}\" epsgCode=\"{e}\" desc=\"horizontal CRS of the source terrain\"/>");
    }
    if let Some((n, d)) = &x.project {
        let _ = writeln!(o, "  <Project name=\"{}\" desc=\"{}\"/>", esc(n), esc(d));
    }
    let _ = writeln!(o, "  <Application name=\"civil-mcp\" desc=\"PRELIMINARY design export; not reviewed or approved by a licensed engineer\" manufacturer=\"civil-mcp\" version=\"{}\" manufacturerURL=\"https://github.com/reacherwu/civil-mcp\"/>", env!("CARGO_PKG_VERSION"));
    if !x.points.is_empty() {
        let _ = writeln!(o, "  <CgPoints>");
        for p in x.points {
            let _ = writeln!(o, "    <CgPoint name=\"{}\" desc=\"{}\">{:.4} {:.4} {:.4}</CgPoint>", esc(&p.id), esc(&p.desc), l(p.n), l(p.e), l(p.z));
        }
        let _ = writeln!(o, "  </CgPoints>");
    }
    if !x.surfaces.is_empty() {
        let _ = writeln!(o, "  <Surfaces>");
        for (name, t) in &x.surfaces {
            let _ = writeln!(o, "    <Surface name=\"{}\">", esc(name));
            if *name == FG_SURFACE {
                if let Some(c) = x.corridor.filter(|c| c.sections.len() >= 2) {
                    let (bl, bnd) = fg_lines(c);
                    let _ = writeln!(o, "      <SourceData>\n        <Boundaries>\n          <Boundary name=\"daylight\" bndType=\"outer\" edgeTrim=\"true\" desc=\"corridor daylight (catch) lines\"><PntList2D>{}</PntList2D></Boundary>\n        </Boundaries>\n        <Breaklines>", bnd.iter().map(|q| p2(*q)).collect::<Vec<_>>().join(" "));
                    for (n, v) in &bl {
                        let _ = writeln!(
                            o,
                            "          <Breakline name=\"{n}\" brkType=\"standard\"><PntList3D>{}</PntList3D></Breakline>",
                            v.iter().map(|q| p3(*q)).collect::<Vec<_>>().join(" ")
                        );
                    }
                    let _ = writeln!(o, "        </Breaklines>\n      </SourceData>");
                }
            }
            let (zmin, zmax) = t.pts.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[2]), a.1.max(p[2])));
            let _ = writeln!(o, "      <Definition surfType=\"TIN\" elevMin=\"{:.4}\" elevMax=\"{:.4}\">\n        <Pnts>", l(zmin), l(zmax));
            for (i, p) in t.pts.iter().enumerate() {
                let _ = writeln!(o, "          <P id=\"{}\">{}</P>", i + 1, p3(*p));
            }
            let _ = writeln!(o, "        </Pnts>\n        <Faces>");
            for fc in &t.tris {
                let _ = writeln!(o, "          <F>{} {} {}</F>", fc[0] + 1, fc[1] + 1, fc[2] + 1);
            }
            let _ = writeln!(o, "        </Faces>\n      </Definition>\n    </Surface>");
        }
        let _ = writeln!(o, "  </Surfaces>");
    }
    if let Some(ps) = x.parcels.filter(|p| !p.parcels.is_empty()) {
        let _ = writeln!(
            o,
            "  <Parcels name=\"imported parcels\" desc=\"user data from {}; ROW screening only, not a boundary survey\">",
            esc(ps.source.rsplit('/').next().unwrap_or(""))
        );
        let ring = |o: &mut String, r: &[[f64; 2]], ind: &str| {
            let _ = writeln!(o, "{ind}<CoordGeom>");
            for k in 0..r.len() {
                let _ = writeln!(o, "{ind}  <Line><Start>{}</Start><End>{}</End></Line>", p2(r[k]), p2(r[(k + 1) % r.len()]));
            }
            let _ = writeln!(o, "{ind}</CoordGeom>");
        };
        for p in &ps.parcels {
            let mut desc = p.zoning.clone().map(|z| format!("zoning {z}")).unwrap_or_default();
            if p.no_go {
                desc = format!("{desc}{}no-go: {}", if desc.is_empty() { "" } else { "; " }, p.no_go_reason.clone().unwrap_or_default());
            }
            let _ = writeln!(o, "    <Parcel name=\"{}\" area=\"{:.4}\" desc=\"{}\">", esc(&p.id), p.area_m2 * f * f, esc(&desc));
            for r in p.rings.iter().filter(|r| crate::parcels::signed_area(r) > 0.0) {
                ring(&mut o, r, "      ");
            }
            // interior rings (holes) as nested parcels marked as excluded areas
            let holes: Vec<_> = p.rings.iter().filter(|r| crate::parcels::signed_area(r) < 0.0).collect();
            if !holes.is_empty() {
                let _ = writeln!(o, "      <Parcels name=\"{} holes\">", esc(&p.id));
                for (k, r) in holes.iter().enumerate() {
                    let mut r = r.to_vec();
                    r.reverse();
                    let _ = writeln!(
                        o,
                        "        <Parcel name=\"{}-hole-{}\" area=\"{:.4}\" desc=\"interior ring (excluded area) of parcel {}\">",
                        esc(&p.id),
                        k + 1,
                        crate::parcels::signed_area(&r) * f * f,
                        esc(&p.id)
                    );
                    ring(&mut o, &r, "          ");
                    let _ = writeln!(o, "        </Parcel>");
                }
                let _ = writeln!(o, "      </Parcels>");
            }
            let _ = writeln!(o, "    </Parcel>");
        }
        let _ = writeln!(o, "  </Parcels>");
    }
    if let Some(al) = x.alignment {
        let _ = writeln!(
            o,
            "  <Alignments name=\"civil-mcp\">\n    <Alignment name=\"{}\" length=\"{:.6}\" staStart=\"{:.6}\">\n      <CoordGeom>",
            esc(&al.name),
            l(al.length),
            l(al.start_station)
        );
        let mut sta = al.start_station;
        let deg = |a: f64| a.to_degrees();
        for e in &al.elements {
            let st = e.start;
            let en = e.point(e.len);
            let chord = ((en[0] - st[0]).powi(2) + (en[1] - st[1]).powi(2)).sqrt();
            match e.kind {
                Kind::Line => {
                    let _ = writeln!(
                        o,
                        "        <Line staStart=\"{:.6}\" length=\"{:.6}\"><Start>{}</Start><End>{}</End></Line>",
                        l(sta),
                        l(e.len),
                        p2(st),
                        p2(en)
                    );
                }
                Kind::Arc => {
                    let r = 1.0 / e.k0.abs();
                    let n = e.az0 + e.k0.signum() * std::f64::consts::FRAC_PI_2;
                    let c = [st[0] + r * n.cos(), st[1] + r * n.sin()];
                    let rot = if e.k0 > 0.0 { "ccw" } else { "cw" };
                    let d = e.len / r;
                    let t = r * (d / 2.0).tan();
                    let pi = [st[0] + t * e.az0.cos(), st[1] + t * e.az0.sin()];
                    let _ = writeln!(
                        o,
                        "        <Curve rot=\"{rot}\" crvType=\"arc\" staStart=\"{:.6}\" radius=\"{:.6}\" length=\"{:.6}\" delta=\"{:.9}\" chord=\"{:.6}\" tangent=\"{:.6}\" external=\"{:.6}\" midOrd=\"{:.6}\"><Start>{}</Start><Center>{}</Center><End>{}</End><PI>{}</PI></Curve>",
                        l(sta),
                        l(r),
                        l(e.len),
                        deg(d),
                        l(chord),
                        l(t),
                        l(r * (1.0 / (d / 2.0).cos() - 1.0)),
                        l(r * (1.0 - (d / 2.0).cos())),
                        p2(st),
                        p2(c),
                        p2(en),
                        p2(pi)
                    );
                }
                Kind::Spiral => {
                    let k = if e.k0 != 0.0 { e.k0 } else { e.k1 };
                    let rot = if k > 0.0 { "ccw" } else { "cw" };
                    let r = |k: f64| if k == 0.0 { "INF".to_string() } else { format!("{:.6}", l(1.0 / k.abs())) };
                    // PI = intersection of start and end tangents
                    let (a0, a1) = (e.az0, e.heading(e.len));
                    let dd = (a1 - a0).sin();
                    let tt = ((en[0] - st[0]) * a1.sin() - (en[1] - st[1]) * a1.cos()) / dd;
                    let pi = [st[0] + tt * a0.cos(), st[1] + tt * a0.sin()];
                    let dist = |a: [f64; 2], b: [f64; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
                    // full spirals (one end straight): theta, A, long / short tangents, X / Y from the tangent end
                    let mut extra = String::new();
                    if (e.k0 == 0.0) != (e.k1 == 0.0) {
                        let entering = e.k0 == 0.0;
                        let rr = 1.0 / k.abs();
                        let (t0, h0, cv) = if entering { (st, a0, en) } else { (en, a1 + std::f64::consts::PI, st) };
                        let (dx, dy) = (cv[0] - t0[0], cv[1] - t0[1]);
                        let xx = dx * h0.cos() + dy * h0.sin();
                        let yy = (-dx * h0.sin() + dy * h0.cos()).abs();
                        extra = format!(
                            " theta=\"{:.9}\" constant=\"{:.6}\" totalX=\"{:.6}\" totalY=\"{:.6}\" tanLong=\"{:.6}\" tanShort=\"{:.6}\"",
                            deg(e.len / (2.0 * rr)),
                            l((rr * e.len).sqrt()),
                            l(xx),
                            l(yy),
                            l(dist(t0, pi)),
                            l(dist(pi, cv))
                        );
                    }
                    let _ = writeln!(
                        o,
                        "        <Spiral staStart=\"{:.6}\" length=\"{:.6}\" radiusStart=\"{}\" radiusEnd=\"{}\" rot=\"{rot}\" spiType=\"clothoid\" chord=\"{:.6}\"{extra}><Start>{}</Start><PI>{}</PI><End>{}</End></Spiral>",
                        l(sta),
                        l(e.len),
                        r(e.k0),
                        r(e.k1),
                        l(chord),
                        p2(st),
                        p2(pi),
                        p2(en)
                    );
                }
            }
            sta += e.len;
        }
        let _ = writeln!(o, "      </CoordGeom>");
        // sampled existing-ground profile (ProfSurf) and the design profiles (ProfAlign)
        let eg = x.surfaces.iter().find(|s| s.0 == "EG").map(|s| s.1);
        if !x.profiles.is_empty() || eg.is_some() {
            let _ = writeln!(o, "      <Profile name=\"{}\" staStart=\"{:.6}\">", esc(&al.name), l(al.start_station));
            if let Some(t) = eg {
                let n = (al.length / 10.0).ceil().max(1.0) as usize;
                let pts: Vec<String> = (0..=n)
                    .filter_map(|i| {
                        let s = al.start_station + al.length * i as f64 / n as f64;
                        let (px, py, _) = al.point_at(s)?;
                        Some(format!("{:.4} {:.4}", l(s), l(t.elevation_at(px, py)?)))
                    })
                    .collect();
                let _ = writeln!(
                    o,
                    "        <ProfSurf name=\"EG\" desc=\"existing ground sampled every {:.2} along the alignment\"><PntList2D>{}</PntList2D></ProfSurf>",
                    l(al.length / n as f64),
                    pts.join(" ")
                );
            }
            for p in &x.profiles {
                let _ = writeln!(o, "        <ProfAlign name=\"{}\">", esc(&p.input.name));
                let v = &p.input.vpis;
                for (i, q) in v.iter().enumerate() {
                    let cl = if i > 0 && i + 1 < v.len() { p.input.curve_lengths[i - 1] } else { 0.0 };
                    if cl > 0.0 {
                        let _ = writeln!(o, "          <ParaCurve length=\"{:.6}\">{:.6} {:.6}</ParaCurve>", l(cl), l(q[0]), l(q[1]));
                    } else {
                        let _ = writeln!(o, "          <PVI>{:.6} {:.6}</PVI>", l(q[0]), l(q[1]));
                    }
                }
                let _ = writeln!(o, "        </ProfAlign>");
            }
            let _ = writeln!(o, "      </Profile>");
        }
        if let Some(c) = x.corridor.filter(|c| !c.sections.is_empty()) {
            // offsets: left negative, right positive (LandXML / Civil 3D convention)
            let _ = writeln!(o, "      <CrossSects name=\"{} sections\" calcMethod=\"AverageEndArea\">", esc(&al.name));
            let pl = |v: &[[f64; 2]]| v.iter().map(|q| format!("{:.4} {:.4}", l(q[0]), l(q[1]))).collect::<Vec<_>>().join(" ");
            for sct in &c.sections {
                let mut eg: Vec<[f64; 2]> = sct.ground.iter().map(|q| [-q[0], q[1]]).collect();
                eg.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
                let mut fg: Vec<[f64; 2]> = sct.left.fg.iter().rev().map(|q| [-q[0], q[1]]).collect();
                fg.extend(sct.right.fg.iter().skip(1).copied());
                let dpts: String = fg.iter().map(|q| format!("<CrossSectPnt>{:.4} {:.4}</CrossSectPnt>", l(q[0]), l(q[1]))).collect();
                let _ = writeln!(
                    o,
                    "        <CrossSect sta=\"{:.4}\" name=\"{:.3}\" areaCut=\"{:.4}\" areaFill=\"{:.4}\">\n          <CrossSectSurf name=\"EG\"><PntList2D>{}</PntList2D></CrossSectSurf>\n          <CrossSectSurf name=\"FG\"><PntList2D>{}</PntList2D></CrossSectSurf>\n          <DesignCrossSectSurf name=\"FG\" side=\"both\" closedArea=\"false\">{}</DesignCrossSectSurf>\n        </CrossSect>",
                    l(sct.station),
                    l(sct.station),
                    sct.cut_area * f * f,
                    sct.fill_area * f * f,
                    pl(&eg),
                    pl(&fg),
                    dpts
                );
            }
            let _ = writeln!(o, "      </CrossSects>");
        }
        if let Some(se) = x.superelevation {
            for c in se.curves.iter().filter(|c| c.e > 0.0 && c.b_in > c.a_in && c.a_out > c.b_out) {
                // outer edge rises linearly from -nc at A to +e at B: level crown at nc/(e+nc) of A-B
                let fr = se.nc / (c.e + se.nc);
                let lvl_in = c.a_in + (c.b_in - c.a_in) * fr;
                let lvl_out = c.a_out - (c.a_out - c.b_out) * fr;
                let _ = writeln!(
                    o,
                    "      <Superelevation staStart=\"{:.6}\" staEnd=\"{:.6}\">\n        <BeginRunoutSta>{:.6}</BeginRunoutSta>\n        <BeginRunoffSta>{:.6}</BeginRunoffSta>\n        <FullSuperSta>{:.6}</FullSuperSta>\n        <FullSuperelev>{:.4}</FullSuperelev>\n        <RunoffSta>{:.6}</RunoffSta>\n        <StartofRunoutSta>{:.6}</StartofRunoutSta>\n        <EndofRunoutSta>{:.6}</EndofRunoutSta>\n        <AdverseSE>non-adverse</AdverseSE>\n      </Superelevation>",
                    l(c.a_in),
                    l(c.a_out),
                    l(c.a_in),
                    l(lvl_in),
                    l(c.b_in),
                    c.e * 100.0,
                    l(c.b_out),
                    l(lvl_out),
                    l(c.a_out)
                );
            }
        }
        let _ = writeln!(o, "    </Alignment>\n  </Alignments>");
    }
    if !x.networks.is_empty() {
        let dia = match x.units {
            LxUnits::Metric => 1000.0,
            LxUnits::Imperial { .. } => 1.0 / 0.0254,
        };
        let q = match x.units {
            LxUnits::Metric => 1.0,
            LxUnits::Imperial { .. } => 1.0 / 0.3048f64.powi(3),
        };
        let _ = writeln!(o, "  <PipeNetworks>");
        for n in &x.networks {
            let ty = if n.pipes.iter().any(|p| p.q_avg > 0.0) { "sanitary" } else { "storm" };
            let _ = writeln!(o, "    <PipeNetwork name=\"{}\" pipeNetType=\"{ty}\">\n      <Structs>", esc(&n.name));
            for (id, sx, sy, rim, sump) in &n.structures {
                let _ = writeln!(o, "        <Struct name=\"{}\" elevRim=\"{:.4}\" elevSump=\"{:.4}\">\n          <Center>{}</Center>\n          <CircStruct diameter=\"{:.1}\" desc=\"placeholder size: structure dimensions are not designed by civil-mcp\"/>", esc(id), l(*rim), l(*sump), p2([*sx, *sy]), 1.0 * dia);
                for p in n.pipes.iter().filter(|p| &p.to == id) {
                    let _ = writeln!(o, "          <Invert elev=\"{:.4}\" flowDir=\"in\" refPipe=\"{}\"/>", l(p.invert_dn), esc(&p.id));
                }
                for p in n.pipes.iter().filter(|p| &p.from == id) {
                    let _ = writeln!(o, "          <Invert elev=\"{:.4}\" flowDir=\"out\" refPipe=\"{}\"/>", l(p.invert_up), esc(&p.id));
                }
                let _ = writeln!(o, "        </Struct>");
            }
            let _ = writeln!(o, "      </Structs>\n      <Pipes>");
            for p in &n.pipes {
                let mat = if p.material.is_empty() { "not specified".to_string() } else { p.material.clone() };
                let _ = writeln!(
                    o,
                    "        <Pipe name=\"{}\" refStart=\"{}\" refEnd=\"{}\" length=\"{:.4}\" slope=\"{:.6}\" desc=\"material: {}\">\n          <CircPipe diameter=\"{:.2}\" desc=\"{}\"/>\n          <PipeFlow flowIn=\"{:.6}\" hglUp=\"{:.4}\" hglDown=\"{:.4}\"/>\n          <Feature name=\"civil-mcp\"><Property label=\"material\" value=\"{}\"/><Property label=\"invertUp\" value=\"{:.4}\"/><Property label=\"invertDown\" value=\"{:.4}\"/></Feature>\n        </Pipe>",
                    esc(&p.id),
                    esc(&p.from),
                    esc(&p.to),
                    l(p.length),
                    p.slope,
                    esc(&mat),
                    p.diameter_m * dia,
                    esc(&mat),
                    p.q_design * q,
                    l(p.hgl_up),
                    l(p.hgl_dn),
                    esc(&mat),
                    l(p.invert_up),
                    l(p.invert_dn)
                );
            }
            let _ = writeln!(o, "      </Pipes>\n    </PipeNetwork>");
        }
        let _ = writeln!(o, "  </PipeNetworks>");
    }
    let _ = writeln!(o, "</LandXML>");
    o
}

/// Finished-grade breaklines (centreline, edges of paved width, daylight lines) and the outer
/// boundary (daylight polygon, closed) from the corridor sections, world x/y/z in metres.
pub fn fg_lines(c: &crate::corridor::Corridor) -> (Vec<(String, Vec<[f64; 3]>)>, Vec<[f64; 2]>) {
    let at = |fg: &[[f64; 2]], off: f64| -> f64 {
        for w in fg.windows(2) {
            if off >= w[0][0] - 1e-9 && off <= w[1][0] + 1e-9 && w[1][0] > w[0][0] {
                return w[0][1] + (w[1][1] - w[0][1]) * (off - w[0][0]) / (w[1][0] - w[0][0]);
            }
        }
        fg.last().map_or(0.0, |q| q[1])
    };
    let pt = |s: &crate::corridor::Section, left: bool, off: f64, z: f64| {
        let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, off, left);
        [q[0], q[1], z]
    };
    let mut cl = vec![];
    let (mut epl, mut epr, mut dll, mut dlr) = (vec![], vec![], vec![], vec![]);
    for s in &c.sections {
        cl.push([s.x, s.y, s.cl_elev]);
        epl.push(pt(s, true, s.left.paved_width, at(&s.left.fg, s.left.paved_width)));
        epr.push(pt(s, false, s.right.paved_width, at(&s.right.fg, s.right.paved_width)));
        let (ql, qr) = (s.left.fg.last().copied().unwrap_or([0.0, s.cl_elev]), s.right.fg.last().copied().unwrap_or([0.0, s.cl_elev]));
        dll.push(pt(s, true, ql[0], ql[1]));
        dlr.push(pt(s, false, qr[0], qr[1]));
    }
    let mut bnd = crate::parcels::row_polygon(c, 0.0);
    if let Some(first) = bnd.first().copied() {
        bnd.push(first);
    }
    (vec![("CL".into(), cl), ("EP-L".into(), epl), ("EP-R".into(), epr), ("DAYLIGHT-L".into(), dll), ("DAYLIGHT-R".into(), dlr)], bnd)
}

/// Build an Alignment from imported data.
pub fn to_alignment(a: &AlignmentData) -> crate::Result<Alignment> {
    Alignment::from_elements(&a.name, a.sta_start, a.elements.clone())
}
