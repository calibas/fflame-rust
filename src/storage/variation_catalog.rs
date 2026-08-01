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

/// The shared merge, re-exported so callers keep one import.
///
/// The state machine moved to [`super::catalog`] when effects needed
/// the identical thing — it only ever reads a version and a
/// `downloadable` flag, so two copies would have been two copies to
/// keep in step.
pub use super::catalog::{merge_state, summarize, CatalogState, CatalogSummary};

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache path is one file, not a directory of them: the catalog
    /// is fetched and replaced whole, so per-entry storage would only
    /// add ways for it to be half-updated.
    #[test]
    fn the_catalog_is_stored_as_one_file() {
        assert_eq!(
            catalog_path(),
            std::path::PathBuf::from("variations").join("_catalog.json")
        );
    }

    /// A corrupt cache is absent, not fatal: it is a convenience copy of
    /// something re-fetchable, so failing the panel over it would be the
    /// wrong trade.
    #[test]
    fn an_unreadable_cache_reads_as_absent() {
        let path = catalog_path();
        let had = backend::read_file(&path).ok();
        backend::write_file(&path, "{ this is not json").expect("write");
        assert!(load().is_none(), "garbage must not panic or half-parse");
        match had {
            Some(original) => backend::write_file(&path, &original).expect("restore"),
            None => {
                let _ = clear();
            }
        }
    }
}
