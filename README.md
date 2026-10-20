# civil-mcp

Agent-native road design tools in the spirit of Civil 3D, exposed as MCP tools. English only. The engineer picks a **standards profile** per project (US AASHTO 2018, Canada TAC 2017, UK DMRB, Australia Austroads, New Zealand Austroads + NZTA); every check cites its clause, and every design parameter is registered as *verified* (from the published source), *formula*, or *engineer input* (copyrighted or paywalled tables are not shipped).

Pipeline: survey points → TIN → **route alternatives** (least-cost search, each built and checked in full, selection recorded with reasons) → alignment with clothoids → superelevation → profile + **earthwork-balance optimiser** → assembly/corridor (curbs, widening, ditches, benches, retaining walls) → **pavement design** (AASHTO 93 flexible/rigid, Austroads AGPT02, DMRB CD 226) → **roadside protection** (clear-zone hazards, barrier runs with length of need, HEC-15 ditch linings and riprap, slope protection) → intersection → storm/sanitary networks with HGL/EGL → **earthwork** (shrink/bulk, free haul/overhaul, borrow/waste) → **19-sheet drawing set** (PDF + DXF) → design, drainage and pavement reports, parameter and departures registers, BoQ, setting-out, RSA brief → LandXML 1.2. Every call goes into a hash-chained audit log. Everything stays **PRELIMINARY – NOT FOR CONSTRUCTION** until an engineer records approval.

**This is not a substitute for a licensed engineer.**

