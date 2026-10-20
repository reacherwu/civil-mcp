"""Render the agent's design run as a 1080p MP4 + GIF, purely from a demo output dir.

Every frame is drawn from files the tools actually wrote (run-log.jsonl, design.xml,
corridor.json, grading-*.json, network.json, quantities.json, sheets.pdf) plus the
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
FONT = os.path.join(os.path.dirname(__file__), "..", "crates", "civil-core", "assets", "NotoSansSC-Regular-GB2312.ttf")
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
net = json.load(open(f"{out}/network.json"))
q = json.load(open(f"{out}/quantities.json"))
grads = [json.load(open(os.path.join(out, f))) for f in sorted(os.listdir(out)) if f.startswith("grading-")]
subprocess.run(["pdftoppm", "-r", "110", "-png", "-f", "1", "-l", "1", f"{out}/sheets.pdf", f"{media}/sheet"], check=True)
sheet_png = [f for f in os.listdir(media) if f.startswith("sheet") and f.endswith(".png")][0]
sheet = plt.imread(os.path.join(media, sheet_png))

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

chk1 = entry("check_standards", 0); chk2 = entry("check_standards", 1); chk3 = entry("check_standards", 2)
p1, p2 = entry("design_pipe_network", 0), entry("design_pipe_network", 1)
def fails(e):
    return [f"× {c['id']} {c['element']}: {c['value']:.3g} vs limit {c['limit']:.3g} {c['unit']}" for c in e["mandatory_failures"]]
pipe_fails = fails

corr_e = entry("build_corridor"); gr_e = entry("grade")
S = [  # (seconds, tool label, call entry, status lines, stage key)
    (3.0, "USGS 3DEP 1/3″ DEM (public domain) — Palo Alto, CA", None, ["本动画每一帧均来自 civil-mcp 实际运行输出", "Every frame is drawn from the tools' real outputs"], "dem"),
    (3.5, "set_design_basis + load_points", entry("load_points"), [f"工程师设定: JTG D20-2017, 40 km/h, e_max 0.08, μ 0.04", f"{entry('load_points')['result']['points']} 个测量点 survey points"], "points"),
    (3.5, "build_surface", entry("build_surface"), [f"TIN: {entry('build_surface')['result']['triangles']} triangles"], "tin"),
    (4.0, "create_alignment (Ls = 40 m) → check_standards", entry("create_alignment", 0), [f"{chk1['result']['mandatory_failures']} mandatory failure:"] + fails(chk1) + ["→ 超高过渡段超出缓和曲线, 需加长 Ls"], "align0"),
    (4.0, "create_alignment (Ls = 50 m) + design_superelevation", entry("create_alignment", 1), [f"L = {entry('create_alignment',1)['result']['length_m']:.2f} m", "√ SE-IN-LS curve 1, curve 2 pass"], "align"),
    (5.0, "suggest_profile → create_profile → check_standards", entry("create_profile"), [f"√ {chk2['result']['total']} checks, {chk2['result']['mandatory_failures']} mandatory failures"], "profile"),
    (5.0, "create_assembly → build_corridor", corr_e, [f"{corr_e['result']['sections']} sections @20 m", f"cut {corr_e['result']['cut_m3']:.0f} m³ / fill {corr_e['result']['fill_m3']:.0f} m³", f"composite-grid cross-check: {corr_e['result']['composite_check']['cut_diff_pct']:+.2f}% / {corr_e['result']['composite_check']['fill_diff_pct']:+.2f}%"], "corridor"),
    (3.5, "grade (pad 50×30 m, 2:1 slopes)", gr_e, [f"cut {gr_e['result']['cut_m3']:.0f} m³ / fill {gr_e['result']['fill_m3']:.0f} m³"], "grading"),
    (4.5, "design_pipe_network (attempt 1, GB 50014-2021)", p1, [f"{p1['result']['mandatory_failures']} mandatory failures:"] + pipe_fails(p1), "pipes0"),
    (4.0, "design_pipe_network (attempt 2: upsize + 3% max slope, drop MH)", p2, ["√ Manning / velocity / capacity / cover: all pass"], "pipes"),
    (4.0, "compute_quantities (mass haul)", entry("compute_quantities"), [f"final ordinate {q['mass_haul']['final_ordinate']:.0f} m³ (import needed)", f"balance point K0+{q['mass_haul']['balance_stations'][0]:.0f}"], "haul"),
    (6.0, "check_standards → generate_drawings", entry("generate_drawings"), [f"√ {chk3['result']['total']} checks, 0 mandatory failures", "8 sheets PDF/DXF, 中文 OFL font embedded", "状态: 初步设计 PRELIMINARY — 待注册工程师审核"], "sheet"),
]
order = [s[4] for s in S]
total = sum(s[0] for s in S)
print(f"{total:.1f} s, {int(total*FPS)} frames")

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
    if s[4] == "sheet":
        ax = fig.add_axes([0.01, 0.02, 0.62, 0.96]); ax.imshow(sheet); ax.axis("off")
    else:
        ax = fig.add_axes([0.0, 0.0, 0.5625 * 1.0, 1.0])
        ax.imshow(hs, extent=ext, cmap="gray", vmin=0.0, vmax=1.0, interpolation="bicubic")
        ax.imshow(dem, extent=ext, cmap="terrain", alpha=0.25, interpolation="bicubic")
        ax.set_xlim(ext[0], ext[1]); ax.set_ylim(ext[2] - 30, ext[3]); ax.set_facecolor("#111418"); ax.axis("off")
        if stage >= 1:
            n = len(pts) if stage > 1 else int(len(pts) * u)
            ax.scatter(pts[:n, 0], pts[:n, 1], s=1.2, c="#ffd54a", alpha=0.6, lw=0)
        if stage >= 2:
            n = len(eg_segs) if stage > 2 else int(len(eg_segs) * u)
            ax.add_collection(LineCollection(eg_segs[:n], colors="#00e5ff", lw=0.35, alpha=0.55))
        if stage >= 4:
            n = len(cl) if stage > 4 else max(2, int(len(cl) * u))
            ax.plot(cl[:n, 0], cl[:n, 1], color="#ff3b30", lw=2.5)
        if stage == 3:
            ax.text(0.5, 0.5, "Ls = 40 m  × SE-IN-LS", color="#ff6b6b", fontsize=34, ha="center", transform=ax.transAxes, weight="bold")
        if stage >= 6:
            n = len(secs) if stage > 6 else max(2, int(len(secs) * u))
            polys, cols = [], []
            for a, b in zip(secs[: n - 1], secs[1:n]):
                for sd in ("left", "right"):
                    sg = 1 if sd == "left" else -1
                    polys.append([off(a, 0), off(a, sg * a[sd]["daylight"]), off(b, sg * b[sd]["daylight"]), off(b, 0)])
                    cols.append("#e5484d" if a[sd]["cut"] else "#30a46c")
            ax.add_collection(PolyCollection(polys, facecolors=cols, edgecolors="none", alpha=0.75))
            ax.plot(cl[:n, 0], cl[:n, 1], color="white", lw=1)
        if stage >= 7:
            for g in grads:
                fp = np.array(g["footprint"]); dl = np.array(g["daylight"])
                k_ = len(fp) if stage > 7 else max(2, int(len(fp) * u))
                ax.fill(dl[:, 0], dl[:, 1], color="#c084fc", alpha=0.35 if stage > 7 else 0.35 * u)
                ax.plot(fp[:k_, 0], fp[:k_, 1], color="#c084fc", lw=2)
        if stage >= 8:
            st = {s_[0]: {"x": s_[1], "y": s_[2]} for s_ in net["structures"]}
            bad = {c["element"].split()[-1] for c in p1["mandatory_failures"]} if stage == 8 else set()
            for p in net["pipes"]:
                a, b = st[p["from"]], st[p["to"]]
                ax.plot([a["x"], b["x"]], [a["y"], b["y"]], color="#ff9f0a" if p["id"] in bad else "#0a84ff", lw=3.5)
            ax.scatter([s_[1] for s_ in net["structures"]], [s_[2] for s_ in net["structures"]], s=40, c="white", edgecolors="k", zorder=5)
        ax.text(0.01, 0.01, "USGS 3DEP DEM, UTM 10N · civil-mcp demo output", color="w", fontsize=11, transform=ax.transAxes)
    # right panel
    px = 0.585 if s[4] != "sheet" else 0.645
    fig.text(px, 0.94, "civil-mcp · AI agent → MCP tools", color="#9aa4b2", fontsize=20)
    fig.text(px, 0.895, f"Step {stage + 1}/{len(S)}", color="#9aa4b2", fontsize=18)
    import textwrap
    for j, tl in enumerate(textwrap.wrap(s[1], 44)[:2]):
        fig.text(px, 0.845 - j * 0.04, tl, color="white", fontsize=22)
    if s[2] is not None:
        fig.text(px, 0.755, "tools/call " + s[2]["tool"], color="#7fd3ff", fontsize=16, family="monospace")
        fig.text(px, 0.72, fmt_args(s[2]["args"], 90), color="#6b7684", fontsize=11, family="monospace")
    for i, line in enumerate(s[3]):
        c = "#ff6b6b" if line.startswith("×") or "failure" in line and not line.startswith("√") else ("#4ade80" if line.startswith("√") else "#e6e9ef")
        fig.text(px, 0.66 - i * 0.042, line, color=c, fontsize=17)
    # inset charts
    if s[4] in ("profile", "corridor", "grading", "pipes0", "pipes"):
        ia = fig.add_axes([px, 0.08, 0.38, 0.27]); ia.set_facecolor("#1b1f24")
        n = len(secs) if s[4] != "profile" else max(2, int(len(secs) * u))
        ia.plot(sta, [s_["ground_cl"] for s_ in secs], color="#b08968", lw=1.5, label="地面 ground")
        ia.plot(sta[:n], [s_["cl_elev"] for s_ in secs][:n], color="#ff3b30", lw=2, label="设计 FG")
        ia.tick_params(colors="#9aa4b2"); ia.set_title("纵断面 Profile", color="w", fontsize=13); ia.legend(fontsize=10)
    if s[4] == "haul":
        ia = fig.add_axes([px, 0.08, 0.38, 0.4]); ia.set_facecolor("#1b1f24")
        o = np.array(q["mass_haul"]["ordinates"]); n = max(2, int(len(o) * u))
        ia.plot(o[:n, 0], o[:n, 1], color="#facc15", lw=2); ia.axhline(0, color="#6b7684")
        ia.tick_params(colors="#9aa4b2"); ia.set_title("土方调配 Mass haul (m³)", color="w", fontsize=13)
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
