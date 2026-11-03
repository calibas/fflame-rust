//! Simulation colourings and models written in Rhai
//! (docs/projects/rhai-plugins.md, P4). A model is below the colourings
//! ([`compile_model`]): the same naming, registry and carrying.
//!
//! A colouring's source -- `fn cell_color(cell, p, ctx)`, a cell's sample
//! to a colour and its coverage -- compiles through the formula language
//! ([`EntryKind::SimColoring`]) to the `sim_color` the colour template
//! calls, and registers as a [`SimColoringDef`] like the built-ins, under
//! a name `custom-` begins: its key (`scripted::key`: its id and its
//! version) when it has an id of its own, else a hash of its source. The
//! features the template measures for it (the gradient, the tensor, the
//! distance, the relief, the cell) come from what it reads.
//!
//! A config carries the source it uses (`SimConfig::custom_coloring`)
//! and [`ensure`] registers it before anything resolves the name, so a
//! `.fflame` with one renders anywhere the app runs.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::{ColoringFeature, ModelDef, ModelFeature, SimColoringDef, SimParamDef, StateKind};
use crate::config::escape::{CustomDialect, CustomSource};
use crate::lang::{self, ir::ParamKind, EntryKind};

/// What a registry name starts with when it names a compiled colouring.
pub const PREFIX: &str = "custom-";

/// The colourings that ship as Rhai, by the built-in each is the twin of:
/// examples to open in the Formula Editor, each held to its built-in by a
/// pixel compare (`the_rhai_channel_colours_as_the_built_in`).
pub const SHIPPED: &[(&str, &str)] = &[("channel", include_str!("../../assets/sim/channel.rhai"))];

#[derive(Default)]
struct Registry {
    colorings: HashMap<String, &'static SimColoringDef>,
    models: HashMap<String, &'static ModelDef>,
    /// Names whose carried source failed to compile: not retried every
    /// frame. A new source is a new name.
    failed: HashMap<String, String>,
}

fn registry() -> &'static RwLock<Registry> {
    static R: OnceLock<RwLock<Registry>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// A registered custom colouring.
pub fn coloring(name: &str) -> Option<&'static SimColoringDef> {
    registry().read().ok()?.colorings.get(name).copied()
}

/// The name a source registers under: its key with an id of its own
/// (design rule 7), else the first 16 hex of its source's SHA-256.
pub fn name_of(source: &str) -> String {
    match lang::rhai::info_string(source, "id").filter(|id| crate::scripted::is_uuid(id)) {
        Some(id) => format!("{PREFIX}{}", crate::scripted::key(&id, source)),
        None => format!("{PREFIX}{}", &crate::scripted::version_of(source)[..16]),
    }
}

fn leak(s: impl Into<String>) -> &'static str {
    Box::leak(s.into().into_boxed_str())
}

/// A Rhai script's `PARAMS` as the simulation's parameters: numbers,
/// bools and choices, a slider's ends the declared ones or a span around
/// the default.
fn sim_params(program: &lang::ir::Program) -> Result<Vec<SimParamDef>, String> {
    let mut parameters = Vec::new();
    for param in &program.params {
        let caption = if param.caption.is_empty() { param.name.clone() } else { param.caption.clone() };
        let default = param.default.0 as f32;
        let (min, max, choices): (f32, f32, &'static [&'static str]) = match param.kind {
            ParamKind::Enum => {
                let choices: Vec<&'static str> = param.choices.iter().map(|c| leak(c.clone())).collect();
                (0.0, choices.len().saturating_sub(1) as f32, Box::leak(choices.into_boxed_slice()))
            }
            ParamKind::Bool => (0.0, 1.0, &["Off", "On"]),
            ParamKind::Float | ParamKind::Int => {
                let span = default.abs().max(1.0) * 10.0;
                (param.min.map_or(default - span, |v| v as f32), param.max.map_or(default + span, |v| v as f32), &[])
            }
            ParamKind::Complex => {
                return Err(format!("`{}`: a simulation's parameters are numbers, bools and choices, not complex numbers", param.name));
            }
        };
        parameters.push(SimParamDef { name: leak(param.name.clone()), display_name: leak(caption), default, min, max, tooltip: leak(param.hint.clone()), choices });
    }
    Ok(parameters)
}

