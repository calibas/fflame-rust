//! The escape producer of the 3D terrain view
//! (docs/projects/heightfield-3d.md, section 5, phase T2).
//!
//! A FOOTPRINT is the escape picture at the view -- the config's own
//! centre, zoom and rotation -- rendered square at the terrain's
//! resolution by an ordinary [`EscapeRenderer`], with the terrain's
//! height source kept in its height field. [`TerrainRenderer`] ingests
//! it as a tile and draws it in 3D.
//!
//! **Where things are.** Everything the terrain shader sees is in the
//! tile's CELL units, which are the footprint's pixels: the footprint
//! is `n` cells across whatever the zoom, the heights are a fraction of
//! that, and so is the camera's distance. Deep zoom stays the 2D
//! renderer's business (plan H3) -- the footprint at 2^60 is the 2D
//! picture at 2^60 -- and the shader never needs a big number.
//!
//! **The camera** is mode D's, read for a plane:
//! - the target is the view's centre, so the terrain and the 2D picture
//!   are of the same place and switching between them keeps it;
//!   `cam_target_*` are mode D's and unused here;
//! - the target stands at the terrain's top, H, so with any pitch above
//!   the horizon the eye is above every point of the terrain;
//! - `cam_pitch` is above the horizon and `cam_yaw` turns about the
//!   target, 0 looking north -- up the 2D picture;
//! - the distance is a fixed multiple of the footprint ([`FRAME_DISTANCE`]),
//!   so a zoom is a new footprint seen from the same place;
//! - the View's `rotation` turns the footprint, as it turns the 2D
//!   picture, rather than rolling the screen as it does in mode D.
//!
//! **The footprint's size is the view's** (plan section 11, the user's
//! choice): about the antialiasing factor's texels per screen pixel
//! where the terrain is nearest the eye (`wanted_resolution`), so the
//! fractal is sampled as finely as a 2D render at that antialiasing.
//! Past one escape render's size it is rendered in tiles
//! (`FootprintLayout`), each a render at its own centre, and the tile
//! being built replaces the one drawn only when it is complete.

use super::ifs::{solid_frame, SolidCamera};
use super::terrain::{TerrainIngest, TerrainRenderer, TerrainView};
use super::EscapeRenderer;
use crate::config::escape::{EscapeConfig, TerrainInterior, TerrainSource};
use crate::config::FractalConfig;
use wgpu::*;

/// The eye's distance from the target, in footprint widths: the whole
/// footprint in a 16:9 frame at the default field of view and pitch,
/// with a tenth of the frame to spare at the near corners. A narrower
/// frame crops them, as any fixed vertical field of view does.
pub const FRAME_DISTANCE: f64 = 1.3;

/// The smallest footprint side; below it a terrain is a few facets.
pub const MIN_RESOLUTION: u32 = 256;

/// The largest side an automatic footprint takes: 8192², about 1.4 GB
/// of tile (twice that while a build replaces it). A fixed resolution
/// may ask for more, within the device's texture side.
pub const MAX_AUTO_RESOLUTION: u32 = 8192;

/// The largest footprint tile: one escape render's side.
pub const MAX_TILE: u32 = 2048;

/// The footprint's side the view wants (plan section 11): the config's
/// fixed resolution, or for `resolution = 0` about `supersample` texels
/// per screen pixel where the terrain is nearest the eye, so the
/// fractal is sampled as finely as a 2D render at that antialiasing.
///
/// Found by casting a grid of the camera's rays at the terrain's box
/// -- the footprint, from the ground to its top -- in footprint widths:
/// the nearest entry is where a texel is largest on the screen.
pub fn wanted_resolution(escape: &EscapeConfig, out_w: u32, out_h: u32) -> u32 {
    if escape.terrain.resolution != 0 {
        return escape.terrain.resolution.max(MIN_RESOLUTION);
    }
    let unit = 10_000u32;
    let cam = terrain_camera(escape, unit);
    let scale = unit as f64;
    let (lo, hi) = ([0.0, 0.0, 0.0], [scale - 1.0, scale - 1.0, escape.terrain.height as f64 * scale]);
    let aspect = out_w.max(1) as f64 / out_h.max(1) as f64;
    let th = (cam.fov as f64 * 0.5).tan();
    let mut nearest = f64::INFINITY;
    const RAYS: usize = 17;
    for j in 0..RAYS {
        for i in 0..RAYS {
            let (u, v) = (i as f64 / (RAYS - 1) as f64 - 0.5, j as f64 / (RAYS - 1) as f64 - 0.5);
            let d: [f64; 3] = std::array::from_fn(|k| {
                cam.forward[k] + cam.right[k] * u * aspect * 2.0 * th - cam.up[k] * v * 2.0 * th
            });
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            // The slab test: the ray's interval inside the box.
            let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
            for k in 0..3 {
                let dk = d[k] / len;
                if dk.abs() < 1e-12 {
                    if cam.eye[k] < lo[k] || cam.eye[k] > hi[k] {
                        t0 = f64::INFINITY;
                    }
                    continue;
                }
                let (a, b) = ((lo[k] - cam.eye[k]) / dk, (hi[k] - cam.eye[k]) / dk);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
            if t0 <= t1 {
                nearest = nearest.min(t0);
            }
        }
    }
    if !nearest.is_finite() {
        return MIN_RESOLUTION;
    }
    // Nothing nearer than a twentieth of the footprint: a camera inside
    // the box would otherwise ask for no end of texels.
    let nearest = (nearest / scale).max(0.05);
    let rho = escape.supersample.max(1) as f64;
    let per_pixel = nearest * 2.0 * th / out_h.max(1) as f64;
    ((rho / per_pixel).ceil() as u32).clamp(MIN_RESOLUTION, MAX_AUTO_RESOLUTION)
}

/// How a footprint of `n` samples a side is rendered: `per_side²`
/// escape renders of `tile²`, `n = per_side * tile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FootprintLayout {
    pub n: u32,
    pub tile: u32,
    pub per_side: u32,
}

