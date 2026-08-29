//! Escape-time fractal rendering (Mandelbrot and kin).
//!
//! The parallel system to the flame pipeline described in
//! `docs/projects/escape-time-fractals.md`. Where the flame renderer
//! runs a chaos game into a histogram, this mode evaluates a per-pixel
//! iteration directly — one compute pass, no accumulation loop — and
//! writes an `Rgba32Float` image shaped exactly like the flame
//! accumulator (`rgb` = mean color, `a` = density), so the existing
//! tonemap → effects → readback tail consumes it unchanged.
//!
//! Architecture mirrors the variation system deliberately:
//! * [`FormulaDef`] / [`ColoringDef`] are `static` definitions with
//!   inline WGSL and self-describing parameters (the panel UI
//!   auto-generates sliders from them, the way variation params do).
//! * Registries are ordered slices; **append-only**, name-addressed.
//! * The assembler splices exactly one formula and one coloring into a
//!   small template — per-combination shaders, cached by the renderer.
//!
//! Unlike variations there is no index-mapping problem: a pipeline
//! holds ONE formula and ONE coloring, so WGSL function names are
//! fixed (`formula_step`, `coloring_map`, `coloring_accum`) rather
//! than prefixed.

pub mod assembler;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod app_repro_test;
#[cfg(test)]
mod gpu_bignum_probe;
pub mod colorings;
pub mod bigfloat;
pub mod bla;
pub mod fixedpoint;
pub mod nucleus;
#[cfg(not(target_arch = "wasm32"))]
pub mod orbit_store;
pub mod reference;
pub mod fields;
pub mod formulas;
pub mod renderer;

pub use renderer::{EscapeRenderer, UsableDepth};

/// Capability flags a formula can opt into — same pattern as the
/// variation system's `Feature` enum (a slice on the def, absence ⇒
/// doesn't have it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulaFeature {
    /// The map has no escape: iterate exactly `max_iter` steps and
    /// color from the orbit accumulators (Kaliset — plan §5.12). The
    /// assembler compiles the bailout test out entirely.
    NonEscaping,
    /// The step reads the PREVIOUS iterate too (Phoenix: `z ← z² + c
    /// + p·z_prev`). The formula's `formula_step` gains a
    /// `z_prev: vec2<f32>` argument and the assembler compiles the
    /// history register into the loop; without the flag the loop
    /// carries no history at all.
    NeedsPrevZ,
    /// The step MUTATES `c` (Fractint Spider: `z ← z²+c; c ← c/2+z`).
    /// `c` becomes a `var` and `formula_step` receives it as
    /// `ptr<function, vec2<f32>>`; without the flag `c` stays a `let`
    /// and the signature is unchanged.
    MutatesC,
    /// The map CONVERGES (root-finders, Magnet's fixed point at 1,
    /// Novaretti's attracting cycles): the loop also terminates on
    /// `|z − z_prev|² < 1e-12`, recording `converged` for the
    /// colorings (root basins, convergence speed). Composes with the
    /// escape test — Magnet needs both. The convergence register is
    /// maintained independently of `NeedsPrevZ`.
    Convergent,
}

/// Which quantity the escape test compares against `bailout`
/// (plan §5.9: "the per-formula escape-test slot exists for these").
/// The runtime biomorph toggle still overrides either.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EscapeMetric {
    /// `|z|² > bailout` — the classic squared-norm test.
    #[default]
    NormSq,
    /// `Re z > bailout` (RAW, not squared): the exponential family,
    /// where |e^z| = e^(Re z), and tetration. Typical thresholds are
    /// ~50, not 4 — the formula tooltips say so.
    Re,
    /// `|Im z| > bailout` (RAW): the trig family and Collatz.
    AbsIm,
}

/// Capability flags for colorings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColoringFeature {
    /// The coloring reads the per-iteration orbit accumulator; the def
    /// must supply `wgsl_accum` + `accum_init`, and the assembler
    /// compiles the per-step update into the loop. Without this flag
    /// the loop carries no accumulator at all (Mandelbrot + smooth
    /// stays two multiplies per step).
    NeedsOrbitAccum,
    /// The coloring reads the derivative orbit |dz| (distance
    /// estimation). Compiled in only when the formula also supplies
    /// `wgsl_derivative`; otherwise the register stays at its seed and
    /// the coloring degrades (documented on the def).
    NeedsDerivative,
    /// The coloring reads the detected cycle length; the assembler
    /// compiles Brent-style period detection (power-of-two
    /// checkpoints, `|z − checkpoint|² < 1e-12`) into the loop. This
    /// is the periodic(k) channel §5.8/§5.17 call for; without the
    /// flag no detection code exists.
    NeedsPeriod,
    /// The coloring produces a palette position for interior
    /// (never-escaped) pixels too. Without it the template paints
    /// interior black — right for escape-count/smooth, wrong for
    /// traps and averages (and for NonEscaping formulas, where every
    /// pixel is "interior").
    ColorsInterior,
}

