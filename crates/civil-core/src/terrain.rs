//! Terrain import: GeoTIFF / COG and ESRI ASCII grid DEMs, LAS / LAZ point clouds
//! (ground class), with coordinate reference handling.
//!
//! CRS handling:
//! - geographic (EPSG 4326, 4269, 4258, 4283, 7844, 4167) and Web Mercator (3857) inputs are
//!   projected to UTM (Krüger series, GRS80/WGS84; the datums are treated as equal, which is
//!   within about 1–2 m), zone chosen from the data centre unless `target_epsg` names one;
//! - projected inputs (UTM, MGA, state plane, national grids) are kept planar. Foot units
//!   (US survey or international, from the GeoTIFF linear-units key, the EPSG table below or an
//!   explicit `xy_units`) are converted to metres; no datum or projection change is made.
//! - vertical units: metres unless `z_units` says ft / us_ft.
use crate::units::{INTL_FT, SURVEY_FT};
use serde::Serialize;
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Serialize)]
pub struct CrsInfo {
    pub source_epsg: Option<u32>,
    pub source_kind: String,
    pub xy_unit_m: f64,
    pub z_unit_m: f64,
    pub target_epsg: Option<u32>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Import {
    pub format: String,
    pub crs: CrsInfo,
    pub source_points: usize,
    pub kept_points: usize,
    pub nodata_or_filtered: usize,
    pub bbox: [f64; 4],
    pub z_range: [f64; 2],
    pub cell_m: Option<f64>,
    #[serde(skip)]
    pub points: Vec<[f64; 3]>,
}

#[derive(Clone)]
pub struct Options {
    pub step: usize,
    /// crop box in the output CRS (metres): [xmin, ymin, xmax, ymax]
    pub crop: Option<[f64; 4]>,
    pub epsg: Option<u32>,
    pub target_epsg: Option<u32>,
    pub xy_units: Option<String>,
    pub z_units: Option<String>,
    pub classes: Vec<u8>,
    pub max_points: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options { step: 1, crop: None, epsg: None, target_epsg: None, xy_units: None, z_units: None, classes: vec![2], max_points: 400_000 }
    }
}

fn unit_m(s: &str) -> crate::Result<f64> {
    match s {
        "m" | "metre" | "meter" => Ok(1.0),
        "ft" | "intl_ft" | "international_foot" => Ok(INTL_FT),
        "us_ft" | "ftus" | "survey_ft" | "us_survey_foot" => Ok(SURVEY_FT),
        o => Err(format!("unknown unit '{o}' (m, ft, us_ft)")),
    }
}

/// Linear unit (metres per unit) of well-known projected EPSG codes that use feet.
pub fn epsg_foot(epsg: u32) -> Option<f64> {
    // NAD83 state plane zones in US survey feet (EPSG 2225-2289 and 3417-3446 ranges hold most ftUS zones),
    // NAD83(2011) ftUS 6418-6627 (subset). Only codes listed here are recognised; pass xy_units otherwise.
    const US_FT: &[(u32, u32)] = &[
        (2222, 2289),
        (2867, 2888),
        (2891, 2930),
        (2964, 2968),
        (3089, 3090),
        (3359, 3361),
        (3363, 3364),
        (3404, 3404),
        (3417, 3438),
        (3441, 3446),
        (3451, 3456),
        (3560, 3570),
        (3734, 3760),
    ];
    if US_FT.iter().any(|(a, b)| (*a..=*b).contains(&epsg)) {
        return Some(SURVEY_FT);
    }
    None
}

fn geographic(epsg: u32) -> bool {
    matches!(epsg, 4326 | 4269 | 4258 | 4283 | 7844 | 4167 | 4617 | 4979 | 6318)
}

// ---------- projections ----------

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_222_101;