/// A source compiled to the definition it registers as, under `name`.
pub fn compile(source: &str, name: &str) -> Result<SimColoringDef, String> {
    let first = |d: &[lang::Diag]| {
        d.iter()
            .find(|x| x.severity == lang::Severity::Error)
            .map(|x| x.render(source))
            .unwrap_or_else(|| "it does not compile".to_string())
    };
    if lang::rhai::kind_of(source) != Some(EntryKind::SimColoring) {
        return Err("a simulation colouring has `fn cell_color(cell, p)`".to_string());
    }
    let program = lang::rhai::sim_coloring(source).map_err(|d| first(&d))?;
    let parameters = sim_params(&program)?;
    let wgsl = lang::wgsl::sim_coloring(&program)?;
    let mut features = Vec::new();
    for (on, f) in [
        (wgsl.gradient, ColoringFeature::NeedsGradient),
        (wgsl.structure, ColoringFeature::NeedsStructure),
        (wgsl.distance, ColoringFeature::NeedsDistance),
        (wgsl.relief, ColoringFeature::NeedsRelief),
        (wgsl.reads_cell, ColoringFeature::ReadsCell),
        (wgsl.memory, ColoringFeature::ReadsMemory),
    ] {
        if on {
            features.push(f);
        }
    }
    let def = SimColoringDef {
        name: leak(name),
        display_name: leak(program.name.clone()),
        description: leak(crate::lang::rhai::source_labels(source).map(|l| l.description.resolve()).unwrap_or_default()),
        features: Box::leak(features.into_boxed_slice()),
        parameters: Box::leak(parameters.into_boxed_slice()),
        wgsl: leak(wgsl.body),
    };
    // The assembled module, as the renderer builds it: a compiler bug is
    // a message here, not a pipeline error at the first frame.
    #[cfg(not(target_arch = "wasm32"))]
    lang::wgsl::validate_effect(&super::assembler::assemble_color(
        &def,
        crate::config::sim::SimBoundary::default(),
        crate::config::sim::SimUpscale::default(),
        crate::config::sim::SimDownscale::default(),
        false,
    ))
    .map_err(|e| format!("the compiler produced invalid WGSL (a bug): {e}"))?;
    Ok(def)
}

/// Compile a source and register it under its name ([`name_of`]),
/// replacing what has that name (the same version, again). Gives the name.
pub fn register(source: &str) -> Result<String, String> {
    let name = name_of(source);
    register_as(source, &name)?;
    Ok(name)
}

/// [`register`] under a name given: the one a config names.
pub fn register_as(source: &str, name: &str) -> Result<(), String> {
    let def = compile(source, name)?;
    let def: &'static SimColoringDef = Box::leak(Box::new(def));
    if let Ok(mut r) = registry().write() {
        r.colorings.insert(name.to_string(), def);
        r.failed.remove(name);
    }
    Ok(())
}

/// A colouring's source as a config carries it: under its name.
pub fn carried(source: &str) -> CustomSource {
    CustomSource { dialect: CustomDialect::Rhai, entry: name_of(source), source: source.to_string(), variations: Vec::new() }
}

