"""Independent re-checks of the v6 calculations of a civil-mcp demo run. Shares NO code with the Rust
crates. Layered elastic responses are compared with PyMastic (Nakhaei, Apache-2.0, an independent
multilayer program) and PyMastic itself with the published KENPAVE table; the other quantities are
re-coded from the published formulas.

usage: python scripts/independent_check_v6.py <demo-out> [path-to-PyMastic-checkout]
"""
import csv, json, math, sys, os
import numpy as np

out = sys.argv[1]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from si_view import si_dir
out = si_dir(out)
pm_dir = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("PYMASTIC", "/workspace/tools/PyMastic")
sys.path.insert(0, os.path.join(pm_dir, "Main"))
from MLE import PyMastic  # noqa: E402

fails, rows = [], []


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
here = os.path.dirname(os.path.abspath(__file__))

# ---------- M0. the referee itself vs the published KENPAVE table ----------
ken = list(csv.DictReader(open(os.path.join(here, "..", "data", "validation", "kenpave_3layer.csv"))))
z = [float(r["Z"]) for r in ken[1:]]
RS = PyMastic(100.0, 5.99, [0], z, [6, 10], [500, 40, 10], [0.35, 0.4, 0.45], 7e-7, isBounded=[1, 1], iteration=40, inverser="solve")
md = max(abs(RS["Displacement_Z"][i, 0] - float(r["Displacement"])) / float(r["Displacement"]) for i, r in enumerate(ken[1:]))
report("PyMastic vs KENPAVE deflection, max rel diff z=1..29 in (%)", md * 100, 0.0, 1.0, "%", rel=False)

# ---------- M1. mechanistic design strains vs PyMastic (dual wheels superposed) ----------
MA = last("check_pavement_mechanistic"); mi, mr = MA["args"], MA["result"]["result"]
p0 = mi.get("tyre_kpa", 750.0); a0 = math.sqrt(mi.get("wheel_kn", 20.0) / (math.pi * p0)); nu = mi.get("nu", 0.35)
H = [mr["h_ac_m"]] + [l["h_m"] for l in mi["layers"][1:]]
E = [l["e_mpa"] for l in mi["layers"]] + [mi["subgrade_e_mpa"]]
NU = [nu] * len(mi["layers"]) + [mi.get("nu_subgrade", 0.45)]
sp = mi.get("dual_spacing_m", 0.33)
zt, zs = H[0] - 1e-6, sum(H) + 1e-6
pts = [(r_[0], float(r_[1])) for r_ in (l.split(",") for l in open(f"{out}/pavement-mechanistic.csv") if not l.startswith("#")) if len(r_) == 5]
for name, x in pts:
    d1, d2 = abs(x - sp / 2), abs(x + sp / 2)
    R = PyMastic(p0, a0, [d1, d2], [zt, zs], H, E, NU, 1e-9, isBounded=[1] * len(H), iteration=80, inverser="solve")
    # radial direction of both wheels lies along x for points on the axle line; PyMastic: compression positive
    sgn = lambda d, x: 1.0
    ex = -(R["Strain_R"][0, 0] + R["Strain_R"][0, 1]); ey = -(R["Strain_T"][0, 0] + R["Strain_T"][0, 1])
    ez = (R["Strain_Z"][1, 0] + R["Strain_Z"][1, 1])
    row = [r_ for r_ in (l.split(",") for l in open(f"{out}/pavement-mechanistic.csv") if not l.startswith("#")) if r_[0] == name][0]
    report(f"LEA {name}: max horizontal tensile strain bottom of asphalt (µε)", float(row[2]), max(ex, ey) * 1e6, 1.0)
    report(f"LEA {name}: vertical compressive strain top of subgrade (µε)", float(row[3]), ez * 1e6, 1.0)
et = max(float(r_[2]) for r_ in (l.split(",") for l in open(f"{out}/pavement-mechanistic.csv") if not l.startswith("#")) if len(r_) == 5)
ev = max(float(r_[3]) for r_ in (l.split(",") for l in open(f"{out}/pavement-mechanistic.csv") if not l.startswith("#")) if len(r_) == 5)
if mi.get("criteria", "agpt02") != "asphalt_institute":
    report("AGPT02 subgrade N = (9300/εv)^7.14", mr["n_sub"], (9300 / ev) ** 7.14, 0.1)
    report("AGPT02 asphalt N = RF[6918(0.856Vb+1.08)/(E^0.36 εt)]^5", mr["n_ac"], mi["rf"] * (6918 * (0.856 * mi["vb_pct"] + 1.08) / (E[0] ** 0.36 * et)) ** 5, 0.1)

