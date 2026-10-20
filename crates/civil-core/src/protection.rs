//! Protection works:
//! - Roadside hazards and barrier need within the clear zone.
//! - Ditch lining by permissible shear stress (FHWA HEC-15).
//! - Slope protection by slope class.
//!
//! The clear-zone width, slope traversability limits and barrier run-out length come from
//! tables that are copyrighted (AASHTO Roadside Design Guide, Austroads AGRD Part 6) or that
//! are risk-based (DMRB CD 377 RRRAP). The engineer enters them. The tool applies them
//! geometrically to every corridor section and reports the hazards it finds and the barrier
//! runs it proposes.
use crate::corridor::{Corridor, SideSection};
use crate::standards::{chk, Check, Standard};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lining {
    pub name: String,
    /// Manning n (ignored for riprap/gravel where Eq 6.1 is used)
    pub n: f64,
    /// permissible shear (N/m²)
    pub tau_p: f64,
    #[serde(default)]
    pub citation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionInput {
    /// clear zone width from the edge of the travelled way (m), engineer input
    pub clear_zone_m: f64,
    #[serde(default)]
    pub clear_zone_source: String,
    /// slopes flatter than 1V:recoverable_h are recoverable; steeper than 1V:traversable_h are critical
    pub recoverable_h: f64,
    pub traversable_h: f64,
    /// fill height above which a critical slope inside the clear zone needs shielding (engineer warrant)
    pub embankment_warrant_height_m: f64,
    /// barrier face offset from the edge of travelled way (m), run-out length L_R (m)
    pub barrier_offset_m: f64,
    pub runout_length_m: f64,
    #[serde(default = "d_barrier")]
    pub barrier_type: String,
    // --- ditch hydraulics (HEC-15) ---
    pub runoff_c: f64,
    pub intensity_mm_h: f64,
    /// contributing width per metre of ditch (m): half pavement + slope + natural catchment
    pub catchment_width_m: f64,
    /// bare-soil permissible shear (N/m²), from HEC-15 Table 2.3 for the site soil
    pub soil_tau_p: f64,
    #[serde(default)]
    pub soil_name: String,
    /// extra linings (vegetation / RECP) with manufacturer or HEC-15 Ch.4/5 values, in preference order
    #[serde(default)]
    pub extra_linings: Vec<Lining>,
    /// rock specific gravity for riprap (HEC-15 typical 2.65)
    #[serde(default = "d_sg")]
    pub riprap_sg: f64,
    // --- slope protection classes (H per 1V) ---
    #[serde(default = "d_s1")]
    pub seed_flatter_than_h: f64,
    #[serde(default = "d_s2")]
    pub blanket_flatter_than_h: f64,
}
fn d_barrier() -> String {
    "W-beam guardrail".into()
}
fn d_sg() -> f64 {
    2.65
}
fn d_s1() -> f64 {
    3.0
}
fn d_s2() -> f64 {
    2.0
}

#[derive(Debug, Clone, Serialize)]
pub struct Hazard {
    pub station: f64,
    pub side: String,
    pub kind: String,
    pub offset_from_etw_m: f64,
    pub height_m: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BarrierRun {
    pub side: String,
    pub hazard_from: f64,
    pub hazard_to: f64,
    pub from: f64,
    pub to: f64,
    pub length_of_need_m: f64,
    pub length_m: f64,
    pub barrier_type: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DitchReach {
    pub side: String,
    pub from: f64,
    pub to: f64,
    pub flow_dir: String,
    pub q_max_m3s: f64,
    pub worst_station: f64,
    pub depth_m: f64,
    pub velocity_mps: f64,
    pub slope: f64,
    pub tau_d: f64,
    pub lining: String,
    pub tau_p: f64,
    pub riprap_d50_m: Option<f64>,
    pub n_used: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlopeArea {
    pub class: String,
    pub area_m2: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Protection {
    pub hazards: Vec<Hazard>,
    pub barriers: Vec<BarrierRun>,
    pub ditches: Vec<DitchReach>,
    pub slope_protection: Vec<SlopeArea>,
    pub checks: Vec<Check>,
    pub notes: Vec<String>,
}

pub const HEC15: &str = "FHWA HEC-15 Design of Roadside Channels with Flexible Linings, 3rd ed. (2005)";
pub fn barrier_cite(std: Standard) -> &'static str {
    match std {
        Standard::Us => "AASHTO Roadside Design Guide (4th ed., 2011): clear zone (Ch. 3), length of need X = L_R(L_A − L_2)/L_A without flare (Ch. 5); table values engineer-entered",
        Standard::Ca => "TAC Geometric Design Guide (2017) Ch. 7 roadside design (clear zone, barrier need); table values engineer-entered",
        Standard::Uk => "DMRB CD 377 Requirements for road restraint systems: need is risk-based (RRRAP); hazards listed here are inputs to RRRAP, barrier runs indicative only",
        Standard::Au | Standard::Nz => "Austroads AGRD Part 6 Roadside Design, Safety and Barriers: clear zone and length of need; table values engineer-entered",
    }
}

/// HEC-15 Table 6.1: Shields parameter F* and safety factor vs particle Reynolds number.
pub fn shields(re: f64) -> (f64, f64) {
    if re <= 4e4 {
        (0.047, 1.0)
    } else if re >= 2e5 {
        (0.15, 1.5)
    } else {
        let t = (re - 4e4) / (2e5 - 4e4);
        (0.047 + t * (0.15 - 0.047), 1.0 + t * 0.5)
    }
}
/// HEC-15 Eq 6.8 with Table 6.1: minimum stable riprap D50 (m) for depth d (m), slope s.
pub fn riprap_d50(d: f64, s: f64, sg: f64) -> f64 {
    let mut d50 = 0.05;
    for _ in 0..50 {
        let vstar = (9.81 * d * s).sqrt();
        let re = vstar * d50 / 1.131e-6;
        let (f, sf) = shields(re);
        let nd = sf * d * s / (f * (sg - 1.0));
        if (nd - d50).abs() < 1e-6 {
            return nd;
        }
        d50 = nd;
    }
    d50
}
/// HEC-15 Eq 6.1 (Blodgett): n = 0.319·da^(1/6) / (2.25 + 5.23·log10(da/D50)), valid 1.5 ≤ da/D50 ≤ 185.
pub fn blodgett_n(da: f64, d50: f64) -> f64 {
    let r = (da / d50).max(1.5);
    0.319 * da.powf(1.0 / 6.0) / (2.25 + 5.23 * r.log10())
}

/// Normal depth in a trapezoid (bottom b, side slopes z1, z2 H:1V) by bisection; returns (depth, area, velocity).
pub fn trap_normal_depth(q: f64, b: f64, z1: f64, z2: f64, s: f64, n: &dyn Fn(f64) -> f64) -> (f64, f64, f64) {
    let (mut lo, mut hi) = (1e-5, 5.0);
    for _ in 0..100 {
        let y = (lo + hi) / 2.0;
        let a = b * y + 0.5 * (z1 + z2) * y * y;
        let p = b + y * ((1.0 + z1 * z1).sqrt() + (1.0 + z2 * z2).sqrt());
        let qq = a * (a / p).powf(2.0 / 3.0) * s.sqrt() / n(y);
        if qq < q {
            lo = y
        } else {
            hi = y
        }
    }
    let y = (lo + hi) / 2.0;
    let a = b * y + 0.5 * (z1 + z2) * y * y;
    (y, a, q / a)
}

fn etw(sd: &SideSection) -> f64 {
    sd.codes.iter().zip(&sd.fg).filter(|(c, _)| c.starts_with("ETW")).map(|(_, p)| p[0]).fold(0.0, f64::max)
}

pub fn design(std: Standard, c: &Corridor, profile_grade_at: &dyn Fn(f64) -> f64, inp: &ProtectionInput) -> crate::Result<Protection> {
    if !(inp.clear_zone_m > 0.0) || !(inp.runout_length_m > 0.0) || inp.recoverable_h < inp.traversable_h {
        return Err("protection: clear_zone_m > 0, runout_length_m > 0 and recoverable_h ≥ traversable_h required".into());
    }
    let mut hazards = Vec::new();
    let mut slope_area = [0.0f64; 3];
    let mut notes = vec![format!(
        "Clear zone {} m ({}); recoverable ≤ 1V:{}H, traversable ≤ 1V:{}H, embankment warrant height {} m: engineer inputs.",
        inp.clear_zone_m,
        if inp.clear_zone_source.is_empty() { "source not given" } else { &inp.clear_zone_source },
        inp.recoverable_h,
        inp.traversable_h,
        inp.embankment_warrant_height_m
    )];
    let secs = &c.sections;
    for (k, s) in secs.iter().enumerate() {
        let dl = if k == 0 {
            (secs[1].station - s.station) / 2.0
        } else if k + 1 == secs.len() {
            (s.station - secs[k - 1].station) / 2.0
        } else {
            (secs[k + 1].station - secs[k - 1].station) / 2.0
        };
        for (side, sd) in [("left", &s.left), ("right", &s.right)] {
            let e = etw(sd);
            // slope = segment from the hinge (or ditch out) to daylight
            let n = sd.fg.len();
            if n < 2 {
                continue;
            }
            let hinge_i = sd
                .codes
                .iter()
                .rposition(|c| c.starts_with("ETW") || c == "SHLD" || c.contains("CURB") || c == "BACK-WALK" || c.starts_with("DITCH"))
                .unwrap_or(0);
            let hp = sd.fg[hinge_i];
            let dp = sd.fg[n - 1];
            let (run, rise) = (dp[0] - hp[0], (dp[1] - hp[1]).abs());
            let slope_len = (run * run + rise * rise).sqrt();
            if rise > 0.05 {
                let h = run / rise;
                let cls = if h >= inp.seed_flatter_than_h {
                    0
                } else if h >= inp.blanket_flatter_than_h {
                    1
                } else {
                    2
                };
                slope_area[cls] += slope_len * dl;
            }
            if let Some(w) = &sd.wall {
                if w.offset - e < inp.clear_zone_m {
                    hazards.push(Hazard {
                        station: s.station,
                        side: side.into(),
                        kind: if w.cut { "retaining wall face (cut)".into() } else { "retaining wall drop-off (fill)".into() },
                        offset_from_etw_m: w.offset - e,
                        height_m: w.top_z - w.bottom_z,
                    });
                    continue;
                }
            }
            if !sd.cut && rise > 0.05 && hp[0] - e < inp.clear_zone_m {
                let h = run / rise;
                if h < inp.traversable_h && rise > inp.embankment_warrant_height_m {
                    hazards.push(Hazard {
                        station: s.station,
                        side: side.into(),
                        kind: format!("critical fill slope 1V:{h:.1}H"),
                        offset_from_etw_m: hp[0] - e,
                        height_m: rise,
                    });
                } else if h < inp.recoverable_h && hp[0] - e + run < inp.clear_zone_m {
                    notes.push(format!(
                        "{side} {:.0}: non-recoverable fill slope 1V:{h:.1}H inside the clear zone; clear run-out area needed at the toe",
                        s.station
                    ));
                }
            }
            if sd.cut && sd.codes.iter().any(|c| c == "DITCH-IN") {
                let i = sd.codes.iter().position(|c| c == "DITCH-IN").unwrap();
                let a = sd.fg[i - 1];
                let b = sd.fg[i];
                let fs = (b[0] - a[0]) / (a[1] - b[1]).max(1e-9);
                if fs < inp.recoverable_h && a[0] - e < inp.clear_zone_m {
                    notes.push(format!("{side} {:.0}: ditch foreslope 1V:{fs:.1}H is steeper than recoverable inside the clear zone", s.station));
                }
            }
        }
    }
    // barrier runs: contiguous hazard stations per side
    let mut barriers = Vec::new();
    let la = inp.clear_zone_m;
    let x = (inp.runout_length_m * (la - inp.barrier_offset_m) / la).max(0.0);
    for side in ["left", "right"] {
        let st: Vec<f64> = secs.iter().map(|s| s.station).collect();
        let hz: Vec<bool> = secs.iter().map(|s| hazards.iter().any(|h| h.side == side && h.station == s.station)).collect();
        let mut k = 0;
        while k < hz.len() {
            if !hz[k] {
                k += 1;
                continue;
            }
            let a = k;
            while k + 1 < hz.len() && hz[k + 1] {
                k += 1;
            }
            let (h0, h1) = (st[a], st[k]);
            let from = (h0 - x).max(st[0]);
            let to = (h1 + x).min(*st.last().unwrap());
            let kinds: std::collections::BTreeSet<String> = hazards
                .iter()
                .filter(|h| h.side == side && h.station >= h0 && h.station <= h1)
                .map(|h| h.kind.split(' ').take(3).collect::<Vec<_>>().join(" "))
                .collect();
            barriers.push(BarrierRun {
                side: side.into(),
                hazard_from: h0,
                hazard_to: h1,
                from,
                to,
                length_of_need_m: x,
                length_m: to - from,
                barrier_type: inp.barrier_type.clone(),
                reason: kinds.into_iter().collect::<Vec<_>>().join("; "),
            });
            k += 1;
        }
    }
    // merge runs closer than 30 m (gaps shorter than this are usually closed)
    barriers.sort_by(|a, b| (a.side.clone(), a.from).partial_cmp(&(b.side.clone(), b.from)).unwrap());
    let mut merged: Vec<BarrierRun> = Vec::new();
    for b in barriers {
        if let Some(l) = merged.last_mut() {
            if l.side == b.side && b.from - l.to < 30.0 {
                l.to = l.to.max(b.to);
                l.hazard_to = b.hazard_to;
                l.length_m = l.to - l.from;
                if !l.reason.contains(&b.reason) {
                    l.reason = format!("{}; {}", l.reason, b.reason);
                }
                continue;
            }
        }
        merged.push(b);
    }
    // ditches
    let mut ditches = Vec::new();
    let mut checks = Vec::new();
    let mut linings: Vec<Lining> = vec![Lining {
        name: format!("bare soil ({})", if inp.soil_name.is_empty() { "site soil" } else { &inp.soil_name }),
        n: 0.020,
        tau_p: inp.soil_tau_p,
        citation: format!("{HEC15} Table 2.3 (engineer-selected soil); n typical Table 2.1"),
    }];
    linings.extend(inp.extra_linings.iter().cloned());
    linings.push(Lining { name: "gravel mulch D50 25 mm".into(), n: f64::NAN, tau_p: 19.0, citation: format!("{HEC15} Table 2.3") });
    linings.push(Lining { name: "gravel mulch D50 50 mm".into(), n: f64::NAN, tau_p: 38.0, citation: format!("{HEC15} Table 2.3") });
    for side in ["left", "right"] {
        let has: Vec<bool> = secs
            .iter()
            .map(|s| {
                let sd = if side == "left" { &s.left } else { &s.right };
                sd.cut && sd.codes.iter().any(|c| c == "DITCH-IN")
            })
            .collect();
        let mut k = 0;
        while k < has.len() {
            if !has[k] {
                k += 1;
                continue;
            }
            let a = k;
            while k + 1 < has.len() && has[k + 1] {
                k += 1;
            }
            let b = k;
            k += 1;
            if b == a {
                continue;
            }
            let (s0, s1) = (secs[a].station, secs[b].station);
            let mean_g = (secs[b].cl_elev - secs[a].cl_elev) / (s1 - s0);
            let down = mean_g < 0.0; // flows up-station when the profile falls
            let sd0 = if side == "left" { &secs[a].left } else { &secs[a].right };
            let i = sd0.codes.iter().position(|c| c == "DITCH-IN").unwrap();
            let fs = (sd0.fg[i][0] - sd0.fg[i - 1][0]) / (sd0.fg[i - 1][1] - sd0.fg[i][1]).max(1e-9);
            let bw = sd0.codes.iter().position(|c| c == "DITCH-OUT").map_or(0.0, |j| sd0.fg[j][0] - sd0.fg[i][0]);
            let bs = c.assembly.side_cfg(side == "left").cut_slope_h;
            // worst section: max τd along the reach
            let mut worst: Option<(f64, f64, f64, f64, f64, f64)> = None; // st, q, d, v, s, tau
            for s in &secs[a..=b] {
                let flen = if down { s.station - s0 } else { s1 - s.station };
                let q = inp.runoff_c * inp.intensity_mm_h * inp.catchment_width_m * flen.max(1.0) / 3.6e6;
                let sl = profile_grade_at(s.station).abs().max(0.001);
                let (d, _, v) = trap_normal_depth(q, bw, fs, bs, sl, &|_| 0.020);
                let tau = 9810.0 * d * sl;
                if worst.is_none_or(|w| tau > w.5) {
                    worst = Some((s.station, q, d, v, sl, tau));
                }
            }
            let (wst, q, _, _, sl, _) = worst.unwrap();
            let mut chosen = None;
            for l in &linings {
                let nf = |y: f64| if l.n.is_nan() { blodgett_n(y, if l.name.contains("25") { 0.025 } else { 0.05 }) } else { l.n };
                let (d, _, v) = trap_normal_depth(q, bw, fs, bs, sl, &nf);
                let tau = 9810.0 * d * sl;
                if tau <= l.tau_p {
                    chosen = Some((l.name.clone(), l.tau_p, None, nf(d), d, v, tau, l.citation.clone()));
                    break;
                }
            }
            if chosen.is_none() {
                // riprap: iterate D50 with Blodgett n
                let mut d50 = 0.15;
                let mut res = None;
                for _ in 0..30 {
                    let (d, _, v) = trap_normal_depth(q, bw, fs, bs, sl, &|y| blodgett_n(y, d50));
                    let need = riprap_d50(d, sl, inp.riprap_sg);
                    res = Some((d, v));
                    if (need - d50).abs() < 0.005 {
                        break;
                    }
                    d50 = need.max(0.05);
                }
                let (d, v) = res.unwrap();
                let d50r = (d50 / 0.05).ceil() * 0.05;
                let tau = 9810.0 * d * sl;
                if d50r <= 0.6 {
                    let (d, _, v) = trap_normal_depth(q, bw, fs, bs, sl, &|y| blodgett_n(y, d50r));
                    let re = (9.81 * d * sl).sqrt() * d50r / 1.131e-6;
                    let (f, sf) = shields(re);
                    let taup = f * (inp.riprap_sg - 1.0) * 9810.0 * d50r / sf;
                    chosen = Some((
                        format!("rock riprap D50 {:.0} mm", d50r * 1000.0),
                        taup,
                        Some(d50r),
                        blodgett_n(d, d50r),
                        d,
                        v,
                        9810.0 * d * sl,
                        format!("{HEC15} Eq 6.7/6.8 with Table 6.1 (F* = {f:.3}, SF = {sf:.2}; τp shown divided by SF), Eq 6.1 n"),
                    ));
                } else {
                    let _ = (d, v, tau);
                    let (d, _, v) = trap_normal_depth(q, bw, fs, bs, sl, &|_| 0.013);
                    chosen = Some((
                        "concrete (rigid) lining".into(),
                        f64::INFINITY,
                        None,
                        0.013,
                        d,
                        v,
                        9810.0 * d * sl,
                        format!("{HEC15} Ch. 3: rigid lining; n from Table 2.1"),
                    ));
                }
            }
            let (name, taup, d50, n, d, v, tau, cite) = chosen.unwrap();
            let mut notes = vec![format!(
                "Q = C·i·A/3.6e6 with A = {} m × flow length; normal depth by Manning in a trapezoid b = {bw:.2} m, z = {fs:.1}/{bs:.1}",
                inp.catchment_width_m
            )];
            if let Some(dd) = d50 {
                if d / dd < 1.5 {
                    notes.push("da/D50 < 1.5: HEC-15 Eq 6.2 (Bathurst) governs n; Eq 6.1 at da/D50 = 1.5 used as an approximation; engineer to confirm".into());
                }
            }
            let el = format!("{side} ditch {s0:.0}–{s1:.0}");
            if taup.is_finite() {
                checks.push(chk(
                    "D-SHEAR",
                    "Ditch lining: design shear ≤ permissible shear",
                    el.clone(),
                    tau,
                    taup,
                    "N/m²",
                    "max",
                    "mandatory",
                    format!("τd = γ·d·S = 9810·{d:.3}·{sl:.4} = {tau:.1} N/m²; lining {name}"),
                    cite.clone(),
                ));
            }
            let freeboard_ok = d <= sd0.fg[i - 1][1] - sd0.fg[i][1];
            checks.push(chk(
                "D-DEPTH",
                "Ditch flow depth within ditch depth",
                el,
                d,
                sd0.fg[i - 1][1] - sd0.fg[i][1],
                "m",
                "max",
                "mandatory",
                format!("normal depth {d:.3} m (freeboard ok: {freeboard_ok})"),
                format!("{HEC15} Ch. 2 (channel capacity)"),
            ));
            ditches.push(DitchReach {
                side: side.into(),
                from: s0,
                to: s1,
                flow_dir: if down { "up-station".into() } else { "down-station".into() },
                q_max_m3s: q,
                worst_station: wst,
                depth_m: d,
                velocity_mps: v,
                slope: sl,
                tau_d: tau,
                lining: name,
                tau_p: taup,
                riprap_d50_m: d50,
                n_used: n,
                notes,
            });
        }
    }
    let slope_protection = vec![
        SlopeArea { class: format!("topsoil and seed (flatter than 1V:{}H)", inp.seed_flatter_than_h), area_m2: slope_area[0] },
        SlopeArea {
            class: format!("rolled erosion control blanket (1V:{}H to 1V:{}H)", inp.blanket_flatter_than_h, inp.seed_flatter_than_h),
            area_m2: slope_area[1],
        },
        SlopeArea {
            class: format!("turf reinforcement mat / rock slope protection (steeper than 1V:{}H)", inp.blanket_flatter_than_h),
            area_m2: slope_area[2],
        },
    ];
    notes.push(format!(
        "Slope protection classes are engineer-set thresholds (agency practice), not values from a standard. Barrier need: {}",
        barrier_cite(std)
    ));
    Ok(Protection { hazards, barriers: merged, ditches, slope_protection, checks, notes })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shields_table() {
        assert_eq!(shields(1e4), (0.047, 1.0));
        assert_eq!(shields(3e5), (0.15, 1.5));
        let (f, s) = shields(1.2e5);
        assert!((f - 0.0985).abs() < 1e-9 && (s - 1.25).abs() < 1e-9);
    }
    #[test]
    fn riprap_eq68_low_re() {
        // d = 0.3 m, S = 0.02, SG 2.65: Re* small → D50 = 0.3·0.02/(0.047·1.65) = 0.0774 m
        let d = riprap_d50(0.3, 0.02, 2.65);
        let vstar = (9.81f64 * 0.3 * 0.02).sqrt();
        assert!(vstar * d / 1.131e-6 < 4e4);
        assert!((d - 0.3 * 0.02 / (0.047 * 1.65)).abs() < 1e-6, "{d}");
        // steep channel: Re* falls in the interpolation band; the result must be self-consistent
        let d2 = riprap_d50(0.5, 0.08, 2.65);
        let re = (9.81f64 * 0.5 * 0.08).sqrt() * d2 / 1.131e-6;
        assert!(re > 4e4 && re < 2e5);
        let (f, sf) = shields(re);
        assert!((d2 - sf * 0.5 * 0.08 / (f * 1.65)).abs() < 1e-5, "{d2}");
    }
    #[test]
    fn trapezoid_depth_roundtrip() {
        let (y, a, v) = trap_normal_depth(0.5, 1.0, 3.0, 3.0, 0.01, &|_| 0.03);
        assert!((a * v - 0.5).abs() < 1e-6 && y > 0.0);
    }
}
