//! Simulation-mode configuration.
//!
//! The per-config state for the neighbour-coupled simulations
//! (reaction–diffusion, cellular automata, growth) — everything
//! `docs/projects/simulation-fractals.md` calls `SimConfig`. Lives
//! inside [`FractalConfig`] behind skip-if-default, so a flame or an
//! escape config that has never touched simulation mode serializes
//! exactly as before, byte for byte.
//!
//! Two shapes worth knowing before reading the fields:
//!
//! * **The grid is a quantity of its own, separate from the
//!   viewport.** An escape render is a function of the pixel's own
//!   coordinates, so it renders at whatever size it is asked for. A
//!   simulation's behaviour depends on its cell count — Gray–Scott at
//!   256² and 2048² are different pictures, not the same picture at
//!   two resolutions — so [`SimGrid`] is either `Fixed`, reproducible
//!   from the config at any output size, or `Viewport { scale }`,
//!   which fills the window and re-simulates when it resizes.
//! * **`steps` is the contract, not a hint.** A still is reproducible
//!   because the renderer runs exactly this many steps from the seed;
//!   models that never settle (spirals, cyclic automata) are animated
//!   subjects whose still is simply "the state at step N". Measured
//!   defaults per model are in `docs/projects/simulation-catalog.md`.
//!
//! Per-model and per-colouring parameters are keyed maps rather than
//! fields, the same choice `Transform::variation_params` and
//! `EscapeConfig` made. `BTreeMap` for deterministic iteration: UI
//! ordering, JSON output and GPU packing order all read it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How the simulation grid relates to the output size.
///
/// Decided with the user 2026-09-01 (master plan D4). Rejected:
/// viewport-only, which gives no reproducible grid, and config-only,
/// which gives no fill-the-window behaviour. The enum is both.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimGrid {
    /// An exact cell count, independent of the output size. The
    /// picture is reproducible at any resolution: the resolve pass
    /// scales the coloured grid to the output.
    Fixed { width: u32, height: u32 },
    /// `round(output × scale)` cells. Fills the window, and
    /// re-simulates when the window resizes — which is why the export
    /// panel has to say which grid an export will use.
    Viewport { scale: f32 },
}

impl Default for SimGrid {
    fn default() -> Self {
        SimGrid::Viewport { scale: 1.0 }
    }
}

impl SimGrid {
    /// Cells for a given output size, clamped to something a device
    /// will accept. The caller still has to ask the renderer whether
    /// the allocation fits — this only keeps the arithmetic sane.
    pub fn cells_for(&self, out_w: u32, out_h: u32) -> (u32, u32) {
        match *self {
            SimGrid::Fixed { width, height } => (width.max(1), height.max(1)),
            SimGrid::Viewport { scale } => {
                let s = if scale.is_finite() { scale.clamp(0.125, 4.0) } else { 1.0 };
                (
                    ((out_w as f32 * s).round() as u32).max(1),
                    ((out_h as f32 * s).round() as u32).max(1),
                )
            }
        }
    }

    pub fn is_bound(&self) -> bool {
        matches!(self, SimGrid::Viewport { .. })
    }
}

/// The initial field.
///
/// Phase 0 measured why this matters rather than being cosmetic:
/// Gray–Scott's 12-pixel blobs die at the mitosis parameters where
/// 24-pixel blobs live, and FitzHugh–Nagumo's spirals need a *cut*
/// wavefront — the same constants from a noise seed relax to the rest
/// state and render a flat field.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimInit {
    /// Uniform noise about the model's home state.
    Noise { amplitude: f32 },
    /// One centred square of the second species.
    Blob { radius: u32 },
    /// `count` squares at pseudo-random positions from the seed.
    Blobs { count: u32, radius: u32 },
    /// A ring, for models whose interesting behaviour is a front.
    Ring { radius: u32 },
    /// A horizontal line — a growing rough front (KPZ) for growth
    /// models, and a wavefront to cut for excitable media.
    Line,
    /// A single centred cell: the growth models' point seed.
    Center,
    /// A wavefront cut in half, with a refractory tail behind it.
    ///
    /// The only way to nucleate a spiral in an excitable medium, and
    /// not a cosmetic choice: measured, FitzHugh-Nagumo's published
    /// constants relax to a flat rest state from a noise seed (spatial
    /// sd 0.0014) and produce textbook counter-rotating spirals from
    /// this one. Excitable models are the reason it exists.
    BrokenWave,
}

