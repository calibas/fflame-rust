//! The surface texture on the 3D terrain (docs/projects/surface-textures.md):
//! a bump from Grain, Paper, Rock or the simulation texture, mapped
//! triplanar, anchored to the plane, with octaves that keep its size on
//! screen as the view zooms, filtered by the pixel's footprint, and masked
//! by the slope. The 2D relief shares its settings (`EscapeShading`'s
//! texture) and its noise.

use crate::config::escape::ShadingTexture;

/// The texture lattice's period, in cells: every per-octave function
/// repeats in it exactly, so a phase taken mod `SURFACE_PERIOD` loses
/// nothing.
pub const SURFACE_PERIOD: f64 = 1024.0;

/// The most octaves a kind spans at once.
pub const SURFACE_OCTAVES: usize = 6;

/// The bump's calibration: at strength 1 a gradient-noise feature tilts
/// the surface by about a quarter of its slope -- texture you see, not
/// a new landform.
pub const SURFACE_BUMP: f32 = 0.25;

/// A kind's octave window (docs/projects/surface-textures.md): octave `k`
/// of the plane's dyadic ladder weighs a window over `j = k - c`, `c` the
/// log2 of the target wavelength, from `-J` (finest) to 1 (coarsest), with
/// a fractal falloff `2^(H j)`.
pub fn kind_window(kind: ShadingTexture) -> (f32, f32) {
    match kind {
        ShadingTexture::Paper => (3.0, 0.7),
        ShadingTexture::Rock => (4.0, 1.0),
        _ => (1.0, 0.0),
    }
}

/// The surface texture as the terrain's shaders read it (`fdata[22..28]`
/// of the terrain's uniform). `kind` 0 is none, and costs nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceTexture {
    /// `ShadingTexture::to_gpu`.
    pub kind: u32,
    /// The bump's amplitude in the world's units: the strength times the
    /// target wavelength times `SURFACE_BUMP`, so the tilt is the same at
    /// any scale.
    pub amplitude: f32,
    /// `TextureWhere::to_gpu`, and the slope it turns at ± its softness,
    /// in radians.
    pub place: u32,
    pub angle: f32,
    pub soft: f32,
    /// `c - k_min`: where the window's centre lies on the octaves sent.
    pub c_rel: f32,
    /// Cells per world unit of the finest octave sent (`k_min`); each
    /// next octave has half.
    pub inv_lambda0: f32,
    /// How many octaves are sent, at most `SURFACE_OCTAVES`.
    pub octaves: u32,
    /// The kind's window (`kind_window`).
    pub window: (f32, f32),
    /// Each octave's phase: the plane position of the world's origin (the
    /// eye's ground point), in that octave's cells, mod `SURFACE_PERIOD`.
    pub phases: [[f32; 2]; SURFACE_OCTAVES],
}

impl SurfaceTexture {
    pub fn off() -> Self {
        Self {
            kind: 0,
            amplitude: 0.0,
            place: 0,
            angle: 0.0,
            soft: 0.0,
            c_rel: 0.0,
            inv_lambda0: 0.0,
            octaves: 0,
            window: (1.0, 0.0),
            phases: [[0.0; 2]; SURFACE_OCTAVES],
        }
    }

    /// `fdata[22..28]` of the terrain's uniform.
    pub fn to_fdata(&self) -> [[f32; 4]; 6] {
        let ph = |i: usize| self.phases[i];
        [
            [self.kind as f32, self.amplitude, self.angle, self.soft],
            [self.c_rel, self.inv_lambda0, self.octaves as f32, self.place as f32],
            [self.window.0, self.window.1, 0.0, 0.0],
            [ph(0)[0], ph(0)[1], ph(1)[0], ph(1)[1]],
            [ph(2)[0], ph(2)[1], ph(3)[0], ph(3)[1]],
            [ph(4)[0], ph(4)[1], ph(5)[0], ph(5)[1]],
        ]
    }
}

/// The octaves a window over `c` (log2 of the target wavelength) spans:
/// the finest, `k_min`, and how many, every `k` with `k - c` in `(-J, 1)`.
pub fn octave_span(c: f64, j: f64) -> (i64, usize) {
    let k_min = (c - j).floor() as i64 + 1;
    let k_max = (c + 1.0).ceil() as i64 - 1;
    let n = (k_max - k_min + 1).clamp(1, SURFACE_OCTAVES as i64) as usize;
    (k_min, n)
}

/// A kind's octave weight at `j = k - c` (`kind_window`): a window that
/// fades in from `-J` and out toward 1, times the fractal falloff. The
/// shader's `surf_weight`, for the tests.
pub fn octave_weight(j: f64, window: (f64, f64)) -> f64 {
    let ss = |a: f64, b: f64, x: f64| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    ss(-window.0, -window.0 + 1.0, j) * (1.0 - ss(0.0, 1.0, j)) * (window.1 * j).exp2()
}

