"""Independent earthwork check (grid method) for a civil-mcp project.

Does NOT reuse any Rust code. Rebuilds the ground two ways:
  A) scipy/Qhull Delaunay + linear interpolation of the same survey CSV
  B) (optional) the original raster DEM, bilinear
then sums (design - ground) over a fine grid inside the daylight footprint.

usage: python independent_check.py <project_dir> <points.csv> [dem.npy x0 y0 px]
"""
import json, sys
import numpy as np
from scipy.interpolate import LinearNDInterpolator
from scipy.spatial import cKDTree

proj, csv = sys.argv[1], sys.argv[2]
d = json.load(open(f"{proj}/earthwork.json"))
t, ew = d["template"], d["earthwork"]
cl = np.array(d["centerline_1m"])  # s x y h z
secs = ew["sections"]
ss = np.array([s["station"] for s in secs]); dl = np.array([s["daylight_left"] for s in secs]); dr = np.array([s["daylight_right"] for s in secs])
pts = np.loadtxt(csv, delimiter=",", skiprows=1, usecols=(1, 2, 3))
ground_tin = LinearNDInterpolator(pts[:, :2], pts[:, 2])

def ground_dem_factory(path, x0, y0, px):
    a = np.load(path)
    def g(x, y):
        c = (x - x0) / px - 0.5; r = (y0 - y) / px - 0.5
        c0 = np.clip(np.floor(c).astype(int), 0, a.shape[1] - 2); r0 = np.clip(np.floor(r).astype(int), 0, a.shape[0] - 2)
        fc = c - c0; fr = r - r0
        return (a[r0, c0] * (1 - fc) * (1 - fr) + a[r0, c0 + 1] * fc * (1 - fr) + a[r0 + 1, c0] * (1 - fc) * fr + a[r0 + 1, c0 + 1] * fc * fr)
    return g

def volumes(ground, cell):
    hw, cs, ch, fh = t["half_width_m"], t["cross_slope"], t["cut_slope_h"], t["fill_slope_h"]
    m = max(dl.max(), dr.max()) + 5
    xs = np.arange(cl[:, 1].min() - m, cl[:, 1].max() + m, cell); ys = np.arange(cl[:, 2].min() - m, cl[:, 2].max() + m, cell)
    X, Y = np.meshgrid(xs, ys); X = X.ravel(); Y = Y.ravel()
    tree = cKDTree(cl[:, 1:3]); _, i = tree.query(np.c_[X, Y])
    h = cl[i, 3]; dx = X - cl[i, 1]; dy = Y - cl[i, 2]
    along = dx * np.cos(h) + dy * np.sin(h); off = -dx * np.sin(h) + dy * np.cos(h)
    s = cl[i, 0] + along
    ok = (s >= ss[0]) & (s <= ss[-1]) & (np.abs(along) <= 0.5 + 1e-9)
    L = np.interp(s, ss, dl); R = np.interp(s, ss, dr)
    ok &= ((off >= 0) & (off <= L)) | ((off < 0) & (-off <= R))
    X, Y, s, off, h = X[ok], Y[ok], s[ok], off[ok], h[ok]
    zc = np.interp(s, cl[:, 0], cl[:, 4]); edge = zc - cs * hw
    a = np.abs(off); sign = np.sign(off) + (off == 0)
    ex = X - (a - hw) * sign * -np.sin(h); ey = Y - (a - hw) * sign * np.cos(h)
    g = ground(X, Y)
    gedge = ground(ex, ey)
    z = np.where(a <= hw, zc - cs * a, np.where(gedge >= edge, edge + (a - hw) / ch, edge - (a - hw) / fh))
    dz = z - g; dz = dz[np.isfinite(dz)]
    return float(-dz[dz < 0].sum() * cell * cell), float(dz[dz > 0].sum() * cell * cell), int(ok.sum())

res = {"rust_average_end_area": {"cut_m3": ew["cut_m3"], "fill_m3": ew["fill_m3"], "interval_m": ew["interval_m"]}}
for cell in (1.0, 0.5):
    c, f, n = volumes(lambda x, y: ground_tin(x, y), cell)
    res[f"grid_{cell}m_same_points_qhull"] = {"cut_m3": c, "fill_m3": f, "cells": n}
if len(sys.argv) > 3:
    g = ground_dem_factory(sys.argv[3], float(sys.argv[4]), float(sys.argv[5]), float(sys.argv[6]))
    c, f, n = volumes(g, 0.5)
    res["grid_0.5m_full_raster_dem"] = {"cut_m3": c, "fill_m3": f, "cells": n}
base = res["rust_average_end_area"]
for k, v in res.items():
    if k != "rust_average_end_area":
        v["cut_diff_pct_vs_rust"] = 100 * (base["cut_m3"] - v["cut_m3"]) / v["cut_m3"]
        v["fill_diff_pct_vs_rust"] = 100 * (base["fill_m3"] - v["fill_m3"]) / v["fill_m3"]
print(json.dumps(res, indent=2))
