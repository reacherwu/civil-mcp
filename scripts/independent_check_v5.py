"""Independent re-checks of the v5 calculations of a civil-mcp demo run. Shares NO code with the
Rust crates: every quantity is recomputed here from the published formulas (re-coded), with a
second numerical method where one exists, and compared with the files the tools wrote.

usage: python scripts/independent_check_v5.py <demo-out>
"""
import csv, heapq, json, math, sys
import numpy as np
import xml.etree.ElementTree as ET
import matplotlib.tri as mtri
from matplotlib.path import Path
from scipy.optimize import brentq, fsolve

out = sys.argv[1]
fails, rows = [], []
G = 9.80665


def report(name, ours, ref, tol, unit="", rel=True):
    diff = (ours - ref) / ref * 100 if rel and ref else ours - ref
    ok = abs(diff) <= tol
    rows.append((name, float(ours), float(ref), float(diff), "%" if rel else unit, bool(ok)))
    print(f"{'OK ' if ok else 'BAD'} {name}: civil-mcp {ours:.6g} | independent {ref:.6g} | diff {diff:+.4g}{'%' if rel else ' ' + unit} (tol {tol})")
    if not ok:
        fails.append(name)


LOG = [json.loads(l) for l in open(f"{out}/run-log.jsonl")]
last = lambda t: [e for e in LOG if e["tool"] == t][-1]
rcsv = lambda f: list(csv.DictReader(l for l in open(f"{out}/{f}") if not l.startswith("#")))

# ---------- H1. terrain sampling + priority flood + D8 accumulation (heapq, own neighbour logic) ----------
ns = "{http://www.landxml.org/schema/LandXML-1.2}"
root = ET.parse(f"{out}/design.xml").getroot()
for sf in root.iter(ns + "Surface"):
    if sf.get("name") == "EG":
        P = np.array([[float(v) for v in p.text.split()] for p in sf.iter(ns + "P")])
        F = np.array([[int(v) - 1 for v in f.text.split()] for f in sf.iter(ns + "F")])
tri = mtri.Triangulation(P[:, 1], P[:, 0], F)
egi = mtri.LinearTriInterpolator(tri, P[:, 2])
hdr = open(f"{out}/flow-grid.csv").readline()
cell = float(hdr.split("cell ")[1].split(" m")[0])
ox, oy = [float(v) for v in hdr.split("origin (")[1].split(")")[0].split(",")]
fg = rcsv("flow-grid.csv")
I = np.array([int(r["i"]) for r in fg]); J = np.array([int(r["j"]) for r in fg])
nx, ny = I.max() + 1, J.max() + 1
zraw_r = np.full((ny, nx), np.nan)
for r, i, j in zip(fg, I, J):
    zraw_r[j, i] = float(r["z_raw"])
XX, YY = np.meshgrid(ox + np.arange(nx) * cell, oy + np.arange(ny) * cell)
z = np.ma.filled(egi(XX, YY), np.nan)
m = ~np.isnan(zraw_r) & ~np.isnan(z)
report("flow grid: terrain samples vs matplotlib TriInterpolator (max |dz| m)", float(np.max(np.abs(z[m] - zraw_r[m]))), 0.0, 1e-3, "m", rel=False)
z = np.where(np.isnan(zraw_r), np.nan, z)
valid = ~np.isnan(z)
zf = z.copy(); done = np.zeros_like(valid)
pq = []
N8 = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
for j in range(ny):
    for i in range(nx):
        if not valid[j, i]: continue
        edge = i in (0, nx - 1) or j in (0, ny - 1) or any(not valid[j + b, i + a] for a, b in N8)
        if edge:
            done[j, i] = True; heapq.heappush(pq, (zf[j, i], j * nx + i))
