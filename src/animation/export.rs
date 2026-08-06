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
use crate::config::{ConfigPath, EditingTarget, FractalConfig, json_to_config_value};
use crate::scene::transforms::Flame;
use crate::signal::{Signal, SignalManager};

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

/// Encoding preset (speed/quality tradeoff)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingPreset {
    // CPU presets (libx264/libx265)
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
    Placebo,

    // NVENC presets
    NvencP1,  // Fastest
    NvencP2,
    NvencP3,
    NvencP4,  // Balanced
    NvencP5,
    NvencP6,
    NvencP7,  // Slowest

    // QSV presets
    QsvVeryfast,
    QsvFaster,
    QsvFast,
    QsvMedium,
    QsvSlow,
    QsvSlower,
    QsvVeryslow,

    // AMF presets
    AmfSpeed,
    AmfBalanced,
    AmfQuality,
}

impl Default for EncodingPreset {
    fn default() -> Self {
        Self::Medium
    }
}

impl EncodingPreset {
    /// Get FFmpeg preset string for this preset + hardware accel combination
    pub fn ffmpeg_arg(&self, hw_accel: HardwareAccel) -> &'static str {
        match hw_accel {
            HardwareAccel::None => match self {
                Self::Ultrafast => "ultrafast",
                Self::Superfast => "superfast",
                Self::Veryfast => "veryfast",
                Self::Faster => "faster",
                Self::Fast => "fast",
                Self::Medium => "medium",
                Self::Slow => "slow",
                Self::Slower => "slower",
                Self::Veryslow => "veryslow",
                Self::Placebo => "placebo",
                _ => "medium",  // Fallback for mismatched preset
            },
            HardwareAccel::Nvenc => match self {
                Self::NvencP1 => "p1",
                Self::NvencP2 => "p2",
                Self::NvencP3 => "p3",
                Self::NvencP4 => "p4",
                Self::NvencP5 => "p5",
                Self::NvencP6 => "p6",
                Self::NvencP7 => "p7",
                _ => "p4",  // Fallback
            },
            HardwareAccel::Qsv => match self {
                Self::QsvVeryfast => "veryfast",
                Self::QsvFaster => "faster",
                Self::QsvFast => "fast",
                Self::QsvMedium => "medium",
                Self::QsvSlow => "slow",
                Self::QsvSlower => "slower",
                Self::QsvVeryslow => "veryslow",
                _ => "medium",
            },
            HardwareAccel::Amf => match self {
                Self::AmfSpeed => "speed",
                Self::AmfBalanced => "balanced",
                Self::AmfQuality => "quality",
                _ => "balanced",
            },
            HardwareAccel::VideoToolbox => "",  // No preset support
        }
    }

    /// Get available presets for a hardware accelerator
    pub fn available_for(hw_accel: HardwareAccel) -> Vec<Self> {
        match hw_accel {
            HardwareAccel::None => vec![
                Self::Ultrafast, Self::Superfast, Self::Veryfast,
                Self::Faster, Self::Fast, Self::Medium,
                Self::Slow, Self::Slower, Self::Veryslow, Self::Placebo,
            ],
            HardwareAccel::Nvenc => vec![
                Self::NvencP1, Self::NvencP2, Self::NvencP3, Self::NvencP4,
                Self::NvencP5, Self::NvencP6, Self::NvencP7,
            ],
            HardwareAccel::Qsv => vec![
                Self::QsvVeryfast, Self::QsvFaster, Self::QsvFast, Self::QsvMedium,
                Self::QsvSlow, Self::QsvSlower, Self::QsvVeryslow,
            ],
            HardwareAccel::Amf => vec![
                Self::AmfSpeed, Self::AmfBalanced, Self::AmfQuality,
            ],
            HardwareAccel::VideoToolbox => vec![],  // No presets
        }
    }

    /// Get default preset for a hardware accelerator
    pub fn default_for(hw_accel: HardwareAccel) -> Self {
        match hw_accel {
            HardwareAccel::None => Self::Medium,
            HardwareAccel::Nvenc => Self::NvencP4,
            HardwareAccel::Qsv => Self::QsvMedium,
            HardwareAccel::Amf => Self::AmfBalanced,
            HardwareAccel::VideoToolbox => Self::Medium,  // Not used
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ultrafast => "Ultrafast (fastest)",
            Self::Superfast => "Superfast",
            Self::Veryfast => "Very Fast",
            Self::Faster => "Faster",
            Self::Fast => "Fast",
            Self::Medium => "Medium (balanced)",
            Self::Slow => "Slow",
            Self::Slower => "Slower",
            Self::Veryslow => "Very Slow",
            Self::Placebo => "Placebo (slowest)",

            Self::NvencP1 => "P1 (fastest)",
            Self::NvencP2 => "P2",
            Self::NvencP3 => "P3",
            Self::NvencP4 => "P4 (balanced)",
            Self::NvencP5 => "P5",
            Self::NvencP6 => "P6",
            Self::NvencP7 => "P7 (slowest)",

            Self::QsvVeryfast => "Very Fast",
            Self::QsvFaster => "Faster",
            Self::QsvFast => "Fast",
            Self::QsvMedium => "Medium (balanced)",
            Self::QsvSlow => "Slow",
            Self::QsvSlower => "Slower",
            Self::QsvVeryslow => "Very Slow",

            Self::AmfSpeed => "Speed (fastest)",
            Self::AmfBalanced => "Balanced",
            Self::AmfQuality => "Quality (slowest)",
        }
    }
}

/// Encoding tune (optimization target) - CPU encoders only
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingTune {
    None,        // No tune parameter
    Film,        // Live action with grain
    Animation,   // Flat colors, sharp edges (recommended for fractals!)
    Grain,       // Preserve film grain
    StillImage,  // Slides/presentations
    FastDecode,  // Optimize for playback
}

impl Default for EncodingTune {
    fn default() -> Self {
        Self::Animation  // Perfect for fractal flames!
    }
}

impl EncodingTune {
    /// Get FFmpeg tune string (None returns None)
    pub fn ffmpeg_arg(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Film => Some("film"),
            Self::Animation => Some("animation"),
            Self::Grain => Some("grain"),
            Self::StillImage => Some("stillimage"),
            Self::FastDecode => Some("fastdecode"),
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Film => "Film (live action)",
            Self::Animation => "Animation (recommended)",
            Self::Grain => "Grain (preserve grain)",
            Self::StillImage => "Still Image (slides)",
            Self::FastDecode => "Fast Decode (playback)",
        }
    }

    /// Get all tune options
    pub fn all() -> &'static [EncodingTune] {
        &[
            Self::None,
            Self::Animation,
            Self::Film,
            Self::Grain,
            Self::StillImage,
            Self::FastDecode,
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
    /// Encoding preset (speed/quality tradeoff)
    pub preset: EncodingPreset,
    /// Encoding tune (optimization target, CPU encoders only)
    pub tune: EncodingTune,
}

impl Default for VideoEncodingSettings {
    fn default() -> Self {
        Self {
            codec: VideoCodec::default(),
            hardware_accel: HardwareAccel::default(),
            quality: 18,
            preset: EncodingPreset::default(),
            tune: EncodingTune::default(),
        }
    }
}

/// Audio export configuration for including audio in video exports
#[derive(Debug, Clone)]
pub struct AudioExportConfig {
    /// Path to audio file (MP3, WAV, FLAC, OGG, etc.)
    pub file: PathBuf,

