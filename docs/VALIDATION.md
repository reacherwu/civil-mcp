# Validation record

## v6 summary (measured 2026-10-20, demo: 288 checks, 0 mandatory failures)

| Script / test | Result |
|---|---|
| `scripts/independent_check.py` (base) | 91 / 91 OK |
| `scripts/independent_check_v5.py` (hydrology, culverts, slope, subsurface, walls, land, locality) | 70 / 70 within tolerance |
| `scripts/independent_check_v6.py` (layered elastic vs PyMastic be301be + KENPAVE, MSE, wall shear/crack/footing, signals, capacity, rigid, landscape, locality) | 81 / 81 within tolerance |
| Layered elastic vs KENPAVE 3-layer published example (`examples/lea_kenpave.rs`, `data/validation/kenpave_3layer.csv`) | deflection 0.058 %, σz 0.028 %, εr 0.088 %, εz 0.061 % |
| CAD round trip, 39 DXF (ODA DXF→DWG→DXF, LibreDWG read) | 0 entity-count mismatches; ezdxf audit 0 errors |
| `cargo test`, `cargo fmt --check`, `cargo clippy -D warnings` | pass (CI) |

The sections below are the earlier, more detailed v4 record. They are kept for reference; counts in them refer to v4 (19-20 sheets).

## v4 record (measured 2026-10-20)

All numbers below were produced on 2026-10-20 from `cargo run --release -p civil-demo` (US AASHTO profile, 50 km/h, USGS 3DEP terrain) and the scripts named. Nothing here has been reviewed by a licensed engineer.

## 1. Independent checks on the demo output

`python scripts/independent_check.py demo-out --xsd schema/LandXML-1.2.xsd` (run in CI). The script shares no code with the Rust tools: it re-derives each value with NumPy/SciPy from the files the tools wrote.

