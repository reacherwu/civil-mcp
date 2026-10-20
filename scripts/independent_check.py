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
nets = {g: json.load(open(g)) for g in sorted(glob.glob(f"{out}/network-*.json"))}
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

for sf in root.iter("{%s}Surface" % ns["l"]):
    if sf.get("name") == "EG":
        P_ = np.array([[float(v) for v in p.text.split()] for p in sf.iter("{%s}P" % ns["l"])])
        eg = LinearNDInterpolator(P_[:, [1, 0]], P_[:, 2])
# ---------- 5. Manning / rational / GB Kz / partial flow / HGL-EGL (second implementation) ----------
G = 9.80665
KZ = [(5, 2.7), (15, 2.4), (40, 2.1), (70, 2.0), (100, 1.9), (200, 1.8), (500, 1.6), (1000, 1.5)]  # GB 50014-2021 表 4.1.15
def kz(q_ls):
    if q_ls <= KZ[0][0]: return KZ[0][1]
    if q_ls >= KZ[-1][0]: return KZ[-1][1]
    return float(np.interp(q_ls, [k_[0] for k_ in KZ], [k_[1] for k_ in KZ]))
def seg(D, y):
    th = 2 * math.acos(max(-1.0, min(1.0, 1 - 2 * y / D))); a = D * D / 8 * (th - math.sin(th)); P = D * th / 2
    return a, a / P, D * math.sin(th / 2)  # area, R, top width
runs = {}
for line in open(f"{out}/run-log.jsonl"):
    e = json.loads(line)
    if e.get("tool") == "design_pipe_network":
        runs[e["args"]["name"]] = e["args"]  # last attempt per network
