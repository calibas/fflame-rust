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
    /// Scripts this run may call, as (id, source). Empty unless the
    /// caller supplied a library, so `run_script` fails with a clear
    /// message rather than silently doing nothing.
    pub scripts: Vec<(String, String)>,
    /// Ids currently executing, outermost first. Both the cycle check
    /// and the depth cap read this.
    pub call_stack: Vec<String>,
    /// Operations reported by each live frame, and the total from frames
    /// that have finished. Rhai counts per evaluation, so a nested run
    /// starts from zero — summing the live frames is what stops a script
    /// buying a fresh budget by calling another one.
    pub frame_ops: Vec<u64>,
    pub ops_finished: u64,
    /// The seed this run started from, so a script can hand it to
    /// another and get a result that reproduces from the same number.
    pub seed: u64,
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
            scripts: Vec::new(),
            call_stack: Vec::new(),
            frame_ops: Vec::new(),
            ops_finished: 0,
            seed,
        }
    }

    /// Total operations charged so far: everything finished, plus the
    /// latest count from every frame still running.
    pub fn ops_total(&self) -> u64 {
        self.ops_finished + self.frame_ops.iter().sum::<u64>()
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
        if self.mode == Mode::Collect {
            self.meta.params.push(decl);
            return Ok(fallback);
        }
        let supplied = self.provided.get(&key).cloned();
        let value = match supplied {
            Some(v) => coerce_to_decl(&key, v, &decl)?,
            None => fallback,
        };
        self.meta.params.push(decl);
        Ok(value)
    }
}

/// Reconcile a supplied value with the declaration it lands on.
///
/// The DECLARATION decides the type, not the value: a caller cannot know
/// what the script it is calling declares, so `run_script("x", #{ scheme:
/// "Triadic" })` hands over a string where a choice is wanted. Resolving
/// it here means every route in — the panel, `--set`, Python, and one
/// script calling another — agrees, instead of each coercing its own way.
fn coerce_to_decl(key: &str, value: ParamValue, decl: &ParamDecl) -> Result<ParamValue, String> {
    Ok(match (decl, value) {
        // Already right.
        (ParamDecl::Float { .. }, v @ ParamValue::Float(_))
        | (ParamDecl::Int { .. }, v @ ParamValue::Int(_))
        | (ParamDecl::Bool { .. }, v @ ParamValue::Bool(_))
        | (ParamDecl::Text { .. }, v @ ParamValue::Text(_))
        | (ParamDecl::Color { .. }, v @ ParamValue::Color(_))
        | (ParamDecl::Choice { .. }, v @ ParamValue::Choice(_)) => v,

        // A choice named rather than numbered — how anyone would write
        // it, and unreadable as an index.
        (ParamDecl::Choice { options, .. }, ParamValue::Text(name)) => {
            let found = options.iter().position(|o| o.eq_ignore_ascii_case(name.trim()));
            match found {
                Some(i) => ParamValue::Choice(i),
                None => {
                    return Err(format!(
                        "`{key}` expects one of [{}], got `{name}`",
                        options.join(", ")
                    ))
                }
            }
        }
        (ParamDecl::Choice { options, .. }, ParamValue::Int(i)) => {
            let i = i.max(0) as usize;
            if i < options.len() {
                ParamValue::Choice(i)
            } else {
                return Err(format!(
                    "`{key}` expects one of [{}], got index {i}",
                    options.join(", ")
                ));
            }
        }

        // Whole numbers are numbers.
        (ParamDecl::Float { .. }, ParamValue::Int(i)) => ParamValue::Float(i as f64),
        (ParamDecl::Int { .. }, ParamValue::Float(f)) => ParamValue::Int(f.round() as i64),
        (ParamDecl::Color { .. }, ParamValue::Text(text)) => {
            ParamValue::Color(super::color::ScriptColor::from_hex(&text)?.to_rgb())
        }
        (ParamDecl::Text { .. }, other) => ParamValue::Text(match other {
            ParamValue::Float(f) => f.to_string(),
            ParamValue::Int(i) => i.to_string(),
            ParamValue::Bool(b) => b.to_string(),
            _ => return Err(format!("`{key}` expects text")),
        }),

        (_, other) => {
            return Err(format!("`{key}` was given an unusable value: {other:?}"))
        }
    })
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
    /// Scripts that `run_script` can call, as (id, source).
    scripts: Vec<(String, String)>,
}

