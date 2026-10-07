//! The path tracer's core (docs/projects/heightfield-3d.md, T3): the
//! sampling, the material, the lens and the integrator, shared by the
//! two geometries that render through it -- a terrain's height field
//! (`terrain.rs`) and mode D's distance field (`renderer/solid_path.rs`).
//!
//! A geometry supplies, in WGSL:
//! - `pt_eye()`: where the camera's rays start;
//! - `pt_scene_begin()`: anything an invocation loads once;
//! - `pt_scene_next(o, d, travelled)`: the surface a ray leaving a
//!   surface reaches -- `o` already off the surface by that surface's
//!   bias, `travelled` the path's length so far -- its normal on the
//!   side that ray sees;
//! - `pt_scene_visible(o, d, travelled)`: whether a shadow ray from a
//!   surface gets away;
//! - `pt_sample(px, py)`: one sample of a pixel, as (radiance times
//!   coverage, coverage) -- the camera ray and the first surface are the
//!   geometry's, since what coverage and fog mean differs between them,
//!   and the rest of the path is [`PT_CORE_WGSL`]'s `pt_path`;
//! - `pt_tile()`: the part of the frame the sum holds -- its origin in
//!   the frame's pixels and its size -- which is the whole frame
//!   (`params.width` by `params.height`) unless a still is drawn in
//!   tiles;
//!
//! and calls `pt_first(h)` with a sample's first surface, which the
//! denoiser's guides are made of.
//!
//! and the rig's accessors (`ifs_fov`, `ifs_forward`, ..., the lights),
//! which both get from the same text (`assembler::IFS_RIG`'s family).

use crate::config::FractalConfig;
use wgpu::*;

/// The path tracer's settings for a pass (plan section 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathSettings {
    /// Bounces after the first surface: 0 is direct light only.
    pub bounces: u32,
    /// The environment's radiance, in the accumulator's units.
    pub environment: [f32; 3],
    /// A sample's radiance is clamped here: the fireflies a rare bright
    /// path would otherwise scatter.
    pub clamp: f32,
    /// Seeds the samples' random streams.
    pub seed: u32,
    /// The gloss coat's reflectance at normal incidence (0: Lambert
    /// alone) and its roughness.
    pub gloss: f32,
    pub roughness: f32,
    /// The albedo's own glow.
    pub emission: f32,
    /// The lens's radius (0: a pinhole) and the focal plane's view
    /// depth, in the world's units.
    pub aperture: f32,
    pub focus: f32,
    /// A terrain lake's roughness.
    pub lake_roughness: f32,
    /// Gather the denoiser's guides with the samples, and filter the
    /// resolve (`PathSum::resolve`).
    pub denoise: bool,
}

impl Default for PathSettings {
    /// Lambert alone through a pinhole: no gloss, no glow, no lens.
    fn default() -> Self {
        PathSettings {
            bounces: 2,
            environment: [0.0; 3],
            clamp: 1.0e30,
            seed: 1,
            gloss: 0.0,
            roughness: 0.5,
            emission: 0.0,
            aperture: 0.0,
            focus: 1.0,
            lake_roughness: 0.05,
            denoise: false,
        }
    }
}

/// The settings from the config (`escape.path`, plan section 4), for a
/// geometry whose target is `target` world units from the eye:
/// - the environment is the background colour brought into the
///   accumulator's units -- through the inverse of the Linear tonemap's
///   exposure and gamma -- so the sky and an albedo-1 surface it lights
///   read as the background does, times Sky light;
/// - a sample's radiance is clamped at ten times the brightest light
///   (the environment and the glow count as lights);
/// - the lens is in the target's terms, so it keeps its look as the
///   camera dollies: the aperture a fraction of the distance, the focus
///   a multiple of it (0, the target itself).
pub fn path_settings(config: &FractalConfig, target: f32) -> PathSettings {
    path_settings_from(&config.escape.path, config, target, config.escape.terrain.lake_roughness)
}

/// [`path_settings`] from any path-tracing block -- a simulation's
/// terrain keeps its own -- with the config's sky and lights.
pub fn path_settings_from(
    t: &crate::config::escape::PathTraceConfig,
    config: &FractalConfig,
    target: f32,
    lake_roughness: f32,
) -> PathSettings {
    let gamma = if config.gamma > 0.0 { config.gamma } else { 1.0 };
    let exposure = config.exposure.max(1.0e-6);
    let env = config.background_color.map(|c| t.environment * c.max(0.0).powf(gamma) / exposure);
    let lights: f32 = config.solid_shading.lights.iter().filter(|l| l.enabled).map(|l| l.intensity.max(0.0)).sum();
    let brightest = lights.max(1.0).max(env.iter().cloned().fold(0.0, f32::max));
    let target = target.max(1.0e-30);
    PathSettings {
        bounces: t.bounces.min(16),
        environment: env,
        clamp: 10.0 * brightest.max(t.emission),
        seed: 1,
        gloss: t.gloss,
        roughness: t.roughness,
        emission: t.emission,
        aperture: t.aperture * target,
        focus: if t.focus > 0.0 { t.focus } else { 1.0 } * target,
        lake_roughness,
        denoise: t.denoise,
    }
}

/// The path tracer's uniform. Mirrored by `PtParams`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PathParamsGpu {
    sample_base: u32,
    samples: u32,
    bounces: u32,
    seed: u32,
    env: [f32; 4],
    misc: [f32; 4],
    mat: [f32; 4],
    lens: [f32; 4],
    band: [u32; 4],
}

