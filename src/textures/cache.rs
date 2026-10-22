//! The generated-texture cache: a recipe's image, keyed by the recipe.
//!
//! Derived data, so it is never the source of truth: a missing, stale or
//! unreadable entry is simply generated again. Desktop keeps PNGs under
//! the data directory (`texture_cache/<key>.png`); the web keeps them in
//! memory for the session (browser storage is too small for images, and
//! the plan leaves IndexedDB for when reloads prove slow).

use crate::config::FractalConfig;

/// Bumped when the simulation engine's output changes, so caches written
/// by an older engine are not taken for this one's pictures.
/// `the_texture_engine_is_the_one_the_cache_was_versioned_for` fails when
/// the shaders a texture is made with change, and says to bump this.
/// 2: SmoothLife's averages by FFT (2026-10-13), the same to rounding a
/// step but not over a long chaotic run.
/// 3: the tone curve read at its texels' centres (2026-10-19): a
/// texture's darkest values were floored toward black.
pub const TEXTURE_CACHE_VERSION: u32 = 3;

/// The cache key: SHA-256 of the recipe's file form, its size and the
/// cache version, in hex.
///
/// Only what makes the picture. The file form's `version` stamp is left
/// out, and so is the crate version, which used to be in: a release, or
/// a config format change like v3 -> v4, missed every entry, and every
/// texture -- some of them minutes of simulation -- generated again.
/// What does change the picture, the engine, is `TEXTURE_CACHE_VERSION`.
pub fn key(recipe: &FractalConfig) -> String {
    key_from_value(recipe.to_json_value().ok(), super::texture_size(recipe))
}

/// `key` from the recipe's file form.
fn key_from_value(file_form: Option<serde_json::Value>, (w, h): (u32, u32)) -> String {
    use sha2::{Digest, Sha256};
    let json = file_form
        .map(|mut v| {
            if let Some(o) = v.as_object_mut() {
                o.remove("version");
            }
            v.to_string()
        })
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    hasher.update(format!("|{w}x{h}|v{TEXTURE_CACHE_VERSION}").as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether the cache holds an image for `key`, without reading it.
#[cfg(not(target_arch = "wasm32"))]
pub fn exists(key: &str) -> bool {
    path(key).is_some_and(|p| p.exists())
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
pub fn exists(key: &str) -> bool {
    SESSION.lock().is_ok_and(|m| m.contains_key(key))
}

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

/// A fingerprint of the shaders a texture is made with: every model's
/// seed and step at every boundary, every colouring, the shared passes,
/// and the tone map the image goes through. Not every input to the
/// picture -- Rust that sets the shaders' parameters is not in it -- but
/// the usual way the simulation's output changes. Line endings are
/// normalised, so a checkout's CRLF does not make it a platform's.
#[cfg(feature = "engine-sim")]
pub fn engine_fingerprint() -> String {
    use crate::config::sim::{SimBoundary, SimDownscale, SimInit, SimUpscale};
    use crate::sim::assembler;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut feed = |wgsl: &str| hasher.update(wgsl.replace("\r\n", "\n").as_bytes());
    for model in crate::sim::MODELS {
        for kind in SimInit::KINDS {
            feed(&assembler::assemble_seed(model, kind));
        }
        for name in SimBoundary::NAMES {
            let boundary = SimBoundary::from_name(name).expect("a listed boundary");
            for pass in 0..model.passes.max(1) {
                feed(&assembler::assemble_step(model, boundary, pass));
            }
        }
    }
    for coloring in crate::sim::COLORINGS {
        feed(&assembler::assemble_color(coloring, SimBoundary::Periodic, SimUpscale::default(), SimDownscale::default(), false));
    }
    feed(&assembler::assemble_seed_zero());
    feed(&assembler::assemble_reduce());
    feed(include_str!("../../shaders/tonemap.wgsl"));
    hasher.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

#[cfg(all(test, feature = "engine-sim"))]
mod tests {
    use super::*;

    /// The shaders a texture is made with, as of `TEXTURE_CACHE_VERSION`.
    const PINNED_ENGINE: &str = "4aae063f0dd69648";

    /// When this fails, the simulation's shaders or the tone map changed,
    /// and a cached texture may no longer be what its recipe makes. If
    /// the change can move a texture's pixels, bump TEXTURE_CACHE_VERSION;
    /// either way, pin the new fingerprint below. (A comment-only edit
    /// moves it too: read the diff.)
    #[test]
    fn the_texture_engine_is_the_one_the_cache_was_versioned_for() {
        assert_eq!(
            engine_fingerprint(),
            PINNED_ENGINE,
            "the texture engine changed: bump TEXTURE_CACHE_VERSION if pictures can move, then re-pin"
        );
    }

    /// The key is what makes the picture: the same recipe under another
    /// config version stamp keeps its key, and a different recipe does not.
    #[test]
    fn the_key_ignores_the_version_stamp() {
        let recipe = crate::textures::TextureLibrary::load().entries.remove(0).config;
        let size = crate::textures::texture_size(&recipe);
        let json = recipe.to_json_value().expect("serialises");
        assert!(json.get("version").is_some(), "the file form carries a version stamp");
        let mut restamped = json.clone();
        restamped["version"] = serde_json::json!(99);
        assert_eq!(key_from_value(Some(restamped), size), key(&recipe));
        let mut other = recipe.clone();
        other.sim.steps += 1;
        assert_ne!(key(&other), key(&recipe), "a recipe change is a new key");
    }
}
