//! Animation export system
//!
//! Renders animation frames directly to video using FFmpeg pipe.
//! Raw RGBA pixels are piped directly to FFmpeg's stdin, avoiding PNG encoding/decoding.
//!
//! This module provides a reusable export engine used by both CLI and UI.
//!
//! ## Video Encoding
//!
//! Pipes raw pixel data directly to FFmpeg for efficient video encoding.
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

/// Hardware acceleration options for video encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HardwareAccel {
    /// Software encoding (CPU) - always available, slower but compatible
    #[default]
    None,
    /// NVIDIA NVENC - fast hardware encoding on NVIDIA GPUs
    Nvenc,
    /// Intel Quick Sync Video - hardware encoding on Intel CPUs with integrated graphics
    Qsv,
    /// AMD AMF - hardware encoding on AMD GPUs
    Amf,
    /// Apple VideoToolbox - hardware encoding on macOS
    VideoToolbox,
}

impl HardwareAccel {
    /// Get the ffmpeg encoder name for this hardware acceleration + codec combination
    /// Returns None if the combination is not supported
    pub fn encoder_for_codec(&self, codec: VideoCodec) -> Option<&'static str> {
        match (self, codec) {
            // Software encoders
            (HardwareAccel::None, VideoCodec::H264) => Some("libx264"),
            (HardwareAccel::None, VideoCodec::H265) => Some("libx265"),
            (HardwareAccel::None, VideoCodec::VP9) => Some("libvpx-vp9"),

            // NVIDIA NVENC
            (HardwareAccel::Nvenc, VideoCodec::H264) => Some("h264_nvenc"),
            (HardwareAccel::Nvenc, VideoCodec::H265) => Some("hevc_nvenc"),
            (HardwareAccel::Nvenc, VideoCodec::VP9) => None, // NVENC doesn't support VP9

            // Intel Quick Sync
            (HardwareAccel::Qsv, VideoCodec::H264) => Some("h264_qsv"),
            (HardwareAccel::Qsv, VideoCodec::H265) => Some("hevc_qsv"),
            (HardwareAccel::Qsv, VideoCodec::VP9) => Some("vp9_qsv"),

            // AMD AMF
            (HardwareAccel::Amf, VideoCodec::H264) => Some("h264_amf"),
            (HardwareAccel::Amf, VideoCodec::H265) => Some("hevc_amf"),
            (HardwareAccel::Amf, VideoCodec::VP9) => None, // AMF doesn't support VP9

            // Apple VideoToolbox
            (HardwareAccel::VideoToolbox, VideoCodec::H264) => Some("h264_videotoolbox"),
            (HardwareAccel::VideoToolbox, VideoCodec::H265) => Some("hevc_videotoolbox"),
            (HardwareAccel::VideoToolbox, VideoCodec::VP9) => None, // VideoToolbox doesn't support VP9
        }
    }

    /// Check if this hardware acceleration supports the given codec
    pub fn supports_codec(&self, codec: VideoCodec) -> bool {
        self.encoder_for_codec(codec).is_some()
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            HardwareAccel::None => "Software (CPU)",
            HardwareAccel::Nvenc => "NVIDIA NVENC",
            HardwareAccel::Qsv => "Intel Quick Sync",
            HardwareAccel::Amf => "AMD AMF",
            HardwareAccel::VideoToolbox => "Apple VideoToolbox",
        }
    }

    /// Get all available hardware acceleration options
    pub fn all() -> &'static [HardwareAccel] {
        &[
            HardwareAccel::None,
            HardwareAccel::Nvenc,
            HardwareAccel::Qsv,
            HardwareAccel::Amf,
            HardwareAccel::VideoToolbox,
        ]
    }
}

/// Video encoding settings
#[derive(Debug, Clone)]
pub struct VideoEncodingSettings {
    /// Video codec to use
    pub codec: VideoCodec,
    /// Hardware acceleration option
    pub hardware_accel: HardwareAccel,
    /// Quality (CRF value: 0-51 for H.264/H.265, 0-63 for VP9; lower = better)
    /// Default: 18 (visually lossless for H.264)
    pub quality: u8,
}

