//! HTTP Resource Fetching System
//!
//! A unified system for fetching resources (palettes, fractals, animations)
//! from HTTP sources, working consistently across desktop and WASM platforms.

mod error;
mod fetch;

pub use error::FetchError;
pub use fetch::{fetch_json, fetch_text, FetchHandle, FetchResult, FetchState};

use serde::{Deserialize, Serialize};

/// Load state for any fetchable resource pack
#[derive(Debug, Clone, PartialEq)]
pub enum LoadState {
    /// Metadata known, content not yet fetched
    NotLoaded,
    /// Fetch is in progress
    Loading,
    /// Content successfully loaded
    Loaded,
    /// Fetch failed with error message
    Failed(String),
}

impl Default for LoadState {
    fn default() -> Self {
        Self::NotLoaded
    }
}

/// Metadata for a resource pack (from manifest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackMetadata {
    /// Unique identifier for this pack
    pub id: String,
    /// Display name
    pub name: String,
    /// Description of pack contents
    #[serde(default)]
    pub description: String,
    /// Relative path/URL to the pack JSON file
    pub file: String,
    /// Number of items in this pack
    #[serde(default)]
    pub item_count: usize,
    /// Whether this pack should be loaded by default
    #[serde(default)]
    pub enabled_by_default: bool,
}

/// Manifest listing available packs for a resource type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceManifest {
    /// Manifest format version
    pub version: u32,
    /// Type of resources: "palettes", "presets", or "animations"
    pub resource_type: String,
    /// Available packs
    pub packs: Vec<PackMetadata>,
}

/// Generic pack info with load state
///
/// This wraps pack metadata with runtime state for tracking
/// whether the pack content has been fetched.
pub struct PackInfo<T> {
    /// Metadata from manifest
    pub metadata: PackMetadata,
    /// Current load state
    pub load_state: LoadState,
    /// Pack content (None until loaded)
    pub content: Option<T>,
}

impl<T> PackInfo<T> {
    /// Create a new PackInfo from metadata (not yet loaded)
    pub fn new(metadata: PackMetadata) -> Self {
        Self {
            metadata,
            load_state: LoadState::NotLoaded,
            content: None,
        }
    }

    /// Check if this pack is loaded and ready to use
    pub fn is_loaded(&self) -> bool {
        matches!(self.load_state, LoadState::Loaded)
    }

    /// Check if this pack is currently loading
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

    /// Mark as loading
    pub fn set_loading(&mut self) {
        self.load_state = LoadState::Loading;
    }

    /// Mark as loaded with content
    pub fn set_loaded(&mut self, content: T) {
        self.load_state = LoadState::Loaded;
        self.content = Some(content);
    }

    /// Mark as failed
    pub fn set_failed(&mut self, error: String) {
        self.load_state = LoadState::Failed(error);
        self.content = None;
    }
}
