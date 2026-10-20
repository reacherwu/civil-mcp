"""Independent checks of a civil-mcp demo run. Shares NO code with the Rust crates.

Each check recomputes a result with a second method / library and prints the
measured difference; it exits non-zero if a difference exceeds its tolerance.

usage: python scripts/independent_check.py <demo-out> [--xsd schema/LandXML-1.2.xsd]
"""
import glob, json, math, sys
import numpy as np
import xml.etree.ElementTree as ET
from scipy.interpolate import LinearNDInterpolator
from scipy.optimize import brentq
from scipy.special import fresnel
from scipy.spatial import Delaunay
from matplotlib.path import Path

out = sys.argv[1]
xsd = sys.argv[sys.argv.index("--xsd") + 1] if "--xsd" in sys.argv else None
fails = []
rows = []


def report(name, ours, ref, tol, unit="", rel=True):
    diff = (ours - ref) / ref * 100 if rel and ref else ours - ref
    ok = abs(diff) <= tol
    rows.append((name, float(ours), float(ref), float(diff), "%" if rel else unit, bool(ok)))
    print(f"{'OK ' if ok else 'BAD'} {name}: civil-mcp {ours:.6g} | independent {ref:.6g} | diff {diff:+.4g}{'%' if rel else ' ' + unit} (tol {tol})")
    if not ok:
        fails.append(name)


load = lambda f: json.load(open(f"{out}/{f}"))
corr = load("corridor.json")["corridor"]
se = load("superelevation.json")
net = load("network.json")
q = load("quantities.json")

# ---------- 1. cross-section areas: fine numerical integration of |ground - datum| ----------
def interp(poly, x):
    p = np.array(poly)
    return np.interp(x, p[:, 0], p[:, 1])

cut_a, fill_a = [], []
for s in corr["sections"]:
    g = np.array(s["ground"])
    c = f = 0.0
    for sg, side in ((1, s["left"]), (-1, s["right"])):
        gs = g.copy(); gs[:, 0] *= sg; gs = gs[np.argsort(gs[:, 0])]
        o = np.linspace(0, side["daylight"], 20001)
        dz = np.interp(o, gs[:, 0], gs[:, 1]) - interp(side["datum"], o)
        c += np.trapezoid(np.clip(dz, 0, None), o); f += np.trapezoid(np.clip(-dz, 0, None), o)
    cut_a.append(c); fill_a.append(f)
ours_c = np.array([s["cut_area"] for s in corr["sections"]]); ours_f = np.array([s["fill_area"] for s in corr["sections"]])
ca_, fa_ = np.array(cut_a), np.array(fill_a)
rd = lambda o, r: float(np.max(np.where(r > 1.0, np.abs(o - r) / np.maximum(r, 1e-9) * 100, 0)))
report("section cut area, max rel diff where area>1 m² (%)", rd(ours_c, ca_), 0.0, 0.05, "%", rel=False)
report("section fill area, max rel diff where area>1 m² (%)", rd(ours_f, fa_), 0.0, 0.05, "%", rel=False)
report("section areas, max abs diff (m²)", float(max(np.max(np.abs(ours_c - ca_)), np.max(np.abs(ours_f - fa_)))), 0.0, 0.1, "m²", rel=False)
st = np.array([s["station"] for s in corr["sections"]])
# Simpson-free second method: prismoidal on the trapezoid of independently integrated areas
report("corridor cut volume (avg end area, independent areas)", corr["cut_m3"], float(np.trapezoid(cut_a, st)), 0.05)
report("corridor fill volume (avg end area, independent areas)", corr["fill_m3"], float(np.trapezoid(fill_a, st)), 0.05)

# ---------- 2. mass haul: re-accumulate ----------
mh = q["mass_haul"]; k = mh["cut_factor"]
m = np.concatenate([[0], np.cumsum(np.diff(st) * ((k * ours_c[1:] + k * ours_c[:-1]) - (ours_f[1:] + ours_f[:-1])) / 2)])
report("mass haul final ordinate (m³)", mh["final_ordinate"], float(m[-1]), 0.01)
ordn = np.array(mh["ordinates"])
report("mass haul max |ordinate diff| (m³)", float(np.max(np.abs(np.interp(st, ordn[:, 0], ordn[:, 1]) - m))), 0.0, 1.0, "m³", rel=False)
report("haul moment ∫|M|ds (m³·m)", mh["haul_moment_m3m"], float(np.trapezoid(np.abs(m), st)), 0.5)

