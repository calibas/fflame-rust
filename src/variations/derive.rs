//! Deriving a variation's forward bound from its shipped WGSL.
//!
//! [forward-bounds.md](../../docs/projects/deep-zoom/forward-bounds.md), phase
//! 1. [`bound`](super::bound) holds bounds written by hand; the corpus
//! meter says that supply cannot reach far enough — 14 hand-derived
//! bounds free 19 of 45 flames, and there are 55 distinct blockers
//! with no head. So this computes them instead.
//!
//! # Interval arithmetic, and why it is the right tool
//!
//! A bound may over-estimate and must never under-estimate
//! ([`bound`](super::bound) module docs: too loose costs efficiency,
//! too tight drops measure from the render silently). Interval
//! arithmetic is exactly an over-estimating evaluator — replace every
//! value by the closed range it could take and every operation by a
//! rule whose result contains every possible result — so the
//! contract's one permitted direction is structural rather than
//! something each rule has to be careful about.
//!
//! Where a body draws a random number the range is `[0, 1)`. That is
//! how `blur` and `julian` got their bounds by hand, so the
//! stochastic variations — the ones the INVERSE direction can never
//! touch, because a map that draws per sample has no inverse — are
//! the easy case here rather than the hard one.
//!
//! # No parser is written
//!
//! `wgpu::naga` is already a dependency, and
//! [`probe::shader::build`](crate::probe::shader::build) already
//! assembles a complete, validating module per batch of variations,
//! with every helper library spliced in. So the bodies arrive as IR
//! with `variation_*` and every helper they call (`cmul`, `csqrt`,
//! `ff_atan2`, `rng_nextf`, …) in one function arena, and this walks
//! that. A construct the walk does not model is a REFUSAL that names
//! it, never a wrong disc.
//!
//! # Control flow (phase 2)
//!
//! A condition evaluates to yes, no, or **maybe**. Yes and no take
//! the arm they name; maybe evaluates BOTH from the same incoming
//! state and unions every local either one assigned, which is sound
//! because the union contains whichever arm the GPU actually took.
//! Loops iterate with the same join until their exit condition is
//! definitely true, and refuse if it has not settled at the cap.
//!
//! Per-thread state and genuine poles stay refused for good: the
//! first because a per-call bound cannot know what a previous
//! iteration stored, the second because the map really is
//! unbounded there.

use super::bound::Ball;
use super::inverse::ParamFn;
pub use super::interval::{Branch, Interval, Refusal};
use super::interval::{atan2_iv, box_to_disc, cmp_le, cmp_lt, pow_iv, powf_iv, rng_for, sqrt_iv, square, INT32_RANGE};
use wgpu::naga;

/// A value in flight: WGSL's scalars and small vectors.
#[derive(Debug, Clone)]
pub enum IVal {
    Scalar(Interval),
    /// `vec2`/`vec3`/`vec4`, componentwise.
    Vec(Vec<Interval>),
    /// Loop counters and `u32` slot indices, which must stay exact —
    /// `get_param(.., .., 3u)` has to resolve to slot 3 and not to a
    /// range of slots.
    Int(i64),
    Bool(Option<bool>),
    /// A value the evaluator carries but cannot put a number to: the
    /// `xform_id` a body passes straight to `get_param`, the RNG
    /// pointer, the running `accum` sum a `NeedsAccum` body reads.
    ///
    /// It is a distinct variant rather than a zero or an unbounded
    /// interval because substituting a number for something unknown
    /// is exactly how a bound becomes too TIGHT, which is the one
    /// direction that corrupts a render. Arithmetic on an opaque
    /// refuses and says which one it was; the intercepted calls
    /// accept it, because they never look at the value.
    Opaque(&'static str),
}

impl IVal {
    fn scalar(&self) -> Result<Interval, Refusal> {
        match self {
            IVal::Scalar(i) => Ok(*i),
            IVal::Int(v) => Ok(Interval::point(*v as f64)),
            IVal::Opaque(w) => Err(Refusal::Opaque(w)),
            _ => Err(Refusal::Shape("expected a scalar")),
        }
    }

    fn lanes(&self) -> Result<Vec<Interval>, Refusal> {
        match self {
            IVal::Vec(v) => Ok(v.clone()),
            IVal::Scalar(i) => Ok(vec![*i]),
            IVal::Int(v) => Ok(vec![Interval::point(*v as f64)]),
            IVal::Opaque(w) => Err(Refusal::Opaque(w)),
            _ => Err(Refusal::Shape("expected a vector")),
        }
    }
}

/// A parsed probe module plus the handle of one variation's body
/// inside it.
pub struct Body {
    module: naga::Module,
    handle: naga::Handle<naga::Function>,
}

/// **Parsed on first use, one variation at a time.**
///
/// This used to build and parse EVERY shipped variation up front,
/// batched through the probe's own planner so each body landed in the
/// module the GPU would compile. Correct, and it cost 201 ms on the
/// first call — paid on the UI thread, the first time a pan engaged
/// targeting, to bound the two or three variations the flame
/// actually uses.
///
/// A batch of one builds the same way, so the body still sees its
/// helpers; it is simply the only one in the module. A flame pays for
/// its own variations and nothing else, and the corpus-wide tests pay
/// the same total as before, spread out.
static MODULES: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, Option<&'static Body>>>,
> = std::sync::OnceLock::new();

fn body_for(name: &str) -> Option<&'static Body> {
    let cache = MODULES.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok()?.get(name) {
        return *hit;
    }

    let built = (|| {
        use crate::probe::batch::{builtin_targets, Batch, Target};
        // A shipped variation, or one of the user's registered (a Rhai
        // variation): its compiled body, in a probe module as a built-in's.
        let target = builtin_targets().into_iter().find(|t| t.name == name).or_else(|| {
            let reg = crate::variations::global_registry();
            let info = reg.get(name).filter(|i| !i.provenance.is_builtin())?;
            Some(Target { name: info.name.clone(), slots: info.slot_count(), needs_init: info.init_param_count > 0, phase: info.phase.clone() })
        })?;
        let slots = target.slots;
        let batch = Batch { targets: vec![target], slots };
        let src = crate::probe::shader::build(&batch, false);
        let module = naga::front::wgsl::parse_str(&src).ok()?;
        let want = format!("variation_{name}");
        let handle = module
            .functions
            .iter()
            .find(|(_, f)| f.name.as_deref() == Some(want.as_str()))
            .map(|(h, _)| h)?;
        // Leaked deliberately: at most one per shipped variation, each
        // parsed once, and the evaluator hands out `&'static` borrows
        // of it.
        Some(&*Box::leak(Box::new(Body { module, handle })))
    })();

    if let Ok(mut c) = cache.lock() {
        c.insert(name.to_string(), built);
    }
    built
}

/// Every variation's `wgsl_init`, parsed on its own.
///
/// Separate from [`modules`] because the probe shader does not emit
/// these: the app runs the init pass as its own dispatch, so the
/// function never appears in the render module. The bodies are
/// self-contained (`fn init_x(user: array<f32, N>) -> array<f32, M>`),
/// so they parse alone.
static INITS: std::sync::OnceLock<
    std::collections::HashMap<String, (naga::Module, naga::Handle<naga::Function>)>,
> = std::sync::OnceLock::new();

fn init_module(name: &str) -> Option<(&'static naga::Module, naga::Handle<naga::Function>)> {
    if let Some(hit) = builtin_init_module(name) {
        return Some(hit);
    }
    custom_init_module(name)
}

/// A user's variation's init (a Rhai variation's `prepare`), parsed once
/// per key -- a key is its source's, so an edit is another.
fn custom_init_module(name: &str) -> Option<(&'static naga::Module, naga::Handle<naga::Function>)> {
    static CUSTOM: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<&'static (naga::Module, naga::Handle<naga::Function>)>>>> =
        std::sync::OnceLock::new();
    let cache = CUSTOM.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok()?.get(name) {
        return hit.map(|(m, h)| (m, *h));
    }
    let built = (|| {
        let src = crate::variations::global_registry().get(name).filter(|i| !i.provenance.is_builtin())?.wgsl_source_init.clone()?;
        // The init pass splices complex.wgsl in beside it (`build_init_shader`).
        let module = naga::front::wgsl::parse_str(&format!("{}\n{src}", include_str!("../../shaders/core/complex.wgsl")))
            .or_else(|_| naga::front::wgsl::parse_str(&src))
            .ok()?;
        let want = format!("init_{name}");
        let handle = module.functions.iter().find(|(_, f)| f.name.as_deref() == Some(want.as_str())).map(|(h, _)| h)?;
        Some(&*Box::leak(Box::new((module, handle))))
    })();
    if let Ok(mut c) = cache.lock() {
        c.insert(name.to_string(), built);
    }
    built.map(|(m, h)| (m, *h))
}

fn builtin_init_module(name: &str) -> Option<(&'static naga::Module, naga::Handle<naga::Function>)> {
    let map = INITS.get_or_init(|| {
        let mut out = std::collections::HashMap::new();
        for def in crate::variations::defs::ALL_VARIATIONS {
            let Some(src) = def.wgsl_init else { continue };
            let Ok(module) = naga::front::wgsl::parse_str(src) else { continue };
            let want = format!("init_{}", def.name);
            let handle = module
                .functions
                .iter()
                .find(|(_, f)| f.name.as_deref() == Some(want.as_str()))
                .map(|(h, _)| h);
            if let Some(h) = handle {
                out.insert(def.name.to_string(), (module, h));
            }
        }
        out
    });
    map.get(name).map(|(m, h)| (m, *h))
}

/// The disc a variation's body sends `ball` into, derived from its
/// WGSL.
///
/// `weight` is the transform's weight for this variation, which a few
/// bodies read directly (`transforms[xform_id].variations[id]`);
/// applying it to the RESULT is the caller's job, exactly as for a
/// hand-written bound.
pub fn derive(
    name: &str,
    params: ParamFn,
    weight: f64,
    ball: Ball,
) -> Result<Ball, Refusal> {
    derive_subdivided(name, params, weight, ball, 1)
}

/// The same, with the variation's random arm pinned.
///
/// For a many-valued body — `julian` and the rest of the julia family
/// — this is the difference between a bound that shrinks and one that
/// cannot. See [`Branch`].
pub fn derive_branch(
    name: &str,
    params: ParamFn,
    weight: f64,
    ball: Ball,
    branch: Branch,
) -> Result<Ball, Refusal> {
    let cell = [
        Interval::new(ball.c[0] - ball.r, ball.c[0] + ball.r),
        Interval::new(ball.c[1] - ball.r, ball.c[1] + ball.r),
    ];
    box_to_disc(derive_box_branched(name, params, weight, cell, Some(branch))?)
}

/// The same, over a `k × k` grid of sub-boxes whose union is taken.
///
/// Interval arithmetic over-estimates whenever a value appears more
/// than once — `x − x` is `[−2r, 2r]`, not `0` — and the error grows
/// with the width of the input. Splitting the input and unioning the
/// pieces shrinks it: each cell is narrower, so each cell's answer is
/// tighter, and the union of exact-enough answers beats one loose
/// one. The cost is `k²` evaluations.
///
/// **A cell that refuses refuses the whole thing.** The union has to
/// cover every input, so a pole in one corner is a pole for the
/// disc — subdivision can only prove that the OTHER cells were fine,
/// never that the bad one does not matter.
pub fn derive_subdivided(
    name: &str,
    params: ParamFn,
    weight: f64,
    ball: Ball,
    k: usize,
) -> Result<Ball, Refusal> {
    let k = k.max(1);
    let step = 2.0 * ball.r / k as f64;
    let mut acc: Option<[Interval; 2]> = None;
    for iy in 0..k {
        for ix in 0..k {
            let lo_x = ball.c[0] - ball.r + step * ix as f64;
            let lo_y = ball.c[1] - ball.r + step * iy as f64;
            let cell = [
                Interval::new(lo_x, lo_x + step),
                Interval::new(lo_y, lo_y + step),
            ];
            let out = derive_box(name, params, weight, cell)?;
            acc = Some(match acc {
                None => out,
                Some(a) => [a[0].union(out[0]), a[1].union(out[1])],
            });
        }
    }
    let out = acc.ok_or(Refusal::Shape("no cells"))?;
    box_to_disc(out)
}

