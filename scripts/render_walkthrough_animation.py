# Step-by-step walkthrough animation of a civil-mcp design (bare terrain -> finished road).
#
# Stage 1 (Blender, headless):  blender -b -P scripts/render_walkthrough_animation.py -- <project dir> <frames dir> [--res 1920x1080] [--engine BLENDER_WORKBENCH]
# Stage 2 (Python + ffmpeg):    python3 scripts/render_walkthrough_animation.py compose <project dir> <frames dir> <out.mp4>
#
# Uses only files written by the run: scene.glb / scene.json (export_blender_scene), design.xml
# (export_landxml: profile + cross-section EG), superelevation.json, quantities.json.
# Captions only quote numbers read from those files.
import json, math, os, re, sys

FPS = 30
# (start frame, end frame) per stage, 1-based inclusive
ST = {1: (1, 105), 2: (106, 225), 3: (226, 345), 4: (346, 465), 5: (466, 585), 6: (586, 765), 7: (766, 885), 8: (886, 1185)}
NF = 1185
CUT_LABEL_FROM = 70  # frames into stage 6 when the layer labels appear


def facts(D):
    meta = json.load(open(os.path.join(D, "scene.json")))
    sel = meta["selected"]
    alts = {a["name"]: a["evaluation"] for a in meta["alternatives"]}
    ev = alts[sel]
    x = open(os.path.join(D, "design.xml")).read()
    pa = re.search(r"<ProfAlign[^>]*>(.*?)</ProfAlign>", x, re.S).group(1)
    pvi = []
    for m in re.finditer(r"<(PVI|ParaCurve)([^>]*)>([^<]+)<", pa):
        s, z = map(float, m.group(3).split()); L = float(re.search(r'length="([^"]+)"', m.group(2)).group(1)) if m.group(1) == "ParaCurve" else 0.0
        pvi.append((s, z, L))
    eg = []
    for m in re.finditer(r'<CrossSect sta="([^"]+)".*?<CrossSectSurf name="EG"><PntList2D>([^<]+)<', x, re.S):
        v = list(map(float, m.group(2).split())); pts = list(zip(v[0::2], v[1::2]))
        for (o1, z1), (o2, z2) in zip(pts, pts[1:]):
            if o1 <= 0 <= o2:
                eg.append((float(m.group(1)), z1 + (z2 - z1) * (0 - o1) / (o2 - o1) if o2 != o1 else z1)); break
    L = float(re.search(r'<Alignment name="[^"]*" length="([^"]+)"', x).group(1))
    se = json.load(open(os.path.join(D, "superelevation.json")))
    q = json.load(open(os.path.join(D, "quantities.json")))["earthwork"]
    grades = [(b[1] - a[1]) / (b[0] - a[0]) * 100 for a, b in zip(pvi, pvi[1:])]
    c1 = se["curves"][0]
    cj = json.load(open(os.path.join(D, "corridor.json")))["corridor"]
    # cutaway at the corridor section nearest the middle of full superelevation on curve 1
    sec = min((x for x in cj["sections"] if c1["b_in"] <= x["station"] <= c1["b_out"]), key=lambda x: abs(x["station"] - (c1["b_in"] + c1["b_out"]) / 2))
    cut_sta = sec["station"]
    return dict(sec=sec, cut_sta=cut_sta, meta=meta, sel=sel, alts=alts, ev=ev, pvi=pvi, eg=eg, L=L, se=se, q=q, grades=grades, asm=json.load(open(os.path.join(D, "corridor.json")))["corridor"]["assembly"])


