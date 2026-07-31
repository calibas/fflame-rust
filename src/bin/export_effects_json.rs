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
//! and both are visible in the output as nulls rather than papered over:
//!
//! * **`EffectParameter` has no `description`.** `VariationParameter`
//!   gained one and 2836 of 2971 `param!` macros populate it; the effect
//!   corpus has nothing equivalent, so every per-param description is
//!   null. Authoring them is the effects half of the bulk-metadata work.
//! * **`EffectInfo` has no `display_name`.** The curated English labels
//!   live in `locales/en.yml` under `effects.<name>.name`, which is
//!   where this reads them from — the panel itself still shows the raw
//!   registry key. Worth promoting to a struct field eventually, the
//!   way variations already have.

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
    let mut missing_shader = Vec::new();
    let mut over_param_cap = Vec::new();

    let mut all: Vec<_> = registry.all().collect();
    all.sort_by(|a, b| a.name.cmp(&b.name));

    for info in all {
        // Raw source, includes NOT spliced. The `// INCLUDE_BLEND_MODES`
        // marker is part of what the server stores; the client splices
        // at compile time, so pre-splicing here would bake a copy of the
        // shared library into all 12 effects that use it.
        let path = format!("shaders/{}", info.shader_path);
        let shader = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                missing_shader.push(format!("{} ({path}: {e})", info.name));
                continue;
            }
        };
        let requires_blend_modes = shader.contains("// INCLUDE_BLEND_MODES");

        // Curated English label. Not on EffectInfo — see the module docs.
        let key = format!("effects.{}.name", info.name);
        let display_name = {
            let t = rust_i18n::t!(&key);
            if t == key { info.name.clone() } else { t.to_string() }
        };

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
                    // No such field on EffectParameter yet.
                    "description": serde_json::Value::Null,
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
                     `description`, `description_plain`, `authors` and every \
                     per-parameter `description` are null: EffectParameter has \
                     no description field yet, and effect prose has not been \
                     authored. Shader source is RAW — the \
                     `// INCLUDE_BLEND_MODES` marker is intact and \
                     requires_blend_modes says whether it is present.",
        "contract_shape": fractal_flame_wgpu::variations::contract::generate()["shape"],
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
    if !missing_shader.is_empty() {
        eprintln!("WARNING: shader unreadable, effect omitted: {}", missing_shader.join(", "));
        std::process::exit(1);
    }
    if !over_param_cap.is_empty() {
        eprintln!(
            "WARNING: over the 48-parameter uniform capacity: {}",
            over_param_cap.join(", ")
        );
        std::process::exit(1);
    }
}
