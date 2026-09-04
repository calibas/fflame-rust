//! Neighbour-coupled simulation rendering (reaction–diffusion,
//! cellular automata, growth).
//!
//! The third render mode described in
//! `docs/projects/simulation-fractals.md`. Where the flame renderer
//! runs a chaos game into a histogram and the escape renderer
//! evaluates each pixel independently, this one steps a **stateful
//! grid**: every cell reads its neighbours, many times per frame, and
//! a colour pass turns the field into an `Rgba32Float` image shaped
//! exactly like the flame accumulator (`rgb` = colour, `a` =
//! coverage), so the existing tonemap → effects → readback tail
//! consumes it unchanged.
//!
//! Architecture mirrors the escape engine deliberately:
//! * [`ModelDef`] / [`SimColoringDef`] are `static` definitions with
//!   inline WGSL and self-describing parameters (the panel UI
//!   auto-generates its controls from them).
//! * Registries are ordered slices; **append-only**, name-addressed.
//! * The assembler splices exactly one model and one colouring into a
//!   small template — per-combination shaders, cached by the renderer.
//!
//! What is genuinely different from escape, and shapes everything
//! below:
//!
//! * **The grid is not the viewport.** A simulation's behaviour
//!   depends on its cell count, so [`crate::config::sim::SimGrid`] is
//!   a config quantity and the resolve pass scales the coloured grid
//!   to the output.
//! * **State persists across frames.** The renderer owns the field
//!   pair and a step counter; a still is "the state at step N from
//!   this seed", which is what makes a never-settling model (spirals,
//!   cyclic automata) reproducible at all.
//! * **Two textures, ping-ponged.** wgpu rejects read-write storage on
//!   `rgba32float`, so a step reads `field[i]` as a sampled texture
//!   and writes `field[1-i]` as a write-only storage texture. That is
//!   the only portable shape, and it is why there is a swap rather
//!   than an in-place update.

pub mod assembler;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod app_repro_test;
pub mod colorings;
pub mod models;
pub mod renderer;

pub use renderer::SimRenderer;

/// A parameter a model or colouring exposes, with everything the UI
/// needs to build a control for it.
///
/// Deliberately the same shape as `EscapeParamDef`: a `choices` list
/// turns the slider into a dropdown, and the value is still a plain
/// `f32` on the wire so animation tracks address it uniformly.
#[derive(Clone, Copy, Debug)]
pub struct SimParamDef {
    pub name: &'static str,
    pub display_name: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub tooltip: &'static str,
    /// Non-empty ⇒ the value is an index into these labels and the UI
    /// shows a dropdown instead of a slider.
    pub choices: &'static [&'static str],
}

/// A named parameter set for a model — the equivalent of an escape
/// formula's presets, and the reason the panel can offer "mitosis"
/// rather than two unlabelled numbers.
///
/// Phase 0's rule applies to everything listed here: **nothing ships
/// as a preset that has not been run.** The catalogue records what
/// each one was measured to do, and a Gray–Scott preset whose blobs
/// die is a preset that does not ship.
#[derive(Clone, Copy, Debug)]
pub struct SimPreset {
    pub name: &'static str,
    pub display_name: &'static str,
    /// `(param name, value)` pairs applied over the model's defaults.
    pub params: &'static [(&'static str, f32)],
    /// Steps to a settled or developed picture, measured. Used as the
    /// config's `steps` when the preset is applied.
    pub steps: u32,
    /// The initial field this preset needs, when it needs a particular
    /// one.
    ///
    /// Not decoration: FitzHugh-Nagumo's excitable constants give
    /// spirals from a cut wavefront and a FLAT FIELD from noise, so a
    /// preset that carried only numbers would ship a picture of
    /// nothing. `None` leaves whatever the user has.
    pub init: Option<crate::config::sim::SimInit>,
}

/// Capability flags a model opts into. Absence means "doesn't have
/// it", the same convention the variation and formula registries use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelFeature {
    /// The step draws random numbers per cell. The assembler compiles
    /// in the PCG helpers and the step index reaches the kernel, so a
    /// run is reproducible from `(seed, cell, step)`.
    NeedsRng,
    /// The model never reaches a still state. The panel says so and
    /// the export contract is "the state at step N" rather than "the
    /// converged picture".
    NeverStills,
    /// The rule has no time step: a cellular automaton advances by one
    /// generation, not by `dt` of model time. The panel hides the dt
    /// slider rather than showing a control that does nothing.
    NoTimeStep,
}

/// Capability flags a colouring opts into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColoringFeature {
    /// The colouring reads `grad`, the central-difference gradient of
    /// channel `.x`. Costs four extra texture reads per output pixel,
    /// so the template computes it ONLY for colourings that declare it
    /// -- measured, the reads were 4 of the 5 the colour pass made, and
    /// the bilinear resolve multiplied them by four again.
    NeedsGradient,
}

