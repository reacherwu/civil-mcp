#!/usr/bin/env python3
"""Independent check of a civil-mcp design package v1 folder (<out>/design-package).

  * every JSON file and the manifest validated with the Python `jsonschema` package
    (Draft 2020-12) against schema/design-package/v1;
  * sizes and SHA-256 recomputed and compared with the manifest;
  * package LandXML: XSD-valid (with --xsd) and its linear unit equal to the manifest;
  * content cross-checked against the run's own SI outputs and the package LandXML:
    alignment element ends chain (end of one = start of next), arc/spiral ends match the
    LandXML Curve/Spiral End points, superelevation stations vs superelevation.json,
    design PVIs vs LandXML PVI/ParaCurve, pipe inverts vs network-*.json, corridor
    cut/fill vs corridor.json, parcel count vs LandXML parcels, sheet count vs registers.

Usage: package_check.py <out-dir> [--xsd schema/LandXML-1.2.xsd]
Writes <out>/design-package-check.json; exit 1 on any failure.
"""
import csv, glob, hashlib, json, os, sys
import xml.etree.ElementTree as ET
import jsonschema

out = sys.argv[1]
xsd = sys.argv[sys.argv.index("--xsd") + 1] if "--xsd" in sys.argv else None
here = os.path.dirname(os.path.abspath(__file__))
sdir = os.path.join(here, "..", "schema", "design-package", "v1")
pk = os.path.join(out, "design-package")
rows = []


def report(name, value, tol=0.0, unit=""):
    ok = abs(value) <= tol
    rows.append({"check": name, "value": value, "tol": tol, "unit": unit, "ok": ok})
    print(f"{'OK ' if ok else 'BAD'} {name}: {value:.3g} {unit} (tol {tol:g})")


def schema(n):
    return json.load(open(os.path.join(sdir, n)))


man = json.load(open(os.path.join(pk, "manifest.json")))
V = jsonschema.Draft202012Validator
errs = list(V(schema("manifest.schema.json")).iter_errors(man))
for e in errs: print("  ", e.message)
report("manifest.json schema errors (jsonschema Draft 2020-12)", len(errs))
D = {}
bad_hash = n_err = 0
for f in man["files"]:
    b = open(os.path.join(pk, f["path"]), "rb").read()
    bad_hash += hashlib.sha256(b).hexdigest() != f["sha256"] or len(b) != f["bytes"]
    if f["path"].endswith(".json"):
        D[f["role"]] = json.loads(b)
        es = list(V(schema(f["schema"])).iter_errors(D[f["role"]]))
        for e in es[:5]: print("  ", f["path"], e.json_path, e.message)
        n_err += len(es)
report(f"{len(man['files'])} files: size or SHA-256 differs from manifest (count)", bad_hash)
report(f"{len(D)} JSON files: schema errors (jsonschema Draft 2020-12)", n_err)
roles = {"alignments", "profiles", "superelevation", "assemblies", "corridors", "surfaces", "pipes", "parcels", "quantities", "sheets", "landxml"}
report("roles missing from manifest (count)", len(roles - {f["role"] for f in man["files"]}))

# LandXML of the package
L = "{http://www.landxml.org/schema/LandXML-1.2}"
lx = os.path.join(pk, "design.xml")
root = ET.parse(lx).getroot()
u = root.find(L + "Units")[0]
report("LandXML linearUnit = manifest landxml_linear_unit (mismatch)", float(u.get("linearUnit") != man["units"]["landxml_linear_unit"]))
LF = {"meter": 1.0, "foot": 0.3048, "USSurveyFoot": 1200 / 3937}[u.get("linearUnit")]
if xsd:
    import xmlschema
    report("package LandXML XSD validation errors", len(list(xmlschema.XMLSchema(xsd).iter_errors(lx))))
cs = root.find(L + "CoordinateSystem")
report("LandXML CoordinateSystem epsgCode = manifest crs.epsg (mismatch)", float((cs.get("epsgCode") if cs is not None else None) != (str(man["crs"]["epsg"]) if man["crs"]["epsg"] else None)))

