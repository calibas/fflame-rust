//! The escape producer of the 3D terrain view
//! (docs/projects/heightfield-3d.md, sections 5 and 12).
//!
//! **The ground to the horizon** (section 12, the user's design). The
//! plane is cut into square SECTIONS on a dyadic grid fixed in the
//! plane, each one ordinary [`EscapeRenderer`] render of the same size
//! ([`SECTION_SAMPLES`]²): a section twice as far from the eye is twice
//! as wide, so its texels stay about `supersample × detail` a screen
//! pixel wide where it is nearest. They are rendered a few a frame --
//! coarse ones first, so the whole view lands quickly, then near ones --
//! and kept by their place in the plane, so a pan renders only what it
//! uncovers and a dolly brings finer sections in near the eye. The
//! ground ends at `far` view widths, where the fog has reached the
//! background.
//!
//! **A section's texels** (docs/projects/terrain-height.md, H4) are its
//! render's samples -- or with a prefilter, the mean of `prefilter²` of
//! them over each texel's footprint -- softened by a Gaussian of
//! `softness` texels, for which the render has a border of three sigmas
//! about the section ([`section_filter`]), so neighbours still meet
//! exactly.
//!
//! **Where things are.** The sections' grid hangs from an ANCHOR: a
//! point of the plane in exact decimals and a zoom, a level-0 section
//! being that zoom's view width. The shader's world is the CURRENT view
//! width, x along Re, y along Im, z up, with the origin at the eye's
//! ground point -- so it never sees a big number, at any depth (plan
//! H3).
//!
//! **The anchor is the config's** ([`canonical_anchor`]): an eight-octave
//! band of zoom and the centre truncated to a decimal lattice thousands
//! of view widths apart. So the sections -- and once they are all in,
//! the picture -- are a function of the config and the picture's size,
//! the viewport's and an export's alike; leaving the band or the
//! lattice cell starts the sections over.
//!
//! **The camera** is mode D's, read for a plane:
//! - the target is the view's centre at the terrain's top, H (a locked
//!   relief's, below), lifted by `target_lift` view widths (0 by
//!   default), so with any pitch above
//!   the horizon the eye is above every point of it -- and lifted, it
//!   can look level or up, at the sky (camera-unification C4);
//! - `cam_pitch` is above the horizon; `cam_yaw` turns about the target,
//!   0 looking up the 2D picture -- which the View's `rotation` turns,
//!   so it turns the heading too;
//! - the eye is [`FRAME_DISTANCE`] view widths from the target, so a
//!   zoom is a dolly.
//!
//! **Heights are the view's.** H and the flanks' width are fractions of
//! the current view width, applied in the walk to the sections' raw
//! estimates: a dolly in flattens the far ground and raises the near
//! detail -- the terrain at every zoom looks like itself. A count's or a
//! relief's range is the ground's own, measured as it is set.
//!
//! **Unless locked** (docs/projects/terrain-height.md, H3): then they are
//! fractions of the LOCK's view width ([`EscapeConfig::terrain_scale`]),
//! and the range is the one pinned at the lock -- one landscape, fixed
//! in the plane, which a dolly approaches.

use super::ifs::{solid_frame, SolidCamera};
use super::terrain::{Ground, GroundNode, GroundSection, PathSettings, SectionFilter, TerrainIngest, TerrainRenderer, TerrainView};
use super::terrain_tiers::{TerrainTiers, TierInputs};
#[cfg(test)]
use crate::config::escape::RenderTier;
use super::EscapeRenderer;
use crate::config::escape::{EscapeConfig, TerrainInterior, TerrainSource};
use crate::config::FractalConfig;
use std::collections::HashMap;
use wgpu::*;

/// The eye's distance from the target, in view widths.
pub const FRAME_DISTANCE: f64 = 1.3;

/// A section's cells a side. Its render is one sample more, so that
/// neighbours share their edge samples: 1024 costs per pixel what 2048
/// does (measured, 4.4 ns), and fits the view more finely.
pub const SECTION_CELLS: u32 = 1024;
pub const SECTION_SAMPLES: u32 = SECTION_CELLS + 1;

/// A hidden root's samples a side, when its render is perturbed (deep):
/// a sixteenth of a section's. A root no primary ray reaches once the
/// view is filled -- split, so the leaves under it cover all of it in
/// view, or out of view -- serves only shadows, bounces and the moment
/// before its leaves arrive, none of which needs a section's detail;
/// and the roots, the widest sections, cost the most a sample in a
/// deep picture: measured at 2^60, the nine of a 1080p view were a
/// quarter of the fill.
pub const COARSE_SAMPLES: u32 = 257;

/// The most sections the atlas holds (each about 21 MB): the viewport's,
/// and an export's floor.
pub const MAX_SECTIONS: u32 = 64;

/// The most an export's atlas holds, where the card has the memory
/// ([`fit_export_sections`]): 256 sections, 5.4 GB. At 4K the export's
/// own view wants ~230 at the default fineness; at 64 its nearest ground
/// was 0.74 texels a screen pixel, magnified.
pub const EXPORT_MAX_SECTIONS: u32 = 256;

/// The finest section, against the view width: a section narrower than
/// 2^-14 of it is never asked for, however near the eye.
const FINEST_REL: f64 = 1.0 / 16384.0;

/// The anchor's zoom band, in octaves: a dolly re-anchors once in this
/// many, and the sections' grid in f64 never has to hold more than this
/// many octaves between the anchor's widths and the view's.
const ANCHOR_BAND: f64 = 8.0;

/// The anchor lattice's spacing, as a power of two of anchor widths, at
/// least: the centre is truncated to a decimal lattice no finer.
const ANCHOR_LATTICE: f64 = 12.0;

/// A section's render config: the escape config, with what the terrain
/// path forces. No supersample: the terrain's own accumulation
/// antialiases what the screen sees, and the sections' sizes are chosen
/// for the screen.
pub fn footprint_config(escape: &EscapeConfig) -> EscapeConfig {
    let mut f = escape.clone();
    f.supersample = 1;
    f
}

/// What the sections' PICTURES depend on: the config with the view --
/// centre, zoom, rotation -- the camera and everything only the 3D view
/// reads set to their defaults. The sections are fixed in the plane:
/// moving the view moves the camera over them.
fn picture_key(escape: &EscapeConfig) -> EscapeConfig {
    let mut k = footprint_config(escape);
    let d = EscapeConfig::default();
    k.center_re = d.center_re;
    k.center_im = d.center_im;
    k.zoom_log2 = d.zoom_log2;
    k.rotation = d.rotation;
    k.cam_target_x = d.cam_target_x;
    k.cam_target_y = d.cam_target_y;
    k.cam_target_z = d.cam_target_z;
    k.cam_pitch = d.cam_pitch;
    k.cam_yaw = d.cam_yaw;
    k.cam_bank = d.cam_bank;
    k.cam_fov = d.cam_fov;
    // The path tracer's and the solid tier's settings, the downsample
    // (a section renders at one sample a pixel) and the relief's LIGHT
    // (the 2D relief is off under a terrain, whose own lights light it)
    // are the walk's or nobody's: kept, a sample count or a lens edit
    // threw away every section and rendered them all again -- seconds
    // at depth -- where the path tracer only needed to restart.
    k.path = d.path.clone();
    k.solid_tier = d.solid_tier;
    k.downsample = d.downsample;
    k.shading.light_angle = d.shading.light_angle;
    k.shading.shadow_color = d.shading.shadow_color;
    k.shading.shadow_strength = d.shading.shadow_strength;
    k.shading.shadow_blend = d.shading.shadow_blend;
    k.shading.highlight_color = d.shading.highlight_color;
    k.shading.highlight_strength = d.shading.highlight_strength;
    k.shading.highlight_blend = d.shading.highlight_blend;
    // The source decides what the iterate pass writes, and the interior
    // and a lake's tint how a section encodes it (the tint is its albedo);
    // the rest is the walk's and the view's.
    let (source, interior, tint) = (k.terrain.source, k.terrain.interior, k.terrain.lake_tint);
    // And the filter makes them (H4).
    let (prefilter, softness) = (k.terrain.prefilter, k.terrain.softness);
    k.terrain = d.terrain.clone();
    k.terrain.enabled = true;
    k.terrain.source = source;
    k.terrain.interior = interior;
    k.terrain.lake_tint = tint;
    k.terrain.prefilter = prefilter;
    k.terrain.softness = softness;
    k
}

/// What else the sections' pictures read from the config: the palette
/// and every setting that writes its texture, the background (a
/// plateau's colour), and the flame when the formula is a 2D IFS that
/// draws it. Keyed by content, not by the flame renderer's palette
/// generation, which every config load bumps.
pub(crate) fn palette_key(config: &FractalConfig) -> String {
    let flame = if super::ifs::get_ifs(&config.escape.formula).is_some() {
        format!("{:?}", config.flame)
    } else {
        String::new()
    };
    format!(
        "{flame}|{:?}|{:?}|{}|{}|{}|{:?}|{}|{}|{}",
        config.background_color,
        config.palette,
        config.palette_rotation,
        config.palette_size,
        config.palette_squeeze,
        config.palette_squeeze_mode,
        config.palette_squeeze_falloff,
        config.palette_log_strength,
        config.palette_reverse
    )
}

/// The terrain's camera in the WORLD (see the module docs): current view
/// widths, the origin at the eye's ground point.
pub fn terrain_camera(escape: &EscapeConfig) -> SolidCamera {
    let (right, up, forward) = solid_frame(
        escape.cam_pitch as f64,
        escape.cam_yaw as f64 + escape.rotation as f64 - std::f64::consts::FRAC_PI_2,
        escape.cam_bank as f64,
        0.0,
    );
    let distance = FRAME_DISTANCE;
    let eye_rel = [-forward[0] * distance, -forward[1] * distance, -forward[2] * distance];
    // The target rests on the relief's top -- a locked relief's, in
    // current widths (H3) -- and is lifted from there.
    let target_z = escape.terrain.height as f64 * escape.terrain_scale() + escape.terrain.target_lift as f64;
    SolidCamera {
        eye: [0.0, 0.0, target_z + eye_rel[2]],
        target: [-eye_rel[0], -eye_rel[1], target_z],
        forward,
        right,
        up,
        fov: escape.cam_fov.clamp(0.05, 3.0),
        distance,
        eye_rel,
    }
}

/// How a config's sections are made from their renders (H4): its
/// prefilter, and its softness with a border of three sigmas, so the
/// blur at a section's edge reads its neighbour's ground.
pub fn section_filter(t: &crate::config::escape::TerrainConfig) -> SectionFilter {
    use crate::config::escape::{MAX_TERRAIN_PREFILTER, MAX_TERRAIN_SOFTNESS};
    let softness = if t.softness.is_finite() { t.softness.clamp(0.0, MAX_TERRAIN_SOFTNESS) } else { 0.0 };
    SectionFilter {
        samples: t.prefilter.clamp(1, MAX_TERRAIN_PREFILTER),
        apron: (3.0 * softness).ceil() as u32,
        softness,
    }
}

/// What an export of `config` at `w x h` holds beside its atlas at its
/// peak, estimated on the high side: the larger of the section renderer
/// while sections render (its prefilter's render pixels at ~148 bytes,
/// within the escape renderer's 1.5 GB budget) and the still's picture
/// (16 bytes a pixel, tiled stills only); the terrain renderer at its
/// tile (~160 bytes a pixel); the tail's output and readback (8 bytes a
/// pixel), and the effect chain's slots when effects are on.
pub fn export_headroom(config: &FractalConfig, w: u32, h: u32) -> u64 {
    let filter = section_filter(&config.escape.terrain);
    let across = (SECTION_SAMPLES + 2 * filter.apron) as u64 * filter.samples as u64;
    let sections = (across * across * 148).min(3 << 29);
    let frame = w as u64 * h as u64;
    let (tw, th) = super::terrain_tiers::still_tile(w, h);
    let tile = tw as u64 * th as u64;
    let picture = if tile < frame { frame * 16 } else { 0 };
    use crate::renderer::effect_chain::EffectChainRunner;
    let effects = if EffectChainRunner::has_enabled_effects(&config.density_effects) || EffectChainRunner::has_enabled_effects(&config.color_effects) { frame * 48 } else { 0 };
    sections.max(picture) + tile * 160 + frame * 8 + effects
}

/// The most sections an export's atlas can hold on this card beside the
/// rest of its peak ([`export_headroom`]): [`EXPORT_MAX_SECTIONS`] and
/// down by halves to [`MAX_SECTIONS`], the first whose memory can be had.
/// wgpu cannot say how much memory is free, so this asks for it -- in
/// pieces, stopping at the first refused, and giving it all back at once.
/// Measured on a 6 GB card: an export ran out of memory where nothing
/// said it would, and the atlas is most of what it holds.
///
/// Only on a discrete card, whose own memory refuses what it does not
/// have. Elsewhere the viewport's [`MAX_SECTIONS`], as before: a unified
/// memory (Apple silicon, an integrated GPU) can grant far past what is
/// comfortable and pay for it in paging; a browser may not allocate a
/// buffer until it is used, so the probe would measure nothing; and a
/// software renderer is the reference, whose pictures should not depend
/// on the machine.
pub async fn fit_export_sections(device: &Device, config: &FractalConfig, w: u32, h: u32) -> u32 {
    if cfg!(target_arch = "wasm32") || device.adapter_info().device_type != DeviceType::DiscreteGpu {
        return MAX_SECTIONS;
    }
    let lim = device.limits();
    let layer = TerrainRenderer::atlas_layer_bytes(SECTION_SAMPLES, SECTION_SAMPLES);
    let headroom = export_headroom(config, w, h) + (256 << 20);
    let piece = lim.max_buffer_size.min(256 << 20).max(1 << 20);
    let mut cap = EXPORT_MAX_SECTIONS.min(lim.max_texture_array_layers).max(MAX_SECTIONS);
    while cap > MAX_SECTIONS {
        let want = cap as u64 * layer + headroom;
        let mut held: Vec<Buffer> = Vec::new();
        let mut got = 0u64;
        let mut refused = false;
        while got < want {
            let size = piece.min(want - got);
            let scope = device.push_error_scope(ErrorFilter::OutOfMemory);
            let b = device.create_buffer(&BufferDescriptor {
                label: Some("Terrain Export Memory Probe"),
                size,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
            let failed = scope.pop().await.is_some();
            held.push(b);
            if failed {
                refused = true;
                break;
            }
            got += size;
        }
        for b in held {
            b.destroy();
        }
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        if !refused {
            log::info!("terrain export: {cap} sections fit ({} MB with the rest of the export)", want >> 20);
            return cap;
        }
        cap /= 2;
    }
    MAX_SECTIONS
}

/// How far below the plain a hole's floor is, in view widths: a sixth of
/// the relief, and at most 0.01 -- the depth every terrain had at the
/// default height (6%) and above, so those keep their holes. At a height
/// of 0 the terrain is a flat plane, its holes included; a hair of slab
/// is kept so the walk's clipping has two faces.
pub fn hole_floor(height: f32) -> f64 {
    -(height.max(0.0) as f64 / 6.0).clamp(1.0e-6, 0.01)
}

/// The terrain's fog: reaching the background just before `far`, from a
/// start the haze brings nearer (none at 0). In the world's units.
pub fn terrain_fog(escape: &EscapeConfig) -> (f32, f32) {
    let t = &escape.terrain;
    let haze = t.haze.clamp(0.0, 1.0);
    if !(haze > 0.0) {
        return (0.0, 0.0);
    }
    let start = t.far * (1.0 - 0.65 * haze);
    // 1 - e^-4.6 is 99%.
    (4.6 / (t.far - start).max(1.0e-6), start)
}

/// How the terrain is seen and lit, in the world: the camera, the Solid
/// lighting, the terrain's fog, and the walk's map.
pub fn terrain_view(config: &FractalConfig, jitter: [f32; 2]) -> TerrainView {
    let t = &config.escape.terrain;
    let (fog, fog_start) = terrain_fog(&config.escape);
    // The relief's lengths, in current widths: a locked relief's are its
    // lock's (H3).
    let s = config.escape.terrain_scale() as f32;
    TerrainView {
        camera: terrain_camera(&config.escape),
        shading: config.solid_shading.clone(),
        fog: (fog, fog_start, config.background_color),
        shadow: t.shadow,
        softness: t.shadow_sharpness,
        occlusion_reach: t.occlusion * s,
        jitter,
        height: t.height * s,
        width: t.de_width * s,
        samples_per_axis: config.escape.supersample.max(1),
        far: t.far,
        sky: super::path_core::sky_seen(&config.escape.path, config),
        material: super::path_core::lit_material(config),
        curve: t.curve.code(),
        invert: t.invert,
        range: t.pinned_range(),
    }
}

/// What the tiers draw an escape terrain as.
fn tier_inputs<'a>(config: &FractalConfig, view: &'a dyn Fn([f32; 2]) -> TerrainView) -> TierInputs<'a> {
    TierInputs {
        view,
        settings: path_settings(config),
        tier: config.escape.terrain.tier,
        samples: config.escape.path.samples,
        supersample: config.escape.supersample,
        filling: false,
    }
}

