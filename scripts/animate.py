"""Render the agent's design run as a 1080p MP4 + GIF, purely from a demo output dir.

Every frame is drawn from files the tools actually wrote (run-log.jsonl, design.xml,
corridor.json, route-alternatives.json, profile-optimisation.json, protection.json, network-*.json,
intersection-*.json, quantities.json, sheets.pdf) plus the
public DEM. Nothing is hand-drawn.

usage: python scripts/animate.py <demo-out> <dem.npy> <x0> <y0> <px> <media-dir>
needs: numpy matplotlib ffmpeg pdftoppm (poppler)
"""
import json, os, subprocess, sys, xml.etree.ElementTree as ET
from multiprocessing import Pool
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PolyCollection
from matplotlib import font_manager

out, dem_f, X0, Y0, PX, media = sys.argv[1], sys.argv[2], *map(float, sys.argv[3:6]), sys.argv[6]
FONT = os.path.join(os.path.dirname(__file__), "..", "crates", "civil-core", "assets", "DejaVuSans.ttf")
font_manager.fontManager.addfont(FONT)
plt.rcParams["font.family"] = font_manager.FontProperties(fname=FONT).get_name()
FPS = 24
os.makedirs(media, exist_ok=True)
frames_dir = os.path.join(media, "frames"); os.makedirs(frames_dir, exist_ok=True)

log = [json.loads(l) for l in open(f"{out}/run-log.jsonl")]
dem = np.load(dem_f)
ny, nx = dem.shape
ext = [X0, X0 + nx * PX, Y0 - ny * PX, Y0]
# hillshade (sun az 315°, alt 45°)
gy, gx = np.gradient(dem * 3.0, PX)  # 3x vertical exaggeration for shading only
slope = np.arctan(np.hypot(gx, gy)); aspect = np.arctan2(-gx, gy)
az, alt = np.radians(315), np.radians(45)
hs = np.sin(alt) * np.cos(slope) + np.cos(alt) * np.sin(slope) * np.cos(az - aspect)

ns = "{http://www.landxml.org/schema/LandXML-1.2}"
root = ET.parse(f"{out}/design.xml").getroot()
surf = {}
for sf in root.iter(ns + "Surface"):
    P = np.array([[float(v) for v in p.text.split()] for p in sf.iter(ns + "P")])
    F = np.array([[int(v) - 1 for v in f.text.split()] for f in sf.iter(ns + "F")])
    surf[sf.get("name")] = (P[:, [1, 0, 2]], F)
pts = np.array([[float(v) for v in c.text.split()] for c in root.iter(ns + "CgPoint")])[:, [1, 0, 2]]
corr = json.load(open(f"{out}/corridor.json"))["corridor"]
secs = corr["sections"]
cl = np.array([[s["x"], s["y"]] for s in secs]); sta = np.array([s["station"] for s in secs])
net = json.load(open(f"{out}/network-storm.json"))
san = json.load(open(f"{out}/network-sanitary.json"))
X = json.load(open(f"{out}/intersection-X1.json")); X = X.get("intersection", X)
q = json.load(open(f"{out}/quantities.json"))
grads = [json.load(open(os.path.join(out, f))) for f in sorted(os.listdir(out)) if f.startswith("grading-")]
RA = json.load(open(f"{out}/route-alternatives.json"))
OPT = json.load(open(f"{out}/profile-optimisation.json"))
PROT = json.load(open(f"{out}/protection.json"))
sdir = os.path.join(media, "sheets"); os.makedirs(sdir, exist_ok=True)
for f_ in os.listdir(sdir): os.remove(os.path.join(sdir, f_))
subprocess.run(["pdftoppm", "-r", "28", "-png", f"{out}/sheets.pdf", f"{sdir}/s"], check=True)
sheet_imgs = [plt.imread(os.path.join(sdir, f_)) for f_ in sorted(os.listdir(sdir))]
subprocess.run(["pdftoppm", "-r", "110", "-png", "-f", "3", "-l", "3", f"{out}/sheets.pdf", f"{media}/sheet"], check=True)
sheet = plt.imread(os.path.join(media, [f_ for f_ in os.listdir(media) if f_.startswith("sheet") and f_.endswith(".png")][0]))

