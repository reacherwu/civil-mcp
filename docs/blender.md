# Blender visualisation

For concept and preliminary visualisation only; the scene is not a design deliverable.

1. Run the design to a corridor (and optionally route alternatives, pipe networks), then call `export_blender_scene`. It writes `scene.glb`, `scene.json` and `blender_scene.py` into the project folder.
2. Render headless:

   ```bash
   blender -b -P blender_scene.py -- /path/to/project --frames 240 --res 1280x720 --samples 16
   # options: --engine CYCLES|BLENDER_EEVEE_NEXT|BLENDER_WORKBENCH  --no-video  --no-usd
   ```

   The script renders `render/overview.png`, `render/alt-<name>.png` (one per alternative, the others hidden), `render/driver-1..3.png` and `render/flythrough.mp4`. It also writes `scene.usdc`, `scene.blend` and `render/render-manifest.json`. Cycles on CPU is the default because EEVEE needs a GPU context, which headless servers often lack.
3. Georeference: Blender coordinates are metres relative to the origin in `scene.json` (`origin.e`, `origin.n`, `origin.z`, `origin.epsg`). Add the origin back to get world coordinates. The same origin is stored in the glTF scene `extras.georeference`.

## Scene contents

- `Existing ground`: the full survey TIN, for pre-design views (route alternatives).
- `Existing ground (trimmed to daylight)`: the same surface with the corridor footprint cut out. Triangles crossing a daylight line are clipped exactly to it, every vertex keeps the survey elevation, and T-junctions are removed.
- `Corridor closure (skirts and end faces)`: vertical faces along the daylight lines wherever the slope edge (straight between sections) and the ground differ by more than 0.01 m, including retaining walls, and at the first and last section between the design line and the ground, so the corridor is closed against the terrain without moving it.
- `Pavement layer n: <name> <thickness> mm`: one closed solid per assembly pavement layer, from the left to the right paved edge. The top of each layer follows the finished surface (crossfall and superelevation) lowered by the layers above it; thicknesses are vertical, as in the typical section. `scene.json` lists them under `pavement_layers`.
- `blender_scene.py` hides the untrimmed ground and the surface-only pavement mesh when these are present.

## Animated walkthrough

`scripts/render_walkthrough_animation.py` renders a captioned step-by-step animation (terrain, alternatives, alignment, profile, earthwork, pavement cutaway with layer labels, drainage, drive-through) from the same files; see the header of the script.

## Pairing with a Blender MCP server

[blender-mcp](https://github.com/ahujasid/blender-mcp) (community project, not affiliated with the Blender Foundation) runs a socket server inside Blender through an add-on and exposes it as an MCP server (`uvx mcp-for-blender`; older configs use `uvx blender-mcp`). A typical session with both servers connected to the same client:

1. civil-mcp: design, then `export_blender_scene`.
2. Blender (with the add-on running): open `scene.blend` or import `scene.glb`.
3. blender-mcp: ask the agent to restyle materials, add trees or vehicles, or set up presentation cameras. Re-run `blender_scene.py` headless for repeatable renders.

civil-mcp does not depend on blender-mcp, and nothing in the design is changed by edits made in Blender.

## Tested

Blender 4.2.3 (official Linux build), headless, Cycles CPU, 8 samples, 640×360, 48 frames, on the demo project: 8 stills, `flythrough.mp4`, `scene.usdc` and `scene.blend` written in 16 s on 8 CPU cores. CI does not download Blender; there, the `.glb` is checked structurally (container, accessors, ground mesh back to world = LandXML EG points).