    /// Time offset in seconds
    /// - Negative: Skip into audio (audio at t=-offset plays at animation t=0)
    /// - Positive: Delay audio start (silence until animation reaches this time)
    /// - Zero: Audio and animation start together
    pub offset: f64,

    /// Fade in duration in seconds (0.0 = no fade)
    pub fade_in: f64,

    /// Fade out duration in seconds (0.0 = no fade)
    pub fade_out: f64,

    /// Audio bitrate in kbps (default: 192)
    pub bitrate_kbps: u32,
}

impl Default for AudioExportConfig {
    fn default() -> Self {
        Self {
            file: PathBuf::new(),
            offset: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            bitrate_kbps: 192,
        }
    }
}

impl AudioExportConfig {
    /// Create a new audio export config with a file path
    pub fn new(file: PathBuf) -> Self {
        Self {
            file,
            ..Default::default()
        }
    }

    /// Set the time offset
    pub fn with_offset(mut self, offset: f64) -> Self {
        self.offset = offset;
        self
    }

    /// Set fade in duration
    pub fn with_fade_in(mut self, fade_in: f64) -> Self {
        self.fade_in = fade_in;
        self
    }

    /// Set fade out duration
    pub fn with_fade_out(mut self, fade_out: f64) -> Self {
        self.fade_out = fade_out;
        self
    }