/// One evaluation of the body over one input box.
fn derive_box(
    name: &str,
    params: ParamFn,
    weight: f64,
    input: [Interval; 2],
) -> Result<[Interval; 2], Refusal> {
    derive_box_branched(name, params, weight, input, None)
}

fn derive_box_branched(
    name: &str,
    params: ParamFn,
    weight: f64,
    input: [Interval; 2],
    branch: Option<Branch>,
) -> Result<[Interval; 2], Refusal> {
    // An unedited twin is its built-in: its body, its parameters and its
    // derived slots (`twins::planner_name`).
    evaluate(super::twins::planner_name(name), params, weight, input, branch)
}

/// [`derive`] of the variation `name`'s own body, not an unedited twin's
/// built-in's: what a twin's compiled code derives (the corpus measure).
#[cfg(test)]
pub(crate) fn derive_own(name: &str, params: ParamFn, weight: f64, ball: Ball) -> Result<Ball, Refusal> {
    let cell = [Interval::new(ball.c[0] - ball.r, ball.c[0] + ball.r), Interval::new(ball.c[1] - ball.r, ball.c[1] + ball.r)];
    box_to_disc(evaluate(name, params, weight, cell, None)?)
}

/// The body of the variation registered as `name`, through the interval
/// evaluator.
fn evaluate(
    name: &str,
    params: ParamFn,
    weight: f64,
    input: [Interval; 2],
    branch: Option<Branch>,
) -> Result<[Interval; 2], Refusal> {
    let body = body_for(name).ok_or(Refusal::NoBody)?;
    let module = &body.module;
    let func = &module.functions[body.handle];

    // A 2D body may still read `p.z` through a lifted vec3 in some
    // helper; the plane has no z, so it is exactly zero.
    let point = vec![input[0], input[1], Interval::point(0.0)];

    // Argument 0 is the point. Everything after it -- `xform_id`,
    // `variation_id`, the RNG pointer, a `NeedsAccum` body's running
    // sum -- is carried opaquely, so a body that only PASSES them to
    // an intercepted call works, and one that does arithmetic on them
    // refuses by name instead of being given a made-up number.
    let mut args = vec![IVal::Vec(point)];
    for a in func.arguments.iter().skip(1) {
        args.push(IVal::Opaque(match a.name.as_deref() {
            Some("xform_id") => "the transform index",
            Some("variation_id") => "the variation index",
            Some("rng") => "the RNG state",
            Some("accum") => "the running variation sum",
            _ => "an opaque argument",
        }));
    }

    let mut ev = Eval { module, params, weight, name, rng: rng_for(branch), types: Default::default() };
    let out = ev.call(func, &args)?;
    if let IVal::Opaque(w) = out {
        return Err(found(w));
    }
    let lanes = out.lanes()?;
    if lanes.len() < 2 {
        return Err(Refusal::Shape("body did not return a vector"));
    }
    Ok([lanes[0], lanes[1]])
}

/// Whether two expressions are the same value: one handle, or the same
/// argument, constant or variable read through the same parts (`z.x *
/// z.x`, the formula language's squared norm). Two loads of one variable
/// in one expression see one value: an expression stores nothing.
fn same_value(func: &naga::Function, a: naga::Handle<naga::Expression>, b: naga::Handle<naga::Expression>) -> bool {
    use naga::Expression as E;
    if a == b {
        return true;
    }
    match (&func.expressions[a], &func.expressions[b]) {
        (E::FunctionArgument(x), E::FunctionArgument(y)) => x == y,
        (E::LocalVariable(x), E::LocalVariable(y)) => x == y,
        (E::GlobalVariable(x), E::GlobalVariable(y)) => x == y,
        (E::Constant(x), E::Constant(y)) => x == y,
        (E::Load { pointer: x }, E::Load { pointer: y }) => same_value(func, *x, *y),
        (E::AccessIndex { base: x, index: i }, E::AccessIndex { base: y, index: j }) => i == j && same_value(func, *x, *y),
        _ => false,
    }
}

/// The formula language's `max` and `min`: `if (a > b) { b = a; }` with no
/// `else` (and its mirrors `b < a`, and `<` for `min`) -- the variable
/// `b` is, the condition's load of it, the other operand, and whether it
/// is the max.
fn as_extremum(
    func: &naga::Function,
    condition: naga::Handle<naga::Expression>,
    accept: &naga::Block,
    reject: &naga::Block,
) -> Option<(naga::Handle<naga::Expression>, naga::Handle<naga::Expression>, naga::Handle<naga::Expression>, bool)> {
    use naga::{BinaryOperator as B, Expression as E, Statement as S};
    let real = |b: &naga::Block| b.iter().filter(|s| !matches!(s, S::Emit(_))).cloned().collect::<Vec<_>>();
    if !real(reject).is_empty() {
        return None;
    }
    let [S::Store { pointer, value }] = real(accept)[..] else { return None };
    let E::Binary { op, left, right } = func.expressions[condition] else { return None };
    // The variable's own load on one side, the stored value on the other.
    let held = |h: naga::Handle<naga::Expression>| matches!(func.expressions[h], E::Load { pointer: p } if same_value(func, p, pointer));
    let (load, other, greater) = match op {
        B::Greater | B::GreaterEqual if held(right) && same_value(func, left, value) => (right, left, true),
        B::Less | B::LessEqual if held(right) && same_value(func, left, value) => (right, left, false),
        B::Less | B::LessEqual if held(left) && same_value(func, right, value) => (left, right, true),
        B::Greater | B::GreaterEqual if held(left) && same_value(func, right, value) => (left, right, false),
        _ => return None,
    };
    Some((pointer, load, other, greater))
}

/// **A cast, by what it casts to.** It used to pass its value through,
/// which left an `i32(x)` a float to the evaluator: integer division then
/// came out as float division, and `f32(k)` of an exact integer stayed an
/// exact integer, divided as one. Now:
///
/// - to `i32`/`u32` by conversion: an exact value becomes an exact
///   integer, truncated and saturated as WGSL's conversion is; a range,
///   the range of its ends' conversions (monotone); between the two
///   integer kinds, the same bits read the other way;
/// - to `i32`/`u32` by `bitcast` (`from_integer`, the source's type):
///   of the other integer kind, the same bits read the other way, which
///   keeps a value in `[0, 2^31)` (the formula language's loop bounds);
///   of a float, any 32-bit pattern;
/// - to `f32` by conversion: the integer's value, rounded to f32 past
///   2^24; by `bitcast`, a value this cannot name (an opaque);
/// - to `bool`: whether it is non-zero, where that is decided.
fn cast(v: IVal, kind: naga::ScalarKind, convert: bool, from_integer: bool) -> Result<IVal, Refusal> {
    use naga::ScalarKind as K;
    const TWO32: i64 = 1 << 32;
    // An integer's bits read as the other integer kind.
    let reinterpret = |k: i64, kind: K| match kind {
        K::Uint => k.rem_euclid(TWO32),
        _ => {
            let u = k.rem_euclid(TWO32);
            if u >= 1 << 31 { u - TWO32 } else { u }
        }
    };
    // WGSL's float-to-integer conversion: toward zero, then saturated.
    let (lo_sat, hi_sat) = if kind == K::Uint { (0.0, 4294967295.0) } else { (-2147483648.0, 2147483647.0) };
    let to_int = |x: f64| x.trunc().clamp(lo_sat, hi_sat);
    // An integer range read as the other integer kind: unchanged inside
    // `[0, 2^31)`, where both read the bits alike; else that kind's span.
    let reread = |i: Interval| -> Interval {
        if i.lo >= 0.0 && i.hi < 2147483648.0 {
            i
        } else if kind == K::Uint {
            Interval::new(0.0, 4294967295.0)
        } else {
            Interval::new(-2147483648.0, 2147483647.0)
        }
    };
    Ok(match (kind, v) {
        (_, IVal::Opaque(w)) => IVal::Opaque(w),
        (K::Sint | K::Uint, IVal::Int(k)) => IVal::Int(reinterpret(k, kind)),
        (K::Sint | K::Uint, IVal::Scalar(i)) if !convert && from_integer => {
            if i.lo == i.hi && i.lo.is_finite() && i.lo.fract() == 0.0 {
                IVal::Int(reinterpret(i.lo as i64, kind))
            } else {
                IVal::Scalar(reread(i))
            }
        }
        (K::Sint | K::Uint, IVal::Vec(l)) if !convert && from_integer => IVal::Vec(l.into_iter().map(reread).collect()),
        (K::Sint | K::Uint, IVal::Scalar(_)) if !convert => IVal::Scalar(INT32_RANGE),
        (K::Sint | K::Uint, IVal::Scalar(i)) if i.lo == i.hi && i.lo.is_finite() => IVal::Int(to_int(i.lo) as i64),
        (K::Sint | K::Uint, IVal::Scalar(i)) => {
            if !i.finite() {
                IVal::Scalar(Interval::new(lo_sat, hi_sat))
            } else {
                IVal::Scalar(Interval::new(to_int(i.lo), to_int(i.hi)))
            }
        }
        (K::Sint | K::Uint, IVal::Vec(l)) if !convert => IVal::Vec(vec![INT32_RANGE; l.len()]),
        (K::Sint | K::Uint, IVal::Vec(l)) => IVal::Vec(
            l.into_iter()
                .map(|i| if i.finite() { Interval::new(to_int(i.lo), to_int(i.hi)) } else { Interval::new(lo_sat, hi_sat) })
                .collect(),
        ),
        (K::Sint | K::Uint, IVal::Bool(b)) => match b {
            Some(b) => IVal::Int(b as i64),
            None => IVal::Scalar(Interval::new(0.0, 1.0)),
        },
        (K::Float, IVal::Int(_)) if !convert => IVal::Opaque("a reinterpreted bit pattern"),
        (K::Float, IVal::Scalar(_) | IVal::Vec(_)) if !convert => IVal::Opaque("a reinterpreted bit pattern"),
        (K::Float, IVal::Int(k)) => {
            if k.abs() <= 1 << 24 {
                IVal::Scalar(Interval::point(k as f64))
            } else {
                IVal::Scalar(Interval::point(k as f64).widen())
            }
        }
        (K::Float, IVal::Bool(b)) => match b {
            Some(b) => IVal::Scalar(Interval::point(if b { 1.0 } else { 0.0 })),
            None => IVal::Scalar(Interval::new(0.0, 1.0)),
        },
        (K::Float, v) => v,
        (K::Bool, IVal::Bool(b)) => IVal::Bool(b),
        (K::Bool, IVal::Int(k)) => IVal::Bool(Some(k != 0)),
        (K::Bool, IVal::Scalar(i)) => IVal::Bool(if i.lo == 0.0 && i.hi == 0.0 {
            Some(false)
        } else if i.lo > 0.0 || i.hi < 0.0 {
            Some(true)
        } else {
            None
        }),
        (_, IVal::Vec(_)) => return Err(Refusal::Shape("a vector cast to bool")),
        _ => return Err(Refusal::Shape("an unmodelled cast")),
    })
}

/// A condition's value, where the evaluator can tell.
fn as_bool(v: &IVal) -> Option<bool> {
    match v {
        IVal::Bool(b) => *b,
        // A numeric used as a condition is not something WGSL allows,
        // so this only guards against a shape slip.
        _ => None,
    }
}

/// Tags for the `transforms[i].variations[j]` access chain. The
/// measurement behind the shortcut: across 647 bodies, `variations`
/// is the ONLY field of `transforms` any of them reads (107 do), so
/// a chain that indexes twice from that global is the weight and
/// nothing else. `the_weight_chain_is_the_only_one` pins it.
/// **A pole or a domain exit is a value, not yet a refusal.** The shader
/// computes an infinity or a NaN there, which matters only if it reaches
/// the returned point: one feeding a colour, or a condition (both arms
/// taken, as for any undecided one), leaves the bound sound. So a
/// division by a range through zero gives an opaque, refused at the
/// return as the pole it was (`found`). Ten twins divided by a range
/// through zero in a computation the point never read (`glsl_squares`'
/// carpet test, which only picks a colour).
const PAST_A_POLE: &str = "a value past a pole";

