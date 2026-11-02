//! Effects written in Rhai (docs/projects/rhai-plugins.md, P2).
//!
//! An effect's source -- `fn effect(c, p, ctx)`, the input's colour at a
//! pixel to the output's -- compiles through the formula language
//! (`escape::lang`, [`EntryKind::Effect`]) to a function of the pixel's
//! position, which [`assemble`] wraps in the effect template: the uniform
//! every effect reads and `get_param` over it, the input image and its
//! sampler, the fullscreen triangle, and the blend library when the
//! effect asks for a blend. It registers as an [`EffectInfo`] like the
//! WGSL effects, under its `INFO.id` -- a UUID for one of the user's
//! (design rule 7), a built-in's name for a shipped one -- with its labels
//! kept as written, which the panel shows in the current language.
//!
//! A config carries the sources it uses (`FractalConfig::custom_effects`)
//! and [`ensure`] registers them before anything renders it, so a
//! `.fflame` with an effect of the user's renders anywhere the app runs.

use std::sync::{Arc, Mutex, OnceLock};

use super::{EffectCategory, EffectInfo, EffectParameter, EffectSource};
use crate::renderer::effect_chain::MAX_EFFECT_PARAMS;
use crate::config::escape::{CustomDialect, CustomSource};
use crate::lang::{self, ir::ParamKind, EntryKind};
use crate::provenance::Provenance;
use crate::scripted::{self, Ledger};
use crate::variations::ParamType;

/// The effect template around a compiled effect's `fx_effect(uv)`.
///
/// The uniform is the runner's `EffectParams` (208 bytes:
/// `effect_chain::EffectParams`), the same for colour and density
/// effects; a parameter is `get_param(slot)`, in the order `PARAMS`
/// declares them. When the effect asks for a blend (`#{ color, blend }`),
/// its colour is blended over the input's by the blend library's
/// `apply_blend`, the input's alpha kept.
pub fn assemble(effect: &lang::wgsl::EffectWgsl) -> String {
    let mut out = String::from(
        "struct EffectParams {\n    params: array<vec4<f32>, 12>,\n    width: u32,\n    height: u32,\n    density_mean: f32,\n    pixel_scale: f32,\n}\n\n\
         fn get_param(index: u32) -> f32 {\n    return effect_params.params[index / 4u][index % 4u];\n}\n\n\
         struct VertexOutput {\n    @builtin(position) position: vec4<f32>,\n    @location(0) uv: vec2<f32>,\n}\n\n\
         @group(0) @binding(0) var input_texture: texture_2d<f32>;\n\
         @group(0) @binding(1) var input_sampler: sampler;\n\
         @group(0) @binding(2) var<uniform> effect_params: EffectParams;\n\n",
    );
    if effect.blends {
        out.push_str("// INCLUDE_BLEND_MODES\n\n");
    }
    out.push_str(&effect.body);
    out.push_str(
        "\n@vertex\nfn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {\n    var output: VertexOutput;\n    \
         let x = f32((vertex_index & 1u) << 2u);\n    let y = f32((vertex_index & 2u) << 1u);\n    \
         output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);\n    output.uv = vec2<f32>(x * 0.5, y * 0.5);\n    return output;\n}\n\n\
         @fragment\nfn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n    let out = fx_effect(input.uv);\n",
    );
    if effect.blends {
        out.push_str(
            "    if (out.blend >= 0) {\n        let base = textureSampleLevel(input_texture, input_sampler, input.uv, 0.0);\n        \
             return vec4<f32>(apply_blend(base.rgb, out.color.rgb, out.blend, out.amount), base.a);\n    }\n",
        );
    }
    out.push_str("    return out.color;\n}\n");
    out
}

