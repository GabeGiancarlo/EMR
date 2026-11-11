"""Draw the Nexus pattern library.

  ring-press   poster of pressed slab strips (cover art, section openers)
  ring-field   one large off-centre cross-section (sign-in, empty workspace)
  bark         seamless bark-and-knot tile
  contour      seamless map-contour tile, in thin line
  understory   seamless scatter of cut-paper forest motifs
  leaf-vein    the midrib and veins that sit under the text of a leaf card

Vector files are clean carved shapes in currentColor. The *-pressed.png files add the
dry-brush texture of a real print and are alpha masks: white is ink, so CSS can colour them.

Run:  python3 brand/tools/build_patterns.py
"""
from __future__ import annotations

import os

import numpy as np
from PIL import Image
from scipy.ndimage import gaussian_filter, map_coordinates
from skimage import measure

from nxgen import _fmt, grid, noise, path_d, press, smoothstep, svg, trace

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "patterns")
PINE = (47, 70, 49)


def save_svg(field, name, w, h, title, tol=1.1, wrap=False):
    if wrap:
        pad = 96
        polys = trace(np.pad(field, pad, mode="wrap"), tol=tol)
        d = path_d(polys, dx=-pad, dy=-pad)
    else:
        d = path_d(trace(field, tol=tol))
        polys = d.split("Z")
    body = f'<path fill="currentColor" fill-rule="evenodd" d="{d}"/>'
    p = os.path.join(OUT, name)
    with open(p, "w") as f:
        f.write(svg(body, w, h, title))
    print(f"{name}: {len(polys)} shapes, {os.path.getsize(p) / 1024:.0f} KB")


def save_pressed(field, name, texture, colour=PINE):
    """Write the print as an alpha mask and as a Pine-on-transparent PNG."""
    a = np.clip((field - 0.5) * 9 + 0.5, 0, 1) * texture
    a8 = (np.clip(a, 0, 1) * 255).astype(np.uint8)
    white = np.full(a8.shape + (4,), 255, np.uint8)
    white[..., 3] = a8
    Image.fromarray(white, "RGBA").save(os.path.join(OUT, name + "-pressed-mask.png"), optimize=True)
    rgba = np.zeros(a8.shape + (4,), np.uint8)
    rgba[..., :3] = colour
    rgba[..., 3] = a8
    Image.fromarray(rgba, "RGBA").save(os.path.join(OUT, name + "-pressed-pine.png"), optimize=True)
    print(f"{name}-pressed: {os.path.getsize(os.path.join(OUT, name + '-pressed-pine.png')) / 1024:.0f} KB")


def slab(w, h, cx, cy, r0, period, seed, duty=0.60, bark=True, n_checks=5):
    """Ring bands of one trunk seen in cross-section. Returns (ink, rho, theta, r)."""
    x, y = grid((w, h))
    dx, dy = x - cx, y - cy
    r = np.hypot(dx, dy)
    th = np.arctan2(dy, dx)
    rng = np.random.default_rng(seed)
    ph = rng.uniform(0, 6.28, 6)
    edge = 1 + 0.05 * np.cos(2 * th + ph[0]) + 0.03 * np.cos(3 * th + ph[1]) + 0.015 * np.cos(7 * th + ph[2])
    rr = r / edge
    rr = rr + 0.22 * period * noise((h, w), period * 4.5, seed + 1) * smoothstep(0, period * 2, rr)
    # ring widths drift: good years and lean years
    k = np.linspace(0, 1, 2048)
    drift = np.cumsum(1 + 0.42 * noise((2048,), 26, seed + 2))
    drift = drift / drift[-1] * (r0 * 1.6 / period)
    u = np.interp(np.clip(rr / (r0 * 1.6), 0, 1), k, drift)
    wv = duty * (1 + 0.16 * noise((h, w), period * 2, seed + 3))
    ink = ((u % 1.0) < wv).astype(float)
    ink = np.where(rr < period * 0.42, 1.0, ink)                      # pith
    cut = np.zeros_like(ink)
    for i in range(n_checks):
        a = rng.uniform(0, 6.28)
        r_in, r_out = rng.uniform(0.05, 0.45) * r0, rng.uniform(0.6, 1.02) * r0
        centre = a + 0.05 * np.sin(r / (period * 3.1) + rng.uniform(0, 6))
        d = np.angle(np.exp(1j * (th - centre)))
        wdt = period * rng.uniform(0.20, 0.34) * (0.5 + smoothstep(r_in, r_out, r))
        cut = np.maximum(cut, (np.abs(r * np.sin(d)) < wdt / 2) & (np.cos(d) > 0) & (r > r_in) & (r < r_out))
    ink = np.where(cut > 0, 0.0, ink)
    if bark:
        ink = np.where(rr > r0, 0.0, ink)
        ink = np.where((rr > r0 - period * 0.9) & (rr <= r0), 1.0, ink)
    return ink, rr, th, r


