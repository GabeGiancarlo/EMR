"""Cut the fixed font files Nexus uses from the variable sources.

Fraunces is always set soft (SOFT 100, WONK 0); fixing those axes in the files means the app and
the PDF need no font-variation-settings, and printers embed real fonts instead of outlines.

Run:  python3 brand/tools/build_fonts.py
Reads brand/fonts/variable/*.ttf, writes brand/fonts/*.ttf. Needs fontTools.
"""
from __future__ import annotations

import os

from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

FONTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "fonts")

# file, source, axis values, family, style
CUTS = [
    ("Fraunces-SemiBold.ttf", "Fraunces-Variable.ttf", {"wght": 600, "SOFT": 100, "WONK": 0, "opsz": 36}, "Fraunces Soft", "SemiBold"),
    ("Fraunces-Regular.ttf", "Fraunces-Variable.ttf", {"wght": 400, "SOFT": 100, "WONK": 0, "opsz": 18}, "Fraunces Soft", "Regular"),
    ("Manrope-ExtraBold.ttf", "Manrope-Variable.ttf", {"wght": 800}, "Manrope", "ExtraBold"),
    ("Manrope-Bold.ttf", "Manrope-Variable.ttf", {"wght": 700}, "Manrope", "Bold"),
]


def main():
    for out, src, axes, family, style in CUTS:
        font = instancer.instantiateVariableFont(TTFont(os.path.join(FONTS, "variable", src)), axes)
        names = {1: family if style in ("Regular", "Bold") else f"{family} {style}", 2: style if style in ("Regular", "Bold") else "Regular",
                 4: f"{family} {style}", 6: f"{family}-{style}".replace(" ", ""), 16: family, 17: style}
        for rec in list(font["name"].names):
            if rec.nameID in names:
                rec.string = names[rec.nameID]
            elif rec.nameID in (3, 25):
                font["name"].removeNames(nameID=rec.nameID)
        for nid, text in names.items():
            if not font["name"].getName(nid, 3, 1, 0x409):
                font["name"].setName(text, nid, 3, 1, 0x409)
        font["OS/2"].usWeightClass = axes["wght"]
        path = os.path.join(FONTS, out)
        font.save(path)
        print(f"{out}: {os.path.getsize(path) / 1024:.0f} KB")


if __name__ == "__main__":
    main()
