"""Origami crease relief: a prototype before the shader work.

The origami formula (src/escape/formulas.rs, ORIGAMI) folds the plane
along a sequence of lines, each line's endpoints folded through every
earlier fold so the creases land on the current wad. This sketches a
HEIGHT FIELD for that folding -- paper that rises or falls with the
distance from each crease -- and lights it, to settle the look before
it becomes a relief field and a terrain source.

Per pixel, per fold i (the formula's own loop):
  d_i   = signed distance of the point (in the wad, before fold i) from
          fold i's line, positive on the side that stays put
  sigma = the fold's direction (+1 valley, -1 mountain), the same on
          every layer: the continuous sheet the shader draws
  h    += decay**i * sigma * profile(|d_i|)

`layered=True` reverses a crease on a face-down layer (par, the earlier
folds that reflected the point, mod 2) -- the flat-folded assignment the
first version used. It tears the height wherever a crease crosses an
earlier one (docs/archive/projects/origami-creases.md).

The first fold is the biggest crease; each later one is finer, laid on
top. Run from the repo root; writes output/origami/*.png.

    python scripts/escape_prototypes/origami_creases.py
"""
import os
import numpy as np
from PIL import Image

OUT = "output/origami"
MAX_FOLDS = 64


def h32(x):
    x = np.uint32(x)
    x ^= x >> np.uint32(16)
    x = np.uint32((int(x) * 0x7FEB352D) & 0xFFFFFFFF)
    x ^= x >> np.uint32(15)
    x = np.uint32((int(x) * 0x846CA68B) & 0xFFFFFFFF)
    x ^= x >> np.uint32(16)
    return int(x)


def unit(x):
    return np.float32((h32(x & 0xFFFFFFFF) >> 8) * (1.0 / 16_777_216.0))


def fold_point(p, l):
    abx, aby = l[2] - l[0], l[3] - l[1]
    d2 = abx * abx + aby * aby
    if abx * (p[1] - l[1]) - aby * (p[0] - l[0]) > 0 or d2 < 1e-12:
        return p
    t = ((p[0] - l[0]) * abx + (p[1] - l[1]) * aby) / d2
    return [(l[0] + abx * t) * 2 - p[0], (l[1] + aby * t) * 2 - p[1]]


def lines(seed, spread):
    """origami_derived_lines, in f32."""
    s = (int(min(max(seed, 0), 4096)) * 2_654_435_761) & 0xFFFFFFFF
    out = []
    for j in range(MAX_FOLDS):
        a = [np.float32((unit(j * 4 + s) * 2 - 1) * spread), np.float32((unit(j * 4 + 1 + s) * 2 - 1) * spread)]
        b = [np.float32((unit(j * 4 + 2 + s) * 2 - 1) * spread), np.float32((unit(j * 4 + 3 + s) * 2 - 1) * spread)]
        for l in out:
            a = fold_point(a, l)
            b = fold_point(b, l)
        out.append([a[0], a[1], b[0], b[1]])
    return np.array(out, dtype=np.float64)


