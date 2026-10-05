"""McCabe on the pyramid: where the axis lean comes from, and what removes it.

mccabe-multiscale plan, section 3, found that tables whose COARSE scales
take the largest steps (Reusser's order) come out leaning to the grid
axes: 1.25 in spectral energy, axes over diagonals, against 0.99 for the
same radii with the steps the other way round. Both Reusser and Chau
average by FFT with exact kernels; only we read a Gaussian pyramid. This
script asks two questions against an exact reference.

PART A, the kernel. For each averaging method and each (activator,
inhibitor) radius pair, the response of `avg(f, ra) - avg(f, rb)` to
plane waves cos(2 pi k.x / N) at every lattice wavevector. The Turing
rule grows the wavevector with the LARGEST response fastest, so the
response's peak sets the pattern's wavelength, and how that peak varies
with ANGLE sets the orientation it prefers. Reported per method: the
peak wavenumber (against the exact disc's, which is the calibration),
and the axis/diagonal ratio of the peak response.

PART B, the dynamics. The whole rule -- per-cell argmin of |a - b|, the
winner's signed step, renormalisation -- on noise, for a table with its
coarse scales fastest and the same radii with the fine scales fastest,
over several seeds; the final field's spectral energy within 10 degrees
of the axes over that within 10 degrees of the diagonals (1.0 is
isotropic, and the seed-to-seed spread says how far from 1.0 is noise).

Methods:
  disc          exact disc, antialiased over one cell, by FFT (Reusser's
                circular kernel; the reference)
  epan          Epanechnikov (1 - d^2/R^2)+, by FFT (Chau's alternative)
  gauss         the Gaussian the pyramid approximates, sigma^2 = 4^l / 3
                at the calibrated level, exactly, by FFT: the pyramid
                with neither its decimation nor its bilinear reads
  pyr           the shader: Gaussian pyramid, bilinear within a level,
                linear between the two levels bracketing log2(0.55 r),
                texel i of level l centred on base cell i * 2^l
  pyr_bspline   the same pyramid, cubic B-spline within a level
  pyr_catmull   the same pyramid, Catmull-Rom within a level
  pyr_over      an OVERSAMPLED pyramid: the same Gaussian per level, but
                level l stored at spacing 2^(l-1) instead of 2^l (level
                1 at full resolution), built by a binomial dilated by 2
                and then a decimation; read bilinearly, four loads. The
                bilinear tent is then half as wide against the blur.
  pyr_over_bspline  the oversampled pyramid read through a B-spline
  pyr_g7        the pyramid built with a 7-tap kernel whose moments match
                a Gaussian's to fourth order (variance 1, fourth moment 3)
                and whose response is zero at Nyquist: [1 10 47 76 47 10 1]
                / 192. A separable kernel k(x) k(y) is round only if k is
                Gaussian; [1 4 6 4 1] / 16 has fourth moment 2.5, not 3.
                Read bilinearly, four loads, as now.
  pyr_g7c       the same with fourth moment 3.25, [1 6 31 52 31 6 1] / 128:
                enough excess to cancel the bilinear tent's own deficit
  pyr_jitter    the shader's pyramid with its LATTICE moved every step: the
                field rolled by a random offset (up to the coarsest texel)
                before the pyramid is built, and the averages rolled back.
                The kernel is the same; the bilinear reads' creases along
                texel lines no longer stay in one place.

Run:  python scripts/sim_prototypes/proto_mccabe_isotropy.py [A] [B]
      env ISO_N, ISO_METHODS (B), ISO_METHODS_A (A), ISO_SEEDS, ISO_TABLES

Measured 2026-10-04 (N = 512; GPU numbers from the shader through the CLI,
the same metric on the rendered field):

PART A, axis/diagonal of the peak response (1.0 is round; at 45/90 the
lattice cannot resolve the angle, so that pair is left out):

                    3/6      10/20    20/40
    disc           1.0000   1.0003   1.0011
    epan           1.0012   0.9993   0.9992
    gauss          1.0004   1.0008   1.0000
    pyr            1.0113   1.0089   1.0136   the shader: ~1% toward the axes
    pyr_bspline    1.0083   1.0050   1.0006
    pyr_catmull    1.0156   1.0171   1.0161
    pyr_over       1.0143   1.0112   1.0138   oversampling does nothing
    pyr_g7         1.0044   1.0032   1.0090
    pyr_g7c        1.0013   1.0006   1.0067   the kernel made round

PART B, coarse_fast, axes/diagonals of the final field (mean +- se):

    disc, CPU, 12 seeds          0.99 +- 0.05
    gauss, CPU, 12 seeds         0.96 +- 0.02
    pyr, CPU, 8 seeds            1.13 +- 0.04
    pyr_jitter, CPU, 8 seeds     0.97 +- 0.05   lower on all 8 seeds, by 0.16 +- 0.03
    pyr_bspline, CPU, 8 seeds    1.02 +- 0.03   lower on 6 of 8, by 0.11 +- 0.04
    bilinear, GPU, 32 seeds      1.08 +- 0.02
    B-spline reads, GPU, 32      1.03 +- 0.02   paired gain 0.05 +- 0.03; 4.1x the cost
    round kernel, GPU, 32        1.10 +- 0.02   no gain at all
    fine_fast: disc 1.02, GPU bilinear 0.95, GPU round 0.98 -- no lean

So the lean is not the kernel's average SHAPE -- making it round at the
kernel level (pyr_g7c) did nothing to the dynamics -- but the pyramid's
fixed LATTICE: moving it every step (pyr_jitter) removes the lean
entirely, and smoother reads across texel lines (B-spline) remove some.
The bilinear reads crease along the coarse levels' texel lines, 32-64
cells apart, and a rule that turns on where a - b crosses zero locks to
them. Jitter costs some motion: direction flips per step 0.32 -> 0.37,
mean |df| per step 0.013 -> 0.016 (seed 3, steps 200-210).

The exact disc's texture also differs from every Gaussian method's -- finer,
sharper detail (output/sim_proto/isotropy/coarse_fast_disc.png) -- which
is the disc-vs-Gaussian question, separate from the lean.
"""
import os
import sys
import time

