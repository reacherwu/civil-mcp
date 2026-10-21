//! 3D CAD handoff (DXF R12 / AC1009, world coordinates in metres) for Civil 3D and other CAD:
//! existing and finished-grade TIN faces (3DFACE), the 3D centreline, corridor feature lines
//! (3D polylines by point code), wall tops, pipes at invert and structures at rim. Layer names
//! follow the active drafting profile (NCS / ISO 13567) or an engineer layer map.
use crate::align::Alignment;
use crate::corridor::Corridor;
use crate::frame::{layer_name, Profile as DProfile};
use crate::pipes::Network;
use crate::profile::Profile;
use crate::tin::Tin;
use std::collections::BTreeMap;
use std::fmt::Write;

pub struct Model<'a> {
    pub eg: Option<&'a Tin>,
    pub fg: Option<&'a Tin>,
    pub al: Option<&'a Alignment>,
    pub pr: Option<&'a Profile>,
    pub cor: Option<&'a Corridor>,
    pub nets: Vec<&'a Network>,
    /// imported parcels (user data) and the ROW polygon of the current corridor, draped on EG
    pub parcels: Option<&'a crate::parcels::ParcelSet>,
    pub row_polygon: Option<Vec<[f64; 2]>>,
}

#[derive(Default, serde::Serialize)]
pub struct Stats {
    pub layers: BTreeMap<String, usize>,
    pub faces: usize,
    pub polylines: usize,
    pub lines: usize,
    pub circles: usize,
}

struct W {
    s: String,
    st: Stats,
}
impl W {
    fn count(&mut self, l: &str) {
        *self.st.layers.entry(l.to_string()).or_default() += 1;
    }
    fn face(&mut self, l: &str, p: [[f64; 3]; 3]) {
        let _ = write!(self.s, "0\n3DFACE\n8\n{l}\n");
        for (i, q) in [p[0], p[1], p[2], p[2]].iter().enumerate() {
            let _ = write!(self.s, "1{i}\n{:.4}\n2{i}\n{:.4}\n3{i}\n{:.4}\n", q[0], q[1], q[2]);
        }
        self.st.faces += 1;
        self.count(l);
    }
    fn poly3d(&mut self, l: &str, v: &[[f64; 3]]) {
        if v.len() < 2 {
            return;
        }
        let _ = write!(self.s, "0\nPOLYLINE\n8\n{l}\n66\n1\n10\n0.0\n20\n0.0\n30\n0.0\n70\n8\n");
        for q in v {
            let _ = write!(self.s, "0\nVERTEX\n8\n{l}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n70\n32\n", q[0], q[1], q[2]);
        }
        let _ = write!(self.s, "0\nSEQEND\n8\n{l}\n");
        self.st.polylines += 1;
        self.count(l);
    }
    fn line(&mut self, l: &str, a: [f64; 3], b: [f64; 3]) {
        let _ = write!(self.s, "0\nLINE\n8\n{l}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n11\n{:.4}\n21\n{:.4}\n31\n{:.4}\n", a[0], a[1], a[2], b[0], b[1], b[2]);
        self.st.lines += 1;
        self.count(l);
    }
    fn circle(&mut self, l: &str, c: [f64; 3], r: f64) {
        let _ = write!(self.s, "0\nCIRCLE\n8\n{l}\n10\n{:.4}\n20\n{:.4}\n30\n{:.4}\n40\n{r:.4}\n", c[0], c[1], c[2]);
        self.st.circles += 1;
        self.count(l);
    }
}

/// World point of a section offset (`off` ≥ 0 outward on `left` or right side).
pub fn sec_xy(x: f64, y: f64, h: f64, off: f64, left: bool) -> [f64; 2] {
    let s = if left { 1.0 } else { -1.0 };
    [x - s * off * h.sin(), y + s * off * h.cos()]
}