    /// Check if this config has a valid audio file
    pub fn has_audio(&self) -> bool {
        !self.file.as_os_str().is_empty() && self.file.exists()
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
    /// Optional audio to include in export
    pub audio: Option<AudioExportConfig>,
    /// Signal data for signal tracks (cloned from SignalManager)
    pub signals: std::collections::HashMap<String, Signal>,
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

/// Format the unified progress detail line for a rendered frame, with a rough
/// ETA derived from the most recent frame's render time. Used by both the CLI
/// and UI reporters via [`crate::export::ExportReporter`].
fn frame_progress_detail(frame_done: u32, total: u32, secs_per_frame: f64) -> String {
    let remaining = total.saturating_sub(frame_done) as f64 * secs_per_frame;
    let eta = if !remaining.is_finite() || remaining <= 0.0 {
        "—".to_string()
    } else if remaining >= 60.0 {
        format!("{}m {:02}s", (remaining / 60.0) as u64, (remaining % 60.0).round() as u64)
    } else {
        format!("{}s", remaining.round() as u64)
    };
    if secs_per_frame > 0.0 {
        format!("Frame {frame_done}/{total} · {secs_per_frame:.2}s/frame · ETA {eta}")
    } else {
        format!("Frame {frame_done}/{total}")
    }
}

/// Apply animation values to a FractalConfig.
///
/// Takes the evaluated animation values and applies them to the config.
/// Each entry carries an `EditingTarget` selecting which flame
/// (Main or a specific subflame) the path resolves against.
pub fn apply_animation_values(
    config: &mut FractalConfig,
    values: &[(EditingTarget, String, serde_json::Value)],
) {
    for (flame_target, path_str, json_value) in values {
        // Parse the path string to ConfigPath
        if let Some(path) = ConfigPath::from_string_key(path_str) {
            // Convert JSON value to ConfigValue
            if let Some(config_value) = json_to_config_value(json_value, &path) {
                // Apply to config, routed via the track's flame target.
                apply_config_value(config, *flame_target, &path, &config_value);
            }
        } else {
            log::warn!("Unknown animation path: {}", path_str);
        }
    }
}

/// Resolve a mutable reference to the flame slot a track targets
/// inside the given `FractalConfig`. Returns `None` when the subflame
/// index is out of range — apply paths treat this as a broken track
/// and skip the write.
fn resolve_flame_mut(config: &mut FractalConfig, target: EditingTarget) -> Option<&mut Flame> {
    match target {
        EditingTarget::Main => Some(&mut config.flame),
        EditingTarget::Subflame { index } => config.flame.subflames.get_mut(index),
    }
}

/// Write one affine coefficient on a transform. Shared across all four
/// pool sites (Normal / legacy-Final / Linked / PoolFinal) so the match
/// arms only differ in how they look the transform up.
fn apply_affine_param(xform: &mut crate::scene::transforms::Transform, param: crate::config::AffineParam, v: f32) {
    use crate::config::AffineParam;
    match param {
        AffineParam::A => xform.a = v,
        AffineParam::B => xform.b = v,
        AffineParam::C => xform.c = v,
        AffineParam::D => xform.d = v,
        AffineParam::E => xform.e = v,
        AffineParam::F => xform.f = v,
        AffineParam::G => xform.g = v,
    }
}

/// Same as `apply_affine_param` but for the post-affine half.
fn apply_post_affine_param(xform: &mut crate::scene::transforms::Transform, param: crate::config::AffineParam, v: f32) {
    use crate::config::AffineParam;
    match param {
        AffineParam::A => xform.post_a = v,
        AffineParam::B => xform.post_b = v,
        AffineParam::C => xform.post_c = v,
        AffineParam::D => xform.post_d = v,
        AffineParam::E => xform.post_e = v,
        AffineParam::F => xform.post_f = v,
        AffineParam::G => xform.post_g = v,
    }
}

/// Apply a single ConfigValue to a FractalConfig, routed via `target`
/// for flame-scoped paths.
fn apply_config_value(
    config: &mut FractalConfig,
    target: EditingTarget,
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
        (ConfigPath::CameraBank, ConfigValue::Float(v)) => config.camera_bank = *v,
        // Camera world-space position. CameraX/CameraY were previously
        // missing here, so animating them did nothing in video export
        // (only CameraZ was applied) — they fell through to the `_ => {}`
        // catch-all below. Single-PNG export was unaffected (it reads
        // config.camera_x/y/z directly).
        (ConfigPath::CameraX, ConfigValue::Float(v)) => config.camera_x = *v,
        (ConfigPath::CameraY, ConfigValue::Float(v)) => config.camera_y = *v,
        (ConfigPath::CameraZ, ConfigValue::Float(v)) => config.camera_z = *v,
        // Depth effects — animatable but were also missing here, so a
        // DoF pull or fog change during a flythrough did nothing in the
        // rendered video.
        (ConfigPath::DofFocusDistance, ConfigValue::Float(v)) => config.dof_focus_distance = *v,
        (ConfigPath::DofBlurStrength, ConfigValue::Float(v)) => config.dof_blur_strength = *v,
        (ConfigPath::FogStrength, ConfigValue::Float(v)) => config.fog_strength = *v,
        (ConfigPath::FogStart, ConfigValue::Float(v)) => config.fog_start = *v,

        // Solid rendering + lighting — FractalConfig-level; without these
        // arms an animated light (e.g. a Light-2 azimuth sweep) renders
        // static in the exported video while previewing fine in-app.
        (ConfigPath::SolidStrength, ConfigValue::Float(v)) => config.solid_strength = *v,
        (ConfigPath::SurfaceThickness, ConfigValue::Float(v)) => config.surface_thickness = *v,
        (ConfigPath::SolidShadowStrength, ConfigValue::Float(v)) => config.solid_shading.shadow_strength = *v,
        (ConfigPath::ShadingStrength, ConfigValue::Float(v)) => config.solid_shading.shading_strength = *v,
        (ConfigPath::SolidAmbient, ConfigValue::Float(v)) => config.solid_shading.ambient = *v,
        (ConfigPath::SolidDiffuse, ConfigValue::Float(v)) => config.solid_shading.diffuse = *v,
        (ConfigPath::SolidSpecular, ConfigValue::Float(v)) => config.solid_shading.specular = *v,
        (ConfigPath::SolidShininess, ConfigValue::Float(v)) => config.solid_shading.shininess = *v,
        (ConfigPath::SsaoStrength, ConfigValue::Float(v)) => config.solid_shading.ssao_strength = *v,
        (ConfigPath::SsaoRadius, ConfigValue::Float(v)) => config.solid_shading.ssao_radius = *v,
        (ConfigPath::NormalSmoothing, ConfigValue::Float(v)) => {
            config.solid_shading.normal_smoothing = (v.round().max(0.0) as u32).min(3);
        }
        (ConfigPath::GapFill, ConfigValue::Float(v)) => {
            config.solid_shading.gap_fill = (v.round().max(0.0) as u32).min(3);
        }
        (ConfigPath::SolidLightEnabled { index }, ConfigValue::Bool(v)) => {
            if let Some(light) = config.solid_shading.lights.get_mut(*index) {
                light.enabled = *v;
            }
        }
        (ConfigPath::SolidLightParam { index, param }, ConfigValue::Float(v)) => {
            if let Some(light) = config.solid_shading.lights.get_mut(*index) {
                match param.as_str() {
                    "azimuth" => light.azimuth = *v,
                    "elevation" => light.elevation = *v,
                    "intensity" => light.intensity = *v,
                    "color_r" => light.color[0] = *v,
                    "color_g" => light.color[1] = *v,
                    "color_b" => light.color[2] = *v,
                    _ => {}
                }
            }
        }

        // Filter (density estimation) — FractalConfig-level, were missing.
        (ConfigPath::FilterRadius, ConfigValue::Float(v)) => config.filter_radius = *v,
        (ConfigPath::FilterBlurEdges, ConfigValue::Float(v)) => config.filter_blur_edges = *v,
        // Adaptive-blend accumulation — FractalConfig-level, were missing.
        (ConfigPath::BlendFactor, ConfigValue::Float(v)) => config.blend_factor = *v,
        (ConfigPath::UseDynamicBlend, ConfigValue::Bool(v)) => config.use_dynamic_blend = *v,

        // Tone mapping
        (ConfigPath::Exposure, ConfigValue::Float(v)) => config.exposure = *v,
        (ConfigPath::Gamma, ConfigValue::Float(v)) => config.gamma = *v,
        (ConfigPath::GammaThreshold, ConfigValue::Float(v)) => config.gamma_threshold = *v,
        (ConfigPath::Brightness, ConfigValue::Float(v)) => config.brightness = *v,
        (ConfigPath::Vibrancy, ConfigValue::Float(v)) => config.vibrancy = *v,
        (ConfigPath::Saturation, ConfigValue::Float(v)) => config.saturation = *v,
        (ConfigPath::HueShift, ConfigValue::Float(v)) => config.hue_shift = *v,
        (ConfigPath::DensityScale, ConfigValue::Float(v)) => config.density_scale = *v,
        // The rest of the tonemap chain — also FractalConfig-level and
        // previously dropped in video export.
        (ConfigPath::WhiteLevel, ConfigValue::Float(v)) => config.white_level = *v,
        (ConfigPath::AlphaBlendLow, ConfigValue::Float(v)) => config.alpha_blend_low = *v,
        (ConfigPath::AlphaBlendHigh, ConfigValue::Float(v)) => config.alpha_blend_high = *v,
        (ConfigPath::TonemapMode, ConfigValue::ToneMapMode(m)) => config.tonemap_mode = *m,
        (ConfigPath::HighlightMode, ConfigValue::HighlightMode(m)) => config.highlight_mode = *m,
        (ConfigPath::LevelsEnabled, ConfigValue::Bool(v)) => config.levels_enabled = *v,
        (ConfigPath::LevelsLow, ConfigValue::Float(v)) => config.levels_low = *v,
        (ConfigPath::LevelsHigh, ConfigValue::Float(v)) => config.levels_high = *v,
        (ConfigPath::LevelsGamma, ConfigValue::Float(v)) => config.levels_gamma = *v,

        // Color
        (ConfigPath::PaletteRotation, ConfigValue::Float(v)) => config.palette_rotation = *v,
        (ConfigPath::SpeedFactor, ConfigValue::Float(v)) => config.speed_factor = *v,
        (ConfigPath::BackgroundColor, ConfigValue::ColorRgb(rgb)) => config.background_color = *rgb,
        (ConfigPath::BackgroundColorR, ConfigValue::Float(v)) => config.background_color[0] = *v,
        (ConfigPath::BackgroundColorG, ConfigValue::Float(v)) => config.background_color[1] = *v,
        (ConfigPath::BackgroundColorB, ConfigValue::Float(v)) => config.background_color[2] = *v,

        // Effect parameters (FractalConfig-level, not per-flame).
        // These intentionally stay outside the per-flame match below
        // because effects live on the FractalConfig directly.
        (ConfigPath::DensityEffectEnabled { index }, ConfigValue::Bool(v)) => {
            if let Some(effect) = config.density_effects.get_mut(*index) {
                effect.enabled = *v;
            }
        }
        (ConfigPath::DensityEffectParam { index, param }, ConfigValue::Float(v)) => {
            if let Some(effect) = config.density_effects.get_mut(*index) {
                effect.params.insert(param.clone(), *v);
            }
        }
        (ConfigPath::ColorEffectEnabled { index }, ConfigValue::Bool(v)) => {
            if let Some(effect) = config.color_effects.get_mut(*index) {
                effect.enabled = *v;
            }
        }
        (ConfigPath::ColorEffectParam { index, param }, ConfigValue::Float(v)) => {
            if let Some(effect) = config.color_effects.get_mut(*index) {
                effect.params.insert(param.clone(), *v);
            }
        }

        // Everything else is per-flame. Resolve the target flame and
        // delegate to apply_flame_value. Broken targets (missing
        // subflame) silently drop — UI surfaces these.
        _ => {
            if let Some(flame) = resolve_flame_mut(config, target) {
                apply_flame_value(flame, path, value);
            } else {
                log::debug!(
                    "apply_animation: skipping {} — flame target {:?} not found",
                    path, target,
                );
            }
        }
    }
}

/// Apply a per-flame ConfigPath to a specific Flame.
///
/// Mirrors the per-flame arms of `ConfigManager::set_value` but
/// operates on a `Flame` directly so it works on subflames too.
fn apply_flame_value(
    flame: &mut Flame,
    path: &ConfigPath,
    value: &crate::config::ConfigValue,
) {
    use crate::config::ConfigValue;

    match (path, value) {
        // Transform parameters
        (ConfigPath::TransformWeight { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.weight = *v;
            }
        }
        (ConfigPath::TransformColor { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.color = *v;
            }
        }
        (ConfigPath::TransformColorSpeed { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.color_speed = *v;
            }
        }
        (ConfigPath::TransformOpacity { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.opacity = *v;
            }
        }
        (ConfigPath::TransformAffine { index, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                apply_affine_param(xform, *param, *v);
            }
        }
        (ConfigPath::TransformVariation { index, variation }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.variations.insert(variation.clone(), *v);
            }
        }
        (ConfigPath::TransformVariationParam { index, variation, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_variation_param(variation, param, *v);
            }
        }

        // High-level transform operations
        (ConfigPath::TransformOriginX { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_origin_x(*v);
            }
        }
        (ConfigPath::TransformOriginY { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_origin_y(*v);
            }
        }
        (ConfigPath::TransformRotation { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_rotation(*v);
            }
        }
        (ConfigPath::TransformScale { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_scale(*v);
            }
        }
        (ConfigPath::TransformPostAffineOriginX { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_post_origin_x(*v);
            }
        }
        (ConfigPath::TransformPostAffineOriginY { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_post_origin_y(*v);
            }
        }
        (ConfigPath::TransformPostAffineRotation { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_post_rotation(*v);
            }
        }
        (ConfigPath::TransformPostAffineScale { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.transforms.get_mut(*index) {
                xform.set_post_scale(*v);
            }
        }

        // Linked pool — animatable per-pool-member parameters.
        (ConfigPath::LinkedTransformAffine { index, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                apply_affine_param(xform, *param, *v);
            }
        }
        (ConfigPath::LinkedTransformPostAffineEnabled { index }, ConfigValue::Bool(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.post_affine_enabled = *v;
            }
        }
        (ConfigPath::LinkedTransformPostAffine { index, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                apply_post_affine_param(xform, *param, *v);
            }
        }
        (ConfigPath::LinkedTransformVariation { index, variation }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.variations.insert(variation.clone(), *v);
            }
        }
        (ConfigPath::LinkedTransformVariationParam { index, variation, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_variation_param(variation, param, *v);
            }
        }
        (ConfigPath::LinkedTransformOriginX { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_origin_x(*v);
            }
        }
        (ConfigPath::LinkedTransformOriginY { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_origin_y(*v);
            }
        }
        (ConfigPath::LinkedTransformRotation { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_rotation(*v);
            }
        }
        (ConfigPath::LinkedTransformScale { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_scale(*v);
            }
        }
        (ConfigPath::LinkedTransformPostAffineOriginX { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_post_origin_x(*v);
            }
        }
        (ConfigPath::LinkedTransformPostAffineOriginY { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_post_origin_y(*v);
            }
        }
        (ConfigPath::LinkedTransformPostAffineRotation { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_post_rotation(*v);
            }
        }
        (ConfigPath::LinkedTransformPostAffineScale { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.linked_transforms.get_mut(*index) {
                xform.set_post_scale(*v);
            }
        }

        // Final pool — animatable per-pool-member parameters.
        (ConfigPath::FinalTransformAffine { index, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                apply_affine_param(xform, *param, *v);
            }
        }
        (ConfigPath::FinalTransformPostAffineEnabled { index }, ConfigValue::Bool(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.post_affine_enabled = *v;
            }
        }
        (ConfigPath::FinalTransformPostAffine { index, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                apply_post_affine_param(xform, *param, *v);
            }
        }
        (ConfigPath::FinalTransformVariation { index, variation }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.variations.insert(variation.clone(), *v);
            }
        }
        (ConfigPath::FinalTransformVariationParam { index, variation, param }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_variation_param(variation, param, *v);
            }
        }
        (ConfigPath::FinalTransformOriginX { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_origin_x(*v);
            }
        }
        (ConfigPath::FinalTransformOriginY { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_origin_y(*v);
            }
        }
        (ConfigPath::FinalTransformRotation { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_rotation(*v);
            }
        }
        (ConfigPath::FinalTransformScale { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_scale(*v);
            }
        }
        (ConfigPath::FinalTransformPostAffineOriginX { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_post_origin_x(*v);
            }
        }
        (ConfigPath::FinalTransformPostAffineOriginY { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_post_origin_y(*v);
            }
        }
        (ConfigPath::FinalTransformPostAffineRotation { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_post_rotation(*v);
            }
        }
        (ConfigPath::FinalTransformPostAffineScale { index }, ConfigValue::Float(v)) => {
            if let Some(xform) = flame.final_transforms.get_mut(*index) {
                xform.set_post_scale(*v);
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

/// Where ffmpeg actually is — cached (spawning a probe process every
/// frame is expensive).
///
/// `Command::new("ffmpeg")` alone breaks on macOS the moment the app is
/// launched from Finder: a bundled GUI app inherits launchd's PATH
/// (`/usr/sbin:/usr/bin:/bin:/sbin`), not the shell's, so a Homebrew
/// ffmpeg in /opt/homebrew/bin is invisible even though `ffmpeg` works
/// fine in every terminal. Probe PATH first (terminal launches, Windows,
/// Linux), then the conventional install locations.
#[cfg(not(target_arch = "wasm32"))]
static FFMPEG_PATH: once_cell::sync::Lazy<Option<std::path::PathBuf>> =
    once_cell::sync::Lazy::new(|| {
        let candidates = [
            "ffmpeg", // PATH — covers terminals, Windows, most Linux
            "/opt/homebrew/bin/ffmpeg", // Homebrew, Apple Silicon
            "/usr/local/bin/ffmpeg",    // Homebrew Intel / manual installs
            "/opt/local/bin/ffmpeg",    // MacPorts
        ];
        for c in candidates {
            let mut probe = std::process::Command::new(c);
            hide_console_window(&mut probe);
            let ok = probe
                .arg("-version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                log::info!("ffmpeg found: {c}");
                return Some(std::path::PathBuf::from(c));
            }
        }
        log::warn!(
            "ffmpeg not found — probed PATH and {:?}",
            &candidates[1..]
        );
        None
    });

/// Keep Windows from opening a console window for a child process.
///
/// The app is GUI-subsystem (see the console handling in `main.rs`), and
/// when a GUI process spawns a console-subsystem child, Windows gives
/// the child its OWN console — so every video export flashed up an empty
/// black ffmpeg window beside the app. `CREATE_NO_WINDOW` suppresses it;
/// stdout/stderr are piped and read by the caller either way, so nothing
/// is lost by having no console attached.
///
/// No-op everywhere else: on macOS and Linux a spawned child has no
/// window to begin with.
///
/// Verified on Windows 10 (2026-08-05): a full video export, no console.
#[cfg(not(target_arch = "wasm32"))]
fn hide_console_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// The command to invoke ffmpeg with, wherever it was found.
///
/// Every ffmpeg spawn goes through here, which is what makes the
/// no-console-window flag a single line rather than four call sites to
/// keep in step.
#[cfg(not(target_arch = "wasm32"))]
pub fn ffmpeg_command() -> std::process::Command {
    let mut cmd = std::process::Command::new(
        FFMPEG_PATH
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new("ffmpeg")),
    );
    hide_console_window(&mut cmd);
    cmd
}

/// Check if ffmpeg is available on the system (cached after first call)
#[cfg(not(target_arch = "wasm32"))]
pub fn is_ffmpeg_available() -> bool {
    FFMPEG_PATH.is_some()
}

/// Get ffmpeg version string (if available)
#[cfg(not(target_arch = "wasm32"))]
pub fn get_ffmpeg_version() -> Option<String> {
    let output = ffmpeg_command()
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
    let output = ffmpeg_command()
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

#[cfg(not(target_arch = "wasm32"))]
impl AnimationExportConfig {
    /// Round the frame size down to even numbers, returning the sizes
    /// actually used.
    ///
    /// H.264/H.265 in yuv420p — the pixel format every path here emits,
    /// because it is what players actually support — subsamples chroma
    /// 2x2 and therefore CANNOT encode an odd dimension. libx264 and
    /// libx265 do not warn and adapt; they refuse to open the encoder
    /// and ffmpeg exits, which downstream looks like the pipe dying a
    /// few frames in (the frame channel buffers a few first).
    ///
    /// Rounding down rather than rejecting: an odd size is almost always
    /// an incidental consequence of a window size or a typed number, not
    /// a deliberate request, and losing one pixel column is a far better
    /// outcome than a failed export. It is logged, not silent.
    fn force_even_dimensions(&mut self) {
        let (w, h) = (self.width & !1, self.height & !1);
        if (w, h) != (self.width, self.height) {
            log::warn!(
                "Video export: {}x{} is not encodable in yuv420p (odd dimension); \
                 using {}x{}",
                self.width, self.height, w, h
            );
            self.width = w;
            self.height = h;
        }
        // A zero dimension cannot be rounded into validity.
        self.width = self.width.max(2);
        self.height = self.height.max(2);
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
    mut export_config: AnimationExportConfig,
    reporter: &mut dyn crate::export::ExportReporter,
) -> Result<AnimationExportResult, AnimationExportError> {
    export_config.force_even_dimensions();
    use std::io::Write;
    use std::process::Stdio;
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
    let instance = egui_wgpu::wgpu::Instance::new(egui_wgpu::wgpu::InstanceDescriptor {
        backends: egui_wgpu::wgpu::Backends::all(),
        ..egui_wgpu::wgpu::InstanceDescriptor::new_without_display_handle()
    });

    let adapter = instance
        .request_adapter(&egui_wgpu::wgpu::RequestAdapterOptions {
            power_preference: egui_wgpu::wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to find adapter: {:?}", e)))?;

    // Request adapter's max storage buffer size (matches main app behavior)
    let adapter_limits = adapter.limits();
    let mut limits = egui_wgpu::wgpu::Limits::default();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    // Compute bind group has 10 storage buffers post-subflames; spec floor is 8.
    limits.max_storage_buffers_per_shader_stage =
        adapter_limits.max_storage_buffers_per_shader_stage;

    let (device, queue) = adapter
        .request_device(
            &egui_wgpu::wgpu::DeviceDescriptor {
                label: Some("Animation Export Device"),
                required_features: egui_wgpu::wgpu::Features::CLEAR_TEXTURE,
                required_limits: limits,
                memory_hints: egui_wgpu::wgpu::MemoryHints::Performance,
                ..Default::default()
            },
        )
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to create device: {:?}", e)))?;

    // Create animation controller for evaluation. Bind tracks against
    // the export config so the rebind hook (n/a here, but kept for
    // consistency) and apply path see resolved IDs.
    let mut controller = AnimationController::new();
    let mut animation = export_config.animation.clone();
    animation.bind_to_config(&export_config.config);
    controller.load(animation);

    // Create signal manager from exported signals for signal track evaluation
    let mut signal_manager = SignalManager::new();
    for (_name, signal) in &export_config.signals {
        signal_manager.insert(signal.clone());
    }
    let has_signals = !export_config.signals.is_empty();

    // Build FFmpeg command for piped raw video input
    let mut ffmpeg = ffmpeg_command();

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
                        // FIX: Use constqp mode for constant quality (like CRF)
                        ffmpeg.arg("-rc").arg("constqp");
                        ffmpeg.arg("-qp").arg(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            ffmpeg.arg("-preset").arg(preset);
                        }
                    }
                    HardwareAccel::Qsv => {
                        // FIX: Add look_ahead 0 to force constant QP mode
                        ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
                        ffmpeg.arg("-look_ahead").arg("0");

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            ffmpeg.arg("-preset").arg(preset);
                        }
                    }
                    HardwareAccel::Amf => {
                        // Already correct: constant QP mode
                        ffmpeg.arg("-rc").arg("cqp");
                        ffmpeg.arg("-qp").arg(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            ffmpeg.arg("-preset").arg(preset);
                        }
                    }
                    HardwareAccel::VideoToolbox => {
                        // FIX: Correct quality scale conversion (CRF 0-51 → VT 100-1)
                        let vt_quality = (100 - (settings.quality as i32 * 100 / 51)).clamp(1, 100);
                        ffmpeg.arg("-q:v").arg(vt_quality.to_string());
                        // Note: VideoToolbox doesn't support -preset parameter
                    }
                    HardwareAccel::None => unreachable!(),
                }
            } else {
                // CPU libx264
                ffmpeg.arg("-crf").arg(settings.quality.to_string());

                // Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                ffmpeg.arg("-preset").arg(preset);

                // Apply tune (CPU only)
                if let Some(tune) = settings.tune.ffmpeg_arg() {
                    ffmpeg.arg("-tune").arg(tune);
                }
            }
            ffmpeg.arg("-pix_fmt").arg("yuv420p"); // Maximum compatibility
        }
        VideoCodec::H265 => {
            if is_hardware {
                match settings.hardware_accel {
                    HardwareAccel::Nvenc => {
                        // FIX: Use constqp mode for constant quality
                        ffmpeg.arg("-rc").arg("constqp");
                        ffmpeg.arg("-qp").arg(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            ffmpeg.arg("-preset").arg(preset);
                        }
                    }
                    HardwareAccel::Qsv => {
                        // FIX: Add look_ahead 0 to force constant QP mode
                        ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
                        ffmpeg.arg("-look_ahead").arg("0");

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            ffmpeg.arg("-preset").arg(preset);
                        }
                    }
                    HardwareAccel::Amf => {
                        // Already correct: constant QP mode
                        ffmpeg.arg("-rc").arg("cqp");
                        ffmpeg.arg("-qp").arg(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            ffmpeg.arg("-preset").arg(preset);
                        }
                    }
                    HardwareAccel::VideoToolbox => {
                        // FIX: Correct quality scale conversion
                        let vt_quality = (100 - (settings.quality as i32 * 100 / 51)).clamp(1, 100);
                        ffmpeg.arg("-q:v").arg(vt_quality.to_string());
                    }
                    HardwareAccel::None => unreachable!(),
                }
            } else {
                // CPU libx265
                ffmpeg.arg("-crf").arg(settings.quality.to_string());

                // Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                ffmpeg.arg("-preset").arg(preset);

                // Apply tune (CPU only)
                if let Some(tune) = settings.tune.ffmpeg_arg() {
                    ffmpeg.arg("-tune").arg(tune);
                }

                ffmpeg.arg("-x265-params").arg("log-level=error");
            }
            ffmpeg.arg("-pix_fmt").arg("yuv420p");
        }
        VideoCodec::VP9 => {
            // VP9 only supports software or QSV
            if settings.hardware_accel == HardwareAccel::Qsv {
                // FIX: Add look_ahead 0 for QSV
                ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
                ffmpeg.arg("-look_ahead").arg("0");

                // Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                if !preset.is_empty() {
                    ffmpeg.arg("-preset").arg(preset);
                }
            } else {
                // CPU libvpx-vp9
                ffmpeg.arg("-crf").arg(settings.quality.to_string());
                ffmpeg.arg("-b:v").arg("0"); // Use CRF mode

                // Apply preset (note: VP9 uses different preset names, but we use CPU presets)
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                ffmpeg.arg("-speed").arg(preset);  // VP9 uses -speed instead of -preset

                // VP9 doesn't support -tune parameter
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
        if reporter.is_cancelled() {
            // Kill FFmpeg process on cancel
            let _ = child.kill();
            return Err(AnimationExportError::Cancelled);
        }

        let frame_start = Instant::now();
        let time = export_config.frame_time(frame);

        reporter.progress(
            frame as f32 / total_frames as f32,
            &format!("Rendering frame {}/{}", frame + 1, total_frames),
        );

        // Evaluate animation at this time (with signal support)
        let values = if has_signals {
            controller.evaluate_at_time_with_signals(time, Some(&signal_manager))
        } else {
            controller.evaluate_at_time(time)
        };

        // Create config copy and apply animation values
        let mut frame_config = export_config.config.clone();
        apply_animation_values(&mut frame_config, &values);

        // Use unified render API
        let job = crate::renderer::RenderJob::new(&frame_config, export_config.width, export_config.height)
            .with_iterations_per_thread(export_config.iterations_per_thread);

        let output = crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)
            .await
            .map_err(|e| AnimationExportError::GpuError(format!("Failed to render frame: {}", e)))?;

