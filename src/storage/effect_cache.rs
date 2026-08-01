//! Persistent cache for API-loaded effects.
//!
//! Stores `EffectDownload` JSON keyed by effect name:
//! - Desktop: `<app data>/effects/<name>.json`
//! - WASM: localStorage under `effects/<name>.json`
//!
//! Built-in effects are not cached — they are compiled in.
//!
//! Deliberately a near-copy of [`super::variation_cache`] rather than a
//! generic abstraction over both. The two differ in what a cache miss
//! means and in what is refused at registration, and the shared part is
//! six lines of `backend` calls; a generic version would hide the
//! difference that matters and save nothing.
//!
//! The WASM half is correct from the start. `variation_cache` shipped
//! with `list_cached` stubbed to return empty on the web, which made
//! that cache **write-only**: every session re-downloaded everything and
//! Clear Cache always reported zero. `backend::list_entries` now exists
//! and both platforms go through it.

use std::path::{Path, PathBuf};

use crate::api::types::EffectDownload;
use crate::storage::backend;

pub type CacheResult<T> = Result<T, String>;

const PREFIX: &str = "effects";

fn cache_path(name: &str) -> PathBuf {
    PathBuf::from(PREFIX).join(format!("{name}.json"))
}

pub fn save(effect: &EffectDownload) -> CacheResult<()> {
    let json = serde_json::to_string(effect)
        .map_err(|e| format!("Failed to serialize effect: {e}"))?;
    backend::write_file(&cache_path(&effect.name), &json)
        .map_err(|e| format!("Failed to write effect cache: {e}"))
}

/// `Ok(None)` when not cached; `Err` only for a real failure.
pub fn load(name: &str) -> CacheResult<Option<EffectDownload>> {
    let path = cache_path(name);
    if !backend::file_exists(&path) {
        return Ok(None);
    }
    let json =
        backend::read_file(&path).map_err(|e| format!("Failed to read effect cache: {e}"))?;
    let effect: EffectDownload = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse cached effect '{name}': {e}"))?;
    Ok(Some(effect))
}

pub fn delete(name: &str) -> CacheResult<()> {
    backend::delete_file(&cache_path(name))
        .map_err(|e| format!("Failed to delete effect cache: {e}"))
}

pub fn list_cached() -> CacheResult<Vec<String>> {
    let entries = backend::list_entries(Path::new(PREFIX))
        .map_err(|e| format!("Failed to list cached effects: {e}"))?;
    Ok(entries
        .into_iter()
        .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
        .filter(|n| !is_metadata_entry(n))
        .collect())
}

/// Entries whose name begins with `_` are metadata, not resources.
///
/// `effect_catalog` stores `_catalog.json` under this same prefix. The
/// variation cache shipped without this filter, so its catalog came back
/// as a cached variation named `_catalog` — a parse warning on every
/// startup, an off-by-one in "Clear Cache (N)", and the catalog deleted
/// as though it were one of the entries.
fn is_metadata_entry(name: &str) -> bool {
    name.starts_with('_')
}

/// Clear the cache. Returns how many entries went.
pub fn clear_all() -> CacheResult<usize> {
    let names = list_cached()?;
    let count = names.len();
    for name in &names {
        let _ = delete(name); // best-effort; one bad entry must not stop the rest
    }
    Ok(count)
}

/// Register every cached effect at startup.
///
/// A cached entry that no longer passes `check_download` is dropped with
/// a warning rather than registered: the refusal rules are part of the
/// app, not of the cache, so a rule tightened in a later build must
/// apply to what is already on disk. Otherwise the cache becomes a way
/// to keep running something the current build would refuse.
pub fn load_all_into_registry() {
    let names = match list_cached() {
        Ok(n) => n,
        Err(e) => {
            log::warn!("Failed to list cached effects: {e}");
            return;
        }
    };
    if names.is_empty() {
        return;
    }
    let mut registry = crate::effects::global_effect_registry_mut();
    let mut ok = 0usize;
    for name in names {
        match load(&name) {
            Ok(Some(dl)) => match registry.register_from_api(&dl) {
                Ok(()) => ok += 1,
                Err(e) => log::warn!("Dropping cached effect '{name}': {e}"),
            },
            Ok(None) => log::warn!("Cached effect '{name}' vanished between listing and read"),
            Err(e) => log::warn!("Failed to load cached effect '{name}': {e}"),
        }
    }
    if ok > 0 {
        log::info!("Registered {ok} cached effect(s)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> EffectDownload {
        EffectDownload {
            id: name.into(),
            name: name.into(),
            display_name: name.into(),
            category: Some("color".into()),
            authors: Vec::new(),
            description: None,
            description_plain: None,
            version: 2,
            parameters: Vec::new(),
            shader: Some("fn main() {}".into()),
            requires_blend_modes: false,
            downloadable: true,
        }
    }

    /// The catalog shares this prefix and must not read back as a
    /// cached effect. This is the bug the variation cache shipped with.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_catalog_file_is_not_listed_as_an_effect() {
        let name = "effect-listing-probe";
        let _ = delete(name);
        save(&sample(name)).expect("save");
        crate::storage::effect_catalog::save(&Default::default()).expect("catalog");

        let listed = list_cached().expect("list");
        assert!(listed.iter().any(|n| n == name), "the real entry is listed");
        assert!(
            !listed.iter().any(|n| n.starts_with('_')),
            "metadata must not be listed as a resource: {listed:?}"
        );

        delete(name).expect("cleanup");
        let _ = crate::storage::effect_catalog::clear();
    }

    #[test]
    fn names_come_back_without_the_json_extension() {
        assert_eq!(cache_path("swirl"), PathBuf::from("effects").join("swirl.json"));
    }

    /// Round-trip against the real backend, desktop only — the WASM half
    /// needs a browser, and the code either side of `backend` is shared.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn an_effect_survives_a_save_and_can_be_cleared() {
        let name = "effect-cache-round-trip";
        let _ = delete(name);

        assert!(load(name).unwrap().is_none(), "not cached yet");
        save(&sample(name)).expect("save");

        let back = load(name).unwrap().expect("cached");
        assert_eq!(back.name, name);
        assert_eq!(back.version, 2);
        assert_eq!(back.shader.as_deref(), Some("fn main() {}"));
        assert!(list_cached().unwrap().iter().any(|n| n == name));

        delete(name).expect("delete");
        assert!(load(name).unwrap().is_none(), "it should actually be gone");
    }
}