impl Default for SimInit {
    fn default() -> Self {
        SimInit::Blobs { count: 6, radius: 24 }
    }
}

impl SimInit {
    /// The discriminant name, for the UI dropdown and the string keys.
    pub fn kind_name(&self) -> &'static str {
        match self {
            SimInit::Noise { .. } => "noise",
            SimInit::Blob { .. } => "blob",
            SimInit::Blobs { .. } => "blobs",
            SimInit::Ring { .. } => "ring",
            SimInit::Line => "line",
            SimInit::Center => "center",
            SimInit::BrokenWave => "broken_wave",
        }
    }

    pub const KINDS: &'static [&'static str] =
        &["noise", "blob", "blobs", "ring", "line", "center", "broken_wave"];

    /// Switch kind, keeping whatever the new kind shares with the old.
    pub fn with_kind(&self, kind: &str) -> SimInit {
        let radius = match *self {
            SimInit::Blob { radius } | SimInit::Blobs { radius, .. } | SimInit::Ring { radius } => {
                radius
            }
            _ => 24,
        };
        let count = match *self {
            SimInit::Blobs { count, .. } => count,
            _ => 6,
        };
        match kind {
            "noise" => SimInit::Noise { amplitude: 0.05 },
            "blob" => SimInit::Blob { radius },
            "blobs" => SimInit::Blobs { count, radius },
            "ring" => SimInit::Ring { radius },
            "line" => SimInit::Line,
            "broken_wave" => SimInit::BrokenWave,
            _ => SimInit::Center,
        }
    }
}

/// What a step reads outside the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimBoundary {
    /// Wrap. The default for pattern models, and the only choice that
    /// leaves no edge artifacts in a Turing field.
    #[default]
    Periodic,
    /// Read the nearest in-bounds cell (zero gradient).
    Clamp,
    /// Read zero — the sink boundary growth and sandpile models want.
    Zero,
    /// Reflect.
    Mirror,
}

impl SimBoundary {
    pub const NAMES: &'static [&'static str] = &["periodic", "clamp", "zero", "mirror"];

    pub fn name(&self) -> &'static str {
        match self {
            SimBoundary::Periodic => "periodic",
            SimBoundary::Clamp => "clamp",
            SimBoundary::Zero => "zero",
            SimBoundary::Mirror => "mirror",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "periodic" => SimBoundary::Periodic,
            "clamp" => SimBoundary::Clamp,
            "zero" => SimBoundary::Zero,
            "mirror" => SimBoundary::Mirror,
            _ => return None,
        })
    }
}

/// Resolve filter when the grid is smaller than the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimUpscale {
    /// Cells stay cells. The honest default: a 256² Gray–Scott shown
    /// at 1080p is 256² of information, and blurring it does not add
    /// any.
    #[default]
    Nearest,
    Bilinear,
}

impl SimUpscale {
    pub const NAMES: &'static [&'static str] = &["nearest", "bilinear"];

    pub fn name(&self) -> &'static str {
        match self {
            SimUpscale::Nearest => "nearest",
            SimUpscale::Bilinear => "bilinear",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "nearest" => SimUpscale::Nearest,
            "bilinear" => SimUpscale::Bilinear,
            _ => return None,
        })
    }
}

/// Resolve filter when the grid is larger than the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimDownscale {
    /// Average the covered cells. Correct, and cheap at the ratios a
    /// bound grid produces.
    #[default]
    Box,
    /// Nearest cell — aliased, but it keeps single-cell structure
    /// visible in models whose subject is the lattice itself.
    Nearest,
}

impl SimDownscale {
    pub const NAMES: &'static [&'static str] = &["box", "nearest"];

    pub fn name(&self) -> &'static str {
        match self {
            SimDownscale::Box => "box",
            SimDownscale::Nearest => "nearest",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "box" => SimDownscale::Box,
            "nearest" => SimDownscale::Nearest,
            _ => return None,
        })
    }
}

