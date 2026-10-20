//! MCP tool surface. Every call that changes design state is written to the
//! audit log; every result carries a `provenance` record.
use crate::align::{Alignment, AlignmentInput};
use crate::audit::{self, sha256_hex, AuditLog};
use crate::corridor::{self, Assembly, Corridor};
use crate::drawing;
use crate::grading::{self, Grading, GradingInput};
use crate::landxml;
use crate::pipes::{self, Network, NetworkInput};
use crate::points::{self, SurveyPoint};
use crate::profile::{Profile, ProfileInput};
use crate::quantities::{self, MassHaul, MaterialRow};
use crate::sheet::{self, Design, SheetInfo};
use crate::standards::{self, Check, DesignBasis, Standard};
use crate::superelev::{self, Superelevation};
use crate::tin::{Tin, TinStats};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Default)]
pub struct Session {
    pub dir: Option<PathBuf>,
    log: Option<AuditLog>,
    pub basis: Option<DesignBasis>,
    pub points: Vec<SurveyPoint>,
    pub tin: Option<(Tin, TinStats)>,
    pub alignment: Option<Alignment>,
    pub profile: Option<Profile>,
    pub superelevation: Option<Superelevation>,
    pub checks: Vec<Check>,
    pub assembly: Option<Assembly>,
    pub corridor: Option<Corridor>,
    pub corridor_fg: Option<Tin>,
    pub corridor_datum: Option<Tin>,
    pub composite: Option<(f64, f64, usize, f64)>,
    pub gradings: Vec<Grading>,
    pub networks: Vec<Network>,
    pub intersections: Vec<crate::intersect::Intersection>,
    pub mass_haul: Option<MassHaul>,
    pub materials: Vec<MaterialRow>,
    pub files: Vec<(String, String)>,
    pub approved_seq: Option<u64>,
}

fn obj(props: Value, req: &[&str]) -> Value {
    json!({"type":"object","properties":props,"required":req})
}