/// The gradient noise both the 2D relief's Rock and the terrain's kinds are
/// made of: quintic-interpolated, with its analytic derivative, periodic in
/// 1024 cells. Returns (value, d/dx, d/dy); the value is about +-0.7.
pub const SURFACE_NOISE_WGSL: &str = r#"
fn surf_grad(c: vec2<i32>) -> vec2<f32> {
    let q = vec2<u32>(vec2<i32>(c.x & 1023, c.y & 1023));
    var h = (q.x * 1597334677u) ^ (q.y * 3812015801u);
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    let a = f32(h >> 8u) * (6.283185307 / 16777216.0);
    return vec2<f32>(cos(a), sin(a));
}

fn surf_noise(x: vec2<f32>) -> vec3<f32> {
    let fl = floor(x);
    let i = vec2<i32>(fl);
    let f = x - fl;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    let ga = surf_grad(i);
    let gb = surf_grad(i + vec2<i32>(1, 0));
    let gc = surf_grad(i + vec2<i32>(0, 1));
    let gd = surf_grad(i + vec2<i32>(1, 1));
    let va = dot(ga, f);
    let vb = dot(gb, f - vec2<f32>(1.0, 0.0));
    let vc = dot(gc, f - vec2<f32>(0.0, 1.0));
    let vd = dot(gd, f - vec2<f32>(1.0, 1.0));
    let k = va - vb - vc + vd;
    let v = va + u.x * (vb - va) + u.y * (vc - va) + u.x * u.y * k;
    let d = ga + u.x * (gb - ga) + u.y * (gc - ga) + u.x * u.y * (ga - gb - gc + gd)
        + du * (u.yx * k + vec2<f32>(vb, vc) - va);
    return vec3<f32>(v, d);
}

// Ridged: stone's creases, where the noise crosses zero, as ridges.
fn surf_ridged(x: vec2<f32>) -> vec3<f32> {
    let n = surf_noise(x);
    let r = 1.0 - 1.6 * abs(n.x);
    let s = select(-1.0, 1.0, n.x >= 0.0);
    return vec3<f32>(r * r - 0.45, 2.0 * r * (-1.6 * s) * n.yz);
}

// Paper's fibres: the noise stretched along four directions. Every
// stretch is an integer matrix, so a fibre layer stays periodic in the
// lattice's cells and a phase taken mod its period loses nothing.
fn surf_fibre(x: vec2<f32>, m: mat2x2<f32>, o: vec2<f32>) -> vec3<f32> {
    let n = surf_noise(m * x + o);
    return vec3<f32>(n.x, transpose(m) * n.yz);
}

fn surf_paper(x: vec2<f32>) -> vec3<f32> {
    return 0.35 * surf_fibre(x, mat2x2<f32>(1.0, 0.0, 0.0, 4.0), vec2<f32>(0.0, 0.0))
        + 0.3 * surf_fibre(x, mat2x2<f32>(4.0, 0.0, 0.0, 1.0), vec2<f32>(17.0, 3.0))
        + 0.2 * surf_fibre(x, mat2x2<f32>(1.0, -3.0, 1.0, 3.0), vec2<f32>(5.0, 29.0))
        + 0.15 * surf_fibre(x, mat2x2<f32>(1.0, 3.0, -1.0, 3.0), vec2<f32>(41.0, 11.0));
}
"#;

/// The terrain's side: the kinds over the octave ladder, triplanar, and the
/// bump. Reads `params.fdata[22..28]` (`SurfaceTexture::to_fdata`) and, for
/// the simulation kind, `surf_image` and `surf_sampler`, which the shader
/// that includes this declares.
pub const SURFACE_WGSL: &str = r#"
fn surf_kind() -> u32 {
    return u32(params.fdata[22].x + 0.5);
}

// An octave's weight: a window over j = k - c from -J to 1, times the
// fractal falloff 2^(H j) (`octave_weight`).
fn surf_weight(j: f32) -> f32 {
    let w = params.fdata[24];
    return smoothstep(-w.x, -w.x + 1.0, j) * (1.0 - smoothstep(0.0, 1.0, j)) * exp2(w.y * j);
}

fn surf_phase(i: u32) -> vec2<f32> {
    let v = params.fdata[25u + i / 2u];
    return select(v.xy, v.zw, (i & 1u) == 1u);
}

