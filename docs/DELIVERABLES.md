# Road design deliverables by country and stage (research for civil-mcp v4)

Researched 2026-10-20. Sources are cited inline as [n] (list at the end). This is a summary of public guidance, written so the scope of civil-mcp can be set from what agencies actually require. It is not a substitute for the contract's own requirements.

## 1. Design stages

| Country | Stages (public process documents) | Safety audit hooks | Departures / exceptions |
|---|---|---|---|
| US (FHWA / state DOTs) | TxDOT PS&E milestones: Initial 30%, Preliminary Bridge Layout, Detailed 60%, Final 90/95%, Sealed 100%, Ready to Let [1][2]. Caltrans PPM defines the plan composition and sheet order [5][6]. | Road safety audits are optional (FHWA RSA program), with agency-specific triggers. | Design exceptions for the controlling criteria (FHWA), documented per the state DOT. |
| Canada | Ontario MTO HPDPG: Planning → Preliminary Design ("design criteria level, roughing out") → Detail Design (initial ≤30%, final 30–60%, contract preparation 60–100%, review, tendering) [7][8]. BC MoTI (BC Supplement to TAC, Ch. 100): Preliminary → Functional → Detailed design. Ch. 1200 covers drawing presentation and CAD layers [9][10]. | Agency RSA policies (TAC Canadian RSA guide). | Design exceptions per the BC Supplement / MTO design criteria. |
| UK (National Highways, DMRB) | Project Control Framework stages: options → preliminary design → construction preparation (detailed design) [12]. GG 101 governs DMRB application [11]. | GG 119 Road Safety Audit: Stage 1 at the end of preliminary design, Stage 2 at the end of detailed design [12]. GG 104 Safety Risk Assessment across all stages [13]. | GG 101 §2.4–2.5: departures must be approved before the design is finalised and before they are built into the works; bulk departures are allowed [11]. |
| Australia (Austroads + TfNSW) | Austroads AGRD Part 1 §4.2: Phase 1 establish the preferred solution, Phase 2 develop it, Phase 3 detailed design. Appendix A covers design documentation [14]. TfNSW WAD process: concept design review → detailed design review → "Accepted for Construction" [15]. | TfNSW: independent Stage 3 RSA at detailed design [15]. | AGRD Part 1 Ed 5.2 §4.4–4.6: extended design domain and design exceptions, with a register template [14]. |
| New Zealand (NZTA Waka Kotahi) | Business-case phases (indicative / detailed business case → pre-implementation → implementation) [18]. Austroads GRD is the primary geometric reference; the SHGDM is supplementary [19]. | Safe System audits at concept/scheme, preliminary, detailed and pre-opening stages [20]. | Exception report reviewed against Austroads Parts 3/4/6 [21]. |

## 2. Deliverables matrix

Abbreviations: C = concept / preliminary (US 30%, MTO preliminary, PCF options/preliminary, AGRD Phase 1–2, NZ DBC); D = detailed (US 60–100%, MTO detail, PCF construction preparation, AGRD Phase 3, NZ pre-implementation). Status: ✅ implemented in civil-mcp, 🟡 partial, ❌ gap.

### 2a. Calculations

