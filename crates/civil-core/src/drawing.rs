//! Drawing model with DXF (R12 ASCII) and PDF (vector, multi-page) writers.
//! Text uses the bundled DejaVu Sans font: embedded as a subset TrueType
//! (CIDFontType2, Identity-H, ToUnicode) in PDF; in DXF a text style "SANS"
//! referencing the font file, with non-ASCII characters as \U+XXXX escapes.
use crate::font::{self, Font};
use std::collections::BTreeSet;
use std::fmt::Write;

#[derive(Debug, Clone)]
pub enum Ent {
    Line { layer: String, a: [f64; 2], b: [f64; 2] },
    Poly { layer: String, pts: Vec<[f64; 2]> },
    Fill { layer: String, pts: Vec<[f64; 2]>, rgb: [f64; 3] },
    Text { layer: String, p: [f64; 2], h: f64, s: String },
}

/// Sheet identity recorded by the sheet generators; the frame (title block) is drawn
/// later by `frame::compose` from this record.
#[derive(Debug, Clone, Default)]
pub struct SheetMeta {
    pub title: String,
    pub scale: String,
    pub page: usize,
    pub pages: usize,
}

/// Output style of one layer: exported name, line weight (mm) and line type.
#[derive(Debug, Clone)]
pub struct LayerStyle {
    pub name: String,
    pub width_mm: f64,
    /// "CONTINUOUS" | "DASHED" | "DASHDOT" | "DOT"
    pub ltype: String,
}

pub fn dash_pattern(lt: &str) -> &'static [f64] {
    match lt {
        "DASHED" => &[6.0, 3.0],
        "DASHDOT" => &[12.0, 3.0, 1.0, 3.0],
        "DOT" => &[1.0, 2.0],
        _ => &[],
    }
}

#[derive(Debug, Clone, Default)]
pub struct Drawing {
    pub ents: Vec<Ent>,
    pub layers: Vec<(String, u8)>,
    /// sheet drawings are in millimetres ($INSUNITS 4); world models in metres (6)
    pub units_mm: bool,
    pub meta: Option<SheetMeta>,
    /// per internal layer name; empty = legacy output (internal names, 0.25 mm, continuous)
    pub styles: std::collections::BTreeMap<String, LayerStyle>,
}

pub fn layer_rgb(l: &str) -> [f64; 3] {
    match l {
        "ALIGNMENT" | "PROFILE-DESIGN" | "XS-DESIGN" | "STAMP" => [0.8, 0.0, 0.0],
        "CONTOUR-MAJOR" | "CONTOUR-MINOR" | "PROFILE-GROUND" | "XS-GROUND" => [0.45, 0.3, 0.15],
        "DAYLIGHT" | "XS-DATUM" => [0.0, 0.5, 0.0],
        "PIPE" | "STRUCTURE" => [0.0, 0.3, 0.8],
        "GRADING" => [0.5, 0.0, 0.6],
        "GRID" => [0.78, 0.78, 0.78],
        "OSM-ROAD-MAJOR" => [0.85, 0.55, 0.2],
        "OSM-ROAD" => [0.6, 0.6, 0.6],
        "OSM-ROAD-MINOR" => [0.82, 0.82, 0.82],
        "OSM-WATER" => [0.35, 0.6, 0.9],
        "OSM-PLACE" => [0.3, 0.3, 0.3],
        "LANDSCAPE" => [0.1, 0.55, 0.15],
        "SIGNAL" => [0.85, 0.1, 0.1],
        _ => [0.0, 0.0, 0.0],
    }
}