/// One simulation model: the rule, its parameters, and how a cell's
/// state is laid out in the four channels.
///
/// The WGSL contract, spliced by [`assembler`]:
///
/// ```wgsl
/// fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32>
/// ```
///
/// `s` is this cell's current state; `p` its integer coordinates.
/// Neighbours come from `sim_read(p + offset)`, which the template
/// provides and which applies the configured boundary — a model never
/// writes boundary handling itself, because getting it wrong is
/// invisible in the middle of the grid and wrong only at the edges.
#[derive(Clone, Copy, Debug)]
pub struct ModelDef {
    pub name: &'static str,
    pub display_name: &'static str,
    /// One-line description for the panel's dropdown tooltip.
    pub description: &'static str,
    pub features: &'static [ModelFeature],
    pub parameters: &'static [SimParamDef],
    pub presets: &'static [SimPreset],
    /// The step rule. See the type docs for the signature.
    pub wgsl: &'static str,
    /// Largest `dt` the explicit scheme is stable at, for the default
    /// diffusion rates. Enforced by the config manager's write arm, the
    /// panel slider's range and the renderer's uniform, so no path can
    /// drive the solver past it. Derived per model from the stencil's
    /// most negative eigenvalue, not guessed (see each model's note).
    pub max_dt: f32,
    /// The initial state for a cell, given the init shape's mask.
    ///
    /// ```wgsl
    /// fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32>
    /// ```
    ///
    /// `inside` is 1.0 where the configured [`SimInit`] shape covers
    /// the cell and 0.0 elsewhere; `noise` is uniform in [0, 1) from
    /// the config seed. Models decide what those mean — Gray–Scott
    /// puts `B = inside`, a growth model puts occupancy there.
    ///
    /// [`SimInit`]: crate::config::sim::SimInit
    pub wgsl_seed: &'static str,
    /// Measured default step count for a still (catalogue).
    pub default_steps: u32,
    /// The `dt` this model is normally run at. Applied when the model
    /// is selected, because the models differ by two orders of
    /// magnitude here -- Gray-Scott runs at 1.0 and Schnakenberg
    /// diverges above 0.02 -- so carrying one model's dt into another
    /// is either unusably slow or unstable.
    pub default_dt: f32,
}

impl ModelDef {
    pub fn has(&self, f: ModelFeature) -> bool {
        self.features.contains(&f)
    }

    /// Parameter values in declaration order, config overriding the
    /// definition's defaults. This is the packing order the shader's
    /// `mparam(i)` indexes, so it is the one place that ordering is
    /// decided.
    pub fn pack_params(&self, cfg: &crate::config::sim::SimConfig) -> Vec<f32> {
        self.parameters
            .iter()
            .map(|p| cfg.model_param(p.name, p.default))
            .collect()
    }

    pub fn preset(&self, name: &str) -> Option<&'static SimPreset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

/// One colouring: field → `(rgb, coverage)`.
///
/// ```wgsl
/// fn sim_color(s: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32>
/// ```
///
/// `grad` is the central-difference gradient of channel `.x`, computed
/// once by the template so a hillshade colouring does not have to
/// re-read neighbours.
#[derive(Clone, Copy, Debug)]
pub struct SimColoringDef {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub features: &'static [ColoringFeature],
    pub parameters: &'static [SimParamDef],
    pub wgsl: &'static str,
}

impl SimColoringDef {
    pub fn has(&self, f: ColoringFeature) -> bool {
        self.features.contains(&f)
    }

    pub fn pack_params(&self, cfg: &crate::config::sim::SimConfig) -> Vec<f32> {
        self.parameters
            .iter()
            .map(|p| cfg.coloring_param(p.name, p.default))
            .collect()
    }
}

/// Every model, in registration order. **Append only** — the order is
/// the UI order and, once presets and configs name them, the names are
/// a compatibility surface.
pub static MODELS: &[&ModelDef] = &[
    &models::GRAY_SCOTT,
    &models::FITZHUGH_NAGUMO,
    &models::BRUSSELATOR,
    &models::SCHNAKENBERG,
    &models::HODGEPODGE,
    &models::CYCLIC_CA,
    &models::SPATIAL_RPS,
    &models::ISING,
];

/// Every colouring, in registration order. Append only.
pub static COLORINGS: &[&SimColoringDef] =
    &[&colorings::CHANNEL, &colorings::TWO_CHANNEL, &colorings::AGE];

/// Look up a model by name, falling back to the first registered one.
///
/// Falling back rather than failing is the same forward-compatibility
/// posture the variation and formula registries take: a config naming
/// a model this build does not have still opens, with a warning,
/// instead of refusing the file.
pub fn model_or_default(name: &str) -> &'static ModelDef {
    MODELS.iter().copied().find(|m| m.name == name).unwrap_or_else(|| {
        log::warn!("unknown simulation model {name:?}; using {:?}", MODELS[0].name);
        MODELS[0]
    })
}

