//! The path tracer's core (docs/archive/terrain/heightfield-3d.md, T3): the
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
//! - `pt_scene_visible(o, d, t_end, travelled)`: whether a shadow ray
//!   from a surface gets away, or gets as far as `t_end` (a light in the
//!   world: lights-in-the-fractal);
//! - `pt_sample(px, py)`: one sample of a pixel, as (radiance times
//!   coverage, coverage) -- the camera ray and the first surface are the
//!   geometry's, since what coverage and fog mean differs between them,
//!   and the rest of the path is [`PT_CORE_WGSL`]'s `pt_path`;
//! - `pt_tile()`: the part of the frame the sum holds -- its origin in
//!   the frame's pixels and its size -- which is the whole frame
//!   (`params.width` by `params.height`) unless a still is drawn in
//!   tiles;
//!
//! and calls `pt_first(h, d)` with a sample's first surface and its ray,
//! which the denoiser's guides are made of.
//!
//! and the rig's accessors (`ifs_fov`, `ifs_forward`, ..., the lights),
//! which both get from the same text (`assembler::IFS_RIG`'s family).

use crate::config::FractalConfig;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
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
    /// How much of the surface is metal (P4): the shaders read it from
    /// the rig (`ifs_metallic`); here for the denoiser (D5).
    pub metallic: f32,
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
    /// A gradient sky's zenith radiance, the environment's at the
    /// horizon (None: the environment is uniform).
    pub sky: Option<[f32; 3]>,
    /// The studio's brightness ([`studio_level`]; 0, none): the
    /// environment is then the studio's, plus the ambient.
    pub studio: f32,
    /// Multiple importance sampling of the coat and the lights (P4; on):
    /// off only to measure it against.
    pub mis: bool,
    /// Adaptive sampling (V2): the noise, in the display's levels of 255,
    /// under which a pixel stops taking samples ([`PathSum::check_if_due`];
    /// 0, off).
    pub noise_threshold: f32,
    /// How the display shows the accumulator's units, for the noise's
    /// levels: the tone map's exposure, and the power the display raises
    /// the exposed value to -- its gamma and the sRGB encoding, `shown`
    /// inverted.
    pub display: [f32; 2],
    /// An HDRI sky ([`environment_settings`]): 1 when there is one, its
    /// turn about the up axis in turns, the accumulator's units a map
    /// unit, and 1 when the camera sees it.
    pub env: [f32; 4],
}

/// An HDRI sky as a pass reads it: which, how far turned, how bright.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvSettings {
    pub def: &'static crate::escape::environment::EnvDef,
    /// Its turn about the up axis, in turns.
    pub rotation: f32,
    /// The accumulator's units a map unit: Sky light, times what shows the
    /// sky's median at its level (`EnvDef::level`) through the Linear tone
    /// map's exposure and gamma and the sRGB encoding -- a picked colour's
    /// `shown`, kept linear so the sun and the sky keep their ratio.
    pub scale: f32,
    pub backdrop: bool,
}

/// The HDRI sky a path-tracing block names, when this build has it.
pub fn environment_settings(t: &crate::config::escape::PathTraceConfig, config: &FractalConfig) -> Option<EnvSettings> {
    if t.environment_map.is_empty() {
        return None;
    }
    let def = crate::escape::environment::get(&t.environment_map)?;
    let gamma = if config.gamma > 0.0 { config.gamma } else { 1.0 };
    let shown = def.level.clamp(0.0, 1.0).powf(2.2 * gamma) / config.exposure.max(1.0e-6);
    let rotation = if t.environment_rotation.is_finite() { t.environment_rotation / 360.0 } else { 0.0 };
    Some(EnvSettings { def, rotation, scale: t.environment.max(0.0) * shown / def.median.max(1.0e-12), backdrop: t.environment_backdrop })
}

impl PathSettings {
    /// What starts a sum over when it changes: everything but the noise
    /// threshold's value -- a pixel's samples do not depend on it, and the
    /// next check flags the pixels by the new one -- though turning it on
    /// or off does (the statistics start with the sum).
    pub fn restart_key(&self) -> String {
        let on = if self.noise_threshold > 0.0 { 1.0 } else { 0.0 };
        format!("{:?}", PathSettings { noise_threshold: on, ..*self })
    }
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
            metallic: 0.0,
            emission: 0.0,
            aperture: 0.0,
            focus: 1.0,
            lake_roughness: 0.05,
            denoise: false,
            sky: None,
            studio: 0.0,
            mis: true,
            noise_threshold: 0.0,
            display: [1.0, 1.0 / 2.2],
            env: [0.0; 4],
        }
    }
}

/// The settings from the config (`escape.path`, plan section 4), for a
/// geometry whose target is `target` world units from the eye:
/// - the environment is the background colour brought into the
///   accumulator's units (`shown`) -- so the sky and an albedo-1 surface
///   it lights read as the background does, times Sky light -- plus the
///   rig's ambient ([`path_ambient`]);
/// - a sample's radiance is clamped at ten times the brightest light
///   (the environment and the glow count as lights);
/// - the lens is in the target's terms, so it keeps its look as the
///   camera dollies: the aperture a fraction of the distance, the focus
///   a multiple of it (0, the target itself).
pub fn path_settings(config: &FractalConfig, target: f32) -> PathSettings {
    path_settings_from(&config.escape.path, config, target, config.escape.terrain.lake_roughness)
}

/// [`path_settings`] from any path-tracing block -- a simulation's
/// terrain keeps its own -- with the config's sky, lights and material.
pub fn path_settings_from(
    t: &crate::config::escape::PathTraceConfig,
    config: &FractalConfig,
    target: f32,
    lake_roughness: f32,
) -> PathSettings {
    let ambient = path_ambient(&config.solid_shading);
    // An HDRI sky, or the studio's light, stands in for the sky's: the
    // environment is the ambient alone, and the map (`pt_env`) or the
    // studio (`ifs_studio`) is added in the shader. The map over the
    // studio.
    let env_map = environment_settings(t, config);
    let studio = if env_map.is_some() { 0.0 } else { studio_level(t, config) };
    let (env, sky) = if studio > 0.0 || env_map.is_some() {
        ([ambient; 3], None)
    } else {
        (
            shown(config, config.background_color).map(|c| t.environment * c + ambient),
            t.sky_gradient.then(|| shown(config, t.zenith).map(|c| t.environment * c + ambient)),
        )
    };
    let lights: f32 = config.solid_shading.lights.iter().filter(|l| l.enabled).map(|l| l.intensity.max(0.0)).sum();
    let brightest = lights
        .max(1.0)
        .max(env.iter().cloned().fold(0.0, f32::max))
        .max(sky.map_or(0.0, |z| z.iter().cloned().fold(0.0, f32::max)))
        .max(studio * STUDIO_PEAK);
    let target = target.max(1.0e-30);
    let material = lit_material(config);
    let gamma = if config.gamma > 0.0 { config.gamma } else { 1.0 };
    PathSettings {
        bounces: t.bounces.min(16),
        environment: env,
        clamp: 10.0 * brightest.max(material[2]),
        seed: 1,
        gloss: material[0],
        roughness: material[1],
        metallic: material[3],
        emission: material[2],
        aperture: t.aperture * target,
        focus: if t.focus > 0.0 { t.focus } else { 1.0 } * target,
        lake_roughness,
        denoise: t.denoise,
        sky,
        studio,
        mis: true,
        noise_threshold: if t.noise_threshold.is_finite() { t.noise_threshold.max(0.0) } else { 0.0 },
        display: [config.exposure.max(1.0e-6), 1.0 / (2.2 * gamma)],
        env: env_map.map_or([0.0; 4], |e| [1.0, e.rotation, e.scale, if e.backdrop { 1.0 } else { 0.0 }]),
    }
}

/// The studio's brightness in the accumulator's units: Sky light, so that
/// a radiance of 1 reads white at any exposure, as the background's does
/// (`shown` of white). 0 when the studio is off.
pub fn studio_level(t: &crate::config::escape::PathTraceConfig, config: &FractalConfig) -> f32 {
    // An HDRI sky lights in its place.
    if t.studio && environment_settings(t, config).is_none() {
        t.environment.max(0.0) / config.exposure.max(1.0e-6)
    } else {
        0.0
    }
}

/// The studio's brightest radiance (its softbox's), for the firefly
/// clamp: a glint of it is no firefly.
const STUDIO_PEAK: f32 = 2.0;

/// The rig's ambient as the path tracer sees it (lights-in-the-fractal,
/// L1): uniform light from every direction, beside the sky, that a
/// bounce ray which gets away brings back. An open Lambert surface it
/// lights reads `albedo x ambient`, the lit tier's own term, and the
/// geometry occludes it where the lit tier multiplies by its occlusion
/// estimate. Before, the path tracer read no ambient at all, and the
/// slider did nothing once a view was path traced.
///
/// None from an untouched rig: its default ambient is the lit tier's
/// stand-in for the sky the path tracer traces, and every picture path
/// traced under the default rig stays as it was.
pub fn path_ambient(shading: &crate::config::SolidShadingSettings) -> f32 {
    if shading.rig_untouched() {
        0.0
    } else {
        shading.ambient.max(0.0)
    }
}

/// A colour picked in sRGB as the accumulator holds what the tonemap
/// shows as that colour: the inverse of the whole chain the background
/// goes through -- the Linear tonemap's exposure and gamma, and the
/// sRGB decode (2.2) the tonemap composites the background with. Light
/// and sight alike: an albedo-1 surface the sky alone lights at Sky
/// light 1 reads exactly as the sky behind it.
///
/// Until 2026-10-08 the light left the decode out (T3's convention, its
/// claim of a match measured only in the accumulator): that surface read
/// as the background raised to 1/2.2, brighter -- the sky lit a scene
/// `bg^(1.2 gamma)` times too strongly, about twice at gamma 1 and two to
/// five times at the escape engine's 2.2.
fn shown(config: &FractalConfig, c: [f32; 3]) -> [f32; 3] {
    let gamma = if config.gamma > 0.0 { config.gamma } else { 1.0 };
    let exposure = config.exposure.max(1.0e-6);
    c.map(|v| v.max(0.0).powf(2.2).powf(gamma) / exposure)
}

