# Validation record (measured 2026-10-20)

## Independent checks on the demo output

`python scripts/independent_check.py demo-out --xsd schema/LandXML-1.2.xsd` (also run in CI):

```
OK  section cut area, max rel diff where area>1 m² (%): civil-mcp 0.0378067 | independent 0 | diff +0.03781 % (tol 0.05)
OK  section fill area, max rel diff where area>1 m² (%): civil-mcp 0.0418465 | independent 0 | diff +0.04185 % (tol 0.05)
OK  section areas, max abs diff (m²): civil-mcp 0.0181002 | independent 0 | diff +0.0181 m² (tol 0.1)
OK  corridor cut volume (avg end area, independent areas): civil-mcp 57602.2 | independent 57601.1 | diff +0.001819% (tol 0.05)
OK  corridor fill volume (avg end area, independent areas): civil-mcp 69288.8 | independent 69291 | diff -0.003128% (tol 0.05)
OK  mass haul final ordinate (m³): civil-mcp -17446.9 | independent -17446.9 | diff -0% (tol 0.01)
OK  mass haul max |ordinate diff| (m³): civil-mcp 3.63798e-12 | independent 0 | diff +3.638e-12 m³ (tol 1.0)
OK  haul moment ∫|M|ds (m³·m): civil-mcp 2.38388e+07 | independent 2.38519e+07 | diff -0.05496% (tol 0.5)
OK  superelevation e, curve 1: civil-mcp 0.0439895 | independent 0.0439895 | diff +0% (tol 1e-06)
OK  superelevation transition length, curve 1: civil-mcp 33.5945 | independent 33.5945 | diff +2.115e-14% (tol 1e-06)
OK  provided transition ≥ required, curve 1 (m spare): civil-mcp 16.4055 | independent 0 | diff +16.41 m (tol 1000000000.0)
OK  superelevation e, curve 2: civil-mcp 0.0649869 | independent 0.0649869 | diff +0% (tol 1e-06)
OK  superelevation transition length, curve 2: civil-mcp 44.6181 | independent 44.6181 | diff +1.592e-14% (tol 1e-06)
OK  provided transition ≥ required, curve 2 (m spare): civil-mcp 5.38189 | independent 0 | diff +5.382 m (tol 1000000000.0)
OK  clothoid chord L=50 R=150 (m): civil-mcp 49.9383 | independent 49.9383 | diff -7.962e-08 m (tol 1e-06)
OK  clothoid chord L=50 R=150 (m): civil-mcp 49.9383 | independent 49.9383 | diff +2.524e-07 m (tol 1e-06)
OK  clothoid chord L=50 R=120 (m): civil-mcp 49.9036 | independent 49.9036 | diff +2.205e-07 m (tol 1e-06)
OK  clothoid chord L=50 R=120 (m): civil-mcp 49.9036 | independent 49.9036 | diff +5.416e-07 m (tol 1e-06)
OK  LandXML 1.2 XSD validation errors: civil-mcp 0 | independent 0 | diff +0  (tol 0)
OK  [污水 Sanitary] design Q (rational + Kz·Q_avg), max abs diff (m³/s): civil-mcp 0 | independent 0 | diff +0 m³/s (tol 1e-09)
OK  [污水 Sanitary] GB 50014 Kz (np.interp), max abs diff: civil-mcp 0 | independent 0 | diff +0  (tol 1e-12)
OK  [污水 Sanitary] Manning full-flow Q, max diff (%): civil-mcp 1.65714e-14 | independent 0 | diff +1.657e-14 % (tol 1e-06)
OK  [污水 Sanitary] partial-flow depth ratio (brentq), max abs diff: civil-mcp 2.42154e-13 | independent 0 | diff +2.422e-13  (tol 0.0001)
OK  [污水 Sanitary] partial-flow velocity, max abs diff (m/s): civil-mcp 2.74381e-12 | independent 0 | diff +2.744e-12 m/s (tol 0.001)
OK  [污水 Sanitary] pipe HGL/EGL ends, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [污水 Sanitary] structure HGL, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [污水 Sanitary] min freeboard rim − HGL (m, must be ≥ 0.3): civil-mcp 1.1039 | independent 0.3 | diff +0.8039 m (tol 1000000000.0)
OK  [雨水 Storm] design Q (rational + Kz·Q_avg), max abs diff (m³/s): civil-mcp 0 | independent 0 | diff +0 m³/s (tol 1e-09)
OK  [雨水 Storm] GB 50014 Kz (np.interp), max abs diff: civil-mcp 0 | independent 0 | diff +0  (tol 1e-12)
OK  [雨水 Storm] Manning full-flow Q, max diff (%): civil-mcp 1.94285e-14 | independent 0 | diff +1.943e-14 % (tol 1e-06)
OK  [雨水 Storm] partial-flow depth ratio (brentq), max abs diff: civil-mcp 2.34923e-13 | independent 0 | diff +2.349e-13  (tol 0.0001)
OK  [雨水 Storm] partial-flow velocity, max abs diff (m/s): civil-mcp 1.27764e-12 | independent 0 | diff +1.278e-12 m/s (tol 0.001)
OK  [雨水 Storm] pipe HGL/EGL ends, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [雨水 Storm] structure HGL, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-06)
OK  [雨水 Storm] min freeboard rim − HGL (m, must be ≥ 0.3): civil-mcp 0.606954 | independent 0.3 | diff +0.307 m (tol 1000000000.0)
OK  walls (16 sections): sliding FS μV/Pa, max abs diff: civil-mcp 8.88178e-16 | independent 0 | diff +8.882e-16  (tol 1e-06)
OK  walls: overturning FS Mr/Mo (numeric pressure integral), max abs diff: civil-mcp 9.09486e-11 | independent 0 | diff +9.095e-11  (tol 1e-06)
OK  walls: eccentricity, max abs diff (m): civil-mcp 4.23013e-11 | independent 0 | diff +4.23e-11 m (tol 1e-06)
OK  walls: q_max V/B(1+6e/B), max abs diff (kPa): civil-mcp 4.72357e-09 | independent 0 | diff +4.724e-09 kPa (tol 0.001)
OK  walls: min sliding FS (GB 50007 §6.7.5 ≥ 1.3): civil-mcp 1.30012 | independent 1.3 | diff +0.0001183  (tol 1000000000.0)
OK  walls: min overturning FS (≥ 1.6): civil-mcp 3.9568 | independent 1.6 | diff +2.357  (tol 1000000000.0)
OK  JTG 表7.6.1 widening by radius, max abs diff (m): civil-mcp 0 | independent 0 | diff +0 m (tol 1e-12)
OK  X1 ISD = 0.278·V·tg (AASHTO 2018 §9.5.3 B1), max abs diff (m): civil-mcp 2.84217e-14 | independent 0 | diff +2.842e-14 m (tol 1e-09)
OK  X1 curb returns: |dist(centre, leg edge) − R| and |arc end − R|, max (m): civil-mcp 3.40387e-10 | independent 0 | diff +3.404e-10 m (tol 1e-06)
OK  X1 北支路 N leg return-patch fill vs 0.1 m grid on its faces (tool samples 0.25 m): civil-mcp 524.297 | independent 521.32 | diff +0.5711% (tol 1.0)
OK  X1 南支路 S leg return-patch fill vs 0.1 m grid on its faces (tool samples 0.25 m): civil-mcp 1050.59 | independent 1048.72 | diff +0.1786% (tol 1.0)
OK  yard grading cut vs analytic mitred model (m³): civil-mcp 2474.96 | independent 2475.01 | diff -0.00195% (tol 1.0)
OK  yard grading fill vs analytic mitred model (m³): civil-mcp 2242.25 | independent 2248.42 | diff -0.2743% (tol 1.0)
OK  DXF audit errors+fixes (all files): civil-mcp 0 | independent 0 | diff +0  (tol 0)
OK  PDF text extract contains 横断面图 and 初步设计: civil-mcp 1 | independent 1 | diff +0  (tol 0)

50 checks, 0 failed
```

