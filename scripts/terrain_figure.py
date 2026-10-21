"""Plot the route alternatives of one terrain-batch site over its DEM hillshade.

usage: python scripts/terrain_figure.py <dem.tif (projected, metres)> <site-out dir> <title> <out.png>
Reads route-alternatives.json written by generate_route_alternatives (scripts/terrain_batch.py).
"""
import json, sys
import numpy as np, tifffile
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

tif, site, title, png = sys.argv[1:5]
with tifffile.TiffFile(tif) as t:
    z = t.pages[0].asarray().astype(float)
    tags = t.pages[0].tags
    sx, sy = tags["ModelPixelScaleTag"].value[:2]
    tp = tags["ModelTiepointTag"].value
x0, y0 = tp[3] - tp[0] * sx, tp[4] + tp[1] * sy
z[z < -1e4] = np.nan
gy, gx = np.gradient(z, sy, sx)
hs = np.clip(0.6 - 0.5 * (gx - gy) / np.sqrt(1 + gx ** 2 + gy ** 2), 0, 1)
ext = (x0, x0 + z.shape[1] * sx, y0 - z.shape[0] * sy, y0)
r = json.load(open(f"{site}/route-alternatives.json"))
fig, ax = plt.subplots(figsize=(9, 8), dpi=110)
ax.imshow(hs, cmap="gray", extent=ext, alpha=0.9)
cs = ax.contour(np.linspace(ext[0], ext[1], z.shape[1]), np.linspace(ext[3], ext[2], z.shape[0]), z, levels=15, colors="#8a6d3b", linewidths=0.4)
cols = ["#d62728", "#1f77b4", "#2ca02c", "#9467bd"]
for a, c in zip(r["alternatives"], cols):
    p = np.array(a["path"]); e = a["evaluation"]
    fails = [f for f in e.get("failed_checks", []) if f.startswith("EW-")]
    lab = f"{a['name']}: {e['length_m']:.0f} m, min R {e['min_radius_m']:.0f} m" + (f", {len(fails)} limit failure(s)" if fails else "")
    ax.plot(p[:, 0], p[:, 1], color=c, lw=2, label=lab)
ax.plot(*np.array(r["alternatives"][0]["path"])[[0, -1]].T, "ko", ms=6)
ax.set_title(title, fontsize=11)
ax.legend(loc="lower right", fontsize=8)
ax.set_xlabel("E (m)"); ax.set_ylabel("N (m)"); ax.ticklabel_format(useOffset=False, style="plain")
plt.tight_layout(); plt.savefig(png)