/// A picture's material (docs/archive/projects/materials.md): the coat -- its
/// reflectance at normal incidence and its roughness -- the albedo's glow,
/// how much of it is metal (P4), and how much of its colour it reflects
/// (the reflectance), beside the lights, which every tier reads. The rig's
/// shaders read the first three in the w of the camera's rows, the
/// metallic in the fog row's (`ifs_metallic`) and the reflectance's
/// complement in the studio row's z (`ifs_reflectance`).
pub fn lit_material(config: &FractalConfig) -> [f32; 5] {
    let s = &config.solid_shading;
    let reflectance = if s.reflectance.is_finite() { s.reflectance.clamp(0.0, 1.0) } else { 1.0 };
    [s.gloss.clamp(0.0, 1.0), s.roughness.clamp(0.02, 1.0), s.glow.max(0.0), s.metallic.clamp(0.0, 1.0), reflectance]
}

// The coat's albedo fit, and the energy it gives back, are
// `crate::renderer::coat`'s: every engine's tiers read them, the flame's
// shade pass with no escape engine built.

/// A gradient sky's zenith as a camera ray sees it, in the
/// accumulator's units (the rig's `ifs_sky_seen`): None for a flat sky,
/// the tonemap's background. Its light is the same times Sky light.
pub fn sky_seen(t: &crate::config::escape::PathTraceConfig, config: &FractalConfig) -> Option<[f32; 3]> {
    t.sky_gradient.then(|| shown(config, t.zenith))
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
    sky: [f32; 4],
    list: [u32; 4],
    envmap: [f32; 4],
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
            lens: [settings.aperture.max(0.0), settings.focus.max(1.0e-6), if settings.mis { 1.0 } else { 0.0 }, 0.0],
            band: [band.0, band.1, u32::from(settings.denoise) | u32::from(settings.noise_threshold > 0.0) << 1, 0],
            sky: settings.sky.map_or([0.0; 4], |z| [z[0], z[1], z[2], 1.0]),
            list: [0; 4],
            envmap: settings.env,
        }
    }

    /// Sampling adaptively once a check has listed the pixels still
    /// sampling (V2): the dispatch takes `entries` of the list from its
    /// `first`, a thread an entry, in place of a band's rect. Its
    /// workgroups go across in a row of at most [`LIST_ROW`].
    pub(crate) fn from_list(mut self, first: u32, entries: u32) -> Self {
        self.list = [first, entries, 1, 0];
        self
    }

    /// The dispatch starts at column `x0` (a tile of a band): its
    /// threads are that column on.
    pub(crate) fn from_column(mut self, x0: u32) -> Self {
        self.band[3] = x0;
        self
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

/// The denoiser's guides' bytes a pixel: three vec4s (`PT_CORE_WGSL`'s
/// `pt_guide`).
pub(crate) const GUIDE_BYTES: u64 = 48;

/// Workgroups in a row of a list's dispatch: `main`'s `PT_LIST_ROW`.
pub(crate) const LIST_ROW: u32 = 1024;

/// A list dispatch's workgroups for `entries` of it, 64 a workgroup, in
/// rows of [`LIST_ROW`].
pub(crate) fn list_groups(entries: u32) -> (u32, u32) {
    let groups = entries.div_ceil(64).max(1);
    (groups.min(LIST_ROW), groups.div_ceil(LIST_ROW))
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
// The denoiser's guides, three a pixel, summed with the samples weighted
// by their coverage: (albedo, the diffuse share's luminance squared),
// (normal, distance) and (the specular share, its luminance squared;
// `pt_spec`). Written only when `pt.band.z`'s first bit asks.
//
// With adaptive sampling (V2; `pt.band.z`'s second bit) the sum holds a
// second vec4 a pixel after the frame's sums: its samples' luminance
// squared, its odd samples' luminance, its count -- negative once it has
// converged -- and its noise at the last check (`PATH_ADAPT_WGSL`); and
// after those the last check's list of the pixels still sampling: their
// number, then their indices, four a vec4.
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
    // depth; z: 1 for multiple importance sampling of the coat and the
    // lights (`pt_path`).
    lens: vec4<f32>,
    // x: the dispatch's first row, y: its rows (a band, for a geometry
    // whose sample would outlast the GPU's watchdog over a whole frame),
    // z: bit 1 to gather the denoiser's guides, bit 2 adaptive sampling,
    // w: its first column (a band too costly for one dispatch goes across
    // in tiles).
    band: vec4<u32>,
    // A gradient sky's zenith radiance, and 1 when there is one: the
    // environment (`env`) is then the horizon's.
    sky: vec4<f32>,
    // Adaptive sampling's list (V2): z 1 when the dispatch traces the
    // pixels the last check listed as still sampling, x entries from its
    // first, y of them -- a thread an entry, 64 a workgroup, the
    // workgroups in rows of `PT_LIST_ROW` -- in place of `band`'s rect.
    list: vec4<u32>,
    // An HDRI sky (`pt_env`): x 1 when there is one, y its turn about the
    // up axis in turns, z the accumulator's units a map unit, w 1 when
    // the camera sees it.
    envmap: vec4<f32>,
};

// The HDRI sky's map (docs/projects/hdri-environment.md): equirectangular,
// its sun lifted out into the rig's lights (`escape::environment`); a 1x1
// stand-in when there is none.
@group(0) @binding(14) var pt_env_tex: texture_2d<f32>;
@group(0) @binding(15) var pt_env_smp: sampler;

// atan2, exact at a zero pair as IEEE has it (Metal's is not: CLAUDE.md),
// for the map's azimuth -- straight up and down are a zero pair.
fn pt_atan2(y: f32, x: f32) -> f32 {
    if (y == 0.0 && x == 0.0) {
        let pi = 3.14159265358979;
        let mag = select(0.0, pi, (bitcast<u32>(x) & 0x80000000u) != 0u);
        return select(mag, -mag, (bitcast<u32>(y) & 0x80000000u) != 0u);
    }
    return atan2(y, x);
}

// A world direction in the HDRI sky's frame (x right, y up, -z ahead): the
// world's, z up, whatever the lights follow -- a sky read in the studio's
// frame (a camera's, at the solids' usual view) stood its horizon at the
// camera's pitch, level only from that view's yaw.
fn pt_env_dir(d: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(d.x, d.z, -d.y);
}

// The HDRI sky's radiance toward `s`, a direction in the sky's frame
// (`pt_env_dir`): `environment::map_direction` inverted, turned. Sampled
// at its finest level, so from any control flow.
fn pt_env(s: vec3<f32>) -> vec3<f32> {
    let u = pt_atan2(s.x, -s.z) / 6.283185307 + 0.5 - pt.envmap.y;
    let v = acos(clamp(s.y, -1.0, 1.0)) / 3.141592654;
    return textureSampleLevel(pt_env_tex, pt_env_smp, vec2<f32>(u, v), 0.0).rgb * pt.envmap.z;
}

// The environment's radiance from direction d: uniform, or the gradient
// sky's -- the horizon's below and at the horizon, the zenith's
// overhead, by the rig's own profile (`ifs_sky_t`).
fn pt_sky(d: vec3<f32>) -> vec3<f32> {
    // An HDRI sky: the ambient in `env`, and the map.
    if (pt.envmap.x > 0.5) {
        return pt.env.rgb + pt_env(pt_env_dir(d));
    }
    // The studio (P4), in place of the sky's light: the ambient in
    // `env`, and the studio's radiance unblurred.
    let studio = ifs_studio_level();
    if (studio > 0.0) {
        return pt.env.rgb + vec3<f32>(studio * ifs_studio(ifs_studio_frame(d)));
    }
    if (pt.sky.w > 0.5) {
        return mix(pt.env.rgb, pt.sky.rgb, ifs_sky_t(d));
    }
    return pt.env.rgb;
}

// What a camera ray that meets nothing adds to the sum, (radiance times
// coverage, coverage): an HDRI sky the camera sees, whole; the
// gradient's zenith over the tonemap's background, at its share; nothing
// for a flat sky.
fn pt_sky_seen(d: vec3<f32>) -> vec4<f32> {
    if (pt.envmap.w > 0.5) {
        return vec4<f32>(pt_env(pt_env_dir(d)), 1.0);
    }
    let s = ifs_sky_seen(d);
    return vec4<f32>(s.rgb * s.a, s.a);
}

// The roughness a coat keeps at least past a diffuse bounce (D3).
const PT_REGULAR_ROUGH: f32 = 0.4;

// A sample's first surface, as its geometry reports it (`pt_first`):
// what the denoiser's guides are made of.
var<private> pt_g_albedo: vec3<f32>;
var<private> pt_g_normal: vec3<f32>;
var<private> pt_g_t: f32;

// The share of a sample's radiance its first surface reflects
// specularly (path-tracing-research D5): its coat's and a metal's share
// of the direct light, and all a bounce brings back when the first
// bounce took the coat's lobe. The rest is diffuse. The
// denoiser filters the two apart: a reflection follows the view and the
// roughness, where the diffuse light follows the surface.
var<private> pt_spec: vec3<f32>;

// The firefly clamp on a path's radiance, its specular share scaled
// alike.
fn pt_clamp(l: vec3<f32>) -> vec3<f32> {
    let c = min(l, vec3<f32>(pt.misc.y));
    pt_spec = pt_spec * select(vec3<f32>(1.0, 1.0, 1.0), c / max(l, vec3<f32>(1.0e-30, 1.0e-30, 1.0e-30)), l > c);
    return c;
}

fn pt_first(h: PtHit, d: vec3<f32>) {
    // The surface's whole reflectance seen along d: the albedo under the
    // coat, and the coat's own share, which reflects whatever the albedo.
    // Divided by the albedo alone, a black glossy surface's reflection
    // was divided by nothing and multiplied back by nothing: flat black,
    // and on a near-black the hue of whichever channel was not zero. No
    // coat, the albedo exactly.
    // A metal's (P4) reflects in its own colour, and nothing is under it.
    let c = max(dot(h.n, -d), 0.0);
    var f = 0.0;
    var fc = vec3<f32>(0.0, 0.0, 0.0);
    if (h.f0 > 0.0) {
        f = pt_fresnel(h.f0, c);
        fc = vec3<f32>(f, f, f);
    }
    let albedo = h.albedo.rgb * ifs_reflectance();
    if (h.metal > 0.0) {
        fc = ifs_coat_fresnel3(mix(vec3<f32>(h.f0, h.f0, h.f0), albedo, h.metal), c);
    }
    pt_g_albedo = albedo * ((1.0 - h.metal) * (1.0 - f)) + fc;
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

// Multiple importance sampling of the coat and the lights (Veach and
// Guibas 1995; 3d-look-and-tools P4). A polished coat's peak is narrower
// than a light's cone: the lights' samples rarely land in it -- speckle --
// and the coat's bounces that do meet a light saw only the sky. Now a
// bounce by the coat's lobe that escapes inside a directional light's
// cone adds the light, and both estimates of the coat's share of it are
// weighted by the power heuristic. The diffuse stays the lights' alone
// (nothing to gain: its density never nears a light's), and a point or a
// headlamp has no area to meet. Off at shadow strength 0, where the
// lights reach through the solid and a bounce cannot see them do so.
//
// A light's direction's density, sampled uniformly over its cone.
fn pt_light_pdf() -> f32 {
    return 1.0 / (6.283185307 * max(1.0 - pt.misc.w, 1.0e-7));
}

// The power heuristic's weight of an estimate of density `a` against one
// of density `b`.
fn pt_power_weight(a: f32, b: f32) -> f32 {
    let a2 = a * a;
    return a2 / max(a2 + b * b, 1.0e-30);
}

// The density of the coat's bounce toward `l` -- its lobe chosen at
// `p_spec`, then the visible normals' (Heitz 2018: G1(v) D(h) / 4 n.v).
fn pt_coat_pdf(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, a2: f32, p_spec: f32) -> f32 {
    let nv = max(dot(n, v), 1.0e-4);
    let h = normalize(v + l);
    return p_spec * pt_g1(nv, a2) * pt_ggx_d(max(dot(n, h), 0.0), a2) / (4.0 * nv);
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
    // -- the config's, or a material's own (a terrain's lake) -- and how
    // much of it is metal (P4; the rig's `ifs_metallic`, a lake's none).
    f0: f32,
    rough: f32,
    metal: f32,
    // The TRUE normal, on the same side, where a surface texture tilts the
    // shading normal `n` (surface-textures.md): the next ray leaves along
    // it, and no light or bounce passes under it. Zero where there is no
    // tilt: `n` is the true normal.
    ng: vec3<f32>,
};

// A surface's shading where a path meets it: what each light and the
// bounce read (`pt_path`, and mode D's fused march). `p` is where the ray
// along `d` met it; `regular`, whether the path has bounced diffusely.
struct PtShade {
    p: vec3<f32>,
    n: vec3<f32>,
    // The true surface, where a texture tilted the shading normal.
    ng: vec3<f32>,
    bumped: bool,
    // What it reflects of its colour (`ifs_reflectance`).
    albedo: vec3<f32>,
    // Where a ray leaving it starts.
    start: vec3<f32>,
    // The coat: Lambert under a gloss of reflectance f0 at normal
    // incidence, the diffuse taking what the coat's Fresnel does not; a
    // metal's (P4) reflectance the albedo's colour, with nothing under
    // it; and either given back what single scattering loses
    // (`ifs_coat_energy`).
    f0: f32,
    metal: f32,
    coated: bool,
    f0c: vec3<f32>,
    a2: f32,
    v: vec3<f32>,
    nv: f32,
    energy: vec3<f32>,
    // The bounce's lobe choice: the coat's at its Fresnel at this view.
    p_spec: f32,
    // Whether the coat's direct light is weighted against its bounces.
    mis: bool,
    shadow_strength: f32,
};

fn pt_shade(h: PtHit, p: vec3<f32>, d: vec3<f32>, regular: bool, bounce: u32) -> PtShade {
    var s: PtShade;
    s.p = p;
    s.n = h.n;
    s.bumped = dot(h.ng, h.ng) > 0.5;
    s.ng = select(s.n, h.ng, s.bumped);
    s.albedo = h.albedo.rgb * ifs_reflectance();
    s.start = p + s.ng * h.bias;
    s.f0 = h.f0;
    s.metal = h.metal;
    s.coated = s.f0 > 0.0 || s.metal > 0.0;
    s.f0c = mix(vec3<f32>(s.f0, s.f0, s.f0), s.albedo, s.metal);
    // Past a diffuse bounce, a coat is no sharper than `PT_REGULAR_ROUGH`
    // (path regularisation, path-tracing-research D3; Kaplanyan and
    // Dachsbacher 2013, Cycles' Filter Glossy): light that found a glossy
    // surface after a diffuse one is the rare bright path a few samples
    // keep as a firefly.
    let rough = select(h.rough, max(h.rough, PT_REGULAR_ROUGH), regular);
    s.a2 = max(rough * rough, 1.0e-4) * max(rough * rough, 1.0e-4);
    s.v = -d;
    s.nv = max(dot(s.n, s.v), 1.0e-4);
    s.energy = ifs_coat_energy(s.f0c, s.nv, rough);
    // Without a coat, no draw to choose a lobe: Lambert's samples are the
    // same stream they were before the coat existed. A metal leans the
    // choice to the coat, wholly at 1, where nothing is under it.
    s.p_spec = select(0.0, mix(clamp(select(0.0, pt_fresnel(s.f0, s.nv), s.f0 > 0.0), 0.1, 0.9), 1.0, s.metal), s.coated);
    // Only when there is a bounce from here to meet a light.
    s.mis = pt.lens.z > 0.5 && s.p_spec > 0.0 && pt.misc.z > 0.0 && bounce < pt.bounces;
    s.shadow_strength = clamp(pt.misc.z, 0.0, 1.0);
    return s;
}

// One light at a surface (`pt_light`): its direction, sampled over its
// angular size, and its reach; whether it lights the surface; and its
// terms, which the shadow ray's verdict weighs (`pt_light_add`). And, at
// the first surface, its shares of the denoiser's guide.
struct PtLight {
    lit: bool,
    dir: vec3<f32>,
    dist: f32,
    color: vec3<f32>,
    // Its power times its fall-off, and the cosine.
    pg: f32,
    ndl: f32,
    // The diffuse and the coat: albedo (diffuse) + the coat's BRDF times
    // pi, the rig's units.
    brdf: vec3<f32>,
    spec: vec3<f32>,
    // The coat's share's weight against its bounces (multiple importance
    // sampling); below 0, none.
    w: f32,
    // The highlight; below 0, none.
    shine: f32,
    // At the first surface, the direct light the albedo shapes and the
    // light it does not (the guide's white share).
    guide: vec2<f32>,
};

// The specular share of what `pt_light_add` adds for a lit light: the
// coat's term, weighted against its bounces where it is. Not the
// highlight: the lit tier's shine on the direct light, no lobe a bounce
// samples -- split from the diffuse it made the matte Mandelbulb's
// denoised picture worse (coarse error 5.3% to 7.7% at 4 samples), and
// with the diffuse it changes nothing.
fn pt_light_spec(through: vec3<f32>, l: PtLight, seen: f32, shadow_strength: f32) -> vec3<f32> {
    var vis = 1.0;
    if (pt.misc.z > 0.0) {
        vis = mix(1.0, seen, shadow_strength);
    }
    var r = through * l.color * l.spec * (l.pg * l.ndl * vis);
    if (l.w >= 0.0) {
        let spec_vis = l.w * seen + (1.0 - shadow_strength) * (1.0 - seen);
        r = r + through * l.color * l.spec * (l.pg * l.ndl * (spec_vis - vis));
    }
    return r;
}

// Every light, each sampled over its angular size. Picking one a surface
// (by power) would save shadow rays, but a face only one light reaches
// then reads all of it or none, sample to sample: measured as speckle over
// a two-light solid's lit faces. The diffuse in the rig's units (its I as
// pi times a radiance's), the gloss's physical BRDF times the same
// irradiance, pi I cos.
fn pt_light(s: PtShade, li: u32, bounce: u32, rough: f32) -> PtLight {
    var out: PtLight;
    // A point or a headlamp from where this surface is, its power fallen
    // off with the distance (the rig's `ifs_light_at`).
    let la = ifs_light_at(li, s.p);
    let ld = pt_cone(la.dir, pt.misc.w);
    let ndl = dot(s.n, ld);
    out.dir = ld;
    out.dist = la.dist;
    out.lit = ndl > 0.0 && (!s.bumped || dot(s.ng, ld) > 0.0);
    out.w = -1.0;
    out.shine = -1.0;
    out.guide = vec2<f32>(0.0, 0.0);
    if (out.lit) {
        var spec = vec3<f32>(0.0, 0.0, 0.0);
        var diff = 1.0;
        if (s.coated) {
            let hv = normalize(ld + s.v);
            let vh = max(dot(s.v, hv), 0.0);
            let dg = 3.141592654 * pt_ggx_d(max(dot(s.n, hv), 0.0), s.a2) * pt_g1(ndl, s.a2) * pt_g1(s.nv, s.a2) / (4.0 * ndl * s.nv);
            spec = ifs_coat_fresnel3(s.f0c, vh) * s.energy * dg;
            diff = (1.0 - s.metal) * select(1.0, 1.0 - pt_fresnel(s.f0, vh), s.f0 > 0.0);
        }
        out.color = ifs_light_color(li);
        out.pg = ifs_light_power(li) * la.gain;
        out.ndl = ndl;
        out.brdf = s.albedo * (ifs_diffuse() * diff) + spec;
        out.spec = spec;
        if (s.mis && ifs_light_kind(li) == 0u) {
            out.w = pt_power_weight(pt_light_pdf(), pt_coat_pdf(s.n, s.v, ld, s.a2, s.p_spec));
        }
        // The highlight, the lit tier's very term (materials.md §4.1):
        // Blinn-Phong on the direct light, unnormalised and without the
        // cosine, so both tiers draw it alike. Not a lobe a bounce
        // samples: it is the lights' shine alone. Weighted by the shading
        // strength, as the lit tier's mix weights it: a rig at strength 0
        // draws no highlight in either tier (a terrain's touched rig
        // carried the flame default 0.35, and glinted from P7 on where it
        // never had).
        let shine_weight = ifs_specular() * clamp(ifs_shading_strength(), 0.0, 1.0);
        if (shine_weight > 0.0) {
            let hb = normalize(ld + s.v);
            out.shine = shine_weight * pow(max(dot(s.n, hb), 0.0), max(ifs_shininess(), 1.0));
        }
    }
    // The guide's shares (path-tracing-research D1): from the light's
    // centre, its lobe widened by the cone's size, and with no visibility
    // -- the same every sample, so the guide holds no noise of its own.
    // (From the sampled direction and the shadow ray's verdict it
    // flickered sample to sample, and the filter, dividing the light by it
    // and multiplying back, put the speckle back on glossy and highlit
    // faces.) A metal's reflection is the albedo's colour: shaped by it,
    // as the diffuse is, where a coat's is white.
    if (bounce == 0u) {
        let lc = la.dir;
        let ndc = dot(s.n, lc);
        if (ndc > 0.0) {
            let lumw = vec3<f32>(0.2126, 0.7152, 0.0722);
            var spec_c = 0.0;
            var diff_c = 1.0;
            if (s.coated) {
                let alpha_c = min(rough * rough + 0.5 * sqrt(max(1.0 - pt.misc.w * pt.misc.w, 0.0)), 1.0);
                let a2c = max(alpha_c * alpha_c, 1.0e-4);
                let hc = normalize(lc + s.v);
                let vhc = max(dot(s.v, hc), 0.0);
                let dgc = 3.141592654 * pt_ggx_d(max(dot(s.n, hc), 0.0), a2c) * pt_g1(ndc, a2c) * pt_g1(s.nv, a2c) / (4.0 * ndc * s.nv);
                spec_c = dot(ifs_coat_fresnel3(s.f0c, vhc) * s.energy, lumw) * dgc;
                diff_c = (1.0 - s.metal) * select(1.0, 1.0 - pt_fresnel(s.f0, vhc), s.f0 > 0.0);
            }
            var shine_c = 0.0;
            let shine_w = ifs_specular() * clamp(ifs_shading_strength(), 0.0, 1.0);
            if (shine_w > 0.0) {
                shine_c = shine_w * pow(max(dot(s.n, normalize(lc + s.v)), 0.0), max(ifs_shininess(), 1.0));
            }
            let lum = dot(ifs_light_color(li), lumw) * ifs_light_power(li) * la.gain;
            out.guide = vec2<f32>(lum * ndc * (ifs_diffuse() * diff_c + spec_c * s.metal), lum * (ndc * spec_c * (1.0 - s.metal) + shine_c));
        }
    }
    return out;
}

// A lit light's light added to `radiance`, `seen` its shadow ray's verdict
// (1 when no ray was traced: shadow strength 0).
fn pt_light_add(radiance: vec3<f32>, through: vec3<f32>, l: PtLight, seen: f32, shadow_strength: f32) -> vec3<f32> {
    var vis = 1.0;
    if (pt.misc.z > 0.0) {
        vis = mix(1.0, seen, shadow_strength);
    }
    let light = l.color * (l.pg * l.ndl * vis);
    var r = radiance + through * light * l.brdf;
    // The coat's share, weighted against its bounces where a bounce can
    // meet this light: the part of it that reaches (`seen`) by the power
    // heuristic, the part a partial shadow strength lets through the solid
    // -- which no bounce sees -- whole. Their sum is `vis`; what it takes
    // from the coat's share added above.
    if (l.w >= 0.0) {
        let spec_vis = l.w * seen + (1.0 - shadow_strength) * (1.0 - seen);
        r = r + through * l.color * l.spec * (l.pg * l.ndl * (spec_vis - vis));
    }
    if (l.shine >= 0.0) {
        r = r + through * l.color * (l.pg * vis * l.shine);
    }
    return r;
}

// A bounce's choice at a surface (`pt_bounce`): its direction, the
// throughput it carries on, the coat's density toward it (for the lights
// it may meet), and whether the path goes on at all.
struct PtBounce {
    go: bool,
    dir: vec3<f32>,
    through: vec3<f32>,
    coat_pdf: f32,
    regular: bool,
    // The coat's lobe, not Lambert's.
    spec: bool,
};

// The gloss's lobe by the visible normals, chosen by the coat's Fresnel at
// this view (`p_spec`); else Lambert's, whose cosine sampling leaves the
// albedo times what the coat lets through. Russian roulette from the
// second bounce.
fn pt_bounce(s: PtShade, through: vec3<f32>, regular: bool, bounce: u32) -> PtBounce {
    var out: PtBounce;
    out.go = false;
    out.through = through;
    out.regular = regular;
    out.coat_pdf = 0.0;
    out.spec = false;
    var nd = vec3<f32>(0.0, 0.0, 1.0);
    if (s.p_spec > 0.0 && pt_rand() < s.p_spec) {
        out.spec = true;
        let frame = pt_frame(s.n);
        let vl = transpose(frame) * s.v;
        let hl = pt_vndf(vl, sqrt(s.a2));
        let hv = frame * hl;
        nd = reflect(-s.v, hv);
        let nl = dot(s.n, nd);
        if (nl <= 0.0) {
            return out;
        }
        out.through = through * ifs_coat_fresnel3(s.f0c, max(dot(s.v, hv), 0.0)) * s.energy * (pt_g1(nl, s.a2) / s.p_spec);
        if (s.mis) {
            out.coat_pdf = pt_coat_pdf(s.n, s.v, nd, s.a2, s.p_spec);
        }
    } else {
        nd = pt_cosine(s.n);
        out.regular = true;
        // No coat at all at reflectance 0: Schlick's term alone would
        // still take (1 - cos)^5 of a glance.
        let coat = select(0.0, pt_fresnel(s.f0, s.nv), s.f0 > 0.0);
        out.through = through * s.albedo * ((1.0 - s.metal) * (1.0 - coat) / (1.0 - s.p_spec));
    }
    // A tilted normal can send a bounce under the true surface: it has
    // nowhere to go.
    if (s.bumped && dot(s.ng, nd) <= 0.0) {
        return out;
    }
    if (bounce >= 1u) {
        let q = clamp(max(out.through.r, max(out.through.g, out.through.b)), 0.05, 0.95);
        if (pt_rand() > q) {
            return out;
        }
        out.through = out.through / q;
    }
    out.dir = nd;
    out.go = true;
    return out;
}

// A bounce along `d` that met nothing: the environment, and the lights it
// meets -- a directional light's radiance over its cone (its irradiance pi
// I spread over the cone's solid angle), weighted against the lights' own
// samples of it.
fn pt_escape(radiance: vec3<f32>, through: vec3<f32>, d: vec3<f32>, coat_pdf: f32) -> vec3<f32> {
    var r = radiance + through * pt_sky(d);
    if (coat_pdf > 0.0) {
        let pl = pt_light_pdf();
        let w = pt_power_weight(coat_pdf, pl);
        for (var li = 0u; li < ifs_light_count(); li = li + 1u) {
            if (ifs_light_kind(li) == 0u && dot(d, ifs_light_dir(li)) >= pt.misc.w) {
                r = r + through * ifs_light_color(li) * (ifs_light_power(li) * 3.141592654 * pl * w);
            }
        }
    }
    return r;
}

// The denoiser divides a pixel's light by its guide to keep the albedo's
// detail out of the blur, and multiplies it back after. Light the albedo
// does not shape -- a highlight, a coat's glint -- divided by a saturated
// albedo's near-zero channels came back as bands of the wrong colour
// (field-reported: a terrain's sun glint, banded red and yellow in its
// valleys). So the guide takes that light's share as white, as it takes
// the coat's reflection of the sky (`pt_first`): exact where there is
// none. `guide`: the first surface's lights' shares, summed.
fn pt_guide_white(guide: vec2<f32>) {
    if (guide.y > 0.0) {
        let w = guide.y / max(guide.x + guide.y, 1.0e-20);
        pt_g_albedo = pt_g_albedo * (1.0 - w) + vec3<f32>(w, w, w);
    }
}

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
    var regular = false;
    // The specular share (`pt_spec`): what the first bounce brings back
    // counts whole when it took the coat's lobe, from the radiance there.
    pt_spec = vec3<f32>(0.0, 0.0, 0.0);
    var spec_from = vec3<f32>(0.0, 0.0, 0.0);
    var spec_path = false;
    for (var bounce = 0u; bounce <= pt.bounces; bounce = bounce + 1u) {
        let p = o + d * h.t;
        travelled = travelled + h.t;
        if (!(h.albedo.a > 0.0)) {
            // A hole's floor: nothing there; past the first surface, the
            // sky through it.
            if (bounce > 0u) {
                radiance = radiance + through * pt_sky(d);
            }
            break;
        }
        let s = pt_shade(h, p, d, regular, bounce);
        // Its glow: the colour's own.
        radiance = radiance + through * h.albedo.rgb * pt.mat.z;
        var guide = vec2<f32>(0.0, 0.0);
        for (var li = 0u; li < ifs_light_count(); li = li + 1u) {
            let l = pt_light(s, li, bounce, h.rough);
            if (l.lit) {
                // No shadow ray at strength 0.
                var seen = 1.0;
                if (pt.misc.z > 0.0) {
                    seen = select(0.0, 1.0, pt_scene_visible(s.start, l.dir, l.dist, travelled));
                }
                radiance = pt_light_add(radiance, through, l, seen, s.shadow_strength);
                if (bounce == 0u) {
                    pt_spec = pt_spec + pt_light_spec(through, l, seen, s.shadow_strength);
                }
            }
            guide = guide + l.guide;
        }
        if (bounce == 0u) {
            pt_guide_white(guide);
        }
        if (bounce >= pt.bounces) {
            break;
        }
        let b = pt_bounce(s, through, regular, bounce);
        if (!b.go) {
            break;
        }
        if (bounce == 0u && b.spec) {
            spec_path = true;
            spec_from = radiance;
        }
        through = b.through;
        regular = b.regular;
        o = s.start;
        d = b.dir;
        h = pt_scene_next(o, d, travelled);
        if (!h.hit) {
            radiance = pt_escape(radiance, through, d, b.coat_pdf);
            break;
        }
    }
    if (spec_path) {
        pt_spec = pt_spec + (radiance - spec_from);
    }
    return radiance;
}

// A pixel's samples, in order, added to its running sum: the same bits
// however the samples are split into dispatches -- and, its random
// numbers keyed by its place in the whole frame, however the frame is
// split into tiles. Sampling adaptively, a pixel draws from its own
// count, so its samples are the same stream however many the frame's
// other pixels have taken, and one that has converged takes none.
fn pt_trace_pixel(x: u32, y: u32, tile: vec4<u32>) {
    let idx = y * tile.z + x;
    let fx = tile.x + x;
    let fy = tile.y + y;
    let adaptive = (pt.band.z & 2u) != 0u;
    let si = tile.z * tile.w + idx;
    var first = pt.sample_base;
    var st = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (adaptive && pt.sample_base != 0u) {
        st = pt_sum[si];
        if (st.z < 0.0) {
            return;
        }
        first = u32(st.z);
    }
    var sum = pt_sum[idx];
    if (pt.sample_base == 0u) {
        sum = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let guided = (pt.band.z & 1u) != 0u;
    var g0 = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var g1 = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var g2 = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (guided && pt.sample_base != 0u) {
        g0 = pt_guide[3u * idx];
        g1 = pt_guide[3u * idx + 1u];
        g2 = pt_guide[3u * idx + 2u];
    }
    let pixel = pt_hash((fy * params.width + fx) * 0x9E3779B9u + pt.seed);
    for (var s = 0u; s < pt.samples; s = s + 1u) {
        pt_seed = pixel;
        pt_index = first + s;
        pt_dim = 0u;
        // A sample that meets no surface -- the sky -- reports none: its
        // guides are an albedo of one, which divides nothing out, and a
        // distance of zero, which the denoiser reads as no surface, so a
        // pixel of sky passes through it as it is.
        pt_g_albedo = vec3<f32>(1.0, 1.0, 1.0);
        pt_g_normal = vec3<f32>(0.0, 0.0, 0.0);
        pt_g_t = 0.0;
        pt_spec = vec3<f32>(0.0, 0.0, 0.0);
        let v = pt_sample(fx, fy);
        sum = sum + v;
        if (guided && v.a > 0.0) {
            // Each share's luminance squared: the denoiser filters each
            // by its own noise (D5), and takes the whole's as their sum.
            let ls = dot(pt_spec, vec3<f32>(0.2126, 0.7152, 0.0722));
            let ld = dot(v.rgb / v.a, vec3<f32>(0.2126, 0.7152, 0.0722)) - ls;
            g0 = g0 + vec4<f32>(pt_g_albedo * v.a, ld * ld * v.a);
            g1 = g1 + vec4<f32>(pt_g_normal * v.a, pt_g_t * v.a);
            g2 = g2 + vec4<f32>(pt_spec * v.a, ls * ls * v.a);
        }
        if (adaptive) {
            // The sample as the pixel shows it, premultiplied.
            let l = dot(v.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
            st.x = st.x + l * l;
            st.y = st.y + select(0.0, l, (pt_index & 1u) != 0u);
        }
    }
    pt_sum[idx] = sum;
    if (adaptive) {
        pt_sum[si] = vec4<f32>(st.x, st.y, f32(first + pt.samples), st.w);
    }
    if (guided) {
        pt_guide[3u * idx] = g0;
        pt_guide[3u * idx + 1u] = g1;
        pt_guide[3u * idx + 2u] = g2;
    }
}

// Workgroups in a row of a list's dispatch (`PathParamsGpu::from_list`).
const PT_LIST_ROW: u32 = 1024u;

// Each invocation, its pixel's samples: the band's rect's, or -- sampling
// adaptively, once a check has listed the pixels still sampling -- a list
// entry's, so a warp's lanes are 32 pixels that all still sample, not a
// block of the frame where most have converged. (A workgroup sharing out
// its own block's would need a barrier, and one in this kernel made the
// march 3.4 times slower on a GTX 1660 SUPER.)
@compute @workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    let tile = pt_tile();
    var x = gid.x + pt.band.w;
    var y = gid.y + pt.band.x;
    if (pt.list.z != 0u) {
        let k = (wg.y * PT_LIST_ROW + wg.x) * 64u + li;
        let p = tile.z * tile.w;
        let listed = u32(pt_sum[2u * p].x);
        if (k >= pt.list.y || pt.list.x + k >= listed) {
            return;
        }
        let e = pt.list.x + k;
        let idx = u32(pt_sum[2u * p + 1u + e / 4u][e % 4u]);
        x = idx % tile.z;
        y = idx / tile.z;
    } else if (x >= tile.z || gid.y >= pt.band.y || y >= tile.w) {
        return;
    }
    pt_scene_begin();
    pt_trace_pixel(x, y, tile);
}
"#;

/// The path tracer's sum into an output: colour the premultiplied mean
/// over the coverage, alpha the mean coverage -- the accumulator
/// contract. Rows above `split` hold one sample more than `count` (a
/// banded pass part-way down the frame), and so do the columns left of
/// `split_col` of the band from it to `band_end` (a band part-way across:
/// [`PathSplit`]). Sampling adaptively, each pixel's count is its own.
pub(crate) const PATH_RESOLVE_WGSL: &str = r#"
struct ResolveParams {
    width: u32,
    height: u32,
    count: u32,
    split: u32,
    band_end: u32,
    split_col: u32,
    adaptive: u32,
    pad1: u32,
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
    let ahead = gid.y < rp.split || (gid.y < rp.band_end && gid.x < rp.split_col);
    var n = rp.count + select(0u, 1u, ahead);
    if (rp.adaptive != 0u) {
        n = u32(abs(pt_sum[rp.width * rp.height + gid.y * rp.width + gid.x].z));
    }
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
    // The band part-way across, as the resolve has it.
    band_end: u32,
    split_col: u32,
    // 1: each pixel's count is its own (adaptive sampling).
    adaptive: u32,
    // D5: 1 to filter the specular share apart from the diffuse; 1 for
    // the a-trous pass over the specular; the coat's roughness, by which
    // that pass reaches.
    split_spec: u32,
    spec_pass: u32,
    rough: f32,
};
@group(0) @binding(0) var<uniform> dp: DenoiseParams;
@group(0) @binding(1) var<storage, read> pt_sum: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> pt_guide: array<vec4<f32>>;
@group(0) @binding(3) var light_in: texture_2d<f32>;
@group(0) @binding(4) var feat_in: texture_2d<f32>;
@group(0) @binding(5) var light_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(6) var feat_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(7) var out_tex: texture_storage_2d<rgba32float, write>;
// D5: the specular light, as `light_out` and `light_in` hold the diffuse.
@group(0) @binding(8) var spec_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(9) var spec_in: texture_2d<f32>;

// The albedo's luminance below which the variance is not divided by it.
const DN_FLOOR: f32 = 0.01;

// How many of a pixel's standard errors the filter may move it by (at
// 16 samples and more; more below), and from how many samples.
const DN_TRUST: f32 = 1.0;
const DN_TRUST_FROM: u32 = 8u;

// How many times its neighbours' brightest a pixel's light may be (D3),
// to `DN_CLAMP_FULL` samples; past them, more in proportion.
const DN_CLAMP_FACTOR: f32 = 2.0;
const DN_CLAMP_FULL: u32 = 16u;

fn dn_lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// A pixel's samples: the count, one more where the pass has been; or,
// sampling adaptively, its own.
fn dn_n(x: u32, y: u32) -> u32 {
    if (dp.adaptive != 0u) {
        return u32(abs(pt_sum[dp.width * dp.height + y * dp.width + x].z));
    }
    let ahead = y < dp.split || (y < dp.band_end && x < dp.split_col);
    return dp.count + select(0u, 1u, ahead);
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
    // The diffuse share's luminance, its mean and its mean square; the
    // specular share's (D5), and its light over the same albedo -- when
    // split (`dp.split_spec`), `light` is the diffuse share's. The whole's
    // variance is taken as the shares' summed: their covariance is not
    // gathered (a highlight and the diffuse light share a light's
    // samples), so it is a little low where both are.
    m1: f32,
    m2: f32,
    spec: vec3<f32>,
    s1: f32,
    s2: f32,
};

// A pixel's samples' luminance variance (`m` the diffuse share's, `s` the
// specular's, or the whole's when both).
fn dn_var(m1: f32, m2: f32) -> f32 {
    return max(m2 - m1 * m1, 0.0);
}

// A colour over an albedo, exactly, channel by channel -- a floor in the
// division leaves a dark channel's light a fraction of its neighbours',
// and averaging them multiplied it back up (a yellow face gained +13
// levels of blue). A channel with no albedo reflects nothing to divide;
// the others' light stands in for its neighbours to average, and it is
// multiplied back by zero.
fn dn_demod(c: vec3<f32>, albedo: vec3<f32>) -> vec3<f32> {
    let has = albedo > vec3<f32>(1.0e-12, 1.0e-12, 1.0e-12);
    let e = select(vec3<f32>(0.0, 0.0, 0.0), c / max(albedo, vec3<f32>(1.0e-12, 1.0e-12, 1.0e-12)), has);
    let k = dot(select(vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0), has), vec3<f32>(1.0, 1.0, 1.0));
    let stand_in = (e.x + e.y + e.z) / max(k, 1.0);
    return select(vec3<f32>(stand_in, stand_in, stand_in), e, has);
}

fn dn_pixel(q: vec2<i32>) -> DnPixel {
    var o: DnPixel;
    o.cover = 0.0;
    let i = u32(q.y) * dp.width + u32(q.x);
    let s = pt_sum[i];
    if (!(s.a > 0.0) || dn_n(u32(q.x), u32(q.y)) == 0u) {
        return o;
    }
    let g0 = pt_guide[3u * i];
    let g1 = pt_guide[3u * i + 1u];
    o.cover = s.a;
    o.albedo = g0.rgb / s.a;
    let c = s.rgb / s.a;
    let g2 = pt_guide[3u * i + 2u];
    let sc = g2.rgb / s.a;
    o.s1 = dn_lum(sc);
    o.s2 = g2.a / s.a;
    o.m1 = dn_lum(c) - o.s1;
    o.m2 = g0.a / s.a;
    // The light: the colour over the albedo (`dn_demod`). Split, the
    // specular share's and the rest's.
    if (dp.split_spec != 0u) {
        o.spec = dn_demod(sc, o.albedo);
        o.light = dn_demod(max(c - sc, vec3<f32>(0.0, 0.0, 0.0)), o.albedo);
    } else {
        o.light = dn_demod(c, o.albedo);
    }
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
    if (!(a > 0.0) || dn_n(u32(q.x), u32(q.y)) == 0u) {
        return 0.0;
    }
    return pt_guide[3u * i + 1u].w / a;
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
        textureStore(spec_out, px, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        textureStore(feat_out, px, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    let al = max(dn_lum(p.albedo), DN_FLOOR);
    var variance = 0.0;
    var spec_variance = 0.0;
    if (dn_n(gid.x, gid.y) >= 4u) {
        // The mean's: the samples' over their number. A coverage-weighted
        // mean's is the samples' times sum(a^2) / sum(a)^2, which lies
        // between 1/n and 1/sum(a) and is neither without a channel for
        // sum(a^2). 1/sum(a) is the bound that never trusts too little
        // noise: exact at a silhouette (hits whole, misses none), and up to
        // 1/a too wide under a uniform haze -- so the hazy distance blurs
        // somewhat more, rather than an edge against the sky keeping its
        // noise.
        // Each share's own, or the whole's (their sum) when not split.
        let vd = dn_var(p.m1, p.m2);
        let vs = dn_var(p.s1, p.s2);
        variance = select(vd + vs, vd, dp.split_spec != 0u) / max(p.cover, 1.0) / (al * al);
        spec_variance = vs / max(p.cover, 1.0) / (al * al);
    } else {
        // Too few samples for moments: the light's spread over the
        // neighbours on the same surface.
        var s1 = 0.0;
        var s2 = 0.0;
        var sw = 0.0;
        var t1 = 0.0;
        var t2 = 0.0;
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
                let ls = dn_lum(o.spec);
                t1 = t1 + w * ls;
                t2 = t2 + w * ls * ls;
            }
        }
        if (sw > 0.0) {
            let m = s1 / sw;
            variance = max(s2 / sw - m * m, 0.0);
            let ms = t1 / sw;
            spec_variance = max(t2 / sw - ms * ms, 0.0);
        }
    }
    // An outlier: no brighter than `DN_CLAMP_FACTOR` times the brightest
    // of its neighbours on the same surface (D3), so one rare path -- a
    // bounce that found a small light, a glint -- does not stand out of
    // the filter as a dot. Loosening past `DN_CLAMP_FULL` samples in
    // proportion, so it fades rather than switching off, and a picture of
    // many samples keeps the spiky light a metal really has.
    var light = p.light;
    var spec = p.spec;
    let nc = dn_n(gid.x, gid.y);
    {
        var top = 0.0;
        var found = false;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let q = px + vec2<i32>(dx, dy);
                if ((dx == 0 && dy == 0) || !dn_inside(q)) {
                    continue;
                }
                let o = dn_pixel(q);
                if (!(o.cover > 0.0) || dot(o.normal, p.normal) < 0.8) {
                    continue;
                }
                top = max(top, dn_lum(o.light + o.spec));
                found = true;
            }
        }
        // The whole light against its neighbours' wholes, split or not,
        // both shares scaled alike: a share against its neighbours' same
        // share clipped a highlight a few pixels wide (the matte
        // Mandelbulb's, measured).
        let l = dn_lum(light + spec);
        let cap = DN_CLAMP_FACTOR * max(1.0, f32(nc) / f32(DN_CLAMP_FULL)) * top;
        if (found && l > cap) {
            light = light * (cap / l);
            spec = spec * (cap / l);
        }
    }
    let gx = dn_slope(p.depth, dn_depth(px - vec2<i32>(1, 0)), dn_depth(px + vec2<i32>(1, 0)));
    let gy = dn_slope(p.depth, dn_depth(px - vec2<i32>(0, 1)), dn_depth(px + vec2<i32>(0, 1)));
    textureStore(light_out, px, vec4<f32>(light, variance));
    textureStore(spec_out, px, vec4<f32>(spec, spec_variance));
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
            var w = dn_b3(kx) * dn_b3(ky) * wn * wz * wl;
            // The specular light's pass (D5): a polished surface's
            // reflection is the view's own, not its neighbours', so the
            // pass reaches by the roughness -- none below 0.1, as the
            // diffuse's from 0.5 -- but whole at a few samples, whose noise
            // is the most of any reflection (`4 / n` of it: measured, by
            // the roughness alone the glossy solids at 4 samples were up
            // to 7% worse than not splitting).
            if (dp.spec_pass != 0u) {
                w = w * max(smoothstep(0.1, 0.5, dp.rough), min(4.0 / f32(max(dp.count, 1u)), 1.0));
            }
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
    let n = dn_n(gid.x, gid.y);
    let p = dn_pixel(px);
    if (n == 0u || !(p.cover > 0.0)) {
        textureStore(out_tex, px, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    var light = textureLoad(light_in, px, 0).rgb;
    if (dp.split_spec != 0u) {
        light = light + textureLoad(spec_in, px, 0).rgb;
    }
    let filtered = light * p.albedo;
    // The filter may move a pixel from its own mean by no more than
    // `DN_TRUST` of that mean's standard errors (its samples' luminance
    // moments over their coverage): within them the change is noise
    // removed; past them it is the picture's detail averaged away --
    // a fractal's sub-pixel relief and colouring, which neighbours cannot
    // stand in for -- left as blotches. As the samples gather the bound
    // closes, and the picture converges to the samples' own.
    // (path-tracing-research: measured against 1024 samples, filtering
    // more made solids worse.) Below `DN_TRUST_FROM` samples the moments
    // are too few to judge by, and the filter stands; past it the bound is
    // `sqrt(16 / n)` errors, at least one, as the moments firm up.
    var out = filtered;
    if (n >= DN_TRUST_FROM) {
        let s = pt_sum[u32(px.y) * dp.width + u32(px.x)];
        let raw = s.rgb / s.a;
        let se = sqrt((dn_var(p.m1, p.m2) + dn_var(p.s1, p.s2)) / max(p.cover, 1.0));
        let gap = abs(filtered - raw);
        let off = max(gap.r, max(gap.g, gap.b));
        let allowed = DN_TRUST * max(sqrt(16.0 / f32(n)), 1.0) * se;
        let r = allowed / max(off, 1.0e-30);
        out = raw + (filtered - raw) * select(1.0, r * r, off > allowed);
    }
    textureStore(out_tex, px, vec4<f32>(out, p.cover / f32(n)));
}
"#;

/// Adaptive sampling's check (path-tracing-research V2), over the sum's
/// per-pixel statistics (the second vec4 a pixel, after the frame's sums;
/// `PT_CORE_WGSL`'s `main` gathers them):
/// - `error`: each pixel's noise -- its mean's standard error, from its
///   samples' moments, or (`halves`) the gap between its odd samples' mean
///   and its whole mean (Dammertz et al. 2010, as Cycles) -- in the
///   display's levels of 255: how far one error above the mean shows,
///   through the tone map's exposure and the display's power on it.
///   Written in the statistics' w.
/// - `converge`: a pixel converges where its noise and its eight
///   neighbours' are under the threshold -- an unconverged pixel keeps
///   its neighbours sampling, and a converged one wakes when a
///   neighbour's noise rises; the flag is its count's sign. The pixels
///   still sampling are counted and listed after the statistics, a
///   workgroup's together (its block of the frame: a warp tracing the
///   list takes neighbours where it can).
/// - `publish`: the list's length at its head, for the tracer.
///
/// Passes apart since each reads what the one before writes.
pub(crate) const PATH_ADAPT_WGSL: &str = r#"
struct AdaptParams {
    width: u32,
    height: u32,
    halves: u32,
    pad0: u32,
    threshold: f32,
    exposure: f32,
    power: f32,
    pad1: f32,
};
@group(0) @binding(0) var<uniform> ap: AdaptParams;
@group(0) @binding(1) var<storage, read_write> pt_sum: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> ap_active: atomic<u32>;

var<workgroup> ap_group: atomic<u32>;
var<workgroup> ap_base: u32;

fn ap_lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// The level of 255 the display shows a radiance at.
fn ap_level(x: f32) -> f32 {
    let v = x * ap.exposure;
    return select(0.0, 255.0 * pow(v, ap.power), v > 0.0);
}

@compute @workgroup_size(8, 8, 1)
fn error(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= ap.width || gid.y >= ap.height) {
        return;
    }
    let i = gid.y * ap.width + gid.x;
    let k = ap.width * ap.height + i;
    let st = pt_sum[k];
    let n = abs(st.z);
    var err = 1.0e30;
    if (n >= 2.0) {
        let m = ap_lum(pt_sum[i].rgb) / n;
        var se = sqrt(max(st.x / n - m * m, 0.0) / (n - 1.0));
        if (ap.halves != 0u) {
            se = abs(st.y / floor(n * 0.5) - m);
        }
        err = ap_level(m + se) - ap_level(m);
    }
    pt_sum[k].w = err;
}

