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
use std::sync::{Arc, OnceLock, RwLock};

use super::{ColoringFeature, ModelDef, ModelFeature, SimColoringDef, SimParamDef, SimPreset, StateKind};
use crate::config::escape::{CustomDialect, CustomSource};
use crate::lang::{self, ir::ParamKind, EntryKind};

/// What a registry name starts with when it names a compiled colouring.
pub const PREFIX: &str = "custom-";

/// The colourings that ship as Rhai, by the built-in each is the twin of
/// (P4 step 7): every built-in, examples to open in the Formula Editor,
/// each held to its built-in's settings
/// (`every_colouring_twin_carries_its_built_ins_settings`) and picture
/// (`the_rhai_colourings_colour_as_the_built_ins`).
pub const SHIPPED: &[(&str, &str)] = &[
    ("channel", include_str!("../../assets/sim/channel.rhai")),
    ("two_channel", include_str!("../../assets/sim/two_channel.rhai")),
    ("age", include_str!("../../assets/sim/age.rhai")),
    ("label", include_str!("../../assets/sim/label.rhai")),
    ("scale_mix", include_str!("../../assets/sim/scale_mix.rhai")),
    ("occupancy", include_str!("../../assets/sim/occupancy.rhai")),
    ("gradient", include_str!("../../assets/sim/gradient.rhai")),
    ("structure", include_str!("../../assets/sim/structure.rhai")),
    ("distance", include_str!("../../assets/sim/distance.rhai")),
    ("lic", include_str!("../../assets/sim/lic.rhai")),
    ("species", include_str!("../../assets/sim/species.rhai")),
    ("scale_memory", include_str!("../../assets/sim/scale_memory.rhai")),
    ("relief", include_str!("../../assets/sim/relief.rhai")),
];