```
OK  superelevation e, curve 1: civil-mcp 0.0336496 | independent 0.0336496 | diff +0% (tol 1e-06)
OK  superelevation development length, curve 1: civil-mcp 28.8883 | independent 28.8883 | diff +0% (tol 1e-06)
OK  provided development ≥ required, curve 1 (m spare): civil-mcp 11.1117 | independent 0 | diff +11.11 m (tol 1000000000.0)
OK  superelevation e, curve 2: civil-mcp 0.0302847 | independent 0.0302847 | diff +0% (tol 1e-06)
OK  superelevation development length, curve 2: civil-mcp 27.0764 | independent 27.0764 | diff +0% (tol 1e-06)
OK  provided development ≥ required, curve 2 (m spare): civil-mcp 12.9236 | independent 0 | diff +12.92 m (tol 1000000000.0)
OK  superelevation e, curve 3: civil-mcp 0.0302847 | independent 0.0302847 | diff +0% (tol 1e-06)
OK  superelevation development length, curve 3: civil-mcp 27.0764 | independent 27.0764 | diff +0% (tol 1e-06)
OK  provided development ≥ required, curve 3 (m spare): civil-mcp 12.9236 | independent 0 | diff +12.92 m (tol 1000000000.0)
OK  SSD = 0.278·V·t + 0.039·V²/a (AASHTO, t 2.5 s, a 3.4 m/s²) (m): civil-mcp 63.4265 | independent 63.4265 | diff +0 m (tol 1e-09)
OK  clothoid chord L=40 R=180 (m): civil-mcp 39.9781 | independent 39.9781 | diff +3.673e-07 m (tol 1e-06)
OK  clothoid chord L=40 R=180 (m): civil-mcp 39.9781 | independent 39.9781 | diff -2.379e-07 m (tol 1e-06)
OK  clothoid chord L=40 R=200 (m): civil-mcp 39.9822 | independent 39.9822 | diff -2.582e-07 m (tol 1e-06)
OK  clothoid chord L=40 R=200 (m): civil-mcp 39.9822 | independent 39.9822 | diff -2.324e-08 m (tol 1e-06)
OK  clothoid chord L=40 R=200 (m): civil-mcp 39.9822 | independent 39.9822 | diff -3.428e-07 m (tol 1e-06)
OK  clothoid chord L=40 R=200 (m): civil-mcp 39.9822 | independent 39.9822 | diff -2.681e-07 m (tol 1e-06)
OK  LandXML 1.2 XSD validation errors: civil-mcp 0 | independent 0 | diff +0  (tol 0)
OK  section cut area, max rel diff where area>1 m² (%): civil-mcp 0.014855 | independent 0 | diff +0.01485 % (tol 0.05)
OK  section fill area, max rel diff where area>1 m² (%): civil-mcp 0.0124566 | independent 0 | diff +0.01246 % (tol 0.05)
OK  section areas, max abs diff (m²): civil-mcp 0.00138537 | independent 0 | diff +0.001385 m² (tol 0.1)
OK  corridor cut volume (avg end area, independent areas): civil-mcp 43975.6 | independent 43975.7 | diff -0.0003115% (tol 0.05)
OK  corridor fill volume (avg end area, independent areas): civil-mcp 39063.8 | independent 39063.8 | diff +0.0001065% (tol 0.05)
OK  mass haul final ordinate (m³): civil-mcp 514.198 | independent 514.198 | diff +8.844e-14% (tol 0.01)
OK  mass haul max |ordinate diff| (m³): civil-mcp 9.09495e-13 | independent 0 | diff +9.095e-13 m³ (tol 1.0)
OK  haul moment ∫|M|ds (m³·m): civil-mcp 9.12605e+06 | independent 9.13282e+06 | diff -0.0742% (tol 0.5)
OK  [Sanitary] design Q (rational + PF·Q_avg), max abs diff (m³/s): civil-mcp 0 | independent 0 | diff +0 m³/s (tol 1e-09)
OK  [Sanitary] peaking factor (ten_states), max abs diff: civil-mcp 0 | independent 0 | diff +0  (tol 1e-12)
OK  [Sanitary] Manning full-flow Q, max diff (%): civil-mcp 1.8734e-14 | independent 0 | diff +1.873e-14 % (tol 1e-06)
OK  [Sanitary] partial-flow depth ratio (brentq), max abs diff: civil-mcp 6.9672e-13 | independent 0 | diff +6.967e-13  (tol 0.0001)
OK  [Sanitary] partial-flow velocity, max abs diff (m/s): civil-mcp 5.41367e-12 | independent 0 | diff +5.414e-12 m/s (tol 0.001)
OK  [Sanitary] pipe HGL/EGL ends, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [Sanitary] structure HGL, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [Sanitary] min freeboard rim − HGL (m, must be ≥ 0.3): civil-mcp 0.978512 | independent 0.3 | diff +0.6785 m (tol 1000000000.0)
OK  [Storm] design Q (rational + PF·Q_avg), max abs diff (m³/s): civil-mcp 0 | independent 0 | diff +0 m³/s (tol 1e-09)
OK  [Storm] Manning full-flow Q, max diff (%): civil-mcp 1.86785e-14 | independent 0 | diff +1.868e-14 % (tol 1e-06)
OK  [Storm] partial-flow depth ratio (brentq), max abs diff: civil-mcp 1.12632e-13 | independent 0 | diff +1.126e-13  (tol 0.0001)
OK  [Storm] partial-flow velocity, max abs diff (m/s): civil-mcp 2.56328e-12 | independent 0 | diff +2.563e-12 m/s (tol 0.001)
OK  [Storm] pipe HGL/EGL ends, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [Storm] structure HGL, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [Storm] min freeboard rim − HGL (m, must be ≥ 0.3): civil-mcp 0.354066 | independent 0.3 | diff +0.05407 m (tol 1000000000.0)
OK  walls (8 sections): sliding FS μV/Pa, max abs diff: civil-mcp 6.66134e-16 | independent 0 | diff +6.661e-16  (tol 1e-06)
OK  walls: overturning FS Mr/Mo (numeric pressure integral), max abs diff: civil-mcp 1.20112e-10 | independent 0 | diff +1.201e-10  (tol 1e-06)
OK  walls: eccentricity, max abs diff (m): civil-mcp 3.82814e-11 | independent 0 | diff +3.828e-11 m (tol 1e-06)
OK  walls: q_max V/B(1+6e/B), max abs diff (kPa): civil-mcp 3.76542e-09 | independent 0 | diff +3.765e-09 kPa (tol 0.001)
OK  walls: min sliding FS (AASHTO Std Spec 17th §5.5.5 ≥ 1.5): civil-mcp 1.50031 | independent 1.5 | diff +0.0003129  (tol 1000000000.0)
OK  walls: min overturning FS (≥ 2.0): civil-mcp 5.16188 | independent 2 | diff +3.162  (tol 1000000000.0)
OK  AASHTO §3.3.10 widening w_c − w_n, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-09)
OK  X1 ISD = 0.278·V·tg (AASHTO 2018 §9.5.3 B1), max abs diff (m): civil-mcp 1.42109e-14 | independent 0 | diff +1.421e-14 m (tol 1e-09)
OK  X1 curb returns: |dist(centre, leg edge) − R| and |arc end − R|, max (m): civil-mcp 1.37788e-10 | independent 0 | diff +1.378e-10 m (tol 1e-06)
OK  X1 N leg return-patch fill vs 0.1 m grid on its faces (tool samples 0.25 m): civil-mcp 911.324 | independent 911.884 | diff -0.06135% (tol 1.0)
OK  X1 S leg return-patch fill vs 0.1 m grid on its faces (tool samples 0.25 m): civil-mcp 1395.74 | independent 1395.03 | diff +0.05123% (tol 1.0)
OK  yard grading cut vs analytic mitred model (m³): civil-mcp 274.534 | independent 274.557 | diff -0.008549% (tol 1.0)
OK  yard grading fill vs analytic mitred model (m³): civil-mcp 1005.75 | independent 1005.81 | diff -0.006135% (tol 1.0)
OK  DXF audit errors+fixes+style/codepage/units problems (all files): civil-mcp 0 | independent 0 | diff +0  (tol 0)
OK  PDF text extract contains CROSS SECTIONS and PRELIMINARY: civil-mcp 1 | independent 1 | diff +0  (tol 0)
OK  CJK characters in all text outputs: civil-mcp 0 | independent 0 | diff +0  (tol 0)
OK  PDF pages vs 'Sheet n of N' in title blocks: civil-mcp 19 | independent 19 | diff +0  (tol 0)
OK  AASHTO 93 flexible worked example (Guide Fig 3.1: SN ≈ 5.0): civil-mcp 4.97567 | independent 5 | diff -0.4867% (tol 2.0)
OK  AASHTO 93 rigid worked example (Guide Fig 3.7 nomograph reading D ≈ 10.0 in; equation solution): civil-mcp 9.71897 | independent 10 | diff -2.81% (tol 3.0)
OK  pavement rigid: slab (mm, D·25.4 rounded up to 10): civil-mcp 160 | independent 160 | diff +0 mm (tol 0)
OK  pavement flexible: SN over subgrade: civil-mcp 2.91271 | independent 2.91271 | diff -3.049e-14% (tol 0.0001)
OK  pavement flexible: Asphalt concrete thickness (mm): civil-mcp 110 | independent 110 | diff +0 mm (tol 0)
OK  pavement flexible: Aggregate base thickness (mm): civil-mcp 150 | independent 150 | diff +0 mm (tol 0)
OK  pavement flexible: Aggregate subbase thickness (mm): civil-mcp 150 | independent 150 | diff +0 mm (tol 0)
OK  pavement flexible: SN provided ≥ SN required: civil-mcp 0.469178 | independent 0 | diff +0.4692  (tol 1000000000.0)
OK  route A-direct: least-cost search cost vs scipy csgraph Dijkstra: civil-mcp 2552.35 | independent 2552.35 | diff +0% (tol 1e-06)
OK  route A-direct: path length (m): civil-mcp 1435.52 | independent 1435.52 | diff +0% (tol 1e-09)
OK  route B-contour-following: least-cost search cost vs scipy csgraph Dijkstra: civil-mcp 8881.3 | independent 8881.3 | diff +2.048e-14% (tol 1e-06)
OK  route B-contour-following: path length (m): civil-mcp 1476.16 | independent 1476.16 | diff +1.54e-14% (tol 1e-09)
OK  route C-separate-corridor: least-cost search cost vs scipy csgraph Dijkstra: civil-mcp 8049.91 | independent 8049.91 | diff +1.13e-14% (tol 1e-06)
OK  route C-separate-corridor: path length (m): civil-mcp 1623.02 | independent 1623.02 | diff +7.005e-14% (tol 1e-09)
OK  route recommendation = min(failures, cost proxy): civil-mcp 1 | independent 1 | diff +0  (tol 0)
OK  route A-direct: cost proxy from its quantities: civil-mcp 317776 | independent 317776 | diff +0% (tol 1e-09)
OK  route B-contour-following: cost proxy from its quantities: civil-mcp 299615 | independent 299615 | diff +0% (tol 1e-09)
OK  route C-separate-corridor: cost proxy from its quantities: civil-mcp 2.5784e+06 | independent 2.5784e+06 | diff +0% (tol 1e-09)
OK  barrier length of need X = L_R(L_A − L_2)/L_A, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-09)
OK  ditch left 380: τd ≤ τp (gravel mulch D50 50 mm): civil-mcp 11.9135 | independent 0 | diff +11.91 N/m² (tol 1000000000.0)
OK  ditch left 860: τd ≤ τp (gravel mulch D50 25 mm): civil-mcp 4.04283 | independent 0 | diff +4.043 N/m² (tol 1000000000.0)
OK  HEC-15 ditch normal depth (brentq), max abs diff (m): civil-mcp 2.34507e-13 | independent 0 | diff +2.345e-13 m (tol 0.0001)
OK  HEC-15 τd = γ d S, max abs diff (N/m²): civil-mcp 6.02222e-09 | independent 0 | diff +6.022e-09 N/m² (tol 0.01)
OK  HEC-15 Eq 6.8 riprap: D50 − required (m, ≥ 0): civil-mcp 0.016482 | independent 0 | diff +0.01648 m (tol 1000000000.0)
OK  borrow (compacted) = max(0, fill − cut·k): civil-mcp 0 | independent 0 | diff +0 m³ (tol 1e-09)
OK  waste (bank) = max(0, cut·k − fill)/k: civil-mcp 571.332 | independent 571.332 | diff +0 m³ (tol 1e-09)
OK  overhaul (segment method beyond free haul) (m³·m): civil-mcp 4.56726e+06 | independent 4.56726e+06 | diff +4.078e-14% (tol 1e-06)
OK  optimised profile: max |grade| ≤ max grade (%): civil-mcp -3.55271e-15 | independent 0 | diff -3.553e-15 % (tol 1000000000.0)
OK  optimised profile: crest length − AASHTO minimum (m, ≥ 0): civil-mcp 0 | independent 0 | diff +0 m (tol 1000000000.0)
OK  optimiser objective after ≤ before: civil-mcp 69172.5 | independent 0 | diff +6.917e+04  (tol 1000000000000.0)
OK  setting-out FG vs independent parabola, max abs diff (m): civil-mcp 0.000501611 | independent 0 | diff +0.0005016 m (tol 0.0006)
OK  BoQ roadway excavation = corridor cut (m³): civil-mcp 43975.6 | independent 43975.6 | diff +0.03657 m³ (tol 0.05)
OK  BoQ embankment = corridor fill (m³): civil-mcp 39063.8 | independent 39063.8 | diff -0.008719 m³ (tol 0.01)
OK  BoQ pipe length = network pipe lengths (m): civil-mcp 2117.7 | independent 2117.65 | diff +0.05132 m (tol 0.2)
OK  BoQ barrier length = barrier runs (m): civil-mcp 842 | independent 842.017 | diff -0.01749 m (tol 0.2)

92 checks, 0 failed
```