| Item | US | CA | UK | AU | NZ | Stage | civil-mcp |
|---|---|---|---|---|---|---|---|
| Route / option assessment (alternatives, cost proxy, selection record) | ✓ | ✓ | ✓ (PCF option selection) | ✓ (Phase 1) | ✓ (DBC) | C | see §3 |
| Horizontal / vertical geometry against standard (SSD, R, K, grades) | ✓ | ✓ | ✓ CD 109 | ✓ AGRD3 | ✓ AGRD3 | C, D | see §3 |
| Superelevation development | ✓ | ✓ | ✓ | ✓ | ✓ | C (basic), D | see §3 |
| Cross sections and earthworks volumes, mass haul, balance | ✓ | ✓ | ✓ | ✓ | ✓ | C, D | see §3 |
| Pavement design (flexible / rigid, subgrade, traffic) | ✓ AASHTO 93 / ME | ✓ | ✓ CD 224/225/226 | ✓ AGPT02 | ✓ AGPT02 + NZ supplement | C (indicative), D | see §3 |
| Drainage: hydrology (rational / ARR), storm network, HGL, inlets, culverts, ditch lining | ✓ HEC-22, HEC-15 | ✓ | ✓ CG 501, CD 521 | ✓ ARR 2019, AGRD5 | ✓ | C (strategy), D | see §3 |
| Sanitary / foul sewer (if in scope) | ✓ | ✓ | ✓ DCG/SfA | ✓ WSAA | ✓ | D | see §3 |
| Retaining walls / earth structures (geotechnical interface) | ✓ | ✓ | ✓ CD 622, CG 300 | ✓ | ✓ Bridge Manual §6 | D | see §3 |
| Road restraint: clear zone, barrier need and length | ✓ RDG | ✓ | ✓ CD 377 (RRRAP) | ✓ AGRD6 | ✓ AGRD6 | C, D | see §3 |
| Intersections: geometry, sight distance, turning paths | ✓ | ✓ | ✓ CD 123 | ✓ AGRD4/4A | ✓ | C, D | see §3 |
| Traffic signals / signing / markings / lighting | ✓ | ✓ | ✓ | ✓ | ✓ | D | see §3 |
| Traffic management / staging | ✓ | ✓ | ✓ | ✓ | ✓ | D | see §3 |
| Structures (bridges, culverts > threshold) | ✓ | ✓ | ✓ | ✓ | ✓ Bridge Manual | C, D | see §3 |
| Cost estimate (parametric at C, unit cost at D) | ✓ | ✓ MTO estimating guide [8] | ✓ | ✓ | ✓ | C, D | see §3 |

### 2b. Reports and registers

| Report | US | CA | UK | AU | NZ | civil-mcp |
|---|---|---|---|---|---|---|
| Design report / design philosophy statement | ✓ | ✓ | ✓ | ✓ AGRD1 App A, TfNSW (major WAD) [15] | ✓ | see §3 |
| Drainage report | ✓ | ✓ | ✓ | ✓ TfNSW detailed [15] | ✓ | see §3 |
| Pavement design report | ✓ | ✓ | ✓ | ✓ TfNSW detailed [15] | ✓ | see §3 |
| Geotechnical report (interface: inputs CBR/Mr, slope/wall parameters) | ✓ | ✓ | ✓ | ✓ | ✓ | interface: inputs + slope-sections.csv; the report itself needs site investigation (see §3) |
| Departures / design exceptions register | ✓ | ✓ | ✓ GG 101 | ✓ AGRD1 §4.6 | ✓ | see §3 |
| Road safety audit / Safe System audit hooks | opt. | ✓ | ✓ GG 119, GG 104 SRA | ✓ RSA3 | ✓ SSA | see §3 (hooks only; audit is independent and human) |
| Quantities / BoQ by measurement method | state pay items | OPSS tender items | MCHW Vol 4 MMHW Series 500/600/700 [16] | agency spec items | NZTA items | see §3 |
| CAD/design-output reports (alignment report, surface checks) | ✓ | ✓ BC Ch.1200 | ✓ | ✓ TfNSW CADD Manual: alignment details, surface checker, conflict analysis reports [17] | ✓ | see §3 |
| Design certificate / design review certificate | PE seal | P.Eng seal | Designer's certificate | Project Verifier certificate [15] | Design + design review certificates [18] | human act; tool provides the review package only |

### 2c. Drawings