impl ScriptHost {
    /// A host with no palette library: scripts keep whatever palette the
    /// base config already has.
    ///
    /// The EMBEDDED starters are always callable, though. They are
    /// compiled in, so there is no situation where they are missing —
    /// and a shipped script that calls another (Basic Random asks
    /// Random Palette for its colours) must work from every entry
    /// point, including the Python bindings and a bare embedder.
    /// `with_scripts` replaces this with a discovered library, which
    /// adds the user's own on top.
    pub fn new() -> Self {
        Self {
            scripts: super::library::EMBEDDED
                .iter()
                .map(|(name, src)| {
                    (name.trim_end_matches(".rhai").to_string(), (*src).to_string())
                })
                .collect(),
            ..Default::default()
        }
    }

    /// A host whose scripts may call each other by id.
    ///
    /// Without this a `run_script` call fails with a message saying so,
    /// rather than quietly doing nothing — the headless CLI and the app
    /// both supply the discovered library.
    pub fn with_scripts(mut self, scripts: Vec<(String, String)>) -> Self {
        self.scripts = scripts;
        self
    }

    /// A host that lets scripts choose from `palettes`.
    ///
    /// Selection uses the script's seeded RNG, so "pick a random palette"
    /// still reproduces from script + seed — unlike the Rust random
    /// generator, which draws from the thread RNG.
    pub fn with_palettes(palettes: Vec<crate::scene::palette::Palette>) -> Self {
        Self { palettes, ..Self::new() }
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
        state.borrow_mut().scripts = self.scripts.clone();

        // Built per run: the registered closures capture this run's
        // config and state.
        let engine = build_engine(Rc::clone(&cfg), Rc::clone(&state));

        let mut scope = Scope::new();
        super::api::push_globals(&mut scope, Rc::clone(&cfg), Rc::clone(&state));

        state.borrow_mut().frame_ops.push(0);
        let result = engine.run_with_scope(&mut scope, text);
        {
            let mut st = state.borrow_mut();
            let spent = st.frame_ops.pop().unwrap_or(0);
            st.ops_finished += spent;
        }
        result.map_err(map_error)?;

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

/// The most nesting `run_script` allows. Cycles are caught separately;
/// this is for long chains that never repeat an id.
const MAX_SCRIPT_DEPTH: usize = 8;

/// Run another script against the SAME config and the same RNG stream.
///
/// Sharing the config is what makes this useful without a return value:
/// a palette script called from a generator simply sets that flame's
/// palette. Sharing the RNG stream (rather than re-using the caller's
/// seed) keeps the whole run reproducible from script + seed while
/// letting two calls produce two different results.
pub(crate) fn run_sub_script(
    cfg: &Rc<RefCell<FractalConfig>>,
    state: &Rc<RefCell<ScriptState>>,
    id: &str,
    params: HashMap<String, ParamValue>,
    seed: Option<u64>,
) -> Result<(), String> {
    let (source, saved) = {
        let mut st = state.borrow_mut();

        if st.scripts.is_empty() {
            return Err(format!(
                "cannot run `{id}`: no script library is available here (try running from the app)"
            ));
        }
        if st.call_stack.iter().any(|c| c == id) {
            let mut chain = st.call_stack.clone();
            chain.push(id.to_string());
            return Err(format!("scripts call each other in a loop: {}", chain.join(" -> ")));
        }
        if st.call_stack.len() >= MAX_SCRIPT_DEPTH {
            return Err(format!(
                "scripts are nested more than {MAX_SCRIPT_DEPTH} deep (at `{id}`)"
            ));
        }

        let source = match st.scripts.iter().find(|(k, _)| k == id) {
            Some((_, src)) => src.clone(),
            None => {
                let mut known: Vec<&str> = st.scripts.iter().map(|(k, _)| k.as_str()).collect();
                known.sort_unstable();
                return Err(format!(
                    "no script with id `{id}` (available: {})",
                    known.join(", ")
                ));
            }
        };

        // The callee declares its OWN parameters; they must not land in
        // the caller's metadata, or they would appear in its panel.
        let saved = (
            std::mem::take(&mut st.meta),
            std::mem::replace(&mut st.provided, params),
            std::mem::take(&mut st.declared),
        );
        st.call_stack.push(id.to_string());
        st.frame_ops.push(0);
        (source, saved)
    };

    // An explicit seed makes the callee reproduce on its own: running
    // `random_palette` at seed 5 gives the palette that seed 5 put on a
    // flame, so a palette worth keeping can be picked up and tweaked.
    // The caller's own stream is saved and restored around it, so asking
    // for that does not shift everything the caller draws afterwards.
    let saved_rng = seed.map(|s| {
        let mut st = state.borrow_mut();
        std::mem::replace(&mut st.rng, Pcg64Mcg::new(expand_seed(s)))
    });

    let engine = build_engine(Rc::clone(cfg), Rc::clone(state));
    let mut scope = Scope::new();
    super::api::push_globals(&mut scope, Rc::clone(cfg), Rc::clone(state));
    let result = engine.run_with_scope(&mut scope, &source);

    {
        let mut st = state.borrow_mut();
        let spent = st.frame_ops.pop().unwrap_or(0);
        st.ops_finished += spent;
        st.call_stack.pop();
        if let Some(rng) = saved_rng {
            st.rng = rng;
        }
        st.meta = saved.0;
        st.provided = saved.1;
        st.declared = saved.2;
    }

    // Attribute the failure: an unattributed line number in a file the
    // reader did not open is a dead end.
    result.map_err(|e| {
        let mapped = map_error(e);
        match mapped.line {
            Some(line) => format!("in `{id}` line {line}: {}", mapped.message),
            None => format!("in `{id}`: {}", mapped.message),
        }
    })
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

    // Rhai's own max_operations is per evaluation, so a script that
    // calls another would get a whole fresh allowance for it — and a
    // loop calling a thousand sub-scripts would buy a thousand of them.
    // Charge every frame against one shared total instead.
    let budget_state = Rc::clone(&state);
    engine.on_progress(move |ops| {
        let mut st = budget_state.borrow_mut();
        if let Some(frame) = st.frame_ops.last_mut() {
            *frame = ops;
        }
        if st.ops_total() > MAX_OPERATIONS {
            Some(rhai::Dynamic::from("operation budget exhausted"))
        } else {
            None
        }
    });

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
    // A budget stop reaches us as ErrorTerminated carrying the token we
    // returned from on_progress. Rhai renders that as bare "Script
    // terminated", which tells the reader nothing about what to change.
    if let rhai::EvalAltResult::ErrorTerminated(token, _) = &*err {
        let reason = token
            .clone()
            .into_string()
            .unwrap_or_else(|_| "stopped".to_string());
        return ScriptError {
            message: format!(
                "{reason} — this script (and anything it called) did too much work.                  Reduce a loop count, or the depth of whatever it is building."
            ),
            line: pos.line(),
            column: pos.position(),
        };
    }
    // Rhai appends its own position suffix to Display; strip it so the
    // message doesn't read "… (line 7, position 3) (line 7:3)".
    let raw = err.to_string();
    let message = match raw.find(" (line ") {
        Some(i) => raw[..i].to_string(),
        None => raw,
    };
    ScriptError { message, line: pos.line(), column: pos.position() }
}
