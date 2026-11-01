//! Unified rendering API
//!
//! This module provides a single entry point for rendering fractal flames to pixels.
//! All export paths (CLI, WASM, thumbnails, video) should use this API to ensure
//! consistent behavior and reduce code duplication.

use egui_wgpu::wgpu::{CommandEncoderDescriptor, Device, PollType, Queue, TextureFormat};

use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::renderer::effect_chain::EffectChainRunner;

/// Engine state a caller keeps ALIVE across renders.
///
/// A still is a complete answer on its own: build an engine, seed it,
/// run it, throw it away. A video is not -- frame n's picture
/// continues frame n-1's run, and rebuilding per frame would re-run
/// the simulation from the seed every time, which is quadratic in the
/// step count and which `simulation-fractals.md` D5 rejects outright.
///
/// So a looping caller owns this and hands it to each job. Without it
/// every path behaves exactly as it did: the still export, thumbnails
/// and the gallery pass nothing and get a fresh engine per render.
#[derive(Default)]
pub struct RenderEngines {
    /// The simulation's field and step counter. `render_sim` advances
    /// this to the frame's `sim.steps` rather than reseeding.
    #[cfg(feature = "engine-sim")]
    pub sim: Option<crate::sim::SimRenderer>,
    /// The escape renderer, kept for its allocations and its warm
    /// reference orbit; each frame still renders to settlement.
    #[cfg(feature = "engine-escape")]
    pub escape: Option<crate::escape::EscapeRenderer>,
    /// The escape terrain: its footprint is re-rendered only when the
    /// 2D picture changes, so a camera move between frames is a 3D
    /// render alone.
    #[cfg(feature = "terrain")]
    pub terrain: Option<crate::escape::footprint::EscapeTerrain>,
    /// A simulation's terrain, beside the simulation it is made from.
    #[cfg(all(feature = "terrain", feature = "engine-sim"))]
    pub sim_terrain: Option<crate::sim::terrain::SimTerrain>,
}

/// Configuration for a render job
pub struct RenderJob<'a> {
    /// Fractal configuration (transforms, colors, view settings)
    pub config: &'a FractalConfig,

    /// Output dimensions
    pub width: u32,
    pub height: u32,

    /// How many iterations to render (None = use config.max_iterations)
    pub target_iterations: Option<u64>,

    /// Iterations per GPU thread per dispatch
    pub iterations_per_thread: u32,

    /// Burn-in iterations (skipped before plotting)
    pub burn_in: u32,

    /// Transparent background (for PNG export)
    pub transparent: bool,

    /// Use premultiplied alpha for transparent export (vs the default
    /// straight-alpha reconstruction). Only meaningful when `transparent`.
    pub premultiplied: bool,

    /// Engine state to CONTINUE rather than rebuild (see
    /// `RenderEngines`). `None` -- the default, and what every still
    /// path passes -- means a fresh engine for this render.
    pub engines: Option<&'a mut RenderEngines>,

    /// Render pixels per output pixel, for the parameters measured in
    /// pixels (filter radii, effect radii, escape-time's relief): 1 -- the
    /// default -- for a render at its output's size, and an export's render
    /// scale for one rendered smaller and drawn up after
    /// (docs/archive/projects/quality-governor.md, *Exports*).
    pub pixel_scale: f32,

    /// The frame's motion-blur moments (docs/archive/projects/motion-blur.md):
    /// the config at each moment of the shutter, averaged before the
    /// tail. Empty -- the default -- renders `config` alone; `config` is
    /// the frame's own either way, and its tone mapping, effects and
    /// background are what the average is finished with.
    pub moments: &'a [FractalConfig],

    /// The renderer is `render`'s own and dies with this job, so its
    /// iteration buffers can go once the tone map has run. Set by
    /// `render`, never by a caller of `render_with`, whose renderer
    /// renders again.
    pub(crate) throwaway_renderer: bool,

    /// Restart every orbit every dispatch, as before orbits persisted
    /// (`docs/archive/projects/persistent-orbits.md`): a gate's baseline.
    #[cfg(test)]
    pub(crate) restarted_orbits: bool,
}

impl<'a> RenderJob<'a> {
    /// Create a render job from a FractalConfig with sensible defaults
    pub fn new(config: &'a FractalConfig, width: u32, height: u32) -> Self {
        Self {
            config,
            width,
            height,
            target_iterations: None, // Use config.max_iterations
            // Where orbits are reset every dispatch, its length is the
            // orbits': the app's default, so a render matches the view.
            iterations_per_thread: if config.resets_orbits() { crate::config::defaults::DEFAULT_ITERATIONS_PER_THREAD } else { 256 },
            burn_in: 20,
            transparent: false,
            premultiplied: false,
            engines: None,
            pixel_scale: 1.0,
            moments: &[],
            throwaway_renderer: false,
            #[cfg(test)]
            restarted_orbits: false,
        }
    }

    /// Set target iterations (overrides config.max_iterations)
    pub fn with_iterations(mut self, iterations: u64) -> Self {
        self.target_iterations = Some(iterations);
        self
    }

    /// Set iterations per thread
    pub fn with_iterations_per_thread(mut self, ipt: u32) -> Self {
        self.iterations_per_thread = ipt;
        self
    }

    /// Set burn-in iterations
    pub fn with_burn_in(mut self, burn_in: u32) -> Self {
        self.burn_in = burn_in;
        self
    }

    /// Set transparent mode
    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    /// Use premultiplied alpha (vs straight-alpha reconstruction) for transparent export
    pub fn with_premultiplied(mut self, premultiplied: bool) -> Self {
        self.premultiplied = premultiplied;
        self
    }

    /// Measure sizes in pixels at `s` render pixels an output pixel (an
    /// export at a render scale).
    pub fn with_pixel_scale(mut self, s: f32) -> Self {
        self.pixel_scale = if s.is_finite() && s > 0.0 { s } else { 1.0 };
        self
    }

    /// Blur the frame over these moments (`animation::export::moment_configs`).
    pub fn with_moments(mut self, moments: &'a [FractalConfig]) -> Self {
        self.moments = moments;
        self
    }

    /// Keep each thread's orbit between dispatches, or restart them every
    /// dispatch as a gate's baseline.
    #[cfg(test)]
    pub(crate) fn with_persistent_orbits(mut self, on: bool) -> Self {
        self.restarted_orbits = !on;
        self
    }

    /// Continue a caller-owned engine instead of building a fresh one.
    /// A video loop passes this; a still render does not.
    pub fn with_engines(mut self, engines: &'a mut RenderEngines) -> Self {
        self.engines = Some(engines);
        self
    }
}

/// Result of a completed render
pub struct RenderOutput {
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
    pub total_iterations: u64,
    pub render_time_ms: f64,
    /// The share of plot attempts that landed inside the frame, as
    /// auto exposure measured it (`FractalConfig::auto_exposure`).
    /// 1.0 when the feature is off, and always 1.0 for the two
    /// non-flame generators, which have no chaos game to miss with.
    pub frame_coverage: f32,
}

/// The names a config gives that this build has no engine entry for: an
/// escape formula, colouring, colouring layer or lens; a simulation's
/// model or colouring, its layers' and colour layers'. The engines fall
/// back to a default for these (a file from a newer build still opens),
/// which in the app comes with a warning -- and from the CLI was a
/// Mandelbrot saved under the unknown formula's name, exit code 0.
pub fn unknown_names(config: &crate::config::FractalConfig) -> Vec<String> {
    // A variation the flame carries the source of is known (a lens).
    #[cfg(feature = "engine-escape")]
    crate::variations::custom::ensure(&config.flame);
    #[allow(unused_mut)]
    let mut out = Vec::new();
    match config.render_mode {
        #[cfg(feature = "engine-escape")]
        crate::scene::transforms::RenderMode::Escape => {
            let e = &config.escape;
            // Sources the config carries name entries of their own: a
            // source that compiles is known; one that does not is not.
            crate::escape::custom::ensure(e);
            let a = crate::escape::find_formula(&e.formula).is_some();
            let b = crate::escape::fields::get_field(&e.formula).is_some();
            let d = crate::escape::ifs::get_ifs(&e.formula).is_some();
            if !(a || b || d) {
                out.push(format!("escape formula `{}`", e.formula));
            }
            // A colouring from another formula family is the state right
            // after a switch, and its default draws; one in no registry
            // is unknown.
            let colouring = |name: &str| {
                crate::escape::find_coloring(name).is_some()
                    || crate::escape::fields::FIELD_COLORINGS.iter().any(|c| c.name == name)
                    || crate::escape::ifs::IFS_COLORINGS.iter().any(|c| c.name == name)
            };
            if !colouring(&e.coloring) {
                out.push(format!("escape colouring `{}`", e.coloring));
            }
            if !e.layer.coloring.is_empty() && !colouring(&e.layer.coloring) {
                out.push(format!("escape colouring layer `{}`", e.layer.coloring));
            }
            if !e.lens.is_empty() && crate::variations::global_registry().get(&e.lens).is_none() {
                out.push(format!("escape lens `{}`", e.lens));
            }
        }
        #[cfg(feature = "engine-sim")]
        crate::scene::transforms::RenderMode::Simulation => {
            let s = &config.sim;
            let model = |name: &str| crate::sim::MODELS.iter().any(|m| m.name == name);
            let colouring = |name: &str| crate::sim::COLORINGS.iter().any(|c| c.name == name);
            for l in 0..s.layer_count() {
                let name = s.layer_model_name(l);
                if !model(name) {
                    out.push(format!("simulation model `{name}`"));
                }
            }
            if s.color_layers.is_empty() {
                if !colouring(&s.coloring) {
                    out.push(format!("simulation colouring `{}`", s.coloring));
                }
            } else {
                for l in &s.color_layers {
                    if !colouring(&l.coloring) {
                        out.push(format!("simulation colouring `{}`", l.coloring));
                    }
                }
            }
        }
        _ => {}
    }
    out.dedup();
    out
}

/// Progress callback for long-running renders
pub trait RenderProgress {
    /// Called periodically with current/total iterations
    fn on_progress(&mut self, current: u64, total: u64);

