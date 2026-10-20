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
    MassHaul {
        cut_factor,
        final_ordinate: m,
        ordinates: ord,
        balance_stations: bal,
        haul_moment_m3m: hm,
        free_haul_m,
        overhaul_note: "overhaul not computed; use the mass ordinates with the contract's free-haul rule".into(),
    }
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
        if rows.last().is_none_or(|r| r.from != from) {
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

/// Earthwork summary with shrinkage, bulking, free haul / overhaul and borrow / waste.
#[derive(Debug, Clone, Serialize)]
pub struct Earthwork {
    pub cut_bank_m3: f64,
    pub fill_compacted_m3: f64,
    /// compacted-volume factor applied to cut (e.g. 0.90 = 10 % shrinkage)
    pub shrink_factor: f64,
    /// loose-volume factor for haulage (e.g. 1.25 = 25 % bulking)
    pub bulking_factor: f64,
    pub usable_cut_compacted_m3: f64,
    pub borrow_compacted_m3: f64,
    pub waste_bank_m3: f64,
    pub waste_loose_m3: f64,
    pub haul_loose_m3: f64,
    pub free_haul_m: f64,
    /// per balanced segment: (from, to, volume m³, haul moment m³·m, average haul m, overhaul m³·m)
    pub segments: Vec<[f64; 6]>,
    pub overhaul_m3m: f64,
    pub method: String,
}

pub fn earthwork(c: &Corridor, mh: &MassHaul, bulking: f64) -> Earthwork {
    let o = &mh.ordinates;
    // split at zero crossings
    let mut segs = Vec::new();
    let mut cur: Vec<[f64; 2]> = vec![o[0]];
    for w in o.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a[1] > 0.0 && b[1] < 0.0) || (a[1] < 0.0 && b[1] > 0.0) {
            let t = a[1] / (a[1] - b[1]);
            let z = [a[0] + t * (b[0] - a[0]), 0.0];
            cur.push(z);
            segs.push(std::mem::replace(&mut cur, vec![z]));
        }
        cur.push(b);
    }
    segs.push(cur);
    let mut rows = Vec::new();
    let mut over = 0.0;
    for s in &segs {
        if s.len() < 2 {
            continue;
        }
        // volume moved inside a segment = peak |M| relative to the segment's end ordinates
        let base = s[0][1];
        let v = s.iter().map(|p| (p[1] - base).abs()).fold(0.0, f64::max);
        let mut h = 0.0;
        for w in s.windows(2) {
            h += ((w[0][1] - base).abs() + (w[1][1] - base).abs()) / 2.0 * (w[1][0] - w[0][0]);
        }
        let avg = if v > 0.0 { h / v } else { 0.0 };
        let oh = if avg > mh.free_haul_m { h - v * mh.free_haul_m } else { 0.0 };
        over += oh;
        rows.push([s[0][0], s.last().unwrap()[0], v, h, avg, oh]);
    }
    let usable = c.cut_m3 * mh.cut_factor;
    let borrow = (c.fill_m3 - usable).max(0.0);
    let waste_c = (usable - c.fill_m3).max(0.0);
    let waste_bank = waste_c / mh.cut_factor;
    Earthwork {
        cut_bank_m3: c.cut_m3,
        fill_compacted_m3: c.fill_m3,
        shrink_factor: mh.cut_factor,
        bulking_factor: bulking,
        usable_cut_compacted_m3: usable,
        borrow_compacted_m3: borrow,
        waste_bank_m3: waste_bank,
        waste_loose_m3: waste_bank * bulking,
        haul_loose_m3: c.cut_m3 * bulking,
        free_haul_m: mh.free_haul_m,
        segments: rows,
        overhaul_m3m: over,
        method: "Mass ordinates M = Σ(cut·shrink − fill). The diagram is split at balance points; in each segment V = peak |M − M_start| and H = ∫|M − M_start| ds. Average haul = H/V; overhaul = H − V·free-haul when the average haul exceeds free haul (approximation of the free-haul-band method). The end ordinate gives borrow (−) or waste (+).".into(),
    }
}