/// An effect's source compiled to the [`EffectInfo`] it registers as.
///
/// Its id is `INFO.id`: a UUID for anything but a built-in, whose id is
/// its name. Its parameters are numbers, bools, choices and colours (an
/// `rgba` parameter is four numbers, `name_r` ...), at most
/// [`MAX_EFFECT_PARAMS`].
pub fn compile(source: &str, provenance: Provenance) -> Result<EffectInfo, String> {
    let first = |d: &[lang::Diag]| {
        d.iter()
            .find(|x| x.severity == lang::Severity::Error)
            .map(|x| x.render(source))
            .unwrap_or_else(|| "it does not compile".to_string())
    };
    if lang::rhai::kind_of(source) != Some(EntryKind::Effect) {
        return Err("an effect has `fn effect(c, p)`".to_string());
    }
    let program = lang::rhai::effect(source).map_err(|d| first(&d))?;
    let id = lang::rhai::info_string(source, "id")
        .ok_or_else(|| "an effect names itself: `const INFO = #{ id: \"...\" }` (one is minted when it is saved)".to_string())?;
    if !provenance.is_builtin() && !scripted::is_uuid(&id) {
        return Err(format!(
            "`{id}` is not an effect's id: one of yours is named by a UUID, which the Formula Editor gives it on Use as effect"
        ));
    }
    let category = match lang::rhai::info_string(source, "category").as_deref() {
        Some("density") => EffectCategory::Density,
        _ => EffectCategory::Color,
    };

    let mut parameters = Vec::new();
    for param in &program.params {
        let caption = if param.caption.is_empty() { param.name.clone() } else { param.caption.clone() };
        let bounded = param.min.is_some() && param.max.is_some();
        let param_type = match param.kind {
            ParamKind::Float if param.angle => ParamType::Angle,
            ParamKind::Float if bounded => ParamType::Float,
            ParamKind::Float => ParamType::UnlimitedFloat,
            ParamKind::Int if bounded => ParamType::Integer,
            ParamKind::Int => ParamType::UnlimitedInteger,
            ParamKind::Bool => ParamType::Boolean,
            ParamKind::Enum => {
                let choices: Vec<&'static str> = param.choices.iter().map(|c| &*Box::leak(c.clone().into_boxed_str())).collect();
                ParamType::Enum { choices: Box::leak(choices.into_boxed_slice()) }
            }
            ParamKind::Complex => {
                return Err(format!("`{}`: an effect's parameters are numbers, bools, choices and colours (`rgba`), not complex numbers", param.name))
            }
        };
        let (min_value, max_value) = match &param_type {
            ParamType::Enum { choices } => (Some(0.0), Some(choices.len().saturating_sub(1) as f32)),
            _ => (param.min.map(|v| v as f32), param.max.map(|v| v as f32)),
        };
        parameters.push(EffectParameter {
            name: param.name.clone(),
            display_name: caption,
            param_type,
            default_value: param.default.0 as f32,
            min_value,
            max_value,
            description: (!param.hint.is_empty()).then(|| param.hint.clone()),
        });
    }
    if parameters.len() > MAX_EFFECT_PARAMS {
        return Err(format!("{} parameters: an effect has at most {MAX_EFFECT_PARAMS}", parameters.len()));
    }

    let wgsl = assemble(&lang::wgsl::effect(&program)?);
    #[cfg(not(target_arch = "wasm32"))]
    lang::wgsl::validate_effect(&splice_blend_library(&wgsl)).map_err(|e| format!("the compiler produced invalid WGSL (a bug): {e}"))?;

    Ok(EffectInfo {
        // Under its key: the version is its name's, so two versions of
        // one effect live side by side (rule 7).
        name: if provenance.is_builtin() { id } else { key_for(&id, source) },
        category,
        display_name: program.name.clone(),
        source: EffectSource::Owned(wgsl),
        parameters,
        provenance,
        labels: lang::rhai::source_labels(source).map(Arc::new),
    })
}

/// The module as the effect chain builds it: the blend library spliced
/// in where the template asks for it.
#[cfg(not(target_arch = "wasm32"))]
fn splice_blend_library(wgsl: &str) -> String {
    wgsl.replace("// INCLUDE_BLEND_MODES", super::embedded_shaders::BLEND_MODES)
}

