"""Assemble the Nexus brand guidelines from the real assets and tokens.

Reads brand/guidelines/template.html (plus the screen-*.html partials), fills in the logo,
icons, patterns and token values straight from the generated files, and writes:

  brand/guidelines/index.html              self-contained page, opens in any browser
  brand/Nexus-Brand-Guidelines-v0.3.pdf    printed from that page with headless Chrome

Run:  python3 brand/tools/build_guidelines.py [--pages DIR]
With --pages, also writes one HTML file per page into DIR for visual review.
"""
from __future__ import annotations

import base64
import csv
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BRAND = os.path.normpath(os.path.join(HERE, ".."))
G = os.path.join(BRAND, "guidelines")
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

TOK = json.load(open(os.path.join(BRAND, "tokens", "nexus-tokens.json")))
ICONS = json.loads(re.search(r"= (\{.*\}) as const", open(os.path.join(BRAND, "icons", "nexus-icons.ts")).read(), re.S).group(1))

PALETTE_NOTES = {
    "linen": "The page. Warm, never white.", "birch": "Cards and fields.", "oat": "Quiet panels, navigation.",
    "reed": "Hairlines and dividers.", "heartwood": "All body text.", "understory": "Secondary text.",
    "pine": "The brand ink: logo, headings, primary actions.", "moss": "Second green, illustration.",
    "sage": "Soft fills, illustration.", "lichen": "Selected rows, tints.",
    "clay": "The one warm note: focus, accents, small highlights.", "pollen": "Illustration and charts only.",
    "bark": "Illustration and charts only.", "creek": "Information messages, charts.",
    "night": "Dark page.", "loam": "Dark cards.", "duff": "Dark panels.",
}


def rgb(h):
    return " ".join(str(int(h[i:i + 2], 16)) for i in (1, 3, 5))


def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    c = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def m_svg(arg):
    path, _, attrs = arg.strip().partition(" ")
    s = open(os.path.join(BRAND, path)).read()
    s = re.sub(r"<title>.*?</title>|<desc>.*?</desc>", "", s, flags=re.S).strip()
    return s.replace("<svg ", f"<svg {attrs} ", 1) if attrs else s


def m_ic(arg):
    name, _, attrs = arg.strip().partition(" ")
    return f'<svg class="ic" {attrs} viewBox="0 0 24 24" aria-hidden="true"><path fill-rule="evenodd" d="{ICONS[name]}"/></svg>'


def m_uri(arg):
    path, colour = arg.split()
    s = open(os.path.join(BRAND, path)).read().replace("currentColor", colour)
    return "url('data:image/svg+xml;base64," + base64.b64encode(s.encode()).decode() + "')"


def m_datauri(arg):
    path = arg.strip()
    return "url('data:image/png;base64," + base64.b64encode(open(os.path.join(BRAND, path), "rb").read()).decode() + "')"


def m_motifs(_):
    names = ["pine", "sprig", "slab", "sprout", "sun", "fern", "cairn", "river", "hill"]
    return "".join(f'<div class="icell big">{m_svg(f"illustrations/motifs/nexus-motif-{n}.svg")}<span>{n}</span></div>' for n in names)


def m_tokens(_):
    return open(os.path.join(BRAND, "tokens", "nexus-tokens.css")).read()


def m_iconsgrid(_):
    return "".join(f'<div class="icell">{m_ic(n)}<span>{n}</span></div>' for n in ICONS)


def m_iconrow(arg):
    return "".join(m_ic(n) for n in (arg.split() or ICONS))


def m_swatches(arg):
    out = ""
    for n in arg.split():
        h = TOK["palette"][n]
        fg = "#232B1E" if lum(h) > 0.4 else "#F4EFE3"
        out += (f'<div class="sw"><div class="chip" style="background:{h};color:{fg}">{n.capitalize()}</div>'
                f'<div class="meta"><b>{h}</b><br>RGB {rgb(h)}<br><i>{PALETTE_NOTES[n]}</i></div></div>')
    return out


def m_roles(arg):
    theme = arg.strip()
    t = TOK[theme]
    groups = [("Surfaces", ["page", "surface", "surface-subtle", "navigation"]),
              ("Text and lines", ["text", "text-secondary", "border", "border-control"]),
              ("Brand and action", ["brand", "action", "action-hover", "selected", "link", "link-visited", "focus", "accent"]),
              ("Status", ["success", "warning", "danger", "info"])]
    out = ""
    for title, keys in groups:
        out += f'<div class="rg"><h3>{title}</h3>'
        for k in keys:
            extra = f'<span class="dot" style="background:{t[k + "-bg"]}"></span>' if k + "-bg" in t else ""
            out += f'<div class="role"><span class="dot" style="background:{t[k]}"></span>{extra}<code>--nx-{k}</code><em>{t[k]}</em></div>'
        out += "</div>"
    return out