pub fn tool_list() -> Value {
    let num = json!({"type":"number"});
    let s = json!({"type":"string"});
    let b = json!({"type":"boolean"});
    let xy = json!({"type":"array","items":{"type":"array","items":{"type":"number"},"minItems":2,"maxItems":2}});
    let xyz = json!({"type":"array","items":{"type":"array","items":{"type":"number"},"minItems":3,"maxItems":3}});
    let arr = |t: Value| json!({"type":"array","items":t});
    json!([
      {"name":"open_project","description":"Open (or create) a project directory. The audit log lives at <dir>/audit.jsonl and is verified on open.","inputSchema":obj(json!({"dir":s}),&["dir"])},
      {"name":"set_design_basis","description":"Record the design basis chosen by the engineer in responsible charge. Must be set before design tools run. actor must be 'engineer:<name>'.","inputSchema":obj(json!({"actor":s,"basis":{"type":"object","properties":{"project":s,"engineer_of_record":s,"standard":{"type":"string","enum":["AASHTO_GREEN_BOOK_2018","JTG_D20_2017"]},"design_speed_kmh":num,"e_max":num,"f_max":num,"max_grade_pct":num,"perception_reaction_s":num,"deceleration_mps2":num,"normal_crown":num,"lane_width_m":num,"lanes_rotated_per_side":num},"required":["project","engineer_of_record","standard","design_speed_kmh","e_max","f_max"]}}),&["actor","basis"])},
      {"name":"load_points","description":"Load survey points from CSV (id,easting,northing,elevation,desc) or LandXML CgPoints. Rejected rows are reported.","inputSchema":obj(json!({"path":s,"format":{"type":"string","enum":["csv","landxml"]}}),&["path"])},
      {"name":"import_landxml","description":"Import a LandXML 1.x file: TIN surface (as existing ground), alignment (line/arc/clothoid) and profile. Reports differences against values stored in the file (areas, elevations, lengths, element end points).","inputSchema":obj(json!({"path":s,"surface":s,"alignment":s,"use_surface":b,"use_alignment":b}),&["path"])},
      {"name":"export_landxml","description":"Write LandXML 1.2 with points, existing-ground and corridor surfaces, alignment and profile.","inputSchema":obj(json!({"file":s}),&[])},
      {"name":"build_surface","description":"Build a Delaunay TIN from loaded points. Optional max_edge_m drops long hull triangles.","inputSchema":obj(json!({"max_edge_m":num}),&[])},
      {"name":"create_alignment","description":"Horizontal alignment from PIs, circular radii and optional symmetric clothoid lengths (tangent–spiral–arc–spiral–tangent). Fails if curves overlap.","inputSchema":obj(json!({"name":s,"pis":xy,"radii":arr(num.clone()),"spirals":arr(num.clone()),"start_station":num}),&["name","pis","radii"])},
      {"name":"sample_ground_profile","description":"Sample existing ground along the alignment.","inputSchema":obj(json!({"interval_m":num}),&[])},
      {"name":"suggest_profile","description":"Propose a design profile: VPIs on ground at the given stations, grades clamped to the basis maximum, vertical curve lengths set to the minimum required, rounded up to 5 m. Call create_profile to adopt it.","inputSchema":obj(json!({"vpi_stations":arr(num.clone())}),&["vpi_stations"])},
      {"name":"create_profile","description":"Design vertical profile from VPIs [station, elevation] and parabolic curve lengths.","inputSchema":obj(json!({"name":s,"vpis":xy,"curve_lengths":arr(num.clone())}),&["name","vpis","curve_lengths"])},
      {"name":"design_superelevation","description":"Superelevation for each curve (AASHTO Method 1 or JTG i = V²/(127R) − μ, capped at e_max) with transition stations; rotation about the centreline.","inputSchema":obj(json!({}),&[])},
      {"name":"check_standards","description":"Run all alignment/profile/superelevation checks (and pipe checks if a network exists). Each result cites the rule and shows the formula.","inputSchema":obj(json!({}),&[])},
      {"name":"create_assembly","description":"Define the typical section: lanes, shoulder or curb/gutter/sidewalk, ditch (cut only), cut/fill slopes with optional benches, pavement layers, retaining-wall rule, and right-side overrides for asymmetric sections.","inputSchema":obj(json!({"name":s,"lanes_m":arr(num.clone()),"shoulder_m":num,"shoulder_slope":num,"shoulder_paved":b,"ditch":{"type":"object","properties":{"foreslope_h":num,"depth_m":num,"bottom_width_m":num}},"cut_slope_h":num,"fill_slope_h":num,"cut_bench":{"type":"object","properties":{"every_m":num,"width_m":num}},"fill_bench":{"type":"object","properties":{"every_m":num,"width_m":num}},"pavement":arr(json!({"type":"object","properties":{"name":s,"thickness_m":num}})),"subgrade_slope_h":num,"curb":{"type":"object","description":"barrier curb + gutter (+ sidewalk): gutter_m, gutter_slope, height_m, top_m, sidewalk_m, sidewalk_slope"},"wall":{"type":"object","description":"retaining-wall rule: kind cantilever|gravity, max_daylight_offset_m (ROW), max_slope_height_m, offset_from_hinge_m, embedment_m, soil {gamma_kn_m3, phi_deg, base_friction, bearing_kpa, surcharge_kpa}"},"right":{"type":"object","description":"right-side overrides for asymmetric sections (lanes_m, shoulder_m, shoulder_slope, shoulder_paved, curb, ditch, cut_slope_h, fill_slope_h, wall, remove:[\"curb\"|\"ditch\"|\"wall\"])"}}),&["name","lanes_m","shoulder_m","shoulder_slope","shoulder_paved","cut_slope_h","fill_slope_h"])},
      {"name":"build_corridor","description":"Apply the assembly at sample lines (regular interval plus geometry key stations), daylight to ground, compute end-area volumes and a composite-volume cross-check, build corridor surfaces.","inputSchema":obj(json!({"interval_m":num,"composite_cell_m":num,"widening":{"type":"object","description":"lane widening on curves: method JTG (jtg_class 1-3) or AASHTO (vehicle {name, track_u_m, wheelbases_m, front_overhang_m})"}}),&["interval_m"])},
      {"name":"get_section","description":"Cross section at a sample-line station (nearest).","inputSchema":obj(json!({"station":num}),&["station"])},
      {"name":"design_intersection","description":"At-grade T or 4-leg intersection on the designed main road: legs (each with its own assembly), curb returns tangent to both edges, leg profiles tied to the main-road finished grade, a paving patch with volumes, and stop-control sight triangles (AASHTO case B1) tested for line-of-sight obstruction. Checks return radius, approach grade and sight distance.","inputSchema":obj(json!({"name":s,"main_station":num,"major_speed_kmh":num,"minor_speed_kmh":num,"min_return_radius_m":num,"max_approach_grade_pct":num,"time_gap_s":num,"legs":arr(json!({"type":"object","properties":{"name":s,"side":{"type":"string","enum":["left","right"]},"angle_deg":num,"length_m":num,"grade_pct":num,"return_radii_m":arr(num.clone()),"assembly":{"type":"object"}},"required":["name","side","length_m","grade_pct","return_radii_m","assembly"]}))}),&["name","main_station","legs","major_speed_kmh","minor_speed_kmh","min_return_radius_m","max_approach_grade_pct"])},
      {"name":"grade","description":"Grade a pad (closed polygon) or feature line (open, side left/right) to existing ground at cut/fill slopes; returns daylight line and composite volumes.","inputSchema":obj(json!({"name":s,"vertices":xyz,"closed":b,"side":s,"cut_slope_h":num,"fill_slope_h":num,"spacing_m":num,"cell_m":num}),&["name","vertices","closed","cut_slope_h","fill_slope_h"])},
      {"name":"design_pipe_network","description":"Gravity storm/sanitary network: inverts from min cover/slope/drop, Manning full and partial flow, rational-method flows; checks per GB 50014-2021 or FHWA HEC-22. Rims default to the corridor surface, else existing ground.","inputSchema":obj(json!({"name":s,"criteria":{"type":"object","properties":{"standard":{"type":"string","enum":["GB50014_2021","FHWA_HEC22"]},"system":{"type":"string","enum":["storm","sanitary"]},"min_cover_m":num,"min_slope":num,"structure_drop_m":num,"intensity_mm_h":num,"tailwater_m":num,"freeboard_m":num},"required":["standard","system","min_cover_m","min_slope"]},"structures":arr(json!({"type":"object","properties":{"id":s,"x":num,"y":num,"rim":num,"area_ha":num,"c":num,"inflow_m3s":num,"population":num,"per_capita_lpd":num},"required":["id","x","y"]})),"pipes":arr(json!({"type":"object","properties":{"id":s,"from":s,"to":s,"diameter_m":num,"n":num,"material":s,"max_slope":num,"min_slope":num},"required":["id","from","to","diameter_m","n"]}))}),&["name","criteria","structures","pipes"])},
      {"name":"compute_quantities","description":"Mass haul ordinates/balance points and material table by station range.","inputSchema":obj(json!({"cut_factor":num,"free_haul_m":num,"table_step_m":num}),&[])},
      {"name":"generate_drawings","description":"Write model.dxf (world), sheets.pdf (all sheets, Chinese/English text, embedded font) and sheet-NN.dxf per sheet. PRELIMINARY stamp unless approved with zero mandatory failures.","inputSchema":obj(json!({}),&[])},
      {"name":"generate_review_report","description":"Write review.html listing every check (pass/fail with citation), quantities, output file hashes and the audit trail.","inputSchema":obj(json!({}),&[])},
      {"name":"record_review","description":"Record the engineer's decision. actor must be 'engineer:<name>'. approve is refused while mandatory checks fail. Any later design change voids the approval.","inputSchema":obj(json!({"actor":s,"decision":{"type":"string","enum":["approve","reject","comment"]},"note":s}),&["actor","decision","note"])},
      {"name":"verify_audit_log","description":"Recompute the hash chain of the audit log and report the first tampered entry, if any.","inputSchema":obj(json!({}),&[])},
      {"name":"get_state","description":"Summary of what is loaded/designed.","inputSchema":obj(json!({}),&[])}
    ])
}

fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}

const DESIGN_TOOLS: &[&str] = &["set_design_basis", "load_points", "import_landxml", "build_surface", "create_alignment", "create_profile", "design_superelevation", "create_assembly", "build_corridor", "design_intersection", "grade", "design_pipe_network"];

impl Session {
    fn dir(&self) -> crate::Result<PathBuf> {
        self.dir.clone().ok_or_else(|| "call open_project first".to_string())
    }
    fn log(&mut self, actor: &str, tool: &str, inputs: &Value, outputs: &Value, rules: &[String]) -> crate::Result<Value> {
        self.log.as_mut().ok_or("call open_project first")?.append(actor, tool, inputs, outputs, rules)
    }
    fn need_basis(&self) -> crate::Result<&DesignBasis> {
        self.basis.as_ref().ok_or_else(|| "no design basis: the engineer must call set_design_basis first".to_string())
    }
    fn write(&mut self, name: &str, bytes: &[u8]) -> crate::Result<String> {
        let p = self.dir()?.join(name);
        std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
        let h = sha256_hex(bytes);
        self.files.retain(|f| f.0 != name);
        self.files.push((name.into(), h.clone()));
        Ok(h)
    }
    pub fn all_checks(&self) -> Vec<Check> {
        let mut c = self.checks.clone();
        for n in &self.networks {
            c.extend(n.checks.iter().cloned());
        }
        for i in &self.intersections {
            c.extend(i.checks.iter().cloned());
        }
        for g in &self.gradings {
            c.push(standards::chk("G-DAYLIGHT", "Grading reaches daylight everywhere", format!("grading {}", g.name), g.warnings.len() as f64, 0.0, "points", "max", "mandatory", "footprint points without daylight".into(), "tool rule".into()));
        }
        if let Some(cor) = &self.corridor {
            let std_jtg = self.basis.as_ref().map_or(true, |b| b.standard == standards::Standard::JtgD20_2017);
            let cr = crate::walls::criteria(std_jtg);
            for left in [true, false] {
                let ws: Vec<(f64, &crate::corridor::WallPlaced)> = cor.sections.iter().filter_map(|s| { let sd = if left { &s.left } else { &s.right }; sd.wall.as_ref().map(|w| (s.station, w)) }).collect();
                if ws.is_empty() {
                    continue;
                }
                let side = if left { "left" } else { "right" };
                let worst = |f: &dyn Fn(&crate::walls::WallDesign) -> f64, max: bool| ws.iter().map(|(st, w)| (f(&w.design), *st)).fold((if max { f64::MIN } else { f64::MAX }, 0.0), |a, b| if (max && b.0 > a.0) || (!max && b.0 < a.0) { b } else { a });
                let (v, st) = worst(&|d| d.fs_sliding, false);
                c.push(standards::chk("W-SLIDE", "挡土墙抗滑稳定 Wall sliding FS", format!("{side} wall (worst at {st:.1})"), v, cr.fs_sliding, "", "min", "mandatory", "Ks = μ·ΣV / Ea, Rankine Ka = tan²(45°−φ/2), Ea = ½γH²Ka + qKaH".into(), cr.citation.into()));
                let (v, st) = worst(&|d| d.fs_overturning, false);
                c.push(standards::chk("W-OVERTURN", "挡土墙抗倾覆稳定 Wall overturning FS", format!("{side} wall (worst at {st:.1})"), v, cr.fs_overturning, "", "min", "mandatory", "Kt = ΣM_resist / ΣM_overturn about the toe".into(), cr.citation.into()));
                let (v, st) = worst(&|d| d.eccentricity / d.base_b, true);
                c.push(standards::chk("W-ECC", "基底偏心距 Base eccentricity e/B", format!("{side} wall (worst at {st:.1})"), v, cr.e_frac, "", "max", "mandatory", "e = B/2 − (ΣMr − ΣMo)/ΣV".into(), cr.citation.into()));
                let (v, st) = worst(&|d| d.q_max_kpa, true);
                let bearing = self.assembly.as_ref().and_then(|a| if left { a.wall.as_ref() } else { a.right.as_ref().and_then(|r| r.wall.as_ref()).or(a.wall.as_ref()) }).map_or(f64::NAN, |w| w.soil.bearing_kpa);
                c.push(standards::chk("W-BEAR", "基底最大压力 Max bearing pressure", format!("{side} wall (worst at {st:.1})"), v, cr.qmax_factor * bearing, "kPa", "max", "mandatory", "p_max = ΣV/B·(1 + 6e/B)".into(), cr.citation.into()));
            }
            c.push(standards::chk("C-DAYLIGHT", "Corridor sections reach daylight", "corridor".into(), cor.sections_with_warnings as f64, 0.0, "sections", "max", "mandatory", "sections whose slopes did not meet ground within data/150 m".into(), "tool rule".into()));
        }
        c
    }
    fn mandatory_failures(&self) -> usize {
        self.all_checks().iter().filter(|c| !c.pass && c.severity == "mandatory").count()
    }
    fn invalidate_from_alignment(&mut self) {
        self.profile = None;
        self.superelevation = None;
        self.invalidate_corridor();
    }
    fn invalidate_corridor(&mut self) {
        self.corridor = None;
        self.corridor_fg = None;
        self.corridor_datum = None;
        self.composite = None;
        self.intersections.clear();
        self.mass_haul = None;
        self.materials.clear();
    }

