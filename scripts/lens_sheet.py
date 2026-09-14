"""Turn a lens-map dump into contact sheets.

`variation_probe lens` evaluates every variation over a screen grid and
dumps where each grid point lands. A lens is applied to the normalised
screen offset BEFORE the view scale, so a lensed render is exactly the
un-lensed image resampled at the lens's output -- which is why a survey
of 600 variations costs one render plus a resample, not 600 renders.

Two sources, because they answer different questions:

  grid     a calibration target -- straight lines, concentric circles,
           coloured quadrants. Says what the lens DOES.
  art      a real render. Says what the lens LOOKS like.

Points whose image leaves the source frame are drawn as dark red, so
"this lens reaches outside the render" reads differently from "this
lens is black".

    python scripts/lens_sheet.py [--src art.png] [--only name,name]
"""

import argparse
import math
import os
import struct
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

DUMP = "output/lens/lens-maps.bin"
OUT = "output/lens"

# The source covers normalised coordinates [-SPAN, SPAN]. The identity
# lens therefore shows the middle 1/SPAN of it, leaving room for lenses
# that reach outward before they run out of picture.
SPAN = 3.0

MAGIC = b"FFLENS01"


def read_dump(path):
    with open(path, "rb") as f:
        blob = f.read()
    if blob[:8] != MAGIC:
        sys.exit(f"{path}: not a lens dump")
    grid, count = struct.unpack_from("<II", blob, 8)
    off = 16
    n = grid * grid
    maps = []
    for _ in range(count):
        (nlen,) = struct.unpack_from("<I", blob, off)
        off += 4
        name = blob[off : off + nlen].decode("utf-8")
        off += nlen
        pts = np.frombuffer(blob, dtype="<f4", count=n * 2, offset=off).reshape(n, 2)
        off += n * 2 * 4
        maps.append((name, pts))
    return grid, maps


def calibration(size):
    """Straight lines, circles and coloured quadrants over [-SPAN, SPAN]."""
    img = Image.new("RGB", (size, size), (16, 16, 20))
    d = ImageDraw.Draw(img)

    def px(x, y):
        return ((x / SPAN * 0.5 + 0.5) * size, (0.5 - y / SPAN * 0.5) * size)

    # Quadrant washes, so orientation and mirroring are readable.
    for (sx, sy), col in [
        ((1, 1), (60, 30, 30)),
        ((-1, 1), (30, 55, 35)),
        ((-1, -1), (30, 35, 62)),
        ((1, -1), (58, 52, 28)),
    ]:
        x0, y0 = px(0 if sx > 0 else -SPAN, SPAN if sy > 0 else 0)
        x1, y1 = px(SPAN if sx > 0 else 0, 0 if sy > 0 else -SPAN)
        d.rectangle([x0, y0, x1, y1], fill=col)

    # Unit grid, with the unit square brighter: a lens's behaviour at
    # radius one is the part that matters.
    step = 0.25
    k = int(SPAN / step)
    for i in range(-k, k + 1):
        v = i * step
        near = abs(v) <= 1.0 + 1e-6
        c = (95, 100, 110) if near else (52, 55, 62)
        w = 1
        if abs(v - round(v)) < 1e-6:
            c = (150, 156, 168) if near else (78, 82, 92)
        d.line([px(v, -SPAN), px(v, SPAN)], fill=c, width=w)
        d.line([px(-SPAN, v), px(SPAN, v)], fill=c, width=w)

    # Concentric circles: a radial lens turns these into its profile.
    for r in [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 2.5]:
        c = (235, 190, 90) if abs(r - 1.0) < 1e-6 else (110, 120, 135)
        w = 3 if abs(r - 1.0) < 1e-6 else 1
        d.ellipse([*px(-r, r), *px(r, -r)], outline=c, width=w)

    # Axes, and a marker at +x so a rotation or an x/y swap is obvious.
    d.line([px(-SPAN, 0), px(SPAN, 0)], fill=(200, 205, 215), width=2)
    d.line([px(0, -SPAN), px(0, SPAN)], fill=(200, 205, 215), width=2)
    d.ellipse([*px(0.92, 0.08), *px(1.08, -0.08)], fill=(255, 80, 80))
    d.ellipse([*px(-0.08, 1.08), *px(0.08, 0.92)], fill=(90, 255, 120))
    return img


