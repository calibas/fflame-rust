//! Every config the repository holds, written in its saved form: the safety
//! net for changes to how a config is written (`FractalConfig::to_json_value`).
//! Dump before the change, dump after, compare the folders byte for byte.
//!
//! ```bash
//! CONFIG_DUMP=output/config_dump/before cargo test --release --lib -- --ignored dump_saved_configs
//! ```
//!
//! The configs: the bundled presets, every `tests/visual/configs/**/*.fflame`,
//! the texture recipes, and the catalog's examples.

use crate::config::FractalConfig;

/// Every config, by a stable name.
pub fn corpus() -> Vec<(String, serde_json::Value)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    let presets: Vec<serde_json::Value> = serde_json::from_str(crate::resources::presets::PRESETS_JSON).unwrap_or_default();
    for (i, p) in presets.into_iter().enumerate() {
        out.push((format!("preset-{i:03}"), p));
    }
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, serde_json::Value)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, root, out);
            } else if p.extension().is_some_and(|e| e == "fflame") {
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let name = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace(['/', '\\'], "__");
                match v {
                    serde_json::Value::Array(items) => {
                        for (i, item) in items.into_iter().enumerate() {
                            out.push((format!("{name}-{i}"), item));
                        }
                    }
                    v => out.push((name, v)),
                }
            }
        }
    }
    walk(&root.join("tests/visual/configs"), root, &mut out);
    walk(&root.join("assets/textures"), root, &mut out);
    let catalog: serde_json::Value = serde_json::from_str(include_str!("../../docs/generated/engine-catalog.json")).unwrap_or_default();
    for (i, ex) in catalog["examples"].as_array().into_iter().flatten().enumerate() {
        out.push((format!("catalog-{i:03}"), ex["config"].clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write every config's saved form to `$CONFIG_DUMP/<name>.json`.
    #[test]
    #[ignore = "a tool: CONFIG_DUMP=<dir>"]
    fn dump_saved_configs() {
        let Ok(dir) = std::env::var("CONFIG_DUMP") else { return };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut written = 0;
        for (name, v) in corpus() {
            let text = match FractalConfig::from_json_value(v) {
                Ok(c) => serde_json::to_string_pretty(&c.to_json_value().unwrap()).unwrap(),
                Err(e) => format!("does not load: {e}"),
            };
            std::fs::write(dir.join(format!("{name}.json")), text).unwrap();
            written += 1;
        }
        eprintln!("wrote {written} configs to {}", dir.display());
    }

    /// Write the bundled presets back through the canonical writer (current
    /// version, `min_reader`, no dead keys), the way a save would. Loading
    /// already migrates them, so each preset reads the same after; check
    /// with `dump_saved_configs` before and after.
    ///
    /// `RESAVE_PRESETS=1 cargo test --release --lib -- --ignored resave_presets`
    #[test]
    #[ignore = "a tool: RESAVE_PRESETS=1"]
    fn resave_presets() {
        if std::env::var("RESAVE_PRESETS").is_err() {
            return;
        }
        let presets: Vec<serde_json::Value> = serde_json::from_str(crate::resources::presets::PRESETS_JSON).unwrap();
        let configs: Vec<FractalConfig> = presets.into_iter().map(|p| FractalConfig::from_json_value(p).expect("a preset loads")).collect();
        let text = FractalConfig::to_json_array(&configs).unwrap() + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/presets.fflame");
        std::fs::write(path, text).unwrap();
        eprintln!("re-saved {} presets", configs.len());
    }

}