# ---------- W. MSE external stability (GEC 11 ASD) re-derived ----------
WA = last("select_wall_types"); wa = WA["args"]
CORR = json.load(open(f"{out}/corridor.json"))["corridor"]
soil = CORR["assembly"]["wall"]["soil"]
ka = math.tan(math.pi / 4 - math.radians(soil["phi_deg"]) / 2) ** 2
gr, q, gb = wa.get("reinforced_gamma_knm3", 20.0), soil["surcharge_kpa"], soil["gamma_kn_m3"]
qa = wa.get("mse_bearing_kpa", soil["bearing_kpa"])
def mse(h, L):
    f1, f2 = 0.5 * ka * gb * h * h, ka * q * h; V = gr * h * L; mo = f1 * h / 3 + f2 * h / 2
    eb = mo / (V + q * L)
    return soil["base_friction"] * V / (f1 + f2), mo / V, (V + q * L) / (L - 2 * eb)
for r in rcsv("wall-types.csv"):
    if r["selected"] != "MSE": continue
    h, L = float(r["h_m"]), float(r["mse_l_m"]); fs, e, sv = mse(h, L)
    tag = f"MSE {float(r['station']):.0f} {r['side']}"
    report(f"{tag}: sliding FS", float(r["mse_fs_sliding"]), fs, 0.01)
    report(f"{tag}: eccentricity (m)", float(r["mse_e_m"]), e, 1e-3, "m", rel=False)
    report(f"{tag}: bearing σv (kPa)", float(r["mse_sigma_v_kpa"]), sv, 1e-3)
    lmin = max(0.7 * h, 2.4)
    if L - 0.1 >= lmin - 1e-9:
        fs2, e2, sv2 = mse(h, L - 0.1)
        report(f"{tag}: L − 0.1 m fails a check (1 = yes)", float(fs2 < 1.5 or e2 > (L - 0.1) / 6 or sv2 > qa), 1.0, 0, "", rel=False)
    report(f"{tag}: selected because H > cantilever range (m spare)", h - wa.get("cantilever_max_h_m", 8.0) > 0, True, 0, "", rel=False)
cant = [float(r["h_m"]) for r in rcsv("wall-types.csv") if r["selected"] == "cantilever"]
report("no cantilever wall above the practical range (max H, m)", max(cant) if cant else 0.0, min(max(cant) if cant else 0.0, wa.get("cantilever_max_h_m", 8.0)), 0, "m", rel=False)

# ---------- W2. cantilever stem shear, crack control, footing heel/toe (re-derived) ----------
RA_ = last("design_wall_reinforcement")["args"]
ka2 = math.tan(math.pi / 4 - math.radians(RA_["phi_soil_deg"]) / 2) ** 2
secs = {(s_["station"], k): s_[("left" if k == "L" else "right")]["wall"] for s_ in CORR["sections"] for k in ("L", "R") if s_[("left" if k == "L" else "right")].get("wall")}
for r in rcsv("wall-reinforcement.csv"):
    hs, d = float(r["stem_h_m"]), float(r["d_m"]); tag = f"wall {float(r['station']):.0f} {r['side']}"
    V = RA_["load_factor"] * (0.5 * ka2 * RA_["gamma_knm3"] * hs * hs + ka2 * RA_.get("surcharge_kpa", 0) * hs)
    dv = max(0.9 * d, 0.72 * (d + RA_["cover_m"] + RA_["bar_d_mm"] / 2000))
    report(f"{tag}: stem shear Vu (kN/m)", float(r["vu_kn_per_m"]), V, 0.01)
    report(f"{tag}: φVc = 0.9·0.166√f'c·b·dv (kN/m)", float(r["phi_vc_kn_per_m"]), 0.9 * 0.166 * math.sqrt(RA_["fc_mpa"]) * 1000 * dv, 0.01)
    t_ = float(r["t_m"]); dc = (t_ - d) * 1000; Ms = float(r["mu_knm_per_m"]) / RA_["load_factor"]; As = float(r["as_mm2_per_m"]) / 1e6
    fss = min(Ms / (As * 0.875 * d) / 1000, 0.6 * RA_["fy_mpa"]); bs = 1 + dc / (0.7 * (t_ * 1000 - dc))
    report(f"{tag}: crack-control max spacing (mm)", float(r["s_crack_mm"]), 123000 * RA_.get("crack_gamma_e", 0.75) / (bs * fss) - 2 * dc, 0.2)
    w_ = secs[(float(r["station"]), r["side"])]["design"]
    B, e, Vv, tb, heel, toe = w_["base_b"], w_["eccentricity"], w_["sum_v_kn"], w_["base_t"], w_["heel"], w_["toe"]
    from scipy.integrate import quad
    qf = (lambda x: Vv / B + 6 * Vv * e / B ** 2 - 12 * Vv * e * x / B ** 3) if e <= B / 6 else (lambda x: max(2 * Vv / (3 * (B / 2 - e)) * (1 - x / (3 * (B / 2 - e))), 0))
    wd = RA_["gamma_knm3"] * (w_["h"] - tb) + RA_.get("surcharge_kpa", 0) + 24 * tb
    mh = quad(lambda x: (wd - qf(x)) * (x - (B - heel)), B - heel, B)[0]
    mt = quad(lambda x: (qf(x) - 24 * tb) * (toe - x), 0, toe)[0]
    report(f"{tag}: footing heel Mu (kNm/m, quad)", float(r["mu_heel_knm_per_m"]), RA_["load_factor"] * max(mh, 0), 0.05)
    report(f"{tag}: footing toe Mu (kNm/m, quad)", float(r["mu_toe_knm_per_m"]), RA_["load_factor"] * max(mt, 0), 0.05)

