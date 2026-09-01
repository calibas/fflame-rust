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
pub mod presets;
pub mod diag;
pub mod renderer;

pub use renderer::{DerivativeGap, EscapeRenderer, UsableDepth};

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
    /// The step needs the ITERATION INDEX, appended to its signature
    /// as `i: u32`.
    ///
    /// For a map whose rule CHANGES from step to step rather than
    /// being the same function iterated — Origami folds the plane
    /// along a different line each time. Without it such a formula can
    /// only repeat one fixed sequence, and the orbit dies as soon as
    /// the point reaches the region every fold leaves alone.
    NeedsIndex,
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
    /// The map has NO parameter `c`: its step ignores the argument and
    /// it seeds at the pixel, so the parameter plane and the dynamical
    /// plane are the same picture and the Julia toggle does nothing.
    ///
    /// Origami folds the plane along a fixed sequence of lines;
    /// Newton, Collatz and Lattès iterate a fixed rational map. For
    /// all four the pixel is the STARTING POINT, not a parameter, and
    /// offering "Julia mode" beside them only invites the user to
    /// toggle something inert. `formula_julia_is_meaningful` is the
    /// gate, and a test keeps the flag honest against the WGSL.
    DynamicalOnly,
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
    /// The coloring's value is BOUNDED to 0..1 and means a level, not
    /// a position along a repeating ramp — a lighting term, say.
    ///
    /// The template wraps a coloring's value with `fract` so that
    /// unbounded ones (escape count, smooth iteration) cycle through
    /// the palette as they grow. For a bounded one that wrap is a bug
    /// at exactly one value: 1.0 wraps to 0.0, so the brightest points
    /// come out the darkest colour. It shows as a thin dark seam
    /// through the highlight — which is how it was found, in the
    /// normal-map shading where the seam traces the points whose
    /// normal aims straight at the light.
    ///
    /// With this flag the value is CLAMPED instead, so out-of-range
    /// values saturate the way an exposure control does.
    Bounded,
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
    /// Labels for a DISCRETE parameter, in index order; empty for a
    /// continuous one.
    ///
    /// The stored value is unchanged — still the same `f32` index the
    /// shader reads, still `min..max` — so this is presentation only.
    /// What it fixes is that a set of named alternatives (Burning Ship
    /// vs Celtic vs Buffalo; point vs cross vs circle) was being
    /// offered as a continuum, where the user could land on 2.4 and
    /// get variant 2 with no way to tell which choices existed short
    /// of reading the tooltip.
    pub choices: &'static [&'static str],
}

/// One named starting point for a formula.
///
/// A formula's identity is not just its step: Origami wants a
/// position map at zoom -1 with 32 iterations, Newton wants root
/// basins at 64, the Mandelbrot wants smooth at 512. None of that
/// carries over from whatever you were looking at a moment ago, which
/// is why switching formulas used to leave a stale view over an
/// unsuitable coloring. A preset carries all of it together.
#[derive(Clone, Copy, Debug)]
pub struct EscapePreset {
    pub name: &'static str,
    /// Exact decimal centre, as `EscapeConfig` stores it.
    pub center_re: &'static str,
    pub center_im: &'static str,
    pub zoom_log2: f64,
    pub max_iter: u32,
    pub coloring: &'static str,
    /// `Some((re, im))` selects the dynamical plane with that seed.
    pub julia: Option<(f32, f32)>,
    pub formula_params: &'static [(&'static str, f32)],
    pub coloring_params: &'static [(&'static str, f32)],
    /// Escape radius, when the preset needs one other than the
    /// config default. `None` leaves the current value alone.
    ///
    /// Root-finders are why this exists: Newton's iterates wander far
    /// outside the unit disc before settling, and a function whose
    /// ROOTS lie past the default bailout (z^8 + 15z^4 - 16 has four
    /// at |z| = 2) would have every one of them classified as an
    /// escape — the basins vanish and the view renders flat.
    pub bailout: Option<f32>,
}

/// A formula's default starting point: its first preset, if it has
/// one. This is what a switch to the formula applies.
pub fn formula_default_preset(formula: &FormulaDef) -> Option<&'static EscapePreset> {
    formula.presets.first()
}

/// The same, for a mode-B field.
pub fn field_default_preset(field: &fields::FieldDef) -> Option<&'static EscapePreset> {
    field.presets.first()
}

/// Which of the iteration controls the assembled shader will read.
///
/// Every one of these is spliced conditionally, so a control the
/// splice omits does nothing at all — the same silent-inert problem
/// the coloring gate solves, in a different corner of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IterationControls {
    /// `bailout` is read only by the escape test, which the assembler
    /// compiles in only for an ESCAPING formula.
    pub bailout: bool,
    /// The biomorph axis lives INSIDE that same escape test.
    pub biomorph: bool,
    /// Mann damping is spliced into the step of any mode-A formula,
    /// escaping or not — it changes the iteration, not the test.
    pub damping: bool,
}

/// What a mode-A formula's shader actually reads.
pub fn iteration_controls(formula: &FormulaDef) -> IterationControls {
    let escapes = !formula.has_feature(FormulaFeature::NonEscaping);
    IterationControls { bailout: escapes, biomorph: escapes, damping: true }
}

/// What a mode-B FIELD shader reads: none of them.
///
/// A field runs a fixed-count accumulation loop with no escape test,
/// no bailout and no step to damp — `max_iter` is a TERM COUNT there.
/// The three controls sat in the panel doing nothing.
pub const FIELD_ITERATION_CONTROLS: IterationControls =
    IterationControls { bailout: false, biomorph: false, damping: false };