#[derive(Default)]
struct Registry {
    colorings: HashMap<String, &'static SimColoringDef>,
    models: HashMap<String, &'static ModelDef>,
    /// Names whose carried source failed to compile: not retried every
    /// frame. A new source is a new name.
    failed: HashMap<String, String>,
    /// The models with functions run on the CPU (`max_dt`, `kernel`,
    /// `discs`), by name (P4 steps 6 and 9b).
    cpu: HashMap<String, Arc<CpuScript>>,
    /// The models with per-scale warps (`INFO.scale_warps`), by name (P4
    /// step 9c).
    warps: HashMap<String, &'static super::ScaleWarps>,
    /// The models' slow parameters (`INFO.slow`), by name.
    slow: HashMap<String, Vec<String>>,
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

/// The models that ship as Rhai, by the built-in each is the twin of
/// (P4 step 7): each held to its built-in's settings
/// (`every_twin_carries_its_built_ins_settings`), field
/// (`the_rhai_models_step_as_the_built_ins`) and speed
/// (`the_rhai_models_are_no_slower`). Assembled by
/// `scripts/assemble_sim_twins.py`.
pub const SHIPPED_MODELS: &[(&str, &str)] = &[
    ("gray_scott", include_str!("../../assets/sim/gray_scott.rhai")),
    ("fitzhugh_nagumo", include_str!("../../assets/sim/fitzhugh_nagumo.rhai")),
    ("brusselator", include_str!("../../assets/sim/brusselator.rhai")),
    ("schnakenberg", include_str!("../../assets/sim/schnakenberg.rhai")),
    ("oregonator", include_str!("../../assets/sim/oregonator.rhai")),
    ("brusselator2", include_str!("../../assets/sim/brusselator2.rhai")),
    ("rossler", include_str!("../../assets/sim/rossler.rhai")),
    ("hodgepodge", include_str!("../../assets/sim/hodgepodge.rhai")),
    ("cyclic_ca", include_str!("../../assets/sim/cyclic_ca.rhai")),
    ("spatial_rps", include_str!("../../assets/sim/spatial_rps.rhai")),
    ("ising", include_str!("../../assets/sim/ising.rhai")),
    ("eden", include_str!("../../assets/sim/eden.rhai")),
    ("ballistic_deposition", include_str!("../../assets/sim/ballistic_deposition.rhai")),
    ("percolation", include_str!("../../assets/sim/percolation.rhai")),
    ("invasion_percolation", include_str!("../../assets/sim/invasion_percolation.rhai")),
    ("wolfram_eca", include_str!("../../assets/sim/wolfram_eca.rhai")),
    ("packard_snowflake", include_str!("../../assets/sim/packard_snowflake.rhai")),
    ("sandpile", include_str!("../../assets/sim/sandpile.rhai")),
    ("swift_hohenberg", include_str!("../../assets/sim/swift_hohenberg.rhai")),
    ("cahn_hilliard", include_str!("../../assets/sim/cahn_hilliard.rhai")),
    ("kobayashi", include_str!("../../assets/sim/kobayashi.rhai")),
    ("snowfake", include_str!("../../assets/sim/snowfake.rhai")),
    // The field's range and a repeated pass (step 9a).
    ("dbm", include_str!("../../assets/sim/dbm.rhai")),
    ("fingering", include_str!("../../assets/sim/fingering.rhai")),
    // Kernel tables, the spectral discs, the drive and the signal (step
    // 9b).
    ("lenia", include_str!("../../assets/sim/lenia.rhai")),
    ("lattice4", include_str!("../../assets/sim/lattice4.rhai")),
    ("smoothlife", include_str!("../../assets/sim/smoothlife.rhai")),
    ("turing", include_str!("../../assets/sim/turing.rhai")),
    ("mccabe", include_str!("../../assets/sim/mccabe.rhai")),
    ("physarum", include_str!("../../assets/sim/physarum.rhai")),
    ("dla", include_str!("../../assets/sim/dla.rhai")),
];

// ============================================================================
// A model's own time step and presets (P4 step 6)
// ============================================================================

/// A model's functions that run on the CPU (P4 steps 6 and 9b): `fn
/// max_dt(p)`, its stable time step, which a built-in writes as a
/// `dt_bound` closure; `fn kernel(p)`, its convolution table, a built-in's
/// `kernel` closure; `fn discs(p)`, the discs the spectral stage averages
/// over (`spectral_scales`). Plain Rhai -- the script's source as Rhai
/// runs it -- with the core language, its maths, arrays and maps,
/// `min`/`max`, an operation budget and nothing else.
struct CpuScript {
    source: String,
    /// The parameters that are ints (`round` of the slot), as the script
    /// reads them on the GPU.
    ints: Vec<String>,
}

fn cpu_engine() -> rhai::Engine {
    use rhai::packages::Package;
    let mut engine = rhai::Engine::new_raw();
    engine.register_global_module(rhai::packages::CorePackage::new().as_shared_module());
    engine.register_global_module(rhai::packages::BasicMathPackage::new().as_shared_module());
    engine.register_global_module(rhai::packages::LogicPackage::new().as_shared_module());
    engine.register_global_module(rhai::packages::BasicArrayPackage::new().as_shared_module());
    engine.register_global_module(rhai::packages::BasicMapPackage::new().as_shared_module());
    // A 65 x 65 table twice over is a few hundred thousand operations.
    engine.set_max_operations(20_000_000);
    engine
}

/// The parameters as `p`: each at `value(name)`, an int where the script
/// reads one.
fn params_map(params: &[SimParamDef], ints: &[String], value: impl Fn(&str) -> f32) -> rhai::Map {
    let mut map = rhai::Map::new();
    for p in params {
        let v = value(p.name);
        let v = if ints.iter().any(|n| n == p.name) { rhai::Dynamic::from_int(v.round() as rhai::INT) } else { rhai::Dynamic::from_float(f64::from(v)) };
        map.insert(p.name.into(), v);
    }
    map
}

/// `f(p)`, the script's function.
fn call_cpu(engine: &rhai::Engine, ast: &rhai::AST, f: &str, p: rhai::Map) -> Result<rhai::Dynamic, String> {
    call_cpu_with(engine, ast, f, (p,))
}

/// `f(args)`, the script's function.
fn call_cpu_with(engine: &rhai::Engine, ast: &rhai::AST, f: &str, args: impl rhai::FuncArgs) -> Result<rhai::Dynamic, String> {
    let options = rhai::CallFnOptions::new().eval_ast(false);
    engine.call_fn_with_options(options, &mut rhai::Scope::new(), ast, f, args).map_err(|e| format!("`{f}`: {e}"))
}

/// `agent_count(p, w, h)`'s result: a number of agents, at least one and
/// at most the engine's ceiling (P4 step 9d).
fn count_of(r: &rhai::Dynamic) -> Result<u32, String> {
    let n = number(r).filter(|n| n.is_finite()).ok_or_else(|| format!("`agent_count(p, w, h)` gives a number, not {}", r.type_name()))?;
    Ok(n.clamp(1.0, f64::from(super::MAX_AGENTS)) as u32)
}

fn number(v: &rhai::Dynamic) -> Option<f64> {
    v.as_float().ok().or_else(|| v.as_int().ok().map(|i| i as f64))
}

/// `max_dt(p)`'s result: a number.
fn dt_of(r: &rhai::Dynamic) -> Result<f64, String> {
    number(r).ok_or_else(|| format!("`max_dt(p)` gives a number, not {}", r.type_name()))
}

/// `kernel(p)`'s result as the table: `#{ radius, weights }`, one block of
/// `(2 radius + 1)^2` weights or two (a model that gathers two kernels,
/// row-major from -radius to radius), finite.
fn kernel_of(r: &rhai::Dynamic) -> Result<super::SimKernel, String> {
    let what = "`kernel(p)` gives `#{ radius, weights }`: a radius in cells and one or two blocks of (2 radius + 1)^2 weights";
    let map = r.clone().try_cast::<rhai::Map>().ok_or(what)?;
    let radius = map.get("radius").and_then(|v| v.as_int().ok()).filter(|r| (1..=i64::from(super::MAX_KERNEL_RADIUS)).contains(r)).ok_or(what)? as u32;
    let weights = map.get("weights").and_then(|v| v.clone().try_cast::<rhai::Array>()).ok_or(what)?;
    let weights: Vec<f32> = weights.iter().map(|w| number(w).map(|v| v as f32)).collect::<Option<_>>().ok_or(what)?;
    let taps = (2 * radius as usize + 1).pow(2);
    if weights.is_empty() || weights.len() % taps != 0 || weights.len() > 2 * taps || weights.iter().any(|w| !w.is_finite()) {
        return Err(format!("{what}; this one has {} weights for radius {radius}", weights.len()));
    }
    Ok(super::SimKernel { radius, weights })
}

/// `discs(p)`'s result as the spectral stage's discs: a list of `#{ ra,
/// rb, stretch, angle, annulus }`, `rb` (0), `stretch` (1), `angle` (0)
/// and `annulus` (false) optional.
fn discs_of(r: &rhai::Dynamic) -> Result<Vec<super::spectral::SpectralScale>, String> {
    let what = "`discs(p)` gives a list of `#{ ra, rb, stretch, angle, annulus }`";
    let list = r.clone().try_cast::<rhai::Array>().ok_or(what)?;
    list.iter()
        .map(|d| {
            let m = d.clone().try_cast::<rhai::Map>().ok_or(what)?;
            let f = |k: &str, default: f64| m.get(k).map_or(Some(default), number).map(|v| v as f32);
            let disc = super::spectral::SpectralScale {
                ra: m.get("ra").and_then(number).ok_or(what)? as f32,
                rb: f("rb", 0.0).ok_or(what)?,
                stretch: f("stretch", 1.0).ok_or(what)?,
                angle: f("angle", 0.0).ok_or(what)?,
                annulus: m.get("annulus").map_or(Some(false), |v| v.as_bool().ok()).ok_or(what)?,
            };
            Ok(disc)
        })
        .collect()
}

/// `f` with the model `name`'s script parsed on this thread (an engine
/// and its AST are not shared between threads: rhai is built without
/// `sync`), parsed again when its source changes.
fn with_script<T>(name: &str, f: impl FnOnce(&rhai::Engine, &rhai::AST, &CpuScript) -> T) -> Option<T> {
    use std::cell::RefCell;
    thread_local! {
        static PARSED: RefCell<HashMap<String, (Arc<CpuScript>, rhai::Engine, rhai::AST)>> = RefCell::new(HashMap::new());
    }
    let script = registry().read().ok().and_then(|r| r.cpu.get(name).cloned())?;
    PARSED.with(|parsed| {
        let mut parsed = parsed.borrow_mut();
        if parsed.get(name).is_none_or(|(s, ..)| !Arc::ptr_eq(s, &script)) {
            let engine = cpu_engine();
            let ast = engine.compile(&script.source).ok()?;
            parsed.insert(name.to_string(), (script.clone(), engine, ast));
        }
        let (_, engine, ast) = &parsed[name];
        Some(f(engine, ast, &script))
    })
}

/// The `dt_bound` of a model with a `fn max_dt(p)`: its script, found by
/// the model's name. Not a number when it cannot run, which `max_dt_for`
/// reads as "the static cap".
fn script_dt_bound(p: &super::Params) -> f32 {
    let model = p.model();
    let r = with_script(model.name, |e, ast, s| call_cpu(e, ast, "max_dt", params_map(model.parameters, &s.ints, |n| p.get(n))).and_then(|r| dt_of(&r)));
    r.and_then(Result::ok).map_or(f32::NAN, |v| v as f32)
}

/// A CPU function's result at the parameters in force, cached on their
/// values: the renderer asks for a kernel every batch, and a table is
/// thousands of taps of Rhai. A failure is logged once and gives
/// `fallback`.
fn cached<T: Clone + 'static>(cache: &'static std::thread::LocalKey<std::cell::RefCell<HashMap<String, (Vec<u32>, T)>>>, model: &ModelDef, values: &dyn Fn(&str) -> f32, f: &str, read: fn(&rhai::Dynamic) -> Result<T, String>, fallback: impl FnOnce() -> T) -> T {
    let key: Vec<u32> = model.parameters.iter().map(|p| values(p.name).to_bits()).collect();
    if let Some(hit) = cache.with(|c| c.borrow().get(model.name).filter(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return hit;
    }
    let r = with_script(model.name, |e, ast, s| call_cpu(e, ast, f, params_map(model.parameters, &s.ints, values)).and_then(|r| read(&r)));
    let value = match r {
        Some(Ok(v)) => v,
        other => {
            cpu_failed(model.name, f, other.and_then(Result::err));
            fallback()
        }
    };
    cache.with(|c| c.borrow_mut().insert(model.name.to_string(), (key, value.clone())));
    value
}

/// The `count` of a model with agents (P4 step 9d): its
/// `fn agent_count(p, w, h)` at the parameters in force and the grid's
/// size; one agent when the script fails. Not cached: it runs when the
/// population is sized, not per step.
fn script_agent_count(p: &super::Params, w: u32, h: u32) -> u32 {
    let model = p.model();
    let r = with_script(model.name, |e, ast, s| {
        let map = params_map(model.parameters, &s.ints, |n| p.get(n));
        call_cpu_with(e, ast, "agent_count", (map, rhai::INT::from(w), rhai::INT::from(h))).and_then(|r| count_of(&r))
    });
    match r {
        Some(Ok(n)) => n,
        other => {
            cpu_failed(model.name, "agent_count", other.and_then(Result::err));
            1
        }
    }
}

/// Log a CPU function's failure once (a run asks again every batch).
fn cpu_failed(model: &str, f: &str, e: Option<String>) {
    let e = e.unwrap_or_else(|| "its script is not registered".to_string());
    if registry().write().is_ok_and(|mut r| r.failed.insert(format!("{model}:{f}"), e.clone()).is_none()) {
        log::warn!("The simulation model `{model}`'s `{f}` failed: {e}");
    }
}

/// The `kernel` of a model with a `fn kernel(p)`: its table at the
/// parameters in force; a zero table when the script fails.
fn script_kernel(p: &super::Params) -> super::SimKernel {
    thread_local! {
        static KERNELS: std::cell::RefCell<HashMap<String, (Vec<u32>, super::SimKernel)>> = std::cell::RefCell::new(HashMap::new());
    }
    cached(&KERNELS, p.model(), &|n| p.get(n), "kernel", kernel_of, || super::SimKernel { radius: 1, weights: vec![0.0; 9] })
}

impl Clone for super::SimKernel {
    fn clone(&self) -> Self {
        super::SimKernel { radius: self.radius, weights: self.weights.clone() }
    }
}

/// Whether `param` of the Rhai model `name` is slow (`INFO.slow`): what
/// `sim::SLOW_PARAMS` says of a built-in's.
pub fn is_slow_param(name: &str, param: &str) -> bool {
    registry().read().is_ok_and(|r| r.slow.get(name).is_some_and(|s| s.iter().any(|p| p == param)))
}

/// The per-scale warps of a Rhai model with `INFO.scale_warps` (P4 step
/// 9c).
pub fn scale_warps(name: &str) -> Option<&'static super::ScaleWarps> {
    registry().read().ok()?.warps.get(name).copied()
}