/// The opaque a read of per-thread state gives.
const STATE: &str = "per-thread state";

/// The opaque a domain exit leaves, by the function's name.
const OUTSIDE: &[(&str, &str)] = &[
    ("log", "a value outside the domain of log"),
    ("log2", "a value outside the domain of log2"),
    ("acosh", "a value outside the domain of acosh"),
    ("atanh", "a value outside the domain of atanh"),
    ("asin", "a value outside the domain of asin"),
    ("acos", "a value outside the domain of acos"),
    ("sqrt", "a value outside the domain of sqrt"),
    ("pow with a negative base", "a value outside the domain of pow with a negative base"),
    ("pow", "a value outside the domain of pow"),
];

/// A pole or a domain exit as an opaque value; anything else as it was.
fn lost(r: Result<IVal, Refusal>) -> Result<IVal, Refusal> {
    match r {
        Err(Refusal::Pole) => Ok(IVal::Opaque(PAST_A_POLE)),
        Err(Refusal::Domain(d)) => Ok(IVal::Opaque(OUTSIDE.iter().find(|(n, _)| *n == d).map_or("a value outside a function's domain", |(_, t)| t))),
        r => r,
    }
}

/// The refusal for an answer that depends on the opaque `w`: a pole or a
/// domain exit by its own name, as when they refused where they happened.
fn found(w: &'static str) -> Refusal {
    if w == PAST_A_POLE {
        return Refusal::Pole;
    }
    if w == STATE {
        return Refusal::State;
    }
    match OUTSIDE.iter().find(|(_, t)| *t == w) {
        Some((d, _)) => Refusal::Domain(d),
        None => Refusal::Reaches(w),
    }
}

const TRANSFORMS: &str = "the transform table";
const ONE_TRANSFORM: &str = "one transform's record";
const WEIGHTS: &str = "the variation weights";

/// One step along that chain: the table, then a transform, then its
/// weight array, and the next index after that is the weight itself.
///
/// Three steps and not two. Collapsing the first two was worth 71
/// bodies of spurious refusal, because the chain then arrived at the
/// weight array one index early and the real `[variation_id]` fell
/// through to ordinary arithmetic on an opaque.
fn step_transform_chain(v: &IVal) -> Option<IVal> {
    match v {
        IVal::Opaque(t) if *t == TRANSFORMS => Some(IVal::Opaque(ONE_TRANSFORM)),
        IVal::Opaque(t) if *t == ONE_TRANSFORM => Some(IVal::Opaque(WEIGHTS)),
        _ => None,
    }
}

struct Eval<'a> {
    module: &'a naga::Module,
    params: ParamFn<'a>,
    weight: f64,
    name: &'a str,
    /// What `rng_nextf` is allowed to return.
    ///
    /// `[0, 1)` for an ordinary bound, which is why a stochastic
    /// variation is usually the easy case: the answer covers every
    /// draw. For `julian` and its kin that is exactly the problem —
    /// the draw picks which ARM the point lands on, so covering every
    /// draw means covering a whole annulus, and the bound is `O(1)`
    /// however small the input. Narrowing this to one arm's slice is
    /// what lets a word through such a variation shrink at all.
    rng: Interval,
    /// Each function's expression types, by the function's address,
    /// grown as asked (`is_integer`).
    types: std::collections::HashMap<usize, naga::front::Typifier>,
}

/// Locals and evaluated expressions for one function activation.
struct Frame {
    args: Vec<IVal>,
    exprs: std::collections::HashMap<usize, IVal>,
    locals: Locals,
    /// **States the body MIGHT have left a loop in.**
    ///
    /// `if (i >= max_loop) { break; }` with a condition the evaluator
    /// cannot decide is a loop that might end here and might not. The
    /// `If` handler over-approximates by continuing down the other
    /// path, which is right for what happens NEXT — but the state at
    /// the break is a real exit state, and dropping it means the loop
    /// reports only the state after its last iteration.
    ///
    /// That is an under-estimate, the one direction a forward bound
    /// may never go: `iconattractor_js` rotates its point once per
    /// degree and breaks out early, and reading only the 24th
    /// rotation put the shader's real output outside the derived
    /// disc. Each enclosing `Loop` drains what its own body pushed
    /// and unions it into the exit.
    maybe_exits: Vec<Locals>,
    /// Every value the body might return. A body with one `return`
    /// puts one here; one that returns from inside a branch the
    /// evaluator could not decide puts several, and the answer is
    /// their union.
    returns: Vec<IVal>,
    /// The argument of each call to the formula language's trig range
    /// reduction (`uf_reduce`), by its result's handle: a `sin`, `cos`
    /// or `tan` of the result is taken of the argument.
    reduced: std::collections::HashMap<usize, IVal>,
}

type Locals = std::collections::HashMap<usize, IVal>;

/// The union of two values, for joining branch arms and loop
/// iterations.
fn join(a: &IVal, b: &IVal) -> Result<IVal, Refusal> {
    Ok(match (a, b) {
        (IVal::Scalar(x), IVal::Scalar(y)) => IVal::Scalar(x.union(*y)),
        (IVal::Int(x), IVal::Int(y)) if x == y => IVal::Int(*x),
        (IVal::Int(x), IVal::Int(y)) => IVal::Scalar(
            Interval::point(*x as f64).union(Interval::point(*y as f64)),
        ),
        (IVal::Scalar(x), IVal::Int(y)) | (IVal::Int(y), IVal::Scalar(x)) => {
            IVal::Scalar(x.union(Interval::point(*y as f64)))
        }
        // A lane one side never wrote is WGSL-zeroed, as `AccessIndex`
        // reads it, so it joins as zero -- not as the other side's lane,
        // which was a bound too tight wherever only one arm wrote it.
        (IVal::Vec(x), IVal::Vec(y)) => {
            let n = x.len().max(y.len());
            let zero = Interval::point(0.0);
            IVal::Vec((0..n).map(|i| x.get(i).unwrap_or(&zero).union(*y.get(i).unwrap_or(&zero))).collect())
        }
        // A vector against a scalar: the scalar is a whole-vector value
        // the evaluator holds as one lane (an unwritten local's zero, a
        // splat), so it joins every lane.
        (IVal::Vec(x), s) | (s, IVal::Vec(x)) if matches!(s, IVal::Scalar(_) | IVal::Int(_)) => {
            let s = s.scalar()?;
            IVal::Vec(x.iter().map(|l| l.union(s)).collect())
        }
        (IVal::Bool(x), IVal::Bool(y)) => {
            IVal::Bool(if x == y { *x } else { None })
        }
        // An opaque on either side: the joined value is one this cannot
        // name either, and it stays opaque -- refused where it reaches
        // the answer, as before the join.
        (IVal::Opaque(x), _) | (_, IVal::Opaque(x)) => IVal::Opaque(x),
        // A bool against a number: a shape slip, and saying anything
        // would be the unsound move.
        _ => return Err(Refusal::Shape("joined two incompatible values")),
    })
}

/// Join two post-branch local tables against the state they both
/// started from. A local only one arm wrote still has the incoming
/// value on the other path, which is what `before` supplies.
///
/// One no path had written before is WGSL-zeroed on the path that did
/// not write it -- the zero of the type the other path's value shows: a
/// `bool` declared inside the branch's scope is `false` there, not the
/// number zero, which no join can meet (the crop family's flags).
fn join_locals(before: &Locals, a: &Locals, b: &Locals) -> Result<Locals, Refusal> {
    let mut out = Locals::new();
    for k in a.keys().chain(b.keys()) {
        if out.contains_key(k) {
            continue;
        }
        let va = a.get(k).or_else(|| before.get(k));
        let vb = b.get(k).or_else(|| before.get(k));
        let joined = match (va, vb) {
            (Some(x), Some(y)) => join(x, y)?,
            (Some(x), None) | (None, Some(x)) => join(x, &zero_like(x))?,
            (None, None) => continue,
        };
        out.insert(*k, joined);
    }
    Ok(out)
}

/// The WGSL zero of the type `v` has.
fn zero_like(v: &IVal) -> IVal {
    match v {
        IVal::Bool(_) => IVal::Bool(Some(false)),
        IVal::Vec(l) => IVal::Vec(vec![Interval::point(0.0); l.len()]),
        IVal::Int(_) => IVal::Int(0),
        IVal::Scalar(_) => IVal::Scalar(Interval::point(0.0)),
        IVal::Opaque(w) => IVal::Opaque(w),
    }
}

impl<'a> Eval<'a> {
    fn call(&mut self, func: &naga::Function, args: &[IVal]) -> Result<IVal, Refusal> {
        let mut frame = Frame {
            args: args.to_vec(),
            exprs: std::collections::HashMap::new(),
            locals: Locals::new(),
            maybe_exits: Vec::new(),
            returns: Vec::new(),
            reduced: std::collections::HashMap::new(),
        };
        // **Declaration initialisers, before anything runs.**
        // `var inv = 1.0;` is a `LocalVariable` carrying an `init`
        // expression, not a `Store`, so a body that only assigns the
        // local inside one arm of a branch leaves it looking
        // unwritten on the other path. Defaulting that to zero is not
        // merely imprecise, it is WRONG -- `yin_yang` joined its
        // `inv` of -1 against a phantom 0 instead of the declared 1,
        // and every output came back on the wrong side of the origin.
        for (h, local) in func.local_variables.iter() {
            if let Some(init) = local.init {
                let v = self.expr(func, init, &mut frame)?;
                frame.locals.insert(h.index(), v);
            }
        }

        self.block(func, &func.body, &mut frame)?;
        let mut it = frame.returns.into_iter();
        let first = it.next().ok_or(Refusal::Shape("returned nothing"))?;
        let mut acc = first;
        for v in it {
            acc = join(&acc, &v)?;
        }
        Ok(acc)
    }

    /// How deep a loop is unrolled before the evaluator gives up.
    ///
    /// A loop with a literal or parameter bound has an exact `Int`
    /// counter, so its exit condition becomes definitely true on the
    /// right iteration and it never approaches this. The cap is for
    /// the ones whose bound the evaluator cannot pin down, where the
    /// honest answer is a refusal rather than a disc from a guess.
    const LOOP_CAP: usize = 64;