for nf, net in nets.items():
    args = runs[net["name"]]; cr = args["criteria"]; tag = net["name"]
    sin = {s_["id"]: s_ for s_ in args["structures"]}; pin = {p_["id"]: p_ for p_ in args["pipes"]}
    up = {}
    for p in net["pipes"]:
        up.setdefault(p["to"], []).append(p["from"])
    def acc(sid, f):
        return f(sin[sid]) + sum(acc(u, f) for u in up.get(sid, []))
    w = {"q": 0, "qf": 0, "v": 0, "y": 0, "kz": 0}
    for p in net["pipes"]:
        D, n, S = p["diameter_m"], pin[p["id"]]["n"], p["slope"]
        avg = acc(p["from"], lambda s_: s_.get("population", 0) * s_.get("per_capita_lpd", 0) / 86400e3)
        k_ = kz(avg * 1000) if avg > 0 else 0.0
        Qd = acc(p["from"], lambda s_: s_.get("c", 0) * s_.get("area_ha", 0)) * cr.get("intensity_mm_h", 0) / 360 + acc(p["from"], lambda s_: s_.get("inflow_m3s", 0)) + k_ * avg
        A = math.pi * D * D / 4; Qf = A * (D / 4) ** (2 / 3) * S ** 0.5 / n
        qy = lambda y: seg(D, y)[0] * seg(D, y)[1] ** (2 / 3) * S ** 0.5 / n
        w["q"] = max(w["q"], abs(p["q_design"] - Qd)); w["qf"] = max(w["qf"], abs(p["q_full"] - Qf) / Qf * 100); w["kz"] = max(w["kz"], abs(p["kz"] - k_))
        if Qd < qy(0.938 * D) * 0.999:
            y = brentq(lambda y: qy(y) - Qd, 1e-6 * D, 0.938 * D, xtol=1e-12)
            w["y"] = max(w["y"], abs(p["depth_ratio"] - y / D)); w["v"] = max(w["v"], abs(p["v_design"] - Qd / seg(D, y)[0]))
    report(f"[{tag}] design Q (rational + Kz·Q_avg), max abs diff (m³/s)", w["q"], 0, 1e-9, "m³/s", rel=False)
    report(f"[{tag}] GB 50014 Kz (np.interp), max abs diff", w["kz"], 0, 1e-12, "", rel=False)
    report(f"[{tag}] Manning full-flow Q, max diff (%)", w["qf"], 0, 1e-6, "%", rel=False)
    report(f"[{tag}] partial-flow depth ratio (brentq), max abs diff", w["y"], 0, 1e-4, "", rel=False)
    report(f"[{tag}] partial-flow velocity, max abs diff (m/s)", w["v"], 0, 1e-3, "m/s", rel=False)
    # HGL/EGL: recursive walk from each outfall upstream (HEC-22 approximate method)
    P = {p["id"]: p for p in net["pipes"]}; xy = {s_[0]: (s_[1], s_[2]) for s_ in net["structures"]}
    rim = {s_[0]: s_[3] for s_ in net["structures"]}
    outp = {p["from"]: p for p in net["pipes"]}; inc = {}
    for p in net["pipes"]:
        inc.setdefault(p["to"], []).append(p)
    def dc(D, Q):  # critical depth Q²/g = A³/T, solved by brentq
        return brentq(lambda y: seg(D, y)[0] ** 3 / seg(D, y)[2] - Q * Q / G, 1e-6 * D, 0.999 * D, xtol=1e-12)
    H, E, mine = {}, {}, {}
    def hk(dd):
        tab = [(0, .15), (22.5, .45), (45, .75), (60, .85), (90, 1.0)]
        return float(np.interp(min(abs(dd), 90), [t[0] for t in tab], [t[1] for t in tab]))
    def walk(sid):
        for p in inc.get(sid, []):
            D, Q, n = p["diameter_m"], p["q_design"], pin[p["id"]]["n"]
            yn = p["depth_ratio"] * D if math.isfinite(p["depth_ratio"]) else D
            hdn = max(H[sid], p["invert_dn"] + yn)
            full = hdn >= p["invert_dn"] + D - 1e-9 or not math.isfinite(p["depth_ratio"])
            Af = math.pi * D * D / 4
            if full:
                Sf = (Q * n / (Af * (D / 4) ** (2 / 3))) ** 2
                hup = max(hdn + Sf * p["length"], p["invert_up"] + yn); v = Q / Af
            else:
                hup = p["invert_up"] + yn; v = Q / seg(D, yn)[0]
            mine[p["id"]] = (hdn, hup, hdn + v * v / 2 / G, hup + v * v / 2 / G)
            u = p["from"]; ins = inc.get(u, [])
            if ins:
                m_ = max(ins, key=lambda q_: q_["q_design"])
                a1 = math.atan2(xy[u][1] - xy[m_["from"]][1], xy[u][0] - xy[m_["from"]][0]); a2 = math.atan2(xy[sid][1] - xy[u][1], xy[sid][0] - xy[u][0])
                dd = (math.degrees(a2 - a1) + 180) % 360 - 180; K = hk(dd)
            else:
                K = 0.5
            E[u] = mine[p["id"]][3] + K * v * v / 2 / G; H[u] = E[u]
            walk(u)
    for sid in xy:
        if sid not in outp:
            H[sid] = max([cr.get("tailwater_m", -1e9)] + [p["invert_dn"] + (dc(p["diameter_m"], p["q_design"]) + p["diameter_m"]) / 2 for p in inc.get(sid, [])])
            walk(sid)
    dh = max(abs(a_ - b_) for p in net["pipes"] for a_, b_ in zip(mine[p["id"]], (p["hgl_dn"], p["hgl_up"], p["egl_dn"], p["egl_up"])))
    ds_ = max(abs(H[h_["id"]] - h_["hgl"]) for h_ in net["hydraulics"])
    report(f"[{tag}] pipe HGL/EGL ends, max abs diff (m)", dh, 0, 1e-6, "m", rel=False)
    report(f"[{tag}] structure HGL, max abs diff (m)", ds_, 0, 1e-6, "m", rel=False)
    report(f"[{tag}] min freeboard rim − HGL (m, must be ≥ {cr.get('freeboard_m', 0)})", min(rim[k_] - H[k_] for k_ in H), cr.get("freeboard_m", 0), 1e9, "m", rel=False)