pub fn write(m: &Model, dp: &DProfile, map: &BTreeMap<String, String>) -> (String, Stats) {
    let ln = |k: &str| layer_name(dp, k, map);
    let mut w = W { s: String::new(), st: Stats::default() };
    for (key, t) in [("SURF-EG", m.eg), ("SURF-FG", m.fg)] {
        if let Some(t) = t {
            let l = ln(key);
            for f in &t.tris {
                w.face(&l, [t.pts[f[0]], t.pts[f[1]], t.pts[f[2]]]);
            }
        }
    }
    if let (Some(al), Some(pr)) = (m.al, m.pr) {
        let (s0, s1) = (al.start_station().max(pr.start()), al.end_station().min(pr.end()));
        let n = ((s1 - s0) / 2.0).ceil().max(1.0) as usize;
        let v: Vec<[f64; 3]> = (0..=n)
            .filter_map(|i| {
                let s = s0 + (s1 - s0) * i as f64 / n as f64;
                Some({
                    let (x, y, _) = al.point_at(s)?;
                    [x, y, pr.elevation_at(s)?]
                })
            })
            .collect();
        w.poly3d(&ln("ALIGNMENT"), &v);
    }
    if let Some(c) = m.cor {
        // feature lines: one 3D polyline per (side, point code, occurrence)
        let mut feats: BTreeMap<(bool, String), Vec<[f64; 3]>> = BTreeMap::new();
        let mut walls: BTreeMap<bool, Vec<[f64; 3]>> = BTreeMap::new();
        for s in &c.sections {
            for (left, side) in [(true, &s.left), (false, &s.right)] {
                let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
                for (k, q) in side.fg.iter().enumerate() {
                    let code = side.codes.get(k).map(|x| x.as_str()).unwrap_or("P");
                    let o = seen.entry(code).or_default();
                    let key = if *o == 0 { code.to_string() } else { format!("{code}{o}") };
                    *o += 1;
                    let p = sec_xy(s.x, s.y, s.heading, q[0], left);
                    feats.entry((left, key)).or_default().push([p[0], p[1], q[1]]);
                }
                if let Some(wl) = &side.wall {
                    let p = sec_xy(s.x, s.y, s.heading, wl.offset, left);
                    walls.entry(left).or_default().push([p[0], p[1], wl.top_z]);
                }
            }
        }
        let l = ln("FEATURE");
        for v in feats.values() {
            w.poly3d(&l, v);
        }
        let l = ln("WALL");
        for v in walls.values() {
            w.poly3d(&l, v);
        }
    }
    for n in &m.nets {
        let at = |id: &str| n.structures.iter().find(|s| s.0 == id);
        let lp = ln("PIPE");
        for p in &n.pipes {
            if let (Some(a), Some(b)) = (at(&p.from), at(&p.to)) {
                w.line(&lp, [a.1, a.2, p.invert_up], [b.1, b.2, p.invert_dn]);
            }
        }
        let ls = ln("STRUCTURE");
        for s in &n.structures {
            w.circle(&ls, [s.1, s.2, s.3], 0.6);
        }
    }
    let drape = |r: &[[f64; 2]]| -> Vec<[f64; 3]> {
        let mut v: Vec<[f64; 3]> = r.iter().map(|q| [q[0], q[1], m.eg.and_then(|t| t.elevation_at(q[0], q[1])).unwrap_or(0.0)]).collect();
        if let Some(f) = v.first().copied() {
            v.push(f);
        }
        v
    };
    if let Some(ps) = m.parcels {
        let l = ln("PARCEL");
        for p in &ps.parcels {
            for r in &p.rings {
                w.poly3d(&l, &drape(r));
            }
        }
    }
    if let Some(r) = &m.row_polygon {
        w.poly3d(&ln("ROW-TAKE"), &drape(r));
    }
    let mut o = String::from(concat!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$DWGCODEPAGE\n3\nANSI_1252\n9\n$INSUNITS\n70\n6\n9\n$TEXTSTYLE\n7\nSANS\n0\nENDSEC\n",
        "0\nSECTION\n2\nTABLES\n",
        "0\nTABLE\n2\nLTYPE\n70\n1\n0\nLTYPE\n2\nCONTINUOUS\n70\n0\n3\nSolid line\n72\n65\n73\n0\n40\n0.0\n0\nENDTAB\n",
        "0\nTABLE\n2\nSTYLE\n70\n1\n0\nSTYLE\n2\nSANS\n70\n0\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n2.5\n3\ntxt\n4\n\n0\nENDTAB\n",
        "0\nTABLE\n2\nLAYER\n70\n"
    ));
    let _ = writeln!(o, "{}", w.st.layers.len() + 1);
    let _ = write!(o, "0\nLAYER\n2\n0\n70\n0\n62\n7\n6\nCONTINUOUS\n");
    for (i, l) in w.st.layers.keys().enumerate() {
        let _ = write!(o, "0\nLAYER\n2\n{l}\n70\n0\n62\n{}\n6\nCONTINUOUS\n", 1 + i % 7);
    }
    o.push_str("0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
    o.push_str(&w.s);
    o.push_str("0\nENDSEC\n0\nEOF\n");
    (o, w.st)
}
