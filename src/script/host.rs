//! Script engine host: sandbox setup, the two-phase run, seeded RNG.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rand_pcg::Pcg64Mcg;
use rhai::{Engine, Scope};

use crate::config::fractal_config::FractalConfig;

use super::{ParamDecl, ParamValue, ScriptError, ScriptKind, ScriptMeta};

/// Which pass is running.
///
/// The same registered functions serve both; only `param*()` behaves
/// differently (records a declaration vs. returns the supplied value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Gather metadata and parameter declarations; result discarded.
    Collect,
    /// The real run, using values supplied for the declared parameters.
    Run,
}

/// Execution budgets. A shared script must not be able to hang the app,
/// so a runaway `loop {}` dies on the operation counter rather than
/// spinning forever. Generation workloads are thousands of operations;
/// the ceiling is far above anything legitimate.
const MAX_OPERATIONS: u64 = 5_000_000;
const MAX_CALL_LEVELS: usize = 64;
const MAX_ARRAY_SIZE: usize = 100_000;
const MAX_STRING_SIZE: usize = 1_000_000;
const MAX_MAP_SIZE: usize = 10_000;

/// Mutable state shared between the engine's registered functions.
pub(crate) struct ScriptState {
    pub mode: Mode,
    pub meta: ScriptMeta,
    /// Values supplied for declared parameters (UI sliders / `--set`).
    pub provided: HashMap<String, ParamValue>,
    /// Keys already declared, to reject duplicates.
    pub declared: HashSet<String>,
    pub warnings: Vec<String>,
    /// Anything the script sent to print()/debug(). Stdout is invisible
    /// in-app and on the web, so the caller surfaces these instead.
    pub messages: Vec<String>,
    /// Palettes a script may choose from. Empty unless the caller
    /// supplied a library — scripts then get a clear error rather than a
    /// silently different result.
    pub palettes: Vec<crate::scene::palette::Palette>,
    pub rng: Pcg64Mcg,
    /// What the script asked to animate. Stays untouched — and so
    /// produces no animation at all — unless the script uses `anim`.
    pub anim: super::anim::AnimBuilder,
}

impl ScriptState {
    fn new(
        mode: Mode,
        seed: u64,
        provided: HashMap<String, ParamValue>,
        palettes: Vec<crate::scene::palette::Palette>,
    ) -> Self {
        Self {
            mode,
            meta: ScriptMeta::default(),
            provided,
            declared: HashSet::new(),
            warnings: Vec::new(),
            messages: Vec::new(),
            palettes,
            // Pinned algorithm (PCG64-MCG), not StdRng: script + seed must
            // reproduce the same flame across platforms and across rand
            // crate versions.
            rng: Pcg64Mcg::new(expand_seed(seed)),
            anim: super::anim::AnimBuilder::default(),
        }
    }

    /// Record a declaration, or fetch the supplied value for it.
    pub(crate) fn declare(&mut self, decl: ParamDecl) -> Result<ParamValue, String> {
        let key = decl.key().to_string();
        if !self.declared.insert(key.clone()) {
            return Err(format!("parameter `{key}` declared more than once"));
        }
        let fallback = match &decl {
            ParamDecl::Float { default, .. } => ParamValue::Float(*default),
            ParamDecl::Int { default, .. } => ParamValue::Int(*default),
            ParamDecl::Bool { default, .. } => ParamValue::Bool(*default),
            ParamDecl::Color { default, .. } => ParamValue::Color(*default),
            ParamDecl::Choice { default, .. } => ParamValue::Choice(*default),
            ParamDecl::Text { default, .. } => ParamValue::Text(default.clone()),
        };
        self.meta.params.push(decl);
        if self.mode == Mode::Collect {
            return Ok(fallback);
        }
        Ok(self.provided.get(&key).cloned().unwrap_or(fallback))
    }
}

/// The result of a successful run.
#[derive(Debug)]
pub struct ScriptOutcome {
    pub config: FractalConfig,
    pub meta: ScriptMeta,
    /// Non-fatal problems worth surfacing (unknown supplied params, …).
    pub warnings: Vec<String>,
    /// Script print()/debug() output, in order.
    pub messages: Vec<String>,
    /// The animation the script defined, if it defined one. Carries the
    /// flame above as its `base_config`, so it stands alone.
    pub animation: Option<crate::animation::Animation>,
}

/// Runs sandboxed flame scripts.
#[derive(Default)]
pub struct ScriptHost {
    /// Palettes `flame.set_palette` / `flame.random_palette` draw from.
    palettes: Vec<crate::scene::palette::Palette>,
}

impl ScriptHost {
    /// A host with no palette library: scripts keep whatever palette the
    /// base config already has.
    pub fn new() -> Self {
        Self::default()
    }

    /// A host that lets scripts choose from `palettes`.
    ///
    /// Selection uses the script's seeded RNG, so "pick a random palette"
    /// still reproduces from script + seed — unlike the Rust random
    /// generator, which draws from the thread RNG.
    pub fn with_palettes(palettes: Vec<crate::scene::palette::Palette>) -> Self {
        Self { palettes }
    }

