//! Sheet frames, title blocks and drafting (CAD) standards for the Five Eyes countries.
//!
//! The sheet generators lay out every drawing on a fixed A1 layout (841 × 594 mm, drawing
//! content in `LAYOUT`). `compose` places that layout on any sheet size (ISO A0–A4, ANSI A–E,
//! ARCH A–E1 or a custom size), in either orientation, scaled uniformly into the drawing space
//! of the selected frame, and then draws the frame, ISO 5457 grid references and centring marks
//! (where the profile uses them), the title block, revision table, sign-off boxes, engineer seal
//! box, north arrow, scale bar and logo placeholder. Layers are renamed to the profile's layer
//! convention and given line weights and line types.
//!
//! Every drafting parameter carries the same provenance as the design parameters:
//! `verified` (read from a public source, cited), `formula` (a convention or rule built from a
//! cited source) or `engineer` (agency CAD manuals, state/provincial rules and copyrighted
//! content: a default is shipped so drawings render, but the engineer must confirm it against
//! the agency's CAD manual or seed files). No table is copied from a copyrighted standard.
use crate::drawing::{Drawing, Ent, LayerStyle, SheetMeta};
use crate::sheet::{wrap, SheetInfo, SHEET_H, SHEET_W};
use crate::standards::{chk, Check, Standard};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Content box of the A1 layout used by all sheet generators (mm).
pub const LAYOUT: [f64; 4] = [12.0, 8.0, SHEET_W - 155.0, SHEET_H - 8.0];

pub const ISO5457: &str = "ISO 5457:1999+A1:2010 Technical product documentation - Sizes and layout of drawing sheets, 4.2-4.4";
pub const ISO7200: &str = "ISO 7200:2004 Data fields in title blocks and document headers, Tables 1-3 and clause 6";
pub const ISO216: &str = "ISO 216:2007 trimmed sizes, A series";
pub const ASME_Y141: &str = "ASME Y14.1-2020 Decimal inch drawing sheet size and format (ANSI A-E)";
pub const ARCH: &str = "ASME Y14.1 / ANSI architectural sheet sizes (ARCH A-E1)";
pub const ISO128: &str = "ISO 128-20 / ISO 128-2:2020 line widths series 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1, 1.4, 2 mm; thick:thin 2:1";
pub const ISO3098: &str = "ISO 3098-0 nominal text heights 1.8, 2.5, 3.5, 5, 7, 10, 14, 20 mm";
pub const ISO5455: &str = "ISO 5455:1979 recommended scales 1:2, 1:5, 1:10 and multiples of 10";
pub const NCS_LAYER: &str = "US National CAD Standard, AIA CAD Layer Guidelines: Discipline-Major[-Minor[-Minor]][-Status], major/minor groups 4 characters";
pub const NCS_UDS: &str = "US National CAD Standard, UDS Module 1 Sheet Identification: discipline designator + sheet type digit (0 general, 1 plans, 2 elevations, 3 sections, 4 large-scale views, 5 details, 6 schedules and diagrams, 7-8 user defined, 9 3D) + 2-digit sequence 01-99";
pub const UKNA: &str = "BS EN ISO 19650-2:2018 UK National Annex (2021): Project-Originator-Functional-Spatial-Form-Discipline-Number; status codes S0-S4, S6, S7, A1..An, B1..Bn, CR; revisions P01.. (preliminary) and C01.. (contractual)";
pub const BS1192: &str =
    "BS 1192:2007+A2:2016 layer naming (Role-Classification-Presentation_Description, presentation M/T/D/H/P), carried into BS EN ISO 19650 practice";

#[derive(Debug, Clone, Serialize)]
pub struct DParam {
    pub name: String,
    pub value: String,
    /// "verified" | "formula" | "engineer"
    pub status: String,
    pub source: String,
}

fn dp(name: &str, value: &str, status: &str, source: &str) -> DParam {
    DParam { name: name.into(), value: value.into(), status: status.into(), source: source.into() }
}

/// (name, long side mm, short side mm, family source)
pub const SIZES: &[(&str, f64, f64, &str)] = &[
    ("ISO A0", 1189.0, 841.0, ISO216),
    ("ISO A1", 841.0, 594.0, ISO216),
    ("ISO A2", 594.0, 420.0, ISO216),
    ("ISO A3", 420.0, 297.0, ISO216),
    ("ISO A4", 297.0, 210.0, ISO216),
    ("ANSI A", 279.4, 215.9, ASME_Y141),
    ("ANSI B", 431.8, 279.4, ASME_Y141),
    ("ANSI C", 558.8, 431.8, ASME_Y141),
    ("ANSI D", 863.6, 558.8, ASME_Y141),
    ("ANSI E", 1117.6, 863.6, ASME_Y141),
    ("ARCH A", 304.8, 228.6, ARCH),
    ("ARCH B", 457.2, 304.8, ARCH),
    ("ARCH C", 609.6, 457.2, ARCH),
    ("ARCH D", 914.4, 609.6, ARCH),
    ("ARCH E", 1219.2, 914.4, ARCH),
    ("ARCH E1", 1066.8, 762.0, ARCH),
];

/// Canonical size name ("A1" → "ISO A1") and dimensions.
pub fn size_lookup(name: &str) -> Option<(&'static str, f64, f64)> {
    let n = name.trim().to_uppercase();
    SIZES.iter().find(|s| s.0 == n || s.0.strip_prefix("ISO ") == Some(n.as_str())).map(|s| (s.0, s.1, s.2))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LayerFmt {
    /// US NCS: C-ROAD-CNTR
    Ncs,
    /// ISO 13567 / BS 1192 style: C-00-M_Alignment
    Iso,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SheetIdFmt {
    Uds,
    Uk19650,
    Agency,
}

/// Drafting profile of one country (defaults; agency-specific items are engineer inputs).
#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub code: &'static str,
    pub name: &'static str,
    pub default_size: &'static str,
    /// left, right, top, bottom (mm)
    pub margins: [f64; 4],
    pub min_margins: [f64; 4],
    pub tb_width: f64,
    pub grid_reference: bool,
    pub centring_marks: bool,
    pub frame_line_mm: f64,
    pub min_text_mm: f64,
    pub text_mm: f64,
    pub heading_mm: f64,
    pub title_mm: f64,
    /// line weights: extra thin, thin, medium, thick (mm)
    pub weights: [f64; 4],
    pub layer_fmt: LayerFmt,
    pub sheet_id: SheetIdFmt,
    pub initial_revision: &'static str,
    pub status_code: &'static str,
    pub seal_label: &'static str,
    pub seal_rule: &'static str,
    pub required_fields: Vec<&'static str>,
    pub preferred_scales: Vec<f64>,
    pub params: Vec<DParam>,
}

pub fn country(s: &str) -> Option<Standard> {
    match s.trim().to_uppercase().as_str() {
        "US" | "USA" | "US_AASHTO_2018" => Some(Standard::Us),
        "CA" | "CAN" | "CANADA" | "CA_TAC_2017" => Some(Standard::Ca),
        "UK" | "GB" | "UK_DMRB" => Some(Standard::Uk),
        "AU" | "AUS" | "AU_AUSTROADS" => Some(Standard::Au),
        "NZ" | "NZ_AUSTROADS_NZTA" => Some(Standard::Nz),
        _ => None,
    }
}

pub fn code(s: Standard) -> &'static str {
    match s {
        Standard::Us => "US",
        Standard::Ca => "CA",
        Standard::Uk => "UK",
        Standard::Au => "AU",
        Standard::Nz => "NZ",
    }
}

fn iso_scales() -> Vec<f64> {
    let mut v = vec![];
    for e in 0..5 {
        for m in [1.0, 2.0, 5.0] {
            v.push(m * 10f64.powi(e));
        }
    }
    v
}

fn iso_common(v: &mut Vec<DParam>) {
    v.push(dp("Sheet sizes (ISO A series)", "A0 1189x841, A1 841x594, A2 594x420, A3 420x297, A4 297x210 mm", "verified", ISO216));
    v.push(dp("Borders", "left 20 mm (filing margin, includes frame), other edges 10 mm", "verified", ISO5457));
    v.push(dp("Drawing-space frame line", "continuous 0.7 mm", "verified", ISO5457));
    v.push(dp("Centring marks", "4 marks on the axes of symmetry of the trimmed sheet, 0.7 mm, into the drawing space", "verified", ISO5457));
    v.push(dp(
        "Grid reference system",
        "fields 50 mm long measured from the centring marks; capital letters top-down (I and O not used), numerals left-right, 3.5 mm characters, 0.35 mm lines",
        "verified",
        ISO5457,
    ));
    v.push(dp("Title block position", "bottom right corner of the drawing space (A0-A3 landscape; A4 at the lower edge)", "verified", ISO5457));
    v.push(dp("Title block width", "180 mm", "verified", ISO7200));
    v.push(dp(
        "Mandatory title block fields",
        "legal owner, identification number, date of issue, segment/sheet number, title, approval person, creator, document type",
        "verified",
        ISO7200,
    ));
    v.push(dp("Revision index", "letters A-Z then AA, AB ... (I and O avoided) or numerals 1, 2, 3", "verified", ISO7200));
    v.push(dp("Line widths", "ISO series; this profile uses 0.18, 0.25, 0.35, 0.5 mm (thick:thin 2:1) and 0.7 mm for the frame", "verified", ISO128));
    v.push(dp(
        "Line types",
        "continuous (design), dashed (existing ground, sight lines, hazards), long-dashed dotted (centre lines, ROW and property lines), dotted (energy grade line)",
        "formula",
        "ISO 128 line types applied to road drawing content (assignment by civil-mcp)",
    ));
    v.push(dp("Text heights", "2.5 mm minimum in the title block, 3.5 mm values, 5 mm headings, 7 mm sheet number", "formula", ISO3098));
    v.push(dp("Scales", "1:1, 1:2, 1:5, 1:10 ... 1:10000; other layout scales are flagged as advisory (FR-12b)", "verified", ISO5455));
}

