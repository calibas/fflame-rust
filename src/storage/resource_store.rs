//! On-disk storage shared by the variation and effect subsystems.
//!
//! Holds the parts that were genuinely the same code twice: the cache
//! primitives, the catalog file, and the rule about which entries are
//! metadata. The parts that only *looked* the same — the two fetch
//! modules — stayed where they are; see §8.7 of the shared-resource
//! plan for why.
//!
//! # Why this exists, concretely
//!
//! Not a similarity score. `is_metadata_entry` was copy-pasted into
//! both caches, because the bug it fixes — the catalog file coming back
//! from `list_cached` as though it were a cached resource, so
//! "Clear Cache (N)" reported one too many and deleted the catalog —
//! lived in `variation_cache` for three weeks and was only found while
//! writing the effect equivalent. It then had to be repaired twice.
//!
//! One rule in one place is the whole point; everything else here is
//! the boilerplate that surrounds it.

use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

use crate::storage::backend;

pub type StoreResult<T> = Result<T, String>;

/// A family of resources with its own storage prefix.
///
/// Implemented by both the cached payload (`VariationDownload`) and the
/// catalog row (`VariationListItem`) for a subsystem, since both live
/// under the same prefix. A test asserts the two agree — that is the
/// only drift this arrangement allows.
pub trait ResourceKind {
    /// Directory on desktop, localStorage key prefix on the web.
    const PREFIX: &'static str;
}

/// A payload the cache can store, keyed by its own name.
pub trait CachedResource: ResourceKind + Serialize + DeserializeOwned {
    fn name(&self) -> &str;
}

/// Entries whose name begins with `_` are metadata, not resources.
///
/// The catalog lives in the same prefix as the cache (`_catalog.json`),
/// so without this it comes back from a listing as a cached entry named
/// `_catalog`: unparseable on load (a warning every startup), counted by
/// Clear Cache, and deleted as though it were one of the entries.
///
/// Safe because a name has to be a valid identifier to reach the cache
/// at all, and no server-issued name begins with an underscore.
pub fn is_metadata_entry(name: &str) -> bool {
    name.starts_with('_')
}

fn entry_path<T: ResourceKind>(name: &str) -> PathBuf {
    Path::new(T::PREFIX).join(format!("{name}.json"))
}

pub fn save<T: CachedResource>(value: &T) -> StoreResult<()> {
    let json = serde_json::to_string(value)
        .map_err(|e| format!("Failed to serialize {}: {e}", T::PREFIX))?;
    backend::write_file(&entry_path::<T>(value.name()), &json)
        .map_err(|e| format!("Failed to write {} cache: {e}", T::PREFIX))
}

/// `Ok(None)` when not cached; `Err` only for a real failure.
pub fn load<T: CachedResource>(name: &str) -> StoreResult<Option<T>> {
    let path = entry_path::<T>(name);
    if !backend::file_exists(&path) {
        return Ok(None);
    }
    let json = backend::read_file(&path)
        .map_err(|e| format!("Failed to read {} cache: {e}", T::PREFIX))?;
    let value = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse cached '{name}': {e}"))?;
    Ok(Some(value))
}

pub fn delete<T: ResourceKind>(name: &str) -> StoreResult<()> {
    backend::delete_file(&entry_path::<T>(name))
        .map_err(|e| format!("Failed to delete from the {} cache: {e}", T::PREFIX))
}

/// Every cached name, metadata excluded.
pub fn list_cached<T: ResourceKind>() -> StoreResult<Vec<String>> {
    let entries = backend::list_entries(Path::new(T::PREFIX))
        .map_err(|e| format!("Failed to list the {} cache: {e}", T::PREFIX))?;
    Ok(entries
        .into_iter()
        .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
        .filter(|n| !is_metadata_entry(n))
        .collect())
}

/// Clear the cache. Returns how many entries went.
pub fn clear_all<T: ResourceKind>() -> StoreResult<usize> {
    let names = list_cached::<T>()?;
    let count = names.len();
    for name in &names {
        // Best-effort: one unreadable entry must not strand the rest.
        let _ = delete::<T>(name);
    }
    Ok(count)
}

// ============================================================================
// The catalog file
// ============================================================================

/// One file per subsystem, not a directory of them: the catalog is
/// fetched and replaced whole, so per-entry storage would only add ways
/// for it to be half-updated.
fn catalog_path<T: ResourceKind>() -> PathBuf {
    Path::new(T::PREFIX).join("_catalog.json")
}

/// A cached server catalog: everything it knows, without payloads.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CachedCatalog<T> {
    pub items: Vec<T>,
    /// Server-supplied version/etag for the catalog as a whole, when it
    /// sends one. Lets a revalidation be conditional rather than a full
    /// re-download.
    #[serde(default)]
    pub version: Option<String>,
}

impl<T> Default for CachedCatalog<T> {
    fn default() -> Self {
        Self { items: Vec::new(), version: None }
    }
}

pub fn save_catalog<T: ResourceKind + Serialize>(
    catalog: &CachedCatalog<T>,
) -> StoreResult<()> {
    let json = serde_json::to_string(catalog)
        .map_err(|e| format!("Failed to serialize the {} catalog: {e}", T::PREFIX))?;
    backend::write_file(&catalog_path::<T>(), &json)
        .map_err(|e| format!("Failed to write the {} catalog cache: {e}", T::PREFIX))
}

/// The cached catalog, or `None` if there is none yet.
///
/// A corrupt cache is treated as absent rather than fatal: it is a
/// convenience copy of something re-fetchable, so failing the panel over
/// it would be the wrong trade.
pub fn load_catalog<T: ResourceKind + DeserializeOwned>() -> Option<CachedCatalog<T>> {
    let path = catalog_path::<T>();
    if !backend::file_exists(&path) {
        return None;
    }
    match backend::read_file(&path) {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("Discarding an unreadable {} catalog cache: {e}", T::PREFIX);
                None
            }
        },
        Err(e) => {
            log::warn!("Failed to read the {} catalog cache: {e}", T::PREFIX);
            None
        }
    }
}

