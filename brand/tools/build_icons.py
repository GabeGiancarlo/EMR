"""Draw the Nexus icon set.

Nexus icons are cut, not outlined: each one is a solid block of ink with its detail carved
out, drawn by the same press as the logo. Three rules give the family its look:

  1. Solid shapes, pressed until every corner is round. No hairline outlines.
  2. One carved detail at most. If an icon needs two, it is two icons.
  3. Anything box-shaped is the Nexus tile: three firm corners, one soft corner top right.
  4. Growth shows up where it is the meaning (care plan, edit, settings), not as decoration.

Run:  python3 brand/tools/build_icons.py
Writes brand/icons/svg/<name>.svg, brand/icons/nexus-icons.svg (sprite) and
brand/icons/nexus-icons.ts (path data for the React app; no runtime dependency).
"""
from __future__ import annotations

import json
import os

import numpy as np
from matplotlib.path import Path

from nxgen import path_d, press, trace

P = 32                       # pixels per icon unit while drawing
SOFTEN = 0.26                # how far the press rounds every corner, in icon units
N = 24 * P
_y, _x = np.mgrid[0:N, 0:N]
X, Y = (_x + 0.5) / P, (_y + 0.5) / P
_PTS = np.column_stack([X.ravel(), Y.ravel()])


# ---- primitives, all in 24-unit icon coordinates -------------------------------------------

def disc(cx, cy, r):
    return np.hypot(X - cx, Y - cy) < r


def ring(cx, cy, r, w):
    return np.abs(np.hypot(X - cx, Y - cy) - r) < w / 2


def arc(cx, cy, r, w, a0, a1):
    """Part of a ring between angles a0 and a1 in degrees, clockwise on screen from +x."""
    th = np.rad2deg(np.arctan2(Y - cy, X - cx)) % 360
    a0, a1 = a0 % 360, a1 % 360
    inside = (th >= a0) & (th <= a1) if a0 <= a1 else (th >= a0) | (th <= a1)
    return ring(cx, cy, r, w) & inside


def ellipse(cx, cy, rx, ry):
    return ((X - cx) / rx) ** 2 + ((Y - cy) / ry) ** 2 < 1


def cap(x0, y0, x1, y1, w):
    px, py = x1 - x0, y1 - y0
    L = px * px + py * py
    t = np.clip(((X - x0) * px + (Y - y0) * py) / L, 0, 1) if L else 0
    return np.hypot(X - x0 - t * px, Y - y0 - t * py) < w / 2


def line(pts, w):
    m = np.zeros((N, N), bool)
    for (a, b) in zip(pts, pts[1:]):
        m |= cap(*a, *b, w)
    return m


def poly(pts):
    return Path(pts).contains_points(_PTS).reshape(N, N)


def tile(x0, y0, x1, y1, r=1.5, soft=5.5):
    """The Nexus tile. Top-right corner takes the `soft` radius."""
    m = (X > x0) & (X < x1) & (Y > y0) & (Y < y1)
    for (cx, cy, rr, sx, sy) in [(x0 + r, y0 + r, r, -1, -1), (x0 + r, y1 - r, r, -1, 1), (x1 - r, y1 - r, r, 1, 1),
                                 (x1 - soft, y0 + soft, soft, 1, -1)]:
        corner = ((X - cx) * sx > 0) & ((Y - cy) * sy > 0)
        m &= ~corner | (np.hypot(X - cx, Y - cy) < rr)
    return m


def leaf(bx, by, angle, length, width):
    a = np.deg2rad(angle)
    u = (X - bx) * np.cos(a) + (Y - by) * np.sin(a)
    v = -(X - bx) * np.sin(a) + (Y - by) * np.cos(a)
    t = np.clip(u / length, 0, 1)
    return (u > 0) & (u < length) & (np.abs(v) < width / 2 * np.sin(np.pi * t) ** 0.8)


def grow(m, by):
    from scipy.ndimage import binary_dilation
    r = int(by * P)
    yy, xx = np.mgrid[-r:r + 1, -r:r + 1]
    return binary_dilation(m, structure=(xx * xx + yy * yy) <= r * r)


def dome(cx, base, rx, ry):
    return ellipse(cx, base, rx, ry) & (Y < base)