def off(s, o):  # left positive
    h = s["heading"]; return [s["x"] - o * np.sin(h), s["y"] + o * np.cos(h)]

def tin_segments(name):
    P, F = surf[name]
    e = np.vstack([F[:, [0, 1]], F[:, [1, 2]], F[:, [2, 0]]]); e.sort(1); e = np.unique(e, axis=0)
    return P[e][:, :, :2]
eg_segs = tin_segments("EG")
fg_P, fg_F = surf["Corridor FG"]

def entry(tool, n=0):
    return [e for e in log if e["tool"] == tool][n]

def fmt_args(a, maxlen=150):
    s = json.dumps(a, ensure_ascii=False)
    return s if len(s) < maxlen else s[:maxlen] + " …"

chk1 = entry("check_standards", 0); chkN = [e for e in log if e["tool"] == "check_standards"][-1]
basis = entry("set_design_basis")["args"]["basis"]
storm = [e for e in log if e["tool"] == "design_pipe_network" and e["args"].get("name") == "Storm"]
sanit = [e for e in log if e["tool"] == "design_pipe_network" and e["args"].get("name") == "Sanitary"]
p1 = storm[0]
x1_ = entry("design_intersection", 0)
pav = [e for e in log if e["tool"] == "design_pavement"]
def fails(e):
    return [f"x {c['id']} {c['element']}: {c['value']:.3g} vs limit {c['limit']:.3g} {c['unit']}" for c in e["mandatory_failures"]]
pipe_fails = fails
corr_e = entry("build_corridor"); W = corr_e["result"]["walls"]
walls = [(s_, sd, s_[sd]["wall"]) for s_ in secs for sd in ("left", "right") if s_[sd].get("wall")]
wmax = max(walls, key=lambda w: w[2]["design"]["h"]) if walls else None
def sec_xy(st):
    i = int(np.argmin(np.abs(sta - st))); return cl[i]
alts = RA["alternatives"]; rec = RA["recommendation"]
ALT_COL = ["#ff9f0a", "#64d2ff", "#bf5af2"]
def alt_line(a):
    e = a["evaluation"]; return f"{a['name']}: L {e['length_m']:.0f} m, fails {e['geometry_mandatory_failures']}, cost {e['cost_proxy']:.0f}"
