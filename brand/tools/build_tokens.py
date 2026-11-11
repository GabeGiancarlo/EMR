"""Write the Nexus design tokens (CSS, JSON, TypeScript) and audit their contrast.

One source of truth, below. Run:  python3 brand/tools/build_tokens.py
"""
from __future__ import annotations

import csv
import json
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tokens")

# ---- the palette: every colour is named for something on the forest floor ----------------
PALETTE = {
    "linen": "#F4EFE3", "birch": "#FCF9F0", "oat": "#EAE3D2", "reed": "#CFC8B2",
    "heartwood": "#232B1E", "understory": "#565E4C",
    "pine": "#2F4631", "moss": "#55693A", "sage": "#A7B48B", "lichen": "#DCE5C3",
    "clay": "#A9502C", "pollen": "#D8A22B", "bark": "#6A5038", "creek": "#3F6468",
    "night": "#161A13", "loam": "#1F251B", "duff": "#283024",
}

LIGHT = {
    "page": "#F4EFE3", "surface": "#FCF9F0", "surface-subtle": "#EAE3D2", "navigation": "#EDE7D7",
    "text": "#232B1E", "text-secondary": "#565E4C", "text-on-brand": "#F4EFE3",
    "border": "#CFC8B2", "border-control": "#7A8066",
    "brand": "#2F4631", "brand-soft": "#DCE5C3", "brand-mid": "#A7B48B",
    "action": "#2F4631", "action-hover": "#263A29", "action-pressed": "#1D2D20", "on-action": "#F4EFE3",
    "selected": "#DCE5C3", "on-selected": "#263A29",
    "link": "#3D5A2C", "link-visited": "#6A5038", "focus": "#A9502C",
    "accent": "#A9502C", "accent-soft": "#F3DDCF", "highlight": "#D8A22B",
    "success": "#2C6947", "success-bg": "#DCEBDA", "warning": "#77520F", "warning-bg": "#F5E4B5",
    "danger": "#9E3926", "danger-bg": "#F7DFD6", "danger-fill": "#9E3926", "on-danger": "#F4EFE3", "info": "#3F6468", "info-bg": "#DDE9E6",
    "pattern-ink": "#D9D2BD", "vein": "#2F463116", "vein-strong": "#2F463126",
    # illustration roles: scenes are drawn with these, so one file serves both themes
    "illo-ink": "#2F4631", "illo-moss": "#55693A", "illo-mid": "#A7B48B", "illo-ground": "#DCE5C3",
    "illo-warm": "#A9502C", "illo-sun": "#D8A22B", "illo-water": "#3F6468", "illo-bark": "#6A5038",
    "illo-day": "inline", "illo-night": "none",
}

DARK = {
    "page": "#161A13", "surface": "#1F251B", "surface-subtle": "#283024", "navigation": "#1A1F16",
    "text": "#EEEDE0", "text-secondary": "#BFC8AC", "text-on-brand": "#161A13",
    "border": "#3C4432", "border-control": "#828D6D",
    "brand": "#DCE5C3", "brand-soft": "#2C3826", "brand-mid": "#6E7D56",
    "action": "#BECB9B", "action-hover": "#CDD9AC", "action-pressed": "#AEBC8A", "on-action": "#161A13",
    "selected": "#313D29", "on-selected": "#E4ECCF",
    "link": "#C6D4A2", "link-visited": "#C9AE92", "focus": "#EDB07E",
    "accent": "#E59A72", "accent-soft": "#43291D", "highlight": "#E3B955",
    "success": "#A6D2A6", "success-bg": "#22362A", "warning": "#E6C27A", "warning-bg": "#3B3119",
    "danger": "#EEA08F", "danger-bg": "#43261F", "danger-fill": "#A8432F", "on-danger": "#FBF3EA", "info": "#A5CDCF", "info-bg": "#203335",
    "pattern-ink": "#2A3225", "vein": "#DCE5C312", "vein-strong": "#DCE5C320",
    "illo-ink": "#B9C69D", "illo-moss": "#8FA06E", "illo-mid": "#56664A", "illo-ground": "#2C3826",
    "illo-warm": "#E59A72", "illo-sun": "#EADDAE", "illo-water": "#7FA9AB", "illo-bark": "#C9AE92",
    "illo-day": "none", "illo-night": "inline",
}

