"""Shared drawing helpers for the Nexus brand generators.

Every Nexus mark and pattern is drawn the same way a linocut is made:
start with a solid block of ink, carve lines out of it, then press it.

  1. build a 0..1 ink field on a pixel grid (solid shapes minus carved lines)
  2. soften and re-threshold it, which rounds every corner the way a gouge does
  3. trace the result into smooth vector outlines

Build-time only. The exported SVG files have no dependencies.
"""
from __future__ import annotations

import numpy as np
from scipy.ndimage import gaussian_filter
from skimage import measure


def noise(shape, scale, seed, wrap=False, octaves=1):
    """Smooth random field with unit variance. `scale` is feature size in px."""
    rng = np.random.default_rng(seed)
    out = np.zeros(shape)
    amp, total = 1.0, 0.0
    for o in range(octaves):
        f = gaussian_filter(rng.standard_normal(shape), scale / (2 ** o),
                            mode="wrap" if wrap else "reflect")
        f /= f.std() + 1e-9
        out += amp * f
        total += amp
        amp *= 0.5
    return out / total


def smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def grid(size):
    """Pixel grid as (x, y) arrays; size may be int or (w, h)."""
    w, h = (size, size) if isinstance(size, int) else size
    y, x = np.mgrid[0:h, 0:w].astype(float)
    return x, y


def press(ink, soften=3.0, rough=0.0, rough_scale=3.0, seed=1, wrap=False):
    """Round the corners of a hard-edged ink field and add a hand-cut wobble."""
    f = gaussian_filter(ink.astype(float), soften, mode="wrap" if wrap else "constant")
    if rough:
        f = f + rough * noise(ink.shape, rough_scale, seed, wrap=wrap)
    return f


def trace(field, level=0.5, tol=0.9, min_pts=10):
    """Trace a pressed field into closed polygons, in (x, y) pixel coordinates."""
    polys = []
    for c in measure.find_contours(np.pad(field, 2, constant_values=0.0), level):
        c = c - 2
        if len(c) < min_pts:
            continue
        p = measure.approximate_polygon(c, tol)
        if len(p) < 4:
            continue
        if np.allclose(p[0], p[-1]):
            p = p[:-1]
        polys.append(p[:, ::-1])
    return polys


def _fmt(v, prec=1):
    s = f"{v:.{prec}f}"
    return s.rstrip("0").rstrip(".") if "." in s else s


def path_d(polys, scale=1.0, dx=0.0, dy=0.0, smooth=0.5, prec=1):
    """Closed Catmull-Rom splines through each polygon, as one SVG path string."""
    parts = []
    for p in polys:
        p = p * scale + np.array([dx, dy])
        n = len(p)
        if n < 3:
            continue
        seg = [f"M{_fmt(p[0][0], prec)} {_fmt(p[0][1], prec)}"]
        for i in range(n):
            p0, p1, p2, p3 = p[i - 1], p[i], p[(i + 1) % n], p[(i + 2) % n]
            c1 = p1 + (p2 - p0) * smooth / 3
            c2 = p2 - (p3 - p1) * smooth / 3
            seg.append("C" + " ".join(_fmt(v, prec) for v in (*c1, *c2, *p2)))
        parts.append("".join(seg) + "Z")
    return "".join(parts)


def svg(body, w, h, title, desc=""):
    d = f"<desc>{desc}</desc>" if desc else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">'
            f"<title>{title}</title>{d}{body}</svg>\n")


def band(u, centre, width):
    """1 where u is within width/2 of centre. width may be an array."""
    return (np.abs(u - centre) < width / 2).astype(float)
