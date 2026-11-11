//! Script tracks (docs/projects/rhai-tracks.md): a Rhai script as an
//! animation track, writing many targets, each set, added to or scaled.
//!
//! A track's source declares, at its top level, `TARGETS` (what it writes
//! and how), optionally `PARAMS` and `INFO`, and `fn value(t, p, ctx)`,
//! which returns a map of target to number. This is the CPU world
//! (rhai-plugins.md, *Two Rhai worlds*): full Rhai, sandboxed, run once a
//! frame. `PARAMS` follows the formula language's rules exactly -- it is
//! read through `lang` -- so a track's parameters are drawn and range-
//! checked as every other kind's are.
//!
//! Rhai is built without `sync`, so a compiled track cannot cross threads:
//! each thread compiles what it evaluates, once, cached by the source
//! ([`with_compiled`]). The video export's thread compiles its own.
//!
//! **What a script reads** (`ctx.get`) is the value under the scripts: the
//! base config with the keyframe and signal tracks applied, never what a
//! script wrote last frame (`AnimationController::script_values`).
//!
//! **State** (step 2): a track with `fn start(p, ctx)` and `fn step(s, dt,
//! p, ctx)` keeps a state, stepped at [`STEP`] from time zero, and its
//! `fn value(t, s, p, ctx)` reads it. The state at `t` is the state after
//! `floor(t / STEP)` steps, whatever came before -- a scrub, a missed
//! frame, a 30 or 60 fps export and a motion-blur moment all see the same
//! one -- replayed from a checkpoint each second ([`state_at`]).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use rhai::{Dynamic, Engine, EvalAltResult, Map, Scope, AST};
use serde::{Deserialize, Serialize};

use crate::config::{ConfigPath, EditingTarget, FractalConfig};
use crate::params::{ParamDef, ParamKind};

/// A track's operation budget per call. A generator's is 5,000,000; a
/// track runs every frame, and an expensive script is a poor fit for live
/// animation (the user, 2026-11-10).
const MAX_OPERATIONS: u64 = 200_000;

/// A stateful track's step: 1/240 s for every track (the user's choice,
/// 2026-11-10) -- fine enough for a spring at any frame rate an export
/// uses.
pub const STEP: f64 = 1.0 / 240.0;

/// Steps between checkpoints: one a second, so a backward seek replays
/// at most a second of steps.
const CHECKPOINT: u64 = 240;

/// How a target's value combines with the value under the script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// The script's value replaces it.
    Set,
    /// The script's value is added to it.
    Add,
    /// It is multiplied by the script's value.
    Scale,
}

impl Mode {
    fn parse(s: &str) -> Option<Mode> {
        match s {
            "set" => Some(Mode::Set),
            "add" => Some(Mode::Add),
            "scale" => Some(Mode::Scale),
            _ => None,
        }
    }

    /// The value written: the script's `v` over the value `under` it.
    pub fn combine(self, under: f64, v: f64) -> f64 {
        match self {
            Mode::Set => v,
            Mode::Add => under + v,
            Mode::Scale => under * v,
        }
    }
}

/// A target a track declares.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// As the script names it (`"zoom"`, `"transform.0.weight"`).
    pub name: String,
    /// Its `ConfigPath` key, what a track's target and the appliers name.
    pub key: String,
    pub mode: Mode,
}

/// What a track's source declares, read once when it compiles.
#[derive(Debug, Clone)]
pub struct Declared {
    pub title: String,
    /// In the order of their names (a Rhai map's), so evaluation is
    /// deterministic.
    pub targets: Vec<Target>,
    pub params: Vec<ParamDef>,
}

/// One target's output for a frame, before it combines with the value
/// under it.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub key: String,
    pub mode: Mode,
    pub value: f64,
}

/// What `ctx` is to a script: the config under the scripts and the
/// animation's length. In a step, the names it reads are recorded: a state
/// depends on them, so a change to one drops its checkpoints.
#[derive(Clone)]
struct Ctx {
    under: Rc<FractalConfig>,
    signals: Rc<BTreeMap<String, f64>>,
    duration: f64,
    reads: Option<Rc<RefCell<std::collections::BTreeSet<String>>>>,
}