SCALE = {
    "space-1": "4px", "space-2": "8px", "space-3": "12px", "space-4": "16px", "space-5": "20px",
    "space-6": "24px", "space-8": "32px", "space-12": "48px", "space-16": "64px",
    "font-display": '"Fraunces", "Iowan Old Style", Georgia, serif',
    "font-ui": '"Source Sans 3", -apple-system, "Segoe UI", system-ui, sans-serif',
    "font-figure": '"Manrope", "Source Sans 3", system-ui, sans-serif',
    "font-mono": 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
    "text-display": "600 40px/1.1", "text-title": "600 28px/1.2", "text-heading": "600 20px/1.3",
    "text-subheading": "600 16px/1.35", "text-body": "400 15px/1.5", "text-dense": "400 14px/1.4",
    "text-small": "400 13px/1.4", "text-label": "600 11px/1.3",
    "text-figure-hero": "800 56px/1", "text-figure": "800 30px/1.1", "text-figure-small": "700 18px/1.2",
    "tracking-label": "0.08em", "tracking-display": "-0.015em", "tracking-figure": "-0.03em",
    "radius-control": "3px", "radius-panel": "4px", "radius-overlay": "6px",
    # one soft corner, top right
    "corner-control": "3px 12px 3px 3px", "corner-control-hover": "3px 17px 3px 3px",
    "corner-tile": "4px 22px 4px 4px", "corner-hero": "6px 56px 6px 6px",
    # the leaf: tip top left, stem bottom right, soft shoulders on the other diagonal
    "corner-leaf": "2px 26px 2px 26px", "corner-leaf-small": "2px 16px 2px 16px", "corner-leaf-large": "3px 48px 3px 48px",
    "corner-card": "2px 26px 2px 26px",
    "control-height": "32px", "control-height-touch": "40px", "row-height": "36px",
    "motion-press": "120ms", "motion-feedback": "160ms", "motion-disclosure": "200ms", "motion-grow": "360ms",
    "ease": "cubic-bezier(0.2, 0, 0, 1)",
}

CHART = ["#2F4631", "#A9502C", "#D8A22B", "#3F6468", "#6A5038", "#A7B48B"]
CHART_DARK = ["#BECB9B", "#E59A72", "#E3B955", "#A5CDCF", "#C9AE92", "#6E7D56"]

# (foreground role, background role, minimum ratio, what it is for)
PAIRS = [
    ("text", "page", 7, "body text on the page"), ("text", "surface", 7, "body text on a card"),
    ("text", "surface-subtle", 7, "body text on a subtle panel"), ("text", "navigation", 7, "navigation labels"),
    ("text-secondary", "page", 4.5, "secondary text on the page"), ("text-secondary", "surface", 4.5, "secondary text on a card"),
    ("text-secondary", "surface-subtle", 4.5, "secondary text on a subtle panel"),
    ("on-action", "action", 4.5, "primary button label"), ("on-action", "action-hover", 4.5, "primary button, hover"),
    ("on-action", "action-pressed", 4.5, "primary button, pressed"),
    ("on-danger", "danger-fill", 4.5, "destructive button label"),
    ("on-selected", "selected", 4.5, "selected row or nav item"), ("link", "page", 4.5, "link on the page"),
    ("link", "surface", 4.5, "link on a card"), ("link-visited", "page", 4.5, "visited link on the page"),
    ("link-visited", "surface", 4.5, "visited link on a card"), ("brand", "page", 4.5, "logo and brand headings"),
    ("brand", "surface", 4.5, "logo on a card"),
    ("border-control", "surface", 3, "field outline on a card"), ("border-control", "page", 3, "field outline on the page"),
    ("focus", "page", 3, "focus ring on the page"), ("focus", "surface", 3, "focus ring on a card"),
    ("accent", "page", 3, "accent mark or icon"), ("accent", "accent-soft", 3, "accent on its tint"),
    ("success", "success-bg", 4.5, "success message"), ("warning", "warning-bg", 4.5, "warning message"),
    ("danger", "danger-bg", 4.5, "error message"), ("info", "info-bg", 4.5, "information message"),
    ("success", "surface", 4.5, "success text on a card"), ("warning", "surface", 4.5, "warning text on a card"),
    ("danger", "surface", 4.5, "error text on a card"), ("info", "surface", 4.5, "information text on a card"),
    ("text", "vein@surface", 7, "body text over a leaf vein"), ("text-secondary", "vein@surface", 4.5, "secondary text over a leaf vein"),
    ("text", "vein@selected", 7, "body text over a vein on a selected card"),
    ("text", "vein-strong@surface", 7, "body text over a vein, card hovered"),
]