def drybrush(shape, rr, th, period, seed, cover=0.90):
    """Ink that skips along the grain, the way a pressed block prints. 0..1 multiplier."""
    h, w = shape
    polar = gaussian_filter(np.random.default_rng(seed + 1).standard_normal((512, 1024)), (0.8, 14), mode="wrap")
    polar = polar / polar.std()
    ri = np.clip(rr / (period * 0.9), 0, 1e9) % 512
    ti = (th + np.pi) / (2 * np.pi) * 1023
    streak = map_coordinates(polar, [ri, ti], order=1, mode="wrap")
    speck = noise(shape, 1.1, seed + 2)
    cloud = noise(shape, 90, seed + 3)
    t = 0.75 * streak + 0.55 * speck + 0.5 * cloud
    thr = np.quantile(t, 1 - cover)
    return np.clip((t - thr) * 3.0 + 0.5, 0, 1)


def ring_press():
    """Five pressed strips, each cut from a different slab."""
    W, H = 1200, 1500
    margin, gap = 70, 30
    n = 5
    sh = (H - 2 * margin - (n - 1) * gap) / n
    x, y = grid((W, H))
    ink = np.zeros((H, W))
    tex = np.ones((H, W))
    spec = [  # (cx, cy relative to strip centre, r0, period, seed)
        (0.30, 1.9, 1500, 30, 3), (0.72, 0.9, 1300, 34, 8), (0.52, -0.2, 1250, 38, 13),
        (0.18, -1.1, 1400, 32, 21), (0.80, 1.4, 1500, 28, 34)]
    edge_wob = 5 * noise((H, W), 40, 99)
    side_wob = 4 * noise((H, W), 40, 98)
    for i, (fx, fy, r0, period, seed) in enumerate(spec):
        y0 = margin + i * (sh + gap)
        yc = y0 + sh / 2
        s, rr, th, r = slab(W, H, fx * W, yc + fy * sh, r0, period, seed, bark=False, n_checks=7)
        m = (y + edge_wob > y0) & (y + edge_wob < y0 + sh) & (x + side_wob > margin) & (x + side_wob < W - margin)
        ink = np.where(m, s, ink)
        tex = np.where(m, drybrush((H, W), rr, th, period, seed + 50), tex)
    f = press(ink, soften=2.6, rough=0.06, rough_scale=4, seed=4)
    save_svg(f, "nexus-pattern-ring-press.svg", W, H, "Nexus pattern: ring press", tol=1.2)
    save_pressed(f, "nexus-pattern-ring-press", tex)


def ring_field():
    """One wide crop of a single slab, pith off to the lower left."""
    W, H = 1600, 1000
    s, rr, th, r = slab(W, H, 380, 760, 1700, 44, 61, bark=False, n_checks=9, duty=0.58)
    f = press(s, soften=3.0, rough=0.06, rough_scale=4, seed=6)
    save_svg(f, "nexus-pattern-ring-field.svg", W, H, "Nexus pattern: ring field", tol=1.3)
    save_pressed(f, "nexus-pattern-ring-field", drybrush((H, W), rr, th, 44, 77, cover=0.88))


