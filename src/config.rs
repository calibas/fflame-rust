use serde::{Deserialize, Serialize};
use crate::scene::transforms::Flame;
use crate::scene::palette::ColorMode;

/// Complete fractal configuration (excludes runtime-only settings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalConfig {
    /// The flame (transforms)
    pub flame: Flame,

    /// View settings
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub rotation: f32,

    /// Rendering settings
    pub density_scale: f32,
    pub speed_factor: f32,

    /// Color settings
    pub color_mode: ColorMode,
    pub palette_index: usize,
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
