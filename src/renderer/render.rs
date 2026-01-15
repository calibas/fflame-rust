//! Unified rendering API
//!
//! This module provides a single entry point for rendering fractal flames to pixels.
//! All export paths (CLI, WASM, thumbnails, video) should use this API to ensure
//! consistent behavior and reduce code duplication.

use egui_wgpu::wgpu::{CommandEncoderDescriptor, Device, Queue, TextureFormat};

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

    /// Time for animated effects (in seconds)
    /// None = static render (effects at time=0), Some(t) = animation frame at time t
    pub effect_time: Option<f32>,
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
            effect_time: None, // Static render (time=0)
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

    /// Set effect time for animated effects (animation export)
    /// For static exports, leave unset (defaults to time=0)
    pub fn with_effect_time(mut self, time: f32) -> Self {
        self.effect_time = Some(time);
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
    let start_time = web_time::Instant::now();

    let target = job.target_iterations.unwrap_or(job.config.max_iterations);

    log::info!(
        "Render: Starting {}x{}, target={} iterations",
        job.width,
        job.height,
        target
    );

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
            NUM_WORKGROUPS,
            job.iterations_per_thread,
            job.burn_in,
            job.config.zoom,
            job.config.pan_x,
            job.config.pan_y,
            job.config.rotation,
            job.config.camera_rotation_x,
            job.config.camera_rotation_y,
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

    // Set transparent mode if requested
    if job.transparent {
        renderer.set_transparent_mode(queue, true, job.config, job.iterations_per_thread);
    }

    // Create effect chain runner for post-processing effects
    let mut effect_chain = EffectChainRunner::new(device, job.width, job.height);

    // Set effect time: 0.0 for static exports, or provided time for animation
    effect_chain.set_time(job.effect_time.unwrap_or(0.0));

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

    if has_density_effects {
        let density_ran = effect_chain.run_density_effects(
            device,
            queue,
            &mut tonemap_encoder,
            renderer.get_accumulation_view(),
            &job.config.density_effects,
        );

        if density_ran {
            if let Some(density_output) = effect_chain.get_density_output() {
                renderer.tonemap_pass_with_input(device, &mut tonemap_encoder, density_output);
            } else {
                renderer.tonemap_pass(&mut tonemap_encoder);
            }
        } else {
            renderer.tonemap_pass(&mut tonemap_encoder);
        }
    } else {
        renderer.tonemap_pass(&mut tonemap_encoder);
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