while pq:
    zc, k = heapq.heappop(pq); j, i = divmod(k, nx)
    for a, b in N8:
        ii, jj = i + a, j + b
        if 0 <= ii < nx and 0 <= jj < ny and valid[jj, ii] and not done[jj, ii]:
            done[jj, ii] = True
            if zf[jj, ii] <= zf[j, i]: zf[jj, ii] = zf[j, i] + 1e-5
            heapq.heappush(pq, (zf[jj, ii], jj * nx + ii))
rec = -np.ones((ny, nx), int)
for j in range(ny):
    for i in range(nx):
        if not valid[j, i]: continue
        best = 0.0
        for a, b in N8:
            ii, jj = i + a, j + b
            if 0 <= ii < nx and 0 <= jj < ny and valid[jj, ii]:
                s = (zf[j, i] - zf[jj, ii]) / (cell * math.hypot(a, b))
                if s > best: best, rec[j, i] = s, jj * nx + ii
order = sorted(np.flatnonzero(valid.ravel()), key=lambda k: -zf.ravel()[k])
acc = np.where(valid.ravel(), cell * cell, 0.0); ln = np.zeros(nx * ny)
for k in order:
    r = rec.ravel()[k]
    if r >= 0:
        acc[r] += acc[k]
        st = cell * (math.sqrt(2) if (r % nx != k % nx and r // nx != k // nx) else 1)
        if ln[k] + st > ln[r]: ln[r] = ln[k] + st
H = last("design_hydrology")
xs = H["result"]["crossings"]; cats = H["result"]["catchments"]; ha = H["args"]
for x in xs:
    k = x["cell"]
    report(f"catchment area {x['id']} (D8, independent)", x["area_ha"], acc[k] / 1e4, 2.0)
    report(f"flow path length {x['id']}", x["length_m"], max(ln[k], cell), 2.0)
# ---------- H2. rational method with log-log IDF and Kirpich tc ----------
pts = np.array(ha["idf"]["points"])
def idf(t):
    t = min(max(t, pts[0, 0]), pts[-1, 0])
    return float(np.exp(np.interp(np.log(t), np.log(pts[:, 0]), np.log(pts[:, 1]))))
for x, c in zip(xs, cats):
    tc = max(0.0195 * x["length_m"] ** 0.77 * x["slope"] ** -0.385, 5.0)
    report(f"tc Kirpich {x['id']} (min)", c["tc_min"], tc, 1e-6)
    report(f"rational Q {x['id']} = C i A / 360 (m3/s)", c["q_m3s"], ha["c"] * idf(tc) * x["area_ha"] / 360, 1e-6)

# ---------- C. culverts: HDS-5 inlet/outlet control re-coded, size selection ----------
CO = {"concrete_square_headwall": (0.0098, 2.0, 0.0398, 0.67, 0.5, False), "concrete_groove_headwall": (0.0018, 2.0, 0.0292, 0.74, 0.2, False),
      "concrete_groove_projecting": (0.0045, 2.0, 0.0317, 0.69, 0.2, False), "cmp_headwall": (0.0078, 2.0, 0.0379, 0.69, 0.5, False),
      "cmp_mitered": (0.0210, 1.33, 0.0463, 0.75, 0.7, True), "cmp_projecting": (0.0340, 1.50, 0.0553, 0.54, 0.9, False)}
def seg(D, y):
    th = 2 * math.acos(1 - 2 * y / D); return D * D / 8 * (th - math.sin(th)), D * math.sin(th / 2), D * th / 2
def dcrit(D, Q):
    return brentq(lambda y: seg(D, y)[0] ** 3 / seg(D, y)[1] - Q * Q / G, 1e-6 * D, 0.999 * D)
def hds5(kind, D, Q, n, L, S, TW):
    K, M, c, Y, ke, mit = CO[kind]
    A = math.pi * D * D / 4; x = 1.811 * Q / (A * math.sqrt(D)); ks = 0.7 if mit else -0.5
    dc = dcrit(D, Q); Ac = seg(D, dc)[0]; Hc = dc + (Q / Ac) ** 2 / (2 * G)
    un = Hc / D + K * x ** M + ks * S; sub = c * x * x + Y + ks * S
    hwi = D * (un if x <= 3.5 else sub if x >= 4.0 else un + (sub - un) * (x - 3.5) / 0.5)
    V = Q / A; Hl = (1 + ke + 2 * G * n * n * L / (D / 4) ** (4 / 3)) * V * V / (2 * G)
    hwo = Hl + max(TW, (dc + D) / 2) - L * S
    return hwi, hwo
CA = last("design_culverts")["args"]
for r in rcsv("culverts.csv"):
    D, Q, L, S = float(r["d_m"]), float(r["q_m3s"]), float(r["length_m"]), float(r["slope"])
    tw = CA.get("tailwater_m", 0.0)
    hi, ho = hds5(r["inlet"], D, Q, CA["n"], L, S, tw)
    report(f"culvert {r['id']} inlet-control HW (m)", float(r["hw_inlet_m"]), hi, 1e-3, "m", rel=False)
    report(f"culvert {r['id']} outlet-control HW (m)", float(r["hw_outlet_m"]), ho, 1e-3, "m", rel=False)
    report(f"culvert {r['id']} allowable - HW (m spare, >= 0)", max(float(r["allow_hw_m"]) - max(hi, ho), 0.0), float(r["allow_hw_m"]) - max(hi, ho), 1e-9, "m", rel=False)
    sizes = CA["sizes_m"]; k = sizes.index(D)
    if k > 0:  # the next smaller size must fail headwater or cover
        d2 = sizes[k - 1]; h2 = max(hds5(r["inlet"], d2, Q, CA["n"], L, S, tw))
        allow2 = min(CA["hw_d_max"] * d2, float(r["cl_elev"]) - CA["freeboard_m"] - float(r["inv_in"]))
        cover2 = float(r["cover_m"]) + (D - d2)
        report(f"culvert {r['id']} next smaller D {d2} m fails HW or cover (1 = yes)", float(h2 > allow2 or cover2 < CA["min_cover_m"]), 1.0, 0, "", rel=False)

# ---------- S. slope stability: Bishop (200 slices) and Spencer (fsolve on F, theta) re-coded ----------
SA = last("check_slope_stability")
soil = SA["args"]["soil"]; c_, ph, gm, ru = soil["c_kpa"], math.radians(soil["phi_deg"]), soil["gamma_knm3"], soil.get("ru", 0.0)
def slices(p, xc, zc, R, a, b, n=200):
    p = np.array(p); xs_ = a + (np.arange(n) + 0.5) * (b - a) / n; bw = (b - a) / n
    zt = np.interp(xs_, p[:, 0], p[:, 1]); zb = zc - np.sqrt(np.maximum(R * R - (xs_ - xc) ** 2, 0))
    h = np.maximum(zt - zb, 0); al = np.arcsin(np.clip((xs_ - xc) / R, -1, 1))
    return bw, h * bw * gm, al
def bishop(bw, W, al):
    t = math.tan(ph); F = 1.5; u = ru * W / bw
    for _ in range(300):
        F2 = np.sum((c_ * bw + (W - u * bw) * t) / (np.cos(al) * (1 + np.tan(al) * t / F))) / np.sum(W * np.sin(al))
        if abs(F2 - F) < 1e-12: break
        F = F2
    return F
def spencer(bw, W, al, F0):
    t = math.tan(ph); u = ru * W / bw
    def Q(F, th):
        num = c_ * bw / np.cos(al) / F + (W * np.cos(al) - u * bw / np.cos(al)) * t / F - W * np.sin(al)
        return num / (np.cos(al - th) * (1 + np.tan(al - th) * t / F))
    def eqs(v):
        F, th = v; q = Q(F, th); return [np.sum(q), np.sum(q * np.cos(al - th))]
    return fsolve(eqs, [F0, 0.2])[0]
rng = np.random.default_rng(1)
for s in SA["result"]["sections"]:
    cr = s["circle"]; bw, W, al = slices(s["profile"], cr["xc"], cr["zc"], cr["r"], cr["x_entry"], cr["x_exit"])
    fb = bishop(bw, W, al)
    tag = f"{s['kind']} {s['station']:.0f} {s['side']}"
    report(f"slope {tag}: Bishop FS on the reported circle (200 slices)", cr["fs_bishop"], fb, 1.5)
    report(f"slope {tag}: Spencer FS (fsolve on F, theta)", cr["fs_spencer"], spencer(bw, W, al, fb), 2.0)
    p = np.array(s["profile"]); toe, crest = s["toe_x"], s["crest_x"]; w = max(abs(crest - toe), 2.0)
    xx = np.linspace(p[0, 0], p[-1, 0], 1500); zt = np.interp(xx, p[:, 0], p[:, 1])
    best = 9e9
    for _ in range(1500):
        xc = rng.uniform(min(toe, crest) - 0.5 * w, max(toe, crest) + 1.5 * w); zc = rng.uniform(p[:, 1].max(), p[:, 1].max() + 2.5 * w)
        R = rng.uniform(0.3 * w, 3.0 * w + (zc - p[:, 1].min()))
        inside = (np.abs(xx - xc) < R) & (zc - np.sqrt(np.maximum(R * R - (xx - xc) ** 2, 0)) < zt)
        if inside.sum() < 10: continue
        a, b = xx[inside][0], xx[inside][-1]
        if a <= p[0, 0] + 1e-6 or b >= p[-1, 0] - 1e-6 or (b - a) < 0.5 * w or inside.sum() < 0.98 * (b - a) / (p[-1, 0] - p[0, 0]) * 1500: continue
        bw_, W_, al_ = slices(p, xc, zc, R, a, b)
        if np.sum(W_ * np.sin(al_)) <= 0: continue
        best = min(best, bishop(bw_, W_, al_))
    report(f"slope {tag}: best of 1500 random circles vs tool critical FS (lower = negative %)", min((best - cr["fs_bishop"]) / cr["fs_bishop"] * 100, 0.0) if best < 9e8 else 0.0, 0.0, 5.0, "%", rel=False)

# ---------- D. subsurface drainage (FHWA / Cedergren, re-coded) ----------
SD = last("design_subsurface_drainage")
a_, r_ = SD["args"], SD["result"]["result"]
W_, nl, sx, g = SD["result"]["half_width_m"], SD["result"]["lanes_per_side"], SD["result"]["cross_slope"], SD["result"]["max_grade"]
qi = a_.get("ic_m3_day_m", 0.223) * ((nl + 1) / W_ + 1 / a_["crack_spacing_m"]) + a_.get("kp_m_day", 0.0)
report("subsurface qi (m3/day/m2)", r_["qi_m3_day_m2"], qi, 1e-9)
SR = math.hypot(sx, g); LR = W_ * math.sqrt(1 + (g / sx) ** 2)
report("subsurface layer capacity k*SR*H", r_["layer_capacity_m3_day_per_m"], a_["k_layer_m_day"] * SR * a_["layer_thickness_m"], 1e-9)
report("subsurface t50 Casagrande-Shannon (h)", r_["t50_hours"], a_["porosity"] * LR ** 2 / (2 * a_["k_layer_m_day"] * (a_["layer_thickness_m"] + LR * SR)) * 24, 1e-9)

# ---------- P. mechanistic: Ahlvin-Ulery centre-line stresses -> strains (different derivation) ----------
MA = last("check_pavement_mechanistic")
mi, mr = MA["args"], MA["result"]["result"]
p0 = mi.get("tyre_kpa", 750.0); a0 = math.sqrt(mi.get("wheel_kn", 20.0) / (math.pi * p0)); nu = mi.get("nu", 0.35); f_ = mi.get("odemark_f", 0.9)
def strains(zd, E):
    R = math.hypot(a0, zd); sz = p0 * (1 - zd ** 3 / R ** 3); sr = p0 / 2 * ((1 + 2 * nu) - 2 * (1 + nu) * zd / R + zd ** 3 / R ** 3)
    return (sz - 2 * nu * sr) / (E * 1000), ((1 - nu) * sr - nu * sz) / (E * 1000)
he = f_ * sum(l["h_m"] * (l["e_mpa"] / mi["subgrade_e_mpa"]) ** (1 / 3) for l in mi["layers"])
report("mechanistic subgrade vertical strain (microstrain)", mr["eps_v_sub"], strains(he, mi["subgrade_e_mpa"])[0] * 1e6, 1e-6)
e2 = mi["layers"][1]["e_mpa"]; he1 = f_ * mi["layers"][0]["h_m"] * (mi["layers"][0]["e_mpa"] / e2) ** (1 / 3)
report("mechanistic asphalt horizontal strain (microstrain)", mr["eps_t_ac"], -strains(he1, e2)[1] * 1e6, 1e-6)

# ---------- W. wall stem flexure: equilibrium re-derived (a = As fy / 0.85 f'c b) ----------
WA = last("design_wall_reinforcement")["args"]
for r in rcsv("wall-reinforcement.csv"):
    hs, t = float(r["stem_h_m"]), float(r["t_m"])
    ka = math.tan(math.pi / 4 - math.radians(WA["phi_soil_deg"]) / 2) ** 2
    Mu = WA["load_factor"] * (ka * WA["gamma_knm3"] * hs ** 3 / 6 + ka * WA.get("surcharge_kpa", 0) * hs ** 2 / 2)
    d = t - WA["cover_m"] - WA["bar_d_mm"] / 2000
    As = brentq(lambda As: WA.get("phi", 0.9) * As * WA["fy_mpa"] * 1e3 * (d - As * WA["fy_mpa"] / (2 * 0.85 * WA["fc_mpa"])) - Mu, 1e-12, 0.85 * WA["fc_mpa"] * d / WA["fy_mpa"])
    report(f"wall {r['station']} {r['side']} Mu (kNm/m)", float(r["mu_knm_per_m"]), Mu, 0.01)
    report(f"wall {r['station']} {r['side']} As required (mm2/m)", float(r["as_req_mm2_per_m"]), As * 1e6, 0.05)
    ab = WA.get("bars_per_bundle", 1) * math.pi * WA["bar_d_mm"] ** 2 / 4
    report(f"wall {r['station']} {r['side']} bar spacing <= Ab/As (mm spare)", ab / max(As * 1e6, float(r["as_min_mm2_per_m"])) * 1000 - float(r["spacing_mm"]), 0.0, 1e9, "mm", rel=False)

# ---------- L. lighting, land, locality, sight distance, marking ----------
LA = last("design_lighting"); la, lr = LA["args"], LA["result"]
report("lighting spacing <= lumen-method S = lm CU LLF / (E W) (m spare)", la["lumens"] * la["cu"] * la["llf"] / (la["e_avg_lux"] * lr["width_m"]) - lr["spacing_m"], 0.0, 1e9, "m", rel=False)
fp = np.array([[float(r["easting"]), float(r["northing"])] for r in rcsv("footprint.csv")])
x_, y_ = fp[:, 0], fp[:, 1]
report("footprint area (shoelace, numpy)", last("land_requirements")["result"]["footprint_area_m2"], 0.5 * abs(np.dot(x_, np.roll(y_, -1)) - np.dot(y_, np.roll(x_, -1))), 1e-3)
fpath = Path(fp)
LR_ = last("land_requirements")
for pc, tk in zip(LR_["args"]["parcels"], LR_["result"]["takes"]):
    poly = np.array(pc["polygon"]); x0, y0 = poly.min(0); x1, y1 = poly.max(0)
    gx, gy = np.meshgrid(np.arange(x0 + 0.25, x1, 0.5), np.arange(y0 + 0.25, y1, 0.5))
    q = np.c_[gx.ravel(), gy.ravel()]
    ref = (Path(poly).contains_points(q) & fpath.contains_points(q)).sum() * 0.25
    report(f"parcel take {pc['id']} (0.5 m raster, matplotlib Path) (m2)", tk["take_m2"], ref, max(0.015 * ref, 50.0), "m2", rel=False)
def utm_inv(E, Nn, zone):  # Krueger n-series (Karney 2011): a different series from the tool's Snyder formula
    a = 6378137.0; f = 1 / 298.257223563; n = f / (2 - f); A = a / (1 + n) * (1 + n * n / 4 + n ** 4 / 64); k0 = 0.9996
    b = [n / 2 - 2 * n * n / 3 + 37 * n ** 3 / 96, n * n / 48 + n ** 3 / 15, 17 * n ** 3 / 480]
    d = [2 * n - 2 * n * n / 3 - 2 * n ** 3, 7 * n * n / 3 - 8 * n ** 3 / 5, 56 * n ** 3 / 15]
    xi = Nn / (k0 * A); eta = (E - 500000) / (k0 * A)
    xi_ = xi - sum(b[j] * math.sin(2 * (j + 1) * xi) * math.cosh(2 * (j + 1) * eta) for j in range(3))
    eta_ = eta - sum(b[j] * math.cos(2 * (j + 1) * xi) * math.sinh(2 * (j + 1) * eta) for j in range(3))
    chi = math.asin(math.sin(xi_) / math.cosh(eta_))
    lat = chi + sum(d[j] * math.sin(2 * (j + 1) * chi) for j in range(3))
    return math.degrees(lat), zone * 6 - 183 + math.degrees(math.atan(math.sinh(eta_) / math.cos(xi_)))
LP = last("locality_plan")["result"]
for k in ("start", "end"):
    la_, lo_ = utm_inv(LP[k]["e"], LP[k]["n"], LP["zone"])
    report(f"locality {k} latitude vs Krueger series (m on ground)", (LP[k]["lat"] - la_) * 111320, 0.0, 0.05, "m", rel=False)
    report(f"locality {k} longitude vs Krueger series (m on ground)", (LP[k]["lon"] - lo_) * 111320 * math.cos(math.radians(la_)), 0.0, 0.05, "m", rel=False)
IS = last("check_intersection_sight")
report("ISD = 0.278 V tg (m)", IS["result"]["required_m"], 0.278 * 50 * IS["args"]["time_gap_s"], 1e-9)
MK = last("design_marking_signing")["result"]
mph = math.ceil(50 / 1.609344 / 5) * 5  # design speed rounded up to the next 5 mph row
report(f"MUTCD Table 3B-1 PSD at {mph} mph = {dict([(25, 450), (30, 500), (35, 550), (40, 600)])[mph]} ft (m)", MK["psd_m"], dict([(25, 450), (30, 500), (35, 550), (40, 600)])[mph] * 0.3048, 1e-6)

# ---------- E. erosion control: check-dam count from L = h / S on the design profile ----------
EA = last("plan_erosion_control")["args"]
vp = last("create_profile") if any(e["tool"] == "create_profile" for e in LOG) else None
for r in rcsv("esc-quantities.csv"):
    if r["item"] == "check dam":
        s0, s1 = float(r["from_station"]), float(r["to_station"]); sp = float(r["basis"].split("spacing ")[1].split(" m")[0])
        report(f"check dams {r['side']} {s0:.0f}-{s1:.0f}: n = floor(L/sp)+1", float(r["quantity"]), math.floor((s1 - s0) / sp) + 1, 1e-9)

print(f"\n{len(rows)} v5 checks, {len(fails)} outside tolerance")
json.dump([dict(zip(["check", "civil_mcp", "independent", "diff", "unit", "ok"], r)) for r in rows], open(f"{out}/independent-check-v5.json", "w"), indent=1)
sys.exit(1 if fails else 0)
