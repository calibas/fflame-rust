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
    /// Library ids that did not come from this app — downloaded scripts.
    /// See [`ScriptState::cross_calls_restricted`].
    pub untrusted: HashSet<String>,
    /// Whether the ENTRY script is untrusted. It has no id on the call
    /// stack, so it cannot be covered by `untrusted` alone — and the
    /// entry script is the ordinary case for "the user pressed Run on
    /// something they downloaded".
    pub entry_untrusted: bool,
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

/// The cross-call rule, as a free function.
///
/// Lifted out of [`ScriptState`] so it can be tested on data. It has to
/// be: the "any frame" reading only differs from "the immediate caller"
/// on `downloaded -> shipped -> user`, and that chain cannot be built
/// from real scripts — reaching it needs a shipped script that calls a
/// user one, and shipped scripts are compiled in. A test driving the
/// engine would therefore pass under either rule while appearing to
/// pin the stricter one.
pub(crate) fn cross_calls_restricted(
    entry_untrusted: bool,
    call_stack: &[String],
    untrusted: &HashSet<String>,
) -> bool {
    entry_untrusted || call_stack.iter().any(|id| untrusted.contains(id))
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
            untrusted: HashSet::new(),
            entry_untrusted: false,
            frame_ops: Vec::new(),
            ops_finished: 0,
            seed,
        }
    }

    /// Whether `run_script` is currently limited to shipped scripts.
    ///
    /// True once any untrusted frame is on the stack — including the
    /// entry script, which has no id there.
    ///
    /// # Why "any frame", not "the immediate caller"
    ///
    /// A downloaded script may only call shipped ones, and shipped
    /// scripts only call each other, so the two rules agree on
    /// everything that exists today. They diverge on
    /// `downloaded -> shipped -> ?`: under an immediate-caller rule the
    /// shipped frame would be unrestricted again and could reach a user
    /// script, laundering the restriction through one hop. No shipped
    /// script does that, and none can be made to without a recompile —
    /// but "safe because of what the shipped corpus happens to contain"
    /// is not a property, and the stricter rule costs one `any`.
    pub fn cross_calls_restricted(&self) -> bool {
        cross_calls_restricted(self.entry_untrusted, &self.call_stack, &self.untrusted)
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
    /// Library ids that came from the online library rather than from
    /// this app or this user.
    untrusted: HashSet<String>,
    /// Whether the script about to be RUN is itself untrusted.
    entry_untrusted: bool,
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

    /// Mark library ids as untrusted — downloaded, not shipped and not
    /// the user's own.
    ///
    /// # What this restricts, and why it is not paranoia
    ///
    /// `run_script(id)` resolves against the *whole* discovered library,
    /// the user's own scripts included. Left alone, a downloaded script
    /// calling `run_script("helper")` binds to whatever that machine
    /// happens to have under that name: it would render differently on
    /// every machine, and a stranger's script would be able to invoke
    /// the user's.
    ///
    /// So an untrusted script may call **shipped stems only**. Those are
    /// reserved names ([`super::library::is_builtin_stem`]) that always
    /// resolve to the compiled-in script, which is what makes the
    /// restriction meaningful rather than a name check.
    ///
    /// # The second reason this shape was chosen
    ///
    /// It also keeps the eventual dependency model honest. If no
    /// published script can have a non-builtin dependency, then when
    /// dependencies are modelled the backfill is provably empty — every
    /// existing script correctly means `[]`, with no old sources to
    /// parse. That is why the API deliberately does **not** reserve a
    /// `dependencies` field today.
    pub fn with_untrusted(mut self, ids: impl IntoIterator<Item = String>) -> Self {
        self.untrusted = ids.into_iter().collect();
        self
    }

    /// Mark the script about to be run as untrusted.
    ///
    /// Separate from [`Self::with_untrusted`] because the entry script
    /// never appears on the call stack, and "the user pressed Run on a
    /// script they downloaded" is the ordinary case rather than the edge
    /// one. Forgetting this would leave exactly the hole the restriction
    /// exists to close.
    pub fn with_untrusted_entry(mut self) -> Self {
        self.entry_untrusted = true;
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
        let mut meta = out.meta;
        meta.warnings = out.warnings;
        Ok(meta)
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
        {
            let mut st = state.borrow_mut();
            st.scripts = self.scripts.clone();
            st.untrusted = self.untrusted.clone();
            st.entry_untrusted = self.entry_untrusted;
        }

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
        // A downloaded script may only call shipped ones. Checked before
        // the id is resolved, so the refusal does not depend on whether
        // the target happens to exist — otherwise the error message
        // would report whether the user has a script by that name, which
        // is the sort of thing a downloaded script should not be able to
        // ask.
        if st.cross_calls_restricted() && !super::library::is_builtin_stem(id) {
            return Err(format!(
                "a downloaded script may only call scripts that ship with the app, \
                 and `{id}` is not one of them"
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

/// The script RNG's draws, pinned to a fixed algorithm.
///
/// Same rationale as `expand_seed` above, learned the hard way: the raw
/// PCG64-MCG stream is a stable published spec, but the mapping FROM
/// that stream to a float or a bounded integer is a library
/// implementation detail. rand 0.9 changed the integer one (it accepts
/// a word 0.8 rejected), which silently rewrote every flame any script
/// had ever produced from a given seed — caught by the `wasm/script`
/// CLI-parity fixtures during the 0.8 → 0.9 upgrade.
///
/// So the mapping is ours now. These reproduce rand 0.8.5's
/// `sample_single` / `Standard` exactly (verified against it across
/// tens of thousands of draws over many seeds and ranges), which keeps
/// every seed shared before this change meaning what it meant. The
/// `random_stream_is_pinned` test guards the result.
///
/// Only `next_u64` is taken from the dependency — the one piece that is
/// the generator's own defined output.
pub(crate) mod draw {
    use rand::RngCore;
    use rand_pcg::Pcg64Mcg;

    /// Uniform `f64` in `[0, 1)` — 53 bits, multiply-based.
    pub fn unit(rng: &mut Pcg64Mcg) -> f64 {
        const SCALE: f64 = 1.0 / ((1u64 << 53) as f64);
        SCALE * ((rng.next_u64() >> 11) as f64)
    }

    /// Uniform `f64` in `[low, high)`.
    ///
    /// Builds a value in `[1, 2)` by pasting an exponent onto random
    /// mantissa bits, then maps it affinely; the retry guards the case
    /// where rounding lands the result exactly on `high`.
    pub fn range_f64(rng: &mut Pcg64Mcg, low: f64, high: f64) -> f64 {
        let scale = high - low;
        loop {
            let v12 = f64::from_bits((rng.next_u64() >> 12) | 0x3FF0_0000_0000_0000);
            let res = (v12 - 1.0) * scale + low;
            if res < high || low >= high {
                return res;
            }
        }
    }

    /// Uniform integer in `0..range` (`range == 0` means the full
    /// 64-bit range). Widening multiply with a rejection zone.
    ///
    /// Fixed-width by construction: `usize` is 64-bit on desktop and
    /// 32-bit on wasm32, and rand dispatched to a different integer
    /// implementation for each — which forked the stream between
    /// platforms before every call site was cast to `u64`.
    pub fn below(rng: &mut Pcg64Mcg, range: u64) -> u64 {
        if range == 0 {
            return rng.next_u64();
        }
        let zone = (range << range.leading_zeros()).wrapping_sub(1);
        loop {
            let prod = (rng.next_u64() as u128) * (range as u128);
            if (prod as u64) <= zone {
                return (prod >> 64) as u64;
            }
        }
    }

    /// Uniform `i64` in `low..=high`.
    pub fn range_i64(rng: &mut Pcg64Mcg, low: i64, high: i64) -> i64 {
        let span = (high as i128 - low as i128 + 1) as u64;
        low.wrapping_add(below(rng, span) as i64)
    }
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

#[cfg(test)]
mod draw_tests {
    use super::draw;
    use rand_pcg::Pcg64Mcg;

    fn rng() -> Pcg64Mcg {
        Pcg64Mcg::new(super::expand_seed(12345))
    }

    /// Golden values for the pinned draws.
    ///
    /// These were produced by rand 0.8.5 — the library whose behaviour
    /// this module froze — and verified against it across tens of
    /// thousands of samples before the 0.9 upgrade landed. They are the
    /// unit-level twin of `random_stream_is_pinned`: if they move, every
    /// script+seed anyone has shared now names a different flame, so
    /// treat a failure as a breaking change rather than a number to
    /// update.
    #[test]
    fn pinned_draws_match_their_golden_values() {
        let mut r = rng();
        let unit: Vec<f64> = (0..3).map(|_| draw::unit(&mut r)).collect();
        assert_eq!(
            unit,
            vec![0.46722037666755534, 0.5145249938710224, 0.024093919998681823]
        );

        let mut r = rng();
        let ranged: Vec<f64> = (0..3).map(|_| draw::range_f64(&mut r, -2.0, 5.0)).collect();
        assert_eq!(
            ranged,
            vec![1.2705426366728876, 1.6016749570971562, -1.8313425600092272]
        );

        let mut r = rng();
        let ints: Vec<i64> = (0..5).map(|_| draw::range_i64(&mut r, 0, 1_000_000)).collect();
        assert_eq!(ints, vec![467220, 514525, 24093, 548020, 641899]);

        let mut r = rng();
        let picks: Vec<u64> = (0..6).map(|_| draw::below(&mut r, 7)).collect();
        assert_eq!(picks, vec![3, 3, 0, 3, 4, 6]);
    }

    /// Edge cases the script API never reaches but the helpers accept.
    #[test]
    fn draw_below_handles_degenerate_ranges() {
        let mut r = rng();
        // A single-value range consumes a word and always yields 0.
        assert_eq!(draw::below(&mut r, 1), 0);
        // Range 0 means "the whole 64-bit space" — no rejection loop, so
        // it is exactly one raw word off the stream.
        use rand::RngCore;
        let mut r = rng();
        assert_eq!(draw::below(&mut r, 0), rng().next_u64());
        // The largest ranges must still terminate and stay in bounds.
        let mut r = rng();
        for _ in 0..64 {
            assert!(draw::below(&mut r, u64::MAX) < u64::MAX);
        }
    }

    /// An inclusive range spanning negatives maps without overflow.
    #[test]
    fn draw_range_i64_spans_negatives() {
        let mut r = rng();
        for _ in 0..200 {
            let v = draw::range_i64(&mut r, -1000, 1000);
            assert!((-1000..=1000).contains(&v), "out of range: {v}");
        }
    }
}
