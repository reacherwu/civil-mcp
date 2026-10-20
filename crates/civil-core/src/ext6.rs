//! v6 tools: wall type selection, traffic signal warrants and phasing, capacity / LOS, landscape
//! concept, AU/NZ rigid pavement (AGPT02 ch. 9 structure), constructability report.
use crate::ext::{cl_panel, csvq, hdr, ExtOut, ExtSheet, Files, Panel, B};
use crate::sheet::sta_label;
use crate::standards::{chk, Standard};
use crate::tools::Session;
use serde_json::{json, Value};
use std::fmt::Write;

pub fn tool_list() -> Vec<Value> {
    let o = |n: &str, d: &str, req: &[&str]| json!({"name": n, "description": d, "inputSchema": {"type": "object", "properties": {}, "required": req, "additionalProperties": true}});
    vec![
        o("select_wall_types", "Wall type selection for every wall section: gravity / cantilever within engineer practical height ranges, profile reshape when the change is small, otherwise MSE / reinforced soil (fill) or soil nail (cut) with external stability per FHWA GEC 11 (BS 8006-1 UK, AS 4678 AU/NZ, CSA S6 + CFEM CA cited). Records the options and the choice per section; design_wall_reinforcement then covers cantilever walls only. Inputs (defaults in brackets): cantilever_max_h_m [8], gravity_max_h_m [3], mse_l_ratio [0.7], mse_l_min_m [2.4], reinforced_gamma_knm3 [20], reinforced_phi_deg [34], sv_m [0.6], mse_fs_sliding [1.5], mse_e_frac [1/6], nail_l_ratio [0.8], nail_spacing_m [1.5], reshape_max_dz_m [1.0], mse_bearing_kpa [wall rule bearing], source.", &[]),
        o("design_signals", "Traffic signal warrant analysis and concept timing at an intersection: MUTCD Warrant 1 (Conditions A, B, 80 % combination; 70 % factor for > 40 mph or isolated), TAC priority points (CA), engineer threshold table for UK/AU/NZ; phasing (protected major left by cross-product rule), Webster cycle, green splits, ITE intergreen, HCM delay and LOS, degree of saturation. Inputs: warrants {hours [[major, minor]], major_lanes, minor_lanes, major_speed_kmh, isolated_community, tac_points, local}, timing {approaches [{name, major, through_vph, left_vph, right_vph, lanes, speed_kmh, crossing_m}], sat_flow_pcuhpl, permissive_left_pcu, lost_per_phase_s, min_cycle_s, max_cycle_s, protected_left_cross_product, source}, x_max, los_target.", &["warrants", "timing"]),
        o("analyse_capacity", "Capacity / LOS at concept depth with engineer inputs: link demand flow v_p = V·D/(PHF·f_HV), f_HV = 1/(1 + P_T(E_T − 1)), v/c against the engineer capacity; multilane density LOS (HCM basic-segment thresholds, verify), two-lane: v/c and the engineer LOS table (HCM 7 follower-density method not reproduced); priority junction movements by HCM gap acceptance (c = vc·e^(−vc·tc/3600)/(1 − e^(−vc·tf/3600))), control delay and LOS; UK RFC ≤ 0.85 and AU/NZ DOS limits reported. Inputs: link {peak_hour_vph, direction_split, phf, heavy_pct, e_t, capacity_pcphpl, lanes_per_direction, ffs_kmh, los_vc}, priority {movements [{name, volume_vph, conflicting_vph, tc_s, tf_s}]}, los_target, source.", &["link"]),
        o("landscape_concept", "Landscape concept: side-slope planting by steepness class (seed + topsoil flatter than 4H:1V, shrubs + erosion blanket to 2H:1V, hydroseed + turf reinforcement mat steeper), street trees on the frontage side outside the clear zone, sight triangles, lighting poles and walls; quantities, landscape.csv and plan sheet. Inputs: tree_spacing_m, tree_offset_m (from centreline), clear_zone_m (engineer, from the roadside design guide), side (L|R), intersection_clear_m, pole_clear_m, shrub_density_per_m2, topsoil_mm, source.", &["tree_spacing_m", "tree_offset_m", "clear_zone_m"]),
        o("design_rigid_pavement", "AU/NZ rigid pavement base thickness (AGPT02 Ch. 9 structure: LSF, effective subgrade support, fatigue + erosion damage ≤ 100 % over the axle-group spectrum). Westergaard edge stress and corner deflection with PCA (1984) fatigue/erosion relations replace the copyrighted AGPT02 tables: concept only, confirm with AGPT02. Inputs: groups [{name, load_kn, tyres, repetitions}], k_mpa_m, fcf_mpa, e_mpa, nu, lsf, tyre_kpa, tied_shoulder, base_type, h_min_mm, h_max_mm, source.", &["groups", "k_mpa_m", "fcf_mpa", "lsf", "base_type"]),
        o("design_foundation", "Pavement foundation (subgrade + capping/subbase) by performance: layered elastic surface modulus E0 = 2(1−ν²)·q·a/w0 under a plate load, stepping the first foundation layer until E0 meets the target foundation class (UK CD 225 classes 1–4: 50/100/200/400 MPa; other profiles: engineer target). Subgrade E from CBR by E = 17.6·CBR^0.64 (CD 225, verify) or entered. Inputs: subgrade_cbr_pct | subgrade_e_mpa, layers [{name, e_mpa, h_m}], h_max_m, target_class | target_e0_mpa, plate_radius_m, plate_kpa, source.", &["layers"]),
        o("constructability_report", "Constructability review from the current design: sequence of works, staging/TTM, earthwork balance and haul, wall construction by type, culverts and drainage first, utility relocations, ESC, pavement, access and working space, risk register and hold points; constructability.md and a sheet. Inputs: max_fill_h_m [10], max_grade_paving_pct [6], mse_hold_h_m [10], source.", &[]),
    ]
}

