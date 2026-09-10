"""Lenia and SmoothLife validation prototypes.

Phase-3 wave 3. Both are continuous cellular automata whose rule is a
LARGE-KERNEL convolution rather than a stencil, and both are catalogue
entries whose formulas were read from a secondary statement rather
than from the papers -- unlike Kobayashi and Tyson-Fife, no PDF was
available for these, so what is checked here is that the formulas AS
RECORDED produce the behaviour claimed for them.

What the catalogue records, and what this runs:

Lenia (catalogue section 8, formulas as stated on Wikipedia's account
of Chan 2019):
    A' = clip( A + dt * G(K * A), 0, 1 )
    K_C(r) = exp(alpha - alpha / (4 r (1 - r))),  alpha = 4
    K      = K_C(|x|/R) / sum K_C
    G(u)   = 2 exp(-(u - mu)^2 / (2 sigma^2)) - 1

SmoothLife (catalogue section 9, from Rafler arXiv:1111.1567):
    m = disc average over |x| < r_i        ("cell filling")
    n = annulus average over r_i < |x| < r_a
    sigma(x,a,al)   = 1 / (1 + exp(-(x-a) * 4/al))
    sigma_n(x,a,b)  = sigma(x,a,al_n) * (1 - sigma(x,b,al_n))
    sigma_m(x,y,m)  = x (1 - sigma(m,0.5,al_m)) + y sigma(m,0.5,al_m)
    s(n,m)          = sigma_n( n, sigma_m(b1,d1,m), sigma_m(b2,d2,m) )
    f' = f + dt (s(n,m) - f)          [the form that stays in [0,1]]
  Glider set: r_a = 21, r_i = 7, b1 = 0.278, b2 = 0.365, d1 = 0.267,
  d2 = 0.445, al_n = 0.028, al_m = 0.147.

The convolutions here are done by FFT because it is exact and this is
a reference; the shader does a direct gather against a weight table,
which is the same sum.

Writes PNGs and a JSON row set into output/sim_proto/.
"""
import json
import os
import sys
import time

import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)


def save(name, f, lo=0.0, hi=1.0):
    a = np.clip((np.asarray(f, dtype=np.float64) - lo) / max(hi - lo, 1e-12), 0, 1)
    Image.fromarray((np.nan_to_num(a) * 255).astype(np.uint8)).save(
        os.path.join(OUT, name))


def wrap_kernel(k, n):
    """Centre a (2R+1)^2 kernel into an n x n periodic array for FFT."""
    r = k.shape[0] // 2
    big = np.zeros((n, n))
    big[:k.shape[0], :k.shape[1]] = k
    return np.roll(np.roll(big, -r, 0), -r, 1)


def convolve(field, kernel_fft):
    return np.real(np.fft.ifft2(np.fft.fft2(field) * kernel_fft))


# =====================================================================
# Lenia
# =====================================================================
def lenia_kernel(R, rings=1, betas=(1.0, 0.0, 0.0), alpha=4.0):
    """The exponential core, scaled to radius R and normalised to 1.

    Multi-ring follows Chan's construction: the radius is multiplied by
    the ring count, the integer part picks the ring's peak beta, and
    the fractional part is the core's argument."""
    r = np.arange(-R, R + 1)
    yy, xx = np.meshgrid(r, r, indexing="ij")
    d = np.sqrt(xx * xx + yy * yy) / R
    k = np.zeros_like(d)
    m = d < 1.0
    dm = d[m] * rings
    idx = np.minimum(dm.astype(int), rings - 1)
    frac = np.clip(dm - idx, 1e-9, 1 - 1e-9)
    core = np.exp(alpha - alpha / (4.0 * frac * (1.0 - frac)))
    k[m] = np.array(betas)[idx] * core
    total = k.sum()
    return k / total if total > 0 else k


def lenia_growth(u, mu, sigma):
    return 2.0 * np.exp(-((u - mu) ** 2) / (2.0 * sigma * sigma)) - 1.0


