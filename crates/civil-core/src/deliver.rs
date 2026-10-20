//! Design-stage deliverables (reports, registers, quantities) written from the live session.
use crate::standards::{self, Standard};
use crate::tools::Session;
use serde_json::Value;
use std::fmt::Write;

/// Deliverables matrix: (category, item, status, output). Status is implemented | partial | gap.
pub const MATRIX: &[(&str, &str, &str, &str)] = &[
    ("calc", "Route / corridor options and selection (options report)", "implemented", "route-alternatives.json, design-report.md §2"),
    ("calc", "Horizontal alignment geometry and standards checks", "implemented", "setting-out.csv, design-report.md §3, review.html"),
    ("calc", "Vertical alignment, stopping sight distance, vertical-curve K", "implemented", "design-report.md §3"),
    ("calc", "Superelevation and development lengths", "implemented", "design-report.md §3, superelevation diagram sheet"),
    ("calc", "Lane widening on curves", "implemented", "AASHTO off-tracking formula or engineer table"),
    ("calc", "Earthwork volumes, mass haul, free haul/overhaul, borrow/waste, shrink/bulk", "implemented", "quantities.json, design-report.md §5, mass-haul sheet"),
    ("calc", "Profile optimisation for earthwork balance under standards", "implemented", "profile-optimisation.json"),
    ("calc", "Pavement thickness design", "partial", "US/CA AASHTO 1993 flexible+rigid; AU/NZ AGPT02 empirical granular; UK CD 226 Eq 2.24/2.43 with Fig 2.20 values by engineer. Gaps: AASHTOWare ME, CIRCLY mechanistic, AU/NZ rigid"),
    ("calc", "Subgrade / foundation design (CD 225, subgrade improvement)", "partial", "design CBR/Mr inputs and subgrade sheet; foundation design itself is a gap"),
    ("calc", "Storm drainage network hydraulics with HGL/EGL", "implemented", "drainage-report.md, pipe profile sheets"),
    ("calc", "Hydrology (IFD/IDF, ARR 2019, FEH/ReFH, peak flow methods)", "gap", "design rainfall intensity is an engineer input"),
    ("calc", "Roadside ditch / channel lining (HEC-15 shear)", "implemented", "protection.json, drainage-report.md"),
    ("calc", "Culvert hydraulics (HDS-5) and structural design", "gap", ""),
    ("calc", "Sanitary sewer design (peaking, depth ratio, velocity)", "implemented", "drainage-report.md"),
    ("calc", "Retaining wall external stability", "partial", "US global factors; other profiles need engineer criteria; structural design of stem/reinforcement is a gap"),
    ("calc", "Barrier need and length of need, clear zone", "partial", "geometric hazard screening with engineer clear zone; UK RRRAP not implemented"),
    ("calc", "Intersection geometry, returns, sight triangles", "partial", "US Case B1 implemented; CD 123 / Austroads SISD via engineer distances not yet separate"),
    ("calc", "Slope stability / geotechnical design", "gap", ""),
    ("calc", "Subsurface (pavement) drainage", "gap", ""),
    ("calc", "Traffic capacity / LOS, signals, signing, markings, lighting", "gap", ""),
    ("calc", "Noise, air, environmental assessment", "gap", ""),
    ("calc", "Utilities conflicts, right-of-way / land requirements", "gap", ""),
    ("calc", "Bridges and major structures", "gap", ""),
    ("calc", "Temporary traffic management / staging", "gap", ""),
    ("report", "Design report / basis of design / design criteria", "implemented", "design-report.md, parameter-register.csv"),
    ("report", "Drainage report", "implemented", "drainage-report.md"),
    ("report", "Pavement design report", "implemented", "pavement-report.md"),
    ("report", "Geotechnical interface (inputs needed from the geotechnical report)", "partial", "design-report.md §12 lists inputs; the geotechnical report itself is a gap"),
    ("report", "Departures / design exceptions / relaxations register", "implemented", "departures-register.csv"),
    ("report", "Road safety audit hooks (brief for the independent auditor)", "implemented", "rsa-brief.md (the audit itself is done by people)"),
    ("report", "Quantities / bill of quantities by measurement method", "partial", "boq.csv (US FP-14 sections, UK SHW/MMHW series; CA/AU/NZ generic groups to map to agency items)"),
    ("report", "Cost estimate", "partial", "cost-estimate.csv: quantities with blank rates for the estimator; route cost proxy in relative units"),
    ("report", "Setting-out / alignment report", "implemented", "setting-out.csv"),
    ("report", "Checks / design review report with audit trail", "implemented", "review.html"),
    ("report", "Environmental, constructability, utilities reports", "gap", ""),
    ("drawing", "Title sheet, sheet index, general notes and legend", "implemented", "sheets.pdf / sheet-NN.dxf"),
    ("drawing", "Key plan / location plan", "implemented", "sheets.pdf"),
    ("drawing", "Route alternatives plan", "implemented", "sheets.pdf"),
    ("drawing", "Typical sections", "implemented", "sheets.pdf"),
    ("drawing", "Plan and profile", "implemented", "sheets.pdf"),
    ("drawing", "Superelevation diagram", "implemented", "sheets.pdf"),
    ("drawing", "Cross sections", "implemented", "sheets.pdf"),
    ("drawing", "Subgrade / earthwork and mass haul", "implemented", "sheets.pdf"),
    ("drawing", "Pavement design details", "implemented", "sheets.pdf"),
    ("drawing", "Storm and sanitary plan and profile", "implemented", "sheets.pdf"),
    ("drawing", "Pipe trench cross-sections, manhole and inlet details", "implemented", "sheets.pdf"),
    ("drawing", "Barrier / protection layout and ditch lining schedule", "implemented", "sheets.pdf"),
    ("drawing", "Quantity summary sheet", "implemented", "sheets.pdf"),
    ("drawing", "Standard details atlas (curb, gutter, barrier, trench bedding, manhole)", "implemented", "sheets.pdf (generic, dimensioned from inputs; replace with agency standard drawings)"),
    ("drawing", "Intersection layout", "partial", "shown on the plan sheet; no separate layout sheet"),
    ("drawing", "Signing, pavement marking, lighting, signals", "gap", ""),
    ("drawing", "Traffic management / staging plans", "gap", ""),
    ("drawing", "Utilities, right-of-way / land plans", "gap", ""),
    ("drawing", "Erosion and sediment control plan, landscaping", "partial", "slope protection areas only"),
    ("drawing", "Structures (bridges, culverts, wall structural)", "gap", ""),
];

