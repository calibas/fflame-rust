//! Persistent cache for API-loaded variations.
//!
//! Storage lives in [`super::resource_store`]; what is here is the part
//! genuinely about variations — the registration rules applied when
//! cached entries come back.
//!
//! Built-in variations are NOT cached; they are compiled in.

use crate::api::types::VariationDownload;
use crate::provenance::Provenance;
use crate::storage::resource_store::{self as store, CachedResource, ResourceKind};

pub type CacheResult<T> = Result<T, String>;

impl ResourceKind for VariationDownload {
    const PREFIX: &'static str = "variations";
}

impl CachedResource for VariationDownload {
    fn name(&self) -> &str {
        &self.name
    }
}

pub fn save(variation: &VariationDownload) -> CacheResult<()> {
    store::save(variation)
}

/// `Ok(None)` if not cached; `Err` only for a real failure.
pub fn load(name: &str) -> CacheResult<Option<VariationDownload>> {
    store::load::<VariationDownload>(name)
}

pub fn delete(name: &str) -> CacheResult<()> {
    store::delete::<VariationDownload>(name)
}

/// All cached variation names.
///
/// One implementation for both platforms. The WASM half used to be a
/// stub returning empty, which made this cache **write-only** on the
/// web: `load_all` found nothing so every session re-downloaded
/// everything, and `clear_all` reported 0 while deleting nothing.
/// localStorage does support enumeration (`length`/`key`); it had just
/// never been wired.
pub fn list_cached() -> CacheResult<Vec<String>> {
    store::list_cached::<VariationDownload>()
}

/// Clear the cache. Returns the number of entries removed.
pub fn clear_all() -> CacheResult<usize> {
    store::clear_all::<VariationDownload>()
}

/// Load every cached variation into memory.
///
/// An individual failure is logged and skipped rather than failing the
/// whole load: one corrupt entry must not cost the rest.
pub fn load_all() -> Vec<VariationDownload> {
    let names = match list_cached() {
        Ok(names) => names,
        Err(e) => {
            log::warn!("Failed to list cached variations: {e}");
            return Vec::new();
        }
    };
    let mut variations = Vec::with_capacity(names.len());
    for name in names {
        match load(&name) {
            Ok(Some(v)) => variations.push(v),
            Ok(None) => log::warn!("Cached variation '{name}' vanished between listing and read"),
            Err(e) => log::warn!("Failed to load cached variation '{name}': {e}"),
        }
    }
    variations
}

/// Register every cached variation.
///
/// Provenance is `Api`, never `Local`: the cache holds downloads, and a
/// plugin registered as one would become clearable cache.
pub fn load_all_into_registry() {
    let cached = load_all();
    if cached.is_empty() {
        return;
    }
    let mut registry = crate::variations::global_registry_mut();
    for download in cached {
        let provenance = Provenance::Api { version: download.version };
        registry.register_from_api(&download, provenance);
    }
}

/// The cache directory, for a status line (desktop only).
#[cfg(not(target_arch = "wasm32"))]
pub fn cache_dir_path() -> CacheResult<std::path::PathBuf> {
    let app_dir = crate::storage::backend::get_app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    Ok(app_dir.join(VariationDownload::PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prefix is what this module contributes to storage; the
    /// primitives are tested once, in `resource_store`.
    #[test]
    fn variations_live_under_their_own_prefix() {
        assert_eq!(VariationDownload::PREFIX, "variations");
    }
}
