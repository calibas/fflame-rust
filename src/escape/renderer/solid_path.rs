//! Mode D's path tracer (docs/projects/heightfield-3d.md, T3c): a
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

/// One dispatch: rows from `y0`, columns from `x0`, at `n` samples.
#[derive(Clone, Copy, Debug)]
struct Band {
    y0: u32,
    rows: u32,
    x0: u32,
    cols: u32,
    n: u32,
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
    /// Whether the picture shown is this one, not the walk's.
    shown: bool,
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
            shown: false,
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
        self.shown = false;
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
            let units: f32 = bands.iter().map(|b| (b.rows * b.n) as f32 * b.cols as f32 / w).sum();
            if generation != self.generation || !(units > 0.0) {
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
    /// True while there is more to do.
    pub fn trace_solid_viewport(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, palette_view: &TextureView) -> bool {
        let tier = config.escape.solid_tier;
        if tier == RenderTier::Lit {
            self.show_solid_path(false);
            return false;
        }
        let target = config.escape.path.samples.max(1);
        // One call on the GPU at a time (`SolidPath::busy`): until the
        // last is done, nothing new, and more to do.
        let busy = self.solid_path.as_ref().is_some_and(|p| p.busy());
        let more = busy || self.trace_solid(device, queue, config, palette_view, target, VIEWPORT_FRAME_MS);
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
        let key = format!("{:?}", path_core::path_settings(config, 1.0));
        if let Some(p) = self.solid_path.as_mut() {
            if p.settings.as_deref() != Some(key.as_str()) {
                if p.settings.is_some() {
                    p.restart();
                }
                p.settings = Some(key);
            }
        }
        if self.solid_path_samples() >= target {
            return false;
        }
        // What the walk has before its first band.
        self.ensure_lens(device, queue, escape);
        self.ensure_ifs_seeds(escape);
        self.upload_ifs(device, queue, true);
        self.upload_ifs_chain(device, queue);
        let key = self.ensure_solid_path_pipeline(device, escape, def);

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

        let ifs_group = self.ifs_bind_group(device);
        let path = self.solid_path.as_mut().expect("made above");
        // The denoiser's guides start with the sum; the pass gathers them
        // only where they fit (`set_guided`).
        if path.sum.set_guided(device, settings.denoise) {
            path.restart();
        }
        settings.denoise = path.sum.guided();
        path.collect();
        #[cfg(test)]
        let ceiling = path.ceiling.unwrap_or(ceiling);
        let mut traced: Vec<Band> = Vec::new();
        let mut first_submit: Option<web_time::Instant> = None;
        queue.write_buffer(&path.params, 0, bytemuck::bytes_of(&params));
        let mut spent = 0.0f32;
        while spent < frame_ms && traced.len() < 16 && path.sum.count() < target {
            let count = path.sum.count();
            let room = (frame_ms - spent).min(ceiling);
            // A whole frame at several samples, where the measurements say
            // they fit and the model's cap -- rows times samples -- agrees.
            let whole = (path.row == 0 && path.band_rows == 0)
                .then(|| path.frame_ms())
                .flatten()
                .filter(|&f| f <= room && h <= cap);
            let (b, ms) = match whole {
                Some(f) => {
                    let n = ((room / f).floor() as u32).min(cap / h).clamp(1, 64).min(target - count);
                    (Band { y0: 0, rows: h, x0: 0, cols: w, n }, f * n as f32)
                }
                None => {
                    if path.band_rows == 0 {
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
                    (Band { y0: path.row, rows, x0: path.col, cols, n: 1 }, band_ms * cols as f32 / w as f32)
                }
            };
            let uniform = PathParamsGpu::new(&settings, count, b.n, per_ray, param("shadow", 0.7), radius, (b.y0, b.rows))
                .from_column(b.x0)
                .buffer(device, queue);
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
                pass.dispatch_workgroups(b.cols.div_ceil(8), b.rows.div_ceil(8), 1);
            }
            first_submit.get_or_insert_with(web_time::Instant::now);
            queue.submit(std::iter::once(enc.finish()));
            traced.push(b);
            #[cfg(test)]
            path.dispatched.push((b.rows * b.n, cap, b.cols));
            spent += ms;
            if whole.is_some() {
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
        if path.sum.count() >= target {
            path.sum.resolve_now(device, queue, &path.output.1, w, h, split);
        } else {
            path.sum.resolve(device, queue, &path.output.1, w, h, split);
        }
        path.sum.count() < target
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
        let key = format!("ifs_path|{}|{}|b{beam}|{lens_id}|{sums}", def.name, coloring.name);
        if !self.pipelines.contains_key(&key) {
            let source = assembler::assemble_ifs_path(def, coloring, beam, lens_src.as_deref(), sums);
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
    fn ifs_bind_group(&self, device: &Device) -> BindGroup {
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
            r.trace_solid_viewport(&device, &queue, &c, &palette);
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
        while r.trace_solid_viewport(&device, &queue, &c, &palette) && guard < 400 {
            wait(&device);
            guard += 1;
        }
        wait(&device);
        assert_eq!(r.solid_path_samples(), 2, "finished");
        let generation = |r: &EscapeRenderer| r.solid_path.as_ref().map(|p| p.generation).unwrap();
        // Nothing changed: it stays finished.
        let g0 = generation(&r);
        assert!(!r.trace_solid_viewport(&device, &queue, &c, &palette));
        assert_eq!(generation(&r), g0);
        // The exposure: it starts over (and, this small, finishes again).
        c.exposure *= 2.0;
        r.trace_solid_viewport(&device, &queue, &c, &palette);
        wait(&device);
        assert_eq!(generation(&r), g0.wrapping_add(1), "started over");
        r.destroy();
    }

    /// No dispatch outgrows the model's band, whatever the measurements
    /// say: given all the time in the world, a 1080p solid still goes out
    /// in bands -- rows times samples -- no larger than the cap, before
    /// anything is measured and after. (Measurement alone once sent a
    /// whole frame at 64 samples and lost the device.)
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
        let worst = d.iter().map(|&(units, cap, _)| units as f32 / cap as f32).fold(0.0f32, f32::max);
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
        assert!(r.trace_solid_viewport(&device, &queue, &c, &palette));
        wait(&device);
        let p = r.solid_path.as_ref().expect("traced");
        assert!(!p.busy(), "done once the GPU is");
        let sent = p.dispatched.len();
        assert!(sent > 0);
        // A call still out: the next frame sends nothing, and has more to do.
        p.clock.lock().unwrap().outstanding = 1;
        assert!(r.trace_solid_viewport(&device, &queue, &c, &palette));
        assert_eq!(r.solid_path.as_ref().unwrap().dispatched.len(), sent, "sent while a call was out");
        r.solid_path.as_ref().unwrap().clock.lock().unwrap().outstanding = 0;
        assert!(r.trace_solid_viewport(&device, &queue, &c, &palette));
        assert!(r.solid_path.as_ref().unwrap().dispatched.len() > sent, "the next once it is in");
        wait(&device);
        r.destroy();
    }
}