pub fn coloring_or_default(name: &str) -> &'static SimColoringDef {
    COLORINGS
        .iter()
        .copied()
        .find(|c| c.name == name)
        .unwrap_or_else(|| {
            log::warn!(
                "unknown simulation colouring {name:?}; using {:?}",
                COLORINGS[0].name
            );
            COLORINGS[0]
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_names_are_unique_and_lowercase() {
        let mut seen = HashSet::new();
        for m in MODELS {
            assert!(seen.insert(m.name), "duplicate model name {:?}", m.name);
            assert!(
                m.name.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
                "model name {:?} must be lowercase snake_case -- it is a wire value",
                m.name
            );
        }
        let mut seen = HashSet::new();
        for c in COLORINGS {
            assert!(seen.insert(c.name), "duplicate colouring name {:?}", c.name);
        }
    }

    /// The default config must name things that exist, or a fresh
    /// entry into the mode falls back with a warning.
    #[test]
    fn the_default_config_names_registered_entries() {
        let cfg = crate::config::sim::SimConfig::default();
        assert!(MODELS.iter().any(|m| m.name == cfg.model), "default model missing");
        assert!(
            COLORINGS.iter().any(|c| c.name == cfg.coloring),
            "default colouring missing"
        );
    }

    /// A parameter whose default falls outside its own slider range is
    /// a control the user cannot return to its default.
    #[test]
    fn every_parameter_default_is_inside_its_range() {
        let all = MODELS
            .iter()
            .flat_map(|m| m.parameters.iter().map(move |p| (m.name, p)))
            .chain(
                COLORINGS
                    .iter()
                    .flat_map(|c| c.parameters.iter().map(move |p| (c.name, p))),
            );
        for (owner, p) in all {
            assert!(p.min <= p.max, "{owner}.{}: min > max", p.name);
            assert!(
                p.default >= p.min && p.default <= p.max,
                "{owner}.{}: default {} outside [{}, {}]",
                p.name,
                p.default,
                p.min,
                p.max
            );
            assert!(!p.tooltip.is_empty(), "{owner}.{}: needs a tooltip", p.name);
        }
    }

    /// Presets may only set parameters the model actually has, and
    /// only to values its sliders can reach -- a preset outside the
    /// range cannot be edited back to after it is applied.
    #[test]
    fn every_preset_sets_real_parameters_within_range() {
        for m in MODELS {
            for pre in m.presets {
                assert!(pre.steps > 0, "{}/{}: steps must be positive", m.name, pre.name);
                for (k, v) in pre.params {
                    let def = m
                        .parameters
                        .iter()
                        .find(|p| p.name == *k)
                        .unwrap_or_else(|| panic!("{}/{}: no parameter {k:?}", m.name, pre.name));
                    assert!(
                        *v >= def.min && *v <= def.max,
                        "{}/{}: {k} = {v} outside [{}, {}]",
                        m.name,
                        pre.name,
                        def.min,
                        def.max
                    );
                }
            }
        }
    }

    /// A stability cap that is zero, negative or absurd would either
    /// freeze the dt slider or let the solver diverge; the value is a
    /// derivation and this keeps a typo from shipping as one.
    #[test]
    fn every_model_declares_a_sane_stability_cap() {
        for m in MODELS {
            assert!(
                m.max_dt > 0.0 && m.max_dt <= 10.0,
                "{}: max_dt {} is not a plausible explicit-Euler bound",
                m.name,
                m.max_dt
            );
            assert!(
                m.default_dt > 0.0 && m.default_dt <= m.max_dt,
                "{}: default_dt {} must be positive and within max_dt {}",
                m.name,
                m.default_dt,
                m.max_dt
            );
        }
    }

    #[test]
    fn an_unknown_name_falls_back_rather_than_panicking() {
        assert_eq!(model_or_default("no_such_model").name, MODELS[0].name);
        assert_eq!(coloring_or_default("no_such_coloring").name, COLORINGS[0].name);
    }

    /// `mparam(i)` indexes this vector, so its order is the shader's
    /// contract, not an implementation detail.
    #[test]
    fn packed_params_follow_declaration_order_with_config_overrides() {
        let m = &models::GRAY_SCOTT;
        let mut cfg = crate::config::sim::SimConfig::default();
        let defaults = m.pack_params(&cfg);
        assert_eq!(defaults.len(), m.parameters.len());
        for (v, p) in defaults.iter().zip(m.parameters) {
            assert_eq!(*v, p.default);
        }
        cfg.model_params.insert(m.parameters[0].name.to_string(), 0.125);
        assert_eq!(m.pack_params(&cfg)[0], 0.125);
    }
}
