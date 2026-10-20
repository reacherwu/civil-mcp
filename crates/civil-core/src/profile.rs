use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInput {
    pub name: String,
    /// Vertical points of intersection: [station, elevation].
    pub vpis: Vec<[f64; 2]>,
    /// Symmetric parabolic vertical curve length at each interior VPI (0 = none).
    pub curve_lengths: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerticalCurve {
    pub vpi_index: usize,
    pub station: f64,
    pub length: f64,
    pub g1_pct: f64,
    pub g2_pct: f64,
    pub a_pct: f64,
    pub k: f64,
    pub crest: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub input: ProfileInput,
    pub grades_pct: Vec<f64>,
    pub curves: Vec<VerticalCurve>,
}

impl Profile {
    pub fn build(input: ProfileInput) -> crate::Result<Profile> {
        let v = &input.vpis;
        if v.len() < 2 {
            return Err("need >= 2 VPIs".into());
        }
        if input.curve_lengths.len() != v.len() - 2 {
            return Err(format!("need {} curve lengths, got {}", v.len() - 2, input.curve_lengths.len()));
        }
        for w in v.windows(2) {
            if w[1][0] <= w[0][0] {
                return Err("VPI stations must increase".into());
            }
        }
        let g: Vec<f64> = v.windows(2).map(|w| (w[1][1] - w[0][1]) / (w[1][0] - w[0][0]) * 100.0).collect();
        let mut curves = Vec::new();
        for i in 1..v.len() - 1 {
            let l = input.curve_lengths[i - 1];
            if l < 0.0 {
                return Err("curve length must be >= 0".into());
            }
            let a = g[i] - g[i - 1];
            curves.push(VerticalCurve {
                vpi_index: i,
                station: v[i][0],
                length: l,
                g1_pct: g[i - 1],
                g2_pct: g[i],
                a_pct: a,
                k: if a.abs() > 1e-12 { l / a.abs() } else { f64::INFINITY },
                crest: a < 0.0,
            });
        }
        // curves must fit between neighbouring curves / ends
        let ends: Vec<(f64, f64)> = curves.iter().map(|c| (c.station - c.length / 2.0, c.station + c.length / 2.0)).collect();
        for (i, c) in curves.iter().enumerate() {
            let lo = if i == 0 { v[0][0] } else { ends[i - 1].1 };
            let hi = if i + 1 == curves.len() { v[v.len() - 1][0] } else { ends[i + 1].0 };
            if ends[i].0 < lo - 1e-9 || ends[i].1 > hi + 1e-9 {
                return Err(format!("vertical curve at VPI {} (L={:.1} m) overlaps its neighbour or the profile end", c.vpi_index, c.length));
            }
        }
        Ok(Profile { input, grades_pct: g, curves })
    }

    pub fn start(&self) -> f64 {
        self.input.vpis[0][0]
    }
    pub fn end(&self) -> f64 {
        self.input.vpis.last().unwrap()[0]
    }

    pub fn elevation_at(&self, s: f64) -> Option<f64> {
        let v = &self.input.vpis;
        if s < self.start() - 1e-6 || s > self.end() + 1e-6 {
            return None;
        }
        for c in &self.curves {
            let (bvc, evc) = (c.station - c.length / 2.0, c.station + c.length / 2.0);
            if c.length > 0.0 && s >= bvc && s <= evc {
                let vp = v[c.vpi_index];
                let zb = vp[1] - c.g1_pct / 100.0 * c.length / 2.0;
                let x = s - bvc;
                return Some(zb + c.g1_pct / 100.0 * x + (c.g2_pct - c.g1_pct) / 100.0 / (2.0 * c.length) * x * x);
            }
        }
        let i = v.windows(2).position(|w| s <= w[1][0] + 1e-9).unwrap_or(v.len() - 2);
        Some(v[i][1] + self.grades_pct[i] / 100.0 * (s - v[i][0]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parabola_matches_closed_form() {
        let p = Profile::build(ProfileInput { name: "p".into(), vpis: vec![[0.0, 100.0], [200.0, 110.0], [400.0, 104.0]], curve_lengths: vec![120.0] }).unwrap();
        let c = &p.curves[0];
        assert!(c.crest && (c.a_pct + 8.0).abs() < 1e-9 && (c.k - 15.0).abs() < 1e-9);
        // at VPI the curve is offset by A*L/800 below the VPI
        let z = p.elevation_at(200.0).unwrap();
        assert!((z - (110.0 - 8.0 * 120.0 / 800.0)).abs() < 1e-9);
        // continuity at BVC/EVC
        assert!((p.elevation_at(140.0).unwrap() - 107.0).abs() < 1e-9);
        assert!((p.elevation_at(260.0).unwrap() - 108.2).abs() < 1e-9);
    }
}
