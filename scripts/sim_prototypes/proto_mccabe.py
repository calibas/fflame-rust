"""McCabe multi-scale Turing validation prototype (NumPy, CPU).

Implements the algorithm exactly as McCabe's paper states it (single
substance; per scale, activator = disc average at the small radius,
inhibitor = disc average at the large radius; the scale with the
LEAST |activator - inhibitor| fires; step the field by that scale's
small amount in the sign of (activator - inhibitor); renormalize to
[-1, 1]; periodic boundaries). Disc averages are done with FFT
convolution here purely for CPU convenience -- the GPU plan uses a
mip pyramid / summed-area table, and the point of this prototype is
to validate the RULE and count steps, not the blur method.

Writes PNGs and timings to output/sim_proto/.
"""
import time, os
import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)
n = 256
rng = np.random.default_rng(7)

def disc_kernel_fft(r):
    y, x = np.mgrid[-n // 2:n // 2, -n // 2:n // 2]
    k = ((x * x + y * y) <= r * r).astype(np.float32)
    k /= k.sum()
    return np.fft.rfft2(np.fft.ifftshift(k))

# (activator radius, inhibitor radius, amount) -- a Softology-style
# ladder; the paper gives no table, so these are the prototype's own.
scales = [(1, 2, 0.05), (2, 4, 0.04), (4, 8, 0.03), (8, 16, 0.02), (16, 32, 0.01)]
Ka = [disc_kernel_fft(a) for a, _, _ in scales]
Ki = [disc_kernel_fft(i) for _, i, _ in scales]

field = rng.uniform(-1, 1, (n, n)).astype(np.float32)

def step(field, symmetry=0):
    F = np.fft.rfft2(field)
    best_var = None
    best_dir = None
    for (a, i, amt), ka, ki in zip(scales, Ka, Ki):
        act = np.fft.irfft2(F * ka, s=field.shape)
        inh = np.fft.irfft2(F * ki, s=field.shape)
        if symmetry > 1:
            # McCabe: average activator/inhibitor with their rotated
            # counterparts. Rotation about the centre by k*2pi/n via
            # nearest-neighbour resampling (validation only).
            act_s, inh_s = act.copy(), inh.copy()
            cy, cx = n / 2, n / 2
            yy, xx = np.mgrid[0:n, 0:n]
            for kk in range(1, symmetry):
                th = 2 * np.pi * kk / symmetry
                xr = (np.cos(th) * (xx - cx) - np.sin(th) * (yy - cy) + cx) % n
                yr = (np.sin(th) * (xx - cx) + np.cos(th) * (yy - cy) + cy) % n
                act_s += act[yr.astype(int), xr.astype(int)]
                inh_s += inh[yr.astype(int), xr.astype(int)]
            act, inh = act_s / symmetry, inh_s / symmetry
        var = np.abs(act - inh)
        d = np.where(act > inh, amt, -amt).astype(np.float32)
        if best_var is None:
            best_var, best_dir = var, d
        else:
            m = var < best_var
            best_var = np.where(m, var, best_var)
            best_dir = np.where(m, d, best_dir)
    field = field + best_dir
    lo, hi = field.min(), field.max()
    field = (field - lo) / max(hi - lo, 1e-9) * 2 - 1
    return field.astype(np.float32)

def save(field, name):
    img = ((field + 1) * 0.5 * 255).astype(np.uint8)
    Image.fromarray(img).save(f"{OUT}/{name}.png")

t0 = time.perf_counter()
f = field.copy()
for s in range(1, 301):
    f = step(f)
    if s in (20, 100, 300):
        save(f, f"mccabe_plain_{s}")
print(f"plain: {(time.perf_counter()-t0)/300*1e3:.1f} ms/step (numpy fft, {n}x{n}, 5 scales)")

# cyclic symmetry variant, 5-fold
t0 = time.perf_counter()
f = field.copy()
for s in range(1, 201):
    f = step(f, symmetry=5)
    if s in (100, 200):
        save(f, f"mccabe_sym5_{s}")
print(f"sym5:  {(time.perf_counter()-t0)/200*1e3:.1f} ms/step")
print("wrote", OUT)
