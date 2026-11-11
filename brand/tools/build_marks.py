"""Draw the Nexus symbol and wordmark as carved-block vector marks.

Run:  python3 brand/tools/build_marks.py [out_dir]
Writes the master drawings into brand/logo/masters (or out_dir); build_logo.py composes the family from them.
"""
from __future__ import annotations

import os
import sys

import numpy as np

from nxgen import band, grid, noise, path_d, press, smoothstep, svg, trace

S = 1024           # working grid
VB = 512           # exported viewBox
INK = "currentColor"


def export(field, name, title, desc, out, tol=1.0):
    polys = trace(field, tol=tol)
    d = path_d(polys, scale=VB / S)
    body = f'<path fill="{INK}" fill-rule="evenodd" d="{d}"/>'
    path = os.path.join(out, name)
    with open(path, "w") as f:
        f.write(svg(body, VB, VB, title, desc))
    print(f"{name}: {len(polys)} shapes, {os.path.getsize(path) / 1024:.1f} KB")


def trunk(seed, cx=506, cy=520, r0=452, wobble=1.0):
    """Polar frame of an irregular trunk: rho is 0 at the pith, 1 at the bark."""
    x, y = grid(S)
    dx, dy = x - cx, y - cy
    r = np.hypot(dx, dy)
    th = np.arctan2(dy, dx)
    rng = np.random.default_rng(seed)
    ph = rng.uniform(0, 2 * np.pi, 6)
    edge = 1 + wobble * (0.040 * np.cos(2 * th + ph[0]) + 0.026 * np.cos(3 * th + ph[1])
                         + 0.014 * np.cos(5 * th + ph[2]) + 0.008 * np.cos(9 * th + ph[3]))
    return x, y, r, th, r / (r0 * edge)


def checks(r, th, angles, r_in, r_out, width, seed, bend=0.07):
    """Radial drying cracks. Returns 1 where wood has split."""
    rng = np.random.default_rng(seed)
    cut = np.zeros_like(r)
    for a in angles:
        p1, p2 = rng.uniform(0, 6.28, 2)
        centre = np.deg2rad(a) + bend * np.sin(r / 95 + p1) + 0.5 * bend * np.sin(r / 41 + p2)
        d = np.angle(np.exp(1j * (th - centre)))
        across = np.abs(r * np.sin(d))
        w = width * (0.75 + 0.55 * smoothstep(r_in, r_out, r))
        cut = np.maximum(cut, ((across < w / 2) & (np.cos(d) > 0) & (r > r_in) & (r < r_out)).astype(float))
    return cut


def mark_convergence(out, rings=(0.265, 0.435, 0.60, 0.755), line=0.056, name="nexus-symbol-convergence.svg",
                     crack=26, soften=3.4, pith=0.098, seed=7, title="Nexus symbol: Convergence",
                     angles=(-97, 12, 131), reaches=(0.755, 0.60, 0.435), bark=0.880, squiggle=0.021):
    """A cross-section. Growth rings around one pith; three checks meet at it; a bark layer holds them."""
    x, y, r, th, rho = trunk(seed)
    warp = rho + 0.020 * noise((S, S), 150, seed + 1) * smoothstep(0.05, 0.5, rho) * (1 - smoothstep(0.70, 0.86, rho))
    ink = (rho < 1.0).astype(float)
    wv = 1 + 0.30 * noise((S, S), 70, seed + 2)
    for c in rings:
        ink -= band(warp, c, line * wv)
    if bark:
        # the bark: one line that is plainly not a growth ring. It wanders in and out in short,
        # uneven waves, the way the inner face of bark keys into the wood.
        rng = np.random.default_rng(seed + 9)
        p = rng.uniform(0, 2 * np.pi, 4)
        wave = (np.sin(19 * th + 1.7 * np.sin(3 * th + p[0]) + p[1])
                + 0.45 * np.sin(31 * th + 1.2 * np.sin(2 * th + p[2]) + p[3]))
        ink -= band(rho, bark + squiggle * wave / 1.45, line * 0.92)
    # the pith: a carved eye with a seed left standing in it
    ink -= ((warp < pith + line * 0.9) & (warp > pith * 0.0)).astype(float) * (warp > 0.050)
    r_in = 452 * (pith + line * 0.5)
    # three checks of unequal reach and spacing: natural splits, not a spoked emblem
    for ang, reach in zip(angles, reaches):
        ink -= checks(r, th, (ang,), r_in, 452 * reach, crack, seed + 3 + abs(int(ang)))
    ink = np.clip(ink, 0, 1)
    f = press(ink, soften=soften, rough=0.05, rough_scale=5, seed=seed + 4)
    export(f, name, title,
           "Tree cross-section. Rings record growth; three checks, for care, billing and oversight, meet at the pith; "
           "a wavy bark line holds them all.", out)