/// The registry key of one of the user's effects: its id and its
/// version (`scripted::key`, rule 7). Every version is its own entry, so a
/// config renders the version it carries whatever else is registered; a
/// built-in's key is its name.
pub fn key_for(id: &str, source: &str) -> String {
    scripted::key(id, source)
}

/// The id a key names, its version dropped (a built-in's key is its id).
pub fn id_of(key: &str) -> String {
    scripted::key_id(key).unwrap_or_else(|| key.to_string())
}

/// Compile a source and register it under its key, replacing what has that
/// key (the same version, again); refused when a built-in has it and this
/// is not one. Gives the key.
pub fn register(source: &str, provenance: Provenance) -> Result<String, String> {
    let info = compile(source, provenance.clone())?;
    register_info(info, provenance)
}

fn register_info(info: EffectInfo, provenance: Provenance) -> Result<String, String> {
    let id = info.name.clone();
    let mut registry = super::global_effect_registry_mut();
    if let Some(existing) = registry.get(&id) {
        if existing.provenance.is_builtin() && !provenance.is_builtin() {
            return Err(format!("`{id}` is a built-in effect's id"));
        }
    }
    registry.register(info);
    Ok(id)
}

/// A source as a config carries it: under its key, the name its
/// instances run.
pub fn carried(source: &str) -> Option<CustomSource> {
    let id = lang::rhai::info_string(source, "id")?;
    Some(CustomSource { dialect: CustomDialect::Rhai, entry: key_for(&id, source), source: source.to_string(), variations: Vec::new() })
}

/// Which version of each effect source is registered, and the text it was
/// last checked against: the viewport asks every frame, and comparing the
/// text it has is cheaper than hashing it again.
#[derive(Default)]
struct Registered {
    ledger: Ledger,
    seen: std::collections::HashMap<String, String>,
}

fn registered() -> &'static Mutex<Registered> {
    static R: OnceLock<Mutex<Registered>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Register the effects a config carries, before anything renders it.
/// Cheap when each is registered with its source already; one that does
/// not compile is logged once, and its instances render as an unknown
/// effect's do -- not at all.
pub fn ensure(config: &crate::config::FractalConfig) {
    for src in &config.custom_effects {
        let Ok(mut r) = registered().lock() else { return };
        if r.seen.get(&src.entry) == Some(&src.source) {
            continue;
        }
        r.seen.insert(src.entry.clone(), src.source.clone());
        let version = scripted::version_of(&src.source);
        let ledger = &mut r.ledger;
        if !ledger.needs(&src.entry, &version) {
            continue;
        }
        // Under the key the config names, which its instances run (a
        // config edited by hand may name one its text no longer hashes to).
        let result = compile(&src.source, Provenance::Custom).and_then(|mut info| {
            info.name = src.entry.clone();
            register_info(info, Provenance::Custom).map(|_| ())
        });
        if let Err(e) = &result {
            log::warn!("Custom effect `{}` did not register: {e}", src.entry);
        }
        ledger.record(&src.entry, &version, result);
    }
}

/// Why the carried effect `id` did not register, when it did not.
pub fn failure(id: &str) -> Option<String> {
    registered().lock().ok()?.ledger.failure(id).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An effect of the user's: an id, a parameter of each kind, a blend.
    pub(crate) const TINT: &str = r#"
const PARAMS = #{
    amount: #{ value: 0.5, min: 0.0, max: 1.0, caption: #{ en: "Amount", ja: "量" } },
    tint: #{ value: rgba(1.0, 0.5, 0.0, 1.0), caption: "Tint" },
    mode: #{ value: 2, choices: ["Normal", "Add", "Multiply"] },
    invert: #{ value: false },
};
const INFO = #{ title: #{ en: "Tint", ja: "色合い" }, id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b" };