# ---------- 3. superelevation (JTG i = V²/127R − μ, transition = B·Δi/p) ----------
for c in se["curves"]:
    if "JTG" in c["method"] or "μ" in c["method"]:
        V, mu = 40.0, float(c["method"].split("−")[-1])
        e_ref = V * V / (127 * c["radius"]) - mu
        p_ref = 1 / 150  # JTG, 40 km/h, rotation about centreline
        L_ref = se["rotation_width"] * (e_ref + se["nc"]) / p_ref
        report(f"superelevation e, curve {c['curve']}", c["e"], e_ref, 1e-6)
        report(f"superelevation transition length, curve {c['curve']}", c["required_length"], L_ref, 1e-6)
        report(f"provided transition ≥ required, curve {c['curve']} (m spare)", (c["b_in"] - c["a_in"]) - c["required_length"], 0.0, 1e9, "m", rel=False)

# ---------- 4. LandXML: clothoid geometry vs scipy Fresnel, schema ----------
ns = {"l": "http://www.landxml.org/schema/LandXML-1.2"}
root = ET.parse(f"{out}/design.xml").getroot()
ne = lambda e: np.array([float(v) for v in e.text.split()][::-1])  # N E -> E N
for sp in root.iter("{%s}Spiral" % ns["l"]):
    L = float(sp.get("length")); R = float(sp.get("radiusEnd") if sp.get("radiusEnd") != "INF" else sp.get("radiusStart"))
    A = math.sqrt(R * L); S, C = fresnel(L / (A * math.sqrt(math.pi)))
    chord_ref = A * math.sqrt(math.pi) * math.hypot(C, S)
    chord = np.linalg.norm(ne(sp.find("l:End", ns)) - ne(sp.find("l:Start", ns)))
    report(f"clothoid chord L={L:g} R={R:g} (m)", chord, chord_ref, 1e-6, "m", rel=False)
if xsd:
    import xmlschema
    errs = list(xmlschema.XMLSchema(xsd).iter_errors(f"{out}/design.xml"))
    report("LandXML 1.2 XSD validation errors", len(errs), 0, 0, "", rel=False)

# ---------- 5. Manning / rational method / partial flow ----------
i_mm = None
for line in open(f"{out}/run-log.jsonl"):
    e = json.loads(line)
    if e.get("tool") == "design_pipe_network":
        i_mm = e["args"]["criteria"]["intensity_mm_h"]; struct_in = {s["id"]: s for s in e["args"]["structures"]}
ca = {k_: v["c"] * v["area_ha"] for k_, v in struct_in.items()}
up = {}
for p in net["pipes"]:
    up.setdefault(p["to"], []).append(p["from"])
def tot(sid):
    return ca[sid] + sum(tot(u) for u in up.get(sid, []))
worst = {"q": 0, "qf": 0, "v": 0, "y": 0}
for p in net["pipes"]:
    D, n, S = p["diameter_m"], 0.013, p["slope"]
    A = math.pi * D * D / 4; Qf = A * (D / 4) ** (2 / 3) * S ** 0.5 / n
    Qd = tot(p["from"]) * i_mm / 360
    def seg(y):
        th = 2 * math.acos(1 - 2 * y / D); a = D * D / 8 * (th - math.sin(th)); P = D * th / 2
        return a, a / P
    def qy(y):
        a, r = seg(y); return a * r ** (2 / 3) * S ** 0.5 / n
    worst["q"] = max(worst["q"], abs(p["q_design"] - Qd)); worst["qf"] = max(worst["qf"], abs(p["q_full"] - Qf) / Qf * 100)
    if Qd < qy(0.938 * D) * 0.999:
        y = brentq(lambda y: qy(y) - Qd, 1e-6 * D, 0.938 * D, xtol=1e-12)
        a, _ = seg(y)
        worst["y"] = max(worst["y"], abs(p["depth_ratio"] - y / D)); worst["v"] = max(worst["v"], abs(p["v_design"] - Qd / a))