    fn block(
        &mut self,
        func: &naga::Function,
        block: &naga::Block,
        frame: &mut Frame,
    ) -> Result<Flow, Refusal> {
        for st in block.iter() {
            match st {
                naga::Statement::Emit(range) => {
                    for h in range.clone() {
                        // **Always recomputed, never read from the
                        // cache.** An `Emit` means "these values are
                        // computed here", and inside a loop or a
                        // re-entered arm the same handle stands for a
                        // different value each time. Serving the
                        // cached one is how a stale value reaches a
                        // bound and makes it too tight -- the same
                        // fault `r_circleblur` caught for locals.
                        let e = &func.expressions[h];
                        let v = self.expr_inner(func, h, e, frame)?;
                        frame.exprs.insert(h.index(), v);
                    }
                }
                naga::Statement::Store { pointer, value } => {
                    let v = self.expr(func, *value, frame)?;
                    self.store(func, *pointer, v, frame)?;
                }
                naga::Statement::Return { value } => {
                    if let Some(h) = value {
                        let v = self.expr(func, *h, frame)?;
                        frame.returns.push(v);
                    }
                    return Ok(Flow::Return);
                }
                naga::Statement::Block(b) => match self.block(func, b, frame)? {
                    Flow::Fell => {}
                    other => return Ok(other),
                },
                naga::Statement::If { condition, accept, reject } if as_extremum(func, *condition, accept, reject).is_some() => {
                    // `if (a > b) { b = a; }`: `b = max(b, a)`, as the
                    // formula language writes Rhai's `max` (a test and an
                    // assignment, for its NaN rule) -- exactly the max on
                    // ranges, where joining the two arms loses the guard
                    // (`max(|y|, 1e-20)` reached zero, a pole).
                    let (pointer, load, other, max) = as_extremum(func, *condition, accept, reject).expect("matched");
                    let held = self.expr(func, load, frame)?;
                    let o = self.expr(func, other, frame)?;
                    if let IVal::Opaque(w) = held.clone() {
                        self.store(func, pointer, IVal::Opaque(w), frame)?;
                        continue;
                    }
                    if let IVal::Opaque(w) = o {
                        self.store(func, pointer, IVal::Opaque(w), frame)?;
                        continue;
                    }
                    if let (IVal::Int(x), IVal::Int(y)) = (&held, &o) {
                        let v = IVal::Int(if max { *x.max(y) } else { *x.min(y) });
                        self.store(func, pointer, v, frame)?;
                        continue;
                    }
                    let (h, o) = (held.lanes()?, o.lanes()?);
                    let pick = |x: Interval, y: Interval| {
                        if max { Interval::new(x.lo.max(y.lo), x.hi.max(y.hi)) } else { Interval::new(x.lo.min(y.lo), x.hi.min(y.hi)) }
                    };
                    let lanes: Vec<Interval> = (0..h.len().max(o.len())).map(|i| pick(*h.get(i).unwrap_or(&h[0]), *o.get(i).unwrap_or(&o[0]))).collect();
                    let v = if lanes.len() == 1 { IVal::Scalar(lanes[0]) } else { IVal::Vec(lanes) };
                    self.store(func, pointer, v, frame)?;
                }
                naga::Statement::If { condition, accept, reject } => {
                    let c = self.expr(func, *condition, frame)?;
                    match as_bool(&c) {
                        Some(true) => match self.block(func, accept, frame)? {
                            Flow::Fell => {}
                            other => return Ok(other),
                        },
                        Some(false) => match self.block(func, reject, frame)? {
                            Flow::Fell => {}
                            other => return Ok(other),
                        },
                        None => {
                            // Both arms, from the same incoming state,
                            // and the union of what either assigned.
                            // A `return` inside one of them has
                            // already put its value in `frame.returns`,
                            // so the caller's answer accounts for it
                            // and execution continues down the other
                            // path -- an over-approximation, which is
                            // the safe direction.
                            let before = frame.locals.clone();
                            let fa = self.block(func, accept, frame)?;
                            let after_a =
                                std::mem::replace(&mut frame.locals, before.clone());
                            let fb = self.block(func, reject, frame)?;
                            let after_b = frame.locals.clone();
                            frame.locals = join_locals(&before, &after_a, &after_b)?;
                            // Only when BOTH arms left the block does
                            // control certainly leave it.
                            match (&fa, &fb) {
                                // One arm may have broken out and the
                                // other did not. Keep going down the
                                // other path, but remember where the
                                // first one stopped, or the loop will
                                // report an iteration count the shader
                                // never reached.
                                (Flow::Break, Flow::Fell) => {
                                    frame.maybe_exits.push(after_a)
                                }
                                (Flow::Fell, Flow::Break) => {
                                    frame.maybe_exits.push(after_b)
                                }
                                // A `continue` on an undecided branch
                                // has no such repair: falling through
                                // runs the rest of the body, which the
                                // real iteration skipped, and the
                                // state that comes out is not a
                                // superset of either path. Refuse
                                // instead of guessing. No shipped body
                                // reaches this today.
                                (Flow::Continue, _) | (_, Flow::Continue) => {
                                    return Err(Refusal::Construct(
                                        "a continue on a branch that could not be decided",
                                    ))
                                }
                                (Flow::Fell, _) | (_, Flow::Fell) => {}
                                (Flow::Break, _) | (_, Flow::Break) => {
                                    return Ok(Flow::Break)
                                }
                                _ => return Ok(Flow::Return),
                            }
                        }
                    }
                }
                naga::Statement::Switch { selector, cases } => {
                    // Sound without deciding the selector: every case
                    // body from the same incoming state, all unioned.
                    let _ = self.expr(func, *selector, frame)?;
                    let before = frame.locals.clone();
                    let mut acc = before.clone();
                    for case in cases.iter() {
                        frame.locals = before.clone();
                        self.block(func, &case.body, frame)?;
                        acc = join_locals(&before, &acc, &frame.locals)?;
                    }
                    frame.locals = acc;
                }
                naga::Statement::Loop { body, continuing, break_if } => {
                    let before = frame.locals.clone();
                    let mut exits: Vec<Locals> = Vec::new();
                    let mut settled = false;
                    // Anything already pending belongs to a loop
                    // further out; only what this body adds is ours.
                    let mark = frame.maybe_exits.len();
                    for _ in 0..Self::LOOP_CAP {
                        let flow = self.block(func, body, frame)?;
                        if matches!(flow, Flow::Break) {
                            exits.push(frame.locals.clone());
                            settled = true;
                            break;
                        }
                        if matches!(flow, Flow::Return) {
                            exits.push(frame.locals.clone());
                            settled = true;
                            break;
                        }
                        self.block(func, continuing, frame)?;
                        match break_if {
                            Some(h) => {
                                let c = self.expr(func, *h, frame)?;
                                match as_bool(&c) {
                                    Some(true) => {
                                        exits.push(frame.locals.clone());
                                        settled = true;
                                    }
                                    // Might have left here; record the
                                    // state and carry on.
                                    None => exits.push(frame.locals.clone()),
                                    Some(false) => {}
                                }
                                if settled {
                                    break;
                                }
                            }
                            None => {}
                        }
                    }
                    if !settled {
                        return Err(Refusal::LoopUnsettled);
                    }
                    exits.extend(frame.maybe_exits.drain(mark..));
                    let mut acc = exits.pop().unwrap_or_else(|| before.clone());
                    for e in &exits {
                        acc = join_locals(&before, &acc, e)?;
                    }
                    frame.locals = acc;
                }
                naga::Statement::Break => return Ok(Flow::Break),
                naga::Statement::Continue => return Ok(Flow::Continue),
                naga::Statement::Call { function, arguments, result } => {
                    let mut vals = Vec::with_capacity(arguments.len());
                    for a in arguments {
                        vals.push(self.expr(func, *a, frame)?);
                    }
                    let callee = self.module.functions[*function].name.as_deref().unwrap_or("");
                    if let (Some(r), true, [t]) = (result, callee.ends_with("_reduce"), &vals[..]) {
                        frame.reduced.insert(r.index(), t.clone());
                    }
                    let v = self.dispatch(*function, &vals)?;
                    if let Some(r) = result {
                        frame.exprs.insert(r.index(), v);
                    }
                }
                naga::Statement::Kill => return Ok(Flow::Return),
                _ => return Err(Refusal::Construct("an unmodelled statement")),
            }
        }
        Ok(Flow::Fell)
    }