impl Drawing {
    pub fn layer(&mut self, name: &str, color: u8) {
        if !self.layers.iter().any(|l| l.0 == name) {
            self.layers.push((name.into(), color));
        }
    }
    pub fn line(&mut self, l: &str, a: [f64; 2], b: [f64; 2]) {
        self.ents.push(Ent::Line { layer: l.into(), a, b })
    }
    pub fn poly(&mut self, l: &str, pts: Vec<[f64; 2]>) {
        if pts.len() >= 2 {
            self.ents.push(Ent::Poly { layer: l.into(), pts })
        }
    }
    pub fn fill(&mut self, l: &str, pts: Vec<[f64; 2]>, rgb: [f64; 3]) {
        if pts.len() >= 3 {
            self.ents.push(Ent::Fill { layer: l.into(), pts, rgb })
        }
    }
    pub fn text(&mut self, l: &str, p: [f64; 2], h: f64, s: &str) {
        self.ents.push(Ent::Text { layer: l.into(), p, h, s: crate::units::text(s) })
    }
    pub fn rect(&mut self, l: &str, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.poly(l, vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]])
    }

    pub fn to_dxf(&self) -> String {
        let mut o = String::new();
        let mut g = |c: i32, v: &str| {
            let _ = writeln!(o, "{c}\n{v}");
        };
        for (c, v) in [
            (0, "SECTION"),
            (2, "HEADER"),
            (9, "$ACADVER"),
            (1, "AC1009"),
            (9, "$DWGCODEPAGE"),
            (3, "ANSI_1252"),
            (9, "$INSUNITS"),
            (70, if self.units_mm { "4" } else { "6" }),
            (9, "$TEXTSTYLE"),
            (7, "SANS"),
            (0, "ENDSEC"),
        ] {
            g(c, v);
        }
        for (c, v) in [(0, "SECTION"), (2, "TABLES"), (0, "TABLE"), (2, "LTYPE"), (70, "4")] {
            g(c, v);
        }
        for (n, desc) in [("CONTINUOUS", "Solid line"), ("DASHED", "Dashed __ __ __"), ("DASHDOT", "Dash dot __ . __ ."), ("DOT", "Dotted . . . .")] {
            let pat = dash_pattern(n);
            g(0, "LTYPE");
            g(2, n);
            g(70, "0");
            g(3, desc);
            g(72, "65");
            g(73, &pat.len().to_string());
            g(40, &format!("{:.1}", pat.iter().sum::<f64>()));
            for (i, d) in pat.iter().enumerate() {
                g(49, &format!("{:.1}", if i % 2 == 0 { *d } else { -*d }));
            }
        }
        g(0, "ENDTAB");
        g(0, "TABLE");
        g(2, "LAYER");
        let mut seen = std::collections::BTreeSet::new();
        let mut rows = vec![];
        for (n, c) in std::iter::once(&("0".to_string(), 7u8)).chain(self.layers.iter()) {
            let (on, lt) = self.styles.get(n).map(|st| (st.name.clone(), st.ltype.clone())).unwrap_or((n.clone(), "CONTINUOUS".into()));
            if seen.insert(on.clone()) {
                rows.push((on, *c, lt));
            }
        }
        g(70, &rows.len().to_string());
        for (n, c, lt) in rows {
            g(0, "LAYER");
            g(2, &n);
            g(70, "0");
            g(62, &c.to_string());
            g(6, &lt);
        }
        g(0, "ENDTAB");
        g(0, "TABLE");
        g(2, "STYLE");
        g(70, "2");
        for (name, file) in [("STANDARD", "txt"), ("SANS", font::FONT_FILE_NAME)] {
            g(0, "STYLE");
            g(2, name);
            g(70, "0");
            g(40, "0.0");
            g(41, "1.0");
            g(50, "0.0");
            g(71, "0");
            g(42, "2.5");
            g(3, file);
            g(4, "");
        }
        g(0, "ENDTAB");
        g(0, "ENDSEC");
        g(0, "SECTION");
        g(2, "ENTITIES");
        let f = |x: f64| format!("{x:.4}");
        let ln = |l: &String| -> String { self.styles.get(l).map(|s| s.name.clone()).unwrap_or_else(|| l.clone()) };
        for e in &self.ents {
            match e {
                Ent::Line { layer, a, b } => {
                    g(0, "LINE");
                    g(8, &ln(layer));
                    g(10, &f(a[0]));
                    g(20, &f(a[1]));
                    g(30, "0.0");
                    g(11, &f(b[0]));
                    g(21, &f(b[1]));
                    g(31, "0.0");
                }
                Ent::Poly { layer, pts } | Ent::Fill { layer, pts, .. } => {
                    g(0, "POLYLINE");
                    g(8, &ln(layer));
                    g(66, "1");
                    g(10, "0.0");
                    g(20, "0.0");
                    g(30, "0.0");
                    g(70, if matches!(e, Ent::Fill { .. }) { "1" } else { "0" });
                    for p in pts {
                        g(0, "VERTEX");
                        g(8, &ln(layer));
                        g(10, &f(p[0]));
                        g(20, &f(p[1]));
                        g(30, "0.0");
                    }
                    g(0, "SEQEND");
                    g(8, &ln(layer));
                }
                Ent::Text { layer, p, h, s } => {
                    g(0, "TEXT");
                    g(8, &ln(layer));
                    g(10, &f(p[0]));
                    g(20, &f(p[1]));
                    g(30, "0.0");
                    g(40, &f(*h));
                    g(1, &dxf_text(s));
                    g(7, "SANS");
                }
            }
        }
        g(0, "ENDSEC");
        g(0, "EOF");
        o
    }
}

