//! Mode D's path tracer (docs/archive/terrain/heightfield-3d.md, T3c): a
//! solid's distance field through the core the terrain renders with
//! (`crate::escape::path_core`), on the same camera and lights as the
//! walk.
//!
//! A sample costs several walks -- the camera ray's march, a shadow
//! march a light, a bounce's march, each surface's normal and colouring
//! -- and a walk at 1080p is already hundreds of milliseconds. So the
//! pass is BANDED: a sample is a pass over the frame's rows in
//! dispatches the watchdog never notices.
//!
//! The bands are sized by MEASUREMENT, under a cap that is not. Each
//! call's time on the GPU is recorded against the rows it traced, and a
//! band is as many rows as a target time holds at those rows' costs --
//! the last pass's where the view has been traced since it last changed;
//! a row not yet measured is priced at the walk's cost model or twice the
//! costliest row since the picture restarted, whichever is more. The model,
//! calibrated to a band of about 300 ms, also CAPS every dispatch, rows
//! times samples: a measurement can only make a band smaller than the
//! model's. (Sizing by the model alone made one-row bands on a machine
//! whose tuning file had halved the walk's budget four times -- 540
//! frames to a sample, which read in the app as a path tracer stuck on
//! its first. Sizing by measurement alone lost the device: completion
//! callbacks run when wgpu next looks, so bands finishing together shared
//! one timestamp, the later ones measured nothing, and a whole frame went
//! out at sixty-four samples.)
//!
//! Losing the device a second way is what the rest of this guards
//! against (measured on a GTX 1660 SUPER, the shipped Menger sponge at
//! 1080p, 2026-10-08: a sample takes about five seconds, and the app
//! lost the device every two or three -- recovered, traced half the
//! frame, lost it again):
//! - **A row nobody has measured was priced off the cheapest.** The
//!   first pass meets the sky's rows first, at a tenth of a millisecond
//!   each; twice that priced the solid's rows, a hundred times dearer,
//!   and the bands that entered it were the cap's -- half a second each,
//!   several a frame.
//! - **Calls overlapped.** The viewport sent a frame's call before the
//!   last had finished, so the GPU's queue held seconds of bands, and a
//!   call finishing with the one before it measured next to nothing --
//!   pricing its rows as free. The viewport now sends a call only once
//!   the last has finished (`SolidPath::busy`), so each is measured alone.
//! - **No dispatch is too small to be too long.** Eight rows is the
//!   least a band holds, and at enough bounces and width eight rows
//!   outlast the watchdog. A band that would pass [`DISPATCH_CEILING_MS`]
//!   goes across the frame in tiles of whole workgroups.
//! - **A loss was never learned from.** The breakers that shrink the
//!   walk count a loss only while ITS bands are out, so the path
//!   tracer's losses went unrecorded and every recovery sent the same
//!   bands again. Its calls are counted now (`PATH_CALLS_IN_FLIGHT`), and
//!   a loss among them halves the ceiling and the cap -- persisted, in
//!   the tuning file, as `path_shift`. A call measured past 700 ms halves
//!   them for the session.
//!
//! Bands are whole multiples of the workgroup's eight rows -- a shorter
//! one costs the same -- and hold at least [`MIN_BAND_PX`] pixels: a
//! band costs at least its slowest pixel's path whatever its size, so on
//! a narrow view eight rows were too few threads to fill the GPU and
//! more cost next to nothing (measured at 320 wide: a sample in eight-row
//! bands 714 ms, in one dispatch 125).

use crate::config::escape::RenderTier;
use crate::config::FractalConfig;
use crate::escape::path_core::{self, PathParamsGpu, PathSum, PATH_SHOW_SAMPLES};
use egui_wgpu::wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType, BufferDescriptor,
    BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipelineDescriptor, Device,
    PipelineLayoutDescriptor, Queue, SamplerBindingType, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    Texture, TextureSampleType, TextureView, TextureViewDimension,
};

use super::{assembler, EscapeParamsGpu, EscapeRenderer};
use std::sync::{Arc, Mutex};

/// The GPU time a viewport frame gives the path tracer, in ms: the UI
/// stays usable (about 20 frames a second while it runs), and several
/// bands go in a frame when they are cheap.
pub const VIEWPORT_FRAME_MS: f32 = 40.0;

/// The GPU time an export's call gives it, in ms: far inside the
/// watchdog, long enough that a call's overhead is noise.
pub const EXPORT_FRAME_MS: f32 = 250.0;

/// The viewport's call once nothing has been touched for a while
/// (escape-sim-followups, the solid path tracer's speed, 1): the
/// export's. A call of a few rows leaves most of the GPU idle -- a 1080p
/// row of the Menger is about 11 ms, and only bands of tens of rows fill
/// the card -- so a sample took 6.5 s in 40 ms calls, 6.2 in 100, 5.8 in
/// 160 and 4.9 in 250 (`menger_band_costs` at 1920). The UI redraws about
/// four times a second meanwhile, which nobody is watching it do.
pub const IDLE_FRAME_MS: f32 = EXPORT_FRAME_MS;

/// The walk's band, as its model is calibrated: about 300 ms.
const MODEL_BAND_MS: f32 = 300.0;

/// A call past this, measured, halves the cap and the ceiling for the
/// session.
const SLOW_CALL_MS: f32 = 700.0;

/// No dispatch is planned past this, in ms, before the path shift: a
/// quarter of the watchdog's two seconds, so a measurement off by four
/// still clears it. A band of the fewest rows priced past it goes across
/// the frame in tiles.
const DISPATCH_CEILING_MS: f32 = 500.0;

/// A tile's price is its share of its band's width times this: a band's
/// cost is not spread evenly across it (the solid in the middle, the sky
/// at the sides).
const TILE_SAFETY: f32 = 3.0;

/// The fewest pixels a band holds: eight rows of a 1080p frame, about as
/// many threads as a GTX 1660 SUPER keeps in flight on this shader --
/// eight rows of 1920 measured 50 to 85 ms in the sponge, eight of 320
/// the same.
const MIN_BAND_PX: u32 = 8 * 1920;

/// The fewest rows a band holds at width `w`: [`MIN_BAND_PX`], in whole
/// workgroups.
fn min_band_rows(w: u32) -> u32 {
    (MIN_BAND_PX.div_ceil(w.max(1))).div_ceil(8).max(1) * 8
}

/// One dispatch: rows from `y0`, columns from `x0`, at `n` samples. Or
/// -- sampling adaptively once a check has listed the pixels still
/// sampling (`list`: the entries of it taken to be there, and the bound
/// the pass goes to) -- `rows` entries of the list from `y0`.
#[derive(Clone, Copy, Debug)]
struct Band {
    y0: u32,
    rows: u32,
    x0: u32,
    cols: u32,
    n: u32,
    list: Option<(u32, u32)>,
}

/// Calls the GPU has finished, for the sizing: (generation, the
/// dispatches, ms), the ms from the later of the call's first submission
/// and the previous call's completion. One measurement a call: callbacks
/// run when wgpu next looks, so bands finishing together could not be
/// told apart. And the calls not yet finished: the viewport waits for
/// none before its next, so each is measured alone.
#[derive(Default)]
struct BandClock {
    done: Vec<(u32, Vec<Band>, f32)>,
    last: Option<web_time::Instant>,
    outstanding: u32,
}

/// What the solid's path tracer keeps between frames.
pub(super) struct SolidPath {
    sum: PathSum,
    output: (Texture, TextureView),
    width: u32,
    height: u32,
    /// Group 0 of the path pipelines: the escape uniform, the palette
    /// and its sampler, the sum and the path's uniform.
    layout: BindGroupLayout,
    /// The escape uniform at the output's size.
    params: Buffer,
    /// Rows of the pass in progress already traced: a sample is a pass
    /// over every row.
    row: u32,
    /// The band in progress across its width (`DISPATCH_CEILING_MS`): its
    /// rows from `row` and the columns done (0: none started).
    band_rows: u32,
    col: u32,
    /// The pass in progress over the last check's list (sampling
    /// adaptively): its entries already traced.
    entry: u32,
    /// An entry-sample's measured cost in ms since the picture last
    /// restarted (0: not yet).
    entry_ms: f32,
    /// Whether the picture shown is this one, not the walk's.
    shown: bool,
    /// The sum is done and its last picture resolved (`resolve_now`):
    /// adaptive sampling learns that every pixel has converged only when a
    /// check's count comes back, after the call that ran it.
    finished: bool,
    /// Each row's measured cost in ms a full-width sample since the
    /// picture last restarted (0: not yet), and the most any row has
    /// cost.
    row_ms: Vec<f32>,
    max_ms: f32,
    /// Restarts so far: a band measured before one is not this picture's.
    generation: u32,
    clock: Arc<Mutex<BandClock>>,
    /// The path tracer's settings the sum was gathered under.
    settings: Option<String>,
    /// Every dispatch's rows times samples, the cap it was under, and its
    /// columns.
    #[cfg(test)]
    dispatched: Vec<(u32, u32, u32)>,
    /// A ceiling in place of [`DISPATCH_CEILING_MS`]'s, for a test to
    /// make tiles without the process-wide shift.
    #[cfg(test)]
    ceiling: Option<f32>,
}