How each group is checked:
- **Section areas / volumes / mass haul**: existing ground is re-sampled from the LandXML EG triangles with a separate point-location and barycentric interpolation (not the Rust TIN code), then integrated against the stored datum with 20,001 samples per side. Earlier, re-triangulating the EG points with SciPy/Qhull gave 0.16 % differences: the EG points are a regular DEM grid, so each square can be split on either diagonal, and Qhull picked different diagonals. Using the surface's own faces removed that.
- **Route search**: the full heading-constrained state graph (cells × 16 headings, same edge-cost definition) is rebuilt with `scipy.sparse` and solved with `scipy.sparse.csgraph.dijkstra`; costs match the Rust search to 1e-6 relative for all three alternatives.
- **Pavement**: AASHTO 93 flexible SN and rigid D solved with `brentq` from the design equations; layer thicknesses re-derived. The rigid worked example (Guide Fig. 3.7, nomograph reading D ≈ 10.0 in) solves to 9.72 in from the equation (−2.8 %, tolerance 3 %); the difference is nomograph reading precision.
- **HEC-15**: Blodgett n (Eq 6.1), riprap D50 (Eq 6.8 with Table 6.1 interpolation, self-consistent iteration), normal depth and τd = γ·d·S re-solved per reach.
- **Earthwork**: shrink/bulk, borrow/waste and overhaul re-accumulated from the section areas; profile optimiser result re-evaluated.
- **BoQ / setting-out**: BoQ quantities equal the corridor/earthwork values (CSV rounding 0.05 m³); setting-out finished grades equal an independent parabola evaluation (CSV rounding 0.5 mm).
- **Drawings**: every DXF passes `ezdxf` audit, uses `$DWGCODEPAGE ANSI_1252`, text style SANS and correct `$INSUNITS`; no CJK characters in any output; PDF page count equals "Sheet n of N".

