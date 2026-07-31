//! Finding scripts: shipped starters plus the user's own.
//!
//! Two sources on desktop — `assets/scripts/{generators,modifiers}/` next
//! to the executable's working directory, and a writable user folder
//! under the app data dir. The starters are ALSO embedded, so the panel
//! is never empty when the binary runs from somewhere without `assets/`.

use std::path::PathBuf;

use crate::config::fractal_config::FractalConfig;

use super::{ScriptHost, ScriptKind};

/// Starter scripts compiled in, so they exist regardless of cwd.
pub(crate) const EMBEDDED: &[(&str, &str)] = &[
    (
        "basic_random.rhai",
        include_str!("../../assets/scripts/generators/basic_random.rhai"),
    ),
    (
        "lsystem.rhai",
        include_str!("../../assets/scripts/generators/lsystem.rhai"),
    ),
    (
        "lsystem_plant.rhai",
        include_str!("../../assets/scripts/generators/lsystem_plant.rhai"),
    ),
    (
        "hilbert3d.rhai",
        include_str!("../../assets/scripts/generators/hilbert3d.rhai"),
    ),
    (
        "iq_palette.rhai",
        include_str!("../../assets/scripts/modifiers/iq_palette.rhai"),
    ),
    (
        "random_palette.rhai",
        include_str!("../../assets/scripts/modifiers/random_palette.rhai"),
    ),
    (
        "turntable.rhai",
        include_str!("../../assets/scripts/modifiers/turntable.rhai"),
    ),
    (
        "jitter.rhai",
        include_str!("../../assets/scripts/modifiers/jitter.rhai"),
    ),
    (
        "mutate.rhai",
        include_str!("../../assets/scripts/modifiers/mutate.rhai"),
    ),
    (
        "decompose_group.rhai",
        include_str!("../../assets/scripts/modifiers/decompose_group.rhai"),
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptOrigin {
    /// Compiled in — read-only; edits are saved as a user copy.
    Builtin,
    /// On disk, under `assets/scripts/` or the user folder.
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ScriptEntry {
    /// Stable identifier: the file stem, without `.rhai`.
    ///
    /// This is what one script names another by, and what the picker
    /// restores its selection with. The DECLARED name cannot serve:
    /// nothing stops two scripts declaring "Random Palette", and using
    /// it as a key made the picker jump between them.
    ///
    /// The stem is the de-facto unique key — `discover` builds its map
    /// on the file name. Shipped stems are **reserved**: a user script
    /// may not take one, so a built-in id always resolves to the
    /// built-in. That is what `run_script("random_palette", …)` needs;
    /// when a user copy could shadow it, one Save on the Random Palette
    /// panel silently changed what Basic Random produced.
    ///
    /// The cost, accepted deliberately: shipped script FILENAMES are a
    /// public API, and renaming one breaks whatever calls it.
    pub id: String,
    /// The script's declared name, falling back to the file stem.
    pub display_name: String,
    pub kind: ScriptKind,
    pub source: String,
    pub origin: ScriptOrigin,
}

impl ScriptEntry {
    /// `Generator · Basic Random` — the picker's label.
    pub fn label(&self) -> String {
        let tag = match self.kind {
            ScriptKind::Generator => "Generator",
            ScriptKind::Modifier => "Modifier",
        };
        let mark = if matches!(self.origin, ScriptOrigin::Builtin) { "" } else { " *" };
        format!("{tag} · {}{mark}", self.display_name)
    }
}

/// Find a script by its id (file stem), for one script to call another.
///
/// A shipped id always resolves to the shipped script — user scripts
/// cannot take those names (see [`merge_sources`]), so what a
/// `run_script("random_palette", …)` call means never depends on what
/// the user happens to have saved.
pub fn find(entries: &[ScriptEntry], id: &str) -> Option<ScriptEntry> {
    entries.iter().find(|e| e.id == id).cloned()
}

/// Where the user's own scripts live (created on demand by [`save_user_script`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn user_script_dir() -> Option<PathBuf> {
    crate::storage::backend::get_app_data_dir()
        .ok()
        .map(|d| d.join("scripts"))
}

/// Is this stem the name of a script that ships with the app?
///
/// Shipped stems are reserved: a user script cannot take one (see
/// [`merge_sources`]). `run_script("random_palette", …)` must always
/// mean the shipped Random Palette.
pub fn is_builtin_stem(stem: &str) -> bool {
    EMBEDDED
        .iter()
        .any(|(name, _)| name.trim_end_matches(".rhai") == stem)
}

/// Merge the three script sources into one library, refusing any user
/// script that would take a shipped name.
///
/// Pure, so the rule can be tested without touching the filesystem.
/// `found` is in precedence order: embedded, then `assets/scripts/`,
/// then the user folder.
///
/// Two different things used to be conflated under "later wins":
///
/// * `assets/scripts/` holds the SAME scripts as `EMBEDDED` — the disk
///   copy is the live, editable version of a shipped script. It should
///   and does win, so editing a file and restarting shows the change
///   without a recompile.
/// * A **user** script sharing a shipped stem is a DIFFERENT script
///   wearing a taken name. That used to silently replace the shipped
///   one, which was reachable in the shipped app: `random_palette.rhai`
///   declares its name as `random_palette`, so Save wrote
///   `random_palette.rhai` into the user folder and quietly changed
///   what `basic_random`'s `run_script("random_palette", …)` call
///   produced. Now it is refused and reported.
///
/// Returns the merged library and the stems that were refused.
///
/// `is_user` is supplied by the caller rather than derived here: telling
/// a user file from a shipped one needs the filesystem
/// ([`is_user_script`] canonicalizes paths), and keeping that out makes
/// the rule itself testable.
struct FoundScript {
    name: String,
    source: String,
    origin: ScriptOrigin,
    is_user: bool,
}

fn merge_sources(
    found: Vec<FoundScript>,
) -> (
    std::collections::BTreeMap<String, (String, ScriptOrigin)>,
    Vec<String>,
) {
    let mut by_name: std::collections::BTreeMap<String, (String, ScriptOrigin)> =
        std::collections::BTreeMap::new();
    let mut refused = Vec::new();

    for f in found {
        let stem = f.name.trim_end_matches(".rhai");
        // Only a USER file can be refused; shipped sources are allowed to
        // supersede each other (embedded <- assets).
        if f.is_user && is_builtin_stem(stem) {
            refused.push(stem.to_string());
            continue;
        }
        by_name.insert(f.name, (f.source, f.origin));
    }

    (by_name, refused)
}

/// All available scripts: embedded starters, `assets/scripts/`, then the
/// user folder.
///
/// A shipped script's disk copy supersedes its embedded copy — same
/// script, live version. A **user** script may not take a shipped name;
/// see [`merge_sources`].
///
/// `base` is only used to read each script's metadata (its declared name
/// and kind) — a modifier inspecting the current flame reports the same
/// way it will when run.
pub fn discover(base: &FractalConfig) -> Vec<ScriptEntry> {
    discover_with_conflicts(base).0
}

/// As [`discover`], plus the stems of user scripts that were refused
/// for taking a shipped name.
///
/// The panel shows these; a log line alone is not a report, since
/// nobody reads the console to find out why their file vanished from a
/// list.
pub fn discover_with_conflicts(base: &FractalConfig) -> (Vec<ScriptEntry>, Vec<String>) {
    let mut raw: Vec<(String, String, ScriptOrigin)> = Vec::new();

    for (name, source) in EMBEDDED {
        raw.push(((*name).to_string(), (*source).to_string(), ScriptOrigin::Builtin));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        for sub in ["generators", "modifiers"] {
            collect_dir(&PathBuf::from("assets/scripts").join(sub), &mut raw);
        }
        if let Some(dir) = user_script_dir() {
            collect_dir(&dir, &mut raw);
        }
    }

    // Classify here, where the filesystem is available; `merge_sources`
    // then applies the rule without touching it.
    let found: Vec<FoundScript> = raw
        .into_iter()
        .map(|(name, source, origin)| {
            let is_user = match &origin {
                ScriptOrigin::Builtin => false,
                #[cfg(not(target_arch = "wasm32"))]
                ScriptOrigin::File(path) => is_user_script(path),
                #[cfg(target_arch = "wasm32")]
                ScriptOrigin::File(_) => true,
            };
            FoundScript { name, source, origin, is_user }
        })
        .collect();

    let (by_name, refused) = merge_sources(found);
    for stem in &refused {
        log::warn!(
            "Ignoring your script `{stem}.rhai`: `{stem}` is a shipped script's name. \
             Rename it to load it."
        );
    }

    let host = ScriptHost::new();
    let mut entries: Vec<ScriptEntry> = by_name
        .into_iter()
        .map(|(name, (source, origin))| {
            let id = name.trim_end_matches(".rhai").to_string();
            // Read the declared name/kind. A script that fails to parse is
            // still listed — under its file name — so the user can select
            // it and see the error rather than wondering where it went.
            let meta = host.collect(&source, base).ok();
            let stem = name.trim_end_matches(".rhai").to_string();
            ScriptEntry {
                id,
                display_name: meta
                    .as_ref()
                    .map(|m| m.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or(stem),
                kind: meta
                    .as_ref()
                    .and_then(|m| m.kind)
                    .unwrap_or(ScriptKind::Generator),
                source,
                origin,
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        // Generators first, then alphabetically.
        (a.kind == ScriptKind::Modifier, a.display_name.to_lowercase())
            .cmp(&(b.kind == ScriptKind::Modifier, b.display_name.to_lowercase()))
    });
    (entries, refused)
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_dir(dir: &std::path::Path, out: &mut Vec<(String, String, ScriptOrigin)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rhai") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        match std::fs::read_to_string(&path) {
            Ok(source) => out.push((name.to_string(), source, ScriptOrigin::File(path.clone()))),
            Err(e) => log::warn!("Cannot read script {}: {e}", path.display()),
        }
    }
}

/// The file stem `save_user_script` would use for this name.
///
/// Exposed so a caller can tell in advance whether a name is free —
/// the sanitizing is what decides, not the raw string.
pub fn user_script_stem(file_name: &str) -> String {
    let safe: String = file_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    safe.trim_matches('_').to_string()
}

/// A stem near `desired` that no shipped script has claimed.
///
/// Returns `desired` untouched when it is free; otherwise appends
/// `-copy`, then `-copy-2`, and so on. Used to fork a shipped script
/// rather than shadow it.
#[cfg(not(target_arch = "wasm32"))]
pub fn free_user_script_stem(desired: &str) -> String {
    let base = user_script_stem(desired);
    if !is_builtin_stem(&base) {
        return base;
    }
    let candidate = format!("{base}-copy");
    if !is_builtin_stem(&candidate) {
        return candidate;
    }
    // Practically unreachable — no shipped script is named `x-copy` —
    // but a bounded search beats an unbounded loop.
    (2..1000)
        .map(|n| format!("{base}-copy-{n}"))
        .find(|s| !is_builtin_stem(s))
        .unwrap_or_else(|| format!("{base}-copy-x"))
}

/// Write a script to the user folder.
///
/// Always the user folder, never `assets/`, so the shipped originals
/// stay intact.
///
/// **Refuses a shipped script's name.** Such a file would be ignored by
/// [`discover`] anyway; failing here means the user finds out at the
/// moment of saving rather than wondering why their script vanished
/// from the list. Callers wanting to fork a shipped script should take
/// a name from [`free_user_script_stem`] first.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_user_script(file_name: &str, source: &str) -> Result<PathBuf, String> {
    let dir = user_script_dir().ok_or("cannot locate the application data folder")?;
    let stem = user_script_stem(file_name);
    if is_builtin_stem(&stem) {
        return Err(format!(
            "`{stem}` is a shipped script's name — save it under a different one"
        ));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{stem}.rhai"));
    std::fs::write(&path, source).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// Whether a script file lives in the user folder, and is therefore the
/// user's to delete.
///
/// This matters because `discover` gives `ScriptOrigin::File` to BOTH
/// the shipped `assets/scripts/` files and the user's own copies — the
/// origin alone does not say who owns a script. Without this check a
/// Delete button would happily remove the starters that ship with the
/// app.
#[cfg(not(target_arch = "wasm32"))]
pub fn is_user_script(path: &std::path::Path) -> bool {
    let Some(dir) = user_script_dir() else {
        return false;
    };
    // Compare canonical paths so `..` or a symlinked data folder cannot
    // dress a shipped file up as a user one.
    match (path.canonicalize(), dir.canonicalize()) {
        (Ok(path), Ok(dir)) => path.starts_with(dir),
        // An un-canonicalizable path (already deleted, say) is not one we
        // are willing to delete.
        _ => false,
    }
}

/// Delete a script from the user folder.
///
/// Refuses anything outside it, so a shipped starter can never be
/// removed — the worst case is that a user script the person wrote
/// themselves goes. Shipped scripts are unaffected by anything here;
/// to "reset" one, delete the fork and select the original.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_user_script(path: &std::path::Path) -> Result<(), String> {
    if !is_user_script(path) {
        return Err(format!(
            "{} is not in your scripts folder, so it will not be deleted",
            path.display()
        ));
    }
    std::fs::remove_file(path).map_err(|e| format!("cannot delete {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(name: &str, source: &str) -> FoundScript {
        FoundScript {
            name: name.to_string(),
            source: source.to_string(),
            origin: ScriptOrigin::Builtin,
            is_user: false,
        }
    }
    fn user(name: &str, source: &str) -> FoundScript {
        FoundScript {
            name: name.to_string(),
            source: source.to_string(),
            origin: ScriptOrigin::File(std::path::PathBuf::from(name)),
            is_user: true,
        }
    }

    /// A user script may not take a shipped script's name.
    ///
    /// This was reachable in the shipped app, not just in theory:
    /// `random_palette.rhai` declares its name as `random_palette`, so
    /// selecting Random Palette and pressing Save wrote
    /// `random_palette.rhai` into the user folder, which then replaced
    /// the shipped one — silently changing what `basic_random`'s
    /// `run_script("random_palette", …)` produced.
    #[test]
    fn a_user_script_cannot_take_a_shipped_name() {
        let (merged, refused) = merge_sources(vec![
            shipped("random_palette.rhai", "SHIPPED"),
            user("random_palette.rhai", "USER"),
        ]);
        assert_eq!(refused, vec!["random_palette".to_string()]);
        assert_eq!(
            merged["random_palette.rhai"].0, "SHIPPED",
            "the shipped script must survive a same-named user file"
        );
    }

    /// The shipped disk copy supersedes the embedded one — same script,
    /// live version. That is NOT the shadowing being prevented, and
    /// breaking it would break editing a script without a recompile.
    #[test]
    fn the_disk_copy_of_a_shipped_script_still_wins() {
        let mut assets = shipped("basic_random.rhai", "FROM_ASSETS");
        assets.origin = ScriptOrigin::File(std::path::PathBuf::from(
            "assets/scripts/generators/basic_random.rhai",
        ));
        let (merged, refused) = merge_sources(vec![
            shipped("basic_random.rhai", "EMBEDDED"),
            assets,
        ]);
        assert!(refused.is_empty(), "a shipped disk copy is not a conflict");
        assert_eq!(merged["basic_random.rhai"].0, "FROM_ASSETS");
    }

    /// A user script with its own name loads normally.
    #[test]
    fn an_ordinary_user_script_loads() {
        let (merged, refused) = merge_sources(vec![user("my_thing.rhai", "MINE")]);
        assert!(refused.is_empty());
        assert_eq!(merged["my_thing.rhai"].0, "MINE");
    }

    /// Forking picks a name that is free, and leaves a free name alone.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn forking_avoids_every_shipped_name() {
        assert_eq!(free_user_script_stem("my_thing"), "my_thing");
        assert_eq!(free_user_script_stem("random_palette"), "random_palette-copy");
        // The sanitizer runs first: "Basic Random" is not a shipped stem.
        assert_eq!(free_user_script_stem("Basic Random"), "Basic_Random");
        for (name, _) in EMBEDDED {
            let stem = name.trim_end_matches(".rhai");
            assert!(
                !is_builtin_stem(&free_user_script_stem(stem)),
                "fork of `{stem}` still collides"
            );
        }
    }

    /// Saving under a shipped name fails loudly rather than writing a
    /// file `discover` would then ignore.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn saving_under_a_shipped_name_is_refused() {
        let err = save_user_script("random_palette", "script(\"x\", \"generator\");")
            .expect_err("should refuse a shipped name");
        assert!(err.contains("shipped script"), "{err}");
    }

    #[test]
    fn embedded_starters_are_valid_and_classified() {
        let base = FractalConfig::default();
        let entries = discover(&base);
        assert!(entries.len() >= 2, "embedded starters always present");

        let gen = entries
            .iter()
            .find(|e| e.display_name == "Basic Random")
            .expect("basic_random is listed under its declared name");
        assert_eq!(gen.kind, ScriptKind::Generator);

        let modifier = entries
            .iter()
            .find(|e| e.display_name == "Jitter")
            .expect("jitter is listed");
        assert_eq!(modifier.kind, ScriptKind::Modifier);

        // Generators sort ahead of modifiers.
        let first_modifier = entries.iter().position(|e| e.kind == ScriptKind::Modifier);
        let last_generator = entries.iter().rposition(|e| e.kind == ScriptKind::Generator);
        if let (Some(m), Some(g)) = (first_modifier, last_generator) {
            assert!(g < m, "generators listed before modifiers");
        }
    }

    #[test]
    fn every_shipped_script_runs() {
        // A starter that errors is worse than no starter: it is the first
        // thing a user runs, and the documentation they copy from.
        let base = FractalConfig::default();
        let host = ScriptHost::new();
        for (name, source) in EMBEDDED {
            let out = host
                .run(source, &base, 12345, Default::default())
                .unwrap_or_else(|e| panic!("shipped script {name} failed: {e}"));
            assert!(
                out.warnings.is_empty(),
                "shipped script {name} warns: {:?}",
                out.warnings
            );
        }
    }

    #[test]
    fn shipped_generator_produces_a_usable_flame() {
        let base = FractalConfig::default();
        let host = ScriptHost::new();
        let source = EMBEDDED[0].1;
        for seed in [1u64, 2, 7, 99, 12345] {
            let out = host.run(source, &base, seed, Default::default()).unwrap();
            let n = out.config.flame.transforms.len();
            assert!(n >= 1, "seed {seed} produced no transforms");
            for t in &out.config.flame.transforms {
                assert!(t.weight > 0.0, "seed {seed}: zero-weight transform");
                assert!(t.a.is_finite() && t.d.is_finite(), "seed {seed}: bad affine");
            }
        }
    }
}
