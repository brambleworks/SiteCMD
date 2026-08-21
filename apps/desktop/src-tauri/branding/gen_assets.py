#!/usr/bin/env python3
"""Generate the committed macOS icon and DMG artwork; see the local README."""

import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, __version__ as PILLOW_VERSION


OUTPUT_DIR = Path(__file__).resolve().parent
EXPECTED_PILLOW_VERSION = "12.1.1"

# Brand palette shared with the public site.
PAPER = (245, 246, 248)
GRID_MINOR = (229, 232, 238)
GRID_MAJOR = (211, 216, 224)
INK = (30, 32, 36)
BLUE = (21, 93, 252)
MUTED = (106, 112, 128)
TILE = (233, 235, 238)

ARIAL_BOLD = Path("/System/Library/Fonts/Supplemental/Arial Bold.ttf")
ARIAL_REGULAR = Path("/System/Library/Fonts/Supplemental/Arial.ttf")


def validate_environment() -> None:
    """Fail before writing assets when the deterministic toolchain is absent."""
    if sys.platform != "darwin":
        raise SystemExit("Brand asset generation requires macOS.")
    if PILLOW_VERSION != EXPECTED_PILLOW_VERSION:
        raise SystemExit(
            f"Install Pillow {EXPECTED_PILLOW_VERSION}; found {PILLOW_VERSION}."
        )
    missing_fonts = [path for path in (ARIAL_BOLD, ARIAL_REGULAR) if not path.is_file()]
    if missing_fonts:
        raise SystemExit(f"Required macOS font is missing: {missing_fonts[0]}")
    if shutil.which("tiffutil") is None:
        raise SystemExit("Brand asset generation requires the macOS tiffutil command.")


def bracket_rects(offset_x: int, offset_y: int) -> list[tuple[int, int, int, int]]:
    """Return the favicon bracket geometry at the requested offset."""
    left = [(32, 48, 192, 104), (32, 48, 96, 464), (32, 408, 192, 464)]
    right = [(320, 48, 480, 104), (416, 48, 480, 464), (320, 408, 480, 464)]
    return [
        (x1 + offset_x, y1 + offset_y, x2 + offset_x, y2 + offset_y)
        for x1, y1, x2, y2 in left + right
    ]


def generate_icon(path: Path, size: int = 1024, supersampling: int = 4) -> None:
    canvas_size = size * supersampling
    image = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    margin = 100 * supersampling
    tile_size = 824 * supersampling
    radius = 185 * supersampling
    draw.rounded_rectangle(
        [(margin, margin), (margin + tile_size, margin + tile_size)],
        radius=radius,
        fill=TILE,
    )
    for x1, y1, x2, y2 in bracket_rects(256, 256):
        draw.rectangle(
            [
                (x1 * supersampling, y1 * supersampling),
                (x2 * supersampling, y2 * supersampling),
            ],
            fill=BLUE,
        )
    image = image.resize((size, size), Image.Resampling.LANCZOS)
    image.save(path)
    print("icon  ->", path, image.size)


def generate_background(path: Path, scale: int) -> None:
    width, height = 660 * scale, 400 * scale
    image = Image.new("RGB", (width, height), PAPER)
    draw = ImageDraw.Draw(image)

    for x in range(0, width + 1, 22 * scale):
        draw.line([(x, 0), (x, height)], fill=GRID_MINOR, width=scale)
    for y in range(0, height + 1, 22 * scale):
        draw.line([(0, y), (width, y)], fill=GRID_MINOR, width=scale)
    for x in range(0, width + 1, 110 * scale):
        draw.line([(x, 0), (x, height)], fill=GRID_MAJOR, width=scale)
    for y in range(0, height + 1, 110 * scale):
        draw.line([(0, y), (width, y)], fill=GRID_MAJOR, width=scale)

    logo_font = ImageFont.truetype(ARIAL_BOLD, round(31 * scale))
    logo_segments = [("Site", INK), ("[", BLUE), ("CMD", INK), ("]", BLUE)]
    bracket_padding = 2 * scale
    logo_width = sum(
        logo_font.getlength(text) + (bracket_padding if text in ("[", "]") else 0)
        for text, _ in logo_segments
    )
    cursor_x = 330 * scale - logo_width / 2
    logo_y = 54 * scale
    for text, color in logo_segments:
        if text == "[":
            cursor_x += bracket_padding
        draw.text((cursor_x, logo_y), text, font=logo_font, fill=color, anchor="lm")
        cursor_x += logo_font.getlength(text)

    caption_font = ImageFont.truetype(ARIAL_REGULAR, round(12 * scale))
    draw.text(
        (330 * scale, 150 * scale),
        "Drag to install",
        font=caption_font,
        fill=MUTED,
        anchor="mm",
    )

    arrow_y = 190 * scale
    draw.line([(245 * scale, arrow_y), (414 * scale, arrow_y)], fill=BLUE, width=2 * scale)
    draw.polygon(
        [
            (414 * scale, arrow_y - 6 * scale),
            (427 * scale, arrow_y),
            (414 * scale, arrow_y + 6 * scale),
        ],
        fill=BLUE,
    )

    image.save(path)
    print(f"bg{scale}x ->", path, image.size)


def main() -> None:
    validate_environment()
    icon_path = OUTPUT_DIR / "icon-source.png"
    background_1x = OUTPUT_DIR / "bg-1x.png"
    background_2x = OUTPUT_DIR / "bg-2x.png"
    dmg_background = OUTPUT_DIR / "dmg-background.tiff"

    generate_icon(icon_path)
    generate_background(background_1x, 1)
    generate_background(background_2x, 2)
    subprocess.run(
        [
            "tiffutil",
            "-cathidpicheck",
            str(background_1x),
            str(background_2x),
            "-out",
            str(dmg_background),
        ],
        check=True,
    )
    print("tiff  ->", dmg_background)


if __name__ == "__main__":
    main()