TxDOT standard sheet sequence [1]: title, index, project layout, roadway typical sections, general notes, estimate and quantity, quantity summaries, traffic control, survey control, horizontal/vertical control, alignment data, removals, roadway plan and profile, intersection details, driveway details, misc. roadway details, retaining wall layouts and details, drainage area map, hydrologic and hydraulic data sheets, culvert layouts, drainage plan and profile, misc. drainage details (pipe bedding, collars, …), utilities, bridges, traffic items, environmental (SWP3) and standards. Caltrans PPM order [5][6]: title, typical cross sections, key map, layouts, profiles and superelevation diagrams, construction details, drainage plans, profiles, details and quantities, and summary of quantities. BC MoTI detailed design [9]: front page, key plan, plans, profiles, sections, geometrics/laning, spot elevations, signing/markings, drainage/utilities, quantities. TfNSW [17]: A1 PDF + CAD, specified scales, longitudinal sections at 5:1 exaggeration.

| Drawing type | Stage | US | CA | UK | AU | NZ | civil-mcp |
|---|---|---|---|---|---|---|---|
| Title / cover sheet, sheet index, general notes, legend | C, D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Key plan / project layout | C, D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Survey / horizontal and vertical control, alignment data (setting-out) | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Typical cross sections | C, D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Plan and profile | C, D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Superelevation diagram | D | ✓ Caltrans | ✓ | ✓ | ✓ | ✓ | see §3 |
| Cross sections (every station) | C (key), D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Earthworks / mass haul diagram | C, D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Pavement layout and details | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Intersection details / geometrics / laning | C, D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Retaining wall layouts and details | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Drainage area map, H&H data sheets | C, D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Drainage (storm) plan and profile, structure schedule | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Sanitary plan and profile | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Misc. drainage details (bedding, manholes, inlets) and standard details | D | ✓ | ✓ OPSD | ✓ HCD | ✓ | ✓ | see §3 |
| Road restraint / barrier layout | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Quantity summary sheets | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |
| Traffic control, signing, markings, signals, lighting, utilities, ROW/land, environmental, landscaping, structures | D | ✓ | ✓ | ✓ | ✓ | ✓ | see §3 |

## 3. civil-mcp v5 coverage (generated from `deliverables-status.csv`, written by `generate_deliverables`)

Totals: 52 implemented, 1 partial, 2 interface (licensed-specialist hand-off only), 0 gap (of 55 items). "Implemented" means the software produces the item for the preliminary / concept design stage; every item still needs review and sealing by the engineer in responsible charge. "Interface" means the software writes a data hand-off template and a deliverables checklist for a licensed specialist and does not produce the item itself.

### 3a. Calculations