# alignment
A = D["alignments"]["alignments"][0]
E = A["elements"]
gap = max(((a["end"][0] - b["start"][0]) ** 2 + (a["end"][1] - b["start"][1]) ** 2) ** 0.5 for a, b in zip(E, E[1:]))
report("alignment element chain: max gap end->next start (m)", gap, 1e-6, "m")
sgap = max(abs(a["sta_start"] + a["length"] - b["sta_start"]) for a, b in zip(E, E[1:]))
report("alignment stationing: max |sta_start + length - next sta_start| (m)", sgap, 1e-6, "m")
dg = max((abs(((a["direction_end_deg"] - b["direction_start_deg"] + 180) % 360) - 180) for a, b in zip(E, E[1:])))
report("alignment tangency: max direction change at element joins (deg)", dg, 1e-6, "deg")
xg = [e for e in root.find(f"{L}Alignments/{L}Alignment/{L}CoordGeom")]
dmax = 0.0
for e, x in zip(E, xg):
    n_, e_ = [float(v) * LF for v in x.find(L + "End").text.split()]
    dmax = max(dmax, ((e["end"][0] - e_) ** 2 + (e["end"][1] - n_) ** 2) ** 0.5)
report(f"alignment element ends vs LandXML End ({len(E)} elements) max (m)", dmax if len(E) == len(xg) else 1e9, 1e-3, "m")
# profile
P = [p for p in D["profiles"]["profiles"] if p["kind"] == "design"][0]
pv = [[float(v) * LF for v in el.text.split()] for el in root.find(f"{L}Alignments/{L}Alignment/{L}Profile/{L}ProfAlign")]
report("design PVIs vs LandXML PVI/ParaCurve max (m)", max(max(abs(a["station"] - b[0]), abs(a["elevation"] - b[1])) for a, b in zip(P["pvis"], pv)) if len(pv) == len(P["pvis"]) else 1e9, 1e-3, "m")
# superelevation
SE = json.load(open(f"{out}/superelevation.json"))
sc = [c for c in SE["curves"] if c["e"] > 0]
pc = D["superelevation"]["curves"]
report("superelevation curves vs superelevation.json max (m or m/m)", max(max(abs(a["runout_start"] - b["a_in"]), abs(a["full_super_start"] - b["b_in"]), abs(a["full_super_end"] - b["b_out"]), abs(a["runout_end"] - b["a_out"]), abs(a["e_design"] - b["e"])) for a, b in zip(pc, sc)) if len(pc) == len(sc) else 1e9, 1e-9)
# corridor
C = json.load(open(f"{out}/corridor.json"))["corridor"]
r = D["corridors"]["corridors"][0]
report("corridor cut+fill vs corridor.json (m³)", abs(r["results"]["cut_m3"] - C["cut_m3"]) + abs(r["results"]["fill_m3"] - C["fill_m3"]), 1e-6, "m³")
reg = r["regions"][0]
report("corridor region station range vs corridor sections (m)", abs(reg["sta_start"] - C["sections"][0]["station"]) + abs(reg["sta_end"] - C["sections"][-1]["station"]), 1e-6, "m")
# pipes
nets = {json.load(open(f))["name"]: json.load(open(f)) for f in glob.glob(f"{out}/network-*.json")}
d = 0.0; n = 0
for N in D["pipes"]["networks"]:
    ref = {p["id"]: p for p in nets[N["name"]]["pipes"]}
    for p in N["pipes"]:
        q = ref[p["id"]]; n += 1
        d = max(d, abs(p["invert_up"] - q["invert_up"]), abs(p["invert_dn"] - q["invert_dn"]), abs(p["diameter"] - q["diameter_m"]), abs(p["length"] - q["length"]))
report(f"pipes ({n}) inverts/diameter/length vs network-*.json max (m)", d if n == sum(len(v["pipes"]) for v in nets.values()) else 1e9, 1e-9, "m")
# parcels
npx = len(root.findall(f"{L}Parcels/{L}Parcel"))
report("parcels in package vs LandXML top-level Parcel elements (count diff)", float(len(D["parcels"]["parcels"]) - npx))
row = D["parcels"]["row"]
report("ROW polygon present with positive area (0 = yes)", 0.0 if row and row["area"] > 0 else 1.0)
# sheets
nreg = sum(max(0, len(list(csv.reader(open(f)))) - 1) for f in [f"{out}/sheet-register.csv", f"{out}/construction-register.csv"] if os.path.exists(f))
report("sheets list vs sheet + construction registers (count diff)", float(len(D["sheets"]["sheets"]) - nreg))
# quantities
report("quantities: BoQ rows present (0 = yes)", 0.0 if D["quantities"]["boq"] else 1.0)

fails = [x for x in rows if not x["ok"]]
json.dump({"checks": rows, "failed": len(fails)}, open(f"{out}/design-package-check.json", "w"), indent=1)
print(f"\n{len(rows)} package checks, {len(fails)} failed")
sys.exit(1 if fails else 0)
