"""3D height-field mode, prototype (docs/projects/heightfield-3d.md).

A 2D fractal as a terrain: a height map and an albedo map on a grid, lit
by a small path tracer -- a sun with soft shadows, a sky, and one bounce
of indirect light -- seen through a pinhole camera above the plane. CPU
and numpy, so slow, but enough to see what the mode would look like and
to measure what the GPU version must do (march steps, samples).

Two subjects:
  mccabe      McCabe multi-scale Turing (proto_mccabe_variants' rule),
              height = the field Gaussian-smoothed, albedo = colour memory
  mandelbrot  the smooth escape count over a seahorse-valley window,
              height = a log curve of it, interior a flat lake

    python scripts/heightfield_prototypes/proto_heightfield_pt.py [mccabe|mandelbrot|both]

Writes output/heightfield_proto/*.png.
"""
import os
import sys
import time

import numpy as np
from PIL import Image

OUT = "output/heightfield_proto"
W, H = 640, 400          # image
SPP = int(os.environ.get("HF_SPP", "8"))


# ---------------------------------------------------------------------
# Subjects: (height, albedo) on an N x N grid spanning [0, 1]^2.
# ---------------------------------------------------------------------
def gaussian_blur(f, sigma):
    if sigma <= 0:
        return f
    n = f.shape[0]
    k = np.fft.fftfreq(n)
    g = np.exp(-2 * (np.pi * sigma) ** 2 * (k[:, None] ** 2 + k[None, :] ** 2))
    return np.real(np.fft.ifft2(np.fft.fft2(f) * g))


def subject_mccabe():
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sim_prototypes"))
    os.environ.setdefault("VAR_N", "384")
    os.environ.setdefault("VAR_STEPS", "300")
    import proto_mccabe_variants as pv
    f, mem = pv.run(pv.make_scales(), "argmin", 0)
    albedo = np.clip(pv.colour_memory(f, mem, value_scale=0.0), 0, 1)
    height = gaussian_blur(f, 2.0)
    # Field in [-1, 1]: the terrain's relief is 4% of its width.
    height = (height - height.min()) / (height.max() - height.min()) * 0.04
    return height, albedo


def subject_mandelbrot(n=1024):
    # Seahorse valley, a window 0.08 wide.
    cx, cy, span = -0.7453, 0.1127, 0.012
    xs = np.linspace(cx - span / 2, cx + span / 2, n)
    ys = np.linspace(cy - span / 2, cy + span / 2, n)
    c = xs[None, :] + 1j * ys[:, None]
    z = np.zeros_like(c)
    mu = np.full(c.shape, np.nan)
    alive = np.ones(c.shape, bool)
    for i in range(2000):
        z[alive] = z[alive] ** 2 + c[alive]
        esc = alive & (np.abs(z) > 256)
        mu[esc] = i + 1 - np.log2(np.log(np.abs(z[esc])))
        alive &= ~esc
        if not alive.any():
            break
    interior = np.isnan(mu)
    m = np.where(interior, np.nanmax(mu), mu)
    # The escape count climbs without bound toward the set: a log curve
    # keeps the valleys and the ridges near it in one range.
    h = np.log1p(m - np.nanmin(mu))
    h = gaussian_blur(h, 1.0)
    h = (h - h.min()) / (h.max() - h.min())
    height = 0.12 * h
    # Interior: a lake at the top.
    height[interior] = height.max()
    t = np.fmod(np.log1p(m) * 0.9, 1.0)
    pal = np.array([[0.05, 0.1, 0.35], [0.2, 0.55, 0.75], [0.95, 0.85, 0.5], [0.8, 0.3, 0.1], [0.05, 0.1, 0.35]])
    idx = t * (len(pal) - 1)
    i0 = np.floor(idx).astype(int)
    fr = (idx - i0)[..., None]
    albedo = pal[i0] * (1 - fr) + pal[np.minimum(i0 + 1, len(pal) - 1)] * fr
    albedo[interior] = [0.02, 0.02, 0.03]
    return height, albedo, interior


