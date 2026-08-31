//! Unified rendering API
//!
//! This module provides a single entry point for rendering fractal flames to pixels.
//! All export paths (CLI, WASM, thumbnails, video) should use this API to ensure
//! consistent behavior and reduce code duplication.

use egui_wgpu::wgpu::{CommandEncoderDescriptor, Device, PollType, Queue, TextureFormat};

use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::renderer::effect_chain::EffectChainRunner;

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
}

impl<'a> RenderJob<'a> {
    /// Create a render job from a FractalConfig with sensible defaults
    pub fn new(config: &'a FractalConfig, width: u32, height: u32) -> Self {
        Self {
            config,
            width,
            height,
            target_iterations: None, // Use config.max_iterations
            iterations_per_thread: 256,
            burn_in: 20,
            transparent: false,
            premultiplied: false,
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
}

/// Result of a completed render
pub struct RenderOutput {
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
    pub total_iterations: u64,
    pub render_time_ms: f64,
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
        }
    }
}

impl std::error::Error for RenderError {}

// Constants for render loop
const NUM_WORKGROUPS: u32 = 128;
const THREADS_PER_WORKGROUP: u64 = 64;
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
    job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
) -> Result<RenderOutput, RenderError> {
    // Refuse a size this device cannot hold BEFORE allocating any of
    // it -- including the flame renderer's own textures, which are
    // built below whichever engine ends up rendering. The limits are
    // knowable up front; the alternative is a rejected allocation
    // that stops nothing and a black image.
    #[cfg(feature = "engine-escape")]
    if job.config.render_mode == crate::scene::transforms::RenderMode::Escape {
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
            return Err(RenderError::OutOfMemory(why));
        }
    }

    // Create renderer with config's palette size
    let surface_format = TextureFormat::Rgba8Unorm;
    let mut renderer = FlameRenderer::with_palette_size(
        device,
        queue,
        surface_format,
        job.width,
        job.height,
        &job.config.flame,
        job.config.palette_size,
    );
    // One-shot path: the sticky superset canonically reorders the local
    // index map, which is an ULP-class trajectory change — meaningless
    // for a persistent renderer, but exports and the visual suite must
    // render exactly the specialized shaders their baselines and
    // reproducibility contracts were made with. A throwaway renderer
    // gains nothing from stickiness anyway.
    renderer.set_sticky_enabled(false);

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
    let start_time = web_time::Instant::now();

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
                job.config.zoom, job.config.pan_x, job.config.pan_y, job.config.rotation,
                job.config.camera_rotation_x, job.config.camera_rotation_y, job.config.camera_bank,
                job.config.camera_x, job.config.camera_y, job.config.camera_z,
                job.config.speed_factor, true, false,
            );
            queue.submit(std::iter::once(enc.finish()));
        }
        let changed = renderer.refresh_shadow_placement_blocking(
            device, queue,
            job.config.zoom, job.config.pan_x, job.config.pan_y,
            job.config.camera_rotation_x, job.config.camera_rotation_y, job.config.camera_bank,
            [job.config.camera_x, job.config.camera_y, job.config.camera_z],
        );
        log::info!("Shadow warmup: fit {}", if changed { "refit to measured bounds" } else { "unchanged" });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Shadow Warmup Reset"),
        });
        renderer.reset(
            &mut enc, queue, job.iterations_per_thread,
            job.config.zoom, job.config.pan_x, job.config.pan_y, job.config.rotation,
            job.config.camera_rotation_x, job.config.camera_rotation_y, job.config.camera_bank,
            job.config.camera_x, job.config.camera_y, job.config.camera_z,
            job.config.speed_factor,
        );
        queue.submit(std::iter::once(enc.finish()));
    }

    // Render loop
    let mut total_rendered = 0u64;
    let mut batch_frame_count = 0u32;

    while total_rendered < target {
        // Check for cancellation
        if progress.is_cancelled() {
            return Err(RenderError::Cancelled);
        }

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Frame"),
        });

        let clear_histogram = batch_frame_count == 0;
        let clear_paths = total_rendered == 0 && clear_histogram;

        renderer.compute_pass(
            &mut encoder,
            queue,
            device,
            NUM_WORKGROUPS,
            job.iterations_per_thread,
            job.burn_in,
            job.config.zoom,
            job.config.pan_x,
            job.config.pan_y,
            job.config.rotation,
            job.config.camera_rotation_x,
            job.config.camera_rotation_y,
            job.config.camera_bank,
            job.config.camera_x,
            job.config.camera_y,
            job.config.camera_z,
            job.config.speed_factor,
            clear_histogram,
            clear_paths,
        );

        let samples_this_frame =
            NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * job.iterations_per_thread as u64;
        total_rendered += samples_this_frame;
        batch_frame_count += 1;

        // Accumulate when batch is complete
        if batch_frame_count >= BATCH_SIZE {
            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
            renderer.accumulate_pass(&mut encoder, queue, device, total_samples_in_batch);
            batch_frame_count = 0;
        }

        queue.submit(std::iter::once(encoder.finish()));

        // Report progress
        progress.on_progress(total_rendered, target);

        // Check if we've reached target
        if total_rendered >= target {
            // Final accumulation for partial batch
            if batch_frame_count > 0 {
                let mut final_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Final Batch Accumulation"),
                });
                let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                renderer.accumulate_pass(&mut final_encoder, queue, device, total_samples_in_batch);
                queue.submit(std::iter::once(final_encoder.finish()));
            }
            break;
        }
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
    }

    // Solid brightness renormalization: exact accepted-density measurement
    // before the single final tonemap.
    renderer.apply_exact_density_fraction(device, queue);

    // Solid-rendering shade pass — same ordering as the interactive frame:
    // shade, then density effects, then tonemap.
    let shade_ran = renderer.run_shade_pass(
        device,
        queue,
        &mut tonemap_encoder,
        job.config.zoom,
        job.config.rotation,
        job.config.pan_x,
        job.config.pan_y,
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
    job: RenderJob<'_>,
    progress: &mut dyn RenderProgress,
    start_time: web_time::Instant,
) -> Result<RenderOutput, RenderError> {
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
    let mut escape_renderer = crate::escape::EscapeRenderer::new(device, job.width, job.height);
    // Config-declared supersampling applies on every path (viewport,
    // CLI, thumbnails): a saved file reproduces exactly.
    let want_ss = job.config.escape.supersample.max(1);
    escape_renderer.resize(device, job.width, job.height, want_ss);
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
    let offsets = if extra > 1 {
        crate::escape::EscapeRenderer::sample_grid(extra)
    } else {
        vec![[0.0f32, 0.0]]
    };
    if extra > 1 {
        escape_renderer.begin_accumulation(device, queue, extra);
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
            &job.config.escape,
            renderer.palette_view(),
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
                &job.config.escape,
                renderer.palette_view(),
            );
            guard += 1;
            if guard > 4_000_000 {
                log::error!("escape chunk loop failed to settle; rendering what we have");
                break;
            }
        }
        // Fold this displaced render into the running average. The
        // encoder still holds the settling chunk's resolve, so the
        // fold is ordered after it.
        if extra > 1 {
            escape_renderer.accumulate_sample(device, queue, &mut encoder);
        }
        queue.submit(std::iter::once(encoder.finish()));
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    }
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Escape Tail"),
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
    }

    let escape_view = match escape_renderer.accumulated_view() {
        Some(v) if extra > 1 => v,
        _ => escape_renderer.output_view(),
    };
    if has_density_effects {
        let chain = effect_chain.as_mut().expect("built above: has_density_effects");
        let density_ran = chain.run_density_effects(
            device,
            queue,
            &mut encoder,
            escape_view,
            &job.config.density_effects,
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
            &job.config.color_effects,
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
        escape_renderer.destroy();
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
            .read_fractal_pixels(device, queue, job.transparent, job.config.background_color)
            .await
            .map_err(|e| RenderError::PixelReadFailed(e.to_string()))
    };

    // Readback completion proves every submission finished — the safe
    // destroy point for the per-call GPU objects, error path included.
    if let Some(chain) = &effect_chain {
        chain.destroy();
    }
    escape_renderer.destroy();
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
    })
}
