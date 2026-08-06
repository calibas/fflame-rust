#!/usr/bin/env python3
"""Build every icon size from the master logo.

    python scripts/make_icons.py

Reads `assets/branding/ffa-logo.png` (256x256) and writes the sizes the
platforms want, plus a multi-resolution `.ico` for the Windows
executable and an `.icns` for the macOS bundle. Re-run it after changing
the logo; the outputs are committed because they are build inputs.

Requires Pillow (`pip install pillow`). Not part of `release.py check` —
it only needs running when the artwork changes, and it would be the one
gate needing a dependency the rest do not.

# Plain Lanczos at every size

The mark is pure black and white with hard edges and thin strokes, so
the tempting move is to round each pixel back to black or white after
scaling and keep it "crisp". Judged at actual size that is worse: the
rounding introduces visible stair-steps, while the greys Lanczos
produces are doing antialiasing work that reads as a smooth edge.

Worth recording because the mistake is easy to repeat — magnifying a
16px icon to inspect it makes the antialiasing look like mush and the
thresholded version look clean, which is the opposite of how they
actually appear at size.

# Why macOS gets a differently-shaped mark

Windows shows the .ico square and edge-to-edge, which is how the mark is
drawn. macOS does not mask app icons the way iOS does — whatever shape
the .icns contains is the shape the Dock draws. A full-bleed square
therefore renders as a hard-cornered tile among rounded ones and reads
as visibly foreign, so the macOS path insets the mark onto Apple's icon
grid (art occupies 824/1024 of the canvas, corner radius 185.4/824 of
the art) and leaves the surround transparent.

That is a real divergence between the platforms rather than an
inconsistency: the same mark, drawn to each platform's convention.

# The .icns is built with iconutil, not Pillow

Pillow can write .icns unaided, which would keep this script working on
Windows and Linux, but its writer emits no 16x16 member at all — Finder
list view would downscale the 32px one, at exactly the size where the
hand-tuned Lanczos matters most. `iconutil` takes the full set.

So the .icns step is macOS-only and skips with a message elsewhere. The
PNGs and the .ico still build everywhere. Regenerating on a non-Mac
leaves the committed .icns untouched rather than replacing it with a
worse one.
"""

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    sys.exit("Pillow is required:  pip install pillow")

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "branding" / "ffa-logo.png"
OUT = ROOT / "assets" / "branding"

# Sizes Windows looks for in an .ico, plus the ones the window and the
# web favicon use.
SIZES = [16, 24, 32, 48, 64, 128, 256]

# The members an .icns wants, as `iconutil` names them: (file stem,
# pixel size). The @2x entries are the same pixel count as the next
# nominal size up but a different member type, and macOS picks between
# them by display scale — both must be present.
ICNS_MEMBERS = [
    ("icon_16x16", 16),
    ("icon_16x16@2x", 32),
    ("icon_32x32", 32),
    ("icon_32x32@2x", 64),
    ("icon_128x128", 128),
    ("icon_128x128@2x", 256),
    ("icon_256x256", 256),
    ("icon_256x256@2x", 512),
    ("icon_512x512", 512),
    ("icon_512x512@2x", 1024),
]

# Apple's icon grid, as fractions of the full canvas.
ICNS_ART_FRACTION = 824 / 1024
ICNS_CORNER_FRACTION = 185.4 / 824


def macos_tile(src: Image.Image, size: int) -> Image.Image:
    """The mark inset onto Apple's icon grid, transparent outside it.

    The mask is built at 4x and downsampled so the corner curve is
    antialiased; drawing it directly at `size` gives a stair-stepped
    arc at the small members, where it is most visible.
    """
    art = round(size * ICNS_ART_FRACTION)
    radius = art * ICNS_CORNER_FRACTION

    ss = 4
    mask = Image.new("L", (art * ss, art * ss), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, art * ss - 1, art * ss - 1], radius=radius * ss, fill=255
    )
    mask = mask.resize((art, art), Image.LANCZOS)

    body = src.resize((art, art), Image.LANCZOS)
    body.putalpha(mask)

    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    offset = (size - art) // 2
    canvas.paste(body, (offset, offset), body)
    return canvas


def build_icns(src: Image.Image) -> None:
    """Write `icon.icns` via iconutil. macOS only — see module docs."""
    icns = OUT / "icon.icns"

    if not shutil.which("iconutil"):
        print(f"  {icns.relative_to(ROOT)}  SKIPPED (needs iconutil, macOS only)")
        return

    # Flag the members the master cannot fill natively. 512 and 1024 are
    # upscales from a 256 source and will be soft; a larger master would
    # fix it without any change here, since every size resizes from SRC.
    upscaled = sorted({
        size for _, size in ICNS_MEMBERS
        if round(size * ICNS_ART_FRACTION) > src.width
    })
    if upscaled:
        print(f"note: {', '.join(str(s) for s in upscaled)}px members are "
              f"upscaled from a {src.width}px master and will be soft; "
              f"a 1024x1024 master would fix it", file=sys.stderr)

    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "icon.iconset"
        iconset.mkdir()
        for stem, size in ICNS_MEMBERS:
            macos_tile(src, size).save(iconset / f"{stem}.png")

        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(icns)],
            check=True,
        )

    print(f"  {icns.relative_to(ROOT)}  ({len(ICNS_MEMBERS)} members)")


def main() -> int:
    if not SRC.exists():
        sys.exit(f"missing {SRC.relative_to(ROOT)}")

    src = Image.open(SRC).convert("RGBA")
    if src.size != (256, 256):
        print(f"note: source is {src.size}, expected (256, 256)", file=sys.stderr)

    frames = []
    for size in SIZES:
        img = src.resize((size, size), Image.LANCZOS)
        path = OUT / f"icon-{size}.png"
        img.save(path)
        frames.append(img)
        print(f"  {path.relative_to(ROOT)}")

    ico = OUT / "icon.ico"
    # Pillow writes every frame it is given; `sizes` selects which of
    # them land in the file, so pass the full set to keep all of them.
    frames[-1].save(ico, format="ICO", sizes=[(s, s) for s in SIZES],
                    append_images=frames[:-1])
    print(f"  {ico.relative_to(ROOT)}  ({len(SIZES)} sizes)")

    build_icns(src)
    return 0


if __name__ == "__main__":
    sys.exit(main())
