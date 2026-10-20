//! Concept-depth traffic engineering layouts: pavement marking with no-passing zones from
//! available sight distance, regulatory/warning sign schedule, street-lighting pole layout by
//! the lumen method, and temporary traffic management (TTM) staging with taper lengths.
//! Country manuals: US MUTCD 11th ed. (2023), Canada TAC MUTCDC, UK TSRGD 2016 + Traffic Signs
//! Manual (Ch. 3/4/5/8), Australia AS 1742 series, NZ MOTSAM / Traffic Control Devices Manual and
//! CoPTTM. Values that the profile cannot verify are engineer inputs; sign codes must be confirmed.
use crate::align::Alignment;
use crate::profile::Profile;
use crate::standards::Standard;
use serde::{Deserialize, Serialize};

pub fn manual(std: Standard) -> &'static str {
    match std {
        Standard::Us => "FHWA MUTCD 11th Edition (2023)",
        Standard::Ca => "TAC Manual of Uniform Traffic Control Devices for Canada (MUTCDC, 6th ed.)",
        Standard::Uk => "Traffic Signs Regulations and General Directions 2016; Traffic Signs Manual Ch. 3, 4, 5, 8",
        Standard::Au => "AS 1742 Manual of uniform traffic control devices (Parts 1, 2, 3)",
        Standard::Nz => "NZTA Traffic Control Devices Manual / MOTSAM; CoPTTM for temporary traffic management",
    }
}

/// MUTCD (2009 ed.) Table 3B-1 minimum passing sight distance (ft) by 85th-percentile / posted speed (mph).
const PSD_MUTCD: [(f64, f64); 10] =
    [(25.0, 450.0), (30.0, 500.0), (35.0, 550.0), (40.0, 600.0), (45.0, 700.0), (50.0, 800.0), (55.0, 900.0), (60.0, 1000.0), (65.0, 1100.0), (70.0, 1200.0)];

pub fn psd_mutcd_m(v_kmh: f64) -> f64 {
    let mph = v_kmh / 1.609344;
    let mph = (mph / 5.0).ceil() * 5.0;
    PSD_MUTCD.iter().find(|r| r.0 >= mph - 1e-9).map_or(1200.0, |r| r.1) * 0.3048
}

/// Available vertical sight distance at station s (eye/object height h) looking up-station.
pub fn sight_vertical(pr: &Profile, s: f64, s_end: f64, h: f64, step: f64) -> f64 {
    let z0 = pr.elevation_at(s).unwrap_or(0.0) + h;
    let mut d = step;
    while s + d <= s_end {
        let z1 = pr.elevation_at(s + d).unwrap_or(0.0) + h;
        let mut k = step;
        let mut clear = true;
        while k < d {
            let zl = z0 + (z1 - z0) * k / d;
            if pr.elevation_at(s + k).unwrap_or(0.0) > zl {
                clear = false;
                break;
            }
            k += step;
        }
        if !clear {
            return d - step;
        }
        d += step;
    }
    s_end - s
}

/// Vertical sight distance in direction dir (+1 up-station, -1 down-station), limited at s_lim.
pub fn sight_vertical_dir(pr: &Profile, s: f64, s_lim: f64, h: f64, step: f64, dir: f64) -> f64 {
    let z0 = pr.elevation_at(s).unwrap_or(0.0) + h;
    let avail = (s_lim - s) * dir;
    let mut d = step;
    while d <= avail {
        let z1 = pr.elevation_at(s + dir * d).unwrap_or(0.0) + h;
        let mut k = step;
        while k < d {
            if pr.elevation_at(s + dir * k).unwrap_or(0.0) > z0 + (z1 - z0) * k / d {
                return d - step;
            }
            k += step;
        }
        d += step;
    }
    avail
}

/// Horizontal sightline offset needed for sight distance S on radius R: M = R(1 - cos(S/2R)).
pub fn hso_m(r: f64, s: f64) -> f64 {
    r * (1.0 - (s / (2.0 * r)).min(std::f64::consts::PI).cos())
}