# ---------- S. signals: MUTCD Warrant 1, Webster, HCM delay, ITE intergreen ----------
SA = last("design_signals"); w, t = SA["args"]["warrants"], SA["args"]["timing"]; res = SA["result"]["timing"]
base = {(False, False): (500, 150, 750, 75), (True, False): (600, 150, 900, 75), (True, True): (600, 200, 900, 100), (False, True): (500, 200, 750, 100)}[(w["major_lanes"] >= 2, w["minor_lanes"] >= 2)]
f = 0.7 if (w["major_speed_kmh"] > 64.4 or w.get("isolated_community")) else 1.0
ha = sum(1 for h in w["hours"] if h[0] >= round(base[0] * f) and h[1] >= round(base[1] * f))
report("MUTCD Warrant 1 Condition A hours", SA["result"]["warrant"]["hours_a"], ha, 0, "h", rel=False)
L = len(res["phases"]) * t.get("lost_per_phase_s", 4.0); Y = sum(p["y"] for p in res["phases"])
report("Webster C0 = (1.5L+5)/(1−Y) (s)", res["cycle_webster_s"], (1.5 * L + 5) / (1 - Y), 1e-6)
C = min(max((1.5 * L + 5) / (1 - Y), t.get("min_cycle_s", 40)), t.get("max_cycle_s", 120))
s = t["sat_flow_pcuhpl"]
maj = [a for a in t["approaches"] if a["major"]]; mnr = [a for a in t["approaches"] if not a["major"]]
ymaj = max((a["through_vph"] + a["right_vph"] + a["left_vph"] * t.get("permissive_left_pcu", 2.0)) / (a["lanes"] * s) for a in maj)
report("major phase flow ratio y (permissive lefts in pcu)", res["phases"][-2]["y"], ymaj, 1e-9)
for p in res["phases"]:
    g = (C - L) * p["y"] / Y; X = p["y"] * C / g; cap = p["volume_vph"] / X
    d1 = 0.5 * C * (1 - g / C) ** 2 / (1 - min(1, X) * g / C); d2 = 900 * 0.25 * ((X - 1) + math.sqrt((X - 1) ** 2 + 8 * 0.5 * X / (cap * 0.25)))
    report(f"signal {p['name']}: green (s)", p["green_s"], g, 1e-6)
    report(f"signal {p['name']}: HCM delay d1+d2 (s)", p["delay_s"], d1 + d2, 1e-6)
v = max(a["speed_kmh"] for a in (maj if p["name"] == "major" else mnr)) / 3.6
report("ITE yellow t + v/2a (minor phase, s)", res["phases"][-1]["yellow_s"], 1 + max(a["speed_kmh"] for a in mnr) / 3.6 / (2 * 3.05), 1e-9)