/// What a script sees at one time: the config under the scripts and the
/// signals' values, 0 to 1. A recorded signal is a function of time, so a
/// replay reads what the first pass read; a live one (a microphone, MIDI)
/// is whatever arrives, and stays live (the user, 2026-11-10).
#[derive(Clone)]
pub struct Moment {
    pub under: Rc<FractalConfig>,
    pub signals: Rc<BTreeMap<String, f64>>,
}

struct Compiled {
    engine: Engine,
    ast: AST,
    declared: Declared,
    /// Whether it keeps a state (`start` and `step`).
    stateful: bool,
}

/// A stateful track's replay: its state at checkpoints and where it last
/// got to.
struct Stepper {
    checkpoints: BTreeMap<u64, Dynamic>,
    last: (u64, Dynamic),
}

/// What a track evaluates against at a frame.
pub struct Frame<'a> {
    pub time: f64,
    pub duration: f64,
    /// What a script sees at `time`.
    pub at: Moment,
    /// The base with the script targets at rest, before any track: where a
    /// state's dependencies are read for its fingerprint.
    pub rested: &'a FractalConfig,
    /// What a script sees at another time, for the steps.
    pub at_time: &'a dyn Fn(f64) -> Moment,
    /// A hash of everything else the steps can see: the keyframe and signal
    /// tracks and the duration.
    pub dependencies: u64,
}

thread_local! {
    static COMPILED: RefCell<HashMap<String, Rc<Result<Compiled, String>>>> = RefCell::new(HashMap::new());
    static ERRORS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    /// Each stateful track's replays, by the fingerprint of what its state
    /// depends on (`fingerprint`): the same fingerprint, the same states.
    static STATES: RefCell<HashMap<u64, Stepper>> = RefCell::new(HashMap::new());
    /// The names each source's steps have read.
    static READS: RefCell<HashMap<String, Rc<RefCell<std::collections::BTreeSet<String>>>>> = RefCell::new(HashMap::new());
}

/// Record how `source` last evaluated on this thread: an error is logged
/// when it first appears, not every frame, and the track editor shows it
/// ([`error_of`]); `None` clears it.
pub fn note_error(source: &str, error: Option<String>) {
    ERRORS.with(|e| {
        let mut errors = e.borrow_mut();
        match error {
            Some(msg) => {
                if errors.get(source) != Some(&msg) {
                    log::warn!("Script track: {msg}");
                    errors.insert(source.to_string(), msg);
                }
            }
            None => {
                errors.remove(source);
            }
        }
    });
}

/// The error `source` last evaluated with on this thread, if it did not
/// evaluate.
pub fn error_of(source: &str) -> Option<String> {
    ERRORS.with(|e| e.borrow().get(source).cloned())
}

/// The compiled track for `source`, compiled on this thread the first time.
fn with_compiled<R>(source: &str, f: impl FnOnce(&Compiled) -> Result<R, String>) -> Result<R, String> {
    let compiled = COMPILED.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some(hit) = cache.get(source) {
            return Rc::clone(hit);
        }
        // A track edited live compiles once a version; a long session
        // must not keep every version it ever saw.
        if cache.len() > 64 {
            cache.clear();
        }
        let built = Rc::new(compile(source));
        cache.insert(source.to_string(), Rc::clone(&built));
        built
    });
    match &*compiled {
        Ok(c) => f(c),
        Err(e) => Err(e.clone()),
    }
}

/// What `source` declares, or why it is not a track.
pub fn declare(source: &str) -> Result<Declared, String> {
    with_compiled(source, |c| Ok(c.declared.clone()))
}

