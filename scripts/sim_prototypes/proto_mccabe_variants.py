"""McCabe variants: what each idea in the mccabe-multiscale plan's section 8
does to the picture, before any of it is built.

One run per variant, all from the same seed on the same exact-disc averages
(antialiased discs by FFT, the spectral stage's arithmetic), so the tiles
differ only by the idea:

  P3   variation radius: |a - b| averaged over a disc before the argmin
       (Softology's variation radius; the paper's "variation around the
       pixel").
  P4   compound mode: the paper's "weighted sum of copies of the simple
       model", both readings -- the weighted sum of each scale's signed
       step, and the sign of the weighted sum of the scales' a - b.
  lean anisotropic kernels: each scale's discs stretched into ellipses
       (area kept) at an angle of its own.
  drift  the activator disc offset from the inhibitor's, so the pattern
       travels.
  maps scale i reads its a - b through a map of its own -- a rotation, a
       zoom, a swirl about the centre -- as a flame transform per scale
       would.
  colour relief shading of the field over the memory colour; Chau's
       luminance-from-the-field, chroma-from-the-scale.

Run:  python scripts/sim_prototypes/proto_mccabe_variants.py
Output: output/sim_proto/variants/sheet_*.png, one labelled tile a variant.
"""
import os
import time

import numpy as np
from PIL import Image, ImageDraw