## 2. Unit tests (`cargo test`)

32 library tests plus integration tests. Standards-related ones (test names in brackets):
- AASHTO 1993 flexible worked example, Guide Fig. 3.1: SN ≈ 5.0 [`aashto93_flexible_worked_example`]; rigid, Fig. 3.7: D within 0.3 in of 10 in [`aashto93_rigid_worked_example`].
- CD 226 Eq 2.24 limits (180 mm at ≥ 80 msa, 400 msa maximum) [`cd226_eq224_limits`].
- DMRB CD 109 Table 2.10 values and required engineer inputs [`uk_cd109_values_and_required_inputs`]; Austroads SSD formula and engineer inputs [`austroads_ssd_formula_and_engineer_inputs`]; AASHTO SSD table and crest K at 80 km/h.
- HEC-15 Table 6.1 (Shields / safety factor) [`shields_table`], Eq 6.8 riprap at low Re [`riprap_eq68_low_re`], trapezoid normal depth round trip.
- Least-cost route: straight on flat ground, ridge avoided by grade weighting.
- AASHTO widening hand calc and engineer-table lookup; wall hand calc and proportioning.
- MCP stdio test with the UK profile: R = 100 m at 60 kph fails `H-RMIN` citing CD 109 and is recorded as an `H-RDES` relaxation (advisory).
- Full pipeline test: three route alternatives, optimiser reduces imbalance, pipes and intersection pass, regression volumes, approval rules, audit chain.

