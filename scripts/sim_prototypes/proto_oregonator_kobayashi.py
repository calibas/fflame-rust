"""Oregonator and Kobayashi validation prototypes.

Phase-3 wave 2. Both catalogue entries were written from memory and
marked `[verify]`; Kobayashi's said outright that "the paper must be
read before any of this ships". Both papers were then supplied, and
this script is the run that the catalogue's rule demands on top of
reading them.

WHAT THE PAPERS SETTLED (before any of this executed):

Kobayashi, Physica D 63 (1993) 410-423, section 2 and section 3:
  tau dp/dt = -d/dx(eps eps' dp/dy) + d/dy(eps eps' dp/dx)
              + div(eps^2 grad p) + p(1-p)(p - 1/2 + m)
  dT/dt     = lap T + K dp/dt
  m(T)      = (alpha/pi) arctan(gamma (T_e - T))     [paper's own choice,
                                                      chosen so |m| < 1/2]
  eps(theta)= eps_bar (1 + delta cos(j(theta - theta0))),  theta = angle of grad p
  Fixed in EVERY simulation: eps_bar = 0.01, tau = 0.0003, alpha = 0.9,
  gamma = 10.0, noise amplitude a = 0.01, dt = 0.0002, domain 9.0 x 9.0
  on a 300 x 300 mesh (so dx = 0.03), T_e = 1, zero-flux boundary.
  Noise is added to the dynamical term as a*p(1-p)*chi, chi uniform on
  [-1/2, 1/2].
  Ice dendrites (fig. 8): delta = 0.040, j = 6, theta0 = pi/2, K varied
  0.8 .. 2.0, nucleation at the centre of the vessel.
  Four-fold dendrites (fig. 7): j = 4, theta0 = 0, K = 2.0, delta varied.

Tyson and Fife, J. Chem. Phys. 73 (1980) 2224, eq. (17):
  eps du/dt = u(1-u) - b v (u-a)/(u+a)
      dv/dt = u - v
  with (a, b) the paper's names for what the later literature calls
  (q, f) -- Table I of the paper gives the correspondence -- and
  eps << 1, a << 1, b ~ 1 (eq. 16). The paper is analytic and gives NO
  numeric set for a 2-D simulation, so the parameter values are the
  thing this prototype has to establish rather than confirm.

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


def save(name, field, lo=None, hi=None):
    f = np.asarray(field, dtype=np.float64)
    lo = np.nanmin(f) if lo is None else lo
    hi = np.nanmax(f) if hi is None else hi
    span = hi - lo
    if span <= 0:
        span = 1.0
    img = np.clip((f - lo) / span, 0, 1)
    img = np.nan_to_num(img)
    Image.fromarray((img * 255).astype(np.uint8)).save(os.path.join(OUT, name))


def lap_sims(a):
    """Karl Sims' 3x3 kernel -- what every second-order reaction-
    diffusion model in this engine already uses."""
    up = np.roll(a, -1, 0)
    dn = np.roll(a, 1, 0)
    lf = np.roll(a, -1, 1)
    rt = np.roll(a, 1, 1)
    return (-a + 0.2 * (up + dn + lf + rt)
            + 0.05 * (np.roll(up, -1, 1) + np.roll(up, 1, 1)
                      + np.roll(dn, -1, 1) + np.roll(dn, 1, 1)))


# =====================================================================
# Oregonator (Tyson-Fife two-variable form)
# =====================================================================
def oreg_step(u, v, eps, q, f, du, dv, dt):
    react_u = u * (1.0 - u) - f * v * (u - q) / (u + q)
    nu = u + dt * (du * lap_sims(u) + react_u) / eps
    nv = v + dt * (dv * lap_sims(v) + u - v)
    # u and v are concentrations.
    return np.maximum(nu, 0.0), np.maximum(nv, 0.0)


def oreg_run(eps, q, f, du, dv, dt, steps, n=256, seed=3, snap=None, name=None):
    """Broken-wavefront seed: the standard way to nucleate a spiral.

    A noise seed in an EXCITABLE medium relaxes to the rest state --
    the same trap FitzHugh-Nagumo fell into in phase 2 -- so the seed
    is a half-plane of excitation with a quarter-plane of refractory
    behind it, and the free end curls."""
    u = np.zeros((n, n))
    v = np.zeros((n, n))
    u[: n // 2, : n // 2] = 1.0
    v[n // 2:, : n // 2] = 0.3
    for i in range(1, steps + 1):
        u, v = oreg_step(u, v, eps, q, f, du, dv, dt)
        if not np.all(np.isfinite(u)) or u.max() > 1e3:
            return None, None, i
        if snap and i in snap and name:
            save(f"{name}_{i}.png", u, 0.0, 1.0)
    return u, v, None


def oregonator(rows):
    print("\n=== Oregonator: stability, then where spirals live ===")
    # Reaction stiffness is 1/eps times the u-nullcline slope, and the
    # (u-q)/(u+q) term's derivative blows up as q -> 0:
    #   d/du [ f v (u-q)/(u+q) ] = 2 f v q / (u+q)^2  -> 2 f v / q at u=0.
    # Plus diffusion du*1.6/eps on the Sims stencil. Ladder against it.
    eps, q, f, du, dv = 0.04, 0.002, 1.4, 1.0, 0.6
    print(f"eps={eps} q={q} f={f} D_u={du} D_v={dv}")
    for dt in [2e-5, 5e-5, 1e-4, 2e-4, 5e-4, 1e-3]:
        u, v, blew = oreg_run(eps, q, f, du, dv, dt, 3000)
        state = f"diverged at {blew}" if blew else f"stable (u max {u.max():.3f})"
        print(f"   dt={dt:.0e}  {state}")
        rows.append(dict(model="oregonator", sweep="dt", dt=dt, diverged=blew))

    print("\n   f sweep at dt = 5e-5, 6,000 steps (the paper's eq. 28 puts")
    print("   the excitability threshold near b = 0.656):")
    print(f"   {'f':>5} {'u mean':>8} {'u sd':>8} {'moving':>8}  verdict")
    for fv in [0.6, 0.9, 1.4, 2.0, 3.0]:
        u, v, blew = oreg_run(eps, q, fv, du, dv, 5e-5, 6000)
        if u is None:
            print(f"   {fv:>5} diverged at {blew}")
            continue
        u2, _, _ = oreg_run(eps, q, fv, du, dv, 5e-5, 6200)
        moving = float(np.abs(u2 - u).mean())
        verdict = "waves" if u.std() > 0.05 and moving > 1e-4 else "flat/dead"
        print(f"   {fv:>5} {u.mean():>8.4f} {u.std():>8.4f} {moving:>8.2e}  {verdict}")
        rows.append(dict(model="oregonator", sweep="f", f=fv, mean=float(u.mean()),
                         sd=float(u.std()), moving=moving, verdict=verdict))

    # Pictures at the most promising set.
    for fv in [1.4, 2.0]:
        oreg_run(eps, q, fv, du, dv, 5e-5, 12000,
                 snap={2000, 6000, 12000}, name=f"oreg_f{fv}")
    print("   wrote oreg_f*.png")


# =====================================================================
# Kobayashi phase-field dendrite
# =====================================================================
def kob_flux(p, dx, eps_bar, delta, j, theta0):
    """Pass 1: the anisotropic flux, on STAGGERED faces.

    THE DISCRETISATION MATTERS MORE THAN THE FORMULA HERE. Composing
    two CENTRAL-difference gradients -- the obvious reading of "pass 1
    takes a gradient, pass 2 takes its divergence" -- gives
    (f[i+2] - 2f[i] + f[i-2]) / 4dx^2, a stencil that skips the
    immediate neighbour. The odd and even sublattices then evolve
    independently, nothing damps the Nyquist mode, and the field fills
    with a diagonal checkerboard while staying bounded in [0, 1] and
    finite -- so an isfinite() ladder calls it stable. That is exactly
    what the first version of this prototype did.

    Instead the flux lives on cell FACES: cell (i,j) stores the flux
    through its +x face and its +y face, built from a forward
    difference across that face. Pass 2's backward difference then
    composes to the compact (f[i+1] - 2f[i] + f[i-1]) / dx^2, which
    damps the checkerboard as it should.

    The transverse derivative at a face is the average of the two
    cells' central differences, which is the standard finite-volume
    treatment.

    Returns (Jx on +x faces, Jy on +y faces).
    """
    # Zero-flux (Neumann) in both directions, as the paper specifies:
    # edge-replicate before differencing.
    pe = np.pad(p, 1, mode="edge")
    # Forward differences ACROSS each face.
    dpdx_xf = (pe[1:-1, 2:] - pe[1:-1, 1:-1]) / dx      # +x face
    dpdy_yf = (pe[2:, 1:-1] - pe[1:-1, 1:-1]) / dx      # +y face
    # Central differences at cells, then averaged onto the faces.
    dpdx_c = (pe[1:-1, 2:] - pe[1:-1, :-2]) / (2 * dx)
    dpdy_c = (pe[2:, 1:-1] - pe[:-2, 1:-1]) / (2 * dx)
    dpdy_xf = 0.5 * (dpdy_c + np.pad(dpdy_c, 1, mode="edge")[1:-1, 2:])
    dpdx_yf = 0.5 * (dpdx_c + np.pad(dpdx_c, 1, mode="edge")[2:, 1:-1])

    def eps_at(gx, gy):
        # The guard: in the bulk grad p is exactly zero and theta is
        # undefined. atan2(0,0) is pi/4 under Metal's fast math and
        # NaN elsewhere, and either would poison the flux through
        # NaN*0. No gradient means no flux, so say that directly.
        th = np.arctan2(gy, gx)
        e = eps_bar * (1.0 + delta * np.cos(j * (th - theta0)))
        ep = -eps_bar * delta * j * np.sin(j * (th - theta0))
        dead = (gx * gx + gy * gy) < 1e-16
        return np.where(dead, eps_bar, e), np.where(dead, 0.0, ep)

    ex, epx = eps_at(dpdx_xf, dpdy_xf)
    ey, epy = eps_at(dpdx_yf, dpdy_yf)
    jx = ex * ex * dpdx_xf - ex * epx * dpdy_xf
    jy = ey * ey * dpdy_yf + ey * epy * dpdx_yf
    return jx, jy


def kob_div(jx, jy, dx):
    """Pass 2: backward difference of the face fluxes -- compact."""
    jxp = np.pad(jx, 1, mode="constant")   # zero flux outside
    jyp = np.pad(jy, 1, mode="constant")
    return ((jxp[1:-1, 1:-1] - jxp[1:-1, :-2])
            + (jyp[1:-1, 1:-1] - jyp[:-2, 1:-1])) / dx


def kob_step(p, T, dx, dt, eps_bar, tau, alpha, gamma, K, delta, j, theta0,
             noise_amp, rng, Te=1.0):
    """One step in the TWO-PASS shape the shader will use."""
    jx, jy = kob_flux(p, dx, eps_bar, delta, j, theta0)
    div = kob_div(jx, jy, dx)

    m = (alpha / np.pi) * np.arctan(gamma * (Te - T))
    noise = noise_amp * p * (1.0 - p) * (rng.random(p.shape) - 0.5)
    dpdt = (div + p * (1.0 - p) * (p - 0.5 + m) + noise) / tau
    np_ = p + dt * dpdt
    Te_ = np.pad(T, 1, mode="edge")
    lapT = (Te_[2:, 1:-1] + Te_[:-2, 1:-1] + Te_[1:-1, 2:] + Te_[1:-1, :-2]
            - 4.0 * T) / (dx * dx)
    nT = T + dt * (lapT + K * dpdt)
    return np_, nT


def kob_run(n, steps, K, delta, j, theta0, dt=0.0002, dx=0.03, seed_r=3,
            snap=None, name=None, noise_amp=0.01, seed=1):
    rng = np.random.default_rng(seed)
    p = np.zeros((n, n))
    T = np.zeros((n, n))
    yy, xx = np.mgrid[0:n, 0:n]
    c = n // 2
    p[((xx - c) ** 2 + (yy - c) ** 2) <= seed_r * seed_r] = 1.0
    for i in range(1, steps + 1):
        p, T = kob_step(p, T, dx, dt, 0.01, 0.0003, 0.9, 10.0, K,
                        delta, j, theta0, noise_amp, rng)
        if not np.all(np.isfinite(p)):
            return None, None, i
        if snap and i in snap and name:
            save(f"{name}_{i}.png", p, 0.0, 1.0)
    return p, T, None


def kob_arms(p):
    """Count the crystal's arms: threshold, then count angular maxima of
    the radius. The whole point of the anisotropy is the arm count, so
    a six-fold run that produces four arms is a wrong theta0 or j."""
    n = p.shape[0]
    c = n // 2
    yy, xx = np.mgrid[0:n, 0:n]
    r = np.sqrt((xx - c) ** 2 + (yy - c) ** 2)
    solid = p > 0.5
    angles = np.arctan2(yy - c, xx - c)
    nb = 360
    idx = ((angles + np.pi) / (2 * np.pi) * nb).astype(int) % nb
    reach = np.zeros(nb)
    rr = np.where(solid, r, 0.0)
    for b in range(nb):
        m = idx == b
        if m.any():
            reach[b] = rr[m].max()
    # Smooth, then count local maxima above the mean.
    k = np.ones(9) / 9.0
    sm = np.convolve(np.concatenate([reach[-8:], reach, reach[:8]]), k, "same")[8:-8]
    thr = sm.mean()
    peaks = 0
    for b in range(nb):
        if sm[b] > thr and sm[b] >= sm[(b - 1) % nb] and sm[b] > sm[(b + 1) % nb]:
            peaks += 1
    return peaks, float(sm.max()), float(sm.mean())


def kobayashi(rows):
    print("\n=== Kobayashi: the paper's own constants ===")
    print("eps_bar=0.01 tau=0.0003 alpha=0.9 gamma=10 dt=0.0002 dx=0.03 noise=0.01")
    n = 300

    # The dt the paper used is very close to the explicit limit for the
    # T equation: dx^2/(4 D_T) = 0.03^2/4 = 2.25e-4 against their
    # 2.0e-4. They used an implicit scheme for T for that reason; this
    # checks that explicit survives at their value.
    print(f"\n   explicit limit for T: dx^2/4 = {0.03 ** 2 / 4:.2e}; paper used 2.0e-4")
    for dt in [1e-4, 2e-4, 2.25e-4, 3e-4, 5e-4]:
        p, T, blew = kob_run(128, 2000, K=1.6, delta=0.04, j=6, theta0=np.pi / 2, dt=dt)
        if blew:
            print(f"   dt={dt:.2e}  diverged at {blew}")
            rows.append(dict(model="kobayashi", sweep="dt", dt=dt, diverged=blew))
            continue
        # Nyquist amplitude: the alternating-sign mean. A checkerboard
        # instability stays bounded in [0,1] and finite, so isfinite
        # cannot see it -- the first version of this ladder called a
        # fully checkerboarded field stable.
        n = p.shape[0]
        yy, xx = np.mgrid[0:n, 0:n]
        nyq = float(np.abs(np.where((xx + yy) % 2 == 0, p, -p).mean()))
        verdict = "stable" if nyq < 1e-3 else f"CHECKERBOARD (nyquist {nyq:.3f})"
        print(f"   dt={dt:.2e}  {verdict}")
        rows.append(dict(model="kobayashi", sweep="dt", dt=dt, nyquist=nyq))

    # Fig. 8: six-fold ice dendrites, delta = 0.040, theta0 = pi/2.
    print(f"\n   fig. 8 (six-fold, delta=0.040, j=6, theta0=pi/2):")
    print(f"   {'K':>5} {'steps':>7} {'arms':>5} {'reach':>7} {'solid%':>7}")
    for K in [0.8, 1.0, 1.2, 1.6, 2.0]:
        t0 = time.time()
        steps = 6000
        p, T, blew = kob_run(n, steps, K=K, delta=0.04, j=6, theta0=np.pi / 2,
                             snap={steps} if K in (1.0, 1.6) else None,
                             name=f"kob_six_K{K}")
        if p is None:
            print(f"   {K:>5} diverged at {blew}")
            continue
        arms, reach, mean_r = kob_arms(p)
        frac = float((p > 0.5).mean())
        print(f"   {K:>5} {steps:>7} {arms:>5} {reach:>7.1f} {frac * 100:>6.2f}%"
              f"   ({time.time() - t0:.0f}s)")
        rows.append(dict(model="kobayashi", sweep="K_sixfold", K=K, steps=steps,
                         arms=arms, reach=reach, solid_fraction=frac))

    # Fig. 7: four-fold, delta = 0.020, K = 2.0.
    print(f"\n   fig. 7 (four-fold, j=4, theta0=0, K=2.0):")
    print(f"   {'delta':>6} {'arms':>5} {'reach':>7} {'solid%':>7}")
    for delta in [0.0, 0.01, 0.02, 0.05]:
        p, T, blew = kob_run(n, 6000, K=2.0, delta=delta, j=4, theta0=0.0,
                             snap={6000} if delta == 0.02 else None,
                             name=f"kob_four_d{delta}")
        if p is None:
            print(f"   {delta:>6} diverged at {blew}")
            continue
        arms, reach, mean_r = kob_arms(p)
        frac = float((p > 0.5).mean())
        print(f"   {delta:>6} {arms:>5} {reach:>7.1f} {frac * 100:>6.2f}%")
        rows.append(dict(model="kobayashi", sweep="delta_fourfold", delta=delta,
                         arms=arms, reach=reach, solid_fraction=frac))


if __name__ == "__main__":
    rows = []
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    if which in ("all", "oreg"):
        oregonator(rows)
    if which in ("all", "kob"):
        kobayashi(rows)
    with open(os.path.join(OUT, "oreg_kob.json"), "w") as fh:
        json.dump(rows, fh, indent=1)
    print(f"\nwrote {OUT}/oreg_kob.json and PNGs")