@compute @workgroup_size(8, 8, 1)
fn converge(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    if (li == 0u) {
        atomicStore(&ap_group, 0u);
    }
    workgroupBarrier();
    var rank = 0xffffffffu;
    if (gid.x < ap.width && gid.y < ap.height) {
        let base = ap.width * ap.height;
        var worst = 0.0;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let q = vec2<i32>(gid.xy) + vec2<i32>(dx, dy);
                if (q.x < 0 || q.y < 0 || q.x >= i32(ap.width) || q.y >= i32(ap.height)) {
                    continue;
                }
                worst = max(worst, pt_sum[base + u32(q.y) * ap.width + u32(q.x)].w);
            }
        }
        let k = base + gid.y * ap.width + gid.x;
        let n = abs(pt_sum[k].z);
        let done = worst < ap.threshold;
        pt_sum[k].z = select(n, -n, done);
        if (!done) {
            rank = atomicAdd(&ap_group, 1u);
        }
    }
    workgroupBarrier();
    if (li == 0u) {
        ap_base = atomicAdd(&ap_active, atomicLoad(&ap_group));
    }
    workgroupBarrier();
    if (rank != 0xffffffffu) {
        let e = ap_base + rank;
        pt_sum[2u * ap.width * ap.height + 1u + e / 4u][e % 4u] = f32(gid.y * ap.width + gid.x);
    }
}