impl FootprintLayout {
    /// The layout for a wanted side: as few tiles as [`MAX_TILE`] allows,
    /// each a multiple of 256, within the device's texture side.
    pub fn for_side(wanted: u32, device_max: u32) -> FootprintLayout {
        let wanted = wanted.clamp(MIN_RESOLUTION, device_max.max(MIN_RESOLUTION));
        let per_side = wanted.div_ceil(MAX_TILE);
        let mut tile = wanted.div_ceil(per_side).div_ceil(256) * 256;
        while per_side * tile > device_max.max(MIN_RESOLUTION) && tile > 256 {
            tile -= 256;
        }
        FootprintLayout { n: per_side * tile, tile, per_side }
    }
}

/// Tile `(i, j)`'s escape config (`i` east, `j` down the picture): its
/// pixels the footprint's own, so the tiles' pixels are the single
/// render's. One tile is the footprint itself, untouched.
pub fn tile_config(fp: &EscapeConfig, layout: &FootprintLayout, i: u32, j: u32) -> EscapeConfig {
    if layout.per_side == 1 {
        return fp.clone();
    }
    use super::fixedpoint::FixedPoint;
    let mut c = fp.clone();
    let (n, t) = (layout.n as f64, layout.tile as f64);
    c.zoom_log2 = fp.zoom_log2 + (layout.per_side as f64).log2();
    // The tile's centre from the footprint's, in its pixels (x right,
    // y down), then to the plane: the pan's own rotation, and a pixel of
    // 4 * 2^-zoom / n kept as a power of two and a mantissa, so the
    // offset reaches any depth.
    let (dx, dy) = (i as f64 * t + t / 2.0 - n / 2.0, j as f64 * t + t / 2.0 - n / 2.0);
    let x = (4.0 / n).log2() - fp.zoom_log2;
    let e = x.floor();
    let m = (x - e).exp2();
    let (s, co) = (fp.rotation as f64).sin_cos();
    let (wx, wy) = (dx * co + dy * s, dx * s - dy * co);
    if let (Some(re), Some(im)) = (
        FixedPoint::decimal_add_floatexp(&fp.center_re, wx * m, e as i64, c.zoom_log2),
        FixedPoint::decimal_add_floatexp(&fp.center_im, wy * m, e as i64, c.zoom_log2),
    ) {
        c.center_re = re;
        c.center_im = im;
    }
    c
}

/// The footprint's render config: the escape config at the view, with
/// what the terrain path forces. No supersample: the terrain's own
/// accumulation antialiases what the screen sees, and a footprint
/// texel is about a screen pixel near the target.
pub fn footprint_config(escape: &EscapeConfig) -> EscapeConfig {
    let mut f = escape.clone();
    f.supersample = 1;
    f
}

/// What the footprint's PICTURE depends on: its config with everything
/// only the 3D view reads set to its default, so an orbit, a light or a
/// height edit does not re-render it.
fn footprint_key(escape: &EscapeConfig) -> EscapeConfig {
    let mut k = footprint_config(escape);
    let d = EscapeConfig::default();
    k.cam_target_x = d.cam_target_x;
    k.cam_target_y = d.cam_target_y;
    k.cam_target_z = d.cam_target_z;
    k.cam_pitch = d.cam_pitch;
    k.cam_yaw = d.cam_yaw;
    k.cam_bank = d.cam_bank;
    k.cam_fov = d.cam_fov;
    // The source decides what the iterate pass writes and the interior
    // how the tile encodes it; the heights, the lighting and the size
    // are the walk's and the layout's.
    let (source, interior) = (k.terrain.source, k.terrain.interior);
    k.terrain = d.terrain.clone();
    k.terrain.enabled = true;
    k.terrain.source = source;
    k.terrain.interior = interior;
    k
}

/// What else the footprint's picture reads from the config: the
/// palette and every setting that writes its texture, and the flame
/// when the formula is a 2D IFS that draws it. Keyed by content, not by
/// the flame renderer's palette generation, which every config load
/// bumps -- a video's frames would otherwise each re-render an
/// unchanged footprint.
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

/// The terrain's camera, in the cells of an `n`-wide footprint centred
/// on the view (see the module docs).
pub fn terrain_camera(escape: &EscapeConfig, n: u32) -> SolidCamera {
    let nf = n as f64;
    let c = (nf - 1.0) * 0.5;
    let target = [c, c, escape.terrain.height as f64 * nf];
    let distance = FRAME_DISTANCE * nf;
    let (right, up, forward) = solid_frame(
        escape.cam_pitch as f64,
        escape.cam_yaw as f64 - std::f64::consts::FRAC_PI_2,
        escape.cam_bank as f64,
        0.0,
    );
    let eye_rel = [-forward[0] * distance, -forward[1] * distance, -forward[2] * distance];
    SolidCamera {
        eye: [target[0] + eye_rel[0], target[1] + eye_rel[1], target[2] + eye_rel[2]],
        target,
        forward,
        right,
        up,
        fov: escape.cam_fov.clamp(0.05, 3.0),
        distance,
        eye_rel,
    }
}

/// The camera when the tile holds the footprint `fp` rather than the
/// view's own: a footprint lagging a dolly or a pan (plan H11). The
/// eye is where the view's camera would be over the OLD terrain --
/// nearer by the zoom since, its target moved by the pan since -- so
/// the motion shows at once, on stretched texels, until the new
/// footprint lands.
pub fn terrain_camera_over(escape: &EscapeConfig, fp: &EscapeConfig, n: u32) -> SolidCamera {
    let mut cam = terrain_camera(escape, n);
    let zoomed = (escape.zoom_log2 - fp.zoom_log2).clamp(-60.0, 60.0).exp2();
    let (dx, dy) = centre_offset_cells(escape, fp, n);
    cam.target[0] += dx;
    cam.target[1] += dy;
    cam.distance /= zoomed;
    cam.eye_rel = [-cam.forward[0] * cam.distance, -cam.forward[1] * cam.distance, -cam.forward[2] * cam.distance];
    cam.eye = [cam.target[0] + cam.eye_rel[0], cam.target[1] + cam.eye_rel[1], cam.target[2] + cam.eye_rel[2]];
    cam
}