/// Transverse Mercator (Krüger 4th-order series, GRS80).
pub fn tm_forward(lat: f64, lon: f64, lon0: f64, k0: f64, fe: f64, fnn: f64) -> (f64, f64) {
    let n = F / (2.0 - F);
    let a_ = A / (1.0 + n) * (1.0 + n * n / 4.0 + n.powi(4) / 64.0);
    let al = [n / 2.0 - 2.0 * n * n / 3.0 + 5.0 * n.powi(3) / 16.0, 13.0 * n * n / 48.0 - 3.0 * n.powi(3) / 5.0, 61.0 * n.powi(3) / 240.0];
    let (phi, lam) = (lat.to_radians(), (lon - lon0).to_radians());
    let e = (F * (2.0 - F)).sqrt();
    let t = (phi.sin().atanh() - e * (e * phi.sin()).atanh()).sinh();
    let xi = t.atan2(lam.cos());
    let eta = (lam.sin() / (1.0 + t * t).sqrt()).atanh();
    let mut x = eta;
    let mut y = xi;
    for (j, a) in al.iter().enumerate() {
        let k = 2.0 * (j + 1) as f64;
        x += a * (k * xi).cos() * (k * eta).sinh();
        y += a * (k * xi).sin() * (k * eta).cosh();
    }
    (fe + k0 * a_ * x, fnn + k0 * a_ * y)
}

pub fn utm_zone(lon: f64) -> u32 {
    (((lon + 180.0) / 6.0).floor() as u32 % 60) + 1
}

/// (lat, lon) → UTM easting/northing in `zone` (north or south by sign of lat or the EPSG).
pub fn to_utm(lat: f64, lon: f64, zone: u32, south: bool) -> (f64, f64) {
    let lon0 = zone as f64 * 6.0 - 183.0;
    tm_forward(lat, lon, lon0, 0.9996, 500_000.0, if south { 10_000_000.0 } else { 0.0 })
}

pub fn webmerc_inverse(x: f64, y: f64) -> (f64, f64) {
    let lon = (x / A).to_degrees();
    let lat = (2.0 * (y / A).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
    (lat, lon)
}

/// UTM EPSG for a zone (WGS84 326zz / 327zz).
pub fn utm_epsg(zone: u32, south: bool) -> u32 {
    if south {
        32700 + zone
    } else {
        32600 + zone
    }
}

fn parse_utm_epsg(e: u32) -> Option<(u32, bool)> {
    match e {
        32601..=32660 => Some((e - 32600, false)),
        32701..=32760 => Some((e - 32700, true)),
        26901..=26923 => Some((e - 26900, false)),
        25828..=25838 => Some((e - 25800, false)),
        28348..=28358 => Some((e - 28300, true)),
        7846..=7859 => Some((e - 7800, true)),
        _ => None,
    }
}

struct Xform {
    kind: u8, // 0 planar, 1 geographic→UTM, 2 web mercator→UTM
    unit: f64,
    zone: u32,
    south: bool,
}

impl Xform {
    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        match self.kind {
            1 => to_utm(y, x, self.zone, self.south),
            2 => {
                let (la, lo) = webmerc_inverse(x, y);
                to_utm(la, lo, self.zone, self.south)
            }
            _ => (x * self.unit, y * self.unit),
        }
    }
}