![the agent designing on USGS terrain, rendered from the demo's real outputs](docs/media/civil-mcp-agent-design.gif)

*Every frame is drawn from files the demo wrote, by [`scripts/animate.py`](scripts/animate.py): alternatives → selection → profile/balance → corridor → pavement → protection → drainage → drawing set. MP4 1080p: [docs/media/civil-mcp-agent-design.mp4](docs/media/civil-mcp-agent-design.mp4).*

## Try it

```bash
cargo run --release -p civil-demo
```

Committed outputs: [sheets.pdf](docs/sheets.pdf) (19 sheets), [review.html](docs/review.html), [design.xml](docs/design.xml), [run-log.jsonl](docs/run-log.jsonl). Deliverables matrix per country and the gap list: [docs/DELIVERABLES.md](docs/DELIVERABLES.md).

Demo (US profile, 50 km/h; e_max 0.08, f_max 0.18, max grade 8 % entered by the placeholder "engineer"), measured 2026-10-20:

1. 5,607 USGS 3DEP points → 11,089-triangle TIN.
2. Pavement: rigid option PCC 160 mm; adopted flexible W18 6.385e5, SN 2.91 required / 3.38 provided (AC 110 / base 150 / subbase 150 mm).
3. Route alternatives: A-direct 1,136 m, 0 mandatory failures, cost proxy 317,776; B-contour-following 3 failures; C-separate-corridor 0 failures but 77,824 m³ wall concrete (proxy 2,578,395). A selected (rule: fewest failures, then cost proxy; rates are placeholders).
4. Profile optimiser (71 evaluations): imbalance −30,954 → +48 m³, haul moment 19.1M → 9.0M m³·m, wall concrete 6,581 → 2,908 m³; every candidate passes the grade/VC checks.
5. Corridor: 76 sections, cut 43,976 / fill 39,064 m³ (composite grid +0.20 % / −0.20 %); AASHTO widening 0.26–0.30 m; walls on 7 sections (80 m).
6. Intersection passes at station 570; pad graded with full daylight.
7. Storm (HEC-22): 7 attempts; resizing stalled twice, the agent lowered cover to 1.5 m; 0 failures. Sanitary: attempt 1 fails P-VMIN, min slope 0.8 % passes.
8. Protection: 35 hazards, 8 barrier runs (842 m), 5 lined ditch reaches (concrete, gravel mulch, riprap D50 100/250 mm).
9. Earthwork: final ordinate 514 m³, borrow 0, waste 571 m³, overhaul 4.57M m³·m.
10. 218 checks, 0 mandatory failures, 10 advisory. 19 sheets: cover/index/notes, route alternatives, plan & profile, typical sections, superelevation diagram, pavement & subgrade, 6 cross-section sheets, protection layout, storm and sanitary profiles, drainage details + structure schedule, mass haul & quantities, standard-details atlas.

## Validation (see [docs/VALIDATION.md](docs/VALIDATION.md))

- `cargo test`: 32 library tests (AASHTO 93 worked examples, CD 226 / CD 109, HEC-15, route search, walls, corridor hand calcs …), LandXML round trip, Civil 3D fixture, MCP stdio test (UK profile), full pipeline test.
- `scripts/independent_check.py` (CI, no shared code): **92 checks, 0 failed** — section areas, volumes, mass haul, superelevation, clothoids, Manning/HGL, walls, widening, route costs (SciPy csgraph Dijkstra, 1e-6), pavement, HEC-15, earthwork, optimiser, BoQ, setting-out, LandXML XSD, DXF audit, no CJK.
- Civil 3D LandXML surfaces/alignments/pipe geometry match (unchanged from v3).
- ODA DXF→DWG→DXF on all 20 files with identical entity counts; LibreDWG reads all; LibreCAD renders all.

## Why the engineer comes first

US boards require the sealing engineer to be in **responsible charge** (e.g. North Carolina 21 NCAC 56.0701; Florida FBPE). NSPE's 2025 BER discussion of AI says that "reviewing drawings or documents after preparation without involvement in the design and development process does not satisfy the definition of Responsible Charge." Canada, the UK, Australia and New Zealand likewise assign the design to a professional engineer (e.g. P.Eng. sealing in Canadian provinces, the Designer/Design Organisation under DMRB GG 101/GG 104, RPEQ in Queensland, CPEng in New Zealand).

So the design is: the engineer sets the basis first, the agent works inside it, every decision is logged with its rule, the engineer reviews the checks and the trail, and only the engineer approves and seals. MCP cannot authenticate a human, and `actor: engineer:<name>` is a declaration, not proof of identity. Deployments need real authentication in front of `set_design_basis` and `record_review`.

## Geometry kernel

Civil work at this stage is TIN math and 2D/3D alignment math, so there is no B-rep kernel. Triangulation uses [`delaunator`](https://crates.io/crates/delaunator) (ISC). Everything else is in this repo. Walls are 2D sections (stability is per metre run), so a B-rep kernel was not needed for them. A permissive kernel ([truck](https://github.com/ricosjp/truck), [solidcore](https://github.com/tangency-labs/solidcore), or NVIDIA [usd-brep](https://github.com/NVIDIA-Omniverse/usd-brep), all Apache-2.0) becomes useful for 3D wall/structure solids and clash checks. No GPL/LGPL code (OpenCASCADE, FreeCAD, SolveSpace) was read or used.

## Limits

- Values from copyrighted tables (Green Book side-friction/e tables, TAC, Austroads, CD 226 Fig 2.20, clear-zone tables) are engineer inputs; the profile records which. Transcribed values must be verified against the edition used.
- Gaps (see DELIVERABLES.md §3d): hydrology, culverts, geotechnics/slope stability, subsurface drainage, traffic/signals/signing/lighting, environment, utilities/ROW, bridges/structures, staging. Partial: mechanistic pavement design, wall structural design, UK RRRAP, non-US intersection sight distance, BoQ mapping for CA/AU/NZ, cost rates.
- Route search is a 10 m grid least-cost path; the cost proxy is relative. Standard details are schematic references to agency drawings.
- Not compared with Civil 3D corridor volumes or hydraulics; DWGs not opened in AutoCAD. The demo engineer is a placeholder; one terrain sample.

## License

Apache-2.0. Bundled font: DejaVu Sans (`crates/civil-core/assets/DejaVuSans-LICENSE.txt`). Test fixture `c3dprofile2.xml`: MIT (RustedGeom/rusted-geom).