fn csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn departures(s: &Session) -> Vec<[String; 7]> {
    let b = s.basis.as_ref();
    let uk = b.map_or(false, |b| b.standard == Standard::Uk);
    let mut v = Vec::new();
    for c in s.all_checks() {
        if c.pass {
            continue;
        }
        let kind = if c.severity == "mandatory" {
            if uk { "departure from standard (GG 101)" } else { "design exception / departure" }
        } else if ["H-RDES", "V-GDES", "V-LVC"].contains(&c.id.as_str()) && uk {
            "relaxation (CD 109)"
        } else {
            "advisory — engineer to note"
        };
        v.push([c.id.clone(), c.element.clone(), kind.into(), format!("{:.3} {} vs {} {:.3}", c.value, c.unit, c.kind, c.limit), c.citation.clone(), "open".into(), String::new()]);
    }
    for d in &s.departures {
        let g = |k: &str| d[k].as_str().unwrap_or("").to_string();
        v.push([g("check_id"), g("element"), g("kind"), String::new(), String::new(), format!("recorded by {}", g("recorded_by")), g("justification")]);
    }
    v
}

pub struct BoqRow {
    pub code: String,
    pub item: String,
    pub unit: String,
    pub qty: f64,
    pub basis: String,
}

pub fn boq(s: &Session) -> (Vec<BoqRow>, String) {
    let std = s.basis.as_ref().map_or(Standard::Us, |b| b.standard);
    let mut rows = Vec::new();
    let code = |us: &str, uk: &str, generic: &str| -> String {
        match std {
            Standard::Us => us.into(),
            Standard::Uk => uk.into(),
            _ => generic.into(),
        }
    };
    let mut add = |c: String, i: &str, u: &str, q: f64, b: &str| {
        if q > 0.0 {
            rows.push(BoqRow { code: c, item: i.into(), unit: u.into(), qty: q, basis: b.into() })
        }
    };
    if let Some(c) = &s.corridor {
        add(code("FP-14 204", "SHW Series 600", "Earthworks"), "Roadway excavation (bank volume)", "m³", c.cut_m3, "corridor end-area volumes");
        add(code("FP-14 204", "SHW Series 600", "Earthworks"), "Embankment construction (compacted volume)", "m³", c.fill_m3, "corridor end-area volumes");
        if let Some(e) = &s.earthwork {
            add(code("FP-14 204", "SHW Series 600", "Earthworks"), "Borrow (imported fill, compacted)", "m³", e.borrow_compacted_m3, "mass haul end ordinate");
            add(code("FP-14 204", "SHW Series 600", "Earthworks"), "Disposal of surplus material (bank)", "m³", e.waste_bank_m3, "mass haul end ordinate");
            add(code("FP-14 204", "SHW Series 600", "Earthworks"), "Overhaul", "m³·m", e.overhaul_m3m, "segment method beyond free haul");
        }
        for (n, v) in &c.layer_m3 {
            let l = n.to_lowercase();
            let (us, uk) = if l.contains("pcc") || l.contains("urc") || l.contains("concrete") { ("FP-14 Div 500", "SHW Series 1000") } else if l.contains("asphalt") || l.contains("surfac") || l.contains("binder") || l.contains("ac") { ("FP-14 401", "SHW Series 900") } else if l.contains("hbgm") { ("FP-14 Div 300", "SHW Series 800") } else { ("FP-14 301", "SHW Series 800") };
            add(code(us, uk, "Pavements"), &format!("Pavement layer: {n}"), "m³", *v, "corridor layer areas");
        }
        let curb_len: f64 = c.sections.windows(2).map(|w| { let l = w[1].station - w[0].station; let k = |sd: &crate::corridor::SideSection| sd.codes.iter().any(|x| x == "TOP-CURB") as u8 as f64; (k(&w[0].left) + k(&w[0].right) + k(&w[1].left) + k(&w[1].right)) / 2.0 * l }).sum();
        add(code("FP-14 609", "SHW Series 1100", "Kerbs and footpaths"), "Curb and gutter", "m", curb_len, "sections with curb");
        let walk: f64 = c.sections.windows(2).map(|w| { let l = w[1].station - w[0].station; let wa = |sd: &crate::corridor::SideSection| { let i = sd.codes.iter().position(|x| x == "BACK-CURB"); let j = sd.codes.iter().position(|x| x == "BACK-WALK"); match (i, j) { (Some(i), Some(j)) => sd.fg[j][0] - sd.fg[i][0], _ => 0.0 } }; (wa(&w[0].left) + wa(&w[0].right) + wa(&w[1].left) + wa(&w[1].right)) / 2.0 * l }).sum();
        add(code("FP-14 615", "SHW Series 1100", "Kerbs and footpaths"), "Sidewalk / footway", "m²", walk, "sections with sidewalk");
        add(code("FP-14 258", "SHW Series 600/1700 (structures)", "Retaining walls"), "Reinforced concrete retaining wall (concrete)", "m³", c.wall_concrete_m3, "wall proportioning per section");
    }
    for n in &s.networks {
        let mut by: std::collections::BTreeMap<String, f64> = Default::default();
        for p in &n.pipes {
            *by.entry(format!("{:.0} mm {}", p.diameter_m * 1000.0, p.material)).or_default() += p.length;
        }
        for (k, l) in by {
            add(code("FP-14 602", "SHW Series 500", "Drainage"), &format!("{} pipe {k}", n.name), "m", l, "network design");
        }
        add(code("FP-14 604", "SHW Series 500", "Drainage"), &format!("{} manholes / inlets", n.name), "each", n.structures.len() as f64, "network design");
    }
    if let Some(p) = &s.protection {
        for b in &p.barriers {
            add(code("FP-14 617", "SHW Series 400", "Road safety barriers"), &format!("{} ({} {:.0}–{:.0})", b.barrier_type, b.side, b.from, b.to), "m", b.length_m, "hazards in clear zone + length of need");
        }
        for d in &p.ditches {
            let l = d.to - d.from;
            if let Some(d50) = d.riprap_d50_m {
                add(code("FP-14 251", "SHW Series 500", "Drainage"), &format!("Rock riprap ditch lining D50 {:.0} mm ({} {:.0}–{:.0})", d50 * 1000.0, d.side, d.from, d.to), "m³", l * 2.5 * 2.0 * d50, "length × 2.5 m wetted perimeter × 2·D50 thickness");
            } else if !d.lining.starts_with("bare") {
                add(code("FP-14 629", "SHW Series 500", "Drainage"), &format!("Ditch lining: {} ({} {:.0}–{:.0})", d.lining, d.side, d.from, d.to), "m", l, "HEC-15 lining selection");
            }
        }
        for a in &p.slope_protection {
            add(code(if a.class.starts_with("topsoil") { "FP-14 625" } else { "FP-14 629" }, "SHW Series 3000 / 600", "Landscaping and erosion control"), &format!("Slope protection: {}", a.class), "m²", a.area_m2, "slope area by class");
        }
    }
    let method = match std {
        Standard::Us => "Items grouped by FHWA FP-14 Standard Specifications section (204 earthwork, 251 riprap, 258 RC walls, 301 aggregate, 401 asphalt, 602 culverts and drains, 604 manholes and inlets, 609 curb and gutter, 615 sidewalks, 617 guardrail, 625 turf, 629 RECP). State DOT item numbers must be mapped by the estimator.",
        Standard::Uk => "Items grouped by Specification for Highway Works series (MCHW Vol 1) as used by the Method of Measurement for Highway Works (MCHW Vol 4): 400 road restraint systems, 500 drainage, 600 earthworks, 800 unbound and hydraulically bound, 900 bituminous, 1000 concrete, 1100 kerbs and footways, 3000 landscape. Item coding to MMHW is for the quantity surveyor.",
        _ => "Generic work groups. Map to the agency's item codes (e.g. MTO OPSS, TfNSW / Main Roads specifications, NZTA specifications) before tender.",
    };
    (rows, method.into())
}