def person(cx, head_y, hr, base, rx, ry, ringed=True):
    m = disc(cx, head_y, hr) | dome(cx, base, rx, ry)
    if ringed:
        inner = dome(cx, base, rx * 0.60, ry * 0.60) & ~dome(cx, base, rx * 0.60 - 1.5, ry * 0.60 - 1.5)
        m &= ~inner
    return m


C = (12, 12)

# ---- the icons -------------------------------------------------------------------------

def overview():
    return (disc(*C, 9.6) & ~ring(*C, 5.2, 2.1)) | disc(*C, 2.0)



def clients():
    return person(12, 7.0, 3.9, 21.4, 7.8, 9.0, ringed=False)



def group():
    back = person(17.0, 7.6, 3.0, 20.8, 5.6, 7.4, ringed=False)
    front = person(8.8, 8.0, 3.6, 21.4, 6.9, 8.6, ringed=False)
    return (back & ~grow(front, 1.5)) | front



def schedule():
    m = tile(3, 4.8, 21, 21.2)
    m &= ~cap(6.6, 10.2, 17.4, 10.2, 1.7)
    return m | cap(8, 2.9, 8, 6.0, 2.3) | cap(16, 2.9, 16, 6.0, 2.3)



def documentation():
    m = tile(4.5, 2.5, 19.5, 21.5, soft=6.5)
    return m & ~(cap(8.4, 10.6, 15.6, 10.6, 1.7) | cap(8.4, 15.0, 13.0, 15.0, 1.7))



def tasks():
    m = tile(3, 3, 21, 21, soft=6.5)
    m &= ~line([(7.4, 12.4), (10.6, 15.6), (16.8, 8.8)], 2.2)
    return m


def billing():
    m = tile(5, 2.5, 19, 20.2, soft=5)
    for cx in (7.33, 12.0, 16.67):          # a soft scalloped tear along the bottom
        m |= disc(cx, 20.2, 2.33)
    return m & ~(cap(8.8, 8.4, 15.2, 8.4, 1.7) | cap(8.8, 12.8, 12.6, 12.8, 1.7))



def reporting():
    return tile(3.4, 14.2, 8.2, 21.4, r=1.3, soft=2.2) | tile(9.6, 9.4, 14.4, 21.4, r=1.3, soft=2.2) \
        | tile(15.8, 3.6, 20.6, 21.4, r=1.3, soft=2.2)



def settings():
    m = disc(*C, 6.2)
    for k in range(6):
        a = np.deg2rad(k * 60 + 30)
        m |= disc(12 + 6.9 * np.cos(a), 12 + 6.9 * np.sin(a), 3.0)
    return m & ~disc(*C, 3.0)



def search():
    return ring(10.4, 10.4, 6.3, 2.5) | cap(15.6, 15.6, 20.3, 20.3, 2.8)



def add():
    return cap(12, 4.4, 12, 19.6, 2.7) | cap(4.4, 12, 19.6, 12, 2.7)


def close():
    return cap(6, 6, 18, 18, 2.7) | cap(18, 6, 6, 18, 2.7)


def check():
    return line([(4.6, 12.6), (9.7, 17.7), (19.4, 6.8)], 2.9)


def edit():
    return leaf(4.0, 20.0, -45, 22.6, 9.4) & ~cap(8.4, 15.6, 14.6, 9.4, 1.5)



def warning():
    m = grow(poly([(12, 4.6), (20.2, 19.2), (3.8, 19.2)]), 1.4)
    return m & ~(cap(12, 9.4, 12, 13.8, 2.0) | disc(12, 17.0, 1.25))


def error():
    return disc(*C, 9.7) & ~(cap(8.5, 8.5, 15.5, 15.5, 2.1) | cap(15.5, 8.5, 8.5, 15.5, 2.1))


def info():
    return disc(*C, 9.7) & ~(disc(12, 7.5, 1.35) | cap(12, 11.3, 12, 16.9, 2.1))


def success():
    return disc(*C, 9.7) & ~line([(7.4, 12.3), (10.6, 15.5), (16.7, 8.9)], 2.2)


