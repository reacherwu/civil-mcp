# LandXML 1.2 export (`export_landxml`)

`export_landxml` writes one LandXML 1.2 file (`design.xml` in the demo). The design package
(`export_design_package`, see [design-package.md](design-package.md)) carries a copy of the same file.

**Status: not yet tested in Civil 3D or OpenRoads Designer.** Every export is validated against the
official LandXML 1.2 XSD ([`schema/LandXML-1.2.xsd`](../schema/LandXML-1.2.xsd)) in CI, and re-read by an
independent Python parser ([`scripts/landxml_roundtrip.py`](../scripts/landxml_roundtrip.py)) in SI and US
units. Schema validity and a self-consistent round trip do not prove that a given Civil 3D or ORD version
imports every element as intended. The import notes below come from vendor documentation; none of them has been tried with these files.

## Units

The file is written in the session units of `set_units`, or in the units given by the `units` argument:

| Session / argument | `<Units>` element | Lengths, elevations | Areas | Volumes | Pipe diameters | Flows |
|---|---|---|---|---|---|---|
| SI, or `units: "metric"` | `Metric linearUnit="meter"` | m | m² (`squareMeter`) | m³ | mm (`millimeter`) | m³/s (`cubicMeterSecond`) |
| US with international foot, or `units: "imperial"` | `Imperial linearUnit="foot"` | ft (`elevationUnit="feet"`) | ft² (`squareFoot`) | yd³ (`cubicYard`) | in (`inch`) | ft³/s (`cubicFeetSecond`) |
| US with US survey foot, or `units: "imperial_survey_ft"` | `Imperial linearUnit="USSurveyFoot"` | US survey ft | ft² | yd³ | in | ft³/s |

Angles and directions are decimal degrees. Coordinates in feet are written with 6 decimals and in metres
with 4 (TIN points) or 6 (alignment geometry). `import_landxml` reads `linearUnit` and converts back to metres.

Before this change the file was always metric, also in a US session.

## Element mapping

| civil-mcp object | LandXML 1.2 element(s) | Notes |
|---|---|---|
| Project (design basis) | `Project name desc` | desc holds the engineer of record and the standards profile |
| Generator | `Application name desc manufacturer version manufacturerURL` | desc states PRELIMINARY |
| Horizontal CRS | `CoordinateSystem name="EPSG:n" epsgCode` | only when the EPSG code is known (`import_terrain` keys or the `epsg` argument of `load_points`); the demo writes 32610 (WGS 84 / UTM 10N, from `data/SOURCE.md`). No vertical datum is written. |
| Survey points | `CgPoints/CgPoint` | |
| Existing ground TIN | `Surface name="EG"/Definition surfType="TIN" elevMin elevMax` with `Pnts/P` and `Faces/F` | |
| Finished grade TIN | `Surface name="Corridor FG"` with `SourceData/Boundaries/Boundary bndType="outer"` (the daylight polygon, closed `PntList2D`) and `SourceData/Breaklines/Breakline brkType="standard"` (`CL`, `EP-L`, `EP-R`, `DAYLIGHT-L`, `DAYLIGHT-R`, `PntList3D` at every corridor station), then the `Definition` TIN | The breaklines and boundary are written for the importer; the TIN faces are complete on their own. |
| Parcels | `Parcels/Parcel area desc` with `CoordGeom/Line`; holes as a nested `Parcels name="<id> holes"` holding `<id>-hole-k` parcels | LandXML 1.2 has no hole element for parcels; `Exclusions` carries an area only. How Civil 3D treats nested parcels has not been checked. |
| Alignment | `Alignment name length staStart` with `CoordGeom` | |
| Line | `Line staStart length` with `Start`, `End` | |
| Circular arc | `Curve rot crvType="arc" staStart radius length delta chord tangent external midOrd` with `Start`, `Center`, `End`, `PI` | delta in degrees |
| Clothoid | `Spiral staStart length radiusStart radiusEnd rot spiType="clothoid" chord` (`INF` at the tangent end); full spirals (one end tangent) also `theta constant totalX totalY tanLong tanShort`; with `Start`, `PI`, `End` | `dirStart`/`dirEnd` are not written. |
| Station equations | none | civil-mcp alignments have none, so no `StaEquation` is written. |
| Design profile | `Profile staStart/ProfAlign` with `PVI` and `ParaCurve length` | symmetric parabolas only |
| Existing ground profile | `Profile/ProfSurf name="EG"` with `PntList2D` (station elevation), sampled about every 10 m | |
| Superelevation | one `Superelevation staStart staEnd` per curve with e > 0: `BeginRunoutSta`, `BeginRunoffSta` (level crown), `FullSuperSta`, `FullSuperelev` (percent), `RunoffSta`, `StartofRunoutSta`, `EndofRunoutSta`, `AdverseSE="non-adverse"` | level-crown stations from normal crown / (e + normal crown) along the runoff, the same linear rotation civil-mcp uses |
| Cross sections | `CrossSects/CrossSect sta` with `CrossSectSurf name="EG"` and `"FG"` (offset, elevation) and `DesignCrossSectSurf name="FG"` with `CrossSectPnt code` points | `CrossSectSurf` FG is kept for older readers of civil-mcp files |
| Pipe networks | `PipeNetwork pipeNetType`, `Struct elevRim elevSump` with `Center`, `CircStruct`, `Invert elev flowDir refPipe`; `Pipe refStart refEnd length slope desc` with `CircPipe diameter desc`, `PipeFlow flowIn hglUp hglDown`, and a `Feature` with `Property` material / invertUp / invertDown | LandXML 1.2 `Pipe` has no material attribute, so material goes into `desc` and a `Feature` property. Structure diameters are placeholders (civil-mcp does not size structures). Manning's n is not written. |

