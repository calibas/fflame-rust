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

    /// Rendering settings
    pub density_scale: f32,
    pub speed_factor: f32,

    /// Color settings
    pub color_mode: ColorMode,
    pub palette_index: usize,
    /// The actual palette data (for complete reproducibility)
    /// If None, will use palette_index from library
    #[serde(default)]
    pub palette: Option<Palette>,
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

fn default_true() -> bool {
    true
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
