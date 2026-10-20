"""Regenerate section 3 of DELIVERABLES.md from deliverables-status.csv (written by generate_deliverables).
usage: python scripts/gen_deliverables_md.py <demo-out>/deliverables-status.csv DELIVERABLES.md [more.md ...]"""
import csv, sys
rows = list(csv.DictReader(open(sys.argv[1])))
cnt = {k: sum(r["status"] == k for r in rows) for k in ("implemented", "partial", "interface", "gap")}
out = [f"## 3. civil-mcp v5 coverage (generated from `deliverables-status.csv`, written by `generate_deliverables`)\n",
       f"Totals: {cnt['implemented']} implemented, {cnt['partial']} partial, {cnt['interface']} interface (licensed-specialist hand-off only), {cnt['gap']} gap (of {len(rows)} items). \"Implemented\" means the software produces the item for the preliminary / concept design stage; every item still needs review and sealing by the engineer in responsible charge. \"Interface\" means the software writes a data hand-off template and a deliverables checklist for a licensed specialist and does not produce the item itself.\n"]
for cat, title in (("calc", "3a. Calculations"), ("report", "3b. Reports and registers"), ("drawing", "3c. Drawings")):
    out.append(f"### {title}\n\n| Item | Status | Output / note |\n|---|---|---|")
    out += [f"| {r['item']} | {r['status']} | {r['output_or_gap_note']} |" for r in rows if r["category"] == cat]
    out.append("")
out.append("### 3d. Remaining gaps, interfaces and partial items\n")
out.append("Gaps (not produced by the software):\n")
out += [f"- {r['item']}" for r in rows if r["status"] == "gap"] or ["- none at item level; see the partial items below"]
out.append("\nLicensed-specialist interfaces (hand-off template + checklist only):\n")
out += [f"- {r['item']}: {r['output_or_gap_note']}" for r in rows if r["status"] == "interface"]
out.append("\nPartial items and what is missing:\n")
out += [f"- {r['item']}: {r['output_or_gap_note']}" for r in rows if r["status"] == "partial"]
out.append("")
sec = "\n".join(out) + "\n"
for f in sys.argv[2:]:
    t = open(f).read()
    a = t.index("## 3. civil-mcp"); b = t.index("### 3e.")
    open(f, "w").write(t[:a] + sec + t[b:])
print(cnt, len(rows))
