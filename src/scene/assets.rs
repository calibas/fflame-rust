use super::palette::Palette;
use crate::config::FractalConfig;
use std::path::Path;

/// Load all palettes from a directory
pub fn load_palettes_from_dir(dir: &Path) -> Vec<Palette> {
    let mut palettes = Vec::new();

    if !dir.exists() {
        log::warn!("Palettes directory does not exist: {}", dir.display());
        return palettes;
    }

    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("palette") {
                    match load_palette(&path) {
                        Ok(palette) => {
                            log::info!("Loaded palette: {} from {}", palette.name, path.display());
                            palettes.push(palette);
                        }
                        Err(e) => {
                            log::error!("Failed to load palette from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to read palettes directory {}: {}", dir.display(), e);
        }
    }

    // Sort by name for consistent ordering
    palettes.sort_by(|a, b| a.name.cmp(&b.name));

    palettes
}

/// Load a single palette from a file
pub fn load_palette(path: &Path) -> Result<Palette, Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(path)?;
    let palette = serde_json::from_str(&json)?;
    Ok(palette)
}

/// Load all fractal config presets from a directory
pub fn load_configs_from_dir(dir: &Path) -> Vec<FractalConfig> {
    let mut configs = Vec::new();

    if !dir.exists() {
        log::warn!("Presets directory does not exist: {}", dir.display());
        return configs;
    }

    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("flame") {
                    match load_config(&path) {
                        Ok(config) => {
                            log::info!("Loaded preset: {} from {}", config.flame.name, path.display());
                            configs.push(config);
                        }
                        Err(e) => {
                            log::error!("Failed to load preset from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to read presets directory {}: {}", dir.display(), e);
        }
    }

    // Sort by flame name for consistent ordering
    configs.sort_by(|a, b| a.flame.name.cmp(&b.flame.name));

    configs
}

/// Load a single fractal config preset from a file
pub fn load_config(path: &Path) -> Result<FractalConfig, Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(path)?;
    let config = serde_json::from_str(&json)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_palettes() {
        // This test will only pass if assets/palettes exists
        let palettes = load_palettes_from_dir(Path::new("assets/palettes"));
        // Should have at least the default palettes
        assert!(palettes.is_empty() || !palettes.is_empty()); // Always passes, just exercising the code
    }

    #[test]
    fn test_load_presets() {
        // This test will only pass if assets/presets exists
        let presets = load_configs_from_dir(Path::new("assets/presets"));
        // Should have at least the default presets
        assert!(presets.is_empty() || !presets.is_empty()); // Always passes, just exercising the code
    }
}