/// Whether `source` reads as a track: it has `TARGETS` and `fn value`
/// (what the Formula Editor tells a track from the GPU kinds by).
pub fn looks_like_track(source: &str) -> bool {
    let Ok(ast) = engine().compile(source) else { return false };
    let has_value = ast.iter_functions().any(|f| f.name == "value");
    let has_targets = ast.statements().iter().any(|s| matches!(s, rhai::Stmt::Var(x, ..) if x.0.name.as_str() == "TARGETS"));
    has_value && has_targets
}

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_call_levels(32);
    engine.set_max_array_size(10_000);
    engine.set_max_string_size(100_000);
    engine.set_max_map_size(1_000);
    engine.disable_symbol("eval");
    // No files: a track is a shared artefact, and its result must depend
    // on its source alone.
    engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
    engine.register_fn("sleep", |_: rhai::INT| -> Result<(), Box<EvalAltResult>> { Err("sleep() is not available to a track".into()) });
    engine.register_fn("sleep", |_: rhai::FLOAT| -> Result<(), Box<EvalAltResult>> { Err("sleep() is not available to a track".into()) });
    engine.register_fn("timestamp", || -> Result<(), Box<EvalAltResult>> {
        Err("timestamp() is not available to a track: the same time gives the same values".into())
    });
    engine
        .register_type_with_name::<Ctx>("Context")
        .register_get("duration", |c: &mut Ctx| c.duration)
        .register_get("dt", |_: &mut Ctx| STEP)
        .register_fn("get", |c: &mut Ctx, name: &str| -> Result<f64, Box<EvalAltResult>> {
            let key = resolve_target(name).map_err(|e| -> Box<EvalAltResult> { e.into() })?;
            if let Some(reads) = &c.reads {
                reads.borrow_mut().insert(key.clone());
            }
            read(&c.under, &key).ok_or_else(|| format!("`{name}` has no number to read").into())
        })
        .register_fn("signal", |c: &mut Ctx, name: &str| -> Result<f64, Box<EvalAltResult>> {
            c.signals.get(name).copied().ok_or_else(|| {
                let known: Vec<&str> = c.signals.keys().map(String::as_str).collect();
                format!("no signal `{name}` here (a live signal never reaches an export); the signals are: {}", known.join(", ")).into()
            })
        });
    // Deterministic randomness, the same bits on every platform: integer
    // hashing and plain arithmetic, no transcendental functions.
    engine.register_fn("hash", |n: rhai::INT| unit(mix(n as u64)));
    engine.register_fn("hash", |n: rhai::INT, seed: rhai::INT| unit(mix(n as u64 ^ mix(seed as u64))));
    engine.register_fn("noise", |x: rhai::FLOAT| noise(x, 0));
    engine.register_fn("noise", |x: rhai::FLOAT, seed: rhai::INT| noise(x, seed as u64));
    engine
}

/// SplitMix64's finalizer: a well-mixed 64-bit hash of `z`.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A hash as a number in [0, 1): its top 53 bits.
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Smooth value noise in [-1, 1]: a value hashed at each whole number,
/// eased between them (the quintic, so its slope is continuous too).
/// `seed` gives independent channels.
fn noise(x: f64, seed: u64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    let i = x.floor();
    let f = x - i;
    let s = mix(seed);
    let at = |k: f64| 2.0 * unit(mix((k as i64) as u64 ^ s)) - 1.0;
    let (a, b) = (at(i), at(i + 1.0));
    let e = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    a + (b - a) * e
}

