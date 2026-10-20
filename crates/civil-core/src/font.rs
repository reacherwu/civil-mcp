//! Bundled Latin font (DejaVu Sans, Bitstream Vera / DejaVu free licence, see assets/DejaVuSans-LICENSE.txt) and a
//! minimal TrueType glyph subsetter for PDF embedding.
use std::collections::BTreeSet;

pub const FONT: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");
pub const FONT_FILE_NAME: &str = "DejaVuSans.ttf";

pub struct Font {
    face: ttf_parser::Face<'static>,
}

impl Font {
    pub fn load() -> Font {
        Font { face: ttf_parser::Face::parse(FONT, 0).expect("bundled font parses") }
    }
    pub fn units_per_em(&self) -> f64 {
        self.face.units_per_em() as f64
    }
    pub fn gid(&self, c: char) -> u16 {
        self.face.glyph_index(c).map_or(0, |g| g.0)
    }
    pub fn advance(&self, gid: u16) -> f64 {
        self.face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or(0) as f64
    }
    pub fn has(&self, c: char) -> bool {
        self.face.glyph_index(c).is_some()
    }
}

fn be16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}
fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Return a TrueType font keeping glyph ids but with outlines only for `used`
/// (plus composite components and .notdef). Other tables are copied.
pub fn subset(used: &BTreeSet<u16>) -> Vec<u8> {
    let f = FONT;
    let num = be16(f, 4) as usize;
    let mut tables = Vec::new();
    for i in 0..num {
        let r = 12 + 16 * i;
        let tag = [f[r], f[r + 1], f[r + 2], f[r + 3]];
        tables.push((tag, be32(f, r + 8) as usize, be32(f, r + 12) as usize));
    }
    let get = |t: &[u8; 4]| tables.iter().find(|x| &x.0 == t).map(|x| &f[x.1..x.1 + x.2]);
    let head = get(b"head").unwrap();
    let long = be16(head, 50) == 1;
    let maxp = get(b"maxp").unwrap();
    let ng = be16(maxp, 4) as usize;
    let loca = get(b"loca").unwrap();
    let glyf = get(b"glyf").unwrap();
    let off = |g: usize| if long { be32(loca, 4 * g) as usize } else { be16(loca, 2 * g) as usize * 2 };
    let mut keep: BTreeSet<u16> = used.clone();
    keep.insert(0);
    let mut stack: Vec<u16> = keep.iter().copied().collect();
    while let Some(g) = stack.pop() {
        let (a, b) = (off(g as usize), off(g as usize + 1));
        if b > a && (be16(glyf, a) as i16) < 0 {
            let mut p = a + 10;
            loop {
                let flags = be16(glyf, p);
                let comp = be16(glyf, p + 2);
                if keep.insert(comp) {
                    stack.push(comp);
                }
                p += 4 + if flags & 1 != 0 { 4 } else { 2 };
                p += if flags & 8 != 0 { 2 } else if flags & 0x40 != 0 { 4 } else if flags & 0x80 != 0 { 8 } else { 0 };
                if flags & 0x20 == 0 {
                    break;
                }
            }
        }
    }
    let mut nglyf = Vec::new();
    let mut nloca = Vec::with_capacity(4 * (ng + 1));
    for g in 0..ng {
        nloca.extend_from_slice(&(nglyf.len() as u32).to_be_bytes());
        if keep.contains(&(g as u16)) {
            nglyf.extend_from_slice(&glyf[off(g)..off(g + 1)]);
            while nglyf.len() % 4 != 0 {
                nglyf.push(0);
            }
        }
    }
    nloca.extend_from_slice(&(nglyf.len() as u32).to_be_bytes());
    let mut nhead = head.to_vec();
    nhead[50] = 0;
    nhead[51] = 1;
    nhead[8..12].copy_from_slice(&[0, 0, 0, 0]);
    let mut out_tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    for (tag, o, l) in &tables {
        let data = match tag {
            b"glyf" => nglyf.clone(),
            b"loca" => nloca.clone(),
            b"head" => nhead.clone(),
            _ => f[*o..*o + *l].to_vec(),
        };
        out_tables.push((*tag, data));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&f[0..12]);
    let mut pos = 12 + 16 * num;
    let mut body = Vec::new();
    for (tag, d) in &out_tables {
        let sum = d.chunks(4).fold(0u32, |s, c| {
            let mut w = [0u8; 4];
            w[..c.len()].copy_from_slice(c);
            s.wrapping_add(u32::from_be_bytes(w))
        });
        out.extend_from_slice(tag);
        out.extend_from_slice(&sum.to_be_bytes());
        out.extend_from_slice(&(pos as u32).to_be_bytes());
        out.extend_from_slice(&(d.len() as u32).to_be_bytes());
        body.extend_from_slice(d);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        pos = 12 + 16 * num + body.len();
    }
    out.extend_from_slice(&body);
    out
}
