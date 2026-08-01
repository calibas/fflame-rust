//! The user's own scripts — one store, both platforms.
//!
//! Until now saving a script was desktop-only: every write, delete and
//! enumerate was `#[cfg(not(target_arch = "wasm32"))]`, so a web user
//! could edit a script in the panel and had **no way to keep it**. The
//! text lived until the tab closed. Routing all four operations through
//! [`crate::storage::backend`] — the filesystem on desktop, localStorage
//! on the web — closes that, and the desktop path is unchanged: the
//! backend resolves `scripts/foo.rhai` to the same
//! `<app data>/scripts/foo.rhai` the old code wrote directly.
//!
//! On the web this store is not a cache of something the server holds.
//! It is the only copy, which is why a quota failure is reported rather
//! than swallowed.
//!
//! # Why the stem is the key
//!
//! A script is identified by its stem (`turntable`), not by a path.
//! There are no paths in localStorage, and the stem was already the
//! de-facto identity — it is what `run_script("random_palette", …)`
//! names, and what the picker restores its selection with. Making it the
//! literal key removes the desktop-only `is_user_script` path
//! canonicalization: ownership is now a property of where a script came
//! from, not a string comparison against a directory.

use std::path::{Path, PathBuf};

use crate::storage::backend;

/// Storage prefix: a directory on desktop, a localStorage key prefix on
/// the web.
const PREFIX: &str = "scripts";

/// Largest script we will store, per script.
///
/// Not about the script engine — the host has its own execution limits.
/// This is about the store: on the web it is a browser-wide localStorage
/// quota (commonly ~5 MB) shared with system settings, the palette and
/// variation caches and the thumbnail cache, so one runaway paste must
/// not be able to evict all of it. 256 KB is roughly 40× the largest
/// script that ships.
pub const MAX_SCRIPT_BYTES: usize = 256 * 1024;

/// The stem [`save`] would store this name under.
///
/// Exposed so a caller can tell in advance whether a name is free — the
/// sanitizing is what decides that, not the raw string.
///
/// Everything outside `[alphanumeric-_]` becomes `_`, which is also what
/// keeps the result usable as both a file name and a storage key: `/`,
/// `\` and `.` cannot survive it, so neither can `..` or an absolute
/// path.
pub fn stem_for(file_name: &str) -> String {
    let safe: String = file_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    safe.trim_matches('_').to_string()
}

/// A stem near `desired` that no shipped script has claimed.
///
/// Returns `desired`'s sanitized form untouched when it is free;
/// otherwise appends `-copy`, then `-copy-2`, and so on. Used to fork a
/// shipped script rather than shadow it.
pub fn free_stem(desired: &str) -> String {
    let base = stem_for(desired);
    if !super::library::is_builtin_stem(&base) {
        return base;
    }
    let candidate = format!("{base}-copy");
    if !super::library::is_builtin_stem(&candidate) {
        return candidate;
    }
    // Practically unreachable — no shipped script is named `x-copy` —
    // but a bounded search beats an unbounded loop.
    (2..1000)
        .map(|n| format!("{base}-copy-{n}"))
        .find(|s| !super::library::is_builtin_stem(s))
        .unwrap_or_else(|| format!("{base}-copy-x"))
}

fn key_for(stem: &str) -> PathBuf {
    Path::new(PREFIX).join(format!("{stem}.rhai"))
}

/// Why a name cannot be stored.
///
/// Separated from `save` so the rule is testable without touching a
/// filesystem or a browser — the same reason `merge_sources` takes its
/// classification as data.
pub fn check_name(name: &str, source: &str) -> Result<String, String> {
    let stem = stem_for(name);
    if stem.is_empty() {
        return Err(
            "that name has nothing usable in it — give it at least one letter or digit"
                .to_string(),
        );
    }
    // A shipped stem is reserved: such a file would be ignored by
    // `discover` anyway, so failing here means the user finds out while
    // saving rather than wondering why their script vanished.
    if super::library::is_builtin_stem(&stem) {
        return Err(format!(
            "`{stem}` is a shipped script's name — save it under a different one"
        ));
    }
    if source.len() > MAX_SCRIPT_BYTES {
        return Err(format!(
            "that script is {} KB; the limit is {} KB",
            source.len() / 1024,
            MAX_SCRIPT_BYTES / 1024
        ));
    }
    Ok(stem)
}