/// The view's centre less the footprint's, in the footprint's cells
/// (x east, y north in the footprint's own rotated frame). Subtracted
/// in fixed point, so it holds at any depth: the difference is a few
/// spans however many digits the two centres carry.
fn centre_offset_cells(escape: &EscapeConfig, fp: &EscapeConfig, n: u32) -> (f64, f64) {
    use super::fixedpoint::{limbs_for_view, FixedPoint};
    let deep = escape.zoom_log2.max(fp.zoom_log2);
    let limbs = limbs_for_view(&escape.center_re, &escape.center_im, deep)
        .max(limbs_for_view(&fp.center_re, &fp.center_im, deep));
    // (a - b) in units of the footprint's 2^-zoom.
    let diff = |a: &str, b: &str| -> f64 {
        match (FixedPoint::from_decimal(a, limbs), FixedPoint::from_decimal(b, limbs)) {
            (Some(a), Some(b)) => {
                let fe = a.sub(&b).to_floatexp();
                fe.m * (fe.e as f64 + fp.zoom_log2).clamp(-1000.0, 1000.0).exp2()
            }
            _ => 0.0,
        }
    };
    let (wx, wy) = (diff(&escape.center_re, &fp.center_re), diff(&escape.center_im, &fp.center_im));
    // The footprint's pixel offset of a world delta: the inverse of
    // `ifs::view_basis` for a square of side 4 * 2^-zoom, which is its
    // own inverse up to the span. Screen y runs down, the tile's north.
    let (s, c) = (fp.rotation as f64).sin_cos();
    let k = n as f64 / 4.0;
    let (u, v) = (c * wx + s * wy, s * wx - c * wy);
    (u * k, -v * k)
}

/// How the terrain is seen and lit, in an `n`-wide footprint's cells:
/// the camera, the Solid lighting, and fog with its distances measured
/// in footprint widths, as mode D's are in the attractor's.
pub fn terrain_view(config: &FractalConfig, n: u32, jitter: [f32; 2]) -> TerrainView {
    terrain_view_with(config, terrain_camera(&config.escape, n), n, jitter)
}

fn terrain_view_with(config: &FractalConfig, camera: SolidCamera, n: u32, jitter: [f32; 2]) -> TerrainView {
    let t = &config.escape.terrain;
    let nf = n as f32;
    TerrainView {
        camera,
        shading: config.solid_shading.clone(),
        fog: (config.fog_strength / nf, config.fog_start * nf, config.background_color),
        shadow: t.shadow,
        softness: t.shadow_sharpness,
        occlusion_reach: t.occlusion * nf,
        jitter,
        height: t.height * nf,
        width: t.de_width * nf,
        samples_per_axis: config.escape.supersample.max(1),
    }
}

/// How the footprint becomes a tile. `derivative` is whether the
/// footprint's iterate pass compiled one: without it the distance
/// source wrote the escape count (`esc_terrain_source`), and the
/// ingest must read it as one.
pub fn terrain_ingest(config: &FractalConfig, derivative: bool) -> TerrainIngest {
    let t = &config.escape.terrain;
    let source = match t.source {
        TerrainSource::Distance if derivative => 8,
        TerrainSource::Distance | TerrainSource::EscapeCount => 9,
        TerrainSource::Relief => config.escape.shading.field.to_gpu(),
    };
    TerrainIngest { source, hole: t.interior == TerrainInterior::Hole, background: config.background_color }
}

/// What a tile was made from: the footprint's picture, the palette's
/// content, and the layout's side.
type FootprintKey = (EscapeConfig, String, u32);

/// A footprint under way: what it is of, and the next tile to render.
struct FootprintBuild {
    key: FootprintKey,
    layout: FootprintLayout,
    next: u32,
    started: web_time::Instant,
}

/// The escape terrain: the footprint's renderer (one tile's size), the
/// terrain renderer, and what the tile was made from, so only a change
/// to the 2D picture or to the footprint's size re-renders it.
pub struct EscapeTerrain {
    footprint: EscapeRenderer,
    terrain: TerrainRenderer,
    /// The layout new footprints are rendered at.
    layout: FootprintLayout,
    /// The footprint renderer's side: one tile's.
    footprint_tile: u32,
    /// The drawn tile's key and its footprint's side.
    rendered: Option<FootprintKey>,
    build: Option<FootprintBuild>,
    /// Footprints completed, for the cache's gates.
    pub footprint_renders: u32,
    /// A footprint's time from its first chunk to its GPU completion,
    /// in ms, written by the queue's completion callback.
    footprint_done: std::sync::Arc<std::sync::Mutex<Option<f32>>>,
    /// The smoothed footprint time: what decides whether the footprint
    /// follows the camera live (H11).
    footprint_ms: Option<f32>,
    /// What the viewport's accumulation is of, and how far it has got.
    viewport_key: Option<String>,
    viewport_samples: u32,
}

/// The footprint time under which a terrain re-renders its footprint
/// DURING a dolly or a pan rather than on its release (H11, the user's
/// 15 ms).
pub const LIVE_FOOTPRINT_MS: f32 = 15.0;

impl EscapeTerrain {
    pub fn new(device: &Device, out_w: u32, out_h: u32) -> Self {
        let layout = FootprintLayout::for_side(MIN_RESOLUTION, MIN_RESOLUTION);
        EscapeTerrain {
            footprint: EscapeRenderer::new(device, layout.tile, layout.tile),
            terrain: TerrainRenderer::new(device, out_w, out_h),
            layout,
            footprint_tile: layout.tile,
            rendered: None,
            build: None,
            footprint_renders: 0,
            footprint_done: std::sync::Arc::new(std::sync::Mutex::new(None)),
            footprint_ms: None,
            viewport_key: None,
            viewport_samples: 0,
        }
    }