impl Default for VideoEncodingSettings {
    fn default() -> Self {
        Self {
            codec: VideoCodec::default(),
            hardware_accel: HardwareAccel::default(),
            quality: 18,
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
    /// Output video file path
    pub output_path: PathBuf,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Iterations per thread (GPU compute setting)
    pub iterations_per_thread: u32,
    /// Video encoding settings
    pub video_settings: VideoEncodingSettings,
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
    /// Output video file path
    pub output_path: PathBuf,
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
    /// FFmpeg not found
    FfmpegNotFound,
    /// FFmpeg process failed
    FfmpegFailed(String),
}

impl std::fmt::Display for AnimationExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationExportError::GpuError(msg) => write!(f, "GPU error: {}", msg),
            AnimationExportError::IoError(e) => write!(f, "IO error: {}", e),
            AnimationExportError::Cancelled => write!(f, "Export cancelled"),
            AnimationExportError::NoAnimation => write!(f, "No animation loaded"),
            AnimationExportError::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
            AnimationExportError::FfmpegNotFound => write!(f, "FFmpeg not found. Please install FFmpeg and ensure it's in your PATH."),
            AnimationExportError::FfmpegFailed(msg) => write!(f, "FFmpeg encoding failed: {}", msg),
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
        (ConfigPath::PanX, ConfigValue::Float(v)) => config.pan_x = *v,
        (ConfigPath::PanY, ConfigValue::Float(v)) => config.pan_y = *v,
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
        (ConfigPath::BackgroundColorR, ConfigValue::Float(v)) => config.background_color[0] = *v,
        (ConfigPath::BackgroundColorG, ConfigValue::Float(v)) => config.background_color[1] = *v,
        (ConfigPath::BackgroundColorB, ConfigValue::Float(v)) => config.background_color[2] = *v,

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

        // High-level transform operations
        (ConfigPath::TransformOriginX { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.set_origin_x(*v);
            }
        }
        (ConfigPath::TransformOriginY { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.set_origin_y(*v);
            }
        }
        (ConfigPath::TransformRotation { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.set_rotation(*v);
            }
        }
        (ConfigPath::TransformScale { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = config.flame.transforms.get_mut(*index) {
                xform.set_scale(*v);
            }
        }

        // Final Transform parameters
        (ConfigPath::FinalTransformAffine { param }, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                use crate::config::AffineParam;
                match param {
                    AffineParam::A => final_xform.a = *v,
                    AffineParam::B => final_xform.b = *v,
                    AffineParam::C => final_xform.c = *v,
                    AffineParam::D => final_xform.d = *v,
                    AffineParam::E => final_xform.e = *v,
                    AffineParam::F => final_xform.f = *v,
                    AffineParam::G => final_xform.g = *v,
                }
            }
        }
        (ConfigPath::FinalTransformColor, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                final_xform.color = *v;
            }
        }
        (ConfigPath::FinalTransformColorSpeed, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                final_xform.color_speed = *v;
            }
        }
        (ConfigPath::FinalTransformVariation { variation }, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                final_xform.variations.insert(variation.clone(), *v);
            }
        }
        (ConfigPath::FinalTransformVariationParam { variation, param }, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                let key = format!("{}:{}", variation, param);
                final_xform.variation_params.insert(key, *v);
            }
        }
        (ConfigPath::FinalTransformOriginX, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                final_xform.set_origin_x(*v);
            }
        }
        (ConfigPath::FinalTransformOriginY, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                final_xform.set_origin_y(*v);
            }
        }
        (ConfigPath::FinalTransformRotation, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                final_xform.set_rotation(*v);
            }
        }
        (ConfigPath::FinalTransformScale, ConfigValue::Float(v)) => {
            if let Some(ref mut final_xform) = config.flame.final_transform {
                final_xform.set_scale(*v);
            }
        }

        // Other parameters can be added as needed
        _ => {
            log::debug!("Unhandled animation path: {:?}", path);
        }
    }
}

// ============================================================================
// FFmpeg Pipe-Based Video Export
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