def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    c = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def over(top, base):
    """An 8-digit #RRGGBBAA colour laid over an opaque one."""
    if len(top) != 9:
        return top
    a = int(top[7:9], 16) / 255
    return "#" + "".join(f"{round(int(top[i:i + 2], 16) * a + int(base[i:i + 2], 16) * (1 - a)):02X}" for i in (1, 3, 5))


def resolve(t, role):
    """A role's colour; 'vein@surface' means the vein colour laid over the surface colour."""
    if "@" in role:
        top, base = role.split("@")
        return over(t[top], t[base])
    return t[role]


def ratio(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def main():
    os.makedirs(OUT, exist_ok=True)
    rows, fails = [], 0
    for theme, t in (("light", LIGHT), ("dark", DARK)):
        for fg, bg, need, use in PAIRS:
            f_hex, b_hex = resolve(t, fg), resolve(t, bg)
            r = ratio(f_hex, b_hex)
            ok = r >= need
            fails += not ok
            rows.append([theme, fg, f_hex, bg, b_hex, f"{r:.2f}", need, "pass" if ok else "FAIL", use])
    with open(os.path.join(OUT, "contrast-audit.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["theme", "foreground", "hex", "background", "hex", "ratio", "required", "result", "use"])
        w.writerows(rows)

    def block(t):
        return "".join(f"  --nx-{k}: {v};\n" for k, v in t.items())

    css = ("/* Nexus design tokens v0.3. Generated by brand/tools/build_tokens.py. Do not edit by hand. */\n"
           ":root {\n" + "".join(f"  --nx-{k}: {v};\n" for k, v in PALETTE.items()) + "\n" + block(LIGHT) + "\n"
           + "".join(f"  --nx-{k}: {v};\n" for k, v in SCALE.items())
           + "".join(f"  --nx-chart-{i + 1}: {c};\n" for i, c in enumerate(CHART)) + "}\n\n"
           '[data-nexus-theme="dark"] {\n' + block(DARK)
           + "".join(f"  --nx-chart-{i + 1}: {c};\n" for i, c in enumerate(CHART_DARK)) + "}\n\n"
           "/* Display type: Fraunces, cut soft (see brand/fonts). */\n"
           ".nx-display { font-family: var(--nx-font-display); letter-spacing: var(--nx-tracking-display); }\n"
           "/* Figures: Manrope, lining and tabular. */\n"
           ".nx-figure { font-family: var(--nx-font-figure); font-variant-numeric: lining-nums tabular-nums;\n"
           "  letter-spacing: var(--nx-tracking-figure); }\n"
           "/* One soft corner, top right: primary buttons, badges, tiles. */\n"
           ".nx-corner-control { border-radius: var(--nx-corner-control); }\n"
           ".nx-corner-tile { border-radius: var(--nx-corner-tile); }\n"
           "/* The leaf card: see brand/guidelines for the vein and stem. */\n"
           ".nx-corner-leaf { border-radius: var(--nx-corner-leaf); }\n"
           ".nx-focus:focus-visible { outline: 2px solid var(--nx-focus); outline-offset: 2px; }\n"
           "@media (prefers-reduced-motion: reduce) {\n"
           "  :root { --nx-motion-press: 0ms; --nx-motion-feedback: 0ms; --nx-motion-disclosure: 0ms; }\n}\n")
    open(os.path.join(OUT, "nexus-tokens.css"), "w").write(css)
    data = {"version": "0.3", "palette": PALETTE, "light": LIGHT, "dark": DARK, "scale": SCALE,
            "chart": {"light": CHART, "dark": CHART_DARK}}
    open(os.path.join(OUT, "nexus-tokens.json"), "w").write(json.dumps(data, indent=2) + "\n")
    open(os.path.join(OUT, "nexus-tokens.ts"), "w").write(
        "// Nexus design tokens v0.3. Generated by brand/tools/build_tokens.py. Do not edit by hand.\n"
        "export const nexusTokens = " + json.dumps(data, indent=2) + " as const;\n")
    print(f"{len(rows)} contrast pairs checked, {fails} failing")
    for r in rows:
        if r[7] == "FAIL":
            print("  FAIL", r[0], r[1], "on", r[3], r[5], "needs", r[6])


if __name__ == "__main__":
    main()
