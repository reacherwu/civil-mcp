# Validation record (measured 2026-10-20)

## Independent checks on the demo output

`python scripts/independent_check.py demo-out --xsd schema/LandXML-1.2.xsd` (also run in CI):

```
OK  section cut area, max rel diff where area>1 m² (%): civil-mcp 0.0237971 | independent 0 | diff +0.0238 % (tol 0.05)
OK  section fill area, max rel diff where area>1 m² (%): civil-mcp 0.027649 | independent 0 | diff +0.02765 % (tol 0.05)
OK  section areas, max abs diff (m²): civil-mcp 0.053903 | independent 0 | diff +0.0539 m² (tol 0.1)
OK  corridor cut volume (avg end area, independent areas): civil-mcp 66179.8 | independent 66178.5 | diff +0.001908% (tol 0.05)
OK  corridor fill volume (avg end area, independent areas): civil-mcp 92473.5 | independent 92476.7 | diff -0.003458% (tol 0.05)
OK  mass haul final ordinate (m³): civil-mcp -32911.7 | independent -32911.7 | diff -2.211e-14% (tol 0.01)
OK  mass haul max |ordinate diff| (m³): civil-mcp 7.27596e-12 | independent 0 | diff +7.276e-12 m³ (tol 1.0)
OK  haul moment ∫|M|ds (m³·m): civil-mcp 3.01354e+07 | independent 3.01456e+07 | diff -0.03371% (tol 0.5)
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
OK  rational Q = CiA/360, max abs diff (m³/s): civil-mcp 0 | independent 0 | diff +0 m³/s (tol 1e-09)
OK  Manning full-flow Q, max diff (%): civil-mcp 1.53893e-14 | independent 0 | diff +1.539e-14 % (tol 1e-06)
OK  partial-flow depth ratio (brentq), max abs diff: civil-mcp 2.34923e-13 | independent 0 | diff +2.349e-13  (tol 0.0001)
OK  partial-flow velocity, max abs diff (m/s): civil-mcp 1.27764e-12 | independent 0 | diff +1.278e-12 m/s (tol 0.001)
OK  yard grading cut vs analytic mitred model (m³): civil-mcp 2474.96 | independent 2475.01 | diff -0.00195% (tol 1.0)
OK  yard grading fill vs analytic mitred model (m³): civil-mcp 2242.25 | independent 2248.42 | diff -0.2743% (tol 1.0)
OK  DXF audit errors+fixes (all files): civil-mcp 0 | independent 0 | diff +0  (tol 0)
OK  PDF text extract contains 横断面图 and 初步设计: civil-mcp 1 | independent 1 | diff +0  (tol 0)

27 checks, 0 failed
```

Notes: section areas are re-integrated from the stored ground/datum polylines with 20,001 samples per side. The grading check uses an analytic mitred-offset model (outward distance = max of edge-normal distances; design = pad ± d/slope, clipped by ground) on a 0.1 m grid over a SciPy/Qhull EG. The fill difference (−0.27 %) comes from the Rust strips interpolating the daylight line linearly between 1 m footprint samples. The Rust corridor also reports its own composite-grid cross-check: cut −0.23 %, fill −0.07 %.

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
