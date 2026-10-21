# Real-terrain route tests

Run on 2026-10-20 (PDT) with `scripts/terrain_batch.py`. Each surface is loaded with `import_terrain`, then `generate_route_alternatives` runs between the points at 15 % and 85 % of the surface extent. Basis: AASHTO 2018, 50 km/h, 10 % maximum grade, 3.5 m lanes; grid cell = max(10 m, import cell); radius_pref 120 m; spiral 30 m; `hairpin_min_radius_m` 30 entered as an engineer test input. Earthwork limits are the agent screening defaults (fill 15 m, cut 15 m, wall 10 m) and need engineer confirmation.

End points are mechanical (bbox fractions), so several sites ask for more climb than the box allows. Those are the cases the infeasibility stop is meant to catch.

| site | source | terrain | CRS in → out | ground pts | z range (m) | result |
|---|---|---|---|---|---|---|
| boulder | USGS 3DEP 1/3″ via 3DEPElevation exportImage, bbox -105.31,39.98,-105.26,40.02, imageSR 32613, 420×440 px, step 2 | mountain (Flatirons foothills) | EPSG:32613 (projected (kept planar)) → same, metres | 46200 | 1610.9–2476.6 | **INFEASIBLE** (Δz 639.0 m over 4321 m; ≥ 6390 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 20.4 m) |
| boulder10 | same file, full resolution (step 1) | mountain | EPSG:32613 (projected (kept planar)) → same, metres | 184800 | 1610.2–2478.4 | **INFEASIBLE** (Δz 644.7 m over 4331 m; ≥ 6447 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 10.2 m) |
| paloalto | USGS 3DEP, bbox -122.21,37.36,-122.16,37.40, imageSR 32610, 440×440, step 2 | hills | EPSG:32610 (projected (kept planar)) → same, metres | 48400 | 53.1–333.3 | recommended A-direct |
| paloalto_stateplane_ft | USGS 3DEP, same bbox, imageSR 2227 (CA zone III, US ft), step 2 | hills, state plane ft | EPSG:2227 (projected (kept planar)) → same, metres | 48400 | 53.1–334.2 | recommended C-separate-corridor |
| kansas | USGS 3DEP, bbox -98.60,38.50,-98.56,38.53, imageSR 32614, step 2 | flat | EPSG:32614 (projected (kept planar)) → same, metres | 28875 | 539.5–568.4 | recommended B-contour-following |
| iowa | USGS 3DEP, bbox -93.66,41.95,-93.61,41.98, imageSR 32615, step 2 | rolling | EPSG:32615 (projected (kept planar)) → same, metres | 34650 | 274.2–315.3 | recommended B-contour-following |
| wv | USGS 3DEP, bbox -80.31,38.60,-80.26,38.64, imageSR 32617, step 2 | Appalachian, dissected | EPSG:32617 (projected (kept planar)) → same, metres | 48400 | 585.8–995.1 | **INFEASIBLE** (Δz 18.8 m over 4407 m; ≥ 188 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 20.3 m) |
| wv10 | same file, full resolution (step 1) | Appalachian | EPSG:32617 (projected (kept planar)) → same, metres | 193600 | 585.8–996.0 | **INFEASIBLE** (Δz 15.7 m over 4418 m; ≥ 157 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: EW-FILL 48.1 m > 15.0 m; EW-CUT 32.6 m > 15.0 m; EW-WALL 49.8 m > 10.0 m |
| autzen_laz | PDAL sample autzen-classified.laz (10.65 M pts; class 2 ground, step 100) | flat urban; LAZ, Oregon GIC Lambert intl ft + NAVD88 US ft | EPSG:2992 (projected (kept planar)) → same, metres | 69095 | 124.0–146.9 | recommended A-direct |
| uk_lake_district | Copernicus GLO-30 COG N54 W004, crop 4 km around 54.45 N 3.21 W | mountain, UK | EPSG:4326 (geographic) → EPSG:32630 | 19107 | 211.4–974.0 | **INFEASIBLE** (Δz 223.2 m over 3944 m; ≥ 2232 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 10.0 m) |
| au_blue_mtns | Copernicus GLO-30 COG S34 E150, crop 4 km around 33.71 S 150.31 E | plateau / escarpment, AU | EPSG:4326 (geographic) → EPSG:32756 | 20158 | 601.5–1084.2 | recommended D-switchback |

## Alternatives on the sites that produced a recommendation

| site | alt | length m | max grade % | min R m | cut m³ | fill m³ | max fill / cut / wall m | limit failures | hairpin exceptions | superelevation failures (before → after) | runoff re-check shortfalls | overlap with earlier % |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| paloalto | A-direct | 4965.1 | 10.0 | 120.0 | 38073 | 28846 | 4.7 / 2.9 / 6.7 | - | 0 | 9 → 0 | 0 | 0 |
| paloalto | B-contour-following | 7588.6 | 10.0 | 85.0 | 113162 | 67585 | 7.0 / 24.4 / 26.5 | EW-CUT 24.4 m > 15.0 m; EW-WALL 26.5 m > 10.0 m | 0 | 30 → 0 | 0 | 21 |
| paloalto | C-separate-corridor | 5309.8 | 10.0 | 120.0 | 43152 | 49591 | 4.7 / 4.6 / 5.2 | - | 0 | 15 → 0 | 0 | 11 |
| paloalto | D-switchback | 7090.3 | 10.0 | 70.0 | 82909 | 135263 | 10.3 / 6.1 / 10.8 | EW-WALL 10.8 m > 10.0 m | 2 | 36 → 0 | 0 | 49 |
| paloalto_stateplane_ft | A-direct | 5137.4 | 10.0 | 115.0 | 40933 | 24434 | 3.2 / 3.2 / 0.0 | - | 0 | 11 → 0 | 0 | 0 |
| paloalto_stateplane_ft | B-contour-following | 5174.5 | 10.0 | 120.0 | 34069 | 17002 | 2.7 / 6.0 / 0.0 | - | 0 | 14 → 0 | 0 | 83 |
| paloalto_stateplane_ft | C-separate-corridor | 5895.8 | 9.6 | 120.0 | 32736 | 29312 | 2.7 / 4.4 / 0.0 | - | 0 | 13 → 0 | 0 | 6 |
| paloalto_stateplane_ft | D-switchback | 5819.2 | 9.4 | 35.0 | 34739 | 36801 | 4.1 / 3.0 / 0.0 | - | 1 | 16 → 0 | 0 | 94 |
| kansas | A-direct | 3404.8 | 4.5 | 145.0 | 12862 | 6311 | 2.6 / 1.6 / 0.0 | - | 0 | 2 → 0 | 0 | 0 |
| kansas | B-contour-following | 3490.3 | 4.6 | 120.0 | 10543 | 2198 | 0.6 / 0.5 / 0.0 | - | 0 | 3 → 0 | 0 | 9 |
| kansas | C-separate-corridor | 3534.2 | 5.0 | 120.0 | 11625 | 6827 | 1.8 / 1.1 / 0.0 | - | 0 | 4 → 0 | 0 | 4 |
| kansas | D-switchback | 3418.0 | 6.0 | 120.0 | 13227 | 8042 | 2.5 / 2.3 / 0.0 | - | 0 | 2 → 0 | 0 | 8 |
| iowa | A-direct | 3834.7 | 4.7 | 135.0 | 17380 | 9016 | 2.1 / 1.5 / 0.0 | - | 0 | 1 → 0 | 0 | 0 |
| iowa | B-contour-following | 3918.6 | 3.7 | 120.0 | 11976 | 5656 | 1.8 / 1.5 / 0.0 | - | 0 | 4 → 0 | 0 | 18 |
| iowa | C-separate-corridor | 3994.0 | 6.1 | 120.0 | 13481 | 5572 | 1.7 / 1.6 / 0.0 | - | 0 | 6 → 0 | 0 | 3 |
| iowa | D-switchback | 3837.6 | 2.8 | 120.0 | 12812 | 5296 | 1.8 / 1.2 / 0.0 | - | 0 | 2 → 0 | 0 | 78 |
| autzen_laz | A-direct | 1276.9 | 4.5 | 120.0 | 4410 | 2486 | 1.2 / 0.9 / 0.0 | - | 0 | 2 → 0 | 0 | 0 |
| autzen_laz | B-contour-following | 1338.2 | 3.0 | 120.0 | 4651 | 3148 | 1.5 / 1.9 / 0.0 | - | 0 | 5 → 0 | 0 | 11 |
| autzen_laz | C-separate-corridor | 1454.6 | 3.9 | 120.0 | 9238 | 5522 | 2.4 / 3.8 / 0.0 | - | 0 | 5 → 0 | 0 | 6 |
| autzen_laz | D-switchback | 1276.9 | 4.5 | 120.0 | 4410 | 2486 | 1.2 / 0.9 / 0.0 | - | 0 | 2 → 0 | 0 | 100 |
| au_blue_mtns | A-direct | 4354.3 | 10.0 | 100.0 | 83245 | 130222 | 14.1 / 7.9 / 16.7 | EW-WALL 16.7 m > 10.0 m | 0 | 17 → 0 | 0 | 0 |
| au_blue_mtns | B-contour-following | 4263.4 | 10.0 | 85.0 | 83278 | 250944 | 20.4 / 8.5 / 21.0 | EW-FILL 20.4 m > 15.0 m; EW-WALL 21.0 m > 10.0 m | 0 | 20 → 0 | 0 | 53 |
| au_blue_mtns | C-separate-corridor | 4825.8 | 10.0 | 85.0 | 130426 | 209352 | 10.1 / 21.4 / 22.3 | EW-CUT 21.4 m > 15.0 m; EW-WALL 22.3 m > 10.0 m | 0 | 19 → 0 | 0 | 2 |
| au_blue_mtns | D-switchback | 4671.8 | 9.9 | 35.0 | 55637 | 44968 | 5.6 / 4.0 / 0.0 | - | 6 | 28 → 0 | 0 | 79 |

## What this shows

- Import works on GeoTIFF (UTM and state-plane US ft), Copernicus COG in geographic coordinates (projected to UTM; the projection matches PROJ to < 1 mm on the unit-test points), and LAZ with a compound WKT (horizontal intl ft, vertical US survey ft).
- On the steep boxes (Boulder, West Virginia, Lake District) the run stops with INFEASIBLE and states why. Before this change the Boulder input returned a recommended route with 150–260 m fills.
- The switchback search (D) produced a hairpin alignment within the grade and height limits at the Blue Mountains (recommended there), using R = 35 m hairpins that depend on the engineer-entered design exception. At Palo Alto the re-fitted D exceeds the 10 m wall limit by 0.8 m. On flat or rolling ground it falls back to near-direct routes; at Autzen it duplicated A (overlap 100 %, flagged).
- Superelevation runoff: transitions are now sized during alignment fitting to at least the required development L = w·(e+nc)/Δmax (AASHTO relative gradient; the DMRB and Austroads profiles supply their own limits through the same parameter), and the radius is changed (down to the floor, or flatter for small deflections) when the tangents are too short. No alternative fails SE-IN-LS or SE-OVERLAP after the change. The 'runoff re-check' column is an independent Python recomputation of L for each fitted curve (count of spirals shorter than L).
- Fixing the runoff moved some alignments, and the earthwork changed with them. At Palo Alto, D now exceeds the wall limit and B's cut/wall exceedances grew. At the Blue Mountains, A now exceeds the wall limit and B now also exceeds the fill limit. All of these alternatives had runoff failures before, so none was a clean design before or after. With the runoff failures gone, the recommendation rule (fewest mandatory failures, then cost) picks a different alternative at the state-plane Palo Alto, Kansas, Iowa and Blue Mountains runs.
- Hairpins whose deflection leaves no room for spirals are fitted as simple curves (Ls 0). Their development runs on the tangents and is covered only by the SE-OVERLAP check; the tangent runout itself is not checked against the alignment start/end.
- Boulder at 10 m and 20 m cells: no path inside the 4 km box holds the ground grade within 20 % between the chosen points. This does not prove a road is impossible on the real terrain outside the box.