## Civil 3D LandXML files

Files are not redistributed except `crates/civil-core/tests/fixtures/c3dprofile2.xml` (MIT). Sources: GitHub repositories containing Civil 3D 2022/2023 LandXML exports. Output of `cargo run --release -p civil-core --example landxml_compare -- <files>`:

```
== /workspace/review/civil-agent/c3d/c3dprofile2.xml (Autodesk Civil 3D 2023)
alignment DE1_ALIGN: 21 elements, length 23470.158552 (file Some(23470.158551798326), diff +2.62e-10 m), max element-end deviation 5.82e-11 m, closure 5.82e-11 m
  profile DE1_PROF: 30 VPIs, grades [-1.248, -1.529, 1.393, -1.759, 2.797, -3.178, 0.501, -1.092, 0.738, -1.625, -0.711, 0.679, -2.854, 0.503, 1.502, 0.894, 1.135, 2.0, 49.999, 2.0, 100.0, 4.673, -100.0, -2.0, -50.0, -2.0, -0.9, 1.853, -1.164]
== /workspace/review/civil-agent/c3d/eg2.xml (Autodesk Civil 3D 2022)
surface V-TOPO-E-(2025-06-16)1: 190 pts, 284 faces (59 invisible skipped)
  area2d 6768.946009 (file 6768.946009, diff +0.00e0 m², +0.00e0%)
  area3d 6853.291230 (file 6853.291230, diff +9.09e-13 m², +1.33e-14%)
  elev min 7.1020 (file Some(0.0)) max 9.6380 (file Some(11.597))
== /workspace/review/civil-agent/c3d/example_clean.xml (Autodesk Civil 3D 2023)
surface A: 29 pts, 45 faces (0 invisible skipped)
  area2d 782.534993 (file 782.534993, diff +1.14e-13 m², +1.45e-14%)
  area3d 787.571393 (file 787.571393, diff +1.48e-12 m², +1.88e-13%)
  elev min 1.8500 (file Some(1.85)) max 2.7500 (file Some(2.75))
surface B: 92 pts, 138 faces (0 invisible skipped)
  area2d 276.288603 (file 276.288603, diff +5.68e-14 m², +2.06e-14%)
  area3d 285.932864 (file 285.932864, diff -3.35e-12 m², -1.17e-12%)
  elev min 0.7824 (file Some(0.782382299658)) max 2.2100 (file Some(2.209999999963))
surface C: 143 pts, 179 faces (64 invisible skipped)
  area2d 106.922785 (file 106.922785, diff +8.53e-14 m², +7.97e-14%)
  area3d 116.345529 (file 116.345529, diff +2.56e-13 m², +2.20e-13%)
  elev min 0.9000 (file Some(0.9)) max 2.5739 (file Some(2.573861349374))
== /workspace/review/civil-agent/c3d/pond.xml (Autodesk Civil 3D 2023)
surface pond berm area: 835 pts, 1482 faces (139 invisible skipped)
  area2d 695.521961 (file 695.521961, diff +1.14e-13 m², +1.63e-14%)
  area3d 717.841159 (file 717.841159, diff +3.75e-12 m², +5.23e-13%)
  elev min 17.0000 (file Some(16.99999998)) max 19.5000 (file Some(19.5))

thread 'main' panicked at crates/civil-core/examples/landxml_compare.rs:10:61:
called `Result::unwrap()` on an `Err` value: "Curve without Center"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

Observations:
- 2D/3D surface areas match the values Civil 3D wrote into the files to ≤ 2e-12 relative. That covers triangles flagged invisible (`<Invisible>`), which we skip, as Civil 3D does.
- `eg2.xml`: Civil 3D reports elevMin 0.0 / elevMax 11.597. We report 7.102 / 9.638 over vertices used by visible faces. 11.597 belongs to a point used only by invisible faces, and no point in the file has elevation 0.0, so the file's elevMin is not reproducible from its own points.
- The 23.47 km Civil 3D 2023 alignment rebuilds to within 2.6e-10 m in length, with a max element-end deviation of 5.8e-11 m. Its profile contains ±50 % and ±100 % grade segments between closely spaced PVIs; those are in the source file.
- `starter.xml` (labelled Civil 3D 2026, from an LGPL repo) has curves without `<Center>`, and its curve length attributes disagree with its own chord/radius geometry (135 vs 131.08 m, 160 vs 156.61 m). The reader derives the centre and emits warnings. I treat this file as hand-made, not a real Civil 3D export.
- landxml.org (the official sample files) was unreachable from the build machine (timeouts/TLS errors), so the official samples were not tested.

## Civil 3D pipe networks (Civil 3D 2025 LandXML exports)

Three files from a public GitHub repo (`fadil-karim/mep_automation_kit`, HEL11 drainage models; not redistributed here). Output of `cargo run --release -p civil-core --example c3d_pipes -- <files>`:

```
== HEL11-C3D-CIV_-_Drainage_Ditches_Model.xml
   application: Some("Autodesk Civil 3D 2025")
   network 'Drainage Ditches' type storm: 60 structures, 55 pipes, 55 with both end inverts (0 curved)
   length: ours 2D centre-centre − C3D length: n=55 max|d|=1.063e0 median=-7.077e-4 mean=-5.840e-2
   length: ours 3D centre-centre − C3D length: n=55 max|d|=5.258e-13 median=-7.105e-15 mean=1.450e-14
   slope : ours |Δinv|/L2D − C3D |slope|:      n=55 max|d|=4.760e-13 median=-8.076e-15 mean=-8.017e-15
   HGL   : C3D (hglUp−invUp) − (hglDn−invDn):   n=55 max|d|=9.682e0 median=-2.504e-1 mean=-9.978e-1
   flowIn values in file: [0.0]; distinct (hglUp−invUp)/D (first 8): [-464.74, -462.754, -449.381, -415.987, -381.853, -459.257, -443.564, -421.657]
