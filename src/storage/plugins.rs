//! Local-only plugins: variations and effects the user installed
//! themselves.
//!
//! # The same object, from a different source
//!
//! A plugin file is **the same JSON as the download payload** —
//! `VariationDownload` / `EffectDownload`. That is the whole design:
//! one registration path, one compile path, one set of ceilings, rather
//! than a parallel system that would drift. It also makes submitting a
//! plugin for curation frictionless, since the file the user already
//! has is the file a curator would receive.
//!
//! # Where they live, and what must never touch them
//!
//! - Desktop: `<app data>/plugins/variations/`, `<app data>/plugins/effects/`
//! - WASM: localStorage under the same `plugins/…` key prefix
//!
//! Deliberately **not** the download cache. Clear Cache must never
//! destroy the user's own work, and the separation is what makes that
//! structural rather than a rule someone has to remember —
//! `variation_cache` and `effect_cache` enumerate their own prefixes and
//! cannot reach this one.
//!
//! # Collisions are refused, never shadowed
//!
//! §0 decision 3. A plugin whose name is taken by a built-in, or by a
//! variation already downloaded, fails to load with a message naming
//! the conflict. The registries enforce this themselves — this module
//! only reports what they refused, so there is one rule rather than two.

use std::path::{Path, PathBuf};

use crate::storage::backend;

/// Prefix for everything in this module. Distinct from the caches by
/// construction.
const PREFIX: &str = "plugins";

fn dir_for(kind: PluginKind) -> PathBuf {
    Path::new(PREFIX).join(kind.dir())
}

fn path_for(kind: PluginKind, name: &str) -> PathBuf {
    dir_for(kind).join(format!("{name}.json"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    Variation,
    Effect,
}

impl PluginKind {
    fn dir(self) -> &'static str {
        match self {
            Self::Variation => "variations",
            Self::Effect => "effects",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Variation => "variation",
            Self::Effect => "effect",
        }
    }
}

/// What happened when plugins were loaded.
///
/// Refusals are returned rather than only logged: a plugin the user
/// installed and that then does not appear is exactly the situation
/// where a console message is not a report.
#[derive(Debug, Default)]
pub struct PluginLoadReport {
    pub loaded: Vec<String>,
    /// `(name, why)` — the message names the conflict or the defect.
    pub refused: Vec<(String, String)>,
}

impl PluginLoadReport {
    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty() && self.refused.is_empty()
    }
}

/// Where plugins live, for a status line or a "open folder" action.
#[cfg(not(target_arch = "wasm32"))]
pub fn plugin_dir(kind: PluginKind) -> Option<PathBuf> {
    backend::get_app_data_dir().ok().map(|d| d.join(PREFIX).join(kind.dir()))
}

fn list_names(kind: PluginKind) -> Vec<String> {
    match backend::list_entries(&dir_for(kind)) {
        Ok(entries) => entries
            .into_iter()
            .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
            // `_`-prefixed entries are metadata everywhere else in
            // storage; keep the convention here so the two never
            // disagree about what is a resource.
            .filter(|n| !n.starts_with('_'))
            .collect(),
        Err(e) => {
            log::warn!("Cannot list {} plugins: {e}", kind.label());
            Vec::new()
        }
    }
}

/// Install a plugin from raw JSON.
///
/// Validated by the same code that validates a download, so a file that
/// would be refused at registration is refused at install — while the
/// user is looking at it, rather than silently missing later.
pub fn install(kind: PluginKind, json: &str) -> Result<String, String> {
    let name = match kind {
        PluginKind::Variation => {
            let dl: crate::api::types::VariationDownload = serde_json::from_str(json)
                .map_err(|e| format!("not a variation plugin: {e}"))?;
            // A fresh registry carries every built-in, so this probe
            // applies exactly the rules the real one will — including
            // the built-in name collision, caught while the user is
            // looking at the file rather than silently at next startup.
            let mut probe = crate::variations::VariationRegistry::new();
            probe.register_from_api(&dl, crate::provenance::Provenance::Local);
            if !probe.has(&dl.name) {
                return Err(format!(
                    "`{}` was refused — see the log for which rule it broke",
                    dl.name
                ));
            }
            dl.name
        }
        PluginKind::Effect => {
            let dl: crate::api::types::EffectDownload = serde_json::from_str(json)
                .map_err(|e| format!("not an effect plugin: {e}"))?;
            crate::effects::check_download(&dl)?;
            dl.name
        }
    };

    backend::write_file(&path_for(kind, &name), json)
        .map_err(|e| format!("cannot save plugin `{name}`: {e}"))?;
    Ok(name)
}