impl Session {
    pub(crate) fn x_walltypes(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let s: crate::wallsel::SelInput = serde_json::from_value(a.clone()).map_err(|e| format!("input: {e}"))?;
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let cr = crate::walls::criteria(b.standard, b.wall_criteria.as_ref())?;
        let ch = crate::wallsel::select(b.standard, c, &cr, &s)?;
        let cite = crate::wallsel::citation(b.standard);
        let mut csv = String::from("station,side,cut,h_m,exposed_h_m,trib_m,selected,reshape_dz_m,mse_l_m,mse_fs_sliding,mse_e_m,mse_sigma_v_kpa,mse_layers,mse_t_max_kn_m,nail_l_m,nail_t_kn,reason\n");
        let mut rows = vec![hdr(&["Sta", "Side", "Cut", "H m", "Selected", "MSE L m", "FS slide", "e m", "σv kPa", "Tmax kN/m", "Nail L m"])];
        for x in &ch {
            let _ = writeln!(
                csv,
                "{:.2},{},{},{:.4},{:.4},{:.3},{},{:.4},{:.2},{:.4},{:.4},{:.3},{},{:.3},{:.3},{:.2},{}",
                x.station,
                x.side,
                x.cut,
                x.h_m,
                x.exposed_h_m,
                x.trib_m,
                x.selected,
                x.reshape_dz_m,
                x.mse.l_m,
                x.mse.fs_sliding,
                x.mse.e_m,
                x.mse.sigma_v_kpa,
                x.mse.layers,
                x.mse.t_max_kn_m,
                x.nail_l_m,
                x.nail_t_kn,
                csvq(&x.reason)
            );
            rows.push(vec![
                sta_label(x.station),
                x.side.clone(),
                if x.cut { "cut".into() } else { "fill".into() },
                format!("{:.2}", x.h_m),
                x.selected.clone(),
                format!("{:.1}", x.mse.l_m),
                format!("{:.2}", x.mse.fs_sliding),
                format!("{:.2}", x.mse.e_m),
                format!("{:.0}", x.mse.sigma_v_kpa),
                format!("{:.1}", x.mse.t_max_kn_m),
                format!("{:.1}", x.nail_l_m),
            ]);
            let el = format!("wall {} {}", sta_label(x.station), x.side);
            if x.selected == "cantilever" || x.selected == "gravity" {
                o.checks.push(chk(
                    "W-TYPE-H",
                    "Wall height within the practical range of the selected type",
                    el.clone(),
                    x.h_m,
                    if x.selected == "gravity" { s.gravity_max_h_m } else { s.cantilever_max_h_m },
                    "m",
                    "max",
                    "mandatory",
                    format!("{} wall", x.selected),
                    format!("{cite}; range set by the engineer ({})", s.source),
                ));
            }
            if x.selected == "MSE" {
                o.checks.push(chk(
                    "W-MSE-SLIDE",
                    "MSE wall sliding FS",
                    el.clone(),
                    x.mse.fs_sliding,
                    s.mse_fs_sliding,
                    "",
                    "min",
                    "mandatory",
                    format!("μ·γr·H·L / (Ka·γ·H²/2 + Ka·q·H), L = {:.1} m", x.mse.l_m),
                    cite.into(),
                ));
                o.checks.push(chk(
                    "W-MSE-ECC",
                    "MSE wall eccentricity e ≤ L/6",
                    el.clone(),
                    x.mse.e_m,
                    s.mse_e_frac * x.mse.l_m,
                    "m",
                    "max",
                    "mandatory",
                    "e = Mo / V".into(),
                    cite.into(),
                ));
                o.checks.push(chk(
                    "W-MSE-BRG",
                    "MSE wall bearing σv = V/(L−2e) ≤ allowable",
                    el.clone(),
                    x.mse.sigma_v_kpa,
                    s.mse_bearing_kpa.unwrap_or(c.assembly.wall.as_ref().map_or(0.0, |w| w.soil.bearing_kpa)),
                    "kPa",
                    "max",
                    "mandatory",
                    "Meyerhof distribution".into(),
                    cite.into(),
                ));
                o.checks.push(chk(
                    "W-MSE-L",
                    "MSE reinforcement length ≥ max(0.7H, L min)",
                    el.clone(),
                    x.mse.l_m,
                    (s.mse_l_ratio * x.h_m).max(s.mse_l_min_m),
                    "m",
                    "min",
                    "mandatory",
                    "".into(),
                    cite.into(),
                ));
            }
            if x.selected == "specialist" {
                o.checks.push(chk("W-TYPE", "A screened wall type passes", el, 0.0, 1.0, "", "min", "mandatory", "no option passed".into(), cite.into()));
            }
        }
        files.push(("wall-types.csv".into(), csv.into_bytes()));
        // sheet: plan of wall runs + typical MSE section of the tallest MSE wall
        let mut plan = cl_panel(self, "Wall types (plan)");
        if let Some(al) = &self.alignment {
            for x in &ch {
                if let Some((px, py, _)) = al.point_at(x.station) {
                    plan.labels.push(("TEXT".into(), [px, py], format!("{} {}", x.selected, x.side)));
                }
            }
        }
        let mut panels = vec![plan];
        if let Some(t) = ch.iter().filter(|x| x.selected == "MSE").max_by(|p, q| p.h_m.partial_cmp(&q.h_m).unwrap()) {
            let (h, l) = (t.h_m, t.mse.l_m);
            let mut p = Panel { title: format!("Typical MSE section, {} {} (H {:.1} m)", sta_label(t.station), t.side, h), equal: true, ..Default::default() };
            p.polys.push(("WALL".into(), vec![[0.0, 0.0], [0.0, h], [-0.15, h], [-0.15, 0.0], [0.0, 0.0]]));
            p.polys.push(("EG".into(), vec![[-4.0, 0.0], [l + 6.0, 0.0]]));
            p.polys.push(("FG".into(), vec![[-0.15, h], [l + 6.0, h]]));
            p.polys.push(("HATCH".into(), vec![[0.0, 0.0], [l, 0.0], [l, h], [0.0, h], [0.0, 0.0]]));
            for k in 0..t.mse.layers {
                let z = s.sv_m * (k as f64 + 0.5);
                if z < h {
                    p.polys.push(("PAVEMENT".into(), vec![[0.0, z], [l, z]]));
                }
            }
            p.labels.push(("TEXT".into(), [l / 2.0, h + 0.8], format!("L = {l:.1} m, {} layers @ {:.2} m", t.mse.layers, s.sv_m)));
            p.labels.push(("TEXT".into(), [l / 2.0, -1.0], format!("FS slide {:.2}, e {:.2} m, σv {:.0} kPa", t.mse.fs_sliding, t.mse.e_m, t.mse.sigma_v_kpa)));
            p.note = "Reinforcement strength, connection, pullout and facing by the wall supplier".into();
            panels.push(p);
        }
        let n_sel = |k: &str| ch.iter().filter(|x| x.selected == k).count();
        o.sheets.push(ExtSheet {
            title: "Retaining wall type selection".into(),
            panels,
            tables: vec![(vec![22.0, 8.0, 10.0, 12.0, 26.0, 14.0, 14.0, 12.0, 14.0, 16.0, 14.0], rows)],
            notes: vec![
                format!("Selection rule: gravity ≤ {:.1} m, cantilever ≤ {:.1} m (engineer practical ranges), profile reshape if the change ≤ {:.1} m, otherwise MSE (fill) or soil nail (cut).", s.gravity_max_h_m, s.cantilever_max_h_m, s.reshape_max_dz_m),
                format!("Reference: {cite}."),
                "MSE external stability by allowable stress (GEC 11): sliding, eccentricity e ≤ L/6, bearing with Meyerhof distribution; internal Tmax = Ka,r·σv·Sv. Global stability, compound failure, seismic and supplier design are by the geotechnical/structures specialist.".into(),
            ],
        });
        o.result = json!({"input": s, "citation": cite, "choices": ch, "summary": {"gravity": n_sel("gravity"), "cantilever": n_sel("cantilever"), "reshape": n_sel("reshape profile"), "mse": n_sel("MSE"), "soil_nail": n_sel("soil nail"), "specialist": n_sel("specialist")}});
        Ok(())
    }
}