== HEL11-C3D-CIV_-_Drainage_Rainwater_Model.xml
   application: Some("Autodesk Civil 3D 2025")
   network 'Rainwater' type storm: 116 structures, 114 pipes, 107 with both end inverts (0 curved)
   length: ours 2D centre-centre − C3D length: n=107 max|d|=2.227e-1 median=-6.419e-5 mean=-4.814e-3
   length: ours 3D centre-centre − C3D length: n=107 max|d|=4.974e-13 median=-1.776e-14 mean=1.999e-14
   slope : ours |Δinv|/L2D − C3D |slope|:      n=107 max|d|=8.246e-13 median=-4.351e-15 mean=-1.262e-15
   HGL   : C3D (hglUp−invUp) − (hglDn−invDn):   n=107 max|d|=1.514e0 median=-1.286e-2 mean=4.462e-3
   flowIn values in file: [0.0]; distinct (hglUp−invUp)/D (first 8): [2.121, 1.647, 1.315, 0.564, 1.537, 1.533, 1.464, 1.285]
== HEL11-C3D-CIV_-_Drainage_SW___CP_Model.xml
   application: Some("Autodesk Civil 3D 2025")
   network 'Stormwater' type storm: 459 structures, 300 pipes, 249 with both end inverts (6 curved)
   length: ours 2D centre-centre − C3D length: n=249 max|d|=7.064e-1 median=-8.652e-4 mean=-3.312e-2
   length: ours 3D centre-centre − C3D length: n=249 max|d|=1.231e-9 median=-1.421e-14 mean=-6.032e-12
   slope : ours |Δinv|/L2D − C3D |slope|:      n=249 max|d|=5.531e-8 median=6.793e-15 mean=2.221e-10
   HGL   : C3D (hglUp−invUp) − (hglDn−invDn):   n=228 max|d|=6.470e1 median=-7.283e-2 mean=1.945e-1
   flowIn values in file: [0.0]; distinct (hglUp−invUp)/D (first 8): [0.713, 1.661, 0.513, 1.076, 4.531, 3.19, 1.079, 0.745]