def subject_mandelbrot_de(n=1024):
    """The same window, height from the distance estimate rather than the
    escape count: H exp(-DE / w) -- the set on top, smooth flanks of
    width w falling away from every filament."""
    cx, cy, span = -0.7453, 0.1127, 0.012
    xs = np.linspace(cx - span / 2, cx + span / 2, n)
    ys = np.linspace(cy - span / 2, cy + span / 2, n)
    c = xs[None, :] + 1j * ys[:, None]
    z = np.zeros_like(c)
    dz = np.zeros_like(c)
    de = np.full(c.shape, np.nan)
    mu = np.full(c.shape, np.nan)
    alive = np.ones(c.shape, bool)
    for i in range(2000):
        dz[alive] = 2 * z[alive] * dz[alive] + 1
        z[alive] = z[alive] ** 2 + c[alive]
        esc = alive & (np.abs(z) > 256)
        az = np.abs(z[esc])
        de[esc] = az * np.log(az) / np.abs(dz[esc])
        mu[esc] = i + 1 - np.log2(np.log(az))
        alive &= ~esc
        if not alive.any():
            break
    interior = np.isnan(de)
    de = np.where(interior, 0.0, de)
    w = span / 150
    h = np.exp(-de / w)
    height = 0.06 * h
    m = np.where(interior, np.nanmax(mu), mu)
    t = np.fmod(np.log1p(m) * 0.9, 1.0)
    pal = np.array([[0.05, 0.1, 0.35], [0.2, 0.55, 0.75], [0.95, 0.85, 0.5], [0.8, 0.3, 0.1], [0.05, 0.1, 0.35]])
    idx = t * (len(pal) - 1)
    i0 = np.floor(idx).astype(int)
    fr = (idx - i0)[..., None]
    albedo = pal[i0] * (1 - fr) + pal[np.minimum(i0 + 1, len(pal) - 1)] * fr
    albedo[interior] = [0.9, 0.9, 0.92]
    return height, albedo


# ---------------------------------------------------------------------
# The terrain: bilinear height over [0, 1]^2, nothing outside.
# ---------------------------------------------------------------------
class Terrain:
    def __init__(self, height, albedo, mirror=None):
        self.h = height
        self.a = albedo
        self.n = height.shape[0]
        self.top = height.max()
        self.mirror = mirror
        gy, gx = np.gradient(height, 1.0 / self.n)
        self.gx, self.gy = gx, gy
        # Lipschitz bound of the height, for safe march steps.
        self.slope = float(np.sqrt(gx ** 2 + gy ** 2).max())

    def _bilinear(self, field, x, y):
        n = self.n
        fx = np.clip(x * n - 0.5, 0, n - 1.001)
        fy = np.clip(y * n - 0.5, 0, n - 1.001)
        x0 = fx.astype(int)
        y0 = fy.astype(int)
        tx = fx - x0
        ty = fy - y0
        if field.ndim == 3:
            tx = tx[..., None]
            ty = ty[..., None]
        return ((1 - ty) * ((1 - tx) * field[y0, x0] + tx * field[y0, x0 + 1])
                + ty * ((1 - tx) * field[y0 + 1, x0] + tx * field[y0 + 1, x0 + 1]))

    def height(self, x, y):
        return self._bilinear(self.h, x, y)

    def normal(self, x, y):
        gx = self._bilinear(self.gx, x, y)
        gy = self._bilinear(self.gy, x, y)
        nrm = np.stack([-gx, -gy, np.ones_like(gx)], -1)
        return nrm / np.linalg.norm(nrm, axis=-1, keepdims=True)

    def albedo(self, x, y):
        return self._bilinear(self.a, x, y)

    def intersect(self, o, d, tmax=10.0):
        """March each ray (o + t d) to the first point under the surface,
        stepping by the height margin over the slope bound, then bisect.
        Returns (hit, t). Rays leaving the unit square or rising above the
        top miss."""
        m = o.shape[0]
        t = np.zeros(m)
        hit = np.zeros(m, bool)
        live = np.ones(m, bool)
        # Start at the slab's top.
        dz = d[:, 2]
        enter = np.where(o[:, 2] > self.top, (self.top - o[:, 2]) / np.minimum(dz, -1e-9), 0.0)
        enter = np.where((o[:, 2] > self.top) & (dz >= 0), np.inf, enter)
        t = np.maximum(t, enter)
        live &= np.isfinite(t)
        steps = 0
        k = 1.0 / (1.0 + self.slope)
        while live.any() and steps < 400:
            steps += 1
            idx = np.nonzero(live)[0]
            p = o[idx] + t[idx, None] * d[idx]
            out = (p[:, 0] < 0) | (p[:, 0] > 1) | (p[:, 1] < 0) | (p[:, 1] > 1) | (p[:, 2] > self.top + 1e-6) & (d[idx, 2] > 0) | (t[idx] > tmax)
            live[idx[out]] = False
            idx = idx[~out]
            p = p[~out]
            gap = p[:, 2] - self.height(p[:, 0], p[:, 1])
            under = gap < 1e-5
            hit[idx[under]] = True
            live[idx[under]] = False
            idx = idx[~under]
            # A step the surface cannot rise into: gap over (|d_z| + slope |d_xy|).
            dd = d[idx]
            rate = np.abs(dd[:, 2]) + self.slope * np.hypot(dd[:, 0], dd[:, 1])
            t[idx] += np.maximum(gap[~under] / np.maximum(rate, 1e-6) * 0.9, 2e-4)
        return hit, t, steps