@compute @workgroup_size(1, 1, 1)
fn publish() {
    pt_sum[2u * ap.width * ap.height].x = f32(atomicLoad(&ap_active));
}
"#;

/// Adaptive sampling's first check (V2), in samples a pixel: from fewer
/// the noise is too rough to judge by, and a rare bright path (a glint
/// the coat's lobe finds now and then) has had little chance to show.
pub(crate) const ADAPTIVE_MIN_SAMPLES: u32 = 16;

/// Whether adaptive sampling checks at `count` samples: at `min`, then
/// each time the count has grown by an eighth (at least one) -- fixed
/// counts, so a picture does not depend on how its samples were batched.
fn adaptive_check_at(count: u32, min: u32) -> bool {
    count > 0 && next_adaptive_check(count - 1, min) == count
}

/// The first count past `count` adaptive sampling checks at.
fn next_adaptive_check(count: u32, min: u32) -> u32 {
    let mut c = min.max(1);
    while c <= count {
        c += (c / 8).max(1);
    }
    c
}

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

/// The strides at `count` samples (D4): the widest dropped as the samples
/// gather -- 16 from 8 samples on, 8 from 32 -- since the noise they average
/// over is gone and what they still cross is the picture's own detail.
fn denoise_steps(count: u32) -> &'static [u32] {
    match count {
        0..8 => &DENOISE_STEPS,
        8..32 => &DENOISE_STEPS[..4],
        _ => &DENOISE_STEPS[..3],
    }
}

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
    /// Two lights (ping-pong) and the features, at their size; and the
    /// specular share's two lights (D5) at theirs -- 1x1 stand-ins, which
    /// nothing reads, while it is not split.
    textures: Option<([(Texture, TextureView); 3], (u32, u32))>,
    spec: Option<([(Texture, TextureView); 2], (u32, u32))>,
}