pub fn profile(s: Standard) -> Profile {
    let mut params = vec![];
    match s {
        Standard::Us => {
            params.push(dp("Sheet sizes (ANSI)", "A 8.5x11, B 11x17, C 17x22, D 22x34, E 34x44 in", "verified", ASME_Y141));
            params.push(dp("Sheet sizes (ARCH)", "A 9x12, B 12x18, C 18x24, D 24x36, E 36x48, E1 30x42 in", "verified", ARCH));
            params.push(dp(
                "Default sheet",
                "ANSI D 22x34 in full size (half size 11x17 = ANSI B)",
                "engineer",
                "State DOT CADD manual (e.g. Caltrans CADD Users Manual, FDOT CADD Manual): confirm the agency plan sheet size",
            ));
            params.push(dp("Margins", "left 1.5 in (binding), others 0.5 in", "engineer", "State DOT CADD manual / agency border cell"));
            params.push(dp(
                "Layer naming",
                "NCS format Discipline-Major-Minor, e.g. C-ROAD-CNTR, V-TOPO-MAJR, C-STRM-PIPE, G-ANNO-TTLB",
                "verified",
                NCS_LAYER,
            ));
            params.push(dp(
                "Layer names used",
                "civil-mcp mapping of its layers to NCS major/minor groups (user-defined minor groups where none is predefined)",
                "formula",
                NCS_LAYER,
            ));
            params.push(dp(
                "DOT level/layer library",
                "agency seed/level library (MicroStation DGNLIB or Civil 3D template) replaces the NCS names via layer_map",
                "engineer",
                "Caltrans CADD Users Manual; FDOT CADD Production Criteria Handbook",
            ));
            params.push(dp(
                "Sheet identification",
                "discipline + sheet type + sequence, e.g. G-001 cover, C-101 plan, C-301 sections, C-501 details, C-601 schedules",
                "verified",
                NCS_UDS,
            ));
            params.push(dp("Line weights", "0.18 / 0.25 / 0.35 / 0.5 mm, frame 0.7 mm", "engineer", "US NCS Plotting Guidelines; agency pen table"));
            params.push(dp(
                "Line types",
                "continuous design, dashed existing, centre (dash-dot) alignment and ROW",
                "formula",
                "NCS / agency symbology convention",
            ));
            params.push(dp("Minimum text height", "2.5 mm (0.10 in) plotted at full size", "engineer", "State DOT CADD manual (half-size legibility rule)"));
            params.push(dp(
                "Scales",
                "engineering scales 1 in = 10, 20, 30, 40, 50, 60, 100 ft (1:120 ... 1:1200); metric 1:n with n in 1, 2, 2.5, 5 x 10^k",
                "engineer",
                "State DOT CADD manual",
            ));
            params.push(dp(
                "Units",
                "SI by default; US customary (ft, CY, SY, LF, 100 ft stations, engineering scales) with set_units",
                "engineer",
                "Agency policy",
            ));
            params.push(dp("Revision block", "Rev / date / description / by; preliminary issues lettered (A, B ...)", "engineer", "State DOT CADD manual"));
            params.push(dp(
                "Seal",
                "PE seal, signature and date of the licensed engineer in responsible charge on final sheets; preliminary sheets carry no seal and a PRELIMINARY - NOT FOR CONSTRUCTION legend",
                "engineer",
                "State engineering board rules (state specific)",
            ));
        }
        Standard::Ca => {
            iso_common(&mut params);
            params.push(dp(
                "Default sheet",
                "ISO A1 landscape",
                "engineer",
                "Provincial CADD standard (e.g. Ontario MTO CADD standards; BC MoTI, Alberta drafting guidelines)",
            ));
            params.push(dp(
                "Layer naming",
                "NCS-style names by default; provincial level libraries via layer_map",
                "engineer",
                "Provincial CADD standard (e.g. MTO)",
            ));
            params.push(dp(
                "Sheet identification",
                "discipline + type + sequence (UDS style) by default",
                "engineer",
                "Provincial CADD standard / owner contract numbering",
            ));
            params.push(dp("Units", "metric (SI)", "formula", "TAC Geometric Design Guide for Canadian Roads (metric)"));
            params.push(dp(
                "Seal",
                "Professional engineer stamp/seal and signature; permit/certificate of authorization where required",
                "engineer",
                "Provincial engineering regulator rules (PEO, APEGA, EGBC, OIQ ...)",
            ));
            params.push(dp(
                "Language",
                "English only in this profile; French or bilingual title block where the owner requires",
                "engineer",
                "Owner requirement",
            ));
        }
        Standard::Uk => {
            iso_common(&mut params);
            params.push(dp(
                "Standards set",
                "BS EN ISO 5457, BS EN ISO 7200, BS 8888 (references ISO 128 / ISO 3098)",
                "formula",
                "BS 8888 technical product documentation framework",
            ));
            params.push(dp("Default sheet", "A1 landscape", "engineer", "National Highways / client CAD requirements"));
            params.push(dp("File / sheet naming", "PRJ-ORG-ZZ-ZZ-DR-C-0001 (Project-Originator-Functional-Spatial-Form-Discipline-Number)", "verified", UKNA));
            params.push(dp(
                "Status and revision",
                "S0-S4 for shared preliminary information with revisions P01, P02 ...; A1.. with C01.. once authorised",
                "verified",
                UKNA,
            ));
            params.push(dp("Layer naming", "Role-Classification-Presentation_Description, e.g. C-00-M_Alignment", "formula", BS1192));
            params.push(dp(
                "Uniclass 2015 classification codes",
                "not shipped: code per layer entered by the engineer via layer_map (placeholder 00)",
                "engineer",
                "NBS Uniclass 2015 tables",
            ));
            params.push(dp(
                "DMRB / National Highways CAD requirements",
                "client CAD and data standard; drawing numbering and title-block content per the client's information requirements",
                "engineer",
                "National Highways information requirements for the scheme",
            ));
            params.push(dp(
                "Seal",
                "No statutory engineer's seal; Checked / Approved signatures recorded per client QA and ISO 19650 authorisation",
                "formula",
                "ISO 7200 approval person field; BS EN ISO 19650 A status",
            ));
        }
        Standard::Au => {
            iso_common(&mut params);
            params.push(dp(
                "National drawing standard",
                "AS 1100.101 general principles and AS 1100.401 engineering survey and survey design drawing (ISO-based sizes, borders, lines, lettering)",
                "formula",
                "Standards Australia AS 1100.101, AS 1100.401 (paywalled: content not reproduced)",
            ));
            params.push(dp(
                "Default sheet",
                "A1 landscape",
                "engineer",
                "TfNSW CAD/drafting standard, DTP (VicRoads) drafting standards, other state road authority",
            ));
            params.push(dp(
                "Layer naming",
                "ISO 13567-style defaults; agency level/layer library via layer_map",
                "engineer",
                "TfNSW / DTP Victoria CAD standards",
            ));
            params.push(dp(
                "Drawing numbering",
                "PRJ-C-0001 by default; agency numbering via sheet_id_pattern",
                "engineer",
                "State road authority drawing register",
            ));
            params.push(dp(
                "Revision",
                "letters for preliminary issues, numbers from issue for construction (common practice)",
                "engineer",
                "Agency drafting standard",
            ));
            params.push(dp(
                "Seal / sign-off",
                "Signature of the responsible engineer (RPEQ in Queensland; state registration schemes elsewhere)",
                "engineer",
                "Professional Engineers Act 2002 (Qld) and state registration schemes",
            ));
        }
        Standard::Nz => {
            iso_common(&mut params);
            params.push(dp(
                "National drawing standard",
                "AS/NZS 1100 series (ISO-based sizes, borders, lines and lettering)",
                "formula",
                "AS/NZS 1100 (paywalled: content not reproduced)",
            ));
            params.push(dp("Default sheet", "A1 landscape", "engineer", "NZTA Waka Kotahi project CAD / drawing requirements"));
            params.push(dp(
                "Layer naming",
                "ISO 13567-style defaults; NZTA or council template via layer_map",
                "engineer",
                "NZTA Waka Kotahi CAD requirements",
            ));
            params.push(dp("Drawing numbering", "PRJ-C-0001 by default", "engineer", "NZTA / client drawing register"));
            params.push(dp(
                "Seal / sign-off",
                "Signature of a Chartered Professional Engineer (CPEng) or the client's authorised signatory",
                "engineer",
                "Engineering New Zealand / client requirements",
            ));
        }
    }
    params.push(dp(
        "Engineer seal / stamp box",
        "on every sheet, empty unless the engineer records approval; the agent never applies a seal",
        "formula",
        "civil-mcp approval gate (audit log)",
    ));
    params.push(dp(
        "North arrow and scale bar",
        "north arrow on plan views; graphic scale bar wherever a numeric scale applies (true at any print size)",
        "formula",
        "drafting practice (NCS, AS 1100.401, agency manuals)",
    ));
    let iso = !matches!(s, Standard::Us);
    let mut req = vec![
        "legal_owner",
        "project",
        "sheet_title",
        "sheet_id",
        "sheet_no",
        "date",
        "drawn_by",
        "approval_person",
        "document_type",
        "revision",
        "scale",
        "seal_box",
        "checked_by",
        "status",
    ];
    match s {
        Standard::Us => req.extend(["designed_by", "north_arrow"]),
        Standard::Uk => req.push("status_code"),
        _ => {}
    }
    let (name, default_size) = match s {
        Standard::Us => ("United States: NCS layer/sheet conventions + state DOT CADD manual (Caltrans/FDOT style)", "ANSI D"),
        Standard::Ca => ("Canada: ISO TPD conventions + provincial CADD standard (MTO style)", "ISO A1"),
        Standard::Uk => {
            ("United Kingdom: BS EN ISO 5457/7200, BS 8888 (ISO 128/3098), BS EN ISO 19650-2 UK NA naming, DMRB/National Highways client CAD", "ISO A1")
        }
        Standard::Au => ("Australia: AS 1100.101/.401 (ISO-based) + state road authority CAD (TfNSW/DTP Victoria style)", "ISO A1"),
        Standard::Nz => ("New Zealand: AS/NZS 1100 (ISO-based) + NZTA Waka Kotahi CAD requirements", "ISO A1"),
    };
    Profile {
        code: code(s),
        name,
        default_size,
        margins: if iso { [20.0, 10.0, 10.0, 10.0] } else { [38.1, 12.7, 12.7, 12.7] },
        min_margins: if iso { [20.0, 10.0, 10.0, 10.0] } else { [12.7, 12.7, 12.7, 12.7] },
        tb_width: if iso { 180.0 } else { 130.0 },
        grid_reference: iso,
        centring_marks: iso,
        frame_line_mm: 0.7,
        min_text_mm: 2.5,
        text_mm: 3.5,
        heading_mm: 5.0,
        title_mm: 7.0,
        weights: [0.18, 0.25, 0.35, 0.5],
        layer_fmt: if matches!(s, Standard::Us | Standard::Ca) { LayerFmt::Ncs } else { LayerFmt::Iso },
        sheet_id: match s {
            Standard::Us | Standard::Ca => SheetIdFmt::Uds,
            Standard::Uk => SheetIdFmt::Uk19650,
            _ => SheetIdFmt::Agency,
        },
        initial_revision: if matches!(s, Standard::Uk) { "P01" } else { "A" },
        status_code: if matches!(s, Standard::Uk) { "S3" } else { "" },
        seal_label: match s {
            Standard::Us => "PROFESSIONAL ENGINEER SEAL",
            Standard::Ca => "ENGINEER STAMP / PERMIT",
            Standard::Uk => "APPROVED (COMPETENT PERSON)",
            Standard::Au => "RESPONSIBLE ENGINEER (RPEQ)",
            Standard::Nz => "CPEng / AUTHORISED SIGNATORY",
        },
        seal_rule: match s {
            Standard::Us => "Seal, signature and date by the licensed PE in responsible charge on final sheets only (state board rules)",
            Standard::Ca => "Stamp and signature of the professional engineer per provincial regulator rules",
            Standard::Uk => "No statutory seal; approval signature and A-status authorisation per ISO 19650 and client QA",
            Standard::Au => "Signature of the responsible engineer (RPEQ in Qld; state registration schemes elsewhere)",
            Standard::Nz => "Signature of a CPEng or the client's authorised signatory",
        },
        required_fields: req,
        preferred_scales: if iso {
            iso_scales()
        } else {
            let mut v = iso_scales();
            v.extend([120.0, 240.0, 250.0, 360.0, 480.0, 600.0, 720.0, 1200.0, 2400.0, 2500.0]);
            if crate::units::cur().us() {
                v.extend([12.0, 24.0, 48.0, 60.0, 3600.0, 4800.0, 6000.0, 12000.0, 24000.0]);
            }
            v
        },
        params,
    }
}

// ---------------------------------------------------------------- layers

/// internal layer → (NCS name, ISO description, presentation, weight class 0..3, line type)
fn layer_def(l: &str) -> (&'static str, &'static str, char, usize, &'static str) {
    match l {
        "ALIGNMENT" => ("C-ROAD-CNTR", "Alignment", 'M', 1, "DASHDOT"),
        "BARRIER" => ("C-ROAD-BARR", "Barrier", 'M', 3, "CONTINUOUS"),
        "BORDER" => ("C-ANNO-VIEW", "Viewport", 'P', 2, "CONTINUOUS"),
        "CONTOUR-MAJOR" => ("V-TOPO-MAJR", "ContourMajor", 'M', 1, "CONTINUOUS"),
        "CONTOUR-MINOR" => ("V-TOPO-MINR", "ContourMinor", 'M', 0, "CONTINUOUS"),
        "CULVERT" => ("C-STRM-CULV", "Culvert", 'M', 2, "CONTINUOUS"),
        "CURB" => ("C-ROAD-CURB", "Kerb", 'M', 2, "CONTINUOUS"),
        "DAYLIGHT" => ("C-GRAD-DAYL", "Daylight", 'M', 1, "DASHED"),
        "DETAIL" => ("C-DETL-LINE", "Detail", 'M', 2, "CONTINUOUS"),
        "EGL" => ("C-STRM-EGLN", "EnergyGrade", 'M', 1, "DOT"),
        "GRADING" => ("C-GRAD-FTPT", "Grading", 'M', 2, "CONTINUOUS"),
        "GRID" => ("C-ANNO-GRID", "Grid", 'P', 0, "CONTINUOUS"),
        "HATCH" => ("C-DETL-PATT", "Hatch", 'H', 0, "CONTINUOUS"),
        "HAZARD" => ("C-ROAD-HAZD", "Hazard", 'M', 2, "DASHED"),
        "HGL" => ("C-STRM-HGLN", "HydraulicGrade", 'M', 1, "DASHED"),
        "INTERSECTION" => ("C-ROAD-EDGE", "Intersection", 'M', 2, "CONTINUOUS"),
        "LANDSCAPE" => ("L-PLNT-PLAN", "Landscape", 'M', 1, "CONTINUOUS"),
        "LIGHT" => ("E-LITE-POLE", "Lighting", 'M', 2, "CONTINUOUS"),
        "LINING" => ("C-STRM-LINE", "DitchLining", 'M', 2, "CONTINUOUS"),
        "OSM-PLACE" => ("V-BASE-TEXT", "BasemapText", 'T', 1, "CONTINUOUS"),
        "OSM-ROAD" => ("V-BASE-ROAD", "BasemapRoad", 'M', 1, "CONTINUOUS"),
        "OSM-ROAD-MAJOR" => ("V-BASE-RDMJ", "BasemapRoadMajor", 'M', 1, "CONTINUOUS"),
        "OSM-ROAD-MINOR" => ("V-BASE-RDMN", "BasemapRoadMinor", 'M', 0, "CONTINUOUS"),
        "OSM-WATER" => ("V-BASE-WATR", "BasemapWater", 'M', 1, "CONTINUOUS"),
        "PARCEL" => ("V-PROP-LINE", "Parcel", 'M', 1, "DASHDOT"),
        "PAVEMENT" => ("C-ROAD-PVMT", "Pavement", 'M', 2, "CONTINUOUS"),
        "PIPE" => ("C-STRM-PIPE", "Pipe", 'M', 2, "CONTINUOUS"),
        "PROFILE-DESIGN" => ("C-ROAD-PROF", "ProfileDesign", 'M', 3, "CONTINUOUS"),
        "PROFILE-GROUND" => ("V-TOPO-PROF", "ProfileExisting", 'M', 1, "DASHED"),
        "ROUTE-A" => ("C-ROUT-ALTA", "RouteA", 'M', 1, "CONTINUOUS"),
        "ROUTE-B" => ("C-ROUT-ALTB", "RouteB", 'M', 1, "CONTINUOUS"),
        "ROUTE-C" => ("C-ROUT-ALTC", "RouteC", 'M', 1, "CONTINUOUS"),
        "ROUTE-SELECTED" => ("C-ROUT-SELD", "RouteSelected", 'M', 3, "CONTINUOUS"),
        "ROW" => ("V-PROP-ROWL", "RightOfWay", 'M', 2, "DASHDOT"),
        "SIGHT" => ("C-ROAD-SGHT", "SightLine", 'M', 1, "DASHED"),
        "SIGN" => ("C-SIGN-POST", "Signs", 'M', 2, "CONTINUOUS"),
        "SIGNAL" => ("E-SIGL-HEAD", "Signals", 'M', 2, "CONTINUOUS"),
        "SLIP" => ("C-GEOT-SLIP", "SlipSurface", 'M', 2, "DASHED"),
        "STAMP" => ("G-ANNO-STMP", "Stamp", 'T', 2, "CONTINUOUS"),
        "STRUCTURE" => ("C-STRM-STRC", "Structure", 'M', 2, "CONTINUOUS"),
        "SUBGRADE" => ("C-ROAD-SUBG", "Subgrade", 'M', 1, "CONTINUOUS"),
        "SURF-EG" => ("V-TOPO-SURF", "SurfaceExisting", 'M', 0, "CONTINUOUS"),
        "SURF-FG" => ("C-ROAD-SURF", "SurfaceFinished", 'M', 0, "CONTINUOUS"),
        "FEATURE" => ("C-ROAD-FEAT", "FeatureLine", 'M', 1, "CONTINUOUS"),
        "TEXT" => ("C-ANNO-TEXT", "Annotation", 'T', 1, "CONTINUOUS"),
        "TITLE" => ("C-ANNO-HEAD", "Heading", 'T', 2, "CONTINUOUS"),
        "TTM" => ("C-TRAF-TEMP", "TempTraffic", 'M', 2, "DASHED"),
        "UTILITY" => ("V-UTIL-EXST", "UtilityExisting", 'M', 2, "DASHED"),
        "WALL" => ("C-ROAD-WALL", "Wall", 'M', 3, "CONTINUOUS"),
        "XS-DATUM" => ("C-ROAD-DATM", "SubgradeDatum", 'M', 2, "CONTINUOUS"),
        "XS-DESIGN" => ("C-ROAD-XSEC", "SectionDesign", 'M', 3, "CONTINUOUS"),
        "XS-GROUND" => ("V-TOPO-XSEC", "SectionExisting", 'M', 1, "DASHED"),
        "FRAME" => ("G-ANNO-BRDR", "Frame", 'P', 3, "CONTINUOUS"),
        "FRAME-GRID" => ("G-ANNO-GREF", "GridReference", 'P', 2, "CONTINUOUS"),
        "TB" => ("G-ANNO-TTLB", "TitleBlock", 'P', 2, "CONTINUOUS"),
        "TB-TEXT" => ("G-ANNO-TTLB-TEXT", "TitleBlockText", 'T', 1, "CONTINUOUS"),
        "REV" => ("G-ANNO-REVS", "Revisions", 'T', 1, "CONTINUOUS"),
        "SEAL" => ("G-ANNO-SEAL", "SealBox", 'P', 2, "CONTINUOUS"),
        "LOGO" => ("G-ANNO-LOGO", "Logo", 'P', 1, "CONTINUOUS"),
        "NORTH" => ("G-ANNO-NARW", "NorthArrow", 'P', 2, "CONTINUOUS"),
        "SCALEBAR" => ("G-ANNO-SCAL", "ScaleBar", 'P', 1, "CONTINUOUS"),
        "TEMPLATE" => ("G-ANNO-TMPL", "Template", 'P', 2, "CONTINUOUS"),
        _ => ("", "", 'M', 1, "CONTINUOUS"),
    }
}

