# Real-terrain route tests

Run on 2026-10-20 (PDT) with `scripts/terrain_batch.py`. Each surface is loaded with `import_terrain`, then `generate_route_alternatives` runs between the points at 15 % and 85 % of the surface extent. Basis: AASHTO 2018, 50 km/h, 10 % maximum grade, 3.5 m lanes; grid cell = max(10 m, import cell); radius_pref 120 m; spiral 30 m; `hairpin_min_radius_m` 30 entered as an engineer test input. Earthwork limits are the agent screening defaults (fill 15 m, cut 15 m, wall 10 m) and need engineer confirmation.

End points are mechanical (bbox fractions), so several sites ask for more climb than the box allows. Those are the cases the infeasibility stop is meant to catch.

| site | source | terrain | CRS in → out | ground pts | z range (m) | result |
|---|---|---|---|---|---|---|
| boulder | USGS 3DEP 1/3″ via 3DEPElevation exportImage, bbox -105.31,39.98,-105.26,40.02, imageSR 32613, 420×440 px, step 2 | mountain (Flatirons foothills) | EPSG:32613 (projected (kept planar)) → same, metres | 46200 | 1610.9–2476.6 | **INFEASIBLE** (Δz 639.0 m over 4321 m; ≥ 6390 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 20.4 m) |
| boulder10 | same file, full resolution (step 1) | mountain | EPSG:32613 (projected (kept planar)) → same, metres | 184800 | 1610.2–2478.4 | **INFEASIBLE** (Δz 644.7 m over 4331 m; ≥ 6447 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 10.2 m) |
| paloalto | USGS 3DEP, bbox -122.21,37.36,-122.16,37.40, imageSR 32610, 440×440, step 2 | hills | EPSG:32610 (projected (kept planar)) → same, metres | 48400 | 53.1–333.3 | recommended A-direct |
| paloalto_stateplane_ft | USGS 3DEP, same bbox, imageSR 2227 (CA zone III, US ft), step 2 | hills, state plane ft | EPSG:2227 (projected (kept planar)) → same, metres | 48400 | 53.1–334.2 | recommended A-direct |
| kansas | USGS 3DEP, bbox -98.60,38.50,-98.56,38.53, imageSR 32614, step 2 | flat | EPSG:32614 (projected (kept planar)) → same, metres | 28875 | 539.5–568.4 | recommended A-direct |
| iowa | USGS 3DEP, bbox -93.66,41.95,-93.61,41.98, imageSR 32615, step 2 | rolling | EPSG:32615 (projected (kept planar)) → same, metres | 34650 | 274.2–315.3 | recommended A-direct |
| wv | USGS 3DEP, bbox -80.31,38.60,-80.26,38.64, imageSR 32617, step 2 | Appalachian, dissected | EPSG:32617 (projected (kept planar)) → same, metres | 48400 | 585.8–995.1 | **INFEASIBLE** (Δz 18.8 m over 4407 m; ≥ 188 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 20.3 m) |
| wv10 | same file, full resolution (step 1) | Appalachian | EPSG:32617 (projected (kept planar)) → same, metres | 193600 | 585.8–996.0 | **INFEASIBLE** (Δz 15.7 m over 4418 m; ≥ 157 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: EW-FILL 22.9 m > 15.0 m; EW-CUT 21.1 m > 15.0 m; EW-WALL 25.0 m > 10.0 m |
| autzen_laz | PDAL sample autzen-classified.laz (10.65 M pts; class 2 ground, step 100) | flat urban; LAZ, Oregon GIC Lambert intl ft + NAVD88 US ft | EPSG:2992 (projected (kept planar)) → same, metres | 69095 | 124.0–146.9 | recommended A-direct |
| uk_lake_district | Copernicus GLO-30 COG N54 W004, crop 4 km around 54.45 N 3.21 W | mountain, UK | EPSG:4326 (geographic) → EPSG:32630 | 19107 | 211.4–974.0 | **INFEASIBLE** (Δz 223.2 m over 3944 m; ≥ 2232 m of road needed at 10 %). A–C exceed the fill/wall limits; D-switchback: no path between start and end inside the surface keeps the ground grade within 20.0 % (2 × the 10.0 % design maximum, grid 10.0 m) |
| au_blue_mtns | Copernicus GLO-30 COG S34 E150, crop 4 km around 33.71 S 150.31 E | plateau / escarpment, AU | EPSG:4326 (geographic) → EPSG:32756 | 20158 | 601.5–1084.2 | recommended A-direct |

## Alternatives on the sites that produced a recommendation

| site | alt | length m | max grade % | min R m | cut m³ | fill m³ | max fill / cut / wall m | limit failures | hairpin exceptions | superelevation failures | overlap with earlier % |
|---|---|---|---|---|---|---|---|---|---|---|---|
| paloalto | A-direct | 4969.4 | 10.0 | 85.0 | 33627 | 33213 | 4.7 / 3.1 / 5.2 | - | 0 | 9 | 0 |
| paloalto | B-contour-following | 7587.6 | 10.0 | 90.0 | 73785 | 58576 | 8.7 / 15.2 / 15.7 | EW-CUT 15.2 m > 15.0 m; EW-WALL 15.7 m > 10.0 m | 0 | 30 | 21 |
| paloalto | C-separate-corridor | 5412.6 | 10.0 | 95.0 | 48399 | 48001 | 4.6 / 5.9 / 5.2 | - | 0 | 15 | 11 |
| paloalto | D-switchback | 7340.3 | 9.8 | 30.0 | 60950 | 60494 | 4.7 / 3.9 / 4.7 | - | 5 | 36 | 49 |
| paloalto_stateplane_ft | A-direct | 5138.1 | 10.0 | 120.0 | 40883 | 24408 | 3.1 / 3.2 / 0.0 | - | 0 | 11 | 0 |
| paloalto_stateplane_ft | B-contour-following | 5175.8 | 10.0 | 120.0 | 33981 | 17198 | 2.8 / 6.0 / 0.0 | - | 0 | 14 | 83 |
| paloalto_stateplane_ft | C-separate-corridor | 5880.2 | 9.6 | 120.0 | 34510 | 25446 | 2.6 / 4.4 / 0.0 | - | 0 | 13 | 6 |
| paloalto_stateplane_ft | D-switchback | 5823.9 | 9.2 | 50.0 | 31965 | 37638 | 5.3 / 2.5 / 0.0 | - | 2 | 16 | 94 |
| kansas | A-direct | 3404.9 | 4.5 | 120.0 | 12852 | 6322 | 2.6 / 1.6 / 0.0 | - | 0 | 2 | 0 |
| kansas | B-contour-following | 3490.5 | 4.6 | 120.0 | 10527 | 2196 | 0.6 / 0.5 / 0.0 | - | 0 | 3 | 9 |
| kansas | C-separate-corridor | 3534.6 | 5.0 | 120.0 | 11685 | 6868 | 1.8 / 1.1 / 0.0 | - | 0 | 4 | 4 |
| kansas | D-switchback | 3418.1 | 6.0 | 120.0 | 13236 | 8039 | 2.5 / 2.3 / 0.0 | - | 0 | 2 | 8 |
| iowa | A-direct | 3834.7 | 4.7 | 120.0 | 17374 | 9026 | 2.1 / 1.5 / 0.0 | - | 0 | 1 | 0 |
| iowa | B-contour-following | 3918.9 | 3.7 | 120.0 | 12030 | 5673 | 1.8 / 1.5 / 0.0 | - | 0 | 4 | 18 |
| iowa | C-separate-corridor | 3994.5 | 6.1 | 120.0 | 13463 | 5570 | 1.6 / 1.6 / 0.0 | - | 0 | 6 | 3 |
| iowa | D-switchback | 3837.8 | 2.8 | 120.0 | 12792 | 5320 | 1.8 / 1.2 / 0.0 | - | 0 | 2 | 78 |
| autzen_laz | A-direct | 1277.0 | 4.4 | 120.0 | 4406 | 2507 | 1.2 / 1.0 / 0.0 | - | 0 | 2 | 0 |
| autzen_laz | B-contour-following | 1345.4 | 2.9 | 100.0 | 4180 | 3796 | 1.6 / 1.8 / 0.0 | - | 0 | 5 | 11 |
| autzen_laz | C-separate-corridor | 1455.2 | 3.9 | 120.0 | 9263 | 5519 | 2.4 / 3.8 / 0.0 | - | 0 | 5 | 6 |
| autzen_laz | D-switchback | 1277.0 | 4.4 | 120.0 | 4406 | 2507 | 1.2 / 1.0 / 0.0 | - | 0 | 2 | 100 |
| au_blue_mtns | A-direct | 4385.7 | 10.0 | 90.0 | 50371 | 74307 | 7.5 / 6.9 / 8.5 | - | 0 | 17 | 0 |
| au_blue_mtns | B-contour-following | 4468.7 | 10.0 | 80.0 | 113482 | 162153 | 14.1 / 12.5 / 16.5 | EW-WALL 16.5 m > 10.0 m | 0 | 20 | 53 |
| au_blue_mtns | C-separate-corridor | 4853.8 | 10.0 | 100.0 | 122225 | 340538 | 11.0 / 21.9 / 22.7 | EW-CUT 21.9 m > 15.0 m; EW-WALL 22.7 m > 10.0 m | 0 | 19 | 2 |
| au_blue_mtns | D-switchback | 4744.0 | 9.8 | 65.0 | 46498 | 41794 | 5.1 / 4.6 / 3.6 | - | 4 | 28 | 79 |

## What this shows

- Import works on GeoTIFF (UTM and state-plane US ft), Copernicus COG in geographic coordinates (projected to UTM; the projection matches PROJ to < 1 mm on the unit-test points), and LAZ with a compound WKT (horizontal intl ft, vertical US survey ft).
- On the steep boxes (Boulder, West Virginia, Lake District) the run stops with INFEASIBLE and states why. Before this change the Boulder input returned a recommended route with 150–260 m fills.
- The switchback search (D) produced hairpin alignments within the grade and height limits at Palo Alto and the Blue Mountains, using R = 30 m hairpins that depend on the engineer-entered design exception. On flat or rolling ground it falls back to near-direct routes; at Autzen it duplicated A (overlap 100 %, flagged).
- Every alternative still fails superelevation runoff checks (SE-IN-LS / SE-OVERLAP) with a 30 m spiral at these radii. The runs were not tuned to remove them, so a route chosen here is a concept-stage (30 %) corridor.
- Boulder at 10 m and 20 m cells: no path inside the 4 km box holds the ground grade within 20 % between the chosen points. This does not prove a road is impossible on the real terrain outside the box.