    /// Return true to cancel the render
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Simple no-op progress (for when caller doesn't need progress updates)
pub struct NoProgress;

impl RenderProgress for NoProgress {
    fn on_progress(&mut self, _current: u64, _total: u64) {}
}

/// Render error types
#[derive(Debug)]
pub enum RenderError {
    NoPaletteFound,
    PixelReadFailed(String),
    Cancelled,
    /// The config asks for an engine this build does not carry.
    ///
    /// A module without the escape engine still PARSES an escape
    /// config -- the mode round-trips, so a file is never silently
    /// rewritten -- and reports this rather than rendering a flame the
    /// file never described.
    EngineMissing(&'static str),
    /// A GPU allocation failed part-way through.
    ///
    /// Worth its own variant because the alternative is what it used
    /// to do: wgpu reports the failure through the uncaptured-error
    /// handler, the render carries on against an invalid buffer, and
    /// every dispatch quietly does nothing -- so the export SUCCEEDS
    /// and writes an all-black PNG. A render that could not allocate
    /// what it needed has to say so.
    OutOfMemory(String),
    /// A size this device's limits refuse before anything is allocated
    /// (a texture side, a binding): the reason says which. Not memory
    /// running out, and not always the export's size -- a fixed
    /// simulation grid is the grid's.
    TooLarge(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::NoPaletteFound => write!(f, "No palette found"),
            RenderError::PixelReadFailed(msg) => write!(f, "Failed to read pixels: {}", msg),
            RenderError::Cancelled => write!(f, "Render cancelled"),
            RenderError::EngineMissing(what) => {
                write!(f, "this build has no {what} engine")
            }
            RenderError::OutOfMemory(what) => write!(
                f,
                "the GPU ran out of memory ({what}); try a smaller size or less antialiasing"
            ),
            RenderError::TooLarge(why) => write!(f, "too large for this GPU: {why}"),
        }
    }
}

impl std::error::Error for RenderError {}

// Constants for render loop
const NUM_WORKGROUPS: u32 = 128;
const BATCH_SIZE: u32 = 4;

/// Unified rendering function
///
/// Renders a fractal flame to RGBA pixels. The caller provides the GPU device and queue,
/// keeping platform-specific GPU setup separate from rendering logic.
///
/// # Arguments
/// * `device` - GPU device (from app context or headless creation)
/// * `queue` - GPU command queue
/// * `job` - Render job configuration
/// * `progress` - Optional progress callback
///
/// # Example
/// ```ignore
/// let job = RenderJob::new(&config, 1920, 1080)
///     .with_iterations(10_000_000)
///     .with_transparent(true);
/// let output = render(&device, &queue, job, &mut NoProgress).await?;
/// ```
pub async fn render(
    device: &Device,
    queue: &Queue,
    mut job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
) -> Result<RenderOutput, RenderError> {
    // A flame the transform buffers cannot hold would panic in packing.
    if let Some(why) = job.config.flame.gpu_capacity_error() {
        return Err(RenderError::TooLarge(format!("this flame cannot be rendered: {why}")));
    }
    // Refuse a size this device cannot hold BEFORE allocating any of
    // it -- including the flame renderer's own textures, which are
    // built below whichever engine ends up rendering. The limits are
    // knowable up front; the alternative is a rejected allocation
    // that stops nothing and a black image.
    #[cfg(feature = "terrain")]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Escape && job.config.escape.terrain_active() {
        if let Some(why) =
            crate::escape::footprint::EscapeTerrain::allocation_error(device, &job.config.escape, job.width, job.height)
        {
            return Err(RenderError::TooLarge(why));
        }
    }
    #[cfg(feature = "engine-escape")]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Escape && !job.config.escape.terrain_active() {
        if let Some(why) = crate::escape::EscapeRenderer::allocation_error(
            device,
            &job.config.escape,
            job.width,
            job.height,
            // The factor the renderer will actually use: checking the
            // REQUESTED one refused renders it would have clamped and
            // then made up by accumulation.
            crate::escape::EscapeRenderer::affordable_supersample(
                device,
                job.width,
                job.height,
                job.config.escape.supersample,
            ),
        ) {
            return Err(RenderError::TooLarge(why));
        }
    }

    #[cfg(feature = "engine-sim")]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Simulation {
        if let Some(why) = crate::sim::SimRenderer::allocation_error(
            device,
            &job.config.sim,
            job.width,
            job.height,
        ) {
            return Err(RenderError::TooLarge(why));
        }
    }

    // Create renderer with config's palette size. An escape picture, a
    // terrain and a simulation use only its tail -- the tonemap of their
    // own image -- so they get one without the flame's per-pixel state:
    // 64 bytes a pixel never read, 2.1 GB of an 8K export (measured: a
    // terrain export at 8K ran out of memory on a 6 GB card).
    let surface_format = TextureFormat::Rgba8Unorm;
    // Caught here, not by an engine's own scope: wgpu reports a failed
    // allocation as an error that stops nothing, and the renderer is made
    // before any engine's scope is pushed.
    let alloc = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let mut renderer = FlameRenderer::for_job(device, queue, surface_format, job.width, job.height, job.config);
    if let Some(err) = alloc.pop().await {
        renderer.destroy();
        return Err(RenderError::OutOfMemory(err.to_string()));
    }
    // One-shot path: the sticky superset canonically reorders the local
    // index map, which is an ULP-class trajectory change — meaningless
    // for a persistent renderer, but exports and the visual suite must
    // render exactly the specialized shaders their baselines and
    // reproducibility contracts were made with. A throwaway renderer
    // gains nothing from stickiness anyway.
    renderer.set_sticky_enabled(false);
    job.throwaway_renderer = true;

    let out = render_with(&mut renderer, device, queue, job, progress).await;

    // On WebGPU, dropping `renderer` frees nothing: wgpu's `Drop` for
    // `WebBuffer`/`WebTexture` is a no-op, so the GPU memory lives until
    // the JS GC collects the wrappers. Callers that own their renderer
    // (see `render_with`) handle this themselves; a throwaway one has to
    // be swept here or repeated renders exhaust the device.
    //
    // Idempotent and safe after the pixels are on the CPU, which they are
    // by the time `render_with` returns — including on its error paths,
    // where nothing was read but nothing is owed either.
    renderer.destroy();

    out
}

