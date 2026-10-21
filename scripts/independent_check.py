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
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from si_view import si_dir
out = si_dir(out)
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

# ---------- 3. superelevation (US profile: e = emax·Rmin/R, development = w·(e+nc)/Δ) ----------
LOG = [json.loads(l) for l in open(f"{out}/run-log.jsonl")]
basis = [e for e in LOG if e["tool"] == "set_design_basis"][-1]["args"]["basis"]
V = basis["design_speed_kmh"]
if basis["standard"] in ("US_AASHTO_2018", "CA_TAC_2017", "AU_AUSTROADS", "NZ_AUSTROADS_NZTA"):
    rmin = V * V / (127 * (basis["e_max"] + basis["f_max"]))
    rg = basis.get("max_relative_gradient_pct") or dict([(20, .8), (30, .75), (40, .7), (50, .65), (60, .6), (70, .55), (80, .5), (90, .47), (100, .44), (110, .41), (120, .38), (130, .35)])[int(round(V / 10) * 10)]
    for c in se["curves"]:
        e_ref = min(basis["e_max"], basis["e_max"] * rmin / c["radius"])
        if e_ref <= se["nc"]:
            e_ref = se["nc"] if e_ref > 0 else 0.0
        L_ref = se["rotation_width"] * (e_ref + se["nc"]) / (rg / 100)
        report(f"superelevation e, curve {c['curve']}", c["e"], e_ref, 1e-6)
        report(f"superelevation development length, curve {c['curve']}", c["required_length"], L_ref, 1e-6)
        report(f"provided development ≥ required, curve {c['curve']} (m spare)", (c["b_in"] - c["a_in"]) - c["required_length"], 0.0, 1e9, "m", rel=False)
ssd_ref = 0.278 * V * 2.5 + 0.039 * V * V / 3.4
ssd_log = [e for e in LOG if e["tool"] == "set_design_basis"][-1]["result"]["ssd_m"]
report("SSD = 0.278·V·t + 0.039·V²/a (AASHTO, t 2.5 s, a 3.4 m/s²) (m)", ssd_log, ssd_ref, 1e-9, "m", rel=False)

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