/// A parameter a formula or coloring exposes. Same shape as variation
/// parameters, minus the type zoo — everything is a float slot in v1
/// (integers and small enums ride as floats the way variation
/// `Integer`/`Enum` params ride their buffer).
pub struct EscapeParamDef {
    /// Key inside `EscapeConfig::formula_params` / `coloring_params`.
    pub name: &'static str,
    pub display_name: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub tooltip: &'static str,
}

/// A formula: the iterated step `z ← f(z, c)`.
///
/// The WGSL must define
/// `fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32>`
/// and may read its parameters via `fparam(slot)` — slots are assigned
/// in `parameters` order. The template provides complex helpers
/// (`esc_cmul`, `esc_cpow`). Additional helper functions must be
/// name-prefixed (`fn myformula_helper`) since colorings share the
/// module namespace.
pub struct FormulaDef {
    /// Registry name — the string `EscapeConfig::formula` stores.
    pub name: &'static str,
    pub display_name: &'static str,
    pub features: &'static [FormulaFeature],
    pub parameters: &'static [EscapeParamDef],
    pub wgsl: &'static str,
    /// WGSL expression seeding `z₀` on the PARAMETER plane (`c` =
    /// pixel). Empty ⇒ the origin — right when 0 is the critical
    /// point (Mandelbrot family). Formulas whose critical point is
    /// elsewhere (Lambda: 0.5) or for which 0 is degenerate
    /// (McMullen's pole, Kaliset's 0/0 — both seed `pixel`) override
    /// it. `pixel` is in scope.
    pub wgsl_param_seed: &'static str,
    /// WGSL expression initializing the `z_prev` history register
    /// (NeedsPrevZ only; ignored otherwise). Empty ⇒ zero, the
    /// Phoenix convention. Manowar starts its auxiliary at the seed:
    /// `"z"` (evaluated after `z` is seeded).
    pub wgsl_prev_init: &'static str,
    /// Which quantity the escape test compares (see [`EscapeMetric`]).
    pub escape_metric: EscapeMetric,
    /// WGSL defining
    /// `fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32>`
    /// — the derivative-orbit update evaluated at the PRE-step iterate
    /// (parameter plane: d/dc, so the chain rule carries a `+ f_c`
    /// term; Julia: d/dz₀, no inhomogeneous term). Empty ⇒ no
    /// derivative available (abs-folds, anti-holomorphic maps);
    /// distance estimation degrades there.
    pub wgsl_derivative: &'static str,
}

impl FormulaDef {
    pub fn has_feature(&self, f: FormulaFeature) -> bool {
        self.features.contains(&f)
    }
}

/// A coloring: maps the per-pixel orbit summary to a palette position.
///
/// The WGSL must define
/// `fn coloring_map(z: vec2<f32>, n: u32, escaped: bool, state: vec2<f32>) -> f32`
/// returning a palette coordinate (wrapped into [0,1) by the caller),
/// reading parameters via `cparam(slot)`. `state` is the orbit
/// accumulator (zero unless [`ColoringFeature::NeedsOrbitAccum`]).
pub struct ColoringDef {
    /// Registry name — the string `EscapeConfig::coloring` stores.
    pub name: &'static str,
    pub display_name: &'static str,
    pub features: &'static [ColoringFeature],
    pub parameters: &'static [EscapeParamDef],
    pub wgsl: &'static str,
    /// WGSL expression initializing the accumulator (e.g.
    /// `"vec2<f32>(1e30, 0.0)"` for a running min). Required with
    /// `NeedsOrbitAccum`, ignored otherwise.
    pub accum_init: &'static str,
    /// WGSL defining
    /// `fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32>`
    /// — the per-iteration accumulator update (`z_prev` is the
    /// pre-step iterate, `c` the current parameter). Required with
    /// `NeedsOrbitAccum`, ignored otherwise.
    pub wgsl_accum: &'static str,
}

impl ColoringDef {
    pub fn has_feature(&self, f: ColoringFeature) -> bool {
        self.features.contains(&f)
    }
}

/// Ordered formula registry. **Append-only** — UI ordering and any
/// future stable-ID scheme both read this order.
pub static FORMULAS: &[&FormulaDef] = &[
    &formulas::MANDELBROT,
    &formulas::MULTIBROT,
    &formulas::TRICORN,
    &formulas::BURNING_SHIP,
    &formulas::MCMULLEN,
    &formulas::KALISET,
    &formulas::PHOENIX,
    &formulas::LAMBDA,
    &formulas::SPIDER,
    &formulas::MANOWAR,
    &formulas::BARNSLEY,
    &formulas::CACTUS,
    &formulas::EXPONENTIAL,
    &formulas::TRIG,
    &formulas::DUCKS,
    &formulas::TETRATION,
    &formulas::COLLATZ,
    &formulas::FEATHER,
    &formulas::NEWTON,
    &formulas::NOVA,
    &formulas::MAGNET,
    &formulas::NOVARETTI,
    &formulas::LITTLEWOOD,
];

