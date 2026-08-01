//! Cached copy of the server's variation catalog.
//!
//! The catalog (`GET /api/variations`) is the list of every variation
//! the server knows, with no shader code. It answers questions the
//! registry cannot:
//!
//! * what exists that this client has **not** downloaded;
//! * whether a downloaded copy is **behind** the server's version;
//! * the prose — `description_plain` and `authors`. Built-in
//!   descriptions live in Rust doc comments, which are invisible at
//!   runtime, so the catalog is the only route by which prose reaches
//!   the app even for variations it already ships.
//!
//! Cached because the panel must work offline. A failed fetch shows the
//! last catalog plus whatever is installed, never an error page — the
//! app renders fractals perfectly well with no network, and a browser
//! that cannot reach the API should not lose the ability to see what it
//! already has.

use crate::api::types::VariationListItem;
use crate::storage::backend;

pub type CatalogResult<T> = Result<T, String>;

/// One file, not a directory of them: the catalog is fetched and
/// replaced whole, so per-entry storage would only add ways for it to
/// be half-updated.
fn catalog_path() -> std::path::PathBuf {
    std::path::PathBuf::from("variations").join("_catalog.json")
}

/// The cached catalog plus when it was fetched.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct CachedCatalog {
    pub items: Vec<VariationListItem>,
    /// Server-supplied version/etag for the catalog as a whole, when it
    /// sends one. Lets a revalidation be conditional rather than a full
    /// re-download.
    #[serde(default)]
    pub version: Option<String>,
}

pub fn save(catalog: &CachedCatalog) -> CatalogResult<()> {
    let json = serde_json::to_string(catalog)
        .map_err(|e| format!("Failed to serialize catalog: {e}"))?;
    backend::write_file(&catalog_path(), &json)
        .map_err(|e| format!("Failed to write catalog cache: {e}"))
}

/// The cached catalog, or `None` if there is none yet.
///
/// A corrupt cache is treated as absent rather than fatal: it is a
/// convenience copy of something re-fetchable, so failing the panel over
/// it would be the wrong trade.
pub fn load() -> Option<CachedCatalog> {
    let path = catalog_path();
    if !backend::file_exists(&path) {
        return None;
    }
    match backend::read_file(&path) {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("Discarding unreadable variation catalog cache: {e}");
                None
            }
        },
        Err(e) => {
            log::warn!("Failed to read variation catalog cache: {e}");
            None
        }
    }
}

/// Drop the cached catalog. Used by Clear Cache alongside the
/// downloaded variations themselves.
pub fn clear() -> CatalogResult<()> {
    let path = catalog_path();
    if !backend::file_exists(&path) {
        return Ok(());
    }
    backend::delete_file(&path).map_err(|e| format!("Failed to delete catalog cache: {e}"))
}

/// What the panel needs to know about one catalogued variation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogState {
    /// Ships with the app. Nothing to fetch.
    BuiltIn,
    /// Downloaded, and the catalog agrees on the version.
    Downloaded { version: u32 },
    /// Downloaded, but the server has a newer version.
    UpdateAvailable { have: u32, available: u32 },
    /// In the catalog, not installed. Fetchable.
    Available,
    /// In the catalog but marked built-in-only, and this client does not
    /// have it — engine-integral, so it can never be fetched. Shown so
    /// the catalog is not silently incomplete.
    BuiltInOnlyElsewhere,
}

/// Merge the catalog against what is installed.
///
/// Pure, so the state machine is testable without a network or a
/// registry singleton. The rules, in order of precedence:
///
/// 1. A variation the client has built in is `BuiltIn` — it cannot be
///    replaced by a download, and `register_from_api` refuses to try.
/// 2. Installed-and-downloaded compares versions; the catalog winning
///    means an update exists.
/// 3. Not installed splits on `downloadable`, because "you can fetch
///    this" and "this exists but is engine-integral" are different
///    answers and collapsing them would misreport `subflame_wf`.
pub fn merge_state(
    item: &VariationListItem,
    installed: Option<(bool, u32)>,
) -> CatalogState {
    match installed {
        Some((true, _)) => CatalogState::BuiltIn,
        Some((false, have)) => {
            if item.version > have {
                CatalogState::UpdateAvailable { have, available: item.version }
            } else {
                CatalogState::Downloaded { version: have }
            }
        }
        None if item.downloadable => CatalogState::Available,
        None => CatalogState::BuiltInOnlyElsewhere,
    }
}