/// Render into a **caller-owned** renderer, reusing it across calls.
///
/// `render` creates a renderer per call, which is right for a one-shot
/// export and wrong for anything rendering repeatedly on WebGPU: buffer
/// and texture `Drop` are no-ops there, so every discarded renderer's
/// memory stays allocated until the JS garbage collector runs. A caller
/// looping over tiles either sweeps explicitly or reuses — and reuse is
/// strictly better, because it also keeps the shader cache warm across
/// configs that share a variation set.
///
/// `load_config` is a full reset point (transforms, variation params,
/// palette *size*, accumulation, solid state), so the renderer does not
/// need to have been built for this config. It does need to be the right
/// SIZE: call `resize` first if the dimensions changed, or the render
/// silently uses the old ones.
pub async fn render_with(
    renderer: &mut FlameRenderer,
    device: &Device,
    queue: &Queue,
    job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
) -> Result<RenderOutput, RenderError> {
    // Callers with their own renderer (video, the gallery) come in here.
    if let Some(why) = job.config.flame.gpu_capacity_error() {
        return Err(RenderError::TooLarge(format!("this flame cannot be rendered: {why}")));
    }
    let start_time = web_time::Instant::now();
    // Sizes in pixels, for every engine's tail (the effect chains read it
    // off the renderer) and the flame's own passes.
    renderer.set_pixel_scale(job.pixel_scale);
    // The variations the flame carries the source of, registered for
    // every engine: a flame's, an escape lens, a simulation's warp.
    #[cfg(feature = "engine-escape")]
    crate::variations::custom::ensure(&job.config.flame);

    // Escape-time mode is a different generator with the same tail:
    // one compute pass instead of the chaos-game loop, then the shared
    // density-effects → tonemap → color-effects → readback pipeline.
    // Dispatching here is what gives thumbnails, CLI export, video and
    // the gallery escape rendering for free (plan: integration map).
    #[cfg(feature = "engine-escape")]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Escape {
        return render_escape(renderer, device, queue, job, progress, start_time).await;
    }
    #[cfg(not(feature = "engine-escape"))]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Escape {
        return Err(RenderError::EngineMissing("escape-time"));
    }

    // Simulation mode: a third generator on the same tail. Dispatching
    // here is what gives thumbnails, CLI export, video and the gallery
    // simulation rendering for free, exactly as it did for escape.
    #[cfg(feature = "engine-sim")]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Simulation {
        return render_sim(renderer, device, queue, job, progress, start_time).await;
    }
    #[cfg(not(feature = "engine-sim"))]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Simulation {
        return Err(RenderError::EngineMissing("simulation"));
    }

    let target = job.target_iterations.unwrap_or(job.config.max_iterations);

    log::info!(
        "Render: Starting {}x{}, target={} iterations",
        job.width,
        job.height,
        target
    );

    // Get palette directly from config (palette is always present)
    let palette = &job.config.palette;

    // Load config into renderer
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Render Config Encoder"),
    });

    log::info!(
        "Render: Loading config '{}' with {} transform(s)",
        job.config.flame.name,
        job.config.flame.transforms.len()
    );

    #[cfg(test)]
    renderer.set_persistent_orbits(!job.restarted_orbits);
    renderer.load_config(
        device,
        &mut encoder,
        queue,
        job.config,
        palette,
        job.iterations_per_thread,
        job.burn_in,
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Shadow-fit warmup: the shadow-map fit needs the flame's real
    // bounds, which only the chaos game can reveal. Run a few batches,
    // read the measured AABB back, re-freeze the fit, and restart
    // accumulation — cheap relative to the full render, and without it a
    // zoom-guessed fit wastes map resolution (or clips the attractor).
    if renderer.shadow_capture_wanted() {
        let warmup_batches = 6u32;
        for _ in 0..warmup_batches {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Shadow Warmup"),
            });
            renderer.compute_pass(
                &mut enc, queue, device, NUM_WORKGROUPS,
                job.iterations_per_thread, job.burn_in,
                job.config.zoom, job.config.pan_x as f32, job.config.pan_y as f32, job.config.rotation,
                job.config.camera_rotation_x, job.config.camera_rotation_y, job.config.camera_bank,
                job.config.camera_x, job.config.camera_y, job.config.camera_z,
                job.config.speed_factor, true, false,
            );
            queue.submit(std::iter::once(enc.finish()));
        }
        let changed = renderer.refresh_shadow_placement(
            device, queue,
            job.config.zoom, job.config.pan_x as f32, job.config.pan_y as f32,
            job.config.camera_rotation_x, job.config.camera_rotation_y, job.config.camera_bank,
            [job.config.camera_x, job.config.camera_y, job.config.camera_z],
        ).await;
        log::info!("Shadow warmup: fit {}", if changed { "refit to measured bounds" } else { "unchanged" });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Shadow Warmup Reset"),
        });
        renderer.reset(
            &mut enc, queue, job.iterations_per_thread,
            job.config.zoom, job.config.pan_x as f32, job.config.pan_y as f32, job.config.rotation,
            job.config.camera_rotation_x, job.config.camera_rotation_y, job.config.camera_bank,
            job.config.camera_x, job.config.camera_y, job.config.camera_z,
            job.config.speed_factor,
        );
        queue.submit(std::iter::once(enc.finish()));
    }

    // Render loop. `total_rendered` counts PLOTTED samples, as the app
    // does: it is what stops the loop and what the output reports. It
    // once counted dispatched iterations, burn-in included, so an export
    // stopped short of the app's count for the same `max_iterations`
    // (by a burn-in, or ~2% on flames whose orbits refresh) and its PNG
    // metadata over-reported.
    let mut total_rendered = 0u64;
    // Dispatches in a row that plotted nothing. Restarted orbits whose
    // burn-in is no shorter than a dispatch plot nothing, ever; without
    // this the loop would never end. Persistent orbits plot once their
    // burn-in, which may span dispatches, is paid: allow that and a margin.
    let mut empty_dispatches = 0u32;
    let stall_limit = job.burn_in / job.iterations_per_thread.max(1) + 4;
    let mut batch_frame_count = 0u32;
    // The samples the batch plotted, which the tone map normalises by:
    // what the renderer says plotted, as the app counts -- a thread's
    // burn-in is paid once, not every dispatch.
    let mut batch_samples = 0u64;
    // Motion blur (docs/archive/projects/motion-blur.md): the dispatches go round
    // the frame's moments, each drawing its moment's flame and view into
    // the one accumulation, and the loop ends on a whole round so every
    // moment has the same share. The orbits go on from moment to moment.
    // Not under Focused Rendering, whose plans are one view's.
    let moments: &[FractalConfig] = if job.config.cylinder_targeting { &[] } else { job.moments };
    let mut dispatches = 0usize;

    while total_rendered < target || (!moments.is_empty() && dispatches % moments.len() != 0) {
        // Check for cancellation
        if progress.is_cancelled() {
            renderer.set_continuing_frame(false);
            return Err(RenderError::Cancelled);
        }

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Frame"),
        });

        let clear_histogram = batch_frame_count == 0;
        let clear_paths = total_rendered == 0 && clear_histogram;

        let view = if moments.is_empty() {
            job.config
        } else {
            let moment = &moments[dispatches % moments.len()];
            renderer.set_moment(device, queue, moment, job.iterations_per_thread, job.burn_in);
            renderer.set_continuing_frame(dispatches > 0);
            moment
        };
        dispatches += 1;
        let plotted = renderer.compute_pass(
            &mut encoder,
            queue,
            device,
            NUM_WORKGROUPS,
            job.iterations_per_thread,
            job.burn_in,
            view.zoom,
            view.pan_x as f32,
            view.pan_y as f32,
            view.rotation,
            view.camera_rotation_x,
            view.camera_rotation_y,
            view.camera_bank,
            view.camera_x,
            view.camera_y,
            view.camera_z,
            view.speed_factor,
            clear_histogram,
            clear_paths,
        );

        total_rendered += plotted;
        batch_samples += plotted;
        batch_frame_count += 1;
        empty_dispatches = if plotted == 0 { empty_dispatches + 1 } else { 0 };
        let stalled = empty_dispatches >= stall_limit;
        if stalled {
            log::warn!(
                "Render: {stall_limit} dispatches in a row plotted nothing \
                 ({} iterations a thread, burn-in {}); stopping at {total_rendered}",
                job.iterations_per_thread,
                job.burn_in,
            );
        }

        // Accumulate when batch is complete
        if batch_frame_count >= BATCH_SIZE {
            renderer.accumulate_pass(&mut encoder, queue, device, batch_samples);
            batch_frame_count = 0;
            batch_samples = 0;
        }

        queue.submit(std::iter::once(encoder.finish()));

        // Report progress
        progress.on_progress(total_rendered, target);

        // Check if we've reached target (on a whole round of the moments)
        let round_done = moments.is_empty() || dispatches % moments.len() == 0;
        if (total_rendered >= target && round_done) || stalled {
            // Final accumulation for partial batch
            if batch_frame_count > 0 {
                let mut final_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Final Batch Accumulation"),
                });
                renderer.accumulate_pass(&mut final_encoder, queue, device, batch_samples);
                queue.submit(std::iter::once(final_encoder.finish()));
            }
            break;
        }
    }
    renderer.set_continuing_frame(false);
    if !moments.is_empty() {
        log::info!("Render: motion blur over {} moments, {dispatches} dispatches", moments.len());
    }

    log::info!("Render: Render loop complete, total_rendered={}", total_rendered);

    // Ensure all GPU work completes before post-processing
    // This is critical for larger resolutions where work takes longer
    let _ = device.poll(PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    // Set transparent mode if requested
    if job.transparent {
        renderer.set_transparent_mode(queue, true, job.premultiplied, job.config, job.iterations_per_thread);
    }

    // Check for enabled effects
    let has_density_effects = EffectChainRunner::has_enabled_effects(&job.config.density_effects);
    let has_color_effects = EffectChainRunner::has_enabled_effects(&job.config.color_effects);

    // The effect chain only exists when an effect is actually enabled,
    // and it is destroyed after the pixel read below. It used to be
    // constructed unconditionally per render and never destroyed — on
    // WebGPU that leaked its params buffer every render (drop frees
    // nothing there; see the gallery module's docs), measured as one of
    // the two per-render buffers growing without bound in the client.
    let mut effect_chain = if has_density_effects || has_color_effects {
        Some(EffectChainRunner::new(device, job.width, job.height))
    } else {
        None
    };

    log::info!(
        "Render: Effects - density: {} enabled, color: {} enabled",
        job.config.density_effects.iter().filter(|e| e.enabled).count(),
        job.config.color_effects.iter().filter(|e| e.enabled).count()
    );

    // Run density effects (before tonemap, on HDR accumulation data)
    let mut tonemap_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Final Tonemap"),
    });

    // Reset effect slot counter (allows multiple effects with unique params in same submit)
    if let Some(chain) = effect_chain.as_mut() {
        chain.reset_slots();
        chain.set_pixel_scale(renderer.pixel_scale());
    }

    // Solid brightness renormalization: exact accepted-density measurement
    // before the single final tonemap.
    renderer.apply_exact_density_fraction(device, queue).await;

    // Solid-rendering shade pass — same ordering as the interactive frame:
    // shade, then density effects, then tonemap.
    let shade_ran = renderer.run_shade_pass(
        device,
        queue,
        &mut tonemap_encoder,
        job.config.zoom,
        job.config.rotation,
        job.config.pan_x as f32,
        job.config.pan_y as f32,
        job.config.camera_rotation_x,
        job.config.camera_rotation_y,
        job.config.camera_bank,
        job.config.camera_x,
        job.config.camera_y,
        job.config.camera_z,
    );
    // Post-process DoF (solid mode) between shade and density
    // effects/tonemap — same ordering as the interactive frame.
    let dof_ran = renderer.run_dof_pass(
        device,
        queue,
        &mut tonemap_encoder,
        shade_ran,
        job.config.zoom,
    );
    let pre_tonemap_view = if dof_ran {
        renderer.dof_output_view()
    } else if shade_ran {
        renderer.shade_output_view()
    } else {
        renderer.get_accumulation_view()
    };

    if has_density_effects {
        let chain = effect_chain.as_mut().expect("built above: has_density_effects");
        let density_ran = chain.run_density_effects(
            device,
            queue,
            &mut tonemap_encoder,
            pre_tonemap_view,
            &job.config.density_effects,
            renderer.mean_density(),
        );

        if density_ran {
            if let Some(density_output) = chain.get_density_output() {
                renderer.tonemap_pass_with_input(device, queue, &mut tonemap_encoder, density_output);
            } else if dof_ran || shade_ran {
                renderer.tonemap_pass_with_input(device, queue, &mut tonemap_encoder, pre_tonemap_view);
            } else {
                renderer.tonemap_pass(queue, &mut tonemap_encoder);
            }
        } else if dof_ran || shade_ran {
            renderer.tonemap_pass_with_input(device, queue, &mut tonemap_encoder, pre_tonemap_view);
        } else {
            renderer.tonemap_pass(queue, &mut tonemap_encoder);
        }
    } else if dof_ran || shade_ran {
        renderer.tonemap_pass_with_input(device, queue, &mut tonemap_encoder, pre_tonemap_view);
    } else {
        renderer.tonemap_pass(queue, &mut tonemap_encoder);
    }

    queue.submit(std::iter::once(tonemap_encoder.finish()));

    // A throwaway renderer's iteration buffers -- several gigabytes at 8K
    // -- are dead once the tone map has run. Free them before the colour
    // chain allocates its full-size ping-pong: beside them, a browser ran
    // out of GPU memory and the 8K export came back black. A destroyed
    // buffer is freed only when the GPU is done with it, so wait for the
    // tone map first.
    if has_color_effects && job.throwaway_renderer {
        let (tx, rx) = futures::channel::oneshot::channel();
        queue.on_submitted_work_done(move || {
            let _ = tx.send(());
        });
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let _ = rx.await;
        renderer.free_iteration_buffers();
    }

    // Run color effects (after tonemap)
    let color_effects_ran = if has_color_effects {
        let chain = effect_chain.as_mut().expect("built above: has_color_effects");
        let mut color_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Color Effects"),
        });

        let ran = chain.run_color_effects(
            device,
            queue,
            &mut color_encoder,
            renderer.get_fractal_texture_view(),
            &job.config.color_effects,
        );

        queue.submit(std::iter::once(color_encoder.finish()));
        ran
    } else {
        false
    };

    // Read pixels from appropriate source
    let pixels = if color_effects_ran {
        // Read from color effect output
        effect_chain
            .as_ref()
            .expect("color_effects_ran implies a chain")
            .read_color_output_pixels(device, queue)
            .await
            .map_err(RenderError::PixelReadFailed)
    } else {
        // Read from renderer's fractal texture (persistent staging,
        // reused across renders)
        renderer
            .read_fractal_pixels(device, queue, job.transparent, job.config.background_color)
            .await
            .map_err(|e| RenderError::PixelReadFailed(e.to_string()))
    };

    // The completed readback proves every submission that used the
    // chain has finished, so this is the safe point to destroy it —
    // on the error path too, where nothing was read but nothing is
    // owed either.
    if let Some(chain) = &effect_chain {
        chain.destroy();
    }
    let (width, height, rgba_data) = pixels?;

    let render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    log::info!(
        "Render: Complete - {}x{}, {} iterations in {:.1}ms",
        width,
        height,
        total_rendered,
        render_time_ms
    );

    Ok(RenderOutput {
        width,
        height,
        rgba_data,
        total_iterations: total_rendered,
        render_time_ms,
        frame_coverage: renderer.frame_coverage_fraction(),
    })
}