pub fn remove(kind: PluginKind, name: &str) -> Result<(), String> {
    backend::delete_file(&path_for(kind, name))
        .map_err(|e| format!("cannot remove plugin `{name}`: {e}"))
}

/// Register every installed plugin.
///
/// Called at startup **after** the built-ins and the download cache, so
/// a collision is detected against everything already present and the
/// plugin is the one refused. That ordering is deliberate: the user's
/// file is the newcomer, and refusing it with a message is recoverable
/// (rename it), whereas silently displacing a curated resource would
/// change what a shared flame renders.
pub fn load_all() -> PluginLoadReport {
    let mut report = PluginLoadReport::default();

    for name in list_names(PluginKind::Variation) {
        let path = path_for(PluginKind::Variation, &name);
        let Ok(json) = backend::read_file(&path) else {
            report.refused.push((name, "could not be read".into()));
            continue;
        };
        match serde_json::from_str::<crate::api::types::VariationDownload>(&json) {
            Ok(mut dl) => {
                // The file name is the identity, not the `name` inside:
                // otherwise two files could claim one name and which one
                // won would depend on directory order.
                dl.name = name.clone();
                let mut registry = crate::variations::global_registry_mut();
                if let Some(existing) = registry.get(&name) {
                    report.refused.push((
                        name,
                        format!(
                            "the name is already taken by a {} variation",
                            existing.provenance.label()
                        ),
                    ));
                    continue;
                }
                registry.register_from_api(&dl, crate::provenance::Provenance::Local);
                if registry.has(&name) {
                    report.loaded.push(name);
                } else {
                    // `register_from_api` logs which rule it broke; the
                    // report says only that it did, because the panel
                    // has no business restating shader diagnostics.
                    report.refused.push((name, "refused at registration".into()));
                }
            }
            Err(e) => report.refused.push((name, format!("not a variation plugin: {e}"))),
        }
    }

    for name in list_names(PluginKind::Effect) {
        let path = path_for(PluginKind::Effect, &name);
        let Ok(json) = backend::read_file(&path) else {
            report.refused.push((name, "could not be read".into()));
            continue;
        };
        match serde_json::from_str::<crate::api::types::EffectDownload>(&json) {
            Ok(mut dl) => {
                dl.name = name.clone();
                let mut registry = crate::effects::global_effect_registry_mut();
                if let Some(existing) = registry.get(&name) {
                    report.refused.push((
                        name,
                        format!(
                            "the name is already taken by a {} effect",
                            existing.provenance.label()
                        ),
                    ));
                    continue;
                }
                match registry.register_from_api(&dl, crate::provenance::Provenance::Local) {
                    Ok(()) => report.loaded.push(name),
                    Err(e) => report.refused.push((name, e)),
                }
            }
            Err(e) => report.refused.push((name, format!("not an effect plugin: {e}"))),
        }
    }

    if !report.loaded.is_empty() {
        log::info!("Loaded {} local plugin(s)", report.loaded.len());
    }
    for (name, why) in &report.refused {
        log::warn!("Plugin `{name}` was not loaded: {why}");
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plugins must not share a prefix with either download cache.
    /// Clear Cache enumerates those prefixes, so an overlap would make
    /// "clear the cache" delete the user's own files.
    #[test]
    fn plugins_live_outside_both_caches() {
        let var = dir_for(PluginKind::Variation);
        let eff = dir_for(PluginKind::Effect);
        assert_eq!(var, Path::new("plugins").join("variations"));
        assert_eq!(eff, Path::new("plugins").join("effects"));
        assert!(!var.starts_with("variations"), "would be inside the variation cache");
        assert!(!eff.starts_with("effects"), "would be inside the effect cache");
    }

    /// The install check is the registration check, not a second copy
    /// of it — a file that would be refused later is refused now.
    #[test]
    fn installing_applies_the_same_rules_as_registration() {
        let bad = r#"{"id":"x","name":"x","display_name":"x","category":"color",
                      "version":1,"parameters":[],"shader":null,
                      "requires_blend_modes":false,"downloadable":true}"#;
        let e = install(PluginKind::Effect, bad).expect_err("no shader");
        assert!(e.contains("no shader"), "{e}");

        let not_json = install(PluginKind::Effect, "{").expect_err("malformed");
        assert!(not_json.contains("not an effect plugin"), "{not_json}");
    }
}

