"""Draw Dyna Island's icon and installer pictures (Pillow).

The icon is a blue rounded tile holding the island: a black pill with a
cream dot (the album art) and three cream bars (the equalizer). The installer
pictures put the island, in blue, on the Light theme's cream ground.

Writes:
  crates/poni/res/poni.ico            16 to 256 px
  installer/art/wizard-*.png          Inno Setup's wizard pictures
  installer/art/nsis-*.bmp            NSIS's welcome and header pictures

Run from anywhere: python tools/make_icon.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "crates" / "poni" / "res"
ART = ROOT / "installer" / "art"

ISLAND = (0x08, 0x08, 0x0A)
BLUE = (0x2E, 0x64, 0xA6)  # poni_core::look::LIGHT_ACCENT
CREAM = (0xFB, 0xF4, 0xE9)  # the Light theme's ground
SS = 4  # supersampling


def island(d: ImageDraw.ImageDraw, x: float, y: float, w: float, h: float, mark=BLUE) -> None:
    """The pill at (x, y), w by h, with its art dot and equalizer."""
    d.rounded_rectangle((x, y, x + w, y + h), radius=h / 2, fill=ISLAND)
    r = h * 0.30
    cx, cy = x + h / 2, y + h / 2
    d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=mark)
    bw = h * 0.12
    gap = h * 0.10
    right = x + w - h * 0.42
    for i, k in enumerate((0.38, 0.62, 0.46)):
        bx = right - (3 - i) * bw - (2 - i) * gap
        bh = h * k
        d.rounded_rectangle((bx, cy - bh / 2, bx + bw, cy + bh / 2), radius=bw / 2, fill=mark)


def icon(size: int) -> Image.Image:
    s = size * SS
    im = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    inset = s * 0.06
    d.rounded_rectangle((inset, inset, s - inset, s - inset), radius=s * 0.22, fill=BLUE)
    w, h = s * 0.70, s * 0.27
    island(d, (s - w) / 2, (s - h) / 2, w, h, CREAM)
    return im.resize((size, size), Image.LANCZOS)


def picture(w: int, h: int, scale: float = 0.62) -> Image.Image:
    """A cream field with the island in the middle."""
    im = Image.new("RGB", (w * SS, h * SS), CREAM)
    d = ImageDraw.Draw(im)
    iw = w * SS * scale
    ih = iw * 0.27 / 0.70
    island(d, (w * SS - iw) / 2, (h * SS - ih) / 2, iw, ih)
    return im.resize((w, h), Image.LANCZOS)


def header(w: int, h: int) -> Image.Image:
    """NSIS's header: white, with the icon on the right."""
    im = Image.new("RGB", (w, h), (255, 255, 255))
    ic = icon(h - 8)
    im.paste(ic, (w - ic.width - 4, 4), ic)
    return im


def main() -> None:
    sizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
    big = icon(256)
    big.save(RES / "poni.ico", sizes=[(n, n) for n in sizes], append_images=[icon(n) for n in sizes if n != 256])
    ART.mkdir(parents=True, exist_ok=True)
    for n in (58, 77, 97, 116, 124, 143, 159):
        bg = Image.new("RGB", (n, n), (255, 255, 255))
        ic = icon(n)
        bg.paste(ic, (0, 0), ic)
        bg.save(ART / f"wizard-small-{n}.png", optimize=True)
    for w, h in ((202, 386), (336, 643), (430, 824)):
        picture(w, h).save(ART / f"wizard-{w}x{h}.png", optimize=True)
    picture(164, 314).save(ART / "nsis-welcome.bmp")
    header(150, 57).save(ART / "nsis-header.bmp")
    for p in sorted([RES / "poni.ico", *ART.iterdir()]):
        print(f"{p.relative_to(ROOT)}: {p.stat().st_size} bytes")


if __name__ == "__main__":
    main()
