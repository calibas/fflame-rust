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
    /// The step reads wide-radius averages. The renderer builds a
    /// GAUSSIAN pyramid of the field before every step -- one dispatch
    /// per level, a 5x5 blur then decimate -- and the step shader
    /// samples it with a manual trilinear read, so an average over any
    /// radius costs eight loads. Gaussian rather than box, and that is
    /// measured: a box pyramid converges to a square kernel however
    /// many levels it has, and McCabe's texture came out visibly
    /// axis-aligned on it. See `scripts/sim_prototypes/proto_mccabe_pyramid.py`.
    NeedsPyramid,
    /// The model carries a population of AGENTS: a storage buffer of
    /// 16-byte records that move themselves and deposit into an
    /// integer accumulation buffer, which the step pass then folds
    /// into the field. See [`AgentDef`].
    NeedsAgents,
    /// The step needs the field's global minimum and maximum. After
    /// every step the renderer reduces the new field into a ring slot
    /// and the NEXT step reads it -- one step of lag, which is the
    /// same dependency the reference algorithm has, since a step can
    /// only normalise by a range that has already been measured.
    NeedsMinMax,
}

/// A model's agent stage.
///
/// The agents are the state: they persist across steps, they move
/// themselves, and what they leave behind is an integer deposit the
/// step pass reads. Two shaders, both supplied by the model:
///
/// ```wgsl
/// fn sim_agent_seed(i: u32) -> SimAgent
/// fn sim_agent(a: SimAgent, i: u32) -> SimAgent
/// ```
///
/// `SimAgent` is `{ pos: vec2<f32>, heading: f32, state: f32 }`.
/// `sim_agent` senses through the ordinary `sim_read`, deposits with
/// `agent_deposit(cell, amount)`, and draws randomness from
/// `agent_rand(i, salt)`.
///
/// **The deposit is INTEGER, and that is what makes an agent model
/// reproducible.** Thousands of agents land in one cell in an order
/// the hardware chooses; `atomicAdd` on a u32 is associative and
/// commutative, so the total does not depend on that order, while an
/// f32 accumulation would give a different sum every run. The value
/// is fixed-point, scaled by [`AGENT_DEPOSIT_SCALE`].
#[derive(Clone, Copy, Debug)]
pub struct AgentDef {
    /// How many agents these parameters ask for on a grid of this
    /// size. Clamped to [`MAX_AGENTS`] by the renderer, which
    /// allocates for exactly this many. The grid is an argument
    /// because a population is normally a FRACTION of the area --
    /// Jones' %p -- so the same setting means the same density at
    /// every grid size.
    pub count: fn(&Params, u32, u32) -> u32,
    /// Dispatches per step. 1 for a population that just moves; 2
    /// when the agents have to AGREE about something first.
    ///
    /// Physarum needs 2. Jones' agents exclude one another -- a cell
    /// holds one agent, and an agent that cannot move stays put and
    /// takes a random heading -- and that is not a detail: measured on
    /// the CPU prototype, dropping it collapses the population into a
    /// few heavy arcs instead of a network. Resolving it needs the
    /// agents to see each other's intentions, so pass 1 turns and
    /// CLAIMS a target cell (`agent_claim`) and pass 2 moves only if
    /// it won (`agent_claim_check`). The claim is an atomic MINIMUM
    /// over agent indices, so the winner is the lowest index rather
    /// than whoever the hardware ran first, and the run reproduces.
    pub passes: u32,
    /// `fn sim_agent_seed(i)`, `fn sim_agent(a, i)`, and for a
    /// two-pass population `fn sim_agent2(a, i)`.
    pub wgsl: &'static str,
}

/// Fixed-point scale for the deposit buffer. A u32 then holds a
/// deposit up to 4.2e6, far above anything a trail reaches.
pub const AGENT_DEPOSIT_SCALE: f32 = 1024.0;

/// Most agents the engine will allocate: 4 million at 16 bytes is
/// 64 MB, and the catalogue's upper end for Physarum.
pub const MAX_AGENTS: u32 = 4_000_000;