fn compile(source: &str) -> Result<Compiled, String> {
    let engine = engine();
    let ast = engine.compile(source).map_err(|e| e.to_string())?;
    // The top level holds the declarations: run it once to read them.
    let mut scope = Scope::new();
    engine.run_ast_with_scope(&mut scope, &ast).map_err(|e| e.to_string())?;
    let targets = match scope.get_value::<Map>("TARGETS") {
        Some(m) => m,
        None => return Err("a track declares what it writes in `TARGETS`, a map of name to \"set\", \"add\" or \"scale\"".into()),
    };
    if targets.is_empty() {
        return Err("`TARGETS` names nothing to write".into());
    }
    let mut out = Vec::new();
    for (name, mode) in &targets {
        let mode_text = mode.clone().into_string().map_err(|_| format!("`TARGETS.{name}` is \"set\", \"add\" or \"scale\""))?;
        let mode = Mode::parse(&mode_text).ok_or_else(|| format!("`TARGETS.{name}` is \"set\", \"add\" or \"scale\", not \"{mode_text}\""))?;
        let key = resolve_target(name)?;
        out.push(Target { name: name.to_string(), key, mode });
    }
    let params = match scope.get_value::<Map>("PARAMS") {
        Some(m) => params_of(&m)?,
        None => Vec::new(),
    };
    let has = |name: &str, n: usize| ast.iter_functions().any(|f| f.name == name && f.params.len() == n);
    let stateful = ast.iter_functions().any(|f| f.name == "start" || f.name == "step");
    if stateful {
        if !has("start", 2) || !has("step", 4) {
            return Err("a track with a state has both `fn start(p, ctx)`, the state at time zero, and `fn step(s, dt, p, ctx)`, the state one step on".into());
        }
        if !has("value", 4) {
            return Err("a track with a state has `fn value(t, s, p, ctx)`, reading the state `s`".into());
        }
    } else if !has("value", 3) {
        return Err("a track has `fn value(t, p, ctx)`, returning a map of target to number".into());
    }
    let title = crate::lang::rhai::source_labels(source).map(|l| l.title.resolve()).filter(|t| !t.is_empty()).unwrap_or_else(|| "Script".to_string());
    Ok(Compiled { engine, ast, declared: Declared { title, targets: out, params }, stateful })
}

/// A target's `ConfigPath` key from its name: `"transform.N.<value>"` for a
/// normal transform's (`"weight"`, `"a"`, `"julian.power"`), else the name
/// `config.set` takes -- the generator `anim` API's spellings, so one name
/// serves every script.
pub fn resolve_target(name: &str) -> Result<String, String> {
    use crate::config::TransformRef;
    let key = if let Some(rest) = name.strip_prefix("transform.") {
        let (index, value) = rest.split_once('.').ok_or_else(|| format!("`{name}`: a transform's value is `transform.N.weight`, `transform.N.a`, `transform.N.julian.power`"))?;
        let index: usize = index.parse().map_err(|_| format!("`{name}`: `{index}` is not a transform's number"))?;
        crate::script::anim::resolve_transform_target(TransformRef::Normal(index), value)?
    } else {
        crate::script::anim::resolve_flame_target(name)?
    };
    if key.starts_with("System.") {
        return Err(format!("`{name}` is a device setting, not part of the picture: a track cannot write it"));
    }
    Ok(key)
}

