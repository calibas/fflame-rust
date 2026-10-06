"""P3's smoothing kernel: does a separable Gaussian stand in for the disc?

The variation radius averages each scale's |a - b| over a disc before the
argmin (mccabe-multiscale plan, section 9). A disc gather costs (2r+1)^2
taps per scale; a Gaussian of the disc's own second moment (sigma = r/2
per axis, for a disc of radius r) is separable, two passes of ~6 sigma + 1
taps. This runs proto_mccabe_variants' rule from the same seed with each
kernel and compares the pictures and which scale wins each cell.

    python scripts/sim_prototypes/proto_mccabe_variation_kernel.py
"""
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(__file__))
import proto_mccabe_variants as pv  # noqa: E402

OUT = "output/sim_proto/variation_kernel"


def gauss_fft(r):
    """A normalised Gaussian with the AA disc of radius r's variance per axis."""
    k = np.clip(r + 0.5 - np.hypot(pv._X, pv._Y), 0.0, 1.0)
    var = (k * pv._X ** 2).sum() / k.sum()
    g = np.exp(-(pv._X ** 2 + pv._Y ** 2) / (2 * var))
    g /= g.sum()
    return np.fft.rfft2(np.fft.ifftshift(g)), var


def winners(mem):
    return np.argmax(mem, axis=0)


def main():
    os.makedirs(OUT, exist_ok=True)
    disc = pv.disc_fft
    tiles = []
    for r in (1, 2, 4):
        pv.disc_fft = disc
        fd, md = pv.run(pv.make_scales(), "argmin", r)
        gk, var = gauss_fft(r)
        pv.disc_fft = lambda _r, gk=gk: gk
        fg, mg = pv.run(pv.make_scales(), "argmin", r)
        pv.disc_fft = disc
        same = (winners(md) == winners(mg)).mean()
        corr = np.corrcoef(fd.ravel(), fg.ravel())[0, 1]
        print(f"r={r}: sigma {np.sqrt(var):.2f}; memory's leading scale agrees on {same:.1%} of cells; "
              f"field correlation {corr:.3f}", flush=True)
        for name, f, m in ((f"disc r{r}", fd, md), (f"gauss r{r}", fg, mg)):
            img = (np.clip(pv.colour_memory(f, m), 0, 1) * 255).astype(np.uint8)
            t = Image.fromarray(img)
            ImageDraw.Draw(t).text((4, 2), name, fill=(255, 255, 255))
            tiles.append(t)
    w, h = tiles[0].size
    sheet = Image.new("RGB", (2 * w, 3 * h))
    for i, t in enumerate(tiles):
        sheet.paste(t, ((i % 2) * w, (i // 2) * h))
    sheet.save(f"{OUT}/sheet.png")
    print("wrote", f"{OUT}/sheet.png")


if __name__ == "__main__":
    main()
