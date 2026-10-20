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
    out.application = root.descendants().find(|n| n.tag_name().name() == "Application").map(|a| format!("{} {}", a.attribute("name").unwrap_or(""), a.attribute("version").unwrap_or("")));
    for p in root.descendants().filter(|n| n.tag_name().name() == "CgPoint") {
        let v = nums(p.text().unwrap_or(""));
        if v.len() >= 3 {
            out.points.push(SurveyPoint { id: p.attribute("name").unwrap_or("").into(), n: v[0], e: v[1], z: v[2], desc: p.attribute("desc").unwrap_or("").into() });
        }
    }
    let dia_scale = match root.descendants().find(|n| n.tag_name().name() == "Metric" || n.tag_name().name() == "Imperial").and_then(|u| u.attribute("diameterUnit")) {
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
            let inverts = st.children().filter(|c| c.tag_name().name() == "Invert").filter_map(|i| Some((attr_f(i, "elev")?, i.attribute("flowDir").unwrap_or("").to_string(), i.attribute("refPipe").unwrap_or("").to_string()))).collect();
            structs.push(StructData { name: st.attribute("name").unwrap_or("").into(), x: c[1], y: c[0], rim: attr_f(st, "elevRim"), sump: attr_f(st, "elevSump"), inverts });
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
            });
        }
        out.pipe_networks.push(PipeNetworkData { name: pn.attribute("name").unwrap_or("").into(), net_type: pn.attribute("pipeNetType").unwrap_or("").into(), structs, pipes });
    }
    for s in root.descendants().filter(|n| n.tag_name().name() == "Surface") {
        let Some(def) = child(s, "Definition") else { continue };
        if def.attribute("surfType").map_or(false, |t| t != "TIN") {
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
    Ok(out)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
fn p2(p: [f64; 2]) -> String {
    format!("{:.6} {:.6}", p[1], p[0])
}

pub struct Export<'a> {
    pub points: &'a [SurveyPoint],
    pub surfaces: Vec<(&'a str, &'a Tin)>,
    pub alignment: Option<&'a Alignment>,
    pub profiles: Vec<&'a Profile>,
    pub networks: Vec<&'a crate::pipes::Network>,
}

pub fn write(x: &Export) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(o, "<LandXML xmlns=\"http://www.landxml.org/schema/LandXML-1.2\" version=\"1.2\" date=\"2026-01-01\" time=\"00:00:00\">");
    let _ = writeln!(o, "  <Units><Metric areaUnit=\"squareMeter\" linearUnit=\"meter\" volumeUnit=\"cubicMeter\" temperatureUnit=\"celsius\" pressureUnit=\"milliBars\" angularUnit=\"decimal degrees\" directionUnit=\"decimal degrees\"/></Units>");
    let _ = writeln!(o, "  <Application name=\"civil-mcp\" manufacturer=\"civil-mcp\" version=\"{}\"/>", env!("CARGO_PKG_VERSION"));
    if !x.points.is_empty() {
        let _ = writeln!(o, "  <CgPoints>");
        for p in x.points {
            let _ = writeln!(o, "    <CgPoint name=\"{}\" desc=\"{}\">{:.4} {:.4} {:.4}</CgPoint>", esc(&p.id), esc(&p.desc), p.n, p.e, p.z);
        }
        let _ = writeln!(o, "  </CgPoints>");
    }
    if !x.surfaces.is_empty() {
        let _ = writeln!(o, "  <Surfaces>");
        for (name, t) in &x.surfaces {
            let _ = writeln!(o, "    <Surface name=\"{}\">\n      <Definition surfType=\"TIN\">\n        <Pnts>", esc(name));
            for (i, p) in t.pts.iter().enumerate() {
                let _ = writeln!(o, "          <P id=\"{}\">{:.4} {:.4} {:.4}</P>", i + 1, p[1], p[0], p[2]);
            }
            let _ = writeln!(o, "        </Pnts>\n        <Faces>");
            for f in &t.tris {
                let _ = writeln!(o, "          <F>{} {} {}</F>", f[0] + 1, f[1] + 1, f[2] + 1);
            }
            let _ = writeln!(o, "        </Faces>\n      </Definition>\n    </Surface>");
        }
        let _ = writeln!(o, "  </Surfaces>");
    }
    if let Some(al) = x.alignment {
        let _ = writeln!(o, "  <Alignments>\n    <Alignment name=\"{}\" length=\"{:.6}\" staStart=\"{:.6}\">\n      <CoordGeom>", esc(&al.name), al.length, al.start_station);
        for e in &al.elements {
            let st = e.start;
            let en = e.point(e.len);
            match e.kind {
                Kind::Line => {
                    let _ = writeln!(o, "        <Line length=\"{:.6}\"><Start>{}</Start><End>{}</End></Line>", e.len, p2(st), p2(en));
                }
                Kind::Arc => {
                    let r = 1.0 / e.k0.abs();
                    let n = e.az0 + e.k0.signum() * std::f64::consts::FRAC_PI_2;
                    let c = [st[0] + r * n.cos(), st[1] + r * n.sin()];
                    let rot = if e.k0 > 0.0 { "ccw" } else { "cw" };
                    let _ = writeln!(o, "        <Curve rot=\"{rot}\" crvType=\"arc\" radius=\"{r:.6}\" length=\"{:.6}\"><Start>{}</Start><Center>{}</Center><End>{}</End></Curve>", e.len, p2(st), p2(c), p2(en));
                }
                Kind::Spiral => {
                    let k = if e.k0 != 0.0 { e.k0 } else { e.k1 };
                    let rot = if k > 0.0 { "ccw" } else { "cw" };
                    let r = |k: f64| if k == 0.0 { "INF".to_string() } else { format!("{:.6}", 1.0 / k.abs()) };
                    // PI = intersection of start and end tangents
                    let (a0, a1) = (e.az0, e.heading(e.len));
                    let d = (a1 - a0).sin();
                    let t = ((en[0] - st[0]) * a1.sin() - (en[1] - st[1]) * a1.cos()) / d;
                    let pi = [st[0] + t * a0.cos(), st[1] + t * a0.sin()];
                    let _ = writeln!(o, "        <Spiral length=\"{:.6}\" radiusStart=\"{}\" radiusEnd=\"{}\" rot=\"{rot}\" spiType=\"clothoid\"><Start>{}</Start><PI>{}</PI><End>{}</End></Spiral>", e.len, r(e.k0), r(e.k1), p2(st), p2(pi), p2(en));
                }
            }
        }
        let _ = writeln!(o, "      </CoordGeom>");
        for p in &x.profiles {
            let _ = writeln!(o, "      <Profile name=\"{}\">\n        <ProfAlign name=\"{}\">", esc(&p.input.name), esc(&p.input.name));
            let v = &p.input.vpis;
            for (i, q) in v.iter().enumerate() {
                let l = if i > 0 && i + 1 < v.len() { p.input.curve_lengths[i - 1] } else { 0.0 };
                if l > 0.0 {
                    let _ = writeln!(o, "          <ParaCurve length=\"{l:.6}\">{:.6} {:.6}</ParaCurve>", q[0], q[1]);
                } else {
                    let _ = writeln!(o, "          <PVI>{:.6} {:.6}</PVI>", q[0], q[1]);
                }
            }
            let _ = writeln!(o, "        </ProfAlign>\n      </Profile>");
        }
        let _ = writeln!(o, "    </Alignment>\n  </Alignments>");
    }
    if !x.networks.is_empty() {
        let _ = writeln!(o, "  <PipeNetworks>");
        for n in &x.networks {
            let ty = if n.pipes.iter().any(|p| p.q_avg > 0.0) { "sanitary" } else { "storm" };
            let _ = writeln!(o, "    <PipeNetwork name=\"{}\" pipeNetType=\"{ty}\">\n      <Structs>", esc(&n.name));
            for (id, sx, sy, rim, sump) in &n.structures {
                let _ = writeln!(o, "        <Struct name=\"{}\" elevRim=\"{rim:.4}\" elevSump=\"{sump:.4}\">\n          <Center>{sy:.4} {sx:.4}</Center>\n          <CircStruct diameter=\"1.0\" desc=\"placeholder size: structure dimensions are not designed by civil-mcp\"/>", esc(id));
                for p in n.pipes.iter().filter(|p| &p.to == id) {
                    let _ = writeln!(o, "          <Invert elev=\"{:.4}\" flowDir=\"in\" refPipe=\"{}\"/>", p.invert_dn, esc(&p.id));
                }
                for p in n.pipes.iter().filter(|p| &p.from == id) {
                    let _ = writeln!(o, "          <Invert elev=\"{:.4}\" flowDir=\"out\" refPipe=\"{}\"/>", p.invert_up, esc(&p.id));
                }
                let _ = writeln!(o, "        </Struct>");
            }
            let _ = writeln!(o, "      </Structs>\n      <Pipes>");
            for p in &n.pipes {
                let _ = writeln!(o, "        <Pipe name=\"{}\" refStart=\"{}\" refEnd=\"{}\" length=\"{:.4}\" slope=\"{:.6}\">\n          <CircPipe diameter=\"{:.4}\"/>\n          <PipeFlow flowIn=\"{:.6}\" hglUp=\"{:.4}\" hglDown=\"{:.4}\"/>\n        </Pipe>", esc(&p.id), esc(&p.from), esc(&p.to), p.length, p.slope, p.diameter_m, p.q_design, p.hgl_up, p.hgl_dn);
            }
            let _ = writeln!(o, "      </Pipes>\n    </PipeNetwork>");
        }
        let _ = writeln!(o, "  </PipeNetworks>");
    }
    let _ = writeln!(o, "</LandXML>");
    o
}

/// Build an Alignment from imported data.
pub fn to_alignment(a: &AlignmentData) -> crate::Result<Alignment> {
    Alignment::from_elements(&a.name, a.sta_start, a.elements.clone())
}
