# Builds docs/terrain-tests.md from docs/terrain-tests.jsonl (output of scripts/terrain_batch.py).
import json, re, sys
SRC = {
    'boulder': ('USGS 3DEP 1/3″ via 3DEPElevation exportImage, bbox -105.31,39.98,-105.26,40.02, imageSR 32613, 420×440 px, step 2', 'mountain (Flatirons foothills)'),
    'boulder10': ('same file, full resolution (step 1)', 'mountain'),
    'paloalto': ('USGS 3DEP, bbox -122.21,37.36,-122.16,37.40, imageSR 32610, 440×440, step 2', 'hills'),
    'paloalto_stateplane_ft': ('USGS 3DEP, same bbox, imageSR 2227 (CA zone III, US ft), step 2', 'hills, state plane ft'),
    'kansas': ('USGS 3DEP, bbox -98.60,38.50,-98.56,38.53, imageSR 32614, step 2', 'flat'),
    'iowa': ('USGS 3DEP, bbox -93.66,41.95,-93.61,41.98, imageSR 32615, step 2', 'rolling'),
    'wv': ('USGS 3DEP, bbox -80.31,38.60,-80.26,38.64, imageSR 32617, step 2', 'Appalachian, dissected'),
    'wv10': ('same file, full resolution (step 1)', 'Appalachian'),
    'autzen_laz': ('PDAL sample autzen-classified.laz (10.65 M pts; class 2 ground, step 100)', 'flat urban; LAZ, Oregon GIC Lambert intl ft + NAVD88 US ft'),
    'uk_lake_district': ('Copernicus GLO-30 COG N54 W004, crop 4 km around 54.45 N 3.21 W', 'mountain, UK'),
    'au_blue_mtns': ('Copernicus GLO-30 COG S34 E150, crop 4 km around 33.71 S 150.31 E', 'plateau / escarpment, AU'),
}
seen = {}
for l in open(sys.argv[1] if len(sys.argv) > 1 else 'docs/terrain-tests.jsonl'):
    d = json.loads(l); seen[d['site']] = d
md = ["# Real-terrain route tests", "",
      "Run on 2026-10-20 (PDT) with `scripts/terrain_batch.py`. Each surface is loaded with `import_terrain`, then `generate_route_alternatives` runs between the points at 15 % and 85 % of the surface extent. Basis: AASHTO 2018, 50 km/h, 10 % maximum grade, 3.5 m lanes; grid cell = max(10 m, import cell); radius_pref 120 m; spiral 30 m; `hairpin_min_radius_m` 30 entered as an engineer test input. Earthwork limits are the agent screening defaults (fill 15 m, cut 15 m, wall 10 m) and need engineer confirmation.", "",
      "End points are mechanical (bbox fractions), so several sites ask for more climb than the box allows. Those are the cases the infeasibility stop is meant to catch.", "",
      "| site | source | terrain | CRS in → out | ground pts | z range (m) | result |", "|---|---|---|---|---|---|---|"]
det = ["", "## Alternatives on the sites that produced a recommendation", "",
       "| site | alt | length m | max grade % | min R m | cut m³ | fill m³ | max fill / cut / wall m | limit failures | hairpin exceptions | superelevation failures | overlap with earlier % |",
       "|---|---|---|---|---|---|---|---|---|---|---|---|"]
for k, (s, t) in SRC.items():
    d = seen[k]
    crs = f"EPSG:{d['epsg']} ({d['crs']}) → " + (f"EPSG:{d['target']}" if d['target'] != d['epsg'] else "same, metres")
    if 'result' in d:
        r = d['result']; m = re.search(r'Elevation difference ([\d.]+) m over (\d+) m straight distance needs at least (\d+) m', r)
        ds = r[r.find('D-switchback:'):].split('. Options')[0]
        ds = re.sub(r' \(agent screening default - engineer to confirm\)', '', ds).split(' (+')[0]
        res = f"**INFEASIBLE** (Δz {m.group(1)} m over {m.group(2)} m; ≥ {m.group(3)} m of road needed at 10 %). A–C exceed the fill/wall limits; {ds}"
    else:
        res = f"recommended {d['rec']}"
        for a in d['alts']:
            if 'error' in a:
                det.append(f"| {k} | {a['name']} | build error: {a['error'][:90]} | | | | | | | | | |"); continue
            det.append(f"| {k} | {a['name']} | {a['length_m']} | {a['max_design_grade_pct']} | {a['min_radius_m']} | {a['cut_m3']:.0f} | {a['fill_m3']:.0f} | {a['max_fill_height_m']} / {a['max_cut_height_m']} / {a['max_wall_height_m']} | {'; '.join(x.split(' (')[0] for x in a['fails']) or '-'} | {len(a['design_exceptions'])} | {a['n_se_fails']} | {a['overlap']} |")
    md.append(f"| {k} | {s} | {t} | {crs} | {d['pts']} | {d['z'][0]}–{d['z'][1]} | {res.replace('|', '/')} |")
md += det + ["", "## What this shows", "",
 "- Import works on GeoTIFF (UTM and state-plane US ft), Copernicus COG in geographic coordinates (projected to UTM; the projection matches PROJ to < 1 mm on the unit-test points), and LAZ with a compound WKT (horizontal intl ft, vertical US survey ft).",
 "- On the steep boxes (Boulder, West Virginia, Lake District) the run stops with INFEASIBLE and states why. Before this change the Boulder input returned a recommended route with 150–260 m fills.",
 "- The switchback search (D) produced hairpin alignments within the grade and height limits at Palo Alto and the Blue Mountains, using R = 30 m hairpins that depend on the engineer-entered design exception. On flat or rolling ground it falls back to near-direct routes; at Autzen it duplicated A (overlap 100 %, flagged).",
 "- Every alternative still fails superelevation runoff checks (SE-IN-LS / SE-OVERLAP) with a 30 m spiral at these radii. The runs were not tuned to remove them, so a route chosen here is a concept-stage (30 %) corridor.",
 "- Boulder at 10 m and 20 m cells: no path inside the 4 km box holds the ground grade within 20 % between the chosen points. This does not prove a road is impossible on the real terrain outside the box."]
open('docs/terrain-tests.md', 'w').write("\n".join(md) + "\n")