    /// The name of an expression's shape, for a refusal that has to
    /// say what it could not handle. `Debug` would carry handles and
    /// make two refusals of the same kind look different.
    fn kind(e: &naga::Expression) -> &'static str {
        use naga::Expression as E;
        match e {
            E::Literal(_) => "a literal",
            E::Constant(_) => "a constant",
            E::Override(_) => "an override",
            E::ZeroValue(_) => "a zero value",
            E::Compose { .. } => "a compose",
            E::Access { .. } => "an access",
            E::AccessIndex { .. } => "an indexed access",
            E::Splat { .. } => "a splat",
            E::Swizzle { .. } => "a swizzle",
            E::FunctionArgument(_) => "a function argument",
            E::GlobalVariable(_) => "a global",
            E::LocalVariable(_) => "a local",
            E::Load { .. } => "a load",
            E::Unary { .. } => "a unary op",
            E::Binary { .. } => "a binary op",
            E::Select { .. } => "a select",
            E::Relational { .. } => "a relational op",
            E::Math { .. } => "an intrinsic",
            E::As { .. } => "a cast",
            E::CallResult(_) => "a call result",
            _ => "something unmodelled",
        }
    }

    /// Assign to a local, or to one lane of one.
    ///
    /// `out[0] = …` and `p.x = …` are both an `Access` on a
    /// `LocalVariable`, and 70 bodies do it; treating only whole-local
    /// stores was most of what stood between phase 1 and them.
    fn store(
        &mut self,
        func: &naga::Function,
        pointer: naga::Handle<naga::Expression>,
        v: IVal,
        frame: &mut Frame,
    ) -> Result<(), Refusal> {
        use naga::Expression as E;
        match &func.expressions[pointer] {
            naga::Expression::LocalVariable(lh) => {
                frame.locals.insert(lh.index(), v);
                Ok(())
            }
            naga::Expression::AccessIndex { base, index } => {
                self.store_lane(func, *base, Some(*index as usize), v, frame)
            }
            naga::Expression::Access { base, index } => {
                let i = self.expr(func, *index, frame)?;
                let lane = match i {
                    IVal::Int(k) if k >= 0 => Some(k as usize),
                    // A lane the evaluator cannot pin down: widen
                    // every lane rather than guess which one moved.
                    _ => None,
                };
                self.store_lane(func, *base, lane, v, frame)
            }
            // **A write to an out-parameter this evaluator holds
            // opaquely, which is a write it can throw away.**
            //
            // 50 bodies take `vc: ptr<function, f32>` -- the colour
            // out-parameter of the dc_* family -- or `hide:
            // ptr<function, bool>`, and assign to it. Where the point
            // lands does not depend on either, and the top-level
            // caller discards them, so the store changes nothing this
            // is computing.
            //
            // Discarding is only sound because the pointer is opaque,
            // and that is the condition tested rather than the
            // parameter's name. An opaque argument is one of the
            // entry point's own (`vc`, `hide`, `rng`, `xform_id`), so
            // it cannot alias a local being tracked; and a later read
            // back through it loads the SAME opaque, so a body that
            // did feed its colour into its geometry refuses on the
            // arithmetic instead of quietly using a stale value. A
            // pointer to one of our own locals arrives as that
            // local's value, not as an opaque, and still refuses
            // below.
            E::FunctionArgument(i) => match frame.args.get(*i as usize) {
                Some(IVal::Opaque(_)) => Ok(()),
                _ => Err(Refusal::Intrinsic(
                    "a store through a pointer to a tracked value".into(),
                )),
            },
            other => Err(Refusal::Intrinsic(format!(
                "a store through {}",
                Self::kind(other)
            ))),
        }
    }

    fn store_lane(
        &mut self,
        func: &naga::Function,
        base: naga::Handle<naga::Expression>,
        lane: Option<usize>,
        v: IVal,
        frame: &mut Frame,
    ) -> Result<(), Refusal> {
        // A lane of an out-parameter held opaquely (`(*vrc).x = v1`, how
        // the formula language writes a direct colour back): thrown away,
        // as a whole store through one is (`store`).
        if let naga::Expression::FunctionArgument(i) = &func.expressions[base] {
            if let Some(IVal::Opaque(_)) = frame.args.get(*i as usize) {
                return Ok(());
            }
        }
        let naga::Expression::LocalVariable(lh) = &func.expressions[base] else {
            return Err(Refusal::Intrinsic(format!(
                "a lane store whose base is {}",
                Self::kind(&func.expressions[base])
            )));
        };
        let key = lh.index();
        let cur = frame.locals.get(&key).cloned();
        // One lane of a vector nothing is known about: the others are
        // still unknown, so the whole stays opaque.
        if let Some(IVal::Opaque(_)) = cur {
            return Ok(());
        }
        let mut lanes = match cur {
            Some(IVal::Vec(l)) => l,
            Some(IVal::Scalar(s)) => vec![s],
            _ => Vec::new(),
        };
        let val = v.scalar().unwrap_or(Interval::UNBOUNDED);
        match lane {
            Some(k) => {
                while lanes.len() <= k {
                    lanes.push(Interval::point(0.0));
                }
                lanes[k] = val;
            }
            None => {
                // Unknown lane: every one could be the one that
                // changed, so every one takes the union.
                if lanes.is_empty() {
                    lanes.push(val);
                } else {
                    for l in lanes.iter_mut() {
                        *l = l.union(val);
                    }
                }
            }
        }
        frame.locals.insert(key, IVal::Vec(lanes));
        Ok(())
    }

    fn expr(
        &mut self,
        func: &naga::Function,
        h: naga::Handle<naga::Expression>,
        frame: &mut Frame,
    ) -> Result<IVal, Refusal> {
        let e = &func.expressions[h];
        // **A local's value is never cached.** Every other expression
        // is pure and its handle names one value forever, but a
        // `var` is reassigned: `bx = round(bx * rad)` and then
        // `bx = bx + …` read the SAME `LocalVariable` handle either
        // side of a store. Caching it returns the value from before
        // the store, which silently drops everything assigned after
        // — a bound that is too TIGHT, and `r_circleblur` escaped its
        // disc by exactly that.
        // **Only `LocalVariable` is uncacheable, and the line is
        // exact.** An `Emit` always recomputes and overwrites, so any
        // expression it covers -- including the `Load` behind a
        // `let` -- is refreshed at the point the source says it is
        // computed, and caching it is what makes `let r1 = r2;` mean
        // r2's value THEN rather than after a later assignment. A
        // `LocalVariable` is a pointer, never emitted and so never
        // refreshed, and caching that is what made `bx = bx + …` read
        // the value from before the preceding store.
        //
        // Both directions were caught by the same gate, one in each
        // phase, and both made a bound too tight.
        let cacheable = !matches!(e, naga::Expression::LocalVariable(_));
        if cacheable {
            if let Some(v) = frame.exprs.get(&h.index()) {
                return Ok(v.clone());
            }
        }
        let v = self.expr_inner(func, h, e, frame)?;
        if cacheable {
            frame.exprs.insert(h.index(), v.clone());
        }
        Ok(v)
    }

    /// Whether the expression `h` of `func` has an integer type (a
    /// scalar or a vector of `i32` or `u32`), by naga's own resolution.
    fn is_integer(&mut self, func: &naga::Function, h: naga::Handle<naga::Expression>) -> Result<bool, Refusal> {
        let ctx = naga::proc::ResolveContext::with_locals(self.module, &func.local_variables, &func.arguments);
        let types = self.types.entry(func as *const naga::Function as usize).or_default();
        types.grow(h, &func.expressions, &ctx).map_err(|_| Refusal::Shape("an expression naga could not type"))?;
        let scalar = match types.get(h, &self.module.types) {
            naga::TypeInner::Scalar(s) | naga::TypeInner::Vector { scalar: s, .. } => *s,
            _ => return Ok(false),
        };
        Ok(matches!(scalar.kind, naga::ScalarKind::Sint | naga::ScalarKind::Uint))
    }

    fn expr_inner(
        &mut self,
        func: &naga::Function,
        h: naga::Handle<naga::Expression>,
        e: &naga::Expression,
        frame: &mut Frame,
    ) -> Result<IVal, Refusal> {
        use naga::Expression as E;
        Ok(match e {
            E::Literal(l) => match l {
                naga::Literal::F32(v) => IVal::Scalar(Interval::point(*v as f64)),
                naga::Literal::F64(v) => IVal::Scalar(Interval::point(*v)),
                naga::Literal::AbstractFloat(v) => IVal::Scalar(Interval::point(*v)),
                naga::Literal::I32(v) => IVal::Int(*v as i64),
                naga::Literal::U32(v) => IVal::Int(*v as i64),
                naga::Literal::I64(v) => IVal::Int(*v),
                naga::Literal::U64(v) => IVal::Int(*v as i64),
                naga::Literal::AbstractInt(v) => IVal::Int(*v),
                naga::Literal::Bool(b) => IVal::Bool(Some(*b)),
                naga::Literal::F16(v) => IVal::Scalar(Interval::point(f32::from(*v) as f64)),
            },
            E::FunctionArgument(i) => frame
                .args
                .get(*i as usize)
                .cloned()
                .ok_or(Refusal::Shape("argument out of range"))?,
            E::LocalVariable(lh) => frame
                .locals
                .get(&lh.index())
                .cloned()
                // An unwritten local is WGSL-zeroed.
                .unwrap_or(IVal::Scalar(Interval::point(0.0))),
            E::Load { pointer } => self.expr(func, *pointer, frame)?,
            E::Compose { components, .. } => {
                let mut lanes = Vec::new();
                for c in components {
                    let v = self.expr(func, *c, frame)?;
                    if let IVal::Opaque(w) = v {
                        return Ok(IVal::Opaque(w));
                    }
                    lanes.extend(v.lanes()?);
                }
                IVal::Vec(lanes)
            }
            E::Splat { size, value } => {
                let v = self.expr(func, *value, frame)?;
                if let IVal::Opaque(w) = v {
                    return Ok(IVal::Opaque(w));
                }
                IVal::Vec(vec![v.scalar()?; *size as usize])
            }
            E::AccessIndex { base, index } => {
                let b = self.expr(func, *base, frame)?;
                if let Some(v) = step_transform_chain(&b) {
                    return Ok(v);
                }
                // A part of an opaque (`(*vrc).x`, a direct colour's
                // channel, which the formula language reads into a local
                // at the top of every body that can write one) is as
                // opaque as the whole.
                if let IVal::Opaque(w) = b {
                    return Ok(IVal::Opaque(w));
                }
                let lanes = b.lanes()?;
                // A lane past what the evaluator has tracked is a
                // lane nothing has written, and WGSL zero-initialises
                // a local. Erroring instead was the single largest
                // refusal after phase 2 landed -- 107 bodies -- and
                // every one of them was reading a component of a
                // `var` it had only partly assigned.
                IVal::Scalar(
                    lanes.get(*index as usize).copied().unwrap_or(Interval::point(0.0)),
                )
            }
            E::Access { base, index } => {
                let b = self.expr(func, *base, frame)?;
                if let IVal::Opaque(WEIGHTS) = b {
                    // `transforms[xform].variations[variation]`: the
                    // weight of this variation on this transform.
                    return Ok(IVal::Scalar(Interval::point(self.weight)));
                }
                if let Some(v) = step_transform_chain(&b) {
                    return Ok(v);
                }
                if let IVal::Opaque(w) = b {
                    return Ok(IVal::Opaque(w));
                }
                let i = self.expr(func, *index, frame)?;
                let lanes = b.lanes()?;
                match i {
                    IVal::Int(k) => IVal::Scalar(
                        lanes.get(k as usize).copied().unwrap_or(Interval::point(0.0)),
                    ),
                    // A non-constant index: the union of every lane is
                    // sound and is all that can be said.
                    _ => {
                        let mut u = lanes[0];
                        for l in &lanes[1..] {
                            u = u.union(*l);
                        }
                        IVal::Scalar(u)
                    }
                }
            }
            E::Swizzle { pattern, vector, size } => {
                let v = self.expr(func, *vector, frame)?;
                if let IVal::Opaque(w) = v {
                    return Ok(IVal::Opaque(w));
                }
                let v = v.lanes()?;
                let mut out = Vec::new();
                for c in &pattern[..*size as usize] {
                    out.push(
                        *v.get(*c as usize)
                            .ok_or(Refusal::Shape("swizzle out of range"))?,
                    );
                }
                IVal::Vec(out)
            }
            E::Unary { op, expr } => {
                let v = self.expr(func, *expr, frame)?;
                match op {
                    naga::UnaryOperator::Negate => match v {
                        IVal::Scalar(i) => IVal::Scalar(i.neg()),
                        IVal::Int(k) => IVal::Int(-k),
                        IVal::Vec(l) => IVal::Vec(l.into_iter().map(|i| i.neg()).collect()),
                        IVal::Bool(_) => return Err(Refusal::Shape("negated a bool")),
                        IVal::Opaque(w) => IVal::Opaque(w),
                    },
                    naga::UnaryOperator::LogicalNot => match v {
                        IVal::Bool(b) => IVal::Bool(b.map(|x| !x)),
                        _ => return Err(Refusal::Shape("negated a non-bool")),
                    },
                    naga::UnaryOperator::BitwiseNot => match v {
                        IVal::Int(k) => IVal::Int(!k & 0xFFFF_FFFF),
                        _ => IVal::Scalar(INT32_RANGE),
                    },
                }
            }
            E::Binary { op, left, right } => {
                let a = self.expr(func, *left, frame)?;
                // **A value times ITSELF is a square, not a product
                // of two independent ranges.** Interval arithmetic
                // has no memory that both factors move together, so
                // `x * x` over `[-1, 1]` comes out `[-1, 1]` when it
                // is really `[0, 1]` -- and a denominator like
                // `x*x + y*y + 1e-6` then straddles zero and reports
                // a pole that does not exist. Recognising the
                // identical operand costs one handle comparison and
                // removes that whole class of false refusal.
                if matches!(op, naga::BinaryOperator::Multiply) && same_value(func, *left, *right) {
                    self.map_lanes(&a, |x| square(x))?
                } else {
                    let b = self.expr(func, *right, frame)?;
                    let v = lost(self.binary(*op, a, b))?;
                    // **An integer division truncates**, and the float
                    // quotient's range does not hold what it truncates to:
                    // `[1, 3] / 2` is `[0.5, 1.5]` and the shader's `1 / 2`
                    // is 0 (lazysensen's parity, `n - (n / 2) * 2`). So an
                    // integer-typed quotient's ends are truncated, a
                    // monotone map. Two exact integers divide exactly
                    // already (`binary`).
                    if matches!(op, naga::BinaryOperator::Divide) && self.is_integer(func, h)? {
                        let t = |i: Interval| Interval::new(i.lo.trunc(), i.hi.trunc());
                        match v {
                            IVal::Scalar(i) => IVal::Scalar(t(i)),
                            IVal::Vec(l) => IVal::Vec(l.into_iter().map(t).collect()),
                            other => other,
                        }
                    } else {
                        v
                    }
                }
            }
            E::Math { fun, arg, arg1, arg2, arg3 } => {
                // `dot(p, p)` -- the squared length, and by far the
                // commonest expression in the catalogue -- is the
                // same identical-operand case as `x * x`, one level
                // up. Without it every radial variation reports a
                // pole at the first division.
                if matches!(fun, naga::MathFunction::Dot) && arg1.as_ref() == Some(arg) {
                    let v = self.expr(func, *arg, frame)?;
                    if let IVal::Opaque(w) = v {
                        return Ok(IVal::Opaque(w));
                    }
                    let mut acc = Interval::point(0.0);
                    for l in v.lanes()? {
                        acc = acc.add(square(l));
                    }
                    return Ok(IVal::Scalar(acc));
                }
                // **A trig function of the formula language's range
                // reduction is of what it reduced.** `uf_reduce(t)` is `t`
                // less a whole number of turns, by Cody-Waite, which the
                // trig functions cannot tell from `t`; but its interval is
                // `t`'s less a RANGE of turns wherever `round(t / 2pi)` is
                // not one integer, and the trig of that lost what `t`
                // knew -- bMod's `cosh(tau) - cos(sigma)` reached zero, a
                // pole the built-in's plain `cos` never shows. Only as a
                // trig argument: the reduced value elsewhere is itself.
                let reduced = match fun {
                    naga::MathFunction::Sin | naga::MathFunction::Cos | naga::MathFunction::Tan => frame.reduced.get(&arg.index()).cloned(),
                    _ => None,
                };
                let first = match reduced {
                    Some(t) => t,
                    None => self.expr(func, *arg, frame)?,
                };
                let mut args = vec![first];
                for a in [arg1, arg2, arg3].into_iter().flatten() {
                    args.push(self.expr(func, *a, frame)?);
                }
                lost(self.math(*fun, &args))?
            }
            E::As { expr, kind, convert } => {
                let v = self.expr(func, *expr, frame)?;
                // A `bitcast` reads the bits: an integer's as the other
                // integer kind, a float's as a pattern.
                let from_integer = convert.is_none() && self.is_integer(func, *expr)?;
                cast(v, *kind, convert.is_some(), from_integer)?
            }
            E::Select { condition, accept, reject } => {
                let c = self.expr(func, *condition, frame)?;
                match as_bool(&c) {
                    Some(true) => self.expr(func, *accept, frame)?,
                    Some(false) => self.expr(func, *reject, frame)?,
                    None => {
                        let a = self.expr(func, *accept, frame)?;
                        let b = self.expr(func, *reject, frame)?;
                        join(&a, &b)?
                    }
                }
            }
            // The `Call` statement ran first and cached the value
            // under this handle; reaching here means it did not.
            E::CallResult(_) => return Err(Refusal::Shape("a call result with no call")),
            E::GlobalVariable(g) => {
                let gname = self.module.global_variables[*g].name.as_deref().unwrap_or("");
                if gname == "transforms" {
                    IVal::Opaque(TRANSFORMS)
                } else {
                    return Err(Refusal::UnknownCall(format!("global `{gname}`")));
                }
            }
            // `any`/`all` over a vector comparison. Deciding these
            // would need the componentwise predicate; undecided is
            // sound and both arms get taken.
            E::Relational { .. } => IVal::Bool(None),
            _ => return Err(Refusal::Construct("an unmodelled expression")),
        })
    }

    /// A call: the intercepted names, then anything else by
    /// evaluating the callee's own IR. Helpers (`cmul`, `csqrt`,
    /// `ff_atan2`, …) are in the same module, so they just work.
    fn dispatch(
        &mut self,
        f: naga::Handle<naga::Function>,
        args: &[IVal],
    ) -> Result<IVal, Refusal> {
        let callee = &self.module.functions[f];
        let cname = callee.name.as_deref().unwrap_or("");
        match cname {
            "get_param" => {
                // (xform_id, variation_id, slot). Only the slot is
                // read, and it has to be exact.
                let IVal::Int(slot) = args.get(2).ok_or(Refusal::Shape("get_param arity"))?
                else {
                    return Err(Refusal::Shape("a non-constant parameter slot"));
                };
                self.param_slot(*slot as usize)
            }
            "rng_nextf" => Ok(IVal::Scalar(self.rng)),
            // The formula language's `pow` (`uf_powf`), by its own rule:
            // through its IR, a base the evaluator could not place on one
            // side of zero took the negative-base arm, `pow(-a, b)`, with
            // `-a` mostly negative, as no path through the real helper
            // does (synth).
            n if n.ends_with("_powf") && args.len() == 2 => match (&args[0], &args[1]) {
                (IVal::Opaque(w), _) | (_, IVal::Opaque(w)) => Ok(IVal::Opaque(w)),
                (a, b) => lost(powf_iv(a.scalar()?, b.scalar()?).map(IVal::Scalar)),
            },
            "rng_next" => Ok(IVal::Opaque("a raw RNG word")),
            // **Per-thread state is opaque, not a refusal.** What a
            // previous iteration stored can be anything, so a read is a
            // value this cannot name, refused where it reaches the point
            // (`found`, as the state it was); a write changes only what
            // a later read gives, opaque already. A body whose state
            // feeds only its colour (littlewood's colour register, read
            // at the top of its twin) bounds as one without.
            "get_state" => Ok(IVal::Opaque(STATE)),
            "set_state" => Ok(IVal::Opaque(STATE)),
            _ => self.call(callee, args),
        }
    }

    /// The value at a packed parameter slot.
    ///
    /// Slots below `parameters.len()` are the user's, by name. Above
    /// that they are INIT-DERIVED: computed once per flame by the
    /// variation's own `wgsl_init`, which the probe module does not
    /// carry because the init pass is a separate shader. So that
    /// function is parsed on its own and evaluated here, with the
    /// user parameters as exact intervals — `julian`'s `cpower` is
    /// the worked example, and 122 bodies read such a slot, which is
    /// too many to refuse.
    fn param_slot(&mut self, slot: usize) -> Result<IVal, Refusal> {
        let reg = crate::variations::global_registry();
        let info = reg.get(self.name).ok_or(Refusal::NoBody)?;
        if let Some(p) = info.parameters.get(slot) {
            return Ok(IVal::Scalar(Interval::point((self.params)(&p.name))));
        }
        let derived = slot - info.parameters.len();
        let user: Vec<Interval> = info
            .parameters
            .iter()
            .map(|p| Interval::point((self.params)(&p.name)))
            .collect();
        let _ = info;
        drop(reg);

        let (module, handle) = init_module(self.name).ok_or(Refusal::InitSlot)?;
        let mut ev = Eval {
            module,
            params: self.params,
            weight: self.weight,
            name: self.name,
            rng: self.rng,
            types: Default::default(),
        };
        let out = ev.call(&module.functions[handle], &[IVal::Vec(user)])?;
        let lanes = out.lanes()?;
        lanes
            .get(derived)
            .map(|i| IVal::Scalar(*i))
            .ok_or(Refusal::InitSlot)
    }

    /// Apply a scalar rule to every lane, keeping the shape.
    fn map_lanes(
        &self,
        v: &IVal,
        f: impl Fn(Interval) -> Interval,
    ) -> Result<IVal, Refusal> {
        Ok(match v {
            IVal::Scalar(x) => IVal::Scalar(f(*x)),
            IVal::Int(k) => IVal::Scalar(f(Interval::point(*k as f64))),
            IVal::Vec(l) => IVal::Vec(l.iter().map(|x| f(*x)).collect()),
            IVal::Opaque(w) => IVal::Opaque(w),
            IVal::Bool(_) => return Err(Refusal::Shape("arithmetic on a bool")),
        })
    }

    fn binary(
        &self,
        op: naga::BinaryOperator,
        a: IVal,
        b: IVal,
    ) -> Result<IVal, Refusal> {
        use naga::BinaryOperator as B;
        // A comparison between two ranges answers yes, no, or
        // MAYBE, and deciding it where the ranges do not overlap is
        // most of what keeps a branch from doubling the result:
        // `if (p.y < 0.0)` over a disc entirely below the axis takes
        // one arm, not the union of both.
        if matches!(op, B::LogicalAnd | B::LogicalOr) {
            let (x, y) = (as_bool(&a), as_bool(&b));
            return Ok(IVal::Bool(match op {
                B::LogicalAnd => match (x, y) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                },
                _ => match (x, y) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                },
            }));
        }
        if matches!(
            op,
            B::Equal | B::NotEqual | B::Less | B::LessEqual | B::Greater | B::GreaterEqual
        ) {
            let (x, y) = match (a.scalar(), b.scalar()) {
                (Ok(x), Ok(y)) => (x, y),
                // Comparing something the evaluator carries without a
                // value: undecided, and both arms will be taken.
                _ => return Ok(IVal::Bool(None)),
            };
            return Ok(IVal::Bool(match op {
                B::Less => cmp_lt(x, y),
                B::LessEqual => cmp_le(x, y),
                B::Greater => cmp_lt(y, x),
                B::GreaterEqual => cmp_le(y, x),
                B::Equal => {
                    if x.lo == x.hi && y.lo == y.hi && x.lo == y.lo {
                        Some(true)
                    } else if x.hi < y.lo || y.hi < x.lo {
                        Some(false)
                    } else {
                        None
                    }
                }
                _ => {
                    if x.hi < y.lo || y.hi < x.lo {
                        Some(true)
                    } else if x.lo == x.hi && y.lo == y.hi && x.lo == y.lo {
                        Some(false)
                    } else {
                        None
                    }
                }
            }));
        }
        // **Arithmetic with an opaque is opaque, not a refusal.** A value
        // the evaluator carries without a number (a direct colour, the
        // RNG's raw word, an index) may feed a colour, a condition or an
        // index without ever touching the point, and refusing at the
        // first `+` lost every `glsl_*` twin, whose bodies load their
        // colour into locals at the top. Where it can matter, it still
        // refuses: at the returned point, in a parameter slot, in a
        // `pow` or a division that needs a number (`lanes`, `scalar`);
        // a condition on one is undecided, both arms taken.
        if let IVal::Opaque(w) = a {
            return Ok(IVal::Opaque(w));
        }
        if let IVal::Opaque(w) = b {
            return Ok(IVal::Opaque(w));
        }
        if let (IVal::Int(x), IVal::Int(y)) = (&a, &b) {
            // Exact integer arithmetic, so a slot index stays a slot
            // index through `2u * n + 1u`.
            // Exact while it cannot wrap. WGSL's integers wrap at 32
            // bits, so a product that leaves the range is a value
            // this cannot name -- and naming it anyway is how a slot
            // index or a loop bound would come out wrong.
            let exact = |v: i64| -> IVal {
                if (-2147483648..=4294967295).contains(&v) {
                    IVal::Int(v)
                } else {
                    IVal::Scalar(INT32_RANGE)
                }
            };
            return Ok(match op {
                B::Add => exact(x + y),
                B::Subtract => exact(x - y),
                B::Multiply => exact(x.saturating_mul(*y)),
                B::Divide if *y != 0 => IVal::Int(x / y),
                B::Modulo if *y != 0 => IVal::Int(x % y),
                B::And => IVal::Int(x & y),
                B::InclusiveOr => IVal::Int(x | y),
                B::ExclusiveOr => IVal::Int(x ^ y),
                B::ShiftLeft => exact(x.checked_shl(*y as u32).unwrap_or(i64::MAX)),
                B::ShiftRight => IVal::Int(x >> y.clamp(&0, &63)),
                _ => return Err(Refusal::Intrinsic("an integer operator".into())),
            });
        }
        let f = |x: Interval, y: Interval| -> Result<Interval, Refusal> {
            Ok(match op {
                B::Add => x.add(y),
                B::Subtract => x.sub(y),
                B::Multiply => x.mul(y),
                B::Divide => {
                    let q = x.div(y);
                    if !q.finite() {
                        return Err(Refusal::Pole);
                    }
                    q
                }
                B::Modulo => {
                    // `x % y` lands in [-|y|, |y|]; sound and loose.
                    let m = y.abs().hi;
                    Interval::new(-m, m)
                }
                // A bitwise operation on something this cannot pin
                // down: the result is some 32-bit pattern, and the
                // range of those is all that can honestly be said.
                B::And | B::InclusiveOr | B::ExclusiveOr | B::ShiftLeft | B::ShiftRight => {
                    INT32_RANGE
                }
                _ => return Err(Refusal::Intrinsic("an operator".into())),
            })
        };
        Ok(match (a, b) {
            (IVal::Vec(x), IVal::Vec(y)) => {
                let n = x.len().min(y.len());
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    out.push(f(x[i], y[i])?);
                }
                IVal::Vec(out)
            }
            (IVal::Vec(x), other) => {
                let s = other.scalar()?;
                IVal::Vec(x.into_iter().map(|i| f(i, s)).collect::<Result<_, _>>()?)
            }
            (other, IVal::Vec(y)) => {
                let s = other.scalar()?;
                IVal::Vec(y.into_iter().map(|i| f(s, i)).collect::<Result<_, _>>()?)
            }
            (x, y) => IVal::Scalar(f(x.scalar()?, y.scalar()?)?),
        })
    }

    fn math(&self, fun: naga::MathFunction, args: &[IVal]) -> Result<IVal, Refusal> {
        use naga::MathFunction as M;
        if let Some(IVal::Opaque(w)) = args.iter().find(|a| matches!(a, IVal::Opaque(_))) {
            return Ok(IVal::Opaque(w));
        }
        let lanes0 = args[0].lanes()?;
        let n = lanes0.len();

        // Reductions first: they take a vector and give a scalar.
        match fun {
            M::Dot => {
                let b = args[1].lanes()?;
                let mut acc = Interval::point(0.0);
                for i in 0..n.min(b.len()) {
                    acc = acc.add(lanes0[i].mul(b[i]));
                }
                return Ok(IVal::Scalar(acc));
            }
            M::Length => {
                let mut acc = Interval::point(0.0);
                for l in &lanes0 {
                    acc = acc.add(l.mul(*l));
                }
                return Ok(IVal::Scalar(sqrt_iv(acc)?));
            }
            M::Distance => {
                let b = args[1].lanes()?;
                let mut acc = Interval::point(0.0);
                for i in 0..n.min(b.len()) {
                    let d = lanes0[i].sub(b[i]);
                    acc = acc.add(d.mul(d));
                }
                return Ok(IVal::Scalar(sqrt_iv(acc)?));
            }
            _ => {}
        }

        // Everything else is componentwise.
        let get = |k: usize, i: usize| -> Result<Interval, Refusal> {
            let l = args[k].lanes()?;
            Ok(if l.len() == 1 { l[0] } else { *l.get(i).unwrap_or(&l[0]) })
        };
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let x = lanes0[i];
            let v = match fun {
                M::Abs => x.abs(),
                M::Min => Interval::new(x.lo.min(get(1, i)?.lo), x.hi.min(get(1, i)?.hi)),
                M::Max => Interval::new(x.lo.max(get(1, i)?.lo), x.hi.max(get(1, i)?.hi)),
                M::Clamp => {
                    let (lo, hi) = (get(1, i)?, get(2, i)?);
                    Interval::new(x.lo.max(lo.lo).min(hi.hi), x.hi.max(lo.lo).min(hi.hi))
                }
                M::Saturate => Interval::new(x.lo.clamp(0.0, 1.0), x.hi.clamp(0.0, 1.0)),
                M::Sin => x.trig(f64::sin, std::f64::consts::FRAC_PI_2),
                M::Cos => x.trig(f64::cos, 0.0),
                M::Sqrt => sqrt_iv(x)?,
                M::Exp => x.monotone(f64::exp),
                M::Exp2 => x.monotone(f64::exp2),
                M::Log => {
                    if x.lo <= 0.0 {
                        return Err(Refusal::Domain("log"));
                    }
                    x.monotone(f64::ln)
                }
                M::Log2 => {
                    if x.lo <= 0.0 {
                        return Err(Refusal::Domain("log2"));
                    }
                    x.monotone(f64::log2)
                }
                // Exact, so not widened: each is monotone and lands on an
                // integer f32 holds exactly, so the ends of the range
                // bound it. Widened, `round(1.0)` straddled 1 and every
                // `power != 1.0` the formula language's `to_float` feeds
                // was undecided (murl's prepare divided by `power - 1`).
                // WGSL's `round` is half-to-even, Rust's half-away from
                // zero: the hull of both is sound.
                M::Floor => x.monotone_exact(f64::floor),
                M::Ceil => x.monotone_exact(f64::ceil),
                M::Round => x.monotone_exact(f64::round).union(x.monotone_exact(f64::round_ties_even)),
                M::Trunc => x.monotone_exact(f64::trunc),
                M::Fract => {
                    // `x - floor(x)` is in [0, 1) always, and tracking
                    // it more tightly needs the same period reasoning
                    // as trig for no gain here.
                    Interval::new(0.0, 1.0)
                }
                M::Sign => Interval::new(
                    if x.lo > 0.0 { 1.0 } else if x.lo < 0.0 { -1.0 } else { 0.0 },
                    if x.hi > 0.0 { 1.0 } else if x.hi < 0.0 { -1.0 } else { 0.0 },
                ),
                M::Sinh => x.monotone(f64::sinh),
                M::Tanh => x.monotone(f64::tanh),
                // Increasing on its whole domain, which starts at 1.
                M::Acosh => {
                    if x.lo < 1.0 {
                        return Err(Refusal::Domain("acosh"));
                    }
                    x.monotone(f64::acosh)
                }
                M::Asinh => x.monotone(f64::asinh),
                M::Atanh => {
                    if x.lo <= -1.0 || x.hi >= 1.0 {
                        return Err(Refusal::Domain("atanh"));
                    }
                    x.monotone(f64::atanh)
                }
                M::Cosh => {
                    // Even, with its minimum at zero.
                    let a = x.abs();
                    Interval::new(
                        if x.contains(0.0) { 1.0 } else { a.lo.cosh() },
                        a.hi.cosh(),
                    )
                    .widen()
                }
                M::Atan => x.monotone(f64::atan),
                M::Asin => {
                    if x.lo < -1.0 || x.hi > 1.0 {
                        return Err(Refusal::Domain("asin"));
                    }
                    x.monotone(f64::asin)
                }
                M::Acos => {
                    if x.lo < -1.0 || x.hi > 1.0 {
                        return Err(Refusal::Domain("acos"));
                    }
                    x.monotone(f64::acos)
                }
                M::Atan2 => atan2_iv(x, get(1, i)?),
                M::Pow => {
                    let e = get(1, i)?;
                    pow_iv(x, e)?
                }
                M::Mix => {
                    let (b, t) = (get(1, i)?, get(2, i)?);
                    // x + (b - x) * t
                    x.add(b.sub(x).mul(t))
                }
                M::Step => {
                    let _ = get(1, i)?;
                    Interval::new(0.0, 1.0)
                }
                M::SmoothStep => Interval::new(0.0, 1.0),
                M::Fma => x.mul(get(1, i)?).add(get(2, i)?),
                M::Radians => x.mul(Interval::point(std::f64::consts::PI / 180.0)),
                M::Degrees => x.mul(Interval::point(180.0 / std::f64::consts::PI)),
                M::Normalize => {
                    // Componentwise here would be wrong; a normalized
                    // vector's every lane is in [-1, 1] and that is
                    // sound.
                    Interval::new(-1.0, 1.0)
                }
                M::Tan => {
                    // tan has a pole every π. Only a span that stays
                    // strictly inside one branch is bounded.
                    let pi = std::f64::consts::PI;
                    let k = (x.lo / pi + 0.5).floor();
                    let (lo_b, hi_b) = ((k - 0.5) * pi, (k + 0.5) * pi);
                    if !x.finite() || x.lo <= lo_b || x.hi >= hi_b {
                        return Err(Refusal::Pole);
                    }
                    x.monotone(f64::tan)
                }
                _ => return Err(Refusal::Intrinsic(format!("{fun:?}"))),
            };
            out.push(v);
        }
        Ok(if n == 1 && matches!(args[0], IVal::Scalar(_) | IVal::Int(_)) {
            IVal::Scalar(out[0])
        } else {
            IVal::Vec(out)
        })
    }
}