impl PathParamsGpu {
    /// A dispatch's uniform: `samples` more after `sample_base`, over the
    /// rows `band` (first, count). `per_ray` is the geometry's share of
    /// a pixel per unit of distance, `shadow` the shadow strength and
    /// `radius` a light's angular radius.
    pub(crate) fn new(
        settings: &PathSettings,
        sample_base: u32,
        samples: u32,
        per_ray: f32,
        shadow: f32,
        radius: f32,
        band: (u32, u32),
    ) -> Self {
        PathParamsGpu {
            sample_base,
            samples,
            bounces: settings.bounces,
            seed: settings.seed,
            env: [settings.environment[0], settings.environment[1], settings.environment[2], 0.0],
            misc: [per_ray, settings.clamp, shadow, radius.cos()],
            mat: [
                settings.gloss.clamp(0.0, 1.0),
                settings.roughness.clamp(0.02, 1.0),
                settings.emission.max(0.0),
                settings.lake_roughness.clamp(0.02, 1.0),
            ],
            lens: [settings.aperture.max(0.0), settings.focus.max(1.0e-6), 0.0, 0.0],
            band: [band.0, band.1, u32::from(settings.denoise), 0],
        }
    }

    /// The uniform buffer holding it.
    pub(crate) fn buffer(&self, device: &Device, queue: &Queue) -> Buffer {
        let buf = device.create_buffer(&BufferDescriptor {
            label: Some("Path Params"),
            size: std::mem::size_of::<PathParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, bytemuck::bytes_of(self));
        buf
    }
}

/// Samples an Auto tier gathers before the path tracer's picture
/// replaces the lit tier's: below this the noise reads worse than the lit
/// picture.
pub const PATH_SHOW_SAMPLES: u32 = 8;

/// A light's angular radius from the rig's shadow sharpness: half its
/// inverse, the lit tier's penumbra in an area light's terms (a
/// sphere-traced penumbra `k d / t` opens over an angle of `1/k`, which
/// is a light's diameter).
pub(crate) fn light_radius(sharpness: f32) -> f32 {
    (0.5 / sharpness.max(1.0e-3)).min(0.5)
}

/// The integrator and everything it samples with.
pub(crate) const PT_CORE_WGSL: &str = r#"
@group(0) @binding(8) var<storage, read_write> pt_sum: array<vec4<f32>>;
@group(0) @binding(9) var<uniform> pt: PtParams;
// The denoiser's guides, two a pixel, summed with the samples weighted
// by their coverage: (albedo, the light's luminance squared) and
// (normal, distance). Written only when `pt.band.z` asks.
@group(0) @binding(10) var<storage, read_write> pt_guide: array<vec4<f32>>;

struct PtParams {
    // Samples already in the sum, and to add now.
    sample_base: u32,
    samples: u32,
    // Bounces after the first surface; 0 is direct light only.
    bounces: u32,
    seed: u32,
    // The environment's radiance.
    env: vec4<f32>,
    // x: a ray's share of a pixel per unit of distance, y: the firefly
    // clamp, z: shadow strength, w: the cosine of a light's angular
    // radius.
    misc: vec4<f32>,
    // x: the gloss coat's reflectance at normal incidence (0: Lambert
    // alone), y: its roughness (GGX alpha is its square), z: emission,
    // the albedo's own glow, w: a terrain lake's roughness.
    mat: vec4<f32>,
    // x: the lens's radius (0: a pinhole), y: the focal plane's view
    // depth.
    lens: vec4<f32>,
    // x: the dispatch's first row, y: its rows (a band, for a geometry
    // whose sample would outlast the GPU's watchdog over a whole frame),
    // z: 1 to gather the denoiser's guides.
    band: vec4<u32>,
};

// A sample's first surface, as its geometry reports it (`pt_first`):
// what the denoiser's guides are made of.
var<private> pt_g_albedo: vec3<f32>;
var<private> pt_g_normal: vec3<f32>;
var<private> pt_g_t: f32;

fn pt_first(h: PtHit) {
    pt_g_albedo = h.albedo.rgb;
    pt_g_normal = h.n;
    pt_g_t = h.t;
}

// The sample's random numbers. Each draw is a point of its own
// shuffled, Owen-scrambled Sobol (0,2)-sequence (Burley 2020, "Practical
// Hash-based Owen Scrambling"): the pixel and the draw's place in the
// sample seed the shuffle and the scrambles, the sample's index walks
// the sequence. Every point is uniform, so nothing is biased; the
// samples of a pixel are stratified in each 2D decision -- the jitter,
// the lens, a light's cone, a bounce -- where independent draws clump.
// Measured against independent (PCG) draws at equal samples: a third
// less error on a terrain and on a solid, the same picture converged.
var<private> pt_seed: u32;
var<private> pt_index: u32;
var<private> pt_dim: u32;

fn pt_hash(x: u32) -> u32 {
    var v = x;
    v = v ^ (v >> 16u);
    v = v * 0x7feb352du;
    v = v ^ (v >> 15u);
    v = v * 0x846ca68bu;
    v = v ^ (v >> 16u);
    return v;
}

// Laine and Karras's permutation: each bit depends on the bits below
// it and the seed.
fn pt_lk(x: u32, seed: u32) -> u32 {
    var v = x + seed;
    v = v ^ (v * 0x6c50b47cu);
    v = v ^ (v * 0xb82f1e52u);
    v = v ^ (v * 0xc7afe638u);
    v = v ^ (v * 0x8d22f6e6u);
    return v;
}

// A nested uniform (Owen) scramble: each bit flipped by a hash of the
// bits above it.
fn pt_owen(x: u32, seed: u32) -> u32 {
    return reverseBits(pt_lk(reverseBits(x), seed));
}

// Sobol's second dimension (its first is the van der Corput sequence,
// the index's bits reversed): direction numbers from v1 = 1/2 by
// v(k+1) = v(k) ^ v(k) / 2.
fn pt_sobol1(i: u32) -> u32 {
    var x = 0u;
    var v = 0x80000000u;
    var b = i;
    loop {
        if (b == 0u) {
            break;
        }
        if ((b & 1u) != 0u) {
            x = x ^ v;
        }
        v = v ^ (v >> 1u);
        b = b >> 1u;
    }
    return x;
}

// The next 2D draw.
fn pt_rand2() -> vec2<f32> {
    let seed = pt_hash(pt_seed ^ pt_hash(pt_dim * 0x9E3779B9u + 0x85EBCA6Bu));
    pt_dim = pt_dim + 1u;
    // Shuffling the index keeps each aligned block of 2^m samples a
    // block of the sequence -- a (0, m, 2)-net -- whatever the seed.
    let i = pt_owen(pt_index, seed);
    let x = pt_owen(reverseBits(i), pt_hash(seed ^ 0x68E31DA4u));
    let y = pt_owen(pt_sobol1(i), pt_hash(seed ^ 0xB5297A4Du));
    return vec2<f32>(f32(x >> 8u), f32(y >> 8u)) * (1.0 / 16777216.0);
}

// The next 1D draw: a sequence of its own, stratified in one dimension.
fn pt_rand() -> f32 {
    return pt_rand2().x;
}

// An orthonormal frame about n (Duff et al. 2017), n its third column.
fn pt_frame(n: vec3<f32>) -> mat3x3<f32> {
    let s = select(-1.0, 1.0, n.z >= 0.0);
    let a = -1.0 / (s + n.z);
    let b = n.x * n.y * a;
    return mat3x3<f32>(
        vec3<f32>(1.0 + s * n.x * n.x * a, s * b, -s * n.x),
        vec3<f32>(b, s + n.y * n.y * a, -n.y),
        n,
    );
}

// A direction about n, cosine-weighted: Lambert's importance.
fn pt_cosine(n: vec3<f32>) -> vec3<f32> {
    let u = pt_rand2();
    let r1 = u.x;
    let r2 = u.y;
    let phi = 6.283185307 * r1;
    let r = sqrt(r2);
    return pt_frame(n) * vec3<f32>(r * cos(phi), r * sin(phi), sqrt(max(1.0 - r2, 0.0)));
}

// A direction within the cone of `cos_max` about `axis`, uniformly.
fn pt_cone(axis: vec3<f32>, cos_max: f32) -> vec3<f32> {
    let u = pt_rand2();
    let c = 1.0 - u.x * (1.0 - cos_max);
    let s = sqrt(max(1.0 - c * c, 0.0));
    let phi = 6.283185307 * u.y;
    return pt_frame(axis) * vec3<f32>(s * cos(phi), s * sin(phi), c);
}

struct PtRay {
    o: vec3<f32>,
    d: vec3<f32>,
};

// The camera's ray through (px + 0.5 + jx, py + 0.5 + jy): `ifs_ray`
// with the sample's own jitter -- and, with a lens, from a point of the
// lens's disc toward where the pinhole's ray meets the focal plane, so
// the focal plane is sharp and the rest blurs with its distance from it.
fn pt_ray(px: u32, py: u32, jx: f32, jy: f32) -> PtRay {
    // `var`: a camera lens rewrites it in place.
    var uv = (vec2<f32>(f32(px) + 0.5 + jx, f32(py) + 0.5 + jy)) / vec2<f32>(f32(params.width), f32(params.height))
        - vec2<f32>(0.5, 0.5);
    let aspect = f32(params.width) / f32(max(params.height, 1u));
    //__LENS_APPLY_RAY__
    let tan_half = tan(ifs_fov() * 0.5);
    let d = normalize(ifs_forward() + ifs_right() * (uv.x * aspect * 2.0 * tan_half) - ifs_up() * (uv.y * 2.0 * tan_half));
    var out: PtRay;
    out.o = pt_eye();
    out.d = d;
    if (pt.lens.x > 0.0) {
        let focal = out.o + d * (pt.lens.y / max(dot(d, ifs_forward()), 1.0e-4));
        let u = pt_rand2();
        let r = pt.lens.x * sqrt(u.x);
        let phi = 6.283185307 * u.y;
        out.o = out.o + ifs_right() * (r * cos(phi)) + ifs_up() * (r * sin(phi));
        out.d = normalize(focal - out.o);
    }
    return out;
}

// The gloss coat: GGX's distribution, Smith's masking for one direction,
// Schlick's Fresnel on a scalar reflectance.
fn pt_ggx_d(nh: f32, a2: f32) -> f32 {
    let k = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (3.141592654 * k * k);
}

fn pt_g1(nx: f32, a2: f32) -> f32 {
    return 2.0 * nx / (nx + sqrt(a2 + (1.0 - a2) * nx * nx));
}

fn pt_fresnel(f0: f32, c: f32) -> f32 {
    let m = clamp(1.0 - c, 0.0, 1.0);
    let m2 = m * m;
    return f0 + (1.0 - f0) * m2 * m2 * m;
}

// A half-vector of the visible normals for view v, in the frame where
// the normal is +z (Heitz 2018): sampling the reflection by the share of
// it a viewer sees.
fn pt_vndf(v: vec3<f32>, alpha: f32) -> vec3<f32> {
    let vh = normalize(vec3<f32>(alpha * v.x, alpha * v.y, v.z));
    let lensq = vh.x * vh.x + vh.y * vh.y;
    var t1 = vec3<f32>(1.0, 0.0, 0.0);
    if (lensq > 0.0) {
        t1 = vec3<f32>(-vh.y, vh.x, 0.0) * inverseSqrt(lensq);
    }
    let t2 = cross(vh, t1);
    let u = pt_rand2();
    let r = sqrt(u.x);
    let phi = 6.283185307 * u.y;
    let p1 = r * cos(phi);
    let s = 0.5 * (1.0 + vh.z);
    let p2 = (1.0 - s) * sqrt(max(1.0 - p1 * p1, 0.0)) + s * r * sin(phi);
    let nh = p1 * t1 + p2 * t2 + sqrt(max(1.0 - p1 * p1 - p2 * p2, 0.0)) * vh;
    return normalize(vec3<f32>(alpha * nh.x, alpha * nh.y, max(nh.z, 0.0)));
}

// A surface a ray reached: the geometry's answer.
struct PtHit {
    hit: bool,
    // Along the ray.
    t: f32,
    // The shading normal, on the side the ray came from: a height
    // field's surface has two, a solid's is outward even where a grazing
    // ray's tolerance stopped it beside the silhouette (flipping that one
    // would point a bounce into the solid).
    n: vec3<f32>,
    // Its albedo; alpha 0 is a hole, the sky through it.
    albedo: vec4<f32>,
    // How far off the surface a ray leaving it starts.
    bias: f32,
    // Its coat: reflectance at normal incidence (0, none) and roughness
    // -- the config's, or a material's own (a terrain's lake).
    f0: f32,
    rough: f32,
};

// The radiance a path brings back from its first surface `first`,
// reached along (o0, d0): at each surface its glow, the lights sampled
// over their angular size with a shadow ray each, and a bounce -- the
// coat's lobe or Lambert's -- whose escape sees the environment, with
// Russian roulette from the second.
fn pt_path(o0: vec3<f32>, d0: vec3<f32>, first: PtHit) -> vec3<f32> {
    var o = o0;
    var d = d0;
    var h = first;
    var radiance = vec3<f32>(0.0, 0.0, 0.0);
    var through = vec3<f32>(1.0, 1.0, 1.0);
    var travelled = 0.0;
    for (var bounce = 0u; bounce <= pt.bounces; bounce = bounce + 1u) {
        let p = o + d * h.t;
        travelled = travelled + h.t;
        if (!(h.albedo.a > 0.0)) {
            // A hole's floor: nothing there; past the first surface, the
            // sky through it.
            if (bounce > 0u) {
                radiance = radiance + through * pt.env.rgb;
            }
            break;
        }
        let n = h.n;
        let albedo = h.albedo.rgb;
        let start = p + n * h.bias;
        // Its glow.
        radiance = radiance + through * albedo * pt.mat.z;
        // The coat: Lambert under a gloss of reflectance f0 at normal
        // incidence, the diffuse taking what the coat's Fresnel does not.
        let f0 = h.f0;
        let a2 = max(h.rough * h.rough, 1.0e-4) * max(h.rough * h.rough, 1.0e-4);
        let v = -d;
        let nv = max(dot(n, v), 1.0e-4);
        // Every light, each sampled over its angular size. Picking one a
        // surface (by power) would save shadow rays, but a face only one
        // light reaches then reads all of it or none, sample to sample:
        // measured as speckle over a two-light solid's lit faces. The
        // diffuse in the rig's units (its I as pi times a radiance's), the
        // gloss's physical BRDF times the same irradiance, pi I cos. No
        // shadow ray at strength 0.
        for (var li = 0u; li < ifs_light_count(); li = li + 1u) {
            let ld = pt_cone(ifs_light_dir(li), pt.misc.w);
            let ndl = dot(n, ld);
            if (ndl > 0.0) {
                var vis = 1.0;
                if (pt.misc.z > 0.0) {
                    vis = mix(1.0, select(0.0, 1.0, pt_scene_visible(start, ld, travelled)), clamp(pt.misc.z, 0.0, 1.0));
                }
                var spec = 0.0;
                var diff = 1.0;
                if (f0 > 0.0) {
                    let hv = normalize(ld + v);
                    let fr = pt_fresnel(f0, max(dot(v, hv), 0.0));
                    spec = 3.141592654 * pt_ggx_d(max(dot(n, hv), 0.0), a2) * pt_g1(ndl, a2) * pt_g1(nv, a2) * fr / (4.0 * ndl * nv);
                    diff = 1.0 - fr;
                }
                let light = ifs_light_color(li) * (ifs_light_power(li) * ndl * vis);
                radiance = radiance + through * light * (albedo * (ifs_diffuse() * diff) + vec3<f32>(spec));
            }
        }
        if (bounce >= pt.bounces) {
            break;
        }
        // The bounce: the gloss's lobe by the visible normals, chosen by
        // the coat's Fresnel at this view; else Lambert's, whose cosine
        // sampling leaves the albedo times what the coat lets through.
        var nd = vec3<f32>(0.0, 0.0, 1.0);
        // Without a coat, no draw to choose a lobe: Lambert's samples are
        // the same stream they were before the coat existed.
        let p_spec = select(0.0, clamp(pt_fresnel(f0, nv), 0.1, 0.9), f0 > 0.0);
        if (p_spec > 0.0 && pt_rand() < p_spec) {
            let frame = pt_frame(n);
            let vl = transpose(frame) * v;
            let hl = pt_vndf(vl, sqrt(a2));
            let hv = frame * hl;
            nd = reflect(-v, hv);
            let nl = dot(n, nd);
            if (nl <= 0.0) {
                break;
            }
            through = through * (pt_fresnel(f0, max(dot(v, hv), 0.0)) * pt_g1(nl, a2) / p_spec);
        } else {
            nd = pt_cosine(n);
            // No coat at all at reflectance 0: Schlick's term alone would
            // still take (1 - cos)^5 of a glance.
            let coat = select(0.0, pt_fresnel(f0, nv), f0 > 0.0);
            through = through * albedo * ((1.0 - coat) / (1.0 - p_spec));
        }
        if (bounce >= 1u) {
            let q = clamp(max(through.r, max(through.g, through.b)), 0.05, 0.95);
            if (pt_rand() > q) {
                break;
            }
            through = through / q;
        }
        o = start;
        d = nd;
        h = pt_scene_next(o, d, travelled);
        if (!h.hit) {
            radiance = radiance + through * pt.env.rgb;
            break;
        }
    }
    return radiance;
}

// Each invocation adds its pixel's samples, in order, to the running
// sum: the same bits however the samples are split into dispatches --
// and, its random numbers keyed by its place in the whole frame, however
// the frame is split into tiles.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tile = pt_tile();
    let y = gid.y + pt.band.x;
    if (gid.x >= tile.z || gid.y >= pt.band.y || y >= tile.w) {
        return;
    }
    let idx = y * tile.z + gid.x;
    let fx = tile.x + gid.x;
    let fy = tile.y + y;
    pt_scene_begin();
    var sum = pt_sum[idx];
    if (pt.sample_base == 0u) {
        sum = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let guided = pt.band.z != 0u;
    var g0 = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var g1 = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (guided && pt.sample_base != 0u) {
        g0 = pt_guide[2u * idx];
        g1 = pt_guide[2u * idx + 1u];
    }
    let pixel = pt_hash((fy * params.width + fx) * 0x9E3779B9u + pt.seed);
    for (var s = 0u; s < pt.samples; s = s + 1u) {
        pt_seed = pixel;
        pt_index = pt.sample_base + s;
        pt_dim = 0u;
        let v = pt_sample(fx, fy);
        sum = sum + v;
        if (guided && v.a > 0.0) {
            let l = dot(v.rgb / v.a, vec3<f32>(0.2126, 0.7152, 0.0722));
            g0 = g0 + vec4<f32>(pt_g_albedo * v.a, l * l * v.a);
            g1 = g1 + vec4<f32>(pt_g_normal * v.a, pt_g_t * v.a);
        }
    }
    pt_sum[idx] = sum;
    if (guided) {
        pt_guide[2u * idx] = g0;
        pt_guide[2u * idx + 1u] = g1;
    }
}
"#;

/// The path tracer's sum into an output: colour the premultiplied mean
/// over the coverage, alpha the mean coverage -- the accumulator
/// contract. Rows above `split` hold one sample more than `count` (a
/// banded pass part-way down the frame).
pub(crate) const PATH_RESOLVE_WGSL: &str = r#"
struct ResolveParams {
    width: u32,
    height: u32,
    count: u32,
    split: u32,
};
@group(0) @binding(0) var<uniform> rp: ResolveParams;
@group(0) @binding(1) var<storage, read> pt_sum: array<vec4<f32>>;
@group(0) @binding(2) var out_tex: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= rp.width || gid.y >= rp.height) {
        return;
    }
    let s = pt_sum[gid.y * rp.width + gid.x];
    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    if (s.a > 0.0) {
        rgb = s.rgb / s.a;
    }
    let n = rp.count + select(0u, 1u, gid.y < rp.split);
    // A row no sample has reached yet: nothing there, whatever the sum
    // still holds from before.
    if (n == 0u) {
        textureStore(out_tex, vec2<i32>(gid.xy), vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    textureStore(out_tex, vec2<i32>(gid.xy), vec4<f32>(rgb, s.a / f32(n)));
}
"#;

/// The denoiser (heightfield plan T5): edge-avoiding à-trous wavelet
/// filtering, after Dammertz et al. 2010 and its variance guidance after
/// Schied et al. 2017 (SVGF), over one accumulation rather than frames.
///
/// - `prepare`: each pixel's mean light divided by its mean albedo -- so
///   the filter sees the light alone and the colouring's detail, which
///   lives in the albedo, is multiplied back untouched -- with that
///   light's variance: of the mean, from the samples' moments, or with
///   fewer than four samples from its neighbours'. And its features: the
///   normal (octahedral) and the distance, with the distance's gradient
///   across a pixel.
/// - `atrous`, five times at strides 1 to 16: a 5x5 B3 kernel whose taps
///   are weighted by the normals' agreement, the distances' against the
///   gradient, and the lights' against the noise -- a few standard
///   deviations of the mean, so as the samples gather and the noise
///   falls the filter stops crossing anything.
/// - `finish`: the light times the albedo, and the coverage as the
///   resolve has it.
pub(crate) const PATH_DENOISE_WGSL: &str = r#"
struct DenoiseParams {
    width: u32,
    height: u32,
    count: u32,
    split: u32,
    step: u32,
    // How many of the noise's standard deviations two lights may differ
    // by and still be averaged.
    sigma: f32,
    pad1: u32,
    pad2: u32,
};
@group(0) @binding(0) var<uniform> dp: DenoiseParams;
@group(0) @binding(1) var<storage, read> pt_sum: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> pt_guide: array<vec4<f32>>;
@group(0) @binding(3) var light_in: texture_2d<f32>;
@group(0) @binding(4) var feat_in: texture_2d<f32>;
@group(0) @binding(5) var light_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(6) var feat_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(7) var out_tex: texture_storage_2d<rgba32float, write>;

// The albedo's luminance below which the variance is not divided by it.
const DN_FLOOR: f32 = 0.01;

fn dn_lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// A row's samples: the count, one more above the split.
fn dn_n(y: u32) -> u32 {
    return dp.count + select(0u, 1u, y < dp.split);
}

fn dn_inside(q: vec2<i32>) -> bool {
    return q.x >= 0 && q.y >= 0 && q.x < i32(dp.width) && q.y < i32(dp.height);
}

// A pixel's means over its coverage (0: no sample found a surface).
struct DnPixel {
    cover: f32,
    light: vec3<f32>,
    albedo: vec3<f32>,
    normal: vec3<f32>,
    depth: f32,
    // The radiance's luminance, its mean and its mean square.
    m1: f32,
    m2: f32,
};

fn dn_pixel(q: vec2<i32>) -> DnPixel {
    var o: DnPixel;
    o.cover = 0.0;
    let i = u32(q.y) * dp.width + u32(q.x);
    let s = pt_sum[i];
    if (!(s.a > 0.0) || dn_n(u32(q.y)) == 0u) {
        return o;
    }
    let g0 = pt_guide[2u * i];
    let g1 = pt_guide[2u * i + 1u];
    o.cover = s.a;
    o.albedo = g0.rgb / s.a;
    let c = s.rgb / s.a;
    // The light: the colour over the albedo, exactly, channel by channel
    // -- a floor in the division leaves a dark channel's light a fraction
    // of its neighbours', and averaging them multiplied it back up (a
    // yellow face gained +13 levels of blue). A channel with no albedo
    // reflects nothing to divide; the others' light stands in for its
    // neighbours to average, and it is multiplied back by zero.
    let has = o.albedo > vec3<f32>(1.0e-12, 1.0e-12, 1.0e-12);
    let e = select(vec3<f32>(0.0, 0.0, 0.0), c / max(o.albedo, vec3<f32>(1.0e-12, 1.0e-12, 1.0e-12)), has);
    let k = dot(select(vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0), has), vec3<f32>(1.0, 1.0, 1.0));
    let stand_in = (e.x + e.y + e.z) / max(k, 1.0);
    o.light = select(vec3<f32>(stand_in, stand_in, stand_in), e, has);
    o.m1 = dn_lum(c);
    o.m2 = g0.a / s.a;
    let l = length(g1.xyz);
    o.normal = select(vec3<f32>(0.0, 0.0, 1.0), g1.xyz / max(l, 1.0e-20), l > 1.0e-12);
    o.depth = g1.w / s.a;
    return o;
}

