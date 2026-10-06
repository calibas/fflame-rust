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
pub const MIN_RESOLUTION: u32 = 16;

/// The footprint's side for this config on this device: the config's,
/// within the device's texture limit.
pub fn footprint_side(device: &Device, escape: &EscapeConfig) -> u32 {
    escape
        .terrain
        .resolution
        .clamp(MIN_RESOLUTION, device.limits().max_texture_dimension_2d.max(MIN_RESOLUTION))
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
    // The source decides what the iterate pass writes; the rest is the
    // ingest's and the lighting's.
    let source = k.terrain.source;
    k.terrain = d.terrain.clone();
    k.terrain.enabled = true;
    k.terrain.source = source;
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
        "{flame}|{:?}|{}|{}|{}|{:?}|{}|{}|{}",
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
    }
}

/// How the footprint becomes a tile. `derivative` is whether the
/// footprint's iterate pass compiled one: without it the distance
/// source wrote the escape count (`esc_terrain_source`), and the
/// ingest must read it as one.
pub fn terrain_ingest(config: &FractalConfig, n: u32, derivative: bool) -> TerrainIngest {
    let t = &config.escape.terrain;
    let source = match t.source {
        TerrainSource::Distance if derivative => 8,
        TerrainSource::Distance | TerrainSource::EscapeCount => 9,
        TerrainSource::Relief => config.escape.shading.field.to_gpu(),
    };
    TerrainIngest {
        source,
        height: t.height * n as f32,
        de_width: t.de_width * n as f32,
        hole: t.interior == TerrainInterior::Hole,
        background: config.background_color,
    }
}

