//! Animation export system
//!
//! Renders animation frames to PNG sequence for high-quality output.
//! Each frame is rendered to completion (max_iterations) for production quality.
//!
//! This module provides a reusable export engine used by both CLI and UI.

use std::path::PathBuf;

use crate::animation::{Animation, AnimationController};
use crate::config::{ConfigPath, FractalConfig, json_to_config_value};

/// Animation export configuration
#[derive(Clone)]
pub struct AnimationExportConfig {
    /// Base fractal config (will be modified by animation)
    pub config: FractalConfig,
    /// Animation to render
    pub animation: Animation,
    /// Output directory for frames
    pub output_dir: PathBuf,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Iterations per thread (GPU compute setting)
    pub iterations_per_thread: u32,
    /// Export transparent PNGs
    pub transparent: bool,
}

impl AnimationExportConfig {
    /// Calculate total number of frames
    pub fn total_frames(&self) -> u32 {
        (self.animation.duration * self.fps as f64).ceil() as u32
    }

    /// Calculate time for a given frame number
    pub fn frame_time(&self, frame: u32) -> f64 {
        frame as f64 / self.fps as f64
    }
}

/// Result of animation export
#[derive(Debug)]
pub struct AnimationExportResult {
    /// Total frames exported
    pub total_frames: u32,
    /// Total render time in milliseconds
    pub total_time_ms: f64,
    /// Average time per frame in milliseconds
    pub avg_frame_time_ms: f64,
    /// Output directory
    pub output_dir: PathBuf,
}

/// Errors that can occur during animation export
#[derive(Debug)]
pub enum AnimationExportError {
    /// GPU/device error
    GpuError(String),
    /// IO error (disk full, permissions, etc.)
    IoError(std::io::Error),
    /// Export was cancelled by user
    Cancelled,
    /// No animation loaded
    NoAnimation,
    /// Invalid configuration
    InvalidConfig(String),
}

impl std::fmt::Display for AnimationExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationExportError::GpuError(msg) => write!(f, "GPU error: {}", msg),
            AnimationExportError::IoError(e) => write!(f, "IO error: {}", e),
            AnimationExportError::Cancelled => write!(f, "Export cancelled"),
            AnimationExportError::NoAnimation => write!(f, "No animation loaded"),
            AnimationExportError::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
        }
    }
}

impl std::error::Error for AnimationExportError {}

impl From<std::io::Error> for AnimationExportError {
    fn from(e: std::io::Error) -> Self {
        AnimationExportError::IoError(e)
    }
}

/// Progress callback for export operations
///
/// Implement this trait to receive progress updates during export.
/// Used by both CLI (prints to stdout) and UI (updates progress bar).
pub trait ExportProgressCallback {
    /// Called when a frame starts rendering
    fn on_frame_start(&mut self, frame: u32, total: u32, time: f64);

    /// Called when a frame completes
    fn on_frame_complete(&mut self, frame: u32, total: u32, elapsed_ms: f64);

    /// Called when export is fully complete
    fn on_export_complete(&mut self, total_frames: u32, total_time_ms: f64);

    /// Check if export should be cancelled
    fn is_cancelled(&self) -> bool;
}

/// Simple CLI progress callback that prints to stdout
pub struct CliProgressCallback {
    cancelled: bool,
}

impl CliProgressCallback {
    pub fn new() -> Self {
        Self { cancelled: false }
    }
}

impl Default for CliProgressCallback {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportProgressCallback for CliProgressCallback {
    fn on_frame_start(&mut self, frame: u32, total: u32, time: f64) {
        print!("\r  Frame {}/{} (time: {:.2}s)...", frame + 1, total, time);
        use std::io::Write;
        std::io::stdout().flush().ok();
    }

    fn on_frame_complete(&mut self, frame: u32, total: u32, elapsed_ms: f64) {
        let percent = ((frame + 1) as f64 / total as f64) * 100.0;
        let remaining = (total - frame - 1) as f64 * elapsed_ms / 1000.0;
        print!("\r  Frame {}/{} ({:.1}%) - {:.2}s/frame - ETA: {:.0}s    ",
            frame + 1, total, percent, elapsed_ms / 1000.0, remaining);
        use std::io::Write;
        std::io::stdout().flush().ok();
    }