# ---------------------------------------------------------------------
# Light: a sun and a sky.
# ---------------------------------------------------------------------
SUN_DIR = np.array([-0.55, -0.45, 0.70])
SUN_DIR = SUN_DIR / np.linalg.norm(SUN_DIR)
SUN_RADIANCE = np.array([3.2, 3.0, 2.7])
SUN_ANGLE = 0.03  # radians: soft shadows


def sky(d):
    up = np.clip(d[..., 2], 0, 1)[..., None]
    return (np.array([0.55, 0.65, 0.8]) * (1 - up) + np.array([0.25, 0.4, 0.75]) * up) * 0.9


def cosine_hemisphere(nrm, rng):
    m = nrm.shape[0]
    u1, u2 = rng.random(m), rng.random(m)
    r = np.sqrt(u1)
    phi = 2 * np.pi * u2
    local = np.stack([r * np.cos(phi), r * np.sin(phi), np.sqrt(1 - u1)], -1)
    # A basis around the normal.
    a = np.where(np.abs(nrm[:, :1]) > 0.9, np.array([[0, 1, 0]]), np.array([[1, 0, 0]]))
    t = np.cross(nrm, a)
    t /= np.linalg.norm(t, axis=-1, keepdims=True)
    b = np.cross(nrm, t)
    return local[:, :1] * t + local[:, 1:2] * b + local[:, 2:3] * nrm


def jittered_sun(m, rng):
    d = SUN_DIR[None, :] + rng.normal(scale=SUN_ANGLE, size=(m, 3))
    return d / np.linalg.norm(d, axis=-1, keepdims=True)


def shade(terrain, p, d, rng, depth):
    """Radiance arriving back along -d from surface point p: sun with a
    shadow ray, plus one bounce of sky and terrain light at depth 0."""
    m = p.shape[0]
    nrm = terrain.normal(p[:, 0], p[:, 1])
    alb = terrain.albedo(p[:, 0], p[:, 1])
    origin = p + nrm * 1e-4
    out = np.zeros((m, 3))
    # The interior lake: a mirror.
    if terrain.mirror is not None:
        lake = terrain._bilinear(terrain.mirror.astype(float), p[:, 0], p[:, 1]) > 0.5
    else:
        lake = np.zeros(m, bool)
    # Sun.
    ls = jittered_sun(m, rng)
    cos = np.clip(np.sum(nrm * ls, -1), 0, None)
    lit = cos > 0
    if lit.any():
        blocked, _, _ = terrain.intersect(origin[lit], ls[lit])
        vis = np.zeros(m)
        vis[np.nonzero(lit)[0][~blocked]] = 1.0
        out += (alb * SUN_RADIANCE[None, :]) * (cos * vis)[:, None] / np.pi * np.pi * 0.35
    # Sky and one bounce, cosine-weighted (the pdf cancels the cosine).
    nd = cosine_hemisphere(nrm, rng)
    hit, t, _ = terrain.intersect(origin, nd)
    inc = np.zeros((m, 3))
    inc[~hit] = sky(nd[~hit])
    if depth == 0 and hit.any():
        q = origin[hit] + t[hit, None] * nd[hit]
        inc[hit] = shade(terrain, q, nd[hit], rng, depth + 1)
    out += alb * inc
    # The lake reflects the sky.
    if lake.any():
        r = d[lake] - 2 * np.sum(d[lake] * nrm[lake], -1, keepdims=True) * nrm[lake]
        h2, _, _ = terrain.intersect(origin[lake], r)
        refl = np.where(h2[:, None], 0.05, sky(r))
        out[lake] = 0.1 * out[lake] + 0.9 * refl
    return out


