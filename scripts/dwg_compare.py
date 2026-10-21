"""Compare civil-mcp DXF files with the DXF read back from ODA-written DWG files.

usage: python scripts/dwg_compare.py <dxf-dir> <dwg-dir> <dxf-back-dir>
For every DXF: DWG version magic, modelspace entity counts by type, layer set (layers that
carry entities) and extents (min/max of entity points); 2D POLYLINE and LWPOLYLINE are counted together before and after DXF -> DWG -> DXF.
Prints one line per file and a summary; exits 1 on any mismatch.
"""
import collections, glob, os, sys
import ezdxf
from ezdxf import bbox

src, dwgd, back = sys.argv[1:4]
MAGIC = {b"AC1027": "AC1027 (2013)", b"AC1032": "AC1032 (2018)", b"AC1024": "AC1024 (2010)", b"AC1018": "AC1018 (2004)"}


def stats(path):
    d = ezdxf.readfile(path)
    msp = d.modelspace()
    # R12 2D POLYLINEs are written back as LWPOLYLINE by DWG-era DXF: same geometry, so both
    # count as "POLYLINE(2D)"; the vertex total is compared separately
    def kind(e):
        t = e.dxftype()
        if t == "LWPOLYLINE" or (t == "POLYLINE" and e.is_2d_polyline):
            return "POLYLINE(2D)"
        return t
    cnt = collections.Counter(kind(e) for e in msp)
    cnt["vertices"] = sum(len(e) if e.dxftype() == "LWPOLYLINE" else len(e.vertices) for e in msp if e.dxftype() in ("LWPOLYLINE", "POLYLINE"))
    layers = {e.dxf.layer.upper() for e in msp}
    ext = bbox.extents(msp, fast=True)
    return cnt, layers, ext, len(d.audit().errors)


bad = 0
rows = 0
for f in sorted(glob.glob(f"{src}/*.dxf")):
    b = os.path.splitext(os.path.basename(f))[0]
    dw = [p for p in glob.glob(f"{dwgd}/*") if os.path.splitext(os.path.basename(p))[0].lower() == b.lower()]
    bk = [p for p in glob.glob(f"{back}/*") if os.path.splitext(os.path.basename(p))[0].lower() == b.lower()]
    if not dw or not bk:
        print(f"MISSING {b}"); bad += 1; continue
    ver = MAGIC.get(open(dw[0], "rb").read(6), "unknown")
    c0, l0, e0, a0 = stats(f)
    c1, l1, e1, a1 = stats(bk[0])
    dext = max(abs(a - b_) for a, b_ in zip((*e0.extmin, *e0.extmax), (*e1.extmin, *e1.extmax))) if e0.has_data and e1.has_data else 0.0
    ok = c0 == c1 and l0 == l1 and dext < 1e-6
    rows += 1
    if not ok:
        bad += 1
        print(f"BAD {b} {ver}: counts {dict(c0 - c1)} / {dict(c1 - c0)}, layers {l0 ^ l1}, extents diff {dext:.3g}")
    else:
        print(f"OK  {b} {ver}: {sum(v for k, v in c0.items() if k != 'vertices')} entities, {c0['vertices']} polyline vertices, {len(l0)} layers, extents diff {dext:.1e}, audit errors {a0}/{a1}")
print(f"{rows} files compared, {bad} mismatches")
sys.exit(1 if bad else 0)
