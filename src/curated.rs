//! Curated content, the client's half (docs/projects/curated-content.md):
//! the content hash, defined as the API defines it, and the manifest of
//! everything the app ships, which the seeding sync imports.
//!
//! **The hash** must agree with ffapi's `src/content_hash.rs`, byte for
//! byte -- the import refuses a record whose claimed hash is not the
//! server's (`hash_mismatch`), and the client trusts curated code by it.
//! Each is defined on the wire form, what the app sends or is served:
//!
//! - a script: `scripted::version_of` (SHA-256, line endings folded);
//! - a palette: `api::sync::palette_content_hash`;
//! - a fractal: the SHA-256 of the canonical JSON `{"kind": "fractal",
//!   "v": 1, "config", "palette", "transforms"}` -- the request's config
//!   blob, the palette's content hash, each root transform's `kind` and
//!   `data` in pool order;
//! - a built-in variation or effect: the canonical JSON of the corpus
//!   record's content fields ([`VARIATION_FIELDS`], [`EFFECT_FIELDS`]),
//!   absence read as zero.
//!
//! Canonical JSON is a `serde_json::Value` written compactly: its maps are
//! `BTreeMap`s, so keys come out sorted (neither crate enables
//! `preserve_order`).
//!
//! **The manifest** ([`manifest`]) is what `export_curated_json` writes:
//! one record an item, `{kind, name, display_name, description, credit,
//! content_hash, content}`, the shape `POST /admin/curated/import` takes.

// The manifest is a desktop tool's (it reads the checkout); the web build
// keeps only the hashes.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The canonical documents' format; a change is a new version (the
/// server's `content_hash::VERSION`).
pub const VERSION: u32 = 1;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 of a canonical JSON document, lowercase hex.
pub fn of_document(doc: &Value) -> String {
    hex(&Sha256::digest(doc.to_string().as_bytes()))
}

/// A script's: its version (`scripted::version_of`).
pub fn of_script(source: &str) -> String {
    crate::scripted::version_of(source)
}

fn pool_rank(kind: &str) -> u8 {
    match kind {
        "linked" => 1,
        "final" => 2,
        _ => 0,
    }
}

/// A fractal's, from its create request (`api::sync::config_to_create_request`)
/// and its palette's content hash.
pub fn of_fractal(config: &Value, palette_hash: Option<&str>, transforms: &[crate::api::types::ApiTransformWire]) -> String {
    let mut ordered: Vec<_> = transforms.iter().collect();
    ordered.sort_by_key(|t| (pool_rank(&t.kind), t.sort_order));
    let transforms: Vec<Value> = ordered.iter().map(|t| json!({ "kind": t.kind, "data": t.data })).collect();
    of_document(&json!({
        "kind": "fractal",
        "v": VERSION,
        "config": config,
        "palette": palette_hash,
        "transforms": transforms,
    }))
}

/// What of a variation is content: the fields that change what it does,
/// as the corpus spells them. Prose is not; aliases are.
pub const VARIATION_FIELDS: &[&str] = &[
    "aliases",
    "category",
    "features",
    "init_param_count",
    "parameters",
    "phase",
    "plot_emits",
    "shader_2d",
    "shader_3d",
    "shader_init",
    "shader_state_init",
    "state_count",
];

/// What of an effect is content.
pub const EFFECT_FIELDS: &[&str] = &["category", "multi_pass", "parameters", "requires_blend_modes", "shader", "temporal"];

/// One field as the hash reads it: absence and its zero are one value.
fn normalized(field: &str, v: Option<&Value>) -> Value {
    match v {
        Some(Value::Null) | None => match field {
            "aliases" | "features" | "parameters" => json!([]),
            "init_param_count" | "plot_emits" | "state_count" => json!(0),
            "multi_pass" | "requires_blend_modes" | "temporal" => json!(false),
            _ => json!(""),
        },
        Some(v) => v.clone(),
    }
}

fn of_fields(kind: &str, fields: &[&str], record: &Value) -> String {
    let mut doc = serde_json::Map::new();
    doc.insert("kind".into(), json!(kind));
    doc.insert("v".into(), json!(VERSION));
    for f in fields {
        doc.insert((*f).into(), normalized(f, record.get(*f)));
    }
    of_document(&Value::Object(doc))
}

/// A built-in variation's, over its corpus record.
pub fn of_variation(record: &Value) -> String {
    of_fields("variation", VARIATION_FIELDS, record)
}

/// A built-in effect's, over its corpus record.
pub fn of_effect(record: &Value) -> String {
    of_fields("effect", EFFECT_FIELDS, record)
}