pub fn layer_name(p: &Profile, internal: &str, map: &BTreeMap<String, String>) -> String {
    if let Some(n) = map.get(internal) {
        return n.clone();
    }
    let (ncs, iso, pres, _, _) = layer_def(internal);
    match p.layer_fmt {
        LayerFmt::Ncs if !ncs.is_empty() => ncs.into(),
        LayerFmt::Ncs => {
            let a: String = internal.chars().filter(|c| c.is_ascii_alphanumeric()).take(4).collect::<String>().to_uppercase();
            format!("C-MISC-{a:X<4}")
        }
        LayerFmt::Iso => {
            let desc: String = if iso.is_empty() {
                internal.split('-').filter(|w| !w.is_empty()).map(|w| w[..1].to_string() + &w[1..].to_lowercase()).collect()
            } else {
                iso.into()
            };
            format!("C-00-{pres}_{desc}")
        }
    }
}

pub fn layer_name_ok(p: &Profile, n: &str, custom: bool) -> bool {
    if custom {
        return !n.is_empty() && n.len() <= 255;
    }
    match p.layer_fmt {
        LayerFmt::Ncs => {
            let parts: Vec<&str> = n.split('-').collect();
            parts.len() >= 2
                && parts.len() <= 5
                && (1..=2).contains(&parts[0].len())
                && parts[0].chars().all(|c| c.is_ascii_uppercase())
                && parts[1..].iter().enumerate().all(|(i, q)| {
                    let status = i + 2 == parts.len() && i > 0 && q.len() == 1;
                    (q.len() == 4 || status) && q.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                })
        }
        LayerFmt::Iso => {
            let parts: Vec<&str> = n.splitn(3, '-').collect();
            parts.len() == 3
                && parts[0].len() == 1
                && parts[0].chars().all(|c| c.is_ascii_uppercase())
                && !parts[1].is_empty()
                && parts[1].chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && parts[2].len() > 2
                && "MTDHP".contains(&parts[2][..1])
                && &parts[2][1..2] == "_"
        }
    }
}