/// The catalog split into the buckets the panel renders.
///
/// Borrows from the catalog rather than cloning: the whole point is a
/// listing of up to several hundred entries rendered every frame.
#[derive(Debug, Default)]
pub struct CatalogSummary<'a> {
    /// Fetchable and not installed.
    pub available: Vec<&'a VariationListItem>,
    /// Installed but behind: `(item, have, available)`.
    pub updatable: Vec<(&'a VariationListItem, u32, u32)>,
    /// Real, catalogued, and unreachable from here — engine-integral
    /// variations this build does not have. Counted rather than listed
    /// because there is no action to offer.
    pub builtin_only_elsewhere: usize,
}

/// Partition a catalog against what is installed.
///
/// Separated from the panel so the bucketing is testable without an
/// egui context: which bucket a variation lands in decides whether the
/// user is offered a download that cannot succeed.
///
/// `installed` answers `(is_core, version)` for a name, or `None`.
pub fn summarize<'a>(
    items: &'a [VariationListItem],
    installed: impl Fn(&str) -> Option<(bool, u32)>,
) -> CatalogSummary<'a> {
    let mut out = CatalogSummary::default();
    for item in items {
        match merge_state(item, installed(&item.name)) {
            CatalogState::Available => out.available.push(item),
            CatalogState::UpdateAvailable { have, available } => {
                out.updatable.push((item, have, available))
            }
            CatalogState::BuiltInOnlyElsewhere => out.builtin_only_elsewhere += 1,
            CatalogState::BuiltIn | CatalogState::Downloaded { .. } => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, version: u32, downloadable: bool) -> VariationListItem {
        VariationListItem {
            id: name.into(),
            name: name.into(),
            display_name: name.into(),
            category: "advanced_2d".into(),
            version,
            description: None,
            description_plain: None,
            authors: Vec::new(),
            downloadable,
            has_shader_3d: true,
        }
    }

    #[test]
    fn built_in_beats_everything() {
        // Even if the catalog claims a newer version: a built-in cannot
        // be replaced by a download, and offering an update the client
        // would then refuse to install is worse than saying nothing.
        let s = merge_state(&item("linear", 99, true), Some((true, 0)));
        assert_eq!(s, CatalogState::BuiltIn);
    }

    #[test]
    fn a_newer_catalog_version_is_an_update() {
        assert_eq!(
            merge_state(&item("x", 5, true), Some((false, 3))),
            CatalogState::UpdateAvailable { have: 3, available: 5 }
        );
        // Same or older is not.
        assert_eq!(
            merge_state(&item("x", 3, true), Some((false, 3))),
            CatalogState::Downloaded { version: 3 }
        );
        assert_eq!(
            merge_state(&item("x", 2, true), Some((false, 3))),
            CatalogState::Downloaded { version: 3 }
        );
    }

    /// The panel's whole listing in one pass: every bucket, and nothing
    /// in two of them.
    #[test]
    fn a_catalog_partitions_into_the_buckets_the_panel_shows() {
        let items = vec![
            item("linear", 9, true),        // built in here
            item("fetched_old", 5, true),   // downloaded, stale
            item("fetched_current", 3, true),
            item("not_here_yet", 1, true),
            item("subflame_wf", 1, false),  // engine-integral, absent
        ];
        let s = summarize(&items, |name| match name {
            "linear" => Some((true, 0)),
            "fetched_old" => Some((false, 3)),
            "fetched_current" => Some((false, 3)),
            _ => None,
        });

        assert_eq!(s.available.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
                   vec!["not_here_yet"]);
        assert_eq!(s.updatable.len(), 1);
        assert_eq!(s.updatable[0].0.name, "fetched_old");
        assert_eq!((s.updatable[0].1, s.updatable[0].2), (3, 5));
        assert_eq!(s.builtin_only_elsewhere, 1);
        // `linear` and `fetched_current` are present and current — they
        // appear in the registry listing, not in any catalog bucket.
    }

    /// Offline with nothing cached: no buckets, and specifically no
    /// "everything is installed" claim derived from an empty catalog.
    #[test]
    fn an_empty_catalog_yields_nothing() {
        let s = summarize(&[], |_| None);
        assert!(s.available.is_empty());
        assert!(s.updatable.is_empty());
        assert_eq!(s.builtin_only_elsewhere, 0);
    }

    #[test]
    fn not_installed_splits_on_downloadable() {
        assert_eq!(merge_state(&item("x", 1, true), None), CatalogState::Available);
        // subflame_wf's shape: catalogued, real, but engine-integral.
        // Reporting it as "Available" would offer a fetch that cannot
        // succeed; omitting it would make the catalog look incomplete.
        assert_eq!(
            merge_state(&item("subflame_wf", 1, false), None),
            CatalogState::BuiltInOnlyElsewhere
        );
    }
}
