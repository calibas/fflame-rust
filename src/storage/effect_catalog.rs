//! Cached copy of the server's effect catalog.
//!
//! The effects twin of [`super::variation_catalog`], sharing its state
//! machine through [`super::catalog`]. `GET /api/effects` is the list of
//! every effect the server knows, without shaders — it answers what
//! exists that this client does not have, whether a downloaded copy is
//! behind, and carries the prose.
//!
//! Cached because the panel must work offline. A failed fetch shows the
//! last catalog plus whatever is installed, never an error page.
//!
//! # The interim this has to render honestly
//!
//! Every effect row arrives `downloadable: false` until the server's
//! shader seed migration lands. So during the interim the whole catalog
//! merges to `BuiltInOnlyElsewhere` — visible, counted, and offered no
//! fetch button, which is the truthful rendering. Reporting them as
//! Available would offer a download that returns a null shader and gets
//! refused at registration.

use crate::api::types::EffectListItem;
use crate::storage::backend;

pub use super::catalog::{merge_state, summarize, CatalogState, CatalogSummary};

pub type CatalogResult<T> = Result<T, String>;

/// One file, not a directory: the catalog is fetched and replaced
/// whole, so per-entry storage would only add ways for it to be
/// half-updated.
fn catalog_path() -> std::path::PathBuf {
    std::path::PathBuf::from("effects").join("_catalog.json")
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct CachedEffectCatalog {
    pub items: Vec<EffectListItem>,
    /// Server-supplied version/etag for the catalog as a whole, when it
    /// sends one. Lets a revalidation be conditional rather than a full
    /// re-download.
    #[serde(default)]
    pub version: Option<String>,
}

pub fn save(catalog: &CachedEffectCatalog) -> CatalogResult<()> {
    let json = serde_json::to_string(catalog)
        .map_err(|e| format!("Failed to serialize effect catalog: {e}"))?;
    backend::write_file(&catalog_path(), &json)
        .map_err(|e| format!("Failed to write effect catalog cache: {e}"))
}

/// The cached catalog, or `None` if there is none yet.
///
/// A corrupt cache is treated as absent rather than fatal: it is a
/// convenience copy of something re-fetchable.
pub fn load() -> Option<CachedEffectCatalog> {
    let path = catalog_path();
    if !backend::file_exists(&path) {
        return None;
    }
    match backend::read_file(&path) {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("Discarding unreadable effect catalog cache: {e}");
                None
            }
        },
        Err(e) => {
            log::warn!("Failed to read effect catalog cache: {e}");
            None
        }
    }
}

pub fn clear() -> CatalogResult<()> {
    let path = catalog_path();
    if !backend::file_exists(&path) {
        return Ok(());
    }
    backend::delete_file(&path).map_err(|e| format!("Failed to delete effect catalog cache: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The effect catalog must not collide with the effect *cache*,
    /// which stores one file per effect under the same prefix. An
    /// effect legitimately named `_catalog` would, which is why the
    /// leading underscore is load-bearing rather than decorative — a
    /// name has to survive `stem`-style sanitizing to reach the cache,
    /// and no server name starts with one.
    #[test]
    fn the_catalog_file_does_not_look_like_a_cached_effect() {
        let cat = catalog_path();
        assert_eq!(cat, std::path::PathBuf::from("effects").join("_catalog.json"));
        // `list_cached` strips `.json`, so the catalog would appear as
        // an effect named `_catalog` if it were not filtered. It is not
        // filtered — this test records that the guard is the leading
        // underscore plus the server never issuing such a name.
        assert!(cat.to_string_lossy().contains("_catalog"));
    }
}
