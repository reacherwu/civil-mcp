//! Scene export for visualisation (Blender and other glTF viewers): binary glTF 2.0 (.glb)
//! with existing ground, road pavement, earthwork slopes, walls, pipes and structures, and
//! route alternatives as draped ribbons. Coordinates are local metres about a georeference
//! origin (E0, N0, Z0) written to the scene JSON and to the glTF `extras`; glTF axes are
//! x = east, y = up, z = −north.
use crate::corridor::Corridor;
use crate::pipes::Network;
use crate::tin::Tin;
use serde_json::{json, Value};

pub struct Mesh {
    pub name: String,
    pub material: usize,
    pub pos: Vec<[f64; 3]>,
    pub idx: Vec<u32>,
}

pub const MATERIALS: &[(&str, [f32; 4], f32)] = &[
    ("Terrain", [0.36, 0.42, 0.25, 1.0], 0.95),
    ("Asphalt", [0.08, 0.08, 0.09, 1.0], 0.8),
    ("Earthwork", [0.55, 0.45, 0.32, 1.0], 0.95),
    ("Concrete", [0.7, 0.7, 0.68, 1.0], 0.7),
    ("Pipe", [0.2, 0.35, 0.8, 1.0], 0.5),
    ("Structure", [0.45, 0.45, 0.5, 1.0], 0.6),
    ("AltA", [0.9, 0.2, 0.2, 1.0], 0.6),
    ("AltB", [0.2, 0.6, 0.95, 1.0], 0.6),
    ("AltC", [0.95, 0.75, 0.1, 1.0], 0.6),
    ("AltD", [0.8, 0.3, 0.9, 1.0], 0.6),
];

pub struct Scene {
    pub origin: [f64; 3],
    pub meshes: Vec<Mesh>,
}

impl Scene {
    fn add(&mut self, name: &str, material: usize, pos: Vec<[f64; 3]>, idx: Vec<u32>) {
        if !idx.is_empty() {
            self.meshes.push(Mesh { name: name.into(), material, pos, idx });
        }
    }
}