    /// Why a terrain of this config cannot be held at `w x h`: a tile's
    /// escape render, the terrain's own buffers, or the tile itself --
    /// twice over, since a build replaces it.
    pub fn allocation_error(device: &Device, escape: &EscapeConfig, w: u32, h: u32) -> Option<String> {
        let lim = device.limits();
        let layout = FootprintLayout::for_side(wanted_resolution(escape, w, h), lim.max_texture_dimension_2d);
        let tile = tile_config(&footprint_config(escape), &layout, 0, 0);
        EscapeRenderer::allocation_error(device, &tile, layout.tile, layout.tile, 1)
            .or_else(|| TerrainRenderer::allocation_error(device, w, h))
            .or_else(|| {
                let n = layout.n as u64;
                (n * n * 16 > lim.max_buffer_size.max(1 << 31)).then(|| {
                    format!("a {0}x{0} terrain footprint is past this device's memory", layout.n)
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

    /// The footprint's renderer, for the caller to give it what any
    /// escape render needs first (a 2D IFS's analysis, a texture's
    /// image).
    pub fn footprint_renderer(&mut self) -> &mut EscapeRenderer {
        &mut self.footprint
    }

    /// The layout new footprints are rendered at: the view's wanted
    /// side (`wanted_resolution`) for an `out_w x out_h` picture.
    ///
    /// `sticky` is the viewport's: the side follows the camera only
    /// when it would grow by a fifth or shrink by half, so an orbit
    /// does not re-render the footprint at every step.
    pub fn choose_layout(&mut self, device: &Device, escape: &EscapeConfig, out_w: u32, out_h: u32, sticky: bool) {
        let wanted = wanted_resolution(escape, out_w, out_h);
        let device_max = device.limits().max_texture_dimension_2d;
        let next = FootprintLayout::for_side(wanted, device_max);
        let keep = sticky
            && escape.terrain.resolution == 0
            && self.rendered.is_some()
            && next.n as f32 <= self.layout.n as f32 * 1.2
            && next.n as f32 >= self.layout.n as f32 * 0.5;
        if !keep && next != self.layout {
            self.layout = next;
            if next.tile != self.footprint_tile {
                self.footprint.resize(device, next.tile, next.tile, 1);
                self.footprint_tile = next.tile;
            }
        }
    }

    /// The layout new footprints are rendered at.
    pub fn layout(&self) -> FootprintLayout {
        self.layout
    }

    /// Render the next footprints at this layout, whatever the view
    /// wants: a test's way to tile a footprint small enough to compare.
    #[cfg(test)]
    pub(crate) fn force_layout(&mut self, device: &Device, layout: FootprintLayout) {
        self.layout = layout;
        if layout.tile != self.footprint_tile {
            self.footprint.resize(device, layout.tile, layout.tile, 1);
            self.footprint_tile = layout.tile;
        }
    }

    fn key(&self, config: &FractalConfig) -> FootprintKey {
        (footprint_key(&config.escape), palette_key(config), self.layout.n)
    }

    /// Whether the footprint must be rendered again for this config: its
    /// 2D picture, the palette, or its size changed.
    pub fn footprint_stale(&self, config: &FractalConfig) -> bool {
        self.rendered.as_ref() != Some(&self.key(config))
    }

    /// Forget the footprint: something it was drawn with that its key
    /// does not see (a texture's image arriving) changed.
    pub fn invalidate_footprint(&mut self) {
        self.rendered = None;
    }

    /// Whether a footprint is part-way through its render.
    pub fn footprint_in_progress(&self) -> bool {
        self.build.is_some()
    }

    /// One step of the footprint: a chunk of its current tile's escape
    /// render, and when that tile settles, its region of the tile being
    /// built; when the last has, the new tile replaces the drawn one.
    /// Submits its own work; true once the footprint is complete (or
    /// was already). A config that changes under a build restarts it.
    pub fn step_footprint(
        &mut self,
        device: &Device,
        queue: &Queue,
        config: &FractalConfig,
        palette_view: &TextureView,
        palette_generation: u64,
    ) -> bool {
        let key = self.key(config);
        if self.rendered.as_ref() == Some(&key) && self.build.is_none() {
            return true;
        }
        if self.build.as_ref().is_none_or(|b| b.key != key) {
            let fp = footprint_config(&config.escape);
            let derivative = self.footprint.derivative_active(&tile_config(&fp, &self.layout, 0, 0));
            let ingest = terrain_ingest(config, derivative);
            let n = self.layout.n;
            self.terrain.begin_build(device, queue, n, n, &ingest);
            self.build = Some(FootprintBuild {
                key: key.clone(),
                layout: self.layout,
                next: 0,
                started: web_time::Instant::now(),
            });
        }
        let b = self.build.as_ref().expect("made above");
        let layout = b.layout;
        let (i, j) = (b.next % layout.per_side, b.next / layout.per_side);
        let tile = tile_config(&footprint_config(&config.escape), &layout, i, j);
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Footprint") });
        let settled = self.footprint.render(device, queue, &mut enc, &tile, palette_view, palette_generation);
        queue.submit(std::iter::once(enc.finish()));
        if !settled {
            return false;
        }
        self.terrain.ingest_region(
            device,
            queue,
            self.footprint.output_view(),
            self.footprint.height_view(),
            i * layout.tile,
            j * layout.tile,
            layout.tile,
            layout.tile,
        );
        let b = self.build.as_mut().expect("made above");
        b.next += 1;
        if b.next < layout.per_side * layout.per_side {
            return false;
        }
        let b = self.build.take().expect("made above");
        self.terrain.finish_build(device, queue);
        self.rendered = Some(b.key);
        self.footprint_renders += 1;
        // Timed to the GPU's completion: the chunks before this are
        // queued, not done.
        let slot = std::sync::Arc::clone(&self.footprint_done);
        let t0 = b.started;
        queue.on_submitted_work_done(move || {
            if let Ok(mut g) = slot.lock() {
                *g = Some(t0.elapsed().as_secs_f32() * 1000.0);
            }
        });
        true
    }

    /// The smoothed footprint render time, in ms, once one has been
    /// measured.
    pub fn footprint_ms(&mut self) -> Option<f32> {
        if let Some(ms) = self.footprint_done.lock().ok().and_then(|mut g| g.take()) {
            self.footprint_ms = Some(match self.footprint_ms {
                Some(avg) => avg + (ms - avg) * 0.3,
                None => ms,
            });
        }
        self.footprint_ms
    }

    /// Whether the footprint should follow the camera during a gesture
    /// (H11): its renders measured under [`LIVE_FOOTPRINT_MS`], or not
    /// measured yet.
    pub fn live_footprints(&mut self) -> bool {
        self.footprint_ms().is_none_or(|ms| ms < LIVE_FOOTPRINT_MS)
    }

    /// The view the tile is seen through: the config's, over whichever
    /// footprint the tile holds (`terrain_camera_over` while one lags),
    /// in that footprint's cells.
    fn current_view(&self, config: &FractalConfig, jitter: [f32; 2]) -> TerrainView {
        let (camera, n) = match &self.rendered {
            Some((fp, _, n)) => (terrain_camera_over(&config.escape, fp, *n), *n),
            None => (terrain_camera(&config.escape, self.layout.n), self.layout.n),
        };
        terrain_view_with(config, camera, n, jitter)
    }

    /// One frame of the viewport: a sample of the config's
    /// antialiasing grid (`supersample²` jittered renders, the export's
    /// own), folded into the accumulation, which any change to the view
    /// or the tile restarts. True while samples remain.
    pub fn render_viewport(&mut self, device: &Device, queue: &Queue, config: &FractalConfig) -> bool {
        if self.terrain.tile_version() == 0 {
            return false;
        }
        let base = self.current_view(config, [0.0, 0.0]);
        let key = format!("{base:?}|{}", self.terrain.tile_version());
        if self.viewport_key.as_deref() != Some(key.as_str()) {
            self.viewport_key = Some(key);
            self.viewport_samples = 0;
            self.terrain.reset_accumulation();
        }
        let grid = EscapeRenderer::sample_grid(config.escape.supersample.max(1));
        let Some(&jitter) = grid.get(self.viewport_samples as usize) else {
            return false;
        };
        let view = self.current_view(config, jitter);
        self.terrain.render(device, queue, &view);
        self.terrain.accumulate(device, queue);
        self.viewport_samples += 1;
        (self.viewport_samples as usize) < grid.len()
    }

    /// Draw the terrain once, its rays offset by `jitter` within their
    /// pixels. Submits its own work.
    pub fn render(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, jitter: [f32; 2]) {
        let view = self.current_view(config, jitter);
        self.terrain.render(device, queue, &view);
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

    /// What the tail reads: the accumulation once there is one, else
    /// the last render.
    pub fn output_view(&self) -> &TextureView {
        self.terrain.accumulated_view().unwrap_or(self.terrain.output_view())
    }

    /// The terrain renderer, for a test to look inside.
    #[cfg(test)]
    pub(crate) fn terrain_for_test(&self) -> &TerrainRenderer {
        &self.terrain
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

    /// The camera's conventions: the target at the screen's centre, yaw
    /// 0 looking north (up the 2D picture) with east to the right, the
    /// eye above the terrain's top, and the whole footprint in frame at
    /// the defaults.
    #[test]
    fn the_terrain_camera_looks_north_at_the_view_centre() {
        let mut esc = EscapeConfig::default();
        esc.terrain.enabled = true;
        let n = 2048;
        let cam = terrain_camera(&esc, n);
        let aspect = 16.0 / 9.0;
        let (cx, cy) = project(&cam, cam.target, aspect);
        assert!(cx.abs() < 1e-9 && cy.abs() < 1e-9, "the target is centred: {cx} {cy}");
        let north = [cam.target[0], cam.target[1] + 100.0, cam.target[2]];
        let east = [cam.target[0] + 100.0, cam.target[1], cam.target[2]];
        let (_, ny) = project(&cam, north, aspect);
        let (ex, ey) = project(&cam, east, aspect);
        assert!(ny < 0.0, "north is up the screen: {ny}");
        assert!(ex > 0.0 && ey.abs() < 1e-9, "east is to the right: {ex} {ey}");
        let top = esc.terrain.height as f64 * n as f64;
        assert!(cam.eye[2] > top, "the eye is above the terrain: {} vs {top}", cam.eye[2]);
        // Every corner of the footprint, at the bottom and the top of
        // the terrain, inside a 16:9 frame.
        let m = (n - 1) as f64;
        for (x, y) in [(0.0, 0.0), (m, 0.0), (0.0, m), (m, m)] {
            for z in [0.0, top] {
                let (sx, sy) = project(&cam, [x, y, z], aspect);
                assert!(sx.abs() < 0.5 && sy.abs() < 0.5, "corner ({x}, {y}, {z}) at ({sx:.3}, {sy:.3})");
            }
        }
        // Yaw a quarter turn: looking east, north is to the left.
        esc.cam_yaw = -std::f32::consts::FRAC_PI_2;
        let cam = terrain_camera(&esc, n);
        let (nx, _) = project(&cam, [cam.target[0], cam.target[1] + 100.0, cam.target[2]], aspect);
        let (_, ey) = project(&cam, [cam.target[0] + 100.0, cam.target[1], cam.target[2]], aspect);
        assert!(nx < 0.0 && ey < 0.0, "yawed: north left {nx}, east up {ey}");
    }

    /// A footprint that lags the view: the camera over it is the view's
    /// camera moved by the pan and nearer by the zoom since. At the
    /// footprint's own view it is the ordinary camera exactly, and the
    /// pan's offset holds at 2^200, where the centres differ in their
    /// sixtieth digit.
    #[test]
    fn the_camera_over_a_lagging_footprint_follows_the_view() {
        let mut fp = EscapeConfig::default();
        fp.terrain.enabled = true;
        fp.center_re = "-0.75".into();
        fp.center_im = "0.1".into();
        fp.zoom_log2 = 3.0;
        let n = 1000;
        assert_eq!(terrain_camera_over(&fp, &fp, n), terrain_camera(&fp, n));
        // A quarter span east and an eighth north, then a zoom of one.
        let mut v = fp.clone();
        v.center_re = "-0.625".into(); // + 0.125 = a quarter of the span 0.5
        v.center_im = "0.1625".into(); // + 0.0625 = an eighth
        v.zoom_log2 = 4.0;
        let base = terrain_camera(&v, n);
        let cam = terrain_camera_over(&v, &fp, n);
        assert!((cam.target[0] - base.target[0] - 250.0).abs() < 1e-6, "{:?}", cam.target);
        assert!((cam.target[1] - base.target[1] - 125.0).abs() < 1e-6, "{:?}", cam.target);
        assert!((cam.distance * 2.0 - base.distance).abs() < 1e-9, "half as far after a zoom of one");
        // Rotated a quarter turn, the footprint's east is the plane's
        // north: the same pan lands on the other axes.
        let mut fr = fp.clone();
        fr.rotation = std::f32::consts::FRAC_PI_2;
        let mut vr = v.clone();
        vr.rotation = fr.rotation;
        vr.zoom_log2 = fr.zoom_log2;
        let r = terrain_camera_over(&vr, &fr, n);
        let b = terrain_camera(&vr, n);
        assert!((r.target[0] - b.target[0] - 125.0).abs() < 1e-3 && (r.target[1] - b.target[1] + 250.0).abs() < 1e-3,
            "rotated offset {:?}", [r.target[0] - b.target[0], r.target[1] - b.target[1]]);
        // Deep: centres 2^-200 apart are a quarter span at 2^198.
        let mut deep = fp.clone();
        deep.zoom_log2 = 198.0;
        deep.center_re = "-0.75".into();
        let mut dv = deep.clone();
        // -0.75, less 6.22e-61: "75" is decimals 1-2, 58 zeros 3-60.
        dv.center_re = format!("-0.75{}6223015277861141707144064053780124240590252168721167", "0".repeat(58));
        let off = centre_offset_cells(&dv, &deep, n);
        let want = 6.223015277861141707e-61 * 2f64.powf(198.0) * n as f64 / 4.0;
        assert!(((off.0 + want) / want).abs() < 1e-9, "deep offset {off:?} want -{want}");
    }

    /// Only the 2D picture keys the footprint: the camera, the lights and
    /// the heights do not; the height source and the interior do.
    #[test]
    fn the_footprint_key_ignores_what_only_the_3d_view_reads() {
        let mut a = EscapeConfig::default();
        a.terrain.enabled = true;
        let mut b = a.clone();
        b.cam_pitch = 1.0;
        b.cam_yaw = 2.0;
        b.cam_fov = 0.4;
        b.terrain.height = 0.2;
        b.terrain.de_width = 0.1;
        b.terrain.shadow = 0.0;
        b.terrain.occlusion = 0.1;
        assert_eq!(footprint_key(&a), footprint_key(&b));
        b.terrain.source = TerrainSource::EscapeCount;
        assert_ne!(footprint_key(&a), footprint_key(&b));
        // The interior is how the tile encodes the set: a new tile.
        let mut h = a.clone();
        h.terrain.interior = TerrainInterior::Hole;
        assert_ne!(footprint_key(&a), footprint_key(&h));
        let mut c = a.clone();
        c.zoom_log2 = 5.0;
        assert_ne!(footprint_key(&a), footprint_key(&c));
    }

    /// A Mandelbrot terrain config: the whole set in the footprint, a
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
        c.escape.max_iter = 500;
        c.escape.terrain.enabled = true;
        c.escape.terrain.resolution = 256;
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
    /// and video have it: sky above the footprint's far edge, lit
    /// terrain at the centre, the same bytes every time, and not the 2D
    /// picture.
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
        assert_eq!(top, w as usize, "the top row is sky");
        let centre = px(w / 2, h / 2);
        assert!(!is_sky(centre), "the centre is terrain: {centre:?}");
        assert!(centre.iter().any(|&v| v > 8), "and lit: {centre:?}");
        let terrain = (0..w * h).filter(|&k| !is_sky(px(k % w, k / w))).count();
        println!("terrain covers {terrain} of {} pixels", w * h);
        assert!(terrain > (w * h / 4) as usize, "{terrain}");
        assert_eq!(a, render(&device, &queue, &c, w, h), "the same config renders the same bytes");
        let mut flat = c.clone();
        flat.escape.terrain.enabled = false;
        assert_ne!(a, render(&device, &queue, &flat, w, h), "the terrain is not the 2D picture");
    }

    /// With a caller-owned engine (video), the footprint renders once
    /// for any number of camera moves and lighting edits, and again
    /// when the 2D picture changes.
    #[test]
    fn a_camera_move_reuses_the_footprint() {
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
                crate::renderer::RenderJob::new(c, 96, 72).with_engines(engines),
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let first = frame(&c, &mut engines);
        c.escape.cam_yaw = 0.4;
        c.escape.cam_pitch = 0.6;
        let turned = frame(&c, &mut engines);
        c.escape.terrain.height = 0.1;
        c.escape.terrain.shadow = 0.2;
        let _ = frame(&c, &mut engines);
        let renders = |e: &crate::renderer::render::RenderEngines| e.terrain.as_ref().unwrap().footprint_renders;
        assert_eq!(renders(&engines), 1, "camera, height and light edits reuse the footprint");
        assert_ne!(first, turned, "and the camera moved");
        c.escape.zoom_log2 = 2.0;
        let _ = frame(&c, &mut engines);
        assert_eq!(renders(&engines), 2, "a zoom is a new footprint");
        // The caller-owned engine's frames match a fresh render.
        assert_eq!(frame(&c, &mut engines), render(&device, &queue, &c, 96, 72));
    }

    /// An escape render of `escape` at n x n, settled: its colour and
    /// height field as `Rgba32Float` texels.
    fn settle(device: &Device, queue: &Queue, config: &FractalConfig, escape: &EscapeConfig, n: u32) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
        use crate::escape::terrain::gpu_tests::read_texture;
        let mut flame = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            device,
            queue,
            TextureFormat::Rgba8Unorm,
            n,
            n,
            &config.flame,
            config.palette_size,
        );
        flame.update_palette(
            device,
            queue,
            &config.palette,
            config.palette_rotation,
            config.palette_squeeze,
            config.palette_squeeze_mode,
            config.palette_squeeze_falloff,
            config.palette_log_strength,
            config.palette_reverse,
        );
        let mut r = EscapeRenderer::new(device, n, n);
        r.resize(device, n, n, 1);
        let mut guard = 0;
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("settle") });
            let done = r.render(
                device,
                queue,
                &mut enc,
                escape,
                flame.escape_palette_view(escape.palette_map.stepped),
                flame.palette_generation(),
            );
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            if done {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "did not settle");
        }
        let colour = bytemuck::cast_slice(&read_texture(device, queue, r.output_texture_for_test(), 0, n, n, 16)).to_vec();
        let height = if escape.terrain_active() {
            bytemuck::cast_slice(&read_texture(device, queue, r.height_texture_for_test(), 0, n, n, 16)).to_vec()
        } else {
            Vec::new()
        };
        (colour, height)
    }

    /// Deep zoom is the 2D renderer's (plan H3): at 2^60, on the
    /// perturbed floatexp path, a terrain's footprint is the 2D picture
    /// pixel for pixel, and its distance estimate is in pixels, the same
    /// sizes it has at any zoom -- no extended precision reaches the
    /// terrain.
    #[test]
    fn a_deep_zoom_footprint_is_the_2d_picture() {
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let text = std::fs::read_to_string("tests/visual/configs/escape/fe-zoom-60-edge.fflame").expect("config");
        let mut config: FractalConfig = serde_json::from_str(&text).expect("parse");
        assert!(config.escape.zoom_log2 >= 60.0);
        config.escape.shading.enabled = false;
        let n = 128u32;
        let flat = footprint_config(&config.escape);
        let mut terrain = flat.clone();
        terrain.terrain.enabled = true;
        let (a, _) = settle(&device, &queue, &config, &flat, n);
        let (b, h) = settle(&device, &queue, &config, &terrain, n);
        let worst = a
            .iter()
            .zip(&b)
            .flat_map(|(p, q)| (0..4).map(move |k| (p[k] - q[k]).abs()))
            .fold(0.0f32, f32::max);
        println!("2^60: worst colour difference {worst:.2e}");
        assert!(worst < 1e-5, "the footprint is the 2D picture: {worst}");
        let mut de: Vec<f32> = h
            .iter()
            .zip(&b)
            .filter(|(g, c)| c[3] > 0.0 && g[1] > -1e29)
            .map(|(g, _)| g[1])
            .collect();
        assert!(de.len() > (n * n / 4) as usize, "{} escaped", de.len());
        de.sort_by(f32::total_cmp);
        let (lo, mid, hi) = (de[de.len() / 20], de[de.len() / 2], de[de.len() * 19 / 20]);
        println!("2^60: distance estimate in pixels, 5% {lo:.3e} median {mid:.3e} 95% {hi:.3e}");
        assert!(de.iter().all(|v| v.is_finite() && *v > 0.0), "every estimate is a finite positive distance");
        assert!(mid > 1e-3 && mid < 1e3, "pixel-sized distances: median {mid}");
    }

    /// The 15 ms line (plan H11), measured: footprint re-render times as
    /// a dolly makes them -- each a step of zoom from the last, timed to
    /// the GPU's completion -- at three sides, on the direct path
    /// (Mandelbrot at 2^3, 2,000 iterations) and the perturbed one
    /// (`fe-zoom-60-edge`, 2^60, 30,000 iterations).
    #[test]
    #[ignore = "measurement"]
    fn footprint_render_times() {
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
            for n in [512u32, 1024, 2048, 4096, 8192] {
                if label.starts_with("perturbed") && n > 4096 {
                    continue;
                }
                let mut c = base.clone();
                c.render_mode = crate::scene::transforms::RenderMode::Escape;
                c.escape.terrain.enabled = true;
                c.escape.terrain.resolution = n;
                c.escape.shading.enabled = false;
                let mut flame = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
                    &device,
                    &queue,
                    TextureFormat::Rgba8Unorm,
                    64,
                    64,
                    &c.flame,
                    c.palette_size,
                );
                flame.update_palette(
                    &device,
                    &queue,
                    &c.palette,
                    c.palette_rotation,
                    c.palette_squeeze,
                    c.palette_squeeze_mode,
                    c.palette_squeeze_falloff,
                    c.palette_log_strength,
                    c.palette_reverse,
                );
                let mut t = EscapeTerrain::new(&device, 64, 64);
                t.choose_layout(&device, &c.escape, 64, 64, false);
                let mut times = Vec::new();
                for k in 0..8 {
                    c.escape.zoom_log2 = base.escape.zoom_log2 + 0.02 * k as f64;
                    let t0 = std::time::Instant::now();
                    loop {
                        let done = t.step_footprint(
                            &device,
                            &queue,
                            &c,
                            flame.escape_palette_view(c.escape.palette_map.stepped),
                            flame.palette_generation(),
                        );
                        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                        if done {
                            break;
                        }
                    }
                    // The first pays the shader compile and the reference.
                    if k >= 2 {
                        times.push(t0.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                times.sort_by(f64::total_cmp);
                println!(
                    "{label}, {n}x{n}: median {:.1} ms, min {:.1}, max {:.1}",
                    times[times.len() / 2],
                    times[0],
                    times[times.len() - 1]
                );
            }
        }
    }

    /// The view's footprint (plan section 11): about `supersample` texels
    /// per screen pixel where the terrain is nearest. Twice the
    /// antialiasing or twice the picture's height is about twice the
    /// side; the default framing at 1080p without antialiasing wants
    /// about 1,600; a fixed resolution is itself.
    #[test]
    fn the_wanted_footprint_follows_the_view() {
        let mut esc = EscapeConfig::default();
        esc.terrain.enabled = true;
        let base = wanted_resolution(&esc, 1920, 1080);
        println!("1080p, no antialiasing: {base}");
        assert!((1400..1900).contains(&base), "{base}");
        esc.supersample = 2;
        let ss2 = wanted_resolution(&esc, 1920, 1080);
        assert!((ss2 as f64 / base as f64 - 2.0).abs() < 0.01, "{ss2} vs {base}");
        esc.supersample = 1;
        let tall = wanted_resolution(&esc, 3840, 2160);
        assert!((tall as f64 / base as f64 - 2.0).abs() < 0.01, "{tall} vs {base}");
        esc.supersample = 8;
        assert_eq!(wanted_resolution(&esc, 3840, 2160), MAX_AUTO_RESOLUTION, "capped");
        esc.terrain.resolution = 3000;
        assert_eq!(wanted_resolution(&esc, 1920, 1080), 3000);
        // Layouts: whole tiles of at most MAX_TILE, multiples of 256.
        for (want, n, tile, per) in [(1633, 1792, 1792, 1), (3266, 3584, 1792, 2), (5000, 5376, 1792, 3), (8192, 8192, 2048, 4)] {
            let l = FootprintLayout::for_side(want, 16384);
            assert_eq!((l.n, l.tile, l.per_side), (n, tile, per), "{want}");
        }
    }

    /// A footprint rendered in tiles is the footprint rendered whole, on
    /// the direct path: the 2x2 tiles' pixels are the whole render's
    /// pixels, computed from another centre. So not byte for byte --
    /// a pixel's coordinate differs in its last bit, and where that
    /// moves an escape count the colour changes (measured: 0.05% of the
    /// raw samples, 0.5% of the colours, at 2^2 rotated) -- but nowhere
    /// more on the tiles' seams than off them.
    #[test]
    fn a_tiled_footprint_is_the_whole_footprint() {
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        use crate::escape::terrain::gpu_tests::read_texture;
        let mut c = terrain_config();
        c.escape.zoom_log2 = 2.0;
        c.escape.rotation = 0.4;
        let mut flame = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            TextureFormat::Rgba8Unorm,
            64,
            64,
            &c.flame,
            c.palette_size,
        );
        flame.update_palette(
            &device,
            &queue,
            &c.palette,
            c.palette_rotation,
            c.palette_squeeze,
            c.palette_squeeze_mode,
            c.palette_squeeze_falloff,
            c.palette_log_strength,
            c.palette_reverse,
        );
        let n = 512u32;
        let build = |layout: FootprintLayout| {
            let mut t = EscapeTerrain::new(&device, 64, 64);
            t.force_layout(&device, layout);
            while !t.step_footprint(&device, &queue, &c, flame.escape_palette_view(false), flame.palette_generation()) {
                let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            }
            let (raw, albedo) = t.terrain_for_test().tile_textures_for_test().unwrap();
            let raw: Vec<f32> = bytemuck::cast_slice(&read_texture(&device, &queue, raw, 0, n, n, 4)).to_vec();
            let albedo: Vec<u16> = bytemuck::cast_slice(&read_texture(&device, &queue, albedo, 0, n, n, 8)).to_vec();
            (raw, albedo)
        };
        let (whole_raw, whole_albedo) = build(FootprintLayout { n, tile: n, per_side: 1 });
        let (tiled_raw, tiled_albedo) = build(FootprintLayout { n, tile: n / 2, per_side: 2 });
        let (mut off, mut worst_a) = (0usize, 0.0f32);
        for k in 0..(n * n) as usize {
            let (a, b) = (whole_raw[k], tiled_raw[k]);
            // Sentinels must agree exactly; distances to f32's agreement,
            // away from the set where a pixel's estimate is percents
            // either way.
            if a.abs() >= 1e29 || b.abs() >= 1e29 || a == 0.0 || b == 0.0 {
                if a != b {
                    off += 1;
                }
            } else if a > 0.05 && (a - b).abs() > 1e-3 * a {
                off += 1;
            }
            for ch in 0..4 {
                let (x, y) = (
                    half::f16::from_bits(whole_albedo[k * 4 + ch]).to_f32(),
                    half::f16::from_bits(tiled_albedo[k * 4 + ch]).to_f32(),
                );
                worst_a = worst_a.max((x - y).abs());
            }
        }
        let (mut colour_off, mut total) = (0usize, 0usize);
        for k in 0..(n * n) as usize {
            total += 1;
            if (0..3).any(|ch| {
                (half::f16::from_bits(whole_albedo[k * 4 + ch]).to_f32() - half::f16::from_bits(tiled_albedo[k * 4 + ch]).to_f32()).abs() > 0.02
            }) {
                colour_off += 1;
            }
        }
        // Where they are: on the tiles' seams, or scattered.
        let mut near_seam = 0usize;
        for k in 0..(n * n) as usize {
            let (x, y) = ((k as u32) % n, (k as u32) / n);
            let differs = (0..3).any(|ch| {
                (half::f16::from_bits(whole_albedo[k * 4 + ch]).to_f32() - half::f16::from_bits(tiled_albedo[k * 4 + ch]).to_f32()).abs() > 0.02
            });
            if differs && ((x as i32 - (n / 2) as i32).abs() <= 2 || (y as i32 - (n / 2) as i32).abs() <= 2) {
                near_seam += 1;
            }
        }
        println!("tiled vs whole: {off} raw samples off, {colour_off} of {total} colours off by > 0.02 (worst {worst_a:.3}), {near_seam} of them within 2 px of a seam");
        assert!(off * 1000 < (n * n) as usize, "{off} raw samples disagree");
        assert!(colour_off * 100 < total, "{colour_off} colours disagree");
        // The seams' 2-pixel bands are 5 rows and columns of n: their
        // share of the differences, were they placed at random.
        let chance = colour_off as f64 * (2.0 * 5.0 * n as f64) / total as f64;
        assert!((near_seam as f64) < 2.0 * chance + 10.0, "{near_seam} on the seams, {chance:.0} by chance");
    }

    /// As a video renders: ONE flame renderer across the frames, whose
    /// config load writes the palette texture every frame. An unchanged
    /// palette still reuses the footprint; a changed one does not.
    #[test]
    fn a_video_reuses_the_footprint_across_frames() {
        let Some((device, queue)) = crate::escape::terrain::gpu_tests::device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut c = terrain_config();
        let mut engines = crate::renderer::render::RenderEngines::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            TextureFormat::Rgba8Unorm,
            96,
            72,
            &c.flame,
            c.palette_size,
        );
        renderer.set_sticky_enabled(false);
        let mut frame = |c: &FractalConfig, engines: &mut crate::renderer::render::RenderEngines| {
            let job = crate::renderer::RenderJob::new(c, 96, 72).with_engines(engines);
            pollster::block_on(crate::renderer::render_with(
                &mut renderer,
                &device,
                &queue,
                job,
                &mut crate::renderer::NoProgress,
            ))
            .expect("render")
            .rgba_data
        };
        let renders = |e: &crate::renderer::render::RenderEngines| e.terrain.as_ref().unwrap().footprint_renders;
        let first = frame(&c, &mut engines);
        for k in 1..4 {
            c.escape.cam_yaw = 0.1 * k as f32;
            let _ = frame(&c, &mut engines);
        }
        assert_eq!(renders(&engines), 1, "four frames, one footprint");
        c.escape.cam_yaw = 0.0;
        c.palette_rotation = 0.25;
        let rotated = frame(&c, &mut engines);
        assert_eq!(renders(&engines), 2, "a palette change is a new footprint");
        assert_ne!(first, rotated, "and it shows");
    }
}