fn arrow(pts: &mut Vec<(String, Vec<[f64; 2]>)>, path: Vec<[f64; 2]>, rot: f64, lay: &str) {
    let r = |p: [f64; 2]| [p[0] * rot.cos() - p[1] * rot.sin(), p[0] * rot.sin() + p[1] * rot.cos()];
    let n = path.len();
    let (a, b) = (path[n - 2], path[n - 1]);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = (dx * dx + dy * dy).sqrt().max(1e-9);
    let (ux, uy) = (dx / l, dy / l);
    let h1 = [b[0] - 3.0 * ux + 1.6 * uy, b[1] - 3.0 * uy - 1.6 * ux];
    let h2 = [b[0] - 3.0 * ux - 1.6 * uy, b[1] - 3.0 * uy + 1.6 * ux];
    pts.push((lay.into(), path.into_iter().map(r).collect()));
    pts.push((lay.into(), vec![r(h1), r(b), r(h2)]));
}

impl Session {
    pub(crate) fn x_signals(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let w: crate::signals::WarrantInput = serde_json::from_value(a["warrants"].clone()).map_err(|e| format!("warrants: {e}"))?;
        let t: crate::signals::TimingInput = serde_json::from_value(a["timing"].clone()).map_err(|e| format!("timing: {e}"))?;
        let wr = crate::signals::warrants(b.standard, &w);
        let tm = crate::signals::timing(&t);
        let xmax = a["x_max"].as_f64().unwrap_or(0.9);
        let lhs = matches!(b.standard, Standard::Uk | Standard::Au | Standard::Nz);
        let name = self.intersections.first().map_or("intersection".to_string(), |i| format!("{} at {}", i.name, sta_label(i.main_station)));
        let cite_t = match b.standard {
            Standard::Uk => "DfT Traffic Signs Manual Ch. 6 / LTN 1/20; timing by Webster (RRL TP 39) with LINSIG for detailed design (verify)",
            Standard::Au | Standard::Nz => "Austroads AGTM09 / AGTM06; Webster (RRL TP 39); SIDRA for detailed design (verify)",
            Standard::Ca => "TAC Traffic Signal Guidelines; HCM delay; Synchro/HCS for detailed design (verify)",
            _ => "MUTCD Part 4; HCM (7th ed.) signalized delay and LOS; ITE change interval; Synchro/HCS for detailed design",
        };
        o.checks.push(chk(
            "SIG-WARRANT",
            "Signal warrant hours (Condition A/B/engineer table)",
            name.clone(),
            wr.hours_a.max(wr.hours_b).max(wr.hours_combo) as f64,
            if wr.method.starts_with("engineer") { w.local.as_ref().map_or(8.0, |l| l.hours_required as f64) } else { 8.0 },
            "h",
            "min",
            "advisory",
            wr.detail.clone(),
            wr.method.clone(),
        ));
        let mut csv = String::from("phase,movements,y,green_s,yellow_s,all_red_s,x,delay_s_per_veh,volume_vph\n");
        let mut rows = vec![hdr(&["Phase", "Movements", "y", "Green s", "Yellow s", "All-red s", "X", "Delay s", "LOS"])];
        for p in &tm.phases {
            let _ = writeln!(
                csv,
                "{},{},{:.4},{:.2},{:.2},{:.2},{:.4},{:.2},{:.0}",
                csvq(&p.name),
                csvq(&p.movements.join(" ")),
                p.y,
                p.green_s,
                p.yellow_s,
                p.all_red_s,
                p.x,
                p.delay_s,
                p.volume_vph
            );
            rows.push(vec![
                p.name.clone(),
                p.movements.join(", "),
                format!("{:.3}", p.y),
                format!("{:.1}", p.green_s),
                format!("{:.1}", p.yellow_s),
                format!("{:.1}", p.all_red_s),
                format!("{:.2}", p.x),
                format!("{:.1}", p.delay_s),
                crate::signals::los_signal(p.delay_s).into(),
            ]);
            o.checks.push(chk(
                "SIG-X",
                "Phase degree of saturation ≤ practical limit",
                format!("{name} {}", p.name),
                p.x,
                xmax,
                "",
                "max",
                "advisory",
                format!("X = y·C/g, C {:.0} s", tm.cycle_s),
                cite_t.into(),
            ));
        }
        let _ = writeln!(
            csv,
            "# cycle_webster_s,{:.3}\n# cycle_s,{:.1}\n# lost_s,{:.1}\n# y_total,{:.4}\n# delay_s,{:.2}\n# los,{}",
            tm.cycle_webster_s, tm.cycle_s, tm.lost_s, tm.y_total, tm.delay_s, tm.los
        );
        files.push(("signal-timing.csv".into(), csv.into_bytes()));
        if let Some(tg) = a["los_target"].as_str() {
            let lim = match tg {
                "A" => 10.0,
                "B" => 20.0,
                "C" => 35.0,
                "D" => 55.0,
                _ => 80.0,
            };
            o.checks.push(chk(
                "SIG-LOS",
                &format!("Intersection control delay within LOS {tg}"),
                name.clone(),
                tm.delay_s,
                lim,
                "s/veh",
                "max",
                "advisory",
                format!("HCM delay, LOS {}", tm.los),
                cite_t.into(),
            ));
        }
        // phasing diagram: one panel per phase, schematic (major east-west, minor north-south)
        let mut panels = vec![];
        let maj: Vec<&crate::signals::Approach> = t.approaches.iter().filter(|x| x.major).collect();
        let min: Vec<&crate::signals::Approach> = t.approaches.iter().filter(|x| !x.major).collect();
        let off = if lhs { 3.0 } else { -3.0 };
        let rot_of = |ap: &crate::signals::Approach| {
            let (list, base) = if ap.major { (&maj, 0.0) } else { (&min, std::f64::consts::FRAC_PI_2) };
            let k = list.iter().position(|x| x.name == ap.name).unwrap_or(0);
            base + if k == 0 { 0.0 } else { std::f64::consts::PI }
        };
        for p in &tm.phases {
            let mut pn = Panel { title: format!("{}: G {:.0} s", p.name, p.green_s), equal: true, ..Default::default() };
            pn.polys.push(("EG".into(), vec![[-40.0, -8.0], [-8.0, -8.0], [-8.0, -40.0]]));
            pn.polys.push(("EG".into(), vec![[40.0, -8.0], [8.0, -8.0], [8.0, -40.0]]));
            pn.polys.push(("EG".into(), vec![[-40.0, 8.0], [-8.0, 8.0], [-8.0, 40.0]]));
            pn.polys.push(("EG".into(), vec![[40.0, 8.0], [8.0, 8.0], [8.0, 40.0]]));
            for m in &p.movements {
                let nm = m.split(' ').next().unwrap_or("");
                let Some(ap) = t.approaches.iter().find(|x| x.name == nm) else { continue };
                let rot = rot_of(ap);
                let rest = &m[nm.len()..];
                let start = [-36.0, off];
                if rest.contains('T') || rest.contains("all") {
                    arrow(&mut pn.polys, vec![start, [30.0, off]], rot, "SIGNAL");
                }
                if rest.contains('L') || rest.contains("all") {
                    let dashed = rest.contains("perm");
                    let xl = if lhs { -4.0 } else { 4.0 };
                    arrow(&mut pn.polys, vec![start, [xl, off], [xl, 30.0]], rot, if dashed { "GRID" } else { "SIGNAL" });
                }
                if rest.contains('R') || rest.contains("all") {
                    let xr = if lhs { 4.0 } else { -4.0 };
                    arrow(&mut pn.polys, vec![start, [xr, off], [xr, -30.0]], rot, "SIGNAL");
                }
            }
            panels.push(pn);
        }
        let wrows = vec![
            hdr(&["Warrant item", "Value"]),
            vec!["Method".into(), wr.method.clone()],
            vec!["Thresholds A (major/minor)".into(), format!("{:.0} / {:.0} veh/h", wr.a_threshold[0], wr.a_threshold[1])],
            vec!["Thresholds B (major/minor)".into(), format!("{:.0} / {:.0} veh/h", wr.b_threshold[0], wr.b_threshold[1])],
            vec!["Hours A / B / combination".into(), format!("{} / {} / {}", wr.hours_a, wr.hours_b, wr.hours_combo)],
            vec!["Warrant met".into(), if wr.met { "yes".into() } else { "no".into() }],
        ];
        o.sheets.push(ExtSheet {
            title: format!("Traffic signals: warrant, phasing and timing ({name})"),
            panels,
            tables: vec![(vec![40.0, 70.0, 14.0, 16.0, 16.0, 16.0, 12.0, 14.0, 10.0], rows), (vec![50.0, 150.0], wrows)],
            notes: vec![
                format!("Cycle: Webster C0 = {:.1} s, used {:.0} s; lost time {:.0} s; ΣY {:.3}; intersection delay {:.1} s/veh, LOS {} (HCM signalized thresholds).", tm.cycle_webster_s, tm.cycle_s, tm.lost_s, tm.y_total, tm.delay_s, tm.los),
                format!("Saturation flow {:.0} pcu/h/lane and volumes are engineer inputs ({}). Dashed (grey) arrows: permissive lefts.", t.sat_flow_pcuhpl, t.source),
                format!("Reference: {cite_t}. Concept depth: detector layout, pedestrian timings, coordination and the signal plan are by the traffic signal engineer."),
            ],
        });
        o.result = json!({"intersection": name, "warrant": wr, "timing": tm, "citation": cite_t});
        Ok(())
    }