/// Render one simulation still: seed, run exactly `sim.steps`, colour.
///
/// The step count is the contract (master plan D5), so this does not
/// settle or converge — it runs the number the config asks for and
/// stops. That is what makes a still of a model that never settles
/// reproducible.
///
/// Shares escape's tail verbatim: density effects → tonemap → colour
/// effects → readback, all fed from an `Rgba32Float` image in the flame
/// accumulator's layout.
#[cfg(feature = "engine-sim")]
async fn render_sim(
    renderer: &mut FlameRenderer,
    device: &Device,
    queue: &Queue,
    mut job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
    start_time: web_time::Instant,
) -> Result<RenderOutput, RenderError> {
    let (grid_w, grid_h) = crate::sim::SimRenderer::grid_for(&job.config.sim, job.width, job.height);
    log::info!(
        "Render: simulation {}x{} output, {}x{} grid, model {:?}, coloring {:?}, {} steps",
        job.width,
        job.height,
        grid_w,
        grid_h,
        job.config.sim.model,
        job.config.sim.coloring,
        job.config.sim.steps
    );
    progress.on_progress(0, 1);

    // Full config load, for the same reason escape does it: it is the
    // one call guaranteed to keep every tail input (palette texture,
    // tonemap uniform, curve LUT, background, levels) in exact sync
    // with the flame path.
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Sim Config Encoder"),
    });
    renderer.load_config(
        device,
        &mut encoder,
        queue,
        job.config,
        &job.config.palette,
        job.iterations_per_thread,
        job.burn_in,
    );
    queue.submit(std::iter::once(encoder.finish()));

    if job.transparent {
        renderer.set_transparent_mode(
            queue,
            true,
            job.premultiplied,
            job.config,
            job.iterations_per_thread,
        );
    }

    let oom_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);

    // Continue a caller-owned run when there is one, else build a
    // throwaway. A still export, a thumbnail and the gallery pass no
    // engines and get exactly what they always did: a fresh field,
    // seeded, run to `steps`. A video loop passes its own, and frame n
    // continues frame n-1's run instead of re-running from the seed
    // (`simulation-fractals.md` D5).
    // Whose renderer this is decides whether the teardown may free it
    // (as in `render_escape`).
    let caller_owned = job.engines.is_some();
    let mut owned: Option<crate::sim::SimRenderer> = None;
    let make = || crate::sim::SimRenderer::new(device, &job.config.sim, job.width, job.height);
    // A simulation's terrain (heightfield plan T4) lives beside it, so a
    // video's frames keep its renderer as they keep the run.
    #[cfg(feature = "terrain")]
    let mut terrain_owned: Option<crate::sim::terrain::SimTerrain> = None;
    #[cfg(feature = "terrain")]
    let mut terrain_slot: Option<&mut Option<crate::sim::terrain::SimTerrain>> = None;
    let sim: &mut crate::sim::SimRenderer = match job.engines {
        Some(ref mut engines) => {
            let engines = &mut **engines;
            #[cfg(feature = "terrain")]
            {
                terrain_slot = Some(&mut engines.sim_terrain);
            }
            engines.sim.get_or_insert_with(make)
        }
        None => owned.insert(make()),
    };
    // A persistent renderer may have been built for another size; a
    // fresh one is already right and this is a no-op.
    sim.resize(device, &job.config.sim, job.width, job.height);
    if job.config.sim.use_transforms {
        sim.set_layer_transforms(device, queue, &job.config.flame);
    }
    // `advance_steps` owns the seed/step decision (`sim::plan_steps`):
    // forward from where the field is, restart to go back, and a fresh
    // renderer is at 0 so it seeds and runs the lot. Batched here for
    // PROGRESS only -- run_steps already submits in watchdog-sized
    // pieces internally, so the driver never sees an unbounded pass
    // however large `steps` is; polling between batches is also what
    // keeps the queue from growing without bound.
    const PROGRESS_BATCH: u32 = 512;
    // A terrain's still follows the steps, and takes most of a path
    // traced export: the bar's share of each, in thousandths.
    #[cfg(feature = "terrain")]
    let still_share = match (job.config.sim.terrain_active(), job.config.sim.terrain.tier) {
        (false, _) => 0.0,
        (true, crate::config::escape::RenderTier::Lit) => 0.1,
        (true, _) => 0.75,
    };
    #[cfg(not(feature = "terrain"))]
    let still_share = 0.0;
    // Motion blur (docs/archive/projects/motion-blur.md): the field at each
    // moment's step count, in turn, coloured as that moment colours it,
    // averaged. A moment behind the field reseeds and runs, exactly as a
    // backward frame does.
    #[cfg(feature = "terrain")]
    let terrain_on = job.config.sim.terrain_active();
    #[cfg(not(feature = "terrain"))]
    let terrain_on = false;
    // A terrain whose moments share its ground -- they differ in the
    // terrain's own settings alone, its camera, its relief -- is drawn
    // once, its passes or samples going round their views. One whose
    // field differs is drawn at each moment, each its share of the
    // samples, and the stills averaged.
    let shared_ground = job.moments.iter().all(|m| {
        let mut sim_cfg = m.sim.clone();
        sim_cfg.terrain = job.config.sim.terrain.clone();
        sim_cfg == job.config.sim
    });
    #[cfg(feature = "terrain")]
    let terrain_moments = terrain_on && !job.moments.is_empty() && !shared_ground;
    let moments: Vec<&FractalConfig> = if job.moments.is_empty() || (terrain_on && shared_ground) {
        vec![job.config]
    } else {
        job.moments.iter().collect()
    };
    let blurred = moments.len() > 1;
    if blurred && !terrain_on {
        sim.begin_moments(device, queue, moments.len() as u32);
    }
    // The terrain: its ground made from the field, then the still --
    // path traced at its samples unless its tier is Lit.
    #[cfg(feature = "terrain")]
    let mut terrain: Option<&mut crate::sim::terrain::SimTerrain> = if terrain_on {
        // The still sizes the renderer itself: to the frame, or to its
        // tiles.
        let (tile_w, tile_h) = crate::escape::terrain_tiers::still_tile(job.width, job.height);
        let make = || crate::sim::terrain::SimTerrain::new(device, tile_w, tile_h);
        let t = match terrain_slot {
            Some(slot) => slot.get_or_insert_with(make),
            None => terrain_owned.insert(make()),
        };
        if terrain_moments {
            t.begin_moments(device, queue, job.width, job.height, moments.len() as u32);
        }
        Some(t)
    } else {
        None
    };
    for (k, moment) in moments.iter().enumerate() {
        let span = |f: f64| ((k as f64 + f) / moments.len() as f64 * 1000.0) as u64;
        if blurred && moment.sim.use_transforms {
            sim.set_layer_transforms(device, queue, &moment.flame);
        }
        let total = moment.sim.steps;
        loop {
            let reached =
                sim.advance_steps(device, queue, &moment.sim, total, Some(PROGRESS_BATCH));
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let steps = sim.step_index() as f64 / total.max(1) as f64;
            progress.on_progress(span((1.0 - still_share) * steps.min(1.0)), 1000);
            if reached {
                break;
            }
            if progress.is_cancelled() {
                if !caller_owned {
                    sim.destroy();
                }
                return Err(RenderError::Cancelled);
            }
        }
        sim.color(device, queue, &moment.sim, renderer.palette_view());
        if blurred && !terrain_on {
            sim.add_moment(device, queue);
        }
        #[cfg(feature = "terrain")]
        if let Some(t) = terrain.as_deref_mut() {
            // A moment on its own ground takes its share of the samples;
            // a shared ground's still goes round the moments' views.
            let share;
            let (still, views): (&FractalConfig, &[FractalConfig]) = if terrain_moments {
                let mut c = (*moment).clone();
                let path = &mut c.sim.terrain.path;
                path.samples = path.samples.div_ceil(moments.len() as u32).max(1);
                share = c;
                (&share, &[])
            } else {
                (moment, job.moments)
            };
            t.update(device, queue, sim, moment, renderer.palette_view());
            t.render_still_moments(device, queue, still, views, (job.width, job.height), |f| {
                let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                progress.on_progress(span((1.0 - still_share) + still_share * f as f64), 1000);
                !progress.is_cancelled()
            });
            if progress.is_cancelled() {
                if let Some(t) = terrain_owned.as_ref() {
                    t.destroy();
                }
                if !caller_owned {
                    sim.destroy();
                }
                return Err(RenderError::Cancelled);
            }
            if terrain_moments {
                t.add_moment(device, queue);
            }
        }
    }

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Sim Tail"),
    });

    // Shared tail: density effects → tonemap → color effects → read.
    let has_density_effects = EffectChainRunner::has_enabled_effects(&job.config.density_effects);
    let has_color_effects = EffectChainRunner::has_enabled_effects(&job.config.color_effects);
    let mut effect_chain = if has_density_effects || has_color_effects {
        Some(EffectChainRunner::new(device, job.width, job.height))
    } else {
        None
    };
    if let Some(chain) = effect_chain.as_mut() {
        chain.reset_slots();
        chain.set_pixel_scale(renderer.pixel_scale());
    }

    let picture = match sim.moments_view() {
        Some(v) if blurred => v,
        _ => sim.output_view(),
    };
    #[cfg(feature = "terrain")]
    let sim_view = match terrain.as_ref() {
        Some(t) if terrain_moments => t.moments_view().unwrap_or(t.output_view()),
        Some(t) => t.output_view(),
        None => picture,
    };
    #[cfg(not(feature = "terrain"))]
    let sim_view = picture;
    if has_density_effects {
        let chain = effect_chain.as_mut().expect("built above: has_density_effects");
        let density_ran = chain.run_density_effects(
            device,
            queue,
            &mut encoder,
            sim_view,
            &job.config.density_effects,
            1.0, // a simulation's alpha is not a hit count
        );
        match (density_ran, chain.get_density_output()) {
            (true, Some(density_output)) => {
                renderer.tonemap_pass_with_input(device, queue, &mut encoder, density_output)
            }
            _ => renderer.tonemap_pass_with_input(device, queue, &mut encoder, sim_view),
        }
    } else {
        renderer.tonemap_pass_with_input(device, queue, &mut encoder, sim_view);
    }
    queue.submit(std::iter::once(encoder.finish()));

    let color_effects_ran = if has_color_effects {
        let chain = effect_chain.as_mut().expect("built above: has_color_effects");
        let mut color_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Sim Color Effects"),
        });
        let ran = chain.run_color_effects(
            device,
            queue,
            &mut color_encoder,
            renderer.get_fractal_texture_view(),
            &job.config.color_effects,
        );
        queue.submit(std::iter::once(color_encoder.finish()));
        ran
    } else {
        false
    };

    if let Some(err) = oom_scope.pop().await {
        if let Some(chain) = &effect_chain {
            chain.destroy();
        }
        #[cfg(feature = "terrain")]
        if let Some(t) = terrain_owned.as_ref() {
            t.destroy();
        }
        if let Some(s) = owned.as_ref() {
            s.destroy();
        }
        return Err(RenderError::OutOfMemory(err.to_string()));
    }

    let pixels = if color_effects_ran {
        effect_chain
            .as_ref()
            .expect("color_effects_ran implies a chain")
            .read_color_output_pixels(device, queue)
            .await
            .map_err(RenderError::PixelReadFailed)
    } else {
        renderer
            .read_fractal_pixels(device, queue, job.transparent, job.config.background_color)
            .await
            .map_err(|e| RenderError::PixelReadFailed(e.to_string()))
    };

    if let Some(chain) = &effect_chain {
        chain.destroy();
    }
    #[cfg(feature = "terrain")]
    if let Some(t) = terrain_owned.as_ref() {
        t.destroy();
    }
    // A throwaway run's grid (a caller's, a video's, goes on).
    if let Some(s) = owned.as_ref() {
        s.destroy();
    }
    let (width, height, rgba_data) = pixels?;

    progress.on_progress(1, 1);
    let render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "Render: simulation complete - {}x{} in {:.1}ms",
        width,
        height,
        render_time_ms
    );

    Ok(RenderOutput {
        width,
        height,
        rgba_data,
        // "Iterations" means the step count here, the way escape reports
        // its per-pixel ceiling: it is what the PNG metadata should
        // record to make the picture reproducible.
        total_iterations: job.config.sim.steps as u64,
        render_time_ms,
        frame_coverage: 1.0,
    })
}

