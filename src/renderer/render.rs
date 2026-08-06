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
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::NoPaletteFound => write!(f, "No palette found"),
            RenderError::PixelReadFailed(msg) => write!(f, "Failed to read pixels: {}", msg),
            RenderError::Cancelled => write!(f, "Render cancelled"),
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

    // Create effect chain runner for post-processing effects
    let mut effect_chain = EffectChainRunner::new(device, job.width, job.height);

    // Check for enabled effects
    let has_density_effects = EffectChainRunner::has_enabled_effects(&job.config.density_effects);
    let has_color_effects = EffectChainRunner::has_enabled_effects(&job.config.color_effects);

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
    effect_chain.reset_slots();

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
        let density_ran = effect_chain.run_density_effects(
            device,
            queue,
            &mut tonemap_encoder,
            pre_tonemap_view,
            &job.config.density_effects,
        );

        if density_ran {
            if let Some(density_output) = effect_chain.get_density_output() {
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
        let mut color_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Color Effects"),
        });

        let ran = effect_chain.run_color_effects(
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
    let (width, height, rgba_data) = if color_effects_ran {
        // Read from color effect output
        effect_chain
            .read_color_output_pixels(device, queue)
            .await
            .map_err(|e| RenderError::PixelReadFailed(e))?
    } else {
        // Read from renderer's fractal texture
        renderer
            .read_fractal_pixels(device, queue, job.transparent, job.config.background_color)
            .await
            .map_err(|e| RenderError::PixelReadFailed(e.to_string()))?
    };

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