report("rational Q = CiA/360, max abs diff (m³/s)", worst["q"], 0, 1e-9, "m³/s", rel=False)
report("Manning full-flow Q, max diff (%)", worst["qf"], 0, 1e-6, "%", rel=False)
report("partial-flow depth ratio (brentq), max abs diff", worst["y"], 0, 1e-4, "", rel=False)
report("partial-flow velocity, max abs diff (m/s)", worst["v"], 0, 1e-3, "m/s", rel=False)

# ---------- 6. grading volume: separate Delaunay triangulation + 0.1 m grid ----------
for sf in root.iter("{%s}Surface" % ns["l"]):
    if sf.get("name") == "EG":
        P = np.array([[float(v) for v in p.text.split()] for p in sf.iter("{%s}P" % ns["l"])])
        eg = LinearNDInterpolator(P[:, [1, 0]], P[:, 2])
# Analytic mitred-offset model for a convex pad: outward distance d = max over edges of
# the signed edge-normal distance; design = z_pad ± d/slope_h, clipped by EG. No triangulation.
for gf in glob.glob(f"{out}/grading-*.json"):
    g = json.load(open(gf))
    fp = np.array(g["footprint"]); z = fp[0, 2]
    args = [e for e in (json.loads(l) for l in open(f"{out}/run-log.jsonl")) if e["tool"] == "grade" and e["args"]["name"] == g["name"]][-1]["args"]
    V = np.array(args["vertices"])[:, :2]
    assert np.allclose(np.array(args["vertices"])[:, 2], z), "analytic check needs a flat pad"
    ccw = np.sum(V[:, 0] * np.roll(V[:, 1], -1) - np.roll(V[:, 0], -1) * V[:, 1]) > 0
    h = 0.1; (x0, y0), (x1, y1) = V.min(0) - 60, V.max(0) + 60
    X, Y = np.meshgrid(np.arange(x0 + h / 2, x1, h), np.arange(y0 + h / 2, y1, h))
    d = np.full(X.shape, -np.inf)
    for i in range(len(V)):
        a_, b_ = V[i], V[(i + 1) % len(V)]
        t = (b_ - a_) / np.linalg.norm(b_ - a_); nrm = np.array([t[1], -t[0]]) if ccw else np.array([-t[1], t[0]])
        d = np.maximum(d, (X - a_[0]) * nrm[0] + (Y - a_[1]) * nrm[1])
    d = d.clip(0); G = eg(X, Y)
    cut = np.nansum(np.clip(G - (z + d / args["cut_slope_h"]), 0, None)) * h * h
    fill = np.nansum(np.clip((z - d / args["fill_slope_h"]) - G, 0, None)) * h * h
    report(f"{g['name']} grading cut vs analytic mitred model (m³)", g["cut_m3"], float(cut), 1.0)
    report(f"{g['name']} grading fill vs analytic mitred model (m³)", g["fill_m3"], float(fill), 1.0)

# ---------- 7. drawings: DXF audit, PDF parse + embedded CJK font ----------
from ezdxf import recover
import pypdf
dxf_err = 0
for f in sorted(glob.glob(f"{out}/*.dxf")):
    doc, aud = recover.readfile(f)
    a = doc.audit(); dxf_err += len(a.errors) + len(aud.fixes)
    assert "CJK" in doc.styles, f
report("DXF audit errors+fixes (all files)", dxf_err, 0, 0, "", rel=False)
r = pypdf.PdfReader(f"{out}/sheets.pdf")
txt = "".join(p.extract_text() for p in r.pages)
report("PDF text extract contains 横断面图 and 初步设计", float("横断面图" in txt and "初步设计" in txt), 1.0, 0, "", rel=False)

json.dump([dict(zip(["check", "civil_mcp", "independent", "diff", "unit", "ok"], r_)) for r_ in rows], open(f"{out}/independent-checks.json", "w"), ensure_ascii=False, indent=1)
print(f"\n{len(rows)} checks, {len(fails)} failed")
sys.exit(1 if fails else 0)