// The octahedral map of a unit normal into two numbers, and back.
fn dn_oct(n: vec3<f32>) -> vec2<f32> {
    let p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if (n.z >= 0.0) {
        return p;
    }
    return (vec2<f32>(1.0, 1.0) - abs(p.yx)) * select(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), p >= vec2<f32>(0.0, 0.0));
}

fn dn_unoct(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    let t = max(-n.z, 0.0);
    n.x = n.x + select(t, -t, n.x >= 0.0);
    n.y = n.y + select(t, -t, n.y >= 0.0);
    return normalize(n);
}

// The distance's change across a pixel along one axis: the smaller of
// the one-sided differences, so a silhouette beside the pixel does not
// read as a slope.
fn dn_slope(c: f32, a: f32, b: f32) -> f32 {
    let da = select(1.0e30, abs(c - a), a > 0.0);
    let db = select(1.0e30, abs(b - c), b > 0.0);
    let m = min(da, db);
    return select(0.0, m, m < 1.0e29);
}

fn dn_depth(q: vec2<i32>) -> f32 {
    if (!dn_inside(q)) {
        return 0.0;
    }
    let i = u32(q.y) * dp.width + u32(q.x);
    let a = pt_sum[i].a;
    if (!(a > 0.0) || dn_n(u32(q.y)) == 0u) {
        return 0.0;
    }
    return pt_guide[2u * i + 1u].w / a;
}