/// Levels in the pyramid for a grid, INCLUDING level 0 (the field
/// itself). One rule, computed identically on the CPU and in WGSL:
/// halve until the smaller side would drop below 4 cells, capped at
/// [`MAX_PYRAMID_LEVELS`]. Both sides must agree, because the shader
/// clamps its sample level to this and the renderer allocates exactly
/// this many textures.
pub fn pyramid_levels(grid_w: u32, grid_h: u32) -> u32 {
    let mut levels = 1u32;
    let mut s = grid_w.min(grid_h);
    while s >= 8 && levels < MAX_PYRAMID_LEVELS {
        s = (s + 1) / 2;
        levels += 1;
    }
    levels
}

/// Pyramid levels the engine binds, counting level 0. Seven extra
/// levels reach a 1/128 reduction, which at the calibrated mapping
/// (`level = log2(0.55 r)`) covers an averaging radius of ~230 cells.
pub const MAX_PYRAMID_LEVELS: u32 = 8;

/// Slots in the min/max ring: one per step of the largest batch, plus
/// one so the slot a step READS (the previous step's) is never among
/// the slots the batch clears before running.
pub const MINMAX_RING: u32 = 257;


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

/// The Sims 3×3 Laplacian's most negative eigenvalue (centre −1,
/// edges 0.2, corners 0.05, at the checkerboard mode): −1.6. Diffusion
/// at rate D contributes `1.6 · D` to the stiffness of that mode.
pub const SIMS_LAPLACIAN_EIGENVALUE: f32 = 1.6;

/// Explicit Euler is stable for `dt · λ < 2`. The cap sits at 0.96 of
/// that rather than AT it, because at the bound the checkerboard is
/// neutrally stable rather than damped: measured, Gray–Scott at
/// exactly `dt · 1.6 · D = 2.00` carries a checkerboard of rms 0.445
/// held in place only by its [0, 1] clamp; at 0.96 the mode decays 8%
/// a step and the rms is 0.0003.
pub const STABILITY_MARGIN: f32 = 0.96;

/// A model's parameters, resolved: the value in force, or the
/// declared default when the config has not set one.
///
/// Exists for [`ModelDef::dt_bound`], which is a plain `fn` pointer in
/// a `static` and so cannot close over anything -- it needs the
/// defaults handed to it rather than looking them up.
pub struct Params<'a> {
    model: &'a ModelDef,
    map: &'a std::collections::BTreeMap<String, f32>,
}

impl Params<'_> {
    /// The value in force for `name`. A parameter the model does not
    /// declare reads 0.0, which is a programming error rather than a
    /// state a config can reach -- the registry's invariant tests
    /// check every name a `dt_bound` uses.
    pub fn get(&self, name: &str) -> f32 {
        self.map
            .get(name)
            .copied()
            .filter(|v| v.is_finite())
            .or_else(|| {
                self.model
                    .parameters
                    .iter()
                    .find(|p| p.name == name)
                    .map(|p| p.default)
            })
            .unwrap_or(0.0)
    }
}

/// A precomputed convolution kernel, built on the CPU and uploaded
/// for the step shader to gather against.
///
/// The large-kernel models are not stencils: Lenia's rule is a ring
/// whose weights vary continuously with radius, and SmoothLife's is a
/// pair of anti-aliased discs. Neither is expressible as arithmetic on
/// a fixed neighbourhood, and both are far cheaper to tabulate once
/// than to evaluate per tap.
pub struct SimKernel {
    /// Half-width in cells. The table is `(2 * radius + 1)^2` taps.
    pub radius: u32,
    /// Weights, row-major from `-radius` to `+radius` in both axes.
    /// A model that needs two kernels (SmoothLife's disc and annulus)
    /// appends the second table after the first and indexes past it;
    /// keeping them as separate blocks rather than interleaving them
    /// keeps each gather's reads contiguous.
    pub weights: Vec<f32>,
}