/// A script's `INFO.scale_warps` (P4 step 9c): the parameter counting the
/// live rows and the rows, each row's five `s{i}_warp_*` in `PARAMS`.
fn scale_warps_of(source: &str, name: &str, parameters: &[SimParamDef]) -> Result<Option<super::ScaleWarps>, String> {
    let Some(v) = lang::rhai::info_json(source, "scale_warps") else { return Ok(None) };
    let what = "`INFO.scale_warps` is `#{ rows_param, rows }`";
    let m = v.as_object().ok_or(what)?;
    if let Some(o) = m.keys().find(|k| !matches!(k.as_str(), "rows_param" | "rows")) {
        return Err(format!("`INFO.scale_warps` has `rows_param` and `rows`, not `{o}`"));
    }
    let rows_param = m.get("rows_param").and_then(serde_json::Value::as_str).filter(|n| parameters.iter().any(|p| p.name == *n));
    let rows_param = rows_param.ok_or("`INFO.scale_warps.rows_param` names a parameter of `PARAMS`, the live rows")?;
    let rows = m.get("rows").and_then(serde_json::Value::as_u64).filter(|n| (1..=8).contains(n)).ok_or("`INFO.scale_warps.rows` is 1 to 8")?;
    let w = super::ScaleWarps { model: leak(name), rows_param: leak(rows_param), rows: rows as usize };
    for r in 0..w.rows {
        for f in super::SCALE_WARP_FIELDS {
            let n = w.param(r, f);
            if !parameters.iter().any(|p| p.name == n) {
                return Err(format!("`INFO.scale_warps` has {} rows: `PARAMS` needs `{n}`", w.rows));
            }
        }
    }
    Ok(Some(w))
}

/// The spectral stage's discs of a Rhai model with a `fn discs(p)` (P4
/// step 9b), at `params`, the defaults filling what they leave out; none
/// for a model without.
pub fn discs(name: &str, params: &std::collections::BTreeMap<String, f32>) -> Option<Vec<super::spectral::SpectralScale>> {
    thread_local! {
        static DISCS: std::cell::RefCell<HashMap<String, (Vec<u32>, Vec<super::spectral::SpectralScale>)>> = std::cell::RefCell::new(HashMap::new());
    }
    let m = model(name).filter(|m| m.has(ModelFeature::ExactDiscs) || m.has(ModelFeature::NeedsPyramid))?;
    let p = super::Params { model: m, map: params };
    Some(cached(&DISCS, m, &|n| p.get(n), "discs", discs_of, Vec::new))
}