#[cfg(test)]
mod rule_tests {
    use super::*;
    use crate::provenance::Provenance;

    fn variation_json(name: &str) -> String {
        format!(
            r#"{{"id":"{name}","name":"{name}","display_name":"{name}",
                "category":"advanced_2d","version":1,"phase":"normal",
                "needs_rng":false,"needs_transform":false,"writes_color":false,
                "parameters":[],
                "shader_2d":"fn variation_{name}(p: vec2<f32>) -> vec2<f32> {{ return p; }}",
                "shader_3d":null,"init_param_count":0,"shader_init":null,
                "features":[],"state_count":0,"shader_state_init":null,
                "aliases":[],"plot_emits":0,"authors":[],"description_plain":null}}"#
        )
    }

    /// A plugin may not take a built-in's name — §0 decision 3, and the
    /// direction that matters: shadowing `linear` would change what
    /// every shared flame renders.
    #[test]
    fn a_plugin_cannot_take_a_builtin_name() {
        // `new()` already carries every built-in.
        let mut reg = crate::variations::VariationRegistry::new();
        assert!(reg.has("linear"), "fixture assumption");

        reg.register_from_api(
            &serde_json::from_str(&variation_json("linear")).unwrap(),
            Provenance::Local,
        );
        assert!(
            reg.get("linear").unwrap().provenance.is_builtin(),
            "the built-in must survive a same-named plugin"
        );
    }

    /// ...and the reverse: a download may not displace a plugin. This is
    /// the worse direction — it replaces the user's own work with
    /// somebody else's, and they never asked for it.
    #[test]
    fn a_download_cannot_displace_a_local_plugin() {
        let mut reg = crate::variations::VariationRegistry::new();
        reg.register_from_api(
            &serde_json::from_str(&variation_json("mine")).unwrap(),
            Provenance::Local,
        );
        assert_eq!(reg.get("mine").unwrap().provenance, Provenance::Local);

        reg.register_from_api(
            &serde_json::from_str(&variation_json("mine")).unwrap(),
            Provenance::Api { version: 9 },
        );
        assert_eq!(
            reg.get("mine").unwrap().provenance,
            Provenance::Local,
            "the user's plugin must survive a same-named download"
        );
    }

    /// Clear Cache removes downloads and nothing else.
    ///
    /// The old filter was `!is_core`, which would have swept local
    /// plugins up with the cache the moment they existed — deleting the
    /// user's own files under a label that says "cache".
    #[test]
    fn clearing_the_cache_leaves_plugins_alone() {
        let mut reg = crate::variations::VariationRegistry::new();
        reg.register_from_api(
            &serde_json::from_str(&variation_json("mine")).unwrap(),
            Provenance::Local,
        );
        reg.register_from_api(
            &serde_json::from_str(&variation_json("theirs")).unwrap(),
            Provenance::Api { version: 1 },
        );

        reg.clear_api();
        assert!(reg.has("mine"), "a local plugin is not cache");
        assert!(!reg.has("theirs"), "a download is");
        assert!(reg.has("linear"), "and built-ins are untouched");
    }
}