class TriInterp:
    """Linear interpolation on given triangles (bucketed point location, barycentric weights)."""
    def __init__(self, x, y, z, F):
        self.x, self.y, self.z, self.F = x, y, z, F
        X, Y = x[F], y[F]
        self.lo = np.c_[X.min(1), Y.min(1)]; self.hi = np.c_[X.max(1), Y.max(1)]
        self.h = float(np.median(np.c_[X.max(1) - X.min(1), Y.max(1) - Y.min(1)]).max()) or 1.0
        self.o = np.array([x.min(), y.min()]); b = {}
        for t in range(len(F)):
            i0, j0 = ((self.lo[t] - self.o) // self.h).astype(int); i1, j1 = ((self.hi[t] - self.o) // self.h).astype(int)
            for i in range(i0, i1 + 1):
                for j in range(j0, j1 + 1):
                    b.setdefault((i, j), []).append(t)
        self.b = b
    def __call__(self, qx, qy):
        qx = np.asarray(qx, float); qy = np.asarray(qy, float); shp = np.broadcast(qx, qy).shape
        qx = np.broadcast_to(qx, shp).ravel(); qy = np.broadcast_to(qy, shp).ravel()
        out = np.full(qx.size, np.nan)
        keys = np.floor((np.c_[qx, qy] - self.o) / self.h).astype(int)
        order = {}
        for n, k in enumerate(map(tuple, keys)):
            order.setdefault(k, []).append(n)
        for k, idx in order.items():
            tris = self.b.get(k)
            if not tris: continue
            idx = np.array(idx); px, py = qx[idx], qy[idx]
            for t in tris:
                a, bb, c = self.F[t]
                x1, y1, x2, y2, x3, y3 = self.x[a], self.y[a], self.x[bb], self.y[bb], self.x[c], self.y[c]
                det = (y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3)
                if det == 0: continue
                l1 = ((y2 - y3) * (px - x3) + (x3 - x2) * (py - y3)) / det
                l2 = ((y3 - y1) * (px - x3) + (x1 - x3) * (py - y3)) / det
                l3 = 1 - l1 - l2
                m = (l1 >= -1e-12) & (l2 >= -1e-12) & (l3 >= -1e-12) & np.isnan(out[idx])
                out[idx[m]] = l1[m] * self.z[a] + l2[m] * self.z[bb] + l3[m] * self.z[c]
        return out.reshape(shp) if shp else float(out[0])

for sf in root.iter("{%s}Surface" % ns["l"]):
    if sf.get("name") == "EG":
        P_ = np.array([[float(v) for v in p.text.split()] for p in sf.iter("{%s}P" % ns["l"])])
        # interpolate on the surface's own triangles (<Faces>): the EG points are a regular DEM
        # grid, where an independent Delaunay splits each square on an arbitrary diagonal
        F_ = np.array([[int(v) - 1 for v in f.text.split()] for f in sf.iter("{%s}F" % ns["l"])])
        eg = TriInterp(P_[:, 1], P_[:, 0], P_[:, 2], F_)
egF = eg
# ---------- 1. cross-section areas: fine numerical integration of |EG (own triangle lookup) - datum| ----------
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
        # existing ground straight from the EG triangles along the section normal
        nx_, ny_ = -math.sin(s["heading"]), math.cos(s["heading"])
        dz = np.nan_to_num(egF(s["x"] + sg * o * nx_, s["y"] + sg * o * ny_) - interp(side["datum"], o))
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

# ---------- 5. Manning / rational / GB Kz / partial flow / HGL-EGL (second implementation) ----------
G = 9.80665
def peak(method, pop):  # P in thousands
    Pk = pop / 1000.0
    if method == "ten_states": return (18 + math.sqrt(Pk)) / (4 + math.sqrt(Pk))  # Ten States Standards Fig. 1
    if method == "harmon": return 1 + 14 / (4 + math.sqrt(Pk))
    raise ValueError(method)
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
        k_ = peak(cr["local"]["peaking"], acc(p["from"], lambda s_: s_.get("population", 0))) if avg > 0 else 0.0
        Qd = acc(p["from"], lambda s_: s_.get("c", 0) * s_.get("area_ha", 0)) * cr.get("intensity_mm_h", 0) / 360 + acc(p["from"], lambda s_: s_.get("inflow_m3s", 0)) + k_ * avg
        A = math.pi * D * D / 4; Qf = A * (D / 4) ** (2 / 3) * S ** 0.5 / n
        qy = lambda y: seg(D, y)[0] * seg(D, y)[1] ** (2 / 3) * S ** 0.5 / n
        w["q"] = max(w["q"], abs(p["q_design"] - Qd)); w["qf"] = max(w["qf"], abs(p["q_full"] - Qf) / Qf * 100); w["kz"] = max(w["kz"], abs(p["peak_factor"] - k_))
        if Qd < qy(0.938 * D) * 0.999:
            y = brentq(lambda y: qy(y) - Qd, 1e-6 * D, 0.938 * D, xtol=1e-12)
            w["y"] = max(w["y"], abs(p["depth_ratio"] - y / D)); w["v"] = max(w["v"], abs(p["v_design"] - Qd / seg(D, y)[0]))
    report(f"[{tag}] design Q (rational + PF·Q_avg), max abs diff (m³/s)", w["q"], 0, 1e-9, "m³/s", rel=False)
    if cr["system"] == "sanitary":
        report(f"[{tag}] peaking factor ({cr['local']['peaking']}), max abs diff", w["kz"], 0, 1e-12, "", rel=False)
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
            Vw = sum(a_ * gm for (a_, _), gm in parts) + heel * q_
            Mr = sum(a_ * gm * cx for (a_, cx), gm in parts) + heel * q_ * (toe + sb + heel / 2)
        else:
            parts = [(poly_wc([(0, 0), (B, 0), (B, H_), (B - st_, H_)]), 24.0)]
            Vw = sum(a_ * gm for (a_, _), gm in parts); Mr = sum(a_ * gm * cx for (a_, cx), gm in parts)
        e_ = B / 2 - (Mr - Mo) / Vw; qm = Vw / B * (1 + 6 * e_ / B)
        wmax["slide"] = max(wmax["slide"], abs(mu * Vw / Pa - d_["fs_sliding"])); wmax["over"] = max(wmax["over"], abs(Mr / Mo - d_["fs_overturning"]))
        wmax["e"] = max(wmax["e"], abs(e_ - d_["eccentricity"])); wmax["q"] = max(wmax["q"], abs(qm - d_["q_max_kpa"]))
        minfs = [min(minfs[0], mu * Vw / Pa), min(minfs[1], Mr / Mo)]
if nwall:
    report(f"walls ({nwall} sections): sliding FS μV/Pa, max abs diff", wmax["slide"], 0, 1e-6, "", rel=False)
    report("walls: overturning FS Mr/Mo (numeric pressure integral), max abs diff", wmax["over"], 0, 1e-6, "", rel=False)
    report("walls: eccentricity, max abs diff (m)", wmax["e"], 0, 1e-6, "m", rel=False)
    report("walls: q_max V/B(1+6e/B), max abs diff (kPa)", wmax["q"], 0, 1e-3, "kPa", rel=False)
    report("walls: min sliding FS (AASHTO Std Spec 17th §5.5.5 ≥ 1.5)", minfs[0], 1.5, 1e9, "", rel=False)
    report("walls: min overturning FS (≥ 2.0)", minfs[1], 2.0, 1e9, "", rel=False)

# ---------- 5c. curve widening: AASHTO off-tracking formula (re-derived), N lanes ----------
wargs = [e for e in LOG if e["tool"] == "build_corridor"][-1]["args"].get("widening") or {}
if wargs.get("method") == "AASHTO":
    veh = wargs["vehicle"]; lanes = len(asm["lanes_m"]) + len((asm.get("right") or {}).get("lanes_m") or asm["lanes_m"])
    lw = sum(asm["lanes_m"]) / len(asm["lanes_m"]); wn = lanes * lw; C = 0.6 if wn <= 6.0 else (0.75 if wn <= 6.6 else 0.9)
    worst = 0
    for wv in corr["widening"]:
        R = wv["radius"]; U = veh["track_u_m"] + R - math.sqrt(R * R - sum(l * l for l in veh["wheelbases_m"]))
        A_ = veh["front_overhang_m"]; L1 = veh["wheelbases_m"][0]; FA = math.sqrt(R * R + A_ * (2 * L1 + A_)) - R
        wc = lanes * (U + C) + (lanes - 1) * FA + 0.1 * V / math.sqrt(R)
        worst = max(worst, abs(wv["widening_m"] - max(0.0, wc - wn)))
    report("AASHTO §3.3.10 widening w_c − w_n, max abs diff (m)", worst, 0, 1e-9, "m", rel=False)

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
        fg = TriInterp(pts[:, 0], pts[:, 1], pts[:, 2], np.array(l["patch_faces"]))(XX, YY); dz = np.where(~np.isnan(fg), eg(XX, YY) - fg, 0)  # region = union of the patch faces
        dz = np.nan_to_num(dz)
        report(f"{X['name']} {l['name']} return-patch fill vs 0.1 m grid on its faces (tool samples 0.25 m)", l["patch_fill_m3"], float(np.clip(-dz, 0, None).sum() * h * h), 1.0)

# ---------- 6. grading volume: separate Delaunay triangulation + 0.1 m grid ----------
for sf in root.iter("{%s}Surface" % ns["l"]):
    if sf.get("name") == "EG":
        P = np.array([[float(v) for v in p.text.split()] for p in sf.iter("{%s}P" % ns["l"])])
        egD = LinearNDInterpolator(P[:, [1, 0]], P[:, 2])
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
    d = d.clip(0); G = egD(X, Y)
    cut = np.nansum(np.clip(G - (z + d / args["cut_slope_h"]), 0, None)) * h * h
    fill = np.nansum(np.clip((z - d / args["fill_slope_h"]) - G, 0, None)) * h * h
    report(f"{g['name']} grading cut vs analytic mitred model (m³)", g["cut_m3"], float(cut), 1.0)
    report(f"{g['name']} grading fill vs analytic mitred model (m³)", g["fill_m3"], float(fill), 1.0)

# ---------- 7. drawings: DXF audit, English text, Latin code page, PDF parse ----------
from ezdxf import recover
import pypdf, re
dxf_err = 0
for f in sorted(glob.glob(f"{out}/*.dxf")):
    doc, aud = recover.readfile(f)
    a = doc.audit(); dxf_err += len(a.errors) + len(aud.fixes)
    if "SANS" not in doc.styles or doc.header.get("$DWGCODEPAGE") != "ANSI_1252":
        dxf_err += 1; print("BAD style/codepage", f)
    want = 6 if f.endswith("model.dxf") else 4  # model in metres, sheets in millimetres
    if doc.header.get("$INSUNITS") != want:
        dxf_err += 1; print("BAD $INSUNITS", f, doc.header.get("$INSUNITS"))
report("DXF audit errors+fixes+style/codepage/units problems (all files)", dxf_err, 0, 0, "", rel=False)
r = pypdf.PdfReader(f"{out}/sheets.pdf")
txt = "".join(p.extract_text() for p in r.pages)
report("PDF text extract contains CROSS SECTIONS and PRELIMINARY", float("CROSS SECTIONS" in txt and "PRELIMINARY" in txt), 1.0, 0, "", rel=False)
CJK = re.compile("[\u3000-\u9fff\uff00-\uffef]")
ncjk = sum(len(CJK.findall(open(f, encoding="utf-8", errors="ignore").read())) for f in glob.glob(f"{out}/*") if f.endswith((".json", ".jsonl", ".md", ".csv", ".html", ".xml", ".dxf")))
report("CJK characters in all text outputs", ncjk + len(CJK.findall(txt)), 0, 0, "", rel=False)
idx = re.findall(r"Sheet (\d+) of (\d+)", txt)
report("PDF pages vs 'Sheet n of N' in title blocks", len(r.pages), int(idx[0][1]) if idx else -1, 0, "", rel=False)

# ---------- 8. pavement: AASHTO 1993 equations re-coded, solved with brentq ----------
pav = [e for e in LOG if e["tool"] == "design_pavement"]
def flex_logw(SN, zr, s0, dpsi, mr):
    return zr * s0 + 9.36 * math.log10(SN + 1) - 0.2 + math.log10(dpsi / 2.7) / (0.4 + 1094 / (SN + 1) ** 5.19) + 2.32 * math.log10(mr) - 8.07
def rigid_logw(D, zr, s0, dpsi, pt, sc, cd, J, ec, k):
    return zr * s0 + 7.35 * math.log10(D + 1) - 0.06 + math.log10(dpsi / 3.0) / (1 + 1.624e7 / (D + 1) ** 8.46) + (4.22 - 0.32 * pt) * math.log10(sc * cd * (D ** 0.75 - 1.132) / (215.63 * J * (D ** 0.75 - 18.42 / (ec / k) ** 0.25)))
sn_ex = brentq(lambda x: flex_logw(x, -1.645, 0.35, 1.9, 5000) - math.log10(5e6), 0.5, 15)
report("AASHTO 93 flexible worked example (Guide Fig 3.1: SN ≈ 5.0)", sn_ex, 5.0, 2.0)
d_ex = brentq(lambda x: rigid_logw(x, -1.645, 0.29, 1.7, 2.5, 650, 1.0, 3.2, 5e6, 72) - math.log10(5.1e6), 5, 20)
report("AASHTO 93 rigid worked example (Guide Fig 3.7 nomograph reading D ≈ 10.0 in; equation solution)", d_ex, 10.0, 3.0)
for e in pav:
    a_ = e["args"]; res = e["result"]; tr = a_["traffic"]
    W = 365 * tr["aadt"] * tr["hv_frac"] * tr["esa_per_hv"] * tr["dir_factor"] * tr["lane_factor"] * ((1 + tr["growth"]) ** tr["years"] - 1) / tr["growth"]
    dpsi = a_["p0"] - a_["pt"]
    if a_["kind"] == "flexible":
        mr = 1500 * a_["subgrade_cbr"]
        sn = brentq(lambda x: flex_logw(x, a_["reliability_zr"], a_["s0"], dpsi, mr) - math.log10(W), 0.05, 20)
        report("pavement flexible: SN over subgrade", res["sn_required"], sn, 1e-4)
        prov = 0.0
        for i, L in enumerate(a_["layers"]):
            mr_b = a_["layers"][i + 1]["modulus_mpa"] * 145.0377 if i + 1 < len(a_["layers"]) else mr
            snr = brentq(lambda x: flex_logw(x, a_["reliability_zr"], a_["s0"], dpsi, mr_b) - math.log10(W), 0.01, 20)
            t = max((snr - prov) / (L["a"] * L["m"]) * 25.4, L["min_mm"]); t = math.ceil(t / 5 - 1e-9) * 5
            prov += L["a"] * L["m"] * t / 25.4
            report(f"pavement flexible: {L['name']} thickness (mm)", res["layers"][i]["thickness_mm"], t, 0, "mm", rel=False)
        report("pavement flexible: SN provided ≥ SN required", res["sn_provided"] - res["sn_required"], 0, 1e9, "", rel=False)
    else:
        D = brentq(lambda x: rigid_logw(x, a_["reliability_zr"], a_["s0"], dpsi, a_["pt"], a_["sc_mpa"] * 145.0377, a_["cd"], a_["j"], a_["ec_mpa"] * 145.0377, a_["k_mpa_m"] * 3.684) - math.log10(W), 4.5, 30)
        report("pavement rigid: slab (mm, D·25.4 rounded up to 10)", res["layers"][0]["thickness_mm"], math.ceil(D * 25.4 / 10 - 1e-9) * 10, 0, "mm", rel=False)

# ---------- 9. route alternatives: search cost re-derived with scipy.sparse.csgraph on the state graph ----------
from scipy.sparse import coo_matrix
from scipy.sparse.csgraph import dijkstra
RA = load("route-alternatives.json"); ra_args = [e for e in LOG if e["tool"] == "generate_route_alternatives"][-1]["args"]
bbox = [e for e in LOG if e["tool"] == "build_surface"][-1]["result"]["bbox"]
cell = ra_args.get("cell_m", 10.0); nx = int(math.floor((bbox[2] - bbox[0]) / cell)) + 1; ny = int(math.floor((bbox[3] - bbox[1]) / cell)) + 1
gx, gy = np.meshgrid(bbox[0] + np.arange(nx) * cell, bbox[1] + np.arange(ny) * cell)
Z = eg(gx, gy)  # same EG triangles (LandXML Faces), independent interpolation
gm = RA["recommendation"]["max_grade_pct_used"] / 100
DIRS = [(1, 0), (2, 1), (1, 1), (1, 2), (0, 1), (-1, 2), (-1, 1), (-2, 1), (-1, 0), (-2, -1), (-1, -1), (-1, -2), (0, -1), (1, -2), (1, -1), (2, -1)]
def seg_dist(P, A, B):
    v = B - A; l2 = (v ** 2).sum(); t = np.clip(((P - A) @ v) / l2 if l2 > 0 else 0, 0, 1)
    return np.hypot(*(A + t[:, None] * v - P).T)
found = []
worst_cost = 0
for alt in RA["alternatives"]:
    sp = alt["weights"]
    pen = np.zeros((ny, nx))
    if sp.get("avoid_previous_m", 0) > 0 and found:
        P_ = np.c_[gx.ravel(), gy.ravel()]; dmin = np.full(len(P_), np.inf)
        for poly in found:
            poly = np.array(poly)
            for k_ in range(len(poly) - 1):
                dmin = np.minimum(dmin, seg_dist(P_, poly[k_], poly[k_ + 1]))
        pen = np.where(dmin.reshape(ny, nx) < sp["avoid_previous_m"], 3.0, 0.0)
    rows_, cols_, vals_ = [], [], []
    J_, I_ = np.mgrid[0:ny, 0:nx]
    for d in range(16):
        for dd in (-1, 0, 1):
            nd = (d + dd) % 16; di, dj = DIRS[nd]
            ni, nj = I_ + di, J_ + dj
            ok = (ni >= 0) & (nj >= 0) & (ni < nx) & (nj < ny)
            i0, j0, i1, j1 = I_[ok], J_[ok], ni[ok], nj[ok]
            z0, z1 = Z[j0, i0], Z[j1, i1]
            l = math.hypot(di, dj) * cell
            if abs(di) == 2 or abs(dj) == 2:
                ai, aj = i0 + int(di / 2), j0 + int(dj / 2); bi, bj = i0 + int((di + np.sign(di)) / 2), j0 + int((dj + np.sign(dj)) / 2)
                inb = (ai >= 0) & (aj >= 0) & (ai < nx) & (aj < ny) & (bi >= 0) & (bj >= 0) & (bi < nx) & (bj < ny)
                zm = np.full(z0.shape, np.nan)
                zm[inb] = (Z[aj[inb], ai[inb]] + Z[bj[inb], bi[inb]]) / 2
                gr = np.maximum(np.abs(zm - z0), np.abs(z1 - zm)) / (l / 2)
            else:
                gr = np.abs(z1 - z0) / l
            ex = np.clip(gr - gm, 0, None) * 100
            c_ = (1 + sp["w_grade"] * ex * ex + sp["w_terrain"] * 100 * gr + pen[j1, i1]) * l + sp["w_turn"] * abs(dd) * cell
            good = np.isfinite(c_)
            if sp.get("hard_grade"):
                good &= ~(gr > 2 * gm)  # switchback search: moves steeper than 2 x max grade are forbidden
            rows_.append(((j0 * nx + i0) * 16 + d)[good]); cols_.append(((j1 * nx + i1) * 16 + nd)[good]); vals_.append(c_[good])
    N = nx * ny * 16
    Gm = coo_matrix((np.concatenate(vals_), (np.concatenate(rows_), np.concatenate(cols_))), shape=(N, N)).tocsr()
    snap = lambda p: (int(np.clip(round((p[0] - bbox[0]) / cell), 0, nx - 1)), int(np.clip(round((p[1] - bbox[1]) / cell), 0, ny - 1)))
    si, sj = snap(ra_args["start"]); ei, ej = snap(ra_args["end"])
    dist = dijkstra(Gm, indices=[(sj * nx + si) * 16 + d for d in range(16)], min_only=True)
    ref = min(dist[(ej * nx + ei) * 16 + d] for d in range(16))
    report(f"route {alt['name']}: least-cost search cost vs scipy csgraph Dijkstra", alt["search_cost"], float(ref), 1e-6)
    found.append(alt["path"])
    plen = sum(math.dist(alt["path"][k_], alt["path"][k_ + 1]) for k_ in range(len(alt["path"]) - 1))
    report(f"route {alt['name']}: path length (m)", alt["path_length_m"], plen, 1e-9)
best = min((a_ for a_ in RA["alternatives"] if "error" not in a_["evaluation"]), key=lambda a_: (a_["evaluation"]["geometry_mandatory_failures"], a_["evaluation"]["cost_proxy"]))
report("route recommendation = min(failures, cost proxy)", float(best["name"] == RA["recommendation"]["recommended"]), 1.0, 0, "", rel=False)
for a_ in RA["alternatives"]:
    e_ = a_["evaluation"]
    if "error" in e_: continue
    rt = RA["recommendation"]["rates"]
    cp = rt["excavation"] * e_["cut_m3"] + rt["embankment"] * e_["fill_m3"] + rt["borrow_waste"] * abs(e_["imbalance_m3"]) + rt["wall_concrete"] * e_["wall_concrete_m3"] + rt["pavement_m2"] * e_["pavement_m2"]
    report(f"route {a_['name']}: cost proxy from its quantities", e_["cost_proxy"], cp, 1e-9)

# ---------- 10. protection: HEC-15 normal depth / shear / riprap and length of need ----------
PR = load("protection.json"); pa = [e for e in LOG if e["tool"] == "design_protection"][-1]["args"]
X_ = pa["runout_length_m"] * (pa["clear_zone_m"] - pa["barrier_offset_m"]) / pa["clear_zone_m"]
worst_lon = max([abs(b_["length_of_need_m"] - X_) for b_ in PR["barriers"]] + [0])
report("barrier length of need X = L_R(L_A − L_2)/L_A, max abs diff (m)", worst_lon, 0, 1e-9, "m", rel=False)
dt = asm.get("ditch")
def nb(da, d50):
    r_ = max(da / d50, 1.5); return 0.319 * da ** (1 / 6) / (2.25 + 5.23 * math.log10(r_))
wd = wt = 0; rip = 9e9
for dch in PR["ditches"]:
    b_, z1, z2, S = dt["bottom_width_m"], dt["foreslope_h"], asm["cut_slope_h"], dch["slope"]
    name = dch["lining"]
    if name.startswith("rock riprap"): d50 = dch["riprap_d50_m"]; nf = lambda y: nb(y, d50)
    elif "25 mm" in name: nf = lambda y: nb(y, 0.025)
    elif "50 mm" in name: nf = lambda y: nb(y, 0.05)
    elif name.startswith("concrete"): nf = lambda y: 0.013
    else: nf = lambda y: 0.020
    def qy(y):
        A = b_ * y + 0.5 * (z1 + z2) * y * y; P = b_ + y * (math.hypot(1, z1) + math.hypot(1, z2))
        return A * (A / P) ** (2 / 3) * S ** 0.5 / nf(y)
    y = brentq(lambda y: qy(y) - dch["q_max_m3s"], 1e-6, 5)
    wd = max(wd, abs(y - dch["depth_m"])); wt = max(wt, abs(9810 * y * S - dch["tau_d"]))
    if name.startswith("rock riprap"):
        re_ = math.sqrt(9.81 * y * S) * d50 / 1.131e-6
        F, SF = (0.047, 1.0) if re_ <= 4e4 else ((0.15, 1.5) if re_ >= 2e5 else (0.047 + (re_ - 4e4) / 1.6e5 * 0.103, 1 + (re_ - 4e4) / 1.6e5 * 0.5))
        rip = min(rip, d50 - SF * y * S / (F * (pa.get("riprap_sg", 2.65) - 1)))
    elif dch["tau_p"] is not None and math.isfinite(dch["tau_p"]):
        report(f"ditch {dch['side']} {dch['from']:.0f}: τd ≤ τp ({name})", dch["tau_p"] - 9810 * y * S, 0, 1e9, "N/m²", rel=False)
report("HEC-15 ditch normal depth (brentq), max abs diff (m)", wd, 0, 1e-4, "m", rel=False)
report("HEC-15 τd = γ d S, max abs diff (N/m²)", wt, 0, 1e-2, "N/m²", rel=False)
if rip < 9e9:
    report("HEC-15 Eq 6.8 riprap: D50 − required (m, ≥ 0)", rip, 0, 1e9, "m", rel=False)

# ---------- 11. earthwork: borrow/waste and overhaul (independent segment code) ----------
EW = q["earthwork"]; kf = EW["shrink_factor"]
report("borrow (compacted) = max(0, fill − cut·k)", EW["borrow_compacted_m3"], max(0.0, corr["fill_m3"] - corr["cut_m3"] * kf), 1e-9, "m³", rel=False)
report("waste (bank) = max(0, cut·k − fill)/k", EW["waste_bank_m3"], max(0.0, corr["cut_m3"] * kf - corr["fill_m3"]) / kf, 1e-9, "m³", rel=False)
o = np.array(mh["ordinates"]); FH = mh["free_haul_m"]
cuts = [0]
xs, ys = list(o[:, 0]), list(o[:, 1])
X2, Y2 = [xs[0]], [ys[0]]; segs = []
for k_ in range(1, len(xs)):
    if ys[k_ - 1] * ys[k_] < 0:
        t = ys[k_ - 1] / (ys[k_ - 1] - ys[k_]); xz = xs[k_ - 1] + t * (xs[k_] - xs[k_ - 1])
        X2.append(xz); Y2.append(0.0); segs.append((X2, Y2)); X2, Y2 = [xz], [0.0]
    X2.append(xs[k_]); Y2.append(ys[k_])
segs.append((X2, Y2))
oh = 0
for X3, Y3 in segs:
    if len(X3) < 2: continue
    Y3 = np.abs(np.array(Y3) - Y3[0]); Vv = Y3.max(); Hh = np.trapezoid(Y3, X3)
    if Vv > 0 and Hh / Vv > FH: oh += Hh - Vv * FH
report("overhaul (segment method beyond free haul) (m³·m)", EW["overhaul_m3m"], oh, 1e-6)

# ---------- 12. profile optimiser result: grades / crest lengths re-checked, objective improved ----------
OP = [e for e in LOG if e["tool"] == "optimize_profile"][-1]["result"]
vp = np.array(OP["vpis_after"]); cl = OP["curve_lengths_after"]
g_ = np.diff(vp[:, 1]) / np.diff(vp[:, 0]) * 100
report("optimised profile: max |grade| ≤ max grade (%)", basis["max_grade_pct"] - np.abs(g_).max(), 0, 1e9, "%", rel=False)
worst_k = 9e9
for k_ in range(1, len(vp) - 1):
    A_ = g_[k_] - g_[k_ - 1]
    if A_ < 0:
        S_ = ssd_ref; L_ = abs(A_) * S_ * S_ / 658 if abs(A_) * S_ * S_ / 658 >= S_ else max(0, 2 * S_ - 658 / abs(A_))
        worst_k = min(worst_k, cl[k_ - 1] - L_)
report("optimised profile: crest length − AASHTO minimum (m, ≥ 0)", worst_k if worst_k < 9e9 else 0.0, 0, 1e9, "m", rel=False)
report("optimiser objective after ≤ before", OP["objective_before"] - OP["objective_after"], 0, 1e12, "", rel=False)

# ---------- 13. setting-out: FG elevations vs independent parabola evaluation ----------
def zprof(s_):
    for k_ in range(1, len(vp) - 1):
        L_ = cl[k_ - 1]
        if L_ > 0 and vp[k_][0] - L_ / 2 <= s_ <= vp[k_][0] + L_ / 2:
            x = s_ - (vp[k_][0] - L_ / 2); g1, g2 = g_[k_ - 1] / 100, g_[k_] / 100
            return vp[k_][1] - g1 * L_ / 2 + g1 * x + (g2 - g1) * x * x / (2 * L_)
    k_ = max(0, min(len(vp) - 2, int(np.searchsorted(vp[:, 0], s_) - 1)))
    return vp[k_][1] + g_[k_] / 100 * (s_ - vp[k_][0])
so = [l.split(",") for l in open(f"{out}/setting-out.csv").read().split("# Centreline")[1].splitlines()[2:] if l and not l.startswith("#")]
report("setting-out FG vs independent parabola, max abs diff (m)", max(abs(float(r_[4]) - zprof(float(r_[0]))) for r_ in so), 0, 6e-4, "m", rel=False)  # CSV writes 3 decimals (±0.0005 m) plus station rounding

# ---------- 14. bill of quantities ties to the design ----------
bq = [l.split(",") for l in open(f"{out}/boq.csv") if l[0] != "#" and not l.startswith("code")]
gq = lambda key: sum(float(r_[-2]) for r_ in bq if key in ",".join(r_))
report("BoQ roadway excavation = corridor cut (m³)", gq("Roadway excavation"), corr["cut_m3"], 0.05, "m³", rel=False)  # CSV writes 1 decimal
report("BoQ embankment = corridor fill (m³)", gq("Embankment construction"), corr["fill_m3"], 0.01, "m³", rel=False)
pl = sum(p["length"] for n_ in nets.values() for p in n_["pipes"])
report("BoQ pipe length = network pipe lengths (m)", sum(float(r_[-2]) for r_ in bq if " pipe " in r_[1] and "Culvert" not in r_[1]), pl, 0.2, "m", rel=False)
report("BoQ barrier length = barrier runs (m)", sum(float(r_[-2]) for r_ in bq if "guardrail" in r_[1].lower()), sum(b_["length_m"] for b_ in PR["barriers"]), 0.2, "m", rel=False)

# ---------- 15. LandXML cross sections re-read with ElementTree: count, FG at CL vs parabola, EG at CL vs TIN ----------
import xml.etree.ElementTree as ET_
NS_ = {"l": "http://www.landxml.org/schema/LandXML-1.2"}
xs_ = ET_.parse(f"{out}/design.xml").findall(".//l:CrossSects/l:CrossSect", NS_)
secs_ = corr["sections"]
report("LandXML CrossSect count = corridor sections", len(xs_), len(secs_), 0, "", rel=False)
def at0(el, nm):
    v = np.array(el.find(f"l:CrossSectSurf[@name='{nm}']/l:PntList2D", NS_).text.split(), float).reshape(-1, 2)
    return float(np.interp(0.0, v[:, 0], v[:, 1]))
report("LandXML FG at CL vs independent parabola, max abs diff (m)", max(abs(at0(e_, "FG") - zprof(float(e_.get("sta")))) for e_ in xs_), 0, 1e-3, "m", rel=False)
report("LandXML EG at CL vs EG TIN at section point, max abs diff (m)", max(abs(at0(e_, "EG") - float(eg(np.array([s_["x"]]), np.array([s_["y"]]))[0])) for e_, s_ in zip(xs_, secs_)), 0, 0.01, "m", rel=False)  # EG sampled every 1 m across; interpolation between samples

# ---------- 16. 3D CAD model (DXF R12) re-read with ezdxf: faces = LandXML TIN faces, 3D centreline z vs parabola ----------
import ezdxf as EZ_
cm_ = EZ_.readfile(f"{out}/civil3d-model.dxf"); msp_ = cm_.modelspace()
nf_ = sum(len(sf_.findall(".//l:Faces/l:F", NS_)) for sf_ in ET_.parse(f"{out}/design.xml").findall(".//l:Surfaces/l:Surface", NS_))
report("CAD model 3DFACE count = LandXML surface faces", len(msp_.query("3DFACE")), nf_, 0, "", rel=False)
cl_pl_ = [e_ for e_ in msp_.query("POLYLINE") if e_.dxf.layer == [e2 for e2 in msp_.query("POLYLINE")][0].dxf.layer][0]
vz_ = []
for v_ in cl_pl_.vertices:
    x_, y_, z_ = v_.dxf.location
    vz_.append((x_, y_, z_))
AL_ = [e_ for e_ in LOG if e_["tool"] == "export_cad_model"][-1]["result"]
report("CAD model layers present (centreline, surfaces, features)", float(all(any(k_ in l_ for l_ in AL_["stats"]["layers"]) for k_ in ["CNTR", "SURF", "FEAT"])), 1.0, 0, "", rel=False)
# centreline vertices are written at equal station steps from the first to the last station
sv_ = np.linspace(float(xs_[0].get("sta")), float(xs_[-1].get("sta")), len(vz_))
report("CAD centreline z vs independent parabola, max abs diff (m)", max(abs(vz_[i_][2] - zprof(sv_[i_])) for i_ in range(len(vz_))), 0, 1e-3, "m", rel=False)

# ---------- 17. Blender scene (glTF 2.0 .glb): container, accessor bounds, ground mesh = EG TIN ----------
import struct as ST_
gb_ = open(f"{out}/scene.glb", "rb").read()
mg_, ver_, tot_ = ST_.unpack("<4sII", gb_[:12]); jl_, jt_ = ST_.unpack("<I4s", gb_[12:20])
gj_ = json.loads(gb_[20:20 + jl_]); bl_, bt_ = ST_.unpack("<I4s", gb_[20 + jl_:28 + jl_]); bb_ = gb_[28 + jl_:28 + jl_ + bl_]
report("glTF container (magic, version 2, length, JSON+BIN chunks)", float(mg_ == b"glTF" and ver_ == 2 and tot_ == len(gb_) and jt_ == b"JSON" and bt_ == b"BIN\x00"), 1.0, 0, "", rel=False)
bad_ = 0
for a_ in gj_["accessors"]:
    v_ = gj_["bufferViews"][a_["bufferView"]]
    if a_["type"] == "VEC3":
        arr_ = np.frombuffer(bb_[v_["byteOffset"]:v_["byteOffset"] + v_["byteLength"]], "<f4").reshape(-1, 3)
        bad_ += int(len(arr_) != a_["count"] or np.abs(arr_.min(0) - a_["min"]).max() > 1e-3 or np.abs(arr_.max(0) - a_["max"]).max() > 1e-3)
    else:
        ix_ = np.frombuffer(bb_[v_["byteOffset"]:v_["byteOffset"] + v_["byteLength"]], "<u4")
        bad_ += int(len(ix_) != a_["count"] or len(ix_) % 3 != 0)
report("glTF accessors consistent with buffer data (count, min/max, triangles)", bad_, 0, 0, "", rel=False)
SJ_ = json.load(open(f"{out}/scene.json")); org_ = SJ_["origin"]
eg_xml_ = ET_.parse(f"{out}/design.xml").find(".//l:Surfaces/l:Surface[@name='EG']", NS_)
egp_ = np.array([p_.text.split() for p_ in eg_xml_.findall(".//l:Pnts/l:P", NS_)], float)  # LandXML: N E Z
m0_ = gj_["meshes"][0]["primitives"][0]; v0_ = gj_["bufferViews"][gj_["accessors"][m0_["attributes"]["POSITION"]]["bufferView"]]
g0_ = np.frombuffer(bb_[v0_["byteOffset"]:v0_["byteOffset"] + v0_["byteLength"]], "<f4").reshape(-1, 3).astype(float)
world_ = np.c_[g0_[:, 0] + org_["e"], -g0_[:, 2] + org_["n"], g0_[:, 1] + org_["z"]]
report("glTF ground mesh back to world = LandXML EG points, max abs diff (m)", float(np.abs(world_ - egp_[:, [1, 0, 2]]).max()) if len(world_) == len(egp_) else 1e9, 0, 0.01, "m", rel=False)  # f32 local coordinates

# pavement layer solids: vertical extent at each section = assembly thickness; trimmed ground on the EG surface
def mesh_pts_(name):
    for m_ in gj_["meshes"]:
        if m_["name"] == name:
            v_ = gj_["bufferViews"][gj_["accessors"][m_["primitives"][0]["attributes"]["POSITION"]]["bufferView"]]
            g_ = np.frombuffer(bb_[v_["byteOffset"]:v_["byteOffset"] + v_["byteLength"]], "<f4").reshape(-1, 3).astype(float)
            return np.c_[g_[:, 0] + org_["e"], -g_[:, 2] + org_["n"], g_[:, 1] + org_["z"]]
    return None
for L_ in SJ_.get("pavement_layers", []):
    P_ = mesh_pts_(L_["mesh"])
    if P_ is None:
        report(f"glTF pavement layer mesh present: {L_['mesh']}", 0.0, 1.0, 0, "", rel=False); continue
    k_ = np.round(P_[:, :2], 2); _, inv_ = np.unique(k_, axis=0, return_inverse=True); inv_ = inv_.ravel()
    ext_ = [P_[inv_ == u_, 2].max() - P_[inv_ == u_, 2].min() for u_ in np.unique(inv_)]
    report(f"glTF pavement layer '{L_['name']}' vertical thickness, max abs diff (m)", float(np.abs(np.array(ext_) - L_["thickness_m"]).max()), 0, 2e-3, "m", rel=False)
if SJ_.get("ground_trimmed_mesh"):
    from scipy.interpolate import LinearNDInterpolator as LI_
    from scipy.spatial import cKDTree as KD_
    T_ = mesh_pts_(SJ_["ground_trimmed_mesh"]); f_ = LI_(egp_[:, [1, 0]], egp_[:, 2]); zt_ = f_(T_[:, 0], T_[:, 1])
    dd_, ii_ = KD_(egp_[:, [1, 0]]).query(T_[:, :2]); orig_ = dd_ < 1e-3
    report("glTF trimmed ground: kept survey vertices = LandXML EG points, max abs dz (m)", float(np.abs(T_[orig_, 2] - egp_[ii_[orig_], 2]).max()), 0, 0.01, "m", rel=False)
    ok_ = ~orig_ & ~np.isnan(zt_)
    # against the planes of the untrimmed ground mesh (same TIN triangles, independent interpolation)
    import matplotlib.tri as MT_
    ix0_ = gj_["accessors"][m0_["indices"]]; vi_ = gj_["bufferViews"][ix0_["bufferView"]]
    tri0_ = np.frombuffer(bb_[vi_["byteOffset"]:vi_["byteOffset"] + vi_["byteLength"]], "<u4").reshape(-1, 3)
    zi_ = np.asarray(MT_.LinearTriInterpolator(MT_.Triangulation(world_[:, 0], world_[:, 1], tri0_), world_[:, 2])(T_[~orig_, 0], T_[~orig_, 1]).filled(np.nan))
    okk_ = ~np.isnan(zi_)
    report("glTF trimmed ground: clip / seam vertices vs untrimmed TIN planes, max abs dz (m)", float(np.abs(T_[~orig_, 2][okk_] - zi_[okk_]).max()), 0, 1e-3, "m", rel=False)

json.dump([dict(zip(["check", "civil_mcp", "independent", "diff", "unit", "ok"], r_)) for r_ in rows], open(f"{out}/independent-checks.json", "w"), ensure_ascii=False, indent=1)

# ---------- 18. superelevation runoff: e, required development and spiral lengths re-derived (AASHTO) ----------
# e by AASHTO Method 1 (e = emax·Rmin/R, Rmin = V²/(127(emax+f))), maximum relative gradient from
# AASHTO 2018 Table 3-15 (metric), required L = w·(e+nc)/Δmax; the selected route's spirals must be
# at least L, fit in the deflection (Ls/R ≤ Δ) and not overlap between curves.
log_ = [json.loads(l) for l in open(f"{out}/run-log.jsonl")]
bas_ = [e for e in log_ if e["tool"] == "set_design_basis"][-1]["args"]["basis"]
sel_ = [e for e in log_ if e["tool"] == "select_route"]
if bas_.get("standard") == "US_AASHTO_2018" and sel_:
    sr_ = sel_[-1]["result"]
    P_, R_, L_ = np.array(sr_["pis"]), sr_["radii"], sr_["spirals"]
    V_ = bas_["design_speed_kmh"]
    dmax_ = {20: .80, 30: .75, 40: .70, 50: .65, 60: .60, 70: .55, 80: .50, 90: .47, 100: .44, 110: .41, 120: .38, 130: .35}[int(round(V_ / 10) * 10)] / 100
    rmin_ = V_ ** 2 / (127 * (bas_["e_max"] + bas_["f_max"]))
    nc_, w_ = bas_["normal_crown"], bas_["lane_width_m"]
    T_ = []
    for k, (R, Ls) in enumerate(zip(R_, L_)):
        a1, a2 = np.arctan2(*(P_[k + 1] - P_[k])[::-1]), np.arctan2(*(P_[k + 2] - P_[k + 1])[::-1])
        D = abs((a2 - a1 + math.pi) % (2 * math.pi) - math.pi)
        e_ = bas_["e_max"] * rmin_ / R
        e_ = 0.0 if e_ < nc_ else e_  # below normal crown: no superelevation (removing adverse crown handled separately)
        need = w_ * (e_ + nc_) / dmax_ if e_ > 0 else 0.0
        sc_ = se["curves"][k]
        report(f"SE curve {k+1} e (proportional, AASHTO Method 1)", sc_["e"], e_ if e_ > 0 else sc_["e"], 1e-6, "", rel=False)
        report(f"SE curve {k+1} required development L = w(e+nc)/Δmax (m)", sc_["required_length"], need if e_ > 0 else sc_["required_length"], 0.01, "m", rel=False)
        report(f"SE curve {k+1} spiral Ls - required L (m, >= 0)", min(0.0, Ls - need), 0.0, 1e-6, "m", rel=False)
        report(f"SE curve {k+1} spirals fit the deflection, max(0, Ls/R - Δ) (rad)", max(0.0, Ls / R - D), 0.0, 1e-9, "rad", rel=False)
        # tangent length with spirals: (R+p)·tan(Δ/2) + k, p ≈ Ls²/24R, k ≈ Ls/2 - Ls³/240R²
        T_.append((R + Ls ** 2 / (24 * R)) * math.tan(D / 2) + Ls / 2 - Ls ** 3 / (240 * R * R))
    for k in range(len(T_) - 1):
        leg = float(np.hypot(*(P_[k + 2] - P_[k + 1])))
        report(f"SE curves {k+1}-{k+2} no overlap, max(0, Ts1+Ts2 - leg) (m)", max(0.0, T_[k] + T_[k + 1] - leg), 0.0, 0.5, "m", rel=False)
    # tangent runout w·nc/Δmax before the first / after the last curve (informational)
    ro_ = w_ * nc_ / dmax_
    print(f"info: tangent runout {ro_:.1f} m; first curve TS at {float(np.hypot(*(P_[1]-P_[0]))) - T_[0]:.1f} m from the start, last ST {float(np.hypot(*(P_[-1]-P_[-2]))) - T_[-1]:.1f} m before the end")
else:
    print("info: runoff re-check covers US_AASHTO_2018 runs only; skipped")

# ---------- ROW screening: take areas by rasterising the ROW polygon x parcels (0.5 m cells) ----------
imp_ = [e for e in LOG if e["tool"] == "import_parcels"]
ra_ = load("route-alternatives.json") if os.path.exists(f"{out}/route-alternatives.json") else None
if imp_ and ra_ and any(a.get("row") for a in ra_["alternatives"]):
    ia_ = imp_[-1]["args"]
    ro_ = {"buffer_m": 0.0, "min_remnant_m2": 0.0, "min_remnant_fraction": 0.0, "admin_factor": 0.0, "severance_factor": 0.0, **ia_.get("row", {})}
    gj_ = json.load(open(os.path.join(out, os.path.basename(ia_["path"]))))
    unit_ = {"m2": 1.0, "ha": 1e4, "acre": 4046.8564224, "sf": 0.09290304}[ia_.get("rate_unit", "m2")]
    fb_ = {k: v / unit_ for k, v in ia_.get("fallback_rates", {}).items()}
    def area_(r):
        r = np.asarray(r, float); return 0.5 * float(np.dot(r[:-1, 0], r[1:, 1]) - np.dot(r[1:, 0], r[:-1, 1]))
    par_, bld_ = {}, []
    for f in gj_["features"]:
        pr = f["properties"]; ring = f["geometry"]["coordinates"][0]
        if pr.get("feature_type") == "building":
            bld_.append((pr["id"], pr.get("parcel_id"), Path(ring)))
        else:
            par_[pr["id"]] = (pr, Path(ring), abs(area_(ring)))
    def rate_(pr, a):
        if pr.get("land_rate") is not None: return pr["land_rate"] / unit_
        if pr.get("land_value") is not None: return pr["land_value"] / a
        return fb_.get(pr.get("zoning"))
    h_ = 0.5
    for alt in ra_["alternatives"]:
        w = alt.get("row")
        if not w: continue
        poly = np.asarray(w["polygon"]); rp = Path(poly)
        x0, y0 = poly.min(0); x1, y1 = poly.max(0)
        X, Y = np.meshgrid(np.arange(x0 + h_ / 2, x1, h_), np.arange(y0 + h_ / 2, y1, h_))
        P = np.c_[X.ravel(), Y.ravel()]; P = P[rp.contains_points(P)]
        report(f"ROW {alt['name']}: ROW polygon area (m², shoelace vs raster)", w["row_area_m2"], len(P) * h_ * h_, 1.0)
        hit_ = {b[0]: b[1] for b in bld_ if b[2].contains_points(P).any()}
        tot_, unknown_, worst_, nfull_, nref_, cost_bad_ = 0.0, False, 0.0, 0, 0, 0
        sum_ours_, sum_ref_ = 0.0, 0.0
        for t in w["takes"]:
            pr, pp, pa = par_[t["parcel_id"]]
            a_ref = float(pp.contains_points(P).sum()) * h_ * h_
            sum_ours_ += t["take_area_m2"]; sum_ref_ += a_ref
            # raster error bound: half a cell along the take perimeter (approximated as 4·sqrt(area) + 2 x ROW crossing)
            worst_ = max(worst_, abs(t["take_area_m2"] - a_ref) / (0.5 * h_ * (4 * math.sqrt(max(a_ref, 1.0))) + 1.0))
            rem = pa - t["take_area_m2"]
            bh = [b for b, p in hit_.items() if p == t["parcel_id"]]
            full = bool(bh) or (ro_["min_remnant_m2"] > 0 and rem < ro_["min_remnant_m2"]) or (ro_["min_remnant_fraction"] > 0 and rem < ro_["min_remnant_fraction"] * pa)
            nfull_ += int(bool(t["full_take"])); nref_ += int(full)
            if bool(t["full_take"]) != full: cost_bad_ += 1
            r = rate_(pr, pa)
            if r is None or (full and pr.get("improvement_value") is None):
                if t["cost"] is not None: cost_bad_ += 1
                unknown_ = True; continue
            land = r * (pa if full else t["take_area_m2"])
            c = (land + ((pr.get("improvement_value") or 0.0) if full else 0.0) + (0 if full else ro_["severance_factor"] * land)) * (1 + ro_["admin_factor"])
            if t["cost"] is None or abs(t["cost"] - c) > 1e-6 * max(1.0, c): cost_bad_ += 1
            tot_ += c
        report(f"ROW {alt['name']}: total take area over {len(w['takes'])} parcels (m², raster 0.5 m)", sum_ours_, sum_ref_, 1.0)
        report(f"ROW {alt['name']}: worst per-parcel take-area difference / raster error bound (<= 1)", min(worst_, 99.0), 0.0, 1.0, "", rel=False)
        report(f"ROW {alt['name']}: full takes (building hit or remnant below threshold)", float(nfull_), float(nref_), 0, "", rel=False)
        report(f"ROW {alt['name']}: takes whose type or cost (land + improvements + severance)(1 + admin) differs", float(cost_bad_), 0.0, 0, "", rel=False)
        report(f"ROW {alt['name']}: relocations (buildings in the ROW)", float(w["relocations"]), float(len(hit_)), 0, "", rel=False)
        if unknown_:
            report(f"ROW {alt['name']}: cost unknown (null 1 = yes), known part checked", float(w["cost"] is None), 1.0, 0, "", rel=False)
        else:
            report(f"ROW {alt['name']}: ROW cost (sum of takes)", w["cost"], tot_, 1e-6)
    # ranking re-derived from the per-alternative numbers
    rk = ra_["recommendation"]
    nogo = lambda a: rk["row_options"]["no_go"] == "infeasible" and bool(a["row"]["no_go_hits"])
    feas = [a for a in ra_["alternatives"] if not nogo(a)]
    known = all(a["row"]["cost"] is not None for a in feas)
    o_ = rk["row_options"]
    sc = lambda a: (nogo(a), a["evaluation"]["geometry_mandatory_failures"], o_["w_construction"] * a["evaluation"]["cost_proxy"] + (o_["w_row"] * (a["row"]["cost"] or 0.0) if known else 0) + o_["relocation_penalty"] * a["row"]["relocations"])
    ref_rank = [a["name"] for a in sorted(ra_["alternatives"], key=sc)]
    ok_ = ref_rank == rk["ranking_with_row"]
    report("ROW ranking with ROW re-derived (1 = same order)", float(ok_), 1.0, 0, "", rel=False)
    if ok_: print("info: ranking with ROW", " > ".join(ref_rank))
    lx_ = glob.glob(f"{out}/*.xml")
    for f in lx_:
        n_ = len(ET.parse(f).getroot().findall(".//{http://www.landxml.org/schema/LandXML-1.2}Parcel"))
        if n_:
            report(f"LandXML {os.path.basename(f)}: Parcel elements = imported parcels", float(n_), float(len(par_)), 0, "", rel=False)

print(f"\n{len(rows)} checks, {len(fails)} failed")
sys.exit(1 if fails else 0)