impl Drawing {
    /// Standalone SVG of a sheet (w×h mm), y up in the model → y down in SVG.
    pub fn to_svg(&self, w: f64, h: f64) -> String {
        let mut o = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"white\"/>\n"
        );
        let rgb = |r: [f64; 3]| format!("rgb({},{},{})", (r[0] * 255.0) as u8, (r[1] * 255.0) as u8, (r[2] * 255.0) as u8);
        let pts = |p: &[[f64; 2]]| p.iter().map(|q| format!("{:.2},{:.2}", q[0], h - q[1])).collect::<Vec<_>>().join(" ");
        // one <g> per layer keeps the exported layer names visible to SVG editors
        let mut by: Vec<(String, Vec<&Ent>)> = vec![];
        for e in &self.ents {
            let l = match e {
                Ent::Line { layer, .. } | Ent::Poly { layer, .. } | Ent::Fill { layer, .. } | Ent::Text { layer, .. } => layer,
            };
            match by.iter_mut().find(|x| &x.0 == l) {
                Some(x) => x.1.push(e),
                None => by.push((l.clone(), vec![e])),
            }
        }
        for (l, es) in by {
            let st = self.styles.get(&l);
            let name = st.map(|s| s.name.clone()).unwrap_or(l.clone());
            let wmm = st.map(|s| s.width_mm).unwrap_or(0.25);
            let dash = st.map(|s| dash_pattern(&s.ltype).iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")).unwrap_or_default();
            let c = rgb(layer_rgb(&l));
            let _ = write!(
                o,
                "<g id=\"{}\" stroke=\"{c}\" stroke-width=\"{wmm}\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
                xml_esc(&name)
            );
            if !dash.is_empty() {
                let _ = write!(o, " stroke-dasharray=\"{dash}\"");
            }
            o += ">\n";
            for e in es {
                match e {
                    Ent::Line { a, b, .. } => {
                        let _ = writeln!(o, "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\"/>", a[0], h - a[1], b[0], h - b[1]);
                    }
                    Ent::Poly { pts: p, .. } => {
                        let _ = writeln!(o, "<polyline points=\"{}\"/>", pts(p));
                    }
                    Ent::Fill { pts: p, rgb: r, .. } => {
                        let _ = writeln!(o, "<polygon points=\"{}\" fill=\"{}\" stroke=\"none\"/>", pts(p), rgb(*r));
                    }
                    Ent::Text { p, h: th, s, .. } => {
                        let _ = writeln!(
                            o,
                            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"DejaVu Sans, Arial, sans-serif\" font-size=\"{:.3}\" fill=\"{c}\" stroke=\"none\">{}</text>",
                            p[0],
                            h - p[1],
                            th / 0.729,
                            xml_esc(s)
                        );
                    }
                }
            }
            o += "</g>\n";
        }
        o + "</svg>\n"
    }
}

pub fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// ASCII as-is, everything else as \U+XXXX (AutoCAD/ezdxf convention).
pub fn dxf_text(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if c.is_ascii() && !c.is_control() {
            o.push(c)
        } else if (c as u32) <= 0xFFFF {
            let _ = write!(o, "\\U+{:04X}", c as u32);
        } else {
            o.push('?')
        }
    }
    o
}

fn deflate(b: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(b, 6)
}

