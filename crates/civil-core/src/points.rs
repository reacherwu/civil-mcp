use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurveyPoint {
    pub id: String,
    pub e: f64,
    pub n: f64,
    pub z: f64,
    pub desc: String,
}

/// Parse a PNEZD-style CSV with header `id,easting,northing,elevation,desc`.
/// Rows that fail to parse are reported, never silently dropped.
pub fn parse_csv(text: &str) -> crate::Result<(Vec<SurveyPoint>, Vec<String>)> {
    let mut pts = Vec::new();
    let mut rejected = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || (i == 0 && line.to_ascii_lowercase().starts_with("id")) {
            continue;
        }
        let f: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if f.len() < 4 {
            rejected.push(format!("line {}: expected >=4 fields", i + 1));
            continue;
        }
        let p = (f[1].parse::<f64>(), f[2].parse::<f64>(), f[3].parse::<f64>());
        match p {
            (Ok(e), Ok(n), Ok(z)) if e.is_finite() && n.is_finite() && z.is_finite() => pts.push(SurveyPoint {
                id: f[0].to_string(),
                e,
                n,
                z,
                desc: f.get(4).unwrap_or(&"").to_string(),
            }),
            _ => rejected.push(format!("line {}: non-numeric coordinate", i + 1)),
        }
    }
    if pts.len() < 3 {
        return Err(format!("need at least 3 valid points, got {}", pts.len()));
    }
    Ok((pts, rejected))
}

/// Minimal LandXML <CgPoint> reader (northing easting elevation order, as LandXML specifies).
pub fn parse_landxml_cgpoints(text: &str) -> crate::Result<Vec<SurveyPoint>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("<CgPoint") {
        rest = &rest[i..];
        let gt = rest.find('>').ok_or("unterminated CgPoint")?;
        let tag = &rest[..gt];
        if tag.ends_with('/') {
            rest = &rest[gt..];
            continue;
        }
        let end = rest.find("</CgPoint>").ok_or("missing </CgPoint>")?;
        let body = &rest[gt + 1..end];
        let name = attr(tag, "name").unwrap_or_else(|| format!("{}", out.len() + 1));
        let v: Vec<f64> = body.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        if v.len() >= 3 {
            out.push(SurveyPoint { id: name, n: v[0], e: v[1], z: v[2], desc: attr(tag, "desc").unwrap_or_default() });
        }
        rest = &rest[end..];
    }
    if out.len() < 3 {
        return Err("fewer than 3 CgPoints with elevation".into());
    }
    Ok(out)
}

fn attr(tag: &str, key: &str) -> Option<String> {
    let k = format!("{key}=\"");
    let i = tag.find(&k)? + k.len();
    let j = tag[i..].find('"')?;
    Some(tag[i..i + j].to_string())
}
