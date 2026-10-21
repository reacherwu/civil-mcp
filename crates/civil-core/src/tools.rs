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
use crate::standards::{self, Check, DesignBasis};
use crate::superelev::{self, Superelevation};
use crate::tin::{Tin, TinStats};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Default)]
pub struct Session {
    pub dir: Option<PathBuf>,
    pub log: Option<AuditLog>,
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
    /// horizontal EPSG code of the imported terrain (None for local / unknown)
    pub crs_epsg: Option<u32>,
    pub corridor_datum: Option<Tin>,
    pub composite: Option<(f64, f64, usize, f64)>,
    pub gradings: Vec<Grading>,
    pub networks: Vec<Network>,
    pub intersections: Vec<crate::intersect::Intersection>,
    pub mass_haul: Option<MassHaul>,
    pub materials: Vec<MaterialRow>,
    pub files: Vec<(String, String)>,
    pub approved_seq: Option<u64>,
    pub pavement: Option<crate::pavement::PavementDesign>,
    pub route: Option<Value>,
    pub route_inputs: Vec<(String, crate::align::AlignmentInput)>,
    pub route_choice: Option<Value>,
    pub optimisation: Option<crate::workflow::OptResult>,
    pub protection: Option<crate::protection::Protection>,
    pub earthwork: Option<quantities::Earthwork>,
    pub departures: Vec<Value>,
    pub traffic: Option<Value>,
    pub ext: std::collections::BTreeMap<String, crate::ext::ExtOut>,
    /// sheet frame chosen with set_sheet_frame (None = profile defaults of the design-basis country)
    pub frame: Option<crate::frame::FrameSpec>,
    /// drawing-standard checks of the last generate_drawings run
    pub frame_checks: Vec<Check>,
    /// unit system chosen with set_units (None = SI)
    pub units: Option<crate::units::UnitSpec>,
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
    let mut v = json!([
      {"name":"open_project","description":"Open (or create) a project directory. The audit log lives at <dir>/audit.jsonl and is verified on open.","inputSchema":obj(json!({"dir":s}),&["dir"])},
      {"name":"set_design_basis","description":"Record the design basis chosen by the engineer in responsible charge: the standards profile (US/CA/UK/AU/NZ) and design parameters. Values the profile cannot ship (copyrighted or unverified tables) must be entered here; the call fails and lists them otherwise. Returns the parameter register with the provenance of every value. actor must be 'engineer:<name>'.","inputSchema":obj(json!({"actor":s,"basis":{"type":"object","properties":{"project":s,"engineer_of_record":s,"standard":{"type":"string","enum":["US_AASHTO_2018","CA_TAC_2017","UK_DMRB","AU_AUSTROADS","NZ_AUSTROADS_NZTA"]},"design_speed_kmh":num,"e_max":num,"f_max":num,"max_grade_pct":num,"road_type":{"type":"string","enum":["motorway","dual","single"]},"perception_reaction_s":num,"deceleration_mps2":num,"deceleration_coeff":num,"eye_height_m":num,"object_height_m":num,"sag_k_min":num,"max_relative_gradient_pct":num,"transition_q":num,"wall_criteria":{"type":"object"},"normal_crown":num,"lane_width_m":num,"lanes_rotated_per_side":num},"required":["project","engineer_of_record","standard","design_speed_kmh","e_max"]}}),&["actor","basis"])},
      {"name":"load_points","description":"Load survey points from CSV (id,easting,northing,elevation,desc) or LandXML CgPoints. Rejected rows are reported.","inputSchema":obj(json!({"path":s,"format":{"type":"string","enum":["csv","landxml"]}}),&["path"])},
      {"name":"import_landxml","description":"Import a LandXML 1.x file: TIN surface (as existing ground), alignment (line/arc/clothoid) and profile. Reports differences against values stored in the file (areas, elevations, lengths, element end points).","inputSchema":obj(json!({"path":s,"surface":s,"alignment":s,"use_surface":b,"use_alignment":b}),&["path"])},
      {"name":"export_landxml","description":"Write LandXML 1.2 with points, existing-ground and corridor surfaces, alignment and profile.","inputSchema":obj(json!({"file":s}),&[])},
      {"name":"generate_construction_set","description":"Write the construction-scale sheet set (construction-set.pdf, cs-NN.dxf/svg, construction-register.csv): survey control and curve/vertical data tables, plan-and-profile sheets split at match lines at a standard scale (1:500 metric, 1\" = 50' US), existing-conditions base (contours, engineer-entered property lines and right-of-way), gridded cross sections at a standard interval (20 m / 50 ft) with ROW and construction limits, and a quantity summary by pay item with earthwork per plan sheet. Missing engineer inputs are flagged on the sheets and in the checks. The main set (generate_drawings) is unchanged.","inputSchema":obj(json!({"row_left_m":num,"row_right_m":num,"property_lines":arr(json!({"type":"object","properties":{"id":s,"pts":arr(arr(num.clone()))}})),"control":arr(json!({"type":"object","properties":{"id":s,"e":num,"n":num,"z":num,"desc":s}})),"xs_interval_m":num,"plan_scale":num}),&[])},
      {"name":"export_blender_scene","description":"Write a Blender-ready visualisation scene: scene.glb (glTF 2.0: existing ground, existing ground trimmed exactly to the corridor daylight lines, road pavement, pavement structure layers as closed solids following crossfall / superelevation, earthwork slopes, retaining walls, pipes, structures, route alternatives as coloured ribbons), scene.json (georeference origin E0/N0/Z0 + EPSG, camera path along the design centreline, alternative metrics) and blender_scene.py, a headless script (blender -b -P blender_scene.py -- <dir>) that builds materials, lighting and camera, renders an overview, one still per alternative, driver-eye stills and a flythrough MP4, and exports USD (scene.usdc) and scene.blend. Concept / preliminary visualisation only.","inputSchema":obj(json!({}),&[])},
      {"name":"export_cad_model","description":"Write a 3D CAD model for Civil 3D / MicroStation handoff (DXF R12, metres, world coordinates): existing and finished-grade TIN faces, 3D centreline, corridor feature lines by point code, wall tops, pipes at invert, structures at rim. Layers follow the active drafting profile (NCS for US, ISO 13567 elsewhere) or 'layer_map' {internal: agency layer} (keys SURF-EG, SURF-FG, ALIGNMENT, FEATURE, WALL, PIPE, STRUCTURE). Use with export_landxml for alignments, profiles, cross sections and pipe networks. Optional DWG: 'dwg_version' (ACAD2013 = AC1027, ACAD2018 = AC1032) converts the model (and with 'dwg_include_sheets' every sheet-NN / cs-NN DXF in the project) into dwg/ through ODA File Converter when it is installed (ODA_FILE_CONVERTER or ODAFileConverter on PATH); errors if it is not.","inputSchema":obj(json!({"file":s,"layer_map":{"type":"object"},"dwg_version":{"type":"string","enum":["ACAD2004","ACAD2007","ACAD2010","ACAD2013","ACAD2018"]},"dwg_include_sheets":{"type":"boolean"}}),&[])},
      {"name":"import_terrain","description":"Import terrain from a GeoTIFF / Cloud-Optimised GeoTIFF DEM, an ESRI ASCII grid (.asc), or a LAS/LAZ point cloud (ASPRS classes, default [2] ground), and build the existing-ground TIN. CRS: EPSG from GeoTIFF keys or LAS VLRs (or 'epsg'); geographic and Web Mercator data are projected to UTM; projected data (UTM, MGA, state plane, national grids) are kept planar with foot units (US survey / international) converted to metres. 'crop' [xmin,ymin,xmax,ymax] in output metres, 'step' subsamples, 'z_units' m|ft|us_ft.","inputSchema":obj(json!({"path":s,"step":num,"crop":arr(num.clone()),"epsg":num,"target_epsg":num,"xy_units":{"type":"string","enum":["m","ft","us_ft"]},"z_units":{"type":"string","enum":["m","ft","us_ft"]},"classes":arr(num.clone()),"max_points":num,"max_edge_m":num,"build_surface":b}),&["path"])},
      {"name":"build_surface","description":"Build a Delaunay TIN from loaded points. Optional max_edge_m drops long hull triangles.","inputSchema":obj(json!({"max_edge_m":num}),&[])},
      {"name":"create_alignment","description":"Horizontal alignment from PIs, circular radii and optional symmetric clothoid lengths (tangent–spiral–arc–spiral–tangent). Fails if curves overlap.","inputSchema":obj(json!({"name":s,"pis":xy,"radii":arr(num.clone()),"spirals":arr(num.clone()),"start_station":num}),&["name","pis","radii"])},
      {"name":"sample_ground_profile","description":"Sample existing ground along the alignment.","inputSchema":obj(json!({"interval_m":num}),&[])},
      {"name":"suggest_profile","description":"Propose a design profile: VPIs on ground at the given stations, grades clamped to the basis maximum, vertical curve lengths set to the minimum required, rounded up to 5 m. Call create_profile to adopt it.","inputSchema":obj(json!({"vpi_stations":arr(num.clone())}),&["vpi_stations"])},
      {"name":"create_profile","description":"Design vertical profile from VPIs [station, elevation] and parabolic curve lengths.","inputSchema":obj(json!({"name":s,"vpis":xy,"curve_lengths":arr(num.clone())}),&["name","vpis","curve_lengths"])},
      {"name":"design_superelevation","description":"Superelevation for each curve (proportional AASHTO Method 1 for US/CA/AU/NZ, or DMRB CD 109 Eq 4.2 S = V²/(2.828R) for UK; capped at e_max) with transition stations; rotation about the centreline.","inputSchema":obj(json!({}),&[])},
      {"name":"check_standards","description":"Run all alignment/profile/superelevation checks (and pipe checks if a network exists). Each result cites the rule and shows the formula.","inputSchema":obj(json!({}),&[])},
      {"name":"create_assembly","description":"Define the typical section: lanes, shoulder or curb/gutter/sidewalk, ditch (cut only), cut/fill slopes with optional benches, pavement layers, retaining-wall rule, and right-side overrides for asymmetric sections.","inputSchema":obj(json!({"name":s,"lanes_m":arr(num.clone()),"shoulder_m":num,"shoulder_slope":num,"shoulder_paved":b,"ditch":{"type":"object","properties":{"foreslope_h":num,"depth_m":num,"bottom_width_m":num}},"cut_slope_h":num,"fill_slope_h":num,"cut_bench":{"type":"object","properties":{"every_m":num,"width_m":num}},"fill_bench":{"type":"object","properties":{"every_m":num,"width_m":num}},"pavement":arr(json!({"type":"object","properties":{"name":s,"thickness_m":num}})),"subgrade_slope_h":num,"curb":{"type":"object","description":"barrier curb + gutter (+ sidewalk): gutter_m, gutter_slope, height_m, top_m, sidewalk_m, sidewalk_slope"},"wall":{"type":"object","description":"retaining-wall rule: kind cantilever|gravity, max_daylight_offset_m (ROW), max_slope_height_m, offset_from_hinge_m, embedment_m, soil {gamma_kn_m3, phi_deg, base_friction, bearing_kpa, surcharge_kpa}"},"right":{"type":"object","description":"right-side overrides for asymmetric sections (lanes_m, shoulder_m, shoulder_slope, shoulder_paved, curb, ditch, cut_slope_h, fill_slope_h, wall, remove:[\"curb\"|\"ditch\"|\"wall\"])"}}),&["name","lanes_m","shoulder_m","shoulder_slope","shoulder_paved","cut_slope_h","fill_slope_h"])},
      {"name":"build_corridor","description":"Apply the assembly at sample lines (regular interval plus geometry key stations), daylight to ground, compute end-area volumes and a composite-volume cross-check, build corridor surfaces.","inputSchema":obj(json!({"interval_m":num,"composite_cell_m":num,"widening":{"type":"object","description":"lane widening on curves: method AASHTO (vehicle {name, track_u_m, wheelbases_m, front_overhang_m}) or TABLE (engineer table [[radius_upper_m, widening_m]], table_citation)"}}),&["interval_m"])},
      {"name":"get_section","description":"Cross section at a sample-line station (nearest).","inputSchema":obj(json!({"station":num}),&["station"])},
      {"name":"design_intersection","description":"At-grade T or 4-leg intersection on the designed main road: legs (each with its own assembly), curb returns tangent to both edges, leg profiles tied to the main-road finished grade, a paving patch with volumes, and stop-control sight triangles (AASHTO case B1) tested for line-of-sight obstruction. Checks return radius, approach grade and sight distance.","inputSchema":obj(json!({"name":s,"main_station":num,"major_speed_kmh":num,"minor_speed_kmh":num,"min_return_radius_m":num,"max_approach_grade_pct":num,"time_gap_s":num,"legs":arr(json!({"type":"object","properties":{"name":s,"side":{"type":"string","enum":["left","right"]},"angle_deg":num,"length_m":num,"grade_pct":num,"return_radii_m":arr(num.clone()),"assembly":{"type":"object"}},"required":["name","side","length_m","grade_pct","return_radii_m","assembly"]}))}),&["name","main_station","legs","major_speed_kmh","minor_speed_kmh","min_return_radius_m","max_approach_grade_pct"])},
      {"name":"grade","description":"Grade a pad (closed polygon) or feature line (open, side left/right) to existing ground at cut/fill slopes; returns daylight line and composite volumes.","inputSchema":obj(json!({"name":s,"vertices":xyz,"closed":b,"side":s,"cut_slope_h":num,"fill_slope_h":num,"spacing_m":num,"cell_m":num}),&["name","vertices","closed","cut_slope_h","fill_slope_h"])},
      {"name":"design_pipe_network","description":"Gravity storm/sanitary network: inverts from min cover/slope/drop, Manning full and partial flow, rational-method flows; checks per FHWA HEC-22 (storm) or the engineer's sewer criteria (sanitary peaking: Harmon or Ten States Standards). Rims default to the corridor surface, else existing ground.","inputSchema":obj(json!({"name":s,"criteria":{"type":"object","properties":{"standard":{"type":"string","enum":["FHWA_HEC22","ENGINEER"]},"system":{"type":"string","enum":["storm","sanitary"]},"min_cover_m":num,"min_slope":num,"structure_drop_m":num,"intensity_mm_h":num,"tailwater_m":num,"freeboard_m":num},"required":["standard","system","min_cover_m","min_slope"]},"structures":arr(json!({"type":"object","properties":{"id":s,"x":num,"y":num,"rim":num,"area_ha":num,"c":num,"inflow_m3s":num,"population":num,"per_capita_lpd":num},"required":["id","x","y"]})),"pipes":arr(json!({"type":"object","properties":{"id":s,"from":s,"to":s,"diameter_m":num,"n":num,"material":s,"max_slope":num,"min_slope":num},"required":["id","from","to","diameter_m","n"]}))}),&["name","criteria","structures","pipes"])},
      {"name":"compute_quantities","description":"Mass haul ordinates and balance points, shrinkage (cut_factor), bulking, free haul and overhaul, borrow and waste, and a material table by station range.","inputSchema":obj(json!({"cut_factor":num,"bulking_factor":num,"free_haul_m":num,"table_step_m":num}),&[])},
      {"name":"generate_drawings","description":"Write model.dxf (world), sheets.pdf (all sheets, embedded font), sheet-NN.dxf and sheet-NN.svg per sheet, sheet-register.csv and drawing-standards.json. Every sheet is placed on the sheet frame (set_sheet_frame, or the drafting profile of the design-basis country) with its title block, revision table, sign-off, seal box, north arrow and scale bar; layers, line weights and line types follow the profile. Optional 'frame' overrides the stored frame for this run. PRELIMINARY stamp unless approved with zero mandatory failures.","inputSchema":obj(json!({"frame":crate::frame::spec_schema()}),&[])},
      {"name":"generate_review_report","description":"Write review.html listing every check (pass/fail with citation), quantities, output file hashes and the audit trail.","inputSchema":obj(json!({}),&[])},
      {"name":"record_review","description":"Record the engineer's decision. actor must be 'engineer:<name>'. approve is refused while mandatory checks fail. Any later design change voids the approval.","inputSchema":obj(json!({"actor":s,"decision":{"type":"string","enum":["approve","reject","comment"]},"note":s}),&["actor","decision","note"])},
      {"name":"verify_audit_log","description":"Recompute the hash chain of the audit log and report the first tampered entry, if any.","inputSchema":obj(json!({}),&[])},
      {"name":"design_pavement","description":"Pavement thickness design using the method of the project's standards profile. US/CA: AASHTO 1993 flexible (SN with layered analysis) or rigid (D). AU/NZ: Austroads AGPT02 empirical granular chart (Figure 8.4 equation). UK: DMRB CD 226 (Eq 2.24 HBGM asphalt, Eq 2.43 URC; Figure 2.20 asphalt-base thickness entered by the engineer). Optional traffic block computes design traffic from AADT. Layers feed create_assembly when it has no pavement.","inputSchema":obj(json!({"kind":{"type":"string","enum":["flexible","rigid"]},"subgrade_cbr":num,"subgrade_mr_mpa":num,"design_traffic":num,"traffic":{"type":"object","description":"aadt, hv_frac, esa_per_hv, growth, years, dir_factor, lane_factor"},"reliability_zr":num,"s0":num,"p0":num,"pt":num,"layers":arr(json!({"type":"object"})),"sc_mpa":num,"ec_mpa":num,"j":num,"cd":num,"k_mpa_m":num,"surfacing_mm":num,"base_mm":num,"foundation_class":num,"uk_type":s,"uk_chart_asphalt_mm":num,"uk_chart_hbgm_mm":num,"uk_rc_mpa":num,"uk_tied_shoulder":b}),&["kind","subgrade_cbr"])},
      {"name":"generate_route_alternatives","description":"Route/corridor selection. Least-cost paths over the terrain grid between start and end, with heading-constrained moves and a penalty on grades above the profile maximum. Several weightings or a separate-corridor penalty give distinct alternatives. Each path is simplified to PIs and given fitted radii. Each alternative is then built in full (alignment, superelevation, suggested profile, corridor with the current assembly) and compared on length, radii, max grade, cut/fill, imbalance, walls, mandatory failures and a unit-rate cost proxy. Returns the agent's recommendation with reasons.","inputSchema":obj(json!({"hairpin_min_radius_m":num,"limits":{"type":"object","properties":{"max_fill_m":num,"max_cut_m":num,"max_wall_m":num,"source":s}},"allow_infeasible":b,"start":{"type":"array","items":num},"end":{"type":"array","items":num},"cell_m":num,"radius_pref_m":num,"spiral_m":num,"simplify_tol_m":num,"vpi_spacing_m":num,"interval_m":num,"shrink":num,"rates":{"type":"object"},"alternatives":arr(json!({"type":"object"}))}),&["start","end"])},
      {"name":"select_route","description":"Adopt one route alternative as the alignment and record the choice and reasons in the audit log. actor 'agent' records a recommendation; 'engineer:<name>' records the engineer's decision.","inputSchema":obj(json!({"alternative":s,"actor":s,"reasons":arr(s.clone())}),&["alternative"])},
      {"name":"optimize_profile","description":"Earthwork-balance optimiser. Coordinate descent on interior VPI elevations of the current profile to minimise |cut·shrink − fill| + w_volume·(cut+fill) + w_wall·wall concrete + w_haul·haul moment. Every candidate must pass all mandatory grade and vertical-curve checks. Adopts the result; before/after values are logged.","inputSchema":obj(json!({"interval_m":num,"shrink":num,"w_volume":num,"w_wall":num,"w_haul":num,"steps_m":arr(num.clone())}),&[])},
      {"name":"design_protection","description":"Protection works on the built corridor. Roadside hazards inside the engineer-entered clear zone (critical fill slopes above the warrant height, wall faces and drop-offs) give barrier runs with length of need X = L_R(L_A − L_2)/L_A. Ditch linings are chosen by HEC-15 shear stress (bare soil → engineer linings → gravel mulch → riprap D50 by Eq 6.8 → concrete). Slope-protection areas are reported by slope class.","inputSchema":obj(json!({"clear_zone_m":num,"clear_zone_source":s,"recoverable_h":num,"traversable_h":num,"embankment_warrant_height_m":num,"barrier_offset_m":num,"runout_length_m":num,"barrier_type":s,"runoff_c":num,"intensity_mm_h":num,"catchment_width_m":num,"soil_tau_p":num,"soil_name":s,"extra_linings":arr(json!({"type":"object"})),"riprap_sg":num,"seed_flatter_than_h":num,"blanket_flatter_than_h":num}),&["clear_zone_m","recoverable_h","traversable_h","embankment_warrant_height_m","barrier_offset_m","runout_length_m","runoff_c","intensity_mm_h","catchment_width_m","soil_tau_p"])},
      {"name":"record_departure","description":"Add an entry to the departures / design-exceptions / relaxations register (UK GG 101 departure, US design exception, AU/NZ design exception). Failing checks are added automatically at generate_deliverables. The engineer's justification is recorded here.","inputSchema":obj(json!({"actor":s,"check_id":s,"element":s,"kind":s,"justification":s}),&["actor","check_id","element","justification"])},
      {"name":"generate_deliverables","description":"Write the design-stage deliverables: design report, drainage report, pavement design report, parameter register (verified vs engineer input), departures register, bill of quantities grouped by the profile's measurement method, setting-out report, cost-proxy estimate and deliverables matrix status (Markdown/CSV).","inputSchema":obj(json!({"stage":s}),&[])},
      {"name":"get_state","description":"Summary of what is loaded/designed.","inputSchema":obj(json!({}),&[])},
      {"name":"set_units","description":"Choose the unit system. Default SI (metres). system US switches to US customary: tool outputs (keys renamed _m→_ft, _m3→_cy, _kmh→_mph, _mm→_in ...; stations, coordinates and radii in ft), drawings (engineering scales 1\" = 50', stations 12+34.56, labels in ft/in), reports, BoQ (CY, SY, LF, TON, EA) and CSV/Markdown files. foot: international (0.3048 m, default) or survey (1200/3937 m, legacy state plane). inputs/outputs override the system for the JSON boundary only. In US mode, inputs may use US-suffixed keys (radius_ft, design_speed_mph) or SI keys; unsuffixed lengths are read in feet. Design and checks stay in SI internally; the audit log is SI. Call with no arguments to read the current setting.","inputSchema":obj(json!({"system":{"type":"string","enum":["SI","US"]},"foot":{"type":"string","enum":["international","survey"]},"inputs":{"type":"string","enum":["SI","US"]},"outputs":{"type":"string","enum":["SI","US"]}}),&[])}
    ]);
    if let Some(a) = v.as_array_mut() {
        a.extend(crate::ext::tool_list());
        a.extend(crate::frame::tool_list());
    }
    v
}

fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}

const DESIGN_TOOLS: &[&str] = &[
    "import_terrain",
    "set_design_basis",
    "design_pavement",
    "select_route",
    "optimize_profile",
    "design_protection",
    "load_points",
    "import_landxml",
    "build_surface",
    "create_alignment",
    "create_profile",
    "design_superelevation",
    "create_assembly",
    "build_corridor",
    "design_intersection",
    "grade",
    "design_pipe_network",
];

impl Session {
    pub(crate) fn dir(&self) -> crate::Result<PathBuf> {
        self.dir.clone().ok_or_else(|| "call open_project first".to_string())
    }
    pub(crate) fn log(&mut self, actor: &str, tool: &str, inputs: &Value, outputs: &Value, rules: &[String]) -> crate::Result<Value> {
        self.log.as_mut().ok_or("call open_project first")?.append(actor, tool, inputs, outputs, rules)
    }
    pub(crate) fn need_basis(&self) -> crate::Result<&DesignBasis> {
        self.basis.as_ref().ok_or_else(|| "no design basis: the engineer must call set_design_basis first".to_string())
    }
    pub(crate) fn write(&mut self, name: &str, bytes: &[u8]) -> crate::Result<String> {
        let p = self.dir()?.join(name);
        let conv = crate::units::file_in(name, bytes, crate::units::cur());
        let bytes: &[u8] = conv.as_deref().unwrap_or(bytes);
        std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
        let h = sha256_hex(bytes);
        self.files.retain(|f| f.0 != name);
        self.files.push((name.into(), h.clone()));
        Ok(h)
    }
    fn call_frame(&mut self, name: &str, a: &Value) -> crate::Result<Value> {
        use crate::frame;
        let country = self.basis.as_ref().map(|b| b.standard).unwrap_or(standards::Standard::Us);
        let info = || -> SheetInfo {
            let all = self.all_checks();
            SheetInfo {
                project: self.basis.as_ref().map(|b| b.project.clone()).unwrap_or_else(|| "Project".into()),
                engineer: self.basis.as_ref().map(|b| b.engineer_of_record.clone()).unwrap_or_else(|| "Engineer of record".into()),
                standard: serde_json::to_value(country).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
                design_speed: self.basis.as_ref().map(|b| b.design_speed_kmh).unwrap_or(0.0),
                audit_head: self.log.as_ref().map(|l| l.head.clone()).unwrap_or_default(),
                checks_failed: all.iter().filter(|c| !c.pass && c.severity == "mandatory").count(),
                checks_total: all.len(),
                approved_seq: self.approved_seq,
            }
        };
        match name {
            "list_sheet_frames" => {
                let sizes: Vec<Value> = frame::SIZES.iter().map(|s| json!({"name": s.0, "long_mm": s.1, "short_mm": s.2, "source": s.3})).collect();
                let profiles: Vec<Value> = [standards::Standard::Us, standards::Standard::Ca, standards::Standard::Uk, standards::Standard::Au, standards::Standard::Nz]
                    .iter()
                    .map(|s| {
                        let p = frame::profile(*s);
                        json!({"code": p.code, "name": p.name, "default_size": p.default_size, "margins_mm": p.margins, "title_block_width_mm": p.tb_width, "layer_format": p.layer_fmt, "sheet_id_format": p.sheet_id, "seal": p.seal_label})
                    })
                    .collect();
                let cur = frame::resolve(&self.frame.clone().unwrap_or_default(), country)?;
                Ok(json!({"result": {"sizes": sizes, "profiles": profiles, "current": cur, "current_spec": self.frame}}))
            }
            "get_drafting_standard" => {
                let c = a["country"].as_str().ok_or("country required (US, CA, UK, AU, NZ)")?;
                let s = frame::country(c).ok_or("country must be US, CA, UK, AU or NZ")?;
                Ok(json!({"result": frame::profile_json(s)}))
            }
            "set_sheet_frame" => {
                let spec: frame::FrameSpec = serde_json::from_value(a.get("frame").cloned().unwrap_or(json!({}))).map_err(|e| format!("frame: {e}"))?;
                let r = frame::resolve(&spec, country)?;
                let out = json!({"frame": r, "profile": r.profile.name, "required_title_block_fields": r.profile.required_fields});
                let prov =
                    self.log("agent", name, a, &json!({"profile": r.country, "size": r.size, "orientation": r.orientation, "layout_factor": r.k}), &[])?;
                self.frame = Some(spec);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "preview_sheet_frame" => {
                let spec: frame::FrameSpec = match a.get("frame") {
                    Some(f) if !f.is_null() => serde_json::from_value(f.clone()).map_err(|e| format!("frame: {e}"))?,
                    _ => self.frame.clone().unwrap_or_default(),
                };
                let r = frame::resolve(&spec, country)?;
                let inf = info();
                let d = frame::preview(&r, &inf);
                let ids = frame::sheet_ids(&r, &["Plan preview".into()]);
                let checks = frame::check_set(std::slice::from_ref(&d), &r, &ids, &inf);
                let mut files = json!({});
                files["frame-preview.pdf"] = json!(self.write("frame-preview.pdf", &drawing::to_pdf(&[(&d, r.w, r.h)]))?);
                files["frame-preview.svg"] = json!(self.write("frame-preview.svg", d.to_svg(r.w, r.h).as_bytes())?);
                files["frame-preview.dxf"] = json!(self.write("frame-preview.dxf", d.to_dxf().as_bytes())?);
                let fail = checks.iter().filter(|c| !c.pass && c.severity == "mandatory").count();
                Ok(json!({"result": {"frame": r, "files": files, "checks": checks, "mandatory_failures": fail}}))
            }
            "check_drawing_standards" => {
                if self.frame_checks.is_empty() {
                    return Err("run generate_drawings first".into());
                }
                let fail = self.frame_checks.iter().filter(|c| !c.pass && c.severity == "mandatory").count();
                let adv = self.frame_checks.iter().filter(|c| !c.pass && c.severity != "mandatory").count();
                Ok(json!({"result": {"checks": self.frame_checks, "mandatory_failures": fail, "advisory_flags": adv}}))
            }
            _ => Err(format!("unknown tool {name}")),
        }
    }

    pub fn all_checks(&self) -> Vec<Check> {
        let mut c = self.checks.clone();
        c.extend(self.ext_checks());
        for n in &self.networks {
            c.extend(n.checks.iter().cloned());
        }
        for i in &self.intersections {
            c.extend(i.checks.iter().cloned());
        }
        if let Some(p) = &self.protection {
            c.extend(p.checks.iter().cloned());
        }
        for g in &self.gradings {
            c.push(standards::chk(
                "G-DAYLIGHT",
                "Grading reaches daylight everywhere",
                format!("grading {}", g.name),
                g.warnings.len() as f64,
                0.0,
                "points",
                "max",
                "mandatory",
                "footprint points without daylight".into(),
                "tool rule".into(),
            ));
        }
        if let Some(cor) = &self.corridor {
            let cr = self.wall_criteria().unwrap_or_else(|e| crate::walls::Criteria {
                fs_sliding: f64::INFINITY,
                fs_overturning: f64::INFINITY,
                e_frac: 0.0,
                qmax_factor: 0.0,
                citation: e,
            });
            for left in [true, false] {
                let ws: Vec<(f64, &crate::corridor::WallPlaced)> = cor
                    .sections
                    .iter()
                    .filter_map(|s| {
                        let sd = if left { &s.left } else { &s.right };
                        sd.wall.as_ref().map(|w| (s.station, w))
                    })
                    .collect();
                if ws.is_empty() {
                    continue;
                }
                let side = if left { "left" } else { "right" };
                let worst = |f: &dyn Fn(&crate::walls::WallDesign) -> f64, max: bool| {
                    ws.iter().map(|(st, w)| (f(&w.design), *st)).fold((if max { f64::MIN } else { f64::MAX }, 0.0), |a, b| {
                        if (max && b.0 > a.0) || (!max && b.0 < a.0) {
                            b
                        } else {
                            a
                        }
                    })
                };
                let (v, st) = worst(&|d| d.fs_sliding, false);
                c.push(standards::chk(
                    "W-SLIDE",
                    "Wall sliding FS",
                    format!("{side} wall (worst at {st:.1})"),
                    v,
                    cr.fs_sliding,
                    "",
                    "min",
                    "mandatory",
                    "Ks = μ·ΣV / Ea, Rankine Ka = tan²(45°−φ/2), Ea = ½γH²Ka + qKaH".into(),
                    cr.citation.clone(),
                ));
                let (v, st) = worst(&|d| d.fs_overturning, false);
                c.push(standards::chk(
                    "W-OVERTURN",
                    "Wall overturning FS",
                    format!("{side} wall (worst at {st:.1})"),
                    v,
                    cr.fs_overturning,
                    "",
                    "min",
                    "mandatory",
                    "Kt = ΣM_resist / ΣM_overturn about the toe".into(),
                    cr.citation.clone(),
                ));
                let (v, st) = worst(&|d| d.eccentricity / d.base_b, true);
                c.push(standards::chk(
                    "W-ECC",
                    "Base eccentricity e/B",
                    format!("{side} wall (worst at {st:.1})"),
                    v,
                    cr.e_frac,
                    "",
                    "max",
                    "mandatory",
                    "e = B/2 − (ΣMr − ΣMo)/ΣV".into(),
                    cr.citation.clone(),
                ));
                let (v, st) = worst(&|d| d.q_max_kpa, true);
                let bearing = self
                    .assembly
                    .as_ref()
                    .and_then(|a| if left { a.wall.as_ref() } else { a.right.as_ref().and_then(|r| r.wall.as_ref()).or(a.wall.as_ref()) })
                    .map_or(f64::NAN, |w| w.soil.bearing_kpa);
                c.push(standards::chk(
                    "W-BEAR",
                    "Max bearing pressure",
                    format!("{side} wall (worst at {st:.1})"),
                    v,
                    cr.qmax_factor * bearing,
                    "kPa",
                    "max",
                    "mandatory",
                    "p_max = ΣV/B·(1 + 6e/B)".into(),
                    cr.citation.clone(),
                ));
            }
            c.push(standards::chk(
                "C-DAYLIGHT",
                "Corridor sections reach daylight",
                "corridor".into(),
                cor.sections_with_warnings as f64,
                0.0,
                "sections",
                "max",
                "mandatory",
                "sections whose slopes did not meet ground within data/150 m".into(),
                "tool rule".into(),
            ));
        }
        c
    }
    pub fn wall_criteria(&self) -> crate::Result<crate::walls::Criteria> {
        let b = self.need_basis()?;
        crate::walls::criteria(b.standard, b.wall_criteria.as_ref())
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
        self.protection = None;
        self.earthwork = None;
    }

    /// Unit settings: SI unless set_units chose US customary.
    pub fn unit_spec(&self) -> crate::units::UnitSpec {
        self.units.unwrap_or(crate::units::UnitSpec { system: crate::units::System::Si, foot: crate::units::Foot::International, inputs: None, outputs: None })
    }

    pub fn call(&mut self, name: &str, a: &Value) -> crate::Result<Value> {
        let spec = self.unit_spec();
        if name == "set_units" {
            return self.set_units(a);
        }
        if !spec.files().us() && !spec.inputs().us() && !spec.outputs().us() {
            crate::units::set_cur(crate::units::Units::SI);
            return self.call_si(name, a);
        }
        crate::units::set_plain_lengths(!crate::units::RATIO_XY_TOOLS.contains(&name));
        let si = crate::units::args_to_si(a, spec.inputs());
        let r = crate::units::with(spec.files(), || self.call_si(name, &si));
        let spec = self.unit_spec();
        match r {
            Ok(v) if spec.outputs().us() => {
                let mut v = crate::units::result_to_us(&v, spec.outputs());
                if let Some(o) = v.as_object_mut() {
                    o.insert("units".into(), json!(spec.outputs().label()));
                }
                Ok(v)
            }
            Err(e) if spec.outputs().us() => Err(crate::units::text_in(&e, spec.outputs())),
            r => r,
        }
    }

    fn set_units(&mut self, a: &Value) -> crate::Result<Value> {
        if a.get("system").is_some() {
            let spec: crate::units::UnitSpec = serde_json::from_value(a.clone()).map_err(|e| format!("units: {e}"))?;
            self.units = Some(spec);
            let d = crate::units::describe(&spec, false);
            if self.dir.is_some() {
                self.write("units.json", serde_json::to_string_pretty(&d).unwrap().as_bytes())?;
            }
            if self.log.is_some() {
                let prov = self.log("agent", "set_units", a, &d, &["unit system".to_string()])?;
                return Ok(json!({"result": d, "provenance": prov}));
            }
            return Ok(json!({"result": d}));
        }
        Ok(json!({"result": crate::units::describe(&self.unit_spec(), self.units.is_none())}))
    }

    fn call_si(&mut self, name: &str, a: &Value) -> crate::Result<Value> {
        let r = if crate::frame::TOOLS.contains(&name) {
            self.call_frame(name, a)
        } else {
            match self.call_ext(name, a) {
                Some(r) => r,
                None => self.call_inner(name, a),
            }
        };
        if r.is_ok() && (DESIGN_TOOLS.contains(&name) || crate::ext::TOOLS.contains(&name)) {
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
                let units = self.units;
                *self = Session::default();
                self.units = units;
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
                let r = standards::resolve(&b)?;
                let out = json!({"standard": b.standard.label(), "ssd_m": r.ssd, "parameters": r.params, "engineer_inputs": r.params.iter().filter(|p| p.status == "engineer").map(|p| &p.name).collect::<Vec<_>>()});
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
                    if let Some(s) = x.surfaces.iter().find(|s| want.is_none_or(|w| w == s.name)) {
                        self.tin = Some(Tin::from_faces(&s.points, &s.faces)?);
                        self.invalidate_corridor();
                    }
                }
                if a["use_alignment"].as_bool().unwrap_or(true) {
                    let want = a["alignment"].as_str();
                    if let Some(al) = x.alignments.iter().find(|s| want.is_none_or(|w| w == s.name)) {
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
                let xml = landxml::write(&landxml::Export {
                    points: &self.points,
                    surfaces,
                    alignment: self.alignment.as_ref(),
                    profiles: self.profile.iter().collect(),
                    networks: self.networks.iter().collect(),
                    corridor: self.corridor.as_ref(),
                    epsg: self.crs_epsg,
                });
                let h = self.write(&file, xml.as_bytes())?;
                let out = json!({"file": file, "sha256": h, "bytes": xml.len()});
                let prov = self.log("agent", name, a, &out, &["LandXML 1.2 schema".into()])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "generate_construction_set" => {
                let b = self.need_basis()?.clone();
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let cor = self.corridor.as_ref().ok_or("build_corridor first")?;
                let inp: crate::cset::Inputs = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let (boq, _) = crate::deliver::boq(self);
                let c = crate::cset::Ctx { tin: t, al, pr, cor, boq, inp: &inp };
                let p = crate::cset::plan(&c);
                let (al_a, al_b) = (al.start_station().max(pr.start()), al.end_station().min(pr.end()));
                let mut titles: Vec<String> = vec!["Survey control and alignment data".into()];
                for w in &p.windows {
                    titles.push(format!("Plan and profile STA {} to {}", crate::sheet::sta_label(w.s0), crate::sheet::sta_label(w.s1)));
                }
                let nxs = p.xs_stations.len().div_ceil(8);
                for i in 0..nxs {
                    titles.push(format!("Cross sections at standard interval ({} of {nxs})", i + 1));
                }
                titles.push("Summary of quantities".into());
                let spec: crate::frame::FrameSpec = self.frame.clone().unwrap_or_default();
                let rf = crate::frame::resolve(&spec, b.standard)?;
                let ids: Vec<String> = crate::frame::sheet_ids(&rf, &titles).into_iter().map(|i| format!("CS-{i}")).collect();
                let pp_ids: Vec<String> = ids[1..1 + p.windows.len()].to_vec();
                let mut sheets = vec![crate::cset::control_sheet(&c)];
                for k in 0..p.windows.len() {
                    sheets.push(crate::cset::plan_profile(&c, &p, k, &pp_ids));
                }
                sheets.extend(crate::cset::cross_sections(&c, &p));
                sheets.push(crate::cset::quantity_summary(&c, &p, &pp_ids));
                drop(c);
                let pages = sheets.len();
                for (i, d) in sheets.iter_mut().enumerate() {
                    if let Some(m) = d.meta.as_mut() {
                        m.page = i + 1;
                        m.pages = pages;
                    }
                }
                let head = self.log.as_ref().map(|l| l.head.clone()).unwrap_or_default();
                let all = self.all_checks();
                let info = SheetInfo {
                    project: b.project.clone(),
                    engineer: b.engineer_of_record.clone(),
                    standard: serde_json::to_value(b.standard).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
                    design_speed: b.design_speed_kmh,
                    audit_head: head,
                    checks_failed: all.iter().filter(|c| !c.pass && c.severity == "mandatory").count(),
                    checks_total: all.len(),
                    approved_seq: self.approved_seq,
                };
                let sheets: Vec<drawing::Drawing> = sheets.iter().enumerate().map(|(i, d)| crate::frame::compose(d, &rf, &info, &ids[i])).collect();
                let pdf = drawing::to_pdf(&sheets.iter().map(|d| (d, rf.w, rf.h)).collect::<Vec<_>>());
                let mut files = json!({});
                files["construction-set.pdf"] = json!(self.write("construction-set.pdf", &pdf)?);
                let mut reg = String::from("page,sheet_id,title,scale\n");
                for (i, d) in sheets.iter().enumerate() {
                    let n = format!("cs-{:02}.dxf", i + 1);
                    files[&n] = json!(self.write(&n, d.to_dxf().as_bytes())?);
                    let sv = format!("cs-{:02}.svg", i + 1);
                    files[&sv] = json!(self.write(&sv, d.to_svg(rf.w, rf.h).as_bytes())?);
                    let sc = d.meta.as_ref().map(|m| m.scale.clone()).unwrap_or_default();
                    reg.push_str(&format!("{},{},\"{}\",\"{}\"\n", i + 1, ids[i], titles[i], sc.replace('"', "\"\"")));
                }
                files["construction-register.csv"] = json!(self.write("construction-register.csv", reg.as_bytes())?);
                // set checks: continuous match lines, full coverage, standard scale
                let mut checks = vec![];
                let std_ok = if crate::units::cur().us() { crate::cset::US_PLAN.contains(&p.plan_scale) } else { crate::cset::SI_PLAN.contains(&p.plan_scale) };
                checks.push(json!({"id": "CS-SCALE", "pass": std_ok, "detail": format!("plan scale {}", crate::units::scale(p.plan_scale))}));
                let cont = p.windows.windows(2).all(|w| (w[0].s1 - w[1].s0).abs() < 1e-9);
                checks.push(json!({"id": "CS-MATCH", "pass": cont, "detail": "each match line ends one sheet and starts the next"}));
                let cover = p.windows.first().is_some_and(|w| (w.s0 - al_a).abs() < 1e-6) && p.windows.last().is_some_and(|w| (w.s1 - al_b).abs() < 1e-6);
                checks.push(json!({"id": "CS-COVER", "pass": cover, "detail": "plan-profile sheets cover the alignment"}));
                checks.push(json!({"id": "CS-INPUTS", "pass": p.missing_inputs.is_empty(), "detail": if p.missing_inputs.is_empty() { "all engineer inputs present".to_string() } else { format!("missing: {}", p.missing_inputs.join("; ")) }}));
                let out = json!({"sheets": pages, "plan": p, "sheet_ids": ids, "checks": checks, "files": files});
                let prov = self.log("agent", name, a, &out, &["construction plan conventions: match lines, standard engineering scales".into()])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "export_blender_scene" => {
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let alts: Vec<(String, Vec<[f64; 2]>)> = self
                    .route
                    .as_ref()
                    .and_then(|r| r["alternatives"].as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|r| {
                        let p: Vec<[f64; 2]> = serde_json::from_value(r["path"].clone()).ok()?;
                        Some((r["name"].as_str()?.to_string(), p))
                    })
                    .collect();
                let sc = crate::gltf::build(t, self.corridor.as_ref(), &self.networks, &alts);
                let o = sc.origin;
                // camera path along the design centreline (Blender axes: x east, y north, z up; local metres)
                let mut cam = vec![];
                if let (Some(al), Some(pr)) = (&self.alignment, &self.profile) {
                    let (s0, s1) = (al.start_station().max(pr.start()), al.end_station().min(pr.end()));
                    let n = ((s1 - s0) / 5.0).ceil().max(1.0) as usize;
                    for i in 0..=n {
                        let s = s0 + (s1 - s0) * i as f64 / n as f64;
                        if let (Some((x, y, _)), Some(z)) = (al.point_at(s), pr.elevation_at(s)) {
                            cam.push([x - o[0], y - o[1], z - o[2]]);
                        }
                    }
                }
                let (mut mn, mut mx) = ([f64::MAX; 3], [f64::MIN; 3]);
                for p in &t.pts {
                    for k in 0..3 {
                        mn[k] = mn[k].min(p[k] - o[k]);
                        mx[k] = mx[k].max(p[k] - o[k]);
                    }
                }
                let alt_eval: Vec<Value> = self
                    .route
                    .as_ref()
                    .and_then(|r| r["alternatives"].as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .map(|r| json!({"name": r["name"], "evaluation": r["evaluation"]}))
                    .collect();
                let meta = json!({"origin": {"e": o[0], "n": o[1], "z": o[2], "epsg": self.crs_epsg, "note": "Blender/glTF coordinates are metres relative to this origin; glTF y-up (x east, y up, z south), Blender after import z-up (x east, y north)"},
                    "bbox_local": [mn[0], mn[1], mn[2], mx[0], mx[1], mx[2]], "camera_path": cam, "eye_height_m": 1.2,
                    "meshes": sc.meshes.iter().map(|m| json!({"name": m.name, "material": crate::gltf::MATERIALS[m.material].0, "vertices": m.pos.len(), "triangles": m.idx.len() / 3})).collect::<Vec<_>>(),
                    "pavement_layers": self.corridor.as_ref().map(|c| c.assembly.pavement.iter().enumerate().map(|(i, l)| json!({"mesh": format!("{}{}: {} {:.0} mm", crate::gltf::LAYER_PREFIX, i + 1, l.name, l.thickness_m * 1000.0), "name": l.name, "thickness_m": l.thickness_m})).collect::<Vec<_>>()).unwrap_or_default(),
                    "ground_trimmed_mesh": self.corridor.as_ref().map(|_| crate::gltf::GROUND_TRIMMED),
                    "closure_mesh": self.corridor.as_ref().map(|_| crate::gltf::CLOSURE),
                    "alternatives": alt_eval, "selected": self.route_choice.as_ref().map(|r| r["selected"].clone())});
                let glb = crate::gltf::to_glb(&sc, json!({"georeference": meta["origin"]}));
                let mut files = json!({});
                files["scene.glb"] = json!(self.write("scene.glb", &glb)?);
                files["scene.json"] = json!(self.write("scene.json", serde_json::to_string_pretty(&meta).unwrap().as_bytes())?);
                files["blender_scene.py"] = json!(self.write("blender_scene.py", include_str!("../../../scripts/blender_scene.py").as_bytes())?);
                let out = json!({"files": files, "origin": meta["origin"], "meshes": meta["meshes"], "camera_path_points": cam.len(),
                    "render": "blender -b -P blender_scene.py -- <project dir>  (stills, flythrough.mp4, scene.usdc, scene.blend)", "purpose": "concept / preliminary visualisation only; not a design deliverable"});
                let prov = self.log("agent", name, a, &out, &["glTF 2.0 (Khronos)".into()])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "export_cad_model" => {
                let b = self.need_basis()?.clone();
                let file = a["file"].as_str().unwrap_or("civil3d-model.dxf").to_string();
                let spec: crate::frame::FrameSpec = self.frame.clone().unwrap_or_default();
                let rf = crate::frame::resolve(&spec, b.standard)?;
                let mut map = rf.spec.layer_map.clone();
                if let Some(m) = a.get("layer_map").and_then(|v| v.as_object()) {
                    for (k, v) in m {
                        if let Some(v) = v.as_str() {
                            map.insert(k.clone(), v.to_string());
                        }
                    }
                }
                let m = crate::cad3d::Model {
                    eg: self.tin.as_ref().map(|t| &t.0),
                    fg: self.corridor_fg.as_ref(),
                    al: self.alignment.as_ref(),
                    pr: self.profile.as_ref(),
                    cor: self.corridor.as_ref(),
                    nets: self.networks.iter().collect(),
                };
                let (dxf, st) = crate::cad3d::write(&m, &rf.profile, &map);
                let h = self.write(&file, dxf.as_bytes())?;
                // optional DWG through an installed ODA File Converter (not bundled)
                let dwg = match a.get("dwg_version").and_then(|v| v.as_str()) {
                    None => json!("not written: no DWG writer is bundled; convert with ODA File Converter or LibreDWG dxf2dwg"),
                    Some(ver) => {
                        let dir = self.dir()?;
                        let mut files = vec![file.clone()];
                        if a["dwg_include_sheets"].as_bool().unwrap_or(false) {
                            let mut sh: Vec<String> = std::fs::read_dir(&dir)
                                .map_err(|e| e.to_string())?
                                .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
                                .filter(|n| n.ends_with(".dxf") && (n.starts_with("sheet-") || n.starts_with("cs-")))
                                .collect();
                            sh.sort();
                            files.extend(sh);
                        }
                        let paths = crate::dwg::convert(&dir, &files, ver, "dwg")?;
                        let mut list = vec![];
                        for p in &paths {
                            let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
                            let rel = format!("dwg/{}", p.file_name().unwrap().to_string_lossy());
                            let hh = sha256_hex(&bytes);
                            self.files.retain(|f| f.0 != rel);
                            self.files.push((rel.clone(), hh.clone()));
                            list.push(json!({"file": rel, "sha256": hh, "magic": String::from_utf8_lossy(&bytes[..6.min(bytes.len())])}));
                        }
                        json!({"version": ver, "converter": "ODA File Converter (external, not bundled)", "files": list})
                    }
                };
                let out = json!({"file": file, "sha256": h, "format": "DXF R12 (AC1009), metres, world coordinates", "drafting_profile": rf.country, "stats": st, "epsg": self.crs_epsg,
                    "dwg": dwg, "note": "verified by re-reading with ezdxf in CI; not opened in Civil 3D"});
                let prov = self.log("agent", name, a, &out, &["DXF R12 reference (Autodesk DXF), layer names from the drafting profile".into()])?;
                Ok(json!({"result": out, "provenance": prov}))
            }
            "import_terrain" => {
                let p = a["path"].as_str().ok_or("path required")?;
                let crop = a.get("crop").and_then(|v| v.as_array()).map(|v| {
                    let g = |i: usize| v.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0);
                    [g(0), g(1), g(2), g(3)]
                });
                let o = crate::terrain::Options {
                    step: a["step"].as_u64().unwrap_or(1) as usize,
                    crop,
                    epsg: a["epsg"].as_u64().map(|x| x as u32),
                    target_epsg: a["target_epsg"].as_u64().map(|x| x as u32),
                    xy_units: a["xy_units"].as_str().map(String::from),
                    z_units: a["z_units"].as_str().map(String::from),
                    classes: a
                        .get("classes")
                        .and_then(|v| v.as_array())
                        .map(|v| v.iter().filter_map(|x| x.as_u64()).map(|x| x as u8).collect())
                        .unwrap_or(vec![2]),
                    max_points: a["max_points"].as_u64().unwrap_or(400_000) as usize,
                };
                let bytes = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
                let imp = crate::terrain::read_any(p, &o)?;
                self.points = imp
                    .points
                    .iter()
                    .enumerate()
                    .map(|(i, q)| points::SurveyPoint { id: format!("T{}", i + 1), e: q[0], n: q[1], z: q[2], desc: imp.format.clone() })
                    .collect();
                self.crs_epsg = imp.crs.target_epsg.filter(|_| imp.crs.xy_unit_m == 1.0);
                let mut out = serde_json::to_value(&imp).unwrap();
                out["file_sha256"] = json!(sha256_hex(&bytes));
                if a["build_surface"].as_bool().unwrap_or(true) {
                    let pts: Vec<[f64; 3]> = imp.points.clone();
                    let (t, st) = Tin::build(&pts, f(a, "max_edge_m"))?;
                    out["surface"] = serde_json::to_value(&st).unwrap();
                    self.tin = Some((t, st));
                    self.invalidate_corridor();
                }
                let prov = self.log(
                    "agent",
                    name,
                    a,
                    &out,
                    &["terrain import (GeoTIFF / ASCII grid / LAS / LAZ)".into(), out["crs"]["note"].as_str().unwrap_or("").to_string()],
                )?;
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
                let (pin, clamped, gmax) = crate::workflow::suggest_profile(&b, t, al, &st)?;
                let s = standards::ssd(&b);
                let proposal = serde_json::to_value(&pin).unwrap();
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
                let prov = self.log(
                    "agent",
                    name,
                    a,
                    &json!({"curves": se.curves.len(), "detail_sha256": h, "e": se.curves.iter().map(|c| c.e).collect::<Vec<_>>()}),
                    &[format!("{:?} superelevation", b.standard)],
                )?;
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
                let mut asm: Assembly = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                if asm.pavement.is_empty() {
                    if let Some(p) = &self.pavement {
                        asm.pavement = p
                            .layers
                            .iter()
                            .filter(|l| l.thickness_mm > 0.0)
                            .map(|l| corridor::Layer { name: l.name.clone(), thickness_m: l.thickness_mm / 1000.0 })
                            .collect();
                    }
                }
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
                let wcr = self.wall_criteria().ok();
                let ws: Option<corridor::WideningSpec> = match a.get("widening") {
                    Some(v) if !v.is_null() => Some(serde_json::from_value(v.clone()).map_err(|e| e.to_string())?),
                    _ => None,
                };
                let c = corridor::build_opts(
                    t,
                    al,
                    pr,
                    self.superelevation.as_ref(),
                    asm,
                    f(a, "interval_m").unwrap_or(10.0),
                    ws.as_ref(),
                    b.design_speed_kmh,
                    wcr.as_ref(),
                )?;
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
                    rules.push(wcr.as_ref().map_or("wall criteria missing".to_string(), |c| c.citation.clone()));
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
                let ew = quantities::earthwork(c, &mh, f(a, "bulking_factor").unwrap_or(1.0));
                let full = json!({"mass_haul": mh, "material_table": rows, "earthwork": ew});
                let h = self.write("quantities.json", full.to_string().as_bytes())?;
                let out = json!({"balance_stations": mh.balance_stations, "final_ordinate_m3": mh.final_ordinate, "haul_moment_m3m": mh.haul_moment_m3m, "rows": rows.len(), "borrow_compacted_m3": ew.borrow_compacted_m3, "waste_bank_m3": ew.waste_bank_m3, "waste_loose_m3": ew.waste_loose_m3, "overhaul_m3m": ew.overhaul_m3m, "segments": ew.segments, "detail_sha256": h});
                let prov = self.log("agent", name, a, &out, &["mass ordinate M = Σ(cut·factor − fill)".into()])?;
                self.mass_haul = Some(mh);
                self.materials = rows;
                self.earthwork = Some(ew);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "generate_drawings" => {
                let b = self.need_basis()?.clone();
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let head = self.log.as_ref().map(|l| l.head.clone()).unwrap_or_default();
                let all = self.all_checks();
                let info = SheetInfo {
                    project: b.project.clone(),
                    engineer: b.engineer_of_record.clone(),
                    standard: serde_json::to_value(b.standard).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
                    design_speed: b.design_speed_kmh,
                    audit_head: head.clone(),
                    checks_failed: all.iter().filter(|c| !c.pass && c.severity == "mandatory").count(),
                    checks_total: all.len(),
                    approved_seq: self.approved_seq,
                };
                let ds = Design {
                    tin: t,
                    al,
                    pr,
                    corridor: self.corridor.as_ref(),
                    networks: &self.networks,
                    gradings: &self.gradings,
                    intersections: &self.intersections,
                };
                // sheet order follows a design-stage plan set: cover/index/notes, route selection,
                // plan & profile, typicals, superelevation, pavement, cross sections, protection,
                // drainage profiles and details, earthwork/quantities, standard details
                let nxs = self.corridor.as_ref().map_or(0, sheet::n_xs_pages);
                let mut titles: Vec<String> = vec!["Cover, sheet index and general notes".into()];
                if self.route.is_some() {
                    titles.push("Route alternatives and selection".into());
                }
                titles.push("Plan and profile".into());
                if self.assembly.is_some() {
                    titles.push("Typical sections".into());
                }
                if self.superelevation.is_some() {
                    titles.push("Superelevation diagram".into());
                }
                if self.pavement.is_some() {
                    titles.push("Pavement and subgrade details".into());
                }
                for i in 0..nxs {
                    titles.push(format!("Cross sections ({} of {nxs})", i + 1));
                }
                if self.protection.is_some() && self.corridor.is_some() {
                    titles.push("Roadside protection layout".into());
                }
                for n in &self.networks {
                    titles.push(format!("Pipe plan/profile {}", n.name));
                }
                if !self.networks.is_empty() {
                    titles.push("Drainage details and structure schedule".into());
                }
                if self.corridor.is_some() && self.mass_haul.is_some() {
                    titles.push("Mass haul and quantities".into());
                }
                let exts = self.ext_sheets();
                for e in &exts {
                    titles.push(e.title.clone());
                }
                titles.push("Standard details atlas (references)".into());
                let pages = titles.len();
                let spec: crate::frame::FrameSpec = match a.get("frame") {
                    Some(f) if !f.is_null() => serde_json::from_value(f.clone()).map_err(|e| format!("frame: {e}"))?,
                    _ => self.frame.clone().unwrap_or_default(),
                };
                let rf = crate::frame::resolve(&spec, b.standard)?;
                let ids = crate::frame::sheet_ids(&rf, &titles);
                let index: Vec<(String, String)> = titles.iter().enumerate().map(|(i, t)| (t.clone(), ids[i].clone())).collect();
                let mut notes = vec![
                    format!("Design to {} - parameter register lists each value as verified (from the published standard), formula, or engineer input.", crate::sheetset::std_label(b.standard)),
                    "Values from copyrighted or paywalled tables are not shipped with the software; they are entered by the engineer and marked as engineer inputs in the parameter register and design report.".into(),
                    "Dimensions in metres unless noted; elevations in metres on the survey datum; stations in km+m along the control line.".into(),
                    "Existing ground from the supplied survey/terrain points (TIN). Verify on site before construction.".into(),
                    "Earthwork volumes by average end area with a composite-grid cross check; shrink/bulk factors and haul limits as stated in the earthwork report.".into(),
                    "Standard details are schematic references; construct to the cited agency standard drawings and specification.".into(),
                    "These drawings are generated by agent tools from the audit-logged design. Status: PRELIMINARY unless the engineer has recorded approval; sealing is the engineer's act.".into(),
                ];
                if let Some(rc) = &self.route_choice {
                    notes.push(format!(
                        "Selected route: {} (recorded with reasons in the audit log).",
                        rc["selected"].as_str().or(rc["alternative"].as_str()).unwrap_or("see route sheet")
                    ));
                }
                let mut sheets = vec![crate::sheetset::cover(&info, b.standard, &index, &notes, pages)];
                let mut pg = 2;
                if let Some(rt) = &self.route {
                    let sel = self
                        .route_choice
                        .as_ref()
                        .and_then(|r| r["selected"].as_str().or(r["alternative"].as_str()).map(String::from))
                        .unwrap_or_else(|| rt["recommendation"]["recommended"].as_str().unwrap_or("").to_string());
                    sheets.push(crate::sheetset::route_sheet(t, rt, &sel, &info, pg, pages));
                    pg += 1;
                }
                sheets.push(sheet::plan_profile(&ds, &info, pg, pages));
                pg += 1;
                if let Some(asm) = &self.assembly {
                    sheets.push(crate::sheetset::typical_sections(asm, b.normal_crown, &info, pg, pages));
                    pg += 1;
                }
                if let Some(se) = &self.superelevation {
                    sheets.push(crate::sheetset::superelevation_sheet(se, al, &info, pg, pages));
                    pg += 1;
                }
                if let Some(pv) = &self.pavement {
                    sheets.push(crate::sheetset::pavement_sheet(pv, &info, pg, pages));
                    pg += 1;
                }
                if let Some(c) = &self.corridor {
                    sheets.extend(sheet::cross_sections(c, &info, pg, pages));
                    pg += nxs;
                }
                if let (Some(pt), Some(c)) = (&self.protection, &self.corridor) {
                    sheets.push(crate::sheetset::protection_sheet(pt, c, &info, pg, pages));
                    pg += 1;
                }
                for n in self.networks.iter() {
                    sheets.push(sheet::pipe_profile(n, &info, pg, pages));
                    pg += 1;
                }
                if !self.networks.is_empty() {
                    sheets.push(crate::sheetset::drainage_details(&self.networks, &info, pg, pages));
                    pg += 1;
                }
                if let Some(c) = &self.corridor {
                    if let Some(mh) = &self.mass_haul {
                        sheets.push(sheet::mass_haul_sheet(mh, &self.materials, c, &self.networks, &self.gradings, &info, pg, pages));
                        pg += 1;
                    }
                }
                for e in &exts {
                    sheets.push(crate::ext::render(e, &info, pg, pages));
                    pg += 1;
                }
                sheets.push(crate::sheetset::details_atlas(b.standard, &info, pg, pages));
                debug_assert_eq!(sheets.len(), pages);
                let sheets: Vec<drawing::Drawing> = sheets.iter().enumerate().map(|(i, d)| crate::frame::compose(d, &rf, &info, &ids[i])).collect();
                let fchecks = crate::frame::check_set(&sheets, &rf, &ids, &info);
                let model = sheet::model(&ds, 1.0).to_dxf();
                let pdf = drawing::to_pdf(&sheets.iter().map(|d| (d, rf.w, rf.h)).collect::<Vec<_>>());
                let mut files = json!({});
                files["model.dxf"] = json!(self.write("model.dxf", model.as_bytes())?);
                files["sheets.pdf"] = json!(self.write("sheets.pdf", &pdf)?);
                let usu = crate::units::cur().us();
                let mut reg = String::from(if usu {
                    "page,sheet_id,title,revision,status,size,dxf,svg,scale,units\n"
                } else {
                    "page,sheet_id,title,revision,status,size,dxf,svg\n"
                });
                let rev = rf.spec.fields.revisions.last().map(|x| x.rev.clone()).unwrap_or_else(|| rf.profile.initial_revision.to_string());
                for (i, d) in sheets.iter().enumerate() {
                    let n = format!("sheet-{:02}.dxf", i + 1);
                    files[&n] = json!(self.write(&n, d.to_dxf().as_bytes())?);
                    let sv = format!("sheet-{:02}.svg", i + 1);
                    files[&sv] = json!(self.write(&sv, d.to_svg(rf.w, rf.h).as_bytes())?);
                    reg += &format!(
                        "{},{},{},{},{},{},{n},{sv}\n",
                        i + 1,
                        ids[i],
                        crate::ext::csvq(&titles[i]),
                        rev,
                        if info.approved_seq.is_some() && info.checks_failed == 0 { "approved" } else { "preliminary" },
                        rf.size
                    );
                    if usu {
                        reg.pop();
                        let sc = d.meta.as_ref().map(|m| crate::units::scale_text(&m.scale)).unwrap_or_default();
                        reg += &format!(",{},US customary\n", crate::ext::csvq(&sc));
                    }
                }
                files["sheet-register.csv"] = json!(self.write("sheet-register.csv", reg.as_bytes())?);
                let fail = fchecks.iter().filter(|c| !c.pass && c.severity == "mandatory").count();
                let mut fjson = json!({"frame": rf, "checks": fchecks, "mandatory_failures": fail});
                if usu {
                    let u = crate::units::cur();
                    fjson["checks"] = crate::units::result_to_us(&fjson["checks"], u);
                    fjson["units"] = crate::units::describe(&self.unit_spec(), false);
                    fjson["sheet_in"] = json!([rf.w / 25.4, rf.h / 25.4]);
                }
                files["drawing-standards.json"] = json!(self.write("drawing-standards.json", serde_json::to_string_pretty(&fjson).unwrap().as_bytes())?);
                self.frame_checks = fchecks.clone();
                let status = if info.approved_seq.is_some() && info.checks_failed == 0 {
                    "APPROVED IN AUDIT LOG (seal by engineer)"
                } else {
                    "PRELIMINARY - NOT FOR CONSTRUCTION"
                };
                let frame_summary = json!({"profile": rf.country, "size": rf.size, "orientation": rf.orientation, "layout_factor": rf.k, "sheet_ids": ids, "checks": self.frame_checks.len(), "mandatory_failures": fail, "advisory": self.frame_checks.iter().filter(|c| !c.pass && c.severity != "mandatory").map(|c| format!("{} {}: {}", c.id, c.item, c.element)).collect::<Vec<_>>(), "warnings": rf.warnings});
                let out = json!({"files": files, "pages": pages, "sheet_index": titles, "status": status, "audit_head_on_sheet": head, "drawing_standards": frame_summary});
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
                        h += &format!(
                            "<tr><td>{}</td><td>{:.1}</td><td>{:.4}</td><td>{}</td><td>{:.2}–{:.2}</td><td>{:.2}–{:.2}</td><td>{:.3}</td><td>{}</td></tr>",
                            c.curve,
                            c.radius,
                            c.e,
                            c.method,
                            c.a_in,
                            c.b_in,
                            c.b_out,
                            c.a_out,
                            c.relative_gradient * 100.0,
                            c.note
                        );
                    }
                    blocks.push(("Superelevation".to_string(), h + "</table>"));
                }
                if let Some(c) = &self.corridor {
                    let mut h = format!("<p>{} sample lines. Method: {}. <b>Cut {:.1} m³, fill {:.1} m³</b>.", c.sections.len(), c.method, c.cut_m3, c.fill_m3);
                    if let Some((cc, cf, n, cell)) = self.composite {
                        h += &format!(
                            " Composite cross-check ({cell} m grid, {n} cells): cut {cc:.1} m³ ({:+.2}%), fill {cf:.1} m³ ({:+.2}%).",
                            (c.cut_m3 - cc) / cc.max(1e-9) * 100.0,
                            (c.fill_m3 - cf) / cf.max(1e-9) * 100.0
                        );
                    }
                    h += "</p><ul>";
                    for (n, v) in &c.layer_m3 {
                        h += &format!("<li>{n}: {v:.1} m³</li>");
                    }
                    blocks.push(("Corridor and earthwork".to_string(), h + "</ul>"));
                }
                if let Some(mh) = &self.mass_haul {
                    blocks.push((
                        "Mass haul".to_string(),
                        format!(
                            "<p>Cut factor {:.2}. Balance stations: {}. Final ordinate {:.0} m³ (positive = surplus). Haul moment ∫|M|ds = {:.0} m³·m.</p>",
                            mh.cut_factor,
                            mh.balance_stations.iter().map(|s| sheet::sta_label(*s)).collect::<Vec<_>>().join(", "),
                            mh.final_ordinate,
                            mh.haul_moment_m3m
                        ),
                    ));
                }
                for g in &self.gradings {
                    blocks.push((
                        format!("Grading {}", g.name),
                        format!("<p>Cut {:.1} m³, fill {:.1} m³ ({} m composite grid). Warnings: {}</p>", g.cut_m3, g.fill_m3, g.cell_m, g.warnings.len()),
                    ));
                }
                for n in &self.networks {
                    let mut h = String::from("<table><tr><th>Pipe</th><th>From→To</th><th>L m</th><th>D m</th><th>S %</th><th>Inv up/dn</th><th>Q design m³/s</th><th>Q full</th><th>V full m/s</th><th>h/D</th><th>cover min m</th><th>HGL up/dn</th><th>regime</th></tr>");
                    for p in &n.pipes {
                        h += &format!("<tr><td>{}</td><td>{}→{}</td><td>{:.1}</td><td>{:.3}</td><td>{:.3}</td><td>{:.3}/{:.3}</td><td>{:.4}</td><td>{:.4}</td><td>{:.2}</td><td>{:.3}</td><td>{:.2}</td><td>{:.3}/{:.3}</td><td>{}</td></tr>", p.id, p.from, p.to, p.length, p.diameter_m, p.slope * 100.0, p.invert_up, p.invert_dn, p.q_design, p.q_full, p.v_full, p.depth_ratio, p.cover_up.min(p.cover_dn), p.hgl_up, p.hgl_dn, p.regime);
                    }
                    blocks.push((format!("Pipe network: {}", n.name), h + "</table><p>" + &n.hgl_method + "</p>"));
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
            "design_pavement" => {
                let b = self.need_basis()?.clone();
                let mut a2 = a.clone();
                if let Some(tr) = a.get("traffic").filter(|v| !v.is_null()) {
                    let g = |k: &str| tr.get(k).and_then(|x| x.as_f64()).ok_or(format!("traffic.{k} required"));
                    let n = crate::pavement::cumulative_traffic(
                        g("aadt")?,
                        g("hv_frac")?,
                        g("esa_per_hv")?,
                        g("growth")?,
                        g("years")?,
                        g("dir_factor")?,
                        g("lane_factor")?,
                    );
                    let n = if b.standard == standards::Standard::Uk { n / 1e6 } else { n };
                    a2["design_traffic"] = json!(n);
                    self.traffic = Some(
                        json!({"inputs": tr, "design_traffic": n, "unit": if b.standard == standards::Standard::Uk { "msa" } else if matches!(b.standard, standards::Standard::Au | standards::Standard::Nz) { "DESA" } else { "ESAL (W18)" }, "formula": "N = 365·AADT·HV·ESA/HV·DF·LF·((1+g)^n − 1)/g"}),
                    );
                }
                let inp: crate::pavement::PavementInput = serde_json::from_value(a2).map_err(|e| e.to_string())?;
                let pd = crate::pavement::design(b.standard, &inp)?;
                let out = serde_json::to_value(&pd).unwrap();
                let prov = self.log(
                    "agent",
                    name,
                    a,
                    &json!({"layers": pd.layers, "total_mm": pd.total_mm, "sn_required": pd.sn_required}),
                    std::slice::from_ref(&pd.citation),
                )?;
                self.pavement = Some(pd);
                Ok(json!({"result": out, "traffic": self.traffic, "provenance": prov}))
            }
            "generate_route_alternatives" => {
                let b = self.need_basis()?.clone();
                let (t, st) = self.tin.as_ref().ok_or("build_surface first")?;
                let asm = self.assembly.as_ref().ok_or("create_assembly first (alternatives are compared with the typical section)")?.clone();
                let rs = standards::resolve(&b)?;
                let p2 = |k: &str| -> crate::Result<[f64; 2]> {
                    let v = a[k].as_array().ok_or(format!("{k} required"))?;
                    Ok([v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)])
                };
                let (s0, s1) = (p2("start")?, p2("end")?);
                let cell = f(a, "cell_m").unwrap_or(10.0);
                let alts: Vec<crate::route::AltSpec> = match a.get("alternatives").filter(|v| !v.is_null()) {
                    Some(v) => serde_json::from_value(v.clone()).map_err(|e| e.to_string())?,
                    None => crate::route::default_alts(),
                };
                let rates: crate::workflow::UnitRates = match a.get("rates").filter(|v| !v.is_null()) {
                    Some(v) => serde_json::from_value(v.clone()).map_err(|e| e.to_string())?,
                    None => crate::workflow::UnitRates { excavation: 1.0, embankment: 1.0, borrow_waste: 1.5, wall_concrete: 30.0, pavement_m2: 0.0 },
                };
                let gmax = rs.g_desirable.unwrap_or(rs.g_max);
                let grid = crate::route::Grid::from_tin(t, cell, st.bbox);
                let r_pref = f(a, "radius_pref_m").unwrap_or(3.0 * rs.r_min);
                let ls = f(a, "spiral_m").unwrap_or(0.0);
                let tol = f(a, "simplify_tol_m").unwrap_or(15.0);
                let shrink = f(a, "shrink").unwrap_or(0.9);
                let wc = self.wall_criteria().ok();
                let hairpin_r = f(a, "hairpin_min_radius_m");
                let limits: crate::workflow::Limits = match a.get("limits").filter(|v| !v.is_null()) {
                    Some(v) => serde_json::from_value(v.clone()).map_err(|e| e.to_string())?,
                    None => crate::workflow::Limits::default(),
                };
                let mut found: Vec<Vec<[f64; 2]>> = vec![];
                let mut rows = vec![];
                self.route_inputs.clear();
                for spec in &alts {
                    let (path, cost, nexp) = match crate::route::least_cost(&grid, s0, s1, gmax, spec, &found) {
                        Ok(r) => r,
                        Err(e) if spec.hard_grade => {
                            rows.push(json!({"name": spec.name, "weights": spec, "evaluation": {"error": e}}));
                            continue;
                        }
                        Err(e) => return Err(e),
                    };
                    let overlap_pct = found.iter().map(|q| crate::route::overlap(&path, q, 2.0 * cell)).fold(0.0, f64::max) * 100.0;
                    found.push(path.clone());
                    let (plen, pg) = crate::route::path_stats(t, &path);
                    let mut pis = crate::route::clean_pis(crate::route::simplify(&path, tol), 4.0, 40.0);
                    // switchback alternatives keep their hairpins when the engineer has entered a
                    // hairpin minimum radius (design exception); others drop sub-minimum PIs
                    let r_floor = if spec.hard_grade { hairpin_r.unwrap_or(rs.r_min) } else { rs.r_min };
                    // spirals sized to the superelevation development length of each radius
                    let req = |r: f64| crate::superelev::required_development(&b, r).unwrap_or(0.0);
                    // the original fit (preferred spiral clamped to the deflection) is kept whenever all of
                    // its spirals already meet the required development; otherwise curves are refit
                    let pis0 = pis.clone();
                    let mut radii = crate::route::fit_radii(&pis, r_pref, ls, 0.45);
                    for _ in 0..60 {
                        match radii.iter().position(|r| *r < r_floor) {
                            Some(k) if pis.len() > 3 => {
                                pis.remove(k + 1);
                                pis = crate::route::clean_pis(pis, 4.0, 40.0);
                                radii = crate::route::fit_radii(&pis, r_pref, ls, 0.45);
                            }
                            _ => break,
                        }
                    }
                    let mut spirals: Vec<f64> = radii
                        .iter()
                        .enumerate()
                        .map(|(k, r)| (ls.min(0.9 * r * crate::route::deflection(pis[k], pis[k + 1], pis[k + 2])) / 5.0).floor() * 5.0)
                        .collect();
                    let short = radii.iter().zip(&spirals).any(|(r, l)| *r < r_floor || (*l > 0.0 && *l + 1e-9 < req(*r)));
                    if short {
                        pis = pis0;
                        (radii, spirals) = crate::route::fit_curves(&pis, r_pref, ls, 0.45, r_floor, &req);
                    }
                    // drop PIs where no radius ≥ the floor fits with its transitions, and refit
                    for _ in 0..60 {
                        match radii.iter().position(|r| *r < r_floor) {
                            Some(k) if short && pis.len() > 3 => {
                                pis.remove(k + 1);
                                pis = crate::route::clean_pis(pis, 4.0, 40.0);
                                (radii, spirals) = crate::route::fit_curves(&pis, r_pref, ls, 0.45, r_floor, &req);
                            }
                            _ => break,
                        }
                    }
                    // a single remaining PI that cannot take its transitions: largest radius that fits
                    // with a shortened spiral; the SE-IN-LS check then fails and says why
                    let mut transition_notes = vec![];
                    if radii.iter().any(|r| *r < r_floor) {
                        let fr = crate::route::fit_radii(&pis, r_pref, ls, 0.45);
                        for k in 0..radii.len() {
                            if radii[k] < r_floor {
                                let d = crate::route::deflection(pis[k], pis[k + 1], pis[k + 2]);
                                radii[k] = fr[k];
                                spirals[k] = (ls.min(0.9 * fr[k] * d) / 5.0).floor() * 5.0;
                                transition_notes.push(format!(
                                    "curve {}: no radius from {:.0} m to {:.0} m fits its transitions in the available tangents; kept R {:.0} m with Ls {:.0} m, below the {:.1} m required development (design exception, engineer to resolve)",
                                    k + 1,
                                    r_floor,
                                    4.0 * r_pref,
                                    fr[k],
                                    spirals[k],
                                    req(fr[k])
                                ));
                            }
                        }
                    }
                    let ai = crate::align::AlignmentInput {
                        name: spec.name.clone(),
                        pis: pis.clone(),
                        radii: radii.clone(),
                        spirals: spirals.clone(),
                        start_station: 0.0,
                    };
                    let ev = crate::workflow::evaluate(
                        &b,
                        t,
                        &asm,
                        ai.clone(),
                        f(a, "vpi_spacing_m").unwrap_or(150.0),
                        f(a, "interval_m").unwrap_or(20.0),
                        shrink,
                        &rates,
                        wc.as_ref(),
                    );
                    let ev = ev.map(|(mut e, _)| {
                        if spec.hard_grade {
                            if let Some(h) = hairpin_r {
                                let mut ex = vec![];
                                e.failed_checks.retain(|c| {
                                    let hp = c.starts_with("H-RMIN") || c.starts_with("H-RDES") || c.starts_with("H-LS");
                                    if hp {
                                        ex.push(format!("{c} accepted as hairpin design exception (engineer-entered minimum radius {h:.0} m)"));
                                    }
                                    !hp
                                });
                                e.design_exceptions = ex;
                            }
                        }
                        crate::workflow::apply_limits(&mut e, &limits);
                        e
                    });
                    self.route_inputs.push((spec.name.clone(), ai));
                    rows.push(json!({"name": spec.name, "weights": spec, "path": path, "pis": pis, "search_cost": cost, "nodes_expanded": nexp, "path_length_m": plen, "max_ground_grade_pct": pg, "overlap_with_earlier_pct": overlap_pct, "duplicate_of_earlier": overlap_pct > 90.0, "transition_notes": transition_notes, "fitted_radii": radii, "spirals": spirals,
                        "evaluation": ev.as_ref().map(|e| serde_json::to_value(e).unwrap()).unwrap_or_else(|e| json!({"error": e}))}));
                }
                // infeasibility: every alternative fails a mandatory check or could not be built
                // (physical: search failure, maximum grade or earthwork/wall limits; other geometry
                // failures such as superelevation runoff remain fixable and do not stop the run)
                let ok = |r: &Value| {
                    r["evaluation"].get("error").is_none()
                        && !r["evaluation"]["failed_checks"]
                            .as_array()
                            .is_some_and(|v| v.iter().filter_map(|x| x.as_str()).any(|c| c.starts_with("EW-") || c.starts_with("V-GMAX")))
                };
                if !rows.iter().any(ok) && !a["allow_infeasible"].as_bool().unwrap_or(false) {
                    let zs = t.elevation_at(s0[0], s0[1]).unwrap_or(f64::NAN);
                    let ze = t.elevation_at(s1[0], s1[1]).unwrap_or(f64::NAN);
                    let d = ((s1[0] - s0[0]).powi(2) + (s1[1] - s0[1]).powi(2)).sqrt();
                    let why: Vec<String> = rows
                        .iter()
                        .map(|r| {
                            let e = &r["evaluation"];
                            match e.get("error") {
                                Some(x) => format!("{}: {}", r["name"].as_str().unwrap_or(""), x.as_str().unwrap_or("")),
                                None => {
                                    let fc: Vec<&str> =
                                        e["failed_checks"].as_array().map(|v| v.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
                                    let phys: Vec<&str> = fc.iter().copied().filter(|c| c.starts_with("EW-") || c.starts_with("V-GMAX")).collect();
                                    format!(
                                        "{}: {} (+{} other mandatory geometry failures)",
                                        r["name"].as_str().unwrap_or(""),
                                        phys.join("; "),
                                        fc.len() - phys.len()
                                    )
                                }
                            }
                        })
                        .collect();
                    let full = json!({"alternatives": rows, "infeasible": true});
                    self.write("route-alternatives.json", full.to_string().as_bytes())?;
                    return Err(format!(
                        "INFEASIBLE: every alternative fails the maximum grade or the earthwork/wall height limits, or could not be found. Elevation difference {:.1} m over {:.0} m straight distance needs at least {:.0} m of road at the {:.1} % maximum grade. Earthwork limits: fill {:.1} m, cut {:.1} m, wall {:.1} m ({}). Per alternative: {}. Options: enlarge the surface so switchbacks fit, enter hairpin_min_radius_m (design exception), raise limits after engineering review, or move the end points. Detail written to route-alternatives.json; pass allow_infeasible=true to inspect the alternatives anyway.",
                        (ze - zs).abs(), d, (ze - zs).abs() / (gmax / 100.0), gmax, limits.max_fill_m, limits.max_cut_m, limits.max_wall_m, limits.source, why.join(" | ")
                    ));
                }
                // recommendation: fewest mandatory failures, then lowest cost proxy
                let key = |r: &Value| {
                    (r["evaluation"]["geometry_mandatory_failures"].as_u64().unwrap_or(999), r["evaluation"]["cost_proxy"].as_f64().unwrap_or(f64::INFINITY))
                };
                let best = rows.iter().min_by(|x, y| key(x).partial_cmp(&key(y)).unwrap()).cloned().ok_or("no alternatives")?;
                let mut reasons = vec![];
                for r in &rows {
                    if r["name"] == best["name"] {
                        continue;
                    }
                    let (e, eb) = (&r["evaluation"], &best["evaluation"]);
                    if e.get("error").is_some() {
                        reasons.push(format!("{} rejected: {}", r["name"].as_str().unwrap(), e["error"]));
                        continue;
                    }
                    reasons.push(format!("vs {}: cost proxy {:.0} vs {:.0} ({:+.1}%), length {:.0} vs {:.0} m, cut+fill {:.0} vs {:.0} m³, imbalance {:.0} vs {:.0} m³, wall concrete {:.0} vs {:.0} m³, max grade {:.1} vs {:.1} %, mandatory failures {} vs {}",
                        r["name"].as_str().unwrap(), eb["cost_proxy"].as_f64().unwrap_or(0.0), e["cost_proxy"].as_f64().unwrap_or(0.0), (eb["cost_proxy"].as_f64().unwrap_or(0.0) / e["cost_proxy"].as_f64().unwrap_or(1.0) - 1.0) * 100.0,
                        eb["length_m"].as_f64().unwrap_or(0.0), e["length_m"].as_f64().unwrap_or(0.0), eb["cut_m3"].as_f64().unwrap_or(0.0) + eb["fill_m3"].as_f64().unwrap_or(0.0), e["cut_m3"].as_f64().unwrap_or(0.0) + e["fill_m3"].as_f64().unwrap_or(0.0),
                        eb["imbalance_m3"].as_f64().unwrap_or(0.0), e["imbalance_m3"].as_f64().unwrap_or(0.0), eb["wall_concrete_m3"].as_f64().unwrap_or(0.0), e["wall_concrete_m3"].as_f64().unwrap_or(0.0), eb["max_design_grade_pct"].as_f64().unwrap_or(0.0), e["max_design_grade_pct"].as_f64().unwrap_or(0.0), eb["geometry_mandatory_failures"], e["geometry_mandatory_failures"]));
                }
                let rec = json!({"recommended": best["name"], "rule": "fewest mandatory geometry failures, then lowest cost proxy (unit rates are agent placeholders unless the engineer enters rates)", "reasons": reasons, "rates": rates, "grid_cell_m": cell, "max_grade_pct_used": gmax, "r_min_m": rs.r_min});
                let full = json!({"alternatives": rows, "recommendation": rec});
                let h = self.write("route-alternatives.json", full.to_string().as_bytes())?;
                let summary: Vec<Value> =
                    rows.iter().map(|r| json!({"name": r["name"], "pis": r["pis"].as_array().map_or(0, |p| p.len()), "overlap_with_earlier_pct": r["overlap_with_earlier_pct"], "duplicate_of_earlier": r["duplicate_of_earlier"], "evaluation": r["evaluation"]})).collect();
                let prov = self.log(
                    "agent",
                    name,
                    a,
                    &json!({"alternatives": summary, "recommendation": rec, "detail_sha256": h}),
                    &["least-cost path (heading-constrained Dijkstra) + full corridor evaluation".into()],
                )?;
                self.route = Some(full);
                Ok(json!({"result": {"alternatives": summary, "recommendation": rec, "file": "route-alternatives.json"}, "provenance": prov}))
            }
            "select_route" => {
                let alt = a["alternative"].as_str().ok_or("alternative required")?;
                let actor = a["actor"].as_str().unwrap_or("agent").to_string();
                let ai = self.route_inputs.iter().find(|r| r.0 == alt).map(|r| r.1.clone()).ok_or("unknown alternative; run generate_route_alternatives")?;
                let mut ai = ai;
                ai.name = "CL".into();
                let al = Alignment::build(ai.clone())?;
                let reasons = a
                    .get("reasons")
                    .cloned()
                    .filter(|v| !v.is_null())
                    .or_else(|| self.route.as_ref().map(|r| r["recommendation"]["reasons"].clone()))
                    .unwrap_or(json!([]));
                let out =
                    json!({"selected": alt, "by": actor, "reasons": reasons, "length_m": al.length, "pis": ai.pis, "radii": ai.radii, "spirals": ai.spirals});
                let prov = self.log(&actor, name, a, &out, &["route selection record".into()])?;
                self.route_choice = Some(out.clone());
                self.alignment = Some(al);
                self.invalidate_from_alignment();
                Ok(json!({"result": out, "provenance": prov}))
            }
            "optimize_profile" => {
                let b = self.need_basis()?.clone();
                let (t, _) = self.tin.as_ref().ok_or("build_surface first")?;
                let al = self.alignment.as_ref().ok_or("create_alignment first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let asm = self.assembly.as_ref().ok_or("create_assembly first")?;
                let steps: Vec<f64> =
                    a.get("steps_m").and_then(|v| v.as_array()).map(|v| v.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or(vec![2.0, 1.0, 0.5]);
                let wc = self.wall_criteria().ok();
                let r = crate::workflow::optimize_profile(
                    &b,
                    t,
                    al,
                    self.superelevation.as_ref(),
                    asm,
                    &pr.input,
                    f(a, "interval_m").unwrap_or(20.0),
                    f(a, "shrink").unwrap_or(0.9),
                    f(a, "w_volume").unwrap_or(0.05),
                    f(a, "w_wall").unwrap_or(5.0),
                    f(a, "w_haul").unwrap_or(0.0),
                    &steps,
                    wc.as_ref(),
                )?;
                let p = Profile::build(r.after.clone())?;
                let out = json!({"objective_before": r.objective_before, "objective_after": r.objective_after, "before": r.before_eval, "after": r.after_eval, "evaluations": r.evaluations, "accepted_moves": r.steps.len(), "vpis_after": r.after.vpis, "curve_lengths_after": r.after.curve_lengths, "method": r.method});
                let h = self.write("profile-optimisation.json", serde_json::to_string(&r).unwrap().as_bytes())?;
                let prov = self.log("agent", name, a, &json!({"summary": out, "detail_sha256": h}), std::slice::from_ref(&r.method))?;
                self.profile = Some(p);
                self.optimisation = Some(r);
                self.invalidate_corridor();
                Ok(json!({"result": out, "provenance": prov}))
            }
            "design_protection" => {
                let b = self.need_basis()?.clone();
                let c = self.corridor.as_ref().ok_or("build_corridor first")?;
                let pr = self.profile.as_ref().ok_or("create_profile first")?;
                let inp: crate::protection::ProtectionInput = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
                let g = |s: f64| {
                    let h = 1.0;
                    (pr.elevation_at(s + h).unwrap_or(0.0) - pr.elevation_at(s - h).unwrap_or(0.0)) / (2.0 * h)
                };
                let p = crate::protection::design(b.standard, c, &g, &inp)?;
                let h = self.write("protection.json", serde_json::to_string(&p).unwrap().as_bytes())?;
                let fails = p.checks.iter().filter(|c| !c.pass && c.severity == "mandatory").count();
                let out = json!({"hazards": p.hazards.len(), "barriers": p.barriers, "ditches": p.ditches.iter().map(|d| json!({"side": d.side, "from": d.from, "to": d.to, "q_m3s": d.q_max_m3s, "depth_m": d.depth_m, "v_mps": d.velocity_mps, "tau_d": d.tau_d, "lining": d.lining, "tau_p": d.tau_p})).collect::<Vec<_>>(), "slope_protection": p.slope_protection, "mandatory_failures": fails, "notes": p.notes, "detail_sha256": h});
                let mut rules: Vec<String> = p.checks.iter().map(|c| c.citation.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                rules.push(crate::protection::barrier_cite(b.standard).into());
                let prov = self.log("agent", name, a, &out, &rules)?;
                self.protection = Some(p);
                Ok(json!({"result": out, "provenance": prov}))
            }
            "record_departure" => {
                let actor = a["actor"].as_str().unwrap_or("agent").to_string();
                let e = json!({"check_id": a["check_id"], "element": a["element"], "kind": a.get("kind").cloned().unwrap_or(json!("departure")), "justification": a["justification"], "recorded_by": actor});
                let prov = self.log(&actor, name, a, &e, &[])?;
                self.departures.push(e.clone());
                Ok(json!({"result": e, "provenance": prov}))
            }
            "generate_deliverables" => {
                let files = crate::deliver::write_all(self, a["stage"].as_str().unwrap_or("preliminary"))?;
                let mut out = json!({});
                for (n, bytes) in files {
                    out[&n] = json!(self.write(&n, &bytes)?);
                }
                let prov = self.log("agent", name, a, &json!({"files": out}), &[])?;
                Ok(json!({"result": {"files": out}, "provenance": prov}))
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
