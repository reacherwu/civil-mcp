//! Minimal drawing model with DXF (R12 ASCII) and PDF (vector) writers.
//! Written from the public DXF reference and PDF 1.4 spec; no third-party code.
use std::fmt::Write;

#[derive(Debug, Clone)]
pub enum Ent {
    Line { layer: String, a: [f64; 2], b: [f64; 2] },
    Poly { layer: String, pts: Vec<[f64; 2]> },
    Text { layer: String, p: [f64; 2], h: f64, s: String, rot: f64 },
}

#[derive(Debug, Clone, Default)]
pub struct Drawing {
    pub ents: Vec<Ent>,
    pub layers: Vec<(String, u8)>,
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
    pub fn text(&mut self, l: &str, p: [f64; 2], h: f64, s: &str) {
        self.ents.push(Ent::Text { layer: l.into(), p, h, s: s.into(), rot: 0.0 })
    }
    pub fn rect(&mut self, l: &str, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.poly(l, vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]])
    }

    pub fn to_dxf(&self) -> String {
        let mut o = String::new();
        let mut g = |c: i32, v: &str| {
            let _ = writeln!(o, "{c}\n{v}");
        };
        g(0, "SECTION");
        g(2, "HEADER");
        g(9, "$ACADVER");
        g(1, "AC1009");
        g(9, "$INSUNITS");
        g(70, "6");
        g(0, "ENDSEC");
        g(0, "SECTION");
        g(2, "TABLES");
        g(0, "TABLE");
        g(2, "LTYPE");
        g(70, "1");
        g(0, "LTYPE");
        g(2, "CONTINUOUS");
        g(70, "0");
        g(3, "Solid line");
        g(72, "65");
        g(73, "0");
        g(40, "0.0");
        g(0, "ENDTAB");
        g(0, "TABLE");
        g(2, "LAYER");
        g(70, &(self.layers.len() + 1).to_string());
        for (n, c) in std::iter::once(&("0".to_string(), 7u8)).chain(self.layers.iter()) {
            g(0, "LAYER");
            g(2, n);
            g(70, "0");
            g(62, &c.to_string());
            g(6, "CONTINUOUS");
        }
        g(0, "ENDTAB");
        g(0, "TABLE");
        g(2, "STYLE");
        g(70, "1");
        g(0, "STYLE");
        g(2, "STANDARD");
        g(70, "0");
        g(40, "0.0");
        g(41, "1.0");
        g(50, "0.0");
        g(71, "0");
        g(42, "2.5");
        g(3, "txt");
        g(4, "");
        g(0, "ENDTAB");
        g(0, "ENDSEC");
        g(0, "SECTION");
        g(2, "ENTITIES");
        let f = |x: f64| format!("{x:.4}");
        for e in &self.ents {
            match e {
                Ent::Line { layer, a, b } => {
                    g(0, "LINE");
                    g(8, layer);
                    g(10, &f(a[0]));
                    g(20, &f(a[1]));
                    g(30, "0.0");
                    g(11, &f(b[0]));
                    g(21, &f(b[1]));
                    g(31, "0.0");
                }
                Ent::Poly { layer, pts } => {
                    g(0, "POLYLINE");
                    g(8, layer);
                    g(66, "1");
                    g(10, "0.0");
                    g(20, "0.0");
                    g(30, "0.0");
                    g(70, "0");
                    for p in pts {
                        g(0, "VERTEX");
                        g(8, layer);
                        g(10, &f(p[0]));
                        g(20, &f(p[1]));
                        g(30, "0.0");
                    }
                    g(0, "SEQEND");
                    g(8, layer);
                }
                Ent::Text { layer, p, h, s, rot } => {
                    g(0, "TEXT");
                    g(8, layer);
                    g(10, &f(p[0]));
                    g(20, &f(p[1]));
                    g(30, "0.0");
                    g(40, &f(*h));
                    g(1, &ascii(s));
                    if *rot != 0.0 {
                        g(50, &f(*rot));
                    }
                }
            }
        }
        g(0, "ENDSEC");
        g(0, "EOF");
        o
    }

    /// PDF of a drawing whose coordinates are in millimetres on a `w`×`h` mm sheet.
    pub fn to_pdf(&self, w_mm: f64, h_mm: f64) -> Vec<u8> {
        let k = 72.0 / 25.4;
        let mut c = String::new();
        let _ = writeln!(c, "0.25 w 1 J 1 j");
        let col = |l: &str| -> &'static str {
            match l {
                "ALIGNMENT" | "PROFILE-DESIGN" => "0.8 0 0 RG",
                "CONTOUR-MAJOR" | "CONTOUR-MINOR" | "PROFILE-GROUND" => "0.45 0.3 0.15 RG",
                "DAYLIGHT" => "0 0.5 0 RG",
                "GRID" => "0.75 0.75 0.75 RG",
                _ => "0 0 0 RG",
            }
        };
        for e in &self.ents {
            match e {
                Ent::Line { layer, a, b } => {
                    let _ = writeln!(c, "{} {:.2} {:.2} m {:.2} {:.2} l S", col(layer), a[0] * k, a[1] * k, b[0] * k, b[1] * k);
                }
                Ent::Poly { layer, pts } => {
                    let _ = write!(c, "{} {:.2} {:.2} m", col(layer), pts[0][0] * k, pts[0][1] * k);
                    for p in &pts[1..] {
                        let _ = write!(c, " {:.2} {:.2} l", p[0] * k, p[1] * k);
                    }
                    let _ = writeln!(c, " S");
                }
                Ent::Text { p, h, s, .. } => {
                    let esc = ascii(s).replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
                    let _ = writeln!(c, "BT /F1 {:.2} Tf {:.2} {:.2} Td ({}) Tj ET", h * k, p[0] * k, p[1] * k, esc);
                }
            }
        }
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>", w_mm * k, h_mm * k),
            format!("<< /Length {} >>\nstream\n{}endstream", c.len(), c),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        ];
        let mut out = String::from("%PDF-1.4\n");
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            let _ = write!(out, "{} 0 obj\n{}\nendobj\n", i + 1, o);
        }
        let xref = out.len();
        let _ = write!(out, "xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
        for o in offs {
            let _ = write!(out, "{o:010} 00000 n \n");
        }
        let _ = write!(out, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, xref);
        out.into_bytes()
    }
}

fn ascii(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii() && !c.is_control() { c } else { '?' }).collect()
}