/// Register the colourings and models a simulation carries (flat and
/// per layer), before anything resolves their names. Cheap when they are
/// registered already; one that does not compile is logged once, and its
/// name falls back as an unknown one's does (`coloring_or_default`,
/// `model_or_default`).
pub fn ensure(sim: &crate::config::sim::SimConfig) {
    let failed = |name: &str| registry().read().is_ok_and(|r| r.failed.contains_key(name));
    let note = |name: &str, e: String| {
        log::warn!("The simulation's `{name}` did not compile: {e}");
        if let Ok(mut r) = registry().write() {
            r.failed.insert(name.to_string(), e);
        }
    };
    // The flat colouring and every colour layer's; the flat model and
    // every layer's (P4 step 4).
    let colorings = std::iter::once((&sim.coloring, &sim.custom_coloring)).chain(sim.color_layers.iter().map(|l| (&l.coloring, &l.custom_coloring)));
    for (name, src) in colorings {
        let Some(src) = src else { continue };
        if name.starts_with(PREFIX) && coloring(name).is_none() && !failed(name) {
            if let Err(e) = register_as(&src.source, name) {
                note(name, e);
            }
        }
    }
    let models = std::iter::once((&sim.model, &sim.custom_model)).chain(sim.layers.iter().map(|l| (&l.model, &l.custom_model)));
    for (name, src) in models {
        let Some(src) = src else { continue };
        if name.starts_with(PREFIX) && model(name).is_none() && !failed(name) {
            if let Err(e) = register_model_as(&src.source, name) {
                note(name, e);
            }
        }
    }
}

// ============================================================================
// Models (P4, step 2)
// ============================================================================

/// A registered custom model.
pub fn model(name: &str) -> Option<&'static ModelDef> {
    registry().read().ok()?.models.get(name).copied()
}

/// The models that ship as Rhai, by the built-in each is the twin of.
pub const SHIPPED_MODELS: &[(&str, &str)] = &[("gray_scott", include_str!("../../assets/sim/gray_scott.rhai"))];

/// A model's source compiled to the definition it registers as, under
/// `name`: each pass of its step (`cell_step`, `cell_step2` ...) and its
/// seed (`cell_seed`; without one, the init shape in `.x`), its working
/// values from `INFO` (`dt`, `max_dt`, `steps`, `diffusion`, `state`;
/// `time_step: false` and `settles: false` its `NoTimeStep` and
/// `NeverStills`),
/// and `NeedsRng` when a pass draws. The single-layer class: no kernel
/// table, no agents, no measure or repeated pass.
pub fn compile_model(source: &str, name: &str) -> Result<ModelDef, String> {
    use lang::rhai::SimFn;
    let first = |d: &[lang::Diag]| {
        d.iter()
            .find(|x| x.severity == lang::Severity::Error)
            .map(|x| x.render(source))
            .unwrap_or_else(|| "it does not compile".to_string())
    };
    if lang::rhai::kind_of(source) != Some(EntryKind::SimModel) {
        return Err("a simulation model has `fn cell_step(cell, p)`".to_string());
    }
    let passes = lang::rhai::sim_model_passes(source);
    let mut wgsl = String::new();
    let mut rng = false;
    let mut program = None;
    for n in 0..passes {
        let p = lang::rhai::sim_model(source, SimFn::Step(n)).map_err(|d| first(&d))?;
        let w = lang::wgsl::sim_model(&p, SimFn::Step(n))?;
        wgsl.push_str(&w.body);
        rng |= w.rng;
        program.get_or_insert(p);
    }
    let program = program.ok_or("a simulation model has `fn cell_step`")?;
    let seed = if lang::rhai::has_function(source, "cell_seed") {
        let p = lang::rhai::sim_model(source, SimFn::Seed).map_err(|d| first(&d))?;
        lang::wgsl::sim_model(&p, SimFn::Seed)?.body
    } else {
        "fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    return vec4<f32>(inside, 0.0, 0.0, 0.0);
}
".to_string()
    };
    let number = |key: &str| lang::rhai::info_number(source, key);
    let default_dt = number("dt").unwrap_or(1.0) as f32;
    let diffusion: Vec<&'static str> = lang::rhai::info_strings(source, "diffusion").into_iter().map(leak).collect();
    for d in &diffusion {
        if !program.params.iter().any(|p| p.name == *d) {
            return Err(format!("`INFO.diffusion` names `{d}`, which is not in `PARAMS`"));
        }
    }
    // What the panel and the export say of it (P4 step 5): a rule with no
    // time step (a cellular automaton's generations), one that never
    // settles.
    let mut features = Vec::new();
    for (on, f) in [
        (rng, ModelFeature::NeedsRng),
        (lang::rhai::info_bool(source, "time_step") == Some(false), ModelFeature::NoTimeStep),
        (lang::rhai::info_bool(source, "settles") == Some(false), ModelFeature::NeverStills),
    ] {
        if on {
            features.push(f);
        }
    }
    let def = ModelDef {
        name: leak(name),
        state: if lang::rhai::info_string(source, "state").as_deref() == Some("discrete") { StateKind::Discrete } else { StateKind::Continuous },
        display_name: leak(program.name.clone()),
        description: leak(lang::rhai::source_labels(source).map(|l| l.description.resolve()).unwrap_or_default()),
        features: Box::leak(features.into_boxed_slice()),
        parameters: Box::leak(sim_params(&program)?.into_boxed_slice()),
        presets: &[],
        wgsl: leak(wgsl),
        passes: u32::from(passes),
        repeat: None,
        measure: None,
        max_dt: number("max_dt").map_or(default_dt, |v| v as f32),
        diffusion: Box::leak(diffusion.into_boxed_slice()),
        agents: None,
        kernel: None,
        dt_bound: None,
        wgsl_seed: leak(seed),
        default_steps: number("steps").map_or(1000, |v| v.max(1.0) as u32),
        default_dt,
    };
    // Every module, as the renderer builds it: a compiler bug is a message
    // here, not a pipeline error at the first step.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let boundary = crate::config::sim::SimBoundary::default();
        for pass in 0..def.passes {
            lang::wgsl::validate_effect(&super::assembler::assemble_step(&def, boundary, pass)).map_err(|e| format!("the compiler produced invalid WGSL (a bug): {e}"))?;
        }
        lang::wgsl::validate_effect(&super::assembler::assemble_seed(&def, "blob")).map_err(|e| format!("the compiler produced an invalid seed (a bug): {e}"))?;
    }
    Ok(def)
}

