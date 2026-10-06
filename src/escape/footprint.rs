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
//! - the target is the view's centre at the terrain's top, H, so with
//!   any pitch above the horizon the eye is above every point of it;
//! - `cam_pitch` is above the horizon; `cam_yaw` turns about the target,
//!   0 looking up the 2D picture -- which the View's `rotation` turns,
//!   so it turns the heading too;
//! - the eye is [`FRAME_DISTANCE`] view widths from the target, so a
//!   zoom is a dolly.
//!
//! **Heights are the view's.** H and the flanks' width are fractions of
//! the current view width, applied in the walk to the sections' raw
//! estimates: a dolly in flattens the far ground and raises the near
//! detail -- the terrain at every zoom looks like itself.

use super::ifs::{solid_frame, SolidCamera};
use super::terrain::{Ground, GroundNode, GroundSection, PathSettings, TerrainIngest, TerrainRenderer, TerrainView};
use crate::config::escape::TerrainTier;
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

/// The most sections the atlas holds (each about 21 MB).
pub const MAX_SECTIONS: u32 = 64;

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
    // The source decides what the iterate pass writes and the interior
    // how a section encodes it; the rest is the walk's and the view's.
    let (source, interior) = (k.terrain.source, k.terrain.interior);
    k.terrain = d.terrain.clone();
    k.terrain.enabled = true;
    k.terrain.source = source;
    k.terrain.interior = interior;
    k
}

/// What else the sections' pictures read from the config: the palette
/// and every setting that writes its texture, the background (a
/// plateau's colour), and the flame when the formula is a 2D IFS that
/// draws it. Keyed by content, not by the flame renderer's palette
/// generation, which every config load bumps.
fn palette_key(config: &FractalConfig) -> String {
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
    let top = escape.terrain.height as f64;
    SolidCamera {
        eye: [0.0, 0.0, top + eye_rel[2]],
        target: [-eye_rel[0], -eye_rel[1], top],
        forward,
        right,
        up,
        fov: escape.cam_fov.clamp(0.05, 3.0),
        distance,
        eye_rel,
    }
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
    TerrainView {
        camera: terrain_camera(&config.escape),
        shading: config.solid_shading.clone(),
        fog: (fog, fog_start, config.background_color),
        shadow: t.shadow,
        softness: t.shadow_sharpness,
        occlusion_reach: t.occlusion,
        jitter,
        height: t.height,
        width: t.de_width,
        samples_per_axis: config.escape.supersample.max(1),
        far: t.far,
    }
}

