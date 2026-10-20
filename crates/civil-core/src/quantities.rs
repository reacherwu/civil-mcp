//! Mass haul and material tables.
use crate::corridor::Corridor;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MassHaul {
    /// Cut volumes are multiplied by this factor before balancing (e.g. 0.9 shrink).
    pub cut_factor: f64,
    /// (station, cumulative mass ordinate m³)
    pub ordinates: Vec<[f64; 2]>,
    pub balance_stations: Vec<f64>,
    pub final_ordinate: f64,
    /// ∫|M| ds over the alignment (m³·m) – proxy for total haul effort, not a pay quantity.
    pub haul_moment_m3m: f64,
    pub free_haul_m: f64,
    /// Volume moved further than the free-haul distance, by the balanced-segment rule (approximation).
    pub overhaul_note: String,
}

pub fn mass_haul(c: &Corridor, cut_factor: f64, free_haul_m: f64) -> MassHaul {
    let mut m = 0.0;
    let mut ord = vec![[c.sections[0].station, 0.0]];
    for w in c.sections.windows(2) {
        let l = w[1].station - w[0].station;
        let cut = (w[0].cut_area + w[1].cut_area) / 2.0 * l * cut_factor;
        let fill = (w[0].fill_area + w[1].fill_area) / 2.0 * l;
        m += cut - fill;
        ord.push([w[1].station, m]);
    }
    let mut bal = Vec::new();
    for w in ord.windows(2) {
        if (w[0][1] > 0.0 && w[1][1] < 0.0) || (w[0][1] < 0.0 && w[1][1] > 0.0) {
            let t = w[0][1] / (w[0][1] - w[1][1]);
            bal.push(w[0][0] + t * (w[1][0] - w[0][0]));
        }
    }
    let mut hm = 0.0;
    for w in ord.windows(2) {
        let (a, b, l) = (w[0][1], w[1][1], w[1][0] - w[0][0]);
        hm += if (a >= 0.0) == (b >= 0.0) { (a.abs() + b.abs()) / 2.0 * l } else { (a * a + b * b) / (2.0 * (a.abs() + b.abs())) * l };
    }
    MassHaul { cut_factor, final_ordinate: m, ordinates: ord, balance_stations: bal, haul_moment_m3m: hm, free_haul_m, overhaul_note: "overhaul not computed; use the mass ordinates with the contract's free-haul rule".into() }
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterialRow {
    pub from: f64,
    pub to: f64,
    pub cut_m3: f64,
    pub fill_m3: f64,
    pub layers_m3: Vec<f64>,
}

/// Material table by station ranges of `step` metres.
pub fn material_table(c: &Corridor, step: f64) -> Vec<MaterialRow> {
    let mut rows: Vec<MaterialRow> = Vec::new();
    for w in c.sections.windows(2) {
        let l = w[1].station - w[0].station;
        let k = (w[0].station / step).floor();
        let (from, to) = (k * step, (k + 1.0) * step);
        if rows.last().map_or(true, |r| r.from != from) {
            rows.push(MaterialRow { from, to, cut_m3: 0.0, fill_m3: 0.0, layers_m3: vec![0.0; c.assembly.pavement.len()] });
        }
        let r = rows.last_mut().unwrap();
        r.cut_m3 += (w[0].cut_area + w[1].cut_area) / 2.0 * l;
        r.fill_m3 += (w[0].fill_area + w[1].fill_area) / 2.0 * l;
        for (i, v) in r.layers_m3.iter_mut().enumerate() {
            *v += (w[0].layer_areas[i] + w[1].layer_areas[i]) / 2.0 * l;
        }
    }
    rows
}
