//! Local storage system for persistent application data
//!
//! This module provides cross-platform storage for:
//! - System settings (device-specific preferences)
//! - Custom fractal library
//! - Custom palette library
//! - Custom workspace layouts
//! - Auto-backup system
//! - Thumbnail cache for gallery previews
//!
//! Storage backends:
//! - Desktop (Windows/macOS): Filesystem (via `directories` crate)
//! - WASM (Browser): localStorage for small data, IndexedDB for large data

pub mod backend;
#[cfg(not(target_arch = "wasm32"))]
pub mod credentials;
pub mod custom_palettes;
pub mod settings;
pub mod thumbnail_cache;
pub mod catalog;
pub mod effect_cache;
pub mod plugins;
pub mod effect_catalog;
pub mod variation_cache;
pub mod variation_catalog;

pub use backend::{StorageError, StorageResult};
pub use custom_palettes::CustomPaletteLibrary;
pub use settings::{FlyCameraMode, SystemSettings};
pub use thumbnail_cache::{ThumbnailCache, GalleryItem, TextureCache};

/// Register everything installed on this machine that did not ship with
/// the app: cached downloads, then local plugins.
///
/// **Every entry point that renders must call this**, not just the GUI.
/// It was `App::new`-only, so a headless export silently dropped both —
/// a flame using a downloaded variation or a local plugin rendered
/// without it and reported nothing, because a missing variation is a
/// weight that contributes zero rather than an error. That is the exact
/// failure mode this project keeps designing against, arrived at through
/// an entry point rather than through a code path.
///
/// Order matters and is the same as everywhere else: downloads first,
/// plugins last, so a name collision is caught against everything
/// already present and the user's file is the one refused.
///
/// Idempotent — registering the same resource twice is a replace, and
/// the collision rules make a second call a no-op.
pub fn load_installed_resources() -> plugins::PluginLoadReport {
    crate::variations::load_cached_api_variations();
    effect_cache::load_all_into_registry();
    plugins::load_all()
}
