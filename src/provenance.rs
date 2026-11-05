//! Where a thing came from: one answer for every kind -- variations,
//! effects, formulas, scripts, textures -- in three values, [`Origin`]:
//! built in, local, or from the online API. The library's lists, the
//! Scripts panel and the Textures panel read it; a registry entry
//! carries it as a [`Provenance`], which adds the server's version to a
//! download (what an update compares).
//!
//! Where a thing is KEPT is a different question, answered per store:
//! a CPU script's file is compiled in, on disk with the app, in the
//! user's store or outside every store (`script::library::ScriptHome`).
//!
//! # Why this replaces `is_core: bool`
//!
//! A bool could answer "did this ship with the app", which is what the
//! call sites happened to need while downloads were the only other
//! source. It cannot answer the three questions they were really
//! asking, and those come apart the moment locally installed resources
//! exist:
//!
//! | | shipped | downloaded | the user's own |
//! |---|---|---|---|
//! | third-party code? | no | **yes** | **yes** |
//! | in the download cache? | no | **yes** | no |
//! | updatable from the server? | no | **yes** | no |
//!
//! The user's own is code the app did not ship, but Clear Cache must not
//! remove it and no update can be offered for it.
//!
//! (A `Local` provenance for WGSL plug-in files was retired with them
//! on 2026-11-01, and the user's Rhai entries were `Custom` until the
//! one origin model of 2026-11-04 named them `Local`.)

use serde::{Deserialize, Serialize};

/// Where a thing came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Ships with the app: read-only, edited as a copy.
    Builtin,
    /// The user's: a file in their stores, a plug-in, or a source a
    /// config carries (the config is its home, wherever the config came
    /// from).
    Local,
    /// Somebody else's, from the online library: a download in the
    /// cache, or a file adopted into the user's stores (adopting it does
    /// not make it theirs: they chose to keep it, they did not write it).
    Api,
}

impl Origin {
    /// What a list shows after an entry's title: ` ↓` for somebody
    /// else's, ` *` for the user's own, nothing for a built-in.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Api => " ↓",
            Self::Local => " *",
            Self::Builtin => "",
        }
    }
}

/// A registry entry's origin, with the server's version on a download.
///
/// A plug-in is `Local` whoever wrote it: the registry's question is who
/// keeps the entry -- Clear Cache leaves it, no catalog update is offered
/// -- and the user does. Who wrote it is the library file's [`Origin`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Ships with the app. Cannot be replaced by a download, and
    /// `register_from_api` refuses to try.
    Builtin,
    /// Fetched from the online library. Lives in the download cache,
    /// carries the server's version, and can be updated or cleared.
    Api { version: u32 },
    /// The user's, written in the formula language: carried in the
    /// config that uses it (a Rhai transformation as a flame variation,
    /// `variations::custom`), or a plug-in from their formulas. Not a
    /// download: the config or the file is its home, and loading either
    /// registers it again.
    #[serde(alias = "custom")]
    Local,
}

impl Provenance {
    /// The [`Origin`] this is.
    pub fn origin(&self) -> Origin {
        match self {
            Self::Builtin => Origin::Builtin,
            Self::Api { .. } => Origin::Api,
            Self::Local => Origin::Local,
        }
    }

    /// Did this ship with the app?
    pub fn is_builtin(&self) -> bool {
        matches!(self, Self::Builtin)
    }

    /// Is this code the app did not ship?
    ///
    /// The question §1 of the shared-resource plan actually asks. Both
    /// downloaded and local resources answer yes: a local plugin is not
    /// safer for having arrived through the filesystem.
    pub fn is_third_party(&self) -> bool {
        !self.is_builtin()
    }

    /// Would Clear Cache remove this?
    ///
    /// Only downloads. A local plugin sitting in the same list is not
    /// the app's to delete — the user put it there, and clearing a
    /// cache is not an invitation to remove their files.
    pub fn is_cached_download(&self) -> bool {
        matches!(self, Self::Api { .. })
    }

    /// The server-assigned version, when there is one.
    ///
    /// `None` for built-ins and local plugins, which is what makes an
    /// update offer impossible to raise for them by accident.
    pub fn version(&self) -> Option<u32> {
        match self {
            Self::Api { version } => Some(*version),
            _ => None,
        }
    }

    /// Short label for a listing row.
    pub fn label(&self) -> String {
        match self {
            Self::Builtin => "built-in".to_string(),
            Self::Api { version } => format!("API v{version}"),
            Self::Local => "local".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three questions come apart, which is the whole reason this
    /// is not a bool.
    #[test]
    fn the_users_own_is_third_party_but_not_cache_and_not_updatable() {
        let own = Provenance::Local;
        assert!(own.is_third_party(), "the user must be told it is not ours");
        assert!(!own.is_cached_download(), "Clear Cache must not remove the user's work");
        assert!(own.version().is_none(), "no update can be offered for it");
    }

    #[test]
    fn a_download_answers_yes_to_all_three() {
        let api = Provenance::Api { version: 4 };
        assert!(api.is_third_party());
        assert!(api.is_cached_download());
        assert_eq!(api.version(), Some(4));
    }

    #[test]
    fn a_builtin_answers_no_to_all_three() {
        let b = Provenance::Builtin;
        assert!(!b.is_third_party());
        assert!(!b.is_cached_download());
        assert!(b.version().is_none());
    }

    /// The wire spelling is part of the contract once a local plugin
    /// manifest exists, so pin it rather than let a rename slip.
    #[test]
    fn the_wire_spelling_is_stable() {
        let j = |p: &Provenance| serde_json::to_string(p).unwrap();
        assert_eq!(j(&Provenance::Builtin), "\"builtin\"");
        assert_eq!(j(&Provenance::Local), "\"local\"");
        assert_eq!(serde_json::from_str::<Provenance>("\"custom\"").unwrap(), Provenance::Local, "the old spelling still reads");
        assert_eq!(j(&Provenance::Api { version: 2 }), "{\"api\":{\"version\":2}}");
    }
}
