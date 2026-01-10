//! Preset resource loading
//!
//! Loads fractal presets from assets/presets.fflame for both Desktop and WASM.

use super::{fetch_text, FetchError};
use crate::config::FractalConfig;

/// Preset file path (relative to assets)
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

/// Load presets from file (desktop - synchronous)
#[cfg(not(target_arch = "wasm32"))]
pub fn load_presets() -> Result<Vec<FractalConfig>, FetchError> {
    let text = fetch_text(PRESETS_PATH)?;
    FractalConfig::from_json_multi(&text)
        .map_err(|e| FetchError::Parse(format!("Failed to parse presets: {}", e)))
}

/// Load presets from file (WASM - async)
#[cfg(target_arch = "wasm32")]
pub async fn load_presets() -> Result<Vec<FractalConfig>, FetchError> {
    let text = fetch_text(PRESETS_PATH).await?;
    FractalConfig::from_json_multi(&text)
        .map_err(|e| FetchError::Parse(format!("Failed to parse presets: {}", e)))
}

/// Load presets with fallback to default
///
/// Returns the loaded presets, or a single default preset if loading fails.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_presets_with_fallback() -> Vec<FractalConfig> {
    match load_presets() {
        Ok(presets) if !presets.is_empty() => {
            log::info!("Loaded {} presets from {}", presets.len(), PRESETS_PATH);
            presets
        }
        Ok(_) => {
            log::warn!("Preset file was empty, using default preset");
            vec![create_default_preset()]
        }
        Err(e) => {
            log::error!("Failed to load presets from {}: {}", PRESETS_PATH, e);
            log::info!("Using default preset as fallback");
            vec![create_default_preset()]
        }
    }
}

/// Load presets with fallback to default (WASM - async)
#[cfg(target_arch = "wasm32")]
pub async fn load_presets_with_fallback() -> Vec<FractalConfig> {
    match load_presets().await {
        Ok(presets) if !presets.is_empty() => {
            log::info!("Loaded {} presets from {}", presets.len(), PRESETS_PATH);
            presets
        }
        Ok(_) => {
            log::warn!("Preset file was empty, using default preset");
            vec![create_default_preset()]
        }
        Err(e) => {
            log::error!("Failed to load presets from {}: {}", PRESETS_PATH, e);
            log::info!("Using default preset as fallback");
            vec![create_default_preset()]
        }
    }
}
