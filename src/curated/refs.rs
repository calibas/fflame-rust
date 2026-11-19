//! Curated items a config names (docs/projects/curated-content.md,
//! *Names in configs*; D9, D10, D11).
//!
//! A config carries the Rhai it uses as sources (`CustomSource`:
//! `{dialect, entry, source}`, `entry` the registry key). When a source is
//! a curated item's, a `.fflame` save records it in the config's
//! `curated` table -- `{kind, name, hash, entry}`, the canonical name
//! pinned by the content hash -- and leaves its source out (D10: unless
//! the save is for offline use). On load, a referenced source is fetched
//! by its name and hash (`POST /curated/resolve`, then the item) and put
//! back before anything compiles it (`app::trust`). Curated versions are
//! static, so the name and hash give back exactly the source that was
//! saved.
//!
//! **What the app knows is curated** ([`remember`]) comes from the trust
//! lookups, the curated scripts adopted from the Catalog, and the
//! references resolved on load; it is kept on this device, so a save needs
//! no network. A source the app does not know as curated -- the user's
//! own, an edited copy of a curated one (D1: editing makes a copy) -- is
//! carried as before. Conversion happens on save only (D11): loading never
//! changes what a file means.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::FractalConfig;

/// One curated item a config uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratedRef {
    /// Its kind as the server names it (`formula`, `variation`, ...).
    pub kind: String,
    /// Its canonical name.
    pub name: String,
    /// Its content hash: the exact version (`scripted::version_of`).
    pub hash: String,
    /// The registry key the config's carried source has (`u<id>_<hash8>`):
    /// where the fetched source goes back.
    pub entry: String,
}

/// What this device knows is curated: hash -> (kind, name).
static KNOWN: LazyLock<Mutex<Option<HashMap<String, (String, String)>>>> = LazyLock::new(Default::default);

const KNOWN_FILE: &str = "curated_known.json";

fn known_map() -> std::sync::MutexGuard<'static, Option<HashMap<String, (String, String)>>> {
    let mut guard = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        // Tests keep it in memory: the app's data folder is the user's.
        #[cfg(not(test))]
        let read = crate::storage::backend::read_file(std::path::Path::new(KNOWN_FILE))
            .ok()
            .and_then(|s| serde_json::from_str::<HashMap<String, (String, String)>>(&s).ok())
            .unwrap_or_default();
        #[cfg(test)]
        let read = HashMap::new();
        *guard = Some(read);
    }
    guard
}

/// Remember that the content with `hash` is the curated `kind/name`.
pub fn remember(kind: &str, name: &str, hash: &str) {
    let mut guard = known_map();
    let map = guard.get_or_insert_with(HashMap::new);
    if map.get(hash).is_some_and(|(k, n)| k == kind && n == name) {
        return;
    }
    map.insert(hash.to_string(), (kind.to_string(), name.to_string()));
    if cfg!(test) {
        return;
    }
    if let Ok(json) = serde_json::to_string(map) {
        if let Err(e) = crate::storage::backend::write_file(std::path::Path::new(KNOWN_FILE), &json) {
            log::warn!("curated names not saved: {e}");
        }
    }
}

/// The curated `(kind, name)` of the content with `hash`, if known.
pub fn known(hash: &str) -> Option<(String, String)> {
    known_map().as_ref()?.get(hash).cloned()
}

