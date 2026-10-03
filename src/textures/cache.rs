//! The generated-texture cache: a recipe's image, keyed by the recipe.
//!
//! Derived data, so it is never the source of truth: a missing, stale or
//! unreadable entry is simply generated again. Desktop keeps PNGs under
//! the data directory (`texture_cache/<key>.png`); the web keeps them in
//! memory for the session (browser storage is too small for images, and
//! the plan leaves IndexedDB for when reloads prove slow).

use crate::config::FractalConfig;

/// Bumped when the simulation engine's output changes, so caches written
/// by an older engine are not taken for this one's pictures. The crate
/// version is in the key as well.
pub const TEXTURE_CACHE_VERSION: u32 = 1;

/// The cache key: SHA-256 of the recipe's file form, its size, the cache
/// version and the crate version, in hex.
pub fn key(recipe: &FractalConfig) -> String {
    use sha2::{Digest, Sha256};
    let json = recipe
        .to_json_value()
        .map(|v| v.to_string())
        .unwrap_or_default();
    let (w, h) = super::texture_size(recipe);
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    hasher.update(format!("|{w}x{h}|v{TEXTURE_CACHE_VERSION}|{}", env!("CARGO_PKG_VERSION")).as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn path(key: &str) -> Option<std::path::PathBuf> {
    crate::storage::backend::get_app_data_dir()
        .ok()
        .map(|d| d.join("texture_cache").join(format!("{key}.png")))
}

/// The cached image, if there is one.
#[cfg(not(target_arch = "wasm32"))]
pub fn load(key: &str) -> Option<image::RgbaImage> {
    let path = path(key)?;
    image::open(&path).ok().map(|i| i.to_rgba8())
}

/// Cache an image. A failure is logged and otherwise ignored: the cache
/// only saves time.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(key: &str, image: &image::RgbaImage) {
    let Some(path) = path(key) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = image.save(&path) {
        log::warn!("could not cache texture {key}: {e}");
    }
}

#[cfg(target_arch = "wasm32")]
static SESSION: once_cell::sync::Lazy<std::sync::Mutex<std::collections::HashMap<String, image::RgbaImage>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(target_arch = "wasm32")]
pub fn load(key: &str) -> Option<image::RgbaImage> {
    SESSION.lock().ok()?.get(key).cloned()
}

#[cfg(target_arch = "wasm32")]
pub fn save(key: &str, image: &image::RgbaImage) {
    if let Ok(mut m) = SESSION.lock() {
        m.insert(key.to_string(), image.clone());
    }
}
