#!/usr/bin/env python3
"""Generates schema/design-package/v1/*.schema.json (JSON Schema 2020-12). Run from the repo root."""
import json
B = "https://github.com/reacherwu/civil-mcp/schema/design-package/v1/"
num = {"type": "number"}; nn = {"type": "number", "minimum": 0}; st = {"type": "string"}; it = {"type": "integer", "minimum": 0}
xy = {"type": "array", "items": num, "minItems": 2, "maxItems": 2}
xyz = {"type": "array", "items": num, "minItems": 3, "maxItems": 3}
numn = {"type": ["number", "null"]}
def arr(i, mn=0): return {"type": "array", "items": i, "minItems": mn}
def o(props, req=None, extra=False, desc=None):
    d = {"type": "object", "properties": props, "required": list(props) if req is None else req, "additionalProperties": extra}
    if desc: d["description"] = desc
    return d
def doc(name, title, body, defs=None):
    s = {"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": B + name, "title": title}
    s.update(body)
    if defs: s["$defs"] = defs
    return s
head = {"schema": {"const": None}, "schema_version": {"const": "1.0.0"}, "units": {"const": "SI"}}
def top(kind, props, req=None, defs=None, title=""):
    p = {"schema": {"const": f"civil-mcp/design-package/{kind}"}, "schema_version": {"const": "1.0.0"}, "units": {"const": "SI", "description": "lengths m, elevations m, areas m², volumes m³, flows m³/s, slopes m/m, angles degrees"}}
    p.update(props)
    return doc(f"{kind}.schema.json", title, o(p, (["schema", "schema_version", "units"] + (req if req is not None else list(props)))), defs)

S = {}
S["manifest"] = doc("manifest.schema.json", "civil-mcp design package manifest", o({
  "schema": {"const": "civil-mcp/design-package"},
  "schema_version": {"const": "1.0.0"},
  "created": {"type": "string", "description": "YYYY-MM-DD (box-local date of the run)"},
  "status": {"type": "string", "enum": ["PRELIMINARY"], "description": "not reviewed or approved by a licensed engineer"},
  "generator": o({"name": st, "version": st, "url": st}),
  "project": o({"name": st, "engineer_of_record": st, "standard": st, "design_speed_kmh": nn}),
  "units": o({"canonical": {"const": "SI"}, "display_system": {"type": "string", "enum": ["SI", "US"]}, "display_foot": {"type": ["string", "null"], "enum": ["international", "us_survey", None]},
              "landxml_linear_unit": {"type": "string", "enum": ["meter", "foot", "USSurveyFoot"]}}),
  "crs": o({"epsg": {"type": ["integer", "null"]}, "horizontal_unit": {"const": "m"}, "vertical_datum": {"type": ["string", "null"]}, "note": st}),
  "source_run": o({"project_dir": st, "audit_log": st, "audit_seq": it, "audit_head_sha256": st, "files_in_run": it}),
  "files": arr(o({"path": st, "role": {"type": "string", "enum": ["alignments", "profiles", "superelevation", "assemblies", "corridors", "surfaces", "pipes", "parcels", "quantities", "sheets", "landxml"]},
                  "schema": {"type": ["string", "null"]}, "media_type": st, "bytes": it, "sha256": st}), 1),
}))
elem = o({"type": {"type": "string", "enum": ["line", "arc", "spiral"]}, "sta_start": num, "length": nn, "start": xy, "end": xy,
          "direction_start_deg": {"type": "number", "description": "grid azimuth, degrees clockwise from north"},
          "direction_end_deg": num, "radius_start": numn, "radius_end": numn, "turn": {"type": ["string", "null"], "enum": ["left", "right", None]},
          "center": {"type": ["array", "null"], "items": num}, "spiral_type": {"type": ["string", "null"], "enum": ["clothoid", None]}},
         desc="radius null = infinite (tangent end of a spiral, or a line)")
S["alignments"] = top("alignments", {"alignments": arr(o({"name": st, "sta_start": num, "length": nn, "pis": arr(xy, 2),
    "station_equations": arr(o({"sta_back": num, "sta_ahead": num})), "elements": arr({"$ref": "#/$defs/element"}, 1)}), 1)},
    defs={"element": elem}, title="Horizontal alignments")