    pub fn call(&mut self, name: &str, a: &Value) -> crate::Result<Value> {
        let r = self.call_inner(name, a);
        if r.is_ok() && DESIGN_TOOLS.contains(&name) {
            self.approved_seq = None;
        }
        r
    }

    fn call_inner(&mut self, name: &str, a: &Value) -> crate::Result<Value> {
        match name {
            "open_project" => {
                let d = PathBuf::from(a["dir"].as_str().ok_or("dir required")?);
                std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                let log = AuditLog::open(&d.join("audit.jsonl"))?;
                *self = Session::default();
                let r = json!({"dir": d, "existing_entries": log.seq, "head": log.head});
                self.dir = Some(d);
                self.log = Some(log);
                Ok(r)
            }
            "set_design_basis" => {
                let actor = a["actor"].as_str().unwrap_or("");
                if !actor.starts_with("engineer:") {
                    return Err("design basis must be set by the engineer (actor 'engineer:<name>')".into());
                }
                let b: DesignBasis = serde_json::from_value(a["basis"].clone()).map_err(|e| e.to_string())?;
                standards::validate_basis(&b)?;
                let out = json!({"ssd_m": standards::ssd(&b), "jtg_row": standards::jtg_row(b.design_speed_kmh).map(|r| format!("{r:?}"))});
                let prov = self.log(actor, name, a, &out, &[format!("{:?} design basis", b.standard)])?;
                self.basis = Some(b);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "load_points" => {
                let p = a["path"].as_str().ok_or("path required")?;
                let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
                let (pts, rej) = if a["format"] == "landxml" { (points::parse_landxml_cgpoints(&text)?, vec![]) } else { points::parse_csv(&text)? };
                let out = json!({"points": pts.len(), "rejected": rej, "file_sha256": sha256_hex(text.as_bytes())});
                let prov = self.log("agent", name, a, &out, &[])?;
                self.points = pts;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "import_landxml" => {
                let p = a["path"].as_str().ok_or("path required")?;
                let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
                let x = landxml::read(&text)?;
                let mut surfs = Vec::new();
                for s in &x.surfaces {
                    let r = Tin::from_faces(&s.points, &s.faces);
                    surfs.push(match r {
                        Ok((_, st)) => json!({"name": s.name, "points": s.points.len(), "faces": s.faces.len(), "invisible_faces": s.invisible_faces,
                            "area2d": st.area_m2, "area3d": st.area3d_m2, "elev_min": st.min_z, "elev_max": st.max_z,
                            "file_area2d": s.reported_area2d, "file_area3d": s.reported_area3d, "file_elev_min": s.reported_elev_min, "file_elev_max": s.reported_elev_max,
                            "area2d_diff_pct": s.reported_area2d.map(|v| (st.area_m2 - v) / v * 100.0), "area3d_diff_pct": s.reported_area3d.map(|v| (st.area3d_m2 - v) / v * 100.0)}),
                        Err(e) => json!({"name": s.name, "error": e}),
                    });
                }
                let mut als = Vec::new();
                for al in &x.alignments {
                    let built = landxml::to_alignment(al);
                    als.push(json!({"name": al.name, "elements": al.elements.len(), "length": built.as_ref().map(|b| b.length).ok(), "file_length": al.reported_length,
                        "length_diff_m": built.as_ref().ok().and_then(|b| al.reported_length.map(|r| b.length - r)), "max_element_end_deviation_m": al.max_end_deviation,
                        "closure_error_m": built.as_ref().map(|b| b.closure_error).ok(), "profiles": al.profiles.iter().map(|p| &p.name).collect::<Vec<_>>(), "warnings": al.warnings}));
                }
                let out = json!({"application": x.application, "cg_points": x.points.len(), "surfaces": surfs, "alignments": als});
                if a["use_surface"].as_bool().unwrap_or(true) {
                    let want = a["surface"].as_str();
                    if let Some(s) = x.surfaces.iter().find(|s| want.map_or(true, |w| w == s.name)) {
                        self.tin = Some(Tin::from_faces(&s.points, &s.faces)?);
                        self.invalidate_corridor();
                    }
                }
                if a["use_alignment"].as_bool().unwrap_or(true) {
                    let want = a["alignment"].as_str();
                    if let Some(al) = x.alignments.iter().find(|s| want.map_or(true, |w| w == s.name)) {
                        if !al.elements.is_empty() {
                            self.alignment = Some(landxml::to_alignment(al)?);
                            self.invalidate_from_alignment();
                            if let Some(pi) = al.profiles.first() {
                                self.profile = Some(Profile::build(pi.clone())?);
                            }
                        }
                    }
                }
                if !x.points.is_empty() && self.points.is_empty() {
                    self.points = x.points.clone();
                }
                let prov = self.log("agent", name, a, &out, &["LandXML 1.x import".into()])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "export_landxml" => {
                let file = a["file"].as_str().unwrap_or("design.xml").to_string();
                let mut surfaces = Vec::new();
                if let Some((t, _)) = &self.tin {
                    surfaces.push(("EG", t));
                }
                if let Some(t) = &self.corridor_fg {
                    surfaces.push(("Corridor FG", t));
                }
                let xml = landxml::write(&landxml::Export { points: &self.points, surfaces, alignment: self.alignment.as_ref(), profiles: self.profile.iter().collect(), networks: self.networks.iter().collect() });
                let h = self.write(&file, xml.as_bytes())?;
                let out = json!({"file": file, "sha256": h, "bytes": xml.len()});
                let prov = self.log("agent", name, a, &out, &["LandXML 1.2 schema".into()])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "build_surface" => {
                if self.points.is_empty() {
                    return Err("load_points first".into());
                }
                let p: Vec<[f64; 3]> = self.points.iter().map(|p| [p.e, p.n, p.z]).collect();
                let (t, s) = Tin::build(&p, f(a, "max_edge_m"))?;
                let out = serde_json::to_value(&s).unwrap();
                let prov = self.log("agent", name, a, &out, &["Delaunay triangulation, linear facet interpolation".into()])?;
                self.tin = Some((t, s));
                self.invalidate_corridor();
                Ok(json!({"result": out, "provenance": prov}))
            }
            "create_alignment" => {
                self.need_basis()?;
                let inp: AlignmentInput = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let al = Alignment::build(inp)?;
                let out = json!({"length_m": al.length, "start_station": al.start_station(), "end_station": al.end_station(), "closure_error_m": al.closure_error, "curves": al.curves()});
                let prov = self.log("agent", name, a, &out, &["tangent–clothoid–arc geometry: Ts = (R+p)·tan(Δ/2)+k, θs = Ls/(2R)".into()])?;
                self.alignment = Some(al);
                self.invalidate_from_alignment();
                Ok(json!({"result": out, "provenance": prov}))
            }
            "sample_ground_profile" => {
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let step = f(a, "interval_m").unwrap_or(10.0);
                let mut v = Vec::new();
                let mut s = al.start_station();
                while s <= al.end_station() + 1e-9 {
                    let (x, y, _) = al.point_at(s.min(al.end_station())).unwrap();
                    v.push(json!([s, t.elevation_at(x, y)]));
                    s += step;
                }
                Ok(json!({"result": {"samples": v}}))
            }
            "suggest_profile" => {
                let b = self.need_basis()?.clone();
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let st: Vec<f64> = a["vpi_stations"].as_array().ok_or("vpi_stations required")?.iter().filter_map(|x| x.as_f64()).collect();
                if st.len() < 2 {
                    return Err("need >= 2 stations".into());
                }
                let gmax = match b.standard {
                    Standard::Aashto2018 => b.max_grade_pct.unwrap_or(0.0),
                    Standard::JtgD20_2017 => standards::jtg_row(b.design_speed_kmh).ok_or("unsupported speed")?.max_grade,
                };
                let mut v = Vec::new();
                for &s in &st {
                    let (x, y, _) = al.point_at(s).ok_or(format!("station {s} off alignment"))?;
                    v.push([s, t.elevation_at(x, y).ok_or(format!("station {s} outside surface"))?]);
                }
                let mut clamped = vec![];
                for i in 1..v.len() {
                    let g = (v[i][1] - v[i - 1][1]) / (v[i][0] - v[i - 1][0]) * 100.0;
                    if g.abs() > gmax {
                        v[i][1] = v[i - 1][1] + g.signum() * gmax / 100.0 * (v[i][0] - v[i - 1][0]);
                        clamped.push(i);
                    }
                }
                let s = standards::ssd(&b);
                let mut lens = vec![];
                for i in 1..v.len() - 1 {
                    let g1 = (v[i][1] - v[i - 1][1]) / (v[i][0] - v[i - 1][0]) * 100.0;
                    let g2 = (v[i + 1][1] - v[i][1]) / (v[i + 1][0] - v[i][0]) * 100.0;
                    let aa = g2 - g1;
                    let need = match b.standard {
                        Standard::Aashto2018 => if aa < 0.0 { standards::crest_len_min(aa, s) } else { standards::sag_len_min(aa, s) },
                        Standard::JtgD20_2017 => {
                            let r = standards::jtg_row(b.design_speed_kmh).unwrap();
                            let rv = if aa < 0.0 { r.crest_rv } else { r.sag_rv };
                            let ssd_l = if aa < 0.0 { standards::crest_len_min_h(aa, r.ssd, 1.2, 0.1) } else { 0.0 };
                            (rv * aa.abs() / 100.0).max(r.min_vc_len).max(ssd_l)
                        }
                    };
                    lens.push(((need / 5.0).ceil() * 5.0).max(0.0));
                }
                let proposal = json!({"name":"suggested","vpis":v,"curve_lengths":lens});
                let fits = Profile::build(serde_json::from_value(proposal.clone()).unwrap());
                let out = json!({"proposal": proposal, "grades_clamped_at_vpi": clamped, "max_grade_pct": gmax, "ssd_m": s, "fits": fits.is_ok(), "fit_error": fits.err()});
                let prov = self.log("agent", name, a, &out, &[format!("{:?}: max grade and minimum vertical curve length", b.standard)])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "create_profile" => {
                self.need_basis()?;
                self.alignment.as_ref().ok_or("create_alignment first")?;
                let inp: ProfileInput = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let p = Profile::build(inp)?;
                let out = json!({"grades_pct": p.grades_pct, "curves": p.curves});
                let prov = self.log("agent", name, a, &out, &["symmetric parabolic vertical curves, K = L/|A|".into()])?;
                self.profile = Some(p);
                self.invalidate_corridor();
                Ok(json!({"result": out, "provenance": prov}))
            }
            "design_superelevation" => {
                let b = self.need_basis()?.clone();
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let se = superelev::design(&b, al)?;
                let out = serde_json::to_value(&se).unwrap();
                let h = self.write("superelevation.json", out.to_string().as_bytes())?;
                let prov = self.log("agent", name, a, &json!({"curves": se.curves.len(), "detail_sha256": h, "e": se.curves.iter().map(|c| c.e).collect::<Vec<_>>()}), &[format!("{:?} superelevation", b.standard)])?;
                self.superelevation = Some(se);
                self.invalidate_corridor();
                Ok(json!({"result": out, "provenance": prov}))
            }
            "check_standards" => {
                let b = self.need_basis()?.clone();
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                self.checks = standards::run_checks(&b, al, self.profile.as_ref());
                let c = self.all_checks();
                let fails: Vec<&Check> = c.iter().filter(|c| !c.pass && c.severity == "mandatory").collect();
                let out = json!({"total": c.len(), "mandatory_failures": fails.len(), "advisory": c.iter().filter(|c| !c.pass && c.severity != "mandatory").count(), "checks": c});
                let rules: Vec<String> = c.iter().map(|c| c.citation.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let summary = json!({"total": c.len(), "mandatory_failures": fails.len(), "failed": fails.iter().map(|c| format!("{} {} {:.3}{} vs {} {:.3}", c.id, c.element, c.value, c.unit, c.kind, c.limit)).collect::<Vec<_>>()});
                let prov = self.log("agent", name, &json!({}), &summary, &rules)?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "create_assembly" => {
                let asm: Assembly = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                asm.validate()?;
                let out = json!({"lane_width_each_side": asm.lane_width(), "paved_width_each_side": asm.paved_width(), "pavement_thickness": asm.pavement_thickness()});
                let prov = self.log("agent", name, a, &out, &[])?;
                self.assembly = Some(asm);
                self.invalidate_corridor();
                Ok(json!({"result": out, "provenance": prov}))
            }
            "build_corridor" => {
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let asm = self.assembly.as_ref().ok_or("create_assembly first")?;
                let b = self.need_basis()?;
                let std_jtg = b.standard == standards::Standard::JtgD20_2017;
                let ws: Option<corridor::WideningSpec> = match a.get("widening") {
                    Some(v) if !v.is_null() => Some(serde_json::from_value(v.clone()).map_err(|e| e.to_string())?),
                    _ => None,
                };
                let c = corridor::build_opts(t, al, pr, self.superelevation.as_ref(), asm, f(a, "interval_m").unwrap_or(10.0), ws.as_ref(), b.design_speed_kmh, std_jtg)?;
                let (fg, _) = corridor::surface(&c, false)?;
                let (dt, _) = corridor::surface(&c, true)?;
                let cell = f(a, "composite_cell_m").unwrap_or(0.5);
                let (ccut, cfill, cells) = corridor::composite_volume(&dt, t, cell);
                let full = json!({"corridor": c, "superelevation": self.superelevation});
                let h = self.write("corridor.json", full.to_string().as_bytes())?;
                let out = json!({"sections": c.sections.len(), "sample_stations": c.sample_stations.len(), "cut_m3": c.cut_m3, "fill_m3": c.fill_m3, "layers_m3": c.layer_m3, "method": c.method,
                    "composite_check": {"cut_m3": ccut, "fill_m3": cfill, "cell_m": cell, "cells": cells, "cut_diff_pct": (c.cut_m3 - ccut) / ccut.max(1e-9) * 100.0, "fill_diff_pct": (c.fill_m3 - cfill) / cfill.max(1e-9) * 100.0},
                    "sections_with_warnings": c.sections_with_warnings, "widening": c.widening, "walls": {"concrete_m3": c.wall_concrete_m3, "length_left_m": c.wall_length_left_m, "length_right_m": c.wall_length_right_m, "sections_with_wall": c.sections.iter().filter(|s| s.left.wall.is_some() || s.right.wall.is_some()).count()}, "detail_file": "corridor.json", "detail_sha256": h});
                let mut rules = vec!["average end area: V = (A1+A2)/2·L; composite: Σ(Δz·cell²)".to_string()];
                rules.extend(c.widening.iter().map(|w| w.citation.clone()).collect::<std::collections::BTreeSet<_>>());
                if c.wall_concrete_m3 > 0.0 {
                    rules.push(crate::walls::criteria(std_jtg).citation.to_string());
                }
                let prov = self.log("agent", name, a, &out, &rules)?;
                self.composite = Some((ccut, cfill, cells, cell));
                self.intersections.clear();
                self.corridor = Some(c);
                self.corridor_fg = Some(fg);
                self.corridor_datum = Some(dt);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "get_section" => {
                let c = self.corridor.as_ref().ok_or("build_corridor first")?;
                let s = f(a, "station").ok_or("station required")?;
                let sec = c.sections.iter().min_by(|x, y| (x.station - s).abs().partial_cmp(&(y.station - s).abs()).unwrap()).unwrap();
                Ok(json!({"result": sec}))
            }
            "design_intersection" => {
                let (eg, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let c = self.corridor.as_ref().ok_or("build_corridor first")?;
                let fg = self.corridor_fg.as_ref().ok_or("build_corridor first")?;
                let inp: crate::intersect::IntersectionInput = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let x = crate::intersect::design(&inp, al, pr, c, fg, eg)?;
                let fname = format!("intersection-{}.json", inp.name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>());
                let h = self.write(&fname, serde_json::to_string(&x).unwrap().as_bytes())?;
                let fails: Vec<&Check> = x.checks.iter().filter(|c| !c.pass && c.severity == "mandatory").collect();
                let out = json!({"legs": x.legs.iter().map(|l| json!({"name": l.name, "returns": l.returns.iter().map(|r| r.radius).collect::<Vec<_>>(), "tie_in_elev": l.tie_in_elev, "leg_cut_m3": l.corridor.cut_m3, "leg_fill_m3": l.corridor.fill_m3, "patch_cut_m3": l.patch_cut_m3, "patch_fill_m3": l.patch_fill_m3, "sight": l.sight.iter().map(|s| json!({"direction": s.direction, "isd_m": s.along_major_m, "max_obstruction_m": s.max_obstruction_m, "clear": s.clear})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
                    "checks": x.checks.len(), "mandatory_failures": fails.len(), "failures": fails, "file": fname, "detail_sha256": h});
                let rules: Vec<String> = x.checks.iter().map(|c| c.citation.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let prov = self.log("agent", name, a, &json!({"legs": x.legs.len(), "mandatory_failures": fails.len()}), &rules)?;
                self.intersections.retain(|i| i.name != x.name);
                self.intersections.push(x);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "grade" => {
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let gi: GradingInput = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let g = grading::grade(t, &gi, f(a, "cell_m").unwrap_or(0.5))?;
                let out = json!({"name": g.name, "cut_m3": g.cut_m3, "fill_m3": g.fill_m3, "cells": g.cells, "cell_m": g.cell_m, "daylight_points": g.daylight.len(), "warnings": g.warnings, "method": "composite grid volume between grading surface and existing ground"});
                let h = self.write(&format!("grading-{}.json", g.name), serde_json::to_string(&g).unwrap().as_bytes())?;
                let prov = self.log("agent", name, a, &json!({"summary": out, "detail_sha256": h}), &["slope projection at H:1V to daylight".into()])?;
                self.gradings.retain(|x| x.name != g.name);
                self.gradings.push(g);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "design_pipe_network" => {
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let ni: NetworkInput = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let n = match &self.corridor_fg {
                    Some(fg) => pipes::design(&ni, fg, Some(t))?,
                    None => pipes::design(&ni, t, None)?,
                };
                let fname = format!("network-{}.json", ni.criteria.system);
                let h = self.write(&fname, serde_json::to_string(&n).unwrap().as_bytes())?;
                let fails = n.checks.iter().filter(|c| !c.pass && c.severity == "mandatory").count();
                let out = json!({"pipes": n.pipes, "structures": n.structures, "hydraulics": n.hydraulics, "checks": n.checks.len(), "mandatory_failures": fails, "file": fname, "detail_sha256": h});
                let rules: Vec<String> = n.checks.iter().map(|c| c.citation.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let prov = self.log("agent", name, a, &json!({"pipes": n.pipes.len(), "mandatory_failures": fails}), &rules)?;
                self.networks.retain(|x| x.name != n.name);
                self.networks.push(n);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "compute_quantities" => {
                let c = self.corridor.as_ref().ok_or("build_corridor first")?;
                let cf = f(a, "cut_factor").unwrap_or(1.0);
                let mh = quantities::mass_haul(c, cf, f(a, "free_haul_m").unwrap_or(0.0));
                let rows = quantities::material_table(c, f(a, "table_step_m").unwrap_or(100.0));
                let full = json!({"mass_haul": mh, "material_table": rows});
                let h = self.write("quantities.json", full.to_string().as_bytes())?;
                let out = json!({"balance_stations": mh.balance_stations, "final_ordinate_m3": mh.final_ordinate, "haul_moment_m3m": mh.haul_moment_m3m, "rows": rows.len(), "detail_sha256": h});
                let prov = self.log("agent", name, a, &out, &["mass ordinate M = Σ(cut·factor − fill)".into()])?;
                self.mass_haul = Some(mh);
                self.materials = rows;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "generate_drawings" => {
                let b = self.need_basis()?.clone();
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let head = self.log.as_ref().map(|l| l.head.clone()).unwrap_or_default();
                let all = self.all_checks();
                let info = SheetInfo { project: b.project.clone(), engineer: b.engineer_of_record.clone(), standard: format!("{:?}", b.standard), design_speed: b.design_speed_kmh, audit_head: head.clone(), checks_failed: all.iter().filter(|c| !c.pass && c.severity == "mandatory").count(), checks_total: all.len(), approved_seq: self.approved_seq };
                let ds = Design { tin: t, al, pr, corridor: self.corridor.as_ref(), networks: &self.networks, gradings: &self.gradings, intersections: &self.intersections };
                let nxs = self.corridor.as_ref().map_or(0, sheet::n_xs_pages);
                let has_q = self.mass_haul.is_some();
                let npp = self.networks.len();
                let pages = 1 + nxs + npp + has_q as usize;
                let mut sheets = vec![sheet::plan_profile(&ds, &info, 1, pages)];
                if let Some(c) = &self.corridor {
                    sheets.extend(sheet::cross_sections(c, &info, 2, pages));
                }
                for (i, n) in self.networks.iter().enumerate() {
                    sheets.push(sheet::pipe_profile(n, &info, 2 + nxs + i, pages));
                }
                if let Some(c) = &self.corridor {
                    if let Some(mh) = &self.mass_haul {
                        sheets.push(sheet::mass_haul_sheet(mh, &self.materials, c, &self.networks, &self.gradings, &info, pages, pages));
                    }
                }
                let model = sheet::model(&ds, 1.0).to_dxf();
                let pdf = drawing::to_pdf(&sheets.iter().map(|d| (d, sheet::SHEET_W, sheet::SHEET_H)).collect::<Vec<_>>());
                let dxfs: Vec<String> = sheets.iter().map(|d| d.to_dxf()).collect();
                let mut files = json!({});
                files["model.dxf"] = json!(self.write("model.dxf", model.as_bytes())?);
                files["sheets.pdf"] = json!(self.write("sheets.pdf", &pdf)?);
                for (i, d) in dxfs.iter().enumerate() {
                    let n = format!("sheet-{:02}.dxf", i + 1);
                    files[&n] = json!(self.write(&n, d.as_bytes())?);
                }
                let status = if info.approved_seq.is_some() && info.checks_failed == 0 { "APPROVED IN AUDIT LOG (seal by engineer)" } else { "PRELIMINARY - NOT FOR CONSTRUCTION" };
                let out = json!({"files": files, "pages": pages, "status": status, "audit_head_on_sheet": head});
                let prov = self.log("agent", name, a, &out, &[])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "generate_review_report" => {
                let b = serde_json::to_value(self.need_basis()?).unwrap();
                let p = self.dir()?.join("audit.jsonl");
                let ver = match audit::verify(&p) {
                    Ok((n, h)) => format!("OK — {n} entries, head {h}"),
                    Err(e) => format!("BROKEN — {e}"),
                };
                let entries: Vec<Value> = std::fs::read_to_string(&p).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
                let mut blocks = Vec::new();
                if let Some(se) = &self.superelevation {
                    let mut h = String::from("<table><tr><th>Curve</th><th>R m</th><th>e</th><th>method</th><th>transition start–full</th><th>full–end</th><th>rel. gradient %</th><th>note</th></tr>");
                    for c in &se.curves {
                        h += &format!("<tr><td>{}</td><td>{:.1}</td><td>{:.4}</td><td>{}</td><td>{:.2}–{:.2}</td><td>{:.2}–{:.2}</td><td>{:.3}</td><td>{}</td></tr>", c.curve, c.radius, c.e, c.method, c.a_in, c.b_in, c.b_out, c.a_out, c.relative_gradient * 100.0, c.note);
                    }
                    blocks.push(("Superelevation 超高".to_string(), h + "</table>"));
                }
                if let Some(c) = &self.corridor {
                    let mut h = format!("<p>{} sample lines. Method: {}. <b>Cut {:.1} m³, fill {:.1} m³</b>.", c.sections.len(), c.method, c.cut_m3, c.fill_m3);
                    if let Some((cc, cf, n, cell)) = self.composite {
                        h += &format!(" Composite cross-check ({cell} m grid, {n} cells): cut {cc:.1} m³ ({:+.2}%), fill {cf:.1} m³ ({:+.2}%).", (c.cut_m3 - cc) / cc.max(1e-9) * 100.0, (c.fill_m3 - cf) / cf.max(1e-9) * 100.0);
                    }
                    h += "</p><ul>";
                    for (n, v) in &c.layer_m3 {
                        h += &format!("<li>{n}: {v:.1} m³</li>");
                    }
                    blocks.push(("Corridor & earthwork 路基土方".to_string(), h + "</ul>"));
                }
                if let Some(mh) = &self.mass_haul {
                    blocks.push(("Mass haul 土方调配".to_string(), format!("<p>Cut factor {:.2}. Balance stations: {}. Final ordinate {:.0} m³ (positive = surplus). Haul moment ∫|M|ds = {:.0} m³·m.</p>", mh.cut_factor, mh.balance_stations.iter().map(|s| sheet::sta_label(*s)).collect::<Vec<_>>().join(", "), mh.final_ordinate, mh.haul_moment_m3m)));
                }
                for g in &self.gradings {
                    blocks.push((format!("Grading {}", g.name), format!("<p>Cut {:.1} m³, fill {:.1} m³ ({} m composite grid). Warnings: {}</p>", g.cut_m3, g.fill_m3, g.cell_m, g.warnings.len())));
                }
                for n in &self.networks {
                    let mut h = String::from("<table><tr><th>Pipe</th><th>From→To</th><th>L m</th><th>D m</th><th>S %</th><th>Inv up/dn</th><th>Q design m³/s</th><th>Q full</th><th>V full m/s</th><th>h/D</th><th>cover min m</th><th>HGL up/dn</th><th>regime</th></tr>");
                    for p in &n.pipes {
                        h += &format!("<tr><td>{}</td><td>{}→{}</td><td>{:.1}</td><td>{:.3}</td><td>{:.3}</td><td>{:.3}/{:.3}</td><td>{:.4}</td><td>{:.4}</td><td>{:.2}</td><td>{:.3}</td><td>{:.2}</td><td>{:.3}/{:.3}</td><td>{}</td></tr>", p.id, p.from, p.to, p.length, p.diameter_m, p.slope * 100.0, p.invert_up, p.invert_dn, p.q_design, p.q_full, p.v_full, p.depth_ratio, p.cover_up.min(p.cover_dn), p.hgl_up, p.hgl_dn, p.regime);
                    }
                    blocks.push((format!("Pipe network 管网: {}", n.name), h + "</table><p>" + &n.hgl_method + "</p>"));
                }
                let approved = self.approved_seq.is_some() && self.mandatory_failures() == 0;
                let html = crate::report::html(&b, &self.all_checks(), &blocks, &entries, &ver, &self.files.clone(), approved);
                let h = self.write("review.html", html.as_bytes())?;
                let out = json!({"file": "review.html", "sha256": h, "audit": ver});
                let prov = self.log("agent", name, a, &out, &[])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "record_review" => {
                let actor = a["actor"].as_str().unwrap_or("");
                if !actor.starts_with("engineer:") {
                    return Err("only an engineer (actor 'engineer:<name>') can record a review".into());
                }
                let d = a["decision"].as_str().unwrap_or("");
                if !["approve", "reject", "comment"].contains(&d) {
                    return Err("decision must be approve|reject|comment".into());
                }
                let fails = self.mandatory_failures();
                if d == "approve" && (self.checks.is_empty() || fails > 0) {
                    return Err(format!("cannot approve: {} mandatory checks failing (or checks not run)", fails));
                }
                let prov = self.log(actor, name, a, &json!({"mandatory_failures_at_review": fails}), &[])?;
                if d == "approve" {
                    self.approved_seq = prov["audit_seq"].as_u64();
                } else if d == "reject" {
                    self.approved_seq = None;
                }
                Ok(json!({"result": {"recorded": d}, "provenance": prov}))
            }
            "verify_audit_log" => {
                let p = self.dir()?.join("audit.jsonl");
                match audit::verify(&p) {
                    Ok((n, h)) => Ok(json!({"result": {"ok": true, "entries": n, "head": h}})),
                    Err(e) => Ok(json!({"result": {"ok": false, "error": e}})),
                }
            }
            "get_state" => Ok(json!({"result": {
                "dir": self.dir, "basis": self.basis, "points": self.points.len(),
                "surface": self.tin.as_ref().map(|t| &t.1), "alignment_length": self.alignment.as_ref().map(|a| a.length),
                "profile": self.profile.is_some(), "superelevation": self.superelevation.is_some(), "checks": self.all_checks().len(),
                "corridor": self.corridor.as_ref().map(|c| json!({"cut_m3": c.cut_m3, "fill_m3": c.fill_m3, "sections": c.sections.len()})),
                "gradings": self.gradings.iter().map(|g| &g.name).collect::<Vec<_>>(), "networks": self.networks.iter().map(|n| (&n.name, n.pipes.len())).collect::<Vec<_>>(),
                "approved_seq": self.approved_seq, "files": self.files}})),
            _ => Err(format!("unknown tool {name}")),
        }
    }
}