/// The escape terrain: the footprint's renderer, the terrain renderer,
/// and what the tile was made from, so only a change to the 2D picture
/// re-renders the footprint.
pub struct EscapeTerrain {
    footprint: EscapeRenderer,
    terrain: TerrainRenderer,
    n: u32,
    /// The footprint's picture and palette keys, once it has settled.
    rendered: Option<(EscapeConfig, String)>,
    /// The footprint the last chunk rendered, while it settles.
    pending: Option<(EscapeConfig, String)>,
    ingested: Option<TerrainIngest>,
    /// Footprint renders run to settlement, for the cache's gate.
    pub footprint_renders: u32,
    /// When the footprint in progress submitted its first chunk.
    footprint_started: Option<web_time::Instant>,
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
        let n = MIN_RESOLUTION;
        EscapeTerrain {
            footprint: EscapeRenderer::new(device, n, n),
            terrain: TerrainRenderer::new(device, out_w, out_h),
            n,
            rendered: None,
            pending: None,
            ingested: None,
            footprint_renders: 0,
            footprint_started: None,
            footprint_done: std::sync::Arc::new(std::sync::Mutex::new(None)),
            footprint_ms: None,
            viewport_key: None,
            viewport_samples: 0,
        }
    }

    /// Why a terrain of this config cannot be held at `w x h`: the
    /// footprint's escape render, or the terrain's own buffers.
    pub fn allocation_error(device: &Device, escape: &EscapeConfig, w: u32, h: u32) -> Option<String> {
        let n = footprint_side(device, escape);
        EscapeRenderer::allocation_error(device, &footprint_config(escape), n, n, 1)
            .or_else(|| TerrainRenderer::allocation_error(device, w, h))
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

    /// The footprint's side, sized for this config: resizes the
    /// footprint renderer when it changed.
    pub fn size_footprint(&mut self, device: &Device, escape: &EscapeConfig) -> u32 {
        let n = footprint_side(device, escape);
        if n != self.n {
            self.footprint.resize(device, n, n, 1);
            self.n = n;
            self.rendered = None;
            self.pending = None;
        }
        n
    }

    /// Whether the footprint must be rendered again for this config:
    /// its 2D picture or the palette changed.
    pub fn footprint_stale(&self, config: &FractalConfig) -> bool {
        self.rendered.as_ref() != Some(&(footprint_key(&config.escape), palette_key(config)))
    }

    /// One chunk of the footprint's render, into `encoder`; true once
    /// it has settled. The caller submits, and then calls [`ingest`]
    /// (Self::ingest) once it has.
    pub fn render_footprint(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        config: &FractalConfig,
        palette_view: &TextureView,
        palette_generation: u64,
    ) -> bool {
        let key = (footprint_key(&config.escape), palette_key(config));
        self.footprint_started.get_or_insert_with(web_time::Instant::now);
        let settled = self.footprint.render(
            device,
            queue,
            encoder,
            &footprint_config(&config.escape),
            palette_view,
            palette_generation,
        );
        self.pending = Some(key);
        settled
    }

    /// Forget the footprint: something it was drawn with that its key
    /// does not see (a texture's image arriving) changed.
    pub fn invalidate_footprint(&mut self) {
        self.rendered = None;
    }

    /// Whether a footprint is part-way through its render.
    pub fn footprint_in_progress(&self) -> bool {
        self.pending.is_some()
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

    /// Make the tile from the settled footprint, if it or the ingest
    /// changed. Submits its own work, after the footprint's.
    pub fn ingest(&mut self, device: &Device, queue: &Queue, config: &FractalConfig) {
        let n = self.n;
        let derivative = self.footprint.derivative_active(&footprint_config(&config.escape));
        let ingest = terrain_ingest(config, n, derivative);
        let fresh = self.pending.is_some();
        if !fresh && self.ingested == Some(ingest) {
            return;
        }
        if let Some(key) = self.pending.take() {
            self.rendered = Some(key);
            self.footprint_renders += 1;
            // Time it to the GPU's completion: the chunks before this
            // are queued, not done.
            if let Some(t0) = self.footprint_started.take() {
                let slot = std::sync::Arc::clone(&self.footprint_done);
                queue.on_submitted_work_done(move || {
                    if let Ok(mut g) = slot.lock() {
                        *g = Some(t0.elapsed().as_secs_f32() * 1000.0);
                    }
                });
            }
        }
        self.terrain
            .set_tile_from_escape(device, queue, self.footprint.output_view(), self.footprint.height_view(), n, n, &ingest);
        self.ingested = Some(ingest);
    }

    /// The view the tile is seen through: the config's, over whichever
    /// footprint the tile holds (`terrain_camera_over` while one lags).
    fn current_view(&self, config: &FractalConfig, jitter: [f32; 2]) -> TerrainView {
        let camera = match &self.rendered {
            Some((fp, _)) => terrain_camera_over(&config.escape, fp, self.n),
            None => terrain_camera(&config.escape, self.n),
        };
        terrain_view_with(config, camera, self.n, jitter)
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
        let view = terrain_view(config, self.n, jitter);
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

    /// Only the 2D picture keys the footprint: the camera, the lights,
    /// the heights and the interior do not, the height source does.
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
        b.terrain.interior = TerrainInterior::Hole;
        b.terrain.shadow = 0.0;
        b.terrain.occlusion = 0.1;
        assert_eq!(footprint_key(&a), footprint_key(&b));
        b.terrain.source = TerrainSource::EscapeCount;
        assert_ne!(footprint_key(&a), footprint_key(&b));
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
            for n in [512u32, 1024, 2048] {
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
                t.size_footprint(&device, &c.escape);
                let mut times = Vec::new();
                for k in 0..8 {
                    c.escape.zoom_log2 = base.escape.zoom_log2 + 0.02 * k as f64;
                    let t0 = std::time::Instant::now();
                    loop {
                        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
                        let done = t.render_footprint(
                            &device,
                            &queue,
                            &mut enc,
                            &c,
                            flame.escape_palette_view(c.escape.palette_map.stepped),
                            flame.palette_generation(),
                        );
                        queue.submit(std::iter::once(enc.finish()));
                        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                        if done {
                            break;
                        }
                    }
                    t.ingest(&device, &queue, &c);
                    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
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
