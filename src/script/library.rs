//! Finding scripts: shipped starters plus the user's own.
//!
//! Three sources. The starters are **embedded**, so the panel is never
//! empty when the binary runs from somewhere without `assets/`; on
//! desktop their live disk copies under `assets/scripts/{generators,
//! modifiers}/` supersede them, so editing a file and restarting shows
//! the change without a recompile. The user's own scripts come from
//! [`super::store`], which works on both desktop and the web.
//!
//! Writing, deleting and enumerating the user's scripts all live in
//! `store`. What lives here is the merge: which source wins, and which
//! user script is refused for taking a shipped name.

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
        "mandala.rhai",
        include_str!("../../assets/scripts/generators/mandala.rhai"),
    ),
    (
        "gnarl.rhai",
        include_str!("../../assets/scripts/generators/gnarl.rhai"),
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
    (
        "kaleidoscope.rhai",
        include_str!("../../assets/scripts/modifiers/kaleidoscope.rhai"),
    ),
    (
        "zoom_dive.rhai",
        include_str!("../../assets/scripts/modifiers/zoom_dive.rhai"),
    ),
];

/// Where a script came from — and therefore who owns it.
///
/// The old two-variant form lumped the shipped `assets/scripts/` files
/// in with the user's own under a single `File(PathBuf)`, so origin
/// alone could not answer "may this be deleted"; the panel asked a
/// separate path-canonicalizing check to find out — which could not
/// exist on the web, where there are no paths.
/// Splitting the variants makes ownership structural: a `User` script is
/// the user's, and nothing else is, on either platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptOrigin {
    /// Compiled in — read-only; edits are saved as a user copy.
    Builtin,
    /// A shipped script's live disk copy under `assets/scripts/`.
    /// Desktop only, and still the app's file rather than the user's.
    Shipped(PathBuf),
    /// The user's own, in [`super::store`]. Editable and deletable.
    User,
    /// Opened from outside every store — an ad-hoc file the panel is
    /// holding but does not own, so Save must ask where to put it.
    External,
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
    /// Somebody else wrote this — it came from the online library.
    ///
    /// Read from the store's link record, not from the origin, and that
    /// distinction is the point: adopting a downloaded script makes it
    /// `ScriptOrigin::User`, and if trust were derived from the origin
    /// alone, pressing Save would launder away the cross-call
    /// restriction. The user chose to keep it; they did not read it.
    pub untrusted: bool,
}

impl ScriptEntry {
    /// `Generator · Basic Random` — the picker's label.
    pub fn label(&self) -> String {
        let tag = match self.kind {
            ScriptKind::Generator => "Generator",
            ScriptKind::Modifier => "Modifier",
        };
        let mark = if self.untrusted {
            " ↓"
        } else if matches!(self.origin, ScriptOrigin::User) {
            " *"
        } else {
            ""
        };
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
/// The classification used to be passed in as data, because telling a
/// user file from a shipped one needed the filesystem. It no longer
/// does: [`ScriptOrigin::User`] says so directly, on both platforms.
struct FoundScript {
    name: String,
    source: String,
    origin: ScriptOrigin,
    untrusted: bool,
}

fn merge_sources(
    found: Vec<FoundScript>,
) -> (
    std::collections::BTreeMap<String, (String, ScriptOrigin, bool)>,
    Vec<String>,
) {
    let mut by_name: std::collections::BTreeMap<String, (String, ScriptOrigin, bool)> =
        std::collections::BTreeMap::new();
    let mut refused = Vec::new();

    for f in found {
        let stem = f.name.trim_end_matches(".rhai");
        // Only a USER script can be refused; shipped sources are allowed
        // to supersede each other (embedded <- assets).
        if f.origin == ScriptOrigin::User && is_builtin_stem(stem) {
            refused.push(stem.to_string());
            continue;
        }
        by_name.insert(f.name, (f.source, f.origin, f.untrusted));
    }

    (by_name, refused)
}

/// All available scripts: embedded starters, `assets/scripts/`, then the
/// user's own store.
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
    // Precedence order: embedded, then the shipped disk copies, then the
    // user's own. `merge_sources` applies the rule.
    let mut found: Vec<FoundScript> = EMBEDDED
        .iter()
        .map(|(name, source)| FoundScript {
            name: (*name).to_string(),
            source: (*source).to_string(),
            origin: ScriptOrigin::Builtin,
            untrusted: false,
        })
        .collect();

    #[cfg(not(target_arch = "wasm32"))]
    for sub in ["generators", "modifiers"] {
        collect_dir(&crate::resources::resource_path("assets/scripts").join(sub), &mut found);
    }

    // The user's own, from the cross-platform store. This is the half
    // that did not exist on the web at all.
    let links = super::store::load_links();
    for (stem, source) in super::store::list() {
        let untrusted = links.get(&stem).is_some_and(|l| l.from_others);
        found.push(FoundScript {
            name: format!("{stem}.rhai"),
            source,
            origin: ScriptOrigin::User,
            untrusted,
        });
    }

    let (by_name, refused) = merge_sources(found);
    for stem in &refused {
        log::warn!(
            "Ignoring your script `{stem}`: that is a shipped script's name. Rename it to load it."
        );
    }

    let host = ScriptHost::new();
    let mut entries: Vec<ScriptEntry> = by_name
        .into_iter()
        .map(|(name, (source, origin, untrusted))| {
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
                untrusted,
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
fn collect_dir(dir: &std::path::Path, out: &mut Vec<FoundScript>) {
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
            Ok(source) => out.push(FoundScript {
                name: name.to_string(),
                source,
                origin: ScriptOrigin::Shipped(path.clone()),
                untrusted: false,
            }),
            Err(e) => log::warn!("Cannot read script {}: {e}", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(name: &str, source: &str) -> FoundScript {
        FoundScript {
            name: name.to_string(),
            source: source.to_string(),
            origin: ScriptOrigin::Builtin,
            untrusted: false,
        }
    }
    fn user(name: &str, source: &str) -> FoundScript {
        FoundScript {
            name: name.to_string(),
            source: source.to_string(),
            origin: ScriptOrigin::User,
            untrusted: false,
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
        assets.origin = ScriptOrigin::Shipped(std::path::PathBuf::from(
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
