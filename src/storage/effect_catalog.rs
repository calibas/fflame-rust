//! Cached copy of the server's effect catalog.
//!
//! The effects twin of [`super::variation_catalog`], sharing both its
//! file handling ([`super::resource_store`]) and its merge state
//! machine ([`super::catalog`]).
//!
//! # The interim this has to render honestly
//!
//! Every effect row arrives `downloadable: false` until the server's
//! shader seed migration lands. So during the interim the whole catalog
//! merges to `BuiltInOnlyElsewhere` — visible, counted, and offered no
//! fetch button, which is the truthful rendering. Reporting them as
//! Available would offer a download that returns a null shader and gets
//! refused at registration.

use crate::api::types::{EffectDownload, EffectListItem};
use crate::storage::resource_store::{self as store, ResourceKind};

pub use super::catalog::{merge_state, summarize, CatalogState, CatalogSummary};

pub type CatalogResult<T> = Result<T, String>;

impl ResourceKind for EffectListItem {
    const PREFIX: &'static str = EffectDownload::PREFIX;
}

pub type CachedEffectCatalog = store::CachedCatalog<EffectListItem>;

pub fn save(catalog: &CachedEffectCatalog) -> CatalogResult<()> {
    store::save_catalog(catalog)
}

pub fn load() -> Option<CachedEffectCatalog> {
    store::load_catalog::<EffectListItem>()
}

pub fn clear() -> CatalogResult<()> {
    store::clear_catalog::<EffectListItem>()
}