// One octave of the kind at x (in its cells), its derivative in cells.
// The simulation texture: its luminance, one tile a cell, at the mip the
// footprint asks (`texel` world units a texel at this octave's size).
fn surf_octave(x: vec2<f32>, lod: f32) -> vec3<f32> {
    let kind = surf_kind();
    if (kind == 2u) {
        return surf_paper(x);
    }
    if (kind == 4u) {
        return surf_ridged(x);
    }
    if (kind == 3u) {
        let dims = vec2<f32>(textureDimensions(surf_image));
        let e = exp2(max(lod, 0.0)) / dims;
        let w = vec3<f32>(0.2126, 0.7152, 0.0722);
        let c = dot(textureSampleLevel(surf_image, surf_sampler, x, lod).rgb, w);
        let cx = dot(textureSampleLevel(surf_image, surf_sampler, x + vec2<f32>(e.x, 0.0), lod).rgb, w);
        let cy = dot(textureSampleLevel(surf_image, surf_sampler, x + vec2<f32>(0.0, e.y), lod).rgb, w);
        return vec3<f32>(c - 0.5, (cx - c) / e.x, (cy - c) / e.y);
    }
    return surf_noise(x);
}

// The texture over one projection: (a, b) the world coordinates in its
// plane, `pa`/`pb` whether each takes the plane's phase (x 0, y 1; 2 none:
// a height, absolute from the plain). Returns the height and its gradient
// in the world's units; octaves finer than the footprint fade out.
fn surf_layer(a: f32, b: f32, pa: u32, pb: u32, foot: f32) -> vec3<f32> {
    let s = params.fdata[23];
    let n = u32(s.z + 0.5);
    var sum = vec3<f32>(0.0);
    var norm = 0.0;
    var inv = s.y;
    for (var i = 0u; i < n; i = i + 1u) {
        let wgt = surf_weight(f32(i) - s.x);
        norm = norm + wgt;
        // Fades in from two footprints a wavelength to four.
        let lambda = 1.0 / inv;
        let fade = clamp(lambda / max(foot, 1.0e-30) * 0.5 - 1.0, 0.0, 1.0);
        if (wgt * fade > 0.0) {
            let ph = surf_phase(i);
            let pha = select(select(0.0, ph.y, pa == 1u), ph.x, pa == 0u);
            let phb = select(select(0.0, ph.y, pb == 1u), ph.x, pb == 0u);
            let x = vec2<f32>(pha + a * inv, phb + b * inv);
            let dims = max(f32(textureDimensions(surf_image).x), 1.0);
            let lod = log2(max(foot * inv * dims, 1.0e-30));
            let o = surf_octave(x, lod);
            sum = sum + wgt * fade * vec3<f32>(o.x, o.yz * inv);
        }
        inv = inv * 0.5;
    }
    return sum / max(norm, 1.0e-6);
}

// The slope mask: everywhere, the steep faces, or the flats, by the angle
// of the true normal from vertical, turning at the angle +- its softness.
fn surf_mask(n: vec3<f32>) -> f32 {
    let place = u32(params.fdata[23].w + 0.5);
    if (place == 0u) {
        return 1.0;
    }
    let slope = acos(clamp(abs(n.z), 0.0, 1.0));
    let a = params.fdata[22].z;
    let w = max(params.fdata[22].w, 1.0e-4);
    let steep = smoothstep(a - w, a + w, slope);
    return select(1.0 - steep, steep, place == 1u);
}

// The bumped shading normal at world point p, of the true normal n, the
// footprint `foot` world units. Triplanar: each projection's gradient in
// its plane, lifted to the world and blended by |n|^4; then the surface
// gradient, tilting n by the amplitude.
fn surf_bump(n: vec3<f32>, p: vec3<f32>, foot: f32) -> vec3<f32> {
    if (surf_kind() == 0u) {
        return n;
    }
    let m = surf_mask(n);
    if (!(m > 0.0)) {
        return n;
    }
    var bw = pow(abs(n), vec3<f32>(4.0));
    bw = bw / max(bw.x + bw.y + bw.z, 1.0e-6);
    var g = vec3<f32>(0.0);
    if (bw.z > 0.01) {
        let t = surf_layer(p.x, p.y, 0u, 1u, foot);
        g = g + bw.z * vec3<f32>(t.y, t.z, 0.0);
    }
    if (bw.x > 0.01) {
        let t = surf_layer(p.y, p.z, 1u, 2u, foot);
        g = g + bw.x * vec3<f32>(0.0, t.y, t.z);
    }
    if (bw.y > 0.01) {
        let t = surf_layer(p.x, p.z, 0u, 2u, foot);
        g = g + bw.y * vec3<f32>(t.y, 0.0, t.z);
    }
    let gs = g - dot(g, n) * n;
    return normalize(n - (params.fdata[22].y * m) * gs);
}
"#;