# ---------- C. capacity: f_HV, v/c, TWSC gap acceptance ----------
CA = last("analyse_capacity"); l = CA["args"]["link"]; cr = CA["result"]
fhv = 1 / (1 + l["heavy_pct"] / 100 * (l["e_t"] - 1)); vp = l["peak_hour_vph"] * l["direction_split"] / (l["phf"] * fhv)
report("link v_p (pc/h)", cr["link"]["v_p"], vp, 1e-9)
report("link v/c", cr["link"]["v_c"], vp / (l["capacity_pcphpl"] * l["lanes_per_direction"]), 1e-9)
for m, rm in zip(CA["args"]["priority"]["movements"], cr["movements"]):
    vc, tc, tf = m["conflicting_vph"], m["tc_s"], m["tf_s"]
    c = vc * math.exp(-vc * tc / 3600) / (1 - math.exp(-vc * tf / 3600)); x = m["volume_vph"] / c
    d = 3600 / c + 900 * 0.25 * ((x - 1) + math.sqrt((x - 1) ** 2 + (3600 / c) * x / (450 * 0.25))) + 5
    report(f"TWSC {m['name']}: capacity (veh/h)", rm["capacity_vph"], c, 1e-9)
    report(f"TWSC {m['name']}: control delay (s)", rm["delay_s"], d, 1e-9)

# ---------- R. rigid pavement: Westergaard edge stress, PCA fatigue and erosion ----------
RA = last("design_rigid_pavement"); ri, rr = RA["args"], RA["result"]["result"]
def rigid(h):
    k = ri["k_mpa_m"] / 1000; Ec, nu_ = ri.get("e_mpa", 30000), ri.get("nu", 0.15)
    l_ = (Ec * h ** 3 / (12 * (1 - nu_ ** 2) * k)) ** 0.25; fd = ed = 0; out_ = []
    for g in ri["groups"]:
        P = g["load_kn"] * 1000 * ri["lsf"] / g["tyres"]; a = math.sqrt(P / (math.pi * ri.get("tyre_kpa", 750) / 1000))
        sig = 3 * (1 + nu_) * P / (math.pi * (3 + nu_) * h * h) * (math.log(Ec * h ** 3 / (100 * k * a ** 4)) + 1.84 - 4 * nu_ / 3 + (1 - nu_) / 2 + 1.18 * (1 + 2 * nu_) * a / l_)
        sr = sig / ri["fcf_mpa"]
        nf = 10 ** ((0.9719 - sr) / 0.0828) if sr > 0.55 else ((4.258 / (sr - 0.4325)) ** 3.268 if sr > 0.45 else math.inf)
        wc = P / (k * l_ * l_) * (1.1 - 0.88 * a * math.sqrt(2) / l_); pc = k * wc * 145.0377
        hin, kp = h / 25.4, ri["k_mpa_m"] * 3.684; pw = 268.7 * pc * pc / (hin * kp ** 0.73); c1 = 1 - (kp / 2000 * 4 / hin) ** 2
        ne = 10 ** (14.524 - 6.777 * (c1 * pw - 9) ** 0.103) if c1 * pw > 9 else math.inf
        fd += 100 * g["repetitions"] / nf; ed += 100 * (0.94 if ri.get("tied_shoulder") else 0.06) * g["repetitions"] / ne; out_.append(sig)
    return fd, ed, out_
fd, ed, sg = rigid(rr["h_mm"])
report("rigid erosion damage at design thickness (%)", rr["erosion_pct"], ed, 1e-6)
report("rigid fatigue damage at design thickness (%)", rr["fatigue_pct"], fd, 1e-6, "%", rel=False)
for g, s_ in zip(rr["groups"], sg):
    report(f"rigid {g['name']}: Westergaard edge stress (MPa)", g["stress_mpa"], s_, 1e-9)
if rr["h_mm"] - 5 >= ri.get("h_min_mm", 150):
    f2, e2, _ = rigid(rr["h_mm"] - 5)
    report("rigid h − 5 mm exceeds 100 % damage (1 = yes)", float(f2 > 100 or e2 > 100), 1.0, 0, "", rel=False)

# ---------- L. landscape: slope classes from corridor sections ----------
LA = last("landscape_concept"); lr = LA["result"]
corr = CORR
def per(sd):
    v = [0.0, 0.0, 0.0]
    if sd.get("wall") or len(sd["fg"]) < 2: return v
    p, q = sd["fg"][-2], sd["fg"][-1]; dx, dz = q[0] - p[0], q[1] - p[1]
    r = abs(dx) / max(abs(dz), 1e-9); v[0 if r >= 4 else (1 if r >= 2 else 2)] += math.hypot(dx, dz); return v