def bark():
    """Seamless bark tile. The grain is the contour map of a slope with a few hills on it:
    lines run straight where the wood is clear and close into rings around each knot."""
    T, period = 640, 40
    x, y = grid(T)
    psi = x + 13 * noise((T, T), 80, 5, wrap=True, octaves=2) + 6 * np.sin(2 * np.pi * y / T * 2 + 1.0)
    for (kx, ky, s, lift) in [(190, 180, 62, 4.2), (470, 480, 74, 5.0)]:
        for ox in (-T, 0, T):
            for oy in (-T, 0, T):
                d2 = ((x - kx - ox) / s) ** 2 + ((y - ky - oy) / (1.9 * s)) ** 2
                psi = psi + lift * period * np.exp(-d2 / 2)
    wv = 1 + 0.28 * noise((T, T), 40, 6, wrap=True)
    # constant line weight in pixels, however tightly the grain bunches
    u = psi / period
    gy, gx = np.gradient(np.pad(u, 2, mode="wrap"))
    slope = np.hypot(gx, gy)[2:-2, 2:-2]
    ink = np.abs(((u + 0.5) % 1.0) - 0.5) < np.clip(5.6 * wv * slope, 0, 0.36)
    gaps = noise((T, T), 14, 7, wrap=True) + 0.8 * noise((T, T), 70, 8, wrap=True)
    ink = ink & (gaps < 1.9)
    f = press(ink.astype(float), soften=2.6, rough=0.06, rough_scale=4, seed=9, wrap=True)
    save_svg(f, "nexus-pattern-bark.svg", T, T, "Nexus pattern: bark", tol=1.0, wrap=True)
    tex = np.clip((noise((T, T), 1.2, 3, wrap=True) * 0.6 + noise((T, T), 50, 4, wrap=True) * 0.5 + 1.25) * 2, 0, 1)
    save_pressed(f, "nexus-pattern-bark", tex)


def contour():
    """Seamless contour-map tile. Thin lines; every fourth is an index line."""
    T = 720
    f = noise((T, T), 85, 12, wrap=True, octaves=3)
    f = np.pad(f, 3, mode="wrap")
    levels = np.linspace(-1.9, 1.9, 17)
    thin, bold = [], []
    for i, lv in enumerate(levels):
        for c in measure.find_contours(f, lv):
            p = measure.approximate_polygon(c, 0.9)[:, ::-1] - 3
            if len(p) < 4:
                continue
            closed = np.allclose(p[0], p[-1])
            n = len(p)
            seg = [f"M{_fmt(p[0][0])} {_fmt(p[0][1])}"]
            for j in range(n - 1):
                p0 = p[j - 1] if (j > 0 or closed) else p[j]
                p1, p2 = p[j], p[j + 1]
                p3 = p[(j + 2) % n] if (j + 2 < n or closed) else p[j + 1]
                if closed and j == 0:
                    p0 = p[-2]
                if closed and j + 2 >= n:
                    p3 = p[1]
                c1, c2 = p1 + (p2 - p0) / 6, p2 - (p3 - p1) / 6
                seg.append("C" + " ".join(_fmt(v) for v in (*c1, *c2, *p2)))
            (bold if i % 4 == 0 else thin).append("".join(seg))
    body = (f'<g fill="none" stroke="currentColor" stroke-linecap="round">'
            f'<path stroke-width="1.1" d="{"".join(thin)}"/><path stroke-width="2.2" d="{"".join(bold)}"/></g>')
    p = os.path.join(OUT, "nexus-pattern-contour.svg")
    with open(p, "w") as fh:
        fh.write(svg(body, T, T, "Nexus pattern: contour"))
    print(f"nexus-pattern-contour.svg: {os.path.getsize(p) / 1024:.0f} KB")


# ---- cut-paper motifs -------------------------------------------------------------------

def rot(x, y, cx, cy, deg):
    a = np.deg2rad(deg)
    dx, dy = x - cx, y - cy
    return dx * np.cos(a) + dy * np.sin(a), -dx * np.sin(a) + dy * np.cos(a)


def lens(u, v, length, width):
    """Leaf shape along +u from the origin."""
    t = np.clip(u / length, 0, 1)
    return (u > 0) & (u < length) & (np.abs(v) < width / 2 * np.sin(np.pi * t) ** 0.8)


def capsule(u, v, x0, y0, x1, y1, w):
    px, py = x1 - x0, y1 - y0
    t = np.clip(((u - x0) * px + (v - y0) * py) / (px * px + py * py), 0, 1)
    return np.hypot(u - x0 - t * px, v - y0 - t * py) < w / 2


def m_pine(u, v, s):
    ink = capsule(u, v, 0, 0.1 * s, 0, 0.5 * s, 0.09 * s)
    for i, (top, bot, half) in enumerate([(-0.50, -0.20, 0.17), (-0.34, 0.02, 0.24), (-0.16, 0.26, 0.31)]):
        t = (v / s - top) / (bot - top)
        ink |= (t > 0) & (t < 1) & (np.abs(u / s) < half * np.clip(t, 0, 1) ** 0.85 + 0.01)
    return ink