/// The path tracer's settings for a terrain: the shared ones, with the
/// lens in the world's units -- view widths, the target
/// [`FRAME_DISTANCE`] away.
pub fn path_settings(config: &FractalConfig) -> PathSettings {
    super::path_core::path_settings(config, FRAME_DISTANCE as f32)
}

/// How a section becomes atlas samples. `derivative` is whether its
/// iterate pass compiled one: without it the distance source wrote the
/// escape count (`esc_terrain_source`), and the ingest must read it as
/// one.
pub fn terrain_ingest(config: &FractalConfig, derivative: bool) -> TerrainIngest {
    let t = &config.escape.terrain;
    let source = match t.source {
        TerrainSource::Distance if derivative => 8,
        TerrainSource::Distance | TerrainSource::EscapeCount => 9,
        TerrainSource::Relief => config.escape.shading.field.to_gpu(),
    };
    let interior = match t.interior {
        TerrainInterior::Plateau => 0,
        TerrainInterior::Hole => 1,
        TerrainInterior::Lake => 2,
    };
    TerrainIngest { source, interior, background: config.background_color, tint: t.lake_tint }
}

/// The plane's frame for the sections: a point in exact decimals and a
/// zoom whose view width is a level-0 section's side.
#[derive(Debug, Clone, PartialEq)]
struct Anchor {
    re: String,
    im: String,
    zoom: f64,
}

/// The anchor a config's sections hang from: its zoom band's floor, and
/// its centre truncated toward zero to the decimal lattice of `10^-D`
/// whose spacing is at least 2^12 of that zoom's widths. A function of
/// the config alone, so every engine places its sections alike.
fn canonical_anchor(escape: &EscapeConfig) -> Anchor {
    let zoom = (escape.zoom_log2 / ANCHOR_BAND).floor() * ANCHOR_BAND;
    // 10^-D >= 2^(lattice + 2 - zoom): a width is 4 * 2^-zoom.
    let digits = ((zoom - ANCHOR_LATTICE - 2.0) * std::f64::consts::LOG10_2).floor() as i64;
    Anchor { re: truncate_decimal(&escape.center_re, digits), im: truncate_decimal(&escape.center_im, digits), zoom }
}

/// A decimal string truncated toward zero to `digits` places after the
/// point (a negative count zeroes that many integer digits).
fn truncate_decimal(s: &str, digits: i64) -> String {
    let s = s.trim();
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    // A malformed centre (mid-edit text) anchors at zero.
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit() || c == '.') || body.matches('.').count() > 1 {
        return "0".to_string();
    }
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    let int = int.trim_start_matches('0');
    let (int, frac): (String, String) = if digits >= 0 {
        (int.to_string(), frac.chars().take(digits as usize).collect())
    } else {
        let keep = int.len().saturating_sub((-digits) as usize);
        let zeros = int.len() - keep;
        (format!("{}{}", &int[..keep], "0".repeat(zeros)), String::new())
    };
    let frac = frac.trim_end_matches('0');
    let int = if int.trim_start_matches('0').is_empty() { "0".to_string() } else { int };
    let out = if frac.is_empty() { int } else { format!("{int}.{frac}") };
    if neg && out.chars().any(|c| c.is_ascii_digit() && c != '0') {
        format!("-{out}")
    } else {
        out
    }
}

/// The view's centre less the anchor, in the anchor's widths (x along
/// Re, y along Im). Subtracted in fixed point, so it holds at any depth.
fn view_offset(escape: &EscapeConfig, a: &Anchor) -> [f64; 2] {
    use super::fixedpoint::{limbs_for_view, FixedPoint};
    let deep = escape.zoom_log2.max(a.zoom);
    let limbs = limbs_for_view(&escape.center_re, &escape.center_im, deep).max(limbs_for_view(&a.re, &a.im, deep));
    let diff = |p: &str, q: &str| -> f64 {
        match (FixedPoint::from_decimal(p, limbs), FixedPoint::from_decimal(q, limbs)) {
            (Some(p), Some(q)) => {
                let fe = p.sub(&q).to_floatexp();
                // A width is 4 * 2^-zoom.
                fe.m * (fe.e as f64 + a.zoom - 2.0).clamp(-1000.0, 1000.0).exp2()
            }
            _ => 0.0,
        }
    };
    [diff(&escape.center_re, &a.re), diff(&escape.center_im, &a.im)]
}

/// A section: its level (a side of 2^level anchor widths) and its place
/// on that level's grid, (0, 0) the square whose south-west corner is
/// the anchor.
pub type SectionKey = (i32, i64, i64);

/// A section's render config: rotation 0, its samples on its grid
/// points -- `samples` of them across a span one texel wider than its
/// side, centred on its centre, so sample (0, 0) is its south-west
/// corner and neighbours share their edges.
fn section_config(fp: &EscapeConfig, a: &Anchor, key: SectionKey, samples: u32) -> EscapeConfig {
    bordered_section_config(fp, a, key, samples, 0)
}

/// [`section_config`] with a border of `apron` texels about the section
/// (H4): `samples + 2 apron` a side on the same grid, the same centre.
fn bordered_section_config(fp: &EscapeConfig, a: &Anchor, key: SectionKey, samples: u32, apron: u32) -> EscapeConfig {
    use super::fixedpoint::FixedPoint;
    let (level, i, j) = key;
    let mut c = fp.clone();
    c.rotation = 0.0;
    let across = (samples + 2 * apron) as f64;
    c.zoom_log2 = a.zoom - level as f64 - (across / (samples - 1) as f64).log2();
    // The centre from the anchor: (i + 1/2) 2^level anchor widths, a
    // width 4 * 2^-zoom, kept as a power of two and a mantissa.
    let x = 2.0 + level as f64 - a.zoom;
    let e = x.floor();
    let m = (x - e).exp2();
    if let (Some(re), Some(im)) = (
        FixedPoint::decimal_add_floatexp(&a.re, (i as f64 + 0.5) * m, e as i64, c.zoom_log2),
        FixedPoint::decimal_add_floatexp(&a.im, (j as f64 + 0.5) * m, e as i64, c.zoom_log2),
    ) {
        c.center_re = re;
        c.center_im = im;
    }
    c
}

/// Where auto contrast's one fit for a terrain is measured: the config's
/// own 2D view, unrotated -- the picture the terrain was turned on from,
/// whose colours it then keeps. Measured when the sections start (a new
/// picture, a new anchor) and held while they last, so a pan within the
/// anchor keeps it: an export from another view fits there. Fitted each
/// its own way, every section was a palette turn of its own, a step of
/// colour at every edge.
fn contrast_probe_config(fp: &EscapeConfig) -> EscapeConfig {
    let mut c = fp.clone();
    c.rotation = 0.0;
    c
}

/// What the view wants: the root grid's level and its first square, the
/// grid's size, and the sections wanted -- the roots, and the leaves of
/// the split -- each with its nearest distance from the eye (current
/// widths), roots first; and the roots no primary ray reaches once the
/// leaves are in: split, or out of view.
#[derive(Debug, Clone)]
pub struct Wanted {
    pub root_level: i32,
    pub root_first: [i64; 2],
    pub root_dims: [u32; 2],
    pub sections: Vec<(SectionKey, f64, bool)>,
    pub hidden_roots: std::collections::HashSet<SectionKey>,
}

/// The camera relative to the anchor: the eye's ground point in anchor
/// widths, and current widths per anchor width.
fn eye_in_anchor(escape: &EscapeConfig, a: &Anchor) -> ([f64; 2], f64) {
    let z = (escape.zoom_log2 - a.zoom).exp2();
    let cam = terrain_camera(escape);
    let off = view_offset(escape, a);
    // The target is at the view's centre; the eye's ground point is
    // `eye_rel` back from it.
    ([off[0] + cam.eye_rel[0] / z, off[1] + cam.eye_rel[1] / z], z)
}

/// The sections a view wants (plan section 12): a root grid of sections
/// at least `far` wide covering the disc of `far` about the eye, each
/// split while its texels are coarser than `supersample × detail` a
/// screen pixel at its nearest point, inside the view (widened by a
/// margin) and within `far`. Past `capacity`, the texels coarsen until
/// it fits.
pub fn wanted_sections(escape: &EscapeConfig, out_w: u32, out_h: u32, capacity: usize, anchor_zoom: f64, eye: [f64; 2]) -> Wanted {
    let t = &escape.terrain;
    let z = (escape.zoom_log2 - anchor_zoom).exp2();
    let cam = terrain_camera(escape);
    let far = t.far.max(0.1) as f64;
    let top = t.height as f64 * escape.terrain_scale();
    let aspect = out_w.max(1) as f64 / out_h.max(1) as f64;
    let th = (cam.fov as f64 * 0.5).tan();
    let per_pixel = 2.0 * th / out_h.max(1) as f64;
    let rho = (escape.supersample.max(1) as f64 * t.detail.clamp(0.1, 8.0) as f64).max(0.1);
    // The view's side planes, widened a fifth: inward normals.
    let margin = 1.2;
    let corner = |u: f64, v: f64| -> [f64; 3] {
        std::array::from_fn(|k| cam.forward[k] + cam.right[k] * u * aspect * 2.0 * th * margin - cam.up[k] * v * 2.0 * th * margin)
    };
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cs = [corner(-0.5, -0.5), corner(0.5, -0.5), corner(0.5, 0.5), corner(-0.5, 0.5)];
    let planes: Vec<[f64; 3]> = (0..4)
        .map(|k| {
            let n = cross(cs[k], cs[(k + 1) % 4]);
            if dot(n, cam.forward) < 0.0 {
                [-n[0], -n[1], -n[2]]
            } else {
                n
            }
        })
        .collect();
    let eye_z = cam.eye[2];
    // A node's box in the world (current widths, origin the eye's ground
    // point): its nearest distance from the eye, and whether it is in
    // view -- inside the widened frustum, and nearer than the walk's
    // clip, `far` deep along the view (not `far` away: the clip's corners
    // reach further, and the ground there was drawn from the roots).
    let geometry = |level: i32, i: i64, j: i64| -> (f64, bool) {
        let side = (level as f64).exp2();
        let (x0, y0) = ((i as f64 * side - eye[0]) * z, (j as f64 * side - eye[1]) * z);
        let s = side * z;
        let dx = 0.0f64.clamp(x0, x0 + s);
        let dy = 0.0f64.clamp(y0, y0 + s);
        let dz = if eye_z > top { eye_z - top } else { 0.0 };
        let near = (dx * dx + dy * dy + dz * dz).sqrt();
        // The box's least depth: depth is linear, so at a corner.
        let depth = [(x0, y0), (x0 + s, y0), (x0, y0 + s), (x0 + s, y0 + s)]
            .into_iter()
            .flat_map(|(x, y)| [[x, y, -eye_z], [x, y, top - eye_z]])
            .map(|p| dot(p, cam.forward))
            .fold(f64::INFINITY, f64::min);
        let mut inside = true;
        for n in &planes {
            let mut any = false;
            for (cx, cy, cz) in [(x0, y0, 0.0), (x0 + s, y0, 0.0), (x0, y0 + s, 0.0), (x0 + s, y0 + s, 0.0)]
                .into_iter()
                .flat_map(|(x, y, _)| [(x, y, 0.0), (x, y, top)])
            {
                if dot(*n, [cx, cy, cz - eye_z]) >= 0.0 {
                    any = true;
                    break;
                }
            }
            if !any {
                inside = false;
                break;
            }
        }
        (near, inside && depth <= far)
    };
    // The roots: the coarsest level at least `far` wide, about the eye.
    let root_level = (far / z).log2().ceil() as i32;
    let rs = (root_level as f64).exp2();
    let far_a = far / z;
    let lo = [((eye[0] - far_a) / rs).floor() as i64, ((eye[1] - far_a) / rs).floor() as i64];
    let hi = [((eye[0] + far_a) / rs).floor() as i64, ((eye[1] + far_a) / rs).floor() as i64];
    let root_dims = [(hi[0] - lo[0] + 1) as u32, (hi[1] - lo[1] + 1) as u32];
    let finest = ((FINEST_REL / z).log2().floor() as i32).min(root_level);
    let mut coarsen = 1.0f64;
    loop {
        let mut out: Vec<(SectionKey, f64, bool)> = Vec::new();
        let mut hidden_roots = std::collections::HashSet::new();
        let mut stack: Vec<(i32, i64, i64)> = Vec::new();
        for j in lo[1]..=hi[1] {
            for i in lo[0]..=hi[0] {
                let (near, _) = geometry(root_level, i, j);
                out.push(((root_level, i, j), near, true));
                stack.push((root_level, i, j));
            }
        }
        while let Some((level, i, j)) = stack.pop() {
            let (near, inside) = geometry(level, i, j);
            if !inside {
                if level == root_level {
                    hidden_roots.insert((level, i, j));
                }
                continue;
            }
            let texel = (level as f64).exp2() * z / SECTION_CELLS as f64;
            let need = near.max(1.0e-9) * per_pixel / rho * coarsen;
            if texel > need && level > finest {
                if level == root_level {
                    hidden_roots.insert((level, i, j));
                }
                for (di, dj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    stack.push((level - 1, i * 2 + di, j * 2 + dj));
                }
            } else if level < root_level {
                out.push(((level, i, j), near, false));
            }
        }
        if out.len() <= capacity || coarsen > 1.0e6 {
            // Roots first, then nearest first.
            out.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.total_cmp(&b.1)));
            return Wanted { root_level, root_first: lo, root_dims, sections: out, hidden_roots };
        }
        coarsen *= 1.5;
    }
}

/// A cached section: its atlas layer, its samples a side, and the frame
/// it was last wanted.
struct Cached {
    layer: u32,
    samples: u32,
    wanted: u64,
}

/// A section being rendered, at `samples` a side.
struct Building {
    key: SectionKey,
    layer: u32,
    samples: u32,
    started: web_time::Instant,
}

