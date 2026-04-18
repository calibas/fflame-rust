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

/// List all cached variation names (desktop only — WASM returns empty).
#[cfg(not(target_arch = "wasm32"))]
pub fn list_cached() -> CacheResult<Vec<String>> {
    let app_dir = backend::get_app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let variations_dir = app_dir.join("variations");
    if !variations_dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    let entries = std::fs::read_dir(&variations_dir)
        .map_err(|e| format!("Failed to read variations dir: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    Ok(names)
}

/// List all cached variation names. WASM stub — returns empty since
/// localStorage doesn't support directory enumeration.
#[cfg(target_arch = "wasm32")]
pub fn list_cached() -> CacheResult<Vec<String>> {
    // TODO: enumerate localStorage keys with the "variations/" prefix
    Ok(Vec::new())
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

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn _path_unused() -> &'static Path { Path::new("") } // silence unused import on WASM
