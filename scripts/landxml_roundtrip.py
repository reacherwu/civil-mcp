#!/usr/bin/env python3
"""Round-trip check of a civil-mcp LandXML 1.2 export with an independent parser.

Reads <out>/design.xml with ElementTree only (no civil-mcp code), converts the file's
linear unit back to metres from its <Units> element, evaluates the geometry itself and
compares it with the run's SI JSON outputs (corridor.json, superelevation.json,
network-*.json):

  * alignment: own evaluator of Line / Curve (arc about Center) / clothoid Spiral
    (curvature linear in length, integrated numerically) at every corridor section
    station, compared with the section x, y;
  * profile: own PVI / ParaCurve evaluator vs section cl_elev;
  * surfaces: EG and Corridor FG TINs sampled (barycentric) at the section points;
    FG at the centreline vs cl_elev; FG breakline CL vertices vs cl_elev;
  * superelevation: FullSuperSta / RunoffSta / FullSuperelev vs superelevation.json;
  * pipes: Struct Invert elevations, pipe diameters and lengths vs network-*.json.

Usage: landxml_roundtrip.py <out-dir> [--xsd schema/LandXML-1.2.xsd]
Writes <out>/landxml-roundtrip.json and exits 1 if any comparison is outside tolerance.
"""
import glob, json, math, os, sys
import xml.etree.ElementTree as ET
import numpy as np

out = sys.argv[1]
xsd = sys.argv[sys.argv.index("--xsd") + 1] if "--xsd" in sys.argv else None
L = "{http://www.landxml.org/schema/LandXML-1.2}"
root = ET.parse(f"{out}/design.xml").getroot()
rows = []


def report(name, value, tol, unit=""):
    ok = bool(np.isfinite(value) and abs(value) <= tol)
    rows.append({"check": name, "value": float(value), "tol": tol, "unit": unit, "ok": ok})
    print(f"{'OK ' if ok else 'BAD'} {name}: {value:.3g} {unit} (tol {tol:g})")


units = root.find(L + "Units")[0]
lin = units.get("linearUnit")
LF = {"meter": 1.0, "foot": 0.3048, "USSurveyFoot": 1200 / 3937}[lin]
DF = {"millimeter": 1e-3, "inch": 0.0254, "meter": 1.0, "foot": 0.3048}[units.get("diameterUnit", "millimeter")]
print(f"file {out}/design.xml: {units.tag[len(L):]} linearUnit={lin} ({LF} m), diameterUnit={units.get('diameterUnit')}")
report("Units element matches elevationUnit (feet for imperial, meter for metric)",
       0.0 if (units.get("elevationUnit") == ("meter" if LF == 1 else "feet")) else 1.0, 0)

if xsd:
    import xmlschema
    report("XSD validation errors (LandXML-1.2.xsd)", len(list(xmlschema.XMLSchema(xsd).iter_errors(f"{out}/design.xml"))), 0)

en = lambda e: np.array([float(v) for v in e.text.split()][:2][::-1]) * LF  # "N E" -> (E, N) metres