/// The denoiser's working texture.
fn denoise_texture(device: &Device, (w, h): (u32, u32)) -> (Texture, TextureView) {
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
}

/// Whether the denoiser filters the specular share apart (D5): where
/// the material has a coat or a metal. Without, the share is none and
/// the split changes nothing. Test-only `PT_D5=none` measures without.
fn split_spec(coated: bool) -> bool {
    #[cfg(test)]
    if std::env::var("PT_D5").is_ok_and(|v| v == "none") {
        return false;
    }
    coated
}

/// Where a pass over the frame has reached: the pixels it has given one
/// sample more than the sum's count. Every row above `row`; and of the
/// band from `row` to `band_end`, the columns left of `col` -- a band
/// too costly for one dispatch goes across its width in tiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PathSplit {
    pub(crate) row: u32,
    pub(crate) band_end: u32,
    pub(crate) col: u32,
}

impl PathSplit {
    /// Whole rows: those above `row`.
    pub(crate) fn rows(row: u32) -> Self {
        PathSplit { row, band_end: row, col: 0 }
    }
}

/// Adaptive sampling's check (V2): its passes, and the counter of the
/// pixels still sampling.
struct Adapt {
    error: ComputePipeline,
    converge: ComputePipeline,
    publish: ComputePipeline,
    active: Buffer,
}