/// Ordered coloring registry. **Append-only.**
pub static COLORINGS: &[&ColoringDef] = &[
    &colorings::ESCAPE_COUNT,
    &colorings::SMOOTH,
    &colorings::ORBIT_TRAP,
    &colorings::ORBIT_AVERAGE,
    &colorings::STRIPE_AVERAGE,
    &colorings::ROOT_BASIN,
    &colorings::TRIANGLE_INEQUALITY,
    &colorings::PERIOD,
    &colorings::DISTANCE_ESTIMATE,
    &colorings::MAGNITUDE_AVERAGE,
];

/// Look up a formula by name. An unknown name renders the default
/// formula with a warning rather than failing the load — the same
/// forward-compatibility posture `EscapeConfig::formula` documents
/// (a file from a newer build should degrade, not error).
pub fn get_formula(name: &str) -> &'static FormulaDef {
    FORMULAS
        .iter()
        .find(|f| f.name == name)
        .copied()
        .unwrap_or_else(|| {
            log::warn!("Unknown escape formula '{name}', rendering '{}'", FORMULAS[0].name);
            FORMULAS[0]
        })
}

/// Look up a coloring by name, with the same unknown-name fallback.
pub fn get_coloring(name: &str) -> &'static ColoringDef {
    COLORINGS
        .iter()
        .find(|c| c.name == name)
        .copied()
        .unwrap_or_else(|| {
            log::warn!("Unknown escape coloring '{name}', using '{}'", COLORINGS[0].name);
            COLORINGS[0]
        })
}

/// Pack a keyed param map into slot order for one def, filling absent
/// keys with defaults. Slot `i` is `parameters[i]` — the contract the
/// WGSL `fparam`/`cparam` helpers index against.
pub fn pack_params(
    defs: &[EscapeParamDef],
    values: &std::collections::BTreeMap<String, f32>,
    out: &mut [f32],
) {
    for (i, def) in defs.iter().enumerate() {
        if i >= out.len() {
            // More params than GPU slots — a def bug, not a user state.
            log::error!("Escape param '{}' exceeds the {}-slot budget", def.name, out.len());
            break;
        }
        out[i] = values.get(def.name).copied().unwrap_or(def.default);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_unique_and_lookup_works() {
        let mut seen = std::collections::HashSet::new();
        for f in FORMULAS {
            assert!(seen.insert(f.name), "duplicate formula name {}", f.name);
            assert_eq!(get_formula(f.name).name, f.name);
        }
        seen.clear();
        for c in COLORINGS {
            assert!(seen.insert(c.name), "duplicate coloring name {}", c.name);
            assert_eq!(get_coloring(c.name).name, c.name);
        }
    }

    #[test]
    fn unknown_names_fall_back_instead_of_failing() {
        // A file from a newer build must degrade, not error.
        assert_eq!(get_formula("from_the_future").name, FORMULAS[0].name);
        assert_eq!(get_coloring("from_the_future").name, COLORINGS[0].name);
    }

    #[test]
    fn config_defaults_name_real_registry_entries() {
        // EscapeConfig::default() says "mandelbrot"/"smooth"; if either
        // string drifts from the registry the default config would
        // silently render the fallback.
        let esc = crate::config::escape::EscapeConfig::default();
        assert!(FORMULAS.iter().any(|f| f.name == esc.formula));
        assert!(COLORINGS.iter().any(|c| c.name == esc.coloring));
    }

    #[test]
    fn pack_params_fills_defaults_and_overrides_in_slot_order() {
        let mut values = std::collections::BTreeMap::new();
        values.insert("scale".to_string(), 0.25f32);
        let mut out = [0.0f32; 4];
        pack_params(colorings::SMOOTH.parameters, &values, &mut out);
        assert_eq!(out[0], 0.25); // overridden
    }

    #[test]
    fn accum_colorings_carry_their_snippets() {
        for c in COLORINGS {
            if c.has_feature(ColoringFeature::NeedsOrbitAccum) {
                assert!(!c.accum_init.trim().is_empty(), "{} missing accum_init", c.name);
                assert!(
                    c.wgsl_accum.contains("fn coloring_accum"),
                    "{} missing coloring_accum",
                    c.name
                );
            }
        }
    }

    #[test]
    fn non_escaping_formulas_pair_with_interior_colorings() {
        // A NonEscaping formula never sets `escaped`; if no registered
        // coloring colors interior pixels, such a formula could only
        // ever render black.
        assert!(COLORINGS.iter().any(|c| c.has_feature(ColoringFeature::ColorsInterior)));
    }
}
