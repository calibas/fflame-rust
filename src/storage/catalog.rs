//! Merging a server catalog against what is installed.
//!
//! Shared by variations and effects. The state machine is the same for
//! both — it only ever reads a version and a `downloadable` flag — and
//! two copies of it would be two copies that must stay identical.
//!
//! What it decides matters more than it looks: which bucket a resource
//! lands in is what decides whether the user is offered a download that
//! cannot succeed.

/// What a catalog row needs to expose to be merged.
///
/// Deliberately three accessors rather than a shared struct: the wire
/// types are the API's shape, not ours, and they carry different prose
/// fields. This is the part they genuinely have in common.
pub trait CatalogItem {
    fn name(&self) -> &str;
    /// The server's version. Compared against an installed copy's.
    fn version(&self) -> u32;
    /// Whether a client may fetch and register this at runtime.
    ///
    /// False for a resource that is real and catalogued but part of the
    /// engine rather than downloadable code — and, for effects, false
    /// for every row until the server's shaders are seeded.
    fn downloadable(&self) -> bool;
}

/// What the panel needs to know about one catalogued resource.
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
    /// In the catalog but not fetchable, and this client does not have
    /// it. Shown so the catalog is not silently incomplete.
    BuiltInOnlyElsewhere,
}

/// Merge one catalog row against what is installed.
///
/// Pure, so the state machine is testable without a network or a
/// registry singleton. `installed` answers `(is_builtin, version)`.
///
/// The rules, in order of precedence:
///
/// 1. A resource the client has built in is `BuiltIn` — it cannot be
///    replaced by a download, and both registries refuse to try.
/// 2. Installed-and-downloaded compares versions; the catalog winning
///    means an update exists.
/// 3. Not installed splits on `downloadable`, because "you can fetch
///    this" and "this exists but you cannot have it" are different
///    answers, and collapsing them offers a fetch that fails.
pub fn merge_state<T: CatalogItem>(item: &T, installed: Option<(bool, u32)>) -> CatalogState {
    match installed {
        Some((true, _)) => CatalogState::BuiltIn,
        Some((false, have)) => {
            if item.version() > have {
                CatalogState::UpdateAvailable { have, available: item.version() }
            } else {
                CatalogState::Downloaded { version: have }
            }
        }
        None if item.downloadable() => CatalogState::Available,
        None => CatalogState::BuiltInOnlyElsewhere,
    }
}

/// A catalog split into the buckets a panel renders.
///
/// Borrows rather than clones: the point is a listing of up to several
/// hundred rows rendered every frame.
#[derive(Debug)]
pub struct CatalogSummary<'a, T> {
    /// Fetchable and not installed.
    pub available: Vec<&'a T>,
    /// Installed but behind: `(item, have, available)`.
    pub updatable: Vec<(&'a T, u32, u32)>,
    /// Real, catalogued, and unreachable from here. Counted rather than
    /// listed because there is no action to offer.
    pub builtin_only_elsewhere: usize,
}

impl<T> Default for CatalogSummary<'_, T> {
    fn default() -> Self {
        Self { available: Vec::new(), updatable: Vec::new(), builtin_only_elsewhere: 0 }
    }
}

