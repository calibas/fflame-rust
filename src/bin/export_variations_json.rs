//! Dump the shipped variation corpus as JSON, for the API's bulk import.
//!
//! The Rust half of the hybrid exporter specified in
//! `docs/projects/VARIATIONS_BULK_METADATA_IMPORT.md` §4.1. This walks
//! the loaded registry and emits every variation's **structural** data —
//! the things that live in code and would be miserable to re-parse from
//! outside (raw-string WGSL bodies, macro-expanded parameter tables,
//! feature slices).
//!
//! It also emits the variation-level `description`, `description_plain`
//! and `authors`. Those live in `///` doc comments, invisible at
//! runtime, so they are recovered from the source by
//! [`fractal_flame_wgpu::variations::docs`].
//!
//! §4.1 planned a separate Python pass for that merge. It was never
//! written, so every corpus this binary produced carried 647 null
//! descriptions — and since prose reaches the app ONLY through the API
//! catalog, those nulls were exactly why variation descriptions were
//! missing downstream. One command that emits a complete corpus is
//! worth more than the language split. Per-parameter descriptions come
//! straight from the struct fields, as before.
//!
//! Vocabularies come from `to_api_str`, the same source as
//! `docs/generated/engine-contract.json`, so the dump cannot disagree
//! with the contract about what a category or feature is called.
//!
//! ```text
//! cargo run --release --bin export_variations_json -- out.json
//! ```
//!
//! Defaults to `output/variations-corpus.json` (gitignored) — the file
//! is multi-megabyte because it carries every WGSL body, so it is
//! generated on demand rather than committed.

use fractal_flame_wgpu::variations::docs::{parse_sources, VariationDoc};
use fractal_flame_wgpu::variations::{global_registry, ParamType};

/// Read the prose out of `defs/*.rs`.
///
/// `CARGO_MANIFEST_DIR` is baked in at compile time and points at the
/// tree that built this binary, which is the right anchor for a
/// repo-local tool: the corpus is regenerated from a checkout, never
/// from an installed copy.
fn variation_docs() -> std::collections::BTreeMap<String, VariationDoc> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/variations/defs");
    let sources: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            eprintln!("Error: cannot read {}: {e}", dir.display());
            std::process::exit(1);
        })
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect();
    parse_sources(sources.iter().map(String::as_str))
}

fn param_type_to_wire(t: &ParamType) -> serde_json::Value {
    match t {
        ParamType::Float => serde_json::json!("float"),
        ParamType::UnlimitedFloat => serde_json::json!("unlimited_float"),
        ParamType::Integer => serde_json::json!("integer"),
        ParamType::UnlimitedInteger => serde_json::json!("unlimited_integer"),
        ParamType::Boolean => serde_json::json!("boolean"),
        ParamType::Angle => serde_json::json!("angle"),
        // Externally tagged, matching ApiParamType's serde shape.
        ParamType::Enum { choices } => serde_json::json!({ "enum": { "choices": choices } }),
    }
}

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "output/variations-corpus.json".to_string());

    let registry = global_registry();
    let mut rows = Vec::new();
    let docs = variation_docs();
    let mut skipped_non_core = 0usize;
    let mut missing_description = Vec::new();
    let mut with_authors = 0usize;

    for name in registry.names() {
        let Some(info) = registry.get(name) else { continue };
        if !info.provenance.is_builtin() {
            // A cached API download that happens to be registered in
            // this session is not part of the shipped corpus.
            skipped_non_core += 1;
            continue;
        }

        let parameters: Vec<serde_json::Value> = info
            .parameters
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "display_name": p.display_name,
                    "param_type": param_type_to_wire(&p.param_type),
                    "default_value": p.default_value,
                    "min_value": p.min_value,
                    "max_value": p.max_value,
                    "description": p.description,
                })
            })
            .collect();

        // Flag-like features as wire strings; the payload-carrying one
        // gets its own field, matching the wire format.
        let mut features: Vec<&'static str> = info
            .features
            .iter()
            .filter(|f| !matches!(f, fractal_flame_wgpu::variations::Feature::PlotEmits(_)))
            .map(|f| f.clone().to_api_str())
            .collect();
        features.sort();

        let doc = docs.get(&info.name);
        match doc {
            Some(d) if !d.description.trim().is_empty() => {
                if !d.authors.is_empty() {
                    with_authors += 1;
                }
            }
            _ => missing_description.push(info.name.clone()),
        }

        rows.push(serde_json::json!({
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
            // A variation with no prose is a hard error below, not a
            // null that travels quietly all the way to the API.
            "description": doc.map(|d| d.description.clone()),
            "description_plain": doc.map(|d| d.description_plain.clone()),
            "authors": doc.map(|d| d.authors.clone()).unwrap_or_default(),
        }));
    }

    // Stable order, so a re-export diffs cleanly against the last one.
    rows.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

    let doc = serde_json::json!({
        "$comment": "GENERATED by `cargo run --bin export_variations_json`. \
                     Complete: structural data from the registry, plus \
                     `description` / `description_plain` / `authors` read \
                     from the `///` doc comments in src/variations/defs. \
                     `description` is markdown, `description_plain` has \
                     the syntax stripped. \
                     Vocabularies match docs/generated/engine-contract.json \
                     by construction — both read to_api_str.",
        "contract_shape": fractal_flame_wgpu::contract::generate()["shape"],
        "count": rows.len(),
        "variations": rows,
    });

    // Refuse to write a corpus with holes in it. The API has no other
    // source for this prose, so a silent null here is a variation that
    // reaches the browser with no description at all.
    if !missing_description.is_empty() {
        eprintln!(
            "Error: {} shipped variation(s) have no `///` description:",
            missing_description.len()
        );
        for name in missing_description.iter().take(20) {
            eprintln!("  {name}");
        }
        if missing_description.len() > 20 {
            eprintln!("  ...and {} more", missing_description.len() - 20);
        }
        eprintln!("Add a doc comment above each `pub static` and re-run.");
        std::process::exit(1);
    }

    let json = serde_json::to_string_pretty(&doc).expect("serialize corpus");
    if let Some(dir) = std::path::Path::new(&out_path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&out_path, &json).unwrap_or_else(|e| {
        eprintln!("Error: cannot write `{out_path}`: {e}");
        std::process::exit(1);
    });

    eprintln!(
        "Wrote {out_path} — {} variations ({with_authors} with authors), {:.1} MB{}",
        doc["count"],
        json.len() as f64 / 1e6,
        if skipped_non_core > 0 {
            format!(" ({skipped_non_core} non-core skipped)")
        } else {
            String::new()
        }
    );
}
