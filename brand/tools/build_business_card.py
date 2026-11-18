"""Build a two-sided Nexus business card from the current logo masters.

Run with --name, --phone and --output for a personal PDF. Without arguments,
render the guide's fictional example. Dimensions are in PostScript points.
"""
import argparse
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory
from xml.etree import ElementTree

from reportlab.graphics.barcode import qrencoder
from reportlab.lib.colors import HexColor
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen.canvas import Canvas

BRAND = Path(__file__).resolve().parents[1]
DEMO = "https://nexus-demo-orcin.vercel.app"
PINE, LINEN = "#2F4631", "#F4EFE3"
WIDTH, HEIGHT, BLEED = 252, 144, 9
FONTS = ["SourceSans3-Regular", "SourceSans3-Semibold", "Fraunces-SemiBold"]


def draw_art(canvas, asset, x, top, scale, colour):
    """Draw a brand master, whose compound path contains absolute M/C/Z curves."""
    path = ElementTree.parse(BRAND / asset).getroot().find("{*}path").attrib["d"]
    canvas.saveState()
    canvas.translate(x, HEIGHT - top)
    canvas.scale(scale, -scale)
    curves = canvas.beginPath()
    for command, coordinates in re.findall(r"([MCZ])([^MCZ]*)", path):
        points = [float(n) for n in coordinates.split()]
        if command == "M":
            curves.moveTo(*points)
        elif command == "C":
            curves.curveTo(*points)
        else:
            curves.close()
    canvas.setFillColor(HexColor(colour))
    canvas.drawPath(curves, fill=1, stroke=0, fillMode=0)
    canvas.restoreState()


def draw_qr(canvas, x, y, side):
    qr = qrencoder.QRCode(None, qrencoder.QRErrorCorrectLevel.M)
    qr.addData(DEMO)
    qr.make()
    quiet = 4
    step = side / (len(qr.modules) + 2 * quiet)
    canvas.setFillColor(HexColor("#FCF9F0"))
    canvas.rect(x, y, side, side, fill=1, stroke=0)
    modules = canvas.beginPath()
    for row, cells in enumerate(qr.modules):
        for col, filled in enumerate(cells):
            if filled:
                modules.rect(x + (col + quiet) * step, y + side - (row + quiet + 1) * step, step, step)
    canvas.setFillColor(HexColor(PINE))
    canvas.drawPath(modules, fill=1, stroke=0)
    canvas.linkURL(DEMO, (x, y, x + side, y + side), relative=1, thickness=0)


def card(output, name, phone, role):
    digits = re.sub(r"\D", "", phone)
    if len(digits) == 11 and digits.startswith("1"):
        digits = digits[1:]
    if len(digits) != 10:
        raise ValueError("Enter a ten-digit North American phone number.")
    for font in FONTS:
        pdfmetrics.registerFont(TTFont(font, str(BRAND / "fonts" / f"{font}.ttf")))
    canvas = Canvas(str(output), pagesize=(WIDTH + 2*BLEED, HEIGHT + 2*BLEED), invariant=1)
    canvas.setTitle(f"Nexus | {name} | Business card")
    canvas.setAuthor("Nexus")

    def face(background):
        canvas.setTrimBox((BLEED, BLEED, WIDTH + BLEED, HEIGHT + BLEED))
        canvas.setBleedBox((0, 0, WIDTH + 2*BLEED, HEIGHT + 2*BLEED))
        canvas.translate(BLEED, BLEED)
        canvas.setFillColor(HexColor(background))
        canvas.rect(-BLEED, -BLEED, WIDTH + 2*BLEED, HEIGHT + 2*BLEED, fill=1, stroke=0)

    def label(text, x, top, font, size, colour=PINE):
        canvas.setFillColor(HexColor(colour))
        canvas.setFont(font, size)
        canvas.drawString(x, HEIGHT - top, text)

    face(PINE)
    canvas.saveState()
    clip = canvas.beginPath()
    clip.rect(184, -BLEED, WIDTH + BLEED - 184, HEIGHT + 2*BLEED)
    canvas.clipPath(clip, stroke=0)
    draw_art(canvas, "patterns/nexus-pattern-ring-field.svg", 182, -22, .193, "#496046")
    canvas.restoreState()
    draw_art(canvas, "logo/masters/nexus-symbol-convergence.svg", 16, 44, 56/512, LINEN)
    draw_art(canvas, "logo/masters/nexus-wordmark.svg", 78, 59, 91/730, LINEN)
    canvas.showPage()

    face(LINEN)
    label(name, 15, 40, "Fraunces-SemiBold", 20)
    label(role, 16, 55, "SourceSans3-Regular", 11, "#232B1E")
    canvas.setStrokeColor(HexColor("#B7B8A3"))
    canvas.setLineWidth(.6)
    canvas.line(16, 74.7, 236, 74.7)
    label(f"({digits[:3]}) {digits[3:6]}-{digits[6:]}", 16, 94, "SourceSans3-Semibold", 12.5)
    canvas.linkURL(f"tel:+1{digits}", (16, 47, 106, 63), relative=1, thickness=0)
    label("Nexus demo", 16, 114, "SourceSans3-Semibold", 10, "#232B1E")
    label(DEMO.removeprefix("https://"), 16, 127, "SourceSans3-Regular", 10.5)
    canvas.linkURL(DEMO, (16, 14, 161, 41), relative=1, thickness=0)
    draw_qr(canvas, 179, 11, 59)
    canvas.showPage()
    canvas.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name")
    parser.add_argument("--role", default="Co-founder")
    parser.add_argument("--phone")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if any((args.name, args.phone, args.output)):
        if not all((args.name, args.phone, args.output)):
            parser.error("--name, --phone and --output are required together")
        card(args.output, args.name, args.phone, args.role)
        return
    with TemporaryDirectory() as temp:
        sample = Path(temp) / "card.pdf"
        card(sample, "Alex Morgan", "(202) 555-0148", "Co-founder")
        # 240 dpi gives a 30 px bleed and exactly 840 x 480 px at US trim.
        for page, side in enumerate(("front", "back"), 1):
            subprocess.run(["pdftoppm", "-png", "-r", "240", "-f", str(page), "-l", str(page),
                            "-x", "30", "-y", "30", "-W", "840", "-H", "480", "-singlefile",
                            str(sample), str(BRAND / "guidelines" / f"card-{side}")], check=True)


if __name__ == "__main__":
    main()
