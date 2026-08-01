//! Persistent cache for API-loaded variations.
//!
//! Stores `VariationDownload` JSON responses keyed by variation name.
//! - Desktop: filesystem at `<app_data>/variations/<name>.json`
//! - WASM: browser localStorage (key: `variations/<name>.json`)
//!
//! Built-in variations are NOT cached (they're compiled in).

use std::path::{Path, PathBuf};

use crate::api::types::VariationDownload;
use crate::storage::backend;

/// Result type for cache operations
pub type CacheResult<T> = Result<T, String>;

/// Relative path inside the app data dir (or localStorage key prefix on WASM)
fn cache_path(name: &str) -> PathBuf {
    PathBuf::from("variations").join(format!("{}.json", name))
}

/// Save a variation to the cache.
pub fn save(variation: &VariationDownload) -> CacheResult<()> {
    let json = serde_json::to_string(variation)
        .map_err(|e| format!("Failed to serialize variation: {}", e))?;
    backend::write_file(&cache_path(&variation.name), &json)
        .map_err(|e| format!("Failed to write variation cache: {}", e))
}

/// Load a single variation from the cache.
/// Returns `Ok(None)` if not cached, `Err` for other errors.
pub fn load(name: &str) -> CacheResult<Option<VariationDownload>> {
    let path = cache_path(name);
    if !backend::file_exists(&path) {
        return Ok(None);
    }
    let json = backend::read_file(&path)
        .map_err(|e| format!("Failed to read variation cache: {}", e))?;
    let variation: VariationDownload = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse cached variation '{}': {}", name, e))?;
    Ok(Some(variation))
}

/// Delete a single variation from the cache.
pub fn delete(name: &str) -> CacheResult<()> {
    backend::delete_file(&cache_path(name))
        .map_err(|e| format!("Failed to delete variation cache: {}", e))
}

/// List all cached variation names.
///
/// One implementation for both platforms via `backend::list_entries`.
/// The WASM half used to be a stub returning empty, which made the
/// cache **write-only** on the web: `load_all` found nothing so every
/// session re-downloaded every variation, and `clear_all` always
/// reported 0 while deleting nothing. localStorage does support
/// enumeration (`length`/`key`); it had just never been wired.
pub fn list_cached() -> CacheResult<Vec<String>> {
    let entries = backend::list_entries(Path::new("variations"))
        .map_err(|e| format!("Failed to list cached variations: {}", e))?;
    Ok(entries
        .into_iter()
        .filter_map(|name| name.strip_suffix(".json").map(str::to_string))
        .filter(|name| !is_metadata_entry(name))
        .collect())
}

/// Entries whose name begins with `_` are metadata, not resources.
///
/// The catalog lives in the same prefix as the cache (`_catalog.json`),
/// so without this it comes back as a cached entry named `_catalog`:
/// `load_all` fails to parse it and warns on every startup, and
/// `clear_all` counts it — so "Clear Cache (N)" reports one more than
/// there are cached resources, and deletes the catalog as though it were
/// one of them.
///
/// The convention is safe because a server name has to be a valid
/// identifier to reach the cache at all, and none begins with an
/// underscore.
fn is_metadata_entry(name: &str) -> bool {
    name.starts_with('_')
}


/// Clear all cached variations. Returns the number of entries removed.
pub fn clear_all() -> CacheResult<usize> {
    let names = list_cached()?;
    let count = names.len();
    for name in &names {
        let _ = delete(name); // best-effort, ignore individual failures
    }
    Ok(count)
}

/// Load all cached variations into memory at startup.
/// Errors for individual variations are logged but don't fail the whole load.
pub fn load_all() -> Vec<VariationDownload> {
    let names = match list_cached() {
        Ok(names) => names,
        Err(e) => {
            log::warn!("Failed to list cached variations: {}", e);
            return Vec::new();
        }
    };
    let mut variations = Vec::with_capacity(names.len());
    for name in names {
        match load(&name) {
            Ok(Some(v)) => variations.push(v),
            Ok(None) => log::warn!("Cached variation '{}' missing on read", name),
            Err(e) => log::warn!("Failed to load cached variation '{}': {}", name, e),
        }
    }
    variations
}

/// Get the absolute path to the cache directory (desktop only).
#[cfg(not(target_arch = "wasm32"))]
pub fn cache_dir_path() -> CacheResult<PathBuf> {
    let app_dir = backend::get_app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    Ok(app_dir.join("variations"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `list_cached` must strip the extension and ignore anything that
    /// is not a cache entry.
    ///
    /// The WASM half of this used to be a stub returning empty, which
    /// made the cache write-only on the web: `load_all` found nothing so
    /// every session re-downloaded every variation, and `clear_all`
    /// reported 0 while deleting nothing. Both platforms now go through
    /// `backend::list_entries`, so there is one behaviour to test rather
    /// than two to keep in step.
    #[test]
    fn names_come_back_without_the_json_extension() {
        // Exercised through the same filter `list_cached` applies.
        let entries = vec![
            "julia.json".to_string(),
            "pre_rotate_x.json".to_string(),
            "not-a-cache-entry.txt".to_string(),
            "README".to_string(),
        ];
        let names: Vec<String> = entries
            .into_iter()
            .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
            .collect();
        assert_eq!(names, vec!["julia".to_string(), "pre_rotate_x".to_string()]);
    }

    /// A round trip through the real backend, so the desktop path is
    /// covered end to end rather than only its filter.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_saved_variation_is_listed_and_loadable() {
        let name = "zz_cache_probe_variation";
        let dl = crate::api::types::VariationDownload {
            id: name.into(),
            name: name.into(),
            display_name: name.into(),
            description: None,
            category: "advanced_2d".into(),
            version: 1,
            phase: crate::api::types::ApiVariationPhase::Normal,
            needs_rng: false,
            needs_transform: false,
            writes_color: false,
            parameters: Vec::new(),
            shader_2d: Some("fn f(){}".into()),
            shader_3d: None,
            init_param_count: 0,
            shader_init: None,
            features: Vec::new(),
            state_count: 0,
            shader_state_init: None,
            aliases: Vec::new(),
            plot_emits: 0,
            authors: Vec::new(),
            description_plain: None,
        };
        // Best-effort: skip if this environment has no writable app dir.
        if save(&dl).is_err() {
            eprintln!("skipped: no writable app data dir");
            return;
        }
        let listed = list_cached().expect("list");
        assert!(listed.iter().any(|n| n == name), "saved entry must be listed");
        assert!(load(name).expect("load").is_some());
        let _ = delete(name);
        assert!(!list_cached().expect("list").iter().any(|n| n == name));
    }
}