def m_sprig(u, v, s):
    ink = capsule(u, v, 0, 0.5 * s, 0, -0.42 * s, 0.05 * s)
    for i, yy in enumerate([0.30, 0.12, -0.06, -0.24]):
        for side in (-1, 1):
            a, b = rot(u, v, 0, yy * s, -90 + side * 52)
            ink |= lens(a, b, (0.30 - 0.03 * i) * s, (0.15 - 0.012 * i) * s)
    a, b = rot(u, v, 0, -0.40 * s, -90)
    ink |= lens(a, b, 0.2 * s, 0.11 * s)
    return ink


def m_sprout(u, v, s):
    ink = capsule(u, v, 0, 0.45 * s, 0, -0.05 * s, 0.07 * s)
    a, b = rot(u, v, 0, -0.02 * s, -150)
    ink |= lens(a, b, 0.42 * s, 0.24 * s)
    a, b = rot(u, v, 0, -0.08 * s, -36)
    ink |= lens(a, b, 0.36 * s, 0.21 * s)
    return ink


def m_slab(u, v, s):
    r = np.hypot(u, v * 1.04) / s * (1 + 0.05 * np.cos(3 * np.arctan2(v, u) + 1))
    ink = r < 0.44
    for c in (0.14, 0.26, 0.36):
        ink &= ~(np.abs(r - c) < 0.026)
    ink |= r < 0.05
    return ink


def m_sun(u, v, s):
    r, th = np.hypot(u, v), np.arctan2(v, u)
    ink = r < 0.19 * s
    ray = (np.abs(((th / (2 * np.pi) * 9) % 1.0) - 0.5) < 0.17) & (r > 0.27 * s) & (r < 0.45 * s)
    return ink | ray


def m_river(u, v, s):
    c = 0.11 * s * np.sin(u / s * 9.5)
    return ((np.abs(v - c) < 0.045 * s) | (np.abs(v - c - 0.17 * s) < 0.045 * s)) & (np.abs(u) < 0.42 * s)


def m_cairn(u, v, s):
    ink = np.zeros_like(u, bool)
    for cy, rx, ry in [(0.30, 0.34, 0.14), (0.04, 0.24, 0.11), (-0.17, 0.16, 0.085), (-0.34, 0.09, 0.06)]:
        ink |= ((u / (rx * s)) ** 2 + ((v - cy * s) / (ry * s)) ** 2) < 1
    return ink


def m_fern(u, v, s):
    bend = 0.10 * s * (v / s) ** 2
    ink = capsule(u - bend, v, 0, 0.5 * s, 0, -0.45 * s, 0.045 * s)
    for i in range(7):
        yy = 0.36 - i * 0.115
        ln = (0.30 - 0.028 * i) * s
        for side in (-1, 1):
            a, b = rot(u - 0.10 * s * yy * yy, v, 0, yy * s, -90 + side * 68)
            ink |= lens(a, b, ln, 0.085 * s)
    return ink


def m_hill(u, v, s):
    ink = np.zeros_like(u, bool)
    for cx, h, w in [(-0.14, 0.66, 0.42), (0.20, 0.44, 0.32)]:
        t = (v / s - (0.34 - h)) / h
        ink |= (t > 0) & (t < 1) & (np.abs(u / s - cx) < w * t)
    snow = (v / s < -0.14) & (np.abs(u / s + 0.14) < 0.42 * ((v / s + 0.32) / 0.66)) & \
           (v / s > -0.19 + 0.035 * np.sin(u / s * 34))
    return ink & ~snow & ~(np.abs(v / s + 0.19 - 0.035 * np.sin(u / s * 34)) < 0.018)


MOTIFS = [m_pine, m_sprig, m_slab, m_sprout, m_sun, m_fern, m_cairn, m_river, m_hill]