    pub(crate) fn x_capacity(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let l = &a["link"];
        let g = |k: &str| l[k].as_f64().ok_or(format!("link.{k} required (engineer input)"));
        let (v, d, phf, pt, et, cap, nl, ffs) = (
            g("peak_hour_vph")?,
            g("direction_split")?,
            g("phf")?,
            g("heavy_pct")? / 100.0,
            g("e_t")?,
            g("capacity_pcphpl")?,
            g("lanes_per_direction")?,
            g("ffs_kmh")?,
        );
        let src = a["source"].as_str().unwrap_or("engineer");
        let fhv = 1.0 / (1.0 + pt * (et - 1.0));
        let vp = v * d / (phf * fhv);
        let vc = vp / (cap * nl);
        let (los_link, how) = if nl >= 2.0 {
            let dens = vp / nl / (ffs / 1.609344);
            let t = [11.0, 18.0, 26.0, 35.0, 45.0];
            let k = t.iter().position(|x| dens <= *x).unwrap_or(5);
            (["A", "B", "C", "D", "E", "F"][k].to_string(), format!("density {dens:.1} pc/mi/ln (HCM basic segment thresholds 11/18/26/35/45, verify)"))
        } else if let Some(tb) = l["los_vc"].as_array() {
            let mut los = "F".to_string();
            for r in tb {
                if vc <= r[0].as_f64().unwrap_or(0.0) {
                    los = r[1].as_str().unwrap_or("?").into();
                    break;
                }
            }
            (los, "engineer v/c → LOS table (two-lane; HCM 7 follower-density method not reproduced)".to_string())
        } else {
            ("n/a".into(), "two-lane: v/c only (enter los_vc table for a LOS letter)".to_string())
        };
        let cite = match b.standard {
            Standard::Uk => "DMRB / TA 79/99 link capacity (engineer), RFC from the same v/c; PICADY for priority junctions (verify)",
            Standard::Au | Standard::Nz => "Austroads AGTM03 (capacity, LOS) and AGTM06 (unsignalised intersections, gap acceptance) (verify)",
            Standard::Ca => "HCM methods as used in Canada (TAC); capacity and LOS criteria by the agency (verify)",
            _ => "HCM 7th ed.: PCE/f_HV, basic-segment density LOS, TWSC gap-acceptance capacity and control delay",
        };
        o.checks.push(chk(
            "CAP-LINK",
            "Link volume/capacity ≤ 1.0",
            "mainline".into(),
            vc,
            1.0,
            "",
            "max",
            "advisory",
            format!("v_p = V·D/(PHF·f_HV) = {vp:.0} pc/h, c = {:.0}", cap * nl),
            format!("{cite} ({src})"),
        ));
        let mut csv = format!("# link: v_p,{vp:.2}\n# link: f_HV,{fhv:.5}\n# link: v/c,{vc:.5}\n# link: LOS,{los_link}\nmovement,volume_vph,conflicting_vph,tc_s,tf_s,capacity_vph,v_c,delay_s,los\n");
        let mut rows = vec![hdr(&["Movement", "v veh/h", "vc veh/h", "tc s", "tf s", "c veh/h", "v/c (RFC/DOS)", "Delay s", "LOS"])];
        rows.push(vec![
            "Mainline link".into(),
            format!("{v:.0}"),
            "".into(),
            "".into(),
            "".into(),
            format!("{:.0}", cap * nl),
            format!("{vc:.2}"),
            "".into(),
            los_link.clone(),
        ]);
        let lim_unsig = match b.standard {
            Standard::Uk => 0.85,
            Standard::Au | Standard::Nz => 0.8,
            _ => 1.0,
        };
        let mut mv = vec![];
        for m in a["priority"]["movements"].as_array().into_iter().flatten() {
            let gm = |k: &str| m[k].as_f64().unwrap_or(0.0);
            let nm = m["name"].as_str().unwrap_or("").to_string();
            let c = crate::signals::twsc_capacity(gm("conflicting_vph"), gm("tc_s"), gm("tf_s"));
            let x = gm("volume_vph") / c;
            let dl = crate::signals::twsc_delay(gm("volume_vph"), c);
            let los = crate::signals::los_twsc(dl);
            let _ =
                writeln!(csv, "{},{},{},{},{},{:.3},{:.5},{:.3},{}", csvq(&nm), gm("volume_vph"), gm("conflicting_vph"), gm("tc_s"), gm("tf_s"), c, x, dl, los);
            rows.push(vec![
                nm.clone(),
                format!("{:.0}", gm("volume_vph")),
                format!("{:.0}", gm("conflicting_vph")),
                format!("{:.1}", gm("tc_s")),
                format!("{:.1}", gm("tf_s")),
                format!("{c:.0}"),
                format!("{x:.2}"),
                format!("{dl:.1}"),
                los.into(),
            ]);
            o.checks.push(chk(
                "CAP-MOVE",
                "Priority movement v/c within the practical limit (RFC UK 0.85, DOS AU/NZ 0.80, v/c 1.0 US/CA)",
                nm.clone(),
                x,
                lim_unsig,
                "",
                "max",
                "advisory",
                format!("c = vc·e^(−vc·tc/3600)/(1 − e^(−vc·tf/3600)) = {c:.0} veh/h; delay {dl:.1} s, LOS {los}"),
                format!("{cite} ({src})"),
            ));
            mv.push(json!({"name": nm, "capacity_vph": c, "v_c": x, "delay_s": dl, "los": los}));
        }
        files.push(("capacity.csv".into(), csv.into_bytes()));
        o.sheets.push(ExtSheet {
            title: "Capacity and level of service (concept)".into(),
            panels: vec![],
            tables: vec![(vec![50.0, 22.0, 22.0, 14.0, 14.0, 22.0, 26.0, 18.0, 12.0], rows)],
            notes: vec![
                format!("Link: V = {v:.0} veh/h, D = {d:.2}, PHF = {phf:.2}, P_T = {:.0} %, E_T = {et:.1} → f_HV = {fhv:.3}, v_p = {vp:.0} pc/h; capacity {cap:.0} pc/h/ln × {nl:.0}; {how}.", pt * 100.0),
                format!("Reference: {cite}. All demand volumes, PHF, E_T, capacity and gap parameters are engineer inputs ({src})."),
                "Concept screening only: the agency's capacity software (HCS, Synchro, SIDRA, LINSIG, PICADY/ARCADY) governs the detailed design.".into(),
            ],
        });
        o.result = json!({"link": {"v_p": vp, "f_hv": fhv, "v_c": vc, "los": los_link, "method": how}, "movements": mv, "citation": cite});
        Ok(())
    }