    /// Collect metadata and parameter declarations without keeping the
    /// generated config. `base` is the config the script would run
    /// against, so a Modifier inspecting the current flame behaves the
    /// same way it will on the real run.
    pub fn collect(&self, text: &str, base: &FractalConfig) -> Result<ScriptMeta, ScriptError> {
        let out = self.execute(text, base, 0, HashMap::new(), Mode::Collect)?;
        Ok(out.meta)
    }

    /// Run a script for real.
    ///
    /// `base` is the starting config — a default one for Generators, the
    /// current config for Modifiers (the caller decides; the script's
    /// declared kind is advisory metadata, returned in the outcome).
    pub fn run(
        &self,
        text: &str,
        base: &FractalConfig,
        seed: u64,
        params: HashMap<String, ParamValue>,
    ) -> Result<ScriptOutcome, ScriptError> {
        self.execute(text, base, seed, params, Mode::Run)
    }

    fn execute(
        &self,
        text: &str,
        base: &FractalConfig,
        seed: u64,
        params: HashMap<String, ParamValue>,
        mode: Mode,
    ) -> Result<ScriptOutcome, ScriptError> {
        let cfg = Rc::new(RefCell::new(base.clone()));
        let state = Rc::new(RefCell::new(ScriptState::new(
            mode,
            seed,
            params,
            self.palettes.clone(),
        )));

        // Built per run: the registered closures capture this run's
        // config and state.
        let engine = build_engine(Rc::clone(&cfg), Rc::clone(&state));

        let mut scope = Scope::new();
        super::api::push_globals(&mut scope, Rc::clone(&cfg), Rc::clone(&state));

        engine.run_with_scope(&mut scope, text).map_err(map_error)?;

        let mut state = state.borrow_mut();

        // Supplied values nobody declared: usually a typo, or a param
        // hidden behind an `if` that collect mode didn't reach.
        let declared: HashSet<String> = state.declared.clone();
        let unknown: Vec<String> = state
            .provided
            .keys()
            .filter(|k| !declared.contains(*k))
            .cloned()
            .collect();
        for key in unknown {
            state
                .warnings
                .push(format!("value supplied for undeclared parameter `{key}` (ignored)"));
        }

        if state.meta.kind.is_none() {
            state.warnings.push(
                "script did not call script(name, kind) — defaulting to a generator".to_string(),
            );
            state.meta.kind = Some(ScriptKind::Generator);
        }

        let config = cfg.borrow().clone();
        // Built here rather than as the script runs: duration can default
        // to the last keyframe, which isn't known until the script ends.
        let script_name = state.meta.name.clone();
        let anim = state.anim.clone();
        let animation = anim.build(&config, &script_name, &mut state.warnings);
        Ok(ScriptOutcome {
            config,
            meta: state.meta.clone(),
            warnings: std::mem::take(&mut state.warnings),
            messages: std::mem::take(&mut state.messages),
            animation,
        })
    }
}

fn build_engine(cfg: Rc<RefCell<FractalConfig>>, state: Rc<RefCell<ScriptState>>) -> Engine {
    let mut engine = Engine::new();

    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_call_levels(MAX_CALL_LEVELS);
    engine.set_max_array_size(MAX_ARRAY_SIZE);
    engine.set_max_string_size(MAX_STRING_SIZE);
    engine.set_max_map_size(MAX_MAP_SIZE);
    // No dynamic code generation: scripts are shared artifacts, and
    // eval would let one smuggle in constructs past review.
    engine.disable_symbol("eval");

    super::api::register(&mut engine, cfg, state);
    engine
}

/// Expand a user-facing seed into PCG64-MCG's 128-bit state.
///
/// The multiplicative generator needs an ODD state, but forcing the low
/// bit on the raw seed maps 8842 and 8843 onto the *same* stream — which
/// would quietly break "reroll = seed + 1", the most common way anyone
/// will use this. SplitMix64 (two rounds, one per 64-bit half) scrambles
/// first, so consecutive seeds land far apart.
///
/// Implemented here rather than via `SeedableRng::seed_from_u64` so the
/// mapping is ours and can never shift under a dependency bump — script
/// + seed is a shareable artifact and must mean the same thing forever.
fn expand_seed(seed: u64) -> u128 {
    let mut z = seed;
    let mut next = || {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut x = z;
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    };
    let lo = next() as u128;
    let hi = next() as u128;
    ((hi << 64) | lo) | 1
}

/// Map a Rhai failure to our error type, keeping source position.
fn map_error(err: Box<rhai::EvalAltResult>) -> ScriptError {
    let pos = err.position();
    // Rhai appends its own position suffix to Display; strip it so the
    // message doesn't read "… (line 7, position 3) (line 7:3)".
    let raw = err.to_string();
    let message = match raw.find(" (line ") {
        Some(i) => raw[..i].to_string(),
        None => raw,
    };
    ScriptError { message, line: pos.line(), column: pos.position() }
}