impl SolidPath {
    fn new(device: &Device, w: u32, h: u32) -> Self {
        let entry = |binding: u32, ty: BindingType| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let uniform = BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Escape Solid Path"),
            entries: &[
                entry(0, uniform),
                entry(
                    2,
                    BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(3, BindingType::Sampler(SamplerBindingType::Filtering)),
                entry(
                    8,
                    BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(9, uniform),
                entry(
                    10,
                    BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                // The HDRI sky's map and its sampler (`ifs_env`).
                entry(
                    14,
                    BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(15, BindingType::Sampler(SamplerBindingType::Filtering)),
            ],
        });
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Solid Path Params"),
            size: std::mem::size_of::<EscapeParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        SolidPath {
            sum: PathSum::new(device, w * h),
            output: EscapeRenderer::create_output(device, w, h),
            width: w,
            height: h,
            layout,
            params,
            row: 0,
            band_rows: 0,
            col: 0,
            entry: 0,
            entry_ms: 0.0,
            shown: false,
            finished: false,
            row_ms: vec![0.0; h as usize],
            max_ms: 0.0,
            generation: 0,
            clock: Arc::new(Mutex::new(BandClock::default())),
            settings: None,
            #[cfg(test)]
            dispatched: Vec::new(),
            #[cfg(test)]
            ceiling: None,
        }
    }

    /// Start the picture over: its samples, and what its rows cost.
    fn restart(&mut self) {
        self.sum.reset();
        self.row = 0;
        self.band_rows = 0;
        self.col = 0;
        self.entry = 0;
        self.entry_ms = 0.0;
        self.shown = false;
        self.finished = false;
        self.row_ms.iter_mut().for_each(|c| *c = 0.0);
        self.max_ms = 0.0;
        self.generation = self.generation.wrapping_add(1);
    }

    /// Whether a call is still on the GPU: the viewport sends its next
    /// once it is not, so each call is measured alone and the queue
    /// never holds more than one.
    fn busy(&self) -> bool {
        self.clock.lock().map_or(false, |c| c.outstanding > 0)
    }

    /// The pixels the pass in progress has reached.
    fn split(&self) -> path_core::PathSplit {
        path_core::PathSplit { row: self.row, band_end: self.row + self.band_rows, col: self.col }
    }

    /// Fold in the calls the GPU has finished since the last look: a
    /// call's time spread evenly over the rows it traced, a tile's as
    /// the whole row's at its share of the width.
    fn collect(&mut self) {
        let done = self.clock.lock().map(|mut c| std::mem::take(&mut c.done)).unwrap_or_default();
        let w = self.width.max(1) as f32;
        for (generation, bands, ms) in done {
            if ms > SLOW_CALL_MS {
                let shift = super::PATH_BUDGET_SHIFT.load(std::sync::atomic::Ordering::Relaxed);
                super::PATH_BUDGET_SHIFT.store((shift + 1).min(6), std::sync::atomic::Ordering::Relaxed);
                log::warn!("escape solid path: a call took {ms:.0} ms; halving its bands for the session");
            }
            if generation != self.generation {
                continue;
            }
            // A call that traced the list prices an entry-sample by the
            // entries taken to be there (past the list's end threads return
            // at once and cost nothing to speak of), and prices no row.
            if bands.iter().any(|b| b.list.is_some()) {
                if bands.iter().all(|b| b.list.is_some()) {
                    let entries: f32 = bands.iter().map(|b| (b.list.map_or(0, |(real, _)| real) * b.n) as f32).sum();
                    if entries > 0.0 {
                        self.entry_ms = ms / entries;
                    }
                }
                continue;
            }
            let units: f32 = bands.iter().map(|b| (b.rows * b.n) as f32 * b.cols as f32 / w).sum();
            if !(units > 0.0) {
                continue;
            }
            let per = ms / units;
            for b in &bands {
                for c in self.row_ms.iter_mut().skip(b.y0 as usize).take(b.rows as usize) {
                    *c = per;
                }
            }
            self.max_ms = self.max_ms.max(per);
        }
    }

    /// A row's price in ms a full-width sample: measured, or -- not yet
    /// -- the model's `model` or twice the costliest row since the
    /// picture restarted, whichever is more. (Twice the costliest alone
    /// priced the solid's rows off the sky's, a hundred times cheaper.)
    fn price(&self, y: u32, model: f32) -> f32 {
        match self.row_ms[y as usize] {
            m if m > 0.0 => m,
            _ => model.max(2.0 * self.max_ms),
        }
    }

    /// The rows from `y0` a band of `target` ms holds at one sample, at
    /// each row's [`Self::price`], and that price: the fewest a band
    /// holds ([`min_band_rows`]), then whole workgroups while they fit --
    /// to the frame's end, and never more than `cap`, the model's band.
    fn plan(&self, y0: u32, target: f32, model: f32, cap: u32) -> (u32, f32) {
        let h = self.height;
        let cap = cap.max(8);
        let mut rows = 0u32;
        let mut ms = 0.0f32;
        while y0 + rows < h && rows < cap {
            let block = if rows == 0 { min_band_rows(self.width).min(cap) } else { 8.min(cap - rows) };
            let block = block.min(h - (y0 + rows));
            let c: f32 = (y0 + rows..y0 + rows + block).map(|y| self.price(y, model)).sum();
            if rows > 0 && ms + c > target {
                break;
            }
            ms += c;
            rows += block;
        }
        (rows.max(1), ms)
    }

    /// A whole frame's cost at one sample, once every row is measured.
    fn frame_ms(&self) -> Option<f32> {
        self.row_ms.iter().all(|&c| c > 0.0).then(|| self.row_ms.iter().sum())
    }

    /// The output's size; the sum starts over when it changes.
    fn resize(&mut self, device: &Device, w: u32, h: u32) {
        if (w, h) == (self.width, self.height) {
            return;
        }
        self.output.0.destroy();
        self.output = EscapeRenderer::create_output(device, w, h);
        self.sum.ensure(device, w * h);
        self.width = w;
        self.height = h;
        self.row_ms = vec![0.0; h as usize];
        self.restart();
    }

    pub(super) fn view(&self) -> Option<&TextureView> {
        self.shown.then_some(&self.output.1)
    }

    pub(super) fn destroy(&self) {
        self.sum.destroy();
        self.output.0.destroy();
        self.params.destroy();
    }
}

impl EscapeRenderer {
    /// Start the solid's path-traced picture over, and show the walk's.
    pub fn reset_solid_path(&mut self) {
        if let Some(p) = self.solid_path.as_mut() {
            p.restart();
        }
    }

    /// Complete path-traced samples of the solid so far.
    pub fn solid_path_samples(&self) -> u32 {
        self.solid_path.as_ref().map_or(0, |p| p.sum.count())
    }

    /// Samples so far with a part-done pass counted as its share of the
    /// rows.
    pub fn solid_path_progress(&self) -> f32 {
        self.solid_path
            .as_ref()
            .map_or(0.0, |p| p.sum.count() as f32 + p.row as f32 / p.height.max(1) as f32)
    }

    /// Whether the output is the path tracer's picture.
    pub fn solid_path_shown(&self) -> bool {
        self.solid_path.as_ref().is_some_and(|p| p.shown)
    }

    /// Show the path tracer's picture, or the walk's.
    pub fn show_solid_path(&mut self, on: bool) {
        if let Some(p) = self.solid_path.as_mut() {
            p.shown = on;
        }
    }

    /// One frame of a solid's viewport in the config's tier (plan
    /// section 8): Auto path traces once the walk has settled -- the
    /// caller asks only then -- and shows the path tracer's picture from
    /// [`PATH_SHOW_SAMPLES`]; Path Traced shows it from the first band.
    /// A call has `frame_ms` of GPU time: [`VIEWPORT_FRAME_MS`] while the
    /// user is at it, [`IDLE_FRAME_MS`] once they are not. True while
    /// there is more to do.
    pub fn trace_solid_viewport(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, palette_view: &TextureView, frame_ms: f32) -> bool {
        let tier = config.escape.solid_tier;
        if tier == RenderTier::Lit {
            self.show_solid_path(false);
            return false;
        }
        let target = config.escape.path.samples.max(1);
        // One call on the GPU at a time (`SolidPath::busy`): until the
        // last is done, nothing new, and more to do.
        let busy = self.solid_path.as_ref().is_some_and(|p| p.busy());
        let more = busy || self.trace_solid(device, queue, config, palette_view, target, frame_ms);
        let show = tier == RenderTier::PathTraced || self.solid_path_samples() >= PATH_SHOW_SAMPLES.min(target);
        self.show_solid_path(show);
        more
    }

    /// The export's picture: `samples` path-traced samples of the solid,
    /// in bands of the full budget, `wait` called between them (a
    /// blocking poll on the desktop). Shown once done.
    pub fn render_solid_still(
        &mut self,
        device: &Device,
        queue: &Queue,
        config: &FractalConfig,
        palette_view: &TextureView,
        samples: u32,
        mut wait: impl FnMut() -> bool,
    ) {
        self.reset_solid_path();
        // False from `wait` stops it where it is (a cancelled export).
        while self.trace_solid(device, queue, config, palette_view, samples.max(1), EXPORT_FRAME_MS) {
            if !wait() {
                break;
            }
        }
        wait();
        let any = self.solid_path_samples() > 0;
        self.show_solid_path(any);
    }

    /// About `frame_ms` of the solid's path tracer on the GPU: bands of
    /// rows at one sample, a band too costly for one dispatch in tiles
    /// across it -- or, where a whole frame fits, every row at as many
    /// samples as fit, up to `target` -- then the mean into the output.
    /// Submits its own work. True while the sum holds fewer than `target`
    /// samples.
    pub fn trace_solid(
        &mut self,
        device: &Device,
        queue: &Queue,
        config: &FractalConfig,
        palette_view: &TextureView,
        target: u32,
        frame_ms: f32,
    ) -> bool {
        let escape = &config.escape;
        let Some(def) = crate::escape::ifs::get_ifs(&escape.formula).filter(|d| d.solid) else {
            return false;
        };
        if self.ifs.is_none() {
            return false;
        }
        let (w, h) = (self.out_width.max(1), self.out_height.max(1));
        match self.solid_path.as_mut() {
            Some(p) => p.resize(device, w, h),
            None => self.solid_path = Some(SolidPath::new(device, w, h)),
        }
        // What lights the scene that no re-render reports: the tone
        // map's exposure and gamma enter the sky's light
        // (`path_core::shown`) and are tone-map-only edits, so a finished
        // sum kept the old light under the new tone map. A change starts
        // it over, as a terrain's tiers do. (The lens's focus is in the
        // config's units; the distance only scales it.)
        let key = path_core::path_settings(config, 1.0).restart_key();
        if let Some(p) = self.solid_path.as_mut() {
            if p.settings.as_deref() != Some(key.as_str()) {
                if p.settings.is_some() {
                    p.restart();
                }
                p.settings = Some(key);
            }
        }
        // Done: every sample in, or -- sampling adaptively -- every pixel
        // converged at a check whose count has come back since, under the
        // threshold the config has now (a new one reopens it).
        if let Some(p) = self.solid_path.as_mut() {
            if p.sum.retune(path_core::path_settings(config, 1.0).noise_threshold) {
                p.finished = false;
            }
            p.sum.poll(device);
            if p.sum.done(target) {
                if !p.finished {
                    p.finished = true;
                    let split = p.split();
                    p.sum.resolve_now(device, queue, &p.output.1, w, h, split);
                }
                return false;
            }
        }
        // What the walk has before its first band.
        self.ensure_lens(device, queue, escape);
        self.ensure_ifs_seeds(escape);
        self.upload_ifs(device, queue, true);
        self.upload_ifs_chain(device, queue);

        // The escape uniform at the output's size: the path traces
        // display pixels, its jitter the antialiasing.
        let mut params = self.params_for(escape);
        params.width = w;
        params.height = h;
        params.stride = 1;
        params.tile_y0 = 0;
        params.flags &= !8;
        // The eye's offset from the target, and the field of view.
        let eye = params.fdata[2];
        let distance = (eye[0] * eye[0] + eye[1] * eye[1] + eye[2] * eye[2]).sqrt();
        let mut settings = path_core::path_settings(config, distance);
        // No sky's map loaded (this build cannot, or it failed): the
        // background lights, as it did.
        if self.environment_in_use().is_none() {
            settings.env = [0.0; 4];
        }
        // A sky's sun is a light the clamps follow (`path_settings` knows
        // the rig's alone): with the rig's lights it draws (an untouched
        // rig's stand-in gives way to it), as bright as their sum.
        if let (Some(set), Some(sun)) = (self.environment_in_use(), self.environment.as_ref().and_then(|e| e.sun)) {
            let rig = &config.solid_shading;
            let lights: f32 = if rig.rig_untouched() { 0.0 } else { rig.lights.iter().filter(|l| l.enabled).map(|l| l.intensity.max(0.0)).sum() };
            let brightest = lights + sun.power(set.scale).iter().cloned().fold(0.0f32, f32::max);
            settings.clamp = settings.clamp.max(10.0 * brightest);
            settings.clamp_indirect = settings.clamp_indirect.max(brightest);
        }
        #[cfg(test)]
        {
            settings.seed = settings.seed.wrapping_add(self.path_seed_offset);
            settings.mis = !self.path_no_mis;
            if self.path_no_clamp {
                settings.clamp = 1.0e30;
                settings.clamp_indirect = 1.0e30;
            }
        }
        // The studio the bounces see, the config's (as the lit tier's).
        params.fdata[32][0] = settings.studio;
        let per_ray = 2.0 * (eye[3] * 0.5).tan() / h as f32;
        let param = |name: &str, fallback: f32| {
            escape.formula_params.get(name).copied().unwrap_or_else(|| {
                def.parameters.iter().find(|p| p.name == name).map_or(fallback, |p| p.default)
            })
        };
        let radius = path_core::light_radius(param("shadow_sharpness", 12.0));
        // The model's calibrated band prices a row no call has measured;
        // halved by the path shift, it caps every dispatch, rows times
        // samples, as the shift halves the ceiling. (Not the walk's
        // shift: see `PATH_BUDGET_SHIFT`.)
        let model_rows = self.solid_path_rows(escape, def, settings.bounces, w);
        let model_ms = MODEL_BAND_MS / model_rows as f32;
        let shift = super::path_budget_shift();
        let cap = (model_rows >> shift).max(8);
        let ceiling = DISPATCH_CEILING_MS / (1u32 << shift) as f32;

        let adaptive = {
            let path = self.solid_path.as_mut().expect("made above");
            // The denoiser's guides start with the sum; the pass gathers
            // them only where they fit (`set_guided`).
            if path.sum.set_guided(device, settings.denoise) {
                path.restart();
            }
            settings.denoise = path.sum.guided();
            path.sum.set_material(&settings);
            // Adaptive sampling's statistics likewise (V2).
            #[cfg(test)]
            if let Some((min, halves)) = self.path_adapt {
                path.sum.tune_adaptive(min, halves);
            }
            if path.sum.set_adaptive(device, settings.noise_threshold > 0.0) {
                path.restart();
            }
            path.sum.adaptive()
        };
        if !adaptive {
            settings.noise_threshold = 0.0;
        }
        let key = self.ensure_solid_path_pipeline(device, escape, def);
        let ifs_group = self.ifs_bind_group(device);
        let (env_view, env_sampler) = {
            let (v, s) = self.environment_bindings(device);
            (v.clone(), s.clone())
        };
        let path = self.solid_path.as_mut().expect("made above");
        path.collect();
        #[cfg(test)]
        let ceiling = path.ceiling.unwrap_or(ceiling);
        let mut traced: Vec<Band> = Vec::new();
        let mut first_submit: Option<web_time::Instant> = None;
        queue.write_buffer(&path.params, 0, bytemuck::bytes_of(&params));
        let mut spent = 0.0f32;
        // A list's chunk is capped in pixel-samples: the model's band,
        // halved, as its price is doubled.
        let cap_entries = (cap as u64 * w as u64 / 2).max(64);
        while spent < frame_ms && traced.len() < 16 && !path.sum.done(target) {
            let count = path.sum.count();
            let room = (frame_ms - spent).min(ceiling);
            let next = path.sum.next_check().map_or(u32::MAX, |c| c - count);
            // A whole frame at several samples, where the measurements say
            // they fit and the model's cap -- rows times samples -- agrees.
            let whole = (!path.sum.listed() && path.row == 0 && path.band_rows == 0)
                .then(|| path.frame_ms())
                .flatten()
                .filter(|&f| f <= room && h <= cap);
            let (b, ms) = if path.sum.listed() {
                // Sampling adaptively once a check has listed the pixels
                // still sampling (V2): a pass is the list, in chunks priced
                // an entry-sample at a time -- measured, or twice the
                // model's pixel, those still sampling being the costly
                // ones (a crevice's long marches) -- and the whole list at
                // several samples where it fits. The pass goes to the
                // list's length once the check's count is back, and until
                // then to the frame's pixels, the threads past the list's
                // end returning at once; it is priced by the latest count
                // back, the list's length changing by a few percent a
                // check, and capped at twice that.
                let known = path.sum.active();
                let bound = known.unwrap_or(w * h).max(1);
                // The list's length back, and short of the entries already
                // traced: they were all of it, and the pass is done.
                if path.entry >= bound {
                    path.entry = 0;
                    path.sum.add(1);
                    let _ = path.sum.check_if_due(device, queue, w, h, &settings);
                    continue;
                }
                let est = known.or(path.sum.active_estimate()).unwrap_or(w * h).clamp(1, bound);
                let guard = est.saturating_mul(2).min(bound);
                let per = if path.entry_ms > 0.0 { path.entry_ms } else { 2.0 * model_ms / w as f32 };
                let real = est.saturating_sub(path.entry);
                let n = if path.entry == 0 {
                    let by_time = room / (per * est as f32);
                    let by_cap = cap_entries as f32 / guard as f32;
                    (by_time.min(by_cap).floor() as u32).clamp(1, 64).min(target - count).min(next)
                } else {
                    1
                };
                let entries = if n > 1 {
                    bound
                } else {
                    let by_time = ((room / per) as u64 / 64 * 64).max(64);
                    let take = (real as u64).min(by_time).min((cap_entries / 2 / 64 * 64).max(64)) as u32;
                    // The chunk that reaches the list's end goes on to the
                    // bound: what is past it costs nothing.
                    if take >= real { bound - path.entry } else { take }
                };
                let taken = entries.min(real);
                (Band { y0: path.entry, rows: entries, x0: 0, cols: 0, n, list: Some((taken, bound)) }, taken as f32 * n as f32 * per)
            } else {
                match whole {
                    Some(f) => {
                        // Sampling adaptively, no further than the next check.
                        let n = ((room / f).floor() as u32)
                            .min(cap / h)
                            .clamp(1, 64)
                            .min(target - count)
                            .min(next);
                        (Band { y0: 0, rows: h, x0: 0, cols: w, n, list: None }, f * n as f32)
                    }
                    None => {
                        // A band not yet begun across is planned again when
                        // the cap has fallen below it since (the path shift
                        // halves it after a slow call or a lost device); one
                        // part-way across finishes its rows, in tiles each
                        // a share of the width.
                        if path.band_rows == 0 || (path.col == 0 && path.band_rows > cap) {
                            path.band_rows = path.plan(path.row, frame_ms - spent, model_ms, cap).0;
                            path.col = 0;
                        }
                        let rows = path.band_rows;
                        // At today's prices: a tile measured since the band
                        // began has priced its rows.
                        let band_ms: f32 = (path.row..path.row + rows).map(|y| path.price(y, model_ms)).sum();
                        let cols = if path.col == 0 && band_ms <= ceiling {
                            w
                        } else {
                            // Across in tiles of whole workgroups, each its
                            // share of the band's price, padded for a band
                            // whose cost is not spread evenly.
                            let fit = (w as f32 * ceiling / (band_ms * TILE_SAFETY)) as u32 / 8 * 8;
                            fit.max(8).min(w - path.col)
                        };
                        (Band { y0: path.row, rows, x0: path.col, cols, n: 1, list: None }, band_ms * cols as f32 / w as f32)
                    }
                }
            };
            let mut uniform = PathParamsGpu::new(&settings, count, b.n, per_ray, param("shadow", 0.7), radius, (b.y0, b.rows)).from_column(b.x0);
            if b.list.is_some() {
                uniform = uniform.from_list(b.y0, b.rows);
            }
            let uniform = uniform.buffer(device, queue);
            let group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Escape Solid Path"),
                layout: &path.layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: path.params.as_entire_binding() },
                    BindGroupEntry { binding: 2, resource: BindingResource::TextureView(palette_view) },
                    BindGroupEntry { binding: 3, resource: BindingResource::Sampler(&self.palette_sampler) },
                    BindGroupEntry { binding: 8, resource: path.sum.buffer().as_entire_binding() },
                    BindGroupEntry { binding: 9, resource: uniform.as_entire_binding() },
                    BindGroupEntry { binding: 10, resource: path.sum.guide_buffer().as_entire_binding() },
                    BindGroupEntry { binding: 14, resource: BindingResource::TextureView(&env_view) },
                    BindGroupEntry { binding: 15, resource: BindingResource::Sampler(&env_sampler) },
                ],
            });
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Escape Solid Path") });
            {
                let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Escape Solid Path"), timestamp_writes: None });
                pass.set_pipeline(&self.pipelines[&key]);
                pass.set_bind_group(0, &group, &[]);
                pass.set_bind_group(1, &ifs_group, &[]);
                if let Some(l) = self.lens_gpu.as_ref() {
                    pass.set_bind_group(2, l.bind_group(), &[]);
                }
                if b.list.is_some() {
                    let (gx, gy) = path_core::list_groups(b.rows);
                    pass.dispatch_workgroups(gx, gy, 1);
                } else {
                    pass.dispatch_workgroups(b.cols.div_ceil(8), b.rows.div_ceil(8), 1);
                }
            }
            first_submit.get_or_insert_with(web_time::Instant::now);
            queue.submit(std::iter::once(enc.finish()));
            traced.push(b);
            #[cfg(test)]
            path.dispatched.push(match b.list {
                Some(_) => (b.rows * b.n, cap_entries as u32, 0),
                None => (b.rows * b.n, cap, b.cols),
            });
            spent += ms;
            if let Some((_, bound)) = b.list {
                path.entry += b.rows;
                if path.entry >= bound {
                    path.entry = 0;
                    path.sum.add(b.n);
                }
            } else if whole.is_some() {
                path.sum.add(b.n);
            } else {
                path.col += b.cols;
                if path.col >= w {
                    path.col = 0;
                    path.row += path.band_rows;
                    path.band_rows = 0;
                    if path.row >= h {
                        path.row = 0;
                        path.sum.add(1);
                    }
                }
            }
            // A pass complete at a check's count: which pixels go on. (Or a
            // new threshold's check, part-way through a pass over the list:
            // the pass starts again on the new list.)
            if path.sum.check_if_due(device, queue, w, h, &settings) {
                path.entry = 0;
            }
        }
        if let Some(submitted) = first_submit {
            let (clock, generation) = (Arc::clone(&path.clock), path.generation);
            if let Ok(mut c) = clock.lock() {
                c.outstanding += 1;
            }
            // A device lost from here to the call's completion is this
            // call's (`note_device_lost`).
            super::PATH_CALLS_IN_FLIGHT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            queue.on_submitted_work_done(move || {
                use std::sync::atomic::Ordering;
                let _ = super::PATH_CALLS_IN_FLIGHT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.saturating_sub(1)));
                if let Ok(mut c) = clock.lock() {
                    let now = web_time::Instant::now();
                    let from = c.last.map_or(submitted, |l| l.max(submitted));
                    c.done.push((generation, traced, now.duration_since(from).as_secs_f32() * 1000.0));
                    c.last = Some(now);
                    c.outstanding = c.outstanding.saturating_sub(1);
                }
            });
        }
        // The last picture current, whatever the denoiser's schedule.
        let split = path.split();
        if path.sum.done(target) {
            path.finished = true;
            path.sum.resolve_now(device, queue, &path.output.1, w, h, split);
        } else {
            path.sum.resolve(device, queue, &path.output.1, w, h, split);
        }
        !path.sum.done(target)
    }

    /// Rows of a `w`-wide frame the walk's model (`direct_rows_per_dispatch`)
    /// gives a band of one sample, with a sample's marches counted -- at
    /// each of the camera's surface and `bounces` more, a shadow march a
    /// light, the next march, and the normal's six distances and the
    /// colouring's walk -- under the walk's budget, unshifted: about
    /// 300 ms of rows, where every pixel is the solid's. (Measured on the
    /// sponge at 1080p: 52 rows, its rows through the solid 6 to 11 ms.)
    fn solid_path_rows(&self, escape: &crate::config::escape::EscapeConfig, def: &crate::escape::ifs::IfsDef, bounces: u32, w: u32) -> u32 {
        let param = |name: &str, fallback: f32| {
            escape.formula_params.get(name).copied().unwrap_or_else(|| {
                def.parameters.iter().find(|p| p.name == name).map_or(fallback, |p| p.default)
            })
        };
        let steps = param("steps", 96.0).max(4.0) as u64;
        let (sh, _, _, _) = &self.solid_lighting;
        let lights = if sh.rig_untouched() {
            1
        } else {
            sh.lights.iter().filter(|l| l.enabled && l.intensity > 0.0).count() as u64
        };
        let per_sample = (1 + bounces as u64) * (steps * (1 + lights) + 7);
        let maps = self.ifs.as_ref().and_then(|p| p.solid.as_ref()).map_or(1, |(_, r)| r.len()) as u64;
        // `ifs_rows_per_dispatch`'s arithmetic, unclamped by a height: the
        // caller wants to know how many FRAMES fit as well.
        let per_pixel = (param("levels", 24.0).max(1.0) as u64)
            .saturating_mul(param("beam", 8.0).max(1.0) as u64)
            .saturating_mul(per_sample)
            .saturating_mul(maps.max(1));
        let per_row = (w.max(1) as u64).saturating_mul(per_pixel);
        let rows = super::IFS_SOLID_BUDGET / per_row.max(1);
        rows.clamp(1, u32::MAX as u64) as u32
    }

    /// The path pipeline for this formula, colouring, beam and lens, as
    /// `ensure_pipeline` keys the walk's.
    fn ensure_solid_path_pipeline(
        &mut self,
        device: &Device,
        escape: &crate::config::escape::EscapeConfig,
        def: &crate::escape::ifs::IfsDef,
    ) -> String {
        let coloring = crate::escape::ifs::get_ifs_coloring(&escape.coloring, def);
        let beam = escape
            .formula_params
            .get("beam")
            .copied()
            .unwrap_or_else(|| def.parameters.iter().find(|p| p.name == "beam").map_or(1.0, |p| p.default))
            .clamp(1.0, assembler::IFS_MAX_BEAM as f32) as u32;
        let registry = crate::variations::global_registry();
        let lens_src = crate::escape::lens::lens_source(escape, &registry);
        let lens_id = crate::escape::lens::lens_key(escape, &registry);
        let sums = self.ifs.as_ref().is_some_and(|p| p.rows.iter().any(|r| r.kind == 7.0));
        // Points and headlamps compiled in, or not (`assembler::ifs_rig`).
        let placed = self.solid_lighting.0.placed_lights();
        let key = format!("ifs_path|{}|{}|b{beam}|{lens_id}|{sums}|{placed}", def.name, coloring.name);
        if !self.pipelines.contains_key(&key) {
            let source = assembler::assemble_ifs_path(def, coloring, beam, lens_src.as_deref(), sums, placed);
            #[cfg(test)]
            let source = match self.path_source_patch {
                Some(patch) => patch(source),
                None => source,
            };
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&format!("Escape Shader {key}")),
                source: ShaderSource::Wgsl(source.into()),
            });
            let path = self.solid_path.as_ref().expect("made before the pipeline");
            let mut groups = vec![Some(&path.layout), Some(&self.ifs_bind_group_layout)];
            if lens_src.is_some() {
                groups.push(Some(&self.lens_bind_group_layout));
            }
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Solid Path Pipeline Layout"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(&format!("Escape Pipeline {key}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline);
        }
        key
    }

    /// Mode D's group 1: the maps, the chain, the geometry cache, the
    /// coarse pass, the transition graph and the reference.
    pub(super) fn ifs_bind_group(&self, device: &Device) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Solid Path IFS"),
            layout: &self.ifs_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: self.ifs_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.ifs_chain_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.ifs_geom_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: self.ifs_coarse_buffer.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: self.ifs_xaos_buffer.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: self.ifs_ref_buffer.as_entire_binding() },
            ],
        })
    }

    /// The path tracer's sum, for a test to read back.
    #[cfg(test)]
    pub(crate) fn solid_path_sum_for_test(&self) -> Option<&Buffer> {
        self.solid_path.as_ref().map(|p| p.sum.buffer())
    }

    /// The path tracer's output texture, for a test to read back.
    #[cfg(test)]
    pub(crate) fn solid_path_texture_for_test(&self) -> Option<&Texture> {
        self.solid_path.as_ref().map(|p| &p.output.0)
    }

    /// Test-only: the solid path tracer's sum and guide buffers.
    #[cfg(test)]
    pub(crate) fn solid_path_buffers_for_test(&self) -> Option<(&Buffer, &Buffer)> {
        self.solid_path.as_ref().map(|p| (p.sum.sum_buffer(), p.sum.guide_buffer()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::escape::RenderTier;
    use crate::scene::transforms::{Flame, RenderMode, Transform};
    use egui_wgpu::wgpu::{
        Backends, Extent3d, Features, Instance, InstanceDescriptor, MapMode, MemoryHints, PowerPreference,
        RequestAdapterOptions, TexelCopyBufferInfo, TexelCopyBufferLayout, TextureDescriptor, TextureDimension,
        TextureFormat, TextureUsages, TextureViewDescriptor,
    };
    use std::collections::HashMap;

    fn device() -> Option<(Device, Queue)> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&egui_wgpu::wgpu::DeviceDescriptor {
            label: Some("solid path tests"),
            required_features: adapter.features() & Features::CLEAR_TEXTURE,
            required_limits: adapter.limits(),
            memory_hints: MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .ok()?;
        device.on_uncaptured_error(std::sync::Arc::new(|e| panic!("wgpu error in solid path tests: {e}")));
        Some((device, queue))
    }

    fn wait(device: &Device) {
        let _ = device.poll(egui_wgpu::wgpu::PollType::Wait { submission_index: None, timeout: None });
    }

    fn read_buffer(device: &Device, queue: &Queue, src: &Buffer, size: u64) -> Vec<u8> {
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        queue.submit(std::iter::once(enc.finish()));
        staging.slice(..).map_async(MapMode::Read, |_| {});
        wait(device);
        let out = staging.slice(..).get_mapped_range().to_vec();
        staging.unmap();
        out
    }

    /// An Rgba32Float texture's texels.
    fn read_texture(device: &Device, queue: &Queue, tex: &Texture, w: u32, h: u32) -> Vec<[f32; 4]> {
        let row = (w * 16).div_ceil(256) * 256;
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &staging,
                layout: TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        staging.slice(..).map_async(MapMode::Read, |_| {});
        wait(device);
        let bytes = staging.slice(..).get_mapped_range().to_vec();
        staging.unmap();
        let mut out = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                let o = (y * row + x * 16) as usize;
                let f: &[f32] = bytemuck::cast_slice(&bytes[o..o + 16]);
                out.push([f[0], f[1], f[2], f[3]]);
            }
        }
        out
    }

    /// A white palette, so a surface's albedo is its colouring's
    /// brightness alone.
    fn white_palette(device: &Device, queue: &Queue) -> (Texture, TextureView) {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("white palette"),
            size: Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            tex.as_image_copy(),
            &[255u8; 256 * 4],
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256 * 4), rows_per_image: Some(1) },
            Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    /// The unit cube as an IFS: eight half-scale maps to its corners. A
    /// solid with no concavity, so a ray leaving its surface never meets
    /// it again.
    /// The Menger sponge: the 20 cells of the 3x3x3 grid not centred on
    /// two axes, each a third the size.
    fn sponge() -> Flame {
        let mut fl = cube();
        fl.transforms.clear();
        for (i, j, k) in (0..27).map(|n| (n % 3, n / 3 % 3, n / 9)) {
            if (i == 1) as u32 + (j == 1) as u32 + (k == 1) as u32 >= 2 {
                continue;
            }
            let mut t = Transform::default();
            t.a = 1.0;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 1.0;
            t.e = i as f32;
            t.f = j as f32;
            t.g = k as f32;
            t.color = fl.transforms.len() as f32 / 20.0;
            t.variations = HashMap::from([("linear3D".to_string(), 1.0 / 3.0)]);
            t.variation_order = vec!["linear3D".to_string()];
            fl.transforms.push(t);
        }
        fl
    }

    fn cube() -> Flame {
        let mut fl = Flame::default();
        fl.transforms.clear();
        for k in 0..8u32 {
            let mut t = Transform::default();
            t.a = 1.0;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 1.0;
            t.e = (k & 1) as f32;
            t.f = ((k >> 1) & 1) as f32;
            t.g = ((k >> 2) & 1) as f32;
            t.color = k as f32 / 8.0;
            t.variations = HashMap::from([("linear3D".to_string(), 0.5)]);
            t.variation_order = vec!["linear3D".to_string()];
            fl.transforms.push(t);
        }
        fl.final_transforms.clear();
        fl.xaos = None;
        fl
    }

    /// The cube in mode D, coloured flat (address colouring, no halo: a
    /// brightness of one), lit by nothing until a test says otherwise.
    fn cube_config() -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.flame = cube();
        c.escape.formula = "ifs_flame_3d".to_string();
        c.escape.coloring = "ifs_address".to_string();
        c.escape.coloring_params.insert("reach".to_string(), 0.0);
        c.escape.zoom_log2 = 0.0;
        c.escape.cam_yaw = 0.9;
        c.escape.cam_pitch = 0.42;
        c.escape.solid_tier = RenderTier::PathTraced;
        c.gamma = 1.0;
        c.exposure = 1.0;
        c.background_color = [0.0; 3];
        c.solid_shading.shading_strength = 1.0;
        c.solid_shading.ambient = 0.0;
        c.solid_shading.specular = 0.0;
        c.solid_shading.ssao_strength = 0.0;
        for l in c.solid_shading.lights.iter_mut() {
            l.enabled = false;
        }
        c.solid_shading.gloss = 0.0;
        c
    }

    fn renderer_for(device: &Device, c: &FractalConfig, w: u32, h: u32) -> EscapeRenderer {
        let mut r = EscapeRenderer::new(device, w, h);
        r.resize(device, w, h, 1);
        let def = crate::escape::ifs::get_ifs(&c.escape.formula).expect("solid");
        let registry = crate::variations::global_registry();
        let packed = crate::escape::ifs::pack_for(def, c, &registry);
        assert!(packed.is_some(), "the cube does not qualify");
        r.set_ifs(packed);
        r.set_solid_lighting(&c.solid_shading, (c.fog_strength, c.fog_start, c.background_color));
        r.set_solid_sky(crate::escape::path_core::sky_seen(&c.escape.path, c));
        r.set_solid_material(crate::escape::path_core::lit_material(c));
        r.set_solid_studio(crate::escape::path_core::studio_level(&c.escape.path, c));
        r
    }

    /// The denoiser (T5) on a solid: a few samples of a lit, shadowed
    /// sponge under the sky -- its holes occlude the sky, which is the
    /// noise; a cube is convex and has next to none -- denoised, are
    /// nearer 256 than they are alone, inside the silhouette, and a
    /// picture of more samples is no worse for it. Less nearer than a
    /// terrain's: much of a solid's error at a few samples is its
    /// sub-pixel geometry's antialiasing, which filtering the light
    /// cannot touch (measured here: 16% less at 4 samples, 5% at 16).
    #[test]
    fn a_denoised_solid_is_nearer_converged() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (128u32, 96u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.flame = sponge();
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.lights[0].azimuth = 30.0;
        c.solid_shading.lights[0].elevation = 35.0;
        // The scene the bars were measured on: the light at those angles
        // from the camera (the fractal's frame lights other faces).
        c.solid_shading.lights_follow_camera = true;
        c.background_color = [0.5, 0.6, 0.8];
        c.escape.path.bounces = 2;
        let mut r = renderer_for(&device, &c, w, h);
        let mut picture = |samples: u32, denoise: bool| {
            c.escape.path.denoise = denoise;
            r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
            read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h)
        };
        let reference = picture(256, false);
        let inside: Vec<bool> = (0..(w * h) as i32)
            .map(|k| {
                let (x, y) = (k % w as i32, k / w as i32);
                (-1..=1).all(|dy| {
                    (-1..=1).all(|dx| {
                        let (qx, qy) = (x + dx, y + dy);
                        qx >= 0 && qy >= 0 && qx < w as i32 && qy < h as i32 && reference[(qy * w as i32 + qx) as usize][3] >= 0.999
                    })
                })
            })
            .collect();
        assert!(inside.iter().filter(|&&b| b).count() > (w * h / 8) as usize, "the solid fills the frame");
        let rmse = |a: &[[f32; 4]]| {
            let (mut e, mut k) = (0.0f64, 0usize);
            for ((p, q), &keep) in a.iter().zip(&reference).zip(&inside) {
                if keep {
                    e += (0..3).map(|c| ((p[c] - q[c]) as f64).powi(2)).sum::<f64>();
                    k += 3;
                }
            }
            (e / k.max(1) as f64).sqrt()
        };
        for (samples, bar) in [(4u32, 0.9), (16, 1.0)] {
            let (raw, dn) = (rmse(&picture(samples, false)), rmse(&picture(samples, true)));
            println!("{samples} samples: raw {raw:.4}, denoised {dn:.4}");
            assert!(dn < bar * raw, "{samples} samples: denoised {dn} against raw {raw}");
        }
        // Where the filter narrows: at a constant width this was 24% worse.
        let (raw, dn) = (rmse(&picture(64, false)), rmse(&picture(64, true)));
        println!("64 samples: raw {raw:.4}, denoised {dn:.4}");
        assert!(dn <= raw * 1.05, "more samples, and the filter made it worse");
    }

    /// A gradient sky on a solid (T5): where the walk and the path
    /// tracer find nothing, both draw the zenith's colour (as seen) at
    /// the profile's share of the ray, and agree on it -- the lit tier at
    /// the pixel's centre, the path tracer over its jitter, denoised or
    /// not (a sky sample once reported no surface and the denoiser
    /// multiplied its light by a zero albedo: a black sky). Off, nothing.
    #[test]
    fn a_solid_draws_the_gradient_sky() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        for (gradient, denoise) in [(false, false), (true, false), (true, true)] {
            let mut c = cube_config();
            // Looking up at it, so the sky above the horizon is in the
            // frame.
            c.escape.cam_pitch = -0.3;
            c.escape.path.denoise = denoise;
            c.escape.path.sky_gradient = gradient;
            c.escape.path.zenith = [0.2, 0.3, 0.9];
            c.escape.solid_tier = RenderTier::Lit;
            let seen = crate::escape::path_core::sky_seen(&c.escape.path, &c);
            let mut r = renderer_for(&device, &c, w, h);
            loop {
                let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
                let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
                queue.submit(std::iter::once(enc.finish()));
                if settled {
                    break;
                }
            }
            let lit = read_texture(&device, &queue, &r.output_texture, w, h);
            r.render_solid_still(&device, &queue, &c, &palette, 16, || { wait(&device); true });
            let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            // The sky's pixels: no surface within a pixel of them, lit -- a
            // jittered sample can reach a solid the centre's ray misses.
            let clear = |x: u32, y: u32| {
                (0..9u32).all(|i| {
                    let (qx, qy) = (x as i32 + (i % 3) as i32 - 1, y as i32 + (i / 3) as i32 - 1);
                    qx < 0 || qy < 0 || qx >= w as i32 || qy >= h as i32 || lit[(qy as u32 * w + qx as u32) as usize][3] < 1.0
                })
            };
            let mut skies = 0;
            for (k, (a, b)) in lit.iter().zip(&path).enumerate() {
                if !clear(k as u32 % w, k as u32 / w) {
                    continue;
                }
                skies += 1;
                match seen {
                    None => assert_eq!((a[3], b[3]), (0.0, 0.0), "pixel {k}: a flat sky draws nothing"),
                    Some(z) => {
                        for ch in 0..3 {
                            if a[3] > 0.0 && b[3] > 0.0 {
                                assert!((a[ch] - z[ch]).abs() < 1.0e-4 && (b[ch] - z[ch]).abs() < 1.0e-4, "pixel {k}: {a:?} {b:?} against {z:?}");
                            }
                        }
                        assert!((a[3] - b[3]).abs() < 0.02, "pixel {k}: coverage {} lit, {} path traced", a[3], b[3]);
                    }
                }
            }
            println!("gradient {gradient}, denoised {denoise}: {skies} pixels of sky");
            assert!(skies > (w * h / 4) as usize, "the sky is in the frame: {skies}");
            if gradient {
                let drawn = lit.iter().filter(|p| p[3] > 0.0 && p[3] < 1.0).count();
                assert!(drawn > (w * h / 8) as usize, "the gradient shows: {drawn}");
            }
        }
    }

    /// The white furnace on a solid: albedo 1, a uniform sky L and no
    /// light. The cube is convex, so every bounce escapes to the sky and
    /// every surface reads L -- whatever the bounce count, exactly but
    /// for Russian roulette's noise past the first.
    #[test]
    fn the_white_furnace_on_a_solid() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let bg = [0.5f32, 0.25, 0.75];
        // The sky's radiance: the background decoded from sRGB, as the
        // tonemap shows it (`path_core::shown`; gamma and exposure 1).
        let l = bg.map(|v| v.powf(2.2));
        for bounces in [1u32, 3] {
            let mut c = cube_config();
            c.background_color = bg;
            c.escape.path.bounces = bounces;
            let mut r = renderer_for(&device, &c, w, h);
            r.render_solid_still(&device, &queue, &c, &palette, 64, || { wait(&device); true });
            assert_eq!(r.solid_path_samples(), 64);
            let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            let hit: Vec<&[f32; 4]> = out.iter().filter(|p| p[3] > 0.0).collect();
            assert!(hit.len() > (w * h / 8) as usize, "the cube covers {} pixels", hit.len());
            for k in 0..3 {
                let mean = hit.iter().map(|p| p[k] as f64).sum::<f64>() / hit.len() as f64;
                let worst = hit.iter().map(|p| (p[k] - l[k]).abs()).fold(0.0f32, f32::max);
                println!("furnace, {bounces} bounces, channel {k}: mean {mean:.5} against {}, worst {worst:.4}", l[k]);
                assert!((mean - l[k] as f64).abs() < 0.01 * l[k] as f64, "{bounces} bounces, channel {k}: {mean}");
            }
            r.destroy();
        }
    }

    /// Sun only, through a pinhole, with no bounce: the path tracer's
    /// face is the lit tier's `albedo I diffuse cos`, its relight of the
    /// walk -- the two tiers light a solid alike.
    #[test]
    fn a_sunlit_solid_is_the_lit_tiers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.lights[0].azimuth = 30.0;
        c.solid_shading.lights[0].elevation = 35.0;
        c.solid_shading.lights[0].intensity = 1.5;
        c.solid_shading.lights[0].color = [1.0, 0.5, 0.25];
        c.escape.formula_params.insert("shadow".to_string(), 0.0);
        c.escape.formula_params.insert("shadow_sharpness".to_string(), 1.0e4);
        c.escape.path.bounces = 0;
        c.escape.solid_tier = RenderTier::Lit;
        let mut r = renderer_for(&device, &c, w, h);
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
            queue.submit(std::iter::once(enc.finish()));
            if settled {
                break;
            }
        }
        let lit = read_texture(&device, &queue, &r.output_texture, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 4, || { wait(&device); true });
        let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        // A face's inside: lit there and alike to all eight neighbours --
        // a jittered sample anywhere in the pixel is on the same face, and
        // clear of the normal's blur across an edge (its step is a
        // pixel) -- with all the path's samples on it.
        let at = |x: u32, y: u32| lit[(y * w + x) as usize];
        let mut n = 0;
        let mut worst = 0.0f32;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let (p, q) = (at(x, y), path[(y * w + x) as usize]);
                let flat = (0..9u32).all(|i| {
                    let o = at(x + i % 3 - 1, y + i / 3 - 1);
                    (0..4).all(|k| (p[k] - o[k]).abs() < 1e-4)
                });
                if p[3] < 1.0 || q[3] < 1.0 || !flat || p[0] <= 0.0 {
                    continue;
                }
                n += 1;
                for k in 0..3 {
                    worst = worst.max((p[k] - q[k]).abs() / p[k].max(1e-3));
                }
            }
        }
        println!("sunlit solid: {n} face pixels, worst relative difference {worst:.2e}");
        assert!(n > 10, "only {n} face pixels");
        assert!(worst < 1e-3, "{worst}");
        r.destroy();
    }

    /// The path tracer covers what the lit tier covers. Its camera rays
    /// are jittered over the pixel where the walk's go through its
    /// centre, so on a fractal full of sub-pixel holes the two agree on
    /// AVERAGE coverage, and inside a patch the walk sees as solid all
    /// round the path tracer leaves nothing of the background showing. A
    /// ray lost to the march -- out of steps, or stopped short -- would
    /// show as the background's colour through the solid.
    #[test]
    fn the_path_tracer_covers_what_the_lit_tier_covers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (_pt, palette) = white_palette(&device, &queue);
        let (w, h) = (320u32, 180u32);
        // The Sierpinski tetrahedron, the shipped preset's view.
        let mut c = cube_config();
        c.flame.transforms.truncate(4);
        for (t, v) in c.flame.transforms.iter_mut().zip([[0.0f32, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]]) {
            t.e = v[0];
            t.f = v[1];
            t.g = v[2];
        }
        c.escape.cam_yaw = 2.6;
        c.escape.cam_pitch = 0.25;
        c.solid_shading.lights[0].enabled = true;
        c.escape.solid_tier = RenderTier::Lit;
        let mut r = renderer_for(&device, &c, w, h);
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
            queue.submit(std::iter::once(enc.finish()));
            if settled {
                break;
            }
        }
        let lit = read_texture(&device, &queue, &r.output_texture, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 16, || { wait(&device); true });
        let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        let mean = |img: &[[f32; 4]]| img.iter().map(|p| p[3] as f64).sum::<f64>() / img.len() as f64;
        let at = |x: u32, y: u32| lit[(y * w + x) as usize][3];
        let (mut n, mut cov) = (0usize, 0.0f64);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                if (0..9u32).all(|i| at(x + i % 3 - 1, y + i / 3 - 1) >= 1.0) {
                    n += 1;
                    cov += path[(y * w + x) as usize][3] as f64;
                }
            }
        }
        let (ml, mp, inside) = (mean(&lit), mean(&path), cov / n.max(1) as f64);
        println!("coverage: lit {ml:.4}, path {mp:.4}; inside {n} solid patch pixels the path covers {inside:.4}");
        assert!(n > 500, "only {n} patch pixels");
        assert!((mp - ml).abs() < 0.02 * ml, "mean coverage {mp} against {ml}");
        assert!(inside > 0.995, "the background shows through a solid patch: {inside}");
        r.destroy();
    }

    /// The viewport's pace, measured: a solid driven a frame at a time as
    /// the app drives it (a step, then the GPU's completion), under a path
    /// shift as a machine's tuning file can leave it (PACE_SHIFT). The
    /// tetrahedron, or the shipped Menger sponge (PACE_MENGER=1); 1080p,
    /// or PACE_W wide. Prints frames and seconds to each of the first
    /// samples, and the longest frame.
    #[test]
    #[ignore = "measurement; needs a GPU"]
    fn the_viewport_pace() {
        use std::sync::atomic::Ordering;
        let (device, queue) = device().expect("gpu");
        let (_pt, palette) = white_palette(&device, &queue);
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u32>().ok());
        let shift = env("PACE_SHIFT").unwrap_or(0);
        let old = super::super::PATH_BUDGET_SHIFT.swap(shift, Ordering::Relaxed);
        let w = env("PACE_W").unwrap_or(1920);
        let h = w * 9 / 16;
        let c = if env("PACE_MENGER") == Some(1) {
            let mut c = crate::resources::load_presets_with_fallback()
                .into_iter()
                .find(|c| c.flame.name == "Menger Sponge")
                .expect("the preset");
            c.escape.solid_tier = RenderTier::PathTraced;
            c
        } else {
            let mut c = cube_config();
            c.flame.transforms.truncate(4);
            for (t, v) in c.flame.transforms.iter_mut().zip([[0.0f32, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]]) {
                t.e = v[0];
                t.f = v[1];
                t.g = v[2];
            }
            c.escape.cam_yaw = 2.6;
            c.escape.cam_pitch = 0.25;
            c.solid_shading.lights[0].enabled = true;
            c.solid_shading.lights[1].enabled = true;
            c.background_color = [0.55, 0.62, 0.72];
            c.escape.path.samples = 64;
            c
        };
        let mut r = renderer_for(&device, &c, w, h);
        let t0 = std::time::Instant::now();
        let mut frames = 0u32;
        let mut seen = 0u32;
        let mut longest = 0.0f32;
        while t0.elapsed().as_secs_f32() < 60.0 && r.solid_path_samples() < 2 {
            let t = std::time::Instant::now();
            r.trace_solid_viewport(&device, &queue, &c, &palette, VIEWPORT_FRAME_MS);
            wait(&device);
            longest = longest.max(t.elapsed().as_secs_f32() * 1000.0);
            frames += 1;
            if r.solid_path_samples() > seen {
                seen = r.solid_path_samples();
                println!("{w}x{h} shift {shift}: sample {seen} after {frames} frames, {:.2} s", t0.elapsed().as_secs_f32());
            }
        }
        let d = &r.solid_path.as_ref().expect("traced").dispatched;
        println!(
            "{w}x{h} shift {shift}: {} samples in {frames} frames, {:.2} s; the longest frame {longest:.0} ms; \
             {} dispatches, {} of them tiles",
            r.solid_path_samples(),
            t0.elapsed().as_secs_f32(),
            d.len(),
            d.iter().filter(|&&(_, _, cols)| cols < w).count(),
        );
        super::super::PATH_BUDGET_SHIFT.store(old, Ordering::Relaxed);
        r.destroy();
    }

    /// The shipped Menger sponge's path tracer, measured: the lit walk of
    /// the view for scale; the first sample a band of the fewest rows a
    /// call, each timed to the GPU's completion; a sample at each of the
    /// viewport's, 100 ms and the export's call; and, at 640 wide or less,
    /// whole frames. MENGER_W wide (320 by default: no band nears the
    /// watchdog), MENGER_BOUNCES, MENGER_SHADOW (the formula's shadow
    /// strength; 0 sends no shadow rays) and MENGER_LIGHTS (how many of
    /// the rig's lights are on).
    #[test]
    #[ignore = "measurement; needs a GPU"]
    fn menger_band_costs() {
        let (device, queue) = device().expect("gpu");
        let (_pt, palette) = white_palette(&device, &queue);
        let w: u32 = std::env::var("MENGER_W").ok().and_then(|v| v.parse().ok()).unwrap_or(320);
        let h = w * 9 / 16;
        let mut c = crate::resources::load_presets_with_fallback()
            .into_iter()
            .find(|c| c.flame.name == "Menger Sponge")
            .expect("the preset");
        c.escape.solid_tier = RenderTier::PathTraced;
        if let Some(b) = std::env::var("MENGER_BOUNCES").ok().and_then(|v| v.parse().ok()) {
            c.escape.path.bounces = b;
        }
        if let Some(v) = std::env::var("MENGER_SHADOW").ok().and_then(|v| v.parse().ok()) {
            c.escape.formula_params.insert("shadow".to_string(), v);
        }
        if let Some(v) = std::env::var("MENGER_LIGHTS").ok().and_then(|v| v.parse::<usize>().ok()) {
            for (i, l) in c.solid_shading.lights.iter_mut().enumerate() {
                l.enabled = i < v;
            }
        }
        let mut r = renderer_for(&device, &c, w, h);
        // The lit walk of the same view, for scale.
        let mut lit = c.clone();
        lit.escape.solid_tier = RenderTier::Lit;
        let t = std::time::Instant::now();
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            let settled = r.render(&device, &queue, &mut enc, &lit.escape, &palette, 1);
            queue.submit(std::iter::once(enc.finish()));
            wait(&device);
            if settled {
                break;
            }
        }
        println!("the lit walk: {:.0} ms", t.elapsed().as_secs_f32() * 1000.0);
        let t0 = std::time::Instant::now();
        let mut rows = Vec::new();
        while r.solid_path_samples() < 1 && t0.elapsed().as_secs_f32() < 120.0 {
            let y0 = r.solid_path.as_ref().map_or(0, |p| p.row);
            let t = std::time::Instant::now();
            r.trace_solid(&device, &queue, &c, &palette, 1, 1.0e-9);
            wait(&device);
            let ms = t.elapsed().as_secs_f32() * 1000.0;
            let band = r.solid_path.as_ref().map_or(0, |p| if p.row == 0 { h - y0 } else { p.row - y0 });
            rows.push((y0, band, ms));
        }
        // A second sample at each target, every row measured: what bigger
        // bands buy.
        for (k, frame_ms) in [(2u32, VIEWPORT_FRAME_MS), (3, 100.0), (4, EXPORT_FRAME_MS)] {
            let t = std::time::Instant::now();
            let mut calls = 0;
            let mut longest = 0.0f32;
            while r.solid_path_samples() < k {
                let t1 = std::time::Instant::now();
                r.trace_solid(&device, &queue, &c, &palette, k, frame_ms);
                wait(&device);
                longest = longest.max(t1.elapsed().as_secs_f32() * 1000.0);
                calls += 1;
            }
            println!("sample {k} at {frame_ms} ms a call: {:.0} ms in {calls} calls, the longest {longest:.0} ms", t.elapsed().as_secs_f32() * 1000.0);
        }
        // Whole frames, once every row is measured: the shader's own cost.
        // Only where a whole frame is far inside the watchdog.
        for k in (5..=6u32).filter(|_| w <= 640) {
            let t = std::time::Instant::now();
            r.trace_solid(&device, &queue, &c, &palette, k, 1.0e9);
            wait(&device);
            println!("sample {k} as one call: {:.0} ms ({} dispatches so far)", t.elapsed().as_secs_f32() * 1000.0,
                r.solid_path.as_ref().unwrap().dispatched.len());
        }
        let total: f32 = rows.iter().map(|r| r.2).sum();
        let worst = rows.iter().map(|r| r.2 / r.1 as f32).fold(0.0f32, f32::max);
        for (y0, band, ms) in &rows {
            println!("rows {y0:>4}+{band:<3} {ms:8.1} ms  ({:.2} ms a row)", ms / *band as f32);
        }
        let cap = r.solid_path_rows(&c.escape, crate::escape::ifs::get_ifs(&c.escape.formula).unwrap(), c.escape.path.bounces, w);
        println!(
            "{w}x{h}, bounces {}: the first sample {total:.0} ms in bands of the fewest rows, the costliest row {worst:.2} ms; \
             the model's cap {cap} rows, at that row {:.0} ms",
            c.escape.path.bounces,
            cap as f32 * worst,
        );
        r.destroy();
    }

    /// The lit tier's occlusion darkens the ambient alone, not the direct
    /// light (3d-look-and-tools P2): the path tracer has no counterpart
    /// for an occluded direct light -- its shadow ray is the direct
    /// light's only occluder -- and the lit tier's darkened lit crevices
    /// the path tracer lights. The sponge, full occlusion strength, no
    /// ambient, no shadows, no bounces: the lit tier's light over the
    /// pixels both tiers cover is the path tracer's (to its jitter across
    /// a pixel). With occlusion on the direct light it was 3.5% short.
    #[test]
    fn occlusion_leaves_the_direct_light_alone() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (192u32, 128u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.flame = sponge();
        c.solid_shading.ssao_strength = 1.0;
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.lights[0].azimuth = 30.0;
        c.solid_shading.lights[0].elevation = 35.0;
        c.escape.formula_params.insert("shadow".to_string(), 0.0);
        c.escape.formula_params.insert("shadow_sharpness".to_string(), 1.0e4);
        c.escape.path.bounces = 0;
        let lit = lit_picture(&device, &queue, &c, &palette, w, h);
        c.escape.solid_tier = RenderTier::PathTraced;
        let mut r = renderer_for(&device, &c, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 16, || { wait(&device); true });
        let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        r.destroy();
        let (mut n, mut sum_lit, mut sum_path) = (0usize, 0.0f64, 0.0f64);
        for (p, q) in lit.iter().zip(&path) {
            if p[3] < 1.0 || q[3] < 0.999 {
                continue;
            }
            n += 1;
            sum_lit += (p[0] + p[1] + p[2]) as f64;
            sum_path += (q[0] + q[1] + q[2]) as f64;
        }
        let ratio = sum_lit / sum_path.max(1e-12);
        println!("  occluded sponge: {n} pixels covered in both, lit over path {ratio:.4}");
        assert!(n > 2000, "only {n} pixels");
        assert!((ratio - 1.0).abs() < 0.01, "the lit tier's direct light is {ratio} of the path tracer's");
    }

    /// A metal (3d-look-and-tools P4) is the same in both tiers: its
    /// direct light, the coat in the albedo's colour with nothing under
    /// it, given back what single scattering loses, is the lit tier's and
    /// the path tracer's alike -- on the sponge and on the Mandelbulb,
    /// whose curve sweeps the lobe. No ambient, no shadows, no bounces:
    /// the lit tier's light over the pixels both cover is the path
    /// tracer's to its jitter across a pixel and a light's cone.
    #[test]
    fn a_metal_is_lit_alike_in_both_tiers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (192u32, 128u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut sponge_config = cube_config();
        sponge_config.flame = sponge();
        let mut bulb = cube_config();
        bulb.escape.formula = "mandelbulb_solid".to_string();
        for (name, mut c) in [("sponge", sponge_config), ("bulb", bulb)] {
            c.solid_shading.lights[0].enabled = true;
            c.solid_shading.lights[0].azimuth = 30.0;
            c.solid_shading.lights[0].elevation = 35.0;
            c.solid_shading.metallic = 1.0;
            c.solid_shading.roughness = 0.6;
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
            c.escape.formula_params.insert("shadow_sharpness".to_string(), 1.0e4);
            c.escape.path.bounces = 0;
            let lit = lit_picture(&device, &queue, &c, &palette, w, h);
            c.escape.solid_tier = RenderTier::PathTraced;
            let mut r = renderer_for(&device, &c, w, h);
            r.render_solid_still(&device, &queue, &c, &palette, 16, || { wait(&device); true });
            let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            r.destroy();
            let (mut n, mut sum_lit, mut sum_path) = (0usize, 0.0f64, 0.0f64);
            for (p, q) in lit.iter().zip(&path) {
                if p[3] < 1.0 || q[3] < 0.999 {
                    continue;
                }
                n += 1;
                sum_lit += (p[0] + p[1] + p[2]) as f64;
                sum_path += (q[0] + q[1] + q[2]) as f64;
            }
            let ratio = sum_lit / sum_path.max(1e-12);
            println!("  metal {name}: {n} pixels covered in both, lit over path {ratio:.4}, path mean {:.4}", sum_path / n.max(1) as f64 / 3.0);
            assert!(n > 2000, "only {n} pixels");
            assert!(sum_path > 0.0, "the metal is lit");
            assert!((ratio - 1.0).abs() < 0.02, "{name}: the lit tier's metal is {ratio} of the path tracer's");
        }
    }

    /// The studio (3d-look-and-tools P4) lights both tiers alike: no
    /// lights, no ambient, the studio alone, one bounce -- the lit tier's
    /// closed forms (its lights' clipped irradiance, the coat's
    /// reflection by roughness) against what the path tracer's bounces
    /// see. The cube is convex, so nothing occludes the studio: its
    /// diffuse is the integral (2%); a flat face reflects one direction,
    /// often near a light's edge, so a metal's is loose (35%). The
    /// Mandelbulb at the default occlusion strength (0.6), which stands in
    /// for the studio its recesses hide: 15%.
    #[test]
    fn the_studio_lights_both_tiers_alike() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (192u32, 128u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut bulb = cube_config();
        bulb.escape.formula = "mandelbulb_solid".to_string();
        bulb.solid_shading.ssao_strength = 0.6;
        for (name, base) in [("cube", cube_config()), ("bulb", bulb)] {
            for (material, metallic, roughness) in [("diffuse", 0.0f32, 0.5f32), ("metal 0.3", 1.0, 0.3), ("metal 0.7", 1.0, 0.7)] {
                let mut c = base.clone();
                c.escape.path.studio = true;
                c.escape.path.bounces = 1;
                c.solid_shading.metallic = metallic;
                c.solid_shading.roughness = roughness;
                c.escape.solid_tier = RenderTier::Lit;
                let lit = lit_picture(&device, &queue, &c, &palette, w, h);
                c.escape.solid_tier = RenderTier::PathTraced;
                let mut r = renderer_for(&device, &c, w, h);
                r.render_solid_still(&device, &queue, &c, &palette, 64, || { wait(&device); true });
                let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
                r.destroy();
                let (mut n, mut sum_lit, mut sum_path) = (0usize, 0.0f64, 0.0f64);
                for (p, q) in lit.iter().zip(&path) {
                    if p[3] < 1.0 || q[3] < 0.999 {
                        continue;
                    }
                    n += 1;
                    sum_lit += (p[0] + p[1] + p[2]) as f64;
                    sum_path += (q[0] + q[1] + q[2]) as f64;
                }
                let ratio = sum_lit / sum_path.max(1e-12);
                println!("  studio {name} {material}: {n} pixels, path mean {:.4}, lit over path {ratio:.4}", sum_path / n.max(1) as f64 / 3.0);
                let tolerance = match (name, metallic > 0.0) {
                    ("cube", false) => 0.02,
                    ("cube", true) => 0.35,
                    _ => 0.15,
                };
                assert!(n > 2000 && sum_path > 0.0, "{name} {material}: {n} pixels");
                assert!((ratio - 1.0).abs() < tolerance, "{name} {material}: the lit tier's studio is {ratio} of the path tracer's");
            }
        }
    }

    /// An HDRI sky lights both tiers alike (docs/projects/hdri-environment.md,
    /// step 4): the Sunflowers sky without its sun (which both tiers light
    /// by as one of the rig's lights), no lights, no ambient, one bounce --
    /// the lit tier's irradiance harmonics and reflection levels against
    /// what the path tracer's bounces see, by the mean over the pixels
    /// both cover. The cube is convex, so nothing hides the sky from it:
    /// its diffuse and its gloss within 1% when built, a metal's flat
    /// faces each reflecting one direction within 10%. The Mandelbulb at
    /// the default occlusion strength (0.6), which stands in for the sky
    /// its recesses hide: 2-11% dark, as the studio's is. Gated at 2%, 12%
    /// and 15%. `PT_ENV_GATE=print` prints without asserting.
    #[test]
    fn an_hdri_sky_lights_both_tiers_alike() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (192u32, 128u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut bulb = cube_config();
        bulb.escape.formula = "mandelbulb_solid".to_string();
        bulb.solid_shading.ssao_strength = 0.6;
        let mut first = cube_config();
        first.escape.path.environment_map = "sunflowers".to_string();
        let set = crate::escape::path_core::environment_settings(&first.escape.path, &first).expect("bundled");
        let map = crate::escape::environment::EnvMap { sun: None, ..crate::escape::environment::load(set.def).expect("the bundled file") };
        let print_only = std::env::var("PT_ENV_GATE").is_ok_and(|v| v == "print");
        for (name, base) in [("cube", cube_config()), ("bulb", bulb)] {
            for (material, gloss, metallic, roughness) in [
                ("diffuse", 0.0f32, 0.0f32, 0.5f32),
                ("gloss 0.04/0.2", 0.04, 0.0, 0.2),
                ("metal 0.1", 0.0, 1.0, 0.1),
                ("metal 0.3", 0.0, 1.0, 0.3),
                ("metal 0.7", 0.0, 1.0, 0.7),
            ] {
                let tolerance = match (name, metallic > 0.0) {
                    ("cube", false) => 0.02f64,
                    ("cube", true) => 0.12,
                    _ => 0.15,
                };
                let mut c = base.clone();
                c.solid_shading.lights[0].enabled = false;
                c.solid_shading.ambient = 0.0;
                c.solid_shading.shading_strength = 1.0;
                c.solid_shading.gloss = gloss;
                c.solid_shading.metallic = metallic;
                c.solid_shading.roughness = roughness;
                c.escape.path.environment_map = "sunflowers".to_string();
                c.escape.path.environment_backdrop = false;
                c.escape.path.bounces = 1;
                let set = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("bundled");
                c.escape.solid_tier = RenderTier::Lit;
                let mut r = renderer_for(&device, &c, w, h);
                r.set_environment_map_for_test(&device, &queue, &map, set);
                loop {
                    let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
                    let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
                    queue.submit(std::iter::once(enc.finish()));
                    if settled {
                        break;
                    }
                }
                let lit = read_texture(&device, &queue, &r.output_texture, w, h);
                r.destroy();
                c.escape.solid_tier = RenderTier::PathTraced;
                let mut r = renderer_for(&device, &c, w, h);
                r.set_environment_map_for_test(&device, &queue, &map, set);
                r.render_solid_still(&device, &queue, &c, &palette, 64, || { wait(&device); true });
                let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
                r.destroy();
                let (mut n, mut sum_lit, mut sum_path) = (0usize, 0.0f64, 0.0f64);
                for (p, q) in lit.iter().zip(&path) {
                    if p[3] < 1.0 || q[3] < 0.999 {
                        continue;
                    }
                    n += 1;
                    sum_lit += (p[0] + p[1] + p[2]) as f64;
                    sum_path += (q[0] + q[1] + q[2]) as f64;
                }
                let ratio = sum_lit / sum_path.max(1e-12);
                println!("  sky {name} {material}: {n} pixels, path mean {:.4}, lit over path {ratio:.4}", sum_path / n.max(1) as f64 / 3.0);
                if !print_only {
                    assert!(n > 2000 && sum_path > 0.0, "{name} {material}: {n} pixels");
                    assert!((ratio - 1.0).abs() < tolerance, "{name} {material}: the lit tier's sky is {ratio} of the path tracer's");
                }
            }
        }
    }

    /// Multiple importance sampling of the coat and the lights
    /// (3d-look-and-tools P4) keeps the picture and lowers its noise: a
    /// polished metal Mandelbulb -- its curve sweeps the light's
    /// reflection, where a sponge's flat faces never mirror it -- under one
    /// light, shadows full and partial, no firefly clamp (which clips the
    /// lights' own samples of a polished peak, a thousand times the light,
    /// and so biases the estimate without MIS low: 0.068 against 0.376).
    /// With MIS at 256 samples, without at 4096, the mean is the same (it
    /// is unbiased: 3%); at 16, two seeds apart differ less with it.
    #[test]
    fn mis_keeps_the_picture_and_lowers_its_noise() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (128u32, 96u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut bulb = cube_config();
        bulb.escape.formula = "mandelbulb_solid".to_string();
        // A light broad enough (0.5 rad) that its reflection stays under the
        // firefly clamp, which would bias both estimates and differently.
        bulb.escape.formula_params.insert("shadow_sharpness".to_string(), 1.0);
        for (name, base, shadow, roughness) in [("bulb", bulb.clone(), 1.0f32, 0.12f32), ("bulb, shadows 0.7", bulb.clone(), 0.7, 0.12), ("bulb, rougher", bulb, 1.0, 0.3)] {
            let mut c = base;
            c.solid_shading.lights[0].enabled = true;
            c.solid_shading.lights[0].azimuth = 30.0;
            c.solid_shading.lights[0].elevation = 35.0;
            c.solid_shading.metallic = 1.0;
            c.solid_shading.roughness = roughness;
            c.escape.formula_params.insert("shadow".to_string(), shadow);
            c.escape.path.bounces = 1;
            let shot = |mis: bool, seed: u32, samples: u32| {
                let mut r = renderer_for(&device, &c, w, h);
                r.path_no_mis = !mis;
                r.path_no_clamp = true;
                r.path_seed_offset = seed;
                r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
                let px = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
                r.destroy();
                px
            };
            let mean = |px: &[[f32; 4]]| px.iter().map(|p| (p[0] + p[1] + p[2]) as f64).sum::<f64>() / px.len() as f64;
            let apart = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).abs() as f64).sum::<f64>()).sum::<f64>() / a.len() as f64;
            let (with, without) = (mean(&shot(true, 0, 256)), mean(&shot(false, 0, 4096)));
            let noise_with = apart(&shot(true, 1, 16), &shot(true, 2, 16));
            let noise_without = apart(&shot(false, 1, 16), &shot(false, 2, 16));
            println!("  {name}: mean with {with:.4}, without {without:.4}; two seeds apart at 16 samples with {noise_with:.4}, without {noise_without:.4}");
            assert!((with / without - 1.0).abs() < 0.03, "{name}: MIS moved the mean {with} from {without}");
            assert!(noise_with < noise_without, "{name}: MIS did not lower the noise");
        }
    }

    /// Fly mode's clearance (3d-look-and-tools P6) is the solid's distance
    /// at the eye: the quaternion Julia set at c = 0 is the unit ball,
    /// whose estimate from any point outside is 0.5 r ln r exactly (the
    /// iterates' log ratio does not depend on how many there were). The
    /// probe reads it at the camera's eye, at two zooms, from the eye it
    /// reports.
    #[test]
    fn the_clearance_probe_reads_the_solid_at_the_eye() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut c = cube_config();
        c.escape.formula = "quaternion_julia_solid".to_string();
        for p in ["cx", "cy", "cz", "cw"] {
            c.escape.formula_params.insert(p.to_string(), 0.0);
        }
        for zoom in [0.0f64, 1.0] {
            c.escape.zoom_log2 = zoom;
            let mut r = renderer_for(&device, &c, 64, 48);
            assert!(r.clearance().is_none(), "nothing measured yet");
            r.probe_clearance(&device, &queue, &c.escape);
            let mut got = None;
            for _ in 0..200 {
                wait(&device);
                r.poll_clearance(&device);
                got = r.clearance();
                if got.is_some() {
                    break;
                }
            }
            r.destroy();
            let cl = got.expect("the probe lands");
            let rr = cl.eye.iter().map(|x| x * x).sum::<f64>().sqrt();
            let want = 0.5 * rr * rr.ln();
            println!("  zoom {zoom}: eye at {rr:.4}, clearance {:.6}, 0.5 r ln r {want:.6}", cl.distance);
            assert!(rr > 1.0, "the eye is outside the ball");
            assert!((cl.distance / want - 1.0).abs() < 1.0e-3, "{} against {want}", cl.distance);
        }
    }

    /// The clearance probe compiles and lands for every shipped solid --
    /// IFS flames, KIFS, the Mandelbox, the Mandelbulb, the Julia set --
    /// its layout holding only what each distance function reads: a
    /// finite distance, positive from a camera outside the solid.
    #[test]
    fn the_clearance_probe_lands_for_every_shipped_solid() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let presets: Vec<FractalConfig> = serde_json::from_str::<Vec<serde_json::Value>>(crate::resources::presets::PRESETS_JSON)
            .expect("the presets parse")
            .into_iter()
            .filter_map(|v| FractalConfig::from_json(&v.to_string()).ok())
            .filter(|c| c.render_mode == RenderMode::Escape && crate::escape::ifs::formula_is_solid(&c.escape.formula))
            .collect();
        assert!(presets.len() >= 15, "{} solid presets", presets.len());
        for c in &presets {
            let mut r = renderer_for(&device, c, 32, 24);
            r.probe_clearance(&device, &queue, &c.escape);
            let mut got = None;
            for _ in 0..200 {
                wait(&device);
                r.poll_clearance(&device);
                got = r.clearance();
                if got.is_some() {
                    break;
                }
            }
            r.destroy();
            let cl = got.unwrap_or_else(|| panic!("{}: the probe never landed", c.flame.name));
            println!("  {}: clearance {:.4}", c.flame.name, cl.distance);
            assert!(cl.distance.is_finite() && cl.distance > 0.0, "{}: {}", c.flame.name, cl.distance);
        }
    }

    /// A Rhai solid calls built-in flame variations (3d-look-and-tools P5,
    /// C3), through the maps flame built in 3D: the shipped KIFS Menger
    /// example calling `linear3D` in its step is the example, pixel for
    /// pixel; calling `spherical3D` (an inversion) is the same inversion
    /// written in Rhai, to the march's edge pixels, and not the example;
    /// and a parameter bound to the solid's own (`power: p.scale`, the
    /// scale's default 3) reaches the variation as the number does.
    #[test]
    fn a_solid_calls_built_in_variations() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        use crate::config::escape::{CustomDialect, CustomSource};
        const N: u32 = 96;
        let base = include_str!("../../../assets/formulas/solid-kifs-menger.rhai").replace("\r\n", "\n");
        assert!(base.contains("fn step(s, p) {\n"), "the example's step");
        let with = |line: &str| base.replacen("fn step(s, p) {\n", &format!("fn step(s, p) {{\n    {line}\n"), 1);
        let coverage = |src: &str| -> Vec<bool> {
            let source = CustomSource { dialect: CustomDialect::Rhai, entry: "menger".to_string(), source: src.to_string(), variations: Vec::new() };
            let mut c = FractalConfig::default();
            c.render_mode = RenderMode::Escape;
            c.escape.formula = crate::escape::custom::name_of(crate::escape::lang::EntryKind::Solid, &source);
            c.escape.custom_formula = Some(source);
            c.escape.cam_yaw = 0.9;
            c.escape.cam_pitch = 0.42;
            c.escape.formula_params.insert("shadow".to_string(), 0.0);
            c.escape.formula_params.insert("occlusion".to_string(), 0.0);
            let white = crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 1.0, 1.0] };
            c.palette = crate::scene::palette::Palette {
                name: "White".to_string(),
                stops: vec![white.clone(), crate::scene::palette::ColorStop { position: 1.0, ..white }],
                locked: false,
                built_in: false,
            };
            let job = crate::renderer::RenderJob::new(&c, N, N);
            let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)).expect("render");
            out.rgba_data.chunks(4).map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).collect()
        };
        let apart = |a: &[bool], b: &[bool]| a.iter().zip(b).filter(|(x, y)| x != y).count();
        let plain = coverage(&base);
        let covered = plain.iter().filter(|x| **x).count();
        let ident = coverage(&with("s.z = vary(\"linear3D\", s.z);"));
        let inverted = coverage(&with("s.z = vary(\"spherical3D\", s.z);"));
        let twin = coverage(&with("s.z = s.z * (1.0 / max(dot(s.z, s.z), 1e-30));"));
        println!(
            "  the example covers {covered} of {}; linear3D apart {}; spherical3D apart from its twin {}, from the example {}",
            N * N,
            apart(&plain, &ident),
            apart(&inverted, &twin),
            apart(&inverted, &plain)
        );
        assert!(covered > (N * N / 20) as usize, "the view shows the sponge: {covered}");
        assert_eq!(apart(&plain, &ident), 0, "linear3D is the identity");
        assert!(apart(&inverted, &twin) <= (N * N / 400) as usize, "spherical3D is its twin");
        assert!(apart(&inverted, &plain) > (N * N / 100) as usize, "spherical3D does something");
        let bound = coverage(&with("s.z = vary(\"julia3D\", s.z, #{ power: p.scale });"));
        let number = coverage(&with("s.z = vary(\"julia3D\", s.z, #{ power: 3 });"));
        println!("  julia3D at the solid's scale apart from at 3: {}", apart(&bound, &number));
        assert_eq!(apart(&bound, &number), 0, "the solid's parameter reaches the variation");
    }

    /// A solid's Orbit transformations (3d-look-and-tools P5) move what
    /// its colouring measures and nothing else, in every solid's walk:
    /// under the orbit trap, an Orbit `linear3D` is the picture to the
    /// byte, and so is `spherical3D` at amount 0; `spherical3D` at 1
    /// recolours it -- but under a white palette, where the colour is not
    /// seen, it is the picture to the byte again: the shape and the light
    /// are the walk's. The path tracer's colour site likewise, on the
    /// Mandelbulb (curved, where light reads truest).
    #[test]
    fn a_solids_orbit_transformations_move_its_colour_alone() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        use crate::config::escape::{CustomDialect, CustomSource, EscapeTransform, TransformStage};
        use crate::scene::palette::{ColorStop, Palette};
        const N: u32 = 96;
        let stop = |position: f32, color: [f32; 3]| ColorStop { position, color };
        let palette = |stops: Vec<ColorStop>| Palette { name: "Test".to_string(), stops, locked: false, built_in: false };
        let rainbow = palette(vec![stop(0.0, [1.0, 0.1, 0.1]), stop(0.33, [0.1, 1.0, 0.1]), stop(0.66, [0.1, 0.1, 1.0]), stop(1.0, [1.0, 0.1, 0.1])]);
        let white = palette(vec![stop(0.0, [1.0; 3]), stop(1.0, [1.0; 3])]);
        let render = |base: &FractalConfig, palette: &Palette, orbit: Option<(&str, f32)>| -> Vec<u8> {
            let mut c = base.clone();
            c.palette = palette.clone();
            c.escape.transforms = orbit.map(|(v, a)| vec![EscapeTransform::new(TransformStage::Orbit, v, a)]).unwrap_or_default();
            let job = crate::renderer::RenderJob::new(&c, N, N);
            pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)).expect("render").rgba_data
        };
        let apart = |a: &[u8], b: &[u8]| a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count();
        let rhai = {
            let src = include_str!("../../../assets/formulas/solid-kifs-menger.rhai").replace("\r\n", "\n");
            CustomSource { dialect: CustomDialect::Rhai, entry: "menger".to_string(), source: src, variations: Vec::new() }
        };
        let mut solids: Vec<FractalConfig> = Vec::new();
        for name in ["mandelbulb_solid", "mandelbox_solid", "kifs_solid", "quaternion_julia_solid", "rhai"] {
            let mut c = FractalConfig::default();
            c.render_mode = RenderMode::Escape;
            if name == "rhai" {
                c.escape.formula = crate::escape::custom::name_of(crate::escape::lang::EntryKind::Solid, &rhai);
                c.escape.custom_formula = Some(rhai.clone());
            } else {
                c.escape.formula = name.to_string();
            }
            c.escape.coloring = "ifs_trap".to_string();
            c.escape.cam_yaw = 0.9;
            c.escape.cam_pitch = 0.42;
            solids.push(c);
        }
        for c in &solids {
            let plain = render(c, &rainbow, None);
            let covered = plain.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).count();
            let ident = render(c, &rainbow, Some(("linear3D", 1.0)));
            let none = render(c, &rainbow, Some(("spherical3D", 0.0)));
            let moved = render(c, &rainbow, Some(("spherical3D", 1.0)));
            let shape = apart(&render(c, &white, None), &render(c, &white, Some(("spherical3D", 1.0))));
            println!(
                "  {}: covers {covered}; linear3D apart {}, at 0 apart {}, spherical3D apart {}, under white apart {shape}",
                c.escape.formula,
                apart(&plain, &ident),
                apart(&plain, &none),
                apart(&plain, &moved)
            );
            assert!(covered > (N * N / 20) as usize, "{}: the view shows the solid", c.escape.formula);
            assert_eq!(apart(&plain, &ident), 0, "{}: an Orbit linear3D changed the picture", c.escape.formula);
            assert_eq!(apart(&plain, &none), 0, "{}: an Orbit transformation at 0 changed the picture", c.escape.formula);
            assert!(apart(&plain, &moved) > covered / 10, "{}: an Orbit spherical3D left the colour", c.escape.formula);
            assert_eq!(shape, 0, "{}: an Orbit transformation moved the shape or the light", c.escape.formula);
        }
        // The path tracer's colour site.
        let mut c = solids[0].clone();
        c.escape.solid_tier = RenderTier::PathTraced;
        c.escape.path.samples = 2;
        c.escape.path.denoise = false;
        let plain = render(&c, &rainbow, None);
        let moved = render(&c, &rainbow, Some(("spherical3D", 1.0)));
        let shape = apart(&render(&c, &white, None), &render(&c, &white, Some(("spherical3D", 1.0))));
        println!("  path traced: spherical3D apart {}, under white apart {shape}", apart(&plain, &moved));
        assert!(apart(&plain, &moved) > (N * N / 50) as usize, "the path tracer's colour left the Orbit transformation out");
        assert_eq!(shape, 0, "the path tracer's shape or light moved");
    }

    /// The material's reflectance scales the albedo every tier lights: the
    /// lit tier and the path tracer's direct light by it, the light a
    /// bounce brings by its square or more -- so shadows the bounces alone
    /// light darken faster than the lit faces, which is what it is for.
    /// The glow is the colour's own. On the Mandelbulb, curved, where the
    /// bounces read truest.
    #[test]
    fn reflectance_darkens_the_bounced_light_faster() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        use crate::scene::palette::{ColorStop, Palette};
        const W: u32 = 128;
        const H: u32 = 96;
        let grey = ColorStop { position: 0.0, color: [0.8, 0.8, 0.8] };
        let mut base = FractalConfig::default();
        base.render_mode = RenderMode::Escape;
        base.escape.formula = "mandelbulb_solid".to_string();
        base.escape.cam_yaw = 0.9;
        base.escape.cam_pitch = 0.42;
        base.escape.formula_params.insert("shadow".to_string(), 1.0);
        base.escape.formula_params.insert("occlusion".to_string(), 0.0);
        base.escape.path.samples = 64;
        base.palette = Palette { name: "Grey".to_string(), stops: vec![grey.clone(), ColorStop { position: 1.0, ..grey }], locked: false, built_in: false };
        base.background_color = [0.0; 3];
        base.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        base.exposure = 1.0;
        base.gamma = 1.0;
        let s = &mut base.solid_shading;
        s.shading_strength = 1.0;
        s.ambient = 0.0;
        s.specular = 0.0;
        for (i, l) in s.lights.iter_mut().enumerate() {
            l.enabled = i == 0;
        }
        s.lights[0].azimuth = 145.0;
        s.lights[0].elevation = 40.0;
        s.lights[0].intensity = 0.8;
        // Each pixel's light, linear: the output is the tone map's 1/2.2.
        let render = |c: &FractalConfig| -> Vec<f64> {
            let job = crate::renderer::RenderJob::new(c, W, H);
            let out = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)).expect("render");
            out.rgba_data.chunks(4).map(|p| p[..3].iter().map(|v| (*v as f64 / 255.0).powf(2.2)).sum::<f64>() / 3.0).collect()
        };
        let with = |tier: RenderTier, bounces: u32, reflectance: f32| {
            let mut c = base.clone();
            c.escape.solid_tier = tier;
            c.escape.path.bounces = bounces;
            c.solid_shading.reflectance = reflectance;
            render(&c)
        };
        let mean = |v: &[f64], mask: &[bool]| v.iter().zip(mask).filter(|(_, m)| **m).map(|(x, _)| *x).sum::<f64>() / mask.iter().filter(|m| **m).count().max(1) as f64;
        let direct = with(RenderTier::PathTraced, 0, 1.0);
        let full = with(RenderTier::PathTraced, 2, 1.0);
        // Lit: what the light reaches. Shadowed: covered, and lit by bounces
        // alone.
        let lit: Vec<bool> = direct.iter().map(|x| *x > 0.05).collect();
        let shadow: Vec<bool> = direct.iter().zip(&full).map(|(d, f)| *d == 0.0 && *f > 0.002).collect();
        let n_lit = lit.iter().filter(|m| **m).count();
        let n_shadow = shadow.iter().filter(|m| **m).count();
        let lit_tier = (mean(&with(RenderTier::Lit, 2, 0.5), &lit), mean(&with(RenderTier::Lit, 2, 1.0), &lit));
        let direct_half = mean(&with(RenderTier::PathTraced, 0, 0.5), &lit) / mean(&direct, &lit);
        let full_half = with(RenderTier::PathTraced, 2, 0.5);
        let faces = mean(&full_half, &lit) / mean(&full, &lit);
        let shadows = mean(&full_half, &shadow) / mean(&full, &shadow);
        println!(
            "  {n_lit} lit and {n_shadow} shadowed pixels; at reflectance 0.5: the lit tier {:.3}, the direct light {direct_half:.3}, path traced lit faces {faces:.3}, shadows {shadows:.3}",
            lit_tier.0 / lit_tier.1
        );
        assert!(n_lit > 200 && n_shadow > 200, "the view has lit faces and shadows");
        assert!((lit_tier.0 / lit_tier.1 - 0.5).abs() < 0.03, "the lit tier scales by the reflectance");
        assert!((direct_half - 0.5).abs() < 0.03, "the direct light scales by the reflectance");
        assert!(shadows < 0.3, "the bounced light falls by the square or more: {shadows}");
        assert!(shadows < faces * 0.7, "the shadows darken faster than the lit faces");
        // The glow: the colour's own, whatever the reflectance.
        let mut glow = base.clone();
        glow.solid_shading.lights[0].enabled = false;
        glow.solid_shading.glow = 0.5;
        glow.escape.solid_tier = RenderTier::Lit;
        let a = render(&glow);
        glow.solid_shading.reflectance = 0.25;
        assert_eq!(a, render(&glow), "the glow moved with the reflectance");
        assert!(a.iter().any(|x| *x > 0.05), "the glow shows");
    }

    /// The Fly Mode section's conversion (2026-10-19): a solid whose lights
    /// follow the camera, its lights fixed in the fractal where they are,
    /// is the same picture -- two directional lights with their shadows,
    /// and the studio, whose fixed frame is the camera's at the solids'
    /// usual view, where this one stands -- and the lights no longer
    /// follow. On the Mandelbulb, both tiers.
    #[cfg(feature = "web-app")]
    #[test]
    fn fixing_a_solids_lights_in_the_fractal_keeps_its_picture() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const N: u32 = 96;
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.formula = "mandelbulb_solid".to_string();
        c.escape.cam_yaw = 0.9;
        c.escape.cam_pitch = 0.42;
        c.escape.formula_params.insert("shadow".to_string(), 1.0);
        c.escape.path.studio = true;
        c.escape.path.samples = 8;
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.solid_shading.shading_strength = 1.0;
        c.solid_shading.ambient = 0.0;
        c.solid_shading.gloss = 0.06;
        c.solid_shading.lights[1].enabled = true;
        c.solid_shading.lights_follow_camera = true;
        let render = |c: &FractalConfig| {
            let job = crate::renderer::RenderJob::new(c, N, N);
            pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)).expect("render").rgba_data
        };
        let apart = |a: &[u8], b: &[u8]| a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 2)).count();
        for tier in [RenderTier::Lit, RenderTier::PathTraced] {
            let mut old = c.clone();
            old.escape.solid_tier = tier;
            let mut fixed = old.clone();
            for (path, value) in crate::ui::render_mode::lights_to_world(&old) {
                crate::config::ConfigManager::apply_value_detached(&mut fixed, crate::config::EditingTarget::Main, &path, value).expect("applies");
            }
            assert!(!fixed.solid_shading.lights_follow_camera, "the lights no longer follow");
            assert_ne!(fixed.solid_shading.lights[0].azimuth, old.solid_shading.lights[0].azimuth, "the angles turned");
            let (a, b) = (render(&old), render(&fixed));
            let lit = a.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).count();
            let d = apart(&a, &b);
            println!("  {tier:?}: {d} of {lit} lit pixels apart");
            assert!(lit > (N * N / 10) as usize, "{tier:?}: the view shows the bulb");
            assert!(d * 500 <= lit, "{tier:?}: {d} of {lit} pixels apart");
        }
    }

    /// The ray tolerance (`SolidShadingSettings::ray_tolerance`) reaches
    /// both tiers' rays leaving a surface: at Coarse a near miss counts as
    /// a hit, so the Mandelbulb -- whose distance estimate is not a strict
    /// bound -- darkens in its fine shadows, lit and path traced, while
    /// the Menger sponge, all flat faces, keeps its picture.
    #[cfg(feature = "web-app")]
    #[test]
    fn the_ray_tolerance_reaches_both_tiers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const N: u32 = 96;
        let presets = crate::resources::load_presets_with_fallback();
        let render = |c: &FractalConfig| {
            let job = crate::renderer::RenderJob::new(c, N, N);
            pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)).expect("render").rgba_data
        };
        let sum = |a: &[u8]| a.chunks(4).map(|p| p[0] as f64 + p[1] as f64 + p[2] as f64).sum::<f64>();
        for (scene, tier) in [("Mandelbulb", RenderTier::Lit), ("Mandelbulb", RenderTier::PathTraced), ("Menger Sponge", RenderTier::Lit), ("Menger Sponge", RenderTier::PathTraced)] {
            let mut c = presets.iter().find(|c| c.flame.name == scene).expect("the preset").clone();
            c.escape.solid_tier = tier;
            c.escape.path.samples = 8;
            c.escape.path.denoise = false;
            let fine = render(&c);
            c.solid_shading.ray_tolerance = 10.0;
            let coarse = render(&c);
            let apart = fine.chunks(4).zip(coarse.chunks(4)).filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 2)).count();
            let ratio = sum(&coarse) / sum(&fine).max(1.0);
            println!("{scene} {tier:?}: {apart} of {} pixels apart, brightness {ratio:.4}", N * N);
            // (Measured 2026-10-20: the Mandelbulb's lit tier 10 pixels
            // apart, its path tracer 0.54% darker; the Menger's within
            // 0.03%.)
            match (scene, tier) {
                ("Mandelbulb", RenderTier::Lit) => assert!(apart > 0, "the lit tier's shadows did not read the tolerance"),
                ("Mandelbulb", _) => assert!(ratio < 0.999, "the path tracer did not darken: {ratio}"),
                _ => assert!((ratio - 1.0).abs() < 0.003, "{scene} {tier:?} moved: {ratio}"),
            }
        }
    }

    /// MEASUREMENT (escape-sim-followups, *the solid path tracer's speed*):
    /// a preset (`PT_PRESET`, the Menger Sponge) path traced at 320x180 --
    /// the time a sample takes as a still renders it, and how far
    /// `PT_SAMPLES` (64) samples are from a reference: the RMSE over the
    /// pixels the solid covers, and the mean's bias. The reference is read
    /// from `PT_REF` when that file exists, else rendered at
    /// `PT_REF_SAMPLES` (1024) by the tracer as it is and written there, so
    /// a candidate change is measured against the tracer before it.
    #[test]
    #[ignore = "measurement: needs a GPU"]
    fn path_quality_against_a_reference() {
        let (device, queue) = device().expect("gpu");
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u32>().ok());
        let (w, h) = (env("PT_W").unwrap_or(320), env("PT_H").unwrap_or(180));
        let name = std::env::var("PT_PRESET").unwrap_or_else(|_| "Menger Sponge".to_string());
        let samples: u32 = env("PT_SAMPLES").unwrap_or(64);
        // `PT_TIME_ONLY`: the time alone, no reference; `PT_W`, `PT_H` the
        // size. `PT_PATCH` a candidate's shader patch (`cost_patch`), the
        // reference rendered without it.
        let time_only = std::env::var("PT_TIME_ONLY").is_ok();
        let patch = std::env::var("PT_PATCH").ok().map(|p| cost_patch(&p));
        let mut c = crate::resources::load_presets_with_fallback().into_iter().find(|c| c.flame.name == name).expect("the preset");
        let (_pt, palette) = config_palette(&device, &queue, &c);
        c.escape.solid_tier = RenderTier::PathTraced;
        c.escape.path.denoise = false;
        let picture = |samples: u32, seed: u32, patched: bool| {
            let mut r = solid_renderer_for(&device, &c, w, h);
            r.path_seed_offset = seed;
            if patched {
                r.path_source_patch = patch;
            }
            // Compiled and warm before the clock starts.
            r.render_solid_still(&device, &queue, &c, &palette, 1, || { wait(&device); true });
            r.reset_solid_path();
            let t = std::time::Instant::now();
            r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
            let ms = t.elapsed().as_secs_f64() * 1000.0 / samples as f64;
            let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            r.destroy();
            (out, ms)
        };
        if time_only {
            let (_, ms) = picture(samples, 0, true);
            println!("{name} at {w}x{h}: {ms:.2} ms a sample over {samples}");
            return;
        }
        let path = std::env::var("PT_REF").ok();
        let reference: Vec<[f32; 4]> = match path.as_ref().and_then(|p| std::fs::read(p).ok()) {
            Some(bytes) => bytemuck::cast_slice(&bytes).to_vec(),
            None => {
                let n: u32 = std::env::var("PT_REF_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(1024);
                // A stream of its own: the samples measured are not in it.
                let (r, ms) = picture(n, 7919, false);
                println!("reference: {n} samples, {ms:.1} ms a sample");
                if let Some(p) = &path {
                    std::fs::write(p, bytemuck::cast_slice::<[f32; 4], u8>(&r)).expect("write the reference");
                }
                r
            }
        };
        let (out, ms) = picture(samples, 0, true);
        let (mut n, mut se, mut sum_o, mut sum_r) = (0usize, 0.0f64, 0.0f64, 0.0f64);
        for (o, r) in out.iter().zip(&reference) {
            if r[3] < 0.999 {
                continue;
            }
            n += 1;
            for k in 0..3 {
                se += ((o[k] - r[k]) as f64).powi(2);
                sum_o += o[k] as f64;
                sum_r += r[k] as f64;
            }
        }
        let rmse = (se / (3 * n.max(1)) as f64).sqrt();
        let mean_r = sum_r / (3 * n.max(1)) as f64;
        println!(
            "{name}, {samples} samples: {ms:.1} ms a sample; RMSE {rmse:.4} ({:.1}% of the mean {mean_r:.4}), bias {:+.2}% over {n} pixels",
            100.0 * rmse / mean_r.max(1e-12),
            100.0 * (sum_o - sum_r) / sum_r.max(1e-12)
        );
    }

    /// MEASUREMENT (path-tracing-research, the denoiser's gates): the
    /// denoiser against the raw sum, by scene and material, at 4, 16 and 64
    /// samples -- the Menger Sponge (flat faces: blotches), the Mandelbulb
    /// (curved: speckle) and the Mandelbox, each matte (as it ships),
    /// glossy (gloss 0.04, roughness 0.15) and metal (metallic 1, roughness
    /// 0.4) -- against a reference of `PT_REF_SAMPLES` (1024) undenoised
    /// samples from a stream of its own, cached in `PT_REF_DIR` when set.
    /// The error is the luminance's over the pixels the solid covers whole,
    /// as a share of the reference's mean, and split by scale: **fine**,
    /// the error less its 3x3 mean (speckle), and **coarse**, its 9x9 mean
    /// (blotches) -- the two RMSE weighs alike and the eye does not.
    /// `PT_SCENES` and `PT_VARIANTS` (comma-separated) pick a subset.
    #[test]
    #[ignore = "measurement: needs a GPU"]
    fn denoise_quality() {
        let (device, queue) = device().expect("gpu");
        let (w, h) = (320u32, 180u32);
        let list = |var: &str, default: &str| -> Vec<String> {
            std::env::var(var).unwrap_or_else(|_| default.to_string()).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        };
        let scenes = list("PT_SCENES", "Menger Sponge,Mandelbulb,Mandelbox");
        let variants = list("PT_VARIANTS", "matte,gloss,metal");
        let ref_samples: u32 = std::env::var("PT_REF_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(1024);
        let ref_dir = std::env::var("PT_REF_DIR").ok();
        // The truth: no firefly clamp in the reference, so a change to the
        // estimator (its clamp, its regularisation) is measured with its
        // bias, not against the old estimator's.
        let truth = std::env::var("PT_REF_TRUTH").is_ok();
        let presets = crate::resources::load_presets_with_fallback();
        let mut table = Vec::new();
        for scene in &scenes {
            let base = presets.iter().find(|c| &c.flame.name == scene).unwrap_or_else(|| panic!("no preset {scene}")).clone();
            // The preset's own palette: a white one makes every albedo
            // white, and the denoiser's albedo guide nothing to separate.
            let (_pt, palette) = config_palette(&device, &queue, &base);
            for variant in &variants {
                let c = gate_variant(&base, variant);
                let picture = |samples: u32, seed: u32, denoise: bool, no_clamp: bool| {
                    let mut c = c.clone();
                    c.escape.path.denoise = denoise;
                    let mut r = solid_renderer_for(&device, &c, w, h);
                    r.path_seed_offset = seed;
                    r.path_no_clamp = no_clamp;
                    r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
                    let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
                    r.destroy();
                    out
                };
                let file = ref_dir.as_ref().map(|d| format!("{d}/{}-{variant}-{ref_samples}-{w}x{h}{}.bin", scene.replace(' ', "_"), if truth { "-truth" } else { "" }));
                let reference: Vec<[f32; 4]> = match file.as_ref().and_then(|f| std::fs::read(f).ok()) {
                    Some(bytes) => bytemuck::cast_slice(&bytes).to_vec(),
                    None => {
                        let r = picture(ref_samples, 7919, false, truth);
                        if let Some(f) = &file {
                            std::fs::write(f, bytemuck::cast_slice::<[f32; 4], u8>(&r)).expect("write the reference");
                        }
                        r
                    }
                };
                let errors = |out: &[[f32; 4]]| quality_errors(out, &reference, w, h);
                for samples in [4u32, 16, 64] {
                    let raw = errors(&picture(samples, 0, false, false));
                    let dn = errors(&picture(samples, 0, true, false));
                    table.push(format!(
                        "{scene:<14} {variant:<6} {samples:>3} | raw {:>6.2}% fine {:>6.2}% coarse {:>6.2}% | denoised {:>6.2}% fine {:>6.2}% coarse {:>6.2}%",
                        raw[0], raw[1], raw[2], dn[0], dn[1], dn[2]
                    ));
                    println!("{}", table.last().unwrap());
                }
            }
        }
        println!("\n{}", table.join("\n"));
    }

    /// MEASUREMENT (path-tracing-research S1): how much of the march loop's
    /// time a warp's lanes sit idle. One sample of each scene at 320x180,
    /// its shader patched to count each lane's distance steps by phase --
    /// the camera ray; then per bounce, the first light's shadow ray, the
    /// others', and the bounce's march -- written over the sum and the
    /// guides. A warp is 32 lanes, an 8x4 block of the frame (the 8x8
    /// workgroup's halves). The march loops run one after another, so a
    /// warp spends on each phase its slowest lane's steps:
    /// - **now**: all the lanes' steps over 32 times the sum, by phase, of
    ///   the warp's slowest;
    /// - **fused** (S2 without refill: one loop, every phase): over 32 times
    ///   the warp's slowest lane's total;
    /// - with refill (S2 whole), the lanes pull new pixels and the
    ///   efficiency nears 1: its gain is 1 / now.
    /// Steps are distance calls, not their cost; a solid's own iteration
    /// count varies within each call too, and is not counted here.
    #[test]
    #[ignore = "measurement: needs a GPU"]
    fn march_lane_efficiency() {
        let (device, queue) = device().expect("gpu");
        let (w, h) = (320u32, 180u32);
        fn patch(src: String) -> String {
            let swap = |s: String, from: &str, to: &str| {
                assert_eq!(s.matches(from).count(), 1, "patch site: {from}");
                s.replace(from, to)
            };
            let mut s = format!("var<private> pt_steps: array<u32, 12>;\nvar<private> pt_phase: u32;\n{src}");
            s = swap(s, "let dist = ifs_distance_at(ray.o + ray.d * t, eps * 0.01);", "let dist = ifs_distance_at(ray.o + ray.d * t, eps * 0.01);\n        pt_steps[0] = pt_steps[0] + 1u;");
            s = swap(s, "let dist = ifs_distance_at(o + d * t, eps);", "let dist = ifs_distance_at(o + d * t, eps);\n        pt_steps[pt_phase] = pt_steps[pt_phase] + 1u;");
            s = swap(
                s,
                "seen = select(0.0, 1.0, pt_scene_visible(s.start, l.dir, l.dist, travelled));",
                "pt_phase = min(1u + 3u * bounce + min(li, 1u), 11u);\n                    seen = select(0.0, 1.0, pt_scene_visible(s.start, l.dir, l.dist, travelled));",
            );
            s = swap(s, "h = pt_scene_next(o, d, travelled);", "pt_phase = min(3u + 3u * bounce, 11u);\n        h = pt_scene_next(o, d, travelled);");
            s = swap(s, "\n    pt_sum[idx] = sum;", "\n    pt_sum[idx] = vec4<f32>(f32(pt_steps[0]), f32(pt_steps[1]), f32(pt_steps[2]), f32(pt_steps[3]));");
            swap(
                s,
                "\n        pt_guide[3u * idx] = g0;\n        pt_guide[3u * idx + 1u] = g1;",
                "\n        pt_guide[3u * idx] = vec4<f32>(f32(pt_steps[4]), f32(pt_steps[5]), f32(pt_steps[6]), f32(pt_steps[7]));\n        pt_guide[3u * idx + 1u] = vec4<f32>(f32(pt_steps[8]), f32(pt_steps[9]), f32(pt_steps[10]), f32(pt_steps[11]));",
            )
        }
        let names = ["camera", "b0 light", "b0 lights", "b0 bounce", "b1 light", "b1 lights", "b1 bounce", "b2 light", "b2 lights", "b2 bounce"];
        let presets = crate::resources::load_presets_with_fallback();
        for scene in ["Menger Sponge", "Mandelbulb", "Mandelbox"] {
            let mut c = presets.iter().find(|c| c.flame.name == scene).expect("the preset").clone();
            let (_pt, palette) = config_palette(&device, &queue, &c);
            c.escape.solid_tier = RenderTier::PathTraced;
            c.escape.path.denoise = true;
            let mut r = solid_renderer_for(&device, &c, w, h);
            r.path_source_patch = Some(patch);
            r.render_solid_still(&device, &queue, &c, &palette, 1, || { wait(&device); true });
            let (sum, guide) = r.solid_path_buffers_for_test().expect("traced");
            let px = (w * h) as usize;
            let a: Vec<[f32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(&device, &queue, sum, px as u64 * 16)).to_vec();
            let g: Vec<[f32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(&device, &queue, guide, px as u64 * path_core::GUIDE_BYTES)).to_vec();
            r.destroy();
            let steps = |k: usize| -> [f64; 12] {
                let (p, q, s) = (a[k], g[3 * k], g[3 * k + 1]);
                [p[0], p[1], p[2], p[3], q[0], q[1], q[2], q[3], s[0], s[1], s[2], s[3]].map(|x| x as f64)
            };
            let (mut work, mut now, mut fused) = (0.0, 0.0, 0.0);
            let (mut phase_work, mut phase_slow) = ([0.0f64; 12], [0.0f64; 12]);
            for by in 0..h / 4 {
                for bx in 0..w / 8 {
                    let lanes: Vec<[f64; 12]> = (0..32).map(|i| steps(((by * 4 + i / 8) * w + bx * 8 + i % 8) as usize)).collect();
                    for ph in 0..12 {
                        let slow = lanes.iter().map(|l| l[ph]).fold(0.0, f64::max);
                        let sum: f64 = lanes.iter().map(|l| l[ph]).sum();
                        phase_work[ph] += sum;
                        phase_slow[ph] += 32.0 * slow;
                        work += sum;
                        now += 32.0 * slow;
                    }
                    fused += 32.0 * lanes.iter().map(|l| l.iter().sum::<f64>()).fold(0.0, f64::max);
                }
            }
            println!("{scene}: {:.1} steps a pixel; efficiency now {:.0}%, fused {:.0}% -- refill's gain {:.2}x, fusing's alone {:.2}x", work / px as f64, 100.0 * work / now, 100.0 * work / fused, now / work, now / fused);
            for (ph, name) in names.iter().enumerate() {
                if phase_slow[ph] > 0.0 {
                    println!("    {name:<10} {:>6.1} steps a pixel, {:>3.0}% of the steps, lanes busy {:>3.0}%", phase_work[ph] / px as f64, 100.0 * phase_work[ph] / work, 100.0 * phase_work[ph] / phase_slow[ph]);
                }
            }
        }
    }

    /// Candidates for a cheaper sample (path-tracing-research, *Found on
    /// the way*), as patches of the solid path tracer's shader for
    /// `path_quality_against_a_reference` (`PT_PATCH`):
    /// - `tetra`: a hit's normal from four distance calls at a
    ///   tetrahedron's corners (Quilez) in place of six central
    ///   differences;
    /// - `wide`: a ray leaving a surface called hit within two pixels of
    ///   its footprint, not 0.4 -- under the same cap;
    /// - `cap`, `cap3`: that cap a thousandth, or three ten-thousandths,
    ///   of the solid's ball, not a ten-thousandth.
    fn cost_patch(name: &str) -> fn(String) -> String {
        const LEAVE: &str = "let eps = max(min(ifs_radius() * ifs_ray_tolerance(), e0 * 0.4), 1.0e-30);";
        fn swap(s: String, from: &str, to: &str) -> String {
            assert_eq!(s.matches(from).count(), 1, "patch site: {from}");
            s.replace(from, to)
        }
        fn tetra(s: String) -> String {
            swap(
                s,
                "    let n = vec3<f32>(
        ifs_distance_at(p + dx, e) - ifs_distance_at(p - dx, e),
        ifs_distance_at(p + dy, e) - ifs_distance_at(p - dy, e),
        ifs_distance_at(p + dz, e) - ifs_distance_at(p - dz, e),
    );",
                "    let k = vec2<f32>(1.0, -1.0);
    let n = k.xyy * ifs_distance_at(p + k.xyy * h, e) + k.yyx * ifs_distance_at(p + k.yyx * h, e) + k.yxy * ifs_distance_at(p + k.yxy * h, e) + k.xxx * ifs_distance_at(p + k.xxx * h, e);",
            )
        }
        fn wide(s: String) -> String {
            swap(s, LEAVE, "let eps = max(min(ifs_radius() * ifs_ray_tolerance(), e0 * 2.0), 1.0e-30);")
        }
        fn cap(s: String) -> String {
            swap(s, LEAVE, "let eps = max(min(ifs_radius() * 1.0e-3, e0 * 0.4), 1.0e-30);")
        }
        fn cap3(s: String) -> String {
            swap(s, LEAVE, "let eps = max(min(ifs_radius() * 3.0e-4, e0 * 0.4), 1.0e-30);")
        }
        match name {
            "tetra" => tetra,
            "wide" => wide,
            "cap" => cap,
            "cap3" => cap3,
            other => panic!("no patch {other}"),
        }
    }

    /// The gates' material variants of a solid preset, path traced:
    /// matte (as it ships), glossy (gloss 0.04, roughness 0.15) and metal
    /// (metallic 1, roughness 0.4).
    fn gate_variant(base: &FractalConfig, variant: &str) -> FractalConfig {
        let mut c = base.clone();
        c.escape.solid_tier = RenderTier::PathTraced;
        match variant {
            "matte" => {}
            "gloss" => {
                c.solid_shading.gloss = 0.04;
                c.solid_shading.roughness = 0.15;
            }
            "metal" => {
                c.solid_shading.metallic = 1.0;
                c.solid_shading.roughness = 0.4;
            }
            other => panic!("no variant {other}"),
        }
        c
    }

    /// MEASUREMENT (path-tracing-research V2, before building it): what
    /// adaptive sampling could gain at best. Each gate scene and variant
    /// rendered twice at `PT_SAMPLES` (64) samples from two streams, each
    /// pixel's luminance variance from the guides' second moment; then, on
    /// the CPU, the gate's error (`quality_errors`'s whole, as a mean
    /// square over the pixels the solid covers) with the same total
    /// samples dealt out per pixel in proportion to its standard deviation
    /// as the FIRST render estimates it -- the allocation that minimises
    /// the error, above a minimum count -- judged by the SECOND render's
    /// variances (an allocation from the variances it is judged by would
    /// count a firefly it happened to see as foreseen). Against the same
    /// samples dealt evenly; the gain is the samples an even deal needs to
    /// match it, over the samples it had. Only pixels some sample hit
    /// are dealt samples (a miss costs next to nothing), and a sample is
    /// priced the same on every one of them: a ceiling, with variances no
    /// sampler knows at its minimum count.
    #[test]
    #[ignore = "measurement: needs a GPU"]
    fn adaptive_sampling_potential() {
        let (device, queue) = device().expect("gpu");
        let (w, h) = (320u32, 180u32);
        let samples: u32 = std::env::var("PT_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(64);
        let presets = crate::resources::load_presets_with_fallback();
        let lum = |p: [f32; 4]| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
        let px = (w * h) as usize;
        let n = samples as f64;
        for scene in ["Menger Sponge", "Mandelbulb", "Mandelbox"] {
            let base = presets.iter().find(|c| c.flame.name == scene).expect("the preset").clone();
            let (_pt, palette) = config_palette(&device, &queue, &base);
            for variant in ["matte", "gloss", "metal"] {
                let mut c = gate_variant(&base, variant);
                c.escape.path.denoise = true;
                // A pixel's sums and its variance, from one stream.
                let render = |seed: u32| {
                    let mut r = solid_renderer_for(&device, &c, w, h);
                    r.path_seed_offset = seed;
                    r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
                    let (sum, guide) = r.solid_path_buffers_for_test().expect("traced");
                    let a: Vec<[f32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(&device, &queue, sum, px as u64 * 16)).to_vec();
                    let g: Vec<[f32; 4]> = bytemuck::cast_slice(&crate::escape::terrain::gpu_tests::read_buffer(&device, &queue, guide, px as u64 * path_core::GUIDE_BYTES)).to_vec();
                    r.destroy();
                    // A sample's premultiplied luminance: its second moment
                    // (the guide's, a hit's luminance squared; a miss adds
                    // nothing) less its mean's square.
                    // (The guides hold each share's moment; their sum, the
                    // whole's but for the shares' covariance.)
                    let var: Vec<f64> = (0..px).map(|k| ((g[3 * k][3] + g[3 * k + 2][3]) as f64 / n - (lum(a[k]) / n).powi(2)).max(0.0)).collect();
                    (a, var)
                };
                let (a, var_a) = render(0);
                let (_, var_b) = render(7919);
                let (wi, hi) = (w as i32, h as i32);
                let covered = |x: i32, y: i32| x >= 0 && y >= 0 && x < wi && y < hi && a[(y * wi + x) as usize][3] as f64 >= n - 0.5;
                let inside: Vec<bool> = (0..wi * hi).map(|k| (-1..=1).all(|dy| (-1..=1).all(|dx| covered(k % wi + dx, k / wi + dy)))).collect();
                let solid: Vec<usize> = (0..px).filter(|&k| a[k][3] > 0.0).collect();
                let count = inside.iter().filter(|&&b| b).count() as f64;
                let mean = (0..px).filter(|&k| inside[k]).map(|k| lum(a[k]) / n).sum::<f64>() / count.max(1.0);
                let sd: Vec<f64> = var_a.iter().map(|v| v.sqrt()).collect();
                // The gate's mean square at counts `ns`, over the inside,
                // by the second render's variances.
                let mse = |ns: &dyn Fn(usize) -> f64| (0..px).filter(|&k| inside[k]).map(|k| var_b[k] / ns(k)).sum::<f64>() / count.max(1.0);
                // How much of the even deal's error the worst 1% of the
                // inside holds: the tail an allocation chases.
                let mut sq: Vec<f64> = (0..px).filter(|&k| inside[k]).map(|k| var_b[k]).collect();
                sq.sort_by(|x, y| y.total_cmp(x));
                let top = sq.iter().take(sq.len() / 100).sum::<f64>() / sq.iter().sum::<f64>().max(1e-30);
                let mut line = format!("{scene:<14} {variant:<6} solid {:>3.0}% inside {:>3.0}% top1% {:>3.0}%", 100.0 * solid.len() as f64 / px as f64, 100.0 * count / px as f64, 100.0 * top);
                for avg in [16.0f64, 64.0] {
                    let even = mse(&|_| avg);
                    for min in [4.0f64, 8.0] {
                        // The scale whose counts, max(min, k sd), average
                        // `avg` over the solid's pixels: by bisection.
                        let total = |k: f64| solid.iter().map(|&i| (k * sd[i]).max(min)).sum::<f64>() / solid.len().max(1) as f64;
                        let (mut lo, mut hi_k) = (0.0f64, 1.0f64);
                        while total(hi_k) < avg {
                            hi_k *= 2.0;
                        }
                        for _ in 0..60 {
                            let mid = 0.5 * (lo + hi_k);
                            if total(mid) < avg { lo = mid } else { hi_k = mid }
                        }
                        let k = hi_k;
                        let best = mse(&|i| (k * sd[i]).max(min));
                        line += &format!(" | {avg:>2}/{min}: {:.2}x ({:.2}% -> {:.2}%)", even / best, 100.0 * even.sqrt() / mean, 100.0 * best.sqrt() / mean);
                    }
                }
                println!("{line}");
            }
        }
    }

    /// MEASUREMENT (path-tracing-research V2): adaptive sampling against
    /// the even deal at equal time. Each gate case (`gate_variant`; subsets
    /// by `PT_SCENES`, `PT_VARIANTS`) at 320x180 (`PT_W`, `PT_H`),
    /// undenoised (`PT_DENOISE` to denoise both), against a reference of
    /// `PT_REF_SAMPLES` (4096) samples of the same estimator from a stream
    /// of its own (cached in `PT_REF_DIR`): the
    /// even deal at 8 to 128 samples, and adaptive sampling to at most
    /// `PT_MAX` (256) at each of `PT_THRESHOLDS` (levels; "8,4,2,1"), its
    /// first check at `PT_MIN` (`ADAPTIVE_MIN_SAMPLES`), its noise from the
    /// odd half with `PT_HALVES`. Each picture timed as a still renders it,
    /// warm; the error is `quality_errors`'s, and as the display shows it
    /// (`display_error`: what the threshold is in). The speedup is the time
    /// the even deal needs for the adaptive picture's error -- interpolated
    /// between its counts in log-log, past them at error ~ time^-1/2 --
    /// over the adaptive picture's time, by each error.
    #[test]
    #[ignore = "measurement: needs a GPU"]
    fn adaptive_quality() {
        let (device, queue) = device().expect("gpu");
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u32>().ok());
        let (w, h) = (env("PT_W").unwrap_or(320), env("PT_H").unwrap_or(180));
        let ref_samples = env("PT_REF_SAMPLES").unwrap_or(4096);
        let px = (w * h) as usize;
        let list = |var: &str, default: &str| -> Vec<String> {
            std::env::var(var).unwrap_or_else(|_| default.to_string()).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        };
        let scenes = list("PT_SCENES", "Menger Sponge,Mandelbulb,Mandelbox");
        let variants = list("PT_VARIANTS", "matte,gloss,metal");
        let thresholds: Vec<f32> = list("PT_THRESHOLDS", "8,4,2,1").iter().map(|t| t.parse().expect("a threshold")).collect();
        let max = env("PT_MAX").unwrap_or(256);
        let min = env("PT_MIN").unwrap_or(crate::escape::path_core::ADAPTIVE_MIN_SAMPLES);
        let halves = std::env::var("PT_HALVES").is_ok();
        let denoise = std::env::var("PT_DENOISE").is_ok();
        let ref_dir = std::env::var("PT_REF_DIR").ok();
        let presets = crate::resources::load_presets_with_fallback();
        let mut table = Vec::new();
        for scene in &scenes {
            let base = presets.iter().find(|c| &c.flame.name == scene).unwrap_or_else(|| panic!("no preset {scene}")).clone();
            let (_pt, palette) = config_palette(&device, &queue, &base);
            for variant in &variants {
                let mut c = gate_variant(&base, variant);
                c.escape.path.denoise = denoise;
                let display = crate::escape::path_core::path_settings(&c, 1.0).display;
                // A picture, its time and its mean samples a pixel.
                let picture = |c: &FractalConfig, samples: u32, seed: u32| {
                    let adaptive = c.escape.path.noise_threshold > 0.0;
                    let mut r = solid_renderer_for(&device, c, w, h);
                    r.path_seed_offset = seed;
                    r.path_adapt = Some((min, halves));
                    // Compiled and warm before the clock starts: the check's
                    // passes too.
                    r.render_solid_still(&device, &queue, c, &palette, if adaptive { min } else { 1 }, || { wait(&device); true });
                    r.reset_solid_path();
                    let t = std::time::Instant::now();
                    r.render_solid_still(&device, &queue, c, &palette, samples, || { wait(&device); true });
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
                    let mean_n = if adaptive {
                        let a: Vec<[f32; 4]> = bytemuck::cast_slice(&read_buffer(&device, &queue, r.solid_path_sum_for_test().expect("traced"), (px * 32) as u64)).to_vec();
                        let n = |k: usize| a[px + k][2].abs() as u32;
                        // How the samples were dealt: the share of the
                        // pixels, and of the warps (8x4 blocks), that went to
                        // the end -- a warp runs while any of its lanes does.
                        let at_end = (0..px).filter(|&k| n(k) >= samples).count();
                        let (mut warps, mut busy) = (0usize, 0usize);
                        for by in 0..h / 4 {
                            for bx in 0..w / 8 {
                                warps += 1;
                                busy += usize::from((0..32).any(|i| n(((by * 4 + i / 8) * w + bx * 8 + i % 8) as usize) >= samples));
                            }
                        }
                        println!("    at the end: {:.1}% of the pixels, {:.1}% of the warps", 100.0 * at_end as f64 / px as f64, 100.0 * busy as f64 / warps as f64);
                        let d = &r.solid_path.as_ref().expect("traced").dispatched;
                        let whole: Vec<u32> = d.iter().filter(|&&(units, _, cols)| cols == w && units >= h && units % h == 0).map(|&(units, _, _)| units / h).collect();
                        let listed: Vec<u32> = d.iter().filter(|&&(_, _, cols)| cols == 0).map(|&(units, _, _)| units).collect();
                        println!("    {} dispatches, {} of them whole frames (samples {:?}); {} of the list, {} entry-samples: {:?}", d.len(), whole.len(), whole, listed.len(), listed.iter().sum::<u32>(), listed);
                        (0..px).map(|k| n(k) as f64).sum::<f64>() / px as f64
                    } else {
                        samples as f64
                    };
                    r.destroy();
                    (out, ms, mean_n)
                };
                let file = ref_dir.as_ref().map(|d| format!("{d}/{}-{variant}-{ref_samples}-{w}x{h}.bin", scene.replace(' ', "_")));
                let reference: Vec<[f32; 4]> = match file.as_ref().and_then(|f| std::fs::read(f).ok()) {
                    Some(bytes) => bytemuck::cast_slice(&bytes).to_vec(),
                    None => {
                        let mut plain = c.clone();
                        plain.escape.path.denoise = false;
                        let (r, ms, _) = picture(&plain, ref_samples, 7919);
                        println!("reference: {ref_samples} samples in {ms:.0} ms");
                        if let Some(f) = &file {
                            std::fs::write(f, bytemuck::cast_slice::<[f32; 4], u8>(&r)).expect("write the reference");
                        }
                        r
                    }
                };
                let mut even = Vec::new();
                for n in [8u32, 16, 32, 64, 128] {
                    let (out, ms, _) = picture(&c, n, 0);
                    let e = quality_errors(&out, &reference, w, h);
                    let d = display_error(&out, &reference, w, h, display);
                    even.push((ms, e[0], d));
                    table.push(format!("{scene:<14} {variant:<6} even {n:>4}        | {ms:>8.0} ms | error {:>6.2}% fine {:>6.2}% coarse {:>6.2}% | {d:>5.2} levels", e[0], e[1], e[2]));
                    println!("{}", table.last().unwrap());
                }
                // The even deal's time to an error (by `which`), in log-log.
                let even_time = |err: f64, which: usize| {
                    let pts: Vec<(f64, f64)> = even.iter().map(|&(t, e, d)| (t.ln(), if which == 0 { e } else { d }.ln())).collect();
                    let le = err.ln();
                    for pair in pts.windows(2) {
                        let ((t0, e0), (t1, e1)) = (pair[0], pair[1]);
                        if (le <= e0 && le >= e1) || (le >= e0 && le <= e1) {
                            let f = if (e1 - e0).abs() > 1e-12 { (le - e0) / (e1 - e0) } else { 0.0 };
                            return (t0 + f * (t1 - t0)).exp();
                        }
                    }
                    // Past the ends: error ~ time^-1/2.
                    let (t, e) = if le < pts.last().unwrap().1 { *pts.last().unwrap() } else { pts[0] };
                    (t - 2.0 * (le - e)).exp()
                };
                for &threshold in &thresholds {
                    let mut a = c.clone();
                    a.escape.path.noise_threshold = threshold;
                    let (out, ms, mean_n) = picture(&a, max, 0);
                    let e = quality_errors(&out, &reference, w, h);
                    let d = display_error(&out, &reference, w, h, display);
                    table.push(format!(
                        "{scene:<14} {variant:<6} T {threshold:>4} n {mean_n:>6.1} | {ms:>8.0} ms | error {:>6.2}% fine {:>6.2}% coarse {:>6.2}% | {d:>5.2} levels | speedup {:.2}x, displayed {:.2}x",
                        e[0], e[1], e[2], even_time(e[0], 0) / ms, even_time(d, 1) / ms
                    ));
                    println!("{}", table.last().unwrap());
                }
            }
        }
        println!("\n{}", table.join("\n"));
    }

    /// A config's palette as the renderer reads it (256 entries).
    fn config_palette(device: &Device, queue: &Queue, c: &FractalConfig) -> (Texture, TextureView) {
        let tex = crate::gpu::buffers::create_palette_texture(device, 256, "test palette");
        crate::gpu::buffers::write_palette_table(queue, &tex, 256, &c.palette.generate_texture_data(256));
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    /// [`renderer_for`] for any solid: the flame's packed when it has one.
    fn solid_renderer_for(device: &Device, c: &FractalConfig, w: u32, h: u32) -> EscapeRenderer {
        let mut r = EscapeRenderer::new(device, w, h);
        r.resize(device, w, h, 1);
        let def = crate::escape::ifs::get_ifs(&c.escape.formula).expect("solid");
        let registry = crate::variations::global_registry();
        r.set_ifs(crate::escape::ifs::pack_for(def, c, &registry));
        drop(registry);
        r.set_solid_lighting(&c.solid_shading, (c.fog_strength, c.fog_start, c.background_color));
        r.set_solid_sky(crate::escape::path_core::sky_seen(&c.escape.path, c));
        r.set_solid_material(crate::escape::path_core::lit_material(c));
        r.set_solid_studio(crate::escape::path_core::studio_level(&c.escape.path, c));
        r
    }

    /// A picture's luminance error against a reference over the pixels the
    /// solid covers whole (with their 3x3 neighbours), as percentages of
    /// the reference's mean: [the whole, the fine part (the error less its
    /// 3x3 mean), the coarse part (its 9x9 mean)].
    fn quality_errors(out: &[[f32; 4]], reference: &[[f32; 4]], w: u32, h: u32) -> [f64; 3] {
        let lum = |p: &[f32; 4]| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
        let (wi, hi) = (w as i32, h as i32);
        let covered = |x: i32, y: i32| x >= 0 && y >= 0 && x < wi && y < hi && reference[(y * wi + x) as usize][3] >= 0.999;
        let inside: Vec<bool> = (0..wi * hi).map(|k| (-1..=1).all(|dy| (-1..=1).all(|dx| covered(k % wi + dx, k / wi + dy)))).collect();
        let e: Vec<f64> = out.iter().zip(reference).map(|(o, r)| lum(o) - lum(r)).collect();
        let mean_of = |x: i32, y: i32, r: i32| {
            let (mut s, mut n) = (0.0, 0.0);
            for dy in -r..=r {
                for dx in -r..=r {
                    if covered(x + dx, y + dy) {
                        s += e[((y + dy) * wi + x + dx) as usize];
                        n += 1.0;
                    }
                }
            }
            s / f64::max(n, 1.0)
        };
        let (mut n, mut whole, mut fine, mut coarse, mut sum_r) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for k in 0..wi * hi {
            if !inside[k as usize] {
                continue;
            }
            let (x, y) = (k % wi, k / wi);
            let ek = e[k as usize];
            n += 1.0;
            whole += ek * ek;
            fine += (ek - mean_of(x, y, 1)).powi(2);
            coarse += mean_of(x, y, 4).powi(2);
            sum_r += lum(&reference[k as usize]);
        }
        let mean = (sum_r / f64::max(n, 1.0)).max(1.0e-12);
        [whole, fine, coarse].map(|s| 100.0 * (s / f64::max(n, 1.0)).sqrt() / mean)
    }

    /// A picture's error against a reference as the display shows it: the
    /// luminance through the Linear tone map's exposure and the display's
    /// power on it (`PathSettings::display`), in levels of 255, RMS over
    /// the pixels the solid covers whole (with their 3x3 neighbours) --
    /// what adaptive sampling's threshold is in, where the gate's error
    /// weighs a bright pixel's noise more than the eye does.
    fn display_error(out: &[[f32; 4]], reference: &[[f32; 4]], w: u32, h: u32, [exposure, power]: [f32; 2]) -> f64 {
        let level = |p: &[f32; 4]| {
            let v = (0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64) * exposure as f64;
            if v > 0.0 { 255.0 * v.powf(power as f64) } else { 0.0 }
        };
        let (wi, hi) = (w as i32, h as i32);
        let covered = |x: i32, y: i32| x >= 0 && y >= 0 && x < wi && y < hi && reference[(y * wi + x) as usize][3] >= 0.999;
        let (mut n, mut se) = (0.0f64, 0.0f64);
        for k in 0..wi * hi {
            let (x, y) = (k % wi, k / wi);
            if (-1..=1).all(|dy| (-1..=1).all(|dx| covered(x + dx, y + dy))) {
                n += 1.0;
                se += (level(&out[k as usize]) - level(&reference[k as usize])).powi(2);
            }
        }
        (se / n.max(1.0)).sqrt()
    }

    /// Denoise asked for where the guides -- 32 bytes a pixel, bound
    /// whole -- exceed what one storage binding holds: the picture is
    /// path traced undenoised, with no validation error (the test
    /// device panics on one). A device whose binding holds 24 bytes a
    /// pixel of the frame: the sum's 16 fit, the guides' 32 do not.
    #[test]
    fn denoise_past_the_binding_limit_is_refused() {
        let (w, h) = (128u32, 96u32);
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..InstanceDescriptor::new_without_display_handle()
        });
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let limits = egui_wgpu::wgpu::Limits { max_storage_buffer_binding_size: (w * h * 24) as u64, ..adapter.limits() };
        let (device, queue) = pollster::block_on(adapter.request_device(&egui_wgpu::wgpu::DeviceDescriptor {
            label: Some("solid path tests (small bindings)"),
            required_features: adapter.features() & Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .expect("device");
        device.on_uncaptured_error(std::sync::Arc::new(|e| panic!("wgpu error in solid path tests: {e}")));
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.escape.solid_tier = RenderTier::PathTraced;
        c.escape.path.denoise = true;
        c.background_color = [0.55, 0.62, 0.72];
        let mut r = renderer_for(&device, &c, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 2, || { wait(&device); true });
        assert_eq!(r.solid_path_samples(), 2);
        assert!(!r.solid_path.as_ref().unwrap().sum.guided(), "the guides do not fit");
        r.destroy();
    }

    /// A finished sum starts over when the light it was gathered under
    /// changes without a re-render: the tone map's exposure, which the
    /// sky's light is the inverse of, is a tone-map-only edit.
    #[test]
    fn a_solids_sum_follows_the_exposure() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.escape.solid_tier = RenderTier::PathTraced;
        c.escape.path.samples = 2;
        // A sky to light it: a black one is no light at any exposure.
        c.background_color = [0.55, 0.62, 0.72];
        let mut r = renderer_for(&device, &c, 48, 32);
        let mut guard = 0;
        while r.trace_solid_viewport(&device, &queue, &c, &palette, VIEWPORT_FRAME_MS) && guard < 400 {
            wait(&device);
            guard += 1;
        }
        wait(&device);
        assert_eq!(r.solid_path_samples(), 2, "finished");
        let generation = |r: &EscapeRenderer| r.solid_path.as_ref().map(|p| p.generation).unwrap();
        // Nothing changed: it stays finished.
        let g0 = generation(&r);
        assert!(!r.trace_solid_viewport(&device, &queue, &c, &palette, VIEWPORT_FRAME_MS));
        assert_eq!(generation(&r), g0);
        // The exposure: it starts over (and, this small, finishes again).
        c.exposure *= 2.0;
        r.trace_solid_viewport(&device, &queue, &c, &palette, VIEWPORT_FRAME_MS);
        wait(&device);
        assert_eq!(generation(&r), g0.wrapping_add(1), "started over");
        r.destroy();
    }

    /// No dispatch outgrows the model's band, whatever the measurements
    /// say: given all the time in the world, a 1080p solid still goes out
    /// in bands -- rows times samples, times the share of the width a tile
    /// across takes -- no larger than the cap, before anything is measured
    /// and after. (Measurement alone once sent a whole frame at 64 samples
    /// and lost the device.) The cap is the process's, which another test's
    /// slow call can halve part-way: a band then begun across finishes its
    /// rows in tiles.
    #[test]
    fn no_dispatch_outgrows_the_model() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (_pt, palette) = white_palette(&device, &queue);
        let (w, h) = (1920u32, 1080u32);
        let c = cube_config();
        let mut r = renderer_for(&device, &c, w, h);
        for _ in 0..3 {
            r.trace_solid(&device, &queue, &c, &palette, 4, 1.0e9);
            wait(&device);
        }
        let d = &r.solid_path.as_ref().expect("traced").dispatched;
        let worst = d.iter().map(|&(units, cap, cols)| units as f32 * cols as f32 / w as f32 / cap as f32).fold(0.0f32, f32::max);
        println!("{} dispatches, the largest {worst:.2} of its cap", d.len());
        assert!(d.len() > 1 && worst <= 1.0, "{worst}");
        r.destroy();
    }

    /// The same samples in any banding are the same bits: three samples
    /// as whole frames, as bands of the fewest rows, and as tiles across
    /// them, a sky and a sun, three bounces and a coat -- and again.
    #[test]
    fn solid_samples_are_band_invariant() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (40u32, 24u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.background_color = [0.3, 0.4, 0.6];
        c.solid_shading.lights[0].enabled = true;
        c.escape.path.bounces = 3;
        c.solid_shading.gloss = 0.04;
        let mut r = renderer_for(&device, &c, w, h);
        r.solid_path = Some(SolidPath::new(&device, w, h));
        let mut sums = Vec::new();
        for (frame_ms, ceiling) in [(1.0e9f32, None), (1.0e-9, None), (1.0e9, Some(1.0e-6f32)), (1.0e-9, Some(1.0e-6)), (1.0e9, None)] {
            r.solid_path.as_mut().expect("made").ceiling = ceiling;
            r.reset_solid_path();
            let mut guard = 0;
            while r.trace_solid(&device, &queue, &c, &palette, 3, frame_ms) {
                wait(&device);
                guard += 1;
                assert!(guard < 10_000);
            }
            assert_eq!(r.solid_path_samples(), 3);
            sums.push(read_buffer(&device, &queue, r.solid_path_sum_for_test().expect("traced"), (w * h * 16) as u64));
        }
        assert!(sums.iter().all(|s| *s == sums[0]), "the banding moved the sum");
        r.destroy();
    }

    /// A small Mandelbulb path traced adaptively (V2) to `target` samples
    /// at a noise threshold, its first check at `min`, its noise from the
    /// odd half when `halves`, in calls of `frame_ms` (and `ceiling`, tiles
    /// across the bands): the sums and the statistics after them.
    #[allow(clippy::too_many_arguments)]
    fn adaptive_bulb(device: &Device, queue: &Queue, c: &FractalConfig, palette: &TextureView, (w, h): (u32, u32), target: u32, (min, halves): (u32, bool), (frame_ms, ceiling): (f32, Option<f32>)) -> (EscapeRenderer, Vec<[f32; 4]>) {
        let mut r = solid_renderer_for(device, c, w, h);
        r.path_adapt = Some((min, halves));
        r.solid_path = Some(SolidPath::new(device, w, h));
        r.solid_path.as_mut().expect("made").ceiling = ceiling;
        let mut guard = 0;
        while r.trace_solid(device, queue, c, palette, target, frame_ms) {
            wait(device);
            guard += 1;
            assert!(guard < 100_000);
        }
        let px = (w * h) as u64;
        let sums = bytemuck::cast_slice(&read_buffer(device, queue, r.solid_path_sum_for_test().expect("traced"), px * 32)).to_vec();
        (r, sums)
    }

    /// The Mandelbulb preset path traced, undenoised, sampling adaptively
    /// at `threshold` levels.
    fn adaptive_config(threshold: f32) -> FractalConfig {
        let mut c = crate::resources::load_presets_with_fallback().into_iter().find(|c| c.flame.name == "Mandelbulb").expect("the preset");
        c.escape.solid_tier = RenderTier::PathTraced;
        c.escape.path.denoise = false;
        c.escape.path.noise_threshold = threshold;
        c
    }

    /// Adaptive sampling (V2) deals samples, it does not change them: each
    /// pixel's sum is, to the bit, the sum the plain tracer gives it at
    /// that pixel's own count -- some stopped at the first check, some
    /// went to the target, traced from the check's list -- however the
    /// frame was banded (whole frames, tiles across the bands); and the
    /// resolve divides each by its own count.
    #[test]
    fn adaptive_pixels_draw_their_own_streams() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let px = (w * h) as usize;
        let c = adaptive_config(3.0);
        let (_pt, palette) = config_palette(&device, &queue, &c);
        const TARGET: u32 = 24;
        let (r, a) = adaptive_bulb(&device, &queue, &c, &palette, (w, h), TARGET, (4, false), (1.0e9, None));
        let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        r.destroy();
        let (r, b) = adaptive_bulb(&device, &queue, &c, &palette, (w, h), TARGET, (4, false), (1.0e-9, Some(1.0e-6)));
        r.destroy();
        assert!(a == b, "the banding moved the adaptive sum");
        let n: Vec<u32> = (0..px).map(|k| a[px + k][2].abs() as u32).collect();
        let mut counts: Vec<u32> = n.clone();
        counts.sort_unstable();
        counts.dedup();
        let early = n.iter().filter(|&&k| k < TARGET).count();
        println!("counts {counts:?}; {early} of {px} pixels stopped early");
        assert!(early > 0 && early < px, "{early} of {px} stopped early: the test needs both");
        let mut plain = c.clone();
        plain.escape.path.noise_threshold = 0.0;
        for &k in &counts {
            let mut r = solid_renderer_for(&device, &plain, w, h);
            r.render_solid_still(&device, &queue, &plain, &palette, k, || { wait(&device); true });
            let u: Vec<[f32; 4]> = bytemuck::cast_slice(&read_buffer(&device, &queue, r.solid_path_sum_for_test().expect("traced"), (px * 16) as u64)).to_vec();
            r.destroy();
            for i in (0..px).filter(|&i| n[i] == k) {
                assert_eq!(a[i], u[i], "pixel {i} at {k} samples");
            }
        }
        // The resolve: the mean over the coverage, and the coverage over
        // the pixel's own count.
        for i in 0..px {
            let s = a[i];
            let want = if s[3] > 0.0 { [s[0] / s[3], s[1] / s[3], s[2] / s[3], s[3] / n[i] as f32] } else { [0.0, 0.0, 0.0, 0.0] };
            for k in 0..4 {
                assert!((out[i][k] - want[k]).abs() <= 1.0e-6 * want[k].abs().max(1.0), "pixel {i}: {:?} against {want:?}", out[i]);
            }
        }
    }

    /// The check (`PATH_ADAPT_WGSL`) is its arithmetic: one check at 8
    /// samples, each pixel's noise what the CPU makes of its statistics --
    /// the mean's standard error from the moments, or the odd half's gap,
    /// one up from the mean in the display's levels -- and a pixel
    /// converged exactly where its noise and its neighbours' are under the
    /// threshold.
    #[test]
    fn the_adaptive_check_is_its_arithmetic() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let px = (w * h) as usize;
        let threshold = 3.0f32;
        let c = adaptive_config(threshold);
        let (_pt, palette) = config_palette(&device, &queue, &c);
        let [exposure, power] = crate::escape::path_core::path_settings(&c, 1.0).display;
        let level = |x: f64| if x * exposure as f64 > 0.0 { 255.0 * (x * exposure as f64).powf(power as f64) } else { 0.0 };
        let lum = |p: [f32; 4]| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
        for halves in [false, true] {
            let (r, a) = adaptive_bulb(&device, &queue, &c, &palette, (w, h), 8, (8, halves), (1.0e9, None));
            r.destroy();
            let mut converged = 0;
            for i in 0..px {
                let st = a[px + i];
                assert_eq!(st[2].abs(), 8.0, "pixel {i}'s count");
                let m = lum(a[i]) / 8.0;
                let se = if halves { (st[1] as f64 / 4.0 - m).abs() } else { ((st[0] as f64 / 8.0 - m * m).max(0.0) / 7.0).sqrt() };
                let err = level(m + se) - level(m);
                assert!((st[3] as f64 - err).abs() <= 2.0e-3 * err.max(1.0), "pixel {i} (halves {halves}): noise {} against {err}", st[3]);
                let (x, y) = ((i as u32 % w) as i32, (i as u32 / w) as i32);
                let mut worst = 0.0f32;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (qx, qy) = (x + dx, y + dy);
                        if qx >= 0 && qy >= 0 && qx < w as i32 && qy < h as i32 {
                            worst = worst.max(a[px + (qy as u32 * w + qx as u32) as usize][3]);
                        }
                    }
                }
                assert_eq!(st[2] < 0.0, worst < threshold, "pixel {i}'s flag (halves {halves}): worst {worst}");
                converged += usize::from(st[2] < 0.0);
            }
            println!("halves {halves}: {converged} of {px} converged at 8");
            assert!(converged > 0 && converged < px, "{converged} of {px}: the test needs both");
        }
    }

    /// A picture sampled adaptively stops once every pixel has converged
    /// (V2), well short of the samples asked for: at a threshold nothing
    /// exceeds, every pixel stops at the first check, and the calls after
    /// it -- before its count came back -- added nothing.
    #[test]
    fn adaptive_sampling_stops_once_every_pixel_converges() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let px = (w * h) as usize;
        let c = adaptive_config(1.0e6);
        let (_pt, palette) = config_palette(&device, &queue, &c);
        let (r, a) = adaptive_bulb(&device, &queue, &c, &palette, (w, h), 4096, (4, false), (EXPORT_FRAME_MS, None));
        let samples = r.solid_path_samples();
        assert!(r.solid_path_shown() || samples > 0, "traced");
        r.destroy();
        println!("stopped at {samples} of 4096");
        assert!(samples < 64, "went on to {samples}");
        assert!((0..px).all(|k| a[px + k][2] == -4.0), "every pixel converged at the first check");
    }

    /// The denoiser's specular share (path-tracing-research D5), gathered
    /// with its guides: none on a matte surface without a highlight; all
    /// the light on a metal -- no glow, nothing diffuse under it, every
    /// bounce the coat's lobe -- to rounding (the share sums the same terms
    /// in another order); and the picture's sum the same bits whether the
    /// guides are gathered or not.
    #[test]
    fn the_specular_share_is_the_coats() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (40u32, 24u32);
        let px = (w * h) as usize;
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.specular = 0.0;
        c.solid_shading.glow = 0.0;
        c.solid_shading.gloss = 0.0;
        c.escape.path.bounces = 2;
        let trace = |c: &FractalConfig| {
            let mut r = renderer_for(&device, c, w, h);
            r.solid_path = Some(SolidPath::new(&device, w, h));
            while r.trace_solid(&device, &queue, c, &palette, 8, 1.0e9) {
                wait(&device);
            }
            let (sum, guide) = r.solid_path_buffers_for_test().expect("traced");
            let a: Vec<[f32; 4]> = bytemuck::cast_slice(&read_buffer(&device, &queue, sum, (px * 16) as u64)).to_vec();
            let g: Vec<[f32; 4]> = if c.escape.path.denoise {
                bytemuck::cast_slice(&read_buffer(&device, &queue, guide, px as u64 * path_core::GUIDE_BYTES)).to_vec()
            } else {
                Vec::new()
            };
            r.destroy();
            (a, g)
        };
        for metallic in [0.0f32, 1.0] {
            c.solid_shading.metallic = metallic;
            c.escape.path.denoise = false;
            let (plain, _) = trace(&c);
            c.escape.path.denoise = true;
            let (a, g) = trace(&c);
            assert!(a == plain, "metallic {metallic}: gathering the guides moved the sum");
            let lit = (0..px).filter(|&k| a[k][3] > 0.0 && a[k][0] + a[k][1] + a[k][2] > 0.0).count();
            assert!(lit > px / 10, "metallic {metallic}: the view shows the cube lit");
            for k in (0..px).filter(|&k| a[k][3] > 0.0) {
                let spec = g[3 * k + 2];
                for ch in 0..3 {
                    if metallic == 0.0 {
                        assert_eq!(spec[ch], 0.0, "pixel {k}: a matte surface reflected specularly");
                    } else {
                        assert!((spec[ch] - a[k][ch]).abs() <= 1.0e-4 * a[k][ch].abs().max(1.0e-3), "pixel {k}: the metal's share {spec:?} of {:?}", a[k]);
                    }
                }
            }
        }
    }

    /// A cube under a sky alone (its lights off), path traced: with the
    /// background's uniform light, or with an HDRI sky that `map` makes,
    /// its settings the config's.
    fn sky_lit_cube(device: &Device, queue: &Queue, map: Option<&crate::escape::environment::EnvMap>, backdrop: bool) -> Vec<[f32; 4]> {
        let (w, h) = (48u32, 32u32);
        let (_pt, palette) = white_palette(device, queue);
        let mut c = cube_config();
        c.solid_shading.lights[0].enabled = false;
        c.solid_shading.shading_strength = 1.0;
        c.solid_shading.ambient = 0.0;
        c.background_color = [0.5, 0.6, 0.7];
        c.escape.path.bounces = 2;
        let mut r = renderer_for(device, &c, w, h);
        if let Some(map) = map {
            c.escape.path.environment_map = "sunflowers".to_string();
            c.escape.path.environment_backdrop = backdrop;
            let set = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("bundled");
            r.set_environment_map_for_test(device, queue, map, set);
        }
        r.solid_path = Some(SolidPath::new(device, w, h));
        while r.trace_solid(device, queue, &c, &palette, 16, 1.0e9) {
            wait(device);
        }
        let out = read_texture(device, queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        r.destroy();
        out
    }

    /// A one-colour map as the uniform sky (docs/projects/hdri-environment.md):
    /// an HDRI sky whose every texel is the uniform sky's radiance over the
    /// map's scale lights the cube as the background's light does, to the
    /// map's half-float rounding -- the lookup, its scale and the ambient
    /// beside it are the sky's; and seen as the backdrop it covers the
    /// pixels the cube does not, at that radiance.
    #[test]
    fn a_one_colour_sky_lights_as_the_uniform_sky() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut c = cube_config();
        c.background_color = [0.5, 0.6, 0.7];
        let uniform = crate::escape::path_core::path_settings(&c, 1.0).environment;
        c.escape.path.environment_map = "sunflowers".to_string();
        let scale = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("bundled").scale;
        let texel = uniform.map(|v| v / scale);
        let map = crate::escape::environment::EnvMap { width: 8, height: 4, texels: vec![texel; 32], sun: None, median: 1.0 };
        let plain = sky_lit_cube(&device, &queue, None, false);
        let mapped = sky_lit_cube(&device, &queue, Some(&map), false);
        let (mut worst, mut lit) = (0.0f32, 0usize);
        for (a, b) in plain.iter().zip(&mapped) {
            assert_eq!(a[3], b[3], "the coverage");
            if a[3] > 0.0 {
                lit += 1;
                for k in 0..3 {
                    worst = worst.max((a[k] - b[k]).abs() / a[k].abs().max(1.0e-3));
                }
            }
        }
        println!("{lit} covered pixels, the map's within {worst:e} of the uniform sky's");
        assert!(lit > 100 && worst < 2.0e-3, "{worst}");
        // The backdrop: every pixel covered, the sky's at its radiance.
        let seen = sky_lit_cube(&device, &queue, Some(&map), true);
        let expect = texel.map(|v| half::f16::from_f32(v).to_f32() * scale);
        for (k, (p, q)) in plain.iter().zip(&seen).enumerate() {
            assert_eq!(q[3], 1.0, "pixel {k}: the backdrop covers");
            if p[3] == 0.0 {
                for ch in 0..3 {
                    assert!((q[ch] - expect[ch]).abs() <= 1.0e-5 * expect[ch].max(1.0), "pixel {k}: {q:?} against {expect:?}");
                }
            }
        }
    }

    /// An HDRI sky's horizon is level from every side (user-reported
    /// 2026-10-20: read in the studio's frame, a camera's at the solids'
    /// usual view, it stood slanted about 45 degrees, level only at yaw 50
    /// and -130). A sky bright above its horizon and dark below, seen as the
    /// backdrop from four yaws: the horizon crosses the frame's left and
    /// right edges at the same row -- a camera without bank sees a level
    /// world's horizon as a level line.
    #[test]
    fn an_hdri_skys_horizon_is_level_from_every_side() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.escape.path.environment_map = "sunflowers".to_string();
        c.escape.path.bounces = 0;
        c.escape.cam_pitch = 0.15;
        let (mw, mh) = (64u32, 32u32);
        let texels: Vec<[f32; 3]> = (0..mw * mh).map(|k| if k / mw < mh / 2 { [1.0; 3] } else { [0.01; 3] }).collect();
        let map = crate::escape::environment::EnvMap { width: mw, height: mh, texels, sun: None, median: 1.0 };
        for yaw in [0.0f32, 1.0, 2.2, -2.6] {
            c.escape.cam_yaw = yaw;
            let set = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("bundled");
            let mut r = renderer_for(&device, &c, w, h);
            r.set_environment_map_for_test(&device, &queue, &map, set);
            r.solid_path = Some(SolidPath::new(&device, w, h));
            while r.trace_solid(&device, &queue, &c, &palette, 1, 1.0e9) {
                wait(&device);
            }
            let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            r.destroy();
            let bright = set.scale * 0.5;
            let horizon = |x: u32| (0..h).find(|&y| out[(y * w + x) as usize][1] < bright);
            let (left, right) = (horizon(1), horizon(w - 2));
            println!("yaw {yaw}: the horizon at row {left:?} on the left, {right:?} on the right");
            let (l, r) = (left.expect("a horizon on the left"), right.expect("a horizon on the right"));
            assert!(l.abs_diff(r) <= 1, "yaw {yaw}: rows {l} and {r}");
        }
    }

    /// The sky's horizon raised and lowered (user request 2026-10-20: "raise
    /// /lower the apparent skybox ... align the horizon with the fractal"):
    /// the half-bright, half-dark sky of the test above, its horizon set to
    /// -8, 0 and 8 degrees -- the line in the picture moves down and up with
    /// it, and stays level from two yaws.
    #[test]
    fn the_skys_horizon_rises_and_stays_level() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.escape.path.environment_map = "sunflowers".to_string();
        c.escape.path.bounces = 0;
        c.escape.cam_pitch = 0.15;
        let (mw, mh) = (64u32, 32u32);
        let texels: Vec<[f32; 3]> = (0..mw * mh).map(|k| if k / mw < mh / 2 { [1.0; 3] } else { [0.01; 3] }).collect();
        let map = crate::escape::environment::EnvMap { width: mw, height: mh, texels, sun: None, median: 1.0 };
        for yaw in [0.4f32, -2.0] {
            c.escape.cam_yaw = yaw;
            let mut rows = Vec::new();
            for horizon in [-8.0f32, 0.0, 8.0] {
                c.escape.path.environment_horizon = horizon;
                let set = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("bundled");
                let mut r = renderer_for(&device, &c, w, h);
                r.set_environment_map_for_test(&device, &queue, &map, set);
                r.solid_path = Some(SolidPath::new(&device, w, h));
                while r.trace_solid(&device, &queue, &c, &palette, 1, 1.0e9) {
                    wait(&device);
                }
                let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
                r.destroy();
                let bright = set.scale * 0.5;
                let line = |x: u32| (0..h).find(|&y| out[(y * w + x) as usize][1] < bright).expect("a horizon in frame");
                let (l, r) = (line(1), line(w - 2));
                println!("yaw {yaw}, horizon {horizon}: rows {l} and {r}");
                assert!(l.abs_diff(r) <= 1, "yaw {yaw}, horizon {horizon}: rows {l} and {r}");
                rows.push(l);
            }
            assert!(rows[0] > rows[1] + 2 && rows[1] > rows[2] + 2, "yaw {yaw}: the line moves with the horizon: {rows:?}");
        }
    }

    /// The Sunflowers sun lifted out is the rig's: a cube with its lights
    /// off, under that sky, lit tier, has faces the sun lights -- brighter
    /// than under the same sky's map without its sun.
    #[test]
    fn the_skys_sun_lights_the_lit_tier() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.solid_shading.lights[0].enabled = false;
        c.solid_shading.shading_strength = 1.0;
        c.escape.path.environment_map = "sunflowers".to_string();
        c.escape.solid_tier = RenderTier::Lit;
        let set = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("bundled");
        let map = crate::escape::environment::load(set.def).expect("the bundled file");
        assert!(map.sun.is_some());
        let lit = |map: &crate::escape::environment::EnvMap| {
            let mut r = renderer_for(&device, &c, w, h);
            r.set_environment_map_for_test(&device, &queue, map, set);
            loop {
                let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
                let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
                queue.submit(std::iter::once(enc.finish()));
                if settled {
                    break;
                }
            }
            let out = read_texture(&device, &queue, &r.output_texture, w, h);
            r.destroy();
            out.iter().map(|p| (p[0] + p[1] + p[2]) as f64).sum::<f64>()
        };
        let with = lit(&map);
        let without = lit(&crate::escape::environment::EnvMap { sun: None, ..map.clone() });
        println!("lit tier: {with:.1} with the sun, {without:.1} without");
        assert!(with > without * 1.2, "{with} against {without}");
    }

    /// More samples are never speckier, denoised (user-reported 2026-10-20:
    /// "noise increases with more samples. There's certainly more
    /// speckles"). The confidence bound switched on whole at 8 samples, and
    /// the neighbourhood clamp loosened by the count from 16, so a rare
    /// bright path clamped at 16 came through at 32. A metal Mandelbulb,
    /// denoised at 7 to 64 samples: the pixels far brighter than 256
    /// undenoised samples show them -- as the display shows them, by more
    /// than 40 levels of 255 -- never more at a count than at the one before.
    #[test]
    fn more_samples_are_never_speckier() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (128u32, 72u32);
        let base = crate::resources::load_presets_with_fallback().into_iter().find(|c| c.flame.name == "Mandelbulb").expect("the preset");
        let c = gate_variant(&base, "metal");
        let (_pt, palette) = config_palette(&device, &queue, &c);
        let [exposure, power] = crate::escape::path_core::path_settings(&c, 1.0).display;
        let level = |p: &[f32; 4]| {
            let v = (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]) * exposure;
            if v > 0.0 { 255.0 * v.powf(power) } else { 0.0 }
        };
        let picture = |samples: u32, denoise: bool, seed: u32| {
            let mut c = c.clone();
            c.escape.path.denoise = denoise;
            let mut r = solid_renderer_for(&device, &c, w, h);
            r.path_seed_offset = seed;
            r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
            let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            r.destroy();
            out
        };
        let reference = picture(256, false, 7919);
        let mut last = usize::MAX;
        for n in [7u32, 8, 16, 32, 64] {
            let out = picture(n, true, 0);
            let speckles = out.iter().zip(&reference).filter(|(o, r)| level(o) - level(r) > 40.0).count();
            println!("{n} samples: {speckles} speckles");
            assert!(speckles <= last, "{n} samples: {speckles} speckles, more than the count before's {last}");
            last = speckles;
        }
    }

    /// A polished gold Mandelbulb under the Sunflowers sky (user-reported
    /// 2026-10-20: speckle in its crevices, more with more samples) is no
    /// speckier denoised at more samples, from 8 to 32, and at 32 has few:
    /// the sun's glints seen in a second surface are clamped on their own
    /// (`PathSettings::clamp_indirect`). The pixels that stand out by 40
    /// levels from 256 samples, at 8, 16 and 32: 322, 234 and 301 without
    /// that clamp, 331, 183 and 106 with it.
    #[test]
    fn a_gold_solids_glints_settle() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (256u32, 144u32);
        // The Gold Mandelbulb preset as the user set it: polished, under
        // the sky, closer in.
        let mut c = crate::resources::load_presets_with_fallback().into_iter().find(|c| c.flame.name == "Gold Mandelbulb").expect("the preset");
        c.escape.solid_tier = RenderTier::PathTraced;
        c.solid_shading.roughness = 0.05;
        c.escape.path.environment_map = "sunflowers".to_string();
        c.escape.cam_pitch = 0.12244971;
        c.escape.cam_yaw = 0.4797994;
        c.escape.cam_fov = 0.699877;
        c.escape.cam_target_x = "-2.336204152037181392602510".to_string();
        c.escape.cam_target_y = "-1.555330670520342686406838".to_string();
        c.escape.cam_target_z = "0.250256241984536461452798".to_string();
        let (_pt, palette) = config_palette(&device, &queue, &c);
        let set = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("bundled");
        let map = crate::escape::environment::load(set.def).expect("the bundled file");
        let [exposure, power] = crate::escape::path_core::path_settings(&c, 1.0).display;
        let level = |p: &[f32; 4]| {
            let v = (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]) * exposure;
            if v > 0.0 { (255.0 * v.powf(power)).min(255.0) } else { 0.0 }
        };
        let picture = |samples: u32, denoise: bool, seed: u32| {
            let mut c = c.clone();
            c.escape.path.denoise = denoise;
            let mut r = solid_renderer_for(&device, &c, w, h);
            r.set_environment_map_for_test(&device, &queue, &map, set);
            r.path_seed_offset = seed;
            r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
            let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            r.destroy();
            out
        };
        let reference = picture(256, false, 7919);
        let pixels = (w * h) as usize;
        let mut last = usize::MAX;
        for n in [8u32, 16, 32] {
            let out = picture(n, true, 0);
            let speckles = out.iter().zip(&reference).filter(|(o, r)| level(o) - level(r) > 40.0).count();
            println!("{n} samples: {speckles} speckles");
            assert!(speckles <= last, "{n} samples: {speckles} speckles, more than the count before's {last}");
            last = speckles;
        }
        assert!(last * 200 <= pixels, "{last} speckles at 32 samples");
    }

    /// Measurement: the speckles under an HDRI sky by the sun's threshold
    /// (`environment::SUN_THRESHOLD`; user-reported 2026-10-20: an orange
    /// glow in the crevices of a gold Mandelbulb under Sunflowers, speckled).
    /// The scene from `PT_SCENE_FILE` (default the user's), at `PT_W` x
    /// `PT_H` (256x144); for each threshold in `PT_SUN_THRESHOLDS`, the
    /// sun's share, a 256-sample raw reference's mean against the first
    /// threshold's, and at 8, 16 and 32 samples the pixels brighter than
    /// the reference by more than 40 levels of 255, raw and denoised. The
    /// 16-sample denoised pictures go to `output/hdri/sun-<t>-16.png`.
    /// `PT_SUN_VARIANT`: `nosun` drops the sun lifted out, `nomap` blacks
    /// the map and keeps the sun; `PT_BOUNCES` overrides the scene's.
    #[test]
    #[ignore = "measurement; needs a GPU"]
    fn sun_threshold_speckles() {
        let (device, queue) = device().expect("gpu");
        let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let w: u32 = var("PT_W", "256").parse().expect("PT_W");
        let h: u32 = var("PT_H", "144").parse().expect("PT_H");
        let file = var("PT_SCENE_FILE", "output/hdri/gold-sunflowers-7samples.fflame");
        let mut c = FractalConfig::from_json(&std::fs::read_to_string(&file).expect("the scene")).expect("parses");
        if let Ok(b) = std::env::var("PT_BOUNCES") {
            c.escape.path.bounces = b.parse().expect("PT_BOUNCES");
        }
        let variant = var("PT_SUN_VARIANT", "full");
        let (_pt, palette) = config_palette(&device, &queue, &c);
        let set = crate::escape::path_core::environment_settings(&c.escape.path, &c).expect("a bundled sky");
        let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/environments").join(set.def.file)).expect("the sky");
        let [exposure, power] = crate::escape::path_core::path_settings(&c, 1.0).display;
        // As the display shows it: clipped at white.
        let level = |p: &[f32; 4]| {
            let v = (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]) * exposure;
            if v > 0.0 { (255.0 * v.powf(power)).min(255.0) } else { 0.0 }
        };
        let mut first_mean = None;
        for t in var("PT_SUN_THRESHOLDS", "1000,300,100,30").split(',') {
            let threshold: f32 = t.trim().parse().expect("a threshold");
            let mut map = crate::escape::environment::EnvMap::decode_at(&bytes, threshold).expect("decodes");
            match variant.as_str() {
                "nosun" => map.sun = None,
                "nomap" => map.texels.iter_mut().for_each(|t| *t = [0.0; 3]),
                _ => {}
            }
            let share = map.sun.map_or(0.0, |s| {
                let l = 0.2126 * s.irradiance[0] + 0.7152 * s.irradiance[1] + 0.0722 * s.irradiance[2];
                l / (l + map.light())
            });
            let radius = map.sun.map_or(0.0, |s| s.radius.to_degrees());
            let power = map.sun.map_or(0.0, |s| s.irradiance.iter().cloned().fold(0.0, f32::max) * set.scale / std::f32::consts::PI);
            println!("the sun's power {power:.2}, the clamp {:.2}", crate::escape::path_core::path_settings(&c, 1.0).clamp);
            let picture = |samples: u32, denoise: bool, seed: u32| {
                let mut c = c.clone();
                c.escape.path.denoise = denoise;
                let mut r = solid_renderer_for(&device, &c, w, h);
                r.set_environment_map_for_test(&device, &queue, &map, set);
                r.path_seed_offset = seed;
                r.render_solid_still(&device, &queue, &c, &palette, samples, || { wait(&device); true });
                let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
                r.destroy();
                out
            };
            let reference = picture(256, false, 7919);
            let mean = reference.iter().map(|p| level(p) as f64).sum::<f64>() / reference.len() as f64;
            let base = *first_mean.get_or_insert(mean);
            let mut line = format!("{variant}, {} bounces, threshold {threshold}: sun", c.escape.path.bounces);
            line.push_str(&format!(" {:.1}% (radius {radius:.2} deg), reference mean {mean:.1} ({:+.1}%);", share * 100.0, (mean / base - 1.0) * 100.0));
            for n in [8u32, 16, 32] {
                let count = |out: &[[f32; 4]]| out.iter().zip(&reference).filter(|(o, r)| level(o) - level(r) > 40.0).count();
                let raw = picture(n, false, 0);
                let dn = picture(n, true, 0);
                line.push_str(&format!(" {n}: {} raw, {} denoised;", count(&raw), count(&dn)));
                if n == 16 {
                    let shown = |v: f32| (255.0 * (v * exposure).max(0.0).powf(power)).min(255.0) as u8;
                    let rgba: Vec<u8> = dn.iter().flat_map(|p| [shown(p[0]), shown(p[1]), shown(p[2]), 255]).collect();
                    let _ = std::fs::create_dir_all("output/hdri");
                    image::save_buffer(format!("output/hdri/sun-{t}-16.png", t = t.trim()), &rgba, w, h, image::ColorType::Rgba8).expect("write");
                }
            }
            println!("{line}");
        }
    }

    /// A row no call has measured is priced at the model at least, not at
    /// twice the costliest row yet: the first pass meets the sky's rows
    /// first, a tenth of a millisecond each, and priced off them the next
    /// band entered the solid at the cap -- half a second, several a
    /// frame, until the device was lost. Measured rows go at their
    /// measure.
    #[test]
    fn an_unmeasured_row_is_not_priced_off_the_sky() {
        let Some((device, _queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut p = SolidPath::new(&device, 1920, 1080);
        for c in p.row_ms.iter_mut().take(200) {
            *c = 0.1;
        }
        p.max_ms = 0.17;
        // The sponge's model at 1080p: 52 rows to 300 ms.
        let model = MODEL_BAND_MS / 52.0;
        let (rows, ms) = p.plan(200, VIEWPORT_FRAME_MS, model, 52);
        assert_eq!(rows, 8, "{rows} unmeasured rows priced at {ms} ms");
        let (rows, ms) = p.plan(0, VIEWPORT_FRAME_MS, model, 52);
        assert_eq!(rows, 52, "{rows} of the sky's rows priced at {ms} ms");
        p.destroy();
    }

    /// The fewest rows a band holds fill the GPU on a narrow view: eight
    /// rows of 1920, or as many pixels in whole workgroups.
    #[test]
    fn a_narrow_view_bands_by_pixels() {
        assert_eq!(min_band_rows(1920), 8);
        assert_eq!(min_band_rows(3840), 8);
        assert_eq!(min_band_rows(960), 16);
        assert_eq!(min_band_rows(320), 48);
        assert_eq!(min_band_rows(1), 15360);
    }

    /// A band priced past the ceiling goes across the frame in tiles of
    /// whole workgroups, and the pass's part-done band resolves as far as
    /// it has gone: where the tiles have been, the picture a whole frame
    /// of the same sample makes, and nothing where they have not.
    #[test]
    fn a_band_past_the_ceiling_goes_across_in_tiles() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (40u32, 24u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.background_color = [0.3, 0.4, 0.6];
        c.solid_shading.lights[0].enabled = true;
        let mut r = renderer_for(&device, &c, w, h);
        // The whole frame's first sample, for reference.
        r.render_solid_still(&device, &queue, &c, &palette, 1, || { wait(&device); true });
        let whole = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        // The same sample three tiles in: every call one.
        let mut p = SolidPath::new(&device, w, h);
        p.ceiling = Some(1.0e-6);
        if let Some(old) = r.solid_path.replace(p) {
            old.destroy();
        }
        for _ in 0..3 {
            r.trace_solid(&device, &queue, &c, &palette, 1, 1.0e-9);
            wait(&device);
        }
        let p = r.solid_path.as_ref().expect("traced");
        assert_eq!(p.dispatched.iter().map(|d| d.2).collect::<Vec<_>>(), vec![8, 8, 8], "tiles of one workgroup");
        assert_eq!((p.row, p.col, p.sum.count()), (0, 24, 0), "a band part-way across");
        let part = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        let rows = p.band_rows;
        let mut covered = 0;
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (part[(y * w + x) as usize], whole[(y * w + x) as usize]);
                if x < 24 && y < rows {
                    assert_eq!(a, b, "({x}, {y}): a tile's pixel");
                    covered += (a[3] > 0.0) as u32;
                } else {
                    assert_eq!(a, [0.0; 4], "({x}, {y}): not reached");
                }
            }
        }
        assert!(covered > 0, "the tiles met the solid");
        r.destroy();
    }

    /// The viewport sends a call only once the last has finished: calls
    /// overlapping on the GPU were measured together -- the later next to
    /// nothing, pricing its rows as free -- and the queue held seconds of
    /// bands. A call is counted out until its completion.
    #[test]
    fn the_viewport_waits_for_its_last_call() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.escape.path.samples = 64;
        let mut r = renderer_for(&device, &c, 48, 32);
        assert!(r.trace_solid_viewport(&device, &queue, &c, &palette, VIEWPORT_FRAME_MS));
        wait(&device);
        let p = r.solid_path.as_ref().expect("traced");
        assert!(!p.busy(), "done once the GPU is");
        let sent = p.dispatched.len();
        assert!(sent > 0);
        // A call still out: the next frame sends nothing, and has more to do.
        p.clock.lock().unwrap().outstanding = 1;
        assert!(r.trace_solid_viewport(&device, &queue, &c, &palette, VIEWPORT_FRAME_MS));
        assert_eq!(r.solid_path.as_ref().unwrap().dispatched.len(), sent, "sent while a call was out");
        r.solid_path.as_ref().unwrap().clock.lock().unwrap().outstanding = 0;
        assert!(r.trace_solid_viewport(&device, &queue, &c, &palette, VIEWPORT_FRAME_MS));
        assert!(r.solid_path.as_ref().unwrap().dispatched.len() > sent, "the next once it is in");
        wait(&device);
        r.destroy();
    }

    /// Ambient in the path tracer (lights-in-the-fractal, L1). It read
    /// none before, so the slider did nothing once a view was path
    /// traced (measured: a sponge's mean 114.66 at Ambient 0, 0.5 and 1
    /// alike). Now the ambient is light the bounces bring back from
    /// every direction:
    /// - the cube, convex so every bounce gets away, lit by the ambient
    ///   alone, path traces to the lit tier's `albedo x ambient` on each
    ///   face -- exactly, one bounce being the whole of it;
    /// - the sponge's path-traced light rises with it in proportion, its
    ///   holes occluding some;
    /// - an untouched rig adds none, so a picture path traced under the
    ///   default rig is unchanged.
    #[test]
    fn the_path_tracer_sees_the_ambient() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);

        // The cube under the ambient alone: no lights, a black sky, and
        // no occlusion estimate in the lit tier (an open face has none).
        let mut c = cube_config();
        c.solid_shading.ambient = 0.5;
        c.escape.path.bounces = 1;
        c.escape.solid_tier = RenderTier::Lit;
        let mut r = renderer_for(&device, &c, w, h);
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
            queue.submit(std::iter::once(enc.finish()));
            if settled {
                break;
            }
        }
        let lit = read_texture(&device, &queue, &r.output_texture, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 4, || { wait(&device); true });
        let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        let at = |x: u32, y: u32| lit[(y * w + x) as usize];
        let (mut n, mut worst) = (0, 0.0f32);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let (p, q) = (at(x, y), path[(y * w + x) as usize]);
                let flat = (0..9u32).all(|i| {
                    let o = at(x + i % 3 - 1, y + i / 3 - 1);
                    (0..4).all(|k| (p[k] - o[k]).abs() < 1e-4)
                });
                if p[3] < 1.0 || q[3] < 1.0 || !flat || p[0] <= 0.0 {
                    continue;
                }
                n += 1;
                for k in 0..3 {
                    worst = worst.max((p[k] - q[k]).abs() / p[k].max(1e-3));
                }
            }
        }
        println!("  cube under the ambient: {n} face pixels, worst relative difference {worst:.2e}");
        assert!(n > 10, "only {n} face pixels");
        assert!(worst < 1e-3, "the path tracer's ambient is not the lit tier's: {worst}");
        r.destroy();

        // The sponge, path traced: its light rises with the ambient, in
        // proportion (the light is linear in it).
        let mut c = cube_config();
        c.flame = sponge();
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.lights[0].intensity = 0.3;
        c.escape.path.bounces = 2;
        let mut r = renderer_for(&device, &c, w, h);
        let mut light = |a: f32| {
            c.solid_shading.ambient = a;
            r.render_solid_still(&device, &queue, &c, &palette, 16, || { wait(&device); true });
            let img = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            img.iter().map(|p| (p[0] + p[1] + p[2]) as f64).sum::<f64>() / img.len() as f64
        };
        let (none, some, more) = (light(0.0), light(0.2), light(0.4));
        r.destroy();
        println!("  sponge path traced at ambient 0 / 0.2 / 0.4: {none:.4} / {some:.4} / {more:.4}");
        assert!(some > none * 1.05, "the ambient added nothing: {none} -> {some}");
        let (d1, d2) = (some - none, more - some);
        assert!((d1 - d2).abs() < 0.1 * d1, "not in proportion: +{d1} then +{d2}");

        // An untouched rig: no ambient in the path tracer.
        let untouched = crate::config::SolidShadingSettings::default();
        assert!(untouched.rig_untouched() && untouched.ambient > 0.0);
        assert_eq!(crate::escape::path_core::path_ambient(&untouched), 0.0);
        let mut d = FractalConfig::default();
        d.background_color = [0.2, 0.3, 0.4];
        let env = crate::escape::path_core::path_settings(&d, 1.0).environment;
        d.solid_shading.ambient = 0.0;
        d.solid_shading.shading_strength = 1.0;
        assert_eq!(env, crate::escape::path_core::path_settings(&d, 1.0).environment, "the default rig's ambient reached the path tracer");
    }

    /// The lit tier's settled picture of `c`, from a fresh renderer.
    fn lit_picture(device: &Device, queue: &Queue, c: &FractalConfig, palette: &TextureView, w: u32, h: u32) -> Vec<[f32; 4]> {
        let mut c = c.clone();
        c.escape.solid_tier = RenderTier::Lit;
        let mut r = renderer_for(device, &c, w, h);
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            let settled = r.render(device, queue, &mut enc, &c.escape, palette, 1);
            queue.submit(std::iter::once(enc.finish()));
            if settled {
                break;
            }
        }
        let out = read_texture(device, queue, &r.output_texture, w, h);
        r.destroy();
        out
    }

    /// A point light far off with a reach past everything is the
    /// directional light it approaches (lights-in-the-fractal, L3): the
    /// same picture, shadows and all, to the light's angle across the
    /// solid. Puts the position where the shader looks for it -- an
    /// offset from the target -- and measures the direction from it.
    #[test]
    fn a_far_point_light_is_a_directional_one() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.escape.cam_target_x = "1".to_string();
        c.escape.cam_target_y = "1".to_string();
        c.escape.cam_target_z = "1".to_string();
        c.escape.formula_params.insert("shadow".to_string(), 1.0);
        let (az, el) = (30.0f32, 35.0f32);
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.lights[0].azimuth = az;
        c.solid_shading.lights[0].elevation = el;
        c.solid_shading.lights[0].color = [1.0, 0.6, 0.3];
        let sun = lit_picture(&device, &queue, &c, &palette, w, h);

        // The same direction in the world: the light fixed in the fractal's
        // space (the default since 2026-10-19), azimuth about z from +x.
        let (a, e) = ((az as f64).to_radians(), (el as f64).to_radians());
        let dir = [e.cos() * a.cos(), e.cos() * a.sin(), e.sin()];
        let far = 1.0e4;
        let l = &mut c.solid_shading.lights[0];
        l.kind = crate::config::LightKind::Point;
        l.position = [1.0 + dir[0] * far, 1.0 + dir[1] * far, 1.0 + dir[2] * far];
        l.reach = 1.0e9;
        let point = lit_picture(&device, &queue, &c, &palette, w, h);

        let (mut n, mut worst) = (0, 0.0f32);
        for (p, q) in sun.iter().zip(&point) {
            if p[3] < 1.0 || p[0] <= 1.0e-3 {
                continue;
            }
            n += 1;
            for k in 0..3 {
                worst = worst.max((p[k] - q[k]).abs() / p[k].max(1.0e-3));
            }
        }
        println!("  far point against a directional light: {n} lit pixels, worst relative difference {worst:.2e}");
        assert!(n > 200, "only {n} lit pixels");
        assert!(worst < 5.0e-3, "{worst}");
    }

    /// Inside the sponge's central cavity, lit by a headlamp alone
    /// (lights-in-the-fractal, L3): the lamp sits at the eye, so it sees
    /// what the eye sees and casts no shadow in sight -- shadows on is
    /// shadows off. A shadow ray that did not stop at the light would
    /// meet the wall behind the eye from every wall in view, and the
    /// cavity would go black.
    #[test]
    fn a_headlamp_lights_the_inside_of_a_sponge() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.flame = sponge();
        // The sponge is [0, 1]^3 (its maps are `(p + (i, j, k)) / 3`) and
        // its cavity the middle third; the eye a tenth of the frame's
        // radius from the cavity's centre, inside it.
        c.escape.cam_target_x = "0.5".to_string();
        c.escape.cam_target_y = "0.5".to_string();
        c.escape.cam_target_z = "0.5".to_string();
        c.escape.zoom_log2 = 5.0;
        let l = &mut c.solid_shading.lights[0];
        l.enabled = true;
        l.kind = crate::config::LightKind::Headlamp;
        l.cone = 180.0;
        l.reach = 4.0;
        let mean = |img: &[[f32; 4]]| img.iter().map(|p| (p[0] + p[1] + p[2]) as f64).sum::<f64>() / img.len() as f64;
        c.escape.formula_params.insert("shadow".to_string(), 0.0);
        let open = mean(&lit_picture(&device, &queue, &c, &palette, w, h));
        c.escape.formula_params.insert("shadow".to_string(), 1.0);
        let shadowed = mean(&lit_picture(&device, &queue, &c, &palette, w, h));
        println!("  inside the sponge by a headlamp: shadows off {open:.4}, on {shadowed:.4}");
        assert!(open > 0.05, "the headlamp lit nothing: {open}");
        assert!(shadowed > 0.97 * open, "the headlamp's own shadows darken what it sees: {shadowed} against {open}");

        // And the lamp's cone: narrowed, the frame's edge goes dark while
        // its centre stays lit.
        c.solid_shading.lights[0].cone = 10.0;
        let spot = lit_picture(&device, &queue, &c, &palette, w, h);
        let at = |x: u32, y: u32| { let p = spot[(y * w + x) as usize]; p[0] + p[1] + p[2] };
        let centre = at(w / 2, h / 2);
        let corner = at(2, 2) + at(w - 3, 2) + at(2, h - 3) + at(w - 3, h - 3);
        println!("  a 10-degree cone: centre {centre:.4}, corners {corner:.4}");
        assert!(centre > 0.05 && corner == 0.0, "the cone: centre {centre}, corners {corner}");
    }

    /// A point light in the path tracer is the lit tier's (lights-in-the-
    /// fractal, L4): with no bounces, no shadows and a light of no size,
    /// the two light the cube alike -- to the path tracer's jitter across
    /// a pixel, over which the light's distance changes.
    #[test]
    fn a_point_lit_solid_is_the_lit_tiers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.escape.cam_target_x = "1".to_string();
        c.escape.cam_target_y = "1".to_string();
        c.escape.cam_target_z = "1".to_string();
        c.escape.formula_params.insert("shadow".to_string(), 0.0);
        c.escape.formula_params.insert("shadow_sharpness".to_string(), 1.0e4);
        c.escape.path.bounces = 0;
        let l = &mut c.solid_shading.lights[0];
        l.enabled = true;
        l.kind = crate::config::LightKind::Point;
        l.position = [3.5, 2.5, 4.0];
        l.reach = 3.0;
        l.intensity = 2.0;
        l.color = [1.0, 0.5, 0.25];
        let lit = lit_picture(&device, &queue, &c, &palette, w, h);
        c.escape.solid_tier = RenderTier::PathTraced;
        let mut r = renderer_for(&device, &c, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 16, || { wait(&device); true });
        let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        r.destroy();
        let at = |x: u32, y: u32| lit[(y * w + x) as usize];
        let (mut n, mut worst, mut sum) = (0, 0.0f32, 0.0f64);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let (p, q) = (at(x, y), path[(y * w + x) as usize]);
                // A face's inside, as the sunlit test takes it: covered,
                // lit, and its neighbours on the same face (the normal's).
                let flat = (0..9u32).all(|i| {
                    let o = at(x + i % 3 - 1, y + i / 3 - 1);
                    (0..3).all(|k| (p[k] - o[k]).abs() < 0.05 * p[k].max(1.0e-3))
                });
                if p[3] < 1.0 || q[3] < 1.0 || !flat || p[0] <= 1.0e-3 {
                    continue;
                }
                n += 1;
                for k in 0..3 {
                    let d = (p[k] - q[k]).abs() / p[k].max(1.0e-3);
                    worst = worst.max(d);
                    sum += d as f64;
                }
            }
        }
        let mean = sum / (3 * n.max(1)) as f64;
        println!("  point-lit cube: {n} face pixels, path against lit: mean {mean:.2e}, worst {worst:.2e}");
        assert!(n > 200, "only {n} face pixels");
        assert!(mean < 2.0e-3 && worst < 2.0e-2, "mean {mean}, worst {worst}");
    }
}