    pub(crate) fn x_landscape(&self, a: &Value, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let al = self.alignment.as_ref().ok_or("create_alignment first")?;
        let g = |k: &str| a[k].as_f64().ok_or(format!("{k} required (engineer input)"));
        let (sp, toff, cz) = (g("tree_spacing_m")?, g("tree_offset_m")?, g("clear_zone_m")?);
        let side_r = a["side"].as_str().unwrap_or("R") == "R";
        let ic = a["intersection_clear_m"].as_f64().unwrap_or(40.0);
        let pc = a["pole_clear_m"].as_f64().unwrap_or(3.0);
        let dens = a["shrub_density_per_m2"].as_f64().unwrap_or(1.0);
        let tops = a["topsoil_mm"].as_f64().unwrap_or(100.0) / 1000.0;
        let src = a["source"].as_str().unwrap_or("engineer");
        // side-slope areas by class (average of adjacent sections × interval)
        let class_of = |dx: f64, dz: f64| {
            let r = dx.abs() / dz.abs().max(1e-9);
            if r >= 4.0 {
                0
            } else if r >= 2.0 {
                1
            } else {
                2
            }
        };
        let per = |sd: &crate::corridor::SideSection| {
            let mut v = [0.0f64; 3];
            if sd.wall.is_some() || sd.fg.len() < 2 {
                return v;
            }
            let h = sd.fg.len() - 2;
            let (p, q) = (sd.fg[h], sd.fg[h + 1]);
            let (dx, dz) = (q[0] - p[0], q[1] - p[1]);
            v[class_of(dx, dz)] += (dx * dx + dz * dz).sqrt();
            v
        };
        let mut area = [0.0f64; 3];
        for w in c.sections.windows(2) {
            let l = w[1].station - w[0].station;
            for (s0, s1) in [(&w[0].left, &w[1].left), (&w[0].right, &w[1].right)] {
                let (a0, a1) = (per(s0), per(s1));
                for k in 0..3 {
                    area[k] += (a0[k] + a1[k]) / 2.0 * l;
                }
            }
        }
        // street trees
        let poles: Vec<f64> =
            self.ext.get("design_lighting").map_or(vec![], |e| e.result["poles"].as_array().into_iter().flatten().filter_map(|p| p[0].as_f64()).collect());
        let ists: Vec<f64> = self.intersections.iter().map(|i| i.main_station).collect();
        let mut trees = vec![];
        let mut omitted = (0, 0, 0, 0);
        let ok_cz = toff >= cz;
        let mut st = al.start_station() + sp / 2.0;
        while st < al.end_station() {
            let sec = crate::ext::nearest_section(c, st);
            let sd = if side_r { &sec.right } else { &sec.left };
            if !ok_cz {
                omitted.0 += 1;
            } else if ists.iter().any(|x| (x - st).abs() < ic) {
                omitted.1 += 1;
            } else if poles.iter().any(|x| (x - st).abs() < pc) {
                omitted.2 += 1;
            } else if sd.wall.is_some() || sd.daylight < toff {
                omitted.3 += 1;
            } else if let Some((x, y, hd)) = al.point_at(st) {
                let o_ = if side_r { -toff } else { toff };
                trees.push((st, x - o_ * hd.sin(), y + o_ * hd.cos()));
            }
            st += sp;
        }
        let names = [
            "seed + 100 mm topsoil (flatter than 4H:1V)",
            "native shrubs/grasses + erosion control blanket (4H:1V to 2H:1V)",
            "hydroseed + turf reinforcement mat (steeper than 2H:1V)",
        ];
        let mut csv = String::from("item,unit,quantity\n");
        for k in 0..3 {
            let _ = writeln!(csv, "{},m2,{:.1}", csvq(names[k]), area[k]);
        }
        let _ = writeln!(
            csv,
            "topsoil (class 0 areas),m3,{:.1}\nshrubs (class 1 areas),each,{:.0}\nstreet trees,each,{}",
            area[0] * tops,
            area[1] * dens,
            trees.len()
        );
        let _ = writeln!(csv, "# trees: station,x,y");
        for t in &trees {
            let _ = writeln!(csv, "# {:.2},{:.3},{:.3}", t.0, t.1, t.2);
        }
        files.push(("landscape.csv".into(), csv.into_bytes()));
        o.checks.push(chk(
            "L-TREE-CZ",
            "Street trees outside the clear zone",
            "street trees".into(),
            toff,
            cz,
            "m",
            "min",
            "advisory",
            format!("tree offset {toff:.1} m vs clear zone {cz:.1} m ({src})"),
            "AASHTO Roadside Design Guide (2011) Ch. 3 / TAC / DMRB CD 377 / Austroads AGRD06 clear zone (engineer value)".into(),
        ));
        let mut p = cl_panel(self, "Landscape concept (plan)");
        for t in &trees {
            let r = 2.0;
            p.polys.push((
                "LANDSCAPE".into(),
                (0..=12)
                    .map(|k| {
                        let th = k as f64 / 12.0 * std::f64::consts::TAU;
                        [t.1 + r * th.cos(), t.2 + r * th.sin()]
                    })
                    .collect(),
            ));
        }
        let rows = vec![
            hdr(&["Item", "Unit", "Quantity"]),
            vec![names[0].into(), "m²".into(), format!("{:.0}", area[0])],
            vec![names[1].into(), "m²".into(), format!("{:.0}", area[1])],
            vec![names[2].into(), "m²".into(), format!("{:.0}", area[2])],
            vec!["Topsoil".into(), "m³".into(), format!("{:.0}", area[0] * tops)],
            vec!["Shrubs".into(), "each".into(), format!("{:.0}", area[1] * dens)],
            vec!["Street trees".into(), "each".into(), format!("{}", trees.len())],
        ];
        o.sheets.push(ExtSheet {
            title: "Landscape concept".into(),
            panels: vec![p],
            tables: vec![(vec![120.0, 16.0, 24.0], rows)],
            notes: vec![
                format!("Trees every {sp:.0} m at {toff:.1} m from the centreline on the {} side; omitted: {} clear zone, {} near intersections (±{ic:.0} m), {} near lighting poles (±{pc:.0} m), {} at walls or within the earthwork footprint.", if side_r { "right" } else { "left" }, omitted.0, omitted.1, omitted.2, omitted.3),
                "Slope classes by side-slope steepness; species, soil testing, irrigation and establishment period by the landscape architect (agency planting guide).".into(),
            ],
        });
        o.result = json!({"areas_m2": {"seed": area[0], "shrub_blanket": area[1], "hydroseed_trm": area[2]}, "topsoil_m3": area[0] * tops, "shrubs": area[1] * dens, "trees": trees.len(), "omitted": [omitted.0, omitted.1, omitted.2, omitted.3]});
        Ok(())
    }