def understory():
    """Seamless scatter of cut-paper motifs, brick-laid so no rows line up."""
    T, cells = 720, 4
    c = T / cells
    x, y = grid(T)
    rng = np.random.default_rng(17)
    ink = np.zeros((T, T), bool)
    order = [0, 2, 1, 4, 5, 3, 6, 0, 7, 1, 8, 2, 3, 5, 0, 4]
    for row in range(cells):
        layer = np.zeros((T, T), bool)
        for col in range(cells):
            m = MOTIFS[order[(row * cells + col) % len(order)]]
            cx, cy = (col + 0.5) * c + rng.uniform(-10, 10), (row + 0.5) * c + rng.uniform(-10, 10)
            u, v = rot(x, y, cx, cy, rng.uniform(-14, 14))
            layer |= m(u, v, c * rng.uniform(0.66, 0.80))
        ink |= np.roll(layer, int(c / 2) if row % 2 else 0, axis=1)
    f = press(ink.astype(float), soften=2.2, rough=0.07, rough_scale=4, seed=3, wrap=True)
    save_svg(f, "nexus-pattern-understory.svg", T, T, "Nexus pattern: understory", tol=0.9, wrap=True)


MOTIF_NAMES = ["pine", "sprig", "slab", "sprout", "sun", "fern", "cairn", "river", "hill"]
ILLO = os.path.join(HERE, "..", "illustrations")
# Scenes are coloured by role, not by palette name, so one file serves the light and dark themes:
# each shape reads a --nx-illo-* custom property and falls back to its daylight colour.
ROLE = {"ink": "#2F4631", "moss": "#55693A", "mid": "#A7B48B", "ground": "#DCE5C3", "warm": "#A9502C",
        "sun": "#D8A22B", "water": "#3F6468", "bark": "#6A5038"}


def m_moon(u, v, s):
    """A crescent. The night form of the sun."""
    return (np.hypot(u, v) < 0.30 * s) & ~(np.hypot(u - 0.15 * s, v + 0.10 * s) < 0.27 * s)


def motifs():
    """Each cut-paper motif as its own file, for empty states and small spot use."""
    T = 240
    x, y = grid(T)
    for name, fn in zip(MOTIF_NAMES + ["moon"], MOTIFS + [m_moon]):
        f = press(fn(x - T / 2, y - T / 2, T * 0.86).astype(float), soften=2.4, rough=0.06, rough_scale=4, seed=2)
        d = path_d(trace(f, tol=0.8))
        with open(os.path.join(ILLO, "motifs", f"nexus-motif-{name}.svg"), "w") as fh:
            fh.write(svg(f'<path fill="currentColor" fill-rule="evenodd" d="{d}"/>', T, T, f"Nexus motif: {name}"))
    print(f"{len(MOTIFS) + 1} motifs")


def scene(name, title, layers, w=480, h=320):
    """A small spot illustration: flat cut-paper layers, one colour each, back to front.

    A layer is (role, mask) or (role, mask, "day" | "night"); day and night layers swap with the theme.
    """
    body = ""
    for i, layer in enumerate(layers):
        role, mask, when = (*layer, None)[:3]
        f = press(mask.astype(float), soften=2.4, rough=0.06, rough_scale=4, seed=10 + i)
        show = f";display:var(--nx-illo-{when},{'inline' if when == 'day' else 'none'})" if when else ""
        body += (f'<path style="fill:var(--nx-illo-{role},{ROLE[role]}){show}" fill-rule="evenodd" '
                 f'd="{path_d(trace(f, tol=0.8))}"/>')
    with open(os.path.join(ILLO, name), "w") as fh:
        fh.write(svg(body, w, h, title))
    print(name, f"{os.path.getsize(os.path.join(ILLO, name)) / 1024:.0f} KB")


