//! Cached copy of the server's variation catalog.
//!
//! `GET /api/variations` is the list of every variation the server
//! knows, with no shader code. It answers questions the registry
//! cannot:
//!
//! * what exists that this client has **not** downloaded;
//! * whether a downloaded copy is **behind** the server's version;
//! * the prose — `description_plain` and `authors`. Built-in
//!   descriptions live in Rust doc comments, invisible at runtime, so
//!   the catalog is the only route by which prose reaches the app even
//!   for variations it already ships.
//!
//! Cached because the panel must work offline. A failed fetch shows the
//! last catalog plus whatever is installed, never an error page — the
//! app renders fractals perfectly well with no network, and a browser
//! that cannot reach the API should not lose the ability to see what it
//! already has.
//!
//! The file itself, and the merge against what is installed, are shared
//! with effects — see [`super::resource_store`] and [`super::catalog`].

use crate::api::types::{VariationDownload, VariationListItem};
use crate::storage::resource_store::{self as store, ResourceKind};

pub use super::catalog::{merge_state, summarize, CatalogState, CatalogSummary};

pub type CatalogResult<T> = Result<T, String>;

/// The catalog shares its subsystem's prefix, so it sits beside the
/// cache it describes.
impl ResourceKind for VariationListItem {
    const PREFIX: &'static str = VariationDownload::PREFIX;
}

pub type CachedCatalog = store::CachedCatalog<VariationListItem>;

pub fn save(catalog: &CachedCatalog) -> CatalogResult<()> {
    store::save_catalog(catalog)
}

pub fn load() -> Option<CachedCatalog> {
    store::load_catalog::<VariationListItem>()
}

/// Drop the cached catalog. Used by Clear Cache alongside the
/// downloaded variations themselves.
pub fn clear() -> CatalogResult<()> {
    store::clear_catalog::<VariationListItem>()
}