A = [0.0, 0.0, 0.0]; S = corr["sections"]
for s0, s1 in zip(S, S[1:]):
    L_ = s1["station"] - s0["station"]
    for k in ("left", "right"):
        a0, a1 = per(s0[k]), per(s1[k])
        for i in range(3): A[i] += (a0[i] + a1[i]) / 2 * L_
for i, k in enumerate(["seed", "shrub_blanket", "hydroseed_trm"]):
    report(f"landscape area {k} (m2)", lr["areas_m2"][k], A[i], 1e-6, "m2", rel=False)

# ---------- F. foundation surface modulus by PyMastic ----------
FA = last("design_foundation"); fa, fr = FA["args"], FA["result"]
Hs = [l["h_m"] for l in fr["layers"]]; Es = [l["e_mpa"] for l in fr["layers"]] + [fr["subgrade_e_mpa"]]
if "subgrade_cbr_pct" in fa:
    report("foundation subgrade E = 17.6 CBR^0.64 (MPa)", fr["subgrade_e_mpa"], 17.6 * fa["subgrade_cbr_pct"] ** 0.64, 1e-9)
qa_, ra_ = fa.get("plate_kpa", 100.0), fa.get("plate_radius_m", 0.15)
R = PyMastic(qa_, ra_, [0], [0.0], Hs, Es, [0.35] * len(Hs) + [0.45], 1e-9, isBounded=[1] * len(Hs), iteration=40, inverser="solve")
w0 = R["Displacement_Z"][0, 0] * 1000  # m -> mm (E in MPa, q in kPa)
report("foundation surface deflection w0 vs PyMastic (mm)", fr["w0_mm"], w0, 1.0)
report("foundation E0 = 2(1-ν²)qa/w0 (MPa)", fr["e0_mpa"], 2 * (1 - 0.35 ** 2) * qa_ * ra_ / (w0 / 1000) / 1000, 1.0)

# ---------- G. locality: independent forward UTM (Krüger n-series, Karney 2011) of the reported start ----------
LP = last("locality_plan")["result"]
def utm(lat, lon, zone):
    a, f = 6378137.0, 1 / 298.257223563; n = f / (2 - f); k0 = 0.9996
    A_ = a / (1 + n) * (1 + n ** 2 / 4 + n ** 4 / 64)
    al = [n / 2 - 2 * n ** 2 / 3 + 5 * n ** 3 / 16, 13 * n ** 2 / 48 - 3 * n ** 3 / 5, 61 * n ** 3 / 240]
    ph, lm = math.radians(lat), math.radians(lon - (zone * 6 - 183))
    e = math.sqrt(f * (2 - f)); t = math.sinh(math.atanh(math.sin(ph)) - e * math.atanh(e * math.sin(ph)))
    xi, eta = math.atan2(t, math.cos(lm)), math.atanh(math.sin(lm) / math.sqrt(1 + t * t))
    x = eta + sum(al[j] * math.cos(2 * (j + 1) * xi) * math.sinh(2 * (j + 1) * eta) for j in range(3))
    y = xi + sum(al[j] * math.sin(2 * (j + 1) * xi) * math.cosh(2 * (j + 1) * eta) for j in range(3))
    return 500000 + k0 * A_ * x, k0 * A_ * y
E_, N_ = utm(LP["start"]["lat"], LP["start"]["lon"], LP["zone"])
report("locality: Krüger forward of reported lat/lon vs alignment start E (m)", E_, LP["start"]["e"], 0.01, "m", rel=False)
report("locality: Krüger forward of reported lat/lon vs alignment start N (m)", N_, LP["start"]["n"], 0.01, "m", rel=False)
gj = json.load(open(os.path.join(here, "..", LP["basemap"])))
report("locality: OSM basemap attribution present (1 = yes)", float("OpenStreetMap" in LP["attribution"] and "ODbL" in gj.get("license", "") + gj.get("attribution", "")), 1.0, 0, "", rel=False)

print(f"\n{len(rows)} v6 checks, {len(fails)} outside tolerance")
json.dump([dict(zip(["check", "civil_mcp", "independent", "diff", "unit", "ok"], r)) for r in rows], open(f"{out}/independent-check-v6.json", "w"), indent=1)
sys.exit(1 if fails else 0)