/// Whether a Julia toggle means anything for this formula.
///
/// False for a map with no `c` (see
/// [`FormulaFeature::DynamicalOnly`]): both planes render the same
/// image, so the control is inert and the UI hides it.
pub fn formula_julia_is_meaningful(formula: &FormulaDef) -> bool {
    !formula.has_feature(FormulaFeature::DynamicalOnly)
}

/// Whether `coloring` can produce a picture for `formula`.
///
/// Two ways a pairing is dead, both of them silent without this:
///
/// 1. A map that never sets `escaped` leaves the templates'
///    `escaped || COLORING_COLORS_INTERIOR` false everywhere, so an
///    escape-time coloring over it renders BLACK — not a poor image,
///    no image. This is the "Orbit Trapping fractals can't use escape
///    time coloring" case.
///
///    `NonEscaping` alone does NOT mean that: a CONVERGENT formula
///    has no bailout test but still sets `escaped` when its orbit
///    settles, which is what lets escape-count and smooth shade
///    convergence SPEED (Novaretti's shipped look). Only a formula
///    that is non-escaping and non-convergent truly never escapes.
///    Getting this wrong hid a coloring that works — caught by the
///    preset smoke test, which found a shipped config the gate had
///    just declared impossible.
/// 2. A DERIVATIVE coloring over a formula that supplies no
///    derivative degrades to a flat constant (the shader returns 0.5
///    rather than divide by a derivative that is really the number
///    one).
///
/// Both are properties of the assembled shader, not of taste, which
/// is why this lives beside the registry rather than in the panel.
pub fn coloring_suits_formula(formula: &FormulaDef, coloring: &ColoringDef) -> bool {
    let never_escapes = formula.has_feature(FormulaFeature::NonEscaping)
        && !formula.has_feature(FormulaFeature::Convergent);
    if never_escapes && !coloring.has_feature(ColoringFeature::ColorsInterior) {
        return false;
    }
    if coloring.has_feature(ColoringFeature::NeedsDerivative)
        && formula.wgsl_derivative.is_empty()
    {
        return false;
    }
    true
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
    /// Computes per-render data on the CPU from the RESOLVED formula
    /// params (slot-ordered, defaults applied) and uploads it into the
    /// uniform's `fdata` array — read in WGSL via `fdata4(i)`. At most
    /// 256 floats (64 vec4s); shorter output is zero-padded. For
    /// tables every pixel would otherwise recompute identically:
    /// Origami's fold lines are the reason this exists — building
    /// them per thread cost ~1000 fold ops and a 1 KB private array
    /// per pixel, slow everywhere and a TDR hang under supersampling.
    pub derived_data: Option<fn(&[f32]) -> Vec<f32>>,
    /// WGSL defining
    /// `fn formula_derivative(z: vec2<f32>, c: vec2<f32>, dz: vec2<f32>, is_julia: bool) -> vec2<f32>`
    /// — the derivative-orbit update evaluated at the PRE-step iterate
    /// (parameter plane: d/dc, so the chain rule carries a `+ f_c`
    /// term; Julia: d/dz₀, no inhomogeneous term). Empty ⇒ no
    /// derivative available (abs-folds, anti-holomorphic maps);
    /// distance estimation degrades there.
    pub wgsl_derivative: &'static str,
    /// Named starting points (see [`EscapePreset`]). The FIRST is the
    /// formula's default, applied when the user switches to it.
    /// Empty is allowed; the switch then leaves the view alone.
    pub presets: &'static [EscapePreset],
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
    &formulas::LAMBDA_SINE,
    &formulas::ORIGAMI,
    &formulas::LATTES,
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
    &colorings::NORMAL_MAP,
    &colorings::POSITION_AVERAGE,
    &colorings::POSITION_MAP,
    &colorings::SPHERE_AVERAGE,
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

    /// A discrete parameter's labels must cover its range EXACTLY.
    ///
    /// The labels are UI-side but the range is what the shader clamps
    /// against, so the two can drift apart silently: add a sixth
    /// Burning Ship variant to the WGSL and bump `max`, forget the
    /// label, and the dropdown quietly cannot reach it. Requiring
    /// `max == choices.len() - 1` makes that a build failure instead.
    #[test]
    fn discrete_params_are_labelled_across_their_whole_range() {
        let mut checked = 0;
        let mut params: Vec<(&str, &EscapeParamDef)> = Vec::new();
        for f in FORMULAS {
            params.extend(f.parameters.iter().map(|p| (f.name, p)));
        }
        for c in COLORINGS {
            params.extend(c.parameters.iter().map(|p| (c.name, p)));
        }
        for f in fields::FIELDS {
            params.extend(f.parameters.iter().map(|p| (f.name, p)));
        }
        for c in fields::FIELD_COLORINGS {
            params.extend(c.parameters.iter().map(|p| (c.name, p)));
        }
        for (owner, p) in params {
            if p.choices.is_empty() {
                continue;
            }
            checked += 1;
            assert_eq!(
                p.min, 0.0,
                "{owner}.{}: a choice list indexes from 0, so min must be 0",
                p.name
            );
            assert_eq!(
                p.max,
                (p.choices.len() - 1) as f32,
                "{owner}.{}: {} labels but the range runs to {} — the dropdown and the \
                 shader's clamp disagree",
                p.name,
                p.choices.len(),
                p.max
            );
            assert!(
                p.default >= 0.0 && p.default <= p.max && p.default.fract() == 0.0,
                "{owner}.{}: default {} is not one of the choices",
                p.name,
                p.default
            );
            assert!(
                p.choices.iter().all(|c| !c.trim().is_empty()),
                "{owner}.{}: a blank choice label",
                p.name
            );
        }
        assert!(checked >= 15, "expected the known discrete params, found {checked}");
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
