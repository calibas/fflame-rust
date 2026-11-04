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

use super::{EffectCategory, EffectInfo, EffectSource};
use crate::renderer::effect_chain::MAX_EFFECT_PARAMS;
use crate::config::escape::{CustomDialect, CustomSource};
use crate::lang::{self, ir::ParamKind, EntryKind};
use crate::provenance::Provenance;
use crate::scripted::{self, Ledger};
use crate::params::{Options, ParamDef, ParamKind as K};

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
        // A number without both bounds is free; a missing bound is the
        // one its control draws (a float's -10..10, an int's -100..100,
        // an angle's 0..360). Its numbers are f32s, the slots'.
        let bounded = param.min.is_some() && param.max.is_some();
        let default = param.default.0 as f32;
        let span = |(lo, hi): (f32, f32)| (f64::from(param.min.map_or(lo, |v| v as f32)), f64::from(param.max.map_or(hi, |v| v as f32)));
        let kind = match param.kind {
            ParamKind::Float => {
                let (min, max) = span(if param.angle { (0.0, 360.0) } else { (-10.0, 10.0) });
                K::Float { default: default.into(), min, max, free: !bounded && !param.angle, log: false, angle: param.angle }
            }
            ParamKind::Int => {
                let (min, max) = span((-100.0, 100.0));
                K::Int { default: default as i64, min: min as i64, max: max as i64, free: !bounded }
            }
            ParamKind::Bool => K::Bool { default: default > 0.5 },
            ParamKind::Enum => K::Choice { default: default.round().max(0.0) as usize, options: Options::Owned(param.choices.clone()) },
            ParamKind::Complex => {
                return Err(format!("`{}`: an effect's parameters are numbers, bools, choices and colours (`rgba`), not complex numbers", param.name))
            }
        };
        parameters.push(ParamDef::owned(param.name.clone(), caption, param.hint.clone(), kind));
    }
    if parameters.len() > MAX_EFFECT_PARAMS {
        return Err(format!("{} parameters: an effect has at most {MAX_EFFECT_PARAMS}", parameters.len()));
    }

    let wgsl = assemble(&lang::wgsl::effect(&program)?);
    // A shipped twin is validated by the tests (`every_built_in_effect_
    // validates`), not again at every start.
    #[cfg(not(target_arch = "wasm32"))]
    if !provenance.is_builtin() {
        lang::wgsl::validate_effect(&splice_blend_library(&wgsl)).map_err(|e| format!("the compiler produced invalid WGSL (a bug): {e}"))?;
    }

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

/// The built-ins that are their Rhai twins (D1; docs/projects/
/// rhai-plugins.md, P2 step 7): each one-pass effect that passed the
/// checks, by name, as `assets/effects/<name>.rhai`. Each is held to the
/// WGSL it replaced (`effects::wgsl_builtins`) by the pixel compares in
/// `effect_chain`.
pub const BUILTIN_TWINS: &[(&str, &str)] = &[
    ("vignette", include_str!("../../assets/effects/vignette.rhai")),
    ("film_grain", include_str!("../../assets/effects/film_grain.rhai")),
    ("chromatic_aberration", include_str!("../../assets/effects/chromatic_aberration.rhai")),
    ("sobel_edges", include_str!("../../assets/effects/sobel_edges.rhai")),
    ("density_blur", include_str!("../../assets/effects/density_blur.rhai")),
    ("bilateral_blur", include_str!("../../assets/effects/bilateral_blur.rhai")),
    ("sharpen", include_str!("../../assets/effects/sharpen.rhai")),
    ("hue_shift", include_str!("../../assets/effects/hue_shift.rhai")),
    ("plasma", include_str!("../../assets/effects/plasma.rhai")),
    ("tunnel", include_str!("../../assets/effects/tunnel.rhai")),
    ("kaleidoscope", include_str!("../../assets/effects/kaleidoscope.rhai")),
    ("julia", include_str!("../../assets/effects/julia.rhai")),
    ("worley_noise", include_str!("../../assets/effects/worley_noise.rhai")),
    ("simplex_noise", include_str!("../../assets/effects/simplex_noise.rhai")),
    ("domain_warp", include_str!("../../assets/effects/domain_warp.rhai")),
];

/// Twins that have not passed every check: the built-in stays its WGSL
/// ("one that fails a check keeps its WGSL until it passes"), and the
/// twin is held to it all the same, timed beside the others
/// (`the_twins_are_no_slower` says when one is ready), and offered by
/// Edit as Rhai. None now: kaleidoscope and julia waited here until the
/// compiler wrote what hand-written WGSL does (rhai-plugins.md, P2
/// step 7).
pub const PENDING_TWINS: &[(&str, &str)] = &[];

/// Every twin, replaced or pending.
pub fn all_twins() -> impl Iterator<Item = &'static (&'static str, &'static str)> {
    BUILTIN_TWINS.iter().chain(PENDING_TWINS)
}

/// The built-in `name` as its Rhai twin, if it has one: on desktop the
/// working copy under `assets/effects/` when it differs and compiles --
/// so a built-in is edited without a rebuild, as its WGSL was under
/// `shaders/` -- else the embedded copy. A working copy that does not
/// compile is logged and the shipped one runs.
pub fn builtin_twin(name: &str) -> Option<EffectInfo> {
    match builtin_source(BUILTIN_TWINS, name)? {
        (_, Ok(info)) => Some(info),
        (_, Err(e)) => {
            log::error!("the built-in effect `{name}`'s twin does not compile ({e}); its WGSL runs");
            None
        }
    }
}