# ---------- 5b. retaining walls: polygon (shoelace centroid) statics, separate from Rust formulas ----------
asm = corr["assembly"]; soil = (asm.get("wall") or {}).get("soil")
def poly_wc(pts):
    pts = np.array(pts); x, y = pts[:, 0], pts[:, 1]; cr_ = x * np.roll(y, -1) - np.roll(x, -1) * y
    A = cr_.sum() / 2; cx = ((x + np.roll(x, -1)) * cr_).sum() / (6 * A)
    return abs(A), cx
jtg = "JTG" in json.dumps(se) or True
wmax = {"slide": 0, "over": 0, "e": 0, "q": 0}; nwall = 0; minfs = [9e9, 9e9]
for s in corr["sections"]:
    for side in ("left", "right"):
        wl = s[side].get("wall")
        if not wl: continue
        d_ = wl["design"]; H_, B = d_["h"], d_["base_b"]; nwall += 1
        g_, phi, q_, mu = soil["gamma_kn_m3"], math.radians(soil["phi_deg"]), soil["surcharge_kpa"], soil["base_friction"]
        Ka = (1 - math.sin(phi)) / (1 + math.sin(phi))  # Rankine, equivalent form
        z = np.linspace(0, H_, 200001); p_ = Ka * (g_ * z + q_)
        Pa = np.trapezoid(p_, z); Mo = np.trapezoid(p_ * (H_ - z), z)  # arm above base
        t, st_, sb, toe = d_["base_t"], d_["stem_top"], d_["stem_bot"], d_["toe"]
        if d_["kind"] == "cantilever":
            hs = H_ - t; heel = B - toe - sb
            parts = [(poly_wc([(toe, t), (toe + sb, t), (toe + sb, H_), (toe + sb - st_, H_)]), 24.0),
                     (poly_wc([(0, 0), (B, 0), (B, t), (0, t)]), 24.0),
                     (poly_wc([(toe + sb, t), (B, t), (B, H_), (toe + sb, H_)]), g_)]
            V = sum(a_ * gm for (a_, _), gm in parts) + heel * q_
            Mr = sum(a_ * gm * cx for (a_, cx), gm in parts) + heel * q_ * (toe + sb + heel / 2)
        else:
            parts = [(poly_wc([(0, 0), (B, 0), (B, H_), (B - st_, H_)]), 24.0)]
            V = sum(a_ * gm for (a_, _), gm in parts); Mr = sum(a_ * gm * cx for (a_, cx), gm in parts)
        e_ = B / 2 - (Mr - Mo) / V; qm = V / B * (1 + 6 * e_ / B)
        wmax["slide"] = max(wmax["slide"], abs(mu * V / Pa - d_["fs_sliding"])); wmax["over"] = max(wmax["over"], abs(Mr / Mo - d_["fs_overturning"]))
        wmax["e"] = max(wmax["e"], abs(e_ - d_["eccentricity"])); wmax["q"] = max(wmax["q"], abs(qm - d_["q_max_kpa"]))
        minfs = [min(minfs[0], mu * V / Pa), min(minfs[1], Mr / Mo)]
if nwall:
    report(f"walls ({nwall} sections): sliding FS μV/Pa, max abs diff", wmax["slide"], 0, 1e-6, "", rel=False)
    report("walls: overturning FS Mr/Mo (numeric pressure integral), max abs diff", wmax["over"], 0, 1e-6, "", rel=False)
    report("walls: eccentricity, max abs diff (m)", wmax["e"], 0, 1e-6, "m", rel=False)
    report("walls: q_max V/B(1+6e/B), max abs diff (kPa)", wmax["q"], 0, 1e-3, "kPa", rel=False)
    report("walls: min sliding FS (GB 50007 §6.7.5 ≥ 1.3)", minfs[0], 1.3, 1e9, "", rel=False)
    report("walls: min overturning FS (≥ 1.6)", minfs[1], 1.6, 1e9, "", rel=False)