/// The escape terrain: the sections' renderer, the terrain renderer, and
/// the sections, kept by their place in the plane.
pub struct EscapeTerrain {
    footprint: EscapeRenderer,
    terrain: TerrainRenderer,
    anchor: Option<Anchor>,
    /// What the sections were drawn from: the picture and the palette.
    picture: Option<(EscapeConfig, String)>,
    cache: HashMap<SectionKey, Cached>,
    free: Vec<u32>,
    capacity: u32,
    building: Option<Building>,
    frame: u64,
    /// What the ground was last set from: the camera relative to the
    /// anchor, the zoom, the sections.
    ground_key: Option<String>,
    /// The sections the view wants and does not have.
    missing: usize,
    /// Sections rendered, for the cache's gates.
    pub footprint_renders: u32,
    /// A section's time from its first chunk to its GPU completion, in
    /// ms, written by the queue's completion callback, and its average.
    section_done: std::sync::Arc<std::sync::Mutex<Option<f32>>>,
    section_ms: Option<f32>,
    /// The tiers: the lit tier's grid and the path tracer.
    tiers: TerrainTiers,
    /// Auto contrast's one fit for every section is measured and pinned
    /// (`contrast_probe_config`).
    contrast_pinned: bool,
    /// A deep picture's hidden roots' samples a side ([`COARSE_SAMPLES`];
    /// a test sets a section's to compare).
    pub(crate) coarse_samples: u32,
    /// The most sections the view may want: [`MAX_SECTIONS`], or what an
    /// export fitted ([`fit_export_sections`]).
    max_sections: u32,
    /// Whether an export has fitted `max_sections` to the card.
    sections_fitted: bool,
    /// Whether the sections' max mipmaps bound a smooth surface's spline
    /// (`TerrainRenderer::reshape_section`): the config's Smooth surface
    /// when they were made, every one re-bounded when it changes.
    mips_smooth: bool,
}

pub use super::path_core::PATH_SHOW_SAMPLES;


impl EscapeTerrain {
    pub fn new(device: &Device, out_w: u32, out_h: u32) -> Self {
        EscapeTerrain {
            footprint: EscapeRenderer::new(device, SECTION_SAMPLES, SECTION_SAMPLES),
            terrain: TerrainRenderer::new(device, out_w, out_h),
            anchor: None,
            picture: None,
            cache: HashMap::new(),
            free: Vec::new(),
            capacity: 0,
            building: None,
            frame: 0,
            ground_key: None,
            missing: 0,
            footprint_renders: 0,
            section_done: std::sync::Arc::new(std::sync::Mutex::new(None)),
            section_ms: None,
            tiers: TerrainTiers::default(),
            contrast_pinned: false,
            coarse_samples: COARSE_SAMPLES,
            max_sections: MAX_SECTIONS,
            sections_fitted: false,
            mips_smooth: false,
        }
    }

    /// Why a terrain of this config cannot be held at `w x h`: a
    /// section's escape render, the terrain's own buffers, or the atlas.
    pub fn allocation_error(device: &Device, escape: &EscapeConfig, w: u32, h: u32) -> Option<String> {
        let probe = Anchor { re: escape.center_re.clone(), im: escape.center_im.clone(), zoom: escape.zoom_log2 };
        let section = section_config(&footprint_config(escape), &probe, (0, 0, 0), SECTION_SAMPLES);
        EscapeRenderer::allocation_error(device, &section, SECTION_SAMPLES, SECTION_SAMPLES, 1)
            .or_else(|| TerrainRenderer::allocation_error(device, w, h))
            .or_else(|| {
                let lim = device.limits();
                (lim.max_texture_array_layers < MAX_SECTIONS).then(|| {
                    format!("the terrain wants {MAX_SECTIONS} texture layers; this device has {}", lim.max_texture_array_layers)
                })
            })
    }

    pub fn resize(&mut self, device: &Device, out_w: u32, out_h: u32) {
        // The accumulation and the path tracer's sum go with a new size.
        // Only then: the app calls this every frame, and asking whether
        // the LIT accumulation was empty instead restarted the Path Traced
        // tier -- which never fills it -- every frame, on its first sample.
        if self.terrain.resize(device, out_w, out_h) {
            self.tiers.invalidate();
        }
    }

    /// The sections' renderer, for the caller to give it what any escape
    /// render needs first (a 2D IFS's analysis, a texture's image).
    pub fn footprint_renderer(&mut self) -> &mut EscapeRenderer {
        &mut self.footprint
    }

    /// Forget every section: something they were drawn with that their
    /// key does not see (a texture's image arriving) changed.
    pub fn invalidate_footprint(&mut self) {
        self.picture = None;
    }

    /// The sections the view wants and does not have yet.
    pub fn sections_missing(&self) -> usize {
        self.missing + usize::from(self.building.is_some() && self.missing == 0)
    }

    /// Whether a section is part-way through its render.
    pub fn footprint_in_progress(&self) -> bool {
        self.building.is_some()
    }

    /// The average section's render time, in ms, once one has been
    /// measured.
    pub fn section_ms(&mut self) -> Option<f32> {
        if let Some(ms) = self.section_done.lock().ok().and_then(|mut g| g.take()) {
            self.section_ms = Some(match self.section_ms {
                Some(avg) => avg + (ms - avg) * 0.3,
                None => ms,
            });
        }
        self.section_ms
    }

    /// Drop every section, and free the atlas's layers.
    fn clear(&mut self) {
        self.cache.clear();
        self.building = None;
        self.free = (0..self.capacity).rev().collect();
        self.ground_key = None;
    }