    pub(crate) fn x_rigid(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let r: crate::rigid::RigidInput = serde_json::from_value(a.clone()).map_err(|e| format!("input: {e}"))?;
        let d = crate::rigid::design(&r);
        let au = matches!(b.standard, Standard::Au | Standard::Nz);
        let scope = if au {
            "AU/NZ project: AGPT02 Ch. 9 structure"
        } else {
            "non-AU/NZ project: shown for comparison only; the agency rigid method governs (AASHTO 93 / DMRB CD 226 / provincial)"
        };
        o.checks.push(chk(
            "P-RIGID-FAT",
            "Rigid base fatigue damage ≤ 100 %",
            format!("{} base {:.0} mm", r.base_type, d.h_mm),
            d.fatigue_pct,
            100.0,
            "%",
            "max",
            "advisory",
            format!("Σ n/Nf, LSF {:.2}, fcf {:.1} MPa", r.lsf, r.fcf_mpa),
            crate::rigid::REF.into(),
        ));
        o.checks.push(chk(
            "P-RIGID-ERO",
            "Rigid base erosion damage ≤ 100 %",
            format!("{} base {:.0} mm", r.base_type, d.h_mm),
            d.erosion_pct,
            100.0,
            "%",
            "max",
            "advisory",
            format!("Σ C2·n/Ne, tied shoulder {}", r.tied_shoulder),
            crate::rigid::REF.into(),
        ));
        let mut csv = format!(
            "# {}\n# {scope}\n# design base thickness mm,{:.0}\ngroup,p_n,a_mm,stress_mpa,sr,nf,fatigue_pct,corner_w_mm,power,ne,erosion_pct\n",
            crate::rigid::REF,
            d.h_mm
        );
        let mut rows = vec![hdr(&["Group", "P/tyre kN", "σ MPa", "Sr", "Nf", "Fatigue %", "Erosion %"])];
        for g in &d.groups {
            let _ = writeln!(
                csv,
                "{},{:.2},{:.3},{:.5},{:.5},{:.5e},{:.5},{:.6},{:.5},{:.5e},{:.5}",
                csvq(&g.name),
                g.p_n,
                g.a_mm,
                g.stress_mpa,
                g.sr,
                g.nf,
                g.fatigue_pct,
                g.corner_w_mm,
                g.power,
                g.ne,
                g.erosion_pct
            );
            rows.push(vec![
                g.name.clone(),
                format!("{:.1}", g.p_n / 1000.0),
                format!("{:.2}", g.stress_mpa),
                format!("{:.3}", g.sr),
                format!("{:.2e}", g.nf),
                format!("{:.1}", g.fatigue_pct),
                format!("{:.1}", g.erosion_pct),
            ]);
        }
        let _ = writeln!(csv, "# trials h_mm,fatigue_pct,erosion_pct");
        for t in &d.trials {
            let _ = writeln!(csv, "# {:.0},{:.4},{:.4}", t.0, t.1, t.2);
        }
        files.push(("rigid-pavement.csv".into(), csv.into_bytes()));
        let mut p = Panel { title: format!("{} base {:.0} mm on subbase (k {:.0} MPa/m)", r.base_type, d.h_mm, r.k_mpa_m), equal: false, ..Default::default() };
        let h = d.h_mm / 1000.0;
        p.polys.push(("PAVEMENT".into(), vec![[0.0, 0.0], [4.0, 0.0], [4.0, -h], [0.0, -h], [0.0, 0.0]]));
        p.polys.push(("SUBGRADE".into(), vec![[0.0, -h - 0.15], [4.0, -h - 0.15]]));
        p.labels.push(("TEXT".into(), [0.1, -h / 2.0], format!("{} {:.0} mm, fcf {:.1} MPa", r.base_type, d.h_mm, r.fcf_mpa)));
        p.labels.push(("TEXT".into(), [0.1, -h - 0.1], "bound / lean-mix subbase (engineer)".into()));
        o.sheets.push(ExtSheet {
            title: "Rigid pavement (AGPT02 Ch. 9 structure, concept)".into(),
            panels: vec![p],
            tables: vec![(vec![40.0, 20.0, 18.0, 16.0, 22.0, 20.0, 20.0], rows)],
            notes: vec![
                format!("{scope}. Thickness {:.0} mm: fatigue {:.1} %, erosion {:.1} % (≤ 100 %); trials from {:.0} mm in 5 mm steps.", d.h_mm, d.fatigue_pct, d.erosion_pct, r.h_min_mm),
                format!("Method: {}. AGPT02 equivalent-stress / erosion-factor tables are not shipped (copyright): confirm the thickness with AGPT02 Tables 9.x or the agency procedure.", crate::rigid::REF),
                format!("Inputs (engineer, {}): axle-group spectrum, LSF {:.2}, effective subgrade support k {:.0} MPa/m, E {:.0} MPa, ν {:.2}.", r.source, r.lsf, r.k_mpa_m, r.e_mpa, r.nu),
            ],
        });
        o.result = json!({"input": r, "result": d, "scope": scope});
        Ok(())
    }
}

