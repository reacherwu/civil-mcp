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
    a = im.crop((int(W * 0.015), int(H * 0.025), int(W * 0.815), int(H * 0.975)))
    bb = ImageOps.invert(a.convert("L")).point(lambda v: 255 if v > 25 else 0).getbbox()
    a = a.crop((max(bb[0] - 10, 0), max(bb[1] - 10, 0), bb[2] + 10, bb[3] + 10))
    if a.width > 1400:
        a = a.resize((1400, int(a.height * 1400 / a.width)), Image.LANCZOS)
    a.quantize(96, method=Image.Quantize.MEDIANCUT).save(f"docs/img/crop/sheet-{n:02d}.png", optimize=True)
print("rendered", n, "sheets")