/// A path tracer's per-pixel sum (premultiplied radiance, coverage),
/// the samples in it, and the pass that resolves it into an output.
pub(crate) struct PathSum {
    /// `px` pixels' sums, and -- sampling adaptively (`sections` 2) -- as
    /// many pixels' statistics and the last check's list after them
    /// ([`Self::size`]).
    sum: Buffer,
    px: u32,
    sections: u32,
    count: u32,
    resolve_layout: BindGroupLayout,
    resolve_pipeline: ComputePipeline,
    /// The denoiser's guides (two vec4s a pixel), once it has been asked
    /// for, and a stand-in for binding 10 while it is not.
    guide: Option<Buffer>,
    guide_px: u32,
    guided: bool,
    /// The guides were asked for and did not fit (warned once).
    guided_refused: bool,
    no_guide: Buffer,
    denoiser: Option<Denoiser>,
    /// When the denoiser last ran: the count and split it filtered, and
    /// the time (`resolve`'s schedule).
    denoised: Option<(u32, PathSplit, web_time::Instant)>,
    /// Adaptive sampling (V2): on, and whether its statistics were asked
    /// for and did not fit (warned once).
    adaptive: bool,
    adaptive_refused: bool,
    adapt: Option<Adapt>,
    /// The first check's count ([`ADAPTIVE_MIN_SAMPLES`]) and the noise
    /// measured from the odd half rather than the moments.
    adapt_min: u32,
    adapt_halves: bool,
    /// The last check: its count and threshold, and its sequence number.
    checked: Option<(u32, f32)>,
    check_seq: u32,
    /// The checks' counts of the pixels still sampling, in flight: the
    /// check's sequence number, whether it has landed (1, 2 failed), and
    /// where it lands.
    pending: Vec<(u32, Arc<AtomicU8>, Buffer)>,
    /// The pixels still sampling at the last check, once its count is
    /// back; and the latest count back from any check, for a price.
    active: Option<u32>,
    last_active: Option<u32>,
    last_active_seq: u32,
    /// The material's coat -- whether it has one or a metal, and its
    /// roughness -- by which the denoiser filters the specular share (D5;
    /// [`Self::set_material`]).
    coated: bool,
    rough: f32,
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
            sum: Self::create(device, px, 1),
            px,
            sections: 1,
            count: 0,
            resolve_layout,
            resolve_pipeline,
            guide: None,
            guide_px: 0,
            guided: false,
            guided_refused: false,
            no_guide,
            denoiser: None,
            denoised: None,
            adaptive: false,
            adaptive_refused: false,
            adapt: None,
            adapt_min: ADAPTIVE_MIN_SAMPLES,
            adapt_halves: false,
            checked: None,
            check_seq: 0,
            pending: Vec::new(),
            active: None,
            last_active: None,
            last_active_seq: 0,
            coated: false,
            rough: 0.5,
        }
    }

    /// The material the picture is traced with: the denoiser filters the
    /// specular share apart where it has a coat or a metal, a polished
    /// surface's reflection little (D5).
    pub(crate) fn set_material(&mut self, settings: &PathSettings) {
        self.coated = settings.gloss > 0.0 || settings.metallic > 0.0;
        self.rough = if settings.roughness.is_finite() { settings.roughness.clamp(0.0, 1.0) } else { 0.5 };
    }

    /// Sample adaptively from now on (V2), or not: each pixel's
    /// statistics after the sums, where both fit in one binding. True
    /// when that changed: the statistics start with the sum, so the caller
    /// starts it over.
    pub(crate) fn set_adaptive(&mut self, device: &Device, on: bool) -> bool {
        let limits = device.limits();
        let fits = Self::size(self.px, 2) <= (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size);
        if on && !fits && !self.adaptive_refused {
            log::warn!("Adaptive sampling is off for this picture: its {} pixels' statistics exceed one storage binding", self.px);
        }
        self.adaptive_refused = on && !fits;
        let on = on && fits;
        let changed = on != self.adaptive;
        self.adaptive = on;
        if on && self.sections < 2 {
            self.sum.destroy();
            self.sections = 2;
            self.sum = Self::create(device, self.px, 2);
        }
        changed
    }

    /// Whether the sum samples adaptively: asked for, and room for its
    /// statistics. What the pass's uniform must say.
    pub(crate) fn adaptive(&self) -> bool {
        self.adaptive
    }

    /// Test-only: the first check's count, and the noise from the odd
    /// half (Dammertz's) rather than the moments.
    #[cfg(test)]
    pub(crate) fn tune_adaptive(&mut self, min: u32, halves: bool) {
        self.adapt_min = min.max(2);
        self.adapt_halves = halves;
    }

    /// Whether the passes trace the last check's list of the pixels still
    /// sampling (`PathParamsGpu::from_list`): sampling adaptively, once a
    /// check has run.
    pub(crate) fn listed(&self) -> bool {
        self.adaptive && self.checked.is_some()
    }

    /// The count the next check is at, sampling adaptively: a dispatch
    /// of several samples stops there.
    pub(crate) fn next_check(&self) -> Option<u32> {
        self.adaptive.then(|| next_adaptive_check(self.count, self.adapt_min))
    }

    /// Whether the sum is done: `target` samples, or -- sampling
    /// adaptively -- every pixel converged at the last check whose count
    /// is back ([`Self::poll`]). A converged pixel wakes only at a check
    /// that finds a neighbour noisier, and with none sampling none can.
    pub(crate) fn done(&self, target: u32) -> bool {
        self.count >= target || (self.adaptive && self.active == Some(0))
    }

    /// The pixels still sampling at the last check, once its count is
    /// back: the list's length.
    pub(crate) fn active(&self) -> Option<u32> {
        self.adaptive.then_some(self.active).flatten()
    }

    /// The pixels still sampling at the latest check whose count is back,
    /// if not the last: what the list's length is likely to be, to price
    /// a pass by until the last's comes back (it changes by a few percent
    /// a check). None after a check under a new threshold, which may wake
    /// any number.
    pub(crate) fn active_estimate(&self) -> Option<u32> {
        self.adaptive.then_some(self.active.or(self.last_active)).flatten()
    }

    /// Collect the checks' counts that have landed, without waiting.
    pub(crate) fn poll(&mut self, device: &Device) {
        if self.pending.is_empty() {
            return;
        }
        let _ = device.poll(PollType::Poll);
        let mut keep = Vec::new();
        for (seq, flag, stage) in std::mem::take(&mut self.pending) {
            match flag.load(Ordering::Acquire) {
                0 => keep.push((seq, flag, stage)),
                state => {
                    if state == 1 {
                        let v = {
                            let data = stage.slice(..4).get_mapped_range();
                            u32::from_le_bytes(data[0..4].try_into().expect("four bytes"))
                        };
                        stage.unmap();
                        // Only the last check's: an earlier one's count
                        // is not the pixels' now, though it is an
                        // estimate of it.
                        if seq == self.check_seq {
                            self.active = Some(v);
                        }
                        if seq.wrapping_sub(self.last_active_seq) as i32 > 0 {
                            self.last_active = Some(v);
                            self.last_active_seq = seq;
                        }
                    }
                    stage.destroy();
                }
            }
        }
        self.pending = keep;
    }

    /// A new noise threshold (V2): a sum the last check found done may
    /// not be under it, so it is not done until a check under it says so
    /// -- the next dispatch's ([`Self::check_if_due`]). The sum's samples
    /// stand: each pixel's are its own, and the check flags them afresh.
    /// True when it was so.
    pub(crate) fn retune(&mut self, threshold: f32) -> bool {
        if !self.adaptive || !(threshold > 0.0) || !self.checked.is_some_and(|(_, t)| t != threshold) {
            return false;
        }
        self.active = None;
        self.last_active = None;
        true
    }

    /// Adaptive sampling's check (V2), when one is due: at the counts
    /// [`adaptive_check_at`] names, or at once when the threshold has
    /// changed since the last. Its passes over the frame (`w` by `h`;
    /// [`PATH_ADAPT_WGSL`]) flag the pixels that have converged and list
    /// those still sampling, and their count comes back without waiting
    /// ([`Self::poll`], [`Self::done`]). Submits its own work. True when
    /// it ran: a pass over the list starts again on the new one.
    pub(crate) fn check_if_due(&mut self, device: &Device, queue: &Queue, w: u32, h: u32, settings: &PathSettings) -> bool {
        let threshold = settings.noise_threshold;
        if !self.adaptive || !(threshold > 0.0) || self.count < self.adapt_min {
            return false;
        }
        let at_point = adaptive_check_at(self.count, self.adapt_min) && self.checked.is_none_or(|(c, _)| c != self.count);
        let retuned = self.checked.is_some_and(|(_, t)| t != threshold);
        if !(at_point || retuned) {
            return false;
        }
        self.checked = Some((self.count, threshold));
        self.check_seq = self.check_seq.wrapping_add(1);
        self.active = None;
        if retuned {
            self.last_active = None;
        }
        let a = self.adapt.get_or_insert_with(|| {
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Path Adapt"),
                source: ShaderSource::Wgsl(PATH_ADAPT_WGSL.into()),
            });
            let make = |entry: &str| {
                device.create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some(&format!("Path Adapt {entry}")),
                    layout: None,
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            };
            Adapt {
                error: make("error"),
                converge: make("converge"),
                publish: make("publish"),
                active: device.create_buffer(&BufferDescriptor {
                    label: Some("Path Adapt Active"),
                    size: 4,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
            }
        });
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("Path Adapt Params"),
            size: 32,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let [exposure, power] = settings.display;
        queue.write_buffer(&params, 0, bytemuck::cast_slice(&[w, h, u32::from(self.adapt_halves), 0, threshold.to_bits(), exposure.to_bits(), power.to_bits(), 0]));
        queue.write_buffer(&a.active, 0, &[0u8; 4]);
        let stage = device.create_buffer(&BufferDescriptor {
            label: Some("Path Adapt Readback"),
            size: 4,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Path Adapt") });
        for (pipeline, counted) in [(&a.error, false), (&a.converge, true), (&a.publish, true)] {
            let mut entries = vec![
                BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.sum.as_entire_binding() },
            ];
            if counted {
                entries.push(BindGroupEntry { binding: 2, resource: a.active.as_entire_binding() });
            }
            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Path Adapt"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &entries,
            });
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Path Adapt"), timestamp_writes: None });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bg, &[]);
            if std::ptr::eq(pipeline, &a.publish) {
                pass.dispatch_workgroups(1, 1, 1);
            } else {
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
        }
        enc.copy_buffer_to_buffer(&a.active, 0, &stage, 0, 4);
        queue.submit(std::iter::once(enc.finish()));
        let landed = Arc::new(AtomicU8::new(0));
        let flag = landed.clone();
        stage.slice(..4).map_async(MapMode::Read, move |r| {
            flag.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
        });
        self.pending.push((self.check_seq, landed, stage));
        true
    }

    /// Gather the denoiser's guides with the samples from now on, or
    /// not. True when that changed: the guides start with the sum, so the
    /// caller starts it over.
    pub(crate) fn set_guided(&mut self, device: &Device, on: bool) -> bool {
        // The guides are 48 bytes a pixel, bound whole: past what one
        // binding holds (128 MB on a browser's floor: 1632 x 1632, 1080p
        // within it), the picture goes undenoised rather than failing
        // validation. A terrain's still tiles stay within it; a solid's
        // frame need not.
        let limits = device.limits();
        let fits = self.px as u64 * GUIDE_BYTES <= (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size);
        if on && !fits && !self.guided_refused {
            log::warn!("Denoise is off for this picture: its {} pixels' guides exceed one storage binding", self.px);
        }
        self.guided_refused = on && !fits;
        let on = on && fits;
        let changed = on != self.guided;
        self.guided = on;
        if on && (self.guide.is_none() || self.guide_px < self.px) {
            if let Some(g) = self.guide.take() {
                g.destroy();
            }
            self.guide = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Path Guide"),
                size: self.px as u64 * GUIDE_BYTES,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            self.guide_px = self.px;
        }
        changed
    }

    /// Whether the guides are gathered: the denoiser asked for, and room
    /// for them. What the pass's uniform must say.
    pub(crate) fn guided(&self) -> bool {
        self.guided
    }

    /// Test-only: the running sum.
    #[cfg(test)]
    pub(crate) fn sum_buffer(&self) -> &Buffer {
        &self.sum
    }

    /// What binding 10 holds: the guides, or a stand-in the shader does
    /// not touch.
    pub(crate) fn guide_buffer(&self) -> &Buffer {
        match (&self.guide, self.guided) {
            (Some(g), true) => g,
            _ => &self.no_guide,
        }
    }

    /// The sum's bytes for `px` pixels: their sums, and -- sampling
    /// adaptively (`sections` 2) -- their statistics, and the list's
    /// length and its entries, four a vec4.
    fn size(px: u32, sections: u32) -> u64 {
        let px = px as u64;
        16 * if sections > 1 { 2 * px + 1 + px.div_ceil(4) } else { px }
    }

    fn create(device: &Device, px: u32, sections: u32) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("Path Sum"),
            size: Self::size(px, sections),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Room for `px` pixels; the sum starts over.
    pub(crate) fn ensure(&mut self, device: &Device, px: u32) {
        if px > self.px {
            self.sum.destroy();
            self.sum = Self::create(device, px, self.sections);
            self.px = px;
        }
        self.reset();
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
        self.checked = None;
        // A count in flight is the old picture's.
        self.check_seq = self.check_seq.wrapping_add(1);
        self.active = None;
        self.last_active = None;
        self.last_active_seq = self.check_seq;
    }

    /// `n` more samples are in it, over the whole frame.
    pub(crate) fn add(&mut self, n: u32) {
        self.count += n;
    }

    /// The mean into `out` (`w` by `h`); the pixels `split` names hold
    /// one sample more than the count. Submits its own work.
    ///
    /// Denoised when the guides are on -- on a schedule, since the filter
    /// costs about a sample of a terrain at 1080p (10 ms on a GTX 1660)
    /// and a few more samples change the picture little: every count up
    /// to 4, then each time the count has grown by a quarter, and at
    /// least every quarter second while anything new is in the sum. In
    /// between, `out` keeps the last. [`Self::resolve_now`] for the
    /// picture that must be current.
    pub(crate) fn resolve(&mut self, device: &Device, queue: &Queue, out: &TextureView, w: u32, h: u32, split: PathSplit) {
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
            size: 32,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, bytemuck::cast_slice(&[w, h, self.count, split.row, split.band_end, split.col, u32::from(self.adaptive), 0]));
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
    pub(crate) fn resolve_now(&mut self, device: &Device, queue: &Queue, out: &TextureView, w: u32, h: u32, split: PathSplit) {
        self.denoised = None;
        self.resolve(device, queue, out, w, h, split);
    }

    /// The denoiser's resolve: `prepare`, the à-trous passes and
    /// `finish`, into `out`.
    fn denoise(&mut self, device: &Device, queue: &Queue, out: &TextureView, w: u32, h: u32, at: PathSplit) {
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
            Denoiser { prepare: make("prepare"), atrous: make("atrous"), finish: make("finish"), textures: None, spec: None }
        });
        if d.textures.as_ref().is_none_or(|(_, s)| *s != (w, h)) {
            if let Some((ts, _)) = d.textures.take() {
                for (t, _) in ts {
                    t.destroy();
                }
            }
            d.textures = Some(([0; 3].map(|_| denoise_texture(device, (w, h))), (w, h)));
        }
        let split_on = split_spec(self.coated);
        let spec_size = if split_on { (w, h) } else { (1, 1) };
        if d.spec.as_ref().is_none_or(|(_, s)| *s != spec_size) {
            if let Some((ts, _)) = d.spec.take() {
                for (t, _) in ts {
                    t.destroy();
                }
            }
            d.spec = Some(([0; 2].map(|_| denoise_texture(device, spec_size)), spec_size));
        }
        let ([a, b, feat], _) = d.textures.as_ref().expect("made above");
        let ([sa, sb], _) = d.spec.as_ref().expect("made above");
        let split = u32::from(split_on);
        let rough = self.rough;
        let guide = self.guide.as_ref().expect("guided");
        let params = |step: u32, spec_pass: bool| {
            let buf = device.create_buffer(&BufferDescriptor {
                label: Some("Path Denoise Params"),
                size: 48,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let sigma = DENOISE_SIGMA * (DENOISE_FULL / self.count.max(1) as f32).sqrt().min(1.0);
            let adaptive = u32::from(self.adaptive);
            queue.write_buffer(
                &buf,
                0,
                bytemuck::cast_slice(&[w, h, self.count, at.row, step, sigma.to_bits(), at.band_end, at.col, adaptive, split, u32::from(spec_pass), rough.to_bits()]),
            );
            buf
        };
        let (gx, gy) = (w.div_ceil(8), h.div_ceil(8));
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Path Denoise") });
        let run = |enc: &mut CommandEncoder, pipeline: &ComputePipeline, entries: &[BindGroupEntry]| {
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
        let p0 = params(0, false);
        run(
            &mut enc,
            &d.prepare,
            &[
                BindGroupEntry { binding: 0, resource: p0.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.sum.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: guide.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: BindingResource::TextureView(&a.1) },
                BindGroupEntry { binding: 6, resource: BindingResource::TextureView(&feat.1) },
                BindGroupEntry { binding: 8, resource: BindingResource::TextureView(&sa.1) },
            ],
        );
        // The diffuse light, or the whole when not split; then the
        // specular (D5). Each pass reads one of a pair and writes the
        // other: the last writes the first of the pair after an even
        // number of passes.
        let steps = denoise_steps(self.count);
        let passes = |enc: &mut CommandEncoder, pair: [&TextureView; 2], spec_pass: bool| {
            for (i, &step) in steps.iter().enumerate() {
                let ps = params(step, spec_pass);
                run(
                    enc,
                    &d.atrous,
                    &[
                        BindGroupEntry { binding: 0, resource: ps.as_entire_binding() },
                        BindGroupEntry { binding: 3, resource: BindingResource::TextureView(pair[i % 2]) },
                        BindGroupEntry { binding: 4, resource: BindingResource::TextureView(&feat.1) },
                        BindGroupEntry { binding: 5, resource: BindingResource::TextureView(pair[(i + 1) % 2]) },
                    ],
                );
            }
        };
        let odd = steps.len() % 2 == 1;
        passes(&mut enc, [&a.1, &b.1], false);
        let light = if odd { b } else { a };
        let spec = if split_on {
            passes(&mut enc, [&sa.1, &sb.1], true);
            if odd { sb } else { sa }
        } else {
            sa
        };
        run(
            &mut enc,
            &d.finish,
            &[
                BindGroupEntry { binding: 0, resource: p0.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.sum.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: guide.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&light.1) },
                BindGroupEntry { binding: 7, resource: BindingResource::TextureView(out) },
                BindGroupEntry { binding: 9, resource: BindingResource::TextureView(&spec.1) },
            ],
        );
        queue.submit(std::iter::once(enc.finish()));
    }

    pub(crate) fn destroy(&self) {
        self.sum.destroy();
        self.no_guide.destroy();
        if let Some(a) = &self.adapt {
            a.active.destroy();
        }
        if let Some(g) = &self.guide {
            g.destroy();
        }
        if let Some(d) = &self.denoiser {
            for (t, _) in d.textures.iter().flat_map(|(ts, _)| ts.iter()).chain(d.spec.iter().flat_map(|(ts, _)| ts.iter())) {
                t.destroy();
            }
        }
    }
}

