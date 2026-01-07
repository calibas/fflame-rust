//! Custom palette storage - user-saved palettes persisted across sessions
//!
//! Palettes are stored as a "Custom" pack that appears in the Palette Library.
//! Desktop: JSON file in user data directory
//! WASM: localStorage

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::scene::palette::Palette;

/// Storage format version for custom palettes
pub const CURRENT_VERSION: u32 = 1;

/// Custom palette storage - persisted across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPaletteLibrary {
    /// Format version for migrations
    #[serde(default = "default_version")]
    pub version: u32,
    /// User-saved palettes
    #[serde(default)]
    pub palettes: Vec<Palette>,
}

fn default_version() -> u32 {
    CURRENT_VERSION
}

impl Default for CustomPaletteLibrary {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            palettes: Vec::new(),
        }
    }
}

impl CustomPaletteLibrary {
    /// Storage path for custom palettes
    const STORAGE_PATH: &'static str = "custom_palettes.json";

    /// Load custom palettes from persistent storage
    /// Returns empty library if file doesn't exist or fails to parse
    pub fn load() -> Self {
        let path = Path::new(Self::STORAGE_PATH);

        match super::backend::read_file(path) {
            Ok(json) => {
                match serde_json::from_str::<Self>(&json) {
                    Ok(mut library) => {
                        // Mark all loaded palettes as non-built-in (editable)
                        for palette in &mut library.palettes {
                            palette.built_in = false;
                        }
                        log::info!("Loaded {} custom palette(s) from storage", library.palettes.len());
                        library
                    }
                    Err(e) => {
                        log::error!("Failed to parse custom palettes: {}", e);
                        Self::default()
                    }
                }
            }
            Err(super::backend::StorageError::NotFound(_)) => {
                log::info!("No custom palettes found, starting with empty library");
                Self::default()
            }
            Err(e) => {
                log::error!("Failed to load custom palettes: {}", e);
                Self::default()
            }
        }
    }

    /// Save custom palettes to persistent storage
    pub fn save(&self) -> Result<(), super::backend::StorageError> {
        let json = serde_json::to_string_pretty(self)?;
        let path = Path::new(Self::STORAGE_PATH);

        super::backend::write_file(path, &json)?;
        log::info!("Saved {} custom palette(s) to storage", self.palettes.len());

        Ok(())
    }

    /// Add a palette to the library and save
    /// If a palette with the same name exists, it will be overwritten
    pub fn add_palette(&mut self, palette: Palette) -> Result<(), super::backend::StorageError> {
        let mut palette = palette;
        palette.built_in = false; // Ensure it's editable

        // Check for existing palette with same name (case-insensitive)
        if let Some(idx) = self.find_palette_index_by_name(&palette.name) {
            // Overwrite existing
            self.palettes[idx] = palette;
        } else {
            // Add new
            self.palettes.push(palette);
        }
        self.save()
    }

    /// Check if a palette with the given name exists (case-insensitive)
    pub fn has_palette_by_name(&self, name: &str) -> bool {
        self.find_palette_index_by_name(name).is_some()
    }

    /// Find palette index by name (case-insensitive)
    fn find_palette_index_by_name(&self, name: &str) -> Option<usize> {
        let name_lower = name.to_lowercase();
        self.palettes.iter().position(|p| p.name.to_lowercase() == name_lower)
    }

    /// Delete a palette by name (case-insensitive)
    /// Returns true if a palette was deleted, false if not found
    pub fn delete_palette_by_name(&mut self, name: &str) -> Result<bool, super::backend::StorageError> {
        if let Some(idx) = self.find_palette_index_by_name(name) {
            self.palettes.remove(idx);
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if library is empty
    pub fn is_empty(&self) -> bool {
        self.palettes.is_empty()
    }

    /// Number of palettes
    pub fn len(&self) -> usize {
        self.palettes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::palette::ColorStop;

    #[test]
    fn test_default_library() {
        let lib = CustomPaletteLibrary::default();
        assert_eq!(lib.version, CURRENT_VERSION);
        assert!(lib.palettes.is_empty());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut lib = CustomPaletteLibrary::default();
        lib.palettes.push(Palette::new(
            "Test Palette",
            vec![
                ColorStop { position: 0.0, color: [1.0, 0.0, 0.0] },
                ColorStop { position: 1.0, color: [0.0, 0.0, 1.0] },
            ],
        ));

        let json = serde_json::to_string(&lib).unwrap();
        let restored: CustomPaletteLibrary = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.palettes.len(), 1);
        assert_eq!(restored.palettes[0].name, "Test Palette");
    }
}