/// The Rhai source of the built-in `name`, replaced or pending
/// ([`builtin_twin`]'s choice of copy): what the Effects panel's Edit as
/// Rhai starts from.
pub fn builtin_twin_source(name: &str) -> Option<String> {
    builtin_source(BUILTIN_TWINS, name)
        .or_else(|| builtin_source(PENDING_TWINS, name))
        .and_then(|(text, compiled)| compiled.is_ok().then_some(text))
}

/// The twin's source in `list` -- the working copy when it differs and
/// compiles, else the embedded one -- and what it compiled to.
fn builtin_source(list: &[(&str, &'static str)], name: &str) -> Option<(String, Result<EffectInfo, String>)> {
    let (_, embedded) = list.iter().find(|(n, _)| *n == name)?;
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(text) = std::fs::read_to_string(crate::resources::resource_path(format!("assets/effects/{name}.rhai"))) {
        if text != *embedded {
            match compile_twin(name, &text) {
                Ok(info) => return Some((text, Ok(info))),
                Err(e) => log::warn!("assets/effects/{name}.rhai does not compile ({e}); the shipped copy runs"),
            }
        }
    }
    Some((embedded.to_string(), compile_twin(name, embedded)))
}

/// A twin compiled as the built-in it is: under that name.
fn compile_twin(name: &str, source: &str) -> Result<EffectInfo, String> {
    let info = compile(source, Provenance::Builtin)?;
    if info.name != name {
        return Err(format!("its id is `{}`, not `{name}`", info.name));
    }
    Ok(info)
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

    /// The built-ins that are their twins run on the web (a D1 check):
    /// each module, as the chain builds it, samples only at an explicit
    /// level (Chrome refuses implicit derivatives in non-uniform control
    /// flow), writes no operator mix Tint refuses, no self-compare or
    /// self-division Metal's fast-math folds, and no subnormal literal.
    #[test]
    fn the_builtin_twins_are_web_safe() {
        use crate::variations::shader_lint;
        for (name, source) in all_twins() {
            let info = compile(source, Provenance::Builtin).unwrap_or_else(|e| panic!("{name}: {e}"));
            let wgsl = splice_blend_library(&info.source.wgsl());
            let code = shader_lint::strip_wgsl_comments(&wgsl);
            for implicit in ["textureSample(", "textureSampleBias(", "textureSampleCompare("] {
                assert!(!code.contains(implicit), "{name}: {implicit}..) samples with implicit derivatives");
            }
            assert!(shader_lint::mixed_operators(&wgsl).is_empty(), "{name}: {:?}", shader_lint::mixed_operators(&wgsl));
            assert!(shader_lint::self_operations(&wgsl).is_empty(), "{name}: {:?}", shader_lint::self_operations(&wgsl));
            assert!(shader_lint::subnormal_literals(&wgsl).is_empty(), "{name}: {:?}", shader_lint::subnormal_literals(&wgsl));
        }
    }

    /// An effect's code is what a hand-written one would be (D1's speed
    /// check): an int parameter read as a float, or compared with an int,
    /// is the float rounded; the image's size the u32 widened; `min`,
    /// `max` and `clamp` WGSL's own; `sin` the GPU's.
    #[test]
    fn an_effect_compiles_as_hand_written_wgsl() {
        let src = r#"
const PARAMS = #{
    n: #{ value: 3, min: 0, max: 9 },
    k: #{ value: 0.5, min: 0.0, max: 1.0 },
};
const INFO = #{ id: "7a1e2d3c-4b5a-4c6d-8e7f-9a0b1c2d3e4f" };
fn effect(c, p, ctx) {
    let a = to_float(p.n) / to_float(ctx.width);
    let on = p.n > 2;
    let x = clamp(c.r + a, 0.0, 1.0);
    let y = max(0.1, p.k) + min(x, 0.5) + sin(x);
    if on { rgba(vec3(y), c.a) } else { c }
}
"#;
        let wgsl = compile(src, Provenance::Custom).unwrap_or_else(|e| panic!("{e}")).source.wgsl();
        for want in ["round(get_param(0u))", "f32(effect_params.width)", "(round(get_param(0u)) > 2.0)", "max(", "min(", "sin("] {
            assert!(wgsl.contains(want), "{want} in\n{wgsl}");
        }
        for not in ["i32(round(get_param(0u)))", "i32(effect_params.width)", "fx_reduce"] {
            assert!(!wgsl.contains(not), "{not} in\n{wgsl}");
        }
    }

    /// Compiling the fifteen twins is what the effect registry's first use
    /// now costs; printed, for the record.
    #[test]
    #[ignore = "a measurement"]
    fn how_long_the_twins_take_to_compile() {
        let start = web_time::Instant::now();
        for (name, _) in BUILTIN_TWINS {
            assert!(builtin_twin(name).is_some(), "{name}");
        }
        println!("{} twins compiled in {:?}", BUILTIN_TWINS.len(), start.elapsed());
    }

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
        let names: Vec<&str> = info.parameters.iter().map(|p| &*p.name).collect();
        assert_eq!(names, ["amount", "tint_r", "tint_g", "tint_b", "tint_a", "mode", "invert"]);
        assert!(matches!(info.parameters[5].options(), Some(o) if o.len() == 3));
        assert!(matches!(info.parameters[6].kind, K::Bool { .. }));
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