    fn on_export_complete(&mut self, total_frames: u32, total_time_ms: f64) {
        println!("\n  Export complete: {} frames in {:.1}s", total_frames, total_time_ms / 1000.0);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

/// Apply animation values to a FractalConfig
///
/// Takes the evaluated animation values and applies them to the config.
pub fn apply_animation_values(
    config: &mut FractalConfig,
    values: &[(String, serde_json::Value)],
) {
    for (path_str, json_value) in values {
        // Parse the path string to ConfigPath
        if let Some(path) = ConfigPath::from_string_key(path_str) {
            // Convert JSON value to ConfigValue
            if let Some(config_value) = json_to_config_value(json_value, &path) {
                // Apply to config
                apply_config_value(config, &path, &config_value);
            }
        } else {
            log::warn!("Unknown animation path: {}", path_str);
        }
    }
}

/// Apply a single ConfigValue to a FractalConfig
fn apply_config_value(
    config: &mut FractalConfig,
    path: &ConfigPath,
    value: &crate::config::ConfigValue,
) {
    use crate::config::ConfigValue;

    match (path, value) {
        // View parameters
        (ConfigPath::Zoom, ConfigValue::Float(v)) => config.zoom = *v,
        (ConfigPath::Pan, ConfigValue::Vec2(x, y)) => {
            config.pan_x = *x;
            config.pan_y = *y;
        }
        (ConfigPath::Rotation, ConfigValue::Float(v)) => config.rotation = *v,
        (ConfigPath::CameraRotationX, ConfigValue::Float(v)) => config.camera_rotation_x = *v,
        (ConfigPath::CameraRotationY, ConfigValue::Float(v)) => config.camera_rotation_y = *v,
        (ConfigPath::CameraZ, ConfigValue::Float(v)) => config.camera_z = *v,

        // Tone mapping
        (ConfigPath::Exposure, ConfigValue::Float(v)) => config.exposure = *v,
        (ConfigPath::Gamma, ConfigValue::Float(v)) => config.gamma = *v,
        (ConfigPath::GammaThreshold, ConfigValue::Float(v)) => config.gamma_threshold = *v,
        (ConfigPath::Brightness, ConfigValue::Float(v)) => config.brightness = *v,
        (ConfigPath::Vibrancy, ConfigValue::Float(v)) => config.vibrancy = *v,
        (ConfigPath::Saturation, ConfigValue::Float(v)) => config.saturation = *v,
        (ConfigPath::HueShift, ConfigValue::Float(v)) => config.hue_shift = *v,
        (ConfigPath::ValueScale, ConfigValue::Float(v)) => config.value_scale = *v,
        (ConfigPath::DensityScale, ConfigValue::Float(v)) => config.density_scale = *v,

        // Color
        (ConfigPath::PaletteRotation, ConfigValue::Float(v)) => config.palette_rotation = *v,
        (ConfigPath::SpeedFactor, ConfigValue::Float(v)) => config.speed_factor = *v,
        (ConfigPath::BackgroundColor, ConfigValue::ColorRgb(rgb)) => config.background_color = *rgb,

        // Transform parameters
        (ConfigPath::TransformWeight { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.weight = *v;
            }
        }
        (ConfigPath::TransformColor { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.color = *v;
            }
        }
        (ConfigPath::TransformColorSpeed { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.color_speed = *v;
            }
        }
        (ConfigPath::TransformOpacity { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.opacity = *v;
            }
        }
        (ConfigPath::TransformAffine { index, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                use crate::config::AffineParam;
                match param {
                    AffineParam::A => xform.a = *v,
                    AffineParam::B => xform.b = *v,
                    AffineParam::C => xform.c = *v,
                    AffineParam::D => xform.d = *v,
                    AffineParam::E => xform.e = *v,
                    AffineParam::F => xform.f = *v,
                    AffineParam::G => xform.g = *v,
                }
            }
        }
        (ConfigPath::TransformVariation { index, variation }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.variations.insert(variation.clone(), *v);
            }
        }
        (ConfigPath::TransformVariationParam { index, variation, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                let key = format!("{}:{}", variation, param);
                xform.variation_params.insert(key, *v);
            }
        }

        // Other parameters can be added as needed
        _ => {
            log::debug!("Unhandled animation path: {:?}", path);
        }
    }
}

/// Export animation to PNG sequence (desktop only)
#[cfg(not(target_arch = "wasm32"))]
pub async fn export_animation(
    export_config: AnimationExportConfig,
    progress: &mut dyn ExportProgressCallback,
) -> Result<AnimationExportResult, AnimationExportError> {
    use crate::renderer::compute_kernel::FlameRenderer;
    use crate::scene::palette::PaletteLibrary;
    use std::time::Instant;

    let total_start = Instant::now();
    let total_frames = export_config.total_frames();

    // Ensure output directory exists
    std::fs::create_dir_all(&export_config.output_dir)?;

    // Create GPU resources (reused across frames)
    let instance = egui_wgpu::wgpu::Instance::new(&egui_wgpu::wgpu::InstanceDescriptor {
        backends: egui_wgpu::wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&egui_wgpu::wgpu::RequestAdapterOptions {
            power_preference: egui_wgpu::wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to find adapter: {:?}", e)))?;

    let (device, queue) = adapter
        .request_device(
            &egui_wgpu::wgpu::DeviceDescriptor {
                label: Some("Animation Export Device"),
                required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
                required_limits: egui_wgpu::wgpu::Limits::default(),
                memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
                ..Default::default()
            },
        )
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to create device: {:?}", e)))?;

    // Create animation controller for evaluation
    let mut controller = AnimationController::new();
    controller.load(export_config.animation.clone());

    // Get palette library
    let palette_library = PaletteLibrary::new();

    // Render each frame
    for frame in 0..total_frames {
        if progress.is_cancelled() {
            return Err(AnimationExportError::Cancelled);
        }

        let frame_start = Instant::now();
        let time = export_config.frame_time(frame);

        progress.on_frame_start(frame, total_frames, time);

        // Evaluate animation at this time
        let values = controller.evaluate_at_time(time);

        // Create config copy and apply animation values
        let mut frame_config = export_config.config.clone();
        apply_animation_values(&mut frame_config, &values);

        // Create fresh renderer for this frame
        let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = FlameRenderer::new(
            &device, &queue, surface_format,
            export_config.width, export_config.height,
            &frame_config.flame,
        );

        // Load config
        let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Frame Setup Encoder"),
        });

        let palette = frame_config.palette.as_ref()
            .or_else(|| palette_library.get(frame_config.palette_index))
            .ok_or_else(|| AnimationExportError::InvalidConfig("No palette found".to_string()))?;

        renderer.load_config(&device, &mut encoder, &queue, &frame_config, palette, export_config.iterations_per_thread);
        queue.submit(std::iter::once(encoder.finish()));

        // Render until max_iterations
        render_frame_to_completion(
            &device, &queue, &mut renderer,
            &frame_config,
            export_config.iterations_per_thread,
        ).await;

        // Set transparent mode if requested
        if export_config.transparent {
            renderer.set_transparent_mode(&queue, true, &frame_config, export_config.iterations_per_thread);
        }

        // Tonemap pass
        let mut final_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Final Tonemap"),
        });
        renderer.tonemap_pass(&mut final_encoder);
        queue.submit(std::iter::once(final_encoder.finish()));

        // Read pixels
        let (width, height, rgba_data) = renderer
            .read_fractal_pixels(&device, &queue, export_config.transparent, frame_config.background_color)
            .await
            .map_err(|e| AnimationExportError::GpuError(format!("Failed to read pixels: {}", e)))?;

        // Encode PNG
        let png_data = crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, None)
            .map_err(|e| AnimationExportError::GpuError(format!("Failed to encode PNG: {}", e)))?;