/// Escape-time render path — the generator swap behind `render_with`.
///
/// Reuses the flame renderer for everything except the generator:
/// `load_config` uploads palette (rotation/squeeze), tonemap params,
/// curve LUT and background exactly as the flame path sees them, and
/// the tail below mirrors the flame tail minus its flame-only stages
/// (solid shade, DoF, density renormalization). The `EscapeRenderer`
/// itself is created per call and destroyed after readback, the same
/// one-shot discipline as `EffectChainRunner` — the interactive app
/// will hold a persistent one instead.
#[cfg(feature = "engine-escape")]
async fn render_escape(
    renderer: &mut FlameRenderer,
    device: &Device,
    queue: &Queue,
    mut job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
    start_time: web_time::Instant,
) -> Result<RenderOutput, RenderError> {
    // A formula or colouring the config carries as source, compiled
    // before anything below resolves its name.
    crate::escape::custom::ensure(&job.config.escape);
    log::info!(
        "Render: escape-time {}x{}, formula '{}', coloring '{}', max_iter {}",
        job.width,
        job.height,
        job.config.escape.formula,
        job.config.escape.coloring,
        job.config.escape.max_iter
    );
    progress.on_progress(0, 1);

    // Full config load. Deliberately the whole thing rather than a
    // targeted palette+tonemap upload: it is the one call guaranteed
    // to keep every tail input (palette texture, tonemap uniform,
    // curve LUT, background, levels) in exact sync with the flame
    // path. It also compiles the config's (unused) flame shaders —
    // wasted work worth revisiting if escape thumbnails ever feel
    // slow, not before.
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Escape Config Encoder"),
    });
    renderer.load_config(
        device,
        &mut encoder,
        queue,
        job.config,
        &job.config.palette,
        job.iterations_per_thread,
        job.burn_in,
    );
    queue.submit(std::iter::once(encoder.finish()));

    if job.transparent {
        renderer.set_transparent_mode(queue, true, job.premultiplied, job.config, job.iterations_per_thread);
    }

    // The 3D terrain view draws the same picture as a height field;
    // only its generator differs, and the tail is this one.
    #[cfg(feature = "terrain")]
    if job.config.escape.terrain_active() {
        return render_escape_terrain(renderer, device, queue, job, progress, start_time).await;
    }

    // The generator. High-iteration deep renders run as bounded
    // chunked dispatches, each its own submission — the driver never
    // sees an unbounded pass (the TDR class of crash). The final
    // chunk's encoder carries the tail passes below.
    // Catch an allocation failure instead of rendering past it. wgpu
    // reports OOM through the uncaptured-error handler, which stops
    // nothing: the buffer comes back invalid, every dispatch against
    // it silently does nothing, and the export "succeeds" with an
    // all-black image. Observed at 4000x3000 with 8x antialiasing,
    // where the export shares a device with the viewport's own escape
    // renderer.
    let oom_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    // Continue a caller-owned renderer when there is one. Unlike the
    // simulation there is no run to lose -- every escape frame renders
    // to settlement from scratch -- so this is purely about not
    // rebuilding several hundred megabytes of textures and orbit state
    // per video frame.
    // Whose renderer this is decides whether the teardown below may
    // free it. `destroy` exists because dropping frees nothing on
    // WebGPU; running it on the CALLER's renderer would hand the next
    // frame a corpse.
    let caller_owned = job.engines.is_some();
    let (config, width, height, transparent) = (job.config, job.width, job.height, job.transparent);
    let mut owned: Option<crate::escape::EscapeRenderer> = None;
    let make = || crate::escape::EscapeRenderer::new(device, job.width, job.height);
    let escape_renderer: &mut crate::escape::EscapeRenderer = match job.engines {
        Some(ref mut engines) => engines.escape.get_or_insert_with(make),
        None => owned.insert(make()),
    };
    // A solid path traced (heightfield plan T3c) renders display
    // pixels: its jitter is the antialiasing, and `samples` its count.
    let solid_path = crate::escape::ifs::formula_is_solid(&job.config.escape.formula)
        && job.config.escape.solid_tier != crate::config::escape::RenderTier::Lit;
    // Config-declared supersampling applies on every path (viewport,
    // CLI, thumbnails): a saved file reproduces exactly.
    let want_ss = if solid_path { 1 } else { job.config.escape.supersample.max(1) };
    escape_renderer.resize(device, job.width, job.height, want_ss);
    escape_renderer.set_pixel_scale(job.pixel_scale);
    // A browser has no blocking wait: the polls below return at once, so
    // chunks go in back to back and the adaptive sizer, which times the
    // gap between calls, would read each as free and grow toward a
    // watchdog-length dispatch. The fixed TDR-calibrated seed cannot
    // change the image (`EscapeRenderer::set_fixed_chunk`).
    #[cfg(target_arch = "wasm32")]
    escape_renderer.set_fixed_chunk(true);
    prepare_escape(escape_renderer, device, queue, job.config).await;
    // A solid path traced, motion-blurred (docs/archive/projects/motion-blur.md):
    // each moment traced with its share of the samples, averaged.
    let solid_blurred = solid_path && !job.moments.is_empty();
    if solid_path {
        let moments: Vec<&FractalConfig> = if solid_blurred { job.moments.iter().collect() } else { vec![job.config] };
        let samples = job.config.escape.path.samples.div_ceil(moments.len() as u32).max(1);
        if solid_blurred {
            escape_renderer.begin_accumulation_of(device, queue, moments.len() as u32);
        }
        for moment in &moments {
            if solid_blurred {
                prepare_escape(escape_renderer, device, queue, moment).await;
            }
            escape_renderer.render_solid_still(
                device,
                queue,
                moment,
                renderer.escape_palette_view(job.config.escape.palette_map.stepped),
                samples,
                || {
                    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                    !progress.is_cancelled()
                },
            );
            if progress.is_cancelled() {
                if !caller_owned {
                    escape_renderer.destroy();
                }
                return Err(RenderError::Cancelled);
            }
            if solid_blurred {
                let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Escape Solid Moment") });
                escape_renderer.accumulate_sample(device, queue, &mut encoder);
                queue.submit(std::iter::once(encoder.finish()));
            }
        }
        log::info!("Render: escape solid path traced at {samples} samples x {} moments", moments.len());
    }
    // No UI to keep responsive here, and every chunk pays a downsample
    // pass over the supersampled image — so chunk for throughput.
    escape_renderer.set_chunk_time_target(200.0);

    // What the supersampled grid could actually give. At export sizes
    // it is often less than was asked for -- 8x over 4000x3000 is 768
    // megapixels of per-pixel state AND 32000 pixels a side, past both
    // the memory budget and the texture-dimension limit every adapter
    // has -- and the shortfall used to be silent, which reads as
    // "antialiasing does nothing on export".
    //
    // The rest is made up by ACCUMULATION: the same sample positions,
    // taken as several ordinary renders each displaced within a pixel
    // and averaged. Same total iteration work, fixed memory, no size
    // limit. `extra == 1` is the single render this always was.
    let got_ss = escape_renderer.effective_supersample();
    let extra = want_ss.div_ceil(got_ss.max(1)).max(1);
    if extra > 1 {
        log::info!(
            "Escape export: {want_ss}x antialiasing = {got_ss}x grid x {extra}x \
             accumulated ({} renders)",
            extra * extra
        );
    }
    let offsets = if solid_path {
        Vec::new()
    } else if extra > 1 {
        crate::escape::EscapeRenderer::sample_grid(extra)
    } else {
        vec![[0.0f32, 0.0]]
    };
    // Motion blur (docs/archive/projects/motion-blur.md): every displacement at
    // every moment, each a whole render -- neighbouring pixels stay at
    // neighbouring positions, so finite-difference shading holds -- all
    // in the one accumulation. Without it the frame is its one moment.
    let moments: Vec<&FractalConfig> = if job.moments.is_empty() || solid_path {
        vec![job.config]
    } else {
        job.moments.iter().collect()
    };
    let renders = (moments.len() * offsets.len()) as u32;
    if moments.len() > 1 {
        log::info!("Escape: motion blur over {} moments ({renders} renders)", moments.len());
    }
    if renders > 1 {
        escape_renderer.begin_accumulation_of(device, queue, renders);
    }
    for (m, moment) in moments.iter().enumerate() {
        // A moment's flame (mode D) or texture is its own.
        if m > 0 {
            prepare_escape(escape_renderer, device, queue, moment).await;
        }
        for off in &offsets {
            escape_renderer.set_sample_offset(*off);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Escape Render"),
            });
            let mut settled = escape_renderer.render(
                device,
                queue,
                &mut encoder,
                &moment.escape,
                renderer.escape_palette_view(job.config.escape.palette_map.stepped),
                renderer.palette_generation(),
            );
            let mut guard = 0u32;
            while !settled {
                queue.submit(std::iter::once(encoder.finish()));
                let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Escape Render Chunk"),
                });
                settled = escape_renderer.render(
                    device,
                    queue,
                    &mut encoder,
                    &moment.escape,
                    renderer.escape_palette_view(job.config.escape.palette_map.stepped),
                    renderer.palette_generation(),
                );
                if progress.is_cancelled() {
                    if !caller_owned {
                        escape_renderer.destroy();
                    }
                    return Err(RenderError::Cancelled);
                }
                guard += 1;
                if guard > 4_000_000 {
                    log::error!("escape chunk loop failed to settle; rendering what we have");
                    break;
                }
            }
            // Fold this render into the running average. The encoder
            // still holds the settling chunk's resolve, so the fold is
            // ordered after it.
            if renders > 1 {
                escape_renderer.accumulate_sample(device, queue, &mut encoder);
            }
            queue.submit(std::iter::once(encoder.finish()));
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        }
    }
    let escape_view = match escape_renderer.accumulated_view() {
        Some(v) if (renders > 1 && !solid_path) || solid_blurred => v,
        _ => escape_renderer.output_view(),
    };
    let pixels = escape_tail(renderer, device, queue, config, (width, height, transparent), escape_view, oom_scope).await;
    // Readback completion (or the memory error found before it) proves
    // every submission finished -- the safe destroy point.
    if !caller_owned {
        escape_renderer.destroy();
    }
    let (width, height, rgba_data) = pixels?;

    progress.on_progress(1, 1);
    let render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "Render: escape complete - {}x{} in {:.1}ms",
        width,
        height,
        render_time_ms
    );

    Ok(RenderOutput {
        width,
        height,
        rgba_data,
        // "Iterations" means something different here: report the
        // per-pixel ceiling, not a chaos-game sample count.
        total_iterations: job.config.escape.max_iter as u64,
        render_time_ms,
        frame_coverage: 1.0,
    })
}