| Item | Status | Output / note |
|---|---|---|
| Route / corridor options and selection (options report) | implemented | route-alternatives.json, design-report.md §2 |
| Horizontal alignment geometry and standards checks | implemented | setting-out.csv, design-report.md §3, review.html |
| Vertical alignment, stopping sight distance, vertical-curve K | implemented | design-report.md §3 |
| Superelevation and development lengths | implemented | design-report.md §3, superelevation diagram sheet |
| Lane widening on curves | implemented | AASHTO off-tracking formula or engineer table |
| Earthwork volumes, mass haul, free haul/overhaul, borrow/waste, shrink/bulk | implemented | quantities.json, design-report.md §5, mass-haul sheet |
| Profile optimisation for earthwork balance under standards | implemented | profile-optimisation.json |
| Pavement thickness design | implemented | US/CA AASHTO 1993 flexible+rigid; AU/NZ AGPT02 empirical granular; UK CD 226 Eq 2.24/2.43 with Fig 2.20 values by engineer; mechanistic-empirical design by multilayer linear-elastic analysis (Burmister, dual wheels; verified vs Boussinesq, the published KENPAVE table and PyMastic) with AGPT02 / Asphalt Institute transfer functions and thickness stepping; AU/NZ rigid base thickness in the AGPT02 Ch. 9 structure (Westergaard + PCA relations replace the copyrighted tables: confirm with AGPT02). Limits: isotropic linear layers, no AASHTOWare ME climate/damage models, no CIRCLY load-spectrum summation |
| Subgrade / foundation design (CD 225, subgrade improvement) | implemented | design_foundation: performance foundation by layered elastic surface modulus under a plate (E0 = 2(1-ν²)qa/w0), subgrade E from CBR (E = 17.6 CBR^0.64, verify), first layer stepped to the CD 225 class (UK) or engineer target; foundation-design.csv. Subgrade improvement (stabilisation) mix design is by the geotechnical/materials engineer |
| Storm drainage network hydraulics with HGL/EGL | implemented | drainage-report.md, pipe profile sheets |
| Hydrology (IFD/IDF, ARR 2019, FEH/ReFH, peak flow methods) | implemented | design_hydrology: TIN catchments (priority-flood + D8), rational (US/CA), NRCS TR-55 Type II, ARR 2019 rational-type with Bransby-Williams tc (AU/NZ); UK FEH/ReFH2 peak entered by the engineer with rational sense check. IFD tables are engineer inputs. hydrology.csv, drainage area map sheet |
| Roadside ditch / channel lining (HEC-15 shear) | implemented | protection.json, drainage-report.md |
| Culvert hydraulics (HDS-5) and structural design | implemented | design_culverts: HDS-5 inlet and outlet control, size selection by HW/D, freeboard, cover; culverts.csv and culvert sheet. Structural class by agency standard drawings; large culverts go to the structures hand-off |
| Sanitary sewer design (peaking, depth ratio, velocity) | implemented | drainage-report.md |
| Retaining wall external stability | implemented | US global factors; other profiles need engineer criteria; wall type selection (gravity / cantilever / profile reshape / MSE external stability per GEC 11 / soil nail; BS 8006, AS 4678, CSA S6 cited); cantilever stem flexure, stem shear (LRFD simplified), crack-control spacing (LRFD 5.6.7) and footing heel/toe flexure. Limits: MSE internal/supplier design, global stability and detailing by the structures/geotechnical specialist |
| Barrier need and length of need, clear zone | implemented | geometric hazard screening with engineer clear zone, length of need; UK: rrrap-inputs.csv for the National Highways RRRAP tool (the risk assessment is run in RRRAP) |
| Intersection geometry, returns, sight triangles | implemented | US Case B1 / CA time gap / AU-NZ Austroads SISD / UK CD 123 y-distance (check_intersection_sight); intersection layout sheet |
| Slope stability / geotechnical design | implemented | check_slope_stability: Bishop simplified + Spencer, critical circle search, horizontal strata (multi-layer, absolute or relative to the toe), parameters from the geotechnical report; slope-sections.csv for the geotechnical designer. Limits: no sloping strata, piezometric surfaces or non-circular surfaces |
| Subsurface (pavement) drainage | implemented | design_subsurface_drainage: Cedergren infiltration, layer capacity, t50, edge drain capacity and outlet spacing; AASHTO m by engineer |
| Traffic capacity / LOS, signals, signing, markings, lighting | implemented | concept signing and marking (no-passing zones vertical + horizontal), lighting by the lumen method, signal warrants (MUTCD Warrant 1 A/B/combination, TAC points, engineer tables UK/AU/NZ), Webster timing with HCM delay/LOS and ITE intergreen, link v/c and density LOS, priority-junction gap-acceptance capacity (HCM), RFC/DOS limits. Limits: concept depth; photometric lighting design and detailed signal design (HCS/Synchro/SIDRA/LINSIG) by the specialist |
| Noise, air, environmental assessment | interface | LICENSED SPECIALIST: handoff-environmental.json + checklist by profile (specialist_handoffs); assessment not produced by this software |
| Utilities conflicts, right-of-way / land requirements | implemented | register_utilities (utility-register.csv), land_requirements (footprint, parcel takes) |
| Bridges and major structures | interface | LICENSED SPECIALIST: handoff-bridge-structures.json + checklist by profile; design not produced by this software |
| Temporary traffic management / staging | implemented | plan_traffic_management: concept staging; MUTCD taper formula (US), engineer tapers elsewhere |