/// Largest kernel half-width the engine will build. At 32 a table is
/// 65 x 65 taps, and a model may store two of them, so the buffer is
/// sized for `2 * 65^2` floats -- 34 KB, which is nothing. The cost
/// that matters is the GATHER: 4,225 taps a cell.
pub const MAX_KERNEL_RADIUS: u32 = 32;

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
    ///
    /// When [`ModelDef::passes`] is 2 this must ALSO define
    /// `fn sim_step2(s: vec4<f32>, p: vec2<i32>) -> vec4<f32>`. The
    /// whole string is spliced into both pass modules and each entry
    /// point calls its own function, so helpers are written once and
    /// shared rather than duplicated across two fields.
    pub wgsl: &'static str,
    /// Dispatches per step. 1 for a rule that is one stencil
    /// application; 2 for a fourth-order PDE, where the first pass
    /// stores a derivative into a spare channel and the second takes
    /// the derivative of THAT.
    ///
    /// A fourth-order operator cannot be done in one pass: a cell
    /// would need its neighbours' neighbours, and the neighbours'
    /// first-pass values do not exist until every cell has been
    /// written. Two dispatches are how the ordering is bought -- the
    /// same "one pass per derivative order" the escape relief blur
    /// uses.
    ///
    /// Both passes of one step carry the SAME `sim_step_index()`: a
    /// step is a step whatever it costs to compute, and the age
    /// channel and the animation track both count steps.
    pub passes: u32,
    /// Largest `dt` the explicit scheme is stable at, for the DEFAULT
    /// diffusion rates. Measured per model (see each model's note); the
    /// reaction terms usually bind before diffusion does.
    ///
    /// Not the whole cap. The diffusion bound scales as `1 / D`, and
    /// the sliders reach several times the default rate, so the cap
    /// that is actually enforced is [`ModelDef::max_dt_for`], which
    /// takes the current parameters. The config manager's write arms,
    /// the panel slider's range and the renderer's uniform all go
    /// through it, so no path can drive the solver past the bound.
    pub max_dt: f32,
    /// Names of the parameters that are diffusion rates on the Sims
    /// stencil. Empty for a rule with no time step.
    pub diffusion: &'static [&'static str],
    /// The agent stage, for the models whose state is a population
    /// rather than a field.
    pub agents: Option<AgentDef>,
    /// Builds this model's convolution kernel, for the models whose
    /// rule is a large-kernel gather rather than a stencil.
    ///
    /// Called whenever the parameters are uploaded, which is once per
    /// batch rather than per step -- a 65 x 65 table is a few
    /// thousand floats and rebuilding it is far cheaper than tracking
    /// whether it went stale.
    pub kernel: Option<fn(&Params) -> SimKernel>,
    /// A stability bound this model derives itself, overriding the
    /// Sims-stencil one.
    ///
    /// For the fourth-order PDEs, whose bound has nothing to do with a
    /// diffusion rate on a 3×3 kernel: Swift–Hohenberg is limited by
    /// `(q0² + ∇²)²` and Cahn–Hilliard by `D γ ∇⁴`. Both are stated
    /// against the 5-POINT Laplacian, which is the one they use --
    /// see the note on each model. The returned value is the raw
    /// bound; [`ModelDef::max_dt_for`] applies [`STABILITY_MARGIN`]
    /// and the declared ceiling to it.
    pub dt_bound: Option<fn(&Params) -> f32>,
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
    /// This model's parameters, resolved against its defaults.
    pub fn params_view<'a>(
        &'a self,
        map: &'a std::collections::BTreeMap<String, f32>,
    ) -> Params<'a> {
        Params { model: self, map }
    }

    /// Build this model's convolution kernel for the parameters in
    /// force, clamped to the buffer the renderer allocated.
    pub fn kernel_for(
        &self,
        params: &std::collections::BTreeMap<String, f32>,
    ) -> Option<SimKernel> {
        let build = self.kernel?;
        let mut k = build(&Params { model: self, map: params });
        k.radius = k.radius.clamp(1, MAX_KERNEL_RADIUS);
        let taps = (2 * k.radius as usize + 1).pow(2);
        // A hand-edited config can carry anything; the buffer cannot.
        k.weights.truncate(2 * taps);
        Some(k)
    }

    /// The stability cap for THESE parameters.
    ///
    /// Linear stability of explicit Euler on the checkerboard mode:
    /// `dt · (λ_reaction + 1.6 · D) < 2`. The reaction stiffness
    /// `λ_reaction` is not derived here; it is INFERRED from the
    /// model's measured cap at its default diffusion rates,
    /// `λ_reaction = 2 / max_dt − 1.6 · D_default`, and the cap at any
    /// other D follows by adding the diffusion term back. A diffusion-
    /// only bound (`1.25 / D`) is not enough: FitzHugh–Nagumo at
    /// D = 4 railed at ±3 under it, because the reaction term
    /// contributes even at rest (`1 − v²` ≈ −0.44, and −8 at the rails).
    ///
    /// `D` is the largest of the diffusion parameters, current value
    /// or default. Using the largest is conservative when the channels
    /// differ, and covers FitzHugh–Nagumo's `D_w / τ` since τ ≥ 1.
    ///
    /// Before this existed the cap was `max_dt` alone, and the sliders
    /// reach 4–5× the default rates. Measured before the fix, 128²
    /// after 200 steps: Brusselator and Schnakenberg infinite in half
    /// their cells, FitzHugh–Nagumo railed with a checkerboard of
    /// rms 5.1.
    pub fn max_dt_for(&self, params: &std::collections::BTreeMap<String, f32>) -> f32 {
        if let Some(bound) = self.dt_bound {
            let raw = bound(&Params { model: self, map: params });
            if raw.is_finite() && raw > 0.0 {
                return (STABILITY_MARGIN * raw).min(self.max_dt).max(1e-4);
            }
            return self.max_dt;
        }
        if self.diffusion.is_empty() {
            return self.max_dt;
        }
        let mut d_now = 0.0f32;
        let mut d_def = 0.0f32;
        for name in self.diffusion {
            let def = self
                .parameters
                .iter()
                .find(|p| p.name == *name)
                .map(|p| p.default)
                .unwrap_or(0.0);
            let v = params.get(*name).copied().filter(|v| v.is_finite()).unwrap_or(def);
            d_now = d_now.max(v.max(0.0));
            d_def = d_def.max(def);
        }
        let lambda_reaction = (2.0 / self.max_dt - SIMS_LAPLACIAN_EIGENVALUE * d_def).max(0.0);
        let lambda = lambda_reaction + SIMS_LAPLACIAN_EIGENVALUE * d_now;
        (STABILITY_MARGIN * 2.0 / lambda).max(1e-4)
    }
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
    &models::EDEN,
    &models::BALLISTIC_DEPOSITION,
    &models::WOLFRAM_ECA,
    &models::PACKARD_SNOWFLAKE,
    &models::PERCOLATION,
    &models::SWIFT_HOHENBERG,
    &models::CAHN_HILLIARD,
    &models::OREGONATOR,
    &models::KOBAYASHI,
    &models::LENIA,
    &models::SMOOTHLIFE,
    &models::MCCABE,
    &models::PHYSARUM,
    &models::DLA,
];