def m_contrast(arg):
    want = {a.strip() for a in arg.split("|")} if arg.strip() else None
    rows = list(csv.DictReader(open(os.path.join(BRAND, "tokens", "contrast-audit.csv"))))
    by = {}
    for r in rows:
        by.setdefault(r["use"], {})[r["theme"]] = r
    out = ""
    for use, d in by.items():
        if want and use not in want:
            continue
        out += (f'<tr><td>{use}</td><td class="num">{d["light"]["ratio"]}</td><td class="num">{d["dark"]["ratio"]}</td>'
                f'<td class="num">{d["light"]["required"]}:1</td></tr>')
    return out


def m_screen(arg):
    return expand(open(os.path.join(G, f"screen-{arg.strip()}.html")).read())


TEMPLATE = open(os.path.join(G, "template.html")).read()
# every page is one <section class="page" id=...>; data-sec and data-toc put it in the contents
SECTIONS = [dict(re.findall(r'([\w-]+)="([^"]*)"', tag)) for tag in re.findall(r'<section class="page[^>]*>', TEMPLATE)]
PAGE = {sec["id"]: n for n, sec in enumerate(SECTIONS, 1) if "id" in sec}

# index: term -> the pages (by section id) where it is defined or shown
INDEX = {
    "Accent colour (Clay)": ["colour", "colour-roles"], "App icon and favicon": ["logo-family", "applications"],
    "Appendix slides": ["teaching"], "Asset library": ["assets"], "Audit, contrast": ["colour-roles"],
    "Badges": ["icon-use", "components"], "Bark (logo layer)": ["symbol"], "Bark (pattern)": ["patterns"],
    "Billing workspace": ["billing"], "Business card": ["applications"], "Buttons": ["components", "interaction"],
    "Cards": ["leaf"], "Checkboxes and toggles": ["components"], "Clear space": ["logo-use"],
    "Client (the word)": ["voice"], "Clinical workspace": ["clinical", "clinical-dark"], "Colour roles": ["colour-roles"],
    "Colourways, logo": ["logo-family"], "Components": ["components"], "Contour (pattern)": ["patterns"],
    "Contrast": ["colour-roles"], "Convergence": ["symbol"], "Corners": ["leaf", "corner"],
    "Dark theme": ["colour-roles", "clinical-dark", "illustration"], "Density": ["corner"], "Documents, exported": ["applications"],
    "Empty states": ["illustration-use"], "Errors, wording": ["voice", "illustration-use"],
    "Fields": ["components"], "Figures and numbers": ["type-scale"], "Focus ring": ["components", "interaction"],
    "Fonts": ["type", "assets"], "Fraunces": ["type", "type-scale"],
    "Hero banner": ["corner"], "Hover states": ["interaction"],
    "Icons": ["icons", "icon-use"], "Illustration": ["illustration", "illustration-use"],
    "Leaf card": ["leaf", "interaction"], "Links, visited": ["interaction", "colour-roles"], "Logo": ["logo-family", "logo-use"],
    "Manrope": ["type", "type-scale"], "Misuse, logo": ["logo-use"], "Motifs": ["illustration"], "Motion": ["interaction"],
    "Navigation": ["clinical", "interaction"], "Night scenes": ["illustration"],
    "Palette": ["colour"], "Patterns": ["ring-press", "patterns"], "Pith": ["symbol"], "Profile and sign-out": ["clinical"],
    "Questions, in teaching": ["teaching"], "React library": ["assets"], "Ring press": ["ring-press"], "Rings": ["symbol"],
    "Session (the word)": ["voice"], "Sign-in screen": ["applications"], "Slides": ["teaching", "applications"],
    "Small-size symbol": ["logo-family"], "Soft corner": ["corner"], "Source Sans 3": ["type", "type-scale"],
    "Spacing": ["corner"], "Status": ["icon-use", "components"], "Stem": ["leaf"], "Symbol": ["symbol", "logo-family"],
    "Tables": ["billing", "type-scale"], "Tabs": ["components"], "Teaching materials": ["teaching"], "Toasts": ["components", "illustration-use"],
    "Tokens": ["colour-roles", "assets"], "Type scale": ["type-scale"], "Understory (pattern)": ["patterns"],
    "Vein": ["leaf"], "Voice": ["voice"], "Wordmark": ["logo-family"], "Zoom, slides for": ["teaching"],
}


def m_toc(_):
    out, last = "", None
    for sec in SECTIONS:
        if "data-toc" not in sec:
            continue
        if sec["data-sec"] != last:
            last = sec["data-sec"]
            out += f'<div class="toc-h">{last}</div>'
        out += f'<a class="toc-r" href="#{sec["id"]}"><span>{sec["data-toc"]}</span><i></i><b>{PAGE[sec["id"]]}</b></a>'
    return out


def m_vein(arg):
    """The leaf vein as a ready-coloured background image: {{vein light}} or {{vein dark strong}}."""
    theme, _, strong = arg.strip().partition(" ")
    return m_uri("patterns/nexus-leaf-vein.svg " + TOK[theme]["vein-strong" if strong else "vein"])