#[derive(Debug, Clone, Serialize)]
pub struct Zone {
    pub from: f64,
    pub to: f64,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sign {
    pub code: String,
    pub name: String,
    pub station: f64,
    pub side: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Marking {
    pub psd_m: f64,
    pub psd_source: String,
    pub centre_colour: String,
    pub edge_colour: String,
    pub no_passing: Vec<Zone>,
    pub signs: Vec<Sign>,
    pub notes: Vec<String>,
}

pub fn marking(
    std: Standard,
    al: &Alignment,
    pr: &Profile,
    v_kmh: f64,
    psd_engineer: Option<f64>,
    h: f64,
    min_npz_m: f64,
    curve_warning_r_m: f64,
    lateral_clear_m: Option<f64>,
) -> crate::Result<Marking> {
    let (psd, src) = match (std, psd_engineer) {
        (_, Some(p)) => (p, "engineer input".to_string()),
        (Standard::Us, None) => (psd_mutcd_m(v_kmh), "MUTCD Table 3B-1 (2009 ed.; verify numbering in the 11th ed.), ft→m".to_string()),
        _ => {
            return Err(
                "passing/overtaking sight distance must be entered by the engineer for this profile (TAC MUTCDC / TSM Ch. 5 / AS 1742.2 / MOTSAM)".into()
            )
        }
    };
    let (s0, s1) = (al.start_station(), al.end_station());
    let step = 5.0;
    let mut zones = vec![];
    for (dir, lim, name) in [(1.0, s1, "up-station"), (-1.0, s0, "down-station")] {
        let mut flags = vec![];
        let mut s = s0;
        while s <= s1 {
            let sd = sight_vertical_dir(pr, s, lim, h, step, dir);
            let room = (lim - s) * dir;
            let mut np = sd < psd && room > psd;
            // horizontal curves: sight line offset needed vs the engineer's lateral clearance
            if let Some(clear) = lateral_clear_m {
                for c in al.curves() {
                    if s >= c.sta_start && s <= c.sta_end && hso_m(c.radius, psd) > clear {
                        np = true;
                    }
                }
            }
            flags.push((s, np));
            s += step;
        }
        if dir < 0.0 {
            flags.reverse();
        }
        let mut cur: Option<f64> = None;
        for (st, np) in flags.iter().chain(std::iter::once(&(f64::NAN, false))) {
            match (cur, *np) {
                (None, true) => cur = Some(*st),
                (Some(a), false) => {
                    let b = if st.is_nan() {
                        if dir > 0.0 {
                            s1
                        } else {
                            s0
                        }
                    } else {
                        *st
                    };
                    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                    if hi - lo >= min_npz_m {
                        zones.push(Zone { from: lo, to: hi, direction: name.into() });
                    }
                    cur = None;
                }
                _ => {}
            }
        }
    }
    let (cc, ec) = match std {
        Standard::Us | Standard::Ca => ("yellow", "white"),
        _ => ("white", "white"),
    };
    let mut signs = vec![];
    let code = |us: &str, ca: &str, uk: &str, au: &str, nz: &str| {
        match std {
            Standard::Us => us,
            Standard::Ca => ca,
            Standard::Uk => uk,
            Standard::Au => au,
            Standard::Nz => nz,
        }
        .to_string()
    };
    for (i, c) in al.curves().iter().enumerate() {
        if c.radius < curve_warning_r_m {
            let st = c.sta_start;
            signs.push(Sign {
                code: code("W1-2", "WA-1/WA-2", "diag. 512", "W1-1/W1-2", "PW-1/PW-2"),
                name: "Curve warning (with advisory speed if required)".into(),
                station: (st - 60.0).max(s0),
                side: "right".into(),
                reason: format!("curve {} R {:.0} m < {:.0} m (engineer threshold)", i + 1, c.radius, curve_warning_r_m),
            });
        }
    }
    signs.push(Sign {
        code: code("R2-1", "Rb-1", "diag. 670", "R4-1", "R1-1"),
        name: "Speed limit".into(),
        station: s0 + 20.0,
        side: "right".into(),
        reason: format!("start of project, {v_kmh:.0} km/h design speed (posted limit set by the road authority)"),
    });
    for z in &zones {
        if std == Standard::Us {
            signs.push(Sign {
                code: "W14-3".into(),
                name: "No passing zone pennant".into(),
                station: if z.direction == "up-station" { z.from } else { z.to },
                side: "left".into(),
                reason: format!("start of no-passing zone ({})", z.direction),
            });
        }
    }
    Ok(Marking { psd_m: psd, psd_source: src, centre_colour: cc.into(), edge_colour: ec.into(), no_passing: zones, signs,
        notes: vec![format!("Manual: {}", manual(std)), "No-passing zones in both directions from vertical sight distance on the design profile (eye = object height, per the manual) and, where lateral_clear_m is given, from the horizontal sight-line offset M = R(1 - cos(PSD/2R)) on curves. Intersection signing is added by the engineer.".into(), "Sign codes are the usual designations in each manual and must be confirmed against the current edition.".into()] })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingInput {
    pub lumens: f64,
    pub cu: f64,
    pub llf: f64,
    pub e_avg_lux: f64,
    pub mounting_h_m: f64,
    #[serde(default = "d_side")]
    pub arrangement: String,
    pub max_spacing_to_height: f64,
    #[serde(default)]
    pub source: String,
}
fn d_side() -> String {
    "one-side".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct Lighting {
    pub width_m: f64,
    pub spacing_m: f64,
    pub spacing_lumen_m: f64,
    pub poles: Vec<(f64, f64, f64, String)>,
    pub steps: Vec<String>,
}

/// Lumen method: S = Φ·CU·LLF / (E·W); capped at max S/H ratio.
pub fn lighting(al: &Alignment, width: f64, l: &LightingInput) -> Lighting {
    let s_l = l.lumens * l.cu * l.llf / (l.e_avg_lux * width);
    let s = s_l.min(l.max_spacing_to_height * l.mounting_h_m);
    let mut poles = vec![];
    let mut st = al.start_station();
    let mut k = 0;
    while st <= al.end_station() + 1e-9 {
        let (x, y, h) = al.point_at(st).unwrap();
        let side = if l.arrangement == "staggered" && k % 2 == 1 { -1.0 } else { 1.0 };
        let o = side * (width / 2.0 + 1.0);
        poles.push((st, x - o * h.sin(), y + o * h.cos(), if side > 0.0 { "left".into() } else { "right".into() }));
        st += s;
        k += 1;
    }
    Lighting {
        width_m: width,
        spacing_m: s,
        spacing_lumen_m: s_l,
        poles,
        steps: vec![format!(
            "S = Φ·CU·LLF/(E·W) = {}·{}·{}/({}·{width:.2}) = {s_l:.1} m; limit {}·H = {:.1} m; used {s:.1} m",
            l.lumens,
            l.cu,
            l.llf,
            l.e_avg_lux,
            l.max_spacing_to_height,
            l.max_spacing_to_height * l.mounting_h_m
        )],
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Stage {
    pub n: usize,
    pub name: String,
    pub from: f64,
    pub to: f64,
    pub traffic: String,
    pub works: String,
    pub taper_m: Option<f64>,
    pub devices: Vec<String>,
}

/// US MUTCD (2009 ed.) Table 6C-4: merging taper L = W·S²/60 (S ≤ 40 mph) or W·S (S ≥ 45 mph), W ft, S mph.
pub fn mutcd_taper_m(w_m: f64, v_kmh: f64) -> f64 {
    let (w, s) = (w_m * 3.28084, v_kmh / 1.609344);
    (if s <= 40.0 { w * s * s / 60.0 } else { w * s }) * 0.3048
}

pub fn ttm(std: Standard, al: &Alignment, existing_v_kmh: f64, lane_w: f64, taper_engineer: Option<f64>) -> crate::Result<(Vec<Stage>, String)> {
    let t = match (std, taper_engineer) {
        (_, Some(t)) => Some(t),
        (Standard::Us, None) => Some(mutcd_taper_m(lane_w, existing_v_kmh)),
        _ => None,
    };
    let (s0, s1) = (al.start_station(), al.end_station());
    let dev = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let st = vec![
        Stage {
            n: 1,
            name: "Off-line construction".into(),
            from: s0 + 50.0,
            to: s1 - 50.0,
            traffic: "existing network unaffected; site access only".into(),
            works: "earthworks, drainage, culverts, pavement away from tie-ins".into(),
            taper_m: None,
            devices: dev(&["site access signs", "construction entrance", "fencing"]),
        },
        Stage {
            n: 2,
            name: "Start tie-in".into(),
            from: s0 - 50.0,
            to: s0 + 50.0,
            traffic: "single-lane alternating flow on the existing road (flagger / portable signals)".into(),
            works: "tie-in pavement, kerb, drainage connection at the start".into(),
            taper_m: t,
            devices: dev(&["advance warning series", "flagger or portable signals", "channelising devices on the taper"]),
        },
        Stage {
            n: 3,
            name: "End tie-in".into(),
            from: s1 - 50.0,
            to: s1 + 50.0,
            traffic: "single-lane alternating flow on the existing road".into(),
            works: "tie-in at the end".into(),
            taper_m: t,
            devices: dev(&["advance warning series", "flagger or portable signals", "channelising devices"]),
        },
        Stage {
            n: 4,
            name: "Switch and finishing".into(),
            from: s0,
            to: s1,
            traffic: "traffic on the new alignment; shoulder closures as needed".into(),
            works: "line marking, signs, barrier, lighting, landscaping".into(),
            taper_m: t.map(|x| x / 3.0),
            devices: dev(&["shoulder taper (L/3)", "works vehicle with arrow board"]),
        },
    ];
    let src = match (std, taper_engineer) {
        (_, Some(_)) => format!("taper length engineer input; {}", manual(std)),
        (Standard::Us, None) => "MUTCD §6C.08 / Table 6C-4 (2009 ed.; verify numbering in the 11th ed.) taper L = WS²/60 (≤ 40 mph) or WS (≥ 45 mph)".into(),
        _ => format!("{} - taper lengths are engineer inputs (not computed)", manual(std)),
    };
    Ok((st, src))
}