/// Every carried Rhai source in a config's JSON, mutably.
fn for_each_source(v: &mut Value, f: &mut impl FnMut(&mut serde_json::Map<String, Value>)) {
    match v {
        Value::Object(o) => {
            if o.get("dialect").and_then(Value::as_str) == Some("rhai") && o.get("entry").is_some() && o.get("source").is_some() {
                f(o);
            }
            for child in o.values_mut() {
                for_each_source(child, f);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|c| for_each_source(c, f)),
        _ => {}
    }
}

/// A config as a `.fflame` save writes it: each carried source this
/// device knows is curated recorded in `curated`, and -- unless
/// `include_sources` (a save for offline use, D10) -- its source left out.
pub fn for_saving(config: &FractalConfig, include_sources: bool) -> Result<Value, serde_json::Error> {
    let mut value = config.to_json_value()?;
    let mut refs: Vec<CuratedRef> = Vec::new();
    // A referenced source already left out (a file loaded unresolved and
    // saved again) keeps its reference.
    let carried: HashMap<&str, &CuratedRef> = config.curated.iter().map(|r| (r.entry.as_str(), r)).collect();
    for_each_source(&mut value, &mut |o| {
        let entry = o.get("entry").and_then(Value::as_str).unwrap_or_default().to_string();
        let source = o.get("source").and_then(Value::as_str).unwrap_or_default().to_string();
        let found = if source.is_empty() {
            carried.get(entry.as_str()).map(|r| (*r).clone())
        } else {
            let hash = crate::scripted::version_of(&source);
            known(&hash).map(|(kind, name)| CuratedRef { kind, name, hash, entry: entry.clone() })
        };
        if let Some(r) = found {
            if !include_sources && !source.is_empty() {
                o.insert("source".into(), Value::String(String::new()));
            }
            if !refs.iter().any(|x| x.entry == r.entry && x.hash == r.hash) {
                refs.push(r);
            }
        }
    });
    if let Some(obj) = value.as_object_mut() {
        if refs.is_empty() {
            obj.remove("curated");
        } else {
            obj.insert("curated".into(), serde_json::to_value(&refs)?);
        }
    }
    Ok(value)
}

/// The references whose sources a config does not carry: what a load
/// fetches first.
pub fn unresolved(config: &FractalConfig) -> Vec<CuratedRef> {
    if config.curated.is_empty() {
        return Vec::new();
    }
    let Ok(mut value) = config.to_json_value() else { return Vec::new() };
    let mut empty = std::collections::HashSet::new();
    for_each_source(&mut value, &mut |o| {
        if o.get("source").and_then(Value::as_str).is_some_and(str::is_empty) {
            if let Some(e) = o.get("entry").and_then(Value::as_str) {
                empty.insert(e.to_string());
            }
        }
    });
    config.curated.iter().filter(|r| empty.contains(&r.entry)).cloned().collect()
}

/// A config with the fetched sources put back (`entry` -> source): the
/// config as it was saved with its sources.
pub fn fill(config: &FractalConfig, sources: &HashMap<String, String>) -> Result<FractalConfig, serde_json::Error> {
    let mut value = config.to_json_value()?;
    for_each_source(&mut value, &mut |o| {
        let empty = o.get("source").and_then(Value::as_str).is_some_and(str::is_empty);
        let entry = o.get("entry").and_then(Value::as_str).map(str::to_string);
        if let (true, Some(source)) = (empty, entry.and_then(|e| sources.get(&e))) {
            o.insert("source".into(), Value::String(source.clone()));
        }
    });
    FractalConfig::from_json_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::escape::{CustomDialect, CustomSource};

    fn with_formula(source: &str) -> FractalConfig {
        let mut config = FractalConfig::default();
        config.escape.custom_formula =
            Some(CustomSource { dialect: CustomDialect::Rhai, entry: "uabc_12345678".into(), source: source.into(), variations: Vec::new() });
        config
    }

    #[test]
    fn a_curated_source_is_saved_as_a_reference_and_fetched_back() {
        let source = format!("// curated test {}\nfn step(s, p, ctx) {{ s }}\n", crate::scripted::mint_id());
        let hash = crate::scripted::version_of(&source);
        let config = with_formula(&source);

        // Not known as curated: carried, no table.
        let saved = for_saving(&config, false).unwrap();
        assert_eq!(saved["escape"]["custom_formula"]["source"], source.as_str());
        assert!(saved.get("curated").is_none());

        remember("formula", "test-spiral", &hash);
        let saved = for_saving(&config, false).unwrap();
        assert_eq!(saved["escape"]["custom_formula"]["source"], "", "left out");
        assert_eq!(saved["curated"][0]["name"], "test-spiral");
        assert_eq!(saved["curated"][0]["hash"], hash.as_str());
        let offline = for_saving(&config, true).unwrap();
        assert_eq!(offline["escape"]["custom_formula"]["source"], source.as_str(), "kept for offline use");
        assert_eq!(offline["curated"][0]["entry"], "uabc_12345678");

        // Loaded: the reference is unresolved until filled; filled, it is
        // the config as saved with its source.
        let loaded = FractalConfig::from_json_value(saved.clone()).unwrap();
        let missing = unresolved(&loaded);
        assert_eq!(missing.len(), 1);
        assert!(unresolved(&FractalConfig::from_json_value(offline).unwrap()).is_empty());
        let filled = fill(&loaded, &HashMap::from([(missing[0].entry.clone(), source.clone())])).unwrap();
        assert!(unresolved(&filled).is_empty());
        assert_eq!(filled.escape.custom_formula.as_ref().unwrap().source, source);

        // Saved again unresolved, it keeps its reference.
        let again = for_saving(&loaded, false).unwrap();
        assert_eq!(again["curated"][0]["name"], "test-spiral");
    }
}
