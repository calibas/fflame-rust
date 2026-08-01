//! Where a resource came from — shared by variations and effects.
//!
//! Defined once, in a neutral place, because both subsystems need the
//! same three answers and a per-subsystem enum would drift. Scripts
//! deliberately do **not** use this: their provenance question is
//! narrower ("did somebody else write this", answered by
//! `script::store::ScriptLink::from_others`) and their storage is not a
//! cache.
//!
//! # Why this replaces `is_core: bool`
//!
//! A bool could answer "did this ship with the app", which is what the
//! call sites happened to need while downloads were the only other
//! source. It cannot answer the three questions they were really
//! asking, and those come apart the moment locally installed resources
//! exist:
//!
//! | | shipped | downloaded | local plugin |
//! |---|---|---|---|
//! | third-party code? | no | **yes** | **yes** |
//! | in the download cache? | no | **yes** | no |
//! | updatable from the server? | no | **yes** | no |
//!
//! A local plugin is third-party code that must be reported as such,
//! but Clear Cache must not remove it and no update can be offered for
//! it. Substituting a `Local` variant into code that reads `!is_core`
//! would get two of those three wrong — silently, since both failures
//! look like "nothing happened".

use serde::{Deserialize, Serialize};

/// Where a variation or effect came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Ships with the app. Cannot be replaced by a download, and
    /// `register_from_api` refuses to try.
    Builtin,
    /// Fetched from the online library. Lives in the download cache,
    /// carries the server's version, and can be updated or cleared.
    Api { version: u32 },
    /// Installed by the user from their own files. Third-party code the
    /// app did not fetch and cannot refresh — removing it is the user's
    /// business, not Clear Cache's.
    Local,
}

impl Provenance {
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
    fn a_local_plugin_is_third_party_but_not_cache_and_not_updatable() {
        let local = Provenance::Local;
        assert!(local.is_third_party(), "the user must be told it is not ours");
        assert!(!local.is_cached_download(), "Clear Cache must not remove the user's files");
        assert!(local.version().is_none(), "no update can be offered for it");
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
        assert_eq!(j(&Provenance::Api { version: 2 }), "{\"api\":{\"version\":2}}");
    }
}