/// The escape engine's tail, shared by the 2D picture and the terrain:
/// density effects -> tonemap -> colour effects -> read back. `input`
/// is the generator's image in the accumulator's layout; the caller
/// frees its own generator after this returns, either way.
#[cfg(feature = "engine-escape")]
async fn escape_tail(
    renderer: &mut FlameRenderer,
    device: &Device,
    queue: &Queue,
    config: &FractalConfig,
    (width, height, transparent): (u32, u32, bool),
    escape_view: &wgpu::TextureView,
    oom_scope: wgpu::ErrorScopeGuard,
) -> Result<(u32, u32, Vec<u8>), RenderError> {
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Escape Tail"),
    });

    // Shared tail: density effects → tonemap → color effects → read.
    let has_density_effects = EffectChainRunner::has_enabled_effects(&config.density_effects);
    let has_color_effects = EffectChainRunner::has_enabled_effects(&config.color_effects);
    let mut effect_chain = if has_density_effects || has_color_effects {
        Some(EffectChainRunner::new(device, width, height))
    } else {
        None
    };
    if let Some(chain) = effect_chain.as_mut() {
        chain.reset_slots();
        chain.set_pixel_scale(renderer.pixel_scale());
    }

    if has_density_effects {
        let chain = effect_chain.as_mut().expect("built above: has_density_effects");
        let density_ran = chain.run_density_effects(
            device,
            queue,
            &mut encoder,
            escape_view,
            &config.density_effects,
            1.0, // an escape image's alpha is not a hit count
        );
        match (density_ran, chain.get_density_output()) {
            (true, Some(density_output)) => {
                renderer.tonemap_pass_with_input(device, queue, &mut encoder, density_output)
            }
            _ => renderer.tonemap_pass_with_input(device, queue, &mut encoder, escape_view),
        }
    } else {
        renderer.tonemap_pass_with_input(device, queue, &mut encoder, escape_view);
    }
    queue.submit(std::iter::once(encoder.finish()));

    let color_effects_ran = if has_color_effects {
        let chain = effect_chain.as_mut().expect("built above: has_color_effects");
        let mut color_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Escape Color Effects"),
        });
        let ran = chain.run_color_effects(
            device,
            queue,
            &mut color_encoder,
            renderer.get_fractal_texture_view(),
            &config.color_effects,
        );
        queue.submit(std::iter::once(color_encoder.finish()));
        ran
    } else {
        false
    };

    // Before reading anything back: did we get the memory we asked
    // for? Checked here rather than at each allocation because the
    // scope covers the whole render, the per-sample accumulation
    // passes included.
    if let Some(err) = oom_scope.pop().await {
        if let Some(chain) = &effect_chain {
            chain.destroy();
        }
        return Err(RenderError::OutOfMemory(err.to_string()));
    }

    let pixels = if color_effects_ran {
        effect_chain
            .as_ref()
            .expect("color_effects_ran implies a chain")
            .read_color_output_pixels(device, queue)
            .await
            .map_err(RenderError::PixelReadFailed)
    } else {
        renderer
            .read_fractal_pixels(device, queue, transparent, config.background_color)
            .await
            .map_err(|e| RenderError::PixelReadFailed(e.to_string()))
    };

    // Readback completion proves every submission finished — the safe
    // destroy point for the per-call GPU objects, error path included.
    if let Some(chain) = &effect_chain {
        chain.destroy();
    }
    pixels
}

/// What any escape render needs before its first chunk: a qualifying
/// flame's IFS analysis (mode D, and the 2D IFS formulas) and a
/// texture layer's image.
#[cfg(feature = "engine-escape")]
async fn prepare_escape(
    escape_renderer: &mut crate::escape::EscapeRenderer,
    device: &Device,
    queue: &Queue,
    config: &FractalConfig,
) {
    // Mode D reads the flame as an IFS, so the analysis runs once per
    // job here rather than per pixel in the shader. A flame that does
    // not qualify hands the renderer `None` and draws nothing — the
    // panel is where the reason is explained.
    if let Some(def) = crate::escape::ifs::get_ifs(&config.escape.formula) {
        let registry = crate::variations::global_registry();
        escape_renderer.set_ifs(crate::escape::ifs::pack_for(def, config, &registry));
        // The MEASURE colouring reads a coarse pass over the ball;
        // without one it renders black. Keyed on the flame, so this
        // is once per flame and not once per view.
        escape_renderer.ensure_coarse(device, queue, &config.escape, &config.flame);
        // A SOLID walk lights itself, from the app's own lighting
        // settings rather than a second vocabulary of its own.
        escape_renderer.set_solid_lighting(
            &config.solid_shading,
            (
                config.fog_strength,
                config.fog_start,
                config.background_color,
            ),
        );
        escape_renderer.set_solid_sky(crate::escape::path_core::sky_seen(&config.escape.path, config));
        escape_renderer.set_solid_material(crate::escape::path_core::lit_material(config));
        escape_renderer.set_solid_studio(crate::escape::path_core::studio_level(&config.escape.path, config));
        escape_renderer.set_environment(device, queue, crate::escape::path_core::environment_settings(&config.escape.path, config));
    }
    // The texture's image (docs/archive/projects/sim-textures.md): the
    // config carries the recipe, and the image comes from the cache or
    // is generated here, so a saved file renders the same anywhere.
    if config.escape.uses_texture() {
        if let Some(texture) = &config.escape.texture {
            let key = crate::textures::cache::key(&texture.config);
            if escape_renderer.texture_key() != Some(key.as_str()) {
                match crate::textures::obtain(device, queue, &texture.config).await {
                    Ok(image) => {
                        escape_renderer.set_texture(device, queue, &key, &image);
                    }
                    Err(e) => log::warn!("texture '{}' could not be generated: {e}", texture.name),
                }
            }
        }
    }
}

/// The 3D terrain view of an escape picture (docs/archive/terrain/heightfield-3d.md,
/// phase T2): the footprint -- the 2D picture at the view, square, at
/// the terrain's resolution -- rendered to settlement and ingested as a
/// tile, then the terrain drawn in the lit tier. Antialiasing is the
/// config's supersample as accumulation: `supersample²` renders, each
/// jittered within the pixel. Then the escape tail, unchanged.

/// A motion-blur moment drawn on its own ground, for its still: its share
/// of the frame's antialiasing -- the lit grid's passes, the path
/// tracer's samples -- so the moments together cost about what the frame
/// does. The ground is drawn from the moment itself: the supersampling
/// is also the sections' detail (`wanted_sections`).
#[cfg(feature = "terrain")]
fn moment_share(moment: &FractalConfig, moments: usize) -> FractalConfig {
    let k = moments.max(1) as f64;
    let mut c = moment.clone();
    c.escape.supersample = ((c.escape.supersample.max(1) as f64 / k.sqrt()).round() as u32).max(1);
    c.escape.path.samples = ((c.escape.path.samples as f64 / k).ceil() as u32).max(1);
    c
}
#[cfg(feature = "terrain")]
async fn render_escape_terrain(
    renderer: &mut FlameRenderer,
    device: &Device,
    queue: &Queue,
    mut job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
    start_time: web_time::Instant,
) -> Result<RenderOutput, RenderError> {
    use crate::escape::footprint::EscapeTerrain;
    let oom_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let caller_owned = job.engines.is_some();
    let (job_w, job_h, transparent) = (job.width, job.height, job.transparent);
    let mut owned: Option<EscapeTerrain> = None;
    // The still sizes the renderer itself: to the frame, or to its tiles.
    let (tile_w, tile_h) = crate::escape::terrain_tiers::still_tile(job.width, job.height);
    let make = || EscapeTerrain::new(device, tile_w, tile_h);
    let terrain: &mut EscapeTerrain = match job.engines {
        Some(ref mut engines) => engines.terrain.get_or_insert_with(make),
        None => owned.insert(make()),
    };
    let config = job.config;
    // Every section the export's own view wants (plan section 12),
    // rendered before a sample is drawn.
    let palette = renderer.escape_palette_view(config.escape.palette_map.stepped);
    let generation = renderer.palette_generation();
    prepare_escape(terrain.footprint_renderer(), device, queue, config).await;
    terrain.footprint_renderer().set_chunk_time_target(200.0);
    // No blocking wait in a browser -- see `render_escape`.
    #[cfg(target_arch = "wasm32")]
    terrain.footprint_renderer().set_fixed_chunk(true);
    // As many sections as the card holds beside the rest of the export:
    // past the viewport's 64, an export's nearest ground was magnified.
    terrain.fit_sections_for_export(device, config, job_w, job_h).await;
    // The bar's shares: the sections, then the still, which takes most
    // of a path traced export.
    let fill_share = if config.escape.terrain.tier == crate::config::escape::RenderTier::Lit { 0.9 } else { 0.25 };
    // Motion blur (docs/archive/projects/motion-blur.md). Moments that draw the
    // frame's picture differ only in the camera: one ground, and the
    // still's passes or samples go round their views, at the frame's own
    // cost. Moments whose pictures differ each need their own ground:
    // each its sections and a still with its share of the antialiasing,
    // averaged.
    let moments = job.moments;
    let separate = !moments.iter().all(|m| EscapeTerrain::same_picture(m, config));
    let grounds: Vec<&FractalConfig> = if separate { moments.iter().collect() } else { vec![config] };
    if separate {
        terrain.begin_moments(device, queue, job_w, job_h, grounds.len() as u32);
    }
    let mut n = 0;
    for (g, ground) in grounds.iter().enumerate() {
        let span = |f: f64| ((g as f64 + f) / grounds.len() as f64 * 1000.0) as u64;
        if separate {
            // A moment's flame (mode D) or texture is its own.
            prepare_escape(terrain.footprint_renderer(), device, queue, ground).await;
        }
        let before = terrain.footprint_renders;
        let mut guard = 0u32;
        while terrain.update(device, queue, ground, job.width, job.height, palette, generation, 1) {
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            if progress.is_cancelled() {
                if !caller_owned {
                    terrain.destroy();
                }
                return Err(RenderError::Cancelled);
            }
            let done = (terrain.footprint_renders - before) as f64;
            let fill = done / (done + terrain.sections_missing() as f64).max(1.0);
            progress.on_progress(span(fill_share * fill), 1000);
            guard += 1;
            if guard > 4_000_000 {
                log::error!("terrain sections failed to settle; using what we have");
                break;
            }
        }
        n += terrain.footprint_renders - before;
        // The picture: path traced at `samples`, or the lit tier's grid
        // -- a moment on its own ground, its share of either.
        let share;
        let (still, views): (&FractalConfig, &[FractalConfig]) = if separate {
            share = moment_share(ground, grounds.len());
            (&share, &[])
        } else {
            (config, moments)
        };
        terrain.render_still_moments(device, queue, still, views, (job_w, job_h), |f| {
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            progress.on_progress(span(fill_share + (1.0 - fill_share) * f as f64), 1000);
            !progress.is_cancelled()
        });
        if progress.is_cancelled() {
            if !caller_owned {
                terrain.destroy();
            }
            return Err(RenderError::Cancelled);
        }
        if separate {
            terrain.add_moment(device, queue);
        }
    }
    let ss = config.escape.supersample.max(1);
    let picture = match terrain.moments_view() {
        Some(v) if separate => v,
        _ => terrain.output_view(),
    };
    let pixels = escape_tail(renderer, device, queue, config, (job_w, job_h, transparent), picture, oom_scope).await;
    if !caller_owned {
        terrain.destroy();
    }
    let (width, height, rgba_data) = pixels?;

    progress.on_progress(1, 1);
    let render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "Render: escape terrain complete - {}x{} ({} sections rendered, {} samples, {}) in {:.1}ms",
        width,
        height,
        n,
        if config.escape.terrain.tier == crate::config::escape::RenderTier::Lit { ss * ss } else { config.escape.path.samples },
        if config.escape.terrain.tier == crate::config::escape::RenderTier::Lit { "lit" } else { "path traced" },
        render_time_ms
    );
    Ok(RenderOutput {
        width,
        height,
        rgba_data,
        total_iterations: config.escape.max_iter as u64,
        render_time_ms,
        frame_coverage: 1.0,
    })
}