impl Session {
    pub(crate) fn x_constructability(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        let c = self.corridor.as_ref().ok_or("build_corridor first")?;
        let pr = self.profile.as_ref().ok_or("create_profile first")?;
        let max_fill = a["max_fill_h_m"].as_f64().unwrap_or(10.0);
        let max_g = a["max_grade_paving_pct"].as_f64().unwrap_or(6.0);
        let mse_hold = a["mse_hold_h_m"].as_f64().unwrap_or(10.0);
        let src = a["source"].as_str().unwrap_or("engineer");
        let e = |k: &str| self.ext.get(k).map(|x| &x.result);
        // geometry facts
        let (mut fill_h, mut fill_st, mut cut_h, mut cut_st) = (0.0f64, 0.0, 0.0f64, 0.0);
        for s in &c.sections {
            for sd in [&s.left, &s.right] {
                if sd.fg.len() >= 2 && sd.wall.is_none() {
                    let h = sd.fg[sd.fg.len() - 1][1] - sd.fg[sd.fg.len() - 2][1];
                    if !sd.cut && -h > fill_h {
                        fill_h = -h;
                        fill_st = s.station;
                    }
                    if sd.cut && h > cut_h {
                        cut_h = h;
                        cut_st = s.station;
                    }
                }
            }
        }
        let gmax = pr.grades_pct.iter().fold(0.0f64, |m, g| m.max(g.abs()));
        let mut r = String::new();
        let _ = writeln!(r, "# Constructability review (concept)\n\nGenerated from the current design state. Standard profile: {:?}. Reviewer inputs: {src}. Items marked HOLD need a decision before the detailed design proceeds.\n", b.standard);
        let _ = writeln!(r, "## 1. Key facts\n\n| Item | Value |\n|---|---:|\n| Corridor cut / fill (m³) | {:.0} / {:.0} |\n| Max side-fill height (m) | {:.1} at {} |\n| Max cut height (m) | {:.1} at {} |\n| Max grade (%) | {:.2} |", c.cut_m3, c.fill_m3, fill_h, sta_label(fill_st), cut_h, sta_label(cut_st), gmax);
        if let Some(m) = &self.mass_haul {
            let _ = writeln!(r, "| Haul moment (m³·m) | {:.0} |\n| Final mass ordinate (m³) | {:.0} |", m.haul_moment_m3m, m.final_ordinate);
        }
        let mut seq: Vec<(String, String)> = vec![];
        seq.push(("1 Enabling".into(), "survey control, site compound and lay-down within the land requirement, stabilised construction entrances, ESC perimeter controls (sediment fence, check dams) before any earthworks".into()));
        if let Some(u) = e("register_utilities") {
            seq.push((
                "2 Utilities".into(),
                format!(
                    "{} utility conflicts in the register: protect / relocate before earthworks; pothole (QL-A) the QL-B/C/D records first",
                    u["conflicts"].as_array().map_or(0, |v| v.len())
                ),
            ));
        }
        if let Some(cu) = e("design_culverts") {
            seq.push((
                "3 Drainage first".into(),
                format!(
                    "{} culverts installed before the embankment over them (temporary stream diversion, dry season); outlet protection at the same time",
                    cu["culverts"].as_array().map_or(0, |v| v.iter().filter(|x| x["diameter_m"].is_number()).count())
                ),
            ));
        }
        let mut risks: Vec<(String, String, String)> = vec![];
        if let Some(w) = e("select_wall_types") {
            let sm = &w["summary"];
            seq.push(("4 Walls".into(), format!("MSE {} sections built with the embankment (lift by lift, compaction control in the reinforced zone, select fill); cantilever {} sections cast before backfilling; soil nail {} sections top-down in the cut", sm["mse"], sm["cantilever"], sm["soil_nail"])));
            let hmax =
                w["choices"].as_array().into_iter().flatten().filter(|x| x["selected"] == "MSE").filter_map(|x| x["h_m"].as_f64()).fold(0.0f64, f64::max);
            if hmax > 0.0 {
                o.checks.push(chk(
                    "C-MSE-H",
                    "MSE wall height within the reviewer's hold limit",
                    "MSE walls".into(),
                    hmax,
                    mse_hold,
                    "m",
                    "max",
                    "advisory",
                    "tall MSE: staged construction, settlement / pore-pressure monitoring, supplier design review".into(),
                    format!("constructability review ({src})"),
                ));
                if hmax > mse_hold {
                    risks.push((
                        "HOLD".into(),
                        format!("MSE walls up to {hmax:.1} m"),
                        "foundation settlement and global stability: geotechnical investigation, instrumentation and supplier design before tender".into(),
                    ));
                }
            }
        }
        seq.push((
            "5 Earthworks".into(),
            format!(
                "cut-to-fill along the mass haul; fill in lifts with compaction testing; benches every 8 m on high slopes{}",
                if fill_h > max_fill {
                    format!("; fill {fill_h:.1} m high at {} needs staged construction / surcharge and settlement monitoring", sta_label(fill_st))
                } else {
                    String::new()
                }
            ),
        ));
        o.checks.push(chk(
            "C-FILL-H",
            "Fill height within the reviewer's limit without staged construction",
            sta_label(fill_st),
            fill_h,
            max_fill,
            "m",
            "max",
            "advisory",
            "max side-fill height from the corridor".into(),
            format!("constructability review ({src})"),
        ));
        if fill_h > max_fill {
            risks.push((
                "HOLD".into(),
                format!("fill {fill_h:.1} m at {}", sta_label(fill_st)),
                "settlement period / wick drains / surcharge decision; programme float".into(),
            ));
        }
        if let Some(sl) = e("check_slope_stability") {
            let fmin = sl["sections"].as_array().into_iter().flatten().filter_map(|x| x["circle"]["fs_bishop"].as_f64()).fold(f64::INFINITY, f64::min);
            risks.push((
                "risk".into(),
                format!("slopes: minimum FS {fmin:.2}"),
                "temporary slopes during construction are steeper/wetter than the final case: limit open cut length, drain before rain".into(),
            ));
        }
        seq.push((
            "6 Pavement".into(),
            format!(
                "subgrade proof-roll, subsurface drains, base and asphalt; max grade {gmax:.1} %{}",
                if gmax > max_g { " (above the paving limit: echelon paving / material transfer vehicle, cooling of asphalt on the grade)" } else { "" }
            ),
        ));
        o.checks.push(chk(
            "C-GRADE",
            "Max grade within the reviewer's paving limit",
            "profile".into(),
            gmax,
            max_g,
            "%",
            "max",
            "advisory",
            "steep grades affect paver and roller productivity and asphalt density".into(),
            format!("constructability review ({src})"),
        ));
        if let Some(t) = e("plan_traffic_management") {
            seq.push((
                "7 Traffic staging".into(),
                format!("{} TTM stages; tie-ins under lane closure at night / off-peak", t["stages"].as_array().map_or(0, |v| v.len())),
            ));
        }
        if e("design_signals").is_some() {
            seq.push((
                "8 Signals".into(),
                "ducts and pits with the kerb/drainage works; temporary signals or flagging during the tie-in; commissioning after the final surface".into(),
            ));
        }
        seq.push(("9 Finishing".into(), "marking and signing, lighting, landscape (planting season), removal of temporary ESC after 70 % cover".into()));
        if let Some(l) = e("land_requirements") {
            risks.push((
                "risk".into(),
                format!("land: footprint {:.0} m²", l["footprint_area_m2"].as_f64().unwrap_or(0.0)),
                "temporary working space and lay-down outside the permanent take need licences / temporary easements".into(),
            ));
        }
        risks.push((
            "risk".into(),
            "ground conditions".into(),
            "no site investigation in the design data: all geotechnical inputs are placeholders until the GI report".into(),
        ));
        let _ = writeln!(r, "\n## 2. Sequence of works\n\n| Stage | Activities |\n|---|---|");
        for (k, v) in &seq {
            let _ = writeln!(r, "| {k} | {v} |");
        }
        let _ = writeln!(r, "\n## 3. Risk register and hold points\n\n| Type | Item | Action |\n|---|---|---|");
        for (t, i, act) in &risks {
            let _ = writeln!(r, "| {t} | {i} | {act} |");
        }
        let _ = writeln!(r, "\n## 4. Limits\n\nConcept review generated from the model; it does not replace a contractor's constructability review, a programme, or a temporary works design (BS 5975 / Falsework standards / OSHA).");
        files.push(("constructability.md".into(), r.into_bytes()));
        let mut rows = vec![hdr(&["Stage", "Activities"])];
        rows.extend(seq.iter().map(|(k, v)| vec![k.clone(), v.clone()]));
        let mut rr = vec![hdr(&["Type", "Item", "Action"])];
        rr.extend(risks.iter().map(|(t, i, a)| vec![t.clone(), i.clone(), a.clone()]));
        o.sheets.push(ExtSheet {
            title: "Constructability: sequence and risks".into(),
            panels: vec![],
            tables: vec![(vec![30.0, 330.0], rows), (vec![16.0, 70.0, 270.0], rr)],
            notes: vec![
                "Concept constructability review generated from the design (constructability.md). Temporary works and programme by the contractor.".into()
            ],
        });
        o.result = json!({"sequence": seq, "risks": risks, "max_fill_h_m": fill_h, "max_cut_h_m": cut_h, "max_grade_pct": gmax});
        Ok(())
    }
}