// ---------------------------------------------------------------- corpora

/// The shipped variation corpus (`export_variations_json`), and what it
/// found missing.
pub struct VariationCorpus {
    pub rows: Vec<Value>,
    /// Variations with no `///` description: the export refuses them.
    pub missing_description: Vec<String>,
    pub with_authors: usize,
    /// Registered but not shipped (a cached download).
    pub skipped_non_core: usize,
}

/// Read the prose out of `defs/*.rs`, from the tree that built this
/// binary (a repo-local tool: the corpus is regenerated from a checkout).
#[cfg(not(target_arch = "wasm32"))]
fn variation_docs() -> BTreeMap<String, crate::variations::docs::VariationDoc> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/variations/defs");
    let sources: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
                .filter_map(|p| std::fs::read_to_string(p).ok())
                .collect()
        })
        .unwrap_or_default();
    crate::variations::docs::parse_sources(sources.iter().map(String::as_str))
}

/// Every shipped variation as the API's corpus record, by name.
#[cfg(not(target_arch = "wasm32"))]
pub fn variation_corpus() -> VariationCorpus {
    let registry = crate::variations::global_registry();
    let docs = variation_docs();
    let mut out = VariationCorpus { rows: Vec::new(), missing_description: Vec::new(), with_authors: 0, skipped_non_core: 0 };
    for name in registry.names() {
        let Some(info) = registry.get(name) else { continue };
        if !info.provenance.is_builtin() {
            out.skipped_non_core += 1;
            continue;
        }
        let parameters: Vec<Value> = info.parameters.iter().map(crate::params::api_json).collect();
        // Flag-like features as wire strings; the payload-carrying one
        // gets its own field, matching the wire format.
        let mut features: Vec<&'static str> = info
            .features
            .iter()
            .filter(|f| !matches!(f, crate::variations::Feature::PlotEmits(_)))
            .map(|f| f.clone().to_api_str())
            .collect();
        features.sort();
        let doc = docs.get(&info.name);
        match doc {
            Some(d) if !d.description.trim().is_empty() => {
                if !d.authors.is_empty() {
                    out.with_authors += 1;
                }
            }
            _ => out.missing_description.push(info.name.clone()),
        }
        out.rows.push(json!({
            "name": info.name,
            "display_name": info.display_name,
            "category": info.category.to_api_str(),
            "phase": info.phase.clone().to_api_str(),
            "version": 1,

            "features": features,
            "plot_emits": info.plot_emit_cap(),

            "parameters": parameters,
            "init_param_count": info.init_param_count,
            "state_count": info.state_count,

            "shader_2d": info.wgsl_source,
            "shader_3d": info.wgsl_source_3d,
            "shader_init": info.wgsl_source_init,
            "shader_state_init": info.wgsl_source_state_init,

            "aliases": registry.aliases_for(&info.name),

            // Recovered from the `///` doc comments above each def.
            "description": doc.map(|d| d.description.clone()),
            "description_plain": doc.map(|d| d.description_plain.clone()),
            "authors": doc.map(|d| d.authors.clone()).unwrap_or_default(),
        }));
    }
    // Stable order, so a re-export diffs cleanly against the last one.
    out.rows.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    out
}

/// Every shipped effect as the API's corpus record, by name, and those
/// over the 48-parameter uniform capacity.
pub fn effect_corpus() -> (Vec<Value>, Vec<String>) {
    use crate::effects::{global_effect_registry, EffectCategory};
    let registry = global_effect_registry();
    let mut rows = Vec::new();
    let mut over_param_cap = Vec::new();
    let mut all: Vec<_> = registry.all().collect();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    for info in all {
        // Raw source, includes NOT spliced: the `// INCLUDE_BLEND_MODES`
        // marker is part of what the server stores; the client splices at
        // compile time. `wgsl()` prefers the working copy under `shaders/`.
        let shader = info.source.wgsl();
        let requires_blend_modes = shader.contains("// INCLUDE_BLEND_MODES");
        let display_name = info.translated_name();
        if info.parameters.len() > 48 {
            over_param_cap.push(format!("{} ({})", info.name, info.parameters.len()));
        }
        let parameters: Vec<Value> = info.parameters.iter().map(crate::params::api_json).collect();
        rows.push(json!({
            "name": info.name,
            "display_name": display_name,
            "category": match info.category {
                EffectCategory::Density => "density",
                EffectCategory::Color => "color",
            },
            "version": 1,
            "parameters": parameters,
            "shader": shader,
            "requires_blend_modes": requires_blend_modes,
            // A multi-pass effect's module has an entry point a pass and no
            // `fs_main` (docs/archive/projects/multipass-effects.md); temporal
            // keeps a history.
            "multi_pass": info.passes.is_some(),
            "temporal": info.passes.as_ref().is_some_and(|p| p.temporal()),
            "description": Value::Null,
            "description_plain": Value::Null,
            "authors": Value::Array(vec![]),
        }));
    }
    (rows, over_param_cap)
}

