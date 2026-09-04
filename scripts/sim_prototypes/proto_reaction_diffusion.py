"""FitzHugh-Nagumo, Brusselator and Schnakenberg validation prototypes.

Phase-0 measurement for simulation mode. These three share Gray-Scott's
machinery -- explicit Euler on a 3x3 Sims Laplacian -- so they share a
script; what differs is the reaction term, the stable dt and whether the
published parameter set actually produces the pattern the catalogue
claims.

Every preset in simulation-catalog.md sections 2, 3 and 5 is marked
`[verify by prototype]`. That is what this is for: the catalogue says
"do not ship a preset without running it", and these are the runs.

Answers, per model:
  - does the named parameter set produce the named pattern
  - the largest dt that stays finite (the slider's cap)
  - steps to a still, or that it never stills

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


def lap(a):
    """Karl Sims' 3x3 weights: centre -1, edge 0.2, corner 0.05, periodic.

    The same kernel Gray-Scott uses and the same one the shader will,
    so a dt measured here is a dt that transfers."""
    up = np.roll(a, -1, 0)
    dn = np.roll(a, 1, 0)
    lf = np.roll(a, -1, 1)
    rt = np.roll(a, 1, 1)
    return (-a + 0.2 * (up + dn + lf + rt)
            + 0.05 * (np.roll(up, -1, 1) + np.roll(up, 1, 1)
                      + np.roll(dn, -1, 1) + np.roll(dn, 1, 1)))


def rng(seed):
    return np.random.default_rng(seed)


def save(name, field, lo=None, hi=None):
    f = np.asarray(field, dtype=np.float64)
    lo = f.min() if lo is None else lo
    hi = f.max() if hi is None else hi
    span = hi - lo
    if span <= 0:
        span = 1.0
    img = np.clip((f - lo) / span, 0, 1)
    Image.fromarray((img * 255).astype(np.uint8)).save(os.path.join(OUT, name))


class Settle:
    """Steps to a still, by the metric the driver will use: mean |delta|
    below `tol` for `window` consecutive STEPS.

    Reports the step at which the quiet run BEGAN. A first version
    counted the window in feeds (one per 20 steps) and reported
    `step - window`, which demanded 4,000 quiet steps and then dated
    the still ~3,800 steps late; every settle figure in the first run
    carried that error.

    Gray-Scott taught that this fires long before a GROWTH pattern is
    finished, and the wavelength sweep taught that it fires during slow
    nucleation before a pattern exists at all, so callers guard it with
    an amplitude floor and the images decide a preset."""

    def __init__(self, tol=1e-4, window=200):
        self.tol, self.window = tol, window
        self.onset, self.at = None, None

    def feed(self, delta, step):
        if delta < self.tol:
            if self.onset is None:
                self.onset = step
            if self.at is None and step - self.onset >= self.window:
                self.at = self.onset
        else:
            self.onset = None
        return self.at


def run_fhn(name, a, b, tau, drive, dv, dw, dt, steps, seed_kind, seed=1, snaps=()):
    """dv/dt = Dv lap(v) + v - v^3/3 - w + I ;  tau dw/dt = Dw lap(w) + v + a - b w"""
    g = rng(seed)
    v = np.full((N, N), -1.2)
    w = np.full((N, N), -0.6)
    if seed_kind == "noise":
        v += g.normal(0, 0.05, (N, N))
    elif seed_kind == "broken_wave":
        # A wavefront cut in half is the standard way to nucleate a
        # spiral in an excitable medium.
        v[N // 2 - 4:N // 2 + 4, :N // 2] = 2.0
        w[N // 2 - 12:N // 2 - 4, :N // 2] = 1.0
    st, out = Settle(), {}
    t0 = time.time()
    for s in range(1, steps + 1):
        prev = v
        v = v + dt * (dv * lap(v) + v - v ** 3 / 3.0 - w + drive)
        w = w + dt / tau * (dw * lap(w) + prev + a - b * w)
        np.clip(v, -3.0, 3.0, out=v)
        if not np.isfinite(v).all():
            return {"model": "fitzhugh_nagumo", "preset": name, "dt": dt,
                    "diverged_at": s}
        if s % 20 == 0:
            st.feed(float(np.abs(v - prev).mean()), s)
        if s in snaps:
            save(f"fhn_{name}_{s}.png", v, -2.2, 2.2)
    out = {"model": "fitzhugh_nagumo", "preset": name, "dt": dt,
           "steps_run": steps, "settled_at": st.at,
           "v_range": [float(v.min()), float(v.max())],
           "spatial_sd": float(v.std()),
           "seconds": round(time.time() - t0, 1)}
    save(f"fhn_{name}.png", v, -2.2, 2.2)
    return out


def run_brusselator(name, A, B, dx, dy, dt, steps, seed=2, snaps=()):
    """dX/dt = Dx lap(X) + A - (B+1)X + X^2 Y ;  dY/dt = Dy lap(Y) + BX - X^2 Y"""
    g = rng(seed)
    X = A + g.normal(0, 0.05, (N, N))
    Y = B / A + g.normal(0, 0.05, (N, N))
    st = Settle()
    t0 = time.time()
    for s in range(1, steps + 1):
        prev = X
        xxy = X * X * Y
        Xn = X + dt * (dx * lap(X) + A - (B + 1.0) * X + xxy)
        Yn = Y + dt * (dy * lap(Y) + B * X - xxy)
        X, Y = np.clip(Xn, 0.0, None), np.clip(Yn, 0.0, None)
        if not np.isfinite(X).all() or X.max() > 1e6:
            return {"model": "brusselator", "preset": name, "dt": dt,
                    "diverged_at": s}
        # Amplitude guard: the run starts at the fixed point plus 0.05
        # noise, quieter than the tolerance, so an unguarded 200-step
        # window fires before the pattern exists.
        if s % 20 == 0 and float(X.std()) > 0.2:
            st.feed(float(np.abs(X - prev).mean()), s)
        if s in snaps:
            save(f"brusselator_{name}_{s}.png", X)
    save(f"brusselator_{name}.png", X)
    return {"model": "brusselator", "preset": name, "dt": dt,
            "steps_run": steps, "settled_at": st.at,
            "x_range": [float(X.min()), float(X.max())],
            "spatial_sd": float(X.std()),
            "seconds": round(time.time() - t0, 1)}


def run_schnakenberg(name, a, b, du, dv, dt, steps, seed=3, snaps=()):
    """du/dt = Du lap(u) + a - u + u^2 v ;  dv/dt = Dv lap(v) + b - u^2 v"""
    g = rng(seed)
    u0, v0 = a + b, b / (a + b) ** 2
    u = u0 + g.normal(0, 0.02, (N, N))
    v = v0 + g.normal(0, 0.02, (N, N))
    st = Settle()
    t0 = time.time()
    for s in range(1, steps + 1):
        prev = u
        uuv = u * u * v
        un = u + dt * (du * lap(u) + a - u + uuv)
        vn = v + dt * (dv * lap(v) + b - uuv)
        u, v = np.clip(un, 0.0, None), np.clip(vn, 0.0, None)
        if not np.isfinite(u).all() or u.max() > 1e6:
            return {"model": "schnakenberg", "preset": name, "dt": dt,
                    "diverged_at": s}
        # Same guard as the Brusselator: measured without it, this
        # reported "settled at step 20" on a run that went on to form a
        # full pattern.
        if s % 20 == 0 and float(u.std()) > 0.2:
            st.feed(float(np.abs(u - prev).mean()), s)
        if s in snaps:
            save(f"schnakenberg_{name}_{s}.png", u)
    save(f"schnakenberg_{name}.png", u)
    return {"model": "schnakenberg", "preset": name, "dt": dt,
            "steps_run": steps, "settled_at": st.at,
            "u_range": [float(u.min()), float(u.max())],
            "spatial_sd": float(u.std()),
            "seconds": round(time.time() - t0, 1)}


def dominant_wavelength(f):
    """Peak of the radially-averaged power spectrum, in cells.

    A Turing pattern has a characteristic wavelength set by the
    diffusion lengths, and at 256 cells the catalogue's presets put it
    at a few cells -- real, but visually just speckle. This puts a
    number on it so the shipped preset can be chosen rather than
    eyeballed."""
    a = np.asarray(f, dtype=np.float64)
    a = a - a.mean()
    p = np.abs(np.fft.fftshift(np.fft.fft2(a))) ** 2
    n = a.shape[0]
    y, x = np.indices(a.shape)
    r = np.hypot(y - n // 2, x - n // 2).astype(int)
    prof = np.bincount(r.ravel(), p.ravel()) / np.maximum(np.bincount(r.ravel()), 1)
    k = int(np.argmax(prof[1:n // 2]) + 1)   # skip DC
    return n / k


def wavelength_sweep():
    """Wavelength against diffusion scale, and what it costs.

    Turing wavelength goes as sqrt(D); explicit-Euler stability caps
    dt at ~const/D. So scaling D by k multiplies the wavelength by
    sqrt(k) and the step count by k -- feature size costs steps
    QUADRATICALLY. This measures both halves of that instead of
    asserting it."""
    rows = []
    for k in (1, 4, 16):
        du, dv, dt = 1.0 * k, 40.0 * k, 0.01 / k
        steps = int(20000 * k)
        g = rng(3)
        u0, v0 = 1.0, 0.9 / 1.0
        u = u0 + g.normal(0, 0.02, (N, N))
        v = v0 + g.normal(0, 0.02, (N, N))
        # Same criterion in MODEL time at every k: a per-step change of
        # 1e-4 at dt = 0.01 is a rate of 1e-2, so the tolerance scales
        # with dt and the window with 1/dt.
        st = Settle(tol=1e-4 / k, window=200 * k)
        t0 = time.time()
        for s2 in range(1, steps + 1):
            prev = u
            uuv = u * u * v
            un = u + dt * (du * lap(u) + 0.1 - u + uuv)
            vn = v + dt * (dv * lap(v) + 0.9 - uuv)
            u, v = np.clip(un, 0.0, None), np.clip(vn, 0.0, None)
            if not np.isfinite(u).all() or u.max() > 1e6:
                rows.append({"model": "schnakenberg", "preset": f"lambda_k{k}",
                             "dt": dt, "diverged_at": s2})
                break
            # The amplitude guard is not optional. Growth from noise is
            # slow before it nucleates, so the settle metric fires
            # DURING the quiet phase and reports a converged run with a
            # blank field -- measured at D scale 16: "settled" at 10,000
            # steps with a spatial sd of 0.017, i.e. nothing there. Only
            # accept stillness once a pattern actually exists.
            if s2 % 50 == 0 and float(u.std()) > 0.2 and st.feed(
                    float(np.abs(u - prev).mean()), s2):
                break
        else:
            s2 = steps
        if rows and rows[-1].get("diverged_at"):
            continue
        save(f"schnakenberg_lambda_k{k}.png", u)
        rows.append({"model": "schnakenberg", "preset": f"lambda_k{k}",
                     "D_scale": k, "dt": dt, "settled_at": st.at,
                     "steps_run": s2,
                     "wavelength_cells": round(dominant_wavelength(u), 2),
                     "spatial_sd": float(u.std()),
                     "seconds": round(time.time() - t0, 1)})
    return rows


def dt_probe():
    """Largest stable dt, measured on the configuration that matters.

    FHN from a noise seed relaxes to rest, so a probe there measures
    the stability of a field doing nothing -- the first version did
    exactly that and reported a cap of 0.5. This runs the spiral. The
    clamp hides divergence, so the signal is the fraction of cells
    pinned to the rails (|v| >= 2.99): a stable spiral has none.

    Brusselator and Schnakenberg diverge honestly, so the ladder just
    needs the missing rungs -- the first run tested 0.01 and 0.05 and
    wrote down 0.02 as the cap without ever running 0.02."""
    rows = []
    for dt in (0.1, 0.25, 0.4, 0.5, 0.75, 1.0):
        steps = int(200 / dt)
        g = rng(1)
        v = np.full((N, N), -1.2)
        w = np.full((N, N), -0.6)
        v[N // 2 - 4:N // 2 + 4, :N // 2] = 2.0
        w[N // 2 - 12:N // 2 - 4, :N // 2] = 1.0
        for _ in range(steps):
            prev = v
            v = v + dt * (lap(v) + v - v ** 3 / 3.0 - w + 0.5)
            w = w + dt / 12.5 * (prev + 0.7 - 0.8 * w)
            np.clip(v, -3.0, 3.0, out=v)
        railed = float((np.abs(v) >= 2.99).mean())
        rows.append({"model": "fitzhugh_nagumo", "dt": dt, "steps": steps,
                     "railed_fraction": railed, "spatial_sd": float(v.std())})
    for dt in (0.01, 0.02, 0.03, 0.04, 0.05):
        r = run_brusselator(f"dtp{dt}", 1.0, 3.0, 1.0, 8.0, dt, int(40 / dt))
        rows.append({"model": "brusselator", "dt": dt,
                     "diverged_at": r.get("diverged_at"), "spatial_sd": r.get("spatial_sd")})
    for dt in (0.01, 0.02, 0.03, 0.04, 0.05):
        r = run_schnakenberg(f"dtp{dt}", 0.1, 0.9, 1.0, 40.0, dt, int(40 / dt))
        rows.append({"model": "schnakenberg", "dt": dt,
                     "diverged_at": r.get("diverged_at"), "spatial_sd": r.get("spatial_sd")})
    return rows


def main():
    if "--dt" in sys.argv:
        rows = dt_probe()
        with open(os.path.join(OUT, "rd_dt_probe.json"), "w") as f:
            json.dump(rows, f, indent=1)
        print(f"{'model':<18}{'dt':>6}  result")
        for r in rows:
            if r["model"] == "fitzhugh_nagumo":
                res = f"railed {100 * r['railed_fraction']:.1f}% of cells, sd {r['spatial_sd']:.3f} ({r['steps']} steps = 200 model time)"
            else:
                res = f"DIVERGED at step {r['diverged_at']}" if r["diverged_at"] else f"stable, sd {r['spatial_sd']:.3f}"
            print(f"{r['model']:<18}{r['dt']:>6}  {res}")
        return
    if "--wavelength" in sys.argv:
        rows = wavelength_sweep()
        with open(os.path.join(OUT, "rd_wavelength.json"), "w") as f:
            json.dump(rows, f, indent=1)
        print(f"{'D scale':>8}{'dt':>8}{'steps':>9}{'lambda (cells)':>16}{'sd':>8}{'sec':>7}")
        for r in rows:
            if "diverged_at" in r:
                print(f"{r['preset']:>8} DIVERGED at {r['diverged_at']}")
                continue
            print(f"{r['D_scale']:>8}{r['dt']:>8.4f}{r['steps_run']:>9,}"
                  f"{r['wavelength_cells']:>16}{r['spatial_sd']:>8.3f}{r['seconds']:>7}")
        return
    rows = []

    # --- FitzHugh-Nagumo. Catalogue constants a=0.7 b=0.8 tau=12.5,
    # excitable at I ~= 0.5, Dw = 0 the classic choice. The catalogue
    # says the Turing/labyrinth regime "is known to exist but the
    # constant set for it is [verify]" -- so try Dw > 0 for it.
    rows.append(run_fhn("excitable_spiral", 0.7, 0.8, 12.5, 0.5, 1.0, 0.0,
                        0.1, 4000, "broken_wave", snaps=(200, 1000, 4000)))
    rows.append(run_fhn("turing_dw4", 0.7, 0.8, 12.5, 0.0, 1.0, 4.0,
                        0.1, 4000, "noise", snaps=(500, 4000)))
    rows.append(run_fhn("noise_excitable", 0.7, 0.8, 12.5, 0.5, 1.0, 0.0,
                        0.1, 2000, "noise"))
    # dt cap: where does it stop being finite?
    for dt in (0.1, 0.25, 0.5, 1.0):
        r = run_fhn(f"dt{dt}", 0.7, 0.8, 12.5, 0.5, 1.0, 0.0, dt, 600, "noise")
        r["preset"] = "dt_probe"
        r["dt"] = dt
        rows.append(r)

    # --- Brusselator. Catalogue presets, both [verify by prototype].
    rows.append(run_brusselator("turing_spots", 1.0, 3.0, 1.0, 8.0,
                                0.01, 20000, snaps=(2000, 20000)))
    rows.append(run_brusselator("oscillating", 1.0, 2.5, 1.0, 1.0,
                                0.01, 20000, snaps=(2000, 20000)))
    for dt in (0.01, 0.05, 0.1, 0.25):
        r = run_brusselator(f"dt{dt}", 1.0, 3.0, 1.0, 8.0, dt, 400)
        r["preset"] = "dt_probe"
        rows.append(r)

    # --- Schnakenberg. Catalogue: a=0.1 b=0.9, Dv/Du ~= 40.
    rows.append(run_schnakenberg("turing_spots", 0.1, 0.9, 1.0, 40.0,
                                 0.01, 20000, snaps=(2000, 20000)))
    for dt in (0.01, 0.02, 0.05, 0.1):
        r = run_schnakenberg(f"dt{dt}", 0.1, 0.9, 1.0, 40.0, dt, 400)
        r["preset"] = "dt_probe"
        rows.append(r)

    path = os.path.join(OUT, "rd_step_costs.json")
    with open(path, "w") as f:
        json.dump(rows, f, indent=1)
    print(f"\n{'model':<18}{'preset':<18}{'dt':>6}{'settled':>10}{'sd':>9}  note")
    for r in rows:
        note = "DIVERGED at %d" % r["diverged_at"] if "diverged_at" in r else ""
        sd = r.get("spatial_sd")
        print(f"{r['model']:<18}{r['preset']:<18}{r['dt']:>6}"
              f"{str(r.get('settled_at')):>10}{('%.4f' % sd) if sd is not None else '-':>9}  {note}")
    print(f"\nwrote {path} and PNGs in {OUT}/")


if __name__ == "__main__":
    main()