def lock():
    m = tile(4.5, 10.6, 19.5, 21.5, soft=5)
    m |= ring(12, 9.2, 4.5, 2.3) & (Y < 10.8)
    m |= cap(7.5, 9.2, 7.5, 10.8, 2.3) | cap(16.5, 9.2, 16.5, 10.8, 2.3)
    return m & ~(disc(12, 15.0, 1.7) | cap(12, 15.2, 12, 18.2, 1.5))


def message():
    m = tile(3, 3.6, 21, 17.2, soft=6) | poly([(6, 16.4), (6, 21.6), (11.6, 16.4)])
    return m & ~cap(8.0, 10.4, 16.0, 10.4, 1.8)



def clock():
    return disc(*C, 9.7) & ~(cap(12, 6.6, 12, 12, 2.0) | cap(12, 12, 15.9, 14.4, 2.0))


def history():
    m = arc(12.6, 12, 7.5, 2.5, -62, 210)
    m |= poly([(1.8, 8.4), (9.0, 8.4), (5.4, 13.8)])
    return m | line([(12.6, 8.2), (12.6, 12.2), (15.4, 13.9)], 2.1)



def filter_():
    return cap(3.6, 6.4, 20.4, 6.4, 2.7) | cap(6.6, 12, 17.4, 12, 2.7) | cap(9.8, 17.6, 14.2, 17.6, 2.7)


def more():
    return disc(5, 12, 1.95) | disc(12, 12, 1.95) | disc(19, 12, 1.95)


def chevron(d):
    pts = {"down": [(5.8, 9), (12, 15.2), (18.2, 9)], "up": [(5.8, 15), (12, 8.8), (18.2, 15)],
           "left": [(15, 5.8), (8.8, 12), (15, 18.2)], "right": [(9, 5.8), (15.2, 12), (9, 18.2)]}[d]
    return lambda: line(pts, 2.7)


def arrow_right():
    return cap(3.8, 12, 19.8, 12, 2.7) | line([(13.4, 5.6), (19.9, 12), (13.4, 18.4)], 2.7)


def arrow_left():
    return cap(4.2, 12, 20.2, 12, 2.7) | line([(10.6, 5.6), (4.1, 12), (10.6, 18.4)], 2.7)


def telehealth():
    return tile(2.4, 6, 15.4, 18, soft=4.8) | grow(poly([(18.4, 12), (20.6, 9.4), (20.6, 14.6)]), 1.2)


def care_plan():
    m = cap(4.4, 20.8, 19.6, 20.8, 2.3) | cap(12, 20.4, 12, 9.8, 2.1)
    return m | leaf(12.2, 14.2, -157, 9.2, 5.4) | leaf(11.8, 11.4, -30, 8.6, 5.0)


def download():
    return cap(12, 3.6, 12, 14.4, 2.7) | line([(6.6, 9.8), (12, 15.2), (17.4, 9.8)], 2.7) | cap(4.4, 20.3, 19.6, 20.3, 2.7)


def notification():
    m = dome(12, 12.0, 6.6, 8.2) | ((X > 5.4) & (X < 18.6) & (Y >= 11.9) & (Y < 16.4))
    m |= tile(3.6, 15.0, 20.4, 18.2, r=1.5, soft=1.5)
    return m | disc(12, 20.6, 1.9)



def sun():
    m = disc(*C, 4.5)
    for k in range(8):
        a = np.deg2rad(k * 45)
        m |= cap(12 + 7.4 * np.cos(a), 12 + 7.4 * np.sin(a), 12 + 9.8 * np.cos(a), 12 + 9.8 * np.sin(a), 2.1)
    return m


def moon():
    return disc(11.4, 12.4, 9.2) & ~disc(15.8, 8.2, 7.6)


def logout():
    m = line([(10.6, 4.3), (5.2, 4.3), (5.2, 19.7), (10.6, 19.7)], 2.5)
    return m | cap(10.4, 12, 20.4, 12, 2.5) | line([(16.4, 7.9), (20.5, 12), (16.4, 16.1)], 2.5)


def mail():
    return tile(2.5, 5, 21.5, 19, soft=5) & ~line([(6.0, 9.0), (12, 13.6), (18.0, 9.0)], 1.7)


def refresh():
    m = arc(12, 12.6, 7.4, 2.5, -15, 262)
    return m | poly([(11.2, 1.4), (11.2, 9.0), (17.0, 5.2)])


