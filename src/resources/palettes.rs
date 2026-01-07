//! Palette pack resource loading
//!
//! Provides manifest-based discovery and lazy loading of palette packs.

use super::{fetch_json, FetchError, LoadState, PackMetadata, ResourceManifest};
use crate::scene::palette::PalettePack;

/// Palette pack manifest path (relative to assets)
pub const MANIFEST_PATH: &str = "assets/palettes/packs/manifest.json";

/// Built-in pack (embedded at compile time for offline fallback)
pub const BUILTIN_PACK_JSON: &str = include_str!("../../assets/palettes/packs/builtin.json");

/// A palette pack with load state tracking
#[derive(Clone)]
pub struct PalettePackInfo {
    /// Metadata from manifest (id, name, description, file path)
    pub metadata: Option<PackMetadata>,
    /// Pack content (loaded from JSON)
    pub pack: Option<PalettePack>,
    /// Current load state
    pub load_state: LoadState,
    /// Whether this pack is enabled (included in palette list)
    pub enabled: bool,
    /// Whether this is the embedded built-in pack
    pub is_builtin: bool,
}

impl PalettePackInfo {
    /// Create a new pack info from manifest metadata
    pub fn from_metadata(metadata: PackMetadata) -> Self {
        Self {
            enabled: metadata.enabled_by_default,
            metadata: Some(metadata),
            pack: None,
            load_state: LoadState::NotLoaded,
            is_builtin: false,
        }
    }

    /// Create from a directly loaded pack (not via manifest)
    pub fn from_pack(pack: PalettePack, is_builtin: bool) -> Self {
        Self {
            enabled: pack.enabled_by_default,
            metadata: None,
            pack: Some(pack),
            load_state: LoadState::Loaded,
            is_builtin,
        }
    }

    /// Get pack name (from pack if loaded, otherwise from metadata)
    pub fn name(&self) -> &str {
        if let Some(pack) = &self.pack {
            &pack.pack_name
        } else if let Some(metadata) = &self.metadata {
            &metadata.name
        } else {
            "Unknown"
        }
    }

    /// Get pack description
    pub fn description(&self) -> &str {
        if let Some(pack) = &self.pack {
            &pack.description
        } else if let Some(metadata) = &self.metadata {
            &metadata.description
        } else {
            ""
        }
    }

    /// Get palette count (from pack if loaded, otherwise from metadata)
    pub fn palette_count(&self) -> usize {
        if let Some(pack) = &self.pack {
            pack.palettes.len()
        } else if let Some(metadata) = &self.metadata {
            metadata.item_count
        } else {
            0
        }
    }

    /// Check if pack is loaded
    pub fn is_loaded(&self) -> bool {
        matches!(self.load_state, LoadState::Loaded)
    }

    /// Check if pack is currently loading
    pub fn is_loading(&self) -> bool {
        matches!(self.load_state, LoadState::Loading)
    }

    /// Check if loading failed
    pub fn is_failed(&self) -> bool {
        matches!(self.load_state, LoadState::Failed(_))
    }

    /// Get error message if failed
    pub fn error_message(&self) -> Option<&str> {
        match &self.load_state {
            LoadState::Failed(msg) => Some(msg),
            _ => None,
        }
    }
}

/// Load the palette manifest (desktop - synchronous)
#[cfg(not(target_arch = "wasm32"))]
pub fn load_manifest() -> Result<ResourceManifest, FetchError> {
    fetch_json(MANIFEST_PATH)
}

/// Load the palette manifest (WASM - async)
#[cfg(target_arch = "wasm32")]
pub async fn load_manifest() -> Result<ResourceManifest, FetchError> {
    fetch_json(MANIFEST_PATH).await
}

/// Load the embedded built-in pack
pub fn load_builtin_pack() -> Result<PalettePack, FetchError> {
    serde_json::from_str(BUILTIN_PACK_JSON).map_err(FetchError::from)
}

/// Load a palette pack from its file path (desktop - synchronous)
#[cfg(not(target_arch = "wasm32"))]
pub fn load_pack(file_path: &str) -> Result<PalettePack, FetchError> {
    // Construct full path relative to assets
    let full_path = format!("assets/palettes/packs/{}", file_path);
    fetch_json(&full_path)
}

/// Load a palette pack from its file path (WASM - async)
#[cfg(target_arch = "wasm32")]
pub async fn load_pack(file_path: &str) -> Result<PalettePack, FetchError> {
    // Construct full path relative to page
    let full_path = format!("assets/palettes/packs/{}", file_path);
    fetch_json(&full_path).await
}