S["profiles"] = top("profiles", {"profiles": arr(o({"name": st, "alignment": st, "kind": {"type": "string", "enum": ["design", "existing_ground"]},
    "pvis": arr(o({"station": num, "elevation": num, "curve_length": nn, "curve_type": {"type": "string", "enum": ["none", "symmetric_parabola"]}})),
    "samples": arr(o({"station": num, "elevation": num})), "grades_pct": arr(num)}), 1)}, title="Vertical profiles")
S["superelevation"] = top("superelevation", {"alignment": st, "normal_crown": nn, "pivot": st, "rotation_width": nn, "curves": arr(o({
    "curve": it, "radius": nn, "turn": {"type": "string", "enum": ["left", "right"]}, "e_design": nn, "runout_start": num, "level_crown": num, "full_super_start": num,
    "full_super_end": num, "level_crown_out": num, "runout_end": num, "relative_gradient": nn, "method": st}))}, title="Superelevation by curve")
sub = o({"name": st, "kind": {"type": "string", "enum": ["lane", "shoulder", "curb_gutter", "sidewalk", "ditch", "daylight", "wall", "pavement_structure"]},
         "side": {"type": "string", "enum": ["left", "right", "both"]}, "order": it, "params": {"type": "object"}})
S["assemblies"] = top("assemblies", {"assemblies": arr(o({"name": st, "subassemblies": arr({"$ref": "#/$defs/subassembly"}, 1), "source_parameters": {"type": "object"}}), 1)},
    defs={"subassembly": sub}, title="Assemblies as ordered subassembly-like parameter sets")
S["corridors"] = top("corridors", {"corridors": arr(o({"name": st, "alignment": st, "profile": st, "regions": arr(o({
    "name": st, "sta_start": num, "sta_end": num, "assembly": st,
    "frequency": o({"tangent": nn, "curve": nn, "spiral": nn, "min_spacing": nn, "max_spacing": nn, "sample_stations": it}),
    "targets": arr(o({"kind": {"type": "string", "enum": ["surface", "width", "offset", "elevation"]}, "name": st, "role": st}, req=["kind", "name", "role"]))}), 1),
    "results": o({"cut_m3": nn, "fill_m3": nn, "sections": it, "sections_with_warnings": it, "method": st})}), 1)}, title="Corridors and regions")
S["surfaces"] = top("surfaces", {"surfaces": arr(o({"name": st, "kind": {"type": "string", "enum": ["existing_ground", "finished_grade", "datum"]},
    "landxml_file": st, "landxml_surface": st, "points": it, "triangles": it, "elev_min": num, "elev_max": num, "breaklines": arr(st), "boundary": {"type": ["string", "null"]}}), 1)},
    title="TIN surfaces stored in the LandXML file of the package")
S["pipes"] = top("pipes", {"networks": arr(o({"name": st, "kind": {"type": "string", "enum": ["storm", "sanitary", "other"]},
    "structures": arr(o({"id": st, "e": num, "n": num, "rim": num, "sump": num})),
    "pipes": arr(o({"id": st, "from": st, "to": st, "length": nn, "diameter": nn, "material": st, "invert_up": num, "invert_dn": num, "slope": num, "design_flow": nn}))}))},
    title="Pipe networks")
S["parcels"] = top("parcels", {"source": st, "parcels": arr(o({"id": st, "area": nn, "zoning": {"type": ["string", "null"]}, "no_go": {"type": "boolean"},
    "outer_rings": arr(arr(xy, 3), 1), "holes": arr(arr(xy, 3))})),
    "row": {"type": ["object", "null"], "properties": {"buffer": nn, "polygon": arr(xy, 3), "area": nn}, "required": ["buffer", "polygon", "area"], "additionalProperties": False}},
    title="Parcels and the corridor right-of-way line")
S["quantities"] = top("quantities", {"earthwork": o({"corridor_cut_m3": nn, "corridor_fill_m3": nn, "layers_m3": arr(o({"name": st, "volume_m3": nn})),
    "mass_haul": {"type": ["object", "null"]}}), "boq": arr(o({"code": st, "item": st, "unit": st, "qty": num, "basis": st}))}, title="Earthwork and bill of quantities")
S["sheets"] = top("sheets", {"sheets": arr(o({"set": st, "file": st, "page": it, "sheet_id": st, "title": st, "scale": {"type": ["string", "null"]}}))}, title="Sheets produced by the run")
for k, v in S.items():
    json.dump(v, open(f"schema/design-package/v1/{k}.schema.json", "w"), indent=1, ensure_ascii=False); open(f"schema/design-package/v1/{k}.schema.json", "a").write("\n")
print(len(S))