        // Save frame
        let frame_path = export_config.output_dir.join(format!("frame_{:04}.png", frame + 1));
        std::fs::write(&frame_path, png_data)?;

        let frame_elapsed = frame_start.elapsed().as_secs_f64() * 1000.0;
        progress.on_frame_complete(frame, total_frames, frame_elapsed);
    }

    let total_time_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let avg_frame_time_ms = total_time_ms / total_frames as f64;

    progress.on_export_complete(total_frames, total_time_ms);

    Ok(AnimationExportResult {
        total_frames,
        total_time_ms,
        avg_frame_time_ms,
        output_dir: export_config.output_dir,
    })
}

/// Render a single frame to completion (until max_iterations reached)
#[cfg(not(target_arch = "wasm32"))]
async fn render_frame_to_completion(
    device: &egui_wgpu::wgpu::Device,
    queue: &egui_wgpu::wgpu::Queue,
    renderer: &mut crate::renderer::compute_kernel::FlameRenderer,
    config: &FractalConfig,
    iterations_per_thread: u32,
) {
    const NUM_WORKGROUPS: u32 = 128;
    const THREADS_PER_WORKGROUP: u64 = 64;
    const BATCH_SIZE: u32 = 4;

    let mut total_rendered = 0u64;
    let target = config.max_iterations;
    let mut batch_frame_count = 0;

    while total_rendered < target {
        let mut encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Render Frame"),
        });

        let clear_histogram = batch_frame_count == 0;

        renderer.compute_pass(
            &mut encoder,
            queue,
            NUM_WORKGROUPS,
            iterations_per_thread,
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_z,
            config.speed_factor,
            clear_histogram,
        );

        let samples_this_frame = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_thread as u64;
        total_rendered += samples_this_frame;
        batch_frame_count += 1;

        let should_accumulate = batch_frame_count >= BATCH_SIZE;
        if should_accumulate {
            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
            renderer.accumulate_pass(&mut encoder, queue, device, total_samples_in_batch);
            batch_frame_count = 0;
        }

        queue.submit(std::iter::once(encoder.finish()));

        if total_rendered >= target {
            // Final accumulation if we have partial batch
            if batch_frame_count > 0 {
                let mut final_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                    label: Some("Final Batch Accumulation"),
                });
                let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                renderer.accumulate_pass(&mut final_encoder, queue, device, total_samples_in_batch);
                queue.submit(std::iter::once(final_encoder.finish()));
            }
            break;
        }
    }
}