def resample(src, pts, grid):
    """Sample `src` at the lens's output. Out-of-frame -> dark red."""
    a = np.asarray(src, dtype=np.uint8)
    h, w = a.shape[:2]
    x = pts[:, 0]
    y = pts[:, 1]
    good = np.isfinite(x) & np.isfinite(y)
    # Normalised -> source pixel.
    sx = (x / SPAN * 0.5 + 0.5) * w
    sy = (0.5 - y / SPAN * 0.5) * h
    ix = np.clip(np.nan_to_num(sx, nan=-1e9), -1, w).astype(np.int64)
    iy = np.clip(np.nan_to_num(sy, nan=-1e9), -1, h).astype(np.int64)
    inside = good & (ix >= 0) & (ix < w) & (iy >= 0) & (iy < h)
    out = np.zeros((grid * grid, 3), dtype=np.uint8)
    out[:] = (40, 8, 8)
    ixc = np.clip(ix, 0, w - 1)
    iyc = np.clip(iy, 0, h - 1)
    out[inside] = a[iyc[inside], ixc[inside], :3]
    return Image.fromarray(out.reshape(grid, grid, 3))


def stats(pts, grid):
    """Numbers that separate a lens from a mess, in one pass."""
    x, y = pts[:, 0], pts[:, 1]
    fin = np.isfinite(x) & np.isfinite(y)
    r = np.hypot(np.nan_to_num(x), np.nan_to_num(y))
    inside = fin & (r <= SPAN)
    # Continuity: how far a step of one grid cell moves the output,
    # measured as a multiple of the input step. A smooth lens has a
    # small median; a shattering one does not.
    g = np.stack([np.nan_to_num(x), np.nan_to_num(y)], axis=1).reshape(grid, grid, 2)
    dx = np.linalg.norm(np.diff(g, axis=1), axis=2)
    dy = np.linalg.norm(np.diff(g, axis=0), axis=2)
    step = 2.0 / grid
    jump = np.concatenate([dx.ravel(), dy.ravel()]) / step
    jump = jump[np.isfinite(jump)]
    # Spread: how much of the plane the output actually covers,
    # relative to the input. A map that collapses to a point -- every
    # z-only variation, in two dimensions -- has a spread of zero and
    # is not a lens however smooth it is.
    if fin.any():
        spread = float(np.hypot(np.std(x[fin]), np.std(y[fin])))
    else:
        spread = 0.0
    return {
        "finite": float(fin.mean()),
        "inframe": float(inside.mean()),
        "median_jump": float(np.median(jump)) if jump.size else float("inf"),
        "p99_jump": float(np.percentile(jump, 99)) if jump.size else float("inf"),
        "span": float(np.percentile(r[fin], 99)) if fin.any() else float("inf"),
        "spread": spread,
    }


# Classes, in the order they are reported.
CLEAN = "clean"
UNBOUNDED = "unbounded"
DEGENERATE = "degenerate"
BROKEN = "broken"


def classify(s):
    """What kind of lens this is.

    Four outcomes rather than a yes/no, because the interesting
    rejections are not all the same rejection:

    `degenerate` -- the map collapses. Every z-only variation lands
    here: in two dimensions `zcone` and its kin return nothing, so
    every pixel samples the origin. Not a lens, and not a bug.

    `unbounded` -- smooth and well behaved except that it throws part
    of the frame to infinity, which is what an inversion like
    `spherical` DOES. Usable, but only with its output clamped, and
    under perturbation the far pixels leave the reference's
    neighbourhood. Worth seeing, worth flagging.

    `broken` -- non-finite, or neighbouring pixels stop being
    neighbours. Nothing to see.
    """
    if s["finite"] <= 0.98:
        return BROKEN
    if s["span"] < 0.05 or s["spread"] < 0.02:
        return DEGENERATE
    # Unbounded BEFORE the smoothness test, because a pole is what
    # makes both measurements large: `spherical` reads a p99 jump of 79
    # for exactly the reason it reads a span of 8.7, and calling that
    # "broken" would hide the most useful lens in the set behind its
    # own definition.
    if s["span"] > 3.0 or s["inframe"] < 0.75:
        return UNBOUNDED
    if s["median_jump"] >= 4.0 or s["p99_jump"] >= 40.0:
        return BROKEN
    return CLEAN


