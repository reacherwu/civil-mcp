#!/usr/bin/env bash
# Third-party CAD verification of civil-mcp DXF output (not run in CI: needs ODA File Converter,
# LibreDWG and LibreCAD installed locally). usage: scripts/cad_roundtrip.sh <demo-out> <work-dir>
# env: ODA=<path to ODAFileConverter AppRun>, LRD=<libredwg bin dir>
set -u
OUT=$1; W=$2; rm -rf "$W"; mkdir -p "$W"/{in,dwg,back,lrd,pdf}
cp "$OUT"/*.dxf "$W/in/"
# 1) ODA: DXF -> DWG (AutoCAD 2018) with audit, then DWG -> DXF back
xvfb-run -a "$ODA" "$W/in" "$W/dwg" ACAD2018 DWG 0 1 >"$W/oda1.log" 2>&1
xvfb-run -a "$ODA" "$W/dwg" "$W/back" ACAD2018 DXF 0 1 >"$W/oda2.log" 2>&1
# 2) LibreDWG reads ODA's DWG
for f in "$W"/dwg/*.dwg; do b=$(basename "$f" .dwg); "$LRD/dwgread" -O JSON -o "$W/lrd/$b.json" "$f" >"$W/lrd/$b.log" 2>&1; echo "$b dwgread rc=$?" ; done
# 3) LibreCAD renders the original DXF to PDF
QT_QPA_PLATFORM=offscreen librecad dxf2pdf -a "$W"/in/*.dxf >"$W/lc.log" 2>&1; echo "librecad rc=$?"; mv "$W"/in/*.pdf "$W/pdf/"
ls -la "$W"/dwg "$W"/back "$W"/pdf