/// Motion blur through `render` (docs/archive/projects/motion-blur.md).
#[cfg(all(test, feature = "engine-escape"))]
mod motion_tests {
    use super::*;

    fn device() -> Option<(Device, Queue)> {
        pollster::block_on(async {
            let instance = egui_wgpu::wgpu::Instance::new(egui_wgpu::wgpu::InstanceDescriptor {
                backends: egui_wgpu::wgpu::Backends::all(),
                ..egui_wgpu::wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance.request_adapter(&egui_wgpu::wgpu::RequestAdapterOptions::default()).await.ok()?;
            adapter
                .request_device(&egui_wgpu::wgpu::DeviceDescriptor {
                    label: Some("motion blur tests"),
                    required_features: adapter.features() & egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
                    required_limits: adapter.limits(),
                    memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
                    experimental_features: Default::default(),
                    trace: Default::default(),
                })
                .await
                .ok()
        })
    }

    /// A Mandelbrot view, linear tone mapping at exposure and gamma 1, so
    /// the tail is the identity on the averaged colour and an average of
    /// finished frames stands for the average before the tail.
    fn view(center_re: &str) -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.exposure = 1.0;
        c.gamma = 1.0;
        c.escape.formula = "mandelbrot".into();
        c.escape.coloring = "smooth".into();
        c.escape.center_re = center_re.into();
        c.escape.center_im = "0.1318259042053119".into();
        c.escape.zoom_log2 = 5.0;
        c.escape.max_iter = 300;
        c.escape.supersample = 1;
        c
    }

    const W: u32 = 96;
    const H: u32 = 72;

    fn draw(device: &Device, queue: &Queue, config: &FractalConfig, moments: &[FractalConfig]) -> Vec<u8> {
        let job = RenderJob::new(config, W, H).with_moments(moments);
        pollster::block_on(render(device, queue, job, &mut NoProgress)).expect("render").rgba_data
    }

    /// Moments that are all the frame blur it into itself.
    #[test]
    fn a_still_frame_blurs_into_itself() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let frame = view("-0.7436438870371587");
        let sharp = draw(&device, &queue, &frame, &[]);
        let blurred = draw(&device, &queue, &frame, &vec![frame.clone(); 4]);
        let worst = sharp.iter().zip(&blurred).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        assert!(worst <= 1, "a still frame changed by {worst} blurred");
    }

