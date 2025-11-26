//! Animation export system
//!
//! Renders animation frames to PNG sequence for high-quality output.
//! Each frame is rendered to completion (max_iterations) for production quality.
//!
//! This module provides a reusable export engine used by both CLI and UI.
//!
//! ## Video Encoding
//!
//! Optionally encodes PNG sequence to video using ffmpeg (must be installed separately).
//! Supports H.264 (best compatibility), H.265 (better compression), and VP9 (royalty-free).

use std::path::PathBuf;

use crate::animation::{Animation, AnimationController};
use crate::config::{ConfigPath, FractalConfig, json_to_config_value};

/// Video codec options for ffmpeg encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCodec {
    /// H.264 (libx264) - Best compatibility, good compression
    H264,
    /// H.265/HEVC (libx265) - Better compression, less compatible
    #[default]
    H265,
    /// VP9 (libvpx-vp9) - Royalty-free, good for web
    VP9,
}

impl VideoCodec {
    /// Get ffmpeg codec argument
    pub fn ffmpeg_codec(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::VP9 => "libvpx-vp9",
        }
    }

    /// Get output file extension
    pub fn extension(&self) -> &'static str {
        match self {
            VideoCodec::H264 | VideoCodec::H265 => "mp4",
            VideoCodec::VP9 => "webm",
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264 (MP4)",
            VideoCodec::H265 => "H.265/HEVC (MP4)",
            VideoCodec::VP9 => "VP9 (WebM)",
        }
    }
}

/// Video encoding settings
#[derive(Debug, Clone)]
pub struct VideoEncodingSettings {
    /// Video codec to use
    pub codec: VideoCodec,
    /// Quality (CRF value: 0-51 for H.264/H.265, 0-63 for VP9; lower = better)
    /// Default: 18 (visually lossless for H.264)
    pub quality: u8,
    /// Output video filename (without extension, will be added based on codec)
    pub output_name: String,
    /// Delete PNG frames after successful video encoding
    pub cleanup_frames: bool,
}

impl Default for VideoEncodingSettings {
    fn default() -> Self {
        Self {
            codec: VideoCodec::default(),
            quality: 18,
            output_name: "animation".to_string(),
            cleanup_frames: false,
        }
    }
}

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
    /// Optional video encoding settings
    pub video_settings: Option<VideoEncodingSettings>,
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
        print!("\r  Frame {}/{} (anim: {:.2}s)...", frame + 1, total, time);
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

/// UI progress callback that updates shared state for display
///
/// Used by the GUI to show progress bar during export.
pub struct UiProgressCallback {
    progress: std::sync::Arc<std::sync::Mutex<crate::ui::animation_panel::ExportProgress>>,
    cancelled: bool,
    last_frame_time_ms: f64,
}

impl UiProgressCallback {
    pub fn new(progress: std::sync::Arc<std::sync::Mutex<crate::ui::animation_panel::ExportProgress>>) -> Self {
        Self {
            progress,
            cancelled: false,
            last_frame_time_ms: 0.0,
        }
    }
}

impl ExportProgressCallback for UiProgressCallback {
    fn on_frame_start(&mut self, frame: u32, total: u32, time: f64) {
        if let Ok(mut p) = self.progress.lock() {
            p.is_exporting = true;
            p.current_frame = frame;
            p.total_frames = total;
            p.status = format!("Rendering frame {} (anim: {:.2}s)", frame + 1, time);
        }
    }

    fn on_frame_complete(&mut self, frame: u32, total: u32, elapsed_ms: f64) {
        self.last_frame_time_ms = elapsed_ms;
        if let Ok(mut p) = self.progress.lock() {
            p.current_frame = frame + 1; // Frame is complete, so increment
            p.total_frames = total;
            p.seconds_per_frame = elapsed_ms / 1000.0;
            p.status = format!("Frame {}/{} complete ({:.1}s)", frame + 1, total, elapsed_ms / 1000.0);
        }
    }

