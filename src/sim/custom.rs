//! Simulation colourings written in Rhai (docs/projects/rhai-plugins.md,
//! P4).
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

use super::{ColoringFeature, SimColoringDef, SimParamDef};
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
                // A slider needs ends: the declared ones, else a span
                // around the default.
                let span = default.abs().max(1.0) * 10.0;
                (param.min.map_or(default - span, |v| v as f32), param.max.map_or(default + span, |v| v as f32), &[])
            }
            ParamKind::Complex => {
                return Err(format!("`{}`: a colouring's parameters are numbers, bools and choices, not complex numbers", param.name));
            }
        };
        parameters.push(SimParamDef { name: leak(param.name.clone()), display_name: leak(caption), default, min, max, tooltip: leak(param.hint.clone()), choices });
    }
    let wgsl = lang::wgsl::sim_coloring(&program)?;
    let mut features = Vec::new();
    for (on, f) in [
        (wgsl.gradient, ColoringFeature::NeedsGradient),
        (wgsl.structure, ColoringFeature::NeedsStructure),
        (wgsl.distance, ColoringFeature::NeedsDistance),
        (wgsl.relief, ColoringFeature::NeedsRelief),
        (wgsl.reads_cell, ColoringFeature::ReadsCell),
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

/// Register the colouring a simulation carries, before anything resolves
/// its name. Cheap when it is registered already; one that does not
/// compile is logged once, and its name falls back as an unknown one's
/// does (`coloring_or_default`).
pub fn ensure(sim: &crate::config::sim::SimConfig) {
    let Some(src) = &sim.custom_coloring else { return };
    if !sim.coloring.starts_with(PREFIX) || coloring(&sim.coloring).is_some() {
        return;
    }
    if registry().read().is_ok_and(|r| r.failed.contains_key(&sim.coloring)) {
        return;
    }
    if let Err(e) = register_as(&src.source, &sim.coloring) {
        log::warn!("The simulation colouring `{}` did not compile: {e}", sim.coloring);
        if let Ok(mut r) = registry().write() {
            r.failed.insert(sim.coloring.clone(), e);
        }
    }
}

/// Why the colouring `name` a config carries did not register, when it
/// did not.
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

    /// A carried source registers under the name the config gives it, and
    /// the lookup every path uses finds it.
    #[test]
    fn a_carried_colouring_is_ensured() {
        let mut sim = crate::config::sim::SimConfig::default();
        let src = carried(CHANNEL);
        sim.coloring = src.entry.clone();
        sim.custom_coloring = Some(src);
        ensure(&sim);
        assert_eq!(super::super::coloring_or_default(&sim.coloring).name, sim.coloring);
    }
}