/// Multi-page PDF. Each page is a drawing in millimetres on a w×h sheet.
pub fn to_pdf(pages: &[(&Drawing, f64, f64)]) -> Vec<u8> {
    let font = Font::load();
    let k = 72.0 / 25.4;
    let upm = font.units_per_em();
    let mut used: BTreeSet<u16> = BTreeSet::new();
    let mut uni: std::collections::BTreeMap<u16, char> = Default::default();
    let mut contents = Vec::new();
    for (d, _, _) in pages {
        let mut c = String::new();
        let _ = writeln!(c, "0.25 w 1 J 1 j");
        let mut cur = (0.25f64, String::from("CONTINUOUS"));
        for e in &d.ents {
            let layer = match e {
                Ent::Line { layer, .. } | Ent::Poly { layer, .. } | Ent::Fill { layer, .. } | Ent::Text { layer, .. } => layer,
            };
            if let Some(st) = d.styles.get(layer) {
                if (st.width_mm - cur.0).abs() > 1e-9 || st.ltype != cur.1 {
                    let pat: Vec<String> = dash_pattern(&st.ltype).iter().map(|v| format!("{:.2}", v * k)).collect();
                    let _ = writeln!(c, "{:.3} w [{}] 0 d", st.width_mm * k, pat.join(" "));
                    cur = (st.width_mm, st.ltype.clone());
                }
            }
            match e {
                Ent::Line { layer, a, b } => {
                    let r = layer_rgb(layer);
                    let _ = writeln!(c, "{} {} {} RG {:.2} {:.2} m {:.2} {:.2} l S", r[0], r[1], r[2], a[0] * k, a[1] * k, b[0] * k, b[1] * k);
                }
                Ent::Poly { layer, pts } => {
                    let r = layer_rgb(layer);
                    let _ = write!(c, "{} {} {} RG {:.2} {:.2} m", r[0], r[1], r[2], pts[0][0] * k, pts[0][1] * k);
                    for p in &pts[1..] {
                        let _ = write!(c, " {:.2} {:.2} l", p[0] * k, p[1] * k);
                    }
                    let _ = writeln!(c, " S");
                }
                Ent::Fill { pts, rgb, .. } => {
                    let _ = write!(c, "{} {} {} rg {:.2} {:.2} m", rgb[0], rgb[1], rgb[2], pts[0][0] * k, pts[0][1] * k);
                    for p in &pts[1..] {
                        let _ = write!(c, " {:.2} {:.2} l", p[0] * k, p[1] * k);
                    }
                    let _ = writeln!(c, " h f");
                }
                Ent::Text { layer, p, h, s } => {
                    let r = layer_rgb(layer);
                    let mut hex = String::new();
                    for ch in s.chars() {
                        let gid = font.gid(ch);
                        used.insert(gid);
                        uni.entry(gid).or_insert(ch);
                        let _ = write!(hex, "{gid:04X}");
                    }
                    // h is the cap height in mm; DejaVu Sans cap height ≈ 0.729 em
                    let _ = writeln!(c, "{} {} {} rg BT /F1 {:.2} Tf {:.2} {:.2} Td <{}> Tj ET", r[0], r[1], r[2], h / 0.729 * k, p[0] * k, p[1] * k, hex);
                }
            }
        }
        contents.push(c);
    }
    // objects: 1 catalog, 2 pages, 3 type0 font, 4 cidfont, 5 descriptor, 6 fontfile, 7 tounicode, then page/content pairs
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let n = pages.len();
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 8 + 2 * i)).collect();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), n).into_bytes());
    objs.push(b"<< /Type /Font /Subtype /Type0 /BaseFont /AAAAAA+DejaVuSans /Encoding /Identity-H /DescendantFonts [4 0 R] /ToUnicode 7 0 R >>".to_vec());
    let mut w = String::new();
    for g in &used {
        let _ = write!(w, "{} [{:.0}] ", g, font.advance(*g) * 1000.0 / upm);
    }
    objs.push(format!("<< /Type /Font /Subtype /CIDFontType2 /BaseFont /AAAAAA+DejaVuSans /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor 5 0 R /CIDToGIDMap /Identity /DW 1000 /W [{w}] >>").into_bytes());
    objs.push(b"<< /Type /FontDescriptor /FontName /AAAAAA+DejaVuSans /Flags 4 /FontBBox [-1000 -300 2000 1100] /ItalicAngle 0 /Ascent 928 /Descent -236 /CapHeight 729 /StemV 80 /FontFile2 6 0 R >>".to_vec());
    let sub = deflate(&font::subset(&used));
    let mut ff = format!("<< /Length {} /Filter /FlateDecode >>\nstream\n", sub.len()).into_bytes();
    ff.extend_from_slice(&sub);
    ff.extend_from_slice(b"\nendstream");
    objs.push(ff);
    let mut cm = String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Adobe-Identity-UCS def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
    let pairs: Vec<(&u16, &char)> = uni.iter().collect();
    for chunk in pairs.chunks(100) {
        let _ = writeln!(cm, "{} beginbfchar", chunk.len());
        for (g, c) in chunk {
            let mut b = [0u16; 2];
            let enc: String = c.encode_utf16(&mut b).iter().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(cm, "<{:04X}> <{}>", g, enc);
        }
        let _ = writeln!(cm, "endbfchar");
    }
    cm += "endcmap CMapName currentdict /CMap defineresource pop end end";
    objs.push(format!("<< /Length {} >>\nstream\n{}\nendstream", cm.len(), cm).into_bytes());
    for (i, ((_, wmm, hmm), c)) in pages.iter().zip(contents).enumerate() {
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>",
                wmm * k,
                hmm * k,
                9 + 2 * i
            )
            .into_bytes(),
        );
        let z = deflate(c.as_bytes());
        let mut s = format!("<< /Length {} /Filter /FlateDecode >>\nstream\n", z.len()).into_bytes();
        s.extend_from_slice(&z);
        s.extend_from_slice(b"\nendstream");
        objs.push(s);
    }
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let mut t = format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
    for o in offs {
        let _ = writeln!(t, "{o:010} 00000 n ");
    }
    let _ = write!(t, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, xref);
    out.extend_from_slice(t.as_bytes());
    out
}