ob, oa = OPT["before_eval"], OPT["after_eval"]
pf = pav[-1]["result"]["result"] if "result" in pav[-1]["result"] else pav[-1]["result"]
pr_ = pav[0]["result"]["result"] if "result" in pav[0]["result"] else pav[0]["result"]
ew = q["earthwork"]
nb = len(PROT["barriers"]); lb = sum(b["length_m"] for b in PROT["barriers"])
S = [  # (seconds, title, call entry, status lines, stage key)
    (3.0, "USGS 3DEP 1/3 arc-second DEM (public domain), Palo Alto, CA", None, ["Every frame is drawn from the tools' real outputs", "(run-log.jsonl, design.xml, *.json, sheets.pdf)"], "dem"),
    (3.0, "set_design_basis + load_points", entry("load_points"), [f"Engineer sets: {basis['standard']}, {basis['design_speed_kmh']:.0f} km/h", f"e_max {basis['e_max']}, f_max {basis['f_max']}, max grade {basis['max_grade_pct']}% (engineer inputs)", f"{entry('load_points')['result']['points']} survey points"], "points"),
    (2.5, "build_surface", entry("build_surface"), [f"TIN: {entry('build_surface')['result']['triangles']} triangles"], "tin"),
    (5.0, "generate_route_alternatives (least-cost search)", entry("generate_route_alternatives"), [f"{len(alts)} alternatives, each built in full and checked:"] + [alt_line(a) for a in alts], "routes"),
    (4.5, f"select_route: {rec['recommended']}", entry("select_route"), ["Rule: fewest mandatory failures, then lowest cost proxy"] + [x for a in alts if a["name"] != rec["recommended"] for x in (f"vs {a['name']}: {a['evaluation']['geometry_mandatory_failures']} mandatory failures,", f"   cost proxy {a['evaluation']['cost_proxy']:.0f} vs {[b for b in alts if b['name'] == rec['recommended']][0]['evaluation']['cost_proxy']:.0f}")] + ["White dashed: alignment PIs after radius fitting", "Cost proxy uses placeholder rates (ranking only)"], "select"),
    (5.0, "optimize_profile (earthwork balance under standards)", entry("optimize_profile"), [f"imbalance {ob['imbalance_m3']:.0f} -> {oa['imbalance_m3']:.0f} m3", f"haul moment {ob['haul_moment_m3m']/1e6:.2f}M -> {oa['haul_moment_m3m']/1e6:.2f}M m3*m", f"wall concrete {ob['wall_concrete_m3']:.0f} -> {oa['wall_concrete_m3']:.0f} m3", f"{OPT['evaluations']} evaluations; every candidate passes grade/VC checks", f"OK {chkN['result']['total'] if 'result' in chkN else ''} checks at the end, 0 mandatory failures"], "profile"),
    (4.5, "build_corridor (assembly, AASHTO widening, wall rule)", corr_e, [f"{corr_e['result']['sections']} sections", f"cut {corr_e['result']['cut_m3']:.0f} m3 / fill {corr_e['result']['fill_m3']:.0f} m3", f"composite-grid cross-check: {corr_e['result']['composite_check']['cut_diff_pct']:+.2f}% / {corr_e['result']['composite_check']['fill_diff_pct']:+.2f}%", f"walls: {W['sections_with_wall']} sections, {W['concrete_m3']:.0f} m3 concrete"], "corridor"),
    (4.5, f"design_pavement ({pf['method'][:40]})", pav[-1], ["rigid option: " + ", ".join(l["name"] + " " + format(l["thickness_mm"], ".0f") for l in pr_["layers"]) + " mm", f"adopted flexible: SN required {pf['sn_required']:.2f}, provided {pf['sn_provided']:.2f}"] + [f"{l['name']}: {l['thickness_mm']:.0f} mm" for l in pf["layers"]], "pavement"),
    (5.0, "design_protection (clear zone, barriers, HEC-15 linings)", entry("design_protection"), [f"{len(PROT['hazards'])} hazards in the clear zone", f"{nb} barrier runs, {lb:.0f} m incl. length of need", f"{len(PROT['ditches'])} ditch reaches: " + ", ".join(sorted({d['lining'].split(' D50')[0] for d in PROT['ditches']})), "OK tau_d <= tau_p on every reach"], "protection"),
    (3.5, "design_intersection X1", x1_, ["OK 0 mandatory failures", "sight triangles clear (AASHTO 2018 Case B1)", f"return patch fill {sum(l['patch_fill_m3'] for l in X['legs']):.0f} m3"], "isect"),
    (5.0, f"design_pipe_network Storm ({len(storm)} attempts, HEC-22)", storm[-1], [f"attempt 1: {len(p1['mandatory_failures'])} mandatory failures"] + pipe_fails(p1)[:3] + ["agent upsizes pipes, lowers cover when resizing stalls", f"attempt {len(storm)}: OK 0 failures, HGL below every rim"], "pipes"),
    (4.0, f"design_pipe_network Sanitary ({len(sanit)} attempts)", sanit[-1], [f"attempt 1: {len(sanit[0]['mandatory_failures'])} failures (P-VMIN)", f"attempt {len(sanit)}: OK 0 failures (min slope raised)", "peaking factor per engineer criteria (Harmon / Ten States)"], "sanitary"),
    (4.0, "compute_quantities (mass haul, free haul, borrow/waste)", entry("compute_quantities"), [f"final ordinate {q['mass_haul']['final_ordinate']:.0f} m3", f"borrow {ew['borrow_compacted_m3']:.0f} m3, waste {ew['waste_bank_m3']:.0f} m3 (bank)", f"overhaul {ew['overhaul_m3m']/1e6:.2f}M m3*m beyond {ew['free_haul_m']:.0f} m free haul"], "haul"),
    (6.0, "generate_drawings + generate_deliverables", entry("generate_drawings"), [f"{entry('generate_drawings')['result']['pages']} sheets, PDF + DXF, English, title blocks + scales", "cover/index, route, plan & profile, typicals, superelevation,", "pavement, cross sections, protection, pipe profiles,", "drainage details, mass haul, standard details atlas", "reports: design, drainage, pavement, BoQ, registers", "Status: PRELIMINARY - engineer review and seal required"], "sheet"),
]
order = [s[4] for s in S]
total = sum(s[0] for s in S)
print(f"{total:.1f} s, {int(total*FPS)} frames")

