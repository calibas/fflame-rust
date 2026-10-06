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
//!   and the rest of the path is [`PT_CORE_WGSL`]'s `pt_path`.
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
            band: [band.0, band.1, 0, 0],
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
    // whose sample would outlast the GPU's watchdog over a whole frame).
    band: vec4<u32>,
};

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
// sum: the same bits however the samples are split into dispatches.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let y = gid.y + pt.band.x;
    if (gid.x >= params.width || gid.y >= pt.band.y || y >= params.height) {
        return;
    }
    let idx = y * params.width + gid.x;
    pt_scene_begin();
    var sum = pt_sum[idx];
    if (pt.sample_base == 0u) {
        sum = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let pixel = pt_hash(idx * 0x9E3779B9u + pt.seed);
    for (var s = 0u; s < pt.samples; s = s + 1u) {
        pt_seed = pixel;
        pt_index = pt.sample_base + s;
        pt_dim = 0u;
        sum = sum + pt_sample(gid.x, y);
    }
    pt_sum[idx] = sum;
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

/// A path tracer's per-pixel sum (premultiplied radiance, coverage),
/// the samples in it, and the pass that resolves it into an output.
pub(crate) struct PathSum {
    sum: Buffer,
    px: u32,
    count: u32,
    resolve_layout: BindGroupLayout,
    resolve_pipeline: ComputePipeline,
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
        PathSum { sum: Self::create(device, px), px, count: 0, resolve_layout, resolve_pipeline }
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
    }

    /// `n` more samples are in it, over the whole frame.
    pub(crate) fn add(&mut self, n: u32) {
        self.count += n;
    }

    /// The mean into `out` (`w` by `h`); rows above `split` hold one
    /// sample more than the count. Submits its own work.
    pub(crate) fn resolve(&self, device: &Device, queue: &Queue, out: &TextureView, w: u32, h: u32, split: u32) {
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

    pub(crate) fn destroy(&self) {
        self.sum.destroy();
    }
}