        let rgba_data = output.rgba_data;

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
        reporter.progress(
            (frame + 1) as f32 / total_frames as f32,
            &frame_progress_detail(frame + 1, total_frames, frame_elapsed / 1000.0),
        );
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
        // Clear paths only on very first batch of the entire export
        let clear_paths = total_rendered == 0 && clear_histogram;

        renderer.compute_pass(
            &mut encoder,
            queue,
            device,
            NUM_WORKGROUPS,
            iterations_per_thread,
            20, // burn_in default
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_bank,

            config.camera_x,

            config.camera_y,

            config.camera_z,
            config.speed_factor,
            clear_histogram,
            clear_paths,
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
    mut export_config: AnimationExportConfig,
    reporter: &mut dyn crate::export::ExportReporter,
) -> Result<AnimationExportResult, AnimationExportError> {
    export_config.force_even_dimensions();
    use crate::renderer::compute_kernel::FlameRenderer;
    use egui_wgpu::wgpu::{
        self, BufferDescriptor, BufferUsages, CommandEncoderDescriptor,
        Extent3d, MapMode, Origin3d, PollType, TextureAspect,
        TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo,
        COPY_BYTES_PER_ROW_ALIGNMENT,
    };
    use std::io::Write;
    use std::process::Stdio;
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
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to find adapter: {:?}", e)))?;

    // Request adapter's max storage buffer size (matches main app behavior)
    let adapter_limits = adapter.limits();
    let mut limits = wgpu::Limits::default();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    // Compute bind group has 10 storage buffers post-subflames; spec floor is 8.
    limits.max_storage_buffers_per_shader_stage =
        adapter_limits.max_storage_buffers_per_shader_stage;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Animation Export Device"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        })
        .await
        .map_err(|e| AnimationExportError::GpuError(format!("Failed to create device: {:?}", e)))?;

    // Create animation controller
    let mut controller = AnimationController::new();
    let mut animation = export_config.animation.clone();
    animation.bind_to_config(&export_config.config);
    controller.load(animation);

    // Create signal manager from exported signals for signal track evaluation
    let mut signal_manager = SignalManager::new();
    for (_name, signal) in &export_config.signals {
        signal_manager.insert(signal.clone());
    }
    let has_signals = !export_config.signals.is_empty();

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
    // Channel buffer: 16 frames (~31 MB at 800×600×4 bytes) to prevent blocking
    // Increased from 4 to reduce starvation risk when FFmpeg writer gets CPU-starved
    let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(16);

    // Build FFmpeg command
    let ffmpeg_args = build_ffmpeg_args(&export_config);
    let output_path = export_config.output_path.clone();

    let writer_handle = std::thread::spawn(move || -> Result<(), String> {
        let mut ffmpeg = ffmpeg_command();
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

    let values = if has_signals {
        controller.evaluate_at_time_with_signals(0.0, Some(&signal_manager))
    } else {
        controller.evaluate_at_time(0.0)
    };
    let mut frame_config = export_config.config.clone();
    apply_animation_values(&mut frame_config, &values);

    let mut renderer = FlameRenderer::new(
        &device, &queue, surface_format,
        export_config.width, export_config.height,
        &frame_config.flame,
    );

    // Process frames sequentially
    for frame in 0..total_frames {
        if reporter.is_cancelled() {
            drop(frame_tx);
            let _ = writer_handle.join();
            return Err(AnimationExportError::Cancelled);
        }

        let frame_start = Instant::now();
        let time = export_config.frame_time(frame);
        reporter.progress(
            frame as f32 / total_frames as f32,
            &format!("Rendering frame {}/{}", frame + 1, total_frames),
        );

        // Evaluate animation (with signal support)
        let values = if has_signals {
            controller.evaluate_at_time_with_signals(time, Some(&signal_manager))
        } else {
            controller.evaluate_at_time(time)
        };
        frame_config = export_config.config.clone();
        apply_animation_values(&mut frame_config, &values);

        let current_palette = &frame_config.palette;
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

        // Solid rendering finalize — mirrors the interactive frame and the
        // CLI export: exact brightness renormalization for occluded
        // renders, then the shade pass (lighting/SSAO), then tonemap from
        // the shaded output.
        renderer.apply_exact_density_fraction(&device, &queue);

        // Tonemap
        let mut tonemap_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Tonemap"),
        });
        let shade_ran = renderer.run_shade_pass(
            &device,
            &queue,
            &mut tonemap_encoder,
            frame_config.zoom,
            frame_config.rotation,
            frame_config.pan_x,
            frame_config.pan_y,
            frame_config.camera_rotation_x,
            frame_config.camera_rotation_y,
            frame_config.camera_bank,
            frame_config.camera_x,
            frame_config.camera_y,
            frame_config.camera_z,
        );
        if shade_ran {
            renderer.tonemap_pass_with_input(&device, &queue, &mut tonemap_encoder, renderer.shade_output_view());
        } else {
            renderer.tonemap_pass(&queue, &mut tonemap_encoder);
        }
        queue.submit(std::iter::once(tonemap_encoder.finish()));

        // Run color effects if enabled
        let has_color_effects = crate::renderer::effect_chain::EffectChainRunner::has_enabled_effects(&frame_config.color_effects);
        let mut effect_chain: Option<crate::renderer::effect_chain::EffectChainRunner> = None;
        let color_effects_ran = if has_color_effects {
            let mut chain = crate::renderer::effect_chain::EffectChainRunner::new(
                &device, export_config.width, export_config.height);

            let mut effect_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Color Effects"),
            });

            chain.reset_slots();
            let ran = chain.run_color_effects(
                &device,
                &queue,
                &mut effect_encoder,
                renderer.get_fractal_texture_view(),
                &frame_config.color_effects,
            );

            queue.submit(std::iter::once(effect_encoder.finish()));
            effect_chain = Some(chain);
            ran
        } else {
            false
        };