/// Check if a specific encoder is available in FFmpeg
#[cfg(not(target_arch = "wasm32"))]
pub fn is_encoder_available(encoder: &str) -> bool {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .ok();

    if let Some(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Encoder list format: " V..... encoder_name   Description"
        // Look for the encoder name as a word
        stdout.lines().any(|line| {
            line.split_whitespace()
                .nth(1) // Second column is encoder name
                .map(|name| name == encoder)
                .unwrap_or(false)
        })
    } else {
        false
    }
}

/// Check which hardware acceleration options are available on this system
#[cfg(not(target_arch = "wasm32"))]
pub fn get_available_hardware_accels() -> Vec<(HardwareAccel, Vec<VideoCodec>)> {
    let mut available = Vec::new();

    // Software is always available
    available.push((HardwareAccel::None, vec![VideoCodec::H264, VideoCodec::H265, VideoCodec::VP9]));

    // Check NVENC (NVIDIA)
    let nvenc_codecs: Vec<VideoCodec> = [
        ("h264_nvenc", VideoCodec::H264),
        ("hevc_nvenc", VideoCodec::H265),
    ]
    .iter()
    .filter(|(enc, _)| is_encoder_available(enc))
    .map(|(_, codec)| *codec)
    .collect();

    if !nvenc_codecs.is_empty() {
        available.push((HardwareAccel::Nvenc, nvenc_codecs));
    }

    // Check QSV (Intel)
    let qsv_codecs: Vec<VideoCodec> = [
        ("h264_qsv", VideoCodec::H264),
        ("hevc_qsv", VideoCodec::H265),
        ("vp9_qsv", VideoCodec::VP9),
    ]
    .iter()
    .filter(|(enc, _)| is_encoder_available(enc))
    .map(|(_, codec)| *codec)
    .collect();

    if !qsv_codecs.is_empty() {
        available.push((HardwareAccel::Qsv, qsv_codecs));
    }

    // Check AMF (AMD)
    let amf_codecs: Vec<VideoCodec> = [
        ("h264_amf", VideoCodec::H264),
        ("hevc_amf", VideoCodec::H265),
    ]
    .iter()
    .filter(|(enc, _)| is_encoder_available(enc))
    .map(|(_, codec)| *codec)
    .collect();

    if !amf_codecs.is_empty() {
        available.push((HardwareAccel::Amf, amf_codecs));
    }

    // Check VideoToolbox (macOS)
    let vt_codecs: Vec<VideoCodec> = [
        ("h264_videotoolbox", VideoCodec::H264),
        ("hevc_videotoolbox", VideoCodec::H265),
    ]
    .iter()
    .filter(|(enc, _)| is_encoder_available(enc))
    .map(|(_, codec)| *codec)
    .collect();

    if !vt_codecs.is_empty() {
        available.push((HardwareAccel::VideoToolbox, vt_codecs));
    }

    available
}

/// Print available hardware acceleration options to stdout
#[cfg(not(target_arch = "wasm32"))]
pub fn print_available_encoders() {
    println!("Checking available FFmpeg encoders...\n");

    if let Some(version) = get_ffmpeg_version() {
        println!("{}\n", version);
    }

    let available = get_available_hardware_accels();

    for (accel, codecs) in &available {
        let codec_names: Vec<&str> = codecs.iter().map(|c| c.display_name()).collect();
        println!("  {} - {}", accel.display_name(), codec_names.join(", "));
    }

    println!();

    // Also check specific encoders for debugging
    let encoders_to_check = [
        "libx264", "libx265", "libvpx-vp9",
        "h264_nvenc", "hevc_nvenc",
        "h264_qsv", "hevc_qsv", "vp9_qsv",
        "h264_amf", "hevc_amf",
        "h264_videotoolbox", "hevc_videotoolbox",
    ];

    println!("Encoder availability:");
    for encoder in encoders_to_check {
        let status = if is_encoder_available(encoder) { "✓" } else { "✗" };
        println!("  {} {}", status, encoder);
    }
}