    /// One frame's work: the sections the view wants, up to `steps`
    /// chunks of rendering the missing ones (roots first, then nearest
    /// first), and the ground set from what is ready. True while
    /// sections are still missing. Submits its own work.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        device: &Device,
        queue: &Queue,
        config: &FractalConfig,
        out_w: u32,
        out_h: u32,
        palette_view: &TextureView,
        palette_generation: u64,
        steps: u32,
    ) -> bool {
        self.frame += 1;
        self.terrain.poll_range(device, queue);
        let escape = &config.escape;
        // A new picture: every section goes.
        let picture = (picture_key(escape), palette_key(config));
        if self.picture.as_ref() != Some(&picture) {
            self.picture = Some(picture);
            self.clear();
            self.terrain.reset_range(queue);
            self.unpin_contrast();
        }
        // Out of the anchor's band or lattice cell: the sections start
        // over on the new one.
        let canonical = canonical_anchor(escape);
        if self.anchor.as_ref() != Some(&canonical) {
            self.anchor = Some(canonical);
            self.clear();
            self.unpin_contrast();
        }
        let anchor = self.anchor.clone().expect("set above");
        let (eye, z) = eye_in_anchor(escape, &anchor);
        let wanted = wanted_sections(escape, out_w, out_h, self.max_sections as usize, anchor.zoom, eye);
        // The atlas holds what the view wants and a quarter more, growing
        // in steps of eight to `max_sections` -- each section is about
        // 21 MB, so a small view does not hold a large one's memory.
        // Growing re-makes it, and the sections start over.
        let need = ((wanted.sections.len() as u32 * 5 / 4).div_ceil(8) * 8).clamp(8, self.max_sections);
        if need > self.capacity || self.terrain.atlas_shape().is_none() {
            let capacity = need.max(self.capacity);
            self.terrain.ensure_atlas(device, SECTION_SAMPLES, SECTION_SAMPLES, capacity);
            self.capacity = capacity;
            self.clear();
        }
        for (key, _, _) in &wanted.sections {
            if let Some(c) = self.cache.get_mut(key) {
                c.wanted = self.frame;
            }
        }
        let fp = footprint_config(escape);
        // A hidden root of a deep picture at a sixteenth of the samples
        // (`COARSE_SAMPLES`); a direct one costs too little to bother.
        let coarse = self.coarse_samples;
        let samples = |key: &SectionKey| -> u32 {
            if wanted.hidden_roots.contains(key) && EscapeRenderer::wants_perturbation(&section_config(&fp, &anchor, *key, coarse)) {
                coarse
            } else {
                SECTION_SAMPLES
            }
        };
        // Missing: not cached, or cached coarser than now wanted (a
        // hidden root come into view), which stands in until it lands.
        let missing: Vec<(SectionKey, u32)> = wanted
            .sections
            .iter()
            .map(|w| (w.0, samples(&w.0)))
            .filter(|(k, n)| self.cache.get(k).is_none_or(|c| c.samples < *n))
            .collect();
        self.missing = missing.len();
        // A section no longer wanted, or no longer at its samples, is not
        // finished.
        if self.building.as_ref().is_some_and(|b| !missing.contains(&(b.key, b.samples))) {
            if let Some(b) = self.building.take() {
                self.free.push(b.layer);
            }
        }
        let derivative = self.footprint.derivative_active(&section_config(&fp, &anchor, (0, 0, 0), SECTION_SAMPLES));
        let ingest = terrain_ingest(config, derivative);
        let filter = section_filter(&escape.terrain);
        // The smooth surface turned on or off: every section bounded again
        // for it, nothing re-rendered.
        if escape.terrain.smooth != self.mips_smooth {
            self.mips_smooth = escape.terrain.smooth;
            let layers: Vec<u32> = self.cache.values().map(|c| c.layer).collect();
            for layer in layers {
                self.terrain.reshape_section(device, queue, layer, &ingest, self.mips_smooth);
            }
        }
        let mut landed = false;
        // Auto contrast: one fit for every section, measured first from a
        // view of the anchor's own (`contrast_probe_config`) and held.
        if escape.contrast.is_active() && !self.contrast_pinned {
            let probe = contrast_probe_config(&fp);
            self.footprint.resize(device, COARSE_SAMPLES, COARSE_SAMPLES, 1);
            let mut settled = false;
            for _ in 0..steps.max(1) {
                let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Contrast Probe") });
                settled = self.footprint.render(device, queue, &mut enc, &probe, palette_view, palette_generation);
                queue.submit(std::iter::once(enc.finish()));
                if settled {
                    break;
                }
            }
            if !settled {
                self.missing = wanted.sections.len();
                self.set_ground(device, queue, config, &wanted, eye, z, false, &ingest);
                return true;
            }
            self.footprint.pin_contrast();
            self.contrast_pinned = true;
        } else if !escape.contrast.is_active() && self.contrast_pinned {
            self.unpin_contrast();
        }
        let current = self.building.as_ref().map(|b| b.key);
        let mut todo = missing.iter().copied().filter(move |(k, _)| Some(*k) != current);
        for _ in 0..steps.max(1) {
            if self.building.is_none() {
                let Some((key, samples)) = todo.next() else { break };
                // At the atlas's capacity, a coarse copy being replaced
                // gives up its own layer rather than wait for one.
                let Some(layer) = self.free.pop().or_else(|| self.evict()).or_else(|| self.cache.remove(&key).map(|c| c.layer)) else { break };
                self.building = Some(Building { key, layer, samples, started: web_time::Instant::now() });
            }
            let b = self.building.as_ref().expect("made above");
            let (key, layer, n) = (b.key, b.layer, b.samples);
            // With the filter's border about it, at its samples a texel
            // (H4).
            let section = bordered_section_config(&fp, &anchor, key, n, filter.apron);
            let across = n + 2 * filter.apron;
            // Resizing keeps the renderer's pipelines and orbit, and is a
            // no-op at the size it has.
            self.footprint.resize(device, across, across, filter.samples);
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Section") });
            let settled = self.footprint.render(device, queue, &mut enc, &section, palette_view, palette_generation);
            queue.submit(std::iter::once(enc.finish()));
            if !settled {
                continue;
            }
            // The samples the render could afford (a factor its budget
            // clamped), the render's own colour beside its height field.
            let filter = SectionFilter { samples: self.footprint.effective_supersample(), ..filter };
            self.terrain.begin_section(queue, layer, n, n, &ingest, filter);
            self.terrain.ingest_region(device, queue, self.footprint.render_view(), self.footprint.height_view(), 0, 0, n, n);
            self.terrain.finish_section(device, queue, self.mips_smooth);
            let b = self.building.take().expect("made above");
            // A coarser copy it replaces gives its layer back.
            if let Some(old) = self.cache.insert(key, Cached { layer, samples: n, wanted: self.frame }) {
                self.free.push(old.layer);
            }
            self.footprint_renders += 1;
            self.missing = self.missing.saturating_sub(1);
            landed = true;
            let slot = std::sync::Arc::clone(&self.section_done);
            let t0 = b.started;
            queue.on_submitted_work_done(move || {
                if let Ok(mut g) = slot.lock() {
                    *g = Some(t0.elapsed().as_secs_f32() * 1000.0);
                }
            });
        }
        // Every section in: the section renderer's per-pixel memory goes
        // back until the next one is wanted -- ~150 bytes a render pixel,
        // 1.5 GB at a 3x3 prefilter, which an export's still and a
        // background export's own terrain need more.
        if self.missing == 0 && self.building.is_none() && self.footprint.holds_pixels() {
            self.footprint.release_pixels(device);
        }
        self.set_ground(device, queue, config, &wanted, eye, z, landed, &ingest);
        self.missing > 0 || self.building.is_some()
    }

    /// Measure auto contrast's fit again (`contrast_probe_config`).
    fn unpin_contrast(&mut self) {
        if self.contrast_pinned {
            self.footprint.unpin_contrast();
            self.contrast_pinned = false;
        }
    }

    /// A layer from the section wanted longest ago that the view does
    /// not want now.
    fn evict(&mut self) -> Option<u32> {
        let frame = self.frame;
        let victim = self.cache.iter().filter(|(_, c)| c.wanted < frame).min_by_key(|(_, c)| c.wanted).map(|(k, _)| *k)?;
        self.cache.remove(&victim).map(|c| c.layer)
    }

    /// The ground from the ready sections: every cached section under
    /// the roots, placed in the world, and the quadtree that finds the
    /// finest of them. Set only when the camera or the sections changed,
    /// so a still view accumulates.
    #[allow(clippy::too_many_arguments)]
    fn set_ground(
        &mut self,
        device: &Device,
        queue: &Queue,
        config: &FractalConfig,
        wanted: &Wanted,
        eye: [f64; 2],
        z: f64,
        landed: bool,
        ingest: &TerrainIngest,
    ) {
        let complete = wanted.sections.iter().all(|w| self.cache.contains_key(&w.0));
        // The relief's height is in the key: the ground's slab top is set
        // here, and the walk clips every ray to it. Out of the key, a
        // RAISED height drew the terrain cut flat at the old one until
        // something else -- a section landing, the camera moving -- set
        // the ground again (field-reported: the Height slider "does not
        // always update"; it depended on what landed). A locked relief's
        // scale too (H3), which a lock at the current zoom changes.
        let scale = config.escape.terrain_scale();
        let key = format!(
            "{eye:?}|{z}|{:?}|{:?}|{complete}|{}|{}",
            wanted.root_first,
            wanted.root_dims,
            config.escape.terrain.height.to_bits(),
            scale.to_bits()
        ) + if self.mips_smooth { "|smooth" } else { "" };
        if !landed && self.ground_key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.ground_key = Some(key);
        let rl = wanted.root_level;
        let (nx, ny) = (wanted.root_dims[0] as i64, wanted.root_dims[1] as i64);
        let mut nodes: Vec<GroundNode> = vec![GroundNode { children: [-1; 4], section: -1 }; (nx * ny) as usize];
        let mut sections: Vec<GroundSection> = Vec::new();
        // Exactly the wanted sections once they are all in, so the picture
        // is the config's and not the cache's history; while some are
        // missing, whatever else is cached stands in for them.
        let complete = wanted.sections.iter().all(|w| self.cache.contains_key(&w.0));
        let wanted_keys: std::collections::HashSet<SectionKey> = wanted.sections.iter().map(|w| w.0).collect();
        let mut keys: Vec<(&SectionKey, &Cached)> =
            self.cache.iter().filter(|(k, _)| !complete || wanted_keys.contains(k)).collect();
        // Coarse first, so a node's section is set before its children
        // are made; the order is otherwise immaterial.
        keys.sort_by_key(|(k, _)| std::cmp::Reverse(k.0));
        for (&(level, i, j), c) in keys {
            if level > rl {
                continue;
            }
            let shift = (rl - level) as u32;
            if shift > 62 {
                continue;
            }
            let (ri, rj) = (i >> shift, j >> shift);
            let (gx, gy) = (ri - wanted.root_first[0], rj - wanted.root_first[1]);
            if gx < 0 || gy < 0 || gx >= nx || gy >= ny {
                continue;
            }
            let mut node = (gy * nx + gx) as usize;
            for l in (level..rl).rev() {
                let s = (l - level) as u32;
                let q = (((j >> s) & 1) * 2 + ((i >> s) & 1)) as usize;
                let child = nodes[node].children[q];
                node = if child >= 0 {
                    child as usize
                } else {
                    nodes.push(GroundNode { children: [-1; 4], section: -1 });
                    let new = nodes.len() - 1;
                    nodes[node].children[q] = new as i32;
                    new
                };
            }
            let side = (level as f64).exp2();
            sections.push(GroundSection {
                layer: c.layer,
                origin: [(i as f64 * side - eye[0]) * z, (j as f64 * side - eye[1]) * z],
                texel: side * z / (c.samples - 1) as f64,
                n: c.samples,
                m: c.samples,
            });
            nodes[node].section = (sections.len() - 1) as i32;
        }
        let rs = (rl as f64).exp2();
        let t = &config.escape.terrain;
        let ground = Ground {
            repeat: false,
            // The samples' spline (H5), its sections bounded for it.
            smooth: self.mips_smooth,
            root_origin: [
                (wanted.root_first[0] as f64 * rs - eye[0]) * z,
                (wanted.root_first[1] as f64 * rs - eye[1]) * z,
            ],
            root_side: rs * z,
            root_dims: wanted.root_dims,
            nodes,
            sections,
            top: t.height as f64 * scale,
            floor: hole_floor(t.height) * scale,
            mode: ingest.mode(),
        };
        // A count's or a relief's range: the ground's sections' own, so
        // the heights are this ground's and not what was panned past or
        // zoomed through before (the doc's "the picture is the config's").
        let layers: Vec<u32> = ground.sections.iter().map(|s| s.layer).collect();
        self.terrain.reduce_range(device, queue, &layers);
        // ...read back, so a lock can pin it (H3).
        self.terrain.read_range_back(device, queue);
        self.terrain.set_ground(device, queue, ground);
    }

    /// One frame of the viewport (plan section 8's tiers). The lit tier:
    /// a sample of the config's antialiasing grid (`supersample²`
    /// jittered renders, the export's own) folded into its
    /// accumulation. Path traced: as many samples as fit the frame,
    /// added to the path tracer's sum, up to `samples`. Auto draws the
    /// lit tier while anything moves and path traces while nothing does,
    /// showing it from [`PATH_SHOW_SAMPLES`]. Any change to the view or
    /// the ground restarts both. True while there is more to do.
    pub fn render_viewport(&mut self, device: &Device, queue: &Queue, config: &FractalConfig) -> bool {
        let view = |jitter| terrain_view(config, jitter);
        let inputs = TierInputs { filling: self.sections_missing() > 0, ..tier_inputs(config, &view) };
        self.tiers.viewport(&mut self.terrain, device, queue, &inputs)
    }

    /// The most sections the view may want.
    pub fn max_sections(&self) -> u32 {
        self.max_sections
    }

    /// Fit an export's section cap to the card, once
    /// ([`fit_export_sections`]): the most it can hold beside the rest
    /// of the export of `config` at `w x h`. Later calls keep it -- a
    /// video's frames share one terrain.
    pub async fn fit_sections_for_export(&mut self, device: &Device, config: &FractalConfig, w: u32, h: u32) -> u32 {
        if !self.sections_fitted {
            self.max_sections = fit_export_sections(device, config, w, h).await;
            self.sections_fitted = true;
        }
        self.max_sections
    }

    /// The ground's count or relief range as last measured: what a lock
    /// pins (docs/projects/terrain-height.md, H3).
    pub fn measured_range(&self) -> Option<[f32; 2]> {
        self.terrain.measured_range()
    }

    /// A path-traced sample's average time, in ms, once measured.
    pub fn path_ms(&mut self) -> Option<f32> {
        self.tiers.path_ms()
    }

    /// The export's picture (plan section 8): path traced at `samples`
    /// unless the tier is Lit, which draws the antialiasing grid. In
    /// dispatches of a few samples each, `wait` called between them (a
    /// blocking poll on the desktop). Submits its own work.
    pub fn render_still(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, frame: (u32, u32), wait: impl FnMut(f32) -> bool) {
        let view = |jitter| terrain_view(config, jitter);
        self.tiers.still(&mut self.terrain, device, queue, &tier_inputs(config, &view), frame, wait);
    }

    /// Draw the terrain once, its rays offset by `jitter` within their
    /// pixels. Submits its own work.
    pub fn render(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, jitter: [f32; 2]) {
        self.terrain.render(device, queue, &terrain_view(config, jitter));
    }

    pub fn reset_accumulation(&mut self) {
        self.terrain.reset_accumulation();
    }

    pub fn accumulate(&mut self, device: &Device, queue: &Queue) {
        self.terrain.accumulate(device, queue);
    }

    pub fn accumulated_samples(&self) -> u32 {
        self.terrain.accumulated_samples()
    }

    /// What the tail reads: the path tracer's resolve when it is shown,
    /// else the lit tier's accumulation once there is one, else the last
    /// render.
    pub fn output_view(&self) -> &TextureView {
        self.tiers.output_view(&self.terrain)
    }

    /// Path-traced samples so far, and whether the picture is theirs.
    pub fn path_progress(&self) -> (u32, bool) {
        self.tiers.path_progress(&self.terrain)
    }

    /// The terrain renderer, for a test to look inside.
    #[cfg(test)]
    pub(crate) fn terrain_for_test(&self) -> &TerrainRenderer {
        &self.terrain
    }

    /// The measured range once the last read of it has landed.
    #[cfg(test)]
    pub(crate) fn settled_range_for_test(&mut self, device: &Device, queue: &Queue) -> Option<[f32; 2]> {
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        self.terrain.poll_range(device, queue);
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        self.terrain.poll_range(device, queue);
        self.terrain.measured_range()
    }

    /// The cached sections' keys, for a test.
    #[cfg(test)]
    pub(crate) fn cached_samples_for_test(&self) -> Vec<(SectionKey, u32)> {
        let mut v: Vec<(SectionKey, u32)> = self.cache.iter().map(|(k, c)| (*k, c.samples)).collect();
        v.sort();
        v
    }

    pub(crate) fn cached_for_test(&self) -> Vec<SectionKey> {
        let mut k: Vec<SectionKey> = self.cache.keys().copied().collect();
        k.sort();
        k
    }

    /// A cached section's atlas layer and samples a side, for a test.
    #[cfg(test)]
    pub(crate) fn layer_for_test(&self, key: SectionKey) -> Option<(u32, u32)> {
        self.cache.get(&key).map(|c| (c.layer, c.samples))
    }

    /// Free the GPU memory now: dropping frees nothing on WebGPU.
    pub fn destroy(&self) {
        self.footprint.destroy();
        self.terrain.destroy();
        self.tiers.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device, and the turn of a test that renders a terrain: one at a
    /// time, since each holds an atlas of sections -- hundreds of MB --
    /// and the suite runs its tests in parallel on one card (two at once
    /// ran a 6 GB card out of memory).
    fn gpu() -> Option<(std::sync::MutexGuard<'static, ()>, Device, Queue)> {
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let (device, queue) = crate::escape::terrain::gpu_tests::device()?;
        Some((turn, device, queue))
    }

    /// Where a world point lands on the screen, as `ifs_ray` spreads
    /// the rays: normalised offsets from the centre, x right, y DOWN.
    fn project(cam: &SolidCamera, p: [f64; 3], aspect: f64) -> (f64, f64) {
        let d = [p[0] - cam.eye[0], p[1] - cam.eye[1], p[2] - cam.eye[2]];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let z = dot(d, cam.forward);
        let th = (cam.fov as f64 * 0.5).tan();
        (dot(d, cam.right) / z / (2.0 * th * aspect), -dot(d, cam.up) / z / (2.0 * th))
    }

    /// The camera's conventions: the eye at the world's origin above the
    /// terrain's top, the target at the screen's centre, yaw 0 looking up
    /// the 2D picture with its right to the right -- and the View's
    /// rotation turning that heading as it turns the picture.
    #[test]
    fn the_terrain_camera_looks_up_the_picture() {
        let mut esc = EscapeConfig::default();
        esc.terrain.enabled = true;
        let aspect = 16.0 / 9.0;
        let cam = terrain_camera(&esc);
        assert_eq!((cam.eye[0], cam.eye[1]), (0.0, 0.0), "the world's origin is the eye's ground point");
        assert!(cam.eye[2] > esc.terrain.height as f64, "the eye is above the terrain");
        let (cx, cy) = project(&cam, cam.target, aspect);
        assert!(cx.abs() < 1e-9 && cy.abs() < 1e-9, "the target is centred: {cx} {cy}");
        let along = |cam: &SolidCamera, v: [f64; 2]| project(cam, [cam.target[0] + v[0], cam.target[1] + v[1], cam.target[2]], aspect);
        let (_, ny) = along(&cam, [0.0, 0.1]);
        let (ex, ey) = along(&cam, [0.1, 0.0]);
        assert!(ny < 0.0, "Im up the picture is up the screen: {ny}");
        assert!(ex > 0.0 && ey.abs() < 1e-9, "Re to the right: {ex} {ey}");
        // A quarter turn of the picture: its up is the plane's -Re.
        esc.rotation = std::f32::consts::FRAC_PI_2;
        let cam = terrain_camera(&esc);
        let (ux, uy) = along(&cam, [-0.1, 0.0]);
        assert!(ux.abs() < 1e-6 && uy < 0.0, "the picture's up is still the screen's: {ux} {uy}");
    }

    /// A lifted target carries the eye with it, straight up: the frame,
    /// the distance and the eye's ground point are the camera's at rest,
    /// so a lift of 0 is every terrain's camera as it was.
    #[test]
    fn a_lifted_target_carries_the_eye() {
        let mut esc = EscapeConfig::default();
        esc.terrain.enabled = true;
        let rest = terrain_camera(&esc);
        esc.terrain.target_lift = 0.75;
        let lifted = terrain_camera(&esc);
        assert_eq!((lifted.forward, lifted.right, lifted.up), (rest.forward, rest.right, rest.up));
        assert_eq!((lifted.eye_rel, lifted.distance), (rest.eye_rel, rest.distance));
        assert_eq!((lifted.eye[0], lifted.eye[1]), (rest.eye[0], rest.eye[1]));
        assert!((lifted.eye[2] - rest.eye[2] - 0.75).abs() < 1e-12);
        assert!((lifted.target[2] - rest.target[2] - 0.75).abs() < 1e-12);
        // Looking up from there: the eye above the ground, the view
        // above the horizon.
        esc.cam_pitch = -0.3;
        let up = terrain_camera(&esc);
        assert!(up.forward[2] > 0.0 && up.eye[2] > esc.terrain.height as f64, "{:?} {:?}", up.forward, up.eye);
    }

    /// Only the picture keys the sections: the view (centre, zoom,
    /// rotation), the camera, the heights and the lights do not; the
    /// formula, the source and the interior do.
    #[test]
    fn the_picture_key_ignores_the_view() {
        let mut a = EscapeConfig::default();
        a.terrain.enabled = true;
        let mut b = a.clone();
        b.center_re = "0.25".into();
        b.center_im = "0.5".into();
        b.zoom_log2 = 7.0;
        b.rotation = 1.0;
        b.cam_pitch = 1.0;
        b.cam_yaw = 2.0;
        b.cam_fov = 0.4;
        b.terrain.height = 0.2;
        b.terrain.de_width = 0.1;
        b.terrain.shadow = 0.0;
        b.terrain.occlusion = 0.1;
        b.terrain.far = 20.0;
        b.terrain.haze = 0.0;
        b.terrain.detail = 3.0;
        b.path.samples = 17;
        b.path.denoise = true;
        b.path.sky_gradient = true;
        b.path.aperture = 0.3;
        b.solid_tier = crate::config::escape::RenderTier::PathTraced;
        b.shading.light_angle = 10.0;
        b.shading.highlight_strength = 0.9;
        assert_eq!(picture_key(&a), picture_key(&b));
        for edit in [
            |c: &mut EscapeConfig| c.max_iter += 1,
            |c: &mut EscapeConfig| c.terrain.source = TerrainSource::EscapeCount,
            |c: &mut EscapeConfig| c.terrain.interior = TerrainInterior::Hole,
            // A lake's tint is its sections' albedo: reported cached in
            // the app, the old colour until a pan brought new sections.
            |c: &mut EscapeConfig| c.terrain.lake_tint = [0.5, 0.1, 0.1],
            // The relief source's field is what a section writes.
            |c: &mut EscapeConfig| c.shading.field = crate::config::escape::ShadingField::Banded,
        ] {
            let mut c = a.clone();
            edit(&mut c);
            assert_ne!(picture_key(&a), picture_key(&c));
        }
    }

    /// The anchor is the config's: an eight-octave band and a truncated
    /// centre, so a pan or a zoom within them keeps it, and two engines
    /// agree on it.
    #[test]
    fn the_anchor_is_the_configs() {
        assert_eq!(truncate_decimal("-0.74531234", 3), "-0.745");
        assert_eq!(truncate_decimal("0.0009", 3), "0");
        assert_eq!(truncate_decimal("-0.0009", 3), "0");
        assert_eq!(truncate_decimal("123.456", -2), "100");
        assert_eq!(truncate_decimal("12.5", -2), "0");
        assert_eq!(truncate_decimal("1.2e3", 2), "0", "malformed anchors at zero");
        let mut esc = EscapeConfig::default();
        esc.center_re = "-0.743643887037158704754810829".into();
        esc.center_im = "0.131825904205311970493132056".into();
        esc.zoom_log2 = 60.3;
        let a = canonical_anchor(&esc);
        assert_eq!(a.zoom, 56.0);
        // The lattice is at least 2^12 anchor widths, at most ten times
        // that: the view is within it.
        let off = view_offset(&esc, &a);
        assert!(off[0].abs() < 10.0 * 4096.0 && off[1].abs() < 10.0 * 4096.0, "{off:?}");
        let mut b = esc.clone();
        b.zoom_log2 = 62.9;
        b.center_re = "-0.74364388703715870475".into();
        assert_eq!(canonical_anchor(&b), a, "a nearby view keeps it");
        b.zoom_log2 = 64.0;
        assert_ne!(canonical_anchor(&b), a, "a new band moves it");
    }

    /// A section's samples sit on its grid points: sample (0, 0), the
    /// picture's bottom-left, is its south-west corner; sample (S, 0) its
    /// north-west -- and the section to its east starts where it ends, so
    /// neighbours share their edges.
    #[test]
    fn section_samples_sit_on_their_grid() {
        let mut esc = EscapeConfig::default();
        esc.center_re = "-0.743".into();
        esc.center_im = "0.131".into();
        esc.zoom_log2 = 9.3;
        esc.rotation = 0.7;
        let a = Anchor { re: esc.center_re.clone(), im: esc.center_im.clone(), zoom: 9.0 };
        let width = 4.0 * (-a.zoom).exp2();
        let (are, aim) = (a.re.parse::<f64>().unwrap(), a.im.parse::<f64>().unwrap());
        // The escape view's pixel centres: x right, y down, the span the
        // render's height.
        let pixel = |c: &EscapeConfig, n: u32, px: f64, py: f64| -> (f64, f64) {
            let n = n as f64;
            let span = 4.0 * (-c.zoom_log2).exp2();
            let (cx, cy) = (c.center_re.parse::<f64>().unwrap(), c.center_im.parse::<f64>().unwrap());
            (cx + ((px + 0.5) / n - 0.5) * span, cy - ((py + 0.5) / n - 0.5) * span)
        };
        // A section's samples and a hidden root's coarse ones alike.
        for (key, n) in [((0, 0, 0), SECTION_SAMPLES), ((-3, 5, -2), SECTION_SAMPLES), ((2, -1, 1), SECTION_SAMPLES), ((2, -1, 1), COARSE_SAMPLES)] {
            let s = (n - 1) as f64;
            let c = section_config(&esc, &a, key, n);
            assert_eq!(c.rotation, 0.0);
            let side = (key.0 as f64).exp2() * width;
            let (x0, y0) = (are + key.1 as f64 * side, aim + key.2 as f64 * side);
            let sw = pixel(&c, n, 0.0, s);
            let nw = pixel(&c, n, 0.0, 0.0);
            let se = pixel(&c, n, s, s);
            let tol = side * 1e-9;
            assert!((sw.0 - x0).abs() < tol && (sw.1 - y0).abs() < tol, "{key:?} south-west {sw:?} vs {:?}", (x0, y0));
            assert!((nw.1 - (y0 + side)).abs() < tol, "{key:?} north edge");
            assert!((se.0 - (x0 + side)).abs() < tol, "{key:?} east edge");
            let east = section_config(&esc, &a, (key.0, key.1 + 1, key.2), n);
            let e_sw = pixel(&east, n, 0.0, s);
            assert!((e_sw.0 - se.0).abs() < tol && (e_sw.1 - se.1).abs() < tol, "{key:?}: the east neighbour starts where it ends");
        }
    }

    /// An export's estimate of the rest of its peak grows with what it
    /// holds: the prefilter's section renderer, a tiled still's picture,
    /// and the frame.
    #[test]
    fn an_exports_headroom_follows_what_it_holds() {
        let mut c = terrain_config();
        let small = export_headroom(&c, 1920, 1080);
        // A tiled still holds its picture beside the tile, and the tail its
        // output -- where the section renderer is smaller (they are never
        // held at once: the sections are in before the still begins).
        let tiled = export_headroom(&c, 7680, 4320);
        assert!(tiled > 7680 * 4320 * 24, "{tiled}");
        c.escape.terrain.prefilter = 3;
        let prefiltered = export_headroom(&c, 1920, 1080);
        assert!(prefiltered > small + (1 << 30), "{small} {prefiltered}");
        // Within the escape renderer's own budget, whatever the samples.
        assert!(prefiltered < (3 << 29) + 1920 * 1080 * 200);
        // A layer is ~21 MB.
        let layer = TerrainRenderer::atlas_layer_bytes(SECTION_SAMPLES, SECTION_SAMPLES);
        assert!((20 << 20..23 << 20).contains(&layer), "{layer}");
    }

    /// An export's section cap is fitted to the card: one of the halvings
    /// from [`EXPORT_MAX_SECTIONS`] to [`MAX_SECTIONS`], never past the
    /// device's layers -- and a larger export never fits more. Off a
    /// discrete card, the viewport's.
    #[test]
    fn an_exports_section_cap_fits_the_card() {
        let Some((_one_at_a_time, device, _queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let c = terrain_config();
        let cap = pollster::block_on(fit_export_sections(&device, &c, 1920, 1080));
        println!("1080p export: {cap} sections");
        if device.adapter_info().device_type != DeviceType::DiscreteGpu {
            assert_eq!(cap, MAX_SECTIONS);
        }
        assert!([64, 128, 256].contains(&cap) && cap <= device.limits().max_texture_array_layers.max(MAX_SECTIONS), "{cap}");
        let mut huge = c.clone();
        huge.escape.terrain.prefilter = 3;
        let fewer = pollster::block_on(fit_export_sections(&device, &huge, 16384, 16384));
        println!("16K export at a 3x3 prefilter: {fewer} sections");
        assert!(fewer <= cap && fewer >= MAX_SECTIONS, "{fewer} vs {cap}");
    }

    /// The smooth surface (H5) is the walk's: turned on, every section
    /// is bounded again for its spline and none is rendered again, and
    /// the ground drawn is the spline.
    #[test]
    fn the_smooth_surface_renders_no_section_again() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut c = terrain_config();
        c.escape.terrain.tier = RenderTier::Lit;
        let mut engines = crate::renderer::render::RenderEngines::default();
        let mut draw = |c: &FractalConfig, engines: &mut crate::renderer::render::RenderEngines| {
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                crate::renderer::RenderJob::new(c, 64, 48).with_engines(engines),
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
        };
        draw(&c, &mut engines);
        let made = engines.terrain.as_ref().unwrap().footprint_renders;
        assert!(made > 0);
        assert!(!engines.terrain.as_ref().unwrap().terrain_for_test().ground_for_test().unwrap().smooth);
        c.escape.terrain.smooth = true;
        draw(&c, &mut engines);
        let t = engines.terrain.as_ref().unwrap();
        assert_eq!(t.footprint_renders, made, "a section rendered again");
        assert!(t.terrain_for_test().ground_for_test().unwrap().smooth);
    }

    /// A bordered section's samples are its own and its border's (H4):
    /// sample (apron + i, apron + j) of the bordered render is sample
    /// (i, j) of the plain one, on the same grid, the border beyond.
    #[test]
    fn a_bordered_section_keeps_its_grid() {
        let mut esc = EscapeConfig::default();
        esc.center_re = "-0.743".into();
        esc.center_im = "0.131".into();
        esc.zoom_log2 = 9.3;
        let a = Anchor { re: esc.center_re.clone(), im: esc.center_im.clone(), zoom: 9.0 };
        let pixel = |c: &EscapeConfig, n: u32, px: f64, py: f64| -> (f64, f64) {
            let n = n as f64;
            let span = 4.0 * (-c.zoom_log2).exp2();
            let (cx, cy) = (c.center_re.parse::<f64>().unwrap(), c.center_im.parse::<f64>().unwrap());
            (cx + ((px + 0.5) / n - 0.5) * span, cy - ((py + 0.5) / n - 0.5) * span)
        };
        for (key, n, apron) in [((0, 0, 0), SECTION_SAMPLES, 12u32), ((-3, 5, -2), SECTION_SAMPLES, 3), ((2, -1, 1), COARSE_SAMPLES, 6)] {
            let plain = section_config(&esc, &a, key, n);
            let bordered = bordered_section_config(&esc, &a, key, n, apron);
            let side = (key.0 as f64).exp2() * 4.0 * (-a.zoom).exp2();
            let tol = side * 1e-9;
            let s = (n - 1) as f64;
            for (i, j) in [(0.0, 0.0), (s, s), (0.0, s), (17.0, 250.0)] {
                let p = pixel(&plain, n, i, j);
                let q = pixel(&bordered, n + 2 * apron, i + apron as f64, j + apron as f64);
                assert!((p.0 - q.0).abs() < tol && (p.1 - q.1).abs() < tol, "{key:?} ({i}, {j}): {p:?} vs {q:?}");
            }
            // The border reaches `apron` texels past each edge.
            let corner = pixel(&bordered, n + 2 * apron, 0.0, 0.0);
            let nw = pixel(&plain, n, 0.0, 0.0);
            let texel = side / s;
            assert!((nw.0 - corner.0 - apron as f64 * texel).abs() < tol, "{key:?} west border");
            assert!((corner.1 - nw.1 - apron as f64 * texel).abs() < tol, "{key:?} north border");
        }
    }

    /// What a view wants, against plan section 12's estimate (34
    /// sections at 8 widths, 1080p, no antialiasing): about that many;
    /// every leaf within reach, finer the nearer it is -- its texels no
    /// coarser than the screen wants at its nearest point -- and within
    /// the atlas however fine the detail.
    #[test]
    fn a_view_wants_what_the_estimate_says() {
        let mut esc = EscapeConfig::default();
        esc.terrain.enabled = true;
        let a = Anchor { re: esc.center_re.clone(), im: esc.center_im.clone(), zoom: esc.zoom_log2 };
        let (eye, z) = eye_in_anchor(&esc, &a);
        let w = wanted_sections(&esc, 1920, 1080, MAX_SECTIONS as usize, a.zoom, eye);
        let leaves: Vec<_> = w.sections.iter().filter(|s| !s.2).collect();
        let roots = w.sections.len() - leaves.len();
        println!("default view: {roots} roots, {} leaves", leaves.len());
        assert!((20..=64).contains(&w.sections.len()), "{}", w.sections.len());
        let per_pixel = 2.0 * (esc.cam_fov as f64 * 0.5).tan() / 1080.0;
        for &&((level, _, _), near, _) in &leaves {
            // Within the clip: `far` deep, so at most `far` along the
            // widened frustum's corner ray.
            let th = (esc.cam_fov as f64 * 0.5).tan() * 1.2;
            let corner = (1.0 + (th * 1920.0 / 1080.0).powi(2) + th * th).sqrt();
            assert!(near <= esc.terrain.far as f64 * corner + 1e-9, "a leaf within reach: {near}");
            let texel = (level as f64).exp2() * z / SECTION_CELLS as f64;
            assert!(texel <= near * per_pixel * 1.0001 || level <= ((FINEST_REL / z).log2().floor() as i32), "texels fine enough at {near}");
        }
        // Nearer is finer.
        let mut by_near: Vec<_> = leaves.iter().map(|s| (s.1, s.0 .0)).collect();
        by_near.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(by_near.first().unwrap().1 <= by_near.last().unwrap().1, "{by_near:?}");
        // Twice the antialiasing would want more than the atlas holds:
        // coarsened to fit.
        esc.supersample = 4;
        let w4 = wanted_sections(&esc, 1920, 1080, MAX_SECTIONS as usize, a.zoom, eye);
        assert!(w4.sections.len() <= MAX_SECTIONS as usize, "{}", w4.sections.len());
    }

    /// The leaves cover every point a primary ray can meet the ground at
    /// -- within the slab, `far` deep along the view, the walk's clip --
    /// so no pixel draws on a root that was split or is out of view -- a
    /// hidden one. Views: the default, deep, turned, low and steep, whose
    /// every root is hidden; a wide angle's and a thumbnail's, where a
    /// root in view can be coarse enough to be a leaf, and draws.
    #[test]
    fn the_leaves_cover_every_ray_the_walk_draws() {
        let deep: FractalConfig = serde_json::from_str(
            &std::fs::read_to_string("tests/visual/configs/escape/fe-zoom-60-edge.fflame").expect("config"),
        )
        .expect("parse");
        let mut views: Vec<(&str, EscapeConfig, u32, u32)> = Vec::new();
        let mut esc = EscapeConfig::default();
        esc.terrain.enabled = true;
        views.push(("default", esc.clone(), 1920, 1080));
        let mut d = deep.escape.clone();
        d.terrain.enabled = true;
        d.terrain.far = 4.0;
        views.push(("2^60", d, 1920, 1080));
        let mut v = esc.clone();
        v.cam_yaw = 2.3;
        v.rotation = 0.4;
        views.push(("turned", v, 1920, 1080));
        let mut v = esc.clone();
        v.cam_pitch = 0.12;
        v.terrain.far = 8.0;
        views.push(("low", v, 1920, 1080));
        let mut v = esc.clone();
        v.cam_pitch = 1.3;
        views.push(("steep", v, 1920, 1080));
        let mut v = esc.clone();
        v.cam_fov = 1.6;
        views.push(("wide", v, 2560, 1080));
        views.push(("thumbnail", esc.clone(), 48, 27));
        for (name, escape, w, h) in views {
            let a = canonical_anchor(&escape);
            let (eye, z) = eye_in_anchor(&escape, &a);
            let wanted = wanted_sections(&escape, w, h, MAX_SECTIONS as usize, a.zoom, eye);
            let leaves: std::collections::HashSet<SectionKey> = wanted.sections.iter().filter(|s| !s.2).map(|s| s.0).collect();
            let roots: Vec<SectionKey> = wanted.sections.iter().filter(|s| s.2).map(|s| s.0).collect();
            let finest = wanted.sections.iter().map(|s| s.0 .0).min().unwrap();
            let cam = terrain_camera(&escape);
            let th = (cam.fov as f64 * 0.5).tan();
            let (far, top) = (escape.terrain.far as f64, escape.terrain.height as f64);
            let mut drawn_roots = std::collections::HashSet::new();
            let (mut points, mut on_roots) = (0u32, 0u32);
            let step = (w / 96).max(1);
            for py in (0..h).step_by(step as usize) {
                for px in (0..w).step_by(step as usize) {
                    let (u, v) = ((px as f64 + 0.5) / w as f64 - 0.5, (py as f64 + 0.5) / h as f64 - 0.5);
                    let d: [f64; 3] = std::array::from_fn(|k| cam.forward[k] + cam.right[k] * u * w as f64 / h as f64 * 2.0 * th - cam.up[k] * v * 2.0 * th);
                    let depth = d[0] * cam.forward[0] + d[1] * cam.forward[1] + d[2] * cam.forward[2];
                    // Where the ray crosses the slab's top and floor, and
                    // between, inside the clip.
                    for zp in [top, top * 0.5, 0.0] {
                        if !(d[2] < 0.0) || cam.eye[2] <= zp {
                            continue;
                        }
                        let t = (zp - cam.eye[2]) / d[2];
                        if t * depth > far {
                            continue;
                        }
                        let (ax, ay) = (eye[0] + (cam.eye[0] + d[0] * t) / z, eye[1] + (cam.eye[1] + d[1] * t) / z);
                        let found = (finest..=wanted.root_level).find_map(|level| {
                            let side = (level as f64).exp2();
                            let key = (level, (ax / side).floor() as i64, (ay / side).floor() as i64);
                            wanted.sections.iter().any(|s| s.0 == key).then_some(key)
                        });
                        let Some(key) = found else { continue };
                        points += 1;
                        if !leaves.contains(&key) {
                            on_roots += 1;
                            drawn_roots.insert(key);
                        }
                    }
                }
            }
            let hidden: Vec<&SectionKey> = roots.iter().filter(|r| wanted.hidden_roots.contains(r)).collect();
            println!("{name}: {} sections, {} roots ({} hidden), {points} points, {on_roots} on a root", wanted.sections.len(), roots.len(), hidden.len());
            assert!(points > 100, "{name}: {points}");
            // A root draws only where no leaf is: it must be a leaf itself
            // (unsplit, in view), never a hidden one.
            for r in &drawn_roots {
                assert!(!wanted.hidden_roots.contains(r), "{name}: hidden root {r:?} draws");
            }
            if !["wide", "thumbnail"].contains(&name) {
                assert_eq!(on_roots, 0, "{name}: the leaves cover the clip");
                assert_eq!(hidden.len(), roots.len(), "{name}: every root split or out of view");
            }
        }
    }

    /// A Mandelbrot terrain config: the whole set about the view, a
    /// Linear tonemap that passes colour through, a sky-blue background.
    fn terrain_config() -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.exposure = 1.0;
        c.gamma = 1.0;
        c.levels_enabled = false;
        c.use_curve = false;
        c.background_color = [0.55, 0.65, 0.8];
        c.escape.center_re = "-0.75".into();
        c.escape.center_im = "0.0".into();
        c.escape.zoom_log2 = 1.0;
        c.escape.max_iter = 300;
        c.escape.terrain.enabled = true;
        c.escape.terrain.far = 4.0;
        c
    }

    fn render(device: &Device, queue: &Queue, c: &FractalConfig, w: u32, h: u32) -> Vec<u8> {
        pollster::block_on(crate::renderer::render(
            device,
            queue,
            crate::renderer::RenderJob::new(c, w, h),
            &mut crate::renderer::NoProgress,
        ))
        .expect("render")
        .rgba_data
    }

    /// A terrain renders through `render_with`, so the CLI, thumbnails
    /// and video have it: the background above the horizon, lit ground
    /// at the centre, the same bytes every time, and not the 2D picture.
    #[test]
    fn a_terrain_renders_through_render_with() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (160u32, 120u32);
        let c = terrain_config();
        let a = render(&device, &queue, &c, w, h);
        let px = |x: u32, y: u32| {
            let k = ((y * w + x) * 4) as usize;
            [a[k], a[k + 1], a[k + 2]]
        };
        let sky = [0.55f32, 0.65, 0.8].map(|v| (v * 255.0).round() as i32);
        let is_sky = |p: [u8; 3]| (0..3).all(|i| (p[i] as i32 - sky[i]).abs() <= 2);
        let top = (0..w).filter(|&x| is_sky(px(x, 0))).count();
        assert_eq!(top, w as usize, "the top row is the background");
        let centre = px(w / 2, h / 2);
        assert!(!is_sky(centre), "the centre is ground: {centre:?}");
        assert!(centre.iter().any(|&v| v > 8), "and lit: {centre:?}");
        assert_eq!(a, render(&device, &queue, &c, w, h), "the same config renders the same bytes");
        let mut flat = c.clone();
        flat.escape.terrain.enabled = false;
        assert_ne!(a, render(&device, &queue, &flat, w, h), "the terrain is not the 2D picture");
    }

    /// The ground has no holes: every ray that points below the horizon
    /// and meets the ground's plane well inside its reach hits ground --
    /// none falls through a seam between sections of different detail.
    #[test]
    fn no_ray_falls_through_the_ground() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (192u32, 108u32);
        let mut c = terrain_config();
        c.escape.zoom_log2 = 6.0;
        c.escape.center_re = "-0.7453".into();
        c.escape.center_im = "0.1127".into();
        // The lit tier: its walk is what fills the geometry record read
        // here (the path tracer's rays walk the same ground).
        c.escape.terrain.tier = RenderTier::Lit;
        let mut engines = crate::renderer::render::RenderEngines::default();
        let _ = pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            crate::renderer::RenderJob::new(&c, w, h).with_engines(&mut engines),
            &mut crate::renderer::NoProgress,
        ))
        .expect("render");
        let t = engines.terrain.as_ref().unwrap();
        let geom: Vec<[u32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(
            &device,
            &queue,
            t.terrain_for_test().geometry_buffer(),
            (w * h * 16) as u64,
        ))
        .to_vec();
        let cam = terrain_camera(&c.escape);
        let (aspect, th) = (w as f64 / h as f64, (cam.fov as f64 * 0.5).tan());
        let (mut checked, mut holes) = (0usize, 0usize);
        for py in 0..h {
            for px in 0..w {
                let (u, v) = ((px as f64 + 0.5) / w as f64 - 0.5, (py as f64 + 0.5) / h as f64 - 0.5);
                let d: [f64; 3] =
                    std::array::from_fn(|k| cam.forward[k] + cam.right[k] * u * aspect * 2.0 * th - cam.up[k] * v * 2.0 * th);
                if d[2] >= 0.0 {
                    continue;
                }
                // Where it meets the plane z = 0, against the reach.
                let s = -cam.eye[2] / d[2];
                let dist = s * (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if dist > 0.6 * c.escape.terrain.far as f64 {
                    continue;
                }
                checked += 1;
                if !(f32::from_bits(geom[(py * w + px) as usize][3]) > 0.0) {
                    holes += 1;
                }
            }
        }
        println!("{checked} rays under the horizon, {holes} through the ground, {} sections", t.cached_for_test().len());
        assert!(checked > (w * h / 3) as usize, "{checked}");
        assert_eq!(holes, 0, "rays fell through the ground");
    }

    /// A camera lifted above the ground can look up (camera-unification
    /// C4): the sky across the top of the picture, the ground along the
    /// bottom, and no ray under the horizon within reach falls through
    /// the ground the sections were chosen for -- the walk from an eye
    /// far above the relief.
    #[test]
    fn a_lifted_camera_looks_at_the_sky() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (160u32, 120u32);
        let mut c = terrain_config();
        c.escape.terrain.tier = RenderTier::Lit;
        c.escape.terrain.target_lift = 0.4;
        c.escape.terrain.far = 8.0;
        // Ten degrees above the horizon, the frame's lower edge below it.
        c.escape.cam_pitch = -10f32.to_radians();
        let mut engines = crate::renderer::render::RenderEngines::default();
        let a = pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            crate::renderer::RenderJob::new(&c, w, h).with_engines(&mut engines),
            &mut crate::renderer::NoProgress,
        ))
        .expect("render")
        .rgba_data;
        let px = |x: u32, y: u32| {
            let k = ((y * w + x) * 4) as usize;
            [a[k], a[k + 1], a[k + 2]]
        };
        let sky = [0.55f32, 0.65, 0.8].map(|v| (v * 255.0).round() as i32);
        let is_sky = |p: [u8; 3]| (0..3).all(|i| (p[i] as i32 - sky[i]).abs() <= 2);
        assert_eq!((0..w).filter(|&x| is_sky(px(x, 0))).count(), w as usize, "the top row is the sky");
        let ground = (0..w).filter(|&x| !is_sky(px(x, h - 1))).count();
        assert!(ground > (w / 2) as usize, "the bottom row is ground: {ground} of {w}");
        // Every ray under the horizon that meets the plane within reach
        // hits the ground.
        let t = engines.terrain.as_ref().unwrap();
        let geom: Vec<[u32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(
            &device,
            &queue,
            t.terrain_for_test().geometry_buffer(),
            (w * h * 16) as u64,
        ))
        .to_vec();
        let cam = terrain_camera(&c.escape);
        assert!(cam.forward[2] > 0.0, "the camera looks up");
        let (aspect, th) = (w as f64 / h as f64, (cam.fov as f64 * 0.5).tan());
        let (mut checked, mut holes) = (0usize, 0usize);
        for py in 0..h {
            for px in 0..w {
                let (u, v) = ((px as f64 + 0.5) / w as f64 - 0.5, (py as f64 + 0.5) / h as f64 - 0.5);
                let d: [f64; 3] =
                    std::array::from_fn(|k| cam.forward[k] + cam.right[k] * u * aspect * 2.0 * th - cam.up[k] * v * 2.0 * th);
                if d[2] >= 0.0 {
                    continue;
                }
                let s = -cam.eye[2] / d[2];
                let dist = s * (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if dist > 0.6 * c.escape.terrain.far as f64 {
                    continue;
                }
                checked += 1;
                if !(f32::from_bits(geom[(py * w + px) as usize][3]) > 0.0) {
                    holes += 1;
                }
            }
        }
        println!("{checked} rays under the horizon within reach, {holes} through the ground");
        assert!(checked > (w * 4) as usize, "{checked}");
        assert_eq!(holes, 0, "rays fell through the ground");
    }

    /// Raising the height redraws the ground at once. The ground's slab
    /// top was the height when the ground was last set, and the height
    /// was not in the ground's key: a raised height drew the terrain cut
    /// flat at the old one, through an engine kept between frames, until
    /// a section landed or the camera moved.
    #[test]
    fn raising_the_height_redraws_the_ground() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let mut c = terrain_config();
        c.escape.terrain.tier = RenderTier::Lit;
        let shot = |c: &FractalConfig, engines: &mut crate::renderer::render::RenderEngines| {
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                crate::renderer::RenderJob::new(c, w, h).with_engines(engines),
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let mut kept = crate::renderer::render::RenderEngines::default();
        let low = shot(&c, &mut kept);
        c.escape.terrain.height = 0.2;
        let raised = shot(&c, &mut kept);
        let fresh = shot(&c, &mut crate::renderer::render::RenderEngines::default());
        assert_ne!(low, fresh, "the height moved the picture");
        assert_eq!(raised, fresh, "a raised height drew through a kept engine as a fresh one draws it");
    }

    /// Where each pixel's ray meets the ground, as a height in view widths
    /// (None for the sky): the walk's geometry record, read back.
    fn hit_heights(device: &Device, queue: &Queue, c: &FractalConfig, w: u32, h: u32) -> Vec<Option<f64>> {
        hit_heights_measured(device, queue, c, w, h).0
    }

    /// `hit_heights`, and the count's or relief's range the ground
    /// measured: what a lock pins (H3).
    fn hit_heights_measured(device: &Device, queue: &Queue, c: &FractalConfig, w: u32, h: u32) -> (Vec<Option<f64>>, Option<[f32; 2]>) {
        let mut engines = crate::renderer::render::RenderEngines::default();
        let _ = pollster::block_on(crate::renderer::render(
            device,
            queue,
            crate::renderer::RenderJob::new(c, w, h).with_engines(&mut engines),
            &mut crate::renderer::NoProgress,
        ))
        .expect("render");
        let range = engines.terrain.as_mut().unwrap().settled_range_for_test(device, queue);
        let t = engines.terrain.as_ref().unwrap();
        let geom: Vec<[u32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(
            device,
            queue,
            t.terrain_for_test().geometry_buffer(),
            (w * h * 16) as u64,
        ))
        .to_vec();
        let cam = terrain_camera(&c.escape);
        let (aspect, th) = (w as f64 / h as f64, (cam.fov as f64 * 0.5).tan());
        let heights = (0..h)
            .flat_map(|py| (0..w).map(move |px| (px, py)))
            .map(|(px, py)| {
                let depth = f32::from_bits(geom[(py * w + px) as usize][3]) as f64;
                if !(depth > 0.0) {
                    return None;
                }
                let (u, v) = ((px as f64 + 0.5) / w as f64 - 0.5, (py as f64 + 0.5) / h as f64 - 0.5);
                let d: [f64; 3] =
                    std::array::from_fn(|k| cam.forward[k] + cam.right[k] * u * aspect * 2.0 * th - cam.up[k] * v * 2.0 * th);
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                Some(cam.eye[2] + d[2] / len * depth)
            })
            .collect();
        (heights, range)
    }

    /// Looking straight down through a narrow lens over the seahorse
    /// valley's busy boundary, an escape count's terrain: each pixel's ray
    /// meets nearly the same column of ground whatever the surface, so two
    /// maps can be compared pixel by pixel.
    fn looking_down() -> FractalConfig {
        let mut c = terrain_config();
        c.escape.terrain.tier = RenderTier::Lit;
        c.escape.terrain.source = TerrainSource::EscapeCount;
        c.escape.terrain.interior = TerrainInterior::Lake;
        c.escape.center_re = "-0.7453".into();
        c.escape.center_im = "0.1127".into();
        c.escape.zoom_log2 = 6.0;
        c.escape.cam_pitch = 1.5707;
        c.escape.cam_fov = 0.1;
        c
    }

    /// Invert turns the land upside down (terrain-height.md, H2): where
    /// the plain map stands at `h`, the inverted one stands at `H − h`.
    /// Over land alone -- the exterior well away from the set -- since a
    /// cell with an interior corner (a lake's 0, a plateau's top) mixes
    /// the land with what Invert leaves as it is.
    #[test]
    fn invert_turns_the_land_upside_down() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (64u32, 64u32);
        // Coarse ground (Detail 0.1: a texel about ten pixels): the two
        // surfaces' hits are a pixel and a half apart however narrow the
        // lens -- their heights differ by up to H over 2.6 frames of depth
        // -- and at a texel a pixel both read one patch of ground, where at
        // full detail an escape count is rough at exactly that scale.
        for source in [TerrainSource::EscapeCount, TerrainSource::Distance] {
            let mut c = looking_down();
            c.escape.terrain.source = source;
            c.escape.terrain.detail = 0.1;
            c.escape.center_re = "0.45".into();
            c.escape.center_im = "0.6".into();
            c.escape.zoom_log2 = 2.0;
            // Wide flanks: the set's hairs reach here, and at the default
            // width their flanks are as steep as the count's roughness.
            c.escape.terrain.de_width = 0.2;
            let top = c.escape.terrain.height as f64;
            let plain = hit_heights(&device, &queue, &c, w, h);
            let mut inv = c.clone();
            inv.escape.terrain.invert = true;
            let inverted = hit_heights(&device, &queue, &inv, w, h);
            let (mut land, mut agree, mut lake) = (0usize, 0usize, 0usize);
            for (a, b) in plain.iter().zip(&inverted) {
                let (Some(a), Some(b)) = (a, b) else { continue };
                if a.abs() < 1e-5 && b.abs() < 1e-5 {
                    lake += 1;
                    continue;
                }
                land += 1;
                agree += usize::from((a + b - top).abs() < 0.02 * top);
            }
            println!("invert {source:?}: {agree} of {land} land pixels stand at H - h, {lake} lake");
            assert!(land > (w * h / 2) as usize, "{land}");
            assert!(agree as f64 > 0.9 * land as f64, "{source:?}: {agree} of {land}");
        }
    }

    /// A locked relief's lengths are its lock's (H3): at the lock they
    /// are the config's, a zoom in doubles them in current widths -- the
    /// same lengths in the plane -- and the camera's target rests on the
    /// locked top. Unlocked, nothing scales.
    #[test]
    fn a_locked_relief_keeps_its_lengths_in_the_plane() {
        let mut c = terrain_config();
        c.escape.terrain.target_lift = 0.05;
        c.escape.terrain.locked = true;
        c.escape.terrain.lock_zoom = c.escape.zoom_log2;
        let (h, w, occ) = (c.escape.terrain.height, c.escape.terrain.de_width, c.escape.terrain.occlusion);
        let at = terrain_view(&c, [0.0; 2]);
        assert_eq!((at.height, at.width, at.occlusion_reach), (h, w, occ));
        c.escape.zoom_log2 += 1.0;
        let v = terrain_view(&c, [0.0; 2]);
        assert_eq!((v.height, v.width, v.occlusion_reach), (2.0 * h, 2.0 * w, 2.0 * occ));
        let cam = terrain_camera(&c.escape);
        assert!((cam.target[2] - (2.0 * h as f64 + 0.05)).abs() < 1e-9, "{:?}", cam.target);
        c.escape.terrain.locked = false;
        let free = terrain_view(&c, [0.0; 2]);
        assert_eq!((free.height, free.width), (h, w));
        // Held where an f32 world still resolves the view.
        c.escape.terrain.locked = true;
        c.escape.zoom_log2 = c.escape.terrain.lock_zoom + 100.0;
        assert_eq!(c.escape.terrain_scale(), 24f64.exp2());
    }

    /// Locking changes nothing on screen (H3): the range it pins is the
    /// one the view measured, so the locked heights are the live ones.
    /// And the pinned range is the one the heights rise over: twice as
    /// wide, a linear count's land stands half as high.
    #[test]
    fn locking_pins_the_range_the_view_measured() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (64u32, 64u32);
        let mut c = looking_down();
        c.escape.terrain.detail = 0.1;
        c.escape.terrain.curve = crate::config::escape::TerrainCurve::Linear;
        c.escape.center_re = "0.45".into();
        c.escape.center_im = "0.6".into();
        c.escape.zoom_log2 = 2.0;
        let top = c.escape.terrain.height as f64;
        let (live, range) = hit_heights_measured(&device, &queue, &c, w, h);
        let [lo, hi] = range.expect("the view measured a range");
        assert!(lo < hi, "{lo} {hi}");

        let mut locked = c.clone();
        locked.escape.terrain.locked = true;
        locked.escape.terrain.lock_zoom = c.escape.zoom_log2;
        locked.escape.terrain.lock_range = [lo, hi];
        let pinned = hit_heights(&device, &queue, &locked, w, h);
        let same = live.iter().zip(&pinned).filter(|(a, b)| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-6 * top,
            (None, None) => true,
            _ => false,
        });
        assert_eq!(same.count(), (w * h) as usize, "locking moved the land");

        locked.escape.terrain.lock_range = [lo, lo + 2.0 * (hi - lo)];
        let wide = hit_heights(&device, &queue, &locked, w, h);
        let (mut land, mut half) = (0usize, 0usize);
        for (a, b) in live.iter().zip(&wide) {
            if let (Some(a), Some(b)) = (a, b) {
                land += 1;
                if (b - a / 2.0).abs() < 0.02 * top {
                    half += 1;
                }
            }
        }
        println!("pinned twice as wide: {half} of {land} land pixels stand half as high");
        assert!(land > (w * h / 2) as usize, "{land}");
        assert!(half as f64 > 0.9 * land as f64, "{half} of {land}");
    }

    /// Zooming in on a locked relief approaches one landscape (H3): the
    /// ground under the view's centre stands at the same height in the
    /// plane -- twice as many current widths, a zoom in -- where an
    /// unlocked relief keeps its widths and so halves in the plane.
    #[test]
    fn a_locked_relief_stands_still_in_the_plane() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (32u32, 32u32);
        let mut c = looking_down();
        c.escape.terrain.source = TerrainSource::Distance;
        c.escape.terrain.de_width = 0.2;
        c.escape.center_re = "0.45".into();
        c.escape.center_im = "0.6".into();
        c.escape.zoom_log2 = 2.0;
        c.escape.terrain.locked = true;
        c.escape.terrain.lock_zoom = 2.0;
        // The centre's four pixels: the ground under the target.
        let centre = |hs: &[Option<f64>]| -> f64 {
            let at = |x: u32, y: u32| hs[(y * w + x) as usize].expect("ground at the centre");
            (at(w / 2 - 1, h / 2 - 1) + at(w / 2, h / 2 - 1) + at(w / 2 - 1, h / 2) + at(w / 2, h / 2)) / 4.0
        };
        let before = centre(&hit_heights(&device, &queue, &c, w, h));
        c.escape.zoom_log2 = 3.0;
        let after = centre(&hit_heights(&device, &queue, &c, w, h));
        println!("locked: the centre's ground at {before:.5} widths, then {after:.5} (twice: {:.5})", 2.0 * before);
        assert!(before > 0.05 * c.escape.terrain.height as f64, "land to measure: {before}");
        assert!((after - 2.0 * before).abs() < 0.05 * after, "{before} then {after}");
    }

    /// The sections' filter smooths the ground (H4). Over the seahorse
    /// valley's busy boundary, the same section's texels: how spiky the
    /// land is -- the mean of the count's log rise's Laplacian, which a
    /// smooth slope leaves at 0 and a lone spike makes four times its
    /// height. A texel the mean of its footprint is less spiky than a
    /// texel a sample, and a softness less again. Measured on the texels
    /// themselves, since on screen a lake's shore is a cliff the filter
    /// rightly leaves alone, and it dominates there.
    #[test]
    fn the_filter_smooths_the_ground() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        // The view's full sections' texels (rows north up) and the
        // ground's range.
        let texels = |prefilter: u32, softness: f32| -> (HashMap<SectionKey, Vec<f32>>, [f32; 2]) {
            let mut c = looking_down();
            c.escape.terrain.prefilter = prefilter;
            c.escape.terrain.softness = softness;
            let mut engines = crate::renderer::render::RenderEngines::default();
            let _ = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                crate::renderer::RenderJob::new(&c, 64, 64).with_engines(&mut engines),
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
            let t = engines.terrain.as_mut().unwrap();
            let range = t.settled_range_for_test(&device, &queue).expect("a range");
            let (raw, _) = t.terrain_for_test().tile_textures_for_test().expect("an atlas");
            let all = t
                .cached_for_test()
                .into_iter()
                .filter_map(|k| {
                    let (layer, n) = t.layer_for_test(k)?;
                    (n == SECTION_SAMPLES).then(|| (k, bytemuck::cast_slice(&read_layer(&device, &queue, raw, layer, n, n)).to_vec()))
                })
                .collect();
            (all, range)
        };
        let n = SECTION_SAMPLES as usize;
        let rough = |v: &[f32], [lo, hi]: [f32; 2]| -> (f64, usize) {
            let rise = |r: f32| (1.0 + (r - lo).max(0.0) as f64).ln() / (1.0 + (hi - lo) as f64).ln();
            let (mut sum, mut count) = (0.0, 0usize);
            for y in 1..n - 1 {
                for x in 1..n - 1 {
                    let around = [v[y * n + x], v[y * n + x - 1], v[y * n + x + 1], v[(y - 1) * n + x], v[(y + 1) * n + x]];
                    if around.iter().all(|r| r.abs() < 1.0e29) {
                        let h = around.map(rise);
                        sum += (4.0 * h[0] - h[1] - h[2] - h[3] - h[4]).abs();
                        count += 1;
                    }
                }
            }
            (sum / count.max(1) as f64, count)
        };
        // The section with the most land, the same in each: the sections
        // are the view's, whatever the filter.
        let (point, range) = texels(1, 0.0);
        let (averaged, range2) = texels(2, 0.0);
        let (soft, range3) = texels(1, 2.0);
        let key = *point.keys().max_by_key(|k| rough(&point[k], range).1).expect("a section");
        let (p, land) = rough(&point[&key], range);
        let (a, _) = rough(&averaged[&key], range2);
        let (s, _) = rough(&soft[&key], range3);
        println!("section {key:?}: {land} land texels; mean |laplacian| a sample a texel {p:.5}, 2x2 {a:.5}, softness 2 {s:.5}");
        assert!(land > n * n / 10, "land to measure: {land}");
        assert!(a < 0.9 * p, "the prefilter: {a} vs {p}");
        assert!(s < 0.5 * p, "the softness: {s} vs {p}");
    }

    /// Softened sections meet without a seam (H4): rendered with a border
    /// of three sigmas, two neighbours blur their shared edge from the
    /// same ground, so its samples agree from both sides as closely as
    /// unsoftened ones do.
    #[test]
    fn softened_sections_meet_without_a_seam() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let edge_gap = |softness: f32| -> (f64, usize) {
            let mut c = looking_down();
            c.escape.terrain.softness = softness;
            c.escape.terrain.prefilter = 2;
            let mut engines = crate::renderer::render::RenderEngines::default();
            let _ = pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                crate::renderer::RenderJob::new(&c, 64, 64).with_engines(&mut engines),
                &mut crate::renderer::NoProgress,
            ))
            .expect("render");
            let t = engines.terrain.as_ref().unwrap();
            let keys = t.cached_for_test();
            let (raw, _) = t.terrain_for_test().tile_textures_for_test().expect("an atlas");
            let column = |layer: u32, n: u32, x: u32| -> Vec<f32> {
                let all: Vec<f32> = bytemuck::cast_slice(&read_layer(&device, &queue, raw, layer, n, n)).to_vec();
                (0..n).map(|y| all[(y * n + x) as usize]).collect()
            };
            let (mut worst, mut pairs) = (0.0f64, 0usize);
            for &(l, i, j) in &keys {
                let (Some((la, na)), Some((lb, nb))) = (t.layer_for_test((l, i, j)), t.layer_for_test((l, i + 1, j))) else {
                    continue;
                };
                if na != SECTION_SAMPLES || nb != SECTION_SAMPLES {
                    continue;
                }
                // Its east column is its east neighbour's west one.
                let (east, west) = (column(la, na, na - 1), column(lb, nb, 0));
                for (a, b) in east.iter().zip(&west) {
                    if a.abs() < 1.0e29 && b.abs() < 1.0e29 {
                        worst = worst.max(((a - b).abs() / a.abs().max(b.abs()).max(1.0)) as f64);
                    }
                }
                pairs += 1;
            }
            (worst, pairs)
        };
        let (plain, n0) = edge_gap(0.0);
        let (soft, n1) = edge_gap(3.0);
        println!("shared edges: {n0} pairs, worst {plain:.2e} unsoftened; {n1} pairs, worst {soft:.2e} softened");
        assert!(n0 > 0 && n1 > 0, "neighbours to compare");
        assert!(soft <= plain.max(1.0e-5) * 2.0, "a seam: {soft} against {plain}");
    }

    /// One layer of an atlas's raw array, `n x n` r32float texels.
    fn read_layer(device: &Device, queue: &Queue, tex: &Texture, layer: u32, w: u32, h: u32) -> Vec<u8> {
        let row = (w * 4).div_ceil(256) * 256;
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: Origin3d { x: 0, y: 0, z: layer }, aspect: TextureAspect::All },
            TexelCopyBufferInfo { buffer: &staging, layout: TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        let slice = staging.slice(..);
        slice.map_async(MapMode::Read, |_| {});
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let at = (y * row) as usize;
            out.extend_from_slice(&data[at..at + (w * 4) as usize]);
        }
        drop(data);
        staging.unmap();
        out
    }

    /// The curves keep their order (H2): a mesa tops out above the log
    /// rise, and the log rise stands above the linear one, pixel by pixel.
    #[test]
    fn the_curves_keep_their_order() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (64u32, 64u32);
        let shot = |curve: crate::config::escape::TerrainCurve| {
            let mut c = looking_down();
            c.escape.terrain.curve = curve;
            hit_heights(&device, &queue, &c, w, h)
        };
        use crate::config::escape::TerrainCurve;
        let (linear, log, mesa, auto) = (shot(TerrainCurve::Linear), shot(TerrainCurve::Log), shot(TerrainCurve::Mesa), shot(TerrainCurve::Auto));
        assert_eq!(log, auto, "a count's own curve is the log rise");
        let top = looking_down().escape.terrain.height as f64;
        let tol = 0.02 * top;
        let (mut n, mut ordered, mut mesa_higher) = (0usize, 0usize, 0usize);
        for k in 0..linear.len() {
            let (Some(a), Some(b), Some(m)) = (linear[k], log[k], mesa[k]) else { continue };
            n += 1;
            ordered += usize::from(a <= b + tol && b <= m + tol);
            mesa_higher += usize::from(m > b + tol);
        }
        println!("curves: {ordered} of {n} pixels linear <= log <= mesa, the mesa higher at {mesa_higher}");
        assert!(ordered as f64 > 0.9 * n as f64, "{ordered} of {n}");
        assert!(mesa_higher > n / 10, "the mesa tops out: {mesa_higher}");
    }

    /// A height of 0 is a flat plane, seen in 3D: every ray that meets
    /// the ground meets it at the plain's level, holes included.
    #[test]
    fn a_height_of_zero_is_a_flat_plane() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let mut c = terrain_config();
        c.escape.terrain.tier = RenderTier::Lit;
        c.escape.terrain.height = 0.0;
        c.escape.terrain.interior = crate::config::escape::TerrainInterior::Hole;
        let mut engines = crate::renderer::render::RenderEngines::default();
        let rgba = pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            crate::renderer::RenderJob::new(&c, w, h).with_engines(&mut engines),
            &mut crate::renderer::NoProgress,
        ))
        .expect("render")
        .rgba_data;
        let t = engines.terrain.as_ref().unwrap();
        let geom: Vec<[u32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(
            &device,
            &queue,
            t.terrain_for_test().geometry_buffer(),
            (w * h * 16) as u64,
        ))
        .to_vec();
        let cam = terrain_camera(&c.escape);
        let (aspect, th) = (w as f64 / h as f64, (cam.fov as f64 * 0.5).tan());
        let (mut hits, mut worst) = (0usize, 0.0f64);
        for py in 0..h {
            for px in 0..w {
                let depth = f32::from_bits(geom[(py * w + px) as usize][3]) as f64;
                if !(depth > 0.0) {
                    continue;
                }
                let (u, v) = ((px as f64 + 0.5) / w as f64 - 0.5, (py as f64 + 0.5) / h as f64 - 0.5);
                let d: [f64; 3] =
                    std::array::from_fn(|k| cam.forward[k] + cam.right[k] * u * aspect * 2.0 * th - cam.up[k] * v * 2.0 * th);
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let z = cam.eye[2] + d[2] / len * depth;
                worst = worst.max(z.abs());
                hits += 1;
            }
        }
        println!("flat plane: {hits} hits, the worst off the plain {worst:.2e} view widths");
        assert!(hits > (w * h / 3) as usize, "{hits}");
        assert!(worst < 1.0e-4, "a hit off the plain: {worst}");
        assert!(rgba.chunks(4).all(|p| p[3] > 0), "every pixel drawn");
    }

    /// Sections are kept by their place in the plane: a camera move or a
    /// light edit renders few or none; a pan renders only what it
    /// uncovers; a picture edit renders them all again. A caller-owned
    /// engine's frames equal a fresh render's.
    #[test]
    fn sections_are_reused_across_frames() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut engines = crate::renderer::render::RenderEngines::default();
        let mut c = terrain_config();
        let mut frame = |c: &FractalConfig, engines: &mut crate::renderer::render::RenderEngines| {
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                crate::renderer::RenderJob::new(c, 384, 216).with_engines(engines),
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let renders = |e: &crate::renderer::render::RenderEngines| e.terrain.as_ref().unwrap().footprint_renders;
        let first = frame(&c, &mut engines);
        let base = renders(&engines);
        c.solid_shading.shading_strength = 1.0;
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.lights[0].intensity = 1.5;
        let _ = frame(&c, &mut engines);
        assert_eq!(renders(&engines), base, "a light edit renders no section");
        c.escape.cam_yaw = 0.15;
        let turned = frame(&c, &mut engines);
        let after_turn = renders(&engines);
        assert_ne!(first, turned, "the camera moved");
        assert!(after_turn - base <= base / 2, "a small turn: {} new of {base}", after_turn - base);
        // A pan of a tenth of a view width.
        c.escape.center_re = "-0.7".into();
        let _ = frame(&c, &mut engines);
        let after_pan = renders(&engines);
        assert!(after_pan - after_turn <= base / 2, "a small pan: {} new of {base}", after_pan - after_turn);
        c.escape.max_iter += 50;
        let _ = frame(&c, &mut engines);
        assert!(renders(&engines) - after_pan >= base / 2, "a new picture renders again");
        assert_eq!(frame(&c, &mut engines), render(&device, &queue, &c, 384, 216), "a reused engine draws a fresh render's picture");
        println!("first {base}, turn +{}, pan +{}", after_turn - base, after_pan - after_turn);
    }

    /// The viewport keeps path tracing (reported from the app: stuck on
    /// its first sample). Driven as the app drives it -- `resize`,
    /// `update`, `render_viewport`, every frame -- a terrain in the Path
    /// Traced tier reaches its samples once its sections are in. `resize`
    /// had restarted it whenever the LIT tier's accumulation was empty,
    /// which in this tier it always is.
    #[test]
    fn the_path_traced_viewport_progresses() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let palette = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("white palette"),
            size: wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            palette.as_image_copy(),
            &[255u8; 256 * 4],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256 * 4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
        );
        let pv = palette.create_view(&wgpu::TextureViewDescriptor::default());
        let (w, h) = (320u32, 180u32);
        let mut c = terrain_config();
        c.escape.terrain.tier = RenderTier::PathTraced;
        c.escape.path.samples = 24;
        let mut t = EscapeTerrain::new(&device, w, h);
        let mut frames = 0;
        loop {
            t.resize(&device, w, h);
            let a = t.update(&device, &queue, &c, w, h, &pv, 1, 4);
            let b = t.render_viewport(&device, &queue, &c);
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            frames += 1;
            if !a && !b {
                break;
            }
            assert!(frames < 2000, "still busy after {frames} frames at {} samples", t.path_progress().0);
        }
        println!("path traced viewport: {} samples in {frames} frames", t.path_progress().0);
        assert_eq!(t.path_progress(), (24, true));
        t.destroy();
    }

    /// Deep zoom is the 2D renderer's (plan H3): at 2^60, on the
    /// perturbed floatexp path, the ground renders -- sections are 2D
    /// renders at their own centres, and the walk sees only view widths.
    #[test]
    fn a_deep_zoom_terrain_renders() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let text = std::fs::read_to_string("tests/visual/configs/escape/fe-zoom-60-edge.fflame").expect("config");
        let mut c: FractalConfig = serde_json::from_str(&text).expect("parse");
        assert!(c.escape.zoom_log2 >= 60.0);
        c.escape.shading.enabled = false;
        c.escape.terrain.enabled = true;
        c.escape.terrain.far = 3.0;
        c.background_color = [0.55, 0.65, 0.8];
        let (w, h) = (96u32, 72u32);
        let out = render(&device, &queue, &c, w, h);
        let sky = [0.55f32, 0.65, 0.8].map(|v| (v * 255.0).round() as i32);
        let ground = out
            .chunks_exact(4)
            .filter(|p| (0..3).any(|i| (p[i] as i32 - sky[i]).abs() > 2))
            .count();
        println!("2^60: {ground} of {} pixels ground", w * h);
        assert!(ground > (w * h / 3) as usize, "{ground}");
    }

    /// Auto contrast is one fit for the whole ground: the anchor's probe
    /// view's, held for every section -- not each section's own, which
    /// stepped the colour at every section's edge, and not whichever was
    /// rendered last.
    #[test]
    fn auto_contrast_is_one_fit_for_the_ground() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let palette = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let palette_view = palette.create_view(&wgpu::TextureViewDescriptor::default());
        let mut c = terrain_config();
        c.escape.center_re = "-0.7453".into();
        c.escape.center_im = "0.1127".into();
        c.escape.zoom_log2 = 9.0;
        c.escape.contrast.mode = crate::config::escape::ContrastMode::AutoRange;
        c.escape.contrast.strength = 1.0;
        let (w, h) = (160u32, 120u32);
        let mut t = EscapeTerrain::new(&device, w, h);
        let mut guard = 0;
        while t.update(&device, &queue, &c, w, h, &palette_view, 1, 1) && guard < 100_000 {
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            guard += 1;
        }
        assert!(t.contrast_pinned);
        let held = t.footprint.contrast_fit_for_test();
        assert!(held.is_some(), "a fit");
        // The probe's own, measured alone.
        let anchor = t.anchor.clone().unwrap();
        let probe = contrast_probe_config(&footprint_config(&c.escape));
        let mut alone = EscapeRenderer::new(&device, COARSE_SAMPLES, COARSE_SAMPLES);
        let mut guard = 0;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            let settled = alone.render(&device, &queue, &mut enc, &probe, &palette_view, 1);
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            guard += 1;
            if settled || guard > 10_000 {
                break;
            }
        }
        assert_eq!(held, alone.contrast_fit_for_test(), "the probe's fit");
        // And a section rendered now keeps it.
        let section = section_config(&footprint_config(&c.escape), &anchor, t.cached_for_test()[0], SECTION_SAMPLES);
        let fp = t.footprint_renderer();
        fp.resize(&device, SECTION_SAMPLES, SECTION_SAMPLES, 1);
        let mut guard = 0;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            let settled = fp.render(&device, &queue, &mut enc, &section, &palette_view, 1);
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            guard += 1;
            if settled || guard > 10_000 {
                break;
            }
        }
        assert_eq!(held, t.footprint.contrast_fit_for_test(), "held through a section");
        alone.destroy();
        t.destroy();
    }

    /// A count's range is the ground's, not the engine's history: filled
    /// over a wide view, then zoomed into another band -- every section
    /// re-made -- an engine's range is a fresh engine's at the new view.
    /// It kept the wide view's least count, and the deep ground came out
    /// nearly flat beside what an export of it draws.
    #[test]
    fn a_counts_range_is_the_grounds() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let palette = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let palette_view = palette.create_view(&wgpu::TextureViewDescriptor::default());
        let (w, h) = (160u32, 120u32);
        let fill = |t: &mut EscapeTerrain, c: &FractalConfig| {
            let mut guard = 0;
            while t.update(&device, &queue, c, w, h, &palette_view, 1, 1) && guard < 100_000 {
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                guard += 1;
            }
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let raw = crate::escape::terrain::gpu_tests::read_buffer(&device, &queue, t.terrain.range_buffer_for_test(), 8);
            let r: [u32; 2] = bytemuck::cast_slice::<u8, u32>(&raw).try_into().unwrap();
            r
        };
        let mut wide = terrain_config();
        wide.escape.terrain.source = TerrainSource::EscapeCount;
        wide.escape.max_iter = 200;
        let mut deep = wide.clone();
        deep.escape.center_re = "-0.7453".into();
        deep.escape.center_im = "0.1127".into();
        deep.escape.zoom_log2 = 9.0;
        // One engine at a time: each holds an atlas of sections, and the
        // suite's other GPU tests run beside it.
        let mut a = EscapeTerrain::new(&device, w, h);
        let first = fill(&mut a, &wide);
        let after = fill(&mut a, &deep);
        a.destroy();
        drop(a);
        let mut b = EscapeTerrain::new(&device, w, h);
        let fresh = fill(&mut b, &deep);
        b.destroy();
        println!("wide {first:?}, then deep {after:?}, fresh deep {fresh:?}");
        assert_ne!(first, fresh, "the views' ranges differ");
        assert_eq!(after, fresh);
    }

    /// An export reports its progress as it goes -- the sections, then the
    /// still's samples, tile by tile -- never backwards, and ends at 1: what
    /// the app's progress bar shows while a terrain exports in the
    /// background. Lit and path traced, and in tiles.
    #[test]
    fn a_terrain_export_reports_its_progress() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        struct Record(Vec<f64>);
        impl crate::renderer::RenderProgress for Record {
            fn on_progress(&mut self, current: u64, total: u64) {
                self.0.push(current as f64 / total.max(1) as f64);
            }
        }
        for (tier, (w, h)) in [(RenderTier::Lit, (160u32, 120u32)), (RenderTier::PathTraced, (160, 120)), (RenderTier::PathTraced, (2200, 60))] {
            let mut c = terrain_config();
            c.escape.terrain.tier = tier;
            c.escape.path.samples = 8;
            let mut record = Record(Vec::new());
            pollster::block_on(crate::renderer::render(&device, &queue, crate::renderer::RenderJob::new(&c, w, h), &mut record)).expect("render");
            let p = &record.0;
            println!("{tier:?} {w}x{h}: {} reports", p.len());
            assert!(p.len() >= 4, "{p:?}");
            assert!(p.windows(2).all(|v| v[1] >= v[0] - 1e-9), "never backwards: {p:?}");
            assert!((p.last().unwrap() - 1.0).abs() < 1e-9, "ends at 1: {p:?}");
            assert!(p.iter().any(|&f| f > 0.05 && f < 0.95), "between: {p:?}");
        }
    }

    /// A deep picture's hidden roots render at [`COARSE_SAMPLES`], and no
    /// pixel shows it: unshadowed and without occlusion, so that only
    /// what primary rays meet is drawn, the picture is the one with
    /// every root a full section's, byte for byte. A shallow picture's
    /// roots stay full.
    #[test]
    fn a_deep_pictures_hidden_roots_are_coarse_and_unseen() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut c: FractalConfig = serde_json::from_str(
            &std::fs::read_to_string("tests/visual/configs/escape/fe-zoom-60-edge.fflame").expect("config"),
        )
        .expect("parse");
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.escape.max_iter = 3000;
        c.escape.shading.enabled = false;
        c.escape.terrain.enabled = true;
        c.escape.terrain.far = 2.0;
        c.escape.terrain.tier = RenderTier::Lit;
        c.escape.terrain.shadow = 0.0;
        c.escape.terrain.occlusion = 0.0;
        let (w, h) = (320u32, 180u32);
        let mut engines = crate::renderer::render::RenderEngines::default();
        let mut picture = |c: &FractalConfig, engines: &mut crate::renderer::render::RenderEngines| {
            pollster::block_on(crate::renderer::render(
                &device,
                &queue,
                crate::renderer::RenderJob::new(c, w, h).with_engines(engines),
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let coarse = picture(&c, &mut engines);
        let t = engines.terrain.as_mut().expect("the terrain engine");
        let cached = t.cached_samples_for_test();
        let roots: Vec<_> = cached.iter().filter(|(_, n)| *n == COARSE_SAMPLES).collect();
        println!("{} sections, {} coarse", cached.len(), roots.len());
        assert!(!roots.is_empty(), "{cached:?}");
        let top = cached.iter().map(|(k, _)| k.0).max().unwrap();
        assert!(roots.iter().all(|(k, _)| k.0 == top), "only roots coarse: {cached:?}");
        // The same picture with every root a full section's.
        t.coarse_samples = SECTION_SAMPLES;
        t.invalidate_footprint();
        let full = picture(&c, &mut engines);
        let t = engines.terrain.as_mut().unwrap();
        assert!(t.cached_samples_for_test().iter().all(|(_, n)| *n == SECTION_SAMPLES));
        let differ = coarse.chunks_exact(4).zip(full.chunks_exact(4)).filter(|(a, b)| a != b).count();
        assert_eq!(differ, 0, "{differ} pixels see a hidden root");
        // Shallow: direct, so every root full.
        t.coarse_samples = COARSE_SAMPLES;
        let mut s = terrain_config();
        s.escape.terrain.tier = RenderTier::Lit;
        let _ = picture(&s, &mut engines);
        let t = engines.terrain.as_mut().unwrap();
        assert!(t.cached_samples_for_test().iter().all(|(_, n)| *n == SECTION_SAMPLES), "a direct picture's roots stay full");
    }

    /// A deep fill in the app, measured: `fe-zoom-60-edge` as a terrain
    /// at 1080p, its sections from nothing on the frame loop's terms (the
    /// orbits on the worker thread, as many steps a frame as fit 8 ms at
    /// the measured section time, each frame's work waited on), alone and
    /// with the viewport drawing each tier -- and how many orbits were
    /// relocated rather than built.
    #[test]
    #[ignore = "measurement"]
    fn deep_fill_in_the_app() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut c: FractalConfig = serde_json::from_str(
            &std::fs::read_to_string("tests/visual/configs/escape/fe-zoom-60-edge.fflame").expect("config"),
        )
        .expect("parse");
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.escape.terrain.enabled = true;
        c.escape.terrain.far = 4.0;
        c.escape.shading.enabled = false;
        println!("{}, zoom 2^{}, max_iter {}", c.escape.formula, c.escape.zoom_log2, c.escape.max_iter);
        let palette = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let palette_view = palette.create_view(&wgpu::TextureViewDescriptor::default());
        let mut app = EscapeTerrain::new(&device, 1920, 1080);
        app.footprint_renderer().progressive = true;
        let passes = [
            ("cold", None),
            ("refill", None),
            ("refill, viewport Lit", Some(RenderTier::Lit)),
            ("refill, viewport Auto", Some(RenderTier::Auto)),
            ("refill, viewport Path Traced", Some(RenderTier::PathTraced)),
        ];
        for (pass, viewport) in passes {
            // A new picture: every section again.
            c.escape.max_iter += 1;
            if let Some(tier) = viewport {
                c.escape.terrain.tier = tier;
            }
            let d0 = crate::escape::diag::snapshot();
            let mut frames = 0u32;
            let t0 = std::time::Instant::now();
            loop {
                let steps = app.section_ms().map_or(1, |ms| (8.0 / ms.max(0.5)).floor().clamp(1.0, 4.0) as u32);
                let more = app.update(&device, &queue, &c, 1920, 1080, &palette_view, 1, steps);
                if viewport.is_some() {
                    app.render_viewport(&device, &queue, &c);
                }
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                frames += 1;
                if !more || frames > 200_000 {
                    break;
                }
            }
            let d1 = crate::escape::diag::snapshot();
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let cached = app.cached_samples_for_test();
            let coarse = cached.iter().filter(|(_, n)| *n == COARSE_SAMPLES).count();
            println!(
                "{pass}: {ms:.0} ms, {} sections ({coarse} coarse), {frames} frames ({:.1} ms a frame); orbits {} built, {} relocated",
                cached.len(),
                ms / frames as f64,
                d1.orbit_rebuilds - d0.orbit_rebuilds,
                d1.orbit_relocations - d0.orbit_relocations,
            );
        }
    }

    /// The fill, measured: every section a view wants rendered from
    /// nothing, on the direct path (Mandelbrot at 2^3, 2,000 iterations)
    /// and the perturbed one (`fe-zoom-60-edge`), at 1080p.
    #[test]
    #[ignore = "measurement"]
    fn section_fill_times() {
        let Some((_one_at_a_time, device, queue)) = gpu() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let deep: FractalConfig = serde_json::from_str(
            &std::fs::read_to_string("tests/visual/configs/escape/fe-zoom-60-edge.fflame").expect("config"),
        )
        .expect("parse");
        let mut shallow = terrain_config();
        shallow.escape.zoom_log2 = 3.0;
        shallow.escape.center_im = "0.1".into();
        shallow.escape.max_iter = 2000;
        for (label, base) in [("direct 2^3", shallow), ("perturbed 2^60", deep)] {
            for (far, ss) in [(4.0f32, 1u32), (8.0, 1), (8.0, 2)] {
                let mut c = base.clone();
                c.render_mode = crate::scene::transforms::RenderMode::Escape;
                c.escape.terrain.enabled = true;
                c.escape.terrain.far = far;
                c.escape.supersample = ss;
                c.escape.shading.enabled = false;
                let mut engines = crate::renderer::render::RenderEngines::default();
                // Warm: shaders, the reference.
                let _ = pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    crate::renderer::RenderJob::new(&c, 1920, 1080).with_engines(&mut engines),
                    &mut crate::renderer::NoProgress,
                ));
                c.escape.max_iter += 1;
                let t0 = std::time::Instant::now();
                let _ = pollster::block_on(crate::renderer::render(
                    &device,
                    &queue,
                    crate::renderer::RenderJob::new(&c, 1920, 1080).with_engines(&mut engines),
                    &mut crate::renderer::NoProgress,
                ));
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                let t = engines.terrain.as_mut().unwrap();
                let n = t.cached_for_test().len();
                println!("{label}, far {far}, AA {ss}: {n} sections, {ms:.0} ms, {:.1} ms a section", ms / n.max(1) as f64);
            }
        }
    }
}