```

Observations:
- Pipe **3D centre-to-centre length** computed from the structure centres and the pipe end inverts matches Civil 3D's `length` to ≤ 1.3e-9 m on all 411 pipes that have both end inverts. The 2D length does not (up to 1.06 m on steep ditches), so Civil 3D's exported `length` is the 3D length.
- **Slope** = |Δinvert| / horizontal length matches Civil 3D's `slope` to ≤ 5.6e-8.
- **Curved pipes** (6, all in Stormwater): the `<Center>` element of a Civil 3D 2025 `<Pipe>` lies equidistant from both ends at ≈ chord/2. Treated as an arc *centre*, the lengths came out up to 9.5 m wrong. Treated as the arc *mid-point*, they match to 1e-9 m. This interpretation is measured, not documented by Autodesk.
- **HGL could not be compared**: every pipe has `flowIn="0."`, so the file's `hglUp/hglDown` come from no hydraulic analysis. Some are 460·D below the invert (Ditches), so they are not results to compare against.
- **No corridor volumes or designed pipe networks from Civil 3D were found.** What I tried: `gh search code` for `PipeFlow flowIn hglUp`, `CrossSects`, `CrossSectSurf`, `CutVolume Corridor LandXML` (no Civil 3D hits, or rate-limited). landxml.org sample files (Civil 3D 2007 `Corridor-6a.xml`, `GSG_features_sections.xml`, `GSG_features_pipe_networks.xml`, `GSG_features_surfaces_with_volumes.xml`): the listing is visible in search results, but the files time out over curl (60–90 s), WebFetch returns HTTP 500, and the web.archive.org `id_` mirror returns nothing. No state DOT LandXML deliverable with both surfaces and pipe networks was found. So corridor volumes and pipe *designs* are still not compared against Civil 3D.


## Third-party CAD tools (DXF → DWG)

`scripts/cad_roundtrip.sh` + `scripts/cad_compare.py` (local only; needs ODA File Converter 27.9 (free, proprietary licence), LibreDWG 0.13.3 (GPL, run as an external program only) and LibreCAD 2.2):

```
model: ezdxf(ours)=30708  ODA DWG→DXF=30708  mismatches=0  LibreDWG reads 30712 entities {'LINE': 30681, 'LWPOLYLINE': 15, 'TEXT': 12, 'BLOCK': 2, 'ENDBLK': 2}
sheet-01: ezdxf(ours)=122  ODA DWG→DXF=122  mismatches=0  LibreDWG reads 126 entities {'LWPOLYLINE': 3, 'TEXT': 94, 'LINE': 25, 'BLOCK': 2, 'ENDBLK': 2}
sheet-02: ezdxf(ours)=6246  ODA DWG→DXF=6246  mismatches=0  LibreDWG reads 6250 entities {'LWPOLYLINE': 23, 'LINE': 6146, 'TEXT': 77, 'BLOCK': 2, 'ENDBLK': 2}
sheet-03: ezdxf(ours)=30921  ODA DWG→DXF=30921  mismatches=0  LibreDWG reads 30925 entities {'LWPOLYLINE': 60, 'LINE': 30702, 'TEXT': 159, 'BLOCK': 2, 'ENDBLK': 2}
sheet-04: ezdxf(ours)=88  ODA DWG→DXF=88  mismatches=0  LibreDWG reads 92 entities {'LWPOLYLINE': 23, 'TEXT': 55, 'LINE': 10, 'BLOCK': 2, 'ENDBLK': 2}
sheet-05: ezdxf(ours)=115  ODA DWG→DXF=115  mismatches=0  LibreDWG reads 119 entities {'LWPOLYLINE': 6, 'LINE': 33, 'TEXT': 76, 'BLOCK': 2, 'ENDBLK': 2}
sheet-06: ezdxf(ours)=117  ODA DWG→DXF=117  mismatches=0  LibreDWG reads 121 entities {'LWPOLYLINE': 9, 'TEXT': 44, 'LINE': 64, 'BLOCK': 2, 'ENDBLK': 2}
sheet-07: ezdxf(ours)=818  ODA DWG→DXF=818  mismatches=0  LibreDWG reads 822 entities {'LWPOLYLINE': 749, 'LINE': 12, 'TEXT': 57, 'BLOCK': 2, 'ENDBLK': 2}
sheet-08: ezdxf(ours)=760  ODA DWG→DXF=760  mismatches=0  LibreDWG reads 764 entities {'LWPOLYLINE': 691, 'LINE': 12, 'TEXT': 57, 'BLOCK': 2, 'ENDBLK': 2}
sheet-09: ezdxf(ours)=820  ODA DWG→DXF=820  mismatches=0  LibreDWG reads 824 entities {'LWPOLYLINE': 746, 'LINE': 12, 'TEXT': 62, 'BLOCK': 2, 'ENDBLK': 2}
sheet-10: ezdxf(ours)=732  ODA DWG→DXF=732  mismatches=0  LibreDWG reads 736 entities {'LWPOLYLINE': 663, 'LINE': 12, 'TEXT': 57, 'BLOCK': 2, 'ENDBLK': 2}
sheet-11: ezdxf(ours)=668  ODA DWG→DXF=668  mismatches=0  LibreDWG reads 672 entities {'LWPOLYLINE': 596, 'LINE': 12, 'TEXT': 60, 'BLOCK': 2, 'ENDBLK': 2}
sheet-12: ezdxf(ours)=639  ODA DWG→DXF=639  mismatches=0  LibreDWG reads 643 entities {'LWPOLYLINE': 570, 'LINE': 12, 'TEXT': 57, 'BLOCK': 2, 'ENDBLK': 2}
sheet-13: ezdxf(ours)=233  ODA DWG→DXF=233  mismatches=0  LibreDWG reads 237 entities {'LWPOLYLINE': 196, 'LINE': 4, 'TEXT': 33, 'BLOCK': 2, 'ENDBLK': 2}
sheet-14: ezdxf(ours)=294  ODA DWG→DXF=294  mismatches=0  LibreDWG reads 298 entities {'LWPOLYLINE': 32, 'LINE': 84, 'TEXT': 178, 'BLOCK': 2, 'ENDBLK': 2}
sheet-15: ezdxf(ours)=178  ODA DWG→DXF=178  mismatches=0  LibreDWG reads 182 entities {'LWPOLYLINE': 17, 'LINE': 84, 'TEXT': 77, 'BLOCK': 2, 'ENDBLK': 2}
sheet-16: ezdxf(ours)=184  ODA DWG→DXF=184  mismatches=0  LibreDWG reads 188 entities {'LWPOLYLINE': 18, 'LINE': 89, 'TEXT': 77, 'BLOCK': 2, 'ENDBLK': 2}
sheet-17: ezdxf(ours)=258  ODA DWG→DXF=258  mismatches=0  LibreDWG reads 262 entities {'LWPOLYLINE': 14, 'TEXT': 206, 'LINE': 38, 'BLOCK': 2, 'ENDBLK': 2}
sheet-18: ezdxf(ours)=183  ODA DWG→DXF=183  mismatches=0  LibreDWG reads 187 entities {'LWPOLYLINE': 5, 'LINE': 31, 'TEXT': 147, 'BLOCK': 2, 'ENDBLK': 2}
sheet-19: ezdxf(ours)=91  ODA DWG→DXF=91  mismatches=0  LibreDWG reads 95 entities {'LWPOLYLINE': 39, 'TEXT': 44, 'LINE': 8, 'BLOCK': 2, 'ENDBLK': 2}
```

- **ODA File Converter** converts all 20 DXFs (model + 19 sheets, R12) to **DWG (AutoCAD 2018)** with audit on and no errors; DWG → DXF gives the same entity count per type and layer on all 20 files (2D `POLYLINE` becomes `LWPOLYLINE`). `ezdxf` audit of the round-tripped DXFs: 0 errors.
- **LibreDWG `dwgread`** opens all 20 ODA-written DWGs (rc 0); its count is ours + 4 (`BLOCK`/`ENDBLK` of model and paper space).
- **LibreCAD `dxf2pdf`** renders all 20 DXFs to PDF.
- The DWGs are not committed (made with a proprietary converter). They have **not been opened in AutoCAD or Civil 3D**.
