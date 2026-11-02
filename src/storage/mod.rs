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
pub mod resource_store;
pub mod effect_cache;
pub mod effect_catalog;
pub mod formula_store;
pub mod links;
pub mod library;
pub mod variation_cache;
pub mod variation_catalog;

pub use backend::{StorageError, StorageResult};
pub use custom_palettes::CustomPaletteLibrary;
pub use settings::{FlyCameraMode, SystemSettings};
pub use thumbnail_cache::{shared_thumbnail_cache, ThumbnailCache, GalleryItem, TextureCache};

/// Register everything installed on this machine that did not ship with
/// the app: the cached downloads.
///
/// **Every entry point that renders must call this**, not just the GUI.
/// It was `App::new`-only, so a headless export silently dropped both —
/// a flame using a downloaded variation or a local plugin rendered
/// without it and reported nothing, because a missing variation is a
/// weight that contributes zero rather than an error. That is the exact
/// failure mode this project keeps designing against, arrived at through
/// an entry point rather than through a code path.
///
/// Idempotent — registering the same resource twice is a replace, and
/// the collision rules make a second call a no-op.
///
/// Returns the files of the retired WGSL plug-in system that are still
/// present, which are no longer loaded ([`retired_plugin_files`]).
pub fn load_installed_resources() -> Vec<String> {
    variation_cache::load_all_into_registry();
    effect_cache::load_all_into_registry();
    // The user's Rhai plug-ins (P3), after the downloads: a download
    // cannot displace one, and one cannot displace a built-in.
    library::load_plugins();
    retired_plugin_files()
}

/// Files left in the retired WGSL plug-in folders
/// (`plugins/variations/*.json`, `plugins/effects/*.json`), which are no
/// longer loaded.
///
/// Retired 2026-11-01 (docs/projects/rhai-plugins.md, D3): nothing in
/// the UI ever installed one, and Rhai plug-ins replace them. Listed for
/// one release so a user who placed one by hand learns why it is gone;
/// the files are left where they are.
pub fn retired_plugin_files() -> Vec<String> {
    let mut found = Vec::new();
    for dir in ["plugins/variations", "plugins/effects"] {
        if let Ok(entries) = backend::list_entries(std::path::Path::new(dir)) {
            found.extend(
                entries
                    .into_iter()
                    .filter(|n| n.ends_with(".json") && !n.starts_with('_'))
                    .map(|n| format!("{dir}/{n}")),
            );
        }
    }
    found.sort();
    found
}
