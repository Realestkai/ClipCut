#!/usr/bin/env python3
"""Makes every icon and banner the builds and the README use from the logo.

    python3 scripts/make-icon.py

Reads the two white-on-transparent originals in assets/brand/:

    clipcut-mark.png      the CC mark
    clipcut-wordmark.png  the word "clipcut"

and writes

    assets/icons/clipcut_logo_<size>.png   the mark on a dark rounded square
    assets/icons/clipcut.ico               the same, for Windows
    assets/brand/clipcut-banner.png        the wordmark on a dark plate, for
                                           the README (white on a white page
                                           would not show)

Needs Pillow.
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "assets" / "brand"
OUT = ROOT / "assets" / "icons"
SIZES = (512, 256, 128, 64, 32, 16)
S = 2048  # the icon is drawn this big, then scaled down
PLATE = (17, 17, 19)  # near-black


def shape(path: Path) -> Image.Image:
    """The logo's coverage as a greyscale mask, cropped to the logo: stray
    near-transparent pixels around it are dropped."""
    alpha = Image.open(path).convert("RGBA").getchannel("A")
    alpha = alpha.point(lambda a: 0 if a < 48 else a)
    return alpha.crop(alpha.getbbox())


def fitted(mask: Image.Image, width: int, height: int) -> Image.Image:
    scale = min(width / mask.width, height / mask.height)
    return mask.resize((round(mask.width * scale), round(mask.height * scale)), Image.LANCZOS)


def icon() -> Image.Image:
    plate = Image.new("L", (S, S), 0)
    ImageDraw.Draw(plate).rounded_rectangle(
        (S * 0.06, S * 0.06, S * 0.94, S * 0.94), radius=S * 0.21, fill=255
    )
    image = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    image.paste(Image.new("RGBA", (S, S), PLATE + (255,)), (0, 0), plate)
    mark = fitted(shape(BRAND / "clipcut-mark.png"), round(S * 0.56), round(S * 0.56))
    at = ((S - mark.width) // 2, (S - mark.height) // 2)
    image.paste(Image.new("RGBA", mark.size, (255, 255, 255, 255)), at, mark)
    return image


def banner() -> Image.Image:
    width, height = 1600, 520
    plate = Image.new("L", (width, height), 0)
    ImageDraw.Draw(plate).rounded_rectangle((0, 0, width - 1, height - 1), radius=48, fill=255)
    image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    image.paste(Image.new("RGBA", (width, height), PLATE + (255,)), (0, 0), plate)
    word = fitted(shape(BRAND / "clipcut-wordmark.png"), round(width * 0.62), round(height * 0.5))
    at = ((width - word.width) // 2, (height - word.height) // 2)
    image.paste(Image.new("RGBA", word.size, (255, 255, 255, 255)), at, word)
    return image


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    image = icon()
    for size in SIZES:
        image.resize((size, size), Image.LANCZOS).save(OUT / f"clipcut_logo_{size}.png")
    image.resize((256, 256), Image.LANCZOS).save(
        OUT / "clipcut.ico", sizes=[(s, s) for s in (256, 128, 64, 48, 32, 24, 16)]
    )
    banner().save(BRAND / "clipcut-banner.png")
    print(f"wrote {len(SIZES)} PNGs and clipcut.ico to {OUT}, and the banner to {BRAND}")


if __name__ == "__main__":
    main()
