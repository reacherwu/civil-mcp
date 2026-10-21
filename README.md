<div align="center">

# civil-mcp

**Road design you can hand to an AI agent, and check.** About 65 Civil 3D-style design tools, served as a local MCP server. Each result carries the clause it was checked against, every call goes into a hash-chained audit log, and nothing leaves *PRELIMINARY* until a licensed engineer approves it.

[![CI](https://github.com/reacherwu/civil-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/reacherwu/civil-mcp/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-stdio%20server-6f42c1.svg)](https://modelcontextprotocol.io/)
[![Standards](https://img.shields.io/badge/profiles-US%20%C2%B7%20CA%20%C2%B7%20UK%20%C2%B7%20AU%20%C2%B7%20NZ-2ea44f.svg)](#standards-coverage-by-country)

![The agent designing a road on USGS terrain. Every frame is drawn from files the demo wrote.](docs/media/civil-mcp-agent-design.gif)

| **65** MCP tools | **288** design checks, **0** mandatory failures | **38**-sheet drawing set (PDF + DXF) | **242 / 242** independent re-checks within tolerance |
|:---:|:---:|:---:|:---:|

<sub>All numbers come from the committed demo: US AASHTO profile, 50 km/h rural collector, 5,607 USGS 3DEP survey points near Palo Alto CA, measured 2026-10-20. Layered elastic pavement analysis matches the published KENPAVE example to ≤ 0.09 %.</sub>

**This is not a substitute for a licensed engineer.** It is a set of tools an engineer can direct, audit and approve.

</div>

---

## Contents

[Works with](#works-with-mcp-clients) · [How it works](#how-it-works) · [Why](#why) · [Quick start](#quick-start) · [Client setup](#mcp-client-setup) · [Feature gallery](#feature-gallery) · [Drawing set](#drawing-set-all-38-sheets) · [Sheet frames](#sheet-frames-and-drafting-standards) · [Standards](#standards-coverage-by-country) · [Validation](#validation-summary) · [Deliverables](#deliverables-matrix) · [Limits](#limits) · [Roadmap](#roadmap) · [License](#license-and-attribution)

## Works with (MCP clients)

civil-mcp is a standard [Model Context Protocol](https://modelcontextprotocol.io/) server that uses the **local stdio transport**. Your AI tool starts it as a local program on your machine and exchanges JSON-RPC messages with it over stdin/stdout. Nothing is hosted, and no network port is opened. Any MCP-capable client that can launch a local command can use it after you add one config entry.

| Client | Where the entry goes (format checked against the official docs, 2026-10-20) | Status |
|---|---|---|
| [Claude Code](https://docs.claude.com/en/docs/claude-code/mcp) | `claude mcp add civil -- /path/to/civil-mcp` (writes `~/.claude.json`), or `.mcp.json` → `mcpServers` | per docs, not yet tested end to end |
| [Claude Desktop](https://modelcontextprotocol.io/docs/develop/connect-local-servers) | `claude_desktop_config.json` → `mcpServers` | per docs, not yet tested end to end |
| [Cursor](https://cursor.com/docs/context/mcp) | `.cursor/mcp.json` or `~/.cursor/mcp.json` → `mcpServers` | per docs, not yet tested end to end |
| [Codex](https://developers.openai.com/codex/mcp) (CLI / IDE extension / ChatGPT desktop app) | `codex mcp add civil -- /path/to/civil-mcp`, or `~/.codex/config.toml` → `[mcp_servers.civil]` | per docs, not yet tested end to end |
| [Gemini CLI](https://geminicli.com/docs/tools/mcp-server/) | `gemini mcp add civil /path/to/civil-mcp`, or `~/.gemini/settings.json` → `mcpServers` | per docs, not yet tested end to end |
| [VS Code (GitHub Copilot agent mode)](https://code.visualstudio.com/docs/copilot/customization/mcp-servers) | `MCP: Add Server` → *Command (stdio)*, or `.vscode/mcp.json` → `servers` | per docs, not yet tested end to end |
| Raw JSON-RPC over stdio | `initialize` → `tools/list` | **tested**: protocol `2024-11-05`, 65 tools listed. The CI test `crates/civil-mcp/tests/stdio.rs` drives a UK-profile session over stdio |

**What does not work yet**

- **Web-only clients** that can only reach a *remote* MCP server over HTTP / Streamable HTTP, for example ChatGPT on the web (plugins), claude.ai connectors and other browser-hosted agents. civil-mcp has no HTTP transport yet. That is first on the [roadmap](#roadmap).
- **Agents without MCP support.** You can still run the tools from Rust (`civil_core::tools::Session::call`), as the demo does.

## How it works

```mermaid
flowchart LR
    E1["👷 Engineer<br/>set_design_basis<br/>(standards profile, speed,<br/>engineer-only inputs)"] --> A["🤖 Agent<br/>calls MCP tools<br/>(route, profile, corridor,<br/>drainage, pavement …)"]
    A --> C["✅ Checks + audit<br/>every check cites its clause;<br/>every call hash-chained<br/>in audit.jsonl"]
    C -->|"failures → agent revises"| A
    C --> R["👷 Engineer review<br/>generate_review_report<br/>record_review (approve / reject)"]
    R -->|"rejected"| A
    R -->|"approved"| S["📐 Sheets + deliverables<br/>PDF / DXF / LandXML / BoQ / reports<br/>(PRELIMINARY until approved)"]
```

1. **The engineer sets the basis first.** They pick the standards profile and project inputs, and enter anything the profile refuses to default, such as copyrighted table values.
2. **The agent designs inside that basis.** It calls tools, reads the checks and revises. In the demo it re-sizes the storm network seven times and lowers the cover twice.
3. **Every call is logged.** `audit.jsonl` records inputs, outputs and SHA-256 hashes, chained like a ledger. `verify_audit_log` detects tampering.
4. **Only the engineer approves.** `record_review` requires an `engineer:<name>` actor. Until then, every sheet says *PRELIMINARY – NOT FOR CONSTRUCTION*.

## Why

- **Civil design software is slow and fragile at corridor scale.** Users report waiting on every operation for a 4-mile corridor ([r/civil3d](https://www.reddit.com/r/civil3d/comments/1ecojp2/cooridor_question/)), files that corrupt and TINs that fail after adding breaklines ([r/civilengineering](https://www.reddit.com/r/civilengineering/comments/13sjpp6/civil_3d_frustration/)), a highly upvoted thread on [why all civil modelling packages are so slow](https://www.reddit.com/r/civilengineering/comments/1kpexvz/why_are_seemingly_all_civil_modelling_packages_so/), and a corridor section editor that takes minutes to open ([Autodesk forum](https://forums.autodesk.com/t5/civil-3d-forum/corridor-section-editor-very-slow-to-open-and-close/td-p/12940839)).
- **AI design tools exist, but as black-box plug-ins** ([Allsite.ai](https://www.allsite.ai/), [drtwrk](https://drtwrk.io/), [RoadEng](https://www.softree.com/RoadEng)). civil-mcp is open, cites every clause, and is re-checked by independent code.
- **The law requires the engineer to stay in charge.** North Carolina [21 NCAC 56.0701](http://reports.oah.state.nc.us/ncac/title%2021%20-%20occupational%20licensing%20boards%20and%20commissions/chapter%2056%20-%20engineers%20and%20surveyors/21%20ncac%2056%20.0701.html) and Florida [FBPE](https://fbpe.org/regulation-in-the-ai-era/) require responsible charge. NSPE's [2025 discussion of AI](https://www.nspe.org/career-growth/pe-magazine/issue-3-2025/use-artificial-intelligence-engineer-practice) says that reviewing documents after preparation, without involvement in the design, "does not satisfy" responsible charge. So the workflow puts the engineer before the agent (the basis) and after it (approval), with the audit trail in between.

## Quick start

```bash
git clone https://github.com/reacherwu/civil-mcp && cd civil-mcp
cargo run --release -p civil-demo -- demo-out     # ~5 s after build; writes everything below
```

<details><summary><b>What the demo writes to <code>demo-out/</code></b> (≈ 80 files)</summary>

| Group | Files |
|---|---|
| Drawings | `sheets.pdf` (38 sheets, A1), `sheet-01.dxf` … `sheet-38.dxf`, `model.dxf` |
| Model exchange | `design.xml` (LandXML 1.2: surface, alignment, profile, pipe networks), `corridor.json`, `setting-out.csv` |
| Reports | `design-report.md`, `drainage-report.md`, `pavement-report.md`, `constructability.md`, `rsa-brief.md`, `specialist-handoffs.md`, `review.html` |
| Registers | `parameter-register.csv`, `departures-register.csv`, `deliverables-status.csv`, `utility-register.csv` |
| Quantities and cost | `boq.csv`, `cost-estimate.csv`, `cost-plan.csv`, `esc-quantities.csv`, `land-requirements.csv`, `footprint.csv` |
| Calculations | `hydrology.csv`, `culverts.csv`, `network-storm.json`, `network-sanitary.json`, `pavement-mechanistic.csv`, `rigid-pavement.csv`, `foundation-design.csv`, `wall-types.csv`, `wall-reinforcement.csv`, `slope-sections.csv`, `signal-timing.csv`, `capacity.csv`, `landscape.csv`, `lighting-poles.csv`, `sign-schedule.csv`, `no-passing-zones.csv`, `route-alternatives.json`, `profile-optimisation.json`, `quantities.json`, `protection.json`, `intersection-X1.json` |
| Trail | `audit.jsonl` (hash chain), `run-log.jsonl` (every tool call and result) |

</details>

Committed copies: [sheets.pdf](docs/sheets.pdf) · [review.html](docs/review.html) · [design.xml](docs/design.xml) · [run-log.jsonl](docs/run-log.jsonl) · 1080p video: [docs/media/civil-mcp-agent-design.mp4](docs/media/civil-mcp-agent-design.mp4)

Run the MCP server itself:

```bash
cargo build --release -p civil-mcp      # binary: target/release/civil-mcp (stdio)
```

## MCP client setup

Replace `/abs/path/to/civil-mcp` with the absolute path of `target/release/civil-mcp`. Each format below was checked against the client's official documentation on 2026-10-20. **Config format per docs, not yet tested end to end in that client.** The server side is tested over raw stdio JSON-RPC.

<details open><summary><b>Claude Code</b> (<a href="https://docs.claude.com/en/docs/claude-code/mcp">docs</a>)</summary>

```bash
claude mcp add --transport stdio civil -- /abs/path/to/civil-mcp
# or share it with the team via .mcp.json:  claude mcp add --scope project --transport stdio civil -- /abs/path/to/civil-mcp
```
</details>

<details><summary><b>Claude Desktop</b> (<a href="https://modelcontextprotocol.io/docs/develop/connect-local-servers">docs</a>): <code>claude_desktop_config.json</code></summary>

macOS `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows `%APPDATA%\Claude\claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "civil": { "command": "/abs/path/to/civil-mcp", "args": [] }
  }
}
```
</details>

<details><summary><b>Cursor</b> (<a href="https://cursor.com/docs/context/mcp">docs</a>): <code>.cursor/mcp.json</code> or <code>~/.cursor/mcp.json</code></summary>

```json
{
  "mcpServers": {
    "civil": { "type": "stdio", "command": "/abs/path/to/civil-mcp", "args": [] }
  }
}
```
</details>

<details><summary><b>Codex</b> (<a href="https://developers.openai.com/codex/mcp">docs</a>): CLI, IDE extension and ChatGPT desktop app share <code>~/.codex/config.toml</code></summary>

```bash
codex mcp add civil -- /abs/path/to/civil-mcp
```
```toml
[mcp_servers.civil]
command = "/abs/path/to/civil-mcp"
args = []
```
</details>

<details><summary><b>Gemini CLI</b> (<a href="https://geminicli.com/docs/tools/mcp-server/">docs</a>): <code>~/.gemini/settings.json</code> or <code>.gemini/settings.json</code></summary>

```bash
gemini mcp add civil /abs/path/to/civil-mcp
```
```json
{
  "mcpServers": {
    "civil": { "command": "/abs/path/to/civil-mcp", "args": [] }
  }
}
```
</details>

<details><summary><b>VS Code — GitHub Copilot agent mode</b> (<a href="https://code.visualstudio.com/docs/copilot/customization/mcp-servers">docs</a>): <code>.vscode/mcp.json</code> (note the key is <code>servers</code>)</summary>

Run `MCP: Add Server` → *Command (stdio)*, or:

```json
{
  "servers": {
    "civil": { "type": "stdio", "command": "/abs/path/to/civil-mcp", "args": [] }
  }
}
```
VS Code currently labels `.vscode/mcp.json` a compatibility destination. Portable files (`~/.copilot/mcp-config.json`, workspace `.mcp.json`) use the `mcpServers` key instead.
</details>

A first prompt to try: *"Open a project in ./job, set a US AASHTO basis for 60 km/h (I am engineer:Jane, e_max 0.08, f_max 0.17, max grade 7 %), load survey.csv, and propose three route alternatives with their checks."*

---

## Feature gallery

The features below follow the pipeline order. Every image is a crop of a sheet the demo actually produced; [all 38 full-size sheets](#drawing-set-all-38-sheets) are further down. Clause references are the ones the tools cite in their output. Values that come from copyrighted tables are *engineer inputs* and are marked that way in the parameter register.

### 1 · Survey points → TIN

5,607 USGS 3DEP points are triangulated (Delaunay) into a TIN, which every later step samples for ground levels.

<img src="docs/media/step-03-tin.jpg" width="49%" alt="TIN"/> <img src="docs/media/step-02-points.jpg" width="49%" alt="survey points"/>

| Points | Triangles | Extent | Elevation range |
|---:|---:|---|---|
| 5,607 | 11,089 | ≈ 895 × 895 m, 4.48 m DEM | 57–163 m |

**Tools:** `load_points`, `build_surface`, `import_landxml` · **Checked:** 2D/3D surface areas match Civil 3D LandXML exports to ≤ 2e-12 relative.

### 2 · Route alternatives and selection

A least-cost search on a 10 m grid produces three corridors. Each is built and checked in full, and the selection rule and reasons are recorded.

<img src="docs/img/crop/sheet-02.png" width="100%" alt="route alternatives sheet"/>

| Alternative | Length | Mandatory failures | Cut / fill m³ | Wall concrete m³ | Cost proxy |
|---|---:|---:|---:|---:|---:|
| **A-direct (selected)** | 1,136 m | **0** | 22,618 / 51,310 | 6,581 | 317,776 |
| B-contour-following | 1,088 m | 3 | 39,482 / 69,081 | 4,691 | 299,615 |
| C-separate-corridor | 1,129 m | 0 | 20,967 / 100,404 | 77,824 | 2,578,395 |

Rule: fewest mandatory failures, then lowest cost proxy. Rates are placeholders. **Tools:** `generate_route_alternatives`, `select_route` · **Checked:** route costs re-derived with SciPy `csgraph` Dijkstra to 1e-6.

### 3 · Horizontal alignment, spirals and superelevation

Circular curves with clothoid transitions, and AASHTO superelevation with runoff by maximum relative gradient.

<img src="docs/img/crop/sheet-05.png" width="100%" alt="superelevation diagram"/>

| Curve | R m | Spiral in/out m | Δ | e (design) | Runoff required → provided |
|---|---:|---:|---:|---:|---|
| 1 | 180 | 40 / 40 | 40.96° | 0.0336 | 28.9 → 40 m |
| 2 | 200 | 40 / 40 | 42.68° | 0.0303 | 27.1 → 40 m |
| 3 | 200 | 40 / 40 | −13.17° | 0.0303 | 27.1 → 40 m |

Clauses: AASHTO Green Book 2018 (7th ed.) minimum radius, spiral transitions, superelevation runoff (Table 3-15 values engineer-entered), SSD 63.4 m. **Tools:** `create_alignment`, `design_superelevation`, `check_standards` · **Checked:** clothoid chords to 1e-6 m, e and runoff lengths exact.

### 4 · Profile and earthwork balance

The profile is sampled from the TIN. A coordinate-descent optimiser then moves the VPIs to balance cut and fill and shorten haul, and it rejects any candidate that fails a grade or vertical-curve check.

<img src="docs/img/crop/sheet-03.png" width="100%" alt="plan and profile"/>

| | Before | After |
|---|---:|---:|
| Imbalance (m³) | −30,954 | **+48** |
| Haul moment (m³·m) | 19.1 M | **9.0 M** |
| Wall concrete (m³) | 6,581 | **2,908** |
| Evaluations / accepted moves | | 71 / 7 |

**Tools:** `sample_ground_profile`, `suggest_profile`, `create_profile`, `optimize_profile` · Clauses: AASHTO vertical curves (K, SSD), max grade 8 % (engineer input).

### 5 · Corridor and cross sections

Assemblies are swept along the alignment. The corridor includes lanes, curbs, sidewalks, curve widening, ditches, benches and retaining walls, with daylight to the TIN at every section.

<img src="docs/img/crop/sheet-04.png" width="49%" alt="typical sections"/> <img src="docs/img/crop/sheet-07.png" width="49%" alt="cross sections"/>

| Sections | Cut m³ | Fill m³ | Composite-grid check | Curve widening | Walls |
|---:|---:|---:|---|---|---|
| 76 | 43,976 | 39,064 | +0.20 % / −0.20 % | 0.26–0.30 m | 7 sections, 80 m |

**Tools:** `create_assembly`, `build_corridor`, `get_section`, `grade` · **Checked:** section areas re-computed independently with NumPy (max difference 0.015 %), volumes to 0.0003 %.

### 6 · Pavement: empirical, layered elastic, rigid and foundation

There are four independent methods: empirical AASHTO 93 (flexible and rigid); mechanistic design by a multilayer elastic solver written from Burmister's theory; AGPT02 Ch. 9 for rigid pavements; and a performance check of the foundation.

<img src="docs/img/crop/sheet-06.png" width="49%" alt="pavement sheet"/> <img src="docs/img/crop/sheet-36.png" width="49%" alt="rigid pavement sheet"/>

| Method | Result |
|---|---|
| AASHTO 93 flexible (adopted) | W18 6.385e5 ESAL; SN 2.91 required / **3.38 provided**; AC 110 / base 150 / subbase 150 mm |
| AASHTO 93 rigid (option) | PCC 160 mm on 150 mm subbase |
| Layered elastic analysis, dual wheels | ε_t 250 µε (AC bottom), ε_v 537 µε (subgrade top) → N_subgrade 6.97e8, N_asphalt 1.14e6 at 100 mm AC |
| AGPT02 Ch. 9 rigid (comparison) | 225 mm JRCP, fatigue 0 %, erosion 79.2 % (≤ 100 %) |
| Foundation (layered elastic E0) | 0.40 m subbase → E0 101 MPa ≥ 100 MPa target |

<details><summary>Layered elastic analysis strains at the three evaluation points</summary>

| Point | x m | ε_t bottom AC (µε) | ε_v top subgrade (µε) | w subgrade (mm) |
|---|---:|---:|---:|---:|
| under wheel | 0.165 | 250.09 | 492.78 | 0.458 |
| wheel edge | 0.073 | 227.59 | 529.00 | 0.472 |
| between wheels | 0.000 | 201.52 | 536.97 | 0.475 |

</details>

Clauses: AASHTO Guide 1993; Huang *Pavement Analysis and Design* (2004) Ch. 2 / KENLAYER; Austroads AGPT02 Ch. 8–9; DMRB CD 225/226 (UK). **Tools:** `design_pavement`, `check_pavement_mechanistic`, `design_rigid_pavement`, `design_foundation` · **Checked:** layered elastic results vs the published KENPAVE example: deflection 0.058 %, σz 0.028 %, εr 0.088 %, εz 0.061 %; vs [PyMastic](https://github.com/Mostafa-Nakhaei/PyMastic): ≤ 0.19 %.

### 7 · Retaining walls: type selection, stability, reinforcement

Wall heights come from the corridor, and the type is chosen by height and cut/fill. The options are gravity, cantilever, profile reshape, MSE for tall fills and soil nail for tall cuts. Each type gets its own external stability checks, and cantilever walls also get stem and footing design.

<img src="docs/img/crop/sheet-25.png" width="49%" alt="wall type selection"/> <img src="docs/img/crop/sheet-26.png" width="49%" alt="wall reinforcement"/>

| Station | H m | Type | Base / reinforcement L | FS sliding | e (m) | σv kPa |
|---|---:|---|---:|---:|---:|---:|
| 0+500 R | 13.8 | MSE | 9.7 | 1.85 | 1.16 | 372 |
| 0+515 R | 16.0 | MSE | 12.9 | 2.15 | 1.15 | 399 |
| 0+520 R | 16.3 | MSE | 13.7 | 2.24 | 1.12 | 400 |
| 0+540 L / R | 15.2 / 16.1 | MSE | 11.1 / 13.2 | 1.94 / 2.18 | 1.22 / 1.14 | 400 |
| 0+780 R | 5.3 | cantilever | 3.8 | 1.69 | 0.51 | 155 |
| 0+800 R | 6.7 | cantilever | 4.7 | 1.73 | 0.61 | 189 |
| 0+820 R | 6.1 | cantilever | 4.3 | 1.71 | 0.58 | 176 |

Cantilever 0+800: Vu 200 kN/m ≤ φVc 432 kN/m; crack-control spacing limit 206 mm ≥ 100 mm provided; heel Mu 258, toe Mu 185 kNm/m. Clauses: FHWA GEC 11 (MSE), GEC 7 (soil nail), FHWA-NHI-07-071 (type selection), AASHTO LRFD 5.7 / 5.6.7, BS 8006 (UK), AS 4678 (AU/NZ), CSA S6 (CA). **Tools:** `select_wall_types`, `design_wall_reinforcement` · **Checked:** MSE, shear, crack spacing and footing moments re-derived in Python (SciPy quad), max difference ≤ 0.06 %.

### 8 · Slope stability

Critical slip circles on the cut and fill sections are found with Bishop simplified and Spencer, on layered soil with pore pressure.

<img src="docs/img/crop/sheet-23.png" width="100%" alt="slope stability"/>

| Section | H m | FS Bishop | FS Spencer | Required |
|---|---:|---:|---:|---:|
| cut 0+711 L | 8.0 | 1.41 | 1.38 | 1.30 |
| cut 0+100 L | 7.9 | 1.40 | 1.38 | 1.30 |
| cut 0+020 L | 7.5 | 1.42 | 1.40 | 1.30 |
| fill 0+560 L | 6.6 | 1.63 | 1.62 | 1.30 |
| fill 0+940 L | 5.2 | 1.70 | 1.68 | 1.30 |
| fill 0+860 R | 5.0 | 1.60 | 1.58 | 1.30 |

Soil parameters are DEMO placeholders until a GI report exists. **Tool:** `check_slope_stability` · **Checked:** independent Bishop implementation, differences < 0.4 %.

### 9 · Roadside protection, barriers and ditch linings

The tool finds clear-zone hazards, lays out barrier runs with length of need, and designs HEC-15 linings for each ditch reach.

<img src="docs/img/crop/sheet-14.png" width="100%" alt="protection layout"/>

| Hazards | Barrier runs | Barrier length | Ditch reaches | Linings chosen |
|---:|---:|---:|---:|---|
| 35 | 8 | 842 m | 5 | concrete; gravel mulch D50 25 / 50 mm; riprap D50 100 / 250 mm |

Clauses: AASHTO Roadside Design Guide 2011 (clear zone Ch. 3, LoN Ch. 5; table values engineer-entered), FHWA HEC-15. UK uses DMRB CD 377 / RRRAP inputs. **Tools:** `design_protection`, `export_rrrap_inputs`.

### 10 · Intersection, signals and capacity

This covers the at-grade intersection with sight triangles, signal warrants for each country, a Webster cycle with HCM delay, and link and unsignalised capacity with LOS.

<img src="docs/img/crop/sheet-20.png" width="49%" alt="intersection"/> <img src="docs/img/crop/sheet-34.png" width="49%" alt="signals"/>

| Item | Result |
|---|---|
| Intersection X1 at 0+570 | 10 checks, 0 mandatory failures; ISD required 104.3 m |
| Signal warrant | MUTCD Warrant 1 **met**: Condition A 8 h (8 required) |
| Timing | Webster C₀ 27.6 s → cycle 40 s (min), lost time 8 s, Y 0.383 |
| Phases | major: green 20.4 s, X 0.48, delay 7.4 s · minor: green 11.6 s, X 0.48, delay 14.1 s |
| Intersection delay / LOS | **9.4 s/veh, LOS A** |
| Link capacity | v_p 482 pc/h, f_HV 0.943, v/c 0.28, **LOS B** |

Clauses: MUTCD 11th ed. Part 4 (US), TAC warrant handbook points (CA), engineer-entered local thresholds (UK/AU/NZ), HCM 7th ed., ITE change interval. **Tools:** `design_intersection`, `check_intersection_sight`, `design_signals`, `analyse_capacity`.

### 11 · Hydrology and culverts

Each crossing has a catchment (area, flow length and slope are inputs). Peak flow comes from the method of the profile in force: rational (US/CA), ARR (AU/NZ) or an FEH input (UK). Culverts are sized by HDS-5 inlet and outlet control.

<img src="docs/img/crop/sheet-21.png" width="49%" alt="drainage areas"/> <img src="docs/img/crop/sheet-22.png" width="49%" alt="culverts"/>

| Crossing | Area ha | tc min | Q m³/s | Culvert | HW/D | Control | V_out m/s |
|---|---:|---:|---:|---|---:|---|---:|
| X1 (0+215) | 2.86 | 5.0 | 0.239 | in cut: flow routed to the ditch | | | |
| X2 (0+330) | 6.34 | 5.1 | 0.524 | D 750 mm | 0.79 | inlet | 6.6 |
| X3 (0+535) | 5.94 | 5.3 | 0.482 | D 600 mm | 1.17 | inlet | 4.9 |

**Tools:** `design_hydrology`, `design_culverts`, `design_subsurface_drainage` (t50 0.75 h, outlets every 150 m) · Clauses: FHWA HDS-5, HEC-22; ARR 2019; FEH (UK, engineer input).

### 12 · Storm and sanitary networks (HGL / EGL)

Gravity networks are sized by Manning, with the HEC-22 HGL/EGL computed through structures. The run log shows the agent iterating.

<img src="docs/img/crop/sheet-15.png" width="49%" alt="storm profile"/> <img src="docs/img/crop/sheet-16.png" width="49%" alt="sanitary profile"/>

| Network | Attempts | What the agent changed | Final |
|---|---:|---|---|
| Storm (HEC-22) | 7 | upsized pipes; resizing stalled twice → min cover 1.2 m, then 1.5 m | 12 pipes, **0 failures** |
| Sanitary | 2 | min slope 0.4 % failed P-VMIN → 0.8 % | 13 pipes, **0 failures** |

**Tool:** `design_pipe_network` · **Checked:** Manning full and partial flow (brentq), Ten States peaking and HGL/EGL ends re-derived to ≤ 1e-6. Pipe lengths and slopes match Civil 3D 2025 exports on 411 pipes to 1.3e-9 m.

### 13 · Earthwork and mass haul

The mass haul applies shrink and bulk factors, free haul and overhaul, balance points, and borrow or waste.

<img src="docs/img/crop/sheet-18.png" width="100%" alt="mass haul"/>

| Final ordinate | Balance points (stations) | Borrow | Waste | Overhaul |
|---:|---|---:|---:|---:|
| 514 m³ | 0+562, 0+666, 0+941, 1+115 | 0 | 571 m³ | 4.57 M m³·m |

**Tool:** `compute_quantities` · **Checked:** ordinates exact; haul moment within 0.07 %.

### 14 · Quantities, BoQ and cost

The BoQ is grouped by FHWA FP-14 sections (US), with equivalent mappings for each other profile. Its 47 line items also feed the cost plan.

<details><summary>First 12 BoQ lines (<code>boq.csv</code>)</summary>

| Code | Item | Unit | Qty |
|---|---|---|---:|
| FP-14 204 | Roadway excavation (bank) | m³ | 43,975.6 |
| FP-14 204 | Embankment construction | m³ | 39,063.8 |
| FP-14 204 | Disposal of surplus | m³ | 571.3 |
| FP-14 204 | Overhaul | m³·m | 4,567,258 |
| FP-14 Div 500 | Asphalt concrete | m³ | 946.8 |
| FP-14 301 | Aggregate base | m³ | 1,291.1 |
| FP-14 301 | Aggregate subbase | m³ | 1,291.1 |
| FP-14 609 | Curb and gutter | m | 1,135.7 |
| FP-14 615 | Sidewalk | m² | 2,271.3 |
| FP-14 258 | RC cantilever wall concrete | m³ | 229.8 |
| FP-14 255 | MSE select reinforced fill | m³ | 7,975.8 |
| FP-14 255 | MSE facing panels | m² | 596.3 |

</details>

**Tools:** `generate_deliverables`, `cost_plan`, `land_requirements` (footprint 34,815 m²; 4 lots).

### 15 · Reports

The tools write Markdown and HTML reports generated from the model state. The demo produces: a design report (basis, parameter register, every check with its clause), drainage, pavement, constructability (9 stages, 4 risks / hold points), an RSA brief, specialist hand-offs (bridges, environment), and a review page for the engineer.

| Report | Contents |
|---|---|
| [`review.html`](docs/review.html) | every check (288), departures, audit-chain status, approval form |
| `design-report.md` | design basis, curve and VPI tables, optimiser log, check summary by clause |
| `constructability.md` | key facts, 9-stage sequence, risk register with HOLD points |

**Tools:** `generate_review_report`, `constructability_report`, `specialist_handoffs`, `generate_deliverables`.

### 16 · Everything else in the set

<img src="docs/img/crop/sheet-19.png" width="32%" alt="locality"/> <img src="docs/img/crop/sheet-32.png" width="32%" alt="landscape"/> <img src="docs/img/crop/sheet-31.png" width="32%" alt="ESC"/>

| Feature | Demo result | Tool |
|---|---|---|
| Locality plan on an offline OpenStreetMap basemap | 5,667 OSM features, UTM grid, ODbL attribution | `locality_plan` |
| Signing and marking | 4 no-passing zones, 8 signs, 3,407 m of lines | `design_marking_signing` |
| Lighting | 24 poles at 48 m | `design_lighting` |
| Utilities | 4 register rows with conflicts | `register_utilities` |
| Right-of-way | 34,815 m² footprint over 4 lots | `land_requirements` |
| Erosion and sediment control | 747 m sediment fence, 41 check dams | `plan_erosion_control` |
| Landscape | 22 trees, 11,570 m² TRM hydroseed, 607 m² shrubs | `landscape_concept` |
| Temporary traffic management | 4 stages | `plan_traffic_management` |
| Subsurface drainage | t50 0.75 h, outlets every 150 m | `design_subsurface_drainage` |

### 17 · Audit log and approval gate

Every tool call is appended to `audit.jsonl` with its actor, inputs, outputs, SHA-256 hashes and the previous entry's hash. `verify_audit_log` recomputes the chain. Approval is a separate, explicit act.

```json
{"actor":"engineer:REVIEWER-PLACEHOLDER","hash":"fe11010e…","inputs":{"basis":{"design_speed_kmh":50.0,"e_max":0.08,…}},"prev_hash":"…","tool":"set_design_basis"}
{"actor":"agent","hash":"3c49811d…","inputs":{"path":"…/usgs3dep_palo_alto_points.csv"},"outputs":{"file_sha256":"3cf736ec…","points":5607},…}
```

| Audit entries (demo) | Chain status | Sheet status before approval |
|---:|---|---|
| 58 | **OK** (`verify_audit_log`) | PRELIMINARY – NOT FOR CONSTRUCTION |

**Tools:** `verify_audit_log`, `generate_review_report`, `record_review`, `record_departure`. MCP cannot authenticate a human, and `engineer:<name>` is a declaration. Production deployments need real authentication in front of `set_design_basis` and `record_review`.

### 18 · Interoperability: LandXML, DXF, DWG

| Format | Direction | Verification |
|---|---|---|
| LandXML 1.2 | import + export | XSD-valid (0 errors). Civil 3D 2022/2023/2025 exports: surfaces to 2e-12, a 23.47 km alignment to 2.6e-10 m, 411 pipes to 1.3e-9 m |
| DXF (AC1009 / R12) | export, 39 files | ezdxf audit 0 errors; LibreCAD 2.2 rendered all DXFs in the v4 run (not re-run for v6) |
| DWG | via ODA File Converter | DXF→DWG→DXF round trip on all 39 files with identical entity counts (0 mismatches); LibreDWG reads all |
| PDF | export | 38 A1 sheets with title block, audit head hash, seal box |

**Tools:** `import_landxml`, `export_landxml`, `generate_drawings`.

---

## Drawing set (all 38 sheets)

Click a thumbnail to open the full-size sheet (2000 px PNG). The vector PDF is [docs/sheets.pdf](docs/sheets.pdf).

| | | | |
|:---:|:---:|:---:|:---:|
| [<img src="docs/img/thumbs/sheet-01.png" width="200"/>](docs/img/sheets/sheet-01.png)<br/><sub>01 Cover, index, notes</sub> | [<img src="docs/img/thumbs/sheet-02.png" width="200"/>](docs/img/sheets/sheet-02.png)<br/><sub>02 Route alternatives</sub> | [<img src="docs/img/thumbs/sheet-03.png" width="200"/>](docs/img/sheets/sheet-03.png)<br/><sub>03 Plan and profile</sub> | [<img src="docs/img/thumbs/sheet-04.png" width="200"/>](docs/img/sheets/sheet-04.png)<br/><sub>04 Typical sections</sub> |
| [<img src="docs/img/thumbs/sheet-05.png" width="200"/>](docs/img/sheets/sheet-05.png)<br/><sub>05 Superelevation</sub> | [<img src="docs/img/thumbs/sheet-06.png" width="200"/>](docs/img/sheets/sheet-06.png)<br/><sub>06 Pavement and subgrade</sub> | [<img src="docs/img/thumbs/sheet-07.png" width="200"/>](docs/img/sheets/sheet-07.png)<br/><sub>07 Cross sections 1</sub> | [<img src="docs/img/thumbs/sheet-08.png" width="200"/>](docs/img/sheets/sheet-08.png)<br/><sub>08 Cross sections 2</sub> |
| [<img src="docs/img/thumbs/sheet-09.png" width="200"/>](docs/img/sheets/sheet-09.png)<br/><sub>09 Cross sections 3</sub> | [<img src="docs/img/thumbs/sheet-10.png" width="200"/>](docs/img/sheets/sheet-10.png)<br/><sub>10 Cross sections 4</sub> | [<img src="docs/img/thumbs/sheet-11.png" width="200"/>](docs/img/sheets/sheet-11.png)<br/><sub>11 Cross sections 5</sub> | [<img src="docs/img/thumbs/sheet-12.png" width="200"/>](docs/img/sheets/sheet-12.png)<br/><sub>12 Cross sections 6</sub> |
| [<img src="docs/img/thumbs/sheet-13.png" width="200"/>](docs/img/sheets/sheet-13.png)<br/><sub>13 Cross sections 7</sub> | [<img src="docs/img/thumbs/sheet-14.png" width="200"/>](docs/img/sheets/sheet-14.png)<br/><sub>14 Roadside protection</sub> | [<img src="docs/img/thumbs/sheet-15.png" width="200"/>](docs/img/sheets/sheet-15.png)<br/><sub>15 Storm profile</sub> | [<img src="docs/img/thumbs/sheet-16.png" width="200"/>](docs/img/sheets/sheet-16.png)<br/><sub>16 Sanitary profile</sub> |
| [<img src="docs/img/thumbs/sheet-17.png" width="200"/>](docs/img/sheets/sheet-17.png)<br/><sub>17 Drainage details</sub> | [<img src="docs/img/thumbs/sheet-18.png" width="200"/>](docs/img/sheets/sheet-18.png)<br/><sub>18 Mass haul and quantities</sub> | [<img src="docs/img/thumbs/sheet-19.png" width="200"/>](docs/img/sheets/sheet-19.png)<br/><sub>19 Locality plan (OSM)</sub> | [<img src="docs/img/thumbs/sheet-20.png" width="200"/>](docs/img/sheets/sheet-20.png)<br/><sub>20 Intersection + sight</sub> |
| [<img src="docs/img/thumbs/sheet-21.png" width="200"/>](docs/img/sheets/sheet-21.png)<br/><sub>21 Drainage areas</sub> | [<img src="docs/img/thumbs/sheet-22.png" width="200"/>](docs/img/sheets/sheet-22.png)<br/><sub>22 Culverts</sub> | [<img src="docs/img/thumbs/sheet-23.png" width="200"/>](docs/img/sheets/sheet-23.png)<br/><sub>23 Slope stability</sub> | [<img src="docs/img/thumbs/sheet-24.png" width="200"/>](docs/img/sheets/sheet-24.png)<br/><sub>24 Subsurface drainage</sub> |
| [<img src="docs/img/thumbs/sheet-25.png" width="200"/>](docs/img/sheets/sheet-25.png)<br/><sub>25 Wall type selection</sub> | [<img src="docs/img/thumbs/sheet-26.png" width="200"/>](docs/img/sheets/sheet-26.png)<br/><sub>26 Wall reinforcement</sub> | [<img src="docs/img/thumbs/sheet-27.png" width="200"/>](docs/img/sheets/sheet-27.png)<br/><sub>27 Signing and marking</sub> | [<img src="docs/img/thumbs/sheet-28.png" width="200"/>](docs/img/sheets/sheet-28.png)<br/><sub>28 Lighting</sub> |
| [<img src="docs/img/thumbs/sheet-29.png" width="200"/>](docs/img/sheets/sheet-29.png)<br/><sub>29 Utilities</sub> | [<img src="docs/img/thumbs/sheet-30.png" width="200"/>](docs/img/sheets/sheet-30.png)<br/><sub>30 Right-of-way</sub> | [<img src="docs/img/thumbs/sheet-31.png" width="200"/>](docs/img/sheets/sheet-31.png)<br/><sub>31 Erosion and sediment</sub> | [<img src="docs/img/thumbs/sheet-32.png" width="200"/>](docs/img/sheets/sheet-32.png)<br/><sub>32 Landscape</sub> |
| [<img src="docs/img/thumbs/sheet-33.png" width="200"/>](docs/img/sheets/sheet-33.png)<br/><sub>33 Traffic management</sub> | [<img src="docs/img/thumbs/sheet-34.png" width="200"/>](docs/img/sheets/sheet-34.png)<br/><sub>34 Signals</sub> | [<img src="docs/img/thumbs/sheet-35.png" width="200"/>](docs/img/sheets/sheet-35.png)<br/><sub>35 Capacity / LOS</sub> | [<img src="docs/img/thumbs/sheet-36.png" width="200"/>](docs/img/sheets/sheet-36.png)<br/><sub>36 Rigid pavement</sub> |
| [<img src="docs/img/thumbs/sheet-37.png" width="200"/>](docs/img/sheets/sheet-37.png)<br/><sub>37 Constructability</sub> | [<img src="docs/img/thumbs/sheet-38.png" width="200"/>](docs/img/sheets/sheet-38.png)<br/><sub>38 Standard details atlas</sub> | | |

---

## Sheet frames and drafting standards

Every sheet in `generate_drawings` (PDF, per-sheet SVG and DXF) is composed into a configurable frame and title block. Five drafting profiles are built in: US, CA, UK, AU and NZ. Selecting a profile sets the default sheet size, borders, title-block fields, layer names, line weights and types, sheet numbering and revision codes. Any field can then be overridden.

| Tool | What it does |
|---|---|
| `list_sheet_frames` | Sheet sizes (ISO A0-A4, ANSI A-E, ARCH A-E1), the five profiles and their default frame, and the current frame |
| `get_drafting_standard` | One profile in full. Each parameter is marked `verified`, `formula` or `engineer` and carries its source |
| `set_sheet_frame` | Profile, size, orientation, margins, title-block fields, revision table, custom DXF/SVG/JSON template, layer map, sheet ID pattern. Logged in the audit chain. It does not void an approval |
| `preview_sheet_frame` | Renders an empty frame to `frame-preview.pdf/.svg/.dxf` and returns the layout and checks |
| `check_drawing_standards` | Runs checks FR-01 to FR-14 on the last drawing set |

```json
{"name":"set_sheet_frame","arguments":{"frame":{"profile":"UK","size":"ISO A1","orientation":"landscape",
 "fields":{"project":"A40 junction","client":"County Council","project_code":"A40J","originator":"ABC",
 "revisions":[{"rev":"P01","date":"2026-10-20","description":"Preliminary issue","by":"AGT"}]}}}}
```

| Profile | Default sheet | Title block | Layer names | Sheet IDs | First revision | Parameters (verified / formula / engineer) |
|---|---|---|---|---|---|---|
| US | ANSI D | right strip, PE seal box | NCS (`C-ROAD-CNTR`) | UDS (`G-001`, `C-101`) | A | 4 / 4 / 9 |
| CA | ISO A1 | ISO 7200, 180 mm | NCS-style | UDS-style | A | 11 / 5 / 5 |
| UK | ISO A1 | ISO 7200, 180 mm | BS 1192 style (`C-00-M_Alignment`) | ISO 19650 UK NA (`PRJ-ORG-ZZ-ZZ-DR-C-0001`) | P01 (status S3) | 13 / 7 / 3 |
| AU | ISO A1 | ISO 7200, 180 mm | ISO-style | `PRJ-C-0001` | A | 11 / 5 / 5 |
| NZ | ISO A1 | ISO 7200, 180 mm | ISO-style | `PRJ-C-0001` | A | 11 / 5 / 4 |

All ISO profiles have ISO 5457 centring marks and grid references. Agency CAD manuals (state DOT, National Highways, TfNSW, NZTA) are not shipped. Their specific values are engineer inputs with a default.

Sheet 3 (plan and profile) in each profile, rendered from the generated PDFs by `scripts/render_readme_images.py --frames`:

| US, ANSI D | CA, ISO A1 |
|---|---|
| ![US](docs/img/frames/US-ANSID-landscape-plan.png) | ![CA](docs/img/frames/CA-A1-landscape-plan.png) |
| **UK, ISO A1** | **AU, ISO A1** |
| ![UK](docs/img/frames/UK-A1-landscape-plan.png) | ![AU](docs/img/frames/AU-A1-landscape-plan.png) |
| **NZ, ISO A1** | **UK, ISO A3** |
| ![NZ](docs/img/frames/NZ-A1-landscape-plan.png) | ![UK A3](docs/img/frames/UK-A3-landscape-plan.png) |

Title-block close-ups:

| US | UK | AU, A4 portrait |
|---|---|---|
| ![US title block](docs/img/frames/US-ANSID-landscape-titleblock.png) | ![UK title block](docs/img/frames/UK-A1-landscape-titleblock.png) | ![AU A4](docs/img/frames/AU-A4-portrait-plan.png) |

Drawing-standard checks (kept separate from the 288 design checks and the approval gate):

| Check | What it covers | Severity |
|---|---|---|
| FR-01 / FR-02 / FR-03 | sheet size, margins, frame line | mandatory |
| FR-04 | title-block width | advisory |
| FR-05 | required title-block fields; checker required once approved | mandatory |
| FR-06 / FR-07 | sheet ID pattern; revision code pattern, no C revisions while preliminary | mandatory |
| FR-08 / FR-08b | seal box present; no seal shown while preliminary | mandatory |
| FR-09 / FR-09b | title-block text at least 2.5 mm; small content text | mandatory / advisory |
| FR-10 / FR-11 | content inside the drawing area; layer names | mandatory |
| FR-12 / FR-12b | scale bar; preferred scales | mandatory / advisory |
| FR-13 / FR-14 | north arrow on plans; grid references (ISO) | mandatory |

Seven profile and size combinations (`cargo run --release --example frames_gallery -p civil-demo -- out`) give 0 mandatory failures and 2 advisories each. The 2 advisories are FR-09b and FR-12b: content is drawn on an A1 layout and scaled to fit the sheet, so a smaller sheet gives smaller text and non-preferred scales. The scale bar shows the plotted scale.

## Standards coverage by country

Each profile classifies every geometric design parameter as **verified** (a value from the published source, cited), **formula** (computed from a published formula), or **engineer input** (from a copyrighted or paywalled table, or not verified; it must be entered and is flagged in the register). The counts below are measured with `cargo run -p civil-core --example profile_register` on a minimal basis.

| Profile | Verified | Formula | Engineer input | Inputs the profile will not default |
|---|---:|---:|---:|---|
| 🇺🇸 US — AASHTO Green Book 2018 | 4 | 2 | 4 | e_max, f_max, max grade, max relative gradient |
| 🇨🇦 Canada — TAC GDG 2017 | 0 | 2 | 9 | + reaction time, deceleration, eye/object height, sag K |
| 🇬🇧 UK — DMRB (CD 109 …) | **10** | 0 | 2 | max relative gradient |
| 🇦🇺 Australia — Austroads GRD | 0 | 2 | 9 | + reaction time, deceleration coefficient, sag K |
| 🇳🇿 New Zealand — Austroads + NZTA | 0 | 2 | 9 | + reaction time, deceleration coefficient, sag K |

The method the tools use in each discipline depends on the profile:

| Discipline | US | CA | UK | AU / NZ |
|---|---|---|---|---|
| Flexible pavement | AASHTO 93 (+ layered elastic) | AASHTO 93 (TAC accepts) | CD 226 | Austroads granular chart; AGPT02 layered elastic |
| Rigid pavement | AASHTO 93 | AASHTO 93 | CD 226 | AGPT02 Ch. 9 |
| Hydrology | Rational | Rational | FEH peak (engineer input) | ARR 2019 rational |
| Roadside | RDG 2011 | TAC Ch. 7 | CD 377 / RRRAP inputs | AGRD Part 6 |
| Signal warrants | MUTCD Warrant 1 | TAC priority points | local thresholds (engineer) | local thresholds (engineer) |
| Signs / markings | MUTCD 11th | MUTCDC | TSRGD 2016 / TSM | AS 1742 / NZ TCD |
| Retaining walls | GEC 11 / GEC 7 / LRFD | CSA S6 refs | BS 8006 refs | AS 4678 refs |
| BoQ coding | FHWA FP-14 | provincial (engineer) | SHW series (MCHW Vol 1) | TfNSW QA (example) / NZTA |

The CI tests cover the non-US paths: `tests/profiles_v5.rs` and `tests/profiles_v6.rs` (UK FEH gate, CD 225 foundation classes, TAC signal points, AU ARR / AGPT02).

## Validation summary

Full record: **[docs/VALIDATION.md](docs/VALIDATION.md)**. Every row runs in [CI](.github/workflows/ci.yml) unless noted.

| Check | What is compared | Result |
|---|---|---|
| Independent re-checks, base ([`independent_check.py`](scripts/independent_check.py)) | areas, volumes, mass haul, superelevation, clothoids, Manning/HGL, walls, route costs, pavement, HEC-15, BoQ, LandXML XSD, DXF audit | **91 / 91** OK |
| Independent re-checks, v5 ([`independent_check_v5.py`](scripts/independent_check_v5.py)) | hydrology, culverts (HDS-5), slope stability, subsurface drainage, walls, land, locality | **70 / 70** OK |
| Independent re-checks, v6 ([`independent_check_v6.py`](scripts/independent_check_v6.py)) | layered elastic vs PyMastic and KENPAVE, MSE, wall shear/crack/footing, signals, capacity, rigid, landscape, locality | **81 / 81** OK |
| Layered elastic vs published KENPAVE 3-layer example | deflection, σz, εr, εz | ≤ **0.088 %** |
| Civil 3D LandXML fixtures (2022/2023/2025) | surfaces, a 23.47 km alignment, 411 pipes | ≤ 2e-12 rel. / 2.6e-10 m / 1.3e-9 m |
| CAD round trip (ODA, local, not in CI) | DXF→DWG→DXF on 39 files | 0 entity-count mismatches; ezdxf audit 0 errors |
| Rust tests (`cargo test`) | unit tests with worked examples, LandXML round trip, MCP stdio, profiles, full pipeline | all pass; `cargo fmt --check` and `clippy -D warnings` clean |

The re-check scripts share no code with the Rust tools. They re-derive each value from the files the tools wrote, using NumPy, SciPy and PyMastic.

## Deliverables matrix

[docs/DELIVERABLES.md](docs/DELIVERABLES.md) maps the 55 deliverables a road design package normally contains, from basis of design to construction staging, to what civil-mcp produces.

| Implemented | Partial | Interface (hand-off to a specialist) | Gap |
|---:|---:|---:|---:|
| **52** | 1 (structures: bridges) | 2 | **0** |

## Units: SI or US customary

civil-mcp works in SI (metres) by default. You can switch between SI and US customary at any time with the `set_units` tool, and switch back the same way:

```json
{"name":"set_units","arguments":{"system":"US"}}
{"name":"set_units","arguments":{"system":"US","foot":"survey"}}
{"name":"set_units","arguments":{"system":"SI"}}
```

With `system` set to `US`:
- Tool results use US units, and keys are renamed to match (`radius_m` becomes `radius_ft`, `cut_m3` becomes `cut_cy`, `design_speed_kmh` becomes `design_speed_mph`).
- Inputs can use US keys (`radius_ft`, `design_speed_mph`) or SI keys. Lengths without a suffix are read in feet.
- Drawings use 100 ft stations (12+34.56), engineering scales (1" = 50', 1" = 5' vertical), labels in ft and in, and a scale bar in feet.
- Reports and CSV files are in US units, and the bill of quantities uses CY, SY, LF, TON and EA.

`foot` is `international` (0.3048 m, the default) or `survey` (1200/3937 m, for older state plane data). `inputs` and `outputs` can be set separately from `system` for the JSON results only. Design calculations and checks always run in SI, and the audit log stays in SI. The demo runs in US units with `cargo run --release -p civil-demo -- demo-out-us --units us`.

## Real terrain, route feasibility and switchbacks

`import_terrain` loads a GeoTIFF / Cloud-Optimised GeoTIFF DEM, an ESRI ASCII grid, or a LAS/LAZ point cloud (ASPRS class 2 ground by default) and builds the existing-ground TIN.

- **CRS.** The EPSG code comes from GeoTIFF keys or the LAS WKT/GeoKey VLRs, or from `epsg`. Geographic and Web Mercator data are projected to UTM (checked against PROJ to < 1 mm). Projected data (UTM, state plane, national grids) stay planar; international and US survey feet are converted to metres, horizontal and vertical separately.
- **Route search.** `generate_route_alternatives` adds a switchback alternative (D) that forbids moves steeper than 2 × the design maximum grade. It keeps hairpin curves when the engineer enters `hairpin_min_radius_m`, recording them as design exceptions.
- **Plausibility limits.** Every alternative is checked against fill, cut and wall-height limits (EW-FILL / EW-CUT / EW-WALL). The defaults are 15 / 15 / 10 m: agent screening values that the engineer should confirm or replace through `limits`.
- **Infeasibility stop.** When every alternative fails the grade or height limits, or no path exists, the tool stops with `INFEASIBLE` and gives the reason. The message states the elevation difference and the minimum road length that difference needs at the maximum grade.
- **Duplicates.** Alternatives that repeat an earlier path are flagged (`overlap_with_earlier_pct`).

Results on 11 runs over 9 public surfaces (USGS 3DEP, Copernicus GLO-30 for the UK and Australia, a LAZ sample in Oregon state-plane feet) are in [docs/terrain-tests.md](docs/terrain-tests.md). They were produced with `scripts/terrain_batch.py`.

## Civil 3D handoff: LandXML cross sections and 3D CAD model

- `export_landxml` now also writes `CrossSects`: EG and FG at every corridor station, offsets negative to the left, with cut/fill areas. When the terrain came from `import_terrain` in metres, it also writes `CoordinateSystem` with the EPSG code. The output validates against the LandXML 1.2 XSD, and CI re-reads the sections to check FG at the centreline against an independent parabola evaluation.
- `export_cad_model` writes `civil3d-model.dxf`, a 3D model in world coordinates and metres. It contains:
  - EG and FG TIN faces;
  - the 3D centreline;
  - corridor feature lines by point code;
  - wall tops;
  - pipes at invert;
  - structures at rim.

  Layers follow the drafting profile (NCS `V-TOPO-SURF`, `C-ROAD-SURF`, `C-ROAD-FEAT`, …) or an agency `layer_map`. CI re-reads the file with ezdxf: the face count matches the LandXML TIN faces, and the centreline z matches the profile.
- No DWG is written by the tool. LibreDWG 0.13.3 `dxf2dwg`, tried on this model, produced a DWG that read back empty, so DWG conversion stays with ODA File Converter. None of these files has been opened in Civil 3D.

## Construction-scale sheet set

`generate_construction_set` writes `construction-set.pdf`, `cs-NN.dxf/svg` and `construction-register.csv`. The main 38-sheet set is not changed. The set contains:

- **Survey control and alignment data.** Control points as entered by the engineer or surveyor, horizontal curve data (Δ, R, Ls, TS/SC/CS/ST stations) and vertical alignment data.
- **Plan-and-profile sheets.** Split at match lines on round stations, at 1:500 metric or 1" = 50' US. Each match line labels the adjoining sheet. The plan shows the existing-conditions base: contours, engineer-entered property lines and right-of-way.
- **Gridded cross sections.** At 20 m metric or 50 ft US, showing EG, FG, datum, R/W lines and construction limits.
- **Summary of quantities by pay item.** Includes earthwork split per plan sheet.

Set checks confirm a standard scale, continuous match lines and full coverage. A further check flags missing engineer inputs (ROW, property lines, control, sections at the interval).

## Blender visualisation (concept / preliminary)

`export_blender_scene` writes three files to the project folder:

- `scene.glb` (glTF 2.0): terrain, pavement, earthwork slopes, walls, pipes, structures and the route alternatives as coloured ribbons.
- `scene.json`: georeference origin E0/N0/Z0 + EPSG, a camera path along the design centreline, and the metrics for each alternative.
- `blender_scene.py`: the render script.

```bash
blender -b -P blender_scene.py -- /path/to/project --frames 240 --res 1280x720 --samples 16
```

The script renders the following into `render/`:

- an overview;
- one still per alternative;
- three driver-eye stills;
- `flythrough.mp4`.

It also exports `scene.usdc` and `scene.blend`. Tested headless with Blender 4.2.3 (Cycles, CPU). For interactive work, pair it with an open-source Blender MCP server such as [blender-mcp](https://github.com/ahujasid/blender-mcp) (`uvx mcp-for-blender`): open `scene.blend` and let the agent adjust materials, cameras or annotations. See [docs/blender.md](docs/blender.md).

## Limits

- **This is not engineering approval.** The demo "engineer" is a placeholder. Nothing here has been reviewed by a licensed engineer, and every sheet stays PRELIMINARY until a real engineer approves it.
- **Copyrighted tables are not shipped.** Values such as Green Book side friction, TAC, Austroads, CD 226 figures and AGPT02 equivalent-stress tables are engineer inputs. Transcribed values must be checked against the edition in use.
- **Geotechnical inputs are placeholders.** Slope, wall and foundation soil parameters stand in for a ground-investigation report. MSE internal stability, global stability and detailing belong to the supplier and the geotechnical engineer.
- **Bridges are out of scope.** They are handed to a structures specialist (`specialist_handoffs`).
- **Signals and capacity are concept-level only.** HCS, Synchro, SIDRA or LINSIG govern detailed design. UK/AU/NZ warrants need local thresholds.
- **Untested against real Civil 3D or AutoCAD runs.** Corridor volumes and hydraulics have not been compared with Civil 3D runs; no such public files were found. The DWGs have not been opened in AutoCAD.
- **Route search is approximate.** It runs on a 10 m grid and its cost proxy is relative.
- **One demo site.** All results come from one terrain sample.
- **Identity is declared, not proven.** MCP cannot authenticate the human approver; deployments need authentication in front of approval.
- **Sheet frames are SI only.** R12 DXF carries no lineweights (they are in PDF and SVG). Templates read lines, rectangles and text; SVG group transforms are ignored. Content is scaled from an A1 layout, so small sheets give text below 2.5 mm (advisory). The default issue date is UTC.
- **Clients are untested end to end.** See [Works with](#works-with-mcp-clients).

## Roadmap

- [ ] **Remote transport:** Streamable HTTP MCP with authentication, so web-only clients can connect and approval can be tied to a real identity.
- [ ] **End-to-end client testing:** Claude Code, Claude Desktop, Cursor, Codex, Gemini CLI and VS Code, with recorded sessions.
- [ ] Comparison against Civil 3D corridor volumes and pipe-network hydraulics, given a public or partner dataset.
- [ ] 3D wall and structure solids for clash checks, using a permissive B-rep kernel such as [truck](https://github.com/ricosjp/truck).
- [ ] More terrain samples and non-US demo projects.

## Geometry kernel

Design at this stage is TIN math plus 2D/3D alignment math, so there is no B-rep kernel. Triangulation uses [`delaunator`](https://crates.io/crates/delaunator) (ISC); everything else is in this repo. No GPL/LGPL code (OpenCASCADE, FreeCAD, SolveSpace) was read or used.

## License and attribution

- Code: **Apache-2.0** ([LICENSE](LICENSE)).
- Font: DejaVu Sans (`crates/civil-core/assets/DejaVuSans-LICENSE.txt`).
- Terrain: USGS 3DEP (public domain).
- Basemap: © OpenStreetMap contributors, ODbL 1.0. See [data/basemap/ATTRIBUTION.md](data/basemap/ATTRIBUTION.md).
- Test fixture `c3dprofile2.xml`: MIT (RustedGeom/rusted-geom).
- Independent check: [PyMastic](https://github.com/Mostafa-Nakhaei/PyMastic), cloned in CI and not redistributed.
