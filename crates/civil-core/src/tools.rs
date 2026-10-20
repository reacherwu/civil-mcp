//! MCP tool surface. Every call that changes design state is written to the
//! audit log; every result carries a `provenance` record.
use crate::align::{Alignment, AlignmentInput};
use crate::audit::{self, sha256_hex, AuditLog};
use crate::earthwork::{self, Earthwork, Template};
use crate::points::{self, SurveyPoint};
use crate::profile::{Profile, ProfileInput};
use crate::sheet::{self, SheetInfo};
use crate::standards::{self, Check, DesignBasis, Standard};
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
    pub checks: Vec<Check>,
    pub earthwork: Option<Earthwork>,
    pub files: Vec<(String, String)>,
}

fn obj(props: Value, req: &[&str]) -> Value {
    json!({"type":"object","properties":props,"required":req})
}

pub fn tool_list() -> Value {
    let num = json!({"type":"number"});
    let s = json!({"type":"string"});
    let xy = json!({"type":"array","items":{"type":"array","items":{"type":"number"},"minItems":2,"maxItems":2}});
    json!([
      {"name":"open_project","description":"Open (or create) a project directory. The audit log lives at <dir>/audit.jsonl and is verified on open.","inputSchema":obj(json!({"dir":s}),&["dir"])},
      {"name":"set_design_basis","description":"Record the design basis chosen by the engineer in responsible charge (standard, design speed, limits). Must be set before design tools run. actor must be 'engineer:<name>'.","inputSchema":obj(json!({"actor":s,"basis":{"type":"object","properties":{"project":s,"engineer_of_record":s,"standard":{"type":"string","enum":["AASHTO_GREEN_BOOK_2018","JTG_D20_2017"]},"design_speed_kmh":num,"e_max":num,"f_max":num,"max_grade_pct":num,"perception_reaction_s":num,"deceleration_mps2":num},"required":["project","engineer_of_record","standard","design_speed_kmh"]}}),&["actor","basis"])},
      {"name":"load_points","description":"Load survey points from CSV (id,easting,northing,elevation,desc) or LandXML CgPoints. Rejected rows are reported.","inputSchema":obj(json!({"path":s,"format":{"type":"string","enum":["csv","landxml"]}}),&["path"])},
      {"name":"build_surface","description":"Build a Delaunay TIN from loaded points. Optional max_edge_m drops long hull triangles.","inputSchema":obj(json!({"max_edge_m":num}),&[])},
      {"name":"create_alignment","description":"Create a horizontal alignment from PIs and circular curve radii (tangent-curve-tangent). Fails if curves overlap.","inputSchema":obj(json!({"name":s,"pis":xy,"radii":{"type":"array","items":num},"start_station":num}),&["name","pis","radii"])},
      {"name":"sample_ground_profile","description":"Sample existing ground along the alignment.","inputSchema":obj(json!({"interval_m":num}),&[])},
      {"name":"suggest_profile","description":"Propose a design profile: VPIs on ground at the given stations, grades clamped to the basis maximum, vertical curve lengths set to the minimum required by the basis, rounded up to 5 m. Returns a proposal; call create_profile to adopt it.","inputSchema":obj(json!({"vpi_stations":{"type":"array","items":num}}),&["vpi_stations"])},
      {"name":"create_profile","description":"Create the design vertical profile from VPIs [station, elevation] and parabolic curve lengths.","inputSchema":obj(json!({"name":s,"vpis":xy,"curve_lengths":{"type":"array","items":num}}),&["name","vpis","curve_lengths"])},
      {"name":"check_standards","description":"Run all design-standard checks for the current alignment/profile. Each result cites the rule and shows the formula.","inputSchema":obj(json!({}),&[])},
      {"name":"compute_earthwork","description":"Cross sections and cut/fill volumes (average end area) for a simple crowned template with daylight side slopes.","inputSchema":obj(json!({"template":{"type":"object","properties":{"half_width_m":num,"cross_slope":num,"cut_slope_h":num,"fill_slope_h":num},"required":["half_width_m","cross_slope","cut_slope_h","fill_slope_h"]},"interval_m":num}),&["template","interval_m"])},
      {"name":"generate_drawings","description":"Write model.dxf (world coordinates), sheet.dxf and sheet.pdf (A1 plan/profile sheet, PRELIMINARY until approved).","inputSchema":obj(json!({}),&[])},
      {"name":"generate_review_report","description":"Write review.html listing every check (pass/fail with citation), earthwork, output file hashes and the audit trail.","inputSchema":obj(json!({}),&[])},
      {"name":"record_review","description":"Record the engineer's review decision. actor must be 'engineer:<name>'. approve is refused while mandatory checks fail.","inputSchema":obj(json!({"actor":s,"decision":{"type":"string","enum":["approve","reject","comment"]},"note":s}),&["actor","decision","note"])},
      {"name":"verify_audit_log","description":"Recompute the hash chain of the audit log and report the first tampered entry, if any.","inputSchema":obj(json!({}),&[])},
      {"name":"get_state","description":"Summary of what is loaded/designed.","inputSchema":obj(json!({}),&[])}
    ])
}

fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}

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

    pub fn call(&mut self, name: &str, a: &Value) -> crate::Result<Value> {
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
                let ssd = standards::ssd(&b);
                let out = json!({"ssd_m": ssd});
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
            "build_surface" => {
                if self.points.is_empty() {
                    return Err("load_points first".into());
                }
                let p: Vec<[f64; 3]> = self.points.iter().map(|p| [p.e, p.n, p.z]).collect();
                let (t, s) = Tin::build(&p, f(a, "max_edge_m"))?;
                let out = serde_json::to_value(&s).unwrap();
                let prov = self.log("agent", name, a, &out, &["Delaunay triangulation, linear facet interpolation".into()])?;
                self.tin = Some((t, s));
                Ok(json!({"result": out, "provenance": prov}))
            }
            "create_alignment" => {
                self.need_basis()?;
                let inp: AlignmentInput = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let al = Alignment::build(inp)?;
                let out = json!({"length_m": al.length, "start_station": al.start_station(), "end_station": al.end_station(),
                    "curves": al.curves().iter().map(|c| json!({"pi":c.0,"radius":c.1,"length":c.2,"deflection_deg":c.3})).collect::<Vec<_>>()});
                let prov = self.log("agent", name, a, &out, &["tangent-circular curve geometry: T = R·tan(Δ/2), L = R·Δ".into()])?;
                self.alignment = Some(al);
                self.profile = None;
                self.earthwork = None;
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
                    Standard::Aashto2018 => b.max_grade_pct.unwrap(),
                    Standard::JtgD20_2017 => standards::jtg_row(b.design_speed_kmh).ok_or("unsupported speed")?.2,
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
                            let r = standards::jtg_row(b.design_speed_kmh).ok_or("unsupported speed")?;
                            let rlim = if aa < 0.0 { r.4 } else { r.5 };
                            (rlim * aa.abs() / 100.0).max(r.6)
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
                self.earthwork = None;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "check_standards" => {
                let b = self.need_basis()?.clone();
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let c = standards::run_checks(&b, al, self.profile.as_ref());
                let fails: Vec<&Check> = c.iter().filter(|c| !c.pass && c.severity == "mandatory").collect();
                let out = json!({"total": c.len(), "mandatory_failures": fails.len(), "advisory": c.iter().filter(|c| !c.pass && c.severity != "mandatory").count(), "checks": c});
                let rules: Vec<String> = c.iter().map(|c| c.citation.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let prov = self.log("agent", name, &json!({}), &json!({"total": c.len(), "mandatory_failures": fails.len(), "failed": fails.iter().map(|c| format!("{} {} {:.3}{} vs {} {:.3}", c.id, c.element, c.value, c.unit, c.kind, c.limit)).collect::<Vec<_>>()}), &rules)?;
                self.checks = c;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "compute_earthwork" => {
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let tp: Template = serde_json::from_value(a["template"].clone()).map_err(|e| e.to_string())?;
                let e = earthwork::compute(t, al, pr, &tp, f(a, "interval_m").unwrap_or(10.0))?;
                let mut cl = vec![];
                let mut s = al.start_station();
                while s <= al.end_station() + 1e-9 {
                    let (x, y, h) = al.point_at(s).unwrap();
                    cl.push([s, x, y, h, pr.elevation_at(s).unwrap_or(f64::NAN)]);
                    s += 1.0;
                }
                let full = json!({"template": tp, "earthwork": e, "centerline_1m": cl});
                let h = self.write("earthwork.json", serde_json::to_string(&full).unwrap().as_bytes())?;
                let out = json!({"cut_m3": e.cut_m3, "fill_m3": e.fill_m3, "net_m3": e.net_m3, "sections": e.sections.len(), "sections_with_warnings": e.sections_with_warnings, "method": e.method, "detail_file": "earthwork.json", "detail_sha256": h});
                let prov = self.log("agent", name, a, &out, &["average end area: V = (A1+A2)/2 · L".into()])?;
                self.earthwork = Some(e);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "generate_drawings" => {
                let b = self.need_basis()?.clone();
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let head = self.log.as_ref().map(|l| l.head.clone()).unwrap_or_default();
                let fails = self.checks.iter().filter(|c| !c.pass && c.severity == "mandatory").count();
                let info = SheetInfo { project: &b.project, engineer: &b.engineer_of_record, standard: &format!("{:?}", b.standard), design_speed: b.design_speed_kmh, audit_head: &head, checks_failed: fails, checks_total: self.checks.len(), cut: self.earthwork.as_ref().map_or(0.0, |e| e.cut_m3), fill: self.earthwork.as_ref().map_or(0.0, |e| e.fill_m3), approved: false };
                let m = sheet::model(t, al, self.earthwork.as_ref(), 1.0).to_dxf();
                let sd = sheet::sheet(t, al, pr, self.earthwork.as_ref(), &info);
                let (dx, pdf) = (sd.to_dxf(), sd.to_pdf(sheet::SHEET_W, sheet::SHEET_H));
                let h1 = self.write("model.dxf", m.as_bytes())?;
                let h2 = self.write("sheet.dxf", dx.as_bytes())?;
                let h3 = self.write("sheet.pdf", &pdf)?;
                let out = json!({"files": {"model.dxf": h1, "sheet.dxf": h2, "sheet.pdf": h3}, "status": "PRELIMINARY - NOT FOR CONSTRUCTION", "audit_head_on_sheet": head});
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
                let ew = self.earthwork.as_ref().map(|e| json!({"method": e.method, "interval_m": e.interval_m, "cut_m3": e.cut_m3, "fill_m3": e.fill_m3, "net_m3": e.net_m3, "sections_with_warnings": e.sections_with_warnings}));
                let html = crate::report::html(&b, &self.checks, ew.as_ref(), &entries, &ver, &self.files.clone());
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
                let fails = self.checks.iter().filter(|c| !c.pass && c.severity == "mandatory").count();
                if d == "approve" && (self.checks.is_empty() || fails > 0) {
                    return Err(format!("cannot approve: {} mandatory checks failing (or checks not run)", fails));
                }
                let prov = self.log(actor, name, a, &json!({"mandatory_failures_at_review": fails}), &[])?;
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
                "profile": self.profile.is_some(), "checks": self.checks.len(),
                "earthwork": self.earthwork.as_ref().map(|e| json!({"cut_m3": e.cut_m3, "fill_m3": e.fill_m3})), "files": self.files}})),
            _ => Err(format!("unknown tool {name}")),
        }
    }
}