// ---------------------------------------------------------------- frame spec

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Revision {
    pub rev: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub checked: String,
    #[serde(default)]
    pub approved: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TitleFields {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub legal_owner: Option<String>,
    #[serde(default)]
    pub contract_no: Option<String>,
    #[serde(default)]
    pub project_code: Option<String>,
    #[serde(default)]
    pub originator: Option<String>,
    #[serde(default)]
    pub designed_by: Option<String>,
    #[serde(default)]
    pub drawn_by: Option<String>,
    #[serde(default)]
    pub checked_by: Option<String>,
    #[serde(default)]
    pub approved_by: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub document_type: Option<String>,
    #[serde(default)]
    pub status_code: Option<String>,
    #[serde(default)]
    pub logo_text: Option<String>,
    #[serde(default)]
    pub revisions: Vec<Revision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Template {
    /// DXF (ASCII) or SVG file with the frame graphics in sheet millimetres
    #[serde(default)]
    pub path: Option<String>,
    /// "dxf" | "svg" | "json"
    #[serde(default)]
    pub format: Option<String>,
    /// inline entities for json: {"lines": [[x0,y0,x1,y1]], "polylines": [[[x,y]..]], "texts": [[x,y,h,"text"]]}
    #[serde(default)]
    pub entities: Option<Value>,
    /// [x0, y0, x1, y1] drawing space (mm)
    #[serde(default)]
    pub drawing_area: Option<[f64; 4]>,
    /// field name → [x, y, text height] (mm)
    #[serde(default)]
    pub field_positions: BTreeMap<String, [f64; 3]>,
    /// true: the template carries the whole title block and only field values are written
    #[serde(default)]
    pub replace_title_block: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FrameSpec {
    /// US | CA | UK | AU | NZ (defaults to the design-basis country)
    #[serde(default)]
    pub profile: Option<String>,
    /// e.g. "ISO A1", "A3", "ANSI D", "ARCH D", "CUSTOM"
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub width_mm: Option<f64>,
    #[serde(default)]
    pub height_mm: Option<f64>,
    /// landscape | portrait
    #[serde(default)]
    pub orientation: Option<String>,
    /// [left, right, top, bottom] mm
    #[serde(default)]
    pub margins_mm: Option<[f64; 4]>,
    #[serde(default)]
    pub title_block_width_mm: Option<f64>,
    #[serde(default)]
    pub grid_reference: Option<bool>,
    #[serde(default)]
    pub north_arrow: Option<bool>,
    #[serde(default)]
    pub scale_bar: Option<bool>,
    #[serde(default)]
    pub fields: TitleFields,
    #[serde(default)]
    pub template: Option<Template>,
    /// internal layer → exported layer name (agency level library)
    #[serde(default)]
    pub layer_map: BTreeMap<String, String>,
    /// sheet identifier pattern, tokens {project} {originator} {n:04} {n:03} {n}
    #[serde(default)]
    pub sheet_id_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    pub country: &'static str,
    pub size: String,
    pub w: f64,
    pub h: f64,
    pub orientation: String,
    pub margins: [f64; 4],
    /// drawing-space frame [x0, y0, x1, y1]
    pub frame: [f64; 4],
    /// title block region
    pub tb: [f64; 4],
    /// "right" | "bottom" | "template"
    pub tb_layout: String,
    /// where the A1 layout content goes
    pub area: [f64; 4],
    /// uniform factor applied to the A1 layout
    pub k: f64,
    pub grid_reference: bool,
    pub north_arrow: bool,
    pub scale_bar: bool,
    #[serde(skip)]
    pub profile: Profile,
    #[serde(skip)]
    pub spec: FrameSpec,
    #[serde(skip)]
    pub template_ents: Vec<Ent>,
    pub warnings: Vec<String>,
}

pub fn resolve(spec: &FrameSpec, default_country: Standard) -> crate::Result<Resolved> {
    let cs = match &spec.profile {
        Some(p) => country(p).ok_or_else(|| format!("unknown profile '{p}': use US, CA, UK, AU or NZ"))?,
        None => default_country,
    };
    let pf = profile(cs);
    let mut warnings = vec![];
    let req = spec.size.clone().unwrap_or_else(|| pf.default_size.to_string());
    let (size, long, short) = if req.trim().eq_ignore_ascii_case("CUSTOM") {
        let (a, b) = (spec.width_mm.ok_or("CUSTOM size needs width_mm")?, spec.height_mm.ok_or("CUSTOM size needs height_mm")?);
        if !(100.0..=5000.0).contains(&a) || !(100.0..=5000.0).contains(&b) {
            return Err("custom sheet dimensions must be 100-5000 mm".into());
        }
        (format!("CUSTOM {:.0}x{:.0}", a.max(b), a.min(b)), a.max(b), a.min(b))
    } else {
        let (n, a, b) =
            size_lookup(&req).ok_or_else(|| format!("unknown sheet size '{req}': {} or CUSTOM", SIZES.iter().map(|s| s.0).collect::<Vec<_>>().join(", ")))?;
        (n.to_string(), a, b)
    };
    let orientation = spec.orientation.clone().unwrap_or_else(|| "landscape".into()).to_lowercase();
    if orientation != "landscape" && orientation != "portrait" {
        return Err("orientation must be landscape or portrait".into());
    }
    let (w, h) = if orientation == "landscape" { (long, short) } else { (short, long) };
    let margins = spec.margins_mm.unwrap_or(pf.margins);
    if margins.iter().any(|m| *m < 0.0) || margins[0] + margins[1] > w * 0.5 || margins[2] + margins[3] > h * 0.5 {
        return Err("margins out of range".into());
    }
    let frame = [margins[0], margins[3], w - margins[1], h - margins[2]];
    let mut template_ents = vec![];
    let (tb, tb_layout, area) = if let Some(t) = &spec.template {
        template_ents = load_template(t)?;
        let da = t.drawing_area.ok_or("template needs drawing_area [x0,y0,x1,y1] (mm)")?;
        if da[0] < 0.0 || da[1] < 0.0 || da[2] > w + 1e-6 || da[3] > h + 1e-6 || da[2] <= da[0] || da[3] <= da[1] {
            return Err("template drawing_area must lie on the sheet".into());
        }
        if t.replace_title_block {
            ([0.0; 4], "template".to_string(), da)
        } else {
            let tw = spec.title_block_width_mm.unwrap_or(pf.tb_width).min((frame[2] - frame[0]) * 0.45);
            ([frame[2] - tw, frame[1], frame[2], frame[3]], "right".to_string(), [da[0], da[1], da[2].min(frame[2] - tw), da[3]])
        }
    } else if w >= 400.0 && orientation == "landscape" {
        let cap = if w < 600.0 { 0.3 } else { 0.4 };
        let tw = spec.title_block_width_mm.unwrap_or(pf.tb_width).clamp(90.0, ((frame[2] - frame[0]) * cap).max(90.0));
        ([frame[2] - tw, frame[1], frame[2], frame[3]], "right".into(), [frame[0], frame[1], frame[2] - tw, frame[3]])
    } else {
        let th = if orientation == "portrait" { 112.0f64 } else { 88.0 }.min((frame[3] - frame[1]) * 0.45);
        ([frame[0], frame[1], frame[2], frame[1] + th], "bottom".into(), [frame[0], frame[1] + th, frame[2], frame[3]])
    };
    let inset = 2.0;
    let (aw, ah) = (area[2] - area[0] - 2.0 * inset, area[3] - area[1] - 2.0 * inset);
    let (lw, lh) = (LAYOUT[2] - LAYOUT[0], LAYOUT[3] - LAYOUT[1]);
    let k = (aw / lw).min(ah / lh);
    if k < 0.45 {
        warnings
            .push(format!("the A1 layout is reduced to {:.0}% on {size}; small annotation falls below the minimum text height (advisory FR-09b)", k * 100.0));
    }
    if size != pf.default_size {
        warnings.push(format!("{} profile default sheet is {}; {} selected", pf.code, pf.default_size, size));
    }
    Ok(Resolved {
        country: pf.code,
        size,
        w,
        h,
        orientation,
        margins,
        frame,
        tb,
        tb_layout,
        area,
        k,
        grid_reference: spec.grid_reference.unwrap_or(pf.grid_reference),
        north_arrow: spec.north_arrow.unwrap_or(true),
        scale_bar: spec.scale_bar.unwrap_or(true),
        profile: pf,
        spec: spec.clone(),
        template_ents,
        warnings,
    })
}

// ---------------------------------------------------------------- templates

fn load_template(t: &Template) -> crate::Result<Vec<Ent>> {
    let fmt = t.format.clone().or_else(|| t.path.as_ref().and_then(|p| p.rsplit('.').next().map(|s| s.to_lowercase()))).unwrap_or_else(|| "json".into());
    let read = || -> crate::Result<String> { std::fs::read_to_string(t.path.as_ref().ok_or("template path required")?).map_err(|e| format!("template: {e}")) };
    match fmt.as_str() {
        "dxf" => parse_dxf(&read()?),
        "svg" => parse_svg(&read()?),
        "json" => {
            let v = match (&t.entities, &t.path) {
                (Some(v), _) => v.clone(),
                (None, Some(_)) => serde_json::from_str(&read()?).map_err(|e| e.to_string())?,
                _ => json!({}),
            };
            let mut out = vec![];
            let num = |x: &Value| x.as_f64().unwrap_or(0.0);
            for l in v["lines"].as_array().into_iter().flatten() {
                out.push(Ent::Line { layer: "TEMPLATE".into(), a: [num(&l[0]), num(&l[1])], b: [num(&l[2]), num(&l[3])] });
            }
            for p in v["polylines"].as_array().into_iter().flatten() {
                let pts: Vec<[f64; 2]> = p.as_array().into_iter().flatten().map(|q| [num(&q[0]), num(&q[1])]).collect();
                if pts.len() > 1 {
                    out.push(Ent::Poly { layer: "TEMPLATE".into(), pts });
                }
            }
            for x in v["texts"].as_array().into_iter().flatten() {
                out.push(Ent::Text { layer: "TEMPLATE".into(), p: [num(&x[0]), num(&x[1])], h: num(&x[2]).max(1.0), s: x[3].as_str().unwrap_or("").into() });
            }
            Ok(out)
        }
        f => Err(format!("template format '{f}' not supported (dxf, svg, json)")),
    }
}

/// Minimal ASCII DXF reader for frame templates: LINE, LWPOLYLINE, POLYLINE/VERTEX, CIRCLE, ARC, TEXT, MTEXT (mm).
pub fn parse_dxf(s: &str) -> crate::Result<Vec<Ent>> {
    let lines: Vec<&str> = s.lines().map(|l| l.trim()).collect();
    let mut pairs = vec![];
    let mut i = 0;
    while i + 1 < lines.len() {
        if let Ok(c) = lines[i].parse::<i32>() {
            pairs.push((c, lines[i + 1].to_string()));
        }
        i += 2;
    }
    let start = pairs.iter().position(|p| p.0 == 2 && p.1 == "ENTITIES").ok_or("DXF template: no ENTITIES section")?;
    let mut out = vec![];
    let mut j = start + 1;
    let mut poly: Option<Vec<[f64; 2]>> = None;
    let tl = || "TEMPLATE".to_string();
    while j < pairs.len() {
        if pairs[j].0 != 0 {
            j += 1;
            continue;
        }
        let kind = pairs[j].1.clone();
        if kind == "ENDSEC" {
            break;
        }
        let mut k = j + 1;
        let mut g: Vec<(i32, String)> = vec![];
        while k < pairs.len() && pairs[k].0 != 0 {
            g.push(pairs[k].clone());
            k += 1;
        }
        let v = |c: i32| g.iter().find(|p| p.0 == c).and_then(|p| p.1.parse::<f64>().ok()).unwrap_or(0.0);
        match kind.as_str() {
            "LINE" => out.push(Ent::Line { layer: tl(), a: [v(10), v(20)], b: [v(11), v(21)] }),
            "LWPOLYLINE" => {
                let xs: Vec<f64> = g.iter().filter(|p| p.0 == 10).filter_map(|p| p.1.parse().ok()).collect();
                let ys: Vec<f64> = g.iter().filter(|p| p.0 == 20).filter_map(|p| p.1.parse().ok()).collect();
                let mut pts: Vec<[f64; 2]> = xs.iter().zip(ys.iter()).map(|(x, y)| [*x, *y]).collect();
                if (v(70) as i64) & 1 == 1 && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                if pts.len() > 1 {
                    out.push(Ent::Poly { layer: tl(), pts });
                }
            }
            "POLYLINE" => poly = Some(vec![]),
            "VERTEX" => {
                if let Some(p) = poly.as_mut() {
                    p.push([v(10), v(20)]);
                }
            }
            "SEQEND" => {
                if let Some(p) = poly.take() {
                    if p.len() > 1 {
                        out.push(Ent::Poly { layer: tl(), pts: p });
                    }
                }
            }
            "CIRCLE" | "ARC" => {
                let (cx, cy, r) = (v(10), v(20), v(40));
                let (a0, mut a1) = if kind == "ARC" { (v(50).to_radians(), v(51).to_radians()) } else { (0.0, std::f64::consts::TAU) };
                if a1 <= a0 {
                    a1 += std::f64::consts::TAU;
                }
                let pts = (0..=36).map(|t| a0 + (a1 - a0) * t as f64 / 36.0).map(|a| [cx + r * a.cos(), cy + r * a.sin()]).collect();
                out.push(Ent::Poly { layer: tl(), pts });
            }
            "TEXT" | "MTEXT" => {
                let t = g.iter().find(|p| p.0 == 1).map(|p| p.1.clone()).unwrap_or_default();
                out.push(Ent::Text { layer: tl(), p: [v(10), v(20)], h: v(40).max(1.0), s: t });
            }
            _ => {}
        }
        j = k;
    }
    Ok(out)
}

fn svg_num(s: Option<&str>) -> Option<(f64, bool)> {
    let s = s?.trim();
    let n: f64 = s.trim_end_matches(|c: char| c.is_alphabetic() || c == '%').parse().ok()?;
    Some((n, s.ends_with("mm")))
}

fn path_tokens(d: &str) -> Vec<String> {
    let mut toks: Vec<String> = vec![];
    let mut cur = String::new();
    for c in d.chars() {
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            toks.push(c.to_string());
        } else if c == ',' || c.is_whitespace() {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
        } else if c == '-' && !cur.is_empty() && !cur.ends_with(['e', 'E']) {
            toks.push(std::mem::take(&mut cur));
            cur.push(c);
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

/// Minimal SVG reader for frame templates: line, rect, polyline, polygon, circle, path (M/L/H/V/Z absolute and
/// relative), text. Group transforms are not applied. The viewBox is mapped to width/height when they are in mm.
pub fn parse_svg(s: &str) -> crate::Result<Vec<Ent>> {
    let doc = roxmltree::Document::parse(s).map_err(|e| format!("SVG template: {e}"))?;
    let root = doc.root_element();
    let vb: Vec<f64> = root.attribute("viewBox").map(|v| v.split([' ', ',']).filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let (wv, hv) = (svg_num(root.attribute("width")), svg_num(root.attribute("height")));
    let (sx, vh, ox, oy) = match (vb.len() == 4, wv) {
        (true, Some((w, true))) => (w / vb[2], vb[3], vb[0], vb[1]),
        (true, _) => (1.0, vb[3], vb[0], vb[1]),
        (false, _) => (1.0, hv.map(|h| h.0).ok_or("SVG template needs a viewBox or height")?, 0.0, 0.0),
    };
    let tp = |x: f64, y: f64| [(x - ox) * sx, (vh - (y - oy)) * sx];
    let a = |n: &roxmltree::Node, k: &str| svg_num(n.attribute(k)).map(|v| v.0).unwrap_or(0.0);
    let tl = || "TEMPLATE".to_string();
    let mut out = vec![];
    for n in doc.descendants().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "line" => out.push(Ent::Line { layer: tl(), a: tp(a(&n, "x1"), a(&n, "y1")), b: tp(a(&n, "x2"), a(&n, "y2")) }),
            "rect" => {
                let (x, y, w, h) = (a(&n, "x"), a(&n, "y"), a(&n, "width"), a(&n, "height"));
                out.push(Ent::Poly { layer: tl(), pts: vec![tp(x, y), tp(x + w, y), tp(x + w, y + h), tp(x, y + h), tp(x, y)] });
            }
            "polyline" | "polygon" => {
                let v: Vec<f64> = n.attribute("points").unwrap_or("").split([' ', ',', '\n']).filter_map(|x| x.parse().ok()).collect();
                let mut pts: Vec<[f64; 2]> = v.chunks(2).filter(|c| c.len() == 2).map(|c| tp(c[0], c[1])).collect();
                if n.tag_name().name() == "polygon" && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                if pts.len() > 1 {
                    out.push(Ent::Poly { layer: tl(), pts });
                }
            }
            "circle" => {
                let (cx, cy, r) = (a(&n, "cx"), a(&n, "cy"), a(&n, "r"));
                let pts = (0..=36).map(|t| t as f64 / 36.0 * std::f64::consts::TAU).map(|t| tp(cx + r * t.cos(), cy + r * t.sin())).collect();
                out.push(Ent::Poly { layer: tl(), pts });
            }
            "path" => {
                let toks = path_tokens(n.attribute("d").unwrap_or(""));
                let (mut x, mut y, mut x0, mut y0) = (0.0, 0.0, 0.0, 0.0);
                let mut pts: Vec<[f64; 2]> = vec![];
                let mut cmd = 'M';
                let mut i = 0;
                let flush = |pts: &mut Vec<[f64; 2]>, out: &mut Vec<Ent>| {
                    if pts.len() > 1 {
                        out.push(Ent::Poly { layer: "TEMPLATE".into(), pts: std::mem::take(pts) });
                    } else {
                        pts.clear();
                    }
                };
                while i < toks.len() {
                    if let Some(c) = toks[i].chars().next().filter(|c| c.is_ascii_alphabetic()) {
                        cmd = c;
                        i += 1;
                        if c == 'Z' || c == 'z' {
                            pts.push(tp(x0, y0));
                            x = x0;
                            y = y0;
                            flush(&mut pts, &mut out);
                        }
                        continue;
                    }
                    let num = |k: usize| toks.get(k).and_then(|t| t.parse::<f64>().ok()).unwrap_or(0.0);
                    match cmd {
                        'M' | 'm' | 'L' | 'l' => {
                            let rel = cmd.is_ascii_lowercase();
                            x = if rel { x + num(i) } else { num(i) };
                            y = if rel { y + num(i + 1) } else { num(i + 1) };
                            if cmd == 'M' || cmd == 'm' {
                                flush(&mut pts, &mut out);
                                x0 = x;
                                y0 = y;
                                cmd = if cmd == 'M' { 'L' } else { 'l' };
                            }
                            pts.push(tp(x, y));
                            i += 2;
                        }
                        'H' | 'h' => {
                            x = if cmd == 'h' { x + num(i) } else { num(i) };
                            pts.push(tp(x, y));
                            i += 1;
                        }
                        'V' | 'v' => {
                            y = if cmd == 'v' { y + num(i) } else { num(i) };
                            pts.push(tp(x, y));
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }
                flush(&mut pts, &mut out);
            }
            "text" => {
                let t: String = n.descendants().filter(|c| c.is_text()).filter_map(|c| c.text()).collect::<String>().trim().to_string();
                let fs = svg_num(n.attribute("font-size")).map(|v| v.0).unwrap_or(3.5);
                out.push(Ent::Text { layer: tl(), p: tp(a(&n, "x"), a(&n, "y")), h: fs * sx * 0.729, s: t });
            }
            _ => {}
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- sheet ids and revisions

fn sheet_type(title: &str) -> u8 {
    let t = title.to_lowercase();
    if t.contains("cover") || t.contains("index") || t.contains("locality") {
        0
    } else if t.contains("cross section") || t.contains("typical section") {
        3
    } else if t.contains("detail") || t.contains("atlas") || t.contains("pavement") {
        5
    } else if t.contains("quantit")
        || t.contains("schedule")
        || t.contains("mass haul")
        || t.contains("diagram")
        || t.contains("capacity")
        || t.contains("register")
    {
        6
    } else if t.contains("plan") || t.contains("layout") || t.contains("route") || t.contains("land") || t.contains("utilit") {
        1
    } else if t.contains("profile") || t.contains("elevation") {
        2
    } else {
        7
    }
}

pub fn sheet_ids(r: &Resolved, titles: &[String]) -> Vec<String> {
    let f = &r.spec.fields;
    let proj = f.project_code.clone().unwrap_or_else(|| "PRJ".into());
    let orig = f.originator.clone().unwrap_or_else(|| "ORG".into());
    if let Some(p) = &r.spec.sheet_id_pattern {
        return (1..=titles.len())
            .map(|n| {
                p.replace("{project}", &proj)
                    .replace("{originator}", &orig)
                    .replace("{n:04}", &format!("{n:04}"))
                    .replace("{n:03}", &format!("{n:03}"))
                    .replace("{n}", &n.to_string())
            })
            .collect();
    }
    match r.profile.sheet_id {
        SheetIdFmt::Uds => {
            let mut seq: BTreeMap<(char, u8), usize> = BTreeMap::new();
            titles
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let ty = sheet_type(t);
                    let disc = if i == 0 { 'G' } else { 'C' };
                    let e = seq.entry((disc, ty)).or_insert(0);
                    *e += 1;
                    format!("{disc}-{ty}{:02}", *e)
                })
                .collect()
        }
        SheetIdFmt::Uk19650 => (1..=titles.len()).map(|n| format!("{proj}-{orig}-ZZ-ZZ-DR-C-{n:04}")).collect(),
        SheetIdFmt::Agency => (1..=titles.len()).map(|n| format!("{proj}-C-{n:04}")).collect(),
    }
}

pub fn sheet_id_ok(r: &Resolved, id: &str) -> bool {
    if r.spec.sheet_id_pattern.is_some() {
        return !id.is_empty();
    }
    let p: Vec<&str> = id.split('-').collect();
    match r.profile.sheet_id {
        SheetIdFmt::Uds => p.len() == 2 && p[0].len() == 1 && p[1].len() == 3 && p[1].chars().all(|c| c.is_ascii_digit()) && &p[1][1..] != "00",
        SheetIdFmt::Uk19650 => p.len() == 7 && p[4] == "DR" && p.iter().all(|x| !x.is_empty()) && p[6].chars().all(|c| c.is_ascii_digit()),
        SheetIdFmt::Agency => p.len() == 3 && p[2].chars().all(|c| c.is_ascii_digit()),
    }
}

pub fn revision_ok(r: &Resolved, rev: &str, preliminary: bool) -> bool {
    match r.profile.code {
        "UK" => {
            let ok = rev.len() == 3 && (rev.starts_with('P') || rev.starts_with('C')) && rev[1..].chars().all(|c| c.is_ascii_digit());
            ok && !(preliminary && rev.starts_with('C'))
        }
        "US" => !rev.is_empty() && rev.len() <= 3 && rev.chars().all(|c| c.is_ascii_alphanumeric()),
        _ => {
            let letters = !rev.is_empty() && rev.len() <= 2 && rev.chars().all(|c| c.is_ascii_uppercase() && c != 'I' && c != 'O');
            let digits = !rev.is_empty() && rev.len() <= 3 && rev.chars().all(|c| c.is_ascii_digit());
            letters || digits
        }
    }
}

// ---------------------------------------------------------------- composition

pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let z = secs.div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// First "1:n" ratio in a scale note.
pub fn layout_scale(scale: &str) -> Option<f64> {
    let i = scale.find("1:")?;
    let n: String = scale[i + 2..].chars().take_while(|c| c.is_ascii_digit()).collect();
    n.parse().ok().filter(|v: &f64| *v > 0.0)
}

pub fn is_plan(title: &str) -> bool {
    let t = title.to_lowercase();
    ["plan", "route", "layout", "locality", "land requirement", "utilit"].iter().any(|k| t.contains(k)) && !t.starts_with("pipe")
}

/// Values of the title-block fields for one sheet.
pub fn field_values(r: &Resolved, info: &SheetInfo, meta: &SheetMeta, id: &str) -> BTreeMap<&'static str, String> {
    let f = &r.spec.fields;
    let approved = info.approved_seq.is_some() && info.checks_failed == 0;
    let rev = f.revisions.last().map(|x| x.rev.clone()).unwrap_or_else(|| r.profile.initial_revision.to_string());
    let mut m = BTreeMap::new();
    m.insert("project", f.project.clone().unwrap_or_else(|| info.project.clone()));
    m.insert("client", f.client.clone().unwrap_or_default());
    m.insert("legal_owner", f.legal_owner.clone().or_else(|| f.client.clone()).unwrap_or_default());
    m.insert("contract_no", f.contract_no.clone().unwrap_or_default());
    m.insert("sheet_title", meta.title.clone());
    m.insert("sheet_id", id.to_string());
    m.insert("sheet_no", format!("{} of {}", meta.page, meta.pages));
    m.insert("date", f.date.clone().unwrap_or_else(today));
    m.insert("designed_by", f.designed_by.clone().unwrap_or_else(|| "civil-mcp agent (engineer-directed)".into()));
    m.insert("drawn_by", f.drawn_by.clone().unwrap_or_else(|| "civil-mcp agent tools".into()));
    m.insert("checked_by", f.checked_by.clone().unwrap_or_default());
    m.insert("approval_person", f.approved_by.clone().unwrap_or_else(|| info.engineer.clone()));
    m.insert("document_type", f.document_type.clone().unwrap_or_else(|| "Road design drawing".into()));
    m.insert("revision", rev);
    m.insert(
        "status_code",
        f.status_code.clone().unwrap_or_else(|| if approved && r.profile.code == "UK" { "A1".into() } else { r.profile.status_code.into() }),
    );
    m.insert("status", if approved { "APPROVED IN AUDIT LOG - seal/sign by engineer".into() } else { "PRELIMINARY - NOT FOR CONSTRUCTION".into() });
    m.insert("scale", crate::units::scale_text(&meta.scale));
    m.insert("standard", info.standard.clone());
    m.insert("audit_head", info.audit_head.clone());
    m.insert("size", r.size.clone());
    m
}

/// Average advance of DejaVu Sans per mm of cap height (font size = cap height / 0.729).
const CW: f64 = 0.78;

/// Width budget for `wrap` (which counts 0.62 em per Latin character).
fn em(width: f64, h: f64) -> f64 {
    width / (h * CW) * 0.62
}

fn clip(s: &str, width: f64, h: f64) -> String {
    let max = ((width / (h * CW)).floor() as usize).max(1);
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

struct Col<'a> {
    d: &'a mut Drawing,
    x0: f64,
    x1: f64,
    y: f64,
    lab: f64,
}

impl Col<'_> {
    fn rule(&mut self) {
        self.d.line("TB", [self.x0, self.y], [self.x1, self.y]);
    }
    fn text(&mut self, s: &str, h: f64, max_lines: usize) {
        let em = em(self.x1 - self.x0 - 4.0, h);
        let ls = wrap(s, em);
        let n = ls.len();
        for (i, l) in ls.into_iter().enumerate().take(max_lines) {
            self.y -= h * 1.45;
            let l = if i + 1 == max_lines && n > max_lines { clip(&(l + " …"), self.x1 - self.x0 - 4.0, h) } else { l };
            self.d.text("TB-TEXT", [self.x0 + 2.0, self.y], h, &l);
        }
    }
    fn field(&mut self, label: &str, value: &str, h: f64, max_lines: usize) {
        self.y -= self.lab * 1.5;
        let lab = self.lab;
        self.d.text("TB-TEXT", [self.x0 + 2.0, self.y], lab, label);
        if value.is_empty() {
            self.y -= h * 1.45;
        } else {
            self.text(value, h, max_lines);
        }
        self.y -= 1.2;
        self.rule();
    }
}

fn rev_table(d: &mut Drawing, x0: f64, x1: f64, ytop: f64, revs: &[Revision], h: f64, rows: usize) -> f64 {
    let w = x1 - x0;
    // absolute widths for the short columns (REV, DATE, BY, CHK, APP), description takes the rest
    let u = h * CW;
    let fixed = [u * 3.8 + 1.5, u * 10.0 + 1.5, u * 3.8 + 1.5, u * 3.8 + 1.5, u * 3.8 + 1.5];
    let desc = (w - fixed.iter().sum::<f64>()).max(u * 4.0);
    let cols = [fixed[0] / w, fixed[1] / w, desc / w, fixed[2] / w, fixed[3] / w, fixed[4] / w];
    let heads = ["REV", "DATE", "DESCRIPTION", "BY", "CHK", "APP"];
    let rh = h * 2.0;
    let mut y = ytop - h * 1.6;
    d.text("TB-TEXT", [x0 + 2.0, y], h, "REVISIONS");
    y -= h * 0.8;
    let y_first = y;
    d.line("TB", [x0, y], [x1, y]);
    let put = |d: &mut Drawing, y: f64, vals: [&str; 6]| {
        let mut x = x0;
        for (i, v) in vals.iter().enumerate() {
            let cw = cols[i] * w;
            d.text("REV", [x + 0.8, y - rh + h * 0.55], h, &clip(v, cw - 1.2, h));
            x += cw;
        }
    };
    put(d, y, heads);
    y -= rh;
    d.line("TB", [x0, y], [x1, y]);
    let shown: Vec<&Revision> = revs.iter().rev().take(rows).collect();
    for k in 0..rows {
        if let Some(r) = shown.get(k) {
            put(d, y, [&r.rev, &r.date, &r.description, &r.by, &r.checked, &r.approved]);
        }
        y -= rh;
        d.line("TB", [x0, y], [x1, y]);
    }
    let mut x = x0;
    for c in cols.iter().take(5) {
        x += c * w;
        d.line("TB", [x, y_first], [x, y]);
    }
    y
}

fn north_arrow(d: &mut Drawing, c: [f64; 2], s: f64, h: f64) {
    d.fill("NORTH", vec![[c[0], c[1] + s], [c[0] - s * 0.35, c[1] - s * 0.6], [c[0], c[1] - s * 0.3]], [0.0, 0.0, 0.0]);
    d.poly("NORTH", vec![[c[0], c[1] + s], [c[0] + s * 0.35, c[1] - s * 0.6], [c[0], c[1] - s * 0.3], [c[0], c[1] + s]]);
    d.text("NORTH", [c[0] - h * 0.36, c[1] + s + 1.0], h, "N");
}

/// Graphic scale bar at the plotted scale (layout ratio / layout factor).
fn scale_bar(d: &mut Drawing, x: f64, y: f64, maxw: f64, n_layout: f64, k: f64, h: f64) {
    let n_plot = n_layout / k;
    let lf = crate::units::lf();
    let target_m = (maxw * n_plot / 1000.0 / lf).max(1e-6);
    let e = 10f64.powf(target_m.log10().floor());
    let l = [5.0, 2.0, 1.0].iter().map(|m| m * e).find(|v| *v <= target_m).unwrap_or(e);
    let len = l * lf * 1000.0 / n_plot;
    for i in 0..4 {
        let (a, b) = (x + len * i as f64 / 4.0, x + len * (i + 1) as f64 / 4.0);
        if i % 2 == 0 {
            d.fill("SCALEBAR", vec![[a, y], [b, y], [b, y + 1.6], [a, y + 1.6]], [0.0, 0.0, 0.0]);
        }
        d.poly("SCALEBAR", vec![[a, y], [b, y], [b, y + 1.6], [a, y + 1.6], [a, y]]);
    }
    let fmt = |v: f64| if v >= 1.0 { format!("{v:.0}") } else { format!("{v}") };
    d.text("SCALEBAR", [x - 0.7, y - h - 1.0], h, "0");
    if len > 28.0 {
        d.text("SCALEBAR", [x + len / 2.0 - 1.5, y - h - 1.0], h, &fmt(l / 2.0));
    }
    d.text("SCALEBAR", [x + len - 1.5, y - h - 1.0], h, &format!("{} {}", fmt(l), crate::units::lu()));
}

/// Place one A1-layout sheet on the resolved frame.
pub fn compose(content: &Drawing, r: &Resolved, info: &SheetInfo, id: &str) -> Drawing {
    let pf = &r.profile;
    let meta = content.meta.clone().unwrap_or(SheetMeta { title: "Sheet".into(), scale: "NTS".into(), page: 1, pages: 1 });
    let mut d = Drawing { units_mm: true, meta: Some(meta.clone()), ..Default::default() };
    for l in &content.layers {
        d.layer(&l.0, l.1);
    }
    for (l, c) in [
        ("FRAME", 7),
        ("FRAME-GRID", 8),
        ("TB", 7),
        ("TB-TEXT", 7),
        ("REV", 7),
        ("SEAL", 1),
        ("LOGO", 8),
        ("NORTH", 7),
        ("SCALEBAR", 7),
        ("STAMP", 1),
        ("TEMPLATE", 7),
    ] {
        d.layer(l, c);
    }
    let k = r.k;
    let (lw, lh) = (LAYOUT[2] - LAYOUT[0], LAYOUT[3] - LAYOUT[1]);
    let ox = r.area[0] + (r.area[2] - r.area[0] - lw * k) / 2.0;
    let oy = r.area[1] + (r.area[3] - r.area[1] - lh * k) / 2.0;
    let t = |p: [f64; 2]| [ox + (p[0] - LAYOUT[0]) * k, oy + (p[1] - LAYOUT[1]) * k];
    for e in &content.ents {
        d.ents.push(match e {
            Ent::Line { layer, a, b } => Ent::Line { layer: layer.clone(), a: t(*a), b: t(*b) },
            Ent::Poly { layer, pts } => Ent::Poly { layer: layer.clone(), pts: pts.iter().map(|p| t(*p)).collect() },
            Ent::Fill { layer, pts, rgb } => Ent::Fill { layer: layer.clone(), pts: pts.iter().map(|p| t(*p)).collect(), rgb: *rgb },
            Ent::Text { layer, p, h, s } => Ent::Text { layer: layer.clone(), p: t(*p), h: h * k, s: s.clone() },
        });
    }
    let fr = r.frame;
    let vals = field_values(r, info, &meta, id);
    let approved = info.approved_seq.is_some() && info.checks_failed == 0;
    let plan = is_plan(&meta.title);
    let n_layout = layout_scale(&meta.scale);
    let h0 = pf.min_text_mm;
    d.ents.extend(r.template_ents.iter().cloned());
    if r.tb_layout == "template" {
        let tpl = r.spec.template.as_ref().expect("template layout");
        for (f, p) in &tpl.field_positions {
            if let Some(v) = vals.get(f.as_str()) {
                let s = if f == "status" { "STAMP" } else { "TB-TEXT" };
                d.text(s, [p[0], p[1]], p[2].max(h0), v);
            } else if f == "north_arrow" && plan && r.north_arrow {
                north_arrow(&mut d, [p[0], p[1]], p[2].max(6.0), h0);
            } else if f == "scale_bar" && r.scale_bar {
                if let Some(n) = n_layout {
                    scale_bar(&mut d, p[0], p[1], 50.0, n, k, h0);
                }
            } else if f == "seal_box" {
                let s = p[2].max(30.0);
                d.rect("SEAL", p[0], p[1], p[0] + s, p[1] + s);
                d.text("SEAL", [p[0] + 1.5, p[1] + s - h0 * 1.5], h0, &clip(pf.seal_label, s - 3.0, h0));
                if !approved {
                    d.text("SEAL", [p[0] + 1.5, p[1] + 2.0], h0, &clip("NO SEAL - PRELIMINARY", s - 3.0, h0));
                }
            }
        }
        return finish(d, r);
    }
    d.rect("FRAME", fr[0], fr[1], fr[2], fr[3]);
    if pf.centring_marks {
        let (cx, cy) = (r.w / 2.0, r.h / 2.0);
        d.line("FRAME", [cx, fr[1] - r.margins[3] * 0.5], [cx, fr[1] + 5.0]);
        d.line("FRAME", [cx, fr[3] + r.margins[2] * 0.5], [cx, fr[3] - 5.0]);
        d.line("FRAME", [fr[0] - 5.0, cy], [fr[0] + 5.0, cy]);
        d.line("FRAME", [fr[2] + r.margins[1] * 0.5, cy], [fr[2] - 5.0, cy]);
    }
    if r.grid_reference {
        // fields 50 mm long measured from the centring marks; letters top-down (no I, O), numerals left-right
        let letters: Vec<char> = ('A'..='Z').filter(|c| *c != 'I' && *c != 'O').collect();
        let cuts = |c: f64, lo: f64, hi: f64| -> Vec<f64> {
            let mut v = vec![lo];
            let mut x = c - 50.0 * ((c - lo) / 50.0).floor();
            if x - lo < 10.0 {
                x += 50.0;
            }
            while x < hi - 10.0 {
                v.push(x);
                x += 50.0;
            }
            v.push(hi);
            v
        };
        let gx = cuts(r.w / 2.0, fr[0], fr[2]);
        let gy = cuts(r.h / 2.0, fr[1], fr[3]);
        let (tm, bm) = ((r.margins[2] * 0.6).min(6.0), (r.margins[3] * 0.6).min(6.0));
        for x in &gx[1..gx.len() - 1] {
            d.line("FRAME-GRID", [*x, fr[3]], [*x, fr[3] + tm]);
            d.line("FRAME-GRID", [*x, fr[1]], [*x, fr[1] - bm]);
        }
        let gh = 3.5f64.min(r.margins[2] * 0.4).max(1.8);
        for (i, w) in gx.windows(2).enumerate() {
            let xm = (w[0] + w[1]) / 2.0 - gh * 0.3;
            d.text("FRAME-GRID", [xm, fr[3] + (r.margins[2] - gh) / 2.0], gh, &(i + 1).to_string());
            d.text("FRAME-GRID", [xm, fr[1] - (r.margins[3] + gh) / 2.0], gh, &(i + 1).to_string());
        }
        for y in &gy[1..gy.len() - 1] {
            d.line("FRAME-GRID", [fr[0], *y], [fr[0] - 6.0f64.min(r.margins[0] * 0.5), *y]);
            d.line("FRAME-GRID", [fr[2], *y], [fr[2] + 6.0f64.min(r.margins[1] * 0.6), *y]);
        }
        let ny = gy.len() - 1;
        for (i, w) in gy.windows(2).enumerate() {
            let ym = (w[0] + w[1]) / 2.0 - gh / 2.0;
            let c = letters[(ny - 1 - i).min(letters.len() - 1)].to_string();
            d.text("FRAME-GRID", [fr[0] - r.margins[0] * 0.5 - gh * 0.3, ym], gh, &c);
            d.text("FRAME-GRID", [fr[2] + r.margins[1] * 0.5 - gh * 0.3, ym], gh, &c);
        }
    }
    let tb = r.tb;
    d.rect("TB", tb[0], tb[1], tb[2], tb[3]);
    let bottom = r.tb_layout == "bottom";
    let cols: Vec<[f64; 2]> = if bottom {
        let w = tb[2] - tb[0];
        let (c1, c2) = (tb[0] + w * 0.37, tb[0] + w * 0.69);
        d.line("TB", [c1, tb[1]], [c1, tb[3]]);
        d.line("TB", [c2, tb[1]], [c2, tb[3]]);
        vec![[tb[0], c1], [c1, c2], [c2, tb[2]]]
    } else {
        vec![[tb[0], tb[2]]]
    };
    let narrow = bottom || tb[2] - tb[0] < 150.0;
    let (hv, hh, ht) = if bottom {
        (h0, h0, pf.text_mm)
    } else if narrow {
        (h0, pf.text_mm, pf.heading_mm)
    } else {
        (pf.text_mm, pf.heading_mm, pf.title_mm)
    };
    let ml = if bottom { 2 } else { 3 };
    // 1: logo + legal owner, project, sheet title, identification
    let c = cols[0];
    let lh = if bottom { 10.0 } else { 18.0 };
    let logo = r.spec.fields.logo_text.clone().unwrap_or_else(|| "LOGO".into());
    let lx1 = c[0] + (c[1] - c[0]) * 0.3;
    d.rect("LOGO", c[0] + 2.0, tb[3] - lh, lx1, tb[3] - 2.0);
    d.text("LOGO", [c[0] + 3.5, tb[3] - lh / 2.0 - h0 / 2.0 - 0.5], h0, &clip(&logo, lx1 - c[0] - 4.0, h0));
    let mut oy = tb[3] - 1.5;
    for l in wrap(&vals["legal_owner"], em(c[1] - lx1 - 4.0, h0)).into_iter().take(if bottom { 2 } else { 3 }) {
        oy -= h0 * 1.45;
        d.text("TB-TEXT", [lx1 + 2.0, oy], h0, &l);
    }
    let mut col = Col { d: &mut d, x0: c[0], x1: c[1], y: tb[3] - lh - 1.0, lab: h0 };
    col.rule();
    col.field("PROJECT", &vals["project"], hh, ml);
    if !bottom && (!vals["client"].is_empty() || !vals["contract_no"].is_empty()) {
        let cc = format!("{} {}", vals["client"], vals["contract_no"]);
        col.field("CLIENT / CONTRACT", cc.trim(), hv, 2);
    }
    col.field("SHEET TITLE", &vals["sheet_title"], hh, ml.min(2));
    col.field("DRAWING / SHEET No.", &vals["sheet_id"], ht, 1);
    if bottom {
        col.field("SHEET / REVISION", &format!("Sheet {}  Rev {}", vals["sheet_no"], vals["revision"]), hv, 1);
        col.field("DATE OF ISSUE", &vals["date"], hv, 1);
    } else {
        let line = format!("Sheet {}  Rev {}  {}", vals["sheet_no"], vals["revision"], vals["date"]);
        col.field("SHEET / REVISION / DATE OF ISSUE", &line, hv, 1);
    }
    col.field("DOCUMENT TYPE / SHEET SIZE", &format!("{} - {} {}", vals["document_type"], r.size, r.orientation), hv, if bottom { 1 } else { 2 });
    let y1 = col.y;
    // 2: revisions + sign-off
    let (c2, y2) = if bottom { (cols[1], tb[3]) } else { (cols[0], y1) };
    let mut revs = r.spec.fields.revisions.clone();
    if revs.is_empty() {
        revs.push(Revision {
            rev: pf.initial_revision.into(),
            date: vals["date"].clone(),
            description: "Preliminary issue for review".into(),
            by: "AGT".into(),
            ..Default::default()
        });
    }
    let y = rev_table(&mut d, c2[0], c2[1], y2, &revs, h0, if bottom { 3 } else { 4 });
    let mut col = Col { d: &mut d, x0: c2[0], x1: c2[1], y, lab: h0 };
    let signoff = [("DESIGNED", "designed_by"), ("DRAWN (CREATOR)", "drawn_by"), ("CHECKED", "checked_by"), ("APPROVED (APPROVAL PERSON)", "approval_person")];
    for (lab, key) in signoff {
        let v = if key == "approval_person" && !approved { format!("{} - not yet approved", vals[key]) } else { vals[key].clone() };
        if bottom && c2[1] - c2[0] >= 100.0 {
            col.y -= h0 * 1.5;
            let t = clip(&format!("{lab}: {v}"), c2[1] - c2[0] - 4.0, h0);
            col.d.text("TB-TEXT", [c2[0] + 2.0, col.y], h0, &t);
        } else if bottom {
            col.y -= h0 * 1.45;
            col.d.text("TB-TEXT", [c2[0] + 2.0, col.y], h0, lab);
            col.y -= h0 * 1.45;
            let t = clip(&v, c2[1] - c2[0] - 6.0, h0);
            col.d.text("TB-TEXT", [c2[0] + 4.0, col.y], h0, &t);
        } else {
            col.field(lab, &v, h0, 1);
        }
    }
    let y2b = col.y;
    // 3: status, seal, north arrow and scale bar
    let (c3, y3) = if bottom { (cols[2], tb[3]) } else { (cols[0], y2b) };
    let mut col = Col { d: &mut d, x0: c3[0], x1: c3[1], y: y3 - 0.5, lab: h0 };
    let st_h = if bottom { h0 } else { hh };
    for l in wrap(&vals["status"], em(c3[1] - c3[0] - 4.0, st_h)) {
        col.y -= st_h * 1.4;
        col.d.text("STAMP", [c3[0] + 2.0, col.y], st_h, &l);
    }
    if !vals["status_code"].is_empty() {
        col.text(&format!("Status {} (ISO 19650-2 UK NA)", vals["status_code"]), h0, 1);
    }
    let head = &info.audit_head[..16.min(info.audit_head.len())];
    col.text(&format!("Checks {}/{} pass; audit {head}", info.checks_total - info.checks_failed, info.checks_total), h0, 1);
    if !bottom {
        col.text(&format!("Design standard: {}", vals["standard"]), h0, 2);
        col.text(&format!("Drafting profile: {}", pf.code), h0, 1);
    }
    if (k - 1.0).abs() > 0.005 {
        col.text(&format!("A1 layout at {:.1}% on {}: use the scale bar.", k * 100.0, r.size), h0, 2);
    }
    col.y -= 1.0;
    col.rule();
    let y4 = col.y;
    // north arrow + scale bar
    let bh = if bottom { 17.0 } else { 26.0 };
    let by1 = y4;
    let by0 = by1 - bh;
    d.line("TB", [c3[0], by0], [c3[1], by0]);
    let shown = crate::units::scale_text(&meta.scale);
    let sline = if n_layout.is_some() { format!("Scale {shown}") } else { format!("Scale {shown} (not to scale)") };
    d.text("TB-TEXT", [c3[0] + 2.0, by1 - h0 * 1.45], h0, &clip(&sline, c3[1] - c3[0] - 18.0, h0));
    if r.north_arrow && plan {
        north_arrow(&mut d, [c3[1] - 8.0, by0 + bh * 0.35], (bh * 0.3).min(8.0), h0);
    }
    if r.scale_bar {
        if let Some(n) = n_layout {
            scale_bar(&mut d, c3[0] + 3.0, by0 + h0 + 3.0, (c3[1] - c3[0]) - 26.0, n, k, h0);
        }
    }
    // seal box
    let avail = by0 - 1.5 - (tb[1] + 1.5);
    let sw = avail.min(c3[1] - c3[0] - 4.0).clamp(10.0, 75.0);
    let sy0 = tb[1] + 1.5;
    let sy1 = sy0 + avail.min(sw.max(avail.min(30.0)));
    let sx0 = c3[0] + ((c3[1] - c3[0]) - sw) / 2.0;
    d.rect("SEAL", sx0, sy0, sx0 + sw, sy1);
    if avail - (sy1 - sy0) > 20.0 {
        d.line("TB", [c3[0], sy1 + 1.5], [c3[1], sy1 + 1.5]);
        d.text("TB-TEXT", [c3[0] + 2.0, by0 - h0 * 1.6], h0, "NOTES / KEY PLAN");
    }
    d.text("SEAL", [sx0 + 1.5, sy1 - h0 * 1.45], h0, &clip(pf.seal_label, sw - 3.0, h0));
    if !approved {
        d.text("SEAL", [sx0 + 1.5, sy0 + 1.5], h0, &clip("NO SEAL - PRELIMINARY", sw - 3.0, h0));
    } else if let Some(seq) = info.approved_seq {
        d.text("SEAL", [sx0 + 1.5, sy0 + 1.5], h0, &clip(&format!("Approved, audit entry #{seq}"), sw - 3.0, h0));
    }
    finish(d, r)
}

fn ent_layer(e: &Ent) -> &String {
    match e {
        Ent::Line { layer, .. } | Ent::Poly { layer, .. } | Ent::Fill { layer, .. } | Ent::Text { layer, .. } => layer,
    }
}

fn finish(mut d: Drawing, r: &Resolved) -> Drawing {
    let pf = &r.profile;
    let mut names: Vec<String> = d.layers.iter().map(|l| l.0.clone()).collect();
    for e in &d.ents {
        let l = ent_layer(e);
        if !names.contains(l) {
            names.push(l.clone());
        }
    }
    for n in names {
        let (_, _, _, wc, lt) = layer_def(&n);
        let width = match n.as_str() {
            "FRAME" => pf.frame_line_mm,
            "FRAME-GRID" => 0.35,
            _ => pf.weights[wc],
        };
        d.styles.insert(n.clone(), LayerStyle { name: layer_name(pf, &n, &r.spec.layer_map), width_mm: width, ltype: lt.into() });
        d.layer(&n, 7);
    }
    d
}

/// A blank frame showing the drawing space and every title-block field.
pub fn preview(r: &Resolved, info: &SheetInfo) -> Drawing {
    let mut c = Drawing { units_mm: true, ..Default::default() };
    c.layer("BORDER", 7);
    c.layer("TEXT", 7);
    c.rect("BORDER", LAYOUT[0], LAYOUT[1], LAYOUT[2], LAYOUT[3]);
    c.line("BORDER", [LAYOUT[0], LAYOUT[1]], [LAYOUT[2], LAYOUT[3]]);
    c.line("BORDER", [LAYOUT[0], LAYOUT[3]], [LAYOUT[2], LAYOUT[1]]);
    c.text("TEXT", [LAYOUT[0] + 20.0, LAYOUT[3] - 30.0], 10.0, "DRAWING SPACE (A1 LAYOUT)");
    c.text("TEXT", [LAYOUT[0] + 20.0, LAYOUT[3] - 48.0], 6.0, &format!("{} {} - {} profile - layout factor {:.3}", r.size, r.orientation, r.country, r.k));
    c.meta = Some(SheetMeta { title: "Frame preview - plan".into(), scale: "1:500".into(), page: 1, pages: 1 });
    compose(&c, r, info, &sheet_ids(r, &["Plan preview".into()])[0])
}

// ---------------------------------------------------------------- compliance checks

fn bbox(e: &Ent) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut add = |p: [f64; 2]| {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    };
    match e {
        Ent::Line { a, b: c, .. } => {
            add(*a);
            add(*c);
        }
        Ent::Poly { pts, .. } | Ent::Fill { pts, .. } => pts.iter().for_each(|p| add(*p)),
        Ent::Text { p, h, s, .. } => {
            add(*p);
            add([p[0] + s.chars().count() as f64 * h * 0.7, p[1] + h]);
        }
    }
    b
}

pub const FRAME_LAYERS: &[&str] = &["FRAME", "FRAME-GRID", "TB", "TB-TEXT", "REV", "SEAL", "LOGO", "NORTH", "SCALEBAR", "TEMPLATE"];

/// Drawing-standard compliance of a composed set. Mandatory checks must all pass; advisory
/// items (layout scales outside the preferred series, small annotation after reduction) are reported.
pub fn check_set(sheets: &[Drawing], r: &Resolved, ids: &[String], info: &SheetInfo) -> Vec<Check> {
    let pf = &r.profile;
    let us = pf.code == "US";
    let mut out = vec![];
    if let Some(s) = SIZES.iter().find(|s| s.0 == r.size) {
        let (a, b) = if r.orientation == "landscape" { (s.1, s.2) } else { (s.2, s.1) };
        out.push(chk(
            "FR-01",
            "sheet size matches the named size",
            r.size.clone(),
            (r.w - a).abs() + (r.h - b).abs(),
            0.05,
            "mm",
            "max",
            "mandatory",
            "|w-W|+|h-H|".into(),
            s.3.into(),
        ));
    }
    if r.tb_layout != "template" {
        let mm = r.margins.iter().zip(pf.min_margins.iter()).map(|(a, b)| a - b).fold(f64::MAX, f64::min);
        out.push(chk(
            "FR-02",
            "borders not less than the profile minimum (L, R, T, B)",
            format!("{:?}", r.margins),
            mm,
            0.0,
            "mm",
            "min",
            "mandatory",
            "min(margin_i - min_i)".into(),
            if us { "State DOT CADD manual (engineer input)".into() } else { ISO5457.into() },
        ));
    }
    let n = sheets.len();
    let approved = info.approved_seq.is_some() && info.checks_failed == 0;
    let mut missing = 0usize;
    let mut miss_names = std::collections::BTreeSet::new();
    let (mut outside, mut small_text, mut no_bar, mut no_north, mut no_seal, mut sealed_prelim) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut outside_where = std::collections::BTreeSet::new();
    let mut bad_layers = std::collections::BTreeSet::new();
    let mut nbad_layers = 0usize;
    let (mut min_tb_text, mut min_content_text) = (f64::MAX, f64::MAX);
    let mut off_series = std::collections::BTreeSet::new();
    let custom = !r.spec.layer_map.is_empty();
    let tpl = r.spec.template.as_ref().filter(|_| r.tb_layout == "template");
    for (i, d) in sheets.iter().enumerate() {
        let meta = d.meta.clone().unwrap_or_default();
        let vals = field_values(r, info, &meta, &ids[i]);
        let tb_texts: Vec<&String> = d
            .ents
            .iter()
            .filter_map(|e| if let Ent::Text { layer, s, .. } = e { ["TB-TEXT", "STAMP", "REV", "SEAL"].contains(&layer.as_str()).then_some(s) } else { None })
            .collect();
        for f in &pf.required_fields {
            let ok = match *f {
                "seal_box" => d.ents.iter().any(|e| matches!(e, Ent::Poly { layer, .. } if layer == "SEAL")),
                "checked_by" if !approved && vals.get("checked_by").is_some_and(|v| v.is_empty()) => {
                    tb_texts.iter().any(|s| s.starts_with("CHECKED")) || tpl.is_some_and(|t| t.field_positions.contains_key("checked_by"))
                }
                "north_arrow" => !is_plan(&meta.title) || !r.north_arrow || d.ents.iter().any(|e| matches!(e, Ent::Fill { layer, .. } if layer == "NORTH")),
                key => {
                    let v = vals.get(key).cloned().unwrap_or_default();
                    let probe: String = v.chars().take(6).collect();
                    !v.is_empty() && tb_texts.iter().any(|s| s.contains(&probe)) && tpl.is_none_or(|t| t.field_positions.contains_key(key))
                }
            };
            if !ok {
                missing += 1;
                miss_names.insert(f.to_string());
            }
        }
        for e in &d.ents {
            let l = ent_layer(e).as_str();
            let frame_layer = FRAME_LAYERS.contains(&l) || l == "STAMP";
            if let Ent::Text { h, .. } = e {
                if frame_layer && l != "TEMPLATE" {
                    min_tb_text = min_tb_text.min(*h);
                } else if !frame_layer {
                    min_content_text = min_content_text.min(*h);
                    if *h < pf.min_text_mm - 1e-6 {
                        small_text += 1;
                    }
                }
            }
            if !frame_layer {
                let b = bbox(e);
                let a = r.area;
                if b[0] < a[0] - 0.5 || b[1] < a[1] - 0.5 || b[2] > a[2] + 0.5 || b[3] > a[3] + 0.5 {
                    outside += 1;
                    let what = if let Ent::Text { s, .. } = e { format!("text '{}'", s.chars().take(30).collect::<String>()) } else { l.to_string() };
                    outside_where.insert(format!("sheet {} {what}", i + 1));
                }
            }
        }
        for st in d.styles.values() {
            if !layer_name_ok(pf, &st.name, custom) {
                nbad_layers += 1;
                bad_layers.insert(st.name.clone());
            }
        }
        let nl = layout_scale(&meta.scale);
        if r.scale_bar
            && nl.is_some()
            && !d.ents.iter().any(|e| matches!(e, Ent::Fill { layer, .. } if layer == "SCALEBAR"))
            && tpl.is_none_or(|t| t.field_positions.contains_key("scale_bar"))
        {
            no_bar += 1;
        }
        if let Some(v) = nl {
            if !pf.preferred_scales.iter().any(|s| (s - v).abs() < 1e-6) {
                off_series.insert(format!("1:{v:.0}"));
            }
        }
        if r.north_arrow
            && is_plan(&meta.title)
            && !d.ents.iter().any(|e| matches!(e, Ent::Fill { layer, .. } if layer == "NORTH"))
            && tpl.is_none_or(|t| t.field_positions.contains_key("north_arrow"))
        {
            no_north += 1;
        }
        if !d.ents.iter().any(|e| matches!(e, Ent::Poly { layer, .. } if layer == "SEAL")) {
            no_seal += 1;
        }
        if !approved && d.ents.iter().any(|e| matches!(e, Ent::Text { layer, s, .. } if layer == "SEAL" && s.starts_with("Approved"))) {
            sealed_prelim += 1;
        }
    }
    let src = |iso: &str, other: &str| -> String {
        if us {
            other.into()
        } else {
            iso.into()
        }
    };
    out.push(chk(
        "FR-03",
        "drawing-space frame line width",
        "frame".into(),
        pf.frame_line_mm,
        if us { 0.5 } else { 0.7 },
        "mm",
        "min",
        "mandatory",
        "frame layer plot weight".into(),
        src(ISO5457, "agency border cell (engineer input)"),
    ));
    if r.tb_layout == "right" && !us {
        out.push(chk(
            "FR-04",
            "title block width (ISO 7200: 180 mm)",
            format!("{:.0} mm", r.tb[2] - r.tb[0]),
            r.tb[2] - r.tb[0],
            180.0,
            "mm",
            "max",
            "advisory",
            "block width".into(),
            ISO7200.into(),
        ));
    }
    out.push(chk(
        "FR-05",
        "mandatory title-block fields present on every sheet",
        if miss_names.is_empty() {
            format!("{n} sheets: {}", pf.required_fields.join(", "))
        } else {
            format!("missing: {}", miss_names.into_iter().collect::<Vec<_>>().join(", "))
        },
        missing as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "(sheet, field) pairs without a value on the sheet".into(),
        src(ISO7200, "State DOT CADD manual title block (engineer input); NCS UDS"),
    ));
    let uniq: std::collections::BTreeSet<&String> = ids.iter().collect();
    let bad_ids = ids.iter().filter(|i| !sheet_id_ok(r, i)).count() + (ids.len() - uniq.len());
    out.push(chk(
        "FR-06",
        "sheet identifiers unique and in the profile format",
        format!("{} .. {}", ids.first().cloned().unwrap_or_default(), ids.last().cloned().unwrap_or_default()),
        bad_ids as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "format + uniqueness".into(),
        match pf.sheet_id {
            SheetIdFmt::Uds => NCS_UDS.into(),
            SheetIdFmt::Uk19650 => UKNA.into(),
            SheetIdFmt::Agency => "agency drawing register numbering (engineer input)".into(),
        },
    ));
    let revs: Vec<String> =
        if r.spec.fields.revisions.is_empty() { vec![pf.initial_revision.into()] } else { r.spec.fields.revisions.iter().map(|x| x.rev.clone()).collect() };
    let bad_rev = revs.iter().filter(|x| !revision_ok(r, x, !approved)).count();
    out.push(chk(
        "FR-07",
        "revision codes follow the profile convention (no contractual revision while preliminary)",
        revs.join(","),
        bad_rev as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "revision pattern".into(),
        if pf.code == "UK" { UKNA.into() } else { src(ISO7200, "State DOT CADD manual (engineer input)") },
    ));
    out.push(chk(
        "FR-08",
        "engineer seal / sign-off box on every sheet",
        format!("{n} sheets"),
        no_seal as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "sheets without a seal box".into(),
        pf.seal_rule.into(),
    ));
    out.push(chk(
        "FR-08b",
        "no seal or approval shown while the set is preliminary",
        "seal box".into(),
        sealed_prelim as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "approval text without a recorded approval".into(),
        "civil-mcp approval gate".into(),
    ));
    if min_tb_text < f64::MAX {
        out.push(chk(
            "FR-09",
            "title-block text height",
            "title block".into(),
            min_tb_text,
            pf.min_text_mm,
            "mm",
            "min",
            "mandatory",
            "smallest plotted text".into(),
            src(ISO3098, "State DOT CADD manual (engineer input)"),
        ));
    }
    if min_content_text < f64::MAX {
        out.push(chk(
            "FR-09b",
            "drawing annotation below the minimum text height (count)",
            format!("smallest {:.2} mm at layout factor {:.3}", min_content_text, r.k),
            small_text as f64,
            0.0,
            "count",
            "max",
            "advisory",
            "texts with plotted height < minimum".into(),
            ISO3098.into(),
        ));
    }
    out.push(chk(
        "FR-10",
        "drawing content inside the drawing space",
        if outside_where.is_empty() { r.size.clone() } else { outside_where.into_iter().take(6).collect::<Vec<_>>().join(", ") },
        outside as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "entities whose extents leave the drawing space".into(),
        ISO5457.into(),
    ));
    out.push(chk(
        "FR-11",
        "layer names follow the profile convention",
        if bad_layers.is_empty() {
            format!("{:?}{}", pf.layer_fmt, if custom { " + layer_map" } else { "" })
        } else {
            bad_layers.into_iter().take(5).collect::<Vec<_>>().join(", ")
        },
        nbad_layers as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "layer name pattern".into(),
        if pf.layer_fmt == LayerFmt::Ncs { NCS_LAYER.into() } else { BS1192.into() },
    ));
    out.push(chk(
        "FR-12",
        "graphic scale bar on every sheet with a numeric scale",
        format!("layout factor {:.3}", r.k),
        no_bar as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "sheets with 1:n but no bar".into(),
        "graphic scale stays true at any print size".into(),
    ));
    out.push(chk(
        "FR-12b",
        "layout scales in the preferred series",
        if off_series.is_empty() { "all".into() } else { off_series.iter().cloned().collect::<Vec<_>>().join(", ") },
        off_series.len() as f64,
        0.0,
        "count",
        "max",
        "advisory",
        "distinct layout scales outside the series".into(),
        src(ISO5455, "agency engineering scales (engineer input)"),
    ));
    out.push(chk(
        "FR-13",
        "north arrow on plan sheets",
        "plan sheets".into(),
        no_north as f64,
        0.0,
        "count",
        "max",
        "mandatory",
        "plan sheets without a north arrow".into(),
        "drafting practice (NCS, AS 1100.401, agency manuals)".into(),
    ));
    if r.grid_reference && tpl.is_none() {
        let has = sheets.iter().filter(|d| d.ents.iter().any(|e| matches!(e, Ent::Text { layer, .. } if layer == "FRAME-GRID"))).count();
        out.push(chk(
            "FR-14",
            "grid reference system and centring marks on every sheet",
            "border".into(),
            (n - has) as f64,
            0.0,
            "count",
            "max",
            "mandatory",
            "sheets without grid references".into(),
            ISO5457.into(),
        ));
    }
    out
}

// ---------------------------------------------------------------- tools

pub const TOOLS: &[&str] = &["list_sheet_frames", "get_drafting_standard", "set_sheet_frame", "preview_sheet_frame", "check_drawing_standards"];

pub fn spec_schema() -> Value {
    let num = json!({"type":"number"});
    let b = json!({"type":"boolean"});
    json!({"type":"object","description":"Sheet frame. Every field is optional; omitted values come from the drafting profile.","properties":{
        "profile":{"type":"string","enum":["US","CA","UK","AU","NZ"],"description":"drafting standard profile; default = design-basis country"},
        "size":{"type":"string","description":"ISO A0-A4 (or A0-A4), ANSI A-E, ARCH A-E1, or CUSTOM with width_mm/height_mm"},
        "width_mm":num,"height_mm":num,
        "orientation":{"type":"string","enum":["landscape","portrait"]},
        "margins_mm":{"type":"array","items":num,"minItems":4,"maxItems":4,"description":"left, right, top, bottom"},
        "title_block_width_mm":num,"grid_reference":b,"north_arrow":b,"scale_bar":b,
        "fields":{"type":"object","description":"title block: project, client, legal_owner, contract_no, project_code, originator, designed_by, drawn_by, checked_by, approved_by, date, document_type, status_code, logo_text, revisions [{rev, date, description, by, checked, approved}]"},
        "template":{"type":"object","description":"custom frame: path + format (dxf | svg | json) or inline entities {lines, polylines, texts} in sheet mm; drawing_area [x0,y0,x1,y1]; field_positions {field: [x, y, h]} (fields: project, client, legal_owner, sheet_title, sheet_id, sheet_no, date, revision, scale, status, status_code, drawn_by, checked_by, designed_by, approval_person, document_type, seal_box, north_arrow, scale_bar); replace_title_block"},
        "layer_map":{"type":"object","description":"internal layer -> exported layer name (agency level library)"},
        "sheet_id_pattern":{"type":"string","description":"e.g. '{project}-C-{n:04}'"}}})
}

pub fn tool_list() -> Vec<Value> {
    let spec = spec_schema();
    vec![
        json!({"name":"list_sheet_frames","description":"List sheet sizes (ISO A0-A4, ANSI A-E, ARCH A-E1), the Five Eyes drafting profiles with their default frame, and the current frame.","inputSchema":{"type":"object","properties":{}}}),
        json!({"name":"get_drafting_standard","description":"Drafting/CAD standard profile for one country (US, CA, UK, AU, NZ): sheet sizes, borders, title-block fields, line weights and types, text heights, scales, layer naming with the layer map, sheet/file naming, revision and seal rules. Each parameter is marked verified / formula / engineer with its source.","inputSchema":{"type":"object","properties":{"country":{"type":"string","enum":["US","CA","UK","AU","NZ"]}},"required":["country"]}}),
        json!({"name":"set_sheet_frame","description":"Set the sheet frame and title block used by generate_drawings for every sheet (PDF, SVG, DXF): drafting profile, size, orientation, margins, title-block fields and revisions, custom DXF/SVG/JSON template, layer map, sheet id pattern. Selecting a profile defaults the frame, layers and styles. Recorded in the audit log; does not change the design or void an approval.","inputSchema":{"type":"object","properties":{"frame":spec},"required":["frame"]}}),
        json!({"name":"preview_sheet_frame","description":"Render the current (or a given) frame with an empty drawing space to frame-preview.pdf/.svg/.dxf; returns the layout (drawing area, title block, layout factor) and the frame compliance checks.","inputSchema":{"type":"object","properties":{"frame":spec}}}),
        json!({"name":"check_drawing_standards","description":"Drawing-standard compliance checks of the last generate_drawings run: sheet size, borders, frame line, title-block fields, sheet ids, revision codes, seal box, text heights, content inside the drawing space, layer names, scale bar, preferred scales, north arrow, grid references.","inputSchema":{"type":"object","properties":{}}}),
    ]
}

pub fn profile_json(s: Standard) -> Value {
    let p = profile(s);
    let empty = BTreeMap::new();
    let layers = [
        "ALIGNMENT",
        "PROFILE-DESIGN",
        "PROFILE-GROUND",
        "XS-DESIGN",
        "XS-GROUND",
        "XS-DATUM",
        "CONTOUR-MAJOR",
        "CONTOUR-MINOR",
        "DAYLIGHT",
        "CURB",
        "WALL",
        "BARRIER",
        "PIPE",
        "STRUCTURE",
        "HGL",
        "EGL",
        "CULVERT",
        "ROW",
        "PARCEL",
        "UTILITY",
        "SIGN",
        "SIGNAL",
        "LIGHT",
        "TTM",
        "TEXT",
        "TITLE",
        "FRAME",
        "TB",
        "TB-TEXT",
        "SEAL",
    ];
    let map: Vec<Value> = layers
        .iter()
        .map(|l| {
            let (_, _, _, wc, lt) = layer_def(l);
            json!({"internal": l, "name": layer_name(&p, l, &empty), "weight_mm": if *l == "FRAME" { p.frame_line_mm } else { p.weights[wc] }, "linetype": lt})
        })
        .collect();
    let cnt = |st: &str| p.params.iter().filter(|x| x.status == st).count();
    json!({
        "code": p.code, "name": p.name, "default_size": p.default_size, "margins_mm": p.margins, "title_block_width_mm": p.tb_width,
        "grid_reference": p.grid_reference, "centring_marks": p.centring_marks, "frame_line_mm": p.frame_line_mm,
        "text_mm": {"min": p.min_text_mm, "values": p.text_mm, "headings": p.heading_mm, "sheet_number": p.title_mm},
        "line_weights_mm": {"extra_thin": p.weights[0], "thin": p.weights[1], "medium": p.weights[2], "thick": p.weights[3]},
        "layer_format": p.layer_fmt, "layers": map, "sheet_id_format": p.sheet_id,
        "initial_revision": p.initial_revision, "status_code": p.status_code, "seal_label": p.seal_label, "seal_rule": p.seal_rule,
        "required_title_block_fields": p.required_fields,
        "preferred_scales": p.preferred_scales.iter().map(|v| format!("1:{v}")).collect::<Vec<_>>(),
        "parameters": p.params,
        "counts": {"verified": cnt("verified"), "formula": cnt("formula"), "engineer": cnt("engineer")}
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_resolution() {
        assert_eq!(size_lookup("a1").unwrap(), ("ISO A1", 841.0, 594.0));
        assert_eq!(size_lookup("ANSI D").unwrap().1, 863.6);
        assert!(size_lookup("B7").is_none());
        // ISO profile: 20/10 mm borders and a 180 mm title block on A1
        let r = resolve(&FrameSpec { profile: Some("UK".into()), ..Default::default() }, Standard::Us).unwrap();
        assert_eq!(r.frame, [20.0, 10.0, 831.0, 584.0]);
        assert_eq!(r.tb[2] - r.tb[0], 180.0);
        assert!(r.k > 0.9 && r.k < 1.0);
        // portrait A4 puts the title block along the lower edge
        let r = resolve(
            &FrameSpec { profile: Some("AU".into()), size: Some("A4".into()), orientation: Some("portrait".into()), ..Default::default() },
            Standard::Us,
        )
        .unwrap();
        assert_eq!((r.w, r.h, r.tb_layout.as_str()), (210.0, 297.0, "bottom"));
    }

    #[test]
    fn layer_and_id_conventions() {
        let us = profile(Standard::Us);
        let uk = profile(Standard::Uk);
        let m = BTreeMap::new();
        assert_eq!(layer_name(&us, "ALIGNMENT", &m), "C-ROAD-CNTR");
        assert_eq!(layer_name(&uk, "ALIGNMENT", &m), "C-00-M_Alignment");
        assert!(layer_name_ok(&us, "C-ROAD-CNTR", false) && layer_name_ok(&us, "G-ANNO-TTLB-TEXT", false) && layer_name_ok(&us, "C-ROAD-CNTR-N", false));
        assert!(!layer_name_ok(&us, "ROAD_CL", false) && !layer_name_ok(&us, "C-RD-CNTR", false));
        assert!(layer_name_ok(&uk, "C-00-M_Alignment", false) && !layer_name_ok(&uk, "C-00-X_Alignment", false));
        // every internal layer used by the sheet generators maps to a valid NCS name
        for l in ["ALIGNMENT", "OSM-ROAD-MAJOR", "ROUTE-SELECTED", "XS-DATUM", "FRAME", "TB-TEXT", "SCALEBAR", "TEMPLATE", "SOMETHING-NEW"] {
            assert!(layer_name_ok(&us, &layer_name(&us, l, &m), false), "{l}");
            assert!(layer_name_ok(&uk, &layer_name(&uk, l, &m), false), "{l}");
        }
        let r = resolve(&FrameSpec { profile: Some("US".into()), ..Default::default() }, Standard::Us).unwrap();
        let ids = sheet_ids(
            &r,
            &[
                "Cover, sheet index".into(),
                "Plan and profile".into(),
                "Cross sections (1 of 2)".into(),
                "Cross sections (2 of 2)".into(),
                "Standard details atlas".into(),
            ],
        );
        assert_eq!(ids, ["G-001", "C-101", "C-301", "C-302", "C-501"]);
        assert!(ids.iter().all(|i| sheet_id_ok(&r, i)));
        let r = resolve(&FrameSpec { profile: Some("UK".into()), ..Default::default() }, Standard::Us).unwrap();
        assert!(revision_ok(&r, "P01", true) && !revision_ok(&r, "C01", true) && revision_ok(&r, "C01", false) && !revision_ok(&r, "A", true));
        let r = resolve(&FrameSpec { profile: Some("NZ".into()), ..Default::default() }, Standard::Us).unwrap();
        assert!(revision_ok(&r, "B", true) && !revision_ok(&r, "I", true) && !revision_ok(&r, "O", true) && revision_ok(&r, "2", false));
    }

    #[test]
    fn templates_parse() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="420mm" height="297mm" viewBox="0 0 840 594"><path d="M10 10 h100 v50 z"/><text x="20" y="40" font-size="10">T</text></svg>"#;
        let e = parse_svg(svg).unwrap();
        assert_eq!(e.len(), 2);
        if let Ent::Poly { pts, .. } = &e[0] {
            // viewBox units are half millimetres, y flipped
            assert_eq!(pts[0], [5.0, 292.0]);
            assert_eq!(pts[1], [55.0, 292.0]);
            assert_eq!(pts.len(), 4);
        } else {
            panic!()
        }
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\n0\n90\n2\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n0\nTEXT\n8\n0\n10\n1\n20\n2\n40\n3.5\n1\nHELLO\n0\nENDSEC\n0\nEOF\n";
        let e = parse_dxf(dxf).unwrap();
        assert!(matches!(&e[0], Ent::Poly { pts, .. } if pts.len() == 3));
        assert!(matches!(&e[1], Ent::Text { s, h, .. } if s == "HELLO" && *h == 3.5));
        assert_eq!(layout_scale("plan 1:500; profile as shown"), Some(500.0));
        assert_eq!(layout_scale("NTS"), None);
        assert_eq!(today().len(), 10);
    }
}