pub fn write_all(s: &Session, stage: &str) -> crate::Result<Vec<(String, Vec<u8>)>> {
    let b = s.basis.as_ref().ok_or("no design basis")?;
    let rs = standards::resolve(b)?;
    let mut out = Vec::new();
    // parameter register
    let mut p = String::from("parameter,value,unit,status,source\n");
    for x in &rs.params {
        let _ = writeln!(p, "{},{:.4},{},{},{}", csv(&x.name), x.value, csv(&x.unit), x.status, csv(&x.source));
    }
    if let Some(pd) = &s.pavement {
        for e in &pd.engineer_inputs {
            let _ = writeln!(p, "{},,,engineer,{}", csv(&format!("pavement: {e}")), csv(&pd.citation));
        }
    }
    if let Some(w) = &b.wall_criteria {
        let _ = writeln!(p, "wall criteria FS sliding/overturning,{}/{},,engineer,{}", w.fs_sliding, w.fs_overturning, csv(&w.citation));
    }
    out.push(("parameter-register.csv".to_string(), p.into_bytes()));
    // departures
    let dep = departures(s);
    let mut d = String::from("check_id,element,type,value_vs_limit,clause,status,justification\n");
    for r in &dep {
        let _ = writeln!(d, "{}", r.iter().map(|x| csv(x)).collect::<Vec<_>>().join(","));
    }
    out.push(("departures-register.csv".into(), d.into_bytes()));
    // BoQ + cost
    let (rows, method) = boq(s);
    let mut q = format!("# {method}\ncode,item,unit,quantity,basis\n");
    let mut cst = String::from("# Quantities from boq.csv. Rates are left blank for the estimator; this tool does not supply market prices.\ncode,item,unit,quantity,rate,amount\n");
    for r in &rows {
        let _ = writeln!(q, "{},{},{},{:.1},{}", csv(&r.code), csv(&r.item), r.unit, r.qty, csv(&r.basis));
        let _ = writeln!(cst, "{},{},{},{:.1},,", csv(&r.code), csv(&r.item), r.unit, r.qty);
    }
    out.push(("boq.csv".into(), q.into_bytes()));
    out.push(("cost-estimate.csv".into(), cst.into_bytes()));
    // setting-out
    if let (Some(al), Some(pr)) = (&s.alignment, &s.profile) {
        let mut so = String::from("# Curve table\ncurve,radius_m,spiral_in_m,spiral_out_m,delta_deg,TS_or_PC,SC,CS,ST_or_PT\n");
        for c in al.curves() {
            let _ = writeln!(so, "{},{:.3},{:.3},{:.3},{:.4},{:.3},{:.3},{:.3},{:.3}", c.index, c.radius, c.ls_in, c.ls_out, c.delta_deg, c.sta_start, c.sta_cs_start, c.sta_cs_end, c.sta_end);
        }
        let _ = writeln!(so, "# PI coordinates\npi,easting,northing");
        for (i, p) in al.pis.iter().enumerate() {
            let _ = writeln!(so, "{},{:.3},{:.3}", i, p[0], p[1]);
        }
        let _ = writeln!(so, "# Centreline every 20 m and at key stations\nstation,easting,northing,bearing_deg_from_north,fg_elev,cross_slope_left,cross_slope_right");
        let mut st: Vec<f64> = (0..).map(|i| al.start_station() + 20.0 * i as f64).take_while(|x| *x < al.end_station()).collect();
        st.extend(al.key_stations());
        st.push(al.end_station());
        st.sort_by(|a, b| a.partial_cmp(b).unwrap());
        st.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        for x in st {
            if let (Some((e, n, h)), Some(z)) = (al.point_at(x), pr.elevation_at(x)) {
                let (sl, sr) = s.superelevation.as_ref().map_or((-b.normal_crown, -b.normal_crown), |se| se.slopes_at(x));
                let brg = (90.0 - h.to_degrees()).rem_euclid(360.0);
                let _ = writeln!(so, "{x:.3},{e:.3},{n:.3},{brg:.4},{z:.3},{sl:.4},{sr:.4}");
            }
        }
        out.push(("setting-out.csv".into(), so.into_bytes()));
    }
    // matrix status
    let mut m = String::from("category,item,status,output_or_gap_note\n");
    for r in MATRIX {
        let _ = writeln!(m, "{},{},{},{}", r.0, csv(r.1), r.2, csv(r.3));
    }
    out.push(("deliverables-status.csv".into(), m.into_bytes()));
    out.push(("design-report.md".into(), design_report(s, &rs, stage, &dep, &rows, &method).into_bytes()));
    out.push(("drainage-report.md".into(), drainage_report(s).into_bytes()));
    if let Some(pd) = &s.pavement {
        let mut r = format!("# Pavement design report\n\nMethod: {}\n\nCitation: {}\n\nType: {}\n\n", pd.method, pd.citation, pd.kind);
        if let Some(t) = &s.traffic {
            let _ = writeln!(r, "## Design traffic\n\n{} = {:.4e} ({})\n\nInputs: `{}`\n", t["unit"].as_str().unwrap_or(""), t["design_traffic"].as_f64().unwrap_or(0.0), t["formula"].as_str().unwrap_or(""), t["inputs"]);
        }
        let _ = writeln!(r, "## Inputs\n\n```json\n{}\n```\n\n## Calculation steps\n", serde_json::to_string_pretty(&pd.inputs).unwrap());
        for x in &pd.steps {
            let _ = writeln!(r, "- {x}");
        }
        let _ = writeln!(r, "\n## Layers\n\n| Layer | Thickness (mm) | Note |\n|---|---:|---|");
        for l in &pd.layers {
            let _ = writeln!(r, "| {} | {:.0} | {} |", l.name, l.thickness_mm, l.note);
        }
        let _ = writeln!(r, "\nTotal: {:.0} mm. SN required {:?}, provided {:?}.\n\n## Engineer inputs\n", pd.total_mm, pd.sn_required.map(|v| (v * 100.0).round() / 100.0), pd.sn_provided.map(|v| (v * 100.0).round() / 100.0));
        for e in &pd.engineer_inputs {
            let _ = writeln!(r, "- {e}");
        }
        for w in &pd.warnings {
            let _ = writeln!(r, "- WARNING: {w}");
        }
        out.push(("pavement-report.md".into(), r.into_bytes()));
    }
    // RSA brief
    let mut a = format!("# Road safety audit brief ({stage} design)\n\nPrepared for the independent road safety audit team (US: FHWA Road Safety Audit Guidelines; UK: DMRB GG 119; AU/NZ: Austroads AGRS Part 6 / NZTA RSA procedures). The audit itself is carried out by people independent of the design team.\n\n## Design basis\n\n{} at {} km/h, standards profile {}.\n\n## Items for the auditor\n\n", b.project, b.design_speed_kmh, b.standard.label());
    if let Some(p) = &s.protection {
        let _ = writeln!(a, "- {} roadside hazards inside the {} m clear zone; {} barrier runs proposed (see barrier layout sheet).", p.hazards.len(), "engineer-entered", p.barriers.len());
        for n in p.notes.iter().take(20) {
            let _ = writeln!(a, "- {n}");
        }
    }
    for x in &s.intersections {
        let _ = writeln!(a, "- Intersection {} at {:.0}: {} checks, sight triangles evaluated against the corridor surface.", x.name, x.main_station, x.checks.len());
    }
    let _ = writeln!(a, "- Departures / relaxations open: {} (departures-register.csv).", dep.len());
    out.push(("rsa-brief.md".into(), a.into_bytes()));
    Ok(out)
}