@compute @workgroup_size(8, 8, 1)
fn prepare(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= dp.width || gid.y >= dp.height) {
        return;
    }
    let px = vec2<i32>(gid.xy);
    let p = dn_pixel(px);
    if (!(p.cover > 0.0)) {
        textureStore(light_out, px, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        textureStore(feat_out, px, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    let al = max(dn_lum(p.albedo), DN_FLOOR);
    var variance = 0.0;
    if (dn_n(gid.y) >= 4u) {
        // The mean's: the samples' over their number.
        variance = max(p.m2 - p.m1 * p.m1, 0.0) / max(p.cover, 1.0) / (al * al);
    } else {
        // Too few samples for moments: the light's spread over the
        // neighbours on the same surface.
        var s1 = 0.0;
        var s2 = 0.0;
        var sw = 0.0;
        for (var dy = -3; dy <= 3; dy = dy + 1) {
            for (var dx = -3; dx <= 3; dx = dx + 1) {
                let q = px + vec2<i32>(dx, dy);
                if (!dn_inside(q)) {
                    continue;
                }
                let o = dn_pixel(q);
                if (!(o.cover > 0.0)) {
                    continue;
                }
                let w = max(dot(o.normal, p.normal), 0.0) * exp(-abs(o.depth - p.depth) / (p.depth * 0.02 + 1.0e-20));
                let l = dn_lum(o.light);
                s1 = s1 + w * l;
                s2 = s2 + w * l * l;
                sw = sw + w;
            }
        }
        if (sw > 0.0) {
            let m = s1 / sw;
            variance = max(s2 / sw - m * m, 0.0);
        }
    }
    let gx = dn_slope(p.depth, dn_depth(px - vec2<i32>(1, 0)), dn_depth(px + vec2<i32>(1, 0)));
    let gy = dn_slope(p.depth, dn_depth(px - vec2<i32>(0, 1)), dn_depth(px + vec2<i32>(0, 1)));
    textureStore(light_out, px, vec4<f32>(p.light, variance));
    textureStore(feat_out, px, vec4<f32>(dn_oct(p.normal), p.depth, max(gx, gy)));
}

// The B3 spline's weights by the tap's distance from the centre.
fn dn_b3(k: i32) -> f32 {
    let a = abs(k);
    return select(select(0.0625, 0.25, a == 1), 0.375, a == 0);
}

@compute @workgroup_size(8, 8, 1)
fn atrous(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= dp.width || gid.y >= dp.height) {
        return;
    }
    let px = vec2<i32>(gid.xy);
    let fc = textureLoad(feat_in, px, 0);
    let c = textureLoad(light_in, px, 0);
    if (!(fc.z > 0.0)) {
        textureStore(light_out, px, c);
        return;
    }
    // The noise's deviation, from the variance blurred 3x3.
    var gv = 0.0;
    var gw = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let q = px + vec2<i32>(dx, dy);
            if (!dn_inside(q)) {
                continue;
            }
            if (!(textureLoad(feat_in, q, 0).z > 0.0)) {
                continue;
            }
            let k = select(select(0.0625, 0.125, dx == 0 || dy == 0), 0.25, dx == 0 && dy == 0);
            gv = gv + k * textureLoad(light_in, q, 0).a;
            gw = gw + k;
        }
    }
    let deviation = dp.sigma * sqrt(max(gv / max(gw, 1.0e-20), 0.0)) + 1.0e-20;
    let nc = dn_unoct(fc.xy);
    let lc = dn_lum(c.rgb);
    let step = i32(dp.step);
    let h0 = 0.375 * 0.375;
    var sum = c.rgb * h0;
    var wsum = h0;
    var vsum = c.a * h0 * h0;
    for (var ky = -2; ky <= 2; ky = ky + 1) {
        for (var kx = -2; kx <= 2; kx = kx + 1) {
            if (kx == 0 && ky == 0) {
                continue;
            }
            let q = px + vec2<i32>(kx, ky) * step;
            if (!dn_inside(q)) {
                continue;
            }
            let fq = textureLoad(feat_in, q, 0);
            if (!(fq.z > 0.0)) {
                continue;
            }
            let lq = textureLoad(light_in, q, 0);
            // The normals': their cosine to the 128th.
            var wn = max(dot(nc, dn_unoct(fq.xy)), 0.0);
            wn = wn * wn;
            wn = wn * wn;
            wn = wn * wn;
            wn = wn * wn;
            wn = wn * wn;
            wn = wn * wn;
            wn = wn * wn;
            let reach = f32(step * max(abs(kx), abs(ky)));
            let wz = exp(-abs(fc.z - fq.z) / (fc.w * reach + fc.z * 1.0e-3));
            let wl = exp(-abs(lc - dn_lum(lq.rgb)) / deviation);
            let w = dn_b3(kx) * dn_b3(ky) * wn * wz * wl;
            sum = sum + lq.rgb * w;
            wsum = wsum + w;
            vsum = vsum + w * w * lq.a;
        }
    }
    textureStore(light_out, px, vec4<f32>(sum / wsum, vsum / (wsum * wsum)));
}

