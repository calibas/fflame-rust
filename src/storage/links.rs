//! What a stored file is linked to, and where it came from: one record
//! per file of the user's script store (`scripts/`) and formula store
//! (`formulas/`), in a sidecar `_links.json` in each
//! (docs/archive/projects/rhai-plugins.md, P3).
//!
//! Kept beside the files rather than in them, because a `.rhai` file has
//! nowhere to put it and its header comment is the script's own prose,
//! not ours to write into. Keyed as the store keys its files: the
//! scripts by stem, the formulas by file name (a formula file's
//! extension is part of its name -- `spiral.rhai` and `spiral.ufm` are
//! two files).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::storage::backend;

/// Per-file metadata that is not the source.
///
/// What the stores needed turned out to be one record:
///
/// * **The cloud link** -- an id and the version last seen. Without it,
///   an update after a restart is impossible: optimistic concurrency
///   needs the version you read, and "the version you read" does not
///   survive a process that forgot which server script this even is.
/// * **Provenance** -- whether this came from somebody else. This is the
///   part that must be persistent rather than a UI flag, because saving
///   a downloaded script would otherwise make it the user's own and
///   trusted, laundering the cross-call restriction through Save.
/// * **Plug-in** -- a GPU Rhai entry the app registers at every start
///   (`storage::library::load_plugins`), so it is in the lists without a
///   config carrying it.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Link {
    /// The server id, when this file exists there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_id: Option<String>,
    /// The version last read from the server -- what an update sends
    /// back, and what a 409 compares against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// `owner_display_name/name`, the globally unique human-readable key
    /// the server's unique display names make possible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Somebody else wrote this.
    ///
    /// Adopting a script -- saving it locally -- does not make it yours in
    /// the sense that matters here. The user chose to keep it; they did
    /// not read it. So this survives the save, and
    /// `ScriptHost::with_untrusted` keeps applying.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub from_others: bool,
    /// Registered at every start (a formula store entry; meaningless for
    /// a CPU script, which the Scripts panel always lists).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub plugin: bool,
}

fn path(store: &str) -> PathBuf {
    Path::new(store).join("_links.json")
}

/// Every link in `store`, keyed as the store keys its files.
///
/// A corrupt file is treated as absent rather than fatal: losing it
/// costs a re-link, and failing every listing over it would be the wrong
/// trade. The one thing that is NOT safe to lose silently is
/// `from_others`, so `script::store::is_untrusted` reads it believing
/// what is there.
pub fn load(store: &str) -> BTreeMap<String, Link> {
    match backend::read_file(&path(store)) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            log::warn!("Discarding unreadable link file in `{store}`: {e}");
            Default::default()
        }),
        Err(_) => Default::default(),
    }
}

fn save(store: &str, links: &BTreeMap<String, Link>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(links).map_err(|e| format!("cannot serialize the links in `{store}`: {e}"))?;
    backend::write_file(&path(store), &json).map_err(|e| format!("cannot write the links in `{store}`: {e}"))
}

/// The link for one file, if it has one.
pub fn get(store: &str, key: &str) -> Option<Link> {
    load(store).get(key).cloned()
}

/// Record (or replace) a file's link. An empty one is removed instead,
/// so the sidecar holds only what says something.
pub fn set(store: &str, key: &str, link: Link) -> Result<(), String> {
    let mut links = load(store);
    if link == Link::default() {
        if links.remove(key).is_none() {
            return Ok(());
        }
    } else {
        links.insert(key.to_string(), link);
    }
    save(store, &links)
}

/// Forget a file's link: what deleting the file does, so a name reused
/// later does not inherit the previous file's cloud identity -- or its
/// provenance, which would mark the user's own file as somebody else's.
pub fn clear(store: &str, key: &str) -> Result<(), String> {
    let mut links = load(store);
    if links.remove(key).is_none() {
        return Ok(());
    }
    save(store, &links)
}