/// How a block ended.
///
/// `Return` carries no value: a returned value goes into
/// [`Frame::returns`] instead, because a return inside a MAYBE branch
/// is only one of the things that might have happened and the
/// function's answer is the union of all of them.
enum Flow {
    Return,
    Break,
    Continue,
    Fell,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params_of(name: &str) -> impl Fn(&str) -> f64 + '_ {
        move |p: &str| {
            let reg = crate::variations::global_registry();
            reg.get(name)
                .and_then(|i| {
                    i.parameters.iter().find(|q| q.name == p).map(|q| q.slot_default() as f64)
                })
                .unwrap_or(0.0)
        }
    }

    /// **Which refusals subdivision actually removes.**
    ///
    /// Interval arithmetic's error grows with the width of the input,
    /// so a refusal at `k = 1` may be the body's real behaviour or
    /// may be the dependency problem — `x − x` is `[−2r, 2r]` and a
    /// denominator built that way straddles zero when the true value
    /// never does. The two look identical from inside. Splitting the
    /// input tells them apart: a refusal that survives a finer grid
    /// is the map, and one that disappears was the arithmetic.
    ///
    /// This decides how much subdivision the enumeration should buy,
    /// and it is the phase-3 measurement the plan asks for first,
    /// because a spurious pole is a flame refused for nothing.
    #[test]
    #[ignore = "a census, not a gate"]
    fn what_subdivision_recovers() {
        use std::collections::BTreeMap;
        let reg = crate::variations::global_registry();
        let names: Vec<String> = reg.ordered_names.clone();
        drop(reg);

        // A disc where a radial body's denominator is genuinely at
        // risk (it reaches the origin) and one where it is not.
        for ball in [Ball::new([0.0, 0.0], 0.4), Ball::new([0.6, -0.4], 0.25)] {
            let mut rows: BTreeMap<String, [usize; 3]> = BTreeMap::new();
            let mut ok = [0usize; 3];
            for (col, k) in [1usize, 3, 8].into_iter().enumerate() {
                for name in &names {
                    let pf = params_of(name);
                    match derive_subdivided(name, &pf, 1.0, ball, k) {
                        Ok(_) => ok[col] += 1,
                        Err(e) => {
                            let key = match &e {
                                Refusal::Intrinsic(_) => "no interval rule".to_string(),
                                other => format!("{other}"),
                            };
                            rows.entry(key).or_insert([0; 3])[col] += 1;
                        }
                    }
                }
            }
            println!();
            println!("  disc c{:?} r{}:", ball.c, ball.r);
            println!("    derive   k=1 {:>4}   k=3 {:>4}   k=8 {:>4}", ok[0], ok[1], ok[2]);
            let mut v: Vec<_> = rows.into_iter().collect();
            v.sort_by_key(|(k, n)| (std::cmp::Reverse(n[0]), k.clone()));
            for (k, n) in v.iter().take(8) {
                println!("    {:<44} {:>4} {:>4} {:>4}", k, n[0], n[1], n[2]);
            }
        }
    }

    /// Phase 1's deliverable: of 647 bodies, how many derive, and what
    /// stops the rest.
    ///
    /// The refusal breakdown IS the output. It says what phase 2 is
    /// worth (branches and loops), which intrinsics to add next, and
    /// which bodies are refused for good rather than for now.
    #[test]
    #[ignore = "a census, not a gate"]
    fn what_derives_today() {
        use std::collections::BTreeMap;
        let reg = crate::variations::global_registry();
        let ball = Ball::new([0.6, -0.4], 0.25);
        let mut ok = 0usize;
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        let mut intrinsics: BTreeMap<String, usize> = BTreeMap::new();
        let mut total = 0usize;

        let mut names: Vec<String> = reg.ordered_names.clone();
        names.sort();
        for name in &names {
            if reg.get(name).is_none() {
                continue;
            }
            total += 1;
            let pf = params_of(name);
            match derive(name, &pf, 1.0, ball) {
                Ok(_) => ok += 1,
                Err(e) => {
                    let key = match &e {
                        Refusal::Intrinsic(n) => {
                            *intrinsics.entry(n.clone()).or_default() += 1;
                            "no interval rule for an intrinsic".to_string()
                        }
                        other => format!("{other}"),
                    };
                    *kinds.entry(key).or_default() += 1;
                }
            }
        }

        println!();
        println!("  {ok} of {total} variation bodies derive a bound today.");
        println!();
        println!("  refusals:");
        let mut rows: Vec<_> = kinds.iter().collect();
        rows.sort_by_key(|(k, n)| (std::cmp::Reverse(**n), (*k).clone()));
        for (k, n) in rows {
            println!("    {n:>4}  {k}");
        }
        if !intrinsics.is_empty() {
            println!();
            println!("  intrinsics with no rule, by bodies blocked:");
            let mut ir: Vec<_> = intrinsics.iter().collect();
            ir.sort_by_key(|(k, n)| (std::cmp::Reverse(**n), (*k).clone()));
            for (k, n) in ir.iter().take(20) {
                println!("    {n:>4}  {k}");
            }
        }
    }

    /// **What the twins derive** (rhai-one-form step 0): each built-in's
    /// bound from its own WGSL against its Rhai twin's, from the twin's
    /// compiled WGSL, at one disc -- how many of each derive, how long a
    /// derivation takes, and where both do, how much wider the twin's
    /// disc is. The twin's numbers are the baseline an interval evaluator
    /// over the Rhai form has to meet. Registers every twin: run alone.
    #[test]
    #[ignore = "a census; registers every twin"]
    fn what_twins_derive() {
        let ball = Ball::new([0.6, -0.4], 0.25);
        let (mut both, mut builtin_only, mut twin_only, mut neither, mut unregistered) = (0usize, Vec::new(), Vec::new(), 0usize, 0usize);
        let (mut t_builtin, mut t_twin, mut n_builtin, mut n_twin) = (0.0f64, 0.0f64, 0usize, 0usize);
        let mut ratios: Vec<(f64, &str)> = Vec::new();
        // The twin's own body, not its built-in's (`twins::planner_name`).
        crate::variations::twins::BORROW.with(|b| b.set(false));
        for (name, src) in crate::variations::twins::TWINS.iter() {
            if crate::variations::global_registry().get(name).is_none() {
                continue;
            }
            let pb = params_of(name);
            let t0 = std::time::Instant::now();
            let b = derive(name, &pb, 1.0, ball);
            t_builtin += t0.elapsed().as_secs_f64();
            n_builtin += 1;
            let Ok(key) = crate::variations::custom::register(src) else {
                unregistered += 1;
                continue;
            };
            let pt = params_of(&key);
            let t0 = std::time::Instant::now();
            let t = derive(&key, &pt, 1.0, ball);
            t_twin += t0.elapsed().as_secs_f64();
            n_twin += 1;
            match (b, t) {
                (Ok(b), Ok(t)) => {
                    both += 1;
                    let dc = ((t.c[0] - b.c[0]).powi(2) + (t.c[1] - b.c[1]).powi(2)).sqrt();
                    ratios.push(((t.r + dc) / b.r.max(1e-30), *name));
                }
                (Ok(_), Err(e)) => builtin_only.push(format!("{name} ({e})")),
                (Err(_), Ok(_)) => twin_only.push(name.to_string()),
                (Err(_), Err(_)) => neither += 1,
            }
        }
        ratios.sort_by(|a, b| a.0.total_cmp(&b.0));
        let q = |f: f64| ratios.get(((ratios.len() as f64 - 1.0) * f) as usize).map_or(f64::NAN, |r| r.0);
        println!();
        println!("  {n_builtin} built-ins with a twin; {unregistered} twins did not register");
        println!("  both derive {both}; the built-in alone {}; the twin alone {}; neither {neither}", builtin_only.len(), twin_only.len());
        println!("  a derivation: built-in {:.2} ms, twin {:.2} ms (first calls parse)", t_builtin * 1e3 / n_builtin as f64, t_twin * 1e3 / n_twin.max(1) as f64);
        println!("  the twin's disc over the built-in's, where both derive: median {:.3}, 90% {:.3}, 99% {:.3}, worst {:.3}", q(0.5), q(0.9), q(0.99), q(1.0));
        let tight = ratios.iter().filter(|r| r.0 <= 1.0 + 1e-9).count();
        println!("  {tight} of {both} as tight or tighter; the widest: {:?}", ratios.iter().rev().take(10).collect::<Vec<_>>());
        println!("  the built-in alone: {}", builtin_only.join(", "));
        println!("  the twin alone: {}", twin_only.join(", "));
        crate::variations::twins::BORROW.with(|b| b.set(true));
    }

    /// The `transforms[i].variations[j]` shortcut rests on a
    /// measurement: that `variations` is the only field of that global
    /// any body reads. If a body ever reads another, the chain would
    /// hand it the WEIGHT — a wrong number, silently — so the
    /// measurement is pinned here rather than left in a commit message.
    #[test]
    fn the_weight_chain_is_the_only_one() {
        let mut others: Vec<String> = Vec::new();
        for def in crate::variations::defs::ALL_VARIATIONS {
            let mut rest = def.wgsl_2d;
            while let Some(i) = rest.find("transforms[") {
                rest = &rest[i + "transforms[".len()..];
                let Some(close) = rest.find(']') else { break };
                let after = &rest[close + 1..];
                if let Some(dot) = after.strip_prefix('.') {
                    let field: String = dot
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if field != "variations" {
                        others.push(format!("{}: .{field}", def.name));
                    }
                }
            }
        }
        assert!(
            others.is_empty(),
            "a body reads a field of `transforms` other than `variations`, so the access \
             chain in `derive` would hand it the weight instead: {others:?}"
        );
    }

    /// An interval really does contain what the function does to every
    /// point inside it. Cheap, no GPU, and the thing that catches a
    /// transcribed rule.
    #[test]
    fn the_rules_contain_their_functions() {
        let cases: Vec<(&str, fn(f64) -> f64, fn(Interval) -> Interval)> = vec![
            ("sin", f64::sin, |i| i.trig(f64::sin, std::f64::consts::FRAC_PI_2)),
            ("cos", f64::cos, |i| i.trig(f64::cos, 0.0)),
            ("abs", f64::abs, |i| i.abs()),
            ("exp", f64::exp, |i| i.monotone(f64::exp)),
            ("tanh", f64::tanh, |i| i.monotone(f64::tanh)),
        ];
        for (name, f, rule) in cases {
            for (lo, hi) in [
                (-0.5, 0.5),
                (0.0, 0.1),
                (-3.0, 3.0),
                (1.0, 1.2),
                (-10.0, 10.0),
                (2.5, 2.5),
            ] {
                let iv = Interval::new(lo, hi);
                let out = rule(iv);
                for k in 0..=400 {
                    let t = lo + (hi - lo) * (k as f64 / 400.0);
                    let v = f(t);
                    assert!(
                        out.contains(v),
                        "{name}: f({t}) = {v} escapes {out:?} for input {iv:?}"
                    );
                }
            }
        }
    }

    /// Division by a range spanning zero is a pole, not a number.
    #[test]
    fn a_zero_crossing_denominator_is_a_pole() {
        let a = Interval::new(1.0, 2.0);
        let z = Interval::new(-1.0, 1.0);
        assert_eq!(a.div(z), Interval::UNBOUNDED);
        // ...and away from zero it is ordinary.
        let q = a.div(Interval::new(2.0, 4.0));
        assert!(q.lo <= 0.25 && q.hi >= 1.0, "{q:?}");
    }

    /// **An integer division truncates, as the shader's does.**
    /// `lazysensen` flips a coordinate by the parity of `floor(c * s)`,
    /// `n - (n / 2) * 2`: read as float division the parity of 1 was 0
    /// and its bound kept the point where the shader flipped it, which
    /// the soundness gate caught once `floor` stopped widening.
    #[test]
    fn an_integer_division_truncates() {
        let reg = crate::variations::global_registry();
        let info = reg.get("lazysensen").expect("lazysensen");
        let defaults: std::collections::HashMap<String, f64> =
            info.parameters.iter().map(|p| (p.name.to_string(), f64::from(p.slot_default()))).collect();
        drop(reg);
        let pf = |q: &str| defaults.get(q).copied().unwrap_or(0.0);
        let ball = Ball::new([1.5, 0.0], 0.05);
        let disc = derive("lazysensen", &pf, 1.0, ball).expect("lazysensen derives");
        let flipped = [-1.5, 0.0];
        let d = ((flipped[0] - disc.c[0]).powi(2) + (flipped[1] - disc.c[1]).powi(2)).sqrt();
        assert!(d <= disc.r, "the flipped point {flipped:?} is outside {disc:?}");
    }

    /// The integer casts: an exact value to an exact integer, truncated
    /// and saturated; a `bitcast` between the integer kinds keeps a
    /// value in `[0, 2^31)` and rereads the rest; of a float, any bits.
    #[test]
    fn casts_follow_wgsl() {
        use naga::ScalarKind as K;
        let int = |v: IVal| match v { IVal::Int(k) => k, other => panic!("{other:?}") };
        assert_eq!(int(cast(IVal::Scalar(Interval::point(-2.7)), K::Sint, true, false).unwrap()), -2);
        assert_eq!(int(cast(IVal::Scalar(Interval::point(-2.7)), K::Uint, true, false).unwrap()), 0);
        assert_eq!(int(cast(IVal::Scalar(Interval::point(3.0)), K::Sint, false, true).unwrap()), 3);
        assert_eq!(int(cast(IVal::Int(-1), K::Uint, false, true).unwrap()), 4294967295);
        assert_eq!(int(cast(IVal::Int(4294967295), K::Sint, false, true).unwrap()), -1);
        let IVal::Scalar(bits) = cast(IVal::Scalar(Interval::point(1.0)), K::Uint, false, false).unwrap() else { panic!() };
        assert_eq!(bits, INT32_RANGE);
        let IVal::Scalar(f) = cast(IVal::Int(7), K::Float, true, false).unwrap() else { panic!() };
        assert_eq!(f, Interval::point(7.0));
    }

    /// The formula language's `pow` over a base reaching below zero: the
    /// negative part to each whole exponent in range, signed.
    #[test]
    fn the_language_pow_holds_its_negative_arm() {
        let r = powf_iv(Interval::new(-0.5, 1.0), Interval::new(0.999, 1.001)).unwrap();
        assert!(r.lo <= -0.5 && r.hi >= 1.0, "{r:?}");
        let r = powf_iv(Interval::new(-2.0, -1.0), Interval::point(2.0)).unwrap();
        assert!(r.lo <= 1.0 && r.hi >= 4.0, "{r:?}");
        assert!(powf_iv(Interval::new(-2.0, -1.0), Interval::point(0.5)).is_err(), "a fractional power of a negative base is NaN");
    }
}