Notes: section areas are re-integrated from the stored ground/datum polylines with 20,001 samples per side. The grading check uses an analytic mitred-offset model (outward distance = max of edge-normal distances; design = pad ± d/slope, clipped by ground) on a 0.1 m grid over a SciPy/Qhull EG. The fill difference (−0.27 %) comes from the Rust strips interpolating the daylight line linearly between 1 m footprint samples. The Rust corridor also reports its own composite-grid cross-check: cut +0.55 %, fill +0.20 %.

New in this round:
- **HGL/EGL** is recomputed by a separate recursive walk from each outfall (critical depth by `brentq` on Q²/g = A³/T, HEC-22 K by deflection via `np.interp`). It reproduces the tool's values exactly, which confirms the implementation matches its stated method, *not* that the method matches HEC-22 in every case (the approximate method is a simplification, see Limits).
- **Walls** are re-analysed from the stored geometry with shoelace polygon centroids and a numerically integrated Rankine pressure (200,001 points), not the Rust closed forms.
- **Intersection return patches**: the volume of the tool's own patch faces integrated on a 0.1 m grid vs the tool's 0.25 m sampling: +0.57 % / +0.18 %. An earlier attempt using SciPy's own Delaunay of the patch vertices gave −1.2 % / +1.0 %: that is a different surface (convex Delaunay over a concave outline), not an error in either.
- The composite cross-check caught a real bug while building curbs: the subgrade datum stopped at the curb in cut, and composite cut differed by +53 %. Fixed (datum steps up at the paved edge) and covered by a unit test.

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