    /// A pan, blurred, is the mean of its moments' pictures -- and not the
    /// frame's own. The mean of the LIGHT: the tail encodes for display
    /// (`pow(1/2.2)`), and the moments are averaged before it, so the
    /// pictures are decoded, averaged and encoded again to compare.
    #[test]
    fn a_blurred_pan_is_the_mean_of_its_moments() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let frame = view("-0.7436438870371587");
        // About a pixel and a half to four and a half pixels either side.
        let moments: Vec<FractalConfig> = [-0.006, -0.002, 0.002, 0.006]
            .iter()
            .map(|d| view(&format!("{:.16}", -0.7436438870371587 + d)))
            .collect();
        let blurred = draw(&device, &queue, &frame, &moments);
        let pictures: Vec<Vec<u8>> = moments.iter().map(|m| draw(&device, &queue, m, &[])).collect();
        let mut far = 0usize;
        for (i, b) in blurred.iter().enumerate() {
            let light = pictures.iter().map(|p| (p[i] as f64 / 255.0).powf(2.2)).sum::<f64>() / pictures.len() as f64;
            let mean = 255.0 * light.powf(1.0 / 2.2);
            if (*b as f64 - mean).abs() > 2.0 {
                far += 1;
            }
        }
        assert!(far * 1000 <= blurred.len(), "{far} of {} channels off the moments' mean", blurred.len());
        let sharp = draw(&device, &queue, &frame, &[]);
        let moved = sharp.iter().zip(&blurred).map(|(a, b)| a.abs_diff(*b) as f64).sum::<f64>() / sharp.len() as f64;
        assert!(moved > 1.0, "the blur changed the frame by only {moved:.3} a channel");
    }

    fn read(device: &Device, queue: &Queue, tex: &egui_wgpu::wgpu::Texture) -> Vec<f32> {
        use egui_wgpu::wgpu::*;
        let (w, h) = (tex.width(), tex.height());
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
            TexelCopyBufferInfo { buffer: &staging, layout: TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        staging.slice(..).map_async(MapMode::Read, |_| {});
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let bytes = staging.slice(..).get_mapped_range().to_vec();
        staging.unmap();
        (0..h).flat_map(|y| bytemuck::cast_slice::<u8, f32>(&bytes[(y * row) as usize..(y * row + w * 16) as usize]).to_vec()).collect()
    }

    /// The viewport's finished blurred frame (`present_accumulation`)
    /// stays as it finished while the next frame builds in the same
    /// accumulation: the viewport shows it, not a part-folded frame.
    #[test]
    fn a_finished_frame_stays_while_the_next_builds() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let at = |d: f64| view(&format!("{:.16}", -0.7436438870371587 + d));
        let flame = FlameRenderer::for_job(&device, &queue, TextureFormat::Rgba8Unorm, W, H, &at(0.0));
        let mut escape = crate::escape::EscapeRenderer::new(&device, W, H);
        // Fold `fold` of a frame's moments, presenting the frame once all are in.
        let build = |escape: &mut crate::escape::EscapeRenderer, moments: &[FractalConfig], fold: usize| {
            escape.begin_accumulation_of(&device, &queue, moments.len() as u32);
            for (i, m) in moments[..fold].iter().enumerate() {
                let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
                while !escape.render(&device, &queue, &mut enc, &m.escape, flame.escape_palette_view(false), flame.palette_generation()) {
                    queue.submit(std::iter::once(enc.finish()));
                    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                    enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
                }
                escape.accumulate_sample(&device, &queue, &mut enc);
                if i + 1 == moments.len() {
                    escape.present_accumulation(&device, &mut enc);
                }
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            }
        };
        let a: Vec<FractalConfig> = [-0.004, 0.0, 0.004].iter().map(|d| at(*d)).collect();
        build(&mut escape, &a, a.len());
        let shown = read(&device, &queue, escape.presented_texture().expect("presented"));
        let finished = read(&device, &queue, escape.accumulated_texture().expect("accumulated"));
        assert!(shown == finished, "the frame shown is the accumulation as it finished");

        let b: Vec<FractalConfig> = [0.02, 0.024, 0.028].iter().map(|d| at(*d)).collect();
        build(&mut escape, &b, 1);
        let still = read(&device, &queue, escape.presented_texture().expect("presented"));
        let building = read(&device, &queue, escape.accumulated_texture().expect("accumulated"));
        assert!(still == shown, "the frame shown changed while the next built");
        assert!(building != finished, "the next frame did not build");
        escape.destroy();
    }

    /// Playback's settled picture (`present_output`) stays as it settled
    /// while the next renders, so the viewport shows it and not the next
    /// one's chunks (docs/archive/projects/quality-governor.md, phase 1).
    #[test]
    fn a_settled_picture_stays_while_the_next_renders() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let at = |d: f64| view(&format!("{:.16}", -0.7436438870371587 + d));
        let flame = FlameRenderer::for_job(&device, &queue, TextureFormat::Rgba8Unorm, W, H, &at(0.0));
        let mut escape = crate::escape::EscapeRenderer::new(&device, W, H);
        let settle = |escape: &mut crate::escape::EscapeRenderer, c: &FractalConfig, hold: bool| {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            while !escape.render(&device, &queue, &mut enc, &c.escape, flame.escape_palette_view(false), flame.palette_generation()) {
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            }
            if hold {
                escape.present_output(&device, &mut enc);
            }
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        };
        settle(&mut escape, &at(0.0), true);
        let shown = read(&device, &queue, escape.presented_texture().expect("presented"));
        let first = read(&device, &queue, escape.output_texture_for_test());
        assert!(shown == first, "the picture held is the one that settled");

        settle(&mut escape, &at(0.02), false);
        let still = read(&device, &queue, escape.presented_texture().expect("presented"));
        let next = read(&device, &queue, escape.output_texture_for_test());
        assert!(still == shown, "the picture held changed while the next rendered");
        assert!(next != first, "the next picture did not render");
        escape.destroy();
    }

    /// Gray-Scott at `steps`, linear tone mapping (as `view`).
    #[cfg(feature = "engine-sim")]
    fn grid(steps: u32) -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = crate::scene::transforms::RenderMode::Simulation;
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.exposure = 1.0;
        c.gamma = 1.0;
        c.sim.steps = steps;
        c
    }

    /// A simulation's moments are its field at their step counts: a
    /// blurred frame is the mean of their light, each drawn on its own,
    /// and one whose moments are all the frame is the frame.
    #[cfg(feature = "engine-sim")]
    #[test]
    fn a_blurred_simulation_is_the_mean_of_its_step_counts() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let frame = grid(550);
        let sharp = draw(&device, &queue, &frame, &[]);
        let still = draw(&device, &queue, &frame, &vec![frame.clone(); 4]);
        let worst = sharp.iter().zip(&still).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        assert!(worst <= 1, "a still frame changed by {worst} blurred");

        let moments: Vec<FractalConfig> = [400, 500, 600, 700].into_iter().map(grid).collect();
        let blurred = draw(&device, &queue, &frame, &moments);
        let pictures: Vec<Vec<u8>> = moments.iter().map(|m| draw(&device, &queue, m, &[])).collect();
        let far = blurred
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                let light = pictures.iter().map(|p| (p[*i] as f64 / 255.0).powf(2.2)).sum::<f64>() / pictures.len() as f64;
                (**b as f64 - 255.0 * light.powf(1.0 / 2.2)).abs() > 2.0
            })
            .count();
        assert!(far * 1000 <= blurred.len(), "{far} of {} channels off the moments' mean", blurred.len());
        let moved = sharp.iter().zip(&blurred).map(|(a, b)| a.abs_diff(*b) as f64).sum::<f64>() / sharp.len() as f64;
        assert!(moved > 1.0, "the blur changed the frame by only {moved:.3} a channel");
    }

    /// A simulation's terrain blurs too: moments that are all the frame
    /// are the frame; a camera's move, drawn on one ground, and the field
    /// moving, a ground a moment, each change it.
    #[cfg(all(feature = "engine-sim", feature = "terrain"))]
    #[test]
    fn a_simulation_terrain_blurs_over_its_moments() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut frame = grid(300);
        frame.sim.terrain.enabled = true;
        frame.sim.terrain.tier = crate::config::escape::RenderTier::Lit;
        let sharp = draw(&device, &queue, &frame, &[]);
        assert_eq!(draw(&device, &queue, &frame, &vec![frame.clone(); 4]), sharp, "a still frame blurred is the frame");
        let moved = |picture: &[u8]| sharp.iter().zip(picture).map(|(a, b)| a.abs_diff(*b) as f64).sum::<f64>() / sharp.len() as f64;
        let camera: Vec<FractalConfig> = [-0.06f32, -0.02, 0.02, 0.06]
            .iter()
            .map(|d| {
                let mut m = frame.clone();
                m.sim.terrain.cam_yaw += d;
                m
            })
            .collect();
        let turned = moved(&draw(&device, &queue, &frame, &camera));
        let steps: Vec<FractalConfig> = [200u32, 266, 333, 400]
            .iter()
            .map(|n| {
                let mut m = frame.clone();
                m.sim.steps = *n;
                m
            })
            .collect();
        let stepped = moved(&draw(&device, &queue, &frame, &steps));
        println!("a turn moved the frame {turned:.3} a channel, the steps {stepped:.3}");
        assert!(turned > 0.2 && stepped > 0.2, "the blur did not reach the terrain: {turned:.3}, {stepped:.3}");
    }

    /// A solid path traced, blurred, traces each moment with its share of
    /// the samples: moments that are all the frame keep its light.
    #[test]
    fn a_traced_solid_keeps_its_light_blurred() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let mut frame = view("0.0");
        frame.escape.formula = "mandelbulb_solid".into();
        frame.escape.solid_tier = crate::config::escape::RenderTier::PathTraced;
        frame.escape.path.samples = 16;
        let light = |p: &[u8]| p.chunks(4).map(|c| c[..3].iter().map(|v| (*v as f64 / 255.0).powf(2.2)).sum::<f64>()).sum::<f64>();
        let sharp = light(&draw(&device, &queue, &frame, &[]));
        let blurred = light(&draw(&device, &queue, &frame, &vec![frame.clone(); 4]));
        println!("traced solid: {blurred:.1} blurred against {sharp:.1} sharp");
        assert!(sharp > 1.0, "the solid drew nothing");
        assert!((blurred / sharp - 1.0).abs() < 0.05, "light {blurred:.1} blurred against {sharp:.1} sharp");
    }

    /// A Sierpinski gasket filling the view, its first map's offset `e`.
    fn gasket(e: f32) -> FractalConfig {
        let mut cfg = FractalConfig::default();
        cfg.render_mode = crate::scene::transforms::RenderMode::TwoD;
        cfg.flame.transforms.clear();
        for (i, (x, y)) in [(e, 0.0f32), (0.5, 0.0), (0.25, 0.5)].into_iter().enumerate() {
            let mut t = crate::scene::transforms::Transform::default();
            (t.a, t.b, t.c, t.d, t.e, t.f) = (0.5, 0.0, 0.0, 0.5, x, y);
            t.weight = 1.0;
            t.color = i as f32 / 2.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            cfg.flame.transforms.push(t);
        }
        cfg.zoom = 1.6;
        (cfg.pan_x, cfg.pan_y) = (-0.25, -0.25);
        cfg.levels_enabled = false;
        cfg
    }

    const N: u32 = 64;

    /// A flame's density per iteration, and how many times its orbits
    /// restarted, rendered as the video export renders a frame.
    fn flame_density(device: &Device, queue: &Queue, config: &FractalConfig, moments: &[FractalConfig], iterations: u64) -> (Vec<f64>, u32) {
        let mut renderer = FlameRenderer::for_job(device, queue, TextureFormat::Rgba8Unorm, N, N, config);
        renderer.set_sticky_enabled(false);
        let before = renderer.orbit_generation();
        let job = RenderJob::new(config, N, N).with_iterations(iterations).with_moments(moments);
        pollster::block_on(render_with(&mut renderer, device, queue, job, &mut NoProgress)).expect("render");
        (renderer.read_density_blocking(device, queue), renderer.orbit_generation().wrapping_sub(before))
    }

    /// 8x8-pixel block sums of a density.
    fn blocks(d: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; ((N / 8) * (N / 8)) as usize];
        for (i, v) in d.iter().enumerate() {
            let (x, y) = (i as u32 % N, i as u32 / N);
            out[((y / 8) * (N / 8) + x / 8) as usize] += v;
        }
        out
    }

    /// The transform and parameter tables write only the slots a flame
    /// uses (`write_regions`): a flame drawn after a larger one, the
    /// larger one's slots left behind, is the flame drawn fresh -- none of
    /// the stale slots is read. (The larger one's extra maps throw points
    /// far across the view, so a read would show.)
    #[test]
    fn a_smaller_flame_after_a_larger_reads_none_of_its_slots() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let small = gasket(0.0);
        let mut large = small.clone();
        for (i, e) in [0.6f32, 0.7, 0.8].into_iter().enumerate() {
            let mut t = large.flame.transforms[i].clone();
            (t.a, t.d, t.e, t.f) = (0.3, 0.3, e, 0.6);
            t.set_variation("spherical", 0.5);
            large.flame.transforms.push(t);
        }
        let samples = 8_000_000u64;
        let mut renderer = FlameRenderer::for_job(&device, &queue, TextureFormat::Rgba8Unorm, N, N, &large);
        renderer.set_sticky_enabled(false);
        let job = RenderJob::new(&large, N, N).with_iterations(samples);
        pollster::block_on(render_with(&mut renderer, &device, &queue, job, &mut NoProgress)).expect("render");
        let job = RenderJob::new(&small, N, N).with_iterations(samples);
        pollster::block_on(render_with(&mut renderer, &device, &queue, job, &mut NoProgress)).expect("render");
        let after = blocks(&renderer.read_density_blocking(&device, &queue));
        let fresh = blocks(&flame_density(&device, &queue, &small, &[], samples).0);
        let total: f64 = fresh.iter().sum();
        let worst = fresh
            .iter()
            .zip(&after)
            .filter(|(f, _)| **f > total / fresh.len() as f64 * 0.25)
            .map(|(f, a)| (a - f).abs() / f)
            .fold(0.0, f64::max);
        assert!(worst < 0.02, "a block {:.1}% off the flame drawn fresh", worst * 100.0);
        let stray: f64 = fresh.iter().zip(&after).filter(|(f, _)| **f == 0.0).map(|(_, a)| a).sum();
        assert!(stray == 0.0, "density {stray} where the flame draws none: a stale slot was read");
    }

    /// A flame whose moments are all the frame keeps its density blurred:
    /// the moments share the samples, and the tone map's count is theirs.
    #[test]
    fn a_still_flame_blurs_into_itself() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let frame = gasket(0.0);
        let (sharp, _) = flame_density(&device, &queue, &frame, &[], 2_000_000);
        let (blurred, _) = flame_density(&device, &queue, &frame, &vec![frame.clone(); 4], 2_000_000);
        let (s, b) = (sharp.iter().sum::<f64>(), blurred.iter().sum::<f64>());
        assert!((b / s - 1.0).abs() < 0.01, "density {b:.4} blurred against {s:.4} sharp");
    }

    /// A flame moving across the shutter, blurred, has the mean density of
    /// its moments -- each drawn sharp on its own -- and its orbits go on
    /// from moment to moment: one restart a frame, not one a moment.
    #[test]
    fn a_blurred_flame_is_the_mean_of_its_moments() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let frame = gasket(0.0);
        let moments: Vec<FractalConfig> = [-0.03, -0.01, 0.01, 0.03].iter().map(|e| gasket(*e)).collect();
        // Enough samples that the blocks' noise is well under the bound:
        // measured 3% at 2M a moment, 0.6% at 8M, 0.15% at 32M -- no bias
        // from the orbits going on across the moments at that level.
        let per_moment = 8_000_000u64;
        let (blurred, restarts) = flame_density(&device, &queue, &frame, &moments, per_moment * moments.len() as u64);
        assert_eq!(restarts, 1, "the moments restarted the orbits");
        let sharp: Vec<Vec<f64>> = moments.iter().map(|m| blocks(&flame_density(&device, &queue, m, &[], per_moment).0)).collect();
        let mean: Vec<f64> = (0..sharp[0].len()).map(|i| sharp.iter().map(|s| s[i]).sum::<f64>() / sharp.len() as f64).collect();
        let got = blocks(&blurred);
        let total: f64 = mean.iter().sum();
        let mut worst = 0.0f64;
        for (m, g) in mean.iter().zip(&got) {
            // Blocks holding a fair share of the light.
            if *m > total / mean.len() as f64 * 0.25 {
                worst = worst.max((g - m).abs() / m);
            }
        }
        assert!(worst < 0.02, "a block {:.1}% off the moments' mean", worst * 100.0);
        // And the blur is real: the moments differ from each other.
        let apart = sharp[0].iter().zip(&sharp[3]).map(|(a, b)| (a - b).abs()).sum::<f64>() / total;
        assert!(apart > 0.05, "the moments barely differ ({apart:.3})");
    }
}