/// Compile a model and register it under its name ([`name_of`]). Gives
/// the name.
pub fn register_model(source: &str) -> Result<String, String> {
    let name = name_of(source);
    register_model_as(source, &name)?;
    Ok(name)
}

/// [`register_model`] under a name given: the one a config names.
pub fn register_model_as(source: &str, name: &str) -> Result<(), String> {
    let def: &'static ModelDef = Box::leak(Box::new(compile_model(source, name)?));
    if let Ok(mut r) = registry().write() {
        r.models.insert(name.to_string(), def);
        r.failed.remove(name);
    }
    Ok(())
}

/// Why the colouring or model `name` a config carries did not register,
/// when it did not.
pub fn failure(name: &str) -> Option<String> {
    registry().read().ok()?.failed.get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Channel's twin: the built-in's first colouring, its parameters in
    /// its order.
    pub(crate) const CHANNEL: &str = SHIPPED[0].1;

    /// A colouring compiles to a definition the colour template takes:
    /// its parameters, its features from what it reads, a module naga
    /// validates (in `compile`).
    #[test]
    fn a_colouring_compiles_to_a_definition() {
        let def = compile(CHANNEL, "custom-test").unwrap_or_else(|e| panic!("{e}"));
        let names: Vec<&str> = def.parameters.iter().map(|p| p.name).collect();
        assert_eq!(names, ["channel", "scale", "offset", "wrap"]);
        assert!(def.features.is_empty(), "it reads the state alone");
        let src = "const INFO = #{ title: \"Edges\" };\nfn cell_color(cell, p, ctx) {\n    let g = vec3(cell.gx.x, cell.gy.x, cell.dist);\n    \
                   let c = palette(length(g));\n    rgba(c.x, c.y, c.z, if ctx.cell.re > 0.0 { 1.0 } else { cell.tensor.x + cell.relief.y })\n}\n";
        let def = compile(src, "custom-edges").unwrap_or_else(|e| panic!("{e}"));
        for f in [ColoringFeature::NeedsGradient, ColoringFeature::NeedsDistance, ColoringFeature::NeedsStructure, ColoringFeature::NeedsRelief, ColoringFeature::ReadsCell] {
            assert!(def.has(f), "{f:?}: {:?}", def.features);
        }
        assert!(compile("fn cell_color(cell) { rand() }", "custom-x").unwrap_err().contains("rand"));
        assert!(compile("fn effect(c, p) { c }", "custom-x").is_err());
    }

    /// Gray-Scott's twin compiles to a model of one pass, its parameters in
    /// the built-in's order, its working values from `INFO`, and modules
    /// naga validates (in `compile_model`).
    #[test]
    fn a_model_compiles_to_a_definition() {
        let def = compile_model(SHIPPED_MODELS[0].1, "custom-gs").unwrap_or_else(|e| panic!("{e}"));
        let builtin = &super::super::models::GRAY_SCOTT;
        let names = |d: &ModelDef| d.parameters.iter().map(|p| (p.name, p.default, p.min, p.max)).collect::<Vec<_>>();
        assert_eq!(names(&def), names(builtin));
        assert_eq!((def.passes, def.default_dt, def.max_dt, def.default_steps), (1, 1.0, 1.25, 10000));
        assert_eq!(def.diffusion, builtin.diffusion);
        assert!(!def.has(ModelFeature::NeedsRng));
        let two = "fn cell_step(cell, p, ctx) {\n    let n = read(1, 0);\n    vec4(n.x, rand(), ctx.dt, ctx.step)\n}\n                   fn cell_step2(cell, p) { vec4(cell.y, cell.x, 0.0, 0.0) }\nfn cell_seed(inside, noise) { vec4(inside, noise, 0.0, 0.0) }\n";
        let def = compile_model(two, "custom-two").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(def.passes, 2);
        assert!(def.has(ModelFeature::NeedsRng));
        let e = compile_model("fn cell_step(cell) { read(1, 0) }\nfn cell_seed(inside, noise) { sin(noise) }\n", "custom-x").unwrap_err();
        assert!(e.contains("a number where a vector"), "{e}");
        let e = compile_model("const INFO = #{ agents: 1000 };\nfn cell_step(cell) { read(1, 0) }\n", "custom-x").unwrap_err();
        assert!(e.contains("INFO.agents") && e.contains("single-layer"), "{e}");
    }

    /// A carried source registers under the name the config gives it, and
    /// the lookup every path uses finds it.
    #[test]
    fn a_carried_colouring_is_ensured() {
        let mut sim = crate::config::sim::SimConfig::default();
        let src = carried(CHANNEL);
        sim.coloring = src.entry.clone();
        sim.custom_coloring = Some(src);
        let model = SHIPPED_MODELS[0].1;
        sim.model = name_of(model);
        sim.custom_model = Some(CustomSource { dialect: CustomDialect::Rhai, entry: sim.model.clone(), source: model.to_string(), variations: Vec::new() });
        ensure(&sim);
        assert_eq!(super::super::coloring_or_default(&sim.coloring).name, sim.coloring);
        assert_eq!(super::super::model_or_default(&sim.model).name, sim.model);
    }

    /// The language the twins need (P4 step 5), each part in modules naga
    /// validates (`compile_model`, `compile`) and visible in the WGSL: a
    /// model's `rand(k)` on its stream, in the step and the seed, its seed
    /// drawing and reading the init shape elsewhere (`init_at`), the
    /// `INFO` flags, `break`, `continue` -- the counted loop kept -- and
    /// an early `return`; a colouring's step, memory, field reads, hash
    /// and seed; an effect's early `return`.
    #[test]
    fn the_language_of_the_twins_compiles() {
        let model = "const INFO = #{ time_step: false, settles: false };\n\
            fn cell_step(cell, p, ctx) {\n    if cell.x > 0.5 { return cell; }\n    let n = 0.0;\n    for dy in -2..=2 {\n        \
            for dx in -2..=2 {\n            if dx == 0 && dy == 0 { continue; }\n            n += read(dx, dy).x;\n            \
            if n > 3.0 { break; }\n        }\n    }\n    vec4(n, rand(0xc1), to_float(pcg(ctx.seed_lo) & 255), 0.0)\n}\n\
            fn cell_seed(inside, noise, p, ctx) { vec4(init_at(ctx.cell.re, 32.0), rand(0x71), rand(), 0.0) }\n";
        let def = compile_model(model, "custom-language").unwrap_or_else(|e| panic!("{e}"));
        for f in [ModelFeature::NoTimeStep, ModelFeature::NeverStills, ModelFeature::NeedsRng] {
            assert!(def.has(f), "{f:?}: {:?}", def.features);
        }
        for want in ["sim_rand(p, bitcast<u32>(", "continue;", "break;", "bitcast<i32>(sim_pcg(bitcast<u32>(", "bitcast<i32>(params.seed_lo)"] {
            assert!(def.wgsl.contains(want), "{want} in\n{}", def.wgsl);
        }
        assert_eq!(def.wgsl.matches("return vec4<f32>(effect_r, effect_g, effect_b, effect_a);").count(), 2, "the early return and the end:\n{}", def.wgsl);
        assert!(!def.wgsl.contains("for (var loop"), "both loops stay counted, `continue` and all:\n{}", def.wgsl);
        assert!(def.wgsl_seed.contains("sim_init_mask(") && def.wgsl_seed.matches("sim_rand(p,").count() == 2, "{}", def.wgsl_seed);

        let colouring = "fn cell_color(cell, p, ctx) {\n    if cell.m0.x > 0.9 { return rgba(1.0, 0.0, 0.0, 1.0); }\n    \
            let t = to_float(pcg(to_int(ctx.cell.re) ^ ctx.seed_hi) & 1023) / 1023.0;\n    let s = 0.0;\n    let i = 0;\n    \
            while i < 8 {\n        s += read(i, 0).x;\n        if s > 2.0 { break; }\n        i += 1;\n    }\n    \
            let c = palette(t + s + ctx.step * 0.001 + cell.m1.w);\n    rgba(c.x, c.y, c.z, 1.0)\n}\n";
        let def = compile(colouring, "custom-language").unwrap_or_else(|e| panic!("{e}"));
        assert!(def.has(ColoringFeature::ReadsCell) && def.has(ColoringFeature::ReadsMemory), "{:?}", def.features);
        for want in ["f32(sim_step_index())", "x.m0.x", "x.m1.w", "break;", "bitcast<i32>(params.seed_hi)"] {
            assert!(def.wgsl.contains(want), "{want} in\n{}", def.wgsl);
        }

        let effect = "fn effect(c) {\n    if c.a < 0.5 { return c; }\n    rgba(1.0 - c.r, c.g, c.b, c.a)\n}\n";
        let p = lang::rhai::effect(effect).unwrap_or_else(|d| panic!("{d:?}"));
        let w = lang::wgsl::effect(&p).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(w.body.matches("return FxOut(").count(), 2, "{}", w.body);

        // And what they refuse, with the reason.
        let refused = |src: &str, why: &str| {
            let e = compile_model(src, "custom-x").unwrap_err();
            assert!(e.contains(why), "{why}: {e}");
        };
        refused("fn cell_step(cell) { vec4(rand(1.5), 0.0, 0.0, 0.0) }", "takes an int");
        refused("fn cell_step(cell) { vec4(init_at(1.0, 2.0), 0.0, 0.0, 0.0) }", "is a seed's");
        refused("fn cell_step(cell) {\n    let i = 0;\n    do { i += 1; continue; } while i < 3;\n    cell\n}", "`continue` in a `do` loop");
        refused("fn helper(x) { if x > 0.0 { return x; } x }\nfn cell_step(cell) { vec4(helper(cell.x), 0.0, 0.0, 0.0) }", "`return` ends a function");
        // A helper's own loop breaks (Rhai's parser refuses a `break`
        // outside one).
        let own = "fn first(x) {\n    let n = 0.0;\n    for i in 0..8 { n += x; if n > 2.0 { break; } }\n    n\n}\nfn cell_step(cell) { vec4(first(cell.x), 0.0, 0.0, 0.0) }";
        assert!(compile_model(own, "custom-x").unwrap_or_else(|e| panic!("{e}")).wgsl.contains("break;"));
        let formula = "fn start(pixel, p) { #{ z: pixel } }\nfn step(s, p) {\n    for i in 0..3 { if i == 1 { break; } }\n    s\n}";
        let e = lang::rhai::program(formula).unwrap_err();
        assert!(e.iter().any(|d| d.message.contains("a formula's loops have no")), "{e:?}");
    }

    /// A source goes with its layer (P4 step 4): promoting the flat model
    /// and colouring into the stacks carries their sources into entry 0,
    /// a later entry's source registers under its own name, and demoting
    /// brings entry 0's back to the flat fields.
    #[test]
    fn a_source_goes_with_its_layer() {
        use crate::config::sim::{SimColorLayer, SimLayer};
        let mut sim = crate::config::sim::SimConfig::default();
        let (model, colouring) = (carried(SHIPPED_MODELS[0].1), carried(CHANNEL));
        sim.model = model.entry.clone();
        sim.custom_model = Some(model.clone());
        sim.coloring = colouring.entry.clone();
        sim.custom_coloring = Some(colouring.clone());
        sim.promote_model_to_layers();
        sim.promote_coloring_to_layers();
        assert_eq!(sim.layers[0].custom_model.as_ref(), Some(&model));
        assert_eq!(sim.color_layers[0].custom_coloring.as_ref(), Some(&colouring));
        // Entry 1 under a name of its own: ensure registers it there.
        let renamed = |c: &CustomSource, name: &str| CustomSource { entry: name.to_string(), ..c.clone() };
        sim.layers.push(SimLayer { model: "custom-layer-test".into(), model_params: Default::default(), enabled: true, custom_model: Some(renamed(&model, "custom-layer-test")) });
        sim.color_layers.push(SimColorLayer { coloring: "custom-colour-layer-test".into(), custom_coloring: Some(renamed(&colouring, "custom-colour-layer-test")), ..Default::default() });
        ensure(&sim);
        assert_eq!(super::super::model_or_default("custom-layer-test").name, "custom-layer-test");
        assert_eq!(super::super::coloring_or_default("custom-colour-layer-test").name, "custom-colour-layer-test");
        sim.layers.pop();
        sim.color_layers.pop();
        sim.custom_model = None;
        sim.custom_coloring = None;
        sim.demote_single_layer();
        sim.demote_single_color_layer();
        assert_eq!((sim.custom_model.as_ref(), sim.custom_coloring.as_ref()), (Some(&model), Some(&colouring)));
    }

    /// Two names are one Rhai model's versions when both are `custom-` and
    /// a key with the same id; a built-in's name, or a hash name, is not.
    #[test]
    fn versions_of_one_script_are_told_apart_from_others() {
        let v1 = format!("custom-{}", crate::scripted::key("6f1e1d6c-6b1a-4c55-9a43-2d4b7c1f0e21", "a"));
        let v2 = format!("custom-{}", crate::scripted::key("6f1e1d6c-6b1a-4c55-9a43-2d4b7c1f0e21", "b"));
        let other = format!("custom-{}", crate::scripted::key("0b7e4a0e-55a9-4d2c-8f7e-3f5c1a2b9d10", "a"));
        assert!(crate::config::sim::same_script(&v1, &v2));
        assert!(!crate::config::sim::same_script(&v1, &other));
        assert!(!crate::config::sim::same_script("gray_scott", "gray_scott"));
        assert!(!crate::config::sim::same_script("custom-0123456789abcdef", "custom-0123456789abcdef"));
    }
}
