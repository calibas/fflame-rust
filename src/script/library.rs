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
const EMBEDDED: &[(&str, &str)] = &[
    (
        "basic_random.rhai",
        include_str!("../../assets/scripts/generators/basic_random.rhai"),
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
        "decompose_schottky.rhai",
        include_str!("../../assets/scripts/modifiers/decompose_schottky.rhai"),
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

/// Where the user's own scripts live (created on demand by [`save_user_script`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn user_script_dir() -> Option<PathBuf> {
    crate::storage::backend::get_app_data_dir()
        .ok()
        .map(|d| d.join("scripts"))
}

/// All available scripts: embedded starters, `assets/scripts/`, then the
/// user folder. Later sources override earlier ones by file name, so a
/// user copy of `basic_random.rhai` shadows the shipped one.
///
/// `base` is only used to read each script's metadata (its declared name
/// and kind) — a modifier inspecting the current flame reports the same
/// way it will when run.
pub fn discover(base: &FractalConfig) -> Vec<ScriptEntry> {
    let mut found: Vec<(String, String, ScriptOrigin)> = Vec::new();

    for (name, source) in EMBEDDED {
        found.push(((*name).to_string(), (*source).to_string(), ScriptOrigin::Builtin));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        for sub in ["generators", "modifiers"] {
            collect_dir(&PathBuf::from("assets/scripts").join(sub), &mut found);
        }
        if let Some(dir) = user_script_dir() {
            collect_dir(&dir, &mut found);
        }
    }

    // Later wins on a name clash.
    let mut by_name: std::collections::BTreeMap<String, (String, ScriptOrigin)> =
        std::collections::BTreeMap::new();
    for (name, source, origin) in found {
        by_name.insert(name, (source, origin));
    }

    let host = ScriptHost::new();
    let mut entries: Vec<ScriptEntry> = by_name
        .into_iter()
        .map(|(name, (source, origin))| {
            // Read the declared name/kind. A script that fails to parse is
            // still listed — under its file name — so the user can select
            // it and see the error rather than wondering where it went.
            let meta = host.collect(&source, base).ok();
            let stem = name.trim_end_matches(".rhai").to_string();
            ScriptEntry {
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
    entries
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

/// Write a script to the user folder.
///
/// Always the user folder, never `assets/` — editing a shipped starter
/// saves a copy that shadows it, so the originals stay intact.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_user_script(file_name: &str, source: &str) -> Result<PathBuf, String> {
    let dir = user_script_dir().ok_or("cannot locate the application data folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let safe: String = file_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let path = dir.join(format!("{}.rhai", safe.trim_matches('_')));
    std::fs::write(&path, source).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

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