### 3b. Reports and registers

| Item | Status | Output / note |
|---|---|---|
| Design report / basis of design / design criteria | implemented | design-report.md, parameter-register.csv |
| Drainage report | implemented | drainage-report.md |
| Pavement design report | implemented | pavement-report.md |
| Geotechnical interface (inputs needed from the geotechnical report) | implemented | design-report.md §12 lists inputs; slope stability takes the report reference and parameters and returns slope-sections.csv. The geotechnical report itself is by the geotechnical engineer |
| Departures / design exceptions / relaxations register | implemented | departures-register.csv |
| Road safety audit hooks (brief for the independent auditor) | implemented | rsa-brief.md (the audit itself is done by people) |
| Quantities / bill of quantities by measurement method | implemented | boq.csv: US FP-14 sections, UK SHW series + CESMM4 class, CA OPSS, AU TfNSW, NZ NZTA references per work group (marked verify for the estimator) |
| Cost estimate | implemented | cost-plan.csv structure: base, contingency, risk P50/P90, optimism bias (UK); rates blank by design (no market prices shipped) |
| Setting-out / alignment report | implemented | setting-out.csv |
| Checks / design review report with audit trail | implemented | review.html |
| Environmental, constructability, utilities reports | implemented | utility conflict register; constructability.md (sequence, staging, walls by type, drainage first, risks and hold points generated from the design); environmental hand-off (licensed specialist, interface only) |

### 3c. Drawings

| Item | Status | Output / note |
|---|---|---|
| Title sheet, sheet index, general notes and legend | implemented | sheets.pdf / sheet-NN.dxf |
| Key plan / location plan | implemented | locality plan sheet (UTM grid, lat/long) on an offline OpenStreetMap basemap (ODbL, attributed, cached in data/basemap) |
| Route alternatives plan | implemented | sheets.pdf |
| Typical sections | implemented | sheets.pdf |
| Plan and profile | implemented | sheets.pdf |
| Superelevation diagram | implemented | sheets.pdf |
| Cross sections | implemented | sheets.pdf |
| Subgrade / earthwork and mass haul | implemented | sheets.pdf |
| Pavement design details | implemented | sheets.pdf |
| Storm and sanitary plan and profile | implemented | sheets.pdf |
| Pipe trench cross-sections, manhole and inlet details | implemented | sheets.pdf |
| Barrier / protection layout and ditch lining schedule | implemented | sheets.pdf |
| Quantity summary sheet | implemented | mass haul and quantities sheet; full BoQ in boq.csv |
| Standard details atlas (curb, gutter, barrier, trench bedding, manhole) | implemented | sheets.pdf (schematic details with the agency standard drawing / specification reference per profile; CA/AU/NZ references are engineer-to-cite) |
| Intersection layout | implemented | intersection layout and sight distance sheet |
| Signing, pavement marking, lighting, signals | implemented | concept signing/marking and lighting sheets; signal sheet with phasing diagrams, timing and warrant tables |
| Traffic management / staging plans | implemented | concept staging sheet |
| Utilities, right-of-way / land plans | implemented | utility plan + register sheet, land requirement plan sheet |
| Erosion and sediment control plan, landscaping | implemented | concept ESC plan sheet (sediment fence, check dams at L = h/S, culvert protection, entrances); landscape concept sheet (slope planting classes, street trees outside clear zone / sight triangles / poles, quantities) |
| Structures (bridges, culverts, wall structural) | partial | culvert layout/section sheet, wall type selection sheet with typical MSE section, cantilever stem reinforcement sheet; bridges by licensed specialist (hand-off); wall structural detailing by the structures engineer |

### 3d. Remaining gaps, interfaces and partial items

Gaps (not produced by the software):

- none at item level; see the partial items below

Licensed-specialist interfaces (hand-off template + checklist only):