def fg_at(pvi, s):
    # vertical profile with symmetric parabolic curves at PVIs carrying a length
    for i in range(len(pvi) - 1):
        if pvi[i][0] <= s <= pvi[i + 1][0]: break
    def line(i, s):
        a, b = pvi[i], pvi[i + 1]; return a[1] + (b[1] - a[1]) * (s - a[0]) / (b[0] - a[0])
    for k in range(1, len(pvi) - 1):
        sk, zk, Lk = pvi[k]
        if Lk and abs(s - sk) <= Lk / 2:
            g1 = (zk - pvi[k - 1][1]) / (sk - pvi[k - 1][0]); g2 = (pvi[k + 1][1] - zk) / (pvi[k + 1][0] - sk)
            x = s - (sk - Lk / 2); z0 = zk - g1 * Lk / 2
            return z0 + g1 * x + (g2 - g1) * x * x / (2 * Lk)
    return line(i, s)


def sta(s):
    return f"{int(s // 1000)}+{s % 1000:06.2f}" if s % 1 else f"{int(s // 1000)}+{int(s % 1000):03d}"


def captions(F):
    ev, se, q, asm = F["ev"], F["se"], F["q"], F["asm"]
    n_alt = len(F["alts"])
    radii = ", ".join(f"{c['radius']:.0f}" for c in se["curves"])
    pav = " / ".join(f"{l['name']} {l['thickness_m'] * 1000:.0f} mm" for l in asm["pavement"])
    emax = max(se["curves"], key=lambda c: c["e"])
    return {
        1: ("1  Existing ground", "Terrain surface built from the survey points (build_surface)"),
        2: ("2  Route alternatives", f"{n_alt} corridors generated and scored; {F['sel']} selected (fewest failures, lowest cost proxy)"),
        3: ("3  Horizontal alignment", f"{F['L']:.2f} m centreline, {len(se['curves'])} spiral-curve-spiral curves, R = {radii} m"),
        4: ("4  Vertical profile", f"Grade line against existing ground; max grade {max(abs(g) for g in F['grades']):.1f}%"),
        5: ("5  Earthwork", f"Cut {q['cut_bank_m3']:,.0f} m³ (bank)  ·  fill {q['fill_compacted_m3']:,.0f} m³ (compacted)"),
        6: ("6  Pavement structure", f"Cutaway at {sta(round(F['cut_sta'], 2))} on curve 1 (R {F['se']['curves'][0]['radius']:.0f} m, e = {F['se']['curves'][0]['e'] * 100:.1f}%), layers from the design"),
        7: ("7  Drainage and structures", "Pipes, manholes and retaining walls (ground drawn as see-through mesh; pipe diameters enlarged 0.5 m for visibility)"),
        8: ("8  Drive-through", f"Selected route {F['sel']}, station {sta(0)} to {sta(round(F['L'], 2))}"),
    }