/// The value `key` holds in `config`, as a number.
pub fn read(config: &FractalConfig, key: &str) -> Option<f64> {
    use crate::config::ConfigValue as V;
    let path = ConfigPath::from_string_key(key)?;
    if key.starts_with("System.") {
        return None;
    }
    match crate::config::ConfigManager::get_value_from_config(config, &path, EditingTarget::Main).ok()? {
        V::Float(x) => Some(x as f64),
        V::Double(x) => Some(x),
        V::Int(x) => Some(x as f64),
        V::UInt(x) => Some(x as f64),
        V::UInt64(x) => Some(x as f64),
        V::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
        // The escape centre is an exact decimal.
        V::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A track's `PARAMS` as parameter definitions, by the formula language's
/// rules: the map printed back as Rhai and read as a field colouring's,
/// the smallest kind there is, so ranges, choices, log sliders and angles
/// mean what they mean everywhere.
fn params_of(params: &Map) -> Result<Vec<ParamDef>, String> {
    let literal = literal(&Dynamic::from_map(params.clone())).ok_or("`PARAMS` holds numbers, strings, lists and maps")?;
    let source = format!("const PARAMS = {literal};\nfn shade(sum, grad, n, p) {{ 0.0 }}\n");
    let program = crate::lang::rhai::field_coloring(&source).map_err(|d| d.first().map(|x| format!("`PARAMS`: {}", x.message)).unwrap_or_else(|| "`PARAMS` does not read".to_string()))?;
    program.param_defs(crate::lang::ir::ComplexParams::Refuse("a track's parameters are real numbers"))
}

/// A constant as Rhai source.
fn literal(d: &Dynamic) -> Option<String> {
    if let Ok(f) = d.as_float() {
        return f.is_finite().then(|| format!("{f:?}"));
    }
    if let Ok(i) = d.as_int() {
        return Some(i.to_string());
    }
    if let Ok(b) = d.as_bool() {
        return Some(b.to_string());
    }
    if d.is_string() {
        return serde_json::to_string(&d.clone().into_string().ok()?).ok();
    }
    if d.is_array() {
        let items: Option<Vec<String>> = d.clone().into_array().ok()?.iter().map(literal).collect();
        return Some(format!("[{}]", items?.join(", ")));
    }
    if d.is_map() {
        let map = d.clone().try_cast::<Map>()?;
        let items: Option<Vec<String>> = map.iter().map(|(k, v)| Some(format!("{}: {}", serde_json::to_string(k.as_str()).ok()?, literal(v)?))).collect();
        return Some(format!("#{{ {} }}", items?.join(", ")));
    }
    None
}

/// A parameter's value as the script reads it: a number a float, a whole
/// number or a choice an int, a switch a bool.
fn param_value(def: &ParamDef, given: Option<f64>) -> Dynamic {
    match &def.kind {
        ParamKind::Float { default, .. } => Dynamic::from_float(given.unwrap_or(*default)),
        ParamKind::Int { default, .. } => Dynamic::from_int(given.map_or(*default, |v| v.round() as i64)),
        ParamKind::Bool { default } => Dynamic::from_bool(given.map_or(*default, |v| v != 0.0)),
        ParamKind::Choice { default, .. } => Dynamic::from_int(given.map_or(*default as i64, |v| v.round() as i64)),
        #[allow(unreachable_patterns)]
        _ => Dynamic::from_float(given.unwrap_or(0.0)),
    }
}

/// The track's outputs at the frame's time: `fn value` run with its
/// parameters (`values` over their defaults), the config under the
/// scripts, and its state there if it keeps one.
pub fn evaluate(source: &str, values: &BTreeMap<String, f64>, frame: &Frame) -> Result<Vec<Output>, String> {
    let t = frame.time;
    with_compiled(source, |c| {
        let mut p = Map::new();
        for def in &c.declared.params {
            p.insert(def.name.as_ref().into(), param_value(def, values.get(def.name.as_ref()).copied()));
        }
        let ctx = Ctx { under: Rc::clone(&frame.at.under), signals: Rc::clone(&frame.at.signals), duration: frame.duration, reads: None };
        let result: Dynamic = if c.stateful {
            let s = state_at(c, source, values, &p, frame)?;
            c.engine.call_fn(&mut Scope::new(), &c.ast, "value", (t, s, p, ctx))
        } else {
            c.engine.call_fn(&mut Scope::new(), &c.ast, "value", (t, p, ctx))
        }
        .map_err(|e| e.to_string())?;
        let map = result.try_cast::<Map>().ok_or("`value` returns a map of target to number, `#{ zoom: 1.5 }`")?;
        let mut out = Vec::new();
        for (name, v) in &map {
            let target = c
                .declared
                .targets
                .iter()
                .find(|x| x.name == name.as_str())
                .ok_or_else(|| format!("`value` writes `{name}`, which `TARGETS` does not declare"))?;
            let value = v.as_float().or_else(|_| v.as_int().map(|i| i as f64)).map_err(|_| format!("`{name}` is a number, not {}", v.type_name()))?;
            if !value.is_finite() {
                return Err(format!("`{name}` is not a finite number at t = {t}"));
            }
            out.push(Output { key: target.key.clone(), mode: target.mode, value });
        }
        Ok(out)
    })
}

/// Run `source` as the Formula Editor's check does: declared, then
/// evaluated at a few times on the default config, its state stepped, so
/// an error a frame would hit shows before the track is used.
pub fn try_run(source: &str) -> Result<Declared, String> {
    let declared = declare(source)?;
    let under = Rc::new(FractalConfig::default());
    let at = Moment { under: Rc::clone(&under), signals: Rc::new(BTreeMap::new()) };
    let m = at.clone();
    let at_time = move |_: f64| m.clone();
    for time in [0.0, 0.25, 1.0] {
        let frame = Frame { time, duration: 1.0, at: at.clone(), rested: &under, at_time: &at_time, dependencies: u64::MAX };
        evaluate(source, &BTreeMap::new(), &frame)?;
    }
    Ok(declared)
}

/// What a stateful track's state depends on, as one number: its source,
/// its parameters, the frame's other tracks and duration, and the rest
/// values of the names its steps have read.
fn fingerprint(source: &str, values: &BTreeMap<String, f64>, frame: &Frame, reads: &std::collections::BTreeSet<String>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    for (k, v) in values {
        k.hash(&mut h);
        v.to_bits().hash(&mut h);
    }
    frame.dependencies.hash(&mut h);
    for key in reads {
        key.hash(&mut h);
        read(frame.rested, key).map(f64::to_bits).hash(&mut h);
    }
    h.finish()
}

/// A stateful track's state at the frame's time: after `floor(t / STEP)`
/// steps from `start`, stepped on from where it last got to, or replayed
/// from the last checkpoint before the time. A step's `ctx` reads the
/// config under the scripts at the step's end.
fn state_at(c: &Compiled, source: &str, values: &BTreeMap<String, f64>, p: &Map, frame: &Frame) -> Result<Dynamic, String> {
    let t = frame.time.clamp(0.0, frame.duration.max(0.0));
    let n = (t / STEP + 1e-9).floor() as u64;
    let reads = READS.with(|r| Rc::clone(r.borrow_mut().entry(source.to_string()).or_default()));
    let key = fingerprint(source, values, frame, &reads.borrow());
    let ctx_at = |time: f64| {
        let m = (frame.at_time)(time);
        Ctx { under: m.under, signals: m.signals, duration: frame.duration, reads: Some(Rc::clone(&reads)) }
    };
    let found = STATES.with(|s| {
        let states = s.borrow();
        let stepper = states.get(&key)?;
        let from_last = (stepper.last.0 <= n).then(|| stepper.last.clone());
        let from_checkpoint = stepper.checkpoints.range(..=n).next_back().map(|(k, v)| (*k, v.clone()));
        match (from_last, from_checkpoint) {
            (Some(a), Some(b)) => Some(if a.0 >= b.0 { a } else { b }),
            (a, b) => a.or(b),
        }
    });
    let mut fresh = BTreeMap::new();
    let (mut at, mut s) = match found {
        Some(start) => start,
        None => {
            let s0: Dynamic = c.engine.call_fn(&mut Scope::new(), &c.ast, "start", (p.clone(), ctx_at(0.0))).map_err(|e| e.to_string())?;
            fresh.insert(0, s0.clone());
            (0, s0)
        }
    };
    while at < n {
        s = c.engine.call_fn(&mut Scope::new(), &c.ast, "step", (s, STEP, p.clone(), ctx_at((at + 1) as f64 * STEP))).map_err(|e| e.to_string())?;
        at += 1;
        if at % CHECKPOINT == 0 {
            fresh.insert(at, s.clone());
        }
    }
    STATES.with(|states| {
        let mut states = states.borrow_mut();
        // Edits make new fingerprints; a session keeps the latest few.
        if states.len() > 32 && !states.contains_key(&key) {
            states.clear();
        }
        let stepper = states.entry(key).or_insert_with(|| Stepper { checkpoints: BTreeMap::new(), last: (0, s.clone()) });
        stepper.checkpoints.extend(fresh);
        stepper.last = (n, s.clone());
    });
    Ok(s)
}

/// What a script target held before scripts wrote it, and what they last
/// wrote: when the config still holds what was written, the value under
/// the script is `rest`; when it holds something else, someone changed
/// it, and that is the new rest. Kept in the animation (runtime only), so
/// it travels into the video export with it: the export's base is the
/// app's config, which holds what the tracks last wrote.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rest {
    pub rest: f64,
    pub written: f64,
}

impl Rest {
    /// Whether `current` is what was written, to the f32 a config holds.
    pub fn holds_written(&self, current: f64) -> bool {
        (current - self.written).abs() <= 1e-6 * self.written.abs().max(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BREATHING: &str = r#"
const PARAMS = #{
    depth: #{ value: 0.1, min: 0.0, max: 1.0, caption: "Depth" },
    rate: #{ value: 0.5, min: 0.01, max: 10.0, caption: "Rate (Hz)" },
};
const INFO = #{ title: "Breathing" };
const TARGETS = #{ zoom: "add", rotation: "set" };
fn value(t, p, ctx) {
    #{ zoom: p.depth * sin(6.2831853 * p.rate * t), rotation: 0.1 * t }
}
"#;

    /// A track declares its title, its targets by `ConfigPath` key in name
    /// order, and its parameters as the formula language reads them.
    #[test]
    fn a_track_declares_its_targets_and_parameters() {
        let d = declare(BREATHING).expect("a track");
        assert_eq!(d.title, "Breathing");
        let keys: Vec<(&str, Mode)> = d.targets.iter().map(|t| (t.name.as_str(), t.mode)).collect();
        assert_eq!(keys, vec![("rotation", Mode::Set), ("zoom", Mode::Add)]);
        assert!(d.targets.iter().all(|t| ConfigPath::from_string_key(&t.key).is_some()), "{:?}", d.targets);
        let names: Vec<&str> = d.params.iter().map(|p| p.name.as_ref()).collect();
        assert_eq!(names, vec!["depth", "rate"]);
        assert_eq!(d.params[1].label, "Rate (Hz)");
        assert!(looks_like_track(BREATHING));
        assert!(!looks_like_track("fn shade(sum, grad, n) { sum }"));
    }

    /// `value` runs with the parameters' values over their defaults, and
    /// reads the config under it.
    /// A frame at `t` over `under`, nothing else animated.
    fn frame_on(under: &Rc<FractalConfig>, t: f64, duration: f64, f: impl FnOnce(&Frame) -> Result<Vec<Output>, String>) -> Result<Vec<Output>, String> {
        let at = Moment { under: Rc::clone(under), signals: Rc::new(BTreeMap::from([("beat".to_string(), 0.25)])) };
        let m = at.clone();
        let at_time = move |_: f64| m.clone();
        let frame = Frame { time: t, duration, at, rested: under, at_time: &at_time, dependencies: 0 };
        f(&frame)
    }

    fn eval(source: &str, values: &BTreeMap<String, f64>, t: f64, duration: f64, under: Rc<FractalConfig>) -> Result<Vec<Output>, String> {
        frame_on(&under, t, duration, |f| evaluate(source, values, f))
    }

    #[test]
    fn a_track_evaluates_with_its_parameters_and_reads_the_config() {
        let under = Rc::new(FractalConfig::default());
        let at = |values: &BTreeMap<String, f64>, t: f64| eval(BREATHING, values, t, 10.0, Rc::clone(&under)).expect("values");
        let out = at(&BTreeMap::new(), 0.5);
        let zoom = out.iter().find(|o| o.mode == Mode::Add).unwrap();
        assert!((zoom.value - 0.1 * (std::f64::consts::PI * 0.5).sin()).abs() < 1e-6, "{zoom:?}");
        let out = at(&BTreeMap::from([("depth".to_string(), 0.5)]), 0.5);
        let zoom = out.iter().find(|o| o.mode == Mode::Add).unwrap();
        assert!((zoom.value - 0.5 * (std::f64::consts::PI * 0.5).sin()).abs() < 1e-6, "{zoom:?}");
        let reader = "const TARGETS = #{ zoom: \"set\" };\nfn value(t, p, ctx) { #{ zoom: ctx.get(\"zoom\") * 2.0 + ctx.duration } }";
        let out = eval(reader, &BTreeMap::new(), 0.0, 10.0, Rc::clone(&under)).expect("values");
        let zoom = read(&under, &resolve_target("zoom").unwrap()).unwrap();
        assert_eq!(out[0].value, zoom * 2.0 + 10.0);
    }

    /// `ctx.signal` reads a signal; `hash` and `noise` are deterministic,
    /// in range, and the noise is smooth: no step between close samples.
    #[test]
    fn signals_hash_and_noise() {
        let under = Rc::new(FractalConfig::default());
        let src = "const TARGETS = #{ zoom: \"set\", rotation: \"set\" };\nfn value(t, p, ctx) { #{ zoom: ctx.signal(\"beat\"), rotation: noise(t) + hash(7) } }";
        let out = eval(src, &BTreeMap::new(), 0.3, 1.0, Rc::clone(&under)).expect("values");
        let get = |k: &str| out.iter().find(|o| o.key == k).unwrap().value;
        assert_eq!(get("Zoom"), 0.25);
        assert_eq!(get("Rotation"), noise(0.3, 0) + unit(mix(7)));
        let missing = "const TARGETS = #{ zoom: \"set\" };\nfn value(t, p, ctx) { #{ zoom: ctx.signal(\"mic\") } }";
        assert!(eval(missing, &BTreeMap::new(), 0.0, 1.0, Rc::clone(&under)).unwrap_err().contains("beat"));
        let mut prev = noise(-3.0, 5);
        for k in 1..=6000 {
            let x = -3.0 + k as f64 * 0.001;
            let v = noise(x, 5);
            assert!((-1.0..=1.0).contains(&v));
            assert!((v - prev).abs() < 0.01, "a step at {x}");
            prev = v;
        }
        assert_ne!(noise(1.5, 0), noise(1.5, 1), "seeds are channels");
        assert!((0..1000).map(|n| unit(mix(n))).all(|h| (0.0..1.0).contains(&h)));
    }

    /// What is not a track says why.
    #[test]
    fn a_broken_track_says_why() {
        let cases = [
            ("fn value(t, p, ctx) { #{} }", "TARGETS"),
            ("const TARGETS = #{ zoom: \"push\" };\nfn value(t, p, ctx) { #{} }", "\"push\""),
            ("const TARGETS = #{ nonsense_setting: \"set\" };\nfn value(t, p, ctx) { #{} }", "not an animatable setting"),
            ("const TARGETS = #{ zoom: \"set\" };", "fn value"),
            ("const TARGETS = #{ \"system.iterations_per_thread\": \"set\" };\nfn value(t, p, ctx) { #{} }", "device setting"),
        ];
        for (src, says) in cases {
            match declare(src) {
                Err(e) => assert!(e.contains(says), "{src}: {e}"),
                Ok(_) => panic!("{src}: declared"),
            }
        }
        let undeclared = "const TARGETS = #{ zoom: \"set\" };\nfn value(t, p, ctx) { #{ rotation: 1.0 } }";
        let e = eval(undeclared, &BTreeMap::new(), 0.0, 1.0, Rc::new(FractalConfig::default())).unwrap_err();
        assert!(e.contains("does not declare"), "{e}");
        let runaway = "const TARGETS = #{ zoom: \"set\" };\nfn value(t, p, ctx) { loop {} }";
        assert!(eval(runaway, &BTreeMap::new(), 0.0, 1.0, Rc::new(FractalConfig::default())).is_err());
        let half = "const TARGETS = #{ zoom: \"set\" };\nfn start(p, ctx) { 0.0 }\nfn value(t, s, p, ctx) { #{} }";
        assert!(declare(half).unwrap_err().contains("fn step"));
        let reads_no_state = "const TARGETS = #{ zoom: \"set\" };\nfn start(p, ctx) { 0.0 }\nfn step(s, dt, p, ctx) { s }\nfn value(t, p, ctx) { #{} }";
        assert!(declare(reads_no_state).unwrap_err().contains("value(t, s, p, ctx)"));
    }
}