fn effect(c, p, ctx) {
    let rgb = if p.invert { vec3(1.0) - c.rgb } else { c.rgb };
    #{ color: rgb * p.tint.rgb, blend: p.mode, amount: p.amount }
}
"#;

    #[test]
    fn an_effect_compiles_to_a_module_naga_accepts() {
        let info = compile(TINT, Provenance::Custom).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(info.name, key_for("6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b", TINT), "under its key: id and version");
        assert_eq!(id_of(&info.name), "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b");
        assert_eq!(info.category, EffectCategory::Color);
        let names: Vec<&str> = info.parameters.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["amount", "tint_r", "tint_g", "tint_b", "tint_a", "mode", "invert"]);
        assert!(matches!(info.parameters[5].param_type, ParamType::Enum { choices } if choices.len() == 3));
        assert!(matches!(info.parameters[6].param_type, ParamType::Boolean));
        let EffectSource::Owned(wgsl) = &info.source else { panic!("owned") };
        assert!(wgsl.contains("// INCLUDE_BLEND_MODES"), "it blends, so the library is spliced");
        assert!(wgsl.contains("fn fs_main") && wgsl.contains("fn vs_main"));
        // Its labels, in each language, for the panel.
        let labels = info.labels.as_ref().expect("labels");
        assert_eq!(labels.title.resolve_in("ja"), "色合い");
        assert_eq!(labels.param("amount").expect("amount").caption.resolve_in("ja"), "量");
    }

    #[test]
    fn an_effect_without_a_blend_returns_its_colour() {
        let src = "const INFO = #{ id: \"0e5c6b2a-1d3f-4a7e-9b8c-2f4d6a8c0e1b\", category: \"density\" };\n\
                   fn effect(c, p, ctx) {\n    let n = input_at(ctx.width - 1, 0);\n    rgba(c.r, c.g, c.b, (c.a + n.a) * 0.5 * ctx.density_mean)\n}\n";
        let info = compile(src, Provenance::Custom).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(info.category, EffectCategory::Density);
        let EffectSource::Owned(wgsl) = &info.source else { panic!("owned") };
        assert!(!wgsl.contains("INCLUDE_BLEND_MODES"));
        assert!(wgsl.contains("textureLoad"));
    }

    #[test]
    fn what_an_effect_cannot_be() {
        let id = "0e5c6b2a-1d3f-4a7e-9b8c-2f4d6a8c0e1b";
        let with = |body: &str| format!("const INFO = #{{ id: \"{id}\" }};\n{body}");
        let refused = |src: &str, why: &str| {
            let e = compile(src, Provenance::Custom).expect_err(src);
            assert!(e.contains(why), "expected `{why}` in: {e}");
        };
        refused("fn effect(c, p) { c }", "names itself");
        refused("const INFO = #{ id: \"my_glow\" };\nfn effect(c, p) { c }", "UUID");
        refused(&with("fn effect(c, p) { let r = rand(); c * r }"), "no `rand()`");
        refused(&with("const PARAMS = #{ k: #{ value: complex(1.0, 0.0) } };\nfn effect(c, p) { c }"), "not complex");
        refused(&with("fn effect(c, p) { quat(1.0, 0.0, 0.0, 0.0) }"), "a colour");
        // An image is an effect's to read.
        let formula = "fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\nfn step(s, p) { let k = input(s.c); s.z = s.z * s.z + s.c; s }\nfn escaped(s, p) { s.z.norm_sqr() > 4.0 }\n";
        let e = lang::rhai::program(formula).expect_err("a formula reads no image");
        assert!(e.iter().any(|d| d.message.contains("effect")), "{e:?}");
        // A built-in keeps its name.
        let builtin = "const INFO = #{ id: \"my_builtin\" };\nfn effect(c, p) { c }";
        assert_eq!(compile(builtin, Provenance::Builtin).expect("a built-in's name").name, "my_builtin");
    }
}