fn design_report(s: &Session, rs: &standards::Resolved, stage: &str, dep: &[[String; 7]], rows: &[BoqRow], method: &str) -> String {
    let b = s.basis.as_ref().unwrap();
    let mut r = format!("# Design report ({stage} design)\n\nProject: {}\n\nEngineer in responsible charge: {}\n\nStatus: PRELIMINARY. Prepared by software agents; not reviewed, approved or sealed.\n\n## 1. Design basis\n\nStandards profile: {}. Design speed {} km/h.\n\n| Parameter | Value | Unit | Status | Source |\n|---|---:|---|---|---|\n", b.project, b.engineer_of_record, b.standard.label(), b.design_speed_kmh);
    for p in &rs.params {
        let _ = writeln!(r, "| {} | {:.3} | {} | {} | {} |", p.name, p.value, p.unit, p.status, p.source);
    }
    let _ = writeln!(r, "\nStatus key: verified = read from a public source and cited; formula = published formula shipped, inputs from the engineer; engineer = value entered by the engineer (copyrighted/paywalled or unverified table).\n\n## 2. Route options and selection\n");
    if let Some(rt) = &s.route {
        let _ = writeln!(r, "| Alternative | Length m | Min R m | Max grade % | Cut m³ | Fill m³ | Imbalance m³ | Wall concrete m³ | Mandatory failures | Cost proxy |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        for a in rt["alternatives"].as_array().unwrap() {
            let e = &a["evaluation"];
            let g = |k: &str| e[k].as_f64().unwrap_or(f64::NAN);
            let _ = writeln!(r, "| {} | {:.0} | {:.0} | {:.2} | {:.0} | {:.0} | {:.0} | {:.0} | {} | {:.0} |", a["name"].as_str().unwrap_or(""), g("length_m"), g("min_radius_m"), g("max_design_grade_pct"), g("cut_m3"), g("fill_m3"), g("imbalance_m3"), g("wall_concrete_m3"), e["geometry_mandatory_failures"], g("cost_proxy"));
        }
        let _ = writeln!(r, "\nRecommendation: **{}** ({}).\n", rt["recommendation"]["recommended"].as_str().unwrap_or(""), rt["recommendation"]["rule"].as_str().unwrap_or(""));
        for x in rt["recommendation"]["reasons"].as_array().unwrap() {
            let _ = writeln!(r, "- {}", x.as_str().unwrap_or(""));
        }
    }
    if let Some(c) = &s.route_choice {
        let _ = writeln!(r, "\nSelected: **{}** by {}.", c["selected"].as_str().unwrap_or(""), c["by"].as_str().unwrap_or(""));
    }
    let _ = writeln!(r, "\n## 3. Alignment\n");
    if let Some(al) = &s.alignment {
        let _ = writeln!(r, "Length {:.2} m.\n\n| Curve | R m | Ls in/out m | Δ° | start | end |\n|---|---:|---:|---:|---:|---:|", al.length);
        for c in al.curves() {
            let _ = writeln!(r, "| {} | {:.1} | {:.1}/{:.1} | {:.2} | {:.2} | {:.2} |", c.index, c.radius, c.ls_in, c.ls_out, c.delta_deg, c.sta_start, c.sta_end);
        }
    }
    if let Some(pr) = &s.profile {
        let _ = writeln!(r, "\n| VPI | Station | Elevation | L m | K | type |\n|---|---:|---:|---:|---:|---|");
        for (i, v) in pr.input.vpis.iter().enumerate() {
            let c = pr.curves.iter().find(|c| c.vpi_index == i);
            let _ = writeln!(r, "| {i} | {:.2} | {:.2} | {} | {} | {} |", v[0], v[1], c.map_or("".into(), |c| format!("{:.0}", c.length)), c.map_or("".into(), |c| format!("{:.1}", c.k)), c.map_or("", |c| if c.crest { "crest" } else { "sag" }));
        }
        let _ = writeln!(r, "\nGrades %: {:?}", pr.grades_pct.iter().map(|g| (g * 100.0).round() / 100.0).collect::<Vec<_>>());
    }
    let _ = writeln!(r, "\n## 4. Typical section\n");
    if let Some(a) = &s.assembly {
        let _ = writeln!(r, "Lanes {:?} m per side, shoulder {} m, cut slope 1V:{}H, fill slope 1V:{}H, pavement {:?}.", a.lanes_m, a.shoulder_m, a.cut_slope_h, a.fill_slope_h, a.pavement.iter().map(|l| format!("{} {:.0} mm", l.name, l.thickness_m * 1000.0)).collect::<Vec<_>>());
    }
    let _ = writeln!(r, "\n## 5. Earthwork\n");
    if let Some(o) = &s.optimisation {
        let _ = writeln!(r, "Profile optimiser ({}). Imbalance before {:.0} m³ → after {:.0} m³; cut {:.0} → {:.0} m³; fill {:.0} → {:.0} m³; wall concrete {:.0} → {:.0} m³; {} evaluations.\n", o.method, o.before_eval.imbalance_m3, o.after_eval.imbalance_m3, o.before_eval.cut_m3, o.after_eval.cut_m3, o.before_eval.fill_m3, o.after_eval.fill_m3, o.before_eval.wall_concrete_m3, o.after_eval.wall_concrete_m3, o.evaluations);
    }
    if let Some(e) = &s.earthwork {
        let _ = writeln!(r, "Cut {:.0} m³ (bank), fill {:.0} m³ (compacted), shrink factor {}, bulking {}. Usable cut {:.0} m³; borrow {:.0} m³; waste {:.0} m³ bank ({:.0} m³ loose). Free haul {} m; overhaul {:.0} m³·m.\n\nMethod: {}\n", e.cut_bank_m3, e.fill_compacted_m3, e.shrink_factor, e.bulking_factor, e.usable_cut_compacted_m3, e.borrow_compacted_m3, e.waste_bank_m3, e.waste_loose_m3, e.free_haul_m, e.overhaul_m3m, e.method);
    }
    let _ = writeln!(r, "## 6. Pavement\n");
    if let Some(p) = &s.pavement {
        let _ = writeln!(r, "{}: {}. See pavement-report.md.", p.method, p.layers.iter().map(|l| format!("{} {:.0} mm", l.name, l.thickness_mm)).collect::<Vec<_>>().join(", "));
    }
    let _ = writeln!(r, "\n## 7. Protection works\n");
    if let Some(p) = &s.protection {
        for x in &p.barriers {
            let _ = writeln!(r, "- Barrier {} {:.0}–{:.0} ({:.0} m, LON {:.1} m): {}", x.side, x.from, x.to, x.length_m, x.length_of_need_m, x.reason);
        }
        for d in &p.ditches {
            let _ = writeln!(r, "- Ditch {} {:.0}–{:.0}: Q {:.3} m³/s, d {:.3} m, τd {:.1} N/m² → {}", d.side, d.from, d.to, d.q_max_m3s, d.depth_m, d.tau_d, d.lining);
        }
        for a in &p.slope_protection {
            let _ = writeln!(r, "- {}: {:.0} m²", a.class, a.area_m2);
        }
    }
    let _ = writeln!(r, "\n## 8. Drainage\n\nSee drainage-report.md.\n\n## 9. Intersections\n");
    for x in &s.intersections {
        let _ = writeln!(r, "- {} at station {:.1}: {} legs, {} checks ({} failing).", x.name, x.main_station, x.legs.len(), x.checks.len(), x.checks.iter().filter(|c| !c.pass).count());
    }
    let _ = writeln!(r, "\n## 10. Departures / relaxations\n\n{} entries; see departures-register.csv.\n\n## 11. Checks\n", dep.len());
    let all = s.all_checks();
    let _ = writeln!(r, "{} checks, {} mandatory failures, {} advisory not met.\n\n| Clause / source | checks |\n|---|---:|", all.len(), all.iter().filter(|c| !c.pass && c.severity == "mandatory").count(), all.iter().filter(|c| !c.pass && c.severity != "mandatory").count());
    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    for c in &all {
        *by.entry(c.citation.clone()).or_default() += 1;
    }
    for (k, v) in by {
        let _ = writeln!(r, "| {k} | {v} |");
    }
    let _ = writeln!(r, "\n## 12. Engineer inputs, geotechnical interface and limits\n");
    for p in rs.params.iter().filter(|p| p.status == "engineer") {
        let _ = writeln!(r, "- {} = {} {} (engineer input; {})", p.name, p.value, p.unit, p.source);
    }
    let _ = writeln!(r, "- Geotechnical report inputs needed: design CBR / resilient modulus, soil parameters for walls (γ, φ, base friction, bearing), slope stability of cuts and fills, shrink/bulk factors, rock/soil classification for excavation.\n- Hydrology: design rainfall intensity is an engineer input (no IDF/ARR/FEH computation).\n\n## 13. Quantities\n\n{method}\n\n| Code | Item | Unit | Quantity |\n|---|---|---|---:|");
    for q in rows {
        let _ = writeln!(r, "| {} | {} | {} | {:.1} |", q.code, q.item, q.unit, q.qty);
    }
    r
}