pub fn clear_catalog<T: ResourceKind>() -> StoreResult<()> {
    let path = catalog_path::<T>();
    if !backend::file_exists(&path) {
        return Ok(());
    }
    backend::delete_file(&path)
        .map_err(|e| format!("Failed to delete the {} catalog cache: {e}", T::PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{
        EffectDownload, EffectListItem, VariationDownload, VariationListItem,
    };

    /// Serialize the tests that touch a real catalog file.
    ///
    /// Two of them write to the same path — one stores a valid catalog,
    /// one deliberately corrupts it — so in parallel they clobber each
    /// other and whichever loses looks broken.
    ///
    /// Same shape as the script-store link tests, and introduced the
    /// same way: by adding a second test against shared state without
    /// noticing the first was already there. The product is unaffected —
    /// catalogs are written from one thread — but a flaky test is worse
    /// than a failing one, because it teaches people to re-run.
    fn catalog_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The payload and the catalog row for a subsystem must agree about
    /// where they live. They are separate impls, so this is the one way
    /// the arrangement could drift — and a mismatch would put the
    /// catalog in one directory and the cache in another, quietly.
    #[test]
    fn a_subsystems_two_types_share_one_prefix() {
        assert_eq!(VariationDownload::PREFIX, VariationListItem::PREFIX);
        assert_eq!(EffectDownload::PREFIX, EffectListItem::PREFIX);
        assert_ne!(
            VariationDownload::PREFIX,
            EffectDownload::PREFIX,
            "and the two subsystems must NOT share one"
        );
    }

    /// An ordinary name is not mistaken for metadata.
    #[test]
    fn a_resource_name_is_not_metadata() {
        assert!(!is_metadata_entry("julian"));
        assert!(!is_metadata_entry("dc_linear"));
        assert!(is_metadata_entry("_catalog"));
    }

    /// The catalog must not read back as a cached resource — tested
    /// against the REAL backend, both subsystems, because that is the
    /// bug this module exists for.
    ///
    /// A pure check on the path string would pass even if `list_cached`
    /// forgot to apply the filter, which is precisely the mistake that
    /// shipped in `variation_cache` for three weeks.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_stored_catalog_is_not_listed_as_a_resource() {
        let _guard = catalog_test_lock();
        let probe = "resource-store-listing-probe";

        // --- variations ---
        let _ = delete::<VariationDownload>(probe);
        save(&sample_variation(probe)).expect("save");
        save_catalog(&CachedCatalog::<VariationListItem>::default()).expect("catalog");
        let listed = list_cached::<VariationDownload>().expect("list");
        assert!(listed.iter().any(|n| n == probe), "the real entry is listed");
        assert!(
            !listed.iter().any(|n| is_metadata_entry(n)),
            "metadata leaked into the listing: {listed:?}"
        );
        delete::<VariationDownload>(probe).expect("cleanup");
        let _ = clear_catalog::<VariationListItem>();

        // --- effects, same rule ---
        let _ = delete::<EffectDownload>(probe);
        save(&sample_effect(probe)).expect("save");
        save_catalog(&CachedCatalog::<EffectListItem>::default()).expect("catalog");
        let listed = list_cached::<EffectDownload>().expect("list");
        assert!(listed.iter().any(|n| n == probe));
        assert!(!listed.iter().any(|n| is_metadata_entry(n)), "{listed:?}");
        delete::<EffectDownload>(probe).expect("cleanup");
        let _ = clear_catalog::<EffectListItem>();
    }

    /// A corrupt catalog reads as absent rather than exploding: it is a
    /// convenience copy of something re-fetchable, so failing the panel
    /// over it would be the wrong trade.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn an_unreadable_catalog_reads_as_absent() {
        let _guard = catalog_test_lock();
        use crate::storage::backend;
        let path = catalog_path::<VariationListItem>();
        let had = backend::read_file(&path).ok();

        backend::write_file(&path, "{ this is not json").expect("write");
        assert!(
            load_catalog::<VariationListItem>().is_none(),
            "garbage must not panic or half-parse"
        );

        match had {
            Some(original) => backend::write_file(&path, &original).expect("restore"),
            None => {
                let _ = clear_catalog::<VariationListItem>();
            }
        }
    }

    fn sample_variation(name: &str) -> VariationDownload {
        VariationDownload {
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
        }
    }

    fn sample_effect(name: &str) -> EffectDownload {
        EffectDownload {
            id: name.into(),
            name: name.into(),
            display_name: name.into(),
            category: Some("color".into()),
            authors: Vec::new(),
            description: None,
            description_plain: None,
            version: 7,
            parameters: Vec::new(),
            shader: Some("fn main() {}".into()),
            requires_blend_modes: false,
            downloadable: true,
        }
    }

    /// Round-trip through the real backend, desktop only — the WASM half
    /// needs a browser, and the code either side of `backend` is shared.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_resource_survives_a_save_and_can_be_cleared() {
        let name = "resource-store-round-trip";
        let dl = sample_effect(name);
        let _ = delete::<EffectDownload>(name);

        assert!(load::<EffectDownload>(name).unwrap().is_none());
        save(&dl).expect("save");
        assert_eq!(load::<EffectDownload>(name).unwrap().unwrap().version, 7);
        assert!(list_cached::<EffectDownload>().unwrap().iter().any(|n| n == name));

        delete::<EffectDownload>(name).expect("delete");
        assert!(load::<EffectDownload>(name).unwrap().is_none());
    }
}
