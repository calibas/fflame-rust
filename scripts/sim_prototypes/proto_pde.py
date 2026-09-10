"""Swift-Hohenberg and Cahn-Hilliard validation prototypes.

Phase-3 measurement for simulation mode. Both are FOURTH-ORDER PDEs
and both are catalogue entries whose every constant is marked
`[verify]` or `[verify by prototype]`. The catalogue's rule is that
nothing ships as a preset that has not been run; these are the runs.

Answers, per model:
  - does the named parameter set produce the named pattern
  - the largest dt that stays finite, against the derived bound
  - the pattern wavelength, against the theory (Swift-Hohenberg's
    lambda = 2*pi/q0 is a PREDICTION, and the thing most likely to be
    wrong if the discretisation is)
  - steps to a still, or that it never stills
  - Cahn-Hilliard's mean composition, which the divergence form
    conserves exactly and a wrong sign or a stray term does not

THE LAPLACIAN IS NOT THE ONE THE OTHER MODELS USE. Phase 1 and 2's
reaction-diffusion models take Karl Sims' 3x3 kernel (centre -1, edge
0.2, corner 0.05), whose Fourier symbol is -0.3*k^2 at small k -- a
Laplacian scaled by 0.3. For a second-order model that scale is
absorbed into the free diffusion constant and nobody can tell. These
two are different: Swift-Hohenberg's operator (q0^2 + lap)^2 SELECTS
the wavelength where lap = -q0^2, so a 0.3 scale would move the
selected wavelength by 1/sqrt(0.3) and the documented lambda = 2*pi/q0
would be wrong by 83%. Both models here use the standard 5-point
Laplacian (centre -4, edge 1), which is also what the catalogue's
stability bounds assume.

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
N = 256


def lap5(a):
    """Standard 5-point Laplacian, periodic, h = 1.

    Symbol -4 + 2cos(kx) + 2cos(ky), i.e. -k^2 at small k and -8 at the
    checkerboard. Both the stability bounds and the wavelength
    prediction below are stated against exactly this."""
    return (np.roll(a, -1, 0) + np.roll(a, 1, 0)
            + np.roll(a, -1, 1) + np.roll(a, 1, 1) - 4.0 * a)


def save(name, field, lo=None, hi=None):
    f = np.asarray(field, dtype=np.float64)
    lo = f.min() if lo is None else lo
    hi = f.max() if hi is None else hi
    span = hi - lo
    if span <= 0:
        span = 1.0
    img = np.clip((f - lo) / span, 0, 1)
    Image.fromarray((img * 255).astype(np.uint8)).save(os.path.join(OUT, name))


def dominant_wavelength(f):
    """Peak of the radially averaged power spectrum, in cells.

    The observable for Swift-Hohenberg: the model's whole claim is that
    it selects one wavelength, so a spectrum with no clear peak is a
    failure even if the picture looks textured."""
    g = f - f.mean()
    p = np.abs(np.fft.fft2(g)) ** 2
    ky = np.fft.fftfreq(g.shape[0]) * g.shape[0]
    kx = np.fft.fftfreq(g.shape[1]) * g.shape[1]
    kr = np.sqrt(ky[:, None] ** 2 + kx[None, :] ** 2)
    bins = np.arange(0.5, min(g.shape) // 2, 1.0)
    idx = np.digitize(kr.ravel(), bins)
    power = np.bincount(idx, weights=p.ravel(), minlength=len(bins) + 1)
    count = np.bincount(idx, minlength=len(bins) + 1)
    prof = power[1:len(bins)] / np.maximum(count[1:len(bins)], 1)
    if prof.size == 0 or prof.max() <= 0:
        return float("nan"), 0.0
    k = bins[:len(prof)][int(np.argmax(prof))]
    # Sharpness: peak over mean, so a flat spectrum reads ~1.
    return float(g.shape[0] / k), float(prof.max() / prof.mean())


# ---------------------------------------------------------------------
# Swift-Hohenberg:  du/dt = r u - (q0^2 + lap)^2 u + g u^2 - u^3
#
# Two passes, exactly as the shader will do it:
#   w = q0^2 u + lap u          (pass 1, into a spare channel)
#   du/dt = r u - (q0^2 w + lap w) + g u^2 - u^3     (pass 2)
# ---------------------------------------------------------------------
def sh_step(u, r, q0sq, g, dt):
    w = q0sq * u + lap5(u)
    bi = q0sq * w + lap5(w)
    return u + dt * (r * u - bi + g * u * u - u * u * u)


def sh_run(r, q0, g, dt, steps, seed=7, snap=None, name=None):
    q0sq = q0 * q0
    rng = np.random.default_rng(seed)
    u = rng.uniform(-0.1, 0.1, (N, N))
    prev = None
    settle = None
    for i in range(1, steps + 1):
        u = sh_step(u, r, q0sq, g, dt)
        if not np.all(np.isfinite(u)):
            return None, i, None, None
        if snap and i in snap and name:
            save(f"{name}_{i}.png", u)
        if i % 100 == 0:
            if prev is not None:
                # Mean |du| per step over the window, relative to the
                # field's own scale: a still is a field that stops
                # moving, not one that stops changing sign.
                d = np.abs(u - prev).mean() / max(np.abs(u).mean(), 1e-9) / 100.0
                if settle is None and d < 1e-4:
                    settle = i
            prev = u.copy()
    return u, None, settle, None


def sh_sweep(rows):
    """Where does Swift-Hohenberg actually make a PATTERN?

    The catalogue's r = 0.2 with q0 = 2*pi/16 does not: it makes
    coarsening blobs at ~100 cells. The reason is that the band-pass is
    only as selective as q0^4. Growth at the band is r; growth at k = 0
    is r - q0^4. With q0 = 0.3927 that is 0.2 against 0.176 -- the
    uniform mode grows nearly as fast as the pattern, and the cubic
    then quenches it into +-1 domains. The textbook q0 = 1 hides this
    because q0^4 = 1 swamps any sensible r.

    So r has to be read RELATIVE to q0^4, and this sweep measures the
    ratio at which selection actually wins."""
    print("\n=== Swift-Hohenberg: drive relative to band selectivity ===")
    lam = 16.0
    q0 = 2.0 * np.pi / lam
    q4 = q0 ** 4
    bound = 2.0 / (8.0 - q0 * q0) ** 2
    dt = bound * 0.9
    print(f"q0={q0:.4f}  q0^4={q4:.5f}  dt={dt:.5f}  lambda_target={lam}")
    print(f"{'r/q0^4':>7} {'r':>8} {'g':>4} {'lambda':>8} {'sharp':>7} {'skew':>7} {'sd':>7} {'settle':>7}")
    for ratio in [0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 8.4]:
        for g in [0.0, 0.5, 1.0]:
            r = ratio * q4
            u, blew, settle, _ = sh_run(r, q0, g, dt, 12000)
            if u is None:
                print(f"{ratio:>7.2f} {r:>8.5f} {g:>4} diverged at {blew}")
                continue
            wl, sharp = dominant_wavelength(u)
            v = (u - u.mean()) / max(u.std(), 1e-12)
            skew = float((v ** 3).mean())
            print(f"{ratio:>7.2f} {r:>8.5f} {g:>4} {wl:>8.2f} {sharp:>7.1f} "
                  f"{skew:>+7.3f} {u.std():>7.4f} {str(settle):>7}")
            rows.append(dict(model="swift_hohenberg", sweep="drive_ratio", ratio=ratio,
                             r=r, g=g, q0=q0, dt=dt, wavelength=wl, sharpness=sharp,
                             skew=skew, sd=float(u.std()), settle=settle))


def sh_g_sweep(rows):
    """Where do HEXAGONS live?

    Every g > 0 in the drive sweep died: the field went uniform. The
    quadratic term g*u^2 competes with the cubic -u^3 at the pattern's
    own amplitude, which is ~sqrt(r) -- so g = 1 against r = 0.05 is
    not a symmetry-breaking nudge, it is the dominant term, and it
    drives the field to the uniform fixed point near u = g. Hexagons
    need g comparable to sqrt(r), not to 1.

    The discriminator is SKEW. A hexagonal lattice has three Fourier
    modes at 120 degrees whose sum has sharp peaks and broad troughs
    (or the reverse), so its one-point distribution is skewed; stripes
    are two modes and symmetric, skew 0. Eyes are not needed."""
    print("\n=== Swift-Hohenberg: hexagons need g ~ sqrt(r) ===")
    lam = 16.0
    q0 = 2.0 * np.pi / lam
    q4 = q0 ** 4
    dt = 0.9 * 2.0 / (8.0 - q0 * q0) ** 2
    print(f"{'r/q0^4':>7} {'r':>8} {'g':>6} {'g/sqrt(r)':>10} {'lambda':>8} "
          f"{'sharp':>7} {'skew':>7} {'sd':>7} {'settle':>7}")
    for ratio in [2.0, 4.0]:
        r = ratio * q4
        for g in [0.0, 0.05, 0.1, 0.15, 0.2, 0.3, 0.45]:
            u, blew, settle, _ = sh_run(r, q0, g, dt, 12000)
            if u is None:
                print(f"{ratio:>7.2f} {r:>8.5f} {g:>6} diverged at {blew}")
                continue
            wl, sharp = dominant_wavelength(u)
            v = (u - u.mean()) / max(u.std(), 1e-12)
            skew = float((v ** 3).mean())
            print(f"{ratio:>7.2f} {r:>8.5f} {g:>6} {g / np.sqrt(r):>10.2f} "
                  f"{wl:>8.2f} {sharp:>7.1f} {skew:>+7.3f} {u.std():>7.4f} {str(settle):>7}")
            rows.append(dict(model="swift_hohenberg", sweep="g", ratio=ratio, r=r, g=g,
                             g_rel=float(g / np.sqrt(r)), wavelength=wl, sharpness=sharp,
                             skew=skew, sd=float(u.std()), settle=settle))
    # Pictures for the two that matter.
    for label, g in [("stripes", 0.0), ("hexagons", 0.15)]:
        r = 2.0 * q4
        sh_run(r, q0, g, dt, 12000, snap={12000}, name=f"shx_{label}")


def swift_hohenberg(rows):
    print("\n=== Swift-Hohenberg ===")
    # The catalogue's derived bound with the 5-point Laplacian:
    # dt <= 2 / (8 - q0^2)^2.
    for label, r, lam, g in [
        ("stripes", 0.2, 16.0, 0.0),
        ("hexagons", 0.1, 16.0, 1.0),
    ]:
        q0 = 2.0 * np.pi / lam
        bound = 2.0 / (8.0 - q0 * q0) ** 2
        print(f"\n{label}: r={r} lambda_target={lam} q0={q0:.4f} g={g} "
              f"derived dt bound {bound:.4f}")
        # Stability ladder around the bound.
        ladder = {}
        for dt in [bound * 0.5, bound * 0.9, bound, bound * 1.1, bound * 1.5, bound * 2.0]:
            _, blew, _, _ = sh_run(r, q0, g, dt, 4000)
            ladder[round(dt, 5)] = "diverged at %d" % blew if blew else "stable"
            print(f"   dt={dt:.5f}  {ladder[round(dt, 5)]}")
        dt = bound * 0.9
        t0 = time.time()
        u, blew, settle, _ = sh_run(r, q0, g, dt, 40000, snap={200, 1000, 5000, 40000},
                                    name=f"sh_{label}")
        ms = (time.time() - t0) * 1e3 / 40000
        wl, sharp = dominant_wavelength(u)
        # Hexagons vs stripes: the third-harmonic content of a hexagonal
        # lattice makes the field's distribution skewed; stripes are
        # symmetric. Skew is the discriminator that does not need eyes.
        v = (u - u.mean()) / max(u.std(), 1e-12)
        skew = float((v ** 3).mean())
        print(f"   dt={dt:.5f} 40,000 steps in {ms:.3f} ms/step")
        print(f"   wavelength {wl:.2f} cells (target {lam}), peak sharpness {sharp:.1f}")
        print(f"   skew {skew:+.3f}   settled at {settle}   sd {u.std():.4f}")
        rows.append(dict(model="swift_hohenberg", preset=label, r=r, q0=q0, g=g,
                         dt=dt, dt_bound=bound, ladder=ladder, wavelength=wl,
                         wavelength_target=lam, sharpness=sharp, skew=skew,
                         settle=settle, sd=float(u.std()), ms_per_step=ms))


# ---------------------------------------------------------------------
# Cahn-Hilliard:  dc/dt = D lap( c^3 - c - gamma lap c )
#
# Two passes:
#   mu = c^3 - c - gamma lap c      (pass 1, into a spare channel)
#   dc/dt = D lap mu                (pass 2)
#
# The mean of c is conserved EXACTLY: the update is a discrete
# divergence, and lap of anything sums to zero on a periodic lattice.
# That is the test no picture can fake.
# ---------------------------------------------------------------------
def ch_step(c, d, gamma, dt):
    mu = c * c * c - c - gamma * lap5(c)
    return c + dt * d * lap5(mu)


def domain_size(f):
    """First moment of the structure factor, inverted: the mean domain
    size in cells. Coarsening is the model's headline behaviour and
    L ~ t^(1/3) is the law to check it against."""
    g = f - f.mean()
    p = np.abs(np.fft.fft2(g)) ** 2
    ky = np.fft.fftfreq(g.shape[0]) * g.shape[0]
    kx = np.fft.fftfreq(g.shape[1]) * g.shape[1]
    kr = np.sqrt(ky[:, None] ** 2 + kx[None, :] ** 2)
    m = kr > 0
    k1 = (kr[m] * p[m]).sum() / max(p[m].sum(), 1e-30)
    return float(g.shape[0] / k1) if k1 > 0 else float("nan")


def cahn_hilliard(rows):
    print("\n=== Cahn-Hilliard ===")
    for label, mean in [("labyrinth", 0.0), ("droplets", 0.4)]:
        d, gamma = 1.0, 0.5
        # The catalogue derived 1/(32 D gamma) = 0.0625 from the gamma
        # term alone. Measured, that is NOT a bound: at 0.05625 the
        # labyrinth run was finite for 400 steps and infinite by 1,000.
        # Linearising about |c| = 1 keeps the cubic, whose contribution
        # is the same order:
        #   symbol = D L (3c^2 - 1 - gamma L),  L in [-8, 0]
        #   most negative at L = -8:  -8 D (3c^2 - 1) - 64 D gamma
        # so dt <= 2 / (D (16 + 64 gamma)).
        bound = 2.0 / (d * (16.0 + 64.0 * gamma))
        print(f"\n{label}: mean={mean} D={d} gamma={gamma} derived dt bound {bound:.5f}")
        ladder = {}
        for dt in [bound * 0.5, bound * 0.96, bound, bound * 1.1, bound * 1.35,
                   bound * 1.5, bound * 2.0]:
            rng = np.random.default_rng(3)
            c = mean + rng.uniform(-0.05, 0.05, (N, N))
            blew = None
            for i in range(1, 4000 + 1):
                c = ch_step(c, d, gamma, dt)
                if not np.all(np.isfinite(c)) or np.abs(c).max() > 10:
                    blew = i
                    break
            ladder[round(dt, 6)] = "diverged at %d" % blew if blew else "stable"
            print(f"   dt={dt:.6f}  {ladder[round(dt, 6)]}")

        dt = bound * 0.96
        rng = np.random.default_rng(3)
        c = mean + rng.uniform(-0.05, 0.05, (N, N))
        m0 = c.mean()
        drift = 0.0
        sizes = []
        t0 = time.time()
        steps = 40000
        for i in range(1, steps + 1):
            c = ch_step(c, d, gamma, dt)
            drift = max(drift, abs(c.mean() - m0))
            if i in (200, 1000, 5000, 20000, steps):
                save(f"ch_{label}_{i}.png", c, -1.1, 1.1)
                sizes.append((i, domain_size(c)))
        ms = (time.time() - t0) * 1e3 / steps
        print(f"   {steps} steps in {ms:.3f} ms/step")
        ok = "OK" if drift < 1e-5 else "FAILED -- not conserved (or the run diverged)"
        print(f"   mean drift {drift:.3e}  {ok}")
        print(f"   c in [{c.min():.3f}, {c.max():.3f}]  sd {c.std():.4f}")
        for i, s in sizes:
            print(f"   step {i:>6}: domain size {s:.2f} cells")
        # Lifshitz-Slyozov: L ~ t^(1/3). Fit the exponent over the run.
        ts = np.array([i for i, _ in sizes], dtype=float)
        ls = np.array([s for _, s in sizes], dtype=float)
        ok = np.isfinite(ls) & (ls > 0)
        expo = float(np.polyfit(np.log(ts[ok]), np.log(ls[ok]), 1)[0]) if ok.sum() > 2 else float("nan")
        print(f"   coarsening exponent {expo:.3f} (Lifshitz-Slyozov says 1/3)")
        rows.append(dict(model="cahn_hilliard", preset=label, mean=mean, D=d,
                         gamma=gamma, dt=dt, dt_bound=bound, ladder=ladder,
                         mean_drift=float(drift), sd=float(c.std()),
                         domain_sizes=sizes, coarsening_exponent=expo,
                         ms_per_step=ms))


if __name__ == "__main__":
    rows = []
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    if which in ("all", "sweep"):
        sh_sweep(rows)
    if which in ("all", "gsweep"):
        sh_g_sweep(rows)
    if which in ("all", "sh"):
        swift_hohenberg(rows)
    if which in ("all", "ch"):
        cahn_hilliard(rows)
    with open(os.path.join(OUT, "pde.json"), "w") as fh:
        json.dump(rows, fh, indent=1)
    print(f"\nwrote {OUT}/pde.json and PNGs")