/// Write a script to the user's store.
///
/// Never touches `assets/`, so the shipped originals stay intact.
/// Callers wanting to fork a shipped script should take a name from
/// [`free_stem`] first.
///
/// Returns **the stem it was actually stored under**, which is not
/// always the name asked for: `stem_for` sanitizes, so "My Script"
/// becomes `My_Script`. The caller needs the real one — it is how the
/// script is found again, and a fork selects the new entry by it. Pair
/// it with [`location_of`] for something to show the user.
pub fn save(name: &str, source: &str) -> Result<String, String> {
    let stem = check_name(name, source)?;
    backend::write_file(&key_for(&stem), source)
        .map_err(|e| format!("cannot save `{stem}`: {e}"))?;
    Ok(stem)
}

/// Read one script back, or `None` if the user has no such script.
pub fn load(stem: &str) -> Option<String> {
    backend::read_file(&key_for(stem)).ok()
}

/// Remove a script from the user's store.
///
/// Only ever reaches the user's own store: the key is built from a stem,
/// so there is no path for a caller to aim somewhere else. That is the
/// structural replacement for the old canonicalize-and-compare guard,
/// which existed because `ScriptOrigin::File` covered shipped files too.
pub fn delete(stem: &str) -> Result<(), String> {
    backend::delete_file(&key_for(stem)).map_err(|e| format!("cannot delete `{stem}`: {e}"))?;
    // Forget the link as well, or a stem reused later inherits the
    // previous script's cloud identity — and, worse, its provenance:
    // deleting a downloaded script and writing your own under the same
    // name would leave yours marked as somebody else's.
    clear_link(stem)
}

/// Every script in the user's store, as `(stem, source)`.
///
/// An unreadable entry is skipped with a warning rather than failing the
/// listing: one corrupt script should not hide the rest.
pub fn list() -> Vec<(String, String)> {
    let names = match backend::list_entries(Path::new(PREFIX)) {
        Ok(n) => n,
        Err(e) => {
            log::warn!("Cannot list your scripts: {e}");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in names {
        let Some(stem) = name.strip_suffix(".rhai") else {
            continue;
        };
        match backend::read_file(&key_for(stem)) {
            Ok(source) => out.push((stem.to_string(), source)),
            Err(e) => log::warn!("Cannot read your script `{stem}`: {e}"),
        }
    }
    out
}

/// Where a stored script lives, phrased for a status line.
#[cfg(not(target_arch = "wasm32"))]
pub fn location_of(stem: &str) -> String {
    match backend::get_app_data_dir() {
        Ok(dir) => dir.join(PREFIX).join(format!("{stem}.rhai")).display().to_string(),
        Err(_) => format!("{PREFIX}/{stem}.rhai"),
    }
}

/// On the web there is no path to show, and saying "saved" without
/// saying *where* invites the reasonable assumption that it went to the
/// server. It did not.
#[cfg(target_arch = "wasm32")]
pub fn location_of(_stem: &str) -> String {
    "this browser (not the cloud — it stays on this device)".to_string()
}

// ============================================================================
// What a stored script is linked to, and where it came from
// ============================================================================

/// Per-script metadata that is not the source.
///
/// Kept in one sidecar file rather than beside each script, because a
/// `.rhai` file has nowhere to put it and a header comment is the
/// script's own prose, not ours to write into.
///
/// Two things live here, and they are needed by different features that
/// turn out to want the same record:
///
/// * **The cloud link** — an id and the version last seen. Without it,
///   an update after a restart is impossible: optimistic concurrency
///   needs the version you read, and "the version you read" does not
///   survive a process that forgot which server script this even is.
/// * **Provenance** — whether this came from somebody else. This is the
///   part that must be persistent rather than a UI flag, because saving
///   a downloaded script would otherwise make it the user's own and
///   trusted, laundering the cross-call restriction through Save.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ScriptLink {
    /// The server id, when this script exists there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_id: Option<String>,
    /// The version last read from the server — what an update sends
    /// back, and what a 409 compares against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// `owner_display_name/name`, the globally unique human-readable key
    /// the server's unique display names make possible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Somebody else wrote this.
    ///
    /// Adopting a script — saving it locally — does not make it yours in
    /// the sense that matters here. The user chose to keep it; they did
    /// not read it. So this survives the save, and
    /// `ScriptHost::with_untrusted` keeps applying.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub from_others: bool,
}

fn links_path() -> PathBuf {
    Path::new(PREFIX).join("_links.json")
}

/// Every stored link, keyed by stem.
///
/// A corrupt file is treated as absent rather than fatal: losing it
/// costs a re-link, and failing every script listing over it would be
/// the wrong trade. The one thing that is NOT safe to lose silently is
/// `from_others`, so [`is_untrusted`] fails closed — see there.
pub fn load_links() -> std::collections::BTreeMap<String, ScriptLink> {
    match backend::read_file(&links_path()) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            log::warn!("Discarding unreadable script link file: {e}");
            Default::default()
        }),
        Err(_) => Default::default(),
    }
}

