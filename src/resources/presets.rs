//! Preset resource loading
//!
//! Loads fractal presets from embedded data (compile-time) or assets file (desktop).
//! Falls back to a simple default preset if loading fails.

use super::FetchError;
use crate::config::FractalConfig;

/// Presets file embedded at compile time for offline/WASM fallback
pub const PRESETS_JSON: &str = include_str!("../../assets/presets.fflame");

/// Preset file path for desktop runtime loading (optional)
pub const PRESETS_PATH: &str = "assets/presets.fflame";

/// Create the default fallback preset (identity transform + linear variation)
///
/// This is used when the presets file fails to load.
pub fn create_default_preset() -> FractalConfig {
    use crate::scene::transforms::Transform;

    let mut transform = Transform::new();
    // Identity affine (already default in Transform::new())
    // Single linear variation at weight 1.0
    transform.variations.insert("linear".to_string(), 1.0);
    transform.weight = 1.0;
    transform.color = 0.5; // Middle of palette

    let mut config = FractalConfig::default();
    config.flame.name = "Default".to_string();
    config.flame.transforms = vec![transform];
    config
}

/// Load presets from embedded data
pub fn load_embedded_presets() -> Result<Vec<FractalConfig>, FetchError> {
    FractalConfig::from_json_multi(PRESETS_JSON)
        .map_err(|e| FetchError::Parse(format!("Failed to parse embedded presets: {}", e)))
}

/// Load presets from file (desktop only - tries file first, falls back to embedded)
#[cfg(not(target_arch = "wasm32"))]
pub fn load_presets() -> Result<Vec<FractalConfig>, FetchError> {
    use super::fetch_text;

    // Try loading from file first (allows runtime updates without recompile)
    match fetch_text(PRESETS_PATH) {
        Ok(text) => {
            FractalConfig::from_json_multi(&text)
                .map_err(|e| FetchError::Parse(format!("Failed to parse presets: {}", e)))
        }
        Err(e) => {
            log::info!("Falling back to embedded presets (file error: {})", e);
            load_embedded_presets()
        }
    }
}

/// Load presets (WASM - uses embedded data, synchronous)
#[cfg(target_arch = "wasm32")]
pub fn load_presets() -> Result<Vec<FractalConfig>, FetchError> {
    load_embedded_presets()
}

/// Load presets with fallback to default
///
/// Returns the loaded presets, or a single default preset if loading fails.
/// Works synchronously on both Desktop and WASM.
pub fn load_presets_with_fallback() -> Vec<FractalConfig> {
    match load_presets() {
        Ok(presets) if !presets.is_empty() => {
            log::info!("Loaded {} presets", presets.len());
            presets
        }
        Ok(_) => {
            log::warn!("Preset file was empty, using default preset");
            vec![create_default_preset()]
        }
        Err(e) => {
            log::error!("Failed to load presets: {}", e);
            log::info!("Using default preset as fallback");
            vec![create_default_preset()]
        }
    }
}