import numpy as np

OUT = "output/sim_proto/isotropy"
os.makedirs(OUT, exist_ok=True)
N = int(os.environ.get("ISO_N", "512"))
CAL = 0.55
_G = np.array([1.0, 4.0, 6.0, 4.0, 1.0]) / 16.0


# ---------------------------------------------------------------------------
# Exact kernels by FFT
# ---------------------------------------------------------------------------
_YC, _XC = np.mgrid[-N // 2:N // 2, -N // 2:N // 2]
_DIST = np.hypot(_XC, _YC)
_FFT_CACHE = {}


def _kernel_fft(kind, r):
    key = (kind, round(float(r), 6))
    if key in _FFT_CACHE:
        return _FFT_CACHE[key]
    if kind == "disc":
        k = np.clip(r + 0.5 - _DIST, 0.0, 1.0)
    elif kind == "epan":
        k = np.clip(1.0 - (_DIST / max(r, 0.5)) ** 2, 0.0, None)
    elif kind == "gauss":
        # The pyramid's Gaussian at its calibrated level, before any
        # reconstruction: sigma^2 = (4^l - 1) / 3 for a [1 4 6 4 1]
        # cascade l levels deep, with l = log2(CAL r).
        l = max(np.log2(max(CAL * r, 1.0)), 0.0)
        var = (4.0 ** l - 1.0) / 3.0
        if var < 1e-6:
            k = (_DIST == 0).astype(np.float64)
        else:
            k = np.exp(-(_DIST ** 2) / (2 * var))
    else:
        raise ValueError(kind)
    k = k / k.sum()
    kf = np.fft.rfft2(np.fft.ifftshift(k))
    _FFT_CACHE[key] = kf
    return kf


def fft_avg(kind):
    def avg(F, r):
        return np.fft.irfft2(F * _kernel_fft(kind, r), s=(N, N))
    return avg


# ---------------------------------------------------------------------------
# The shader's pyramid
# ---------------------------------------------------------------------------
def pyramid_levels(n):
    levels, s = 1, n
    while s >= 8 and levels < 8:
        s = (s + 1) // 2
        levels += 1
    return levels


LEVELS = pyramid_levels(N)


_G7 = np.array([1.0, 10.0, 47.0, 76.0, 47.0, 10.0, 1.0]) / 192.0
_G7C = np.array([1.0, 6.0, 31.0, 52.0, 31.0, 6.0, 1.0]) / 128.0


def build_pyramid(f, kern=_G):
    h = len(kern) // 2
    p = [f]
    for _ in range(LEVELS - 1):
        a = p[-1]
        b = sum(w * np.roll(a, h - k, 1) for k, w in enumerate(kern))
        b = sum(w * np.roll(b, h - k, 0) for k, w in enumerate(kern))
        # Level texel p is centred on source texel 2p.
        p.append(b[0::2, 0::2])
    return p


def _weights(kind, t):
    """Per-tap weights for taps at offsets -1, 0, 1, 2 from floor."""
    if kind == "bilinear":
        z = np.zeros_like(t)
        return [z, 1 - t, t, z]
    if kind == "catmull":
        t2, t3 = t * t, t * t * t
        return [0.5 * (-t3 + 2 * t2 - t), 0.5 * (3 * t3 - 5 * t2 + 2),
                0.5 * (-3 * t3 + 4 * t2 + t), 0.5 * (t3 - t2)]
    if kind == "bspline":
        t2, t3 = t * t, t * t * t
        return [(1 - t) ** 3 / 6, (3 * t3 - 6 * t2 + 4) / 6,
                (-3 * t3 + 3 * t2 + 3 * t + 1) / 6, t3 / 6]
    raise ValueError(kind)


_UP_CACHE = {}


def _up_plan(m, kind):
    """Indices and weights to resample a periodic m-texel axis to N cells,
    texel i centred on cell i * (N / m)."""
    key = (m, kind)
    if key not in _UP_CACHE:
        s = N / m
        x = np.arange(N) / s
        i0 = np.floor(x).astype(int)
        t = x - i0
        ws = _weights(kind, t)
        idx = [(i0 + o) % m for o in (-1, 0, 1, 2)]
        _UP_CACHE[key] = (idx, ws)
    return _UP_CACHE[key]


def upsample(level, kind):
    m = level.shape[0]
    idx, ws = _up_plan(m, kind)
    rows = sum(w[:, None] * level[i, :] for i, w in zip(idx, ws))
    return sum(w[None, :] * rows[:, i] for i, w in zip(idx, ws))


def pyr_avg_factory(kind):
    def avg(p, r):
        l = np.log2(max(CAL * r, 1.0))
        top = LEVELS - 1
        lf = min(max(l, 0.0), top)
        l0 = int(np.floor(lf))
        l1 = min(l0 + 1, top)
        t = lf - np.floor(lf)
        a = upsample(p[l0], kind) if l0 > 0 else p[0]
        if t == 0.0:
            return a
        b = upsample(p[l1], kind) if l1 > 0 else p[0]
        return (1 - t) * a + t * b
    return avg


def build_pyramid_over(f):
    """Level l stored at spacing 2^(l-1), level 1 at full resolution.
    The added variance at each level is 4^(l-1) in base cells, which is
    a plain binomial at level 1 and, in the source level's texels (spacing
    2^(l-2)), a binomial dilated by 2 above it."""
    p = [f]
    a = f
    b = sum(w * np.roll(a, 2 - k, 1) for k, w in enumerate(_G))
    b = sum(w * np.roll(b, 2 - k, 0) for k, w in enumerate(_G))
    p.append(b)
    for _ in range(LEVELS - 2):
        a = p[-1]
        b = sum(w * np.roll(a, 2 * (2 - k), 1) for k, w in enumerate(_G))
        b = sum(w * np.roll(b, 2 * (2 - k), 0) for k, w in enumerate(_G))
        p.append(b[0::2, 0::2])
    return p


def pyr_over_avg_factory(kind):
    def avg(p, r):
        l = np.log2(max(CAL * r, 1.0))
        top = LEVELS - 1
        lf = min(max(l, 0.0), top)
        l0 = int(np.floor(lf))
        l1 = min(l0 + 1, top)
        t = lf - np.floor(lf)
        # Levels 0 and 1 are at full resolution: read directly.
        a = upsample(p[l0], kind) if l0 > 1 else p[l0]
        if t == 0.0:
            return a
        b = upsample(p[l1], kind) if l1 > 1 else p[l1]
        return (1 - t) * a + t * b
    return avg


METHODS = {
    "disc": ("fft", fft_avg("disc")),
    "epan": ("fft", fft_avg("epan")),
    "gauss": ("fft", fft_avg("gauss")),
    "pyr": ("pyr", pyr_avg_factory("bilinear")),
    "pyr_bspline": ("pyr", pyr_avg_factory("bspline")),
    "pyr_catmull": ("pyr", pyr_avg_factory("catmull")),
    "pyr_over": ("over", pyr_over_avg_factory("bilinear")),
    "pyr_over_bspline": ("over", pyr_over_avg_factory("bspline")),
    "pyr_g7": ("g7", pyr_avg_factory("bilinear")),
    "pyr_g7c": ("g7c", pyr_avg_factory("bilinear")),
}


_JITTER = {"rng": np.random.default_rng(0)}


def averager(method):
    if method == "pyr_jitter":
        _, avg = METHODS["pyr"]
        span = 1 << (LEVELS - 1)

        def make(f):
            sy, sx = _JITTER["rng"].integers(0, span, 2)
            p = build_pyramid(np.roll(f, (sy, sx), (0, 1)))
            return lambda r: np.roll(avg(p, r), (-sy, -sx), (0, 1))
        return make
    kind, avg = METHODS[method]
    if kind == "fft":
        return lambda f: (lambda F: (lambda r: avg(F, r)))(np.fft.rfft2(f))
    if kind == "over":
        return lambda f: (lambda p: (lambda r: avg(p, r)))(build_pyramid_over(f))
    if kind == "g7":
        return lambda f: (lambda p: (lambda r: avg(p, r)))(build_pyramid(f, _G7))
    if kind == "g7c":
        return lambda f: (lambda p: (lambda r: avg(p, r)))(build_pyramid(f, _G7C))
    return lambda f: (lambda p: (lambda r: avg(p, r)))(build_pyramid(f))


# ---------------------------------------------------------------------------
# PART A: the kernel's response to plane waves
# ---------------------------------------------------------------------------
_YY, _XX = np.mgrid[0:N, 0:N]


def response(method, ra, rb, m, n):
    f = np.cos(2 * np.pi * (m * _XX + n * _YY) / N)
    a = averager(method)(f)
    d = a(ra) - a(rb)
    return float((d * f).sum() / (f * f).sum())


def part_a(pairs, methods=None):
    print(f"PART A  N={N}: response of avg(ra) - avg(rb) to plane waves")
    print("  peak k: the wavenumber (cycles per grid) the rule grows fastest along an axis;")
    print("  axis/diag: the peak response along the axes over that along the diagonals")
    print(f"  {'method':12s} " + "  ".join(f"{f'{ra:g}/{rb:g}':>22s}" for ra, rb in pairs))
    rows = {}
    for method in (methods or METHODS):
        cells = []
        for ra, rb in pairs:
            # Scan the axis (m, 0) and the diagonal (m, m) for their
            # peaks; the diagonal's wavenumber is m * sqrt(2).
            ks_axis = range(1, min(N // 4, int(4 * N / rb) + 4))
            g_axis = [(response(method, ra, rb, m, 0), m) for m in ks_axis]
            ga, ka = max(g_axis)
            ks_diag = range(1, min(N // 4, int(4 * N / rb / np.sqrt(2)) + 4))
            g_diag = [(response(method, ra, rb, m, m), m * np.sqrt(2)) for m in ks_diag]
            gd, kd = max(g_diag)
            cells.append((ka, ga / gd if gd > 0 else float("nan"), ga))
        rows[method] = cells
        print(f"  {method:12s} " + "  ".join(f"k {k:5.1f} a/d {r:6.4f}      " for k, r, _ in cells))
    return rows


# ---------------------------------------------------------------------------
# PART B: the dynamics
# ---------------------------------------------------------------------------
def step(f, table, method):
    a = averager(method)(f)
    best_v = None
    best_d = None
    cache = {}

    def get(r):
        if r not in cache:
            cache[r] = a(r)
        return cache[r]

    for ra, ratio, amt in table:
        act, inh = get(ra), get(ra * ratio)
        v = np.abs(act - inh)
        d = np.where(act > inh, amt, -amt)
        if best_v is None:
            best_v, best_d = v, d
        else:
            m = v < best_v
            best_v = np.where(m, v, best_v)
            best_d = np.where(m, d, best_d)
    f = f + best_d
    lo, hi = f.min(), f.max()
    return (f - lo) / max(hi - lo, 1e-9) * 2 - 1


def axes_over_diagonals(f):
    g = f - f.mean()
    P = np.abs(np.fft.fftshift(np.fft.fft2(g))) ** 2
    y, x = np.mgrid[-N // 2:N // 2, -N // 2:N // 2]
    r = np.hypot(x, y)
    th = np.degrees(np.arctan2(y, x)) % 180
    band = (r > 2) & (r < 60 * N / 512)
    e = lambda lo, hi: P[band & (th >= lo) & (th < hi)].sum()
    return (e(0, 10) + e(170, 180) + e(80, 100)) / (e(35, 55) + e(125, 145))


TABLES = {
    # Radii 1, 3, 10, 20, 45 at ratio 2: Reusser's order, coarse fastest,
    # and the shipped preset's, fine fastest.
    "coarse_fast": [(1, 2, .01), (3, 2, .02), (10, 2, .02), (20, 2, .03), (45, 2, .04)],
    "fine_fast": [(1, 2, .05), (3, 2, .04), (10, 2, .03), (20, 2, .02), (45, 2, .01)],
}


def part_b(methods, seeds, steps=200):
    print(f"PART B  N={N}, {steps} steps, seeds {list(seeds)}: axes/diagonals of the final field")
    only = os.environ.get("ISO_TABLES")
    for tname, table in TABLES.items():
        if only and tname not in only.split(","):
            continue
        for method in methods:
            vals = []
            t0 = time.time()
            for seed in seeds:
                rng = np.random.default_rng(seed)
                _JITTER["rng"] = np.random.default_rng(1000 + seed)
                f = rng.uniform(-1, 1, (N, N))
                for _ in range(steps):
                    f = step(f, table, method)
                vals.append(axes_over_diagonals(f))
                if seed == seeds[0]:
                    from PIL import Image
                    img = ((np.clip(f, -1, 1) + 1) * 127.5).astype(np.uint8)
                    Image.fromarray(img).save(f"{OUT}/{tname}_{method}.png")
            print(f"  {tname:12s} {method:12s} mean {np.mean(vals):.2f}  sd {np.std(vals):.2f}  "
                  f"({', '.join(f'{v:.2f}' for v in vals)})  {time.time() - t0:.0f}s", flush=True)


if __name__ == "__main__":
    parts = sys.argv[1:] or ["A", "B"]
    if "A" in parts:
        only = os.environ.get("ISO_METHODS_A")
        part_a([(3, 6), (10, 20), (20, 40), (45, 90)], only.split(",") if only else None)
    if "B" in parts:
        meths = os.environ.get("ISO_METHODS", "disc,pyr").split(",")
        nseeds = int(os.environ.get("ISO_SEEDS", "4"))
        part_b(meths, range(1, nseeds + 1), int(os.environ.get("ISO_STEPS", "200")))