fn save_links(links: &std::collections::BTreeMap<String, ScriptLink>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(links)
        .map_err(|e| format!("cannot serialize script links: {e}"))?;
    backend::write_file(&links_path(), &json)
        .map_err(|e| format!("cannot write script links: {e}"))
}

/// The link for one script, if it has one.
pub fn link_of(stem: &str) -> Option<ScriptLink> {
    load_links().get(stem).cloned()
}

/// Record (or replace) a script's link.
pub fn set_link(stem: &str, link: ScriptLink) -> Result<(), String> {
    let mut links = load_links();
    links.insert(stem.to_string(), link);
    save_links(&links)
}

/// Forget a script's link. Called by [`delete`], so a stem that is
/// reused later does not inherit the previous script's cloud identity.
pub fn clear_link(stem: &str) -> Result<(), String> {
    let mut links = load_links();
    if links.remove(stem).is_none() {
        return Ok(());
    }
    save_links(&links)
}

/// Whether a stored script must run under the cross-call restriction.
///
/// Note the asymmetry with [`load_links`]: an unreadable link file is
/// tolerated everywhere else, because the cost is a re-link. Here the
/// cost is running somebody else's script as if it were the user's, so
/// a link that exists and says so is believed, and the *absence* of a
/// link means locally authored — which is true, because every path that
/// brings in a foreign script writes one.
pub fn is_untrusted(stem: &str) -> bool {
    link_of(stem).is_some_and(|l| l.from_others)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sanitizer must not be able to produce a name that escapes the
    /// store — a stem becomes both a file name and a localStorage key.
    #[test]
    fn a_stem_can_never_escape_the_store() {
        for hostile in [
            "../../etc/passwd",
            "..\\..\\windows\\system32",
            "/absolute",
            "C:\\x",
            "a/b",
            "a.rhai",
            "nul.txt",
        ] {
            let stem = stem_for(hostile);
            assert!(!stem.contains('/'), "{hostile} -> {stem}");
            assert!(!stem.contains('\\'), "{hostile} -> {stem}");
            assert!(!stem.contains('.'), "{hostile} -> {stem}");
            assert!(stem != ".." && stem != ".", "{hostile} -> {stem}");
        }
    }

    /// A name that sanitizes away entirely is refused rather than stored
    /// as `.rhai` — a hidden file on Unix, and an unaddressable entry on
    /// the web.
    #[test]
    fn a_name_with_nothing_usable_is_refused() {
        assert_eq!(stem_for("***"), "");
        let err = check_name("***", "").expect_err("must refuse");
        assert!(err.contains("at least one letter"), "{err}");
        // `___` trims to empty too.
        assert!(check_name("___", "").is_err());
    }

    #[test]
    fn a_shipped_name_is_refused() {
        let err = check_name("random_palette", "").expect_err("must refuse");
        assert!(err.contains("shipped script"), "{err}");
        // And the sanitized form is what decides: "Random Palette"
        // sanitizes to `Random_Palette`, which is nobody's name.
        assert_eq!(check_name("Random Palette", "").unwrap(), "Random_Palette");
    }

    /// Oversize is refused *before* the write, so on the web a runaway
    /// paste cannot consume the shared localStorage quota that system
    /// settings and the caches also live in.
    #[test]
    fn an_oversize_script_is_refused() {
        let big = "x".repeat(MAX_SCRIPT_BYTES + 1);
        let err = check_name("mine", &big).expect_err("must refuse");
        assert!(err.contains("limit is"), "{err}");
        // Exactly at the limit is allowed.
        assert!(check_name("mine", &"x".repeat(MAX_SCRIPT_BYTES)).is_ok());
    }

    /// Forking picks a name that is free, and leaves a free name alone.
    #[test]
    fn forking_avoids_every_shipped_name() {
        assert_eq!(free_stem("my_thing"), "my_thing");
        assert_eq!(free_stem("random_palette"), "random_palette-copy");
        // The sanitizer runs first: "Basic Random" is not a shipped stem.
        assert_eq!(free_stem("Basic Random"), "Basic_Random");
        for (name, _) in super::super::library::EMBEDDED {
            let stem = name.trim_end_matches(".rhai");
            assert!(
                !super::super::library::is_builtin_stem(&free_stem(stem)),
                "fork of `{stem}` still collides"
            );
        }
    }

    /// Sanitizing must be idempotent, or the stem `save` returns would
    /// not be a name you can hand back to `save` and reach the same
    /// place.
    #[test]
    fn sanitizing_a_stem_twice_changes_nothing() {
        for name in ["My Script", "___x___", "a/b", "ünïcode", "-dash-", "9"] {
            let once = stem_for(name);
            assert_eq!(stem_for(&once), once, "{name}");
        }
    }

    /// `save` reports the stem it used, not the one it was given.
    ///
    /// They differ whenever the name needs sanitizing, and the caller
    /// needs the real one: it is how the script is found again, and a
    /// fork selects the new entry by it. Getting this wrong is silent —
    /// the write succeeds and the script is simply not where the caller
    /// thinks it is.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn save_reports_the_stem_it_actually_used() {
        let source = "script(\"Round Trip\", \"generator\");\n";
        let stem = save("__store round trip test__", source).expect("save");
        assert_eq!(stem, "store_round_trip_test", "spaces map, underscores trim");
        delete(&stem).expect("cleanup");
    }

    /// Round-trip through the real backend: save, list, load, delete.
    ///
    /// Desktop-only because the WASM half needs a browser; the code path
    /// either side of `backend` is the same.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_script_survives_a_save_and_can_be_deleted() {
        let name = "store-round-trip-test";
        let source = "script(\"Round Trip\", \"generator\");\n";
        let _ = delete(name);

        let stem = save(name, source).expect("save");
        assert_eq!(stem, name, "an already-clean name is left alone");
        assert!(location_of(&stem).contains(name), "{}", location_of(&stem));

        assert_eq!(load(&stem).as_deref(), Some(source));
        assert!(
            list().iter().any(|(s, src)| s == &stem && src == source),
            "a saved script must appear in the listing"
        );

        delete(&stem).expect("delete");
        assert!(load(&stem).is_none(), "it should actually be gone");
        assert!(!list().iter().any(|(s, _)| s == &stem));
    }
}