@compute @workgroup_size(8, 8, 1)
fn finish(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= dp.width || gid.y >= dp.height) {
        return;
    }
    let px = vec2<i32>(gid.xy);
    let n = dn_n(gid.y);
    let p = dn_pixel(px);
    if (n == 0u || !(p.cover > 0.0)) {
        textureStore(out_tex, px, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    let light = textureLoad(light_in, px, 0).rgb;
    textureStore(out_tex, px, vec4<f32>(light * p.albedo, p.cover / f32(n)));
}
"#;

/// How many of the noise's standard deviations two pixels' lights may
/// differ by and still be averaged: SVGF's 4. Measured against 1, 2 and
/// 8 on a terrain and two solids at 4 and 16 samples, it was the best or
/// within 0.03 of it everywhere.
const DENOISE_SIGMA: f32 = 4.0;

/// The samples to which the filter keeps that width; past them it
/// narrows as `sqrt(DENOISE_FULL / n)`. Where a pixel's light varies
/// within it -- a solid's sub-pixel holes -- its samples' spread is the
/// geometry's, not noise to average away, and neighbours that truly
/// differ stay within four deviations of the mean for hundreds of
/// samples: at a constant width a sponge at 128 samples came out 50%
/// further from converged than undenoised. Narrowed from 8 it is never
/// further (measured to 128), and a terrain keeps most of its gain (64
/// samples: 0.0021 from 0.0029; constant width, 0.0017).
const DENOISE_FULL: f32 = 8.0;

/// The à-trous passes' strides.
const DENOISE_STEPS: [u32; 5] = [1, 2, 4, 8, 16];

/// How far the denoiser reaches from a pixel, in pixels: the gradient's
/// and the few-sample variance's neighbours, and each pass's two strides.
/// A tile drawn with this much more around it denoises as the whole
/// frame does.
pub(crate) const DENOISE_REACH: u32 = 3 + 2 * (1 + 2 + 4 + 8 + 16);

/// The denoiser's pipelines and its textures, at a size.
struct Denoiser {
    prepare: ComputePipeline,
    atrous: ComputePipeline,
    finish: ComputePipeline,
    /// Two lights (ping-pong) and the features, at `size`.
    textures: Option<([(Texture, TextureView); 3], (u32, u32))>,
}

/// A path tracer's per-pixel sum (premultiplied radiance, coverage),
/// the samples in it, and the pass that resolves it into an output.
pub(crate) struct PathSum {
    sum: Buffer,
    px: u32,
    count: u32,
    resolve_layout: BindGroupLayout,
    resolve_pipeline: ComputePipeline,
    /// The denoiser's guides (two vec4s a pixel), once it has been asked
    /// for, and a stand-in for binding 10 while it is not.
    guide: Option<Buffer>,
    guide_px: u32,
    guided: bool,
    no_guide: Buffer,
    denoiser: Option<Denoiser>,
    /// When the denoiser last ran: the count and split it filtered, and
    /// the time (`resolve`'s schedule).
    denoised: Option<(u32, u32, web_time::Instant)>,
}

impl PathSum {
    pub(crate) fn new(device: &Device, px: u32) -> Self {
        let resolve_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Path Resolve"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Path Resolve"),
            source: ShaderSource::Wgsl(PATH_RESOLVE_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Path Resolve"),
            bind_group_layouts: &[Some(&resolve_layout)],
            immediate_size: 0,
        });
        let resolve_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Path Resolve"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let px = px.max(1);
        let no_guide = device.create_buffer(&BufferDescriptor {
            label: Some("Path Guide (none)"),
            size: 32,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        PathSum {
            sum: Self::create(device, px),
            px,
            count: 0,
            resolve_layout,
            resolve_pipeline,
            guide: None,
            guide_px: 0,
            guided: false,
            no_guide,
            denoiser: None,
            denoised: None,
        }
    }

    /// Gather the denoiser's guides with the samples from now on, or
    /// not. True when that changed: the guides start with the sum, so the
    /// caller starts it over.
    pub(crate) fn set_guided(&mut self, device: &Device, on: bool) -> bool {
        let changed = on != self.guided;
        self.guided = on;
        if on && (self.guide.is_none() || self.guide_px < self.px) {
            if let Some(g) = self.guide.take() {
                g.destroy();
            }
            self.guide = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Path Guide"),
                size: self.px as u64 * 32,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            self.guide_px = self.px;
        }
        changed
    }

    /// What binding 10 holds: the guides, or a stand-in the shader does
    /// not touch.
    pub(crate) fn guide_buffer(&self) -> &Buffer {
        match (&self.guide, self.guided) {
            (Some(g), true) => g,
            _ => &self.no_guide,
        }
    }

    fn create(device: &Device, px: u32) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("Path Sum"),
            size: px as u64 * 16,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Room for `px` pixels; the sum starts over.
    pub(crate) fn ensure(&mut self, device: &Device, px: u32) {
        if px > self.px {
            self.sum.destroy();
            self.sum = Self::create(device, px);
            self.px = px;
        }
        self.count = 0;
        self.denoised = None;
    }

    pub(crate) fn buffer(&self) -> &Buffer {
        &self.sum
    }

    /// Samples in the sum (complete passes over the frame).
    pub(crate) fn count(&self) -> u32 {
        self.count
    }

    /// Start over: the next dispatch at sample 0 replaces the sum.
    pub(crate) fn reset(&mut self) {
        self.count = 0;
        self.denoised = None;
    }

    /// `n` more samples are in it, over the whole frame.
    pub(crate) fn add(&mut self, n: u32) {
        self.count += n;
    }

    /// The mean into `out` (`w` by `h`); rows above `split` hold one
    /// sample more than the count. Submits its own work.
    ///
    /// Denoised when the guides are on -- on a schedule, since the filter
    /// costs about a sample of a terrain at 1080p (10 ms on a GTX 1660)
    /// and a few more samples change the picture little: every count up
    /// to 4, then each time the count has grown by a quarter, and at
    /// least every quarter second while anything new is in the sum. In
    /// between, `out` keeps the last. [`Self::resolve_now`] for the
    /// picture that must be current.
    pub(crate) fn resolve(&mut self, device: &Device, queue: &Queue, out: &TextureView, w: u32, h: u32, split: u32) {
        if self.guided && self.guide.is_some() {
            let due = match self.denoised {
                None => true,
                Some((count, at_split, at)) => {
                    (self.count, split) != (count, at_split)
                        && (self.count <= 4 || self.count * 4 >= count * 5 || at.elapsed().as_millis() >= 250)
                }
            };
            if due {
                self.denoise(device, queue, out, w, h, split);
                self.denoised = Some((self.count, split, web_time::Instant::now()));
            }
            return;
        }
        let buf = device.create_buffer(&BufferDescriptor {
            label: Some("Path Resolve Params"),
            size: 16,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, bytemuck::cast_slice(&[w, h, self.count, split]));
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Path Resolve"),
            layout: &self.resolve_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.sum.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(out) },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Path Resolve") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Path Resolve"), timestamp_writes: None });
            pass.set_pipeline(&self.resolve_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    /// [`Self::resolve`], denoised now whatever the schedule says: the
    /// last of a still, or of a viewport's target.
    pub(crate) fn resolve_now(&mut self, device: &Device, queue: &Queue, out: &TextureView, w: u32, h: u32, split: u32) {
        self.denoised = None;
        self.resolve(device, queue, out, w, h, split);
    }

    /// The denoiser's resolve: `prepare`, the à-trous passes and
    /// `finish`, into `out`.
    fn denoise(&mut self, device: &Device, queue: &Queue, out: &TextureView, w: u32, h: u32, split: u32) {
        let d = self.denoiser.get_or_insert_with(|| {
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Path Denoise"),
                source: ShaderSource::Wgsl(PATH_DENOISE_WGSL.into()),
            });
            let make = |entry: &str| {
                device.create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some(&format!("Path Denoise {entry}")),
                    layout: None,
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            };
            Denoiser { prepare: make("prepare"), atrous: make("atrous"), finish: make("finish"), textures: None }
        });
        if d.textures.as_ref().is_none_or(|(_, s)| *s != (w, h)) {
            if let Some((ts, _)) = d.textures.take() {
                for (t, _) in ts {
                    t.destroy();
                }
            }
            let make = || {
                let t = device.create_texture(&TextureDescriptor {
                    label: Some("Path Denoise"),
                    size: Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba32Float,
                    usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let v = t.create_view(&TextureViewDescriptor::default());
                (t, v)
            };
            d.textures = Some(([make(), make(), make()], (w, h)));
        }
        let ([a, b, feat], _) = d.textures.as_ref().expect("made above");
        let guide = self.guide.as_ref().expect("guided");
        let params = |step: u32| {
            let buf = device.create_buffer(&BufferDescriptor {
                label: Some("Path Denoise Params"),
                size: 32,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let sigma = DENOISE_SIGMA * (DENOISE_FULL / self.count.max(1) as f32).sqrt().min(1.0);
            queue.write_buffer(&buf, 0, bytemuck::cast_slice(&[w, h, self.count, split, step, sigma.to_bits(), 0, 0]));
            buf
        };
        let (gx, gy) = (w.div_ceil(8), h.div_ceil(8));
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Path Denoise") });
        let mut run = |enc: &mut CommandEncoder, pipeline: &ComputePipeline, entries: &[BindGroupEntry]| {
            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Path Denoise"),
                layout: &pipeline.get_bind_group_layout(0),
                entries,
            });
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Path Denoise"), timestamp_writes: None });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        };
        let p0 = params(0);
        run(
            &mut enc,
            &d.prepare,
            &[
                BindGroupEntry { binding: 0, resource: p0.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.sum.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: guide.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: BindingResource::TextureView(&a.1) },
                BindGroupEntry { binding: 6, resource: BindingResource::TextureView(&feat.1) },
            ],
        );
        let (mut from, mut to) = (a, b);
        for step in DENOISE_STEPS {
            let ps = params(step);
            run(
                &mut enc,
                &d.atrous,
                &[
                    BindGroupEntry { binding: 0, resource: ps.as_entire_binding() },
                    BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&from.1) },
                    BindGroupEntry { binding: 4, resource: BindingResource::TextureView(&feat.1) },
                    BindGroupEntry { binding: 5, resource: BindingResource::TextureView(&to.1) },
                ],
            );
            std::mem::swap(&mut from, &mut to);
        }
        run(
            &mut enc,
            &d.finish,
            &[
                BindGroupEntry { binding: 0, resource: p0.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.sum.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: guide.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&from.1) },
                BindGroupEntry { binding: 7, resource: BindingResource::TextureView(out) },
            ],
        );
        queue.submit(std::iter::once(enc.finish()));
    }

    pub(crate) fn destroy(&self) {
        self.sum.destroy();
        self.no_guide.destroy();
        if let Some(g) = &self.guide {
            g.destroy();
        }
        if let Some((ts, _)) = self.denoiser.as_ref().and_then(|d| d.textures.as_ref()) {
            for (t, _) in ts {
                t.destroy();
            }
        }
    }
}