def scenes():
    W, H = 480, 320
    x, y = grid((W, H))
    ground = lambda y0, amp, ph: y > y0 + amp * np.sin(x / 70 + ph)

    # clear day: nothing on the schedule
    hills = m_hill(x - 250, y - 190, 250)
    scene("nexus-scene-clear-day.svg", "Clear day", [
        ("sun", m_sun(x - 392, y - 78, 120), "day"),
        ("sun", m_moon(x - 392, y - 78, 120), "night"),
        ("mid", hills),
        ("ground", ground(248, 8, 0.5)),
        ("ink", m_pine(x - 96, y - 208, 150) | m_pine(x - 158, y - 226, 104) | m_pine(x - 408, y - 232, 92)),
    ])
    # first shoot: a new record, a first step, setup complete
    scene("nexus-scene-first-shoot.svg", "First shoot", [
        ("warm", m_sun(x - 110, y - 84, 96), "day"),
        ("sun", m_moon(x - 110, y - 84, 100), "night"),
        ("ground", ((x - 240) / 190) ** 2 + ((y - 318) / 78) ** 2 < 1),
        ("ink", m_sprout(x - 240, y - 176, 250)),
        ("mid", m_sprig(x - 372, y - 232, 96) | m_sprig(x - 118, y - 246, 70)),
    ])
    # waymark: nothing found, or a pause; a cairn marks the path
    scene("nexus-scene-waymark.svg", "Waymark", [
        ("ground", ground(262, 6, 1.4)),
        ("water", m_river(x - 110, y - 236, 150)),
        ("mid", m_fern(x - 372, y - 190, 190)),
        ("ink", m_cairn(x - 240, y - 176, 210)),
        ("sun", m_moon(x - 96, y - 72, 84), "night"),
    ])
    # the record: a slab with a shoot, for documentation and history
    scene("nexus-scene-record.svg", "The record", [
        ("ground", ground(256, 5, 2.2)),
        ("ink", m_slab(x - 220, y - 168, 250)),
        ("moss", m_sprout(x - 388, y - 214, 120)),
        ("warm", m_sun(x - 404, y - 70, 76), "day"),
        ("sun", m_moon(x - 404, y - 70, 84), "night"),
    ])


def leaf_vein():
    """The vein that sits under the text of a leaf card.

    Drawn in a 640 x 400 box from the stem (bottom right) to the tip (top left). The file sets
    preserveAspectRatio="none" so it stretches to any card; it is used as a CSS mask in --nx-vein.
    """
    W, H = 640, 400
    x, y = grid((W, H))
    ink = np.zeros((H, W), bool)

    def stroke(pts, w0, w1):
        m = np.zeros((H, W), bool)
        n = len(pts) - 1
        for i in range(n):
            (ax, ay), (bx, by) = pts[i], pts[i + 1]
            if max(ax, bx) < -40 or min(ax, bx) > W + 40 or max(ay, by) < -40 or min(ay, by) > H + 40:
                continue
            m |= capsule(x, y, ax, ay, bx, by, w0 + (w1 - w0) * i / n)
        return m

    stem, tip = np.array([W + 6.0, H + 6.0]), np.array([-4.0, -4.0])
    axis = tip - stem
    L = np.hypot(*axis)
    fwd = axis / L
    nrm = np.array([-fwd[1], fwd[0]])
    mid = lambda t: stem + axis * t + nrm * (26 * np.sin(np.pi * t) + 7 * np.sin(2 * np.pi * t))
    ts = np.linspace(0, 1, 90)
    ink |= stroke([mid(t) for t in ts], 5.2, 1.6)
    rng = np.random.default_rng(31)
    for i, t in enumerate(np.linspace(0.10, 0.90, 11)):
        side = 1 if i % 2 == 0 else -1
        t = t + rng.uniform(-0.012, 0.012)
        p0 = mid(t)
        d = mid(min(t + 0.01, 1)) - mid(max(t - 0.01, 0))
        base = np.arctan2(d[1], d[0])
        reach = (300 if side > 0 else 240) * (1 - 0.55 * t)
        pts, p = [p0], p0.copy()
        for k in range(48):
            ang = base + side * np.deg2rad(52 - 30 * k / 48)       # leaves the midrib wide, then bends toward the tip
            p = p + reach / 48 * np.array([np.cos(ang), np.sin(ang)])
            pts.append(p.copy())
        ink |= stroke(pts, 3.0 - 1.0 * t, 1.0)
    f = press(ink.astype(float), soften=1.3, rough=0.05, rough_scale=4, seed=6)
    d = path_d(trace(f, tol=0.6, min_pts=6))
    path = os.path.join(OUT, "nexus-leaf-vein.svg")
    with open(path, "w") as fh:
        fh.write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" preserveAspectRatio="none">'
                 f'<title>Nexus leaf vein</title><path fill="currentColor" fill-rule="evenodd" d="{d}"/></svg>\n')
    print(f"nexus-leaf-vein.svg: {os.path.getsize(path) / 1024:.0f} KB")


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    ring_press()
    ring_field()
    bark()
    contour()
    understory()
    leaf_vein()
    os.makedirs(os.path.join(ILLO, 'motifs'), exist_ok=True)
    motifs()
    scenes()