// ---------------------------------------------------------------- manifest

/// A canonical name from an item's own: ASCII letters, digits, `_` and
/// `-` (`_` alone for the kinds whose names are identifiers), lower case,
/// at most 64, never empty -- the API's `curated_names` CHECK.
pub fn slug(name: &str, identifier: bool) -> String {
    let sep = if identifier { '_' } else { '-' };
    let mut out = String::new();
    for c in name.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' || (c == '-' && !identifier) {
            out.push(c);
        } else if !out.ends_with(sep) && !out.is_empty() {
            out.push(sep);
        }
    }
    let out = out.trim_matches(|c| c == '-' || c == '_').to_string();
    let mut out: String = out.chars().take(64).collect();
    while out.ends_with('-') || out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() { "item".into() } else { out }
}

/// The kinds whose canonical names are engine identifiers: no `-`.
fn identifier_kind(kind: &str) -> bool {
    matches!(kind, "variation" | "effect")
}

/// Everything the app ships, as import records, and what is wrong with it.
pub struct Manifest {
    pub records: Vec<Value>,
    /// Problems worth reading: renamed duplicates, items left out.
    pub notes: Vec<String>,
}

/// Names taken in a kind's namespace, compared without case: a duplicate
/// gets `-2`, `-3`, ... (`_2` in an identifier kind) and a note.
#[derive(Default)]
struct Names(HashMap<String, std::collections::HashSet<String>>);

impl Names {
    fn claim(&mut self, kind: &str, wanted: &str, notes: &mut Vec<String>) -> String {
        let taken = self.0.entry(kind.to_string()).or_default();
        let sep = if identifier_kind(kind) { '_' } else { '-' };
        let mut name = wanted.to_string();
        let mut n = 2;
        while taken.contains(&name.to_lowercase()) {
            let suffix = format!("{sep}{n}");
            let stem: String = wanted.chars().take(64 - suffix.len()).collect();
            name = format!("{stem}{suffix}");
            n += 1;
        }
        if name != wanted {
            notes.push(format!("{kind}/{wanted}: name taken in the manifest, written as {name}"));
        }
        taken.insert(name.to_lowercase());
        name
    }
}

fn record(kind: &str, name: String, display_name: &str, description: Option<String>, credit: Option<String>, hash: String, content: Value) -> Value {
    json!({
        "kind": kind,
        "name": name,
        "display_name": display_name,
        "description": description,
        "credit": credit,
        "content_hash": hash,
        "content": content,
    })
}

