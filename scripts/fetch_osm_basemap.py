#!/usr/bin/env python3
"""Fetch an OpenStreetMap extract for the locality plan and cache it in the repo (offline use).

Data © OpenStreetMap contributors, available under the Open Database License (ODbL 1.0),
https://www.openstreetmap.org/copyright . The cached file is a Derivative Database extract;
keep the attribution on every sheet that shows it (the locality_plan tool prints it).

usage: fetch_osm_basemap.py south west north east out.geojson
"""
import json, sys, urllib.request, urllib.parse, datetime

s, w, n, e = map(float, sys.argv[1:5]); out = sys.argv[5]
q = f"""[out:json][timeout:120];
(
  way["highway"~"^(motorway|trunk|primary|secondary|tertiary|motorway_link|trunk_link|primary_link|residential|unclassified)$"]({s},{w},{n},{e});
  way["waterway"~"^(river|stream|canal)$"]({s},{w},{n},{e});
  way["natural"="water"]({s},{w},{n},{e});
  node["place"~"^(city|town|suburb|village|hamlet|neighbourhood)$"]({s},{w},{n},{e});
);
out geom;"""
req = urllib.request.Request("https://overpass-api.de/api/interpreter", data=urllib.parse.urlencode({"data": q}).encode(),
                             headers={"User-Agent": "civil-mcp locality basemap cache (contact: repo owner)"})
d = json.load(urllib.request.urlopen(req, timeout=180))
feats = []
for el in d["elements"]:
    t = el.get("tags", {})
    if el["type"] == "node":
        feats.append({"type": "Feature", "properties": {"kind": "place", "place": t.get("place"), "name": t.get("name", "")},
                      "geometry": {"type": "Point", "coordinates": [round(el["lon"], 6), round(el["lat"], 6)]}})
    elif "geometry" in el:
        kind = "road" if "highway" in t else ("water" if t.get("natural") == "water" else "waterway")
        feats.append({"type": "Feature", "properties": {"kind": kind, "class": t.get("highway") or t.get("waterway") or "water", "name": t.get("name", ""), "ref": t.get("ref", "")},
                      "geometry": {"type": "LineString", "coordinates": [[round(p["lon"], 6), round(p["lat"], 6)] for p in el["geometry"]]}})
gj = {"type": "FeatureCollection", "attribution": "© OpenStreetMap contributors, ODbL 1.0 (https://www.openstreetmap.org/copyright)",
      "license": "ODbL-1.0", "source": "Overpass API extract", "retrieved": datetime.date.today().isoformat(), "bbox": [w, s, e, n], "features": feats}
json.dump(gj, open(out, "w"), separators=(",", ":"))
print(len(feats), "features ->", out)
