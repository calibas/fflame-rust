use serde::{Deserialize, Serialize};
use crate::scene::transforms::Flame;
use crate::scene::palette::{ColorMode, Palette};
use crate::scene::tonemap::{ToneMapMode, ToneCurve};

/// Complete fractal configuration (excludes runtime-only settings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalConfig {
    /// The flame (transforms)
    pub flame: Flame,

    /// View settings
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub rotation: f32,  // 2D rotation (around Z axis)

    /// 3D Camera rotation (for 3D mode)
    #[serde(default)]
    pub camera_rotation_x: f32,  // Pitch (rotation around X axis)
    #[serde(default)]
    pub camera_rotation_y: f32,  // Yaw (rotation around Y axis)
    #[serde(default)]
    pub camera_z: f32,  // Camera Z position (height)

    /// Rendering settings
    pub density_scale: f32,
    pub speed_factor: f32,
    /// Maximum total iterations to render (default: 1 billion = ~infinite for interactive use)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u64,
    /// Histogram color scale (precision vs overflow protection, default: 10.0)
    #[serde(default = "default_histogram_color_scale")]
    pub histogram_color_scale: f32,
    /// Low-density smoothing (0.0 = no smoothing, 1.0 = maximum smoothing, default: 0.5)
    #[serde(default = "default_low_density_smoothing")]
    pub low_density_smoothing: f32,
    /// Density compression strength (0.0 = linear, 100.0 = strong compression, default: 0.0)
    #[serde(default)]
    pub density_compression_strength: f32,
    /// Blend factor for accumulation (0.01 = slow/smooth, 1.0 = fast/flickery, default: 0.1)
    #[serde(default = "default_blend_factor")]
    pub blend_factor: f32,
    /// Use dynamic blend (true = exponential convergence, false = fixed blend rate, default: true)
    #[serde(default = "default_use_dynamic_blend")]
    pub use_dynamic_blend: bool,
    /// Per-pixel iteration limit (0 = disabled, default: 0)
    #[serde(default)]
    pub target_iterations_per_pixel: u32,
    /// Iterations per thread (GPU workgroup performance tuning, default: 256)
    #[serde(default = "default_iterations_per_thread")]
    pub iterations_per_thread: u32,
    /// Speed multiplier for frame rate (1x-16x, affects quality consistency, default: 1)
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: u32,

    /// Color settings
    pub color_mode: ColorMode,
    pub palette_index: usize,
    /// The actual palette data (for complete reproducibility)
    /// If None, will use palette_index from library
    #[serde(default)]
    pub palette: Option<Palette>,
    /// Palette rotation: -1.0 to 1.0, shifts palette indices (Apophysis: -128 to 128)
    #[serde(default = "default_palette_rotation")]
    pub palette_rotation: f32,
    pub background_color: [f32; 3],

    /// Tone mapping settings
    #[serde(default)]
    pub tonemap_mode: ToneMapMode,
    #[serde(default)]
    pub tonemap_curve: ToneCurve,
    /// Whether to actually apply the tone curve
    #[serde(default = "default_true")]
    pub use_curve: bool,
    #[serde(default = "default_exposure")]
    pub exposure: f32,
    #[serde(default = "default_gamma")]
    pub gamma: f32,
    /// Gamma threshold: smooths gamma curve at low densities (Apophysis compatibility)
    /// Default 0.0025 prevents harsh darkening in sparse areas
    #[serde(default = "default_gamma_threshold")]
    pub gamma_threshold: f32,
    /// Brightness: logarithmic brightness scaling (Apophysis compatibility)
    /// 1.0 = standard brightness (default), higher = brighter
    #[serde(default = "default_brightness")]
    pub brightness: f32,
    /// Vibrancy: blend between old (gamma-only) and new (vibrant) color algorithms
    /// 1.0 = modern vibrant colors (default), 0.0 = classic gamma-only colors
    #[serde(default = "default_vibrancy")]
    pub vibrancy: f32,
    /// Saturation: color saturation boost (1.0 = no change, >1.0 = more saturated)
    #[serde(default = "default_saturation")]
    pub saturation: f32,

    /// Hue shift: rotate hue in degrees (-180.0 to 180.0, 0.0 = no shift)
    #[serde(default = "default_hue_shift")]
    pub hue_shift: f32,

    /// Value scale: brightness multiplier (1.0 = no change, >1.0 = brighter)
    #[serde(default = "default_value_scale")]
    pub value_scale: f32,

    /// Optional: Deterministic RNG for reproducible renders
    #[serde(default)]
    pub deterministic_rng: bool,
}

