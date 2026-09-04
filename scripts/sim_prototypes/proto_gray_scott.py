"""Gray-Scott validation prototype (NumPy, CPU).

Purpose: validate the discrete scheme the plan will ship (Karl Sims'
3x3 Laplacian weights, DA=1, DB=0.5, dt=1) against Pearson's named
(F,k) classes, and measure how many steps the pattern takes to
settle -- the number that sizes the GPU driver's step budget.

Writes PNGs and a settle table into output/sim_proto/.
"""
import time, os, sys
import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)

def laplacian(a):
    # Karl Sims weights: centre -1, edge 0.2, corner 0.05, periodic wrap.
    up = np.roll(a, -1, 0); dn = np.roll(a, 1, 0)
    lf = np.roll(a, -1, 1); rt = np.roll(a, 1, 1)
    ul = np.roll(up, -1, 1); ur = np.roll(up, 1, 1)
    dl = np.roll(dn, -1, 1); dr = np.roll(dn, 1, 1)
    return -a + 0.2 * (up + dn + lf + rt) + 0.05 * (ul + ur + dl + dr)

def run(name, f, k, n=256, steps=10000, snaps=(500, 2000, 5000, 10000), seed=1):
    rng = np.random.default_rng(seed)
    A = np.ones((n, n), np.float32)
    B = np.zeros((n, n), np.float32)
    # Seed: Sims-style -- a handful of B=1 blobs. A 24px blob is large
    # enough to survive the initial A-depletion at every Pearson class
    # tried; 12px blobs died at mitosis (measured).
    for _ in range(6):
        y, x = rng.integers(0, n - 24, 2)
        B[y:y + 24, x:x + 24] = 1.0
    DA, DB, dt = 1.0, 0.5, 1.0
    t0 = time.perf_counter()
    deltas = []
    for s in range(1, steps + 1):
        lA, lB = laplacian(A), laplacian(B)
        ABB = A * B * B
        A2 = A + (DA * lA - ABB + f * (1 - A)) * dt
        B2 = B + (DB * lB + ABB - (k + f) * B) * dt
        # Clamp to the physical range: without it an overshoot below
        # zero feeds B^2 with the wrong sign and the field blows up to
        # NaN within a few thousand steps (measured on the 8-seed +
        # noise variant). The GPU kernel must do the same.
        A2 = np.clip(A2, 0.0, 1.0)
        B2 = np.clip(B2, 0.0, 1.0)
        d = float(np.abs(B2 - B).mean())
        deltas.append(d)
        A, B = A2, B2
        if s in snaps:
            img = (np.clip(1 - B * 2.0, 0, 1) * 255).astype(np.uint8)
            Image.fromarray(img).save(f"{OUT}/gs_{name}_{s}.png")
    dt_step = (time.perf_counter() - t0) / steps
    # settle: first step where mean |dB| stays below 1e-4 for 200 steps
    settle = None
    for i in range(len(deltas) - 200):
        if max(deltas[i:i + 200]) < 1e-4:
            settle = i
            break
    return dt_step, settle, deltas[-1]

cases = {
    "mitosis_lambda": (0.0367, 0.0649),
    "coral_kappa":    (0.0545, 0.062),
    "spots_delta":    (0.030, 0.057),
    "worms_mu":       (0.046, 0.065),
    "spirals_xi":     (0.010, 0.041),
}
print(f"{'case':16} {'F':>6} {'k':>6} {'us/step':>8} {'settle@':>8} {'final dB':>10}")
for name, (f, k) in cases.items():
    dts, settle, last = run(name, f, k)
    print(f"{name:16} {f:6.4f} {k:6.4f} {dts*1e6:8.0f} {str(settle):>8} {last:10.2e}")
