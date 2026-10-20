"""Compare civil-mcp DXF with what third-party CAD tools read back.
usage: python scripts/cad_compare.py <work-dir from cad_roundtrip.sh>"""
import collections, glob, json, os, sys
import ezdxf
W = sys.argv[1]
NORM = {"POLYLINE": "LWPOLYLINE"}  # ODA writes 2D POLYLINE as LWPOLYLINE; same geometry
def census(doc):
    c = collections.Counter(); g = [0.0, 0]
    for e in doc.modelspace():
        t = e.dxftype(); c[(NORM.get(t, t), e.dxf.layer.upper())] += 1
        pts = []
        if t == "LINE": pts = [e.dxf.start, e.dxf.end]
        elif t == "LWPOLYLINE": pts = [p[:2] for p in e.get_points()]
        elif t == "POLYLINE": pts = [v.dxf.location for v in e.vertices]
        elif t in ("TEXT",): pts = [e.dxf.insert]
        for p in pts:
            g[0] += abs(p[0]) + abs(p[1]); g[1] += 1
    c[("_coords_sum", "")] = round(g[0], 3); c[("_vertices", "")] = g[1]
    return c
rows = []
for f in sorted(glob.glob(f"{W}/in/*.dxf")):
    b = os.path.basename(f)[:-4]
    a = census(ezdxf.readfile(f))
    bk = census(ezdxf.readfile(f"{W}/back/{b}.dxf"))
    # LibreDWG JSON: count entities by type in OBJECTS
    lj = json.loads(open(f"{W}/lrd/{b}.json", encoding="utf-8", errors="replace").read())
    lc = collections.Counter(o.get("entity") for o in lj.get("OBJECTS", []) if o.get("entity"))
    na = sum(v for k, v in a.items() if not k[0].startswith("_")); nb = sum(v for k, v in bk.items() if not k[0].startswith("_"))
    diff = {f"{k[0]}@{k[1]}": (a.get(k, 0), bk.get(k, 0)) for k in set(a) | set(bk) if a.get(k, 0) != bk.get(k, 0)}
    lrd_n = sum(v for k, v in lc.items() if k in ("LINE", "LWPOLYLINE", "POLYLINE_2D", "TEXT", "MTEXT", "HATCH", "SOLID", "VERTEX_2D", "SEQEND", "INSERT"))
    rows.append(dict(file=b, ours=na, oda_roundtrip=nb, mismatched_type_layer=diff, libredwg_entities=dict(lc)))
    print(f"{b}: ezdxf(ours)={na}  ODA DWG→DXF={nb}  mismatches={len(diff)}  LibreDWG reads {sum(lc.values())} entities {dict(lc)}")
    for k, v in list(diff.items())[:6]:
        print("   ", k, v)
json.dump(rows, open(f"{W}/cad-compare.json", "w"), indent=1, ensure_ascii=False)