impl Session {
    pub(crate) fn x_foundation(&self, a: &Value, b: &B, o: &mut ExtOut, files: &mut Files) -> crate::Result<()> {
        use crate::lea::{response, LeaLayer};
        let src = a["source"].as_str().unwrap_or("engineer");
        let (esg, how) = match (a["subgrade_e_mpa"].as_f64(), a["subgrade_cbr_pct"].as_f64()) {
            (Some(e), _) => (e, "entered".to_string()),
            (None, Some(c)) => (17.6 * c.powf(0.64), format!("E = 17.6·CBR^0.64 with CBR {c} % (CD 225, verify)")),
            _ => return Err("subgrade_e_mpa or subgrade_cbr_pct required (engineer input)".into()),
        };
        let mut ls: Vec<(String, f64, f64)> = a["layers"]
            .as_array()
            .ok_or("layers required")?
            .iter()
            .map(|l| (l["name"].as_str().unwrap_or("").to_string(), l["e_mpa"].as_f64().unwrap_or(0.0), l["h_m"].as_f64().unwrap_or(0.0)))
            .collect();
        if ls.is_empty() {
            return Err("at least one foundation layer".into());
        }
        let (target, tlabel) = match (a["target_e0_mpa"].as_f64(), a["target_class"].as_u64()) {
            (Some(t), _) => (t, format!("engineer target {t:.0} MPa")),
            (None, Some(k)) if b.standard == Standard::Uk => {
                let t = [50.0, 100.0, 200.0, 400.0][(k.clamp(1, 4) - 1) as usize];
                (t, format!("CD 225 foundation class {k} (≥ {t:.0} MPa, verify)"))
            }
            _ => return Err("target_e0_mpa required (target_class maps to CD 225 classes for the UK profile only)".into()),
        };
        let (qa, ra) = (a["plate_kpa"].as_f64().unwrap_or(100.0), a["plate_radius_m"].as_f64().unwrap_or(0.15));
        let hmax = a["h_max_m"].as_f64().unwrap_or(0.8);
        let e0 = |ls: &[(String, f64, f64)]| {
            let mut v: Vec<LeaLayer> = ls.iter().map(|l| LeaLayer { h: l.2, e: l.1 * 1000.0, nu: 0.35 }).collect();
            v.push(LeaLayer { h: 0.0, e: esg * 1000.0, nu: 0.45 });
            let w = response(&v, qa, ra, 0.0, 0.0).w;
            (2.0 * (1.0 - 0.35f64 * 0.35) * qa * ra / w / 1000.0, w)
        };
        let mut trials = vec![];
        loop {
            let (e, w) = e0(&ls);
            trials.push((ls[0].2, e, w));
            if e >= target || ls[0].2 + 0.05 > hmax + 1e-9 {
                break;
            }
            ls[0].2 = ((ls[0].2 + 0.05) * 100.0).round() / 100.0;
        }
        let (h, e, w) = *trials.last().unwrap();
        let cite = if b.standard == Standard::Uk {
            "DMRB CD 225 Design for new pavement foundations (performance design, foundation classes)"
        } else {
            "performance foundation check by layered elastic surface modulus (agency target, engineer)"
        };
        o.checks.push(chk(
            "P-FOUND",
            "Foundation surface modulus ≥ target",
            format!("{} {:.2} m", ls[0].0, h),
            e,
            target,
            "MPa",
            "min",
            "mandatory",
            format!("E0 = 2(1−ν²)qa/w0, w0 {:.3} mm, subgrade {esg:.0} MPa ({how})", w * 1000.0),
            format!("{cite}; {tlabel} ({src})"),
        ));
        let mut csv = format!("# {cite}\n# subgrade E {esg:.2} MPa ({how}); plate q {qa} kPa, a {ra} m\n{}_h_m,e0_mpa,w0_mm\n", ls[0].0.replace(' ', "_"));
        for t in &trials {
            let _ = writeln!(csv, "{:.3},{:.3},{:.5}", t.0, t.1, t.2 * 1000.0);
        }
        files.push(("foundation-design.csv".into(), csv.into_bytes()));
        o.result = json!({"subgrade_e_mpa": esg, "subgrade_basis": how, "target_e0_mpa": target, "target": tlabel, "layers": ls.iter().map(|l| json!({"name": l.0, "e_mpa": l.1, "h_m": l.2})).collect::<Vec<_>>(), "e0_mpa": e, "w0_mm": w * 1000.0, "trials": trials, "pass": e >= target, "citation": cite});
        Ok(())
    }
}
