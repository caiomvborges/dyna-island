"""Draw Dyna Island's icon and installer pictures (Pillow).

The icon is a cream rounded tile with the Light theme's wave breaking
across it over deep water, as wave.rs draws it standing still. The installer
pictures put the island, in blue, on the same cream ground.

Writes:
  crates/dyna-island/res/dyna-island.ico            16 to 256 px
  installer/art/wizard-*.png          Inno Setup's wizard pictures
  installer/art/nsis-*.bmp            NSIS's welcome and header pictures

Run from anywhere: python tools/make_icon.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "crates" / "dyna-island" / "res"
ART = ROOT / "installer" / "art"

ISLAND = (0x08, 0x08, 0x0A)
BLUE = (0x2E, 0x64, 0xA6)  # dyna_core::look::LIGHT_ACCENT
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


# The Light theme's wave (crates/dyna-island/src/wave.rs), standing curled
# as in the print: its colours, its 100 x 64 box and the CURLED pose (the
# start point, then six cubics).
DEEP = (0x1F, 0x4E, 0x8C)
MID = (0x5B, 0x8D, 0xC4)
PALE = (0xB9, 0xD2, 0xEA)
FOAM = (0xFF, 0xFF, 0xFF)
BOX_H = 64.0
CURLED = [
    (0.0, 64.0),
    (10.0, 60.0), (22.0, 46.0), (30.0, 32.0),
    (38.0, 16.0), (52.0, 4.0), (68.0, 4.0),
    (82.0, 4.0), (94.0, 12.0), (97.0, 23.0),
    (91.0, 17.0), (81.0, 16.0), (75.0, 21.0),
    (67.0, 28.0), (67.0, 41.0), (75.0, 51.0),
    (81.0, 58.0), (90.0, 62.0), (100.0, 64.0),
]
BANDS = ((1.0, DEEP), (0.78, MID), (0.5, PALE))
FUJI = [(76.0, 61.0), (86.0, 49.0), (97.0, 61.0)]
SNOW = [(83.4, 52.1), (86.0, 49.0), (88.8, 52.4), (87.2, 51.6), (86.0, 52.9), (84.8, 51.5)]


def bezier(c, t: float):
    m = 1.0 - t
    a, b, d, e = m * m * m, 3 * m * m * t, 3 * m * t * t, t * t * t
    return (a * c[0][0] + b * c[1][0] + d * c[2][0] + e * c[3][0], a * c[0][1] + b * c[1][1] + d * c[2][1] + e * c[3][1])


def curve(i: int):
    return CURLED[3 * i : 3 * i + 4]


def body(s: float):
    """The body's outline, shrunk by `s` towards its left foot (as wave::body)."""
    pts = []
    for i in range(6):
        pts += [bezier(curve(i), k / 24) for k in range(24)]
    pts.append(CURLED[-1])
    return [(x * s, BOX_H - (BOX_H - y) * s) for x, y in pts]


def foam(small: bool):
    """The crest's foam at rest (as wave::foam): (centre, radius). Small
    icons get fewer, bigger bubbles so they still read."""
    n = 6 if small else 13
    out = []
    for k in range(1, n):
        u = k / n
        at = bezier(curve(1), 0.5 + 0.5 * u / 0.38) if u < 0.38 else bezier(curve(2), (u - 0.38) / 0.62)
        r = 1.6 + 0.8 * u / 0.38 if u < 0.38 else 2.4 + 0.2 * (u - 0.38) / 0.62
        out.append((at, r * (1.5 if small else 1.0)))
    for k in range(4 if not small else 2):
        u = k / 3
        out.append((bezier(curve(3), 0.75 * u), (2.4 - 1.4 * u) * (1.5 if small else 1.0)))
    return out


def icon(size: int) -> Image.Image:
    """A cream tile with the wave breaking across it over deep water."""
    s = size * SS
    small = size <= 24
    tile = Image.new("RGBA", (s, s), CREAM + (255,))
    d = ImageDraw.Draw(tile)
    # The wave's box fills most of the tile, its foot low on the sea and a
    # little to the left, so the curling lip and its foam stay inside.
    k = s / 100.0 * (0.90 if small else 0.80)
    ox, base = (s - 100.0 * k) / 2 - s * 0.03, s * 0.80
    at = lambda p: (ox + p[0] * k, base + (p[1] - BOX_H) * k)
    d.rectangle((0, base - k, s, s), fill=DEEP)
    if not small:
        d.polygon([at(p) for p in FUJI], fill=MID)
        d.polygon([at(p) for p in SNOW], fill=FOAM)
    for scale, colour in BANDS:
        d.polygon([at(p) for p in body(scale)], fill=colour)
    rim = max(0.7 * k, SS * 0.6)
    dots = [(at(c), r * k) for c, r in foam(small)]
    for (x, y), r in dots:
        d.ellipse((x - r - rim, y - r - rim, x + r + rim, y + r + rim), fill=DEEP)
    for (x, y), r in dots:
        d.ellipse((x - r, y - r, x + r, y + r), fill=FOAM)
    # Cut it to a rounded tile.
    mask = Image.new("L", (s, s), 0)
    inset = s * 0.06
    ImageDraw.Draw(mask).rounded_rectangle((inset, inset, s - inset, s - inset), radius=s * 0.22, fill=255)
    im = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    im.paste(tile, (0, 0), mask)
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
    big.save(RES / "dyna-island.ico", sizes=[(n, n) for n in sizes], append_images=[icon(n) for n in sizes if n != 256])
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
    for p in sorted([RES / "dyna-island.ico", *ART.iterdir()]):
        print(f"{p.relative_to(ROOT)}: {p.stat().st_size} bytes")


if __name__ == "__main__":
    main()