fn default_model() -> String {
    "gray_scott".to_string()
}
fn default_coloring() -> String {
    "channel".to_string()
}
fn default_seed() -> u64 {
    1
}
fn default_steps() -> u32 {
    4000
}
fn default_steps_per_frame() -> u32 {
    4
}
fn default_dt() -> f32 {
    1.0
}
fn is_default_model(s: &String) -> bool {
    s == "gray_scott"
}
fn is_default_coloring(s: &String) -> bool {
    s == "channel"
}
fn is_default_seed(v: &u64) -> bool {
    *v == 1
}
fn is_default_steps(v: &u32) -> bool {
    *v == 4000
}
fn is_default_steps_per_frame(v: &u32) -> bool {
    *v == 4
}
fn is_default_dt(v: &f32) -> bool {
    *v == 1.0
}
fn is_default_grid(v: &SimGrid) -> bool {
    *v == SimGrid::default()
}
fn is_default_init(v: &SimInit) -> bool {
    *v == SimInit::default()
}
fn is_default_boundary(v: &SimBoundary) -> bool {
    *v == SimBoundary::default()
}
fn is_default_upscale(v: &SimUpscale) -> bool {
    *v == SimUpscale::default()
}
fn is_default_downscale(v: &SimDownscale) -> bool {
    *v == SimDownscale::default()
}
fn is_empty_map(m: &BTreeMap<String, f32>) -> bool {
    m.is_empty()
}

/// Simulation-mode settings. See the module docs.
///
/// `PartialEq` is load-bearing: `is_default` compares against
/// `Self::default()`, the same pattern `EscapeConfig` uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimConfig {
    /// Which model to step, by registry name (`"gray_scott"`, …). A
    /// name the build doesn't know steps the default model with a
    /// warning rather than failing the load — the same
    /// forward-compatibility posture as escape formulas.
    #[serde(default = "default_model", skip_serializing_if = "is_default_model")]
    pub model: String,

    /// Which colouring maps the field to a palette coordinate.
    #[serde(default = "default_coloring", skip_serializing_if = "is_default_coloring")]
    pub coloring: String,

    /// Cell count, or how to derive it from the output size.
    #[serde(default, skip_serializing_if = "is_default_grid")]
    pub grid: SimGrid,

    /// Seed for the initial field and for every stochastic model. With
    /// the model, the init and the step count, this is what makes a
    /// still reproducible.
    #[serde(default = "default_seed", skip_serializing_if = "is_default_seed")]
    pub seed: u64,

    /// The initial field.
    #[serde(default, skip_serializing_if = "is_default_init")]
    pub init: SimInit,

    /// Exactly how many steps a still is. Not a hint: the renderer
    /// runs this many from the seed and stops.
    #[serde(default = "default_steps", skip_serializing_if = "is_default_steps")]
    pub steps: u32,

    /// Steps advanced per displayed frame while running, and per video
    /// frame on export.
    #[serde(
        default = "default_steps_per_frame",
        skip_serializing_if = "is_default_steps_per_frame"
    )]
    pub steps_per_frame: u32,

    /// Model time step, where the model has one. Capped per model by
    /// its measured stability bound (catalogue).
    #[serde(default = "default_dt", skip_serializing_if = "is_default_dt")]
    pub dt: f32,

    /// What a step reads outside the grid.
    #[serde(default, skip_serializing_if = "is_default_boundary")]
    pub boundary: SimBoundary,

    /// Per-model parameters, keyed `"name"` (see module docs).
    #[serde(default, skip_serializing_if = "is_empty_map")]
    pub model_params: BTreeMap<String, f32>,

    /// Per-colouring parameters, keyed the same way.
    #[serde(default, skip_serializing_if = "is_empty_map")]
    pub coloring_params: BTreeMap<String, f32>,

    /// Resolve filter when the grid is smaller than the output.
    #[serde(default, skip_serializing_if = "is_default_upscale")]
    pub upscale: SimUpscale,

    /// Resolve filter when the grid is larger than the output.
    #[serde(default, skip_serializing_if = "is_default_downscale")]
    pub downscale: SimDownscale,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            coloring: default_coloring(),
            grid: SimGrid::default(),
            seed: default_seed(),
            init: SimInit::default(),
            steps: default_steps(),
            steps_per_frame: default_steps_per_frame(),
            dt: default_dt(),
            boundary: SimBoundary::default(),
            model_params: BTreeMap::new(),
            coloring_params: BTreeMap::new(),
            upscale: SimUpscale::default(),
            downscale: SimDownscale::default(),
        }
    }
}