- Noise, air, environmental assessment: LICENSED SPECIALIST: handoff-environmental.json + checklist by profile (specialist_handoffs); assessment not produced by this software
- Bridges and major structures: LICENSED SPECIALIST: handoff-bridge-structures.json + checklist by profile; design not produced by this software

Partial items and what is missing:

- Structures (bridges, culverts, wall structural): culvert layout/section sheet, wall type selection sheet with typical MSE section, cantilever stem reinforcement sheet; bridges by licensed specialist (hand-off); wall structural detailing by the structures engineer

### 3e. Standards: verified vs engineer input

- Verified from the published source and coded: DMRB CD 109 Table 2.10 (UK design speed related parameters, OGL); CD 109 Eq 4.2 superelevation; CD 226 Eq 2.24, 2.43a/b and foundation-class limits (§2.4, §2.5, §2.19); AASHTO 1993 flexible and rigid design equations (worked examples reproduced); AASHTO 2018 metric SSD, minimum-radius, crest/sag curve formulas; FHWA HEC-15 (2005) Table 2.1/2.3 values, Eq 6.1 and 6.8 with Table 6.1; HEC-22 HGL approximate method; Ten States and Harmon peaking; FHWA FP-14 section numbers for the BoQ.
- Engineer input (copyrighted/paywalled tables or project-specific): e_max and side friction, maximum grades, relative gradients for non-UK profiles, TAC/Austroads tables (minimum radii, K values where tabulated), CD 226 Figure 2.20 nomograph values, AGPT02 chart readings where outside the coded equation, clear-zone widths (RDG Table 3-1 / CD 377 / AGRD06), wall factors of safety for CA/UK/AU/NZ, local sewer criteria, rainfall intensity. These are marked `engineer` in parameter-register.csv and in the design report.

- v5 formulas coded from the published source: rational method; NRCS TR-55 runoff equation and Type II unit peak discharge (Table F-1 coefficients only); Kirpich and Bransby-Williams tc; FHWA HDS-5 inlet-control forms (Table A.1 coefficients for six concrete/CMP inlets) and outlet-control energy equation with ke (Table C.2); Bishop simplified and Spencer limit equilibrium; Cedergren infiltration, Darcy layer capacity and Casagrande-Shannon t50 (FHWA subsurface drainage manual); Boussinesq/Odemark strains with AGPT02 transfer functions (screening); rectangular-stress-block flexure; MUTCD Table 3B-1 PSD and Part 6 taper formulas; lumen-method spacing; horizontal sight-line offset M = R(1 - cos(S/2R)); Austroads SISD formula; UTM inverse (Snyder 1987).
- v5 engineer inputs: IFD/IDF tables (NOAA Atlas 14, ECCC, BoM, FEH DDF), C and CN, UK FEH/ReFH2 peaks, culvert sizes / HW/D / freeboard / cover, soil parameters and FS criterion from the geotechnical report, AASHTO drainage coefficient m, asphalt Vb / RF, wall material and load factors, non-US passing sight distances and taper lengths, Austroads RT and d, CD 123 y-distance, TAC time gap, luminaire data, utility records and quality levels, parcel boundaries, contingency / optimism bias. Item codes for CA/AU/NZ BoQ mapping and some US/UK series are marked (verify).