/// The manifest: the presets (fractals of every engine), the palette
/// packs, the shipped scripts and Rhai examples, and the built-in
/// variations and effects. The built-ins' Rhai twins are left out: their
/// names are the built-ins' own, which the API's shared namespaces give to
/// the WGSL catalog.
#[cfg(not(target_arch = "wasm32"))]
pub fn manifest() -> Manifest {
    let mut records = Vec::new();
    let mut notes = Vec::new();
    let mut names = Names::default();

    // Fractals: every preset, as the app would save it online.
    match crate::resources::presets::load_embedded_presets() {
        Ok(presets) => {
            for config in presets {
                let display = config.flame.name.clone();
                let request = match crate::api::sync::config_to_create_request(&config, None) {
                    Ok(r) => r,
                    Err(e) => {
                        notes.push(format!("fractal/{display}: left out, not serialisable: {e}"));
                        continue;
                    }
                };
                let palette_hash = crate::api::sync::palette_content_hash(&config.palette);
                let hash = of_fractal(&request.config, Some(&palette_hash), &request.transforms);
                let name = names.claim("fractal", &slug(&display, false), &mut notes);
                let content = json!({
                    "config": request.config,
                    "transforms": request.transforms,
                    "palette": request.palette,
                });
                records.push(record("fractal", name, &display, None, None, hash, content));
            }
        }
        Err(e) => notes.push(format!("fractal: the presets did not load: {e}")),
    }

    // Palettes: every pack in the folder (the folder is the catalog).
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/palettes/packs");
    let mut packs: Vec<_> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    packs.sort();
    for path in packs.iter().filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json")) {
        let pack: crate::scene::palette::PalettePack =
            match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string())) {
                Ok(p) => p,
                Err(e) => {
                    notes.push(format!("palette: {} did not load: {e}", path.display()));
                    continue;
                }
            };
        for palette in &pack.palettes {
            let (stops, color_data) = crate::api::sync::encode_palette_data(palette);
            let hash = crate::api::sync::palette_content_hash(palette);
            let name = names.claim("palette", &slug(&palette.name, false), &mut notes);
            let description = Some(format!("From the {} pack.", pack.pack_name));
            records.push(record("palette", name, &palette.name, description, None, hash, json!({ "color_data": color_data, "stops": stops })));
        }
    }

    // The built-in variations and effects: their corpus records.
    let corpus = variation_corpus();
    for row in corpus.rows {
        let name = row["name"].as_str().unwrap_or_default().to_string();
        names.claim("variation", &name, &mut notes);
        let display = row["display_name"].as_str().unwrap_or(&name).to_string();
        let hash = of_variation(&row);
        records.push(record("variation", name, &display, None, None, hash, row));
    }
    let (effects, _) = effect_corpus();
    for row in effects {
        let name = row["name"].as_str().unwrap_or_default().to_string();
        names.claim("effect", &name, &mut notes);
        let display = row["display_name"].as_str().unwrap_or(&name).to_string();
        let hash = of_effect(&row);
        records.push(record("effect", name, &display, None, None, hash, row));
    }

    // Scripts: the shipped CPU scripts and the Rhai examples.
    let shipped: Vec<&str> = crate::script::library::EMBEDDED
        .iter()
        .map(|(f, _)| *f)
        .chain(crate::storage::formula_store::EXAMPLES.iter().map(|(f, _)| *f))
        .collect();
    for entry in crate::storage::library::scan() {
        if entry.origin != crate::provenance::Origin::Builtin || !shipped.contains(&entry.file.as_str()) {
            continue;
        }
        let Some(kind) = entry.kind.wire() else {
            notes.push(format!("script {}: left out, its kind is not published", entry.file));
            continue;
        };
        let stem = entry.file.trim_end_matches(".rhai");
        let name = names.claim(kind, &slug(stem, identifier_kind(kind)), &mut notes);
        let credit = (!entry.author.trim().is_empty()).then(|| entry.author.clone());
        let description = (!entry.description.trim().is_empty()).then(|| entry.description.clone());
        let hash = of_script(&entry.source);
        records.push(record(kind, name, &entry.title, description, credit, hash, json!({ "source": entry.source })));
    }

    Manifest { records, notes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_canonical_names() {
        assert_eq!(slug("Sierpinski Carpet", false), "sierpinski-carpet");
        assert_eq!(slug("  Fire & Ice!! ", false), "fire-ice");
        assert_eq!(slug("variation-whorl", true), "variation_whorl");
        assert_eq!(slug("Über", false), "ber");
        assert_eq!(slug("***", false), "item");
        assert_eq!(slug(&"a".repeat(80), false).len(), 64);
    }

    /// The vectors ffapi's `content_hash` tests pin: the same inputs must
    /// give the same hashes here.
    #[test]
    fn hashes_agree_with_the_api_definition() {
        assert_eq!(of_script(""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        let corpus = json!({"name": "v", "display_name": "V", "plot_emits": 0, "shader_init": "", "shader_2d": "fn a() {}",
                            "parameters": [{"name": "p", "default": 0.5}], "features": [], "aliases": []});
        let row = json!({"plot_emits": null, "shader_init": null, "shader_2d": "fn a() {}",
                         "parameters": [{"default": 0.5, "name": "p"}], "features": null, "aliases": null});
        assert_eq!(of_variation(&corpus), of_variation(&row));
        assert_ne!(of_variation(&corpus), of_effect(&corpus));
    }

    #[test]
    fn the_manifest_names_every_item_once() {
        let m = manifest();
        let mut seen = std::collections::HashSet::new();
        for r in &m.records {
            let key = format!("{}/{}", r["kind"].as_str().unwrap(), r["name"].as_str().unwrap().to_lowercase());
            assert!(seen.insert(key.clone()), "{key} twice");
            let name = r["name"].as_str().unwrap();
            assert!(!name.is_empty() && name.len() <= 64, "{name}");
            assert_eq!(r["content_hash"].as_str().unwrap().len(), 64);
        }
        let count = |k: &str| m.records.iter().filter(|r| r["kind"] == k).count();
        assert!(count("fractal") >= 80, "the presets: {}", count("fractal"));
        assert!(count("palette") >= 700, "the packs: {}", count("palette"));
        assert!(count("variation") >= 640);
        assert!(count("generator") > 0 && count("formula") > 0);
    }
}
