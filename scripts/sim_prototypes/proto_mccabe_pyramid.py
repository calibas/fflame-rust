"""McCabe: box pyramid vs Gaussian pyramid vs exact disc.

Phase-3 wave 4 measurement, and the answer to an open item both the
catalogue (section 10) and the pipeline doc (section 3.2) carried:
McCabe's paper averages over a DISC, the GPU plan reads a mip pyramid,
and "the electron microscope look may depend on the isotropy" -- so
A/B before committing.

Three ways of taking the same disc averages, run on the same seed
with the same rule and the same five-scale ladder:

  disc          exact discs by FFT -- the reference, and what
                proto_mccabe.py used
  pyramid_plain the plan as written: a 2x2 BOX downsample per level,
                manual bilinear within a level, blended between the
                two levels that bracket log2(r)
  gauss_plain   the same reads on a GAUSSIAN pyramid: a separable
                [1 4 6 4 1]/16 blur, then decimate

Measured (256^2, 200 steps, seed 7):

    disc            sd 0.2665  feature 56.9 cells  spectral peak/mean 11.3
    pyramid_plain   sd 0.3649  feature 56.9 cells  spectral peak/mean  5.3
    gauss_plain     sd 0.3576  feature 102.4 cells spectral peak/mean 19.4

The BOX pyramid is refuted. Its render shows plainly axis-aligned,
rectangular structure -- the box kernel's square symmetry showing
through -- and its spectrum is half as peaked as the disc's. A box
downsample applied L times converges to a square of side 2^L, however
many times it is applied; it never becomes round.

The GAUSSIAN pyramid is adopted. The same reads on it are isotropic
and give the nested "electron microscope" texture, and at that point
the only difference from the disc is the SCALE: a Gaussian of pyramid
scale 2^l is a broader kernel than a disc of radius r, and the feature
size came out 1.8x the reference. The mapping is therefore
calibrated -- level = log2(CAL * r) -- and with CAL = 0.55 the
Gaussian pyramid reproduces the disc reference's feature size (56.9
against 56.9 cells) and amplitude (sd 0.2695 against 0.2665). That
constant is what the shader uses, so the shipped radius ladder means
what the paper's does.

Run with MC_CAL=<value> to re-measure the calibration.
"""
import os
import sys

import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)
N = 256
SCALES = [(1, 2, 0.05), (2, 4, 0.04), (4, 8, 0.03), (8, 16, 0.02), (16, 32, 0.01)]
LEVELS = 7
CAL = float(os.environ.get("MC_CAL", "0.55"))
_G = np.array([1.0, 4.0, 6.0, 4.0, 1.0]) / 16.0
_YY, _XX = np.mgrid[0:N, 0:N]


def save(field, name):
    img = ((np.clip(field, -1, 1) + 1) * 0.5 * 255).astype(np.uint8)
    Image.fromarray(img).save(f"{OUT}/{name}.png")


def disc_fft(r):
    y, x = np.mgrid[-N // 2:N // 2, -N // 2:N // 2]
    k = ((x * x + y * y) <= r * r).astype(np.float64)
    k /= k.sum()
    return np.fft.rfft2(np.fft.ifftshift(k))


KA = [disc_fft(a) for a, _, _ in SCALES]
KI = [disc_fft(i) for _, i, _ in SCALES]


def disc_avgs(f):
    F = np.fft.rfft2(f)
    return ([np.fft.irfft2(F * k, s=f.shape) for k in KA],
            [np.fft.irfft2(F * k, s=f.shape) for k in KI])


def build_pyramid(f, gaussian):
    """Level l+1 from level l. The shader does exactly this: one
    dispatch per level, a 5x5 read at stride 2, periodic in the level's
    own size."""
    p = [f]
    for _ in range(LEVELS):
        a = p[-1]
        if gaussian:
            b = sum(w * np.roll(a, k - 2, 1) for k, w in enumerate(_G))
            b = sum(w * np.roll(b, k - 2, 0) for k, w in enumerate(_G))
            p.append(b[0::2, 0::2])
        else:
            p.append(0.25 * (a[0::2, 0::2] + a[1::2, 0::2]
                             + a[0::2, 1::2] + a[1::2, 1::2]))
    return p


def level_avg(p, l):
    """Manual bilinear within level l, periodic: four textureLoads."""
    lv = p[l]
    m = lv.shape[0]
    s = N // m
    fy = (_YY + 0.5) / s - 0.5
    fx = (_XX + 0.5) / s - 0.5
    y0 = np.floor(fy).astype(int)
    x0 = np.floor(fx).astype(int)
    ty = fy - y0
    tx = fx - x0
    g = lambda a, b: lv[a % m, b % m]
    return ((1 - ty) * ((1 - tx) * g(y0, x0) + tx * g(y0, x0 + 1))
            + ty * ((1 - tx) * g(y0 + 1, x0) + tx * g(y0 + 1, x0 + 1)))


def pyr_avg(p, r):
    """Trilinear: the two levels bracketing log2(CAL * r), blended."""
    l = np.log2(max(r * CAL, 1.0))
    l0 = min(int(np.floor(l)), LEVELS)
    l1 = min(l0 + 1, LEVELS)
    t = l - np.floor(l)
    return (1 - t) * level_avg(p, l0) + t * level_avg(p, l1)


def step(f, mode):
    if mode == "disc":
        acts, inhs = disc_avgs(f)
    else:
        p = build_pyramid(f, gaussian=mode.startswith("gauss"))
        acts = [pyr_avg(p, a) for a, _, _ in SCALES]
        inhs = [pyr_avg(p, i) for _, i, _ in SCALES]
    best_var = None
    best_dir = None
    for (_, _, amt), act, inh in zip(SCALES, acts, inhs):
        var = np.abs(act - inh)
        d = np.where(act > inh, amt, -amt)
        if best_var is None:
            best_var, best_dir = var, d
        else:
            m = var < best_var
            best_var = np.where(m, var, best_var)
            best_dir = np.where(m, d, best_dir)
    f = f + best_dir
    lo, hi = f.min(), f.max()
    return (f - lo) / max(hi - lo, 1e-9) * 2 - 1


def feature_scale(f):
    g = f - f.mean()
    pw = np.abs(np.fft.fft2(g)) ** 2
    ky = np.fft.fftfreq(N) * N
    kr = np.sqrt(ky[:, None] ** 2 + ky[None, :] ** 2)
    bins = np.arange(0.5, N // 2, 1.0)
    idx = np.digitize(kr.ravel(), bins)
    pwr = np.bincount(idx, weights=pw.ravel(), minlength=len(bins) + 1)
    cnt = np.bincount(idx, minlength=len(bins) + 1)
    prof = pwr[1:len(bins)] / np.maximum(cnt[1:len(bins)], 1)
    peak = bins[:len(prof)][int(np.argmax(prof))]
    return float(N / peak), float(prof.max() / prof.mean())


if __name__ == "__main__":
    modes = sys.argv[1:] or ["disc", "pyramid_plain", "gauss_plain"]
    print(f"CAL = {CAL}")
    for mode in modes:
        rng = np.random.default_rng(7)
        f = rng.uniform(-1, 1, (N, N))
        for i in range(1, 201):
            f = step(f, mode)
            if i in (20, 100, 200):
                save(f, f"mccabe_{mode}_{i}")
        scale, sharp = feature_scale(f)
        print(f"{mode:<14} sd {f.std():.4f}  feature {scale:.1f} cells  "
              f"spectral peak/mean {sharp:.1f}")
