"""Render the README images from a demo output folder.

usage: python scripts/render_readme_images.py <demo-out>   (needs poppler pdftoppm + Pillow)
writes docs/img/sheets/sheet-NN.png (full sheet, 2000 px), docs/img/thumbs/ (420 px) and
docs/img/crop/ (drawing area without the title block, <= 1400 px). Palette PNGs keep the total small.
"""
import glob, os, subprocess, sys, tempfile
from PIL import Image, ImageOps

out = sys.argv[1]
for d in ("sheets", "thumbs", "crop"):
    os.makedirs(f"docs/img/{d}", exist_ok=True)
tmp = tempfile.mkdtemp()
subprocess.run(["pdftoppm", "-png", "-scale-to-x", "2000", "-scale-to-y", "-1", f"{out}/sheets.pdf", f"{tmp}/s"], check=True)
for f in sorted(glob.glob(f"{tmp}/s-*.png")):
    n = int(f.rsplit("-", 1)[1][:-4])
    im = Image.open(f).convert("RGB")
    im.quantize(64, method=Image.Quantize.MEDIANCUT).save(f"docs/img/sheets/sheet-{n:02d}.png", optimize=True)
    t = im.copy()
    t.thumbnail((420, 420), Image.LANCZOS)
    t.quantize(128).save(f"docs/img/thumbs/sheet-{n:02d}.png", optimize=True)
    W, H = im.size
    a = im.crop((int(W * 0.045), int(H * 0.025), int(W * 0.833), int(H * 0.975)))
    bb = ImageOps.invert(a.convert("L")).point(lambda v: 255 if v > 25 else 0).getbbox()
    a = a.crop((max(bb[0] - 10, 0), max(bb[1] - 10, 0), bb[2] + 10, bb[3] + 10))
    if a.width > 1400:
        a = a.resize((1400, int(a.height * 1400 / a.width)), Image.LANCZOS)
    a.quantize(96, method=Image.Quantize.MEDIANCUT).save(f"docs/img/crop/sheet-{n:02d}.png", optimize=True)
print("rendered", n, "sheets")

# --frames <dir>: per-country frame gallery from `cargo run -p civil-demo --example frames_gallery -- <dir>`
if "--frames" in sys.argv:
    fdir = sys.argv[sys.argv.index("--frames") + 1] + "/frames"
    os.makedirs("docs/img/frames", exist_ok=True)
    for tag in sorted(os.listdir(fdir)):
        src = f"{fdir}/{tag}/sheets.pdf"
        for page, name in ((3, "plan"), (1, "cover")):
            subprocess.run(["pdftoppm", "-png", "-f", str(page), "-l", str(page), "-scale-to", "1600", src, f"{tmp}/{tag}-{name}"], check=True)
            f = sorted(glob.glob(f"{tmp}/{tag}-{name}*.png"))[0]
            im = Image.open(f).convert("RGB")
            im.quantize(64, method=Image.Quantize.MEDIANCUT).save(f"docs/img/frames/{tag}-{name}.png", optimize=True)
            if name == "plan":
                # title block close-up: right strip (landscape >= A3) or bottom strip (portrait)
                W, H = im.size
                box = (int(W * 0.66), 0, W, H) if W > H else (0, int(H * 0.55), W, H)
                tb = im.crop(box)
                bb = ImageOps.invert(tb.convert("L")).point(lambda v: 255 if v > 25 else 0).getbbox()
                tb = tb.crop(bb)
                hi = subprocess.run(["pdftoppm", "-png", "-f", "3", "-l", "3", "-r", "150", src, f"{tmp}/{tag}-hi"], check=True)
                big = Image.open(sorted(glob.glob(f"{tmp}/{tag}-hi*.png"))[0]).convert("RGB")
                k = big.width / W
                tb = big.crop(tuple(int(v * k) for v in (box[0] + bb[0], box[1] + bb[1], box[0] + bb[2], box[1] + bb[3])))
                if tb.height > 1400:
                    tb = tb.resize((int(tb.width * 1400 / tb.height), 1400), Image.LANCZOS)
                if tb.width > 1400:
                    tb = tb.resize((1400, int(tb.height * 1400 / tb.width)), Image.LANCZOS)
                tb.quantize(64, method=Image.Quantize.MEDIANCUT).save(f"docs/img/frames/{tag}-titleblock.png", optimize=True)
    print("rendered frames for", ", ".join(sorted(os.listdir(fdir))))