def sheet(entries, src, grid, path, cols=8, label_h=16):
    if not entries:
        return
    rows = math.ceil(len(entries) / cols)
    cell = grid + label_h
    img = Image.new("RGB", (cols * grid, rows * cell), (10, 10, 12))
    d = ImageDraw.Draw(img)
    try:
        font = ImageFont.truetype("arial.ttf", 11)
    except Exception:
        font = ImageFont.load_default()
    for i, (name, pts) in enumerate(entries):
        cx, cy = (i % cols) * grid, (i // cols) * cell
        img.paste(resample(src, pts, grid), (cx, cy))
        d.text((cx + 3, cy + grid + 2), name[:26], fill=(200, 205, 215), font=font)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    img.save(path)
    print(f"  {path}  ({len(entries)} lenses)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dump", default=DUMP)
    ap.add_argument("--src", default=None, help="a rendered PNG for the art sheet")
    ap.add_argument("--only", default=None, help="comma-separated variation names")
    ap.add_argument("--all", action="store_true", help="sheet the rejects too")
    args = ap.parse_args()

    grid, maps = read_dump(args.dump)
    print(f"{len(maps)} variations at {grid}x{grid}")

    # Sanity: linear must come back as the identity, or the harness is
    # measuring something other than the variation.
    for name, pts in maps:
        if name == "linear":
            ref = np.array([[p[0], p[1]] for p in pts], dtype=np.float32)
            from_grid = np.zeros_like(ref)
            n = grid
            for j in range(n):
                for i in range(n):
                    from_grid[j * n + i] = [
                        (i + 0.5) / n * 2 - 1,
                        -((j + 0.5) / n * 2 - 1),
                    ]
            err = np.abs(ref - from_grid).max()
            print(f"  linear is the identity to {err:.2e}")
            if err > 1e-5:
                sys.exit("linear is not the identity -- the harness is wrong")
            break

    cal = calibration(1024)
    cal.save(os.path.join(OUT, "source-grid.png"))

    scored = [(n, p, stats(p, grid), classify(stats(p, grid))) for n, p in maps]
    groups = {c: [(n, p) for n, p, _, k in scored if k == c] for c in
              (CLEAN, UNBOUNDED, DEGENERATE, BROKEN)}
    for c in (CLEAN, UNBOUNDED, DEGENERATE, BROKEN):
        print(f"  {len(groups[c]):4} {c}")

    with open(os.path.join(OUT, "lens-scores.txt"), "w") as f:
        f.write(
            f"{'variation':28} {'finite':>7} {'inframe':>8} {'jump':>7} "
            f"{'p99':>8} {'span':>7} {'spread':>7}  class\n"
        )
        for n, _, s, k in sorted(scored, key=lambda e: (e[3], e[0])):
            f.write(
                f"{n:28} {s['finite']:7.3f} {s['inframe']:8.3f} "
                f"{s['median_jump']:7.2f} {s['p99_jump']:8.1f} {s['span']:7.2f} "
                f"{s['spread']:7.3f}  {k}\n"
            )
    print(f"  {OUT}/lens-scores.txt")

    if args.only:
        want = {w.strip() for w in args.only.split(",")}
        picked = [(n, p) for n, p in maps if n in want]
        sheet(picked, cal, grid, f"{OUT}/only-grid.png")
        if args.src:
            art = Image.open(args.src).convert("RGB")
            sheet(picked, art, grid, f"{OUT}/only-art.png")
        return

    art = Image.open(args.src).convert("RGB") if args.src else None
    per = 96
    want = [CLEAN, UNBOUNDED] + ([DEGENERATE, BROKEN] if args.all else [])
    for c in want:
        entries = groups[c]
        for i in range(0, len(entries), per):
            tag = f"{c}-{i // per + 1}"
            sheet(entries[i : i + per], cal, grid, f"{OUT}/grid-{tag}.png")
            if art is not None:
                sheet(entries[i : i + per], art, grid, f"{OUT}/art-{tag}.png")


if __name__ == "__main__":
    main()
