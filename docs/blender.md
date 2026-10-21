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

## Pairing with a Blender MCP server

[blender-mcp](https://github.com/ahujasid/blender-mcp) (community project, not affiliated with the Blender Foundation) runs a socket server inside Blender through an add-on and exposes it as an MCP server (`uvx mcp-for-blender`; older configs use `uvx blender-mcp`). A typical session with both servers connected to the same client:

1. civil-mcp: design, then `export_blender_scene`.
2. Blender (with the add-on running): open `scene.blend` or import `scene.glb`.
3. blender-mcp: ask the agent to restyle materials, add trees or vehicles, or set up presentation cameras. Re-run `blender_scene.py` headless for repeatable renders.

civil-mcp does not depend on blender-mcp, and nothing in the design is changed by edits made in Blender.

## Tested

Blender 4.2.3 (official Linux build), headless, Cycles CPU, 8 samples, 640×360, 48 frames, on the demo project: 8 stills, `flythrough.mp4`, `scene.usdc` and `scene.blend` written in 16 s on 8 CPU cores. CI does not download Blender; there, the `.glb` is checked structurally (container, accessors, ground mesh back to world = LandXML EG points).