fn drainage_report(s: &Session) -> String {
    let mut r = String::from("# Drainage report\n\n");
    for n in &s.networks {
        let _ = writeln!(r, "## {}\n\nHGL method: {}\n\n| Pipe | From→To | L m | D mm | S % | Q design m³/s | Q full | V full m/s | h/D | PF | HGL up/dn |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|", n.name, n.hgl_method);
        for p in &n.pipes {
            let _ = writeln!(r, "| {} | {}→{} | {:.1} | {:.0} | {:.2} | {:.4} | {:.4} | {:.2} | {:.2} | {:.2} | {:.2}/{:.2} |", p.id, p.from, p.to, p.length, p.diameter_m * 1000.0, p.slope * 100.0, p.q_design, p.q_full, p.v_full, p.depth_ratio, p.peak_factor, p.hgl_up, p.hgl_dn);
        }
        let f: Vec<_> = n.checks.iter().filter(|c| !c.pass && c.severity == "mandatory").collect();
        let _ = writeln!(r, "\nChecks: {} total, {} mandatory failures. Citations: {}\n", n.checks.len(), f.len(), n.checks.iter().map(|c| c.citation.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>().join("; "));
    }
    if let Some(p) = &s.protection {
        let _ = writeln!(r, "## Roadside ditches (HEC-15)\n\n| Side | From–To | Flow | Q m³/s | S | d m | V m/s | τd N/m² | Lining | τp N/m² |\n|---|---|---|---:|---:|---:|---:|---:|---|---:|");
        for d in &p.ditches {
            let _ = writeln!(r, "| {} | {:.0}–{:.0} | {} | {:.3} | {:.4} | {:.3} | {:.2} | {:.1} | {} | {:.1} |", d.side, d.from, d.to, d.flow_dir, d.q_max_m3s, d.slope, d.depth_m, d.velocity_mps, d.tau_d, d.lining, d.tau_p);
        }
        for d in &p.ditches {
            for n in &d.notes {
                let _ = writeln!(r, "\n- {} {:.0}–{:.0}: {n}", d.side, d.from, d.to);
            }
        }
    }
    let _ = writeln!(r, "\n## Limits\n\n- Rainfall intensity and runoff coefficients are engineer inputs. No IDF/ARR 2019/FEH hydrology is computed.\n- Culverts and subsurface drainage are not designed.");
    r
}

#[allow(dead_code)]
fn _unused(_: &Value) {}