/// Every colouring, in registration order. Append only.
pub static COLORINGS: &[&SimColoringDef] =
    &[
    &colorings::CHANNEL,
    &colorings::TWO_CHANNEL,
    &colorings::AGE,
    &colorings::LABEL,
    &colorings::SCALE_MIX,
    &colorings::OCCUPANCY,
];

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

    /// The declared cap must be consistent with the default diffusion
    /// rates, and every parameter named as a diffusion rate must
    /// exist -- a typo there would silently drop the bound.
    #[test]
    fn the_declared_cap_respects_the_default_diffusion_bound() {
        for m in MODELS {
            for name in m.diffusion {
                let def = m
                    .parameters
                    .iter()
                    .find(|p| p.name == *name)
                    .unwrap_or_else(|| panic!("{}: diffusion parameter {name} does not exist", m.name));
                assert!(
                    m.max_dt * def.default * 1.6 <= 2.0 + 1e-5,
                    "{}: max_dt {} at default {name} = {} gives dt·D·1.6 = {} > 2",
                    m.name,
                    m.max_dt,
                    def.default,
                    m.max_dt * def.default * 1.6
                );
            }
            // With no parameters set, the cap is the declared one under
            // the safety margin.
            let at_defaults = m.max_dt_for(&std::collections::BTreeMap::new());
            assert!(
                at_defaults <= m.max_dt && at_defaults > 0.0,
                "{}: cap at defaults {} vs declared {}",
                m.name,
                at_defaults,
                m.max_dt
            );
            assert!(
                m.default_dt <= at_defaults + 1e-6,
                "{}: default_dt {} exceeds the cap at defaults {}",
                m.name,
                m.default_dt,
                at_defaults
            );
            // A rule with no time step must declare no bound either.
            // The converse does NOT hold: Lenia advances by dt and has
            // no stability bound at all, because its growth term is
            // bounded in [-1, 1] and the state is clipped to [0, 1],
            // so no dt can make it diverge -- only blur the dynamics,
            // which `max_dt` handles.
            if m.has(ModelFeature::NoTimeStep) {
                assert!(
                    m.diffusion.is_empty() && m.dt_bound.is_none(),
                    "{}: a rule with no time step declares a stability bound",
                    m.name
                );
            }
        }
    }

    /// A two-pass model must actually define its second pass, and a
    /// one-pass model must not carry a stray one -- the assembler
    /// splices by name, so a missing `sim_step2` is a shader that
    /// fails to compile on the device rather than here.
    #[test]
    fn the_pass_count_matches_the_functions_the_model_defines() {
        for m in MODELS {
            assert!(
                m.passes == 1 || m.passes == 2,
                "{}: passes {} is neither 1 nor 2",
                m.name,
                m.passes
            );
            assert!(
                m.wgsl.contains("fn sim_step("),
                "{}: no sim_step",
                m.name
            );
            assert_eq!(
                m.wgsl.contains("fn sim_step2("),
                m.passes == 2,
                "{}: passes = {} but sim_step2 {} defined",
                m.name,
                m.passes,
                if m.passes == 2 { "is not" } else { "is" }
            );
        }
    }

    /// A model that derives its own dt bound must produce a positive,
    /// finite one at its defaults, and its declared `max_dt` must not
    /// contradict it.
    #[test]
    fn a_derived_dt_bound_is_consistent_with_the_declared_one() {
        for m in MODELS {
            let Some(bound) = m.dt_bound else { continue };
            assert!(
                m.diffusion.is_empty(),
                "{}: a model derives its bound OR declares Sims diffusion rates, not both",
                m.name
            );
            let empty = std::collections::BTreeMap::new();
            let raw = bound(&Params { model: m, map: &empty });
            assert!(
                raw.is_finite() && raw > 0.0,
                "{}: derived dt bound {raw} at the defaults",
                m.name
            );
            // Every parameter combination the sliders allow must also
            // give a usable bound: a bound that goes to zero or NaN
            // somewhere in range would freeze the dt slider there.
            for pd in m.parameters {
                for v in [pd.min, pd.default, pd.max] {
                    let mut map = std::collections::BTreeMap::new();
                    map.insert(pd.name.to_string(), v);
                    let r = bound(&Params { model: m, map: &map });
                    assert!(
                        r.is_finite() && r > 0.0,
                        "{}: dt bound {r} at {} = {v}",
                        m.name,
                        pd.name
                    );
                }
            }
        }
    }

    /// A kernel must be buildable, normalised and within the buffer at
    /// every setting the sliders allow -- a radius past the cap would
    /// read off the end of the table, and weights that do not sum to
    /// one would silently rescale the rule.
    #[test]
    fn a_declared_kernel_is_sane_across_its_parameter_range() {
        for m in MODELS {
            let Some(build) = m.kernel else { continue };
            let mut cases = vec![std::collections::BTreeMap::new()];
            for pd in m.parameters {
                for v in [pd.min, pd.default, pd.max] {
                    let mut map = std::collections::BTreeMap::new();
                    map.insert(pd.name.to_string(), v);
                    cases.push(map);
                }
            }
            for map in cases {
                let k = build(&Params { model: m, map: &map });
                let taps = (2 * k.radius as usize + 1).pow(2);
                assert!(
                    k.radius >= 1 && k.radius <= MAX_KERNEL_RADIUS,
                    "{}: kernel radius {} outside 1..={MAX_KERNEL_RADIUS}",
                    m.name,
                    k.radius
                );
                assert!(
                    k.weights.len() == taps || k.weights.len() == 2 * taps,
                    "{}: {} weights for a radius-{} kernel ({taps} taps)",
                    m.name,
                    k.weights.len(),
                    k.radius
                );
                assert!(
                    k.weights.iter().all(|w| w.is_finite() && *w >= 0.0),
                    "{}: kernel has a negative or non-finite weight",
                    m.name
                );
                // Each block must be normalised: the rule reads the
                // gather as an average.
                for (i, block) in k.weights.chunks(taps).enumerate() {
                    let sum: f64 = block.iter().map(|w| *w as f64).sum();
                    assert!(
                        (sum - 1.0).abs() < 1e-4,
                        "{}: kernel block {i} sums to {sum}, not 1",
                        m.name
                    );
                }
            }
        }
    }

    /// The pyramid level count is computed on both sides of the GPU
    /// boundary; this pins the CPU side's shape so the WGSL copy has
    /// something exact to match.
    #[test]
    fn pyramid_levels_halve_to_a_floor_and_cap() {
        assert_eq!(pyramid_levels(4, 4), 1);
        assert_eq!(pyramid_levels(8, 8), 2);
        assert_eq!(pyramid_levels(64, 64), 5);
        assert_eq!(pyramid_levels(256, 256), 7);
        assert_eq!(pyramid_levels(1920, 1080), MAX_PYRAMID_LEVELS);
        // The smaller side rules.
        assert_eq!(pyramid_levels(1920, 8), 2);
    }

    /// Every `mparam(N)` in a model's WGSL, and every `cparam(N)` in a
    /// colouring's, must index a parameter the definition declares.
    ///
    /// The parameter buffer is padded to 16 floats, so an index past
    /// the declared count reads 0.0 without any error -- a model that
    /// listed its parameters in one order and its `mparam` calls in
    /// another would run with a zero where it expected a rate, and
    /// still render something. This is the check the phase-3 review
    /// ran by hand once; it belongs in the suite.
    #[test]
    fn every_parameter_index_names_a_declared_parameter() {
        fn indices(wgsl: &str, accessor: &str) -> Vec<usize> {
            let mut out = Vec::new();
            let mut rest = wgsl;
            while let Some(pos) = rest.find(accessor) {
                rest = &rest[pos + accessor.len()..];
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = digits.parse::<usize>() {
                    out.push(n);
                }
            }
            out
        }
        for m in MODELS {
            for i in indices(m.wgsl, "mparam(").into_iter().chain(indices(m.wgsl_seed, "mparam(")) {
                assert!(
                    i < m.parameters.len(),
                    "{}: mparam({i}) but only {} parameters are declared",
                    m.name,
                    m.parameters.len()
                );
            }
        }
        for c in COLORINGS {
            for i in indices(c.wgsl, "cparam(") {
                assert!(
                    i < c.parameters.len(),
                    "{}: cparam({i}) but only {} parameters are declared",
                    c.name,
                    c.parameters.len()
                );
            }
        }
    }

    /// An agent model must declare the feature and the definition
    /// together, define both of its functions, and ask for a
    /// population the engine can allocate at every slider setting.
    #[test]
    fn an_agent_model_is_declared_consistently() {
        for m in MODELS {
            assert_eq!(
                m.has(ModelFeature::NeedsAgents),
                m.agents.is_some(),
                "{}: the NeedsAgents feature and the AgentDef must agree",
                m.name
            );
            let Some(a) = m.agents else { continue };
            assert!(
                a.wgsl.contains("fn sim_agent_seed(") && a.wgsl.contains("fn sim_agent("),
                "{}: an agent model defines sim_agent_seed and sim_agent",
                m.name
            );
            assert!(a.passes == 1 || a.passes == 2, "{}: agent passes must be 1 or 2", m.name);
            assert_eq!(
                a.wgsl.contains("fn sim_agent2("),
                a.passes == 2,
                "{}: agent passes = {} but sim_agent2 {} defined",
                m.name,
                a.passes,
                if a.passes == 2 { "is not" } else { "is" }
            );
            let mut cases = vec![std::collections::BTreeMap::new()];
            for pd in m.parameters {
                for v in [pd.min, pd.default, pd.max] {
                    let mut map = std::collections::BTreeMap::new();
                    map.insert(pd.name.to_string(), v);
                    cases.push(map);
                }
            }
            for map in cases {
                for (gw, gh) in [(64u32, 64u32), (256, 256), (1920, 1080), (4096, 4096)] {
                    let n = (a.count)(&Params { model: m, map: &map }, gw, gh);
                    assert!(
                        n >= 1 && n <= MAX_AGENTS,
                        "{}: asks for {n} agents at {gw}x{gh}, outside 1..={MAX_AGENTS}",
                        m.name
                    );
                }
            }
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
