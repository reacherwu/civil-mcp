//! v6 tools: wall type selection, traffic signal warrants and phasing, capacity / LOS, landscape
//! concept, AU/NZ rigid pavement (AGPT02 ch. 9 structure), constructability report.
use crate::ext::{cl_panel, csvq, hdr, ExtOut, ExtSheet, Files, Panel, B};
use crate::sheet::sta_label;
use crate::standards::chk;
use crate::tools::Session;
use serde_json::{json, Value};
use std::fmt::Write;

pub fn tool_list() -> Vec<Value> {
    let o = |n: &str, d: &str, req: &[&str]| json!({"name": n, "description": d, "inputSchema": {"type": "object", "properties": {}, "required": req, "additionalProperties": true}});
    vec![
        o("select_wall_types", "Wall type selection for every wall section: gravity / cantilever within engineer practical height ranges, profile reshape when the change is small, otherwise MSE / reinforced soil (fill) or soil nail (cut) with external stability per FHWA GEC 11 (BS 8006-1 UK, AS 4678 AU/NZ, CSA S6 + CFEM CA cited). Records the options and the choice per section; design_wall_reinforcement then covers cantilever walls only. Inputs (defaults in brackets): cantilever_max_h_m [8], gravity_max_h_m [3], mse_l_ratio [0.7], mse_l_min_m [2.4], reinforced_gamma_knm3 [20], reinforced_phi_deg [34], sv_m [0.6], mse_fs_sliding [1.5], mse_e_frac [1/6], nail_l_ratio [0.8], nail_spacing_m [1.5], reshape_max_dz_m [1.0], mse_bearing_kpa [wall rule bearing], source.", &[]),
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