def mark_build(out):
    """The symbol in four cuts, for the guide: pith, rings, checks, bark."""
    steps = [("1-pith", dict(rings=(), reaches=(), angles=(), bark=None)),
             ("2-rings", dict(reaches=(), angles=(), bark=None)),
             ("3-checks", dict(bark=None)),
             ("4-bark", dict())]
    for tag, kw in steps:
        mark_convergence(out, name=f"nexus-symbol-build-{tag}.svg", title=f"Nexus symbol, step {tag}", **kw)


def mark_convergence_small(out):
    """Optical version for 16 to 32 px: two rings, wider cuts, no bark line (it closes up)."""
    mark_convergence(out, rings=(0.46, 0.80), line=0.105, crack=52, soften=5.0, pith=0.13, bark=None,
                     reaches=(0.80, 0.80, 0.46), name="nexus-symbol-convergence-small.svg", title="Nexus symbol: Convergence, small sizes")


def wordmark(out, text="Nexus", name="nexus-wordmark.svg", soften=3.6, track=-10,
             axes=(("opsz", 144), ("wght", 800), ("SOFT", 100), ("WONK", 1))):
    """The name, drawn from Fraunces at its heaviest and softest, then pressed like the symbol.

    It is supplied as outlines, so nothing that shows the logo needs the font.
    """
    from fontTools.ttLib import TTFont
    from PIL import Image, ImageDraw, ImageFont
    font = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "fonts", "variable", "Fraunces-Variable.ttf")
    size = 520
    ft = ImageFont.truetype(font, size)
    want = dict(axes)
    ft.set_variation_by_axes([want[a.axisTag] for a in TTFont(font)["fvar"].axes])
    widths = [ft.getlength(ch) for ch in text]
    W = int(sum(widths) + track * (len(text) - 1)) + 160
    H = 760
    im = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(im)
    xx = 80
    for ch, w in zip(text, widths):
        d.text((xx, 60), ch, fill=255, font=ft)
        xx += w + track
    a = np.asarray(im, float) / 255
    f = press(a, soften=soften, rough=0.035, rough_scale=5, seed=5)
    polys = trace(f, tol=0.9)
    ys, xs = np.where(f > 0.5)
    x0, x1, y0, y1 = xs.min(), xs.max(), ys.min(), ys.max()
    k = 200 / (y1 - y0)        # cap-to-baseline region scaled to 200 units tall
    dpath = path_d(polys, scale=k, dx=-x0 * k, dy=-y0 * k)
    w, h = (x1 - x0) * k, (y1 - y0) * k
    with open(os.path.join(out, name), "w") as fh:
        fh.write(svg(f'<path fill="{INK}" fill-rule="evenodd" d="{dpath}"/>', f"{w:.0f}", f"{h:.0f}", f"{text} wordmark"))
    print(name, f"{w:.0f}x{h:.0f}", f"{os.path.getsize(os.path.join(out, name)) / 1024:.1f} KB")


if __name__ == "__main__":
    here = os.path.dirname(os.path.abspath(__file__))
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "..", "logo", "masters")
    os.makedirs(out, exist_ok=True)
    mark_convergence(out)
    mark_convergence_small(out)
    mark_build(out)
    wordmark(out)
