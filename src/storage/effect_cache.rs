//! Persistent cache for API-loaded effects.
//!
//! Storage lives in [`super::resource_store`]; what is here is the part
//! genuinely about effects — the registration rules applied when cached
//! entries come back.
//!
//! Built-in effects are not cached; they are compiled in.

use crate::api::types::EffectDownload;
use crate::provenance::Provenance;
use crate::storage::resource_store::{self as store, CachedResource, ResourceKind};

pub type CacheResult<T> = Result<T, String>;

impl ResourceKind for EffectDownload {
    const PREFIX: &'static str = "effects";
}

impl CachedResource for EffectDownload {
    fn name(&self) -> &str {
        &self.name
    }
}

pub fn save(effect: &EffectDownload) -> CacheResult<()> {
    store::save(effect)
}

/// `Ok(None)` when not cached; `Err` only for a real failure.
pub fn load(name: &str) -> CacheResult<Option<EffectDownload>> {
    store::load::<EffectDownload>(name)
}

pub fn delete(name: &str) -> CacheResult<()> {
    store::delete::<EffectDownload>(name)
}

pub fn list_cached() -> CacheResult<Vec<String>> {
    store::list_cached::<EffectDownload>()
}

/// Clear the cache. Returns how many entries went.
pub fn clear_all() -> CacheResult<usize> {
    store::clear_all::<EffectDownload>()
}

/// Register every cached effect at startup.
///
/// A cached entry the current build would refuse is **dropped with a
/// warning rather than registered**: the refusal rules belong to the
/// app, not to the cache, so a rule tightened in a later build must
/// apply to what is already on disk. Otherwise the cache becomes a way
/// to keep running something the current build would reject.
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
            Ok(Some(dl)) => {
                let provenance = Provenance::Api { version: dl.version };
                match registry.register_from_api(&dl, provenance) {
                    Ok(()) => ok += 1,
                    Err(e) => log::warn!("Dropping cached effect '{name}': {e}"),
                }
            }
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

    #[test]
    fn effects_live_under_their_own_prefix() {
        assert_eq!(EffectDownload::PREFIX, "effects");
    }
}