        // Copy to staging buffer - from effect chain output if effects ran, otherwise from renderer
        let mut copy_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Copy to Staging"),
        });

        if color_effects_ran {
            if let Some(ref chain) = effect_chain {
                chain.copy_color_to_buffer(&mut copy_encoder, &staging_buffer, padded_bytes_per_row);
            }
        } else {
            copy_encoder.copy_texture_to_buffer(
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
        }
        queue.submit(std::iter::once(copy_encoder.finish()));
        timing_stats.render_time_ms += render_start.elapsed().as_secs_f64() * 1000.0;

        // Map and read buffer
        let map_start = Instant::now();
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Wait for mapping to complete (proper blocking with timeout)
        let _ = device.poll(PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),  // 30s timeout to detect GPU hangs
        });

        // Await the mapping result
        rx.await
            .map_err(|_| AnimationExportError::GpuError(
                format!("GPU mapping timed out or channel closed (frame {})", frame)
            ))?
            .map_err(|e| AnimationExportError::GpuError(
                format!("Buffer map error on frame {}: {:?}", frame, e)
            ))?;

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
        if frame_tx.send(rgba_data).is_err() {
            // The receiver is gone, so the writer thread has already
            // returned — and it holds the ONLY copy of ffmpeg's stderr,
            // which is the thing that actually explains the failure.
            // Reporting "writer thread died" here and dropping the
            // handle threw that away and sent people looking at the
            // pipe instead of at the encoder's complaint.
            let reason = match writer_handle.join() {
                Ok(Err(e)) => e,
                Ok(Ok(())) => "ffmpeg exited before all frames were written".to_string(),
                Err(_) => "writer thread panicked".to_string(),
            };
            return Err(AnimationExportError::FfmpegFailed(reason));
        }
        timing_stats.channel_send_time_ms += send_start.elapsed().as_secs_f64() * 1000.0;

        let frame_secs = frame_start.elapsed().as_secs_f64();
        timing_stats.frame_count += 1;
        timing_stats.total_frame_time_ms += frame_secs * 1000.0;
        reporter.progress(
            (frame + 1) as f32 / total_frames as f32,
            &frame_progress_detail(frame + 1, total_frames, frame_secs),
        );
    }

    // Signal writer thread to finish
    drop(frame_tx);

    // Wait for writer thread
    writer_handle
        .join()
        .map_err(|_| AnimationExportError::FfmpegFailed("Writer thread panicked".to_string()))?
        .map_err(AnimationExportError::FfmpegFailed)?;

    // Ensure all GPU work is complete before device is dropped
    // This prevents driver cleanup from interfering with the main device
    let _ = device.poll(PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(5)),
    });

    let total_time_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let avg_frame_time_ms = total_time_ms / total_frames as f64;

    // Log timing summary
    timing_stats.log_summary();

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

    // Video input format (raw RGBA from stdin)
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

    // Audio input (if present)
    let has_audio = config.audio.as_ref().map(|a| a.has_audio()).unwrap_or(false);
    if let Some(ref audio_config) = config.audio {
        if audio_config.has_audio() {
            // Handle negative offset (skip into audio) with -ss before -i
            if audio_config.offset < 0.0 {
                args.push("-ss".to_string());
                args.push(format!("{:.3}", -audio_config.offset));
            }

            // Audio input file
            args.push("-i".to_string());
            args.push(audio_config.file.to_string_lossy().to_string());
        }
    }

    // Codec and hardware acceleration
    let settings = &config.video_settings;
    let encoder = settings
        .hardware_accel
        .encoder_for_codec(settings.codec)
        .expect("Invalid hardware accel + codec combination");

    args.push("-c:v".to_string());
    args.push(encoder.to_string());

    // Quality settings (matching export_animation_fast implementation)
    let is_hardware = settings.hardware_accel != HardwareAccel::None;

    match settings.codec {
        VideoCodec::H264 => {
            if is_hardware {
                match settings.hardware_accel {
                    HardwareAccel::Nvenc => {
                        // FIX: Use constqp mode for constant quality
                        args.push("-rc".to_string());
                        args.push("constqp".to_string());
                        args.push("-qp".to_string());
                        args.push(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            args.push("-preset".to_string());
                            args.push(preset.to_string());
                        }
                    }
                    HardwareAccel::Qsv => {
                        // FIX: Add look_ahead 0 to force constant QP
                        args.push("-global_quality".to_string());
                        args.push(settings.quality.to_string());
                        args.push("-look_ahead".to_string());
                        args.push("0".to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            args.push("-preset".to_string());
                            args.push(preset.to_string());
                        }
                    }
                    HardwareAccel::Amf => {
                        // Already correct: constant QP mode
                        args.push("-rc".to_string());
                        args.push("cqp".to_string());
                        args.push("-qp".to_string());
                        args.push(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            args.push("-preset".to_string());
                            args.push(preset.to_string());
                        }
                    }
                    HardwareAccel::VideoToolbox => {
                        // FIX: Correct quality scale conversion
                        let vt_quality = (100 - (settings.quality as i32 * 100 / 51)).clamp(1, 100);
                        args.push("-q:v".to_string());
                        args.push(vt_quality.to_string());
                    }
                    HardwareAccel::None => {}
                }
            } else {
                // CPU libx264
                args.push("-crf".to_string());
                args.push(settings.quality.to_string());

                // Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                args.push("-preset".to_string());
                args.push(preset.to_string());

                // Apply tune (CPU only)
                if let Some(tune) = settings.tune.ffmpeg_arg() {
                    args.push("-tune".to_string());
                    args.push(tune.to_string());
                }
            }
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::H265 => {
            if is_hardware {
                match settings.hardware_accel {
                    HardwareAccel::Nvenc => {
                        // FIX: Use constqp mode for constant quality
                        args.push("-rc".to_string());
                        args.push("constqp".to_string());
                        args.push("-qp".to_string());
                        args.push(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            args.push("-preset".to_string());
                            args.push(preset.to_string());
                        }
                    }
                    HardwareAccel::Qsv => {
                        // FIX: Add look_ahead 0 to force constant QP
                        args.push("-global_quality".to_string());
                        args.push(settings.quality.to_string());
                        args.push("-look_ahead".to_string());
                        args.push("0".to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            args.push("-preset".to_string());
                            args.push(preset.to_string());
                        }
                    }
                    HardwareAccel::Amf => {
                        // Already correct: constant QP mode
                        args.push("-rc".to_string());
                        args.push("cqp".to_string());
                        args.push("-qp".to_string());
                        args.push(settings.quality.to_string());

                        // Apply preset
                        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                        if !preset.is_empty() {
                            args.push("-preset".to_string());
                            args.push(preset.to_string());
                        }
                    }
                    HardwareAccel::VideoToolbox => {
                        // FIX: Correct quality scale conversion
                        let vt_quality = (100 - (settings.quality as i32 * 100 / 51)).clamp(1, 100);
                        args.push("-q:v".to_string());
                        args.push(vt_quality.to_string());
                    }
                    HardwareAccel::None => {}
                }
            } else {
                // CPU libx265
                args.push("-crf".to_string());
                args.push(settings.quality.to_string());

                // Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                args.push("-preset".to_string());
                args.push(preset.to_string());

                // Apply tune (CPU only)
                if let Some(tune) = settings.tune.ffmpeg_arg() {
                    args.push("-tune".to_string());
                    args.push(tune.to_string());
                }

                args.push("-x265-params".to_string());
                args.push("log-level=error".to_string());
            }
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::VP9 => {
            if settings.hardware_accel == HardwareAccel::Qsv {
                // FIX: Add look_ahead 0 for QSV
                args.push("-global_quality".to_string());
                args.push(settings.quality.to_string());
                args.push("-look_ahead".to_string());
                args.push("0".to_string());

                // Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                if !preset.is_empty() {
                    args.push("-preset".to_string());
                    args.push(preset.to_string());
                }
            } else {
                // CPU libvpx-vp9
                args.push("-crf".to_string());
                args.push(settings.quality.to_string());
                args.push("-b:v".to_string());
                args.push("0".to_string());

                // Apply preset (VP9 uses -speed instead of -preset)
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                args.push("-speed".to_string());
                args.push(preset.to_string());

                // VP9 doesn't support -tune parameter
            }
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
    }

    // Audio encoding (if audio input is present)
    if has_audio {
        if let Some(ref audio_config) = config.audio {
            // Build audio filter chain
            let mut audio_filters: Vec<String> = Vec::new();

            // Positive offset = delay audio start (adelay filter)
            if audio_config.offset > 0.0 {
                let delay_ms = (audio_config.offset * 1000.0) as u64;
                // Apply delay to both channels
                audio_filters.push(format!("adelay={}|{}", delay_ms, delay_ms));
            }

            // Fade in
            if audio_config.fade_in > 0.0 {
                audio_filters.push(format!("afade=t=in:st=0:d={:.3}", audio_config.fade_in));
            }

            // Fade out
            if audio_config.fade_out > 0.0 {
                // Calculate fade out start time
                // This should be: animation_duration - fade_out
                let animation_duration = config.animation.duration;
                let fade_out_start = (animation_duration - audio_config.fade_out).max(0.0);
                audio_filters.push(format!(
                    "afade=t=out:st={:.3}:d={:.3}",
                    fade_out_start, audio_config.fade_out
                ));
            }

            // Apply audio filter chain if any filters exist
            if !audio_filters.is_empty() {
                args.push("-af".to_string());
                args.push(audio_filters.join(","));
            }

            // Audio codec and bitrate
            args.push("-c:a".to_string());
            args.push("aac".to_string());
            args.push("-b:a".to_string());
            args.push(format!("{}k", audio_config.bitrate_kbps));

            // Use shorter of video or audio duration
            args.push("-shortest".to_string());
        }
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigPath, ConfigValue};
    use crate::scene::transforms::{Flame, Transform};

    /// Regression test for the colon-vs-dot variation-param key bug.
    /// `apply_config_value` was writing variation_params keys with `:`
    /// while reads (via `get_variation_param`) use `.`. After the fix,
    /// values written via `apply_config_value` for any of the four
    /// VariationParam ConfigPath variants must be readable via
    /// `Transform::get_variation_param` (the canonical read path used
    /// by `GpuVariationParams::from_transform` for shader packing).
    /// Regression test: animated camera world-space position must reach
    /// the config during video export. `apply_config_value` previously
    /// handled only `CameraZ` (and the rotations) — `CameraX`/`CameraY`
    /// fell through the `_ => {}` catch-all, so animating them did nothing
    /// in the rendered video (they worked in the live timeline because
    /// that path goes through `ConfigManager::set_value`).
    #[test]
    fn test_apply_config_value_camera_position() {
        let mut config = FractalConfig::default();
        for (path, field) in [
            (ConfigPath::CameraX, 1.5_f32),
            (ConfigPath::CameraY, 2.5),
            (ConfigPath::CameraZ, 3.5),
            (ConfigPath::CameraBank, 0.4),
        ] {
            apply_config_value(&mut config, EditingTarget::Main, &path, &ConfigValue::Float(field));
        }
        assert_eq!(config.camera_x, 1.5, "CameraX track must reach config");
        assert_eq!(config.camera_y, 2.5, "CameraY track must reach config");
        assert_eq!(config.camera_z, 3.5);
        assert_eq!(config.camera_bank, 0.4);
    }

    /// Regression test for the broader cluster of FractalConfig-level
    /// animatable params that `apply_config_value` previously dropped in
    /// video export (depth, filter, blend, white level, alpha blend,
    /// levels, and the tonemap/highlight mode enums).
    #[test]
    fn test_apply_config_value_fractalconfig_cluster() {
        use crate::scene::tonemap::{HighlightMode, ToneMapMode};
        let mut config = FractalConfig::default();
        let f = |p, v| (p, ConfigValue::Float(v));
        let cases: Vec<(ConfigPath, ConfigValue)> = vec![
            f(ConfigPath::DofFocusDistance, 1.1),
            f(ConfigPath::DofBlurStrength, 1.2),
            f(ConfigPath::FogStrength, 1.3),
            f(ConfigPath::FogStart, 1.4),
            f(ConfigPath::FilterRadius, 1.5),
            f(ConfigPath::FilterBlurEdges, 1.6),
            f(ConfigPath::BlendFactor, 1.7),
            f(ConfigPath::WhiteLevel, 1.8),
            f(ConfigPath::AlphaBlendLow, 1.9),
            f(ConfigPath::AlphaBlendHigh, 2.0),
            f(ConfigPath::LevelsLow, 2.1),
            f(ConfigPath::LevelsHigh, 2.2),
            f(ConfigPath::LevelsGamma, 2.3),
            (ConfigPath::UseDynamicBlend, ConfigValue::Bool(true)),
            (ConfigPath::LevelsEnabled, ConfigValue::Bool(true)),
            (ConfigPath::TonemapMode, ConfigValue::ToneMapMode(ToneMapMode::Linear)),
            (ConfigPath::HighlightMode, ConfigValue::HighlightMode(HighlightMode::Clip)),
        ];
        for (path, value) in &cases {
            apply_config_value(&mut config, EditingTarget::Main, path, value);
        }
        assert_eq!(config.dof_focus_distance, 1.1);
        assert_eq!(config.dof_blur_strength, 1.2);
        assert_eq!(config.fog_strength, 1.3);
        assert_eq!(config.fog_start, 1.4);
        assert_eq!(config.filter_radius, 1.5);
        assert_eq!(config.filter_blur_edges, 1.6);
        assert_eq!(config.blend_factor, 1.7);
        assert_eq!(config.white_level, 1.8);
        assert_eq!(config.alpha_blend_low, 1.9);
        assert_eq!(config.alpha_blend_high, 2.0);
        assert_eq!(config.levels_low, 2.1);
        assert_eq!(config.levels_high, 2.2);
        assert_eq!(config.levels_gamma, 2.3);
        assert!(config.use_dynamic_blend);
        assert!(config.levels_enabled);
        assert_eq!(config.tonemap_mode, ToneMapMode::Linear);
        assert_eq!(config.highlight_mode, HighlightMode::Clip);
    }

    #[test]
    fn test_apply_config_value_variation_param_roundtrip() {
        let mut config = FractalConfig::default();
        config.flame = Flame::new();
        config.flame.transforms.push(Transform::new());
        config.flame.linked_transforms.push(Transform::new());
        config.flame.final_transforms.push(Transform::new());

        // Normal pool
        apply_config_value(
            &mut config,
            EditingTarget::Main,
            &ConfigPath::TransformVariationParam {
                index: 0,
                variation: "julian".to_string(),
                param: "power".to_string(),
            },
            &ConfigValue::Float(2.5),
        );
        assert_eq!(
            config.flame.transforms[0].get_variation_param("julian", "power"),
            Some(2.5),
            "Normal pool: param must be readable via canonical key format",
        );

        // Final pool (post-Phase-9 indexed variant; legacy unindexed
        // form lands here too via the migration shim in
        // ConfigPath::from_string_key).
        apply_config_value(
            &mut config,
            EditingTarget::Main,
            &ConfigPath::FinalTransformVariationParam {
                index: 0,
                variation: "bipolar".to_string(),
                param: "shift".to_string(),
            },
            &ConfigValue::Float(1.5),
        );
        assert_eq!(
            config.flame.final_transforms[0].get_variation_param("bipolar", "shift"),
            Some(1.5),
            "Final pool: param must reach final_transforms[0]",
        );

        // Linked pool
        apply_config_value(
            &mut config,
            EditingTarget::Main,
            &ConfigPath::LinkedTransformVariationParam {
                index: 0,
                variation: "blob".to_string(),
                param: "high".to_string(),
            },
            &ConfigValue::Float(3.0),
        );
        assert_eq!(
            config.flame.linked_transforms[0].get_variation_param("blob", "high"),
            Some(3.0),
            "Linked pool: param must be readable",
        );

        // PoolFinal pool
        apply_config_value(
            &mut config,
            EditingTarget::Main,
            &ConfigPath::FinalTransformVariationParam {
                index: 0,
                variation: "blob".to_string(),
                param: "low".to_string(),
            },
            &ConfigValue::Float(0.5),
        );
        assert_eq!(
            config.flame.final_transforms[0].get_variation_param("blob", "low"),
            Some(0.5),
            "PoolFinal: param must be readable",
        );
    }
}