## Sources
[1] TxDOT PS&E Preparation Manual, Ch. 3 Table 3-1 (traditional plan sheet sequence), https://www.txdot.gov/content/dam/txdotoms/des/pse/pse.pdf
[2] TxDOT PS&E Manual 6.2.3 Engineering Review Checklist (milestones), https://www.txdot.gov/manuals/des/pse/chapter-6--ps-e-submission--review-and-processing/section-2--engineering-reviews/engineering-review-checklist.html
[3] TxDOT PS&E Manual 3.1.5 Drainage Plan Sheets, https://www.txdot.gov/manuals/des/pse/chapter-3--plan-set-development/section-1--standard-traditional-plan-sheets/-drainage-plan-sheets---for-culverts--storm-sewer-.html
[5] Caltrans Plans Preparation Manual (2023), https://dot.ca.gov/-/media/dot-media/programs/design/documents/cadd/ppm-text-all-a11y.pdf
[6] Caltrans PPM §2.5 Layouts, Profiles and Superelevation Diagrams; §2.19 Summary of Quantities, https://dot.ca.gov/programs/design/manual-plans-preparation-manual-ppm
[7] MTO Highway Planning and Design Process Guideline (2016), https://tcp.mto.gov.on.ca/sites/default/files/2025-02/Highway%20Planning%20and%20Design%20Process%20Guideline%20September%202016.pdf
[8] MTO Cost Estimating Guide for Designers (2023) §7.3–7.4, https://tcp.mto.gov.on.ca/sites/default/files/2023-03/Estimating%20Guide%20-%20Final%2028-Feb-2023.pdf
[9] BC Supplement to TAC GDG, Ch. 100 Highway Design Process, https://www2.gov.bc.ca/assets/gov/driving-and-transportation/transportation-infrastructure/engineering-standards-and-guidelines/highway-design-and-survey/tac/tac-2019-supplement/bctac2020-chapter-0100.pdf
[10] Geometric Design Guidelines for B.C. Roads (Ch. 1200 drawing presentation), https://www2.gov.bc.ca/gov/content/transportation/transportation-infrastructure/engineering-standards-guidelines/highway-design-survey/tac-bc
[11] DMRB GG 101 Introduction to the DMRB, §2.3–2.8 Departures, https://www.standardsforhighways.co.uk/tses/attachments/25cefcd4-3464-4e41-8e4f-fffc155723ea
[12] National Highways, Key requirements for third-party NSIP projects (DMRB compliance, departures, RSA stages), https://nationalhighways.co.uk/roads-and-travel/road-projects/working-with-national-highways/key-requirements-for-third-party-nsip-projects-guidance-for-promoters/
[13] National Highways, Step-by-step guide to GG 104 safety risk assessment, https://www-deployment.nationalhighways.co.uk/media/o0nm5fb3/the-step-by-step-guide-to-using-gg-104-requirements-for-safety-risk-assessment.pdf
[14] Austroads AGRD Part 1 (AGRD01-25, Ed 5.2) §4.2 phases, §4.4–4.6 design domain/exceptions, App A documentation, https://austroads.gov.au/publications/road-design/agrd01
[15] TfNSW Works Authorisation Deed process manual (2023) §3.1–3.2, https://www.transport.nsw.gov.au/system/files/media/documents/2023/wad-process-manual-developer-works.pdf
[16] MCHW Volume 4 Section 1, Method of Measurement for Highway Works (Series 500/600/700), https://www.standardsforhighways.co.uk/tses/attachments/1e56dda3-cc10-4588-90be-a1e9a8a7870a
[17] TfNSW CADD Manual §3.5 (design output reports), https://standards.transport.nsw.gov.au/_entity/annotation/913cbff6-af35-ed11-9db2-000d3ae019e0
[18] NZTA Business case requirements (structures) and Highway Structures Design Guide certificates, https://www.nzta.govt.nz/roads-and-rail/highways-information-portal/technical-disciplines/structures/business-case-requirements/
[19] NZTA Geometric design (Austroads primary, SHGDM supplementary), https://www.nzta.govt.nz/roads-and-rail/highways-information-portal/technical-disciplines/safety-and-geometric-design/geometric-design/
[20] NZTA Safe System audit guidelines, https://www.nzta.govt.nz/assets/resources/road-safety-audit-procedures/docs/safe-system-audit-guidelines.pdf
[21] NZTA Implementation phase technical requirements, https://www.nzta.govt.nz/roads-and-rail/highways-information-portal/processes/project-delivery/implementation/