    fn on_export_complete(&mut self, total_frames: u32, total_time_ms: f64) {
        if let Ok(mut p) = self.progress.lock() {
            p.is_exporting = false;
            p.current_frame = total_frames;
            p.total_frames = total_frames;
            p.status = format!("Export complete: {} frames in {:.1}s", total_frames, total_time_ms / 1000.0);
        }
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

// ============================================================================
// Video Encoding (ffmpeg)
// ============================================================================

/// Check if ffmpeg is available on the system
#[cfg(not(target_arch = "wasm32"))]
pub fn is_ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get ffmpeg version string (if available)
#[cfg(not(target_arch = "wasm32"))]
pub fn get_ffmpeg_version() -> Option<String> {
    let output = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // First line typically: "ffmpeg version X.Y.Z ..."
    stdout.lines().next().map(|s| s.to_string())
}

/// Result of video encoding
#[derive(Debug)]
pub struct VideoEncodeResult {
    /// Path to the output video file
    pub video_path: PathBuf,
    /// Number of PNG frames that were cleaned up (0 if cleanup disabled)
    pub frames_cleaned: u32,
    /// Encoding time in milliseconds
    pub encode_time_ms: f64,
}

/// Errors that can occur during video encoding
#[derive(Debug)]
pub enum VideoEncodeError {
    /// ffmpeg not found on system
    FfmpegNotFound,
    /// ffmpeg command failed
    FfmpegFailed(String),
    /// IO error
    IoError(std::io::Error),
    /// No frames found in output directory
    NoFramesFound,
}

impl std::fmt::Display for VideoEncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoEncodeError::FfmpegNotFound => write!(f, "ffmpeg not found. Please install ffmpeg and ensure it's in your PATH."),
            VideoEncodeError::FfmpegFailed(msg) => write!(f, "ffmpeg encoding failed: {}", msg),
            VideoEncodeError::IoError(e) => write!(f, "IO error: {}", e),
            VideoEncodeError::NoFramesFound => write!(f, "No PNG frames found in output directory"),
        }
    }
}

impl std::error::Error for VideoEncodeError {}

impl From<std::io::Error> for VideoEncodeError {
    fn from(e: std::io::Error) -> Self {
        VideoEncodeError::IoError(e)
    }
}

/// Encode PNG sequence to video using ffmpeg
///
/// # Arguments
/// * `frames_dir` - Directory containing frame_NNNN.png files
/// * `fps` - Frames per second
/// * `settings` - Video encoding settings
///
/// # Returns
/// * `Ok(VideoEncodeResult)` - Encoding succeeded
/// * `Err(VideoEncodeError)` - Encoding failed
#[cfg(not(target_arch = "wasm32"))]
pub fn encode_video(
    frames_dir: &std::path::Path,
    fps: u32,
    settings: &VideoEncodingSettings,
) -> Result<VideoEncodeResult, VideoEncodeError> {
    use std::time::Instant;

    let start = Instant::now();

    // Check ffmpeg availability
    if !is_ffmpeg_available() {
        return Err(VideoEncodeError::FfmpegNotFound);
    }

    // Verify frames exist
    let frame_pattern = frames_dir.join("frame_%04d.png");
    let first_frame = frames_dir.join("frame_0001.png");
    if !first_frame.exists() {
        return Err(VideoEncodeError::NoFramesFound);
    }

    // Build output path
    let video_path = frames_dir.join(format!("{}.{}", settings.output_name, settings.codec.extension()));

    // Build ffmpeg command
    let mut cmd = std::process::Command::new("ffmpeg");

    // Overwrite output without asking
    cmd.arg("-y");

    // Input settings
    cmd.arg("-framerate").arg(fps.to_string());
    cmd.arg("-i").arg(&frame_pattern);

    // Codec-specific settings
    match settings.codec {
        VideoCodec::H264 => {
            cmd.arg("-c:v").arg("libx264");
            cmd.arg("-crf").arg(settings.quality.to_string());
            cmd.arg("-preset").arg("medium");
            cmd.arg("-pix_fmt").arg("yuv420p"); // Maximum compatibility
        }
        VideoCodec::H265 => {
            cmd.arg("-c:v").arg("libx265");
            cmd.arg("-crf").arg(settings.quality.to_string());
            cmd.arg("-preset").arg("medium");
            cmd.arg("-pix_fmt").arg("yuv420p");
            // Suppress x265 logging
            cmd.arg("-x265-params").arg("log-level=error");
        }
        VideoCodec::VP9 => {
            cmd.arg("-c:v").arg("libvpx-vp9");
            cmd.arg("-crf").arg(settings.quality.to_string());
            cmd.arg("-b:v").arg("0"); // Use CRF mode
            cmd.arg("-pix_fmt").arg("yuv420p");
        }
    }

    // Output file
    cmd.arg(&video_path);

    // Run ffmpeg
    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VideoEncodeError::FfmpegFailed(stderr.to_string()));
    }

    // Clean up frames if requested
    let frames_cleaned = if settings.cleanup_frames {
        cleanup_frames(frames_dir)?
    } else {
        0
    };

    let encode_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(VideoEncodeResult {
        video_path,
        frames_cleaned,
        encode_time_ms,
    })
}

/// Delete all frame_NNNN.png files in a directory
#[cfg(not(target_arch = "wasm32"))]
fn cleanup_frames(frames_dir: &std::path::Path) -> Result<u32, std::io::Error> {
    let mut count = 0;

    for entry in std::fs::read_dir(frames_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // Match frame_NNNN.png pattern
            if name.starts_with("frame_") && name.ends_with(".png") {
                std::fs::remove_file(&path)?;
                count += 1;
            }
        }
    }

    Ok(count)
}