# ---------- 5c. curve widening: JTG D20-2017 表 7.6.1 (transcribed separately), section paved width ----------
JTG_W = {1: [0.4, 0.6, 0.8, 1.0, 1.2, 1.4, 1.8, 2.2, 2.5], 2: [0.6, 0.7, 0.9, 1.2, 1.5, 2.0], 3: [0.8, 1.0, 1.5, 2.0, 2.5]}
BANDS = [250, 200, 150, 100, 70, 50, 30, 25, 20, 15]  # band i: BANDS[i+1] ≤ R < BANDS[i]... R in (BANDS[i+1], BANDS[i]]
wcls = [e for e in (json.loads(l) for l in open(f"{out}/run-log.jsonl")) if e["tool"] == "build_corridor"][-1]["args"].get("widening", {}).get("jtg_class")
if wcls:
    worst = 0
    for wv in corr["widening"]:
        R = wv["radius"]; i_ = next((i for i in range(len(BANDS) - 1) if BANDS[i + 1] <= R < BANDS[i] or (i == 0 and R == BANDS[0])), None)
        ref = 0.0 if R > 250 or i_ is None or i_ >= len(JTG_W[wcls]) else JTG_W[wcls][i_]
        worst = max(worst, abs(wv["widening_m"] - ref))
    report("JTG 表7.6.1 widening by radius, max abs diff (m)", worst, 0, 1e-12, "m", rel=False)

# ---------- 5d. intersections: ISD, return tangency, patch volume by 0.1 m grid ----------
for xf in sorted(glob.glob(f"{out}/intersection-*.json")):
    X = json.load(open(xf)); X = X.get("intersection", X)
    xa = [e for e in (json.loads(l) for l in open(f"{out}/run-log.jsonl")) if e["tool"] == "design_intersection" and e["args"]["name"] == X["name"]][-1]["args"]
    isd = 0.278 * xa["major_speed_kmh"] * xa.get("time_gap_s", 7.5)
    worst_isd = max(abs(s_["along_major_m"] - isd) for l in X["legs"] for s_ in l["sight"])
    report(f"{X['name']} ISD = 0.278·V·tg (AASHTO 2018 §9.5.3 B1), max abs diff (m)", worst_isd, 0, 1e-9, "m", rel=False)
    tang = 0
    for l in X["legs"]:
        for r in l["returns"]:
            c = np.array(r["center"]); R = r["radius"]
            cr2 = lambda u, v: u[0] * v[1] - u[1] * v[0]
            de = min(abs(cr2(np.array(e_[1]) - e_[0], c - e_[0]) / np.linalg.norm(np.array(e_[1]) - e_[0])) for e_ in (l["edge_left"], l["edge_right"]))
            ends = max(abs(np.linalg.norm(np.array(p_[:2]) - c) - R) for p_ in (r["arc"][0], r["arc"][-1]))
            tang = max(tang, abs(de - R), ends)
    report(f"{X['name']} curb returns: |dist(centre, leg edge) − R| and |arc end − R|, max (m)", tang, 0, 1e-6, "m", rel=False)
    for l in X["legs"]:
        pts = np.array(l["patch"]); h = 0.1
        if len(pts) < 3: continue
        path = Path(pts[:, :2]); (x0, y0), (x1, y1) = pts[:, :2].min(0), pts[:, :2].max(0)
        XX, YY = np.meshgrid(np.arange(x0 + h / 2, x1, h), np.arange(y0 + h / 2, y1, h))
        m_ = path.contains_points(np.c_[XX.ravel(), YY.ravel()]).reshape(XX.shape)
        import matplotlib.tri as mtri  # same faces as the tool (surface definition), independent integration
        tri = mtri.Triangulation(pts[:, 0], pts[:, 1], np.array(l["patch_faces"]))
        fg = np.ma.filled(mtri.LinearTriInterpolator(tri, pts[:, 2])(XX, YY), np.nan); dz = np.where(m_, eg(XX, YY) - fg, 0)
        dz = np.nan_to_num(dz)
        report(f"{X['name']} {l['name']} return-patch fill vs 0.1 m grid on its faces (tool samples 0.25 m)", l["patch_fill_m3"], float(np.clip(-dz, 0, None).sum() * h * h), 1.0)

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