def m_tint(arg):
    """A pressed print (alpha-mask PNG) coloured and inlined: {{tint patterns/x-pressed-mask.png #3F5A41}}.

    The guide draws prints as ordinary images, not CSS masks: some PDF viewers drop large masks.
    """
    import io
    from PIL import Image
    path, colour = arg.split()
    im = Image.open(os.path.join(BRAND, path)).convert("RGBA")
    rgb = tuple(int(colour[i:i + 2], 16) for i in (1, 3, 5))
    out = Image.new("RGBA", im.size, rgb + (0,))
    out.putalpha(im.getchannel("A"))
    buf = io.BytesIO()
    out.save(buf, "PNG", optimize=True)
    return "url('data:image/png;base64," + base64.b64encode(buf.getvalue()).decode() + "')"


PATTERNS = {}
for _name in ("bark", "contour", "understory"):
    _svg = open(os.path.join(BRAND, "patterns", f"nexus-pattern-{_name}.svg")).read()
    _size = float(re.search(r'viewBox="0 0 ([\d.]+)', _svg).group(1))
    _body = re.sub(r"<title>.*?</title>|<desc>.*?</desc>", "", re.search(r"<svg[^>]*>(.*)</svg>", _svg, re.S).group(1), flags=re.S)
    PATTERNS[_name] = (_size, _body)
# each tile is defined once, hidden, and drawn wherever a pattern is needed
PATTERN_DEFS = ('<svg width="0" height="0" style="position:absolute" aria-hidden="true"><defs>'
                + "".join(f'<g id="nxp-{n}">{body}</g>' for n, (_, body) in PATTERNS.items()) + "</defs></svg>\n")


def m_pat(arg):
    """A seamless pattern as real vector tiles: {{pat bark #3F5A41 330 [x y]}}.

    Fills a parent that is position:relative and overflow:hidden. Drawn as SVG, not as a CSS
    background or mask, because printed PDFs blur or drop those.
    """
    name, colour, tile, *off = arg.split()
    tile = float(tile)
    ox, oy = (float(v) for v in off) if off else (0.0, 0.0)
    ox, oy = (ox % tile) - (tile if ox % tile else 0), (oy % tile) - (tile if oy % tile else 0)
    size = PATTERNS[name][0]
    cols, rows = int((760 - ox) // tile) + 1, int((440 - oy) // tile) + 1
    uses = "".join(f'<use href="#nxp-{name}" x="{c * size:g}" y="{r * size:g}"/>' for r in range(rows) for c in range(cols))
    return (f'<svg aria-hidden="true" style="position:absolute;left:{ox:g}px;top:{oy:g}px;width:{cols * tile:g}px;height:{rows * tile:g}px;'
            f'max-width:none;color:{colour};pointer-events:none" viewBox="0 0 {cols * size:g} {rows * size:g}">{uses}</svg>')


def m_pg(arg):
    return str(PAGE[arg.strip()])


def m_index(_):
    out, letter = "", ""
    for term in sorted(INDEX, key=str.lower):
        if term[0].upper() != letter:
            letter = term[0].upper()
            out += f'<div class="ix-l">{letter}</div>'
        pages = ", ".join(f'<a href="#{i}">{PAGE[i]}</a>' for i in sorted(set(INDEX[term]), key=PAGE.get))
        out += f'<div class="ix-r"><span>{term}</span><b>{pages}</b></div>'
    return out


MACROS = {"pat": m_pat, "tint": m_tint, "vein": m_vein, "toc": m_toc, "pg": m_pg, "index": m_index, "svg": m_svg, "ic": m_ic, "uri": m_uri, "tokens": m_tokens, "iconsgrid": m_iconsgrid, "iconrow": m_iconrow,
          "swatches": m_swatches, "roles": m_roles, "contrast": m_contrast, "screen": m_screen,
          "iconcount": lambda _: str(len(ICONS)), "datauri": m_datauri, "motifs": m_motifs}


def expand(s):
    return re.sub(r"\{\{(\w+)([^}]*)\}\}", lambda m: MACROS[m.group(1)](m.group(2)), s)


def main():
    html = expand(TEMPLATE).replace("<body>", "<body>\n" + PATTERN_DEFS, 1)
    out = os.path.join(G, "index.html")
    open(out, "w").write(html)
    n = html.count('<section class="page')
    print(f"index.html: {n} pages, {len(html) / 1e6:.1f} MB")
    if "--pages" in sys.argv:
        d = sys.argv[sys.argv.index("--pages") + 1]
        os.makedirs(d, exist_ok=True)
        head, body = html.split("<body>\n" + PATTERN_DEFS, 1)
        head = head.replace("../", BRAND + "/")
        pages = re.findall(r'<section class="page.*?</section><!--/page-->', body, re.S)
        for i, p in enumerate(pages, 1):
            open(os.path.join(d, f"p{i:02d}.html"), "w").write(
                head + f'<body style="counter-reset:pg {i - 1};background:none">' + PATTERN_DEFS + p.replace("../", BRAND + "/") + "</body>")
    if "--no-pdf" not in sys.argv:
        pdf = os.path.join(BRAND, "Nexus-Brand-Guidelines-v0.3.pdf")
        subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-pdf-header-footer", f"--print-to-pdf={pdf}",
                        "file://" + out], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print(f"PDF: {os.path.getsize(pdf) / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