`scripts/cad_roundtrip.sh` + `scripts/cad_compare.py` (local only; needs ODA File Converter 27.9 (free, proprietary licence), LibreDWG 0.13.3 built from source (GPL, run as an external program only) and LibreCAD 2.2):

```
model: ezdxf(ours)=30705  ODA DWG→DXF=30705  mismatches=0  LibreDWG reads 30709 entities {'LINE': 30677, 'LWPOLYLINE': 17, 'TEXT': 11, 'BLOCK': 2, 'ENDBLK': 2}
sheet-01: ezdxf(ours)=30893  ODA DWG→DXF=30893  mismatches=0  LibreDWG reads 30897 entities {'LWPOLYLINE': 56, 'LINE': 30698, 'TEXT': 139, 'BLOCK': 2, 'ENDBLK': 2}
sheet-02: ezdxf(ours)=718  ODA DWG→DXF=718  mismatches=0  LibreDWG reads 722 entities {'LWPOLYLINE': 648, 'LINE': 12, 'TEXT': 58, 'BLOCK': 2, 'ENDBLK': 2}
sheet-03: ezdxf(ours)=551  ODA DWG→DXF=551  mismatches=0  LibreDWG reads 555 entities {'LWPOLYLINE': 483, 'LINE': 12, 'TEXT': 56, 'BLOCK': 2, 'ENDBLK': 2}
sheet-04: ezdxf(ours)=743  ODA DWG→DXF=743  mismatches=0  LibreDWG reads 747 entities {'LWPOLYLINE': 672, 'LINE': 12, 'TEXT': 59, 'BLOCK': 2, 'ENDBLK': 2}
sheet-05: ezdxf(ours)=783  ODA DWG→DXF=783  mismatches=0  LibreDWG reads 787 entities {'LWPOLYLINE': 714, 'TEXT': 57, 'LINE': 12, 'BLOCK': 2, 'ENDBLK': 2}
sheet-06: ezdxf(ours)=770  ODA DWG→DXF=770  mismatches=0  LibreDWG reads 774 entities {'LWPOLYLINE': 699, 'LINE': 12, 'TEXT': 59, 'BLOCK': 2, 'ENDBLK': 2}
sheet-07: ezdxf(ours)=677  ODA DWG→DXF=677  mismatches=0  LibreDWG reads 681 entities {'LWPOLYLINE': 610, 'TEXT': 57, 'LINE': 10, 'BLOCK': 2, 'ENDBLK': 2}
sheet-08: ezdxf(ours)=148  ODA DWG→DXF=148  mismatches=0  LibreDWG reads 152 entities {'LWPOLYLINE': 15, 'LINE': 70, 'TEXT': 63, 'BLOCK': 2, 'ENDBLK': 2}
sheet-09: ezdxf(ours)=165  ODA DWG→DXF=165  mismatches=0  LibreDWG reads 169 entities {'LWPOLYLINE': 17, 'LINE': 81, 'TEXT': 67, 'BLOCK': 2, 'ENDBLK': 2}
sheet-10: ezdxf(ours)=127  ODA DWG→DXF=127  mismatches=0  LibreDWG reads 131 entities {'LWPOLYLINE': 5, 'LINE': 8, 'TEXT': 114, 'BLOCK': 2, 'ENDBLK': 2}
```

- **ODA File Converter** converts every DXF (R12) to **DWG (AutoCAD 2018 format)** with audit on, and with no errors. Converting the DWG back to DXF gives the **same entity count per type and layer, and the same coordinate sum (±0.1 %)**, on all 11 files. The only change is that 2D `POLYLINE` becomes `LWPOLYLINE`.
- **LibreDWG `dwgread`** opens all 11 ODA-written DWGs (rc 0). Its entity count is ours + 4 (`BLOCK`/`ENDBLK` of model and paper space).
- **LibreCAD `dxf2pdf`** renders the sheets. It found a real bug: the sheet DXFs declared `$INSUNITS = 6` (metres) although they are drawn in millimetres, so LibreCAD made a 210 m × 297 m page. Fixed: sheets now declare 4 (mm) and the model declares 6 (m), and CI now checks this. LibreCAD shows the Chinese text as boxes unless a CJK font is installed for it.
- The DWGs are not committed (made with a proprietary converter). They have **not been opened in AutoCAD or Civil 3D**.