/// A model's `INFO.presets` as the built-ins' are: each a map of `name`,
/// `title`, `params` (by name, of `PARAMS`), `steps`, `dt`, and the
/// config's `boundary`, `init`, `coloring` (a built-in's name),
/// `coloring_params`, `matte` and `warp` as a `.fflame` spells them.
fn presets_of(source: &str, params: &[SimParamDef], default_steps: u32) -> Result<Vec<SimPreset>, String> {
    use serde_json::Value;
    let Some(list) = lang::rhai::info_json(source, "presets") else { return Ok(Vec::new()) };
    let Value::Array(list) = list else { return Err("`INFO.presets` is a list of maps".to_string()) };
    let pairs = |v: &Value, what: &str, known: &dyn Fn(&str) -> bool| -> Result<&'static [(&'static str, f32)], String> {
        let Value::Object(m) = v else { return Err(format!("a preset's `{what}` is a map of name to number")) };
        let mut out = Vec::new();
        for (k, x) in m {
            if !known(k) {
                return Err(format!("a preset's `{what}` names `{k}`, which is not one"));
            }
            let x = x.as_f64().ok_or_else(|| format!("a preset's `{what}.{k}` is a number"))?;
            out.push((leak(k.clone()), x as f32));
        }
        Ok(Box::leak(out.into_boxed_slice()))
    };
    let typed = |v: &Value, what: &str| format!("a preset's `{what}` is not one the config reads: {v}");
    let mut out = Vec::new();
    for item in &list {
        let Value::Object(m) = item else { return Err("a preset is a map".to_string()) };
        let text = |k: &str| m.get(k).and_then(Value::as_str).map(str::to_string);
        let name = text("name").ok_or("a preset has a `name`")?;
        let mut preset = SimPreset {
            name: leak(name.clone()),
            display_name: leak(text("title").unwrap_or(name)),
            params: &[],
            steps: default_steps,
            dt: None,
            boundary: Default::default(),
            init: None,
            coloring: None,
            coloring_params: &[],
            matte: None,
            warp: None,
        };
        for (k, v) in m {
            match k.as_str() {
                "name" | "title" => {}
                "params" => preset.params = pairs(v, "params", &|n| params.iter().any(|p| p.name == n))?,
                "steps" => preset.steps = v.as_u64().and_then(|s| u32::try_from(s).ok()).ok_or("a preset's `steps` is a whole number")?,
                "dt" => preset.dt = Some(v.as_f64().ok_or("a preset's `dt` is a number")? as f32),
                "boundary" => preset.boundary = serde_json::from_value(v.clone()).map_err(|_| typed(v, "boundary"))?,
                "init" => preset.init = Some(serde_json::from_value(v.clone()).map_err(|_| typed(v, "init"))?),
                "matte" => preset.matte = Some(serde_json::from_value(v.clone()).map_err(|_| typed(v, "matte"))?),
                "warp" => preset.warp = Some(serde_json::from_value(v.clone()).map_err(|_| typed(v, "warp"))?),
                "coloring" => {
                    let c = v.as_str().ok_or("a preset's `coloring` names a colouring")?;
                    let def = super::COLORINGS.iter().find(|d| d.name == c).ok_or_else(|| format!("a preset's `coloring` is a built-in's name, not `{c}`"))?;
                    preset.coloring = Some(def.name);
                }
                "coloring_params" => {
                    let c = m.get("coloring").and_then(Value::as_str).and_then(|c| super::COLORINGS.iter().find(|d| d.name == c));
                    let known = |n: &str| c.is_some_and(|d| d.parameters.iter().any(|p| p.name == n));
                    preset.coloring_params = pairs(v, "coloring_params", &known)?;
                }
                o => {
                    return Err(format!(
                        "a preset has `name`, `title`, `params`, `steps`, `dt`, `boundary`, `init`, `coloring`, `coloring_params`, `matte` and `warp`; not `{o}`"
                    ))
                }
            }
        }
        out.push(preset);
    }
    Ok(out)
}

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
    let mut min_max = false;
    let mut program = None;
    for n in 0..passes {
        let p = lang::rhai::sim_model(source, SimFn::Step(n)).map_err(|d| first(&d))?;
        let w = lang::wgsl::sim_model(&p, SimFn::Step(n))?;
        wgsl.push_str(&w.body);
        rng |= w.rng;
        min_max |= w.min_max;
        program.get_or_insert(p);
    }
    let program = program.ok_or("a simulation model has `fn cell_step`")?;
    // Its agents (P4 step 9d): `agent_seed`, `agent_step` and
    // `agent_step2`, compiled into the agent template; their number
    // `fn agent_count(p, w, h)`, run on the CPU.
    let has_fn = |f: &str| lang::rhai::has_function(source, f);
    let agent_passes = lang::rhai::sim_agent_passes(source);
    let mut agent_wgsl = String::new();
    if agent_passes > 0 {
        if !has_fn("agent_seed") {
            return Err("a model with agents has `fn agent_seed(p, ctx)`: each agent's first record, `vec4(x, y, heading, state)`".to_string());
        }
        if !has_fn("agent_count") {
            return Err("a model with agents has `fn agent_count(p, w, h)`: how many, on a grid w by h".to_string());
        }
        for f in std::iter::once(SimFn::AgentSeed).chain((0..agent_passes).map(SimFn::Agent)) {
            let p = lang::rhai::sim_model(source, f).map_err(|d| first(&d))?;
            let w = lang::wgsl::sim_model(&p, f)?;
            agent_wgsl.push_str(&w.body);
            rng |= w.rng;
            min_max |= w.min_max;
        }
    } else if has_fn("agent_seed") || has_fn("agent_count") {
        return Err("a model's agents move in `fn agent_step(a, p, ctx)`".to_string());
    }
    if agent_passes == 0 && wgsl.contains("sim_take_deposit(") {
        return Err("the step takes the agents' deposit (`take_deposit`): it needs agents, `fn agent_step(a, p, ctx)`".to_string());
    }
    // Its step reads the spectral stage's disc averages (P4 step 9b).
    // And the pyramid, its averages and spectral differences (step 9c);
    // the memory.
    let pyramid = wgsl.contains("pyr_prepare();");
    let exact_discs = wgsl.contains("pyr_diff_cell(") || (wgsl.contains("pyr_exact()") && !pyramid);
    let memory = wgsl.contains("sim_mem_read(") || wgsl.contains("sim_mem_write(") || wgsl.contains("sim_mem_count()");
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
        (min_max, ModelFeature::NeedsMinMax),
        (wgsl.contains("sim_drive("), ModelFeature::TakesDrive),
        (lang::rhai::info_bool(source, "publishes_signal") == Some(true), ModelFeature::PublishesSignal),
        (exact_discs, ModelFeature::ExactDiscs),
        (pyramid, ModelFeature::NeedsPyramid),
        (memory, ModelFeature::Memory),
        (agent_passes > 0, ModelFeature::NeedsAgents),
        (lang::rhai::info_bool(source, "time_step") == Some(false), ModelFeature::NoTimeStep),
        (lang::rhai::info_bool(source, "settles") == Some(false), ModelFeature::NeverStills),
    ] {
        if on {
            features.push(f);
        }
    }
    let parameters: &'static [SimParamDef] = Box::leak(sim_params(&program)?.into_boxed_slice());
    // A pass run several times a step, its count a parameter (P4 step
    // 9a): `#{ pass: 2, param: "relax" }`, the pass numbered as the
    // functions are (`cell_step` is 1).
    let repeat = match lang::rhai::info_json(source, "repeat") {
        None => None,
        Some(serde_json::Value::Object(m)) => {
            let pass = m.get("pass").and_then(serde_json::Value::as_u64).filter(|n| (1..=u64::from(passes)).contains(n));
            let pass = pass.ok_or_else(|| format!("`INFO.repeat.pass` is a pass, 1 to {passes}"))?;
            let param = m.get("param").and_then(serde_json::Value::as_str).filter(|n| parameters.iter().any(|p| p.name == *n));
            let param = param.ok_or("`INFO.repeat.param` names a parameter of `PARAMS`, the count")?;
            if let Some(o) = m.keys().find(|k| !matches!(k.as_str(), "pass" | "param")) {
                return Err(format!("`INFO.repeat` has `pass` and `param`, not `{o}`"));
            }
            Some((pass as u32 - 1, leak(param)))
        }
        Some(_) => return Err("`INFO.repeat` is `#{ pass, param }`".to_string()),
    };
    let default_steps = number("steps").map_or(1000, |v| v.max(1.0) as u32);
    // Its CPU functions, each run once here at the defaults: a script
    // that cannot give a time step, a table or its discs is a message
    // now, not a fallback at the first step.
    let has = |f: &str| lang::rhai::has_function(source, f);
    let cpu = if has("max_dt") || has("kernel") || has("discs") || has("agent_count") {
        let engine = cpu_engine();
        let ast = engine.compile(source).map_err(|e| format!("its CPU functions: {e}"))?;
        Some((engine, ast, int_params(&program)))
    } else {
        None
    };
    let at_defaults_map = || {
        let (_, _, ints) = cpu.as_ref().expect("a CPU function");
        params_map(parameters, ints, |n| parameters.iter().find(|p| p.name == n).map_or(0.0, |p| p.default))
    };
    let at_defaults = |f: &str| -> Result<rhai::Dynamic, String> {
        let (engine, ast, _) = cpu.as_ref().expect("a CPU function");
        call_cpu(engine, ast, f, at_defaults_map())
    };
    let dt_bound = if has("max_dt") {
        let at = dt_of(&at_defaults("max_dt")?)?;
        if !(at.is_finite() && at > 0.0) {
            return Err(format!("`max_dt(p)` at the defaults gives {at}: a time step is a positive number"));
        }
        Some(script_dt_bound as fn(&super::Params) -> f32)
    } else {
        None
    };
    // The table its step gathers against (`tap(i)`), and the discs the
    // spectral stage averages for it (`disc(i)`).
    let reads_kernel = wgsl.contains("klut(") || wgsl.contains("params.kernel_radius");
    let kernel = if has("kernel") {
        kernel_of(&at_defaults("kernel")?)?;
        Some(script_kernel as fn(&super::Params) -> super::SimKernel)
    } else if reads_kernel {
        return Err("the step reads a kernel table (`tap`, `ctx.kernel_radius`): it needs `fn kernel(p)`".to_string());
    } else {
        None
    };
    if has("discs") {
        discs_of(&at_defaults("discs")?)?;
    } else if exact_discs || (pyramid && wgsl.contains("pyr_diff(")) {
        return Err("the step reads the spectral discs (`disc`, `pyr_diff`, `ctx.exact_discs`): it needs `fn discs(p)`".to_string());
    }
    let agents = if agent_passes > 0 {
        count_of(&call_cpu_with(&cpu.as_ref().expect("a CPU function").0, &cpu.as_ref().expect("a CPU function").1, "agent_count", (at_defaults_map(), 512 as rhai::INT, 512 as rhai::INT))?)?;
        Some(super::AgentDef { count: script_agent_count, passes: u32::from(agent_passes), wgsl: leak(agent_wgsl) })
    } else {
        None
    };
    if memory && !parameters.iter().any(|p| p.name == "memory") {
        return Err("the step reads its memory: it needs a `memory` parameter, which turns it on above 0".to_string());
    }
    // A measure pass, the first, run while its parameter is above 0, its
    // scratch slices those the step writes (step 9c).
    let measure = match lang::rhai::info_json(source, "measure") {
        None => None,
        Some(serde_json::Value::Object(m)) => {
            let param = m.get("param").and_then(serde_json::Value::as_str).filter(|n| parameters.iter().any(|p| p.name == *n));
            let param = param.ok_or("`INFO.measure.param` names a parameter of `PARAMS`, which turns the pass on above 0")?;
            let slices = m.get("slices").and_then(serde_json::Value::as_u64).filter(|n| (1..=4).contains(n));
            let slices = slices.ok_or("`INFO.measure.slices` is the scratch's slices, 1 to 4")? as u32;
            if let Some(o) = m.keys().find(|k| !matches!(k.as_str(), "param" | "slices")) {
                return Err(format!("`INFO.measure` has `param` and `slices`, not `{o}`"));
            }
            Some(super::MeasurePass { param: leak(param), slices })
        }
        Some(_) => return Err("`INFO.measure` is `#{ param, slices }`".to_string()),
    };
    for s in lang::rhai::info_strings(source, "slow") {
        if !parameters.iter().any(|p| p.name == s) {
            return Err(format!("`INFO.slow` names `{s}`, which is not in `PARAMS`"));
        }
    }
    if measure.is_none() && wgsl.contains("sim_scratch_") {
        return Err("the step reads a measure pass's scratch (`scratch`, `ctx.scratch_on`): it needs `INFO.measure`".to_string());
    }
    scale_warps_of(source, name, parameters)?;
    let def = ModelDef {
        name: leak(name),
        state: if agents.is_some() {
            StateKind::Agents
        } else if lang::rhai::info_string(source, "state").as_deref() == Some("discrete") {
            StateKind::Discrete
        } else {
            StateKind::Continuous
        },
        display_name: leak(program.name.clone()),
        description: leak(lang::rhai::source_labels(source).map(|l| l.description.resolve()).unwrap_or_default()),
        features: Box::leak(features.into_boxed_slice()),
        parameters,
        presets: Box::leak(presets_of(source, parameters, default_steps)?.into_boxed_slice()),
        wgsl: leak(wgsl),
        passes: u32::from(passes),
        repeat,
        measure,
        max_dt: number("max_dt").map_or(default_dt, |v| v as f32),
        diffusion: Box::leak(diffusion.into_boxed_slice()),
        agents,
        kernel,
        dt_bound,
        wgsl_seed: leak(seed),
        default_steps,
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
        if let Some(a) = def.agents {
            for pass in 0..a.passes {
                lang::wgsl::validate_effect(&super::assembler::assemble_agents(&def, boundary, pass)).map_err(|e| format!("the compiler produced an invalid agent pass (a bug): {e}"))?;
            }
            lang::wgsl::validate_effect(&super::assembler::assemble_agent_seed(&def, boundary)).map_err(|e| format!("the compiler produced an invalid agent seed (a bug): {e}"))?;
        }
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
    let warps = scale_warps_of(source, name, def.parameters)?.map(|w| &*Box::leak(Box::new(w)));
    // Its CPU functions, which `with_script` finds by the model's name.
    let has_cpu = ["max_dt", "kernel", "discs", "agent_count"].iter().any(|f| lang::rhai::has_function(source, f));
    let cpu = has_cpu.then(|| {
        let ints = lang::rhai::sim_model(source, lang::rhai::SimFn::Step(0)).map(|p| int_params(&p)).unwrap_or_default();
        Arc::new(CpuScript { source: source.to_string(), ints })
    });
    if let Ok(mut r) = registry().write() {
        r.models.insert(name.to_string(), def);
        r.failed.remove(name);
        match cpu {
            Some(cpu) => r.cpu.insert(name.to_string(), cpu),
            None => r.cpu.remove(name),
        };
        match warps {
            Some(w) => r.warps.insert(name.to_string(), w),
            None => r.warps.remove(name),
        };
        r.slow.insert(name.to_string(), lang::rhai::info_strings(source, "slow"));
    }
    Ok(())
}

