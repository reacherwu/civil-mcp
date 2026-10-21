# Headless Blender scene builder for civil-mcp exports.
#
#   blender -b -P blender_scene.py -- <dir with scene.glb + scene.json> [--frames 240] [--res 1280x720]
#                                     [--samples 16] [--engine CYCLES|BLENDER_EEVEE_NEXT|BLENDER_WORKBENCH]
#                                     [--no-video] [--no-usd]
#
# Builds the scene from scene.glb (terrain, pavement, earthwork, walls, drainage, route
# alternatives), adds sun and sky colour, renders stills (overview, one per route alternative,
# driver-eye views) and a flythrough MP4 along the selected alignment, exports USD
# (scene.usdc) and saves scene.blend. Coordinates in Blender are metres relative to the
# georeference origin stored in scene.json (E0, N0, Z0, EPSG).
import json, math, os, sys
import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
if not argv:
    sys.exit("usage: blender -b -P blender_scene.py -- <scene dir> [options]")
D = os.path.abspath(argv[0])
opt = {"frames": 240, "res": "1280x720", "samples": 16, "engine": "CYCLES", "video": True, "usd": True}
i = 1
while i < len(argv):
    a = argv[i]
    if a == "--no-video": opt["video"] = False
    elif a == "--no-usd": opt["usd"] = False
    else:
        opt[a.lstrip("-")] = argv[i + 1]; i += 1
    i += 1
meta = json.load(open(os.path.join(D, "scene.json")))
out = os.path.join(D, "render"); os.makedirs(out, exist_ok=True)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.join(D, "scene.glb"))
sc = bpy.context.scene
w, h = (int(v) for v in str(opt["res"]).split("x"))
sc.render.resolution_x, sc.render.resolution_y, sc.render.resolution_percentage = w, h, 100
sc.render.engine = opt["engine"]
if sc.render.engine == "CYCLES":
    sc.cycles.device = "CPU"; sc.cycles.samples = int(opt["samples"]); sc.cycles.use_denoising = False
sc.world = bpy.data.worlds.new("Sky"); sc.world.use_nodes = True
bg = sc.world.node_tree.nodes["Background"]; bg.inputs[0].default_value = (0.55, 0.7, 0.9, 1.0); bg.inputs[1].default_value = 0.8
sun = bpy.data.objects.new("Sun", bpy.data.lights.new("Sun", "SUN")); sc.collection.objects.link(sun)
sun.data.energy = 3.5; sun.rotation_euler = (math.radians(50), 0, math.radians(35))
cam = bpy.data.objects.new("Camera", bpy.data.cameras.new("Camera")); sc.collection.objects.link(cam); sc.camera = cam
cam.data.lens = 28; cam.data.clip_end = 20000

def look(loc, tgt):
    cam.location = Vector(loc)
    cam.rotation_euler = (Vector(tgt) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()

objs = {o.name: o for o in sc.objects if o.type == "MESH"}
alts = [n for n in objs if n.startswith("Alternative ")]
# with a corridor: show the ground trimmed to the daylight lines and the pavement layer solids
# (the top layer's upper face is the finished surface, so the surface-only mesh is hidden)
if meta.get("ground_trimmed_mesh") in objs:
    objs["Existing ground"].hide_render = True
if any(n.startswith("Pavement layer ") for n in objs) and "Road pavement" in objs:
    objs["Road pavement"].hide_render = True
bb = meta["bbox_local"]  # [xmin, ymin, zmin, xmax, ymax, zmax]
c = Vector(((bb[0] + bb[3]) / 2, (bb[1] + bb[4]) / 2, (bb[2] + bb[5]) / 2)); span = max(bb[3] - bb[0], bb[4] - bb[1])
written = []
def still(name):
    sc.render.image_settings.file_format = "PNG"; sc.render.filepath = os.path.join(out, name)
    bpy.ops.render.render(write_still=True); written.append(name)

look(c + Vector((-0.55 * span, -0.75 * span, 0.6 * span)), c); still("overview.png")
for a in alts:
    for b in alts:
        objs[b].hide_render = b != a
    still(f"alt-{a.split(' ', 1)[1]}.png")
for b in alts:
    objs[b].hide_render = False
path = [Vector(p) for p in meta["camera_path"]]
eye = Vector((0, 0, meta.get("eye_height_m", 1.2)))
for k, f in enumerate((0.15, 0.5, 0.85)):
    j = int(f * (len(path) - 1)); look(path[j] + eye, path[min(j + 15, len(path) - 1)] + eye * 0.8); still(f"driver-{k + 1}.png")
if opt["video"]:
    n = int(opt["frames"]); sc.frame_start, sc.frame_end = 1, n
    for fr in range(1, n + 1):
        t = (fr - 1) / max(n - 1, 1) * (len(path) - 1); j = int(t)
        p = path[j].lerp(path[min(j + 1, len(path) - 1)], t - j)
        q = path[min(j + 15, len(path) - 1)]
        look(p + Vector((0, 0, 12.0)), q + Vector((0, 0, 2.0)))
        cam.keyframe_insert("location", frame=fr); cam.keyframe_insert("rotation_euler", frame=fr)
    sc.render.image_settings.file_format = "FFMPEG"; sc.render.ffmpeg.format = "MPEG4"; sc.render.ffmpeg.codec = "H264"
    sc.render.fps = 24; sc.render.filepath = os.path.join(out, "flythrough.mp4")
    bpy.ops.render.render(animation=True); written.append("flythrough.mp4")
if opt["usd"]:
    bpy.ops.wm.usd_export(filepath=os.path.join(D, "scene.usdc")); written.append("scene.usdc")
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(D, "scene.blend")); written.append("scene.blend")
json.dump({"written": written, "options": opt, "blender": bpy.app.version_string}, open(os.path.join(out, "render-manifest.json"), "w"), indent=1)
print("civil-mcp scene done:", written)