# ---------- alignment ----------
al = root.find(f"{L}Alignments/{L}Alignment")
segs = []
for el in al.find(L + "CoordGeom"):
    tag = el.tag[len(L):]
    s0 = float(el.get("staStart")) * LF
    ln = float(el.get("length")) * LF
    a, b = en(el.find(L + "Start")), en(el.find(L + "End"))
    if tag == "Line":
        segs.append((s0, ln, lambda s, a=a, b=b, ln=ln: a + (b - a) * s / ln))
    elif tag == "Curve":
        c = en(el.find(L + "Center")); R = float(el.get("radius")) * LF
        sg = 1.0 if el.get("rot") == "ccw" else -1.0
        def f(s, a=a, c=c, R=R, sg=sg):
            t = sg * s / R; v = a - c
            return c + np.array([v[0] * math.cos(t) - v[1] * math.sin(t), v[0] * math.sin(t) + v[1] * math.cos(t)])
        segs.append((s0, ln, f))
    elif tag == "Spiral":
        pi = en(el.find(L + "PI"))
        th0 = math.atan2(*(pi - a)[::-1])  # tangent at start points at the PI
        r0, r1 = el.get("radiusStart"), el.get("radiusEnd")
        k0 = 0.0 if r0 == "INF" else 1 / (float(r0) * LF)
        k1 = 0.0 if r1 == "INF" else 1 / (float(r1) * LF)
        sg = 1.0 if el.get("rot") == "ccw" else -1.0
        def f(s, a=a, th0=th0, k0=k0, k1=k1, ln=ln, sg=sg):
            n = max(8, int(s / 0.05) // 2 * 2 + 2); u = np.linspace(0, s, n + 1)
            th = th0 + sg * (k0 * u + (k1 - k0) * u * u / (2 * ln))
            w = np.ones(n + 1); w[1:-1:2] = 4; w[2:-1:2] = 2; h = s / n / 3  # Simpson
            return a + np.array([h * (w * np.cos(th)).sum(), h * (w * np.sin(th)).sum()])
        segs.append((s0, ln, f))
        # the evaluated end must land on the written End
        report(f"spiral at {s0:.1f} m: integrated end vs End element (m)", float(np.linalg.norm(f(ln) - b)), 1e-3, "m")


def xy(s):
    for s0, ln, f in segs:
        if s <= s0 + ln + 1e-9:
            return f(min(max(s - s0, 0.0), ln))
    s0, ln, f = segs[-1]
    return f(ln)


C = json.load(open(f"{out}/corridor.json"))
secs = C["corridor"]["sections"]
d = [np.linalg.norm(xy(s["station"]) - np.array([s["x"], s["y"]])) for s in secs]
report(f"alignment E,N at {len(secs)} section stations vs corridor.json, max (m)", max(d), 1e-3, "m")
report("alignment length attribute vs last section station (m)", float(al.get("length")) * LF - secs[-1]["station"], 1e-3, "m")

# ---------- profile ----------
pa = al.find(f"{L}Profile/{L}ProfAlign")
V = []
for el in pa:
    st, z = [float(v) * LF for v in el.text.split()]
    V.append((st, z, float(el.get("length", 0)) * LF if el.tag == L + "ParaCurve" else 0.0))


def zp(s):
    for i in range(1, len(V) - 1):
        (s0, z0, _), (s1, z1, lc), (s2, z2, _) = V[i - 1], V[i], V[i + 1]
        g1, g2 = (z1 - z0) / (s1 - s0), (z2 - z1) / (s2 - s1)
        if lc > 0 and abs(s - s1) <= lc / 2:
            x = s - (s1 - lc / 2)
            return z1 - g1 * lc / 2 + g1 * x + (g2 - g1) * x * x / (2 * lc)
    for i in range(1, len(V)):
        if s <= V[i][0] + 1e-9 or i == len(V) - 1:
            (s0, z0, _), (s1, z1, _) = V[i - 1], V[i]
            return z0 + (z1 - z0) * (s - s0) / (s1 - s0)


report("profile elevation vs corridor cl_elev, max (m)", max(abs(zp(s["station"]) - s["cl_elev"]) for s in secs), 1e-3, "m")
ps = al.find(f"{L}Profile/{L}ProfSurf")
report("ProfSurf (EG) present with samples", 0.0 if ps is not None and len(ps.find(L + "PntList2D").text.split()) >= 4 else 1.0, 0)

# ---------- surfaces ----------
def tin(name):
    sf = [s for s in root.iter(L + "Surface") if s.get("name") == name][0]
    ids, P = {}, []
    for i, p in enumerate(sf.iter(L + "P")):
        ids[p.get("id")] = i; P.append([float(v) for v in p.text.split()])
    P = np.array(P) * LF
    F = np.array([[ids[v] for v in f.text.split()] for f in sf.iter(L + "F") if f.get("i") != "1"])
    return sf, P[:, [1, 0, 2]], F


def sample(P, F, q):
    A, B, Cc = P[F[:, 0]], P[F[:, 1]], P[F[:, 2]]
    det = (B[:, 1] - Cc[:, 1]) * (A[:, 0] - Cc[:, 0]) + (Cc[:, 0] - B[:, 0]) * (A[:, 1] - Cc[:, 1])
    res = []
    for x, y in q:
        l1 = ((B[:, 1] - Cc[:, 1]) * (x - Cc[:, 0]) + (Cc[:, 0] - B[:, 0]) * (y - Cc[:, 1])) / det
        l2 = ((Cc[:, 1] - A[:, 1]) * (x - Cc[:, 0]) + (A[:, 0] - Cc[:, 0]) * (y - Cc[:, 1])) / det
        l3 = 1 - l1 - l2
        k = np.flatnonzero((l1 >= -1e-6) & (l2 >= -1e-6) & (l3 >= -1e-6))
        res.append(np.nan if len(k) == 0 else l1[k[0]] * A[k[0], 2] + l2[k[0]] * B[k[0], 2] + l3[k[0]] * Cc[k[0], 2])
    return np.array(res)


q = [(s["x"], s["y"]) for s in secs]
_, Pe, Fe = tin("EG")
ze = sample(Pe, Fe, q)
gcl = np.array([s["ground_cl"] for s in secs])
m = ~np.isnan(ze)
report(f"EG TIN at {int(m.sum())} section points vs corridor ground_cl, max (m)", float(np.abs(ze[m] - gcl[m]).max()), 1e-3, "m")
sf, Pf, Ff = tin("Corridor FG")
zf = sample(Pf, Ff, q)
m = ~np.isnan(zf)
report(f"FG TIN at {int(m.sum())} centreline points vs cl_elev, max (m)", float(np.abs(zf[m] - np.array([s['cl_elev'] for s in secs])[m]).max()), 1e-3, "m")
report("FG TIN covers every centreline point (missing count)", float((~m).sum()), 0)
bl = {b.get("name"): np.array(b.find(L + "PntList3D").text.split(), float).reshape(-1, 3) * LF for b in sf.iter(L + "Breakline")}
report("FG breaklines present (CL, EP-L, EP-R, DAYLIGHT-L, DAYLIGHT-R) missing", float(len({"CL", "EP-L", "EP-R", "DAYLIGHT-L", "DAYLIGHT-R"} - set(bl))), 0)
cl = bl["CL"]
report("FG breakline CL vertices vs section x, y, cl_elev, max (m)",
       float(max(np.abs(cl[:, [1, 0, 2]] - np.array([[s["x"], s["y"], s["cl_elev"]] for s in secs])).max(), 0)) if len(cl) == len(secs) else 1e9, 1e-3, "m")
bd = sf.find(f".//{L}Boundary")
report("FG outer boundary present and closed (m gap)", float(np.linalg.norm(np.diff(np.array(bd.find(L + "PntList2D").text.split(), float).reshape(-1, 2)[[0, -1]] * LF, axis=0))) if bd is not None and bd.get("bndType") == "outer" else 1e9, 1e-6, "m")

# ---------- superelevation ----------
SE = json.load(open(f"{out}/superelevation.json"))
curves = [c for c in SE["curves"] if c["e"] > 0]
xse = al.findall(L + "Superelevation")
report("Superelevation elements = superelevated curves (count diff)", float(len(xse) - len(curves)), 0)
dmax = de = 0.0
for c, x in zip(curves, xse):
    g = lambda t: float(x.find(L + t).text) * LF
    dmax = max(dmax, abs(g("BeginRunoutSta") - c["a_in"]), abs(g("FullSuperSta") - c["b_in"]), abs(g("RunoffSta") - c["b_out"]), abs(g("EndofRunoutSta") - c["a_out"]))
    de = max(de, abs(float(x.find(L + "FullSuperelev").text) - 100 * c["e"]))
report("Superelevation runout/runoff/full-super stations vs superelevation.json, max (m)", dmax, 1e-3, "m")
report("FullSuperelev (percent) vs superelevation.json e, max (%)", de, 1e-4, "%")

# ---------- pipes ----------
nets = {json.load(open(f))["name"]: json.load(open(f)) for f in glob.glob(f"{out}/network-*.json")}
di = dd = dl = 0.0; n = 0
for pn in root.iter(L + "PipeNetwork"):
    N = nets[pn.get("name")]
    inv = {}
    for st in pn.iter(L + "Struct"):
        for iv in st.iter(L + "Invert"):
            inv[(iv.get("refPipe"), iv.get("flowDir"))] = float(iv.get("elev")) * LF
    for p in N["pipes"]:
        x = [e for e in pn.iter(L + "Pipe") if e.get("name") == p["id"]][0]
        di = max(di, abs(inv[(p["id"], "out")] - p["invert_up"]), abs(inv[(p["id"], "in")] - p["invert_dn"]))
        props = {q.get("label"): q.get("value") for q in x.iter(L + "Property")}
        di = max(di, abs(float(props["invertUp"]) * LF - p["invert_up"]), abs(float(props["invertDown"]) * LF - p["invert_dn"]))
        dd = max(dd, abs(float(x.find(L + "CircPipe").get("diameter")) * DF - p["diameter_m"]))
        dl = max(dl, abs(float(x.get("length")) * LF - p["length"]))
        n += props.get("material") == p["material"]
npipes = sum(len(N["pipes"]) for N in nets.values())
report(f"pipe inverts ({npipes} pipes, Struct Invert and Property) max (m)", di, 1e-3, "m")
report("pipe diameters max (m)", dd, 1e-3 if DF == 1e-3 else 0.0254 * 0.005 + 1e-9, "m")
report("pipe lengths max (m)", dl, 1e-3, "m")
report("pipe material property mismatches", float(npipes - n), 0)

fails = [r for r in rows if not r["ok"]]
json.dump({"file": "design.xml", "linear_unit": lin, "checks": rows, "failed": len(fails)}, open(f"{out}/landxml-roundtrip.json", "w"), indent=1)
print(f"\n{len(rows)} round-trip checks, {len(fails)} failed")
sys.exit(1 if fails else 0)