/// Export animation directly to video via FFmpeg pipe (desktop only)
///
/// This pipes raw RGBA pixel data directly to FFmpeg's stdin, avoiding:
/// - PNG encoding (CPU-intensive compression)
/// - Disk I/O (writing/reading thousands of files)
/// - PNG decoding (FFmpeg decompressing what we just compressed)
/// - Disk space (no temp files)
#[cfg(not(target_arch = "wasm32"))]
pub async fn export_animation(
    export_config: AnimationExportConfig,
    progress: &mut dyn ExportProgressCallback,
) -> Result<AnimationExportResult, AnimationExportError> {
    use crate::renderer::compute_kernel::FlameRenderer;
    use crate::scene::palette::PaletteLibrary;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let total_start = Instant::now();
    let total_frames = export_config.total_frames();

    // Check ffmpeg availability first
    if !is_ffmpeg_available() {
        return Err(AnimationExportError::FfmpegNotFound);
    }

    // Ensure output directory exists
    if let Some(parent) = export_config.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

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

    // Build FFmpeg command for piped raw video input
    let mut ffmpeg = Command::new("ffmpeg");

    // Overwrite output without asking
    ffmpeg.arg("-y");

    // Input format: raw RGBA video from stdin
    ffmpeg.arg("-f").arg("rawvideo");
    ffmpeg.arg("-pix_fmt").arg("rgba");
    ffmpeg.arg("-s").arg(format!("{}x{}", export_config.width, export_config.height));
    ffmpeg.arg("-r").arg(export_config.fps.to_string());
    ffmpeg.arg("-i").arg("-"); // Read from stdin

    // Codec and hardware acceleration settings
    let settings = &export_config.video_settings;
    let encoder = settings.hardware_accel.encoder_for_codec(settings.codec)
        .expect("Invalid hardware accel + codec combination");

    ffmpeg.arg("-c:v").arg(encoder);

    // Quality and codec-specific settings
    let is_hardware = settings.hardware_accel != HardwareAccel::None;

    match settings.codec {
        VideoCodec::H264 => {
            if is_hardware {
                // Hardware encoders use different quality parameters
                match settings.hardware_accel {
                    HardwareAccel::Nvenc => {
                        // NVENC uses -rc vbr -cq for constant quality (0-51)
                        ffmpeg.arg("-rc").arg("vbr");
                        ffmpeg.arg("-cq").arg(settings.quality.to_string());
                        ffmpeg.arg("-preset").arg("p4"); // Balanced preset
                    }
                    HardwareAccel::Qsv => {
                        // QSV uses -global_quality for CQP mode
                        ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
                        ffmpeg.arg("-preset").arg("medium");
                    }
                    HardwareAccel::Amf => {
                        // AMF uses -qp for constant QP mode
                        ffmpeg.arg("-rc").arg("cqp");
                        ffmpeg.arg("-qp").arg(settings.quality.to_string());
                    }
                    HardwareAccel::VideoToolbox => {
                        // VideoToolbox uses -q:v for quality (1-100, higher = better)
                        // Convert CRF-style (lower=better) to VT-style (higher=better)
                        let vt_quality = 100 - (settings.quality as i32 * 2).min(100).max(0);
                        ffmpeg.arg("-q:v").arg(vt_quality.to_string());
                    }
                    HardwareAccel::None => unreachable!(),
                }
            } else {
                ffmpeg.arg("-crf").arg(settings.quality.to_string());
                ffmpeg.arg("-preset").arg("medium");
            }
            ffmpeg.arg("-pix_fmt").arg("yuv420p"); // Maximum compatibility
        }
        VideoCodec::H265 => {
            if is_hardware {
                match settings.hardware_accel {
                    HardwareAccel::Nvenc => {
                        // NVENC uses -rc vbr -cq for constant quality (0-51)
                        ffmpeg.arg("-rc").arg("vbr");
                        ffmpeg.arg("-cq").arg(settings.quality.to_string());
                        ffmpeg.arg("-preset").arg("p4");
                    }
                    HardwareAccel::Qsv => {
                        ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
                        ffmpeg.arg("-preset").arg("medium");
                    }
                    HardwareAccel::Amf => {
                        ffmpeg.arg("-rc").arg("cqp");
                        ffmpeg.arg("-qp").arg(settings.quality.to_string());
                    }
                    HardwareAccel::VideoToolbox => {
                        let vt_quality = 100 - (settings.quality as i32 * 2).min(100).max(0);
                        ffmpeg.arg("-q:v").arg(vt_quality.to_string());
                    }
                    HardwareAccel::None => unreachable!(),
                }
            } else {
                ffmpeg.arg("-crf").arg(settings.quality.to_string());
                ffmpeg.arg("-preset").arg("medium");
                ffmpeg.arg("-x265-params").arg("log-level=error");
            }
            ffmpeg.arg("-pix_fmt").arg("yuv420p");
        }
        VideoCodec::VP9 => {
            // VP9 only supports software or QSV
            if settings.hardware_accel == HardwareAccel::Qsv {
                ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
            } else {
                ffmpeg.arg("-crf").arg(settings.quality.to_string());
                ffmpeg.arg("-b:v").arg("0"); // Use CRF mode
            }
            ffmpeg.arg("-pix_fmt").arg("yuv420p");
        }
    }

    // Output file
    ffmpeg.arg(&export_config.output_path);

    // Configure stdin pipe
    ffmpeg.stdin(Stdio::piped());
    ffmpeg.stdout(Stdio::null());
    ffmpeg.stderr(Stdio::piped());

    // Spawn FFmpeg process
    let mut child = ffmpeg.spawn()
        .map_err(|e| AnimationExportError::FfmpegFailed(format!("Failed to spawn ffmpeg: {}", e)))?;

    let mut stdin = child.stdin.take()
        .ok_or_else(|| AnimationExportError::FfmpegFailed("Failed to open ffmpeg stdin".to_string()))?;

    // Render each frame and pipe to FFmpeg
    for frame in 0..total_frames {
        if progress.is_cancelled() {
            // Kill FFmpeg process on cancel
            let _ = child.kill();
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

        renderer.load_config(&device, &mut encoder, &queue, &frame_config, palette, export_config.iterations_per_thread, 20); // burn_in default
        queue.submit(std::iter::once(encoder.finish()));

        // Render until max_iterations
        render_frame_to_completion(
            &device, &queue, &mut renderer,
            &frame_config,
            export_config.iterations_per_thread,
        ).await;

        // Tonemap pass
        let mut final_encoder = device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Final Tonemap"),
        });
        renderer.tonemap_pass(&mut final_encoder);
        queue.submit(std::iter::once(final_encoder.finish()));

        // Read raw RGBA pixels (no PNG encoding!)
        let (_width, _height, rgba_data) = renderer
            .read_fractal_pixels(&device, &queue, false, frame_config.background_color)
            .await
            .map_err(|e| AnimationExportError::GpuError(format!("Failed to read pixels: {}", e)))?;

        // Write raw RGBA data directly to FFmpeg stdin
        if let Err(e) = stdin.write_all(&rgba_data) {
            // Try to get FFmpeg's stderr to understand why it failed
            drop(stdin); // Close stdin so FFmpeg terminates
            let output = child.wait_with_output().ok();
            let stderr = output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                .unwrap_or_default();

            let error_msg = if stderr.is_empty() {
                format!("Failed to write frame to ffmpeg: {}", e)
            } else {
                format!("FFmpeg error: {}", stderr.trim())
            };

            return Err(AnimationExportError::FfmpegFailed(error_msg));
        }

        let frame_elapsed = frame_start.elapsed().as_secs_f64() * 1000.0;
        progress.on_frame_complete(frame, total_frames, frame_elapsed);
    }

    // Close stdin to signal end of input
    drop(stdin);

    // Wait for FFmpeg to finish
    let output = child.wait_with_output()
        .map_err(|e| AnimationExportError::FfmpegFailed(format!("Failed to wait for ffmpeg: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AnimationExportError::FfmpegFailed(format!("FFmpeg error: {}", stderr.trim())));
    }

    let total_time_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let avg_frame_time_ms = total_time_ms / total_frames as f64;

    progress.on_export_complete(total_frames, total_time_ms);

    Ok(AnimationExportResult {
        total_frames,
        total_time_ms,
        avg_frame_time_ms,
        output_path: export_config.output_path,
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
            20, // burn_in default
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

/// Timing statistics for export performance analysis
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default)]
pub struct ExportTimingStats {
    /// Time spent in GPU render dispatch
    pub render_time_ms: f64,
    /// Time spent waiting for buffer map
    pub map_wait_time_ms: f64,
    /// Time spent copying from mapped buffer
    pub copy_time_ms: f64,
    /// Time spent in channel send (waiting for writer thread)
    pub channel_send_time_ms: f64,
    /// Total frame time
    pub total_frame_time_ms: f64,
    /// Number of frames processed
    pub frame_count: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl ExportTimingStats {
    pub fn log_summary(&self) {
        if self.frame_count == 0 {
            return;
        }
        let n = self.frame_count as f64;
        println!("\n=== Export Timing Summary ({} frames) ===", self.frame_count);
        println!("  Render dispatch:  {:>8.2} ms avg", self.render_time_ms / n);
        println!("  Buffer map wait:  {:>8.2} ms avg", self.map_wait_time_ms / n);
        println!("  Buffer copy:      {:>8.2} ms avg", self.copy_time_ms / n);
        println!("  Channel send:     {:>8.2} ms avg", self.channel_send_time_ms / n);
        println!("  Total frame:      {:>8.2} ms avg", self.total_frame_time_ms / n);
        println!("  Effective FPS:    {:>8.2}", 1000.0 / (self.total_frame_time_ms / n));
        println!("==========================================");
    }
}

/// Export animation with batched frame rendering
///
/// Renders multiple frames to separate buffers before reading any back.
/// This keeps the GPU saturated while we process completed frames.
#[cfg(not(target_arch = "wasm32"))]
pub async fn export_animation_fast(
    export_config: AnimationExportConfig,
    progress: &mut dyn ExportProgressCallback,
) -> Result<AnimationExportResult, AnimationExportError> {
    use crate::renderer::compute_kernel::FlameRenderer;
    use crate::scene::palette::PaletteLibrary;
    use egui_wgpu::wgpu::{
        self, BufferDescriptor, BufferUsages, CommandEncoderDescriptor,
        Extent3d, MapMode, Origin3d, PollType, TextureAspect,
        TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo,
        COPY_BYTES_PER_ROW_ALIGNMENT,
    };
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Instant;

    let total_start = Instant::now();
    let total_frames = export_config.total_frames();
    let mut timing_stats = ExportTimingStats::default();

    // Single buffer approach - simple and reliable

    // Check ffmpeg availability first
    if !is_ffmpeg_available() {
        return Err(AnimationExportError::FfmpegNotFound);
    }

    // Ensure output directory exists
    if let Some(parent) = export_config.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create GPU resources
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to find adapter: {:?}", e)))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Animation Export Device"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        })
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to create device: {:?}", e)))?;

    // Create animation controller
    let mut controller = AnimationController::new();
    controller.load(export_config.animation.clone());

    // Get palette library
    let palette_library = PaletteLibrary::new();

    // Calculate buffer dimensions
    let bytes_per_pixel = 4u32; // RGBA8
    let unpadded_bytes_per_row = export_config.width * bytes_per_pixel;
    let align = COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
    let buffer_size = (padded_bytes_per_row * export_config.height) as u64;
    let output_size = (export_config.width * export_config.height * bytes_per_pixel) as usize;

    // Create single staging buffer
    let staging_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Spawn FFmpeg writer thread
    let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(4);

    // Build FFmpeg command
    let ffmpeg_args = build_ffmpeg_args(&export_config);
    let output_path = export_config.output_path.clone();

    let writer_handle = std::thread::spawn(move || -> Result<(), String> {
        let mut ffmpeg = Command::new("ffmpeg");
        for arg in &ffmpeg_args {
            ffmpeg.arg(arg);
        }
        ffmpeg.arg(&output_path);
        ffmpeg.stdin(Stdio::piped());
        ffmpeg.stdout(Stdio::null());
        ffmpeg.stderr(Stdio::piped());

        let mut child = ffmpeg
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open ffmpeg stdin".to_string())?;

        for frame_data in frame_rx {
            if let Err(e) = stdin.write_all(&frame_data) {
                drop(stdin);
                let output = child.wait_with_output().ok();
                let stderr = output
                    .as_ref()
                    .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                    .unwrap_or_default();
                return Err(format!("FFmpeg write error: {} (stderr: {})", e, stderr.trim()));
            }
        }

        drop(stdin);
        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait for ffmpeg: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("FFmpeg error: {}", stderr.trim()));
        }

        Ok(())
    });

    // Create renderer
    let surface_format = wgpu::TextureFormat::Rgba8Unorm;

    let values = controller.evaluate_at_time(0.0);
    let mut frame_config = export_config.config.clone();
    apply_animation_values(&mut frame_config, &values);

    let mut renderer = FlameRenderer::new(
        &device, &queue, surface_format,
        export_config.width, export_config.height,
        &frame_config.flame,
    );

    let palette = frame_config
        .palette
        .as_ref()
        .or_else(|| palette_library.get(frame_config.palette_index))
        .ok_or_else(|| AnimationExportError::InvalidConfig("No palette found".to_string()))?
        .clone();

    // Process frames sequentially
    for frame in 0..total_frames {
        if progress.is_cancelled() {
            drop(frame_tx);
            let _ = writer_handle.join();
            return Err(AnimationExportError::Cancelled);
        }

        let frame_start = Instant::now();
        let time = export_config.frame_time(frame);
        progress.on_frame_start(frame, total_frames, time);

        // Evaluate animation
        let values = controller.evaluate_at_time(time);
        frame_config = export_config.config.clone();
        apply_animation_values(&mut frame_config, &values);

        let current_palette = frame_config.palette.as_ref().unwrap_or(&palette);
        let bg_color = frame_config.background_color;

        // Setup and render
        let render_start = Instant::now();
        let mut setup_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Frame Setup"),
        });
        renderer.load_config(
            &device,
            &mut setup_encoder,
            &queue,
            &frame_config,
            current_palette,
            export_config.iterations_per_thread,
            20, // burn_in default
        );
        queue.submit(std::iter::once(setup_encoder.finish()));

        render_frame_to_completion(
            &device,
            &queue,
            &mut renderer,
            &frame_config,
            export_config.iterations_per_thread,
        )
        .await;

        // Tonemap and copy to staging buffer
        let mut tonemap_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Tonemap and Copy"),
        });
        renderer.tonemap_pass(&mut tonemap_encoder);

        tonemap_encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: renderer.fractal_texture(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(export_config.height),
                },
            },
            Extent3d {
                width: export_config.width,
                height: export_config.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(tonemap_encoder.finish()));
        timing_stats.render_time_ms += render_start.elapsed().as_secs_f64() * 1000.0;

        // Map and read buffer
        let map_start = Instant::now();
        let buffer_slice = staging_buffer.slice(..);
        let (tx, mut rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Poll until mapped
        loop {
            let _ = device.poll(PollType::Poll);
            match rx.try_recv() {
                Ok(Some(Ok(()))) => break,
                Ok(Some(Err(e))) => {
                    return Err(AnimationExportError::GpuError(format!("Buffer map error: {:?}", e)));
                }
                _ => {}
            }
        }
        timing_stats.map_wait_time_ms += map_start.elapsed().as_secs_f64() * 1000.0;

        // Copy data
        let copy_start = Instant::now();
        let data = buffer_slice.get_mapped_range();
        let mut rgba_data = Vec::with_capacity(output_size);

        for y in 0..export_config.height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + (export_config.width * bytes_per_pixel) as usize;
            let row_data = &data[row_start..row_end];

            for x in 0..export_config.width {
                let px = (x * bytes_per_pixel) as usize;
                let r = row_data[px];
                let g = row_data[px + 1];
                let b = row_data[px + 2];
                let a = row_data[px + 3];

                let alpha = a as f32 / 255.0;
                let bg_r = (bg_color[0] * 255.0) as u8;
                let bg_g = (bg_color[1] * 255.0) as u8;
                let bg_b = (bg_color[2] * 255.0) as u8;

                let out_r = ((r as f32 * alpha) + (bg_r as f32 * (1.0 - alpha))) as u8;
                let out_g = ((g as f32 * alpha) + (bg_g as f32 * (1.0 - alpha))) as u8;
                let out_b = ((b as f32 * alpha) + (bg_b as f32 * (1.0 - alpha))) as u8;

                rgba_data.extend_from_slice(&[out_r, out_g, out_b, 255]);
            }
        }
        drop(data);
        staging_buffer.unmap();
        timing_stats.copy_time_ms += copy_start.elapsed().as_secs_f64() * 1000.0;

        // Send to FFmpeg writer thread
        let send_start = Instant::now();
        frame_tx
            .send(rgba_data)
            .map_err(|_| AnimationExportError::FfmpegFailed("Writer thread died".to_string()))?;
        timing_stats.channel_send_time_ms += send_start.elapsed().as_secs_f64() * 1000.0;

        timing_stats.frame_count += 1;
        timing_stats.total_frame_time_ms += frame_start.elapsed().as_secs_f64() * 1000.0;
        progress.on_frame_complete(frame, total_frames, 0.0);
    }

    // Signal writer thread to finish
    drop(frame_tx);

    // Wait for writer thread
    writer_handle
        .join()
        .map_err(|_| AnimationExportError::FfmpegFailed("Writer thread panicked".to_string()))?
        .map_err(AnimationExportError::FfmpegFailed)?;

    let total_time_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let avg_frame_time_ms = total_time_ms / total_frames as f64;

    // Log timing summary
    timing_stats.log_summary();

    progress.on_export_complete(total_frames, total_time_ms);

    Ok(AnimationExportResult {
        total_frames,
        total_time_ms,
        avg_frame_time_ms,
        output_path: export_config.output_path,
    })
}

