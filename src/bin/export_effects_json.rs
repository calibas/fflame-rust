//! Dump the built-in effect corpus as JSON, for the API's effects catalog.
//!
//! The effects counterpart to `export_variations_json`. Emits everything
//! the API needs to serve the 15 built-in effects as downloadable
//! resources: WGSL source, parameter schemas, display names, category,
//! and `requires_blend_modes`.
//!
//! Shape matches `api-shared-resources.md` §4.4 — the wire format that
//! mirrors `VariationDownload` one-for-one, per the wire-format doc's
//! "decide once, apply twice".
//!
//! ```text
//! cargo run --release --bin export_effects_json -- out.json
//! ```
//!
//! Defaults to `output/effects-corpus.json` (gitignored). Much smaller
//! than the variation corpus — 15 effects, not 646 — but generated on
//! demand for the same reason: it carries shader source, which belongs
//! in one place.
//!
//! # Two gaps this surfaces
//!
//! Both are places where effects have not caught up with variations,
//! and both are visible in the output as nulls rather than papered over.
//!
//! * **`EffectParameter::description` is never populated.** The field
//!   exists — added in `5410c8a3`, alongside `display_name` — but all 76
//!   shipped parameters leave it `None`, where `VariationParameter` has
//!   prose in 2836 of 2971 `param!` macros. So this is a data-entry job,
//!   not a schema change.
//!
//!   An earlier version of this comment claimed the field did not exist
//!   at all. The emitted null was right for the wrong reason, which is
//!   the kind of wrongness that survives review.
//! * **`EffectInfo::display_name` is empty for every built-in.** That is
//!   deliberate, not missing: the curated English labels live in
//!   `locales/en.yml` under `effects.<name>.name`, which is both where
//!   translations can exist and where a name like "Edge Glow" for
//!   `sobel_edges` is recorded. `translated_name()` reads the locale
//!   first and falls back to the declared label, so this exports the
//!   curated name for a built-in and the author's for anything else.

use fractal_flame_wgpu::effects::{global_effect_registry, EffectCategory};
use fractal_flame_wgpu::variations::ParamType;

rust_i18n::i18n!("locales");

fn param_type_to_wire(t: &ParamType) -> serde_json::Value {
    match t {
        ParamType::Float => serde_json::json!("float"),
        ParamType::UnlimitedFloat => serde_json::json!("unlimited_float"),
        ParamType::Integer => serde_json::json!("integer"),
        ParamType::UnlimitedInteger => serde_json::json!("unlimited_integer"),
        ParamType::Boolean => serde_json::json!("boolean"),
        ParamType::Angle => serde_json::json!("angle"),
        ParamType::Enum { choices } => serde_json::json!({ "enum": { "choices": choices } }),
    }
}

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "output/effects-corpus.json".to_string());

    let registry = global_effect_registry();
    let mut rows = Vec::new();
    let mut over_param_cap = Vec::new();

    let mut all: Vec<_> = registry.all().collect();
    all.sort_by(|a, b| a.name.cmp(&b.name));

    for info in all {
        // Raw source, includes NOT spliced. The `// INCLUDE_BLEND_MODES`
        // marker is part of what the server stores; the client splices
        // at compile time, so pre-splicing here would bake a copy of the
        // shared library into all 12 effects that use it.
        //
        // `wgsl()` prefers the working copy under `shaders/` and falls
        // back to the embedded one, so this exports what the app would
        // actually run — including an edit not yet rebuilt.
        let shader = info.source.wgsl();
        let requires_blend_modes = shader.contains("// INCLUDE_BLEND_MODES");

        // Locale first, falling back to whatever the effect declares —
        // one rule, shared with the panel, rather than a second copy of
        // the miss-detection that could drift from it.
        let display_name = info.translated_name();

        if info.parameters.len() > 48 {
            over_param_cap.push(format!("{} ({})", info.name, info.parameters.len()));
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
                    // Real field, simply not authored yet for any
                    // shipped effect — see the module docs.
                    "description": p.description,
                })
            })
            .collect();

        rows.push(serde_json::json!({
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
            // For the Python/doc-comment pass, as with variations.
            "description": serde_json::Value::Null,
            "description_plain": serde_json::Value::Null,
            "authors": serde_json::Value::Array(vec![]),
        }));
    }

    let doc = serde_json::json!({
        "$comment": "GENERATED by `cargo run --bin export_effects_json`. \
                     Shape follows api-shared-resources.md §4.3/§4.4. \
                     `description`, `description_plain` and `authors` are \
                     null, and every per-parameter `description` is null \
                     because none has been authored — the field exists. \
                     Shader source is RAW — the \
                     `// INCLUDE_BLEND_MODES` marker is intact and \
                     requires_blend_modes says whether it is present.",
        "contract_shape": fractal_flame_wgpu::contract::generate()["shape"],
        "count": rows.len(),
        "effects": rows,
    });

    let json = serde_json::to_string_pretty(&doc).expect("serialize effects");
    if let Some(dir) = std::path::Path::new(&out_path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&out_path, &json).unwrap_or_else(|e| {
        eprintln!("Error: cannot write `{out_path}`: {e}");
        std::process::exit(1);
    });

    eprintln!(
        "Wrote {out_path} — {} effects, {:.0} KB",
        doc["count"],
        json.len() as f64 / 1e3
    );
    if !over_param_cap.is_empty() {
        eprintln!(
            "WARNING: over the 48-parameter uniform capacity: {}",
            over_param_cap.join(", ")
        );
        std::process::exit(1);
    }
}