impl SimConfig {
    /// Whether this is untouched, for the `FractalConfig` field skip.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// A model parameter, or the registry's default when unset.
    pub fn model_param(&self, name: &str, fallback: f32) -> f32 {
        self.model_params.get(name).copied().unwrap_or(fallback)
    }

    /// A colouring parameter, or the registry's default when unset.
    pub fn coloring_param(&self, name: &str, fallback: f32) -> f32 {
        self.coloring_params.get(name).copied().unwrap_or(fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the skip-if-default discipline: a config that
    /// has never entered simulation mode must serialise to nothing, so
    /// every existing flame and escape file is byte-identical.
    #[test]
    fn default_serialises_to_an_empty_object() {
        let json = serde_json::to_string(&SimConfig::default()).unwrap();
        assert_eq!(json, "{}", "a default SimConfig must add nothing to a config file");
    }

    #[test]
    fn round_trips_through_json() {
        let mut c = SimConfig::default();
        c.model = "gray_scott".into();
        c.grid = SimGrid::Fixed { width: 512, height: 384 };
        c.seed = 99;
        c.init = SimInit::Blob { radius: 12 };
        c.steps = 12_345;
        c.dt = 0.5;
        c.boundary = SimBoundary::Zero;
        c.model_params.insert("feed".into(), 0.0545);
        c.model_params.insert("kill".into(), 0.062);
        c.coloring_params.insert("gain".into(), 2.0);
        c.upscale = SimUpscale::Bilinear;
        c.downscale = SimDownscale::Nearest;
        let json = serde_json::to_string(&c).unwrap();
        let back: SimConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    /// Fields absent from an older file must come back as the defaults,
    /// not as zeros — a `steps` of 0 would render an empty grid.
    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let c: SimConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(c, SimConfig::default());
        assert_eq!(c.steps, 4000);
        assert_eq!(c.dt, 1.0);
        assert_eq!(c.model, "gray_scott");
    }

    #[test]
    fn viewport_grid_scales_and_fixed_grid_does_not() {
        let bound = SimGrid::Viewport { scale: 0.5 };
        assert_eq!(bound.cells_for(1920, 1080), (960, 540));
        assert!(bound.is_bound());
        let fixed = SimGrid::Fixed { width: 256, height: 256 };
        assert_eq!(fixed.cells_for(1920, 1080), (256, 256));
        assert_eq!(fixed.cells_for(64, 64), (256, 256));
        assert!(!fixed.is_bound());
    }

    /// A scale that arrived as NaN from an animation track must not
    /// produce a zero-sized or absurd grid.
    #[test]
    fn a_non_finite_scale_falls_back_rather_than_producing_a_bad_grid() {
        let bad = SimGrid::Viewport { scale: f32::NAN };
        assert_eq!(bad.cells_for(800, 600), (800, 600));
        let huge = SimGrid::Viewport { scale: 1e9 };
        let (w, h) = huge.cells_for(1920, 1080);
        assert_eq!((w, h), (7680, 4320), "scale clamps to 4x");
    }

    #[test]
    fn init_kind_switching_keeps_what_the_kinds_share() {
        let blobs = SimInit::Blobs { count: 3, radius: 40 };
        assert_eq!(blobs.with_kind("blob"), SimInit::Blob { radius: 40 });
        assert_eq!(blobs.with_kind("ring"), SimInit::Ring { radius: 40 });
        assert_eq!(blobs.with_kind("line"), SimInit::Line);
        // ...and going back restores the count alongside the radius.
        let ring = SimInit::Ring { radius: 40 };
        assert_eq!(ring.with_kind("blobs"), SimInit::Blobs { count: 6, radius: 40 });
    }


    // ---- ConfigPath / manager integration -------------------------
    //
    // These live here rather than in delta.rs because what they assert
    // is about SimConfig's shape: that every path round-trips through a
    // string key, that the three update types are routed by how much of
    // a run survives, and that the clamps are the ones phase 0
    // measured.

    use crate::config::delta::{ConfigPath, ConfigValue, UpdateType};

    /// Every Sim path must survive `to_string_key` -> `from_string_key`.
    /// Animation tracks address paths BY THAT STRING, so a path that
    /// does not round-trip is a track that silently stops working.
    #[test]
    fn every_sim_path_round_trips_through_its_string_key() {
        let paths = vec![
            ConfigPath::SimModel,
            ConfigPath::SimColoring,
            ConfigPath::SimGridMode,
            ConfigPath::SimGridWidth,
            ConfigPath::SimGridHeight,
            ConfigPath::SimGridScale,
            ConfigPath::SimSeed,
            ConfigPath::SimInitKind,
            ConfigPath::SimInitAmplitude,
            ConfigPath::SimInitRadius,
            ConfigPath::SimInitCount,
            ConfigPath::SimSteps,
            ConfigPath::SimStepsPerFrame,
            ConfigPath::SimDt,
            ConfigPath::SimBoundary,
            ConfigPath::SimUpscale,
            ConfigPath::SimDownscale,
            ConfigPath::SimModelParam { param: "feed".into() },
            ConfigPath::SimColoringParam { param: "scale".into() },
        ];
        for path in paths {
            let key = path.to_string_key();
            assert!(key.starts_with("Sim."), "{key} should be namespaced");
            let back = ConfigPath::from_string_key(&key)
                .unwrap_or_else(|| panic!("{key} did not parse back"));
            assert_eq!(back, path, "{key} round-tripped to a different path");
        }
    }

    /// The three update types exist to protect a long run from a cheap
    /// edit. A colouring change that reseeded would throw away a
    /// 10,000-step field; a model change that only recoloured would
    /// show the old rule's output.
    #[test]
    fn update_types_are_routed_by_how_much_of_the_run_survives() {
        for p in [
            ConfigPath::SimColoring,
            ConfigPath::SimSteps,
            ConfigPath::SimDt,
            ConfigPath::SimUpscale,
            ConfigPath::SimModelParam { param: "feed".into() },
            ConfigPath::SimColoringParam { param: "scale".into() },
        ] {
            assert_eq!(p.update_type(), UpdateType::SimRerender, "{p:?}");
        }
        assert_eq!(ConfigPath::SimGridScale.update_type(), UpdateType::SimResample);
        for p in [
            ConfigPath::SimModel,
            ConfigPath::SimSeed,
            ConfigPath::SimInitKind,
            ConfigPath::SimInitRadius,
            ConfigPath::SimGridWidth,
            ConfigPath::SimGridHeight,
            ConfigPath::SimGridMode,
            ConfigPath::SimBoundary,
        ] {
            assert_eq!(p.update_type(), UpdateType::SimReseed, "{p:?}");
        }
    }

    /// `merge` takes the Ord max, so a change set that both recolours
    /// and reseeds must reseed. Getting this backwards would leave the
    /// old field on screen after a model change.
    #[test]
    fn a_reseed_subsumes_a_rerender_when_merged() {
        assert!(UpdateType::SimReseed > UpdateType::SimRerender);
        assert!(UpdateType::SimReseed > UpdateType::SimResample);
        assert!(UpdateType::SimResample > UpdateType::SimRerender);
    }

    /// Sim.Steps is the track that animates the simulation itself
    /// (master plan D5b), so it must convert from JSON; a model name
    /// must not, because there is no value between two names.
    #[test]
    fn the_step_count_is_animatable_and_the_discrete_choices_are_not() {
        use crate::config::delta::json_to_config_value;
        let n = serde_json::json!(1234);
        assert_eq!(
            json_to_config_value(&n, &ConfigPath::SimSteps),
            Some(ConfigValue::UInt(1234)),
            "Sim.Steps must be animatable -- it IS the progression"
        );
        assert!(json_to_config_value(&n, &ConfigPath::SimModel).is_none());
        assert!(json_to_config_value(&n, &ConfigPath::SimBoundary).is_none());
        assert!(json_to_config_value(&n, &ConfigPath::SimSeed).is_none());
    }

    #[test]
    fn every_enum_name_round_trips() {
        for n in SimBoundary::NAMES {
            assert_eq!(SimBoundary::from_name(n).unwrap().name(), *n);
        }
        for n in SimUpscale::NAMES {
            assert_eq!(SimUpscale::from_name(n).unwrap().name(), *n);
        }
        for n in SimDownscale::NAMES {
            assert_eq!(SimDownscale::from_name(n).unwrap().name(), *n);
        }
        for k in SimInit::KINDS {
            assert_eq!(SimInit::default().with_kind(k).kind_name(), *k);
        }
    }
}
