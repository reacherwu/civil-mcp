"""SI view of a civil-mcp output directory.

When the run used US customary units (units.json with system "US"), the CSV
and Markdown deliverables are in ft, CY, SY, LF, ... This module converts
them back to SI with its own conversion table (independent of the Rust code)
into a temporary copy, so the independent checks can run unchanged in both
unit systems. SI runs are returned as they are.
"""
import json, os, re, shutil, tempfile

INTL_FT = 0.3048
SURVEY_FT = 1200.0 / 3937.0
LBF = 4.4482216152605
KEEP_SI = {"flow-grid.csv"}
# decimals the US writer adds per unit (one for ft/sf/cy, two for in)
EXTRA = {"ft": 1, "sf": 1, "cy": 1, "in": 2}


def factors(ft):
    return {
        "ft": ft, "sf": ft * ft, "cy": 27 * ft ** 3, "cyft": 27 * ft ** 4, "cfs": INTL_FT ** 3,
        "in": 25.4, "in2": 645.16, "mph": 1.609344, "fps": INTL_FT, "fps2": INTL_FT,
        "ac": 43560 * ft * ft / 1e4, "psf": LBF / INTL_FT ** 2 / 1000, "psi": LBF / 0.0254 ** 2 / 1e6,
        "pcf": LBF / INTL_FT ** 3 / 1000, "lbf": LBF, "in_h": 25.4, "inh": 25.4,
    }


# US key suffix -> SI suffix (same table as the tool boundary, longest first)
SUFFIX = [("_in_h", "_mm_h"), ("_inh", "_mmh"), ("_cyft", "_m3m"), ("_cfs", "_m3s"), ("_fps2", "_mps2"),
          ("_pcf", "_knm3"), ("_in2", "_mm2"), ("_mph", "_kmh"), ("_psf", "_kpa"), ("_psi", "_mpa"),
          ("_cy", "_m3"), ("_sf", "_m2"), ("_ac", "_ha"), ("_lbf", "_kn"), ("_in", "_mm"), ("_ft", "_m")]
# "_fps" maps back to "_mps" or "_ms": resolved by the SI header it came from
LEN_COLS = {"station", "easting", "northing", "elevation", "fg_elev", "eg_elev", "TS_or_PC", "SC", "CS", "ST_or_PT",
            "rim", "invert", "inv_in", "inv_out", "from_station", "to_station", "sta_from", "sta_to"}
TEXT = [("ft³/s", "m³/s", "cfs"), ("CY·ft", "m³·m", "cyft"), ("CY", "m³", "cy"), ("ft²", "m²", "sf"), ("ft/s²", "m/s²", "fps2"),
        ("ft/s", "m/s", "fps"), ("in²", "mm²", "in2"), ("in/h", "mm/h", "in_h"), ("mph", "km/h", "mph"), ("pcf", "kN/m³", "pcf"),
        ("lbf", "kN", "lbf"), ("psf", "kPa", "psf"), ("psi", "MPa", "psi"), ("ac", "ha", "ac"), ("ft", "m", "ft"), ("in", "mm", "in")]
BOQ = {"CY": ("m³", "cy"), "SY": ("m²", "sy"), "LF": ("m", "ft"), "TON": ("t", "ton"), "AC": ("ha", "ac"), "EA": ("each", None)}


def us_header(h, si_names):
    """Map one US CSV header back to (SI header, factor key)."""
    if h.endswith("_ft") and h[:-3] in LEN_COLS:
        return h[:-3], "ft"
    for us, si in SUFFIX:
        if h.endswith(us) and len(h) > len(us):
            return h[: -len(us)] + si, us[1:]
    if h.endswith("_fps"):
        return h[:-4] + "_mps", "fps"
    return h, None


def conv_csv(text, F):
    out, header, keys = [], True, []
    for line in text.splitlines():
        if line.startswith("#") or not line.strip():
            out.append(conv_text(line, F)); header = True; continue
        cells = split(line)
        if header:
            keys = []
            hs = []
            for c in cells:
                h, k = us_header(c.strip(), None)
                hs.append(h); keys.append(k)
            out.append(",".join(hs)); header = False
            boq = "unit" in hs and "quantity" in hs
            iu, iq = (hs.index("unit"), hs.index("quantity")) if boq else (None, None)
            continue
        row = []
        for i, c in enumerate(cells):
            k = keys[i] if i < len(keys) else None
            try:
                v = float(c)
                if k:
                    dec = len(c.strip().split(".")[1]) if "." in c else 0
                    sd = max(dec - EXTRA.get(k, 0), 0)
                    row.append(f"{v * F[k]:.{sd}f}")
                else:
                    row.append(c)
            except ValueError:
                row.append(conv_text(c, F) if k is None else c)
        if iu is not None and iu < len(row) and row[iu] in BOQ:
            si, fk = BOQ[row[iu]]
            if fk:
                f = {"cy": F["cy"], "sy": 9 * F["sf"], "ft": F["ft"], "ton": 0.90718474, "ac": F["ac"]}[fk]
                row[iq] = repr(float(row[iq]) * f)
            row[iu] = si
        out.append(",".join(row))
    return "\n".join(out) + "\n"


def split(line):
    v, cur, q = [], "", False
    for ch in line:
        if ch == '"':
            q = not q; cur += ch
        elif ch == "," and not q:
            v.append(cur); cur = ""
        else:
            cur += ch
    v.append(cur)
    return v


NUM = r"(?<![\w.#])(\d+(?:\.\d+)?)( ?)"


def conv_text(s, F):
    # US stations ss+ff.ff -> metric k+mmm.mm
    def sta(m):
        v = (int(m.group(1)) * 100 + float(m.group(2))) * F["ft"]
        k = int(v // 1000)
        return f"{k}+{v - k * 1000:06.2f}"
    s = re.sub(r"(?<![\w.])(\d+)\+(\d{2}\.\d{2})(?!\d)", sta, s)
    for us, si, fk in TEXT:
        pat = NUM + re.escape(us) + r"(?![\w²³])"
        s = re.sub(pat, lambda m, si=si, fk=fk: f"{float(m.group(1)) * F[fk]:.6g}{m.group(2)}{si}", s)
    return s


def si_dir(out):
    p = os.path.join(out, "units.json")
    if not os.path.exists(p):
        return out
    u = json.load(open(p))
    if u.get("system") != "US":
        return out
    F = factors(SURVEY_FT if u.get("foot") == "survey" else INTL_FT)
    tmp = tempfile.mkdtemp(prefix="civil-si-")
    for f in os.listdir(out):
        src, dst = os.path.join(out, f), os.path.join(tmp, f)
        if os.path.isdir(src):
            continue
        if f.endswith(".csv") and f not in KEEP_SI:
            open(dst, "w").write(conv_csv(open(src).read(), F))
        elif f.endswith(".md"):
            open(dst, "w").write(conv_text(open(src).read(), F))
        else:
            shutil.copy(src, dst)
    print(f"US customary run ({u.get('foot')} foot): checking the SI view in {tmp}")
    return tmp