## Documented import behaviour

### Autodesk Civil 3D

- Autodesk's list of supported LandXML data includes Units, Application, CoordinateSystem (EPSG),
  CgPoints, Alignments with CoordGeom lines, curves and spirals, AlignPIs, Cant, StaEquation,
  Superelevation, Profile (ProfSurf and ProfAlign), Parcels, Surfaces (SourceData, Breaklines, Definition
  TIN) and PipeNetworks (Pipe, Structs, diameterUnit, PipeFlow). `CrossSects` is not in that list, so
  Civil 3D is not expected to import the cross sections; they are there for other readers.
  Source: [About Supported LandXML Data for Import and Export (Civil 3D 2025)](https://help.autodesk.com/cloudhelp/2025/ENU/Civil3D-UserGuide/files/GUID-4D10ABA5-5EA0-41A8-BB61-C3F446CE7C6B.htm).
- The LandXML Settings import tab controls how alignments, spirals and surfaces come in; the help notes
  spirals imported as fixed entities.
  Source: [Import tab (LandXML Settings dialog box)](https://help.autodesk.com/cloudhelp/2025/ENU/Civil3D-UserGuide/files/GUID-FB6846E6-9E14-4C2D-B06A-E96FBD1399DB.htm).
- Autodesk forum users report shifted or scaled data when the file units and the drawing units differ.
  Write the file in the drawing's units (`units` argument) and check the import settings.
- Civil 3D's .NET API has no public LandXML import call; LandXML import goes through the UI command
  (`LANDXMLIN`). Source: [Autodesk forum, Importing LandXML in Civil 3D using APIs](https://forums.autodesk.com/t5/civil-3d-customization-forum/importing-landxml-in-civil3d-using-apis/td-p/13352909).

### Bentley OpenRoads Designer

- Horizontal and vertical geometry come in through *Import Geometry*, which reads LandXML alignments and
  profiles. Source: [ORD 2025.1 help, Import Geometry](https://docs.bentley.com/LiveContent/web/OpenRoads%20Designer-v2025.1/Help/en/topics/405971/GUID-27296ADB-2AD5-BFA9-8F68-ACA684F45255.html).
- Terrain models are created from LandXML surfaces through the terrain import.
  Source: [ORD 2025 help, LandXML terrain import](https://docs.bentley.com/LiveContent/web/OpenRoads%20Designer-v2025/Help/en/topics/43823/GUID-2624E81F-9075-7299-FCDF-21830E8C0D85.html).
- Oregon DOT's guide walks through geometry exchange by LandXML between ORD and other tools:
  [Geometry Export/Import Using LandXML (ODOT)](https://www.oregon.gov/odot/EAST/Documents/GeometryExportImportUsingLandXML.pdf).
- Bentley [KB0018569](https://bentleysystems.service-now.com/community?id=kb_article&sysparm_article=KB0018569)
  brings Civil 3D alignment XML into ORD with Import Geometry (Geometry tab > General Tools > Import/Export).
  [KB0023390](https://bentleysystems.service-now.com/community?id=kb_article_view&sysparm_article=KB0023390)
  describes every element restarting at station 0 when a file has no stored stationing; civil-mcp writes
  `staStart` on the alignment and on every Line, Curve and Spiral for that reason.

## What CI checks

For both demo runs (`demo-out` in SI, `demo-out-us` in US customary with the international foot):

1. Every `*.xml` under the output folders (the top-level `design.xml` and the package copy) is validated
   against the LandXML 1.2 XSD with `xmlschema`: 0 errors.
2. `scripts/landxml_roundtrip.py` parses the file with ElementTree, converts feet back to metres from
   `<Units>`, evaluates the geometry with its own code and compares it with the run's SI JSON
   (`corridor.json`, `superelevation.json`, `network-*.json`). Results of the run on 2026-10-21 (UTC+8):

| Check (25 per run) | SI run | US run (ft) | Tolerance |
|---|---|---|---|
| Spiral end from numerical integration vs `End` (6 spirals), max | 1.07e-6 m | 8.03e-7 m | 1e-3 m |
| Alignment E,N at 76 corridor stations vs `corridor.json`, max | 1.07e-6 m | 6.6e-7 m | 1e-3 m |
| Profile elevation (own PVI/ParaCurve evaluator) vs section `cl_elev`, max | 4.07e-7 m | 1.34e-7 m | 1e-3 m |
| EG TIN sampled at 76 section points vs `ground_cl`, max | 0 m | 1.17e-7 m | 1e-3 m |
| FG TIN at 76 centreline points vs `cl_elev`, max | 4.91e-5 m | 1.57e-7 m | 1e-3 m |
| FG breakline CL vertices vs section x, y, `cl_elev`, max | 4.97e-5 m | 1.5e-7 m | 1e-3 m |
| Superelevation stations vs `superelevation.json`, max | 4.97e-7 m | 1.31e-7 m | 1e-3 m |
| `FullSuperelev` vs e, max | 3.6e-5 % | 3.6e-5 % | 1e-4 % |
| Pipe inverts, 25 pipes (Struct `Invert` and pipe properties), max | 4.91e-5 m | 1.51e-5 m | 1e-3 m |
| Pipe diameters, max | 0 m | 1.22e-4 m (inches to 2 decimals) | 1e-3 m / 0.127 mm |
| Pipe lengths, max | 4.99e-5 m | 1.44e-5 m | 1e-3 m |

   plus the presence checks (Units, ProfSurf, all five breaklines, closed outer boundary, one
   Superelevation per superelevated curve, materials): 25 / 25 pass in both runs.
3. `cargo test` round trips (`crates/civil-core/tests/landxml.rs`): metric, international foot and US
   survey foot files are written and read back by `import_landxml`'s parser; alignment points every
   7.3 m and profile elevations every 5 m agree within 1e-5 m.
4. The base independent check re-reads the cross sections, surfaces and parcels from `design.xml` in
   both units.

## Limits

- Not yet tested in Civil 3D or OpenRoads Designer (no licence on the build machine). Element support
  above is taken from vendor documentation.
- No `StaEquation`, `AlignPIs`, `Cant`, `dirStart`/`dirEnd`, or Manning's n.
- Cross sections use `CrossSects`, which Civil 3D's import list does not include.
- Parcel holes use nested parcels, an interpretation; LandXML 1.2 has no parcel hole element.
- Structure sizes in `CircStruct` are placeholders.
- Intersection return patches, grading objects and walls are not written to LandXML (they are in the
  3D DXF model).