/// Quad strip between two polylines of equal length.
fn strip(a: &[[f64; 3]], b: &[[f64; 3]]) -> (Vec<[f64; 3]>, Vec<u32>) {
    let n = a.len().min(b.len());
    let mut pos = Vec::with_capacity(2 * n);
    let mut idx = vec![];
    for i in 0..n {
        pos.push(a[i]);
        pos.push(b[i]);
        if i + 1 < n {
            let k = (2 * i) as u32;
            idx.extend([k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    (pos, idx)
}

fn merge(dst: &mut (Vec<[f64; 3]>, Vec<u32>), src: (Vec<[f64; 3]>, Vec<u32>)) {
    let o = dst.0.len() as u32;
    dst.0.extend(src.0);
    dst.1.extend(src.1.iter().map(|i| i + o));
}

/// Builds the scene. `alts` are (name, plan polyline) route alternatives.
pub fn build(eg: &Tin, cor: Option<&Corridor>, nets: &[Network], alts: &[(String, Vec<[f64; 2]>)]) -> Scene {
    let (mut x0, mut y0, mut z0) = (0.0, 0.0, f64::MAX);
    for p in &eg.pts {
        x0 += p[0];
        y0 += p[1];
        z0 = z0.min(p[2]);
    }
    let n = eg.pts.len().max(1) as f64;
    let origin = [(x0 / n).round(), (y0 / n).round(), z0.floor()];
    let mut sc = Scene { origin, meshes: vec![] };
    sc.add("Existing ground", 0, eg.pts.clone(), eg.tris.iter().flat_map(|t| [t[0] as u32, t[1] as u32, t[2] as u32]).collect());
    if let Some(c) = cor {
        let at = |s: &crate::corridor::Section, left: bool, o: f64, z: f64| {
            let q = crate::cad3d::sec_xy(s.x, s.y, s.heading, o, left);
            [q[0], q[1], z]
        };
        let mut pav = (vec![], vec![]);
        let mut earth = (vec![], vec![]);
        for left in [true, false] {
            fn pick(s: &crate::corridor::Section, left: bool) -> &crate::corridor::SideSection {
                if left {
                    &s.left
                } else {
                    &s.right
                }
            }
            let side = |s| pick(s, left);
            // pavement: centreline to the outermost paved fg vertex
            let cl: Vec<[f64; 3]> = c.sections.iter().map(|s| [s.x, s.y, s.cl_elev]).collect();
            let edge: Vec<[f64; 3]> = c
                .sections
                .iter()
                .map(|s| {
                    let sd = side(s);
                    let k = sd.fg.iter().position(|q| q[0] >= sd.paved_width - 1e-6).unwrap_or(sd.fg.len() - 1);
                    at(s, left, sd.fg[k][0], sd.fg[k][1])
                })
                .collect();
            merge(&mut pav, strip(&cl, &edge));
            // remaining fg vertices to the daylight point, one strip per vertex rank (resampled)
            const RANKS: usize = 6;
            let rows: Vec<Vec<[f64; 3]>> = (0..=RANKS)
                .map(|r| {
                    c.sections
                        .iter()
                        .map(|s| {
                            let sd = side(s);
                            let k0 = sd.fg.iter().position(|q| q[0] >= sd.paved_width - 1e-6).unwrap_or(sd.fg.len() - 1);
                            let k = k0 + (sd.fg.len() - 1 - k0) * r / RANKS;
                            at(s, left, sd.fg[k][0], sd.fg[k][1])
                        })
                        .collect()
                })
                .collect();
            for w in rows.windows(2) {
                merge(&mut earth, strip(&w[0], &w[1]));
            }
        }
        sc.add("Road pavement", 1, pav.0, pav.1);
        sc.add("Earthwork slopes", 2, earth.0, earth.1);
        // walls: consecutive sections with a wall on the same side
        let mut walls = (vec![], vec![]);
        for left in [true, false] {
            let mut run_top = vec![];
            let mut run_bot = vec![];
            for s in &c.sections {
                let w = if left { &s.left.wall } else { &s.right.wall };
                match w {
                    Some(w) => {
                        run_top.push(at(s, left, w.offset, w.top_z));
                        run_bot.push(at(s, left, w.offset, w.bottom_z));
                    }
                    None => {
                        if run_top.len() >= 2 {
                            merge(&mut walls, strip(&run_bot, &run_top));
                        }
                        run_top.clear();
                        run_bot.clear();
                    }
                }
            }
            if run_top.len() >= 2 {
                merge(&mut walls, strip(&run_bot, &run_top));
            }
        }
        sc.add("Retaining walls", 3, walls.0, walls.1);
    }
    // drainage: 8-sided tubes and structure cylinders
    let tube = |a: [f64; 3], b: [f64; 3], r: f64| -> (Vec<[f64; 3]>, Vec<u32>) {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
        let t = [d[0] / l, d[1] / l, d[2] / l];
        let up = if t[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
        let u = [t[1] * up[2] - t[2] * up[1], t[2] * up[0] - t[0] * up[2], t[0] * up[1] - t[1] * up[0]];
        let ul = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        let u = [u[0] / ul, u[1] / ul, u[2] / ul];
        let v = [t[1] * u[2] - t[2] * u[1], t[2] * u[0] - t[0] * u[2], t[0] * u[1] - t[1] * u[0]];
        let mut pos = vec![];
        let mut idx = vec![];
        for (e, c) in [a, b].iter().enumerate() {
            let _ = e;
            for k in 0..8 {
                let ang = k as f64 * std::f64::consts::TAU / 8.0;
                let (cs, sn) = (ang.cos() * r, ang.sin() * r);
                pos.push([c[0] + u[0] * cs + v[0] * sn, c[1] + u[1] * cs + v[1] * sn, c[2] + u[2] * cs + v[2] * sn]);
            }
        }
        for k in 0..8u32 {
            let k1 = (k + 1) % 8;
            idx.extend([k, k1, 8 + k, k1, 8 + k1, 8 + k]);
        }
        (pos, idx)
    };
    let mut pipes = (vec![], vec![]);
    let mut structs = (vec![], vec![]);
    for n in nets {
        let st = |id: &str| n.structures.iter().find(|s| s.0 == id);
        for p in &n.pipes {
            if let (Some(a), Some(b)) = (st(&p.from), st(&p.to)) {
                let r = p.diameter_m / 2.0;
                merge(&mut pipes, tube([a.1, a.2, p.invert_up + r], [b.1, b.2, p.invert_dn + r], r));
            }
        }
        for s in &n.structures {
            merge(&mut structs, tube([s.1, s.2, s.4], [s.1, s.2, s.3], 0.6));
        }
    }
    sc.add("Pipes", 4, pipes.0, pipes.1);
    sc.add("Structures", 5, structs.0, structs.1);
    // route alternatives: 4 m ribbons draped 0.3 m above ground
    for (i, (name, path)) in alts.iter().enumerate() {
        let mut l = vec![];
        let mut r = vec![];
        let dense: Vec<[f64; 2]> = path
            .windows(2)
            .flat_map(|w| {
                let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
                let n = (d / 5.0).ceil().max(1.0) as usize;
                (0..n).map(move |k| {
                    let t = k as f64 / n as f64;
                    [w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1])]
                })
            })
            .chain(path.last().copied())
            .collect();
        for (k, p) in dense.iter().enumerate() {
            let q = dense[(k + 1).min(dense.len() - 1)];
            let pr = dense[k.saturating_sub(1)];
            let h = (q[1] - pr[1]).atan2(q[0] - pr[0]);
            let (nx, ny) = (-h.sin() * 2.0, h.cos() * 2.0);
            let z = |x: f64, y: f64| eg.elevation_at(x, y).unwrap_or(origin[2]) + 0.3;
            l.push([p[0] + nx, p[1] + ny, z(p[0] + nx, p[1] + ny)]);
            r.push([p[0] - nx, p[1] - ny, z(p[0] - nx, p[1] - ny)]);
        }
        let (pos, idx) = strip(&l, &r);
        sc.add(&format!("Alternative {name}"), 6 + i % 4, pos, idx);
    }
    sc
}

/// Binary glTF 2.0.
pub fn to_glb(sc: &Scene, extras: Value) -> Vec<u8> {
    let o = sc.origin;
    let mut bin: Vec<u8> = vec![];
    let (mut views, mut accs, mut meshes, mut nodes) = (vec![], vec![], vec![], vec![]);
    for m in &sc.meshes {
        let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
        let off = bin.len();
        for p in &m.pos {
            let v = [(p[0] - o[0]) as f32, (p[2] - o[2]) as f32, -(p[1] - o[1]) as f32];
            for k in 0..3 {
                mn[k] = mn[k].min(v[k]);
                mx[k] = mx[k].max(v[k]);
                bin.extend_from_slice(&v[k].to_le_bytes());
            }
        }
        views.push(json!({"buffer": 0, "byteOffset": off, "byteLength": bin.len() - off, "target": 34962}));
        accs.push(json!({"bufferView": views.len() - 1, "componentType": 5126, "count": m.pos.len(), "type": "VEC3", "min": mn, "max": mx}));
        let pa = accs.len() - 1;
        let off = bin.len();
        for i in &m.idx {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        views.push(json!({"buffer": 0, "byteOffset": off, "byteLength": bin.len() - off, "target": 34963}));
        accs.push(json!({"bufferView": views.len() - 1, "componentType": 5125, "count": m.idx.len(), "type": "SCALAR"}));
        meshes.push(json!({"name": m.name, "primitives": [{"attributes": {"POSITION": pa}, "indices": accs.len() - 1, "material": m.material}]}));
        nodes.push(json!({"name": m.name, "mesh": meshes.len() - 1}));
    }
    let mats: Vec<Value> = MATERIALS
        .iter()
        .map(|(n, c, r)| json!({"name": n, "doubleSided": true, "pbrMetallicRoughness": {"baseColorFactor": c, "metallicFactor": 0.0, "roughnessFactor": r}}))
        .collect();
    let doc = json!({
        "asset": {"version": "2.0", "generator": concat!("civil-mcp ", env!("CARGO_PKG_VERSION"))},
        "scene": 0,
        "scenes": [{"name": "civil-mcp", "nodes": (0..nodes.len()).collect::<Vec<_>>(), "extras": extras}],
        "nodes": nodes, "meshes": meshes, "materials": mats, "accessors": accs, "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}]
    });
    let mut js = doc.to_string().into_bytes();
    while js.len() & 3 != 0 {
        js.push(b' ');
    }
    while bin.len() & 3 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}