OUT = "output/sim_proto/variants"
os.makedirs(OUT, exist_ok=True)
N = int(os.environ.get("VAR_N", "384"))
STEPS = int(os.environ.get("VAR_STEPS", "300"))
SEED = 7
_Y, _X = np.mgrid[-N // 2:N // 2, -N // 2:N // 2].astype(np.float64)
_GY, _GX = np.mgrid[0:N, 0:N].astype(np.float64)


def kernel(r, aspect=1.0, angle=0.0, offset=(0.0, 0.0)):
    """An antialiased disc of radius r, normalised; stretched into an
    ellipse of the same area along `angle` (radians) by `aspect`, and
    centred at `offset`. Built centred, then rolled to the origin."""
    x = _X - offset[0]
    y = _Y - offset[1]
    c, s = np.cos(angle), np.sin(angle)
    u = c * x + s * y
    v = -s * x + c * y
    q = np.sqrt(aspect)
    d = np.hypot(u / q, v * q)
    k = np.clip(r + 0.5 - d, 0.0, 1.0)
    k /= k.sum()
    return np.fft.rfft2(np.fft.ifftshift(k))


def disc_fft(r):
    return kernel(r)


def bilinear(field, x, y):
    """Periodic bilinear read of `field` at float cell coordinates."""
    x0 = np.floor(x).astype(int)
    y0 = np.floor(y).astype(int)
    tx = x - x0
    ty = y - y0
    g = lambda a, b: field[b % N, a % N]
    return ((1 - ty) * ((1 - tx) * g(x0, y0) + tx * g(x0 + 1, y0))
            + ty * ((1 - tx) * g(x0, y0 + 1) + tx * g(x0 + 1, y0 + 1)))


def rotation_map(theta, zoom=1.0):
    """Cell coordinates of each cell read through a rotation by theta and
    a zoom about the grid centre."""
    c = N / 2
    dx, dy = _GX + 0.5 - c, _GY + 0.5 - c
    cs, sn = np.cos(theta), np.sin(theta)
    return ((cs * dx - sn * dy) / zoom + c - 0.5, (sn * dx + cs * dy) / zoom + c - 0.5)


def swirl_map(strength):
    """Rotation growing with distance from the centre, `strength` radians
    at the rim."""
    c = N / 2
    dx, dy = _GX + 0.5 - c, _GY + 0.5 - c
    r = np.hypot(dx, dy)
    th = strength * r / (N / 2)
    cs, sn = np.cos(th), np.sin(th)
    return (cs * dx - sn * dy + c - 0.5, sn * dx + cs * dy + c - 0.5)


LADDER = [(2, .05), (4, .04), (8, .03), (16, .02), (32, .01)]


def make_scales(**per):
    """The default ladder, every scale given the same extra keys unless a
    key's value is a list (one entry a scale)."""
    out = []
    for i, (ra, amt) in enumerate(LADDER):
        s = dict(ra=ra, ratio=2.0, amount=amt, weight=1.0, aspect=1.0, angle=0.0, offset=(0.0, 0.0), map=None)
        for k, v in per.items():
            s[k] = v[i] if isinstance(v, list) else v
        out.append(s)
    return out


def run(scales, rule="argmin", vr=0.0, memory=0.17, steps=STEPS):
    """The rule from noise; returns (field, memory weights)."""
    kernels = []
    for s in scales:
        ka = kernel(s["ra"], s["aspect"], s["angle"], s["offset"])
        kb = kernel(s["ra"] * s["ratio"], s["aspect"], s["angle"])
        kernels.append(ka - kb)
    vk = disc_fft(vr) if vr > 0 else None
    rng = np.random.default_rng(SEED)
    f = rng.uniform(-1, 1, (N, N))
    mem = np.zeros((len(scales), N, N))
    for _ in range(steps):
        F = np.fft.rfft2(f)
        diffs = []
        for s, k in zip(scales, kernels):
            d = s["weight"] * np.fft.irfft2(F * k, s=(N, N))
            if s["map"] is not None:
                d = bilinear(d, *s["map"])
            diffs.append(d)
        if rule == "argmin":
            var = [np.abs(d) for d in diffs]
            if vk is not None:
                var = [np.fft.irfft2(np.fft.rfft2(v) * vk, s=(N, N)) for v in var]
            best = np.argmin(np.stack(var), axis=0)
            dirs = np.stack([np.where(d > 0, s["amount"], -s["amount"]) for d, s in zip(diffs, scales)])
            step = np.take_along_axis(dirs, best[None], 0)[0]
        elif rule == "compound_sum":
            step = sum(np.where(d > 0, s["amount"], -s["amount"]) for d, s in zip(diffs, scales))
            best = np.argmin(np.stack([np.abs(d) for d in diffs]), axis=0)
        elif rule == "compound_sign":
            total = sum(diffs)
            step = np.where(total > 0, scales[0]["amount"], -scales[0]["amount"])
            best = np.argmin(np.stack([np.abs(d) for d in diffs]), axis=0)
        else:
            raise ValueError(rule)
        f = f + step
        lo, hi = f.min(), f.max()
        f = (f - lo) / max(hi - lo, 1e-9) * 2 - 1
        onehot = np.stack([(best == i).astype(float) for i in range(len(scales))])
        mem = mem * (1 - memory) + onehot * memory
    return f, mem


# A blue-to-gold palette with one colour a scale, for every tile.
SCALE_COLOURS = np.array([
    [0.10, 0.15, 0.55], [0.05, 0.55, 0.65], [0.30, 0.75, 0.30], [0.95, 0.80, 0.20], [0.90, 0.35, 0.10], [0.70, 0.10, 0.50],
])


def colour_memory(f, mem, value_scale=0.5):
    c = np.einsum("knm,kc->nmc", mem, SCALE_COLOURS[:mem.shape[0]])
    v = np.clip(f * 0.5 + 0.5, 0, 1)[..., None]
    return c * (1 - value_scale + value_scale * v)


def colour_grey(f, mem):
    v = np.clip(f * 0.5 + 0.5, 0, 1)
    return np.repeat(v[..., None], 3, axis=2)


def hillshade(f, light=(-1.0, -1.0, 1.4), height=6.0):
    """Lambert shading of the field as a height map, lit from the upper
    left; 1 on a flat."""
    gx = (np.roll(f, -1, 1) - np.roll(f, 1, 1)) * 0.5 * height
    gy = (np.roll(f, -1, 0) - np.roll(f, 1, 0)) * 0.5 * height
    nx, ny, nz = -gx, -gy, np.ones_like(f)
    n = np.sqrt(nx * nx + ny * ny + nz * nz)
    l = np.array(light) / np.linalg.norm(light)
    flat = l[2]
    return np.clip((nx * l[0] + ny * l[1] + nz * l[2]) / n / flat, 0, 2)


def colour_relief(f, mem):
    return colour_memory(f, mem, value_scale=0.0) * hillshade(f)[..., None] * 0.85


def colour_chau(f, mem):
    """Chau: luminance from the field, chroma (YUV's U, V) from the scale
    colours mixed by the memory."""
    rgb = colour_memory(f, mem, value_scale=0.0)
    to_yuv = np.array([[0.299, 0.587, 0.114], [-0.14713, -0.28886, 0.436], [0.615, -0.51499, -0.10001]])
    yuv = rgb @ to_yuv.T
    yuv[..., 0] = np.clip(f * 0.5 + 0.5, 0, 1) * 0.9 + 0.05
    to_rgb = np.linalg.inv(to_yuv)
    return yuv @ to_rgb.T


def tile(rgb, label, size=300):
    img = Image.fromarray((np.clip(rgb, 0, 1) * 255).astype(np.uint8)).resize((size, size), Image.BILINEAR)
    d = ImageDraw.Draw(img)
    d.rectangle([0, 0, size, 16], fill=(0, 0, 0))
    d.text((4, 2), label, fill=(255, 255, 255))
    return img


def sheet(tiles, name, cols=4, size=300):
    rows = (len(tiles) + cols - 1) // cols
    s = Image.new("RGB", (cols * size, rows * size), (40, 40, 40))
    for i, t in enumerate(tiles):
        s.paste(t, ((i % cols) * size, (i // cols) * size))
    s.save(os.path.join(OUT, name))


if __name__ == "__main__":
    deg = np.pi / 180
    VARIANTS = [
        # name, scales, rule, vr, colouring
        ("baseline", make_scales(), "argmin", 0, colour_memory),
        ("P3 variation r1", make_scales(), "argmin", 1, colour_memory),
        ("P3 variation r3", make_scales(), "argmin", 3, colour_memory),
        ("P4 sum of steps", make_scales(), "compound_sum", 0, colour_grey),
        ("P4 sum, weights +-", make_scales(amount=[.05, -.04, .03, -.02, .01]), "compound_sum", 0, colour_grey),
        ("P4 sign of sum", make_scales(), "compound_sign", 0, colour_grey),
        ("P4 sign, weights +-", make_scales(weight=[1, -0.5, 1, -0.5, 1]), "compound_sign", 0, colour_grey),
        ("lean: all 2:1 at 0", make_scales(aspect=2.0), "argmin", 0, colour_memory),
        ("lean: 2:1, 36 deg apart", make_scales(aspect=2.0, angle=[0, 36 * deg, 72 * deg, 108 * deg, 144 * deg]), "argmin", 0, colour_memory),
        ("lean: 3:1 coarse only", make_scales(aspect=[1, 1, 3, 3, 3], angle=[0, 0, 0, 60 * deg, 120 * deg]), "argmin", 0, colour_memory),
        ("lean: 2:1 alt 0/90", make_scales(aspect=2.0, angle=[0, 90 * deg, 0, 90 * deg, 0]), "argmin", 0, colour_memory),
        ("drift: offset 0.3r", make_scales(offset=[(0.3 * r, 0) for r, _ in LADDER]), "argmin", 0, colour_memory),
        ("drift: offset turning", make_scales(offset=[(0.3 * r * np.cos(i * 72 * deg), 0.3 * r * np.sin(i * 72 * deg)) for i, (r, _) in enumerate(LADDER)]), "argmin", 0, colour_memory),
        ("map: rotate +-3 deg", make_scales(map=[rotation_map(a * deg) for a in (3, -3, 3, -3, 3)]), "argmin", 0, colour_memory),
        ("map: zoom coarse 1.02", make_scales(map=[None, None, rotation_map(0, 1.02), rotation_map(0, 1.04), rotation_map(0, 1.06)]), "argmin", 0, colour_memory),
        ("map: swirl", make_scales(map=[swirl_map(s) for s in (0.05, 0.1, 0.2, 0.3, 0.4)]), "argmin", 0, colour_memory),
        ("colour: relief", make_scales(), "argmin", 0, colour_relief),
        ("colour: Chau YUV", make_scales(), "argmin", 0, colour_chau),
    ]
    tiles = []
    cache = {}
    for name, scales, rule, vr, col in VARIANTS:
        t0 = time.time()
        key = (name if col is colour_memory or name.startswith(("P", "lean", "drift", "map")) else "baseline")
        if key not in cache:
            cache[key] = run(scales, rule, vr)
        f, mem = cache[key]
        tiles.append(tile(col(f, mem), name))
        Image.fromarray((np.clip(col(f, mem), 0, 1) * 255).astype(np.uint8)).save(
            os.path.join(OUT, name.replace(" ", "_").replace(":", "").replace("/", "-") + ".png"))
        print(f"{name:24s} {time.time() - t0:5.1f}s", flush=True)
    for k in range(0, len(tiles), 8):
        sheet(tiles[k:k + 8], f"sheet_{k // 8}.png")
