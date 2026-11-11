"""Compose the Nexus logo family from the carved symbol and the pressed wordmark.

Run after build_marks.py:  python3 brand/tools/build_logo.py
Every file is plain SVG paths: no fonts, no filters, no external references.
"""
from __future__ import annotations

import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
MASTERS = os.path.join(HERE, "..", "logo", "masters")
OUT = os.path.join(HERE, "..", "logo")

PINE, LINEN, INK, WHITE = "#2F4631", "#F4EFE3", "#232B1E", "#FFFFFF"
COLORWAYS = {"pine": PINE, "linen": LINEN, "ink": INK}

# The symbol is Convergence (owner decision, 2026-11-11). The whole family is built from these two drawings.
SYMBOL = "nexus-symbol-convergence.svg"
SYMBOL_SMALL = "nexus-symbol-convergence-small.svg"


def read(name):
    s = open(os.path.join(MASTERS, name)).read()
    d = re.search(r' d="([^"]+)"', s).group(1)
    vb = [float(v) for v in re.search(r'viewBox="([^"]+)"', s).group(1).split()]
    return d, vb[2], vb[3]


def tile_path(x, y, w, h, r, big):
    """The Nexus tile: three firm corners and one soft corner, top right."""
    return (f"M{x + r} {y}H{x + w - big}A{big} {big} 0 0 1 {x + w} {y + big}V{y + h - r}"
            f"A{r} {r} 0 0 1 {x + w - r} {y + h}H{x + r}A{r} {r} 0 0 1 {x} {y + h - r}V{y + r}"
            f"A{r} {r} 0 0 1 {x + r} {y}Z")


def write(name, w, h, body, title):
    with open(os.path.join(OUT, name), "w") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:g} {h:g}" role="img" aria-label="{title}">'
                f"<title>{title}</title>{body}</svg>\n")


def placed(d, x, y, k, fill):
    return f'<path fill="{fill}" fill-rule="evenodd" transform="translate({x:g} {y:g}) scale({k:g})" d="{d}"/>'


def main():
    sym, sw, sh = read(SYMBOL)
    small, _, _ = read(SYMBOL_SMALL)
    word, ww, wh = read("nexus-wordmark.svg")

    for cname, c in COLORWAYS.items():
        # symbol alone
        write(f"nexus-symbol-{cname}.svg", 512, 512, placed(sym, 0, 0, 1, c), "Nexus")
        write(f"nexus-symbol-small-{cname}.svg", 512, 512, placed(small, 0, 0, 1, c), "Nexus")
        # wordmark alone
        write(f"nexus-wordmark-{cname}.svg", ww, wh, placed(word, 0, 0, 1, c), "Nexus")
        # horizontal: symbol, then the name on its centre line
        S, cap, gap = 256, 118, 40
        k = cap / wh
        body = placed(sym, 0, 0, S / 512, c) + placed(word, S + gap, (S - cap) / 2 + 3, k, c)
        write(f"nexus-logo-horizontal-{cname}.svg", S + gap + ww * k, S, body, "Nexus")
        # stacked: symbol over the name
        S2, cap2, gap2 = 300, 92, 40
        k2 = cap2 / wh
        W = max(S2, ww * k2)
        body = placed(sym, (W - S2) / 2, 0, S2 / 512, c) + placed(word, (W - ww * k2) / 2, S2 + gap2, k2, c)
        write(f"nexus-logo-stacked-{cname}.svg", W, S2 + gap2 + cap2, body, "Nexus")

    # app tile: Pine block, Linen symbol, one soft corner
    k = 0.70
    off = (512 - 512 * k) / 2
    write("nexus-app-icon.svg", 512, 512,
          f'<path fill="{PINE}" d="{tile_path(0, 0, 512, 512, 64, 176)}"/>' + placed(small, off, off, k, LINEN),
          "Nexus")
    write("nexus-app-icon-linen.svg", 512, 512,
          f'<path fill="{LINEN}" d="{tile_path(0, 0, 512, 512, 64, 176)}"/>' + placed(small, off, off, k, PINE),
          "Nexus")
    # maskable: full-bleed square, symbol inside the 80 percent safe zone
    k = 0.56
    off = (512 - 512 * k) / 2
    write("nexus-app-icon-maskable.svg", 512, 512,
          f'<rect width="512" height="512" fill="{PINE}"/>' + placed(small, off, off, k, LINEN), "Nexus")
    # favicon: small-size symbol that follows the browser theme
    write("nexus-favicon.svg", 512, 512,
          "<style>path{fill:#2F4631}@media(prefers-color-scheme:dark){path{fill:#DCE5C3}}</style>"
          f'<path fill-rule="evenodd" d="{small}"/>', "Nexus")
    export_png()
    print("logo family written:", len([f for f in os.listdir(OUT) if f.endswith(".svg")]), "files")


def export_png():
    """Raster app icons and favicons, rendered from the SVGs with headless Chrome when it is installed."""
    import subprocess
    import tempfile
    chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    if not os.path.exists(chrome):
        print("Chrome not found: skipped PNG exports")
        return
    os.makedirs(os.path.join(OUT, "png"), exist_ok=True)
    jobs = [("nexus-app-icon.svg", "nexus-app-icon-512.png", 512), ("nexus-app-icon.svg", "nexus-app-icon-192.png", 192),
            ("nexus-app-icon-maskable.svg", "nexus-app-icon-maskable-512.png", 512),
            ("nexus-app-icon-maskable.svg", "nexus-apple-touch-icon-180.png", 180),
            ("nexus-app-icon.svg", "nexus-favicon-48.png", 48), ("nexus-app-icon.svg", "nexus-favicon-32.png", 32),
            ("nexus-symbol-small-pine.svg", "nexus-favicon-16.png", 16),
            ("nexus-logo-horizontal-pine.svg", "nexus-logo-horizontal-pine-800.png", 800),
            ("nexus-logo-horizontal-linen.svg", "nexus-logo-horizontal-linen-800.png", 800)]
    with tempfile.TemporaryDirectory() as tmp:
        for src, dst, size in jobs:
            s = open(os.path.join(OUT, src)).read()
            vb = [float(v) for v in re.search(r'viewBox="([^"]+)"', s).group(1).split()]
            h = round(size * vb[3] / vb[2])
            page = os.path.join(tmp, "p.html")
            open(page, "w").write(f'<body style="margin:0;background:transparent"><div style="width:{size}px;height:{h}px">{s}</div>')
            subprocess.run([chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--default-background-color=00000000",
                            f"--window-size={size},{h}", f"--screenshot={os.path.join(OUT, 'png', dst)}", "file://" + page],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print("PNG exports:", len(jobs))


if __name__ == "__main__":
    main()
