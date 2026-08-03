#!/usr/bin/env python3
"""Build every icon size from the master logo.

    python scripts/make_icons.py

Reads `assets/branding/ffa-logo.png` (256x256) and writes the sizes the
platforms want, plus a multi-resolution `.ico` for the Windows
executable. Re-run it after changing the logo; the outputs are committed
because they are build inputs.

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
"""

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    sys.exit("Pillow is required:  pip install pillow")

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "branding" / "ffa-logo.png"
OUT = ROOT / "assets" / "branding"

# Sizes Windows looks for in an .ico, plus the ones the window and the
# web favicon use.
SIZES = [16, 24, 32, 48, 64, 128, 256]


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
    return 0


if __name__ == "__main__":
    sys.exit(main())