/// The names of a script's int parameters (and its choices and bools,
/// which are ints too): what `max_dt` is given as Rhai ints.
fn int_params(program: &lang::ir::Program) -> Vec<String> {
    program.params.iter().filter(|p| matches!(p.kind, ParamKind::Int | ParamKind::Enum | ParamKind::Bool)).map(|p| p.name.clone()).collect()
}

/// The registered model `name` with another kernel table, for the tests
/// (P4 step 9b): a kernel twin's step held against its built-in's alone,
/// on the built-in's own table -- a script builds its table in f64 where
/// a built-in builds it in f32, and a field run amplifies the last bit.
#[cfg(test)]
pub(crate) fn swap_kernel(name: &str, kernel: Option<fn(&super::Params) -> super::SimKernel>) {
    let def = model(name).expect("registered");
    let swapped: &'static ModelDef = Box::leak(Box::new(ModelDef { kernel, ..*def }));
    registry().write().expect("the registry").models.insert(name.to_string(), swapped);
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
        assert!(e.contains("INFO.agents") && e.contains("agent_step"), "{e}");
    }

    /// The framework stages a script asks for (P4 step 9c): the pyramid,
    /// its caches filled where a path first reads it and not on a path
    /// that does not; a measure pass and its scratch; the memory; per-scale
    /// warps -- each from what the step reads, and a message for each one
    /// half asked for. A helper takes the parameters and the context whole,
    /// and a fresh array is its local, not copied into one.
    #[test]
    fn a_model_asks_for_the_pyramid_scratch_and_memory() {
        let params = "const PARAMS = #{ variation: #{ value: 0.0, min: 0.0, max: 2.0 }, memory: #{ value: 0.0, min: 0.0, max: 1.0 } };
";
        let measure = "const INFO = #{ measure: #{ param: \"variation\", slices: 1 } };
";
        let steps = r#"
fn level(p, ctx) { pyr_level(p.variation + to_float(ctx.width) * 0.0) }
fn cell_step(cell, p, ctx) {
    let m = [0.0, 0.0];
    m[0] = pyr_sample(level(p, ctx), ctx.cell.re, ctx.cell.im);
    write_scratch(0, rgba(m[0], m[1], 0.0, 0.0));
    cell
}
fn cell_step2(cell, p, ctx) {
    let v = 0.0;
    if ctx.scratch_on == 1 {
        v = scratch(1, 0, 0).x;
    } else {
        v = pyr_sample(0.0, ctx.cell.re, ctx.cell.im);
    }
    write_memory(0, mix(memory(0), rgba(v, 0.0, 0.0, 0.0), p.memory));
    rgba(v, cell.y, 0.0, 0.0)
}
"#;
        let def = compile_model(&format!("{params}{measure}{steps}"), "custom-stages").unwrap_or_else(|e| panic!("{e}"));
        assert!(def.has(ModelFeature::NeedsPyramid) && def.has(ModelFeature::Memory) && !def.has(ModelFeature::ExactDiscs));
        assert_eq!(def.measure.map(|m| (m.param, m.slices)), Some(("variation", 1)));
        let (one, two) = def.wgsl.split_at(def.wgsl.find("fn sim_step2").expect("two passes"));
        assert_eq!(two.matches("pyr_prepare();").count(), 1, "{two}");
        assert!(two.find("pyr_prepare();") > two.find("sim_scratch_read("), "the scratch's path pays nothing for the pyramid:
{two}");
        assert!(one.contains("pyr_prepare();"));
        assert_eq!(one.matches("array<f32, 2>").count(), 1, "the literal is the local:
{one}");
        // Half asked for: each a message.
        for (src, says) in [
            (format!("{params}{steps}"), "needs `INFO.measure`"),
            (format!("{}{measure}{}", params.replace("memory:", "keep:"), steps.replace("p.memory", "p.keep")), "a `memory` parameter"),
            (format!("{params}{measure}{}", steps.replace("pyr_sample(0.0,", "pyr_diff(0,")), "fn discs(p)"),
            (format!("{params}{}{steps}", measure.replace("\"variation\"", "\"radius\"")), "names a parameter"),
            (format!("{params}{}{steps}", measure.replace("measure:", "scale_warps: #{ rows_param: \"variation\", rows: 1 }, measure:")), "needs `s0_warp_zoom`"),
            (format!("{params}{}{steps}", measure.replace("measure:", "slow: [\"nope\"], measure:")), "`INFO.slow` names `nope`"),
            (format!("{params}{measure}{steps}fn cell_seed(inside, noise, p, ctx) {{ write_memory(0, rgba(0.0, 0.0, 0.0, 0.0)); rgba(inside, 0.0, 0.0, 0.0) }}
"), "a step's"),
        ] {
            let e = compile_model(&src, "custom-half").expect_err(says);
            assert!(e.contains(says), "{says}: {e}");
        }
    }

    /// A model with agents (P4 step 9d): `agent_seed`, `agent_step` and
    /// `agent_step2` compiled into the agent template, their number from
    /// `agent_count(p, w, h)` on the CPU, clamped; `NeedsAgents`,
    /// `StateKind::Agents`, `NeedsRng` from an agent's draws -- and a
    /// message for each half of one, and for each call in the wrong
    /// function.
    #[test]
    fn a_model_with_agents_compiles() {
        let params = "const PARAMS = #{ amount: #{ value: 2.0, min: 0.0, max: 4.0 } };
";
        let step = "fn cell_step(cell, p, ctx) { rgba(take_deposit(), cell.y, 0.0, 0.0) }
";
        let agents = r#"
fn agent_seed(p, ctx) { rgba(rand(1) * to_float(ctx.width), to_float(ctx.agent), 0.0, 0.0) }
fn agent_step(a, p, ctx) {
    let x = to_int(floor(a.x + 0.5));
    let y = to_int(floor(a.y + 0.5));
    if read_at(x, y).x < 1.0 && ctx.periodic == 1 {
        claim(x, y);
    }
    a
}
fn agent_step2(a, p, ctx) {
    let x = to_int(floor(a.x + 0.5));
    let y = to_int(floor(a.y + 0.5));
    if claim_check(x, y) {
        deposit(x, y, p.amount);
    }
    rgba(a.x + rand(2), a.y, a.z, a.w)
}
fn agent_count(p, w, h) { to_float(w * h) * p.amount }
"#;
        let def = compile_model(&format!("{params}{step}{agents}"), "custom-agents").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(def.state, StateKind::Agents);
        assert!(def.has(ModelFeature::NeedsAgents) && def.has(ModelFeature::NeedsRng));
        let a = def.agents.expect("agents");
        assert_eq!(a.passes, 2);
        for want in ["fn sim_agent_seed(i: u32) -> SimAgent", "fn sim_agent(agent: SimAgent, i: u32) -> SimAgent", "fn sim_agent2(", "agent_claim(", "agent_claim_check(", "agent_deposit(", "agent_rand(i, "] {
            assert!(a.wgsl.contains(want), "{want} in
{}", a.wgsl);
        }
        register_model_as(&format!("{params}{step}{agents}"), "custom-agents").unwrap();
        let def = model("custom-agents").expect("registered");
        let at = |amount: f32| [("amount".to_string(), amount)].into();
        assert_eq!((a.count)(&def.params_view(&at(2.0)), 10, 20), 400);
        assert_eq!((a.count)(&def.params_view(&at(0.0)), 10, 20), 1, "at least one");
        assert_eq!((a.count)(&def.params_view(&at(4.0)), 4096, 4096), crate::sim::MAX_AGENTS, "at most the ceiling");
        // Half a model with agents, and calls in the wrong function.
        let no_step = agents.replace("fn agent_step(", "fn agent_move(").replace("fn agent_step2(", "fn agent_move2(");
        for (src, says) in [
            (format!("{params}{step}{}", agents.replace("fn agent_seed(", "fn agent_first(")), "fn agent_seed"),
            (format!("{params}{step}{}", agents.replace("fn agent_count(", "fn agent_number(")), "fn agent_count"),
            (format!("{params}{step}{no_step}"), "agent_step"),
            (format!("{params}{step}"), "take_deposit"),
            (format!("{params}{step}{}", agents.replace("read_at(x, y)", "read(0, 0)")), "read_at(x, y)"),
            (format!("{params}{step}{}", agents.replace("rand(1) * to_float(ctx.width)", "ctx.cell.re")), "no cell"),
            (format!("{params}{}{agents}", step.replace("{ rgba(", "{ claim(0, 0); rgba(")), "an agent's"),
            (format!("{params}{}{agents}", step.replace("cell.y", "to_float(ctx.agent)")), "an agent's index"),
            (format!("{params}{step}{}", agents.replace("claim(x, y);", "write_memory(0, a);")), "a step's"),
            (format!("{params}{step}{}", agents.replace("fn agent_seed(p, ctx)", "fn agent_seed(i, p, ctx)")), "`agent_seed()`"),
        ] {
            let e = compile_model(&src, "custom-half").expect_err(says);
            assert!(e.contains(says), "{says}: {e}");
        }
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
        for want in ["bitcast<i32>(sim_step_index())", "x.m0.x", "x.m1.w", "break;", "bitcast<i32>(params.seed_hi)"] {
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
        // A helper's early return leaves the helper (P7 step 1).
        let helper = compile_model("fn helper(x) { if x > 0.0 { return x; } x }
fn cell_step(cell) { vec4(helper(cell.x), 0.0, 0.0, 0.0) }", "custom-x");
        assert!(helper.is_ok_and(|d| d.wgsl.contains("loop {")));
        // A helper's own loop breaks (Rhai's parser refuses a `break`
        // outside one).
        let own = "fn first(x) {\n    let n = 0.0;\n    for i in 0..8 { n += x; if n > 2.0 { break; } }\n    n\n}\nfn cell_step(cell) { vec4(first(cell.x), 0.0, 0.0, 0.0) }";
        assert!(compile_model(own, "custom-x").unwrap_or_else(|e| panic!("{e}")).wgsl.contains("break;"));
        let formula = "fn start(pixel, p) { #{ z: pixel } }\nfn step(s, p) {\n    for i in 0..3 { if i == 1 { break; } }\n    s\n}";
        // A formula's loops too, since P7 step 1.
        assert!(lang::rhai::program(formula).is_ok());
    }

    /// A model's own time step and presets (P4 step 6): `fn max_dt(p)`
    /// runs on the CPU at the parameters in force, as a built-in's
    /// `dt_bound` does, under the static cap and the stability margin; its
    /// `INFO.presets` read as the built-ins' are; and what they refuse.
    #[test]
    fn a_model_has_its_own_time_step_and_presets() {
        use crate::config::sim::{SimBoundary, SimInit};
        let src = "const PARAMS = #{ d: #{ value: 0.5, min: 0.0, max: 2.0 }, n: #{ value: 3, min: 1, max: 9 } };\n\
            const INFO = #{ dt: 0.2, max_dt: 0.4, steps: 500, presets: [\n    \
            #{ name: \"slow\", title: \"Slow\", params: #{ d: 0.1 }, steps: 900, dt: 0.1, boundary: \"clamp\",\n       \
            init: #{ blobs: #{ count: 6, radius: 24 } }, coloring: \"channel\", coloring_params: #{ channel: 1.0, scale: -3.0 } },\n    \
            #{ name: \"plain\" },\n] };\n\
            fn max_dt(p) { 0.5 / max(p.d, 0.25) + to_float(p.n) * 0.0 }\n\
            fn cell_step(cell, p, ctx) { cell }\n";
        register_model_as(src, "custom-dt-test").unwrap_or_else(|e| panic!("{e}"));
        let def = model("custom-dt-test").expect("registered");
        let at = |d: f32| def.max_dt_for(&[("d".to_string(), d)].into());
        assert_eq!(at(2.0), super::super::STABILITY_MARGIN * 0.25, "its own bound at d = 2");
        assert_eq!(at(0.1), 0.4, "under the static cap");
        assert_eq!(def.presets.len(), 2);
        let slow = &def.presets[0];
        assert_eq!((slow.name, slow.display_name, slow.params, slow.steps, slow.dt), ("slow", "Slow", &[("d", 0.1f32)][..], 900, Some(0.1)));
        assert_eq!((slow.boundary, slow.init, slow.coloring), (SimBoundary::Clamp, Some(SimInit::Blobs { count: 6, radius: 24 }), Some("channel")));
        assert_eq!(slow.coloring_params, &[("channel", 1.0), ("scale", -3.0)][..]);
        let plain = &def.presets[1];
        assert_eq!((plain.display_name, plain.steps, plain.boundary, plain.init), ("plain", 500, SimBoundary::default(), None));

        let refused = |from: &str, to: &str, why: &str| {
            let e = compile_model(&src.replace(from, to), "custom-x").unwrap_err();
            assert!(e.contains(why), "{why}: {e}");
        };
        refused("#{ name: \"plain\" }", "#{ name: \"plain\", speed: 2 }", "not `speed`");
        refused("params: #{ d: 0.1 }", "params: #{ e: 0.1 }", "names `e`");
        refused("coloring: \"channel\"", "coloring: \"chanel\"", "not `chanel`");
        refused("boundary: \"clamp\"", "boundary: \"sideways\"", "`boundary` is not one");
        refused("0.5 / max(p.d, 0.25)", "0.0 * p.d", "a time step is a positive number");
    }

    /// Whether a built-in model is one a script can be yet (P4 steps 7
    /// and 9): none of the framework's stages a script cannot ask for --
    /// the field's range and a repeated pass it can (9a), kernel tables,
    /// the spectral discs and the coupled drive (9b), the pyramid, a
    /// measure pass and the memory (9c).
    pub(crate) fn in_class(_: &ModelDef) -> bool {
        true
    }

    /// JSON as a Rhai literal: maps `#{ }`, an f32's number as its
    /// shortest spelling with a point, an int as an int.
    fn rhai_literal(v: &serde_json::Value) -> String {
        use serde_json::Value;
        match v {
            Value::Null => "()".into(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
            Value::Number(n) => float(n.as_f64().unwrap_or_default()),
            Value::String(s) => format!("{s:?}"),
            Value::Array(a) => format!("[{}]", a.iter().map(rhai_literal).collect::<Vec<_>>().join(", ")),
            Value::Object(m) => format!("#{{ {} }}", m.iter().map(|(k, v)| format!("{k}: {}", rhai_literal(v))).collect::<Vec<_>>().join(", ")),
        }
    }

    fn float(v: f64) -> String {
        if v as f32 as f64 == v {
            format!("{:?}", v as f32)
        } else {
            format!("{v:?}")
        }
    }

    /// A built-in model's `PARAMS` and `INFO` as a twin's source declares
    /// them, the functions left to write (P4 step 7).
    pub(crate) fn skeleton(m: &ModelDef) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "const PARAMS = #{{");
        for p in m.parameters {
            let value = if p.choices.is_empty() { float(f64::from(p.default)) } else { format!("{}", p.default as i64) };
            let _ = writeln!(s, "    {}: #{{", p.name);
            let _ = write!(s, "        value: {value}, ");
            if p.choices.is_empty() {
                let _ = write!(s, "min: {}, max: {}, ", float(f64::from(p.min)), float(f64::from(p.max)));
            } else {
                let _ = write!(s, "choices: {}, ", rhai_literal(&serde_json::json!(p.choices)));
            }
            let _ = writeln!(s, "caption: {:?},", p.display_name);
            let _ = writeln!(s, "        hint: {:?},", p.tooltip);
            let _ = writeln!(s, "    }},");
        }
        let _ = writeln!(s, "}};\nconst INFO = #{{");
        let _ = writeln!(s, "    title: {:?},\n    id: {:?},\n    description: {:?},", m.display_name, m.name, m.description);
        let _ = writeln!(s, "    dt: {},\n    max_dt: {},\n    steps: {},", float(f64::from(m.default_dt)), float(f64::from(m.max_dt)), m.default_steps);
        if !m.diffusion.is_empty() {
            let _ = writeln!(s, "    diffusion: {},", rhai_literal(&serde_json::json!(m.diffusion)));
        }
        if m.state == StateKind::Discrete {
            let _ = writeln!(s, "    state: \"discrete\",");
        }
        if m.has(ModelFeature::NoTimeStep) {
            let _ = writeln!(s, "    time_step: false,");
        }
        if m.has(ModelFeature::NeverStills) {
            let _ = writeln!(s, "    settles: false,");
        }
        if m.has(ModelFeature::PublishesSignal) {
            let _ = writeln!(s, "    publishes_signal: true,");
        }
        if let Some((pass, param)) = m.repeat {
            let _ = writeln!(s, "    repeat: #{{ pass: {}, param: {param:?} }},", pass + 1);
        }
        if let Some(mp) = m.measure {
            let _ = writeln!(s, "    measure: #{{ param: {:?}, slices: {} }},", mp.param, mp.slices);
        }
        if let Some(w) = super::super::scale_warps(m.name) {
            let _ = writeln!(s, "    scale_warps: #{{ rows_param: {:?}, rows: {} }},", w.rows_param, w.rows);
        }
        let slow: Vec<&str> = super::super::SLOW_PARAMS.iter().filter(|(model, _)| *model == m.name).map(|(_, p)| *p).collect();
        if !slow.is_empty() {
            let _ = writeln!(s, "    slow: {},", rhai_literal(&serde_json::json!(slow)));
        }
        if !m.presets.is_empty() {
            let _ = writeln!(s, "    presets: [");
            for p in m.presets {
                let mut fields = vec![format!("name: {:?}", p.name), format!("title: {:?}", p.display_name)];
                let map = |pairs: &[(&str, f32)]| format!("#{{ {} }}", pairs.iter().map(|(k, v)| format!("{k}: {}", float(f64::from(*v)))).collect::<Vec<_>>().join(", "));
                if !p.params.is_empty() {
                    fields.push(format!("params: {}", map(p.params)));
                }
                fields.push(format!("steps: {}", p.steps));
                if let Some(dt) = p.dt {
                    fields.push(format!("dt: {}", float(f64::from(dt))));
                }
                fields.push(format!("boundary: {}", rhai_literal(&serde_json::to_value(p.boundary).unwrap())));
                if let Some(init) = p.init {
                    fields.push(format!("init: {}", rhai_literal(&serde_json::to_value(init).unwrap())));
                }
                if let Some(c) = p.coloring {
                    fields.push(format!("coloring: {c:?}"));
                }
                if !p.coloring_params.is_empty() {
                    fields.push(format!("coloring_params: {}", map(p.coloring_params)));
                }
                if let Some(matte) = p.matte {
                    fields.push(format!("matte: {}", rhai_literal(&serde_json::to_value(matte).unwrap())));
                }
                if let Some(warp) = p.warp {
                    fields.push(format!("warp: {}", rhai_literal(&serde_json::to_value(warp).unwrap())));
                }
                let _ = writeln!(s, "        #{{\n            {},\n        }},", fields.join(",\n            "));
            }
            let _ = writeln!(s, "    ],");
        }
        let _ = writeln!(s, "}};");
        s
    }

    /// Every shipped twin carries its built-in's settings (P4 step 7):
    /// its parameters (names in slot order, labels, defaults, ranges,
    /// choices), presets, working values, state, passes and features, and
    /// a stable time step that agrees with the built-in's `dt_bound` at
    /// the defaults and at each parameter's ends.
    #[test]
    fn every_twin_carries_its_built_ins_settings() {
        for (name, source) in SHIPPED_MODELS {
            let builtin = super::super::MODELS.iter().find(|m| m.name == *name).unwrap_or_else(|| panic!("`{name}` is a built-in"));
            let key = format!("custom-twin-{name}");
            register_model_as(source, &key).unwrap_or_else(|e| panic!("{name}: {e}"));
            let twin = model(&key).expect("registered");
            let params = |m: &ModelDef| m.parameters.iter().map(|p| (p.name, p.display_name, p.default, p.min, p.max, p.tooltip, p.choices)).collect::<Vec<_>>();
            assert_eq!(params(twin), params(builtin), "{name}: parameters");
            let sorted = |pairs: &[(&'static str, f32)]| {
                let mut v = pairs.to_vec();
                v.sort_by(|a, b| a.0.cmp(b.0));
                v
            };
            let presets = |m: &ModelDef| {
                m.presets
                    .iter()
                    .map(|p| (p.name, p.display_name, sorted(p.params), p.steps, p.dt, p.boundary, p.init, p.coloring, sorted(p.coloring_params), p.matte, p.warp))
                    .collect::<Vec<_>>()
            };
            assert_eq!(presets(twin), presets(builtin), "{name}: presets");
            let values = |m: &ModelDef| (m.default_dt, m.max_dt, m.default_steps, m.diffusion, m.state, m.passes, m.display_name, m.description);
            assert_eq!(values(twin), values(builtin), "{name}: working values");
            let passes = |m: &ModelDef| (m.repeat, m.measure.map(|x| (x.param, x.slices)));
            assert_eq!(passes(twin), passes(builtin), "{name}: its passes");
            let warps = |m: &str| super::super::scale_warps(m).map(|w| (w.rows_param, w.rows));
            assert_eq!(warps(&key), warps(name), "{name}: its per-scale warps");
            for p in builtin.parameters {
                assert_eq!(super::super::is_slow_param(&key, p.name), super::super::is_slow_param(name, p.name), "{name}.{}: slow", p.name);
            }
            // Its agents (P4 step 9d): their passes, and their number at
            // the defaults and at each preset, on a few grids.
            assert_eq!(twin.agents.map(|a| a.passes), builtin.agents.map(|a| a.passes), "{name}: its agents' passes");
            if let (Some(a), Some(b)) = (builtin.agents, twin.agents) {
                let at_presets = std::iter::once(std::collections::BTreeMap::new()).chain(builtin.presets.iter().map(|p| p.params.iter().map(|(k, v)| (k.to_string(), *v)).collect()));
                for at in at_presets {
                    for (w, h) in [(64, 64), (512, 512), (1920, 1080), (4096, 4096)] {
                        let (x, y) = ((a.count)(&builtin.params_view(&at), w, h), (b.count)(&twin.params_view(&at), w, h));
                        assert_eq!(x, y, "{name}: the agents on {w}x{h} at {at:?}");
                    }
                }
            }
            let features = |m: &ModelDef| {
                let mut f: Vec<String> = m.features.iter().map(|f| format!("{f:?}")).collect();
                f.sort();
                f
            };
            assert_eq!(features(twin), features(builtin), "{name}: features");
            assert_eq!(twin.dt_bound.is_some(), builtin.dt_bound.is_some(), "{name}: a time step of its own");
            let mut points = vec![std::collections::BTreeMap::new()];
            for p in builtin.parameters {
                for v in [p.min, p.max] {
                    points.push([(p.name.to_string(), v)].into());
                }
            }
            for at in &points {
                let (a, b) = (builtin.max_dt_for(at), twin.max_dt_for(at));
                assert!((a - b).abs() <= 1e-5 * a.abs().max(1e-3), "{name}: max_dt at {at:?}: the built-in's {a}, the twin's {b}");
            }
            // Its kernel table and the spectral stage's discs (P4 step 9b),
            // at the defaults and at every preset. A built-in builds its
            // table in f32 and a script in f64 Rhai: equal to a few units
            // in the last place of the largest weight.
            assert_eq!(twin.kernel.is_some(), builtin.kernel.is_some(), "{name}: a kernel table");
            let at_presets = std::iter::once(std::collections::BTreeMap::new()).chain(builtin.presets.iter().map(|p| p.params.iter().map(|(k, v)| (k.to_string(), *v)).collect()));
            for at in at_presets {
                if let (Some(a), Some(b)) = (builtin.kernel_for(&at), twin.kernel_for(&at)) {
                    assert_eq!((a.radius, a.weights.len()), (b.radius, b.weights.len()), "{name}: the table's size at {at:?}");
                    // Against the table's largest weight: an anti-aliased rim's
                    // tiny weights carry the f32 distance's last bit.
                    let scale = a.weights.iter().fold(0.0f32, |m, w| m.max(w.abs()));
                    let worst = a.weights.iter().zip(&b.weights).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max) / scale;
                    assert!(worst <= 1e-5, "{name}: the table at {at:?} is off by {worst:e} of its largest weight");
                }
                let discs = |m: &str| super::super::spectral_scales(m, &at).map(|v| v.iter().map(|d| (d.ra, d.rb, d.stretch, d.angle, d.annulus)).collect::<Vec<_>>());
                assert_eq!(discs(&key), discs(name), "{name}: the spectral discs at {at:?} ({:?})", failure(&format!("{key}:discs")));
            }
        }
    }

    /// Every shipped colouring twin carries its built-in's settings (P4
    /// step 7): its parameters (names in slot order, labels, defaults,
    /// ranges, choices), title, description and features.
    #[test]
    fn every_colouring_twin_carries_its_built_ins_settings() {
        for (name, source) in SHIPPED {
            let builtin = super::super::COLORINGS.iter().find(|c| c.name == *name).unwrap_or_else(|| panic!("`{name}` is a built-in"));
            let twin = compile(source, &format!("custom-twin-{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
            let params = |c: &SimColoringDef| c.parameters.iter().map(|p| (p.name, p.display_name, p.default, p.min, p.max, p.tooltip, p.choices)).collect::<Vec<_>>();
            assert_eq!(params(&twin), params(builtin), "{name}: parameters");
            assert_eq!((twin.display_name, twin.description), (builtin.display_name, builtin.description), "{name}: labels");
            let features = |c: &SimColoringDef| {
                let mut f: Vec<String> = c.features.iter().map(|f| format!("{f:?}")).collect();
                f.sort();
                f
            };
            assert_eq!(features(&twin), features(builtin), "{name}: features");
        }
        assert_eq!(SHIPPED.len(), super::super::COLORINGS.len(), "every colouring has its twin");
    }

    /// Write every in-class built-in model's skeleton and its WGSL to
    /// `output/sim-twins/` (P4 step 7): what a twin is written from.
    /// Every shipped twin's generated WGSL, step and seed, beside the
    /// built-in's (`write_twin_skeletons`), to read a twin's code against
    /// its built-in's.
    #[test]
    #[ignore]
    fn write_twin_wgsl() {
        let dir = std::path::Path::new("output/sim-twins");
        std::fs::create_dir_all(dir).unwrap();
        for (name, source) in SHIPPED_MODELS {
            let def = compile_model(source, &format!("custom-wgsl-{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
            let agents = def.agents.map_or(String::new(), |a| format!("
// ---- agents
{}", a.wgsl));
            std::fs::write(dir.join(format!("{name}.twin.wgsl")), format!("{}
// ---- seed
{}{agents}", def.wgsl, def.wgsl_seed)).unwrap();
        }
    }

    #[test]
    #[ignore = "a tool: writes files"]
    fn write_twin_skeletons() {
        let dir = std::path::Path::new("output/sim-twins");
        std::fs::create_dir_all(dir).unwrap();
        for m in super::super::MODELS.iter().filter(|m| in_class(m)) {
            std::fs::write(dir.join(format!("{}.rhai", m.name)), skeleton(m)).unwrap();
            std::fs::write(dir.join(format!("{}.wgsl", m.name)), format!("{}\n// ---- seed\n{}", m.wgsl, m.wgsl_seed)).unwrap();
            println!("{}", m.name);
        }
        // The colourings: their parameters, title and description.
        let dir = dir.join("colorings");
        std::fs::create_dir_all(&dir).unwrap();
        for c in super::super::COLORINGS {
            let as_model = ModelDef { parameters: c.parameters, ..super::super::models::GRAY_SCOTT };
            let params = skeleton(&as_model);
            let params = &params[..params.find("const INFO").unwrap()];
            let info = format!("const INFO = #{{\n    title: {:?},\n    id: {:?},\n    description: {:?},\n}};\n", c.display_name, c.name, c.description);
            std::fs::write(dir.join(format!("{}.rhai", c.name)), format!("{params}{info}")).unwrap();
            std::fs::write(dir.join(format!("{}.wgsl", c.name)), format!("// features: {:?}\n{}", c.features, c.wgsl)).unwrap();
            println!("colouring {}", c.name);
        }
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