def claim():
    return tile(4.5, 2.5, 19.5, 21.5, soft=6.5) & ~line([(8.0, 12.8), (10.8, 15.6), (16.0, 9.6)], 1.9)


def payment():
    return tile(2.5, 5, 21.5, 19, soft=5) & ~cap(5.8, 10.2, 18.2, 10.2, 2.0)



def microphone():
    m = cap(12, 5.2, 12, 11.2, 5.4) | (ring(12, 11.2, 6.5, 2.1) & (Y > 11.2))
    return m | cap(12, 17.6, 12, 20.6, 2.1) | cap(8.4, 20.9, 15.6, 20.9, 2.1)


def visible():
    return (leaf(1.6, 12, 0, 20.8, 12.4) & ~disc(*C, 3.7)) | disc(*C, 1.6)


def hidden():
    m = visible() & ~cap(4.2, 19.8, 19.8, 4.2, 4.6)
    return m | cap(4.6, 19.4, 19.4, 4.6, 2.0)


def consent():
    m = tile(4.5, 2.5, 19.5, 21.5, soft=6.5)
    y = 13.4 + 1.9 * np.sin((X - 7.6) * 0.95)
    return m & ~((np.abs(Y - y) < 0.95) & (X > 7.9) & (X < 16.1))



def encounter():
    a = tile(2.6, 3.4, 15.6, 13.6, soft=4.6)
    b = tile(8.4, 10.4, 21.4, 20.6, soft=4.6)
    return (a & ~grow(b, 1.4)) | b


ICONS = {
    "overview": overview, "clients": clients, "group": group, "schedule": schedule,
    "documentation": documentation, "tasks": tasks, "billing": billing, "reporting": reporting,
    "settings": settings, "care-plan": care_plan, "telehealth": telehealth, "encounter": encounter,
    "claim": claim, "payment": payment, "consent": consent, "message": message, "mail": mail,
    "notification": notification, "microphone": microphone, "search": search, "filter": filter_,
    "add": add, "edit": edit, "close": close, "check": check, "more": more, "download": download,
    "refresh": refresh, "history": history, "clock": clock, "lock": lock, "visible": visible, "hidden": hidden,
    "success": success, "warning": warning, "error": error, "info": info,
    "chevron-down": chevron("down"), "chevron-up": chevron("up"), "chevron-left": chevron("left"),
    "chevron-right": chevron("right"), "arrow-left": arrow_left, "arrow-right": arrow_right,
    "sun": sun, "moon": moon, "logout": logout,
}


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    out = os.path.join(here, "..", "icons")
    os.makedirs(os.path.join(out, "svg"), exist_ok=True)
    paths = {}
    for name, fn in ICONS.items():
        f = press(fn().astype(float), soften=SOFTEN * P, rough=0.0)
        d = path_d(trace(f, tol=1.0), scale=1 / P, prec=2)
        paths[name] = d
        with open(os.path.join(out, "svg", f"{name}.svg"), "w") as fh:
            fh.write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">'
                     f'<path fill-rule="evenodd" d="{d}"/></svg>\n')
    with open(os.path.join(out, "nexus-icons.svg"), "w") as fh:
        fh.write('<svg xmlns="http://www.w3.org/2000/svg" style="display:none">'
                 + "".join(f'<symbol id="nx-{n}" viewBox="0 0 24 24"><path fill-rule="evenodd" d="{d}"/></symbol>'
                           for n, d in paths.items()) + "</svg>\n")
    with open(os.path.join(out, "nexus-icons.ts"), "w") as fh:
        fh.write("// Generated by brand/tools/build_icons.py. Do not edit by hand.\n"
                 "// Render with: <svg viewBox=\"0 0 24 24\" fill=\"currentColor\"><path fillRule=\"evenodd\" d={nexusIcons[name]} /></svg>\n"
                 "export const nexusIcons = " + json.dumps(paths, indent=2) + " as const;\n\n"
                 "export type NexusIconName = keyof typeof nexusIcons;\n")
    total = sum(len(d) for d in paths.values())
    print(f"{len(paths)} icons, {total / 1024:.0f} KB of path data, avg {total / len(paths):.0f} bytes")


if __name__ == "__main__":
    main()