/// Splits a (compound) WKT into its horizontal part and its vertical part.
fn wkt_split(w: &str) -> (&str, Option<&str>) {
    match w.find("VERT_CS[").or_else(|| w.find("VERTCRS[")) {
        Some(i) => (&w[..i], Some(&w[i..])),
        None => (w, None),
    }
}
fn wkt_last_epsg(w: &str) -> Option<u32> {
    let i = w.rfind("AUTHORITY[\"EPSG\",\"").or_else(|| w.rfind("ID[\"EPSG\","))?;
    w[i..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}
/// Metres per unit of the last linear UNIT / LENGTHUNIT in a WKT fragment.
fn wkt_last_unit(w: &str) -> Option<f64> {
    let i = w.rfind("LENGTHUNIT[").or_else(|| {
        // last UNIT that is not the angular degree unit
        w.match_indices("UNIT[").map(|m| m.0).filter(|&k| !w[k..].starts_with("UNIT[\"degree")).last()
    })?;
    let t = &w[i..];
    let f: f64 = t.split(',').nth(1)?.trim_end_matches(']').trim().parse().ok()?;
    Some(if (f - SURVEY_FT).abs() < 1e-9 {
        SURVEY_FT
    } else if (f - INTL_FT).abs() < 1e-9 {
        INTL_FT
    } else {
        f
    })
}

fn make_xform(epsg: Option<u32>, unit_key: Option<f64>, o: &Options, centre: (f64, f64)) -> crate::Result<(Xform, CrsInfo)> {
    let zu = o.z_units.as_deref().map(unit_m).transpose()?.unwrap_or(1.0);
    let geo = epsg.is_some_and(geographic);
    let wm = epsg == Some(3857) || epsg == Some(3785) || epsg == Some(900913);
    if geo || wm {
        let (la, lo) = if wm { webmerc_inverse(centre.0, centre.1) } else { (centre.1, centre.0) };
        let (zone, south) = match o.target_epsg.and_then(parse_utm_epsg) {
            Some(z) => z,
            None => (utm_zone(lo), la < 0.0),
        };
        let t = o.target_epsg.unwrap_or(utm_epsg(zone, south));
        let info = CrsInfo {
            source_epsg: epsg,
            source_kind: if wm { "web mercator".into() } else { "geographic".into() },
            xy_unit_m: 1.0,
            z_unit_m: zu,
            target_epsg: Some(t),
            note: format!("projected to UTM zone {zone}{} (Krüger series, GRS80; source datum treated as WGS84)", if south { "S" } else { "N" }),
        };
        return Ok((Xform { kind: if wm { 2 } else { 1 }, unit: 1.0, zone, south }, info));
    }
    if let Some(t) = o.target_epsg {
        if Some(t) != epsg {
            return Err(format!(
                "reprojection from EPSG:{} to EPSG:{t} is not supported (only geographic / web mercator → UTM); omit target_epsg to keep the source CRS",
                epsg.map_or("unknown".into(), |e| e.to_string())
            ));
        }
    }
    let unit = match (&o.xy_units, unit_key, epsg.and_then(epsg_foot)) {
        (Some(u), _, _) => unit_m(u)?,
        (None, Some(u), _) => u,
        (None, None, Some(u)) => u,
        _ => 1.0,
    };
    let info = CrsInfo {
        source_epsg: epsg,
        source_kind: "projected (kept planar)".into(),
        xy_unit_m: unit,
        z_unit_m: zu,
        target_epsg: epsg,
        note: if (unit - 1.0).abs() > 1e-12 {
            format!(
                "coordinates in {} scaled to metres; no reprojection",
                if (unit - SURVEY_FT).abs() < 1e-12 { "US survey feet" } else { "international feet" }
            )
        } else {
            "coordinates kept as metres; no reprojection".into()
        },
    };
    Ok((Xform { kind: 0, unit, zone: 0, south: false }, info))
}

fn finish(format: &str, raw: Vec<[f64; 3]>, src_n: usize, xf: &Xform, info: CrsInfo, o: &Options, cell: Option<f64>) -> crate::Result<Import> {
    let mut pts = Vec::with_capacity(raw.len());
    for p in &raw {
        let (x, y) = xf.apply(p[0], p[1]);
        let z = p[2] * info.z_unit_m;
        if let Some(c) = o.crop {
            if x < c[0] || x > c[2] || y < c[1] || y > c[3] {
                continue;
            }
        }
        pts.push([x, y, z]);
    }
    if pts.len() > o.max_points {
        return Err(format!("{} points after crop/step exceed max_points {}; increase step or crop", pts.len(), o.max_points));
    }
    if pts.len() < 3 {
        return Err(format!("only {} points inside the crop box", pts.len()));
    }
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut zr = [f64::MAX, f64::MIN];
    for p in &pts {
        bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
        zr = [zr[0].min(p[2]), zr[1].max(p[2])];
    }
    Ok(Import {
        format: format.into(),
        crs: info,
        source_points: src_n,
        kept_points: pts.len(),
        nodata_or_filtered: src_n.saturating_sub(raw.len()),
        bbox: bb,
        z_range: zr,
        cell_m: cell,
        points: pts,
    })
}

// ---------- rasters ----------

struct Raster {
    w: usize,
    h: usize,
    x0: f64,
    y0: f64,
    dx: f64,
    dy: f64,
    z: Vec<f64>,
    nodata: Option<f64>,
    epsg: Option<u32>,
    unit_key: Option<f64>,
    pixel_is_point: bool,
}

fn raster_points(r: &Raster, o: &Options, format: &str) -> crate::Result<Import> {
    let off = if r.pixel_is_point { 0.0 } else { 0.5 };
    let centre = (r.x0 + r.dx * r.w as f64 / 2.0, r.y0 - r.dy * r.h as f64 / 2.0);
    let (xf, info) = make_xform(r.epsg, r.unit_key, o, centre)?;
    let st = o.step.max(1);
    let mut raw = vec![];
    let mut n = 0;
    for j in (0..r.h).step_by(st) {
        for i in (0..r.w).step_by(st) {
            n += 1;
            let z = r.z[j * r.w + i];
            if !z.is_finite() || r.nodata.is_some_and(|nd| (z - nd).abs() < 1e-6) || z < -1.0e4 {
                continue;
            }
            raw.push([r.x0 + (i as f64 + off) * r.dx, r.y0 - (j as f64 + off) * r.dy, z]);
        }
    }
    let cell = if xf.kind == 0 { Some(r.dx * xf.unit * st as f64) } else { None };
    finish(format, raw, n, &xf, info, o, cell)
}

/// GeoTIFF / Cloud-Optimised GeoTIFF DEM (single band; strips or tiles).
pub fn read_geotiff(bytes: &[u8], o: &Options) -> crate::Result<Import> {
    use tiff::decoder::{Decoder, DecodingResult};
    use tiff::tags::Tag;
    let mut d = Decoder::new(Cursor::new(bytes)).map_err(|e| format!("tiff: {e}"))?.with_limits(tiff::decoder::Limits::unlimited());
    let (w, h) = d.dimensions().map_err(|e| format!("tiff: {e}"))?;
    let scale = d.get_tag_f64_vec(Tag::from_u16_exhaustive(33550)).map_err(|_| "not a GeoTIFF: ModelPixelScale tag missing")?;
    let tie = d.get_tag_f64_vec(Tag::from_u16_exhaustive(33922)).map_err(|_| "not a GeoTIFF: ModelTiepoint tag missing")?;
    let keys = d.get_tag_u16_vec(Tag::from_u16_exhaustive(34735)).unwrap_or_default();
    let nodata = d.get_tag_ascii_string(Tag::from_u16_exhaustive(42113)).ok().and_then(|s| s.trim().trim_end_matches('\0').parse::<f64>().ok());
    let mut epsg = None;
    let mut unit_key = None;
    let mut pixel_is_point = false;
    if keys.len() >= 4 {
        for k in keys[4..].chunks(4) {
            if k.len() < 4 || k[1] != 0 {
                continue;
            }
            match k[0] {
                1025 => pixel_is_point = k[3] == 2,
                2048 | 3072 if k[3] != 32767 && k[3] != 0 => epsg = Some(k[3] as u32),
                3076 => {
                    unit_key = match k[3] {
                        9001 => Some(1.0),
                        9002 => Some(INTL_FT),
                        9003 => Some(SURVEY_FT),
                        _ => None,
                    }
                }
                _ => {}
            }
        }
        // projected key wins over the geographic one
        for k in keys[4..].chunks(4) {
            if k.len() == 4 && k[0] == 3072 && k[1] == 0 && k[3] != 32767 && k[3] != 0 {
                epsg = Some(k[3] as u32);
            }
        }
    }
    let epsg = o.epsg.or(epsg);
    let img = d.read_image().map_err(|e| format!("tiff: {e}"))?;
    let z: Vec<f64> = match img {
        DecodingResult::F32(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::F64(v) => v,
        DecodingResult::I16(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::U16(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::I32(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::U32(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::U8(v) => v.into_iter().map(|x| x as f64).collect(),
        _ => return Err("unsupported GeoTIFF sample type".into()),
    };
    if z.len() < (w * h) as usize {
        return Err("multi-band or truncated GeoTIFF; export a single elevation band".into());
    }
    let (i0, j0, x, y) = (tie[0], tie[1], tie[3], tie[4]);
    let r = Raster {
        w: w as usize,
        h: h as usize,
        x0: x - i0 * scale[0],
        y0: y + j0 * scale[1],
        dx: scale[0],
        dy: scale[1],
        z: z[..(w * h) as usize].to_vec(),
        nodata,
        epsg,
        unit_key,
        pixel_is_point,
    };
    raster_points(&r, o, "GeoTIFF")
}

/// ESRI ASCII grid (.asc), e.g. OS Terrain 50.
pub fn read_asc(text: &str, o: &Options) -> crate::Result<Import> {
    let mut hdr = std::collections::HashMap::new();
    let mut lines = text.lines();
    let mut data = String::new();
    for l in lines.by_ref() {
        let mut it = l.split_whitespace();
        match (it.next(), it.next()) {
            (Some(k), Some(v)) if k.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) => {
                hdr.insert(k.to_ascii_lowercase(), v.parse::<f64>().map_err(|_| format!("asc header {k}"))?);
            }
            _ => {
                data.push_str(l);
                data.push(' ');
                break;
            }
        }
    }
    for l in lines {
        data.push_str(l);
        data.push(' ');
    }
    let g = |k: &str| hdr.get(k).copied().ok_or(format!("asc header {k} missing"));
    let (w, h, cs) = (g("ncols")? as usize, g("nrows")? as usize, g("cellsize")?);
    let (xll, yll, centre) = match (hdr.get("xllcorner"), hdr.get("xllcenter")) {
        (Some(x), _) => (*x, g("yllcorner")?, false),
        (None, Some(x)) => (*x, g("yllcenter")?, true),
        _ => return Err("asc: xllcorner or xllcenter missing".into()),
    };
    let z: Vec<f64> = data.split_whitespace().map(|s| s.parse::<f64>().unwrap_or(f64::NAN)).collect();
    if z.len() < w * h {
        return Err(format!("asc: {} values, expected {}", z.len(), w * h));
    }
    let off = if centre { 0.5 } else { 0.0 };
    let r = Raster {
        w,
        h,
        x0: xll - off * cs,
        y0: yll + (h as f64 - off) * cs,
        dx: cs,
        dy: cs,
        z,
        nodata: hdr.get("nodata_value").copied(),
        epsg: o.epsg,
        unit_key: None,
        pixel_is_point: false,
    };
    raster_points(&r, o, "ESRI ASCII grid")
}

// ---------- LAS / LAZ ----------

fn le_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn le_f64(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// LAS 1.0–1.4 or LAZ point cloud; keeps the requested ASPRS classes (default 2 = ground).
pub fn read_las(bytes: &[u8], o: &Options) -> crate::Result<Import> {
    if bytes.len() < 227 || &bytes[0..4] != b"LASF" {
        return Err("not a LAS/LAZ file (LASF signature missing)".into());
    }
    let (vmaj, vmin) = (bytes[24], bytes[25]);
    let hsize = le_u16(bytes, 94) as usize;
    let off = le_u32(bytes, 96) as usize;
    let nvlr = le_u32(bytes, 100) as usize;
    let fmt_raw = bytes[104];
    let compressed = fmt_raw & 0x80 != 0 || fmt_raw & 0x40 != 0;
    let fmt = fmt_raw & 0x3f;
    let rlen = le_u16(bytes, 105) as usize;
    let mut n = le_u32(bytes, 107) as u64;
    if vmaj == 1 && vmin >= 4 && hsize >= 375 {
        let n14 = u64::from_le_bytes(bytes[247..255].try_into().unwrap());
        if n14 > 0 {
            n = n14;
        }
    }
    let sc = [le_f64(bytes, 131), le_f64(bytes, 139), le_f64(bytes, 147)];
    let of = [le_f64(bytes, 155), le_f64(bytes, 163), le_f64(bytes, 171)];
    // VLRs: LASzip (22204) and CRS (GeoKeyDirectory 34735, OGC WKT 2112)
    let mut p = hsize;
    let mut laz_vlr = None;
    let mut z_key: Option<f64> = None;
    // the OGC WKT VLR takes precedence over GeoTIFF keys when both are present (writers such
    // as libLAS have been seen to store inconsistent keys)
    let (mut wkt_epsg, mut wkt_unit) = (None, None);
    let mut epsg = None;
    let mut unit_key = None;
    for _ in 0..nvlr {
        if p + 54 > bytes.len() {
            break;
        }
        let user = String::from_utf8_lossy(&bytes[p + 2..p + 18]).trim_end_matches('\0').to_string();
        let rid = le_u16(bytes, p + 18);
        let len = le_u16(bytes, p + 20) as usize;
        let body = &bytes[p + 54..(p + 54 + len).min(bytes.len())];
        if user == "laszip encoded" && rid == 22204 {
            laz_vlr = Some(body.to_vec());
        }
        if user == "LASF_Projection" && rid == 34735 {
            for k in body.chunks(8).skip(1) {
                if k.len() == 8 && le_u16(k, 2) == 0 {
                    let (id, v) = (le_u16(k, 0), le_u16(k, 6));
                    match id {
                        3072 | 2048 if v != 0 && v != 32767 => epsg = epsg.filter(|_| id == 2048).or(Some(v as u32)),
                        3076 => unit_key = [(9001, 1.0), (9002, INTL_FT), (9003, SURVEY_FT)].iter().find(|x| x.0 == v).map(|x| x.1),
                        _ => {}
                    }
                }
            }
        }
        if user == "LASF_Projection" && rid == 2112 {
            let w = String::from_utf8_lossy(body).to_string();
            let (h, v) = wkt_split(&w);
            wkt_epsg = wkt_last_epsg(h);
            wkt_unit = wkt_last_unit(h);
            if let Some(u) = v.and_then(wkt_last_unit) {
                z_key = Some(u);
            }
        }
        p += 54 + len;
    }
    let epsg = o.epsg.or(wkt_epsg).or(epsg);
    let unit_key = if wkt_epsg.is_some() { wkt_unit.or(Some(1.0)) } else { unit_key };
    let zo;
    let o = if o.z_units.is_none() && z_key.is_some_and(|z| z != 1.0) {
        zo = Options { z_units: Some(if z_key == Some(SURVEY_FT) { "us_ft" } else { "ft" }.into()), ..o.clone() };
        &zo
    } else {
        o
    };
    let data: Vec<u8> = if compressed || laz_vlr.is_some() {
        let v = laz_vlr.ok_or("LAZ without a LASzip VLR")?;
        let vlr = laz::LazVlr::from_buffer(&v).map_err(|e| format!("laz vlr: {e}"))?;
        let mut out = vec![0u8; n as usize * rlen];
        // the chunk table offset stored in the stream is absolute, so the reader spans the whole file
        let mut cur = Cursor::new(bytes);
        cur.set_position(off as u64);
        let mut dec = laz::LasZipDecompressor::new(cur, vlr).map_err(|e| format!("laz: {e}"))?;
        dec.decompress_many(&mut out).map_err(|e| format!("laz: {e}"))?;
        out
    } else {
        let mut v = vec![];
        Cursor::new(&bytes[off..]).take((n as usize * rlen) as u64).read_to_end(&mut v).map_err(|e| e.to_string())?;
        v
    };
    let ext = fmt >= 6;
    let mut raw = vec![];
    let mut cnt = 0;
    let st = o.step.max(1);
    let mut gi = 0usize;
    for rec in data.chunks_exact(rlen) {
        cnt += 1;
        let class = if ext { rec[16] } else { rec[15] & 0x1f };
        if !o.classes.is_empty() && !o.classes.contains(&class) {
            continue;
        }
        gi += 1;
        #[allow(clippy::manual_is_multiple_of)]
        if (gi - 1) % st != 0 {
            continue;
        }
        let xi = i32::from_le_bytes(rec[0..4].try_into().unwrap()) as f64;
        let yi = i32::from_le_bytes(rec[4..8].try_into().unwrap()) as f64;
        let zi = i32::from_le_bytes(rec[8..12].try_into().unwrap()) as f64;
        raw.push([xi * sc[0] + of[0], yi * sc[1] + of[1], zi * sc[2] + of[2]]);
    }
    if raw.is_empty() {
        return Err(format!("no points in classes {:?} (of {cnt}); pass classes [] to keep all", o.classes));
    }
    let c = raw.iter().fold((0.0, 0.0), |a, p| (a.0 + p[0], a.1 + p[1]));
    let centre = (c.0 / raw.len() as f64, c.1 / raw.len() as f64);
    let (xf, info) = make_xform(epsg, unit_key, o, centre)?;
    finish(if compressed { "LAZ" } else { "LAS" }, raw, cnt, &xf, info, o, None)
}

/// Read by extension / signature.
pub fn read_any(path: &str, o: &Options) -> crate::Result<Import> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if b.starts_with(b"LASF") {
        return read_las(&b, o);
    }
    if b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") || b.starts_with(b"II+\0") || b.starts_with(b"MM\0+") {
        return read_geotiff(&b, o);
    }
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".asc") || b.starts_with(b"ncols") || b.starts_with(b"NCOLS") {
        return read_asc(&String::from_utf8_lossy(&b), o);
    }
    Err(format!("{path}: unknown terrain format (GeoTIFF, .asc, LAS or LAZ)"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compound_wkt_units() {
        let w = r#"COMPD_CS["x",PROJCS["NAD83 / Oregon LCC (m)",GEOGCS["NAD83",UNIT["degree",0.0174532925199433]],UNIT["metre",1,AUTHORITY["EPSG","9001"]],AUTHORITY["EPSG","2991"]],VERT_CS["NAVD88 height (ftUS)",UNIT["US survey foot",0.304800609601219,AUTHORITY["EPSG","9003"]],AUTHORITY["EPSG","6360"]]]"#;
        let (h, v) = wkt_split(w);
        assert_eq!(wkt_last_epsg(h), Some(2991));
        assert_eq!(wkt_last_unit(h), Some(1.0));
        assert_eq!(v.and_then(wkt_last_unit), Some(SURVEY_FT));
    }
    #[test]
    fn utm_reference_points() {
        // references from PROJ (pyproj 3.x, EPSG:4326 → 32610 / 32733 / 3857 → 32613)
        let (e, n) = to_utm(37.4, -122.165, 10, false);
        assert!((e - 573_904.717_720).abs() < 0.001 && (n - 4_139_574.272_083).abs() < 0.001, "{e} {n}");
        let (e, n) = to_utm(-33.9, 18.0, 33, true);
        assert!((e - 777_415.983_518).abs() < 0.002 && (n - (10_000_000.0 - 3_755_121.242_936)).abs() < 0.002, "{e} {n}");
        let (la, lo) = webmerc_inverse(-11_720_000.0, 4_865_000.0);
        let (e, n) = to_utm(la, lo, 13, false);
        assert!((e - 475_879.207_549).abs() < 0.002 && (n - 4_427_075.717_054).abs() < 0.002, "{e} {n}");
        // equator on the central meridian
        let (e, n) = to_utm(0.0, -123.0, 10, false);
        assert!((e - 500_000.0).abs() < 1e-6 && n.abs() < 1e-6);
        let (la, lo) = webmerc_inverse(0.0, 0.0);
        assert!(la.abs() < 1e-12 && lo.abs() < 1e-12);
    }
    #[test]
    fn asc_grid_and_feet() {
        let t = "ncols 3\nnrows 2\nxllcorner 1000\nyllcorner 2000\ncellsize 10\nNODATA_value -9999\n1 2 3\n4 -9999 6\n";
        let o = Options { xy_units: Some("us_ft".into()), z_units: Some("us_ft".into()), ..Default::default() };
        let r = read_asc(t, &o).unwrap();
        assert_eq!(r.kept_points, 5);
        assert!((r.bbox[0] - 1005.0 * SURVEY_FT).abs() < 1e-9);
        assert!((r.z_range[1] - 6.0 * SURVEY_FT).abs() < 1e-12);
    }
    #[test]
    fn las_ground_filter() {
        // minimal LAS 1.2, point format 0, 3 points (classes 2, 1, 2)
        let mut b = vec![0u8; 227];
        b[0..4].copy_from_slice(b"LASF");
        b[24] = 1;
        b[25] = 2;
        b[94..96].copy_from_slice(&227u16.to_le_bytes());
        b[96..100].copy_from_slice(&227u32.to_le_bytes());
        b[104] = 0;
        b[105..107].copy_from_slice(&20u16.to_le_bytes());
        b[107..111].copy_from_slice(&4u32.to_le_bytes());
        for k in 0..3 {
            b[131 + 8 * k..139 + 8 * k].copy_from_slice(&0.01f64.to_le_bytes());
        }
        for (i, cls) in [2u8, 1, 2, 2].iter().enumerate() {
            let mut r = vec![0u8; 20];
            r[0..4].copy_from_slice(&(100_000 + i as i32 * 100).to_le_bytes());
            r[4..8].copy_from_slice(&(200_000i32 + (i as i32 % 2) * 50).to_le_bytes());
            r[8..12].copy_from_slice(&(5_000i32 + i as i32).to_le_bytes());
            r[15] = *cls;
            b.extend(r);
        }
        let r = read_las(&b, &Options::default()).unwrap();
        assert_eq!((r.source_points, r.kept_points), (4, 3));
        assert!((r.points[1][0] - 1002.0).abs() < 1e-9 && (r.points[0][2] - 50.0).abs() < 1e-9);
    }
}