# ---------------------------------------------------------------- Blender stage
def blender_main(argv):
    import bpy
    from mathutils import Vector, kdtree
    D, OUT = os.path.abspath(argv[0]), os.path.abspath(argv[1])
    res, engine = "1920x1080", "BLENDER_WORKBENCH"
    for i, a in enumerate(argv):
        if a == "--res": res = argv[i + 1]
        if a == "--engine": engine = argv[i + 1]
    only = None
    for i, a in enumerate(argv):
        if a == "--frames": only = [int(v) for v in argv[i + 1].split(",")]
    F = facts(D); meta = F["meta"]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.join(D, "scene.glb"))
    sc = bpy.context.scene
    w, h = map(int, res.split("x")); sc.render.resolution_x, sc.render.resolution_y = w, h; sc.render.resolution_percentage = 100
    sc.render.engine = engine
    sh = sc.display.shading
    sh.light = "STUDIO"; sh.color_type = "MATERIAL"; sh.show_shadows = True; sh.shadow_intensity = 0.35
    sh.show_cavity = True; sh.cavity_type = "WORLD"; sh.show_specular_highlight = False
    sc.display.light_direction = (0.45, -0.35, 0.82)
    sc.world = bpy.data.worlds.new("W"); sc.world.color = (0.86, 0.91, 0.96)
    sc.render.film_transparent = False
    sc.view_settings.view_transform = "Standard"

    def mat(name, rgba):
        m = bpy.data.materials.new(name); m.diffuse_color = rgba; m.use_nodes = False; return m
    O = {o.name: o for o in sc.objects if o.type == "MESH"}
    def setmat(o, m):
        o.data.materials.clear(); o.data.materials.append(m)
    setmat(O["Existing ground"], mat("ground", (0.74, 0.80, 0.60, 1)))
    setmat(O["Road pavement"], mat("asphalt", (0.22, 0.23, 0.26, 1)))
    GT = meta["ground_trimmed_mesh"]; setmat(O[GT], O["Existing ground"].data.materials[0])
    LAY = [L["mesh"] for L in meta["pavement_layers"]]
    laycol = [(0.16, 0.17, 0.19, 1), (0.90, 0.55, 0.15, 1), (0.62, 0.66, 0.74, 1)]
    for k, n in enumerate(LAY): setmat(O[n], mat(n, laycol[k % 3]))
    setmat(O["Retaining walls"], mat("wall", (0.75, 0.75, 0.78, 1)))
    setmat(O["Pipes"], mat("pipe", (0.10, 0.45, 0.85, 1)))
    setmat(O["Structures"], mat("struct", (0.95, 0.55, 0.10, 1)))
    altcol = [(0.90, 0.10, 0.20, 1), (0.15, 0.45, 0.90, 1), (0.60, 0.30, 0.75, 1), (0.10, 0.60, 0.55, 1)]
    alts = sorted(n for n in O if n.startswith("Alternative "))
    def tube(name, pts, r, m):
        c = bpy.data.curves.new(name, "CURVE"); c.dimensions = "3D"; c.bevel_depth = r; c.bevel_resolution = 2
        sp = c.splines.new("POLY"); sp.points.add(len(pts) - 1)
        for i, q in enumerate(pts): sp.points[i].co = (q.x, q.y, q.z + 4.0, 1)
        o = bpy.data.objects.new(name, c); sc.collection.objects.link(o); c.materials.append(m); return o
    # alternative corridors are thin ribbons; draw each as a tube through the ribbon midpoints
    for k, n in enumerate(alts):
        ob = O[n]; v = [ob.matrix_world @ x.co for x in ob.data.vertices]
        mids = [(v[2 * i] + v[2 * i + 1]) / 2 for i in range(len(v) // 2)]
        ob.hide_render = True
        O[n] = tube("tube " + n, mids, 3.0, mat(n, altcol[k % 4]))
    selname = "Alternative " + F["sel"]
    path = [Vector(p) for p in meta["camera_path"]]
    # cut / fill colouring of earthwork faces: centroid above the nearest centreline grade = cut
    ew = O["Earthwork slopes"]; me = ew.data
    me.materials.clear(); me.materials.append(mat("cut", (0.80, 0.45, 0.20, 1))); me.materials.append(mat("fill", (0.25, 0.60, 0.35, 1)))
    kd = kdtree.KDTree(len(path))
    for i, p in enumerate(path): kd.insert((p.x, p.y, 0), i)
    kd.balance()
    mw = ew.matrix_world
    for poly in me.polygons:
        c = mw @ poly.center; _, j, _ = kd.find((c.x, c.y, 0))
        poly.material_index = 0 if c.z > path[j].z else 1
    # centreline curve that grows during stage 3
    cu = bpy.data.curves.new("CL", "CURVE"); cu.dimensions = "3D"; cu.bevel_depth = 3.0; cu.bevel_resolution = 2
    sp = cu.splines.new("POLY"); sp.points.add(len(path) - 1)
    for i, p in enumerate(path): sp.points[i].co = (p.x, p.y, p.z + 4.0, 1)
    cl = bpy.data.objects.new("Centreline", cu); sc.collection.objects.link(cl); cl.data.materials.append(mat("cl", (0.90, 0.10, 0.20, 1)))
    # stations along path (cumulative length scaled to the alignment length)
    cum = [0.0]
    for a, b in zip(path, path[1:]): cum.append(cum[-1] + ((b - a).to_2d()).length)
    k_s = F["L"] / cum[-1]
    def at(s):
        s = s / k_s
        for i in range(len(cum) - 1):
            if cum[i] <= s <= cum[i + 1]:
                return path[i].lerp(path[i + 1], (s - cum[i]) / max(cum[i + 1] - cum[i], 1e-9))
        return path[-1]
    cam = bpy.data.objects.new("Camera", bpy.data.cameras.new("Camera")); sc.collection.objects.link(cam); sc.camera = cam
    cam.data.lens = 30; cam.data.clip_start = 1; cam.data.clip_end = 20000
    labels = []
    txtm = mat("txt", (0.08, 0.10, 0.15, 1)); txtr = mat("txtr", (0.75, 0.05, 0.15, 1))
    def label(text, loc, size, m):
        t = bpy.data.curves.new(text, "FONT"); t.body = text; t.size = size; t.align_x = "CENTER"; t.extrude = 0.3
        o = bpy.data.objects.new("lbl " + text, t); sc.collection.objects.link(o); o.location = loc; t.materials.append(m)
        c = o.constraints.new("TRACK_TO"); c.target = cam; c.track_axis = "TRACK_Z"; c.up_axis = "UP_Y"
        labels.append(o); return o
    step = 200
    s = 0.0
    while s < F["L"] - 50:
        p = at(s); label(sta(s), p + Vector((0, 0, 26)), 24, txtm); s += step
    p = at(F["L"]); label(sta(round(F["L"], 2)), p + Vector((0, 0, 26)), 24, txtm)
    for c in F["se"]["curves"]:
        p = at((c["a_in"] + c["a_out"]) / 2); label(f"R {c['radius']:.0f} m", p + Vector((0, 0, 60)), 30, txtr)

    def vis(o, frames_on):
        for f in range(1, NF + 1, 1):
            pass
        o.hide_render = True; o.keyframe_insert("hide_render", frame=0)
        for a, b in frames_on:
            o.hide_render = False; o.keyframe_insert("hide_render", frame=a)
            o.hide_render = True; o.keyframe_insert("hide_render", frame=b + 1)
    E = lambda n: ST[n][0]; Z = lambda n: ST[n][1]
    vis(O["Existing ground"], [(1, Z(4))])
    CUT_AT = 50  # frames into stage 6 when the cutaway replaces the full model (camera already close)
    CLO = meta.get("closure_mesh") or next((n for n in O if n.startswith("Corridor closure")), None)
    vis(O[GT], [(E(5), Z(5)), (E(6), E(6) + CUT_AT - 1), (E(8), NF)])
    if CLO:
        setmat(O[CLO], mat("closure", (0.55, 0.45, 0.35, 1))); vis(O[CLO], [(E(5), Z(5)), (E(6), E(6) + CUT_AT - 1), (E(8), NF)])
    # stages 5-7: existing ground shown as a light wireframe so cut slopes and buried pipes stay visible
    gw = O["Existing ground"].copy(); gw.data = O["Existing ground"].data.copy(); sc.collection.objects.link(gw); gw.name = "Ground wire"
    md = gw.modifiers.new("wf", "WIREFRAME"); md.thickness = 0.18; setmat(gw, mat("wire", (0.72, 0.76, 0.68, 1)))
    gw.animation_data_clear(); vis(gw, [(E(7), Z(7))])
    for n in alts:
        vis(O[n], [(E(2) + 10, E(2) + 70)] if n != selname else [(E(2) + 10, Z(2))])
    vis(cl, [(E(3), Z(4))])
    for o in labels: vis(o, [(E(3) + 50, Z(4))])
    vis(ew, [(E(5), Z(5)), (E(6), E(6) + CUT_AT - 1), (E(8), NF)])
    # finished surface = top face of the asphalt layer solid; the surface-only mesh stays hidden
    vis(O["Road pavement"], [])
    for n in LAY: vis(O[n], [(E(5), Z(5)), (E(6), E(6) + CUT_AT - 1), (E(7), NF)])
    vis(O["Retaining walls"], [(E(5), Z(5)), (E(6), E(6) + CUT_AT - 1), (E(7), NF)])
    # pipes drawn 0.5 m fatter than designed so they read at video scale (display only)
    dm = O["Pipes"].modifiers.new("fat", "DISPLACE"); dm.mid_level = 0.0; dm.strength = 0.5
    vis(O["Pipes"], [(E(7), Z(7))]); vis(O["Structures"], [(E(7), NF)])
    # centreline grows during stage 3
    cu.bevel_factor_end = 0.0; cu.keyframe_insert("bevel_factor_end", frame=E(3))
    cu.bevel_factor_end = 1.0; cu.keyframe_insert("bevel_factor_end", frame=E(3) + 60)

    # stage 6 cutaway: copies of layers, earthwork, trimmed ground and walls with everything ahead of
    # the section plane at cut_sta removed (exact boolean against a large box)
    sec = F["sec"]; org = meta["origin"]; h = sec["heading"]
    P = Vector((sec["x"] - org["e"], sec["y"] - org["n"], sec["cl_elev"] - org["z"])); T = Vector((math.cos(h), math.sin(h)))
    bpy.ops.mesh.primitive_cube_add(size=1.0); box = bpy.context.active_object; box.name = "cutter"
    box.scale = (4000, 4000, 4000); box.rotation_euler = (0, 0, math.atan2(T.y, T.x))
    box.location = (P.x + T.x * 2000, P.y + T.y * 2000, P.z); box.hide_render = True; box.hide_viewport = True
    cutobjs = []
    for n in LAY + ["Earthwork slopes", GT, "Retaining walls"] + ([CLO] if CLO else []):
        src = O[n]; o = src.copy(); o.data = src.data.copy(); o.animation_data_clear(); sc.collection.objects.link(o); o.name = "cut " + n
        bm = o.modifiers.new("cut", "BOOLEAN"); bm.operation = "DIFFERENCE"; bm.solver = "EXACT"; bm.object = box
        vis(o, [(E(6) + CUT_AT, Z(6))]); cutobjs.append(o)
    R = Vector((T.y, -T.x, 0))  # right of travel
    rw = max(float(F["asm"]["lanes_m"][0]), 1.0)
    # section face at the cut: earth below the design line (formation under the pavement, finished
    # grade elsewhere, so unpaved shoulders show as earth) and below existing ground outside the daylight points, from corridor.json
    def interp(pts, o):
        for a_, b_ in zip(pts, pts[1:]):
            if a_[0] <= o <= b_[0]:
                return a_[1] + (b_[1] - a_[1]) * (o - a_[0]) / max(b_[0] - a_[0], 1e-9)
        return None
    gl = sorted(sec["ground"])  # signed offset (left +), elevation
    def design(o):  # signed offset; None outside the daylight points
        side = sec["left"] if o >= 0 else sec["right"]
        # formation under the pavement layers; finished grade (shoulder, ditch, slope) outside them
        return interp(side["datum"] if abs(o) <= side["paved_width"] else side["fg"], abs(o))
    lo_ = -(sec["right"]["fg"][-1][0]); hi_ = sec["left"]["fg"][-1][0]
    W_ = max(abs(lo_), hi_) + 12.0; base = min(min(z for _, z in gl), sec["cl_elev"]) - 40.0
    capv, capf, capm = [], [], []
    Lv = Vector((-T.y, T.x, 0))
    def cp(o, z): return P.to_2d().to_3d() + Lv * o + Tv0 * 0.005 + Vector((0, 0, z - org["z"]))
    Tv0 = Vector((T.x, T.y, 0))
    # sample at every breakpoint (both sides of each paved edge) plus a uniform grid
    br = {-W_ + 2 * W_ * i / 400 for i in range(401)} | {o for o, _ in gl}
    for sgn, side in ((1, sec["left"]), (-1, sec["right"])):
        for pts in (side["fg"], side["datum"]):
            br |= {sgn * o for o, _ in pts}
        br |= {sgn * (side["paved_width"] - 1e-4), sgn * (side["paved_width"] + 1e-4)}
    br = sorted(o for o in br if -W_ <= o <= W_)
    for o0, o1 in zip(br, br[1:]):
        g0, g1 = interp(gl, o0), interp(gl, o1)
        if g0 is None or g1 is None: continue
        d0, d1 = design(o0) if lo_ <= o0 <= hi_ else None, design(o1) if lo_ <= o1 <= hi_ else None
        t0 = d0 if d0 is not None else g0; t1 = d1 if d1 is not None else g1
        n0, n1 = min(g0, t0), min(g1, t1)  # natural ground left in place
        for (zb0, zb1, zt0, zt1, m_) in ((base, base, n0, n1, 0), (n0, n1, t0, t1, 1)):
            if zt0 - zb0 < 1e-4 and zt1 - zb1 < 1e-4: continue
            k = len(capv); capv += [cp(o0, zb0), cp(o1, zb1), cp(o1, zt1), cp(o0, zt0)]; capf.append((k, k + 1, k + 2, k + 3)); capm.append(m_)
    cm_ = bpy.data.meshes.new("section face"); cm_.from_pydata(capv, [], capf)
    cm_.materials.append(mat("soil", (0.55, 0.45, 0.35, 1))); cm_.materials.append(mat("fillbody", (0.25, 0.60, 0.35, 1)))
    for poly, m_ in zip(cm_.polygons, capm): poly.material_index = m_
    capo = bpy.data.objects.new("Section face", cm_); sc.collection.objects.link(capo); vis(capo, [(E(6) + CUT_AT, Z(6))])

    bb = meta["bbox_local"]
    C = Vector(((bb[0] + bb[3]) / 2, (bb[1] + bb[4]) / 2, (bb[2] + bb[5]) / 2)); span = max(bb[3] - bb[0], bb[4] - bb[1])
    mid = at(F["L"] / 2)
    def look(loc, tgt, f):
        cam.location = Vector(loc); cam.rotation_mode = "QUATERNION"
        cam.rotation_quaternion = (Vector(tgt) - Vector(loc)).to_track_quat("-Z", "Y")
        cam.keyframe_insert("location", frame=f); cam.keyframe_insert("rotation_quaternion", frame=f)
    def orbit(ang, dist, hgt, tgt):
        return Vector(tgt) + Vector((math.cos(ang) * dist, math.sin(ang) * dist, hgt))
    # overview orbit for stages 1-4
    a0 = math.radians(-120)
    keys = [(1, a0, 1.0, 0.8), (E(2), a0 + 0.25, 1.15, 0.95), (E(3), a0 + 0.45, 1.0, 0.95), (E(4), a0 + 0.6, 0.95, 0.6), (Z(4), a0 + 0.75, 0.9, 0.5)]
    for f, a, d, hh in keys:
        look(orbit(a, d * span, hh * span, C), C, f)
    c1 = F["se"]["curves"][0]; pc = at((c1["a_in"] + c1["a_out"]) / 2)
    tgt5 = at(F["L"] * 0.45)
    look(orbit(a0 + 0.9, 0.6 * span, 0.5 * span, tgt5), tgt5, E(5))
    look(orbit(a0 + 1.1, 0.5 * span, 0.42 * span, tgt5), tgt5, Z(5))
    Tv = Vector((T.x, T.y, 0))
    look(P + Tv * 70 + R * 35 + Vector((0, 0, 30)), P, E(6))
    look(P + Tv * 22 + R * 12 + Vector((0, 0, 7)), P + R * 2, E(6) + 45)
    look(P + Tv * 4.6 + R * 4.4 + Vector((0, 0, 2.4)), P + R * (rw * 0.55) - Tv * 2.0 + Vector((0, 0, -0.6)), E(6) + CUT_LABEL_FROM)
    look(P + Tv * 4.0 + R * 3.4 + Vector((0, 0, 2.1)), P + R * (rw * 0.5) - Tv * 2.0 + Vector((0, 0, -0.6)), Z(6))
    pv = [O["Pipes"].matrix_world @ x.co for x in O["Pipes"].data.vertices]
    pm = min(pv, key=lambda q: (q - mid).length)
    look(orbit(a0 + 1.2, 230, 140, pm), pm, E(7) + 10)
    look(orbit(a0 + 1.6, 180, 110, pm), pm, Z(7))
    n8 = Z(8) - E(8) + 1
    for k in range(n8):
        t = k / (n8 - 1); s = t * F["L"]
        p = at(s)
        ahead = s + 60
        if ahead <= F["L"]:
            q = at(ahead)
        else:  # past the end: extend along the end tangent
            e1, e0 = at(F["L"]), at(F["L"] - 1.0); q = e1 + (e1 - e0).normalized() * (ahead - F["L"])
        look(p + Vector((0, 0, 9.0)), q + Vector((0, 0, 3.0)), E(8) + k)
    if only:
        frames = only
    else:
        frames = range(1, NF + 1)
    from bpy_extras.object_utils import world_to_camera_view
    # label anchors: mid-depth of each layer on the cut face, 0.6 of the right lane width from the CL
    sec_top = P.z
    anchors = []
    depth = 0.0
    hit = None
    for k, L in enumerate(meta["pavement_layers"]):
        mid_d = depth + L["thickness_m"] / 2; depth += L["thickness_m"]
        # layer top at this offset: highest vertex of the layer solid near the anchor plan position
        q = P + R * (rw * 0.6)
        ztop = interp(sec["right"]["fg"], rw * 0.6) - org["z"] - (depth - L["thickness_m"])  # top of this layer
        anchors.append((f"{L['name']} {L['thickness_m'] * 1000:.0f} mm", Vector((q.x, q.y, ztop - (L['thickness_m'] / 2)))))
    proj = {}
    for f in frames:
        sc.frame_set(f); sc.render.filepath = os.path.join(OUT, f"f{f:04d}.png")
        if ST[6][0] + CUT_LABEL_FROM <= f <= ST[6][1]:
            proj[f] = [(t, tuple(world_to_camera_view(sc, cam, a))[:2]) for t, a in anchors]
        bpy.ops.render.render(write_still=True)
    old = json.load(open(os.path.join(OUT, "labels.json"))) if os.path.exists(os.path.join(OUT, "labels.json")) else {}
    old.update({str(k): v for k, v in proj.items()}); json.dump(old, open(os.path.join(OUT, "labels.json"), "w"))
    print("walkthrough frames done")


# ---------------------------------------------------------------- compose stage
def profile_inset(F, w, h):
    import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
    fig = plt.figure(figsize=(w / 100, h / 100), dpi=100); ax = fig.add_axes([0.09, 0.2, 0.88, 0.68])
    eg = F["eg"]; ax.plot([e[0] for e in eg], [e[1] for e in eg], color="#7a6a4f", lw=2, label="Existing ground")
    ss = [F["L"] * i / 400 for i in range(401)]; ax.plot(ss, [fg_at(F["pvi"], s) for s in ss], color="#d0142c", lw=2.5, label="Design grade")
    ax.plot([p[0] for p in F["pvi"]], [p[1] for p in F["pvi"]], "o", color="#d0142c", ms=4, mfc="white")
    ax.set_xlabel("Station (m)"); ax.set_ylabel("Elevation (m)"); ax.grid(alpha=0.3); ax.legend(loc="upper right", frameon=False, fontsize=11)
    ax.set_title("Profile along the selected centreline", fontsize=13, loc="left")
    fig.patch.set_facecolor("white"); fig.patch.set_alpha(0.93)
    fig.canvas.draw(); import numpy as np
    from PIL import Image
    img = Image.frombuffer("RGBA", fig.canvas.get_width_height(), bytes(fig.canvas.buffer_rgba())); plt.close(fig); return img


def compose(D, FR, out):
    from PIL import Image, ImageDraw, ImageFont
    import subprocess
    F = facts(D); cap = captions(F)
    fb = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"; fr = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
    first = Image.open(os.path.join(FR, "f0001.png")); W, H = first.size; s = W / 1920
    T1, T2 = ImageFont.truetype(fb, int(44 * s)), ImageFont.truetype(fr, int(30 * s)); TB = ImageFont.truetype(fr, int(22 * s))
    inset = profile_inset(F, int(820 * s), int(380 * s))
    LB = json.load(open(os.path.join(FR, "labels.json"))) if os.path.exists(os.path.join(FR, "labels.json")) else {}
    ff = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-",
                           "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo", "-shortest", "-c:v", "libx264", "-preset", "slow", "-crf", "20",
                           "-pix_fmt", "yuv420p", "-profile:v", "high", "-movflags", "+faststart", "-c:a", "aac", "-b:a", "64k", out], stdin=subprocess.PIPE)
    for f in range(1, NF + 1):
        im = Image.open(os.path.join(FR, f"f{f:04d}.png")).convert("RGBA")
        st = next(k for k, (a, b) in ST.items() if a <= f <= b); a, b = ST[st]
        al = min(1.0, (f - a) / 9, (b - f) / 9 + (0.0 if st < 8 else 1.0)); al = max(0.0, al)
        ov = Image.new("RGBA", im.size, (0, 0, 0, 0)); d = ImageDraw.Draw(ov)
        t, sub = cap[st]
        bw = max(d.textlength(t, font=T1), d.textlength(sub, font=T2)) + 60 * s
        d.rounded_rectangle([40 * s, H - 190 * s, 40 * s + bw, H - 40 * s], radius=int(14 * s), fill=(255, 255, 255, int(225 * al)))
        d.text((70 * s, H - 172 * s), t, font=T1, fill=(20, 30, 50, int(255 * al)))
        d.text((70 * s, H - 104 * s), sub, font=T2, fill=(60, 70, 90, int(255 * al)))
        d.text((W - 40 * s - d.textlength("civil-mcp", font=TB), 30 * s), "civil-mcp", font=TB, fill=(40, 50, 70, 200))
        if st == 6 and str(f) in LB:
            for k, (txt, (u, v)) in enumerate(LB[str(f)]):
                ax_, ay_ = u * W, (1 - v) * H
                tx_, ty_ = 70 * s, 150 * s + k * 70 * s
                d.line([(tx_ + d.textlength(txt, font=T2) + 16 * s, ty_ + 20 * s), (ax_, ay_)], fill=(20, 30, 50, 230), width=max(2, int(3 * s)))
                d.ellipse([ax_ - 7 * s, ay_ - 7 * s, ax_ + 7 * s, ay_ + 7 * s], fill=(255, 255, 255, 255), outline=(20, 30, 50, 255), width=max(2, int(3 * s)))
                d.rounded_rectangle([tx_ - 14 * s, ty_ - 6 * s, tx_ + d.textlength(txt, font=T2) + 14 * s, ty_ + 46 * s], radius=int(10 * s), fill=(255, 255, 255, 235))
                d.text((tx_, ty_), txt, font=T2, fill=(20, 30, 50, 255))
        if st == 4:
            ia = inset.copy(); ia.putalpha(ia.getchannel("A").point(lambda v: int(v * al)))
            ov.alpha_composite(ia, (int(W - 40 * s - ia.width), int(H - 40 * s - ia.height)))
        im.alpha_composite(ov); ff.stdin.write(im.convert("RGB").tobytes())
    ff.stdin.close(); ff.wait()
    print("wrote", out)


if __name__ == "__main__":
    if "--" in sys.argv:
        blender_main(sys.argv[sys.argv.index("--") + 1:])
    elif len(sys.argv) > 1 and sys.argv[1] == "compose":
        compose(*sys.argv[2:5])