#[cfg(test)]
mod branch_tests {
    use super::*;

    /// **Does pinning the arm make a julia bound shrink?**
    ///
    /// `grand-julian` is refused today with "`julian` has no forward
    /// bound", and §2 of `docs/projects/deep-zoom/inversive-targeting.md` says
    /// why: the body picks one of `|power|` arms with a random draw,
    /// so a bound that covers every draw covers a whole annulus about
    /// the origin — `O(1)` however small the input. A word through it
    /// cannot shrink.
    ///
    /// This is the measurement that decides whether family J is worth
    /// building: with the arm pinned, does the same body bound a small
    /// disc by a small disc?
    #[test]
    fn pinning_the_arm_makes_julian_shrink() {
        let reg = crate::variations::global_registry();
        let info = reg.get("julian").expect("julian");
        // `grand-julian`'s first transform: power 2, dist -1.
        let pf = |q: &str| -> f64 {
            match q {
                "power" => 2.0,
                "dist" => -1.0,
                _ => info
                    .parameters
                    .iter()
                    .find(|p| p.name == q)
                    .map_or(0.0, |p| p.slot_default() as f64),
            }
        };
        println!("  julian, power 2, dist -1 -- image radius for a disc at distance 1");
        println!("     input r     free bound     arm 0        arm 1");
        let mut any_shrunk = false;
        for r in [3e-1f64, 1e-1, 3e-2, 1e-2, 3e-3, 1e-3] {
            let ball = Ball::new([1.0, 0.3], r);
            let free = derive("julian", &pf, 1.0, ball)
                .map(|b| format!("{:.3e}", b.r))
                .unwrap_or_else(|e| format!("{e}"));
            let mut arms = Vec::new();
            for k in 0..2u32 {
                arms.push(
                    derive_branch("julian", &pf, 1.0, ball, (k, 2))
                        .map(|b| {
                            if b.r < r {
                                any_shrunk = true;
                            }
                            format!("{:.3e}", b.r)
                        })
                        .unwrap_or_else(|e| format!("{e}")),
                );
            }
            println!("     {r:<10.0e} {free:>12}  {:>10}  {:>10}", arms[0], arms[1]);
        }
        assert!(
            any_shrunk,
            "pinning the arm never produced an image smaller than its input -- \
             family J would not terminate either"
        );
    }

    /// The pinned arms together cover the free bound: pinning is a
    /// refinement, not a different answer.
    #[test]
    fn the_arms_together_cover_the_free_bound() {
        let pf = |q: &str| -> f64 {
            match q {
                "power" => 5.0,
                "dist" => 1.0,
                _ => 0.0,
            }
        };
        let ball = Ball::new([0.7, -0.4], 0.05);
        let free = derive("julian", &pf, 1.0, ball).expect("a free bound");
        let mut worst = 0.0f64;
        for k in 0..5u32 {
            let arm = derive_branch("julian", &pf, 1.0, ball, (k, 5)).expect("an arm");
            // Every point of the arm's disc must be inside the free
            // one, which covers all arms at once.
            let d = ((arm.c[0] - free.c[0]).powi(2) + (arm.c[1] - free.c[1]).powi(2)).sqrt();
            worst = worst.max(d + arm.r - free.r);
        }
        println!("  free radius {:.4e}, worst arm overhang {worst:.3e}", free.r);
        assert!(
            worst <= free.r * 1e-6 + 1e-9,
            "an arm escaped the bound that is supposed to cover every arm, by {worst:.3e}"
        );
    }
}