def camera_rays(eye, target, fov_deg, rng):
    fwd = target - eye
    fwd /= np.linalg.norm(fwd)
    right = np.cross(fwd, np.array([0.0, 0.0, 1.0]))
    right /= np.linalg.norm(right)
    up = np.cross(right, fwd)
    sx = np.tan(np.radians(fov_deg) / 2)
    sy = sx * H / W
    jx, jy = rng.random((H, W)), rng.random((H, W))
    px = ((np.arange(W)[None, :] + jx) / W * 2 - 1) * sx
    py = (1 - (np.arange(H)[:, None] + jy) / H * 2) * sy
    d = fwd[None, None, :] + px[..., None] * right + py[..., None] * up
    d = d / np.linalg.norm(d, axis=-1, keepdims=True)
    return np.broadcast_to(eye, d.shape).reshape(-1, 3).copy(), d.reshape(-1, 3)


def render(name, terrain, eye, target, fov=45.0):
    rng = np.random.default_rng(3)
    acc = np.zeros((W * H, 3))
    t0 = time.time()
    steps_seen = []
    for s in range(SPP):
        o, d = camera_rays(eye, target, fov, rng)
        hit, t, steps = terrain.intersect(o, d)
        steps_seen.append(steps)
        col = np.zeros((W * H, 3))
        col[~hit] = sky(d[~hit])
        if hit.any():
            p = o[hit] + t[hit, None] * d[hit]
            col[hit] = shade(terrain, p, d[hit], rng, 0)
        acc += col
        print(f"  {name}: sample {s + 1}/{SPP}, {time.time() - t0:.0f}s, primary march {steps} steps", flush=True)
    img = acc / SPP
    # Filmic-ish: exposure then a simple Reinhard and the sRGB curve.
    img = img * 1.2
    img = img / (1 + img)
    img = np.clip(img, 0, 1) ** (1 / 2.2)
    Image.fromarray((img.reshape(H, W, 3) * 255).astype(np.uint8)).save(f"{OUT}/{name}.png")
    print(f"wrote {OUT}/{name}.png ({SPP} spp, {time.time() - t0:.0f}s, primary march at most {max(steps_seen)} steps)")


def main():
    os.makedirs(OUT, exist_ok=True)
    which = sys.argv[1] if len(sys.argv) > 1 else "both"
    if which in ("mccabe", "both"):
        h, a = subject_mccabe()
        Image.fromarray((a * 255).astype(np.uint8)).save(f"{OUT}/mccabe_flat.png")
        terrain = Terrain(h, a)
        render("mccabe_terrain", terrain, eye=np.array([0.5, -0.25, 0.42]), target=np.array([0.5, 0.45, 0.0]))
    if which in ("mandelbrot_de", "all"):
        h, a = subject_mandelbrot_de()
        terrain = Terrain(h, a)
        render("mandelbrot_de_terrain", terrain, eye=np.array([0.5, -0.3, 0.45]), target=np.array([0.5, 0.5, 0.0]))
    if which in ("mandelbrot", "both"):
        h, a, interior = subject_mandelbrot()
        Image.fromarray((a * 255).astype(np.uint8)).save(f"{OUT}/mandelbrot_flat.png")
        terrain = Terrain(h, a, mirror=interior)
        render("mandelbrot_terrain", terrain, eye=np.array([0.5, -0.3, 0.55]), target=np.array([0.5, 0.5, 0.0]))


if __name__ == "__main__":
    main()