/// Build FFmpeg command-line arguments from export config
#[cfg(not(target_arch = "wasm32"))]
fn build_ffmpeg_args(config: &AnimationExportConfig) -> Vec<String> {
    let mut args = Vec::new();

    // Overwrite output
    args.push("-y".to_string());

    // Input format
    args.push("-f".to_string());
    args.push("rawvideo".to_string());
    args.push("-pix_fmt".to_string());
    args.push("rgba".to_string());
    args.push("-s".to_string());
    args.push(format!("{}x{}", config.width, config.height));
    args.push("-r".to_string());
    args.push(config.fps.to_string());
    args.push("-i".to_string());
    args.push("-".to_string());

    // Codec and hardware acceleration
    let settings = &config.video_settings;
    let encoder = settings
        .hardware_accel
        .encoder_for_codec(settings.codec)
        .expect("Invalid hardware accel + codec combination");

    args.push("-c:v".to_string());
    args.push(encoder.to_string());

    // Quality settings
    let is_hardware = settings.hardware_accel != HardwareAccel::None;

    match settings.codec {
        VideoCodec::H264 | VideoCodec::H265 => {
            if is_hardware {
                match settings.hardware_accel {
                    HardwareAccel::Nvenc => {
                        args.push("-rc".to_string());
                        args.push("vbr".to_string());
                        args.push("-cq".to_string());
                        args.push(settings.quality.to_string());
                        args.push("-preset".to_string());
                        args.push("p4".to_string());
                    }
                    HardwareAccel::Qsv => {
                        args.push("-global_quality".to_string());
                        args.push(settings.quality.to_string());
                        args.push("-preset".to_string());
                        args.push("medium".to_string());
                    }
                    HardwareAccel::Amf => {
                        args.push("-rc".to_string());
                        args.push("cqp".to_string());
                        args.push("-qp".to_string());
                        args.push(settings.quality.to_string());
                    }
                    HardwareAccel::VideoToolbox => {
                        let vt_quality = 100 - (settings.quality as i32 * 2).min(100).max(0);
                        args.push("-q:v".to_string());
                        args.push(vt_quality.to_string());
                    }
                    HardwareAccel::None => {}
                }
            } else {
                args.push("-crf".to_string());
                args.push(settings.quality.to_string());
                args.push("-preset".to_string());
                args.push("medium".to_string());
                if settings.codec == VideoCodec::H265 {
                    args.push("-x265-params".to_string());
                    args.push("log-level=error".to_string());
                }
            }
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::VP9 => {
            if settings.hardware_accel == HardwareAccel::Qsv {
                args.push("-global_quality".to_string());
                args.push(settings.quality.to_string());
            } else {
                args.push("-crf".to_string());
                args.push(settings.quality.to_string());
                args.push("-b:v".to_string());
                args.push("0".to_string());
            }
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
    }

    args
}