fn default_exposure() -> f32 {
    1.0
}

fn default_gamma() -> f32 {
    2.2
}

fn default_gamma_threshold() -> f32 {
    super::defaults::DEFAULT_GAMMA_THRESHOLD
}

fn default_brightness() -> f32 {
    super::defaults::DEFAULT_BRIGHTNESS
}

fn default_vibrancy() -> f32 {
    1.0  // Modern vibrant colors by default
}

fn default_saturation() -> f32 {
    super::defaults::DEFAULT_SATURATION
}

fn default_hue_shift() -> f32 {
    super::defaults::DEFAULT_HUE_SHIFT
}

fn default_value_scale() -> f32 {
    super::defaults::DEFAULT_VALUE_SCALE
}

fn default_palette_rotation() -> f32 {
    super::defaults::DEFAULT_PALETTE_ROTATION
}

fn default_true() -> bool {
    true
}

fn default_max_iterations() -> u64 {
    super::defaults::DEFAULT_MAX_ITERATIONS
}

fn default_histogram_color_scale() -> f32 {
    super::defaults::DEFAULT_HISTOGRAM_COLOR_SCALE
}

fn default_low_density_smoothing() -> f32 {
    super::defaults::DEFAULT_LOW_DENSITY_SMOOTHING
}

fn default_blend_factor() -> f32 {
    super::defaults::DEFAULT_BLEND_FACTOR
}

fn default_use_dynamic_blend() -> bool {
    super::defaults::DEFAULT_USE_DYNAMIC_BLEND
}

fn default_iterations_per_thread() -> u32 {
    super::defaults::DEFAULT_ITERATIONS_PER_THREAD
}

fn default_speed_multiplier() -> u32 {
    super::defaults::DEFAULT_SPEED_MULTIPLIER
}

impl Default for FractalConfig {
    fn default() -> Self {
        use crate::scene::transforms::Flame;
        use crate::scene::tonemap::ToneCurve;

        Self {
            flame: Flame::default(),
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            camera_z: 0.0,
            density_scale: 1.0,
            speed_factor: 0.5,
            max_iterations: default_max_iterations(),
            histogram_color_scale: default_histogram_color_scale(),
            low_density_smoothing: default_low_density_smoothing(),
            density_compression_strength: 0.0,
            blend_factor: default_blend_factor(),
            use_dynamic_blend: default_use_dynamic_blend(),
            target_iterations_per_pixel: 0,
            iterations_per_thread: default_iterations_per_thread(),
            speed_multiplier: default_speed_multiplier(),
            color_mode: ColorMode::Transform,
            palette_index: 0,
            palette: None,
            palette_rotation: default_palette_rotation(),
            background_color: [0.0, 0.0, 0.0],
            tonemap_mode: ToneMapMode::default(),
            tonemap_curve: ToneCurve::default(),
            use_curve: default_true(),
            exposure: default_exposure(),
            gamma: default_gamma(),
            gamma_threshold: default_gamma_threshold(),
            brightness: default_brightness(),
            vibrancy: default_vibrancy(),
            saturation: default_saturation(),
            hue_shift: default_hue_shift(),
            value_scale: default_value_scale(),
            deterministic_rng: false,
        }
    }
}

impl FractalConfig {
    /// Export configuration to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Import configuration from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Export to JSON file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import from JSON file
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&json)?)
    }
}