/// Partition a catalog against what is installed.
///
/// `installed` answers `(is_builtin, version)` for a name, or `None`.
pub fn summarize<'a, T: CatalogItem>(
    items: &'a [T],
    installed: impl Fn(&str) -> Option<(bool, u32)>,
) -> CatalogSummary<'a, T> {
    let mut out = CatalogSummary::default();
    for item in items {
        match merge_state(item, installed(item.name())) {
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

impl CatalogItem for crate::api::types::VariationListItem {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> u32 {
        self.version
    }
    fn downloadable(&self) -> bool {
        self.downloadable
    }
}

impl CatalogItem for crate::api::types::EffectListItem {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> u32 {
        self.version
    }
    fn downloadable(&self) -> bool {
        self.downloadable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{EffectListItem, VariationListItem};

    fn variation(name: &str, version: u32, downloadable: bool) -> VariationListItem {
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

    fn effect(name: &str, version: u32, downloadable: bool) -> EffectListItem {
        EffectListItem {
            id: name.into(),
            name: name.into(),
            display_name: name.into(),
            category: Some("color".into()),
            authors: Vec::new(),
            description: None,
            description_plain: None,
            version,
            downloadable,
        }
    }

    #[test]
    fn built_in_beats_everything() {
        // Even if the catalog claims a newer version: a built-in cannot
        // be replaced by a download, and offering an update the client
        // would then refuse to install is worse than saying nothing.
        assert_eq!(
            merge_state(&variation("linear", 99, true), Some((true, 0))),
            CatalogState::BuiltIn
        );
        assert_eq!(
            merge_state(&effect("vignette", 99, true), Some((true, 0))),
            CatalogState::BuiltIn
        );
    }

    #[test]
    fn a_newer_catalog_version_is_an_update() {
        assert_eq!(
            merge_state(&variation("x", 5, true), Some((false, 3))),
            CatalogState::UpdateAvailable { have: 3, available: 5 }
        );
        assert_eq!(
            merge_state(&variation("x", 3, true), Some((false, 3))),
            CatalogState::Downloaded { version: 3 }
        );
        assert_eq!(
            merge_state(&variation("x", 2, true), Some((false, 3))),
            CatalogState::Downloaded { version: 3 }
        );
    }

    #[test]
    fn not_installed_splits_on_downloadable() {
        assert_eq!(merge_state(&variation("x", 1, true), None), CatalogState::Available);
        // `subflame_wf`'s shape: catalogued, real, engine-integral.
        assert_eq!(
            merge_state(&variation("subflame_wf", 1, false), None),
            CatalogState::BuiltInOnlyElsewhere
        );
    }

    /// Effects reach the same bucket for a different reason, and that
    /// is the case worth pinning: every effect row is `downloadable:
    /// false` until the server seeds its shaders, so the whole catalog
    /// lands here during the interim. Reporting them as Available would
    /// offer a fetch that returns a null shader and gets refused.
    #[test]
    fn an_unseeded_effect_is_not_offered_as_available() {
        assert_eq!(
            merge_state(&effect("swirl", 1, false), None),
            CatalogState::BuiltInOnlyElsewhere
        );
        let items = vec![effect("a", 1, false), effect("b", 1, false)];
        let s = summarize(&items, |_| None);
        assert!(s.available.is_empty(), "nothing fetchable yet");
        assert_eq!(s.builtin_only_elsewhere, 2, "but the catalog is not empty either");
    }

    #[test]
    fn a_catalog_partitions_into_the_buckets_the_panel_shows() {
        let items = vec![
            variation("linear", 9, true),
            variation("fetched_old", 5, true),
            variation("fetched_current", 3, true),
            variation("not_here_yet", 1, true),
            variation("subflame_wf", 1, false),
        ];
        let s = summarize(&items, |name| match name {
            "linear" => Some((true, 0)),
            "fetched_old" => Some((false, 3)),
            "fetched_current" => Some((false, 3)),
            _ => None,
        });

        assert_eq!(
            s.available.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["not_here_yet"]
        );
        assert_eq!(s.updatable.len(), 1);
        assert_eq!(s.updatable[0].0.name, "fetched_old");
        assert_eq!((s.updatable[0].1, s.updatable[0].2), (3, 5));
        assert_eq!(s.builtin_only_elsewhere, 1);
    }

    #[test]
    fn an_empty_catalog_yields_nothing() {
        let s = summarize::<VariationListItem>(&[], |_| None);
        assert!(s.available.is_empty());
        assert!(s.updatable.is_empty());
        assert_eq!(s.builtin_only_elsewhere, 0);
    }
}