/// The path tracer's settings from the config (plan section 4): the
/// environment is the background colour brought into the accumulator's
/// units -- through the inverse of the Linear tonemap's exposure and
/// gamma -- so the sky and an albedo-1 surface it lights read as the
/// background does; a sample's radiance is clamped at ten times the
/// brightest light.
pub fn path_settings(config: &FractalConfig) -> PathSettings {
    let t = &config.escape.terrain;
    let gamma = if config.gamma > 0.0 { config.gamma } else { 1.0 };
    let exposure = config.exposure.max(1.0e-6);
    let env = config.background_color.map(|c| t.environment * c.max(0.0).powf(gamma) / exposure);
    let lights: f32 = config.solid_shading.lights.iter().filter(|l| l.enabled).map(|l| l.intensity.max(0.0)).sum();
    let brightest = lights.max(1.0).max(env.iter().cloned().fold(0.0, f32::max));
    PathSettings { bounces: t.bounces.min(16), environment: env, clamp: 10.0 * brightest, seed: 1 }
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
    TerrainIngest { source, hole: t.interior == TerrainInterior::Hole, background: config.background_color }
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
/// points -- `SECTION_SAMPLES` of them across a span one texel wider
/// than its side, centred on its centre, so sample (0, 0) is its
/// south-west corner and neighbours share their edges.
fn section_config(fp: &EscapeConfig, a: &Anchor, key: SectionKey) -> EscapeConfig {
    use super::fixedpoint::FixedPoint;
    let (level, i, j) = key;
    let mut c = fp.clone();
    c.rotation = 0.0;
    c.zoom_log2 = a.zoom - level as f64 - (SECTION_SAMPLES as f64 / SECTION_CELLS as f64).log2();
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

/// What the view wants: the root grid's level and its first square, the
/// grid's size, and the sections wanted -- the roots, and the leaves of
/// the split -- each with its nearest distance from the eye (current
/// widths), roots first.
#[derive(Debug, Clone)]
pub struct Wanted {
    pub root_level: i32,
    pub root_first: [i64; 2],
    pub root_dims: [u32; 2],
    pub sections: Vec<(SectionKey, f64, bool)>,
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
    let top = t.height as f64;
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
    // view.
    let geometry = |level: i32, i: i64, j: i64| -> (f64, bool) {
        let side = (level as f64).exp2();
        let (x0, y0) = ((i as f64 * side - eye[0]) * z, (j as f64 * side - eye[1]) * z);
        let s = side * z;
        let dx = 0.0f64.clamp(x0, x0 + s);
        let dy = 0.0f64.clamp(y0, y0 + s);
        let dz = if eye_z > top { eye_z - top } else { 0.0 };
        let near = (dx * dx + dy * dy + dz * dz).sqrt();
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
        (near, inside && near <= far)
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
                continue;
            }
            let texel = (level as f64).exp2() * z / SECTION_CELLS as f64;
            let need = near.max(1.0e-9) * per_pixel / rho * coarsen;
            if texel > need && level > finest {
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
            return Wanted { root_level, root_first: lo, root_dims, sections: out };
        }
        coarsen *= 1.5;
    }
}

/// A cached section: its atlas layer, and the frame it was last wanted.
struct Cached {
    layer: u32,
    wanted: u64,
}

/// A section being rendered.
struct Building {
    key: SectionKey,
    layer: u32,
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
    /// What the viewport's accumulation is of, and how far it has got.
    viewport_key: Option<String>,
    viewport_samples: u32,
    /// Whether the output is the path tracer's, not the lit tier's.
    showing_path: bool,
    /// A path-traced sample's time, in ms, measured to the GPU's
    /// completion of a batch, and its average.
    path_done: std::sync::Arc<std::sync::Mutex<Option<f32>>>,
    path_ms: Option<f32>,
}

/// Path-traced samples the Auto tier gathers before its picture replaces
/// the lit tier's: below this the noise reads worse than the lit picture.
pub const PATH_SHOW_SAMPLES: u32 = 8;

/// The time a viewport frame gives the path tracer, in ms.
const PATH_FRAME_MS: f32 = 12.0;

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
            viewport_key: None,
            viewport_samples: 0,
            showing_path: false,
            path_done: std::sync::Arc::new(std::sync::Mutex::new(None)),
            path_ms: None,
        }
    }

    /// Why a terrain of this config cannot be held at `w x h`: a
    /// section's escape render, the terrain's own buffers, or the atlas.
    pub fn allocation_error(device: &Device, escape: &EscapeConfig, w: u32, h: u32) -> Option<String> {
        let probe = Anchor { re: escape.center_re.clone(), im: escape.center_im.clone(), zoom: escape.zoom_log2 };
        let section = section_config(&footprint_config(escape), &probe, (0, 0, 0));
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
        self.terrain.resize(device, out_w, out_h);
        // The accumulation went with the old size.
        if self.terrain.accumulated_samples() == 0 {
            self.viewport_key = None;
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
        let escape = &config.escape;
        // A new picture: every section goes.
        let picture = (picture_key(escape), palette_key(config));
        if self.picture.as_ref() != Some(&picture) {
            self.picture = Some(picture);
            self.clear();
            self.terrain.reset_range(queue);
        }
        // Out of the anchor's band or lattice cell: the sections start
        // over on the new one.
        let canonical = canonical_anchor(escape);
        if self.anchor.as_ref() != Some(&canonical) {
            self.anchor = Some(canonical);
            self.clear();
        }
        let anchor = self.anchor.clone().expect("set above");
        let (eye, z) = eye_in_anchor(escape, &anchor);
        let wanted = wanted_sections(escape, out_w, out_h, MAX_SECTIONS as usize, anchor.zoom, eye);
        // The atlas holds what the view wants and a quarter more, growing
        // in steps of eight to `MAX_SECTIONS` -- each section is about
        // 21 MB, so a small view does not hold a large one's memory.
        // Growing re-makes it, and the sections start over.
        let need = ((wanted.sections.len() as u32 * 5 / 4).div_ceil(8) * 8).clamp(8, MAX_SECTIONS);
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
        let missing: Vec<SectionKey> = wanted.sections.iter().map(|w| w.0).filter(|k| !self.cache.contains_key(k)).collect();
        self.missing = missing.len();
        // A section no longer wanted is not finished.
        if self.building.as_ref().is_some_and(|b| !missing.contains(&b.key)) {
            if let Some(b) = self.building.take() {
                self.free.push(b.layer);
            }
        }
        let fp = footprint_config(escape);
        let derivative = self.footprint.derivative_active(&section_config(&fp, &anchor, (0, 0, 0)));
        let ingest = terrain_ingest(config, derivative);
        let mut landed = false;
        let current = self.building.as_ref().map(|b| b.key);
        let mut todo = missing.iter().copied().filter(move |k| Some(*k) != current);
        for _ in 0..steps.max(1) {
            if self.building.is_none() {
                let Some(key) = todo.next() else { break };
                let Some(layer) = self.free.pop().or_else(|| self.evict()) else { break };
                self.building = Some(Building { key, layer, started: web_time::Instant::now() });
            }
            let b = self.building.as_ref().expect("made above");
            let (key, layer) = (b.key, b.layer);
            let section = section_config(&fp, &anchor, key);
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Section") });
            let settled = self.footprint.render(device, queue, &mut enc, &section, palette_view, palette_generation);
            queue.submit(std::iter::once(enc.finish()));
            if !settled {
                continue;
            }
            self.terrain.begin_section(layer, SECTION_SAMPLES, SECTION_SAMPLES, &ingest);
            self.terrain.ingest_region(
                device,
                queue,
                self.footprint.output_view(),
                self.footprint.height_view(),
                0,
                0,
                SECTION_SAMPLES,
                SECTION_SAMPLES,
            );
            self.terrain.finish_section(device, queue);
            let b = self.building.take().expect("made above");
            self.cache.insert(key, Cached { layer, wanted: self.frame });
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
        self.set_ground(device, queue, config, &wanted, eye, z, landed, &ingest);
        self.missing > 0 || self.building.is_some()
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
        let key = format!("{eye:?}|{z}|{:?}|{:?}|{complete}", wanted.root_first, wanted.root_dims);
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
                texel: side * z / SECTION_CELLS as f64,
                n: SECTION_SAMPLES,
                m: SECTION_SAMPLES,
            });
            nodes[node].section = (sections.len() - 1) as i32;
        }
        let rs = (rl as f64).exp2();
        let t = &config.escape.terrain;
        let ground = Ground {
            root_origin: [
                (wanted.root_first[0] as f64 * rs - eye[0]) * z,
                (wanted.root_first[1] as f64 * rs - eye[1]) * z,
            ],
            root_side: rs * z,
            root_dims: wanted.root_dims,
            nodes,
            sections,
            top: t.height as f64,
            floor: -0.01,
            mode: ingest.mode(),
        };
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
        if self.terrain.tile_version() == 0 {
            return false;
        }
        let t = &config.escape.terrain;
        let base = terrain_view(config, [0.0, 0.0]);
        let key = format!("{base:?}|{}|{:?}", self.terrain.tile_version(), path_settings(config));
        let moved = self.viewport_key.as_deref() != Some(key.as_str());
        if moved {
            self.viewport_key = Some(key);
            self.viewport_samples = 0;
            self.terrain.reset_accumulation();
            self.terrain.reset_path();
            self.showing_path = false;
        }
        if t.tier != TerrainTier::PathTraced {
            let grid = EscapeRenderer::sample_grid(config.escape.supersample.max(1));
            if let Some(&jitter) = grid.get(self.viewport_samples as usize) {
                self.terrain.render(device, queue, &terrain_view(config, jitter));
                self.terrain.accumulate(device, queue);
                self.viewport_samples += 1;
                self.showing_path = false;
                return true;
            }
            if t.tier == TerrainTier::Lit {
                return false;
            }
        }
        let target = t.samples.max(1);
        let have = self.terrain.path_samples();
        if have >= target {
            return false;
        }
        // As many samples as fit the frame at the measured cost.
        let per_frame = self.path_ms().map_or(1, |ms| (PATH_FRAME_MS / ms.max(0.05)).floor().clamp(1.0, 64.0) as u32);
        let n = per_frame.min(target - have);
        self.trace(device, queue, config, n, n);
        self.showing_path = t.tier == TerrainTier::PathTraced || self.terrain.path_samples() >= PATH_SHOW_SAMPLES;
        self.terrain.path_samples() < target
    }

    /// Add `samples` path-traced samples of the config's view, in
    /// dispatches of at most `per_dispatch`, timing them. Submits its
    /// own work.
    fn trace(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, samples: u32, per_dispatch: u32) {
        let t0 = web_time::Instant::now();
        self.terrain.render_path(device, queue, &terrain_view(config, [0.0, 0.0]), &path_settings(config), samples, per_dispatch);
        let slot = std::sync::Arc::clone(&self.path_done);
        queue.on_submitted_work_done(move || {
            if let Ok(mut g) = slot.lock() {
                *g = Some(t0.elapsed().as_secs_f32() * 1000.0 / samples.max(1) as f32);
            }
        });
    }

    /// A path-traced sample's average time, in ms, once measured.
    pub fn path_ms(&mut self) -> Option<f32> {
        if let Some(ms) = self.path_done.lock().ok().and_then(|mut g| g.take()) {
            self.path_ms = Some(match self.path_ms {
                Some(avg) => avg + (ms - avg) * 0.3,
                None => ms,
            });
        }
        self.path_ms
    }

    /// The export's picture (plan section 8): path traced at `samples`
    /// unless the tier is Lit, which draws the antialiasing grid. In
    /// dispatches of a few samples each, `wait` called between them (a
    /// blocking poll on the desktop). Submits its own work.
    pub fn render_still(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, mut wait: impl FnMut()) {
        let t = &config.escape.terrain;
        if t.tier == TerrainTier::Lit {
            self.terrain.reset_accumulation();
            for jitter in EscapeRenderer::sample_grid(config.escape.supersample.max(1)) {
                self.terrain.render(device, queue, &terrain_view(config, jitter));
                self.terrain.accumulate(device, queue);
            }
            self.showing_path = false;
            return;
        }
        self.terrain.reset_path();
        let target = t.samples.max(1);
        // A batch the watchdog never notices: about a quarter second at
        // the measured cost, from a cautious start.
        while self.terrain.path_samples() < target {
            let per = self.path_ms().map_or(2, |ms| (250.0 / ms.max(0.05)).floor().clamp(1.0, 64.0) as u32);
            let n = per.min(target - self.terrain.path_samples());
            self.trace(device, queue, config, n, n);
            wait();
        }
        self.showing_path = true;
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
        if self.showing_path {
            return self.terrain.output_view();
        }
        self.terrain.accumulated_view().unwrap_or(self.terrain.output_view())
    }

    /// Path-traced samples so far, and whether the picture is theirs.
    pub fn path_progress(&self) -> (u32, bool) {
        (self.terrain.path_samples(), self.showing_path)
    }

    /// The terrain renderer, for a test to look inside.
    #[cfg(test)]
    pub(crate) fn terrain_for_test(&self) -> &TerrainRenderer {
        &self.terrain
    }

    /// The cached sections' keys, for a test.
    #[cfg(test)]
    pub(crate) fn cached_for_test(&self) -> Vec<SectionKey> {
        let mut k: Vec<SectionKey> = self.cache.keys().copied().collect();
        k.sort();
        k
    }

    /// Free the GPU memory now: dropping frees nothing on WebGPU.
    pub fn destroy(&self) {
        self.footprint.destroy();
        self.terrain.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(picture_key(&a), picture_key(&b));
        for edit in [
            |c: &mut EscapeConfig| c.max_iter += 1,
            |c: &mut EscapeConfig| c.terrain.source = TerrainSource::EscapeCount,
            |c: &mut EscapeConfig| c.terrain.interior = TerrainInterior::Hole,
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
        let pixel = |c: &EscapeConfig, px: f64, py: f64| -> (f64, f64) {
            let n = SECTION_SAMPLES as f64;
            let span = 4.0 * (-c.zoom_log2).exp2();
            let (cx, cy) = (c.center_re.parse::<f64>().unwrap(), c.center_im.parse::<f64>().unwrap());
            (cx + ((px + 0.5) / n - 0.5) * span, cy - ((py + 0.5) / n - 0.5) * span)
        };
        let s = SECTION_CELLS as f64;
        for key in [(0, 0, 0), (-3, 5, -2), (2, -1, 1)] {
            let c = section_config(&esc, &a, key);
            assert_eq!(c.rotation, 0.0);
            let side = (key.0 as f64).exp2() * width;
            let (x0, y0) = (are + key.1 as f64 * side, aim + key.2 as f64 * side);
            let sw = pixel(&c, 0.0, s);
            let nw = pixel(&c, 0.0, 0.0);
            let se = pixel(&c, s, s);
            let tol = side * 1e-9;
            assert!((sw.0 - x0).abs() < tol && (sw.1 - y0).abs() < tol, "{key:?} south-west {sw:?} vs {:?}", (x0, y0));
            assert!((nw.1 - (y0 + side)).abs() < tol, "{key:?} north edge");
            assert!((se.0 - (x0 + side)).abs() < tol, "{key:?} east edge");
            let east = section_config(&esc, &a, (key.0, key.1 + 1, key.2));
            let e_sw = pixel(&east, 0.0, s);
            assert!((e_sw.0 - se.0).abs() < tol && (e_sw.1 - se.1).abs() < tol, "{key:?}: the east neighbour starts where it ends");
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
            assert!(near <= esc.terrain.far as f64 + 1e-9, "a leaf within reach: {near}");
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
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
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
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
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
        c.escape.terrain.tier = TerrainTier::Lit;
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

    /// Sections are kept by their place in the plane: a camera move or a
    /// light edit renders few or none; a pan renders only what it
    /// uncovers; a picture edit renders them all again. A caller-owned
    /// engine's frames equal a fresh render's.
    #[test]
    fn sections_are_reused_across_frames() {
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
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

    /// Deep zoom is the 2D renderer's (plan H3): at 2^60, on the
    /// perturbed floatexp path, the ground renders -- sections are 2D
    /// renders at their own centres, and the walk sees only view widths.
    #[test]
    fn a_deep_zoom_terrain_renders() {
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
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

    /// The fill, measured: every section a view wants rendered from
    /// nothing, on the direct path (Mandelbrot at 2^3, 2,000 iterations)
    /// and the perturbed one (`fe-zoom-60-edge`), at 1080p.
    #[test]
    #[ignore = "measurement"]
    fn section_fill_times() {
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
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