def crease_height(w, h, *, seed=8, spread=2.0, folds=32, zoom_log2=-1.0, centre=(0.0, 0.0),
                  decay=0.6, pattern="valley", flip=False, depth=1.0, layered=False, reach=0.0):
    """The crease height and its exact gradient, per pixel.

    The fold count is the formula's iteration count: one crease a fold.
    The gradient is carried through the folds rather than differenced:
    every fold is a reflection, so the point's position is an
    orthogonal map of the pixel, M p + t, and the gradient of a crease's
    distance is M^T n.
    """
    span_y = 4.0 / 2 ** zoom_log2
    span_x = span_y * w / h
    xs = (np.arange(w) + 0.5) / w - 0.5
    ys = (np.arange(h) + 0.5) / h - 0.5
    zx, zy = np.meshgrid(centre[0] + xs * span_x, centre[1] + ys * span_y)
    par = np.zeros_like(zx)
    height = np.zeros_like(zx)
    gx = np.zeros_like(zx)
    gy = np.zeros_like(zx)
    # The accumulated linear part, M (rows m00 m01 / m10 m11), per pixel.
    m00, m01, m10, m11 = np.ones_like(zx), np.zeros_like(zx), np.zeros_like(zx), np.ones_like(zx)
    L = lines(seed, spread)
    rng = np.random.default_rng(seed)
    base = {
        "valley": np.ones(folds),
        "alternate": np.array([1 if i % 2 == 0 else -1 for i in range(folds)], dtype=float),
        "random": rng.choice([-1.0, 1.0], folds),
    }[pattern] * (-1.0 if flip else 1.0)
    for i in range(folds):
        ax, ay, bx, by = L[i]
        abx, aby = bx - ax, by - ay
        n = np.hypot(abx, aby)
        if n < 1e-6:
            continue
        cross = abx * (zy - ay) - aby * (zx - ax)
        d = cross / n
        nx, ny = -aby / n, abx / n
        sigma = base[i] * (np.where(par > 0.5, -1.0, 1.0) if layered else 1.0)
        dist = np.abs(d)
        if reach > 0:
            dist = np.minimum(dist, reach)
        w_i = depth * decay ** i * sigma
        height += w_i * dist
        sgn = np.sign(d)
        # grad_p d = M^T n
        gx += w_i * sgn * (m00 * nx + m10 * ny)
        gy += w_i * sgn * (m01 * nx + m11 * ny)
        moving = cross <= 0
        t = ((zx - ax) * abx + (zy - ay) * aby) / (n * n)
        rx = (ax + abx * t) * 2 - zx
        ry = (ay + aby * t) * 2 - zy
        zx = np.where(moving, rx, zx)
        zy = np.where(moving, ry, zy)
        par = np.where(moving, 1 - par, par)
        # A reflection about the line's direction u: R = 2 u u^T - I.
        ux, uy = abx / n, aby / n
        r00, r01, r10, r11 = 2 * ux * ux - 1, 2 * ux * uy, 2 * ux * uy, 2 * uy * uy - 1
        n00 = r00 * m00 + r01 * m10
        n01 = r00 * m01 + r01 * m11
        n10 = r10 * m00 + r11 * m10
        n11 = r10 * m01 + r11 * m11
        m00 = np.where(moving, n00, m00)
        m01 = np.where(moving, n01, m01)
        m10 = np.where(moving, n10, m10)
        m11 = np.where(moving, n11, m11)
    return height, span_y / h, (gx, gy)


def shade_gradient(g, light_az=135.0, light_el=40.0):
    gx, gy = g
    nx, ny, nz = -gx, -gy, np.ones_like(gx)
    nn = np.sqrt(nx * nx + ny * ny + nz * nz)
    az, el = np.radians(light_az), np.radians(light_el)
    l = np.array([np.cos(el) * np.cos(az), np.cos(el) * np.sin(az), np.sin(el)])
    lam = np.clip((nx * l[0] + ny * l[1] + nz * l[2]) / nn, 0, 1)
    return 0.25 + 0.75 * lam


def shade(height, px, light_az=135.0, light_el=40.0, strength=1.0):
    gy, gx = np.gradient(height, px)
    nx, ny, nz = -gx * strength, -gy * strength, np.ones_like(height)
    nn = np.sqrt(nx * nx + ny * ny + nz * nz)
    az, el = np.radians(light_az), np.radians(light_el)
    l = np.array([np.cos(el) * np.cos(az), np.cos(el) * np.sin(az), np.sin(el)])
    lam = np.clip((nx * l[0] + ny * l[1] + nz * l[2]) / nn, 0, 1)
    return 0.25 + 0.75 * lam


def save(img, name):
    os.makedirs(OUT, exist_ok=True)
    a = (np.clip(img, 0, 1) * 255).astype(np.uint8)
    Image.fromarray(a).save(os.path.join(OUT, name))
    print("wrote", os.path.join(OUT, name))


if __name__ == "__main__":
    W, H = 640, 480
    for pattern in ("valley", "alternate", "random"):
        for decay in (0.6, 0.85):
            hgt, px, g = crease_height(W, H, pattern=pattern, decay=decay)
            save(shade(hgt, px), f"creases-{pattern}-decay{decay}.png")
            save(shade_gradient(g), f"analytic-{pattern}-decay{decay}.png")
    hgt, px, g = crease_height(W, H, pattern="valley", decay=0.6, flip=True)
    save(shade_gradient(g), "analytic-valley-decay0.6-flip.png")
    hgt, px, g = crease_height(W, H, pattern="valley", decay=0.6, folds=4)
    save(shade_gradient(g), "analytic-valley-4folds.png")
    # The finite differences of the height agree with the carried gradient
    # away from the creases themselves.
    hgt, px, g = crease_height(W, H, pattern="alternate", decay=0.6)
    fy, fx = np.gradient(hgt, px)
    agree = np.median(np.hypot(fx - g[0], fy - g[1]))
    print("median |numeric - carried gradient|:", agree)