def lenia_run(N, R, mu, sigma, dt, steps, seed=1, rings=1,
              betas=(1.0, 0.0, 0.0), snaps=(), name="lenia"):
    rg = np.random.default_rng(seed)
    # A coarse soup: Lenia's structures are R-scale, so seeding at the
    # cell scale gives noise the kernel simply averages away.
    small = rg.random((N // R + 1, N // R + 1))
    a = np.kron(small, np.ones((R, R)))[:N, :N]
    kf = np.fft.fft2(wrap_kernel(lenia_kernel(R, rings, betas), N))
    hist = []
    for i in range(1, steps + 1):
        u = convolve(a, kf)
        a = np.clip(a + dt * lenia_growth(u, mu, sigma), 0.0, 1.0)
        if not np.all(np.isfinite(a)):
            return None, i, hist
        if i in snaps:
            save(f"{name}_{i}.png", a)
        hist.append((float(a.mean()), float(a.std())))
    save(f"{name}.png", a)
    return a, None, hist


def lenia(rows):
    print("\n=== Lenia ===")
    print(f"{'R':>4} {'mu':>6} {'sigma':>7} {'dt':>6} {'mean':>7} {'sd':>7} "
          f"{'edge%':>7} {'moving':>9}  verdict")
    for R, mu, sigma, dt in [
        (13, 0.15, 0.015, 0.1),   # the catalogue's Orbium constants
        (13, 0.15, 0.03, 0.1),
        (13, 0.3, 0.05, 0.1),
        (13, 0.35, 0.07, 0.1),    # the catalogue's "Life-like" pair
        (10, 0.2, 0.04, 0.1),
    ]:
        a, blew, hist = lenia_run(256, R, mu, sigma, dt, 600,
                                  snaps=(600,), name=f"lenia_R{R}_m{mu}_s{sigma}")
        if a is None:
            print(f"{R:>4} {mu:>6} {sigma:>7} {dt:>6} diverged at {blew}")
            continue
        # "Alive" means structure: a field that is neither all-empty
        # nor all-full, with soft edges, and still changing.
        edge = float(((a > 0.05) & (a < 0.95)).mean()) * 100
        moving = abs(hist[-1][0] - hist[-30][0])
        verdict = ("structures" if 0.02 < a.mean() < 0.7 and edge > 3
                   else "dead" if a.mean() < 0.02 else "saturated")
        print(f"{R:>4} {mu:>6} {sigma:>7} {dt:>6} {a.mean():>7.4f} {a.std():>7.4f} "
              f"{edge:>6.1f}% {moving:>9.2e}  {verdict}")
        rows.append(dict(model="lenia", R=R, mu=mu, sigma=sigma, dt=dt,
                         mean=float(a.mean()), sd=float(a.std()), edge_pct=edge,
                         verdict=verdict))


# =====================================================================
# SmoothLife
# =====================================================================
def smoothlife_kernels(ri, ra, n):
    """Anti-aliased disc and annulus, each normalised to sum 1.

    The paper ramps the weight linearly across the boundary pixel
    (band width 1), which is what keeps a smooth rule from inheriting
    the lattice's own square symmetry."""
    R = int(np.ceil(ra)) + 1
    r = np.arange(-R, R + 1)
    yy, xx = np.meshgrid(r, r, indexing="ij")
    d = np.sqrt(xx * xx + yy * yy)
    inner = np.clip(ri + 0.5 - d, 0.0, 1.0)
    outer = np.clip(ra + 0.5 - d, 0.0, 1.0) * (1.0 - inner)
    inner = inner / inner.sum()
    outer = outer / outer.sum()
    return (np.fft.fft2(wrap_kernel(inner, n)),
            np.fft.fft2(wrap_kernel(outer, n)), R)


def sl_sigma(x, a, al):
    return 1.0 / (1.0 + np.exp(-(x - a) * 4.0 / al))


def smoothlife_run(N, ri, ra, b1, b2, d1, d2, al_n, al_m, dt, steps,
                   seed=2, snaps=(), name="smoothlife"):
    rg = np.random.default_rng(seed)
    # Patches the size of the inner disc: a per-cell random field is
    # averaged flat by a radius-21 kernel before anything can happen.
    blk = max(int(ri), 1)
    small = (rg.random((N // blk + 1, N // blk + 1)) < 0.5).astype(float)
    f = np.kron(small, np.ones((blk, blk)))[:N, :N]
    kin, kout, R = smoothlife_kernels(ri, ra, N)
    for i in range(1, steps + 1):
        m = convolve(f, kin)
        n_ = convolve(f, kout)
        thr_lo = b1 * (1.0 - sl_sigma(m, 0.5, al_m)) + d1 * sl_sigma(m, 0.5, al_m)
        thr_hi = b2 * (1.0 - sl_sigma(m, 0.5, al_m)) + d2 * sl_sigma(m, 0.5, al_m)
        s = sl_sigma(n_, thr_lo, al_n) * (1.0 - sl_sigma(n_, thr_hi, al_n))
        f = np.clip(f + dt * (s - f), 0.0, 1.0)
        if not np.all(np.isfinite(f)):
            return None, i, R
        if i in snaps:
            save(f"{name}_{i}.png", f)
    save(f"{name}.png", f)
    return f, None, R


def smoothlife(rows):
    print("\n=== SmoothLife (Rafler's glider set) ===")
    print(f"{'r_i':>5} {'r_a':>5} {'dt':>6} {'mean':>7} {'sd':>7} {'edge%':>7}  verdict")
    for ri, ra, dt in [(7.0, 21.0, 0.1), (7.0, 21.0, 0.3), (4.0, 12.0, 0.1)]:
        f, blew, R = smoothlife_run(256, ri, ra, 0.278, 0.365, 0.267, 0.445,
                                    0.028, 0.147, dt, 400,
                                    snaps=(100, 400), name=f"sl_ri{ri}_dt{dt}")
        if f is None:
            print(f"{ri:>5} {ra:>5} {dt:>6} diverged at {blew}")
            continue
        edge = float(((f > 0.05) & (f < 0.95)).mean()) * 100
        verdict = ("structures" if 0.02 < f.mean() < 0.7 else
                   "dead" if f.mean() < 0.02 else "saturated")
        print(f"{ri:>5} {ra:>5} {dt:>6} {f.mean():>7.4f} {f.std():>7.4f} "
              f"{edge:>6.1f}%  {verdict}  (kernel radius {R})")
        rows.append(dict(model="smoothlife", ri=ri, ra=ra, dt=dt,
                         mean=float(f.mean()), sd=float(f.std()),
                         edge_pct=edge, verdict=verdict))


if __name__ == "__main__":
    rows = []
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    if which in ("all", "lenia"):
        lenia(rows)
    if which in ("all", "sl"):
        smoothlife(rows)
    with open(os.path.join(OUT, "large_kernel.json"), "w") as fh:
        json.dump(rows, fh, indent=1)
    print(f"\nwrote {OUT}/large_kernel.json and PNGs")