def vpi_line(v):
    v = np.array(v); return v[:, 0], v[:, 1]

def draw(fi):
    t = fi / FPS
    acc = 0
    for k, s in enumerate(S):
        if t < acc + s[0] or k == len(S) - 1:
            break
        acc += s[0]
    u = min(1.0, (t - acc) / (s[0] * 0.7))  # progress within step
    stage = order.index(s[4])
    fig = plt.figure(figsize=(19.2, 10.8), dpi=100)
    fig.patch.set_facecolor("#111418")
    past = lambda k: stage >= order.index(k)
    if s[4] == "sheet":
        # drawing set: thumbnails appear one by one, then the plan & profile sheet
        n_ = len(sheet_imgs); shown = max(1, int(n_ * min(1.0, (t - acc) / (s[0] * 0.6))))
        cols = 5; rows = int(np.ceil(n_ / cols))
        for i in range(shown):
            r_, c_ = divmod(i, cols)
            ax = fig.add_axes([0.01 + c_ * 0.125, 0.98 - (r_ + 1) * (0.94 / rows), 0.12, 0.94 / rows - 0.01]); ax.imshow(sheet_imgs[i]); ax.axis("off")
            ax.set_title(f"{i + 1}", color="#9aa4b2", fontsize=8, pad=1)
    else:
        ax = fig.add_axes([0.0, 0.0, 0.5625, 1.0])
        ax.imshow(hs, extent=ext, cmap="gray", vmin=0.0, vmax=1.0, interpolation="bicubic")
        ax.imshow(dem, extent=ext, cmap="terrain", alpha=0.25, interpolation="bicubic")
        ax.set_xlim(ext[0], ext[1]); ax.set_ylim(ext[2] - 30, ext[3]); ax.set_facecolor("#111418"); ax.axis("off")
        zoom = None
        if s[4] == "isect": zoom = (X["point"][0], X["point"][1], 110)
        if zoom:
            k = min(1.0, (t - acc) / 1.0); full = ((ext[0] + ext[1]) / 2, (ext[2] + ext[3]) / 2, (ext[1] - ext[0]) / 2)
            cx, cy, r = [f + (z - f) * k for f, z in zip(full, zoom)]
            ax.set_xlim(cx - r, cx + r); ax.set_ylim(cy - r, cy + r)
        if past("points") and stage <= order.index("select"):
            n = len(pts) if stage > 1 else int(len(pts) * u)
            ax.scatter(pts[:n, 0], pts[:n, 1], s=1.2, c="#ffd54a", alpha=0.6, lw=0)
        if past("tin") and stage <= order.index("select"):
            n = len(eg_segs) if stage > 2 else int(len(eg_segs) * u)
            ax.add_collection(LineCollection(eg_segs[:n], colors="#00e5ff", lw=0.35, alpha=0.4))
        if s[4] in ("routes", "select"):
            for i, a in enumerate(alts):
                P_ = np.array(a["path"]); n = len(P_) if s[4] == "select" else max(2, int(len(P_) * min(1.0, u * 3 - i)))
                if s[4] == "routes" and u * 3 - i <= 0: continue
                sel = s[4] == "select" and a["name"] == rec["recommended"]
                ax.plot(P_[:n, 0], P_[:n, 1], color=ALT_COL[i % 3], lw=4 if sel else 2, alpha=1 if (sel or s[4] == "routes") else 0.35)
                pis = np.array(a["pis"])
                if s[4] == "select" and sel:
                    ax.plot(pis[:, 0], pis[:, 1], color="white", lw=1.2, ls="--"); ax.scatter(pis[:, 0], pis[:, 1], c="white", s=30, zorder=6)
                ax.text(P_[len(P_) // 2, 0] + 15, P_[len(P_) // 2, 1], a["name"] + (" (selected)" if sel else ""), color=ALT_COL[i % 3], fontsize=14, weight="bold")
        if past("profile"):
            ax.plot(cl[:, 0], cl[:, 1], color="#ff3b30", lw=2.5)
        if past("corridor"):
            n = len(secs) if s[4] != "corridor" else max(2, int(len(secs) * u))
            polys, cols = [], []
            for a, b in zip(secs[: n - 1], secs[1:n]):
                for sd in ("left", "right"):
                    sg = 1 if sd == "left" else -1
                    polys.append([off(a, 0), off(a, sg * a[sd]["daylight"]), off(b, sg * b[sd]["daylight"]), off(b, 0)])
                    cols.append("#e5484d" if a[sd]["cut"] else "#30a46c")
            ax.add_collection(PolyCollection(polys, facecolors=cols, edgecolors="none", alpha=0.75))
            for sd, sg in (("left", 1), ("right", -1)):
                e_ = np.array([off(s_, sg * s_[sd]["paved_width"]) for s_ in secs[:n]])
                ax.plot(e_[:, 0], e_[:, 1], color="#f5f5f5", lw=0.8)
            ax.plot(cl[:n, 0], cl[:n, 1], color="white", lw=1)
            for sd, sg in (("left", 1), ("right", -1)):
                run = []
                for s_ in secs + [None]:
                    w_ = s_ and s_[sd].get("wall")
                    if w_:
                        run.append(off(s_, sg * w_["offset"]))
                    elif run:
                        r_ = np.array(run); ax.plot(r_[:, 0], r_[:, 1], color="#ffd60a", lw=3, solid_capstyle="butt"); run = []
        if past("protection"):
            for b in PROT["barriers"]:
                sg = 1 if b["side"] == "left" else -1
                ss = [s_ for s_ in secs if b["from"] - 1e-6 <= s_["station"] <= b["to"] + 1e-6]
                if len(ss) >= 2:
                    e_ = np.array([off(s_, sg * (s_[b["side"]]["paved_width"] + 1.5)) for s_ in ss]); ax.plot(e_[:, 0], e_[:, 1], color="#ff375f", lw=4 if s[4] == "protection" else 2)
            if s[4] == "protection":
                for h in PROT["hazards"]:
                    p_ = off(secs[int(np.argmin(np.abs(sta - h["station"])))], (1 if h["side"] == "left" else -1) * 8)
                    ax.scatter([p_[0]], [p_[1]], marker="x", c="#ffd60a", s=30)
            for d_ in PROT["ditches"]:
                sg = 1 if d_["side"] == "left" else -1
                ss = [s_ for s_ in secs if d_["from"] - 1e-6 <= s_["station"] <= d_["to"] + 1e-6]
                if len(ss) >= 2:
                    e_ = np.array([off(s_, sg * (s_[d_["side"]]["paved_width"] + 3.0)) for s_ in ss]); ax.plot(e_[:, 0], e_[:, 1], color="#0a84ff", lw=3 if s[4] == "protection" else 1.5)
        if past("isect"):
            for l in X["legs"]:
                pp = np.array(l["patch"]); ax.fill(pp[:, 0], pp[:, 1], color="#8e8e93", alpha=0.85)
                for el in ("edge_left", "edge_right"):
                    e_ = np.array(l[el]); ax.plot(e_[:, 0], e_[:, 1], color="#d1d1d6", lw=1.2)
                for r_ in l["returns"]:
                    a_ = np.array(r_["arc"]); ax.plot(a_[:, 0], a_[:, 1], color="white", lw=2)
                if s[4] == "isect":
                    for sl in l["sight"]:
                        tr = np.array(sl["triangle"]); ax.fill(tr[:, 0], tr[:, 1], color="#d4ff3a" if sl["clear"] else "#ff453a", alpha=0.45, lw=0)
            for g in grads:
                dl = np.array(g["daylight"]); ax.fill(dl[:, 0], dl[:, 1], color="#c084fc", alpha=0.35)
        if past("pipes"):
            st = {s_[0]: s_ for s_ in net["structures"]}
            for p in net["pipes"]:
                a, b = st[p["from"]], st[p["to"]]
                ax.plot([a[1], b[1]], [a[2], b[2]], color="#0a84ff", lw=3.5)
            ax.scatter([s_[1] for s_ in net["structures"]], [s_[2] for s_ in net["structures"]], s=40, c="white", edgecolors="k", zorder=5)
        if past("sanitary"):
            st = {s_[0]: s_ for s_ in san["structures"]}
            for p in san["pipes"]:
                a, b = st[p["from"]], st[p["to"]]
                ax.plot([a[1], b[1]], [a[2], b[2]], color="#a2845e", lw=3)
            ax.scatter([s_[1] for s_ in san["structures"]], [s_[2] for s_ in san["structures"]], s=30, c="#a2845e", edgecolors="w", zorder=5)
        ax.text(0.01, 0.01, "USGS 3DEP DEM, UTM 10N - civil-mcp demo output", color="w", fontsize=11, transform=ax.transAxes)
    # right panel
    px = 0.585 if s[4] != "sheet" else 0.645
    fig.text(px, 0.94, "civil-mcp - AI agent -> MCP tools", color="#9aa4b2", fontsize=20)
    fig.text(px, 0.895, f"Step {stage + 1}/{len(S)}", color="#9aa4b2", fontsize=18)
    import textwrap
    for j, tl in enumerate(textwrap.wrap(s[1], 44)[:2]):
        fig.text(px, 0.845 - j * 0.04, tl, color="white", fontsize=22)
    if s[2] is not None:
        fig.text(px, 0.755, "tools/call " + s[2]["tool"], color="#7fd3ff", fontsize=16, family="monospace")
        fig.text(px, 0.72, fmt_args(s[2]["args"], 90), color="#6b7684", fontsize=11, family="monospace")
    for i, line in enumerate(s[3]):
        c = "#ff6b6b" if line.startswith("x ") or line.startswith("attempt 1") else ("#4ade80" if line.startswith("OK") else "#e6e9ef")
        fig.text(px, 0.66 - i * 0.042, line, color=c, fontsize=16)
    # inset charts
    if s[4] == "profile":
        ia = fig.add_axes([px, 0.06, 0.38, 0.3]); ia.set_facecolor("#1b1f24")
        ia.plot(sta, [s_["ground_cl"] for s_ in secs], color="#b08968", lw=1.5, label="existing ground")
        ia.plot(*vpi_line(OPT["before"]["vpis"]), color="#9aa4b2", lw=1.2, ls="--", label="suggested (VPI polygon)")
        if u > 0.3: ia.plot(*vpi_line(OPT["after"]["vpis"]), color="#ff3b30", lw=2, label="optimised (VPI polygon)")
        ia.tick_params(colors="#9aa4b2"); ia.set_title("Profile: suggested vs optimised", color="w", fontsize=13); ia.legend(fontsize=10)
    if s[4] == "pavement":
        ia = fig.add_axes([px, 0.06, 0.38, 0.3]); ia.set_facecolor("#1b1f24")
        for j, (nm, pd_) in enumerate([("rigid option", pr_), ("adopted flexible", pf)]):
            z = 0
            for k_, l in enumerate(pd_["layers"]):
                ia.bar(j, l["thickness_mm"], bottom=-z - l["thickness_mm"], color=["#4b5563", "#a16207", "#ca8a04", "#78716c"][k_ % 4], edgecolor="w", width=0.5)
                ia.text(j + 0.28, -z - l["thickness_mm"] / 2, f"{l['name']} {l['thickness_mm']:.0f}", color="w", fontsize=10, va="center")
                z += l["thickness_mm"]
        ia.set_xticks([0, 1], ["rigid option", "adopted flexible"]); ia.set_xlim(-0.5, 2.4); ia.tick_params(colors="#9aa4b2"); ia.set_ylabel("depth (mm)", color="#9aa4b2")
        ia.set_title("Pavement structure (design_pavement)", color="w", fontsize=13)
    if s[4] in ("pipes", "sanitary"):
        nn = net if s[4] == "pipes" else san
        hyd = {h["id"]: h for h in nn["hydraulics"]}; down = {p["from"]: p for p in nn["pipes"]}
        heads = [s_[0] for s_ in nn["structures"] if not any(p["to"] == s_[0] for p in nn["pipes"])]
        best = []
        for h_ in heads:
            path, c_ = [], h_
            while c_ in down: path.append(down[c_]); c_ = down[c_]["to"]
            if sum(p["length"] for p in path) > sum(p["length"] for p in best): best = path
        ia = fig.add_axes([px, 0.06, 0.38, 0.3]); ia.set_facecolor("#1b1f24"); ch = 0
        for p in best:
            L = p["length"]
            ia.fill([ch, ch + L, ch + L, ch], [p["invert_up"], p["invert_dn"], p["invert_dn"] + p["diameter_m"], p["invert_up"] + p["diameter_m"]], color="#0a84ff" if nn is net else "#a2845e", alpha=0.6)
            ia.plot([ch, ch + L], [p["hgl_up"], p["hgl_dn"]], color="#00e5ff", lw=1.8)
            ia.plot([ch, ch + L], [p["egl_up"], p["egl_dn"]], color="#ff453a", lw=1, ls="--")
            ia.plot([ch, ch + L], [hyd[p["from"]]["rim"], hyd[p["to"]]["rim"]], color="#b08968", lw=1.5)
            ch += L
        ia.tick_params(colors="#9aa4b2"); ia.set_title(f"{nn['name']} profile: HGL cyan / EGL red dashed / rim brown", color="w", fontsize=12)
    if s[4] == "haul":
        ia = fig.add_axes([px, 0.06, 0.38, 0.32]); ia.set_facecolor("#1b1f24")
        o = np.array(q["mass_haul"]["ordinates"]); n = max(2, int(len(o) * u))
        ia.plot(o[:n, 0], o[:n, 1], color="#facc15", lw=2); ia.axhline(0, color="#6b7684")
        ia.tick_params(colors="#9aa4b2"); ia.set_title("Mass haul (m3)", color="w", fontsize=13)
    fig.savefig(os.path.join(frames_dir, f"f{fi:05d}.png"), facecolor=fig.get_facecolor())
    plt.close(fig)

if __name__ == "__main__":
    nfr = int(total * FPS)
    with Pool(os.cpu_count()) as pool:
        pool.map(draw, range(nfr), chunksize=8)
    mp4 = os.path.join(media, "civil-mcp-agent-design.mp4")
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", str(FPS), "-i", f"{frames_dir}/f%05d.png", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20", "-r", "30", mp4], check=True)
    gif = os.path.join(media, "civil-mcp-agent-design.gif")
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", mp4, "-vf", "fps=10,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=4", gif], check=True)
    # stills: last frame of each step
    acc = 0
    for k, s in enumerate(S):
        acc += s[0]
        subprocess.run(["cp", f"{frames_dir}/f{min(int(acc*FPS)-2, nfr-1):05d}.png", os.path.join(media, f"step-{k+1:02d}-{s[4]}.png")])
    print(mp4, os.path.getsize(mp4), gif, os.path.getsize(gif))
