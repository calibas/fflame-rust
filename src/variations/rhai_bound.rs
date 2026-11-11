//! **A forward bound from a Rhai variation's own code**
//! (docs/projects/rhai-one-form.md, route B).
//!
//! [`derive`](super::derive) bounds a variation by interval arithmetic
//! over its WGSL, parsed by naga, which the web build does not carry. This
//! takes the same rules ([`interval`](super::interval)) over the Rhai
//! variation's own typed tree (`lang::ir`), which every build has: a Rhai
//! variation's bound on every platform, and on the web each built-in's
//! through its twin, which computes the built-in's function (the twins'
//! parity tests).
//!
//! The rules are `derive`'s:
//!
//! - every value is the closed range it can take over the input box, each
//!   operation's result widened for the GPU's f32 rounding;
//! - a draw (`rand()`) is `[0, 1)`, or a pinned arm's slice of it;
//! - a condition is yes, no or **maybe**; maybe runs both arms from the
//!   same state and joins what they leave, as does a return inside one;
//! - a loop runs until its exit is certain, and refuses if it is not by
//!   the cap;
//! - a pole or a domain exit is a value with no bound ([`V::Bad`]), not
//!   yet a refusal: it refuses only where it reaches the returned point,
//!   so one feeding a colour leaves the bound sound;
//! - per-thread state (`init`'s) refuses where it is read before it is
//!   written in the call.
//!
//! `prepare` runs through the same rules at the flame's parameters (the
//! GPU computes it in f32 too); where they refuse it, its values are the
//! interpreter's numbers, widened.

use super::bound::Ball;
use super::interval::{atan2_iv, box_to_disc, cmp_le, cmp_lt, pow_iv, powf_iv, rng_for, sqrt_iv, square, Branch, Interval, Refusal};
use super::inverse::ParamFn;
use crate::lang::ast::{BinOp, UnOp};
use crate::lang::builtins::{Builtin, Host};
use crate::lang::interp::{ParamValues, Value};
use crate::lang::ir::*;
use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

/// Every value an `i32` holds, and a `u32`.
const I32: Interval = Interval { lo: i32::MIN as f64, hi: i32::MAX as f64 };
const U32: Interval = Interval { lo: 0.0, hi: u32::MAX as f64 };

/// How many times a loop runs before its exit must be certain.
const LOOP_CAP: usize = 512;

/// The relative margin a `prepare` value is widened by: the GPU computes
/// it in f32, through a few operations, where the interpreter's is f64.
/// 64 f32 ulps.
const PREPARED: f64 = 64.0 / 8_388_608.0;

/// A value over the input box.
#[derive(Clone, Debug)]
enum V {
    B(Option<bool>),
    /// An int or a u32: whole-number ends, one number where both are.
    I(Interval),
    F(Interval),
    C(Interval, Interval),
    /// A value with no bound -- past a pole, off a function's domain, or
    /// something the evaluator does not model -- refused only where it
    /// reaches the point.
    Bad(Refusal),
}

fn pt(v: f64) -> Interval {
    Interval::point(v)
}

fn domain(what: &'static str) -> V {
    V::Bad(Refusal::Domain(what))
}

fn pole() -> V {
    V::Bad(Refusal::Pole)
}

/// An interval, or a value with no bound where it is not finite or a rule
/// refused it.
fn real(r: Result<Interval, Refusal>) -> V {
    match r {
        Ok(x) if x.lo.is_nan() || x.hi.is_nan() => V::Bad(Refusal::Unbounded),
        Ok(x) => V::F(x),
        Err(w) => V::Bad(w),
    }
}

impl V {
    /// As the interpreter's `as_f64`: a complex's real part, a bool's 0
    /// or 1.
    fn real(&self) -> Result<Interval, Refusal> {
        match self {
            V::F(x) | V::I(x) | V::C(x, _) => Ok(*x),
            V::B(Some(b)) => Ok(pt(f64::from(u8::from(*b)))),
            V::B(None) => Ok(Interval::new(0.0, 1.0)),
            V::Bad(w) => Err(w.clone()),
        }
    }

    /// As `as_c64`: a real on the real axis.
    fn complex(&self) -> Result<(Interval, Interval), Refusal> {
        match self {
            V::C(x, y) => Ok((*x, *y)),
            other => Ok((other.real()?, pt(0.0))),
        }
    }

    /// As `as_bool`, where it is certain: anything not zero is true.
    fn truth(&self) -> Option<bool> {
        let nonzero = |x: &Interval| if x.lo == 0.0 && x.hi == 0.0 { Some(false) } else if !x.contains(0.0) { Some(true) } else { None };
        match self {
            V::B(b) => *b,
            V::I(x) | V::F(x) => nonzero(x),
            V::C(x, y) => match (nonzero(x), nonzero(y)) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            },
            V::Bad(_) => None,
        }
    }

    fn bad(&self) -> Option<&Refusal> {
        match self {
            V::Bad(w) => Some(w),
            _ => None,
        }
    }
}

/// Two values either path may leave: every value either holds.
fn join(a: &V, b: &V) -> V {
    match (a, b) {
        (V::Bad(w), _) | (_, V::Bad(w)) => V::Bad(w.clone()),
        (V::B(x), V::B(y)) => V::B(if x == y { *x } else { None }),
        (V::I(x), V::I(y)) => V::I(x.union(*y)),
        (V::F(x), V::F(y)) => V::F(x.union(*y)),
        (V::C(a, b), V::C(c, d)) => V::C(a.union(*c), b.union(*d)),
        _ => V::Bad(Refusal::Shape("two paths leave a variable different types")),
    }
}

/// What a call holds at a point in its body: its variables, its point and
/// its colour.
#[derive(Clone, Debug)]
struct St {
    vars: Vec<V>,
    /// Each array variable's elements; empty for a scalar.
    arrays: Vec<Vec<V>>,
    pixel: V,
    color: V,
}

impl St {
    fn join(mut self, o: &St) -> St {
        for (a, b) in self.vars.iter_mut().zip(&o.vars) {
            *a = join(a, b);
        }
        for (xs, ys) in self.arrays.iter_mut().zip(&o.arrays) {
            for (a, b) in xs.iter_mut().zip(ys) {
                *a = join(a, b);
            }
        }
        self.pixel = join(&self.pixel, &o.pixel);
        self.color = join(&self.color, &o.color);
        self
    }
}

fn join_opt(a: Option<St>, b: Option<St>) -> Option<St> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.join(&b)),
        (a, None) => a,
        (None, b) => b,
    }
}

/// How a run of statements can end: going on past them, returning,
/// leaving the innermost loop or going to its next round -- each with
/// every state that way can hold, several after a maybe.
#[derive(Default)]
struct Exit {
    fall: Option<St>,
    ret: Option<St>,
    brk: Option<St>,
    cont: Option<St>,
}

impl Exit {
    fn and(self, o: Exit) -> Exit {
        Exit { fall: join_opt(self.fall, o.fall), ret: join_opt(self.ret, o.ret), brk: join_opt(self.brk, o.brk), cont: join_opt(self.cont, o.cont) }
    }
}

// ---------------------------------------------------------------- ints

/// An integer range, wrapped as an `i32` wraps: past the type, any value.
fn int_range(x: Interval) -> Interval {
    if x.lo >= I32.lo && x.hi <= I32.hi {
        x
    } else {
        I32
    }
}

fn uint_range(x: Interval) -> Interval {
    if x.lo >= U32.lo && x.hi <= U32.hi {
        x
    } else {
        U32
    }
}

/// `op` over the corners: exact for `+ - *` of whole numbers below 2^53.
fn corners(a: Interval, b: Interval, op: impl Fn(f64, f64) -> f64) -> Interval {
    let c = [op(a.lo, b.lo), op(a.lo, b.hi), op(a.hi, b.lo), op(a.hi, b.hi)];
    Interval::new(c.iter().copied().fold(f64::INFINITY, f64::min), c.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

fn is_point(x: Interval) -> bool {
    x.lo == x.hi
}

/// `a % b` truncating, as WGSL's and Rust's: its sign is `a`'s, its size
/// under `|b|`'s largest. Whole numbers where `whole` (an int's, whose
/// remainder is at most `|b| - 1`); a zero divisor is 0 for an int and
/// NaN for a float.
fn rem(a: Interval, b: Interval, whole: bool) -> V {
    let m = b.lo.abs().max(b.hi.abs());
    let least = if b.contains(0.0) { 0.0 } else { b.lo.abs().min(b.hi.abs()) };
    if b.contains(0.0) && !whole {
        return pole();
    }
    // Every `|a|` under every `|b|`: `a` itself.
    if a.lo > -least && a.hi < least {
        return if whole { V::I(a) } else { V::F(a) };
    }
    let r = if whole { m - 1.0 } else { m };
    let out = if a.lo >= 0.0 {
        Interval::new(0.0, r.min(a.hi))
    } else if a.hi <= 0.0 {
        Interval::new(-r.min(-a.lo), 0.0)
    } else {
        Interval::new(-r.min(-a.lo), r.min(a.hi))
    };
    if whole {
        V::I(out)
    } else {
        V::F(out.widen())
    }
}

// ------------------------------------------------------------- complex

type Cx = (Interval, Interval);

fn real_point(x: Interval) -> bool {
    x.lo == 0.0 && x.hi == 0.0
}

fn cmul(a: Cx, b: Cx) -> Cx {
    // A real factor scales each part, without the cross terms' slack.
    if real_point(b.1) {
        return (a.0.mul(b.0), a.1.mul(b.0));
    }
    if real_point(a.1) {
        return (b.0.mul(a.0), b.1.mul(a.0));
    }
    (a.0.mul(b.0).sub(a.1.mul(b.1)), a.0.mul(b.1).add(a.1.mul(b.0)))
}

fn cdiv(a: Cx, b: Cx) -> V {
    // By a real: each part, `x / c` and not `x c / c^2`.
    if real_point(b.1) {
        return if b.0.contains(0.0) { pole() } else { V::C(a.0.div(b.0), a.1.div(b.0)) };
    }
    let d = square(b.0).add(square(b.1));
    if d.lo <= 0.0 {
        return pole();
    }
    let re = a.0.mul(b.0).add(a.1.mul(b.1)).div(d);
    let im = a.1.mul(b.0).sub(a.0.mul(b.1)).div(d);
    V::C(re, im)
}

fn cexp(z: Cx) -> V {
    let m = z.0.monotone(f64::exp);
    V::C(m.mul(z.1.trig(f64::cos, 0.0)), m.mul(z.1.trig(f64::sin, FRAC_PI_2)))
}

fn clog(z: Cx) -> V {
    let r2 = square(z.0).add(square(z.1));
    if r2.lo <= 0.0 {
        return domain("log");
    }
    V::C(r2.monotone(f64::ln).mul(pt(0.5)), atan2_iv(z.1, z.0))
}

/// The principal root, `sqrt|z|` at half the angle.
fn csqrt(z: Cx) -> V {
    let r2 = square(z.0).add(square(z.1));
    let Ok(r) = sqrt_iv(r2).and_then(sqrt_iv) else { return domain("sqrt") };
    let half = atan2_iv(z.1, z.0).mul(pt(0.5));
    V::C(r.mul(half.trig(f64::cos, 0.0)), r.mul(half.trig(f64::sin, FRAC_PI_2)))
}

/// `z^w` as `exp(w log z)`, the GPU's `uf_cpow`; a range through zero to
/// a positive real power is the disc of its largest modulus.
fn cpow(z: Cx, w: Cx) -> V {
    let r2 = square(z.0).add(square(z.1));
    if r2.lo <= 0.0 {
        if is_point(w.1) && w.1.lo == 0.0 && w.0.lo > 0.0 {
            let r = Interval::new(0.0, r2.hi.max(0.0)).monotone(|x| x.powf(0.5 * w.0.hi).max(x.powf(0.5 * w.0.lo)));
            return V::C(Interval::new(-r.hi, r.hi), Interval::new(-r.hi, r.hi));
        }
        return domain("pow");
    }
    match clog(z) {
        V::C(a, b) => cexp(cmul(w, (a, b))),
        bad => bad,
    }
}

fn cosh_iv(x: Interval) -> Interval {
    let a = x.abs();
    Interval::new(if x.contains(0.0) { 1.0 } else { a.lo.cosh() }, a.hi.cosh()).widen()
}

fn sinh_iv(x: Interval) -> Interval {
    x.monotone(f64::sinh)
}

fn sin_iv(x: Interval) -> Interval {
    x.trig(f64::sin, FRAC_PI_2)
}

fn cos_iv(x: Interval) -> Interval {
    x.trig(f64::cos, 0.0)
}

// ----------------------------------------------------------- built-ins

/// A real function of a real range, as `Builtin::eval_f` and the GPU
/// compute it.
fn real_fn(f: Builtin, x: Interval) -> V {
    use Builtin::*;
    let ok = V::F;
    match f {
        Sqr => ok(square(x)),
        Sqrt => real(sqrt_iv(x)),
        Log if x.lo <= 0.0 => domain("log"),
        Log => ok(x.monotone(f64::ln)),
        Log2 if x.lo <= 0.0 => domain("log2"),
        Log2 => ok(x.monotone(f64::log2)),
        Exp => ok(x.monotone(f64::exp)),
        Expm1 => ok(x.monotone(f64::exp_m1)),
        Log1p if x.lo <= -1.0 => domain("log1p"),
        Log1p => ok(x.monotone(f64::ln_1p)),
        Abs | Cabs => ok(x.abs()),
        Recip if x.contains(0.0) => pole(),
        Recip => ok(pt(1.0).div(x)),
        Ident | Real => ok(x),
        Zero | Imag => ok(pt(0.0)),
        Sin => ok(sin_iv(x)),
        Cos => ok(cos_iv(x)),
        Tan | Cotan => {
            let k = (x.lo / PI + 0.5).floor();
            if !x.finite() || x.lo <= (k - 0.5) * PI || x.hi >= (k + 0.5) * PI {
                return pole();
            }
            let t = x.monotone(f64::tan);
            if f == Tan {
                ok(t)
            } else if t.contains(0.0) {
                pole()
            } else {
                ok(pt(1.0).div(t))
            }
        }
        Sinh => ok(sinh_iv(x)),
        Cosh => ok(cosh_iv(x)),
        Tanh => ok(x.monotone(f64::tanh)),
        Cotanh if x.contains(0.0) => pole(),
        Cotanh => ok(pt(1.0).div(x.monotone(f64::tanh))),
        Asin | Acos if x.lo < -1.0 || x.hi > 1.0 => domain("asin"),
        Asin => ok(x.monotone(f64::asin)),
        // Decreasing.
        Acos => ok(Interval::new(x.hi.acos(), x.lo.acos()).widen()),
        Atan => ok(x.monotone(f64::atan)),
        Asinh => ok(x.monotone(f64::asinh)),
        Acosh if x.lo < 1.0 => domain("acosh"),
        Acosh => ok(x.monotone(f64::acosh)),
        Atanh if x.lo <= -1.0 || x.hi >= 1.0 => domain("atanh"),
        Atanh => ok(x.monotone(f64::atanh)),
        // The angle of (x, 0): 0 to its right, pi to its left.
        Atan2 if x.lo > 0.0 => ok(pt(0.0)),
        Atan2 if x.hi < 0.0 => ok(pt(PI).widen()),
        Atan2 => ok(Interval::new(0.0, PI).widen()),
        // Exact: the hull of halves away from zero (the interpreter's) and
        // to even (WGSL's).
        Round | RoundEven => ok(x.monotone_exact(f64::round).union(x.monotone_exact(f64::round_ties_even))),
        Trunc => ok(x.monotone_exact(f64::trunc)),
        Ceil => ok(x.monotone_exact(f64::ceil)),
        Floor => ok(x.monotone_exact(f64::floor)),
        F32 => ok(x.widen()),
        // Within a turn of zero, whatever the argument.
        Reduce => ok(Interval::new(-PI, PI).widen()),
        Fract => ok(Interval::new(0.0, 1.0)),
        other => V::Bad(Refusal::Intrinsic(other.name().to_string())),
    }
}

/// A complex function of a complex range, as `Builtin::eval_c`.
fn complex_fn(f: Builtin, z: Cx) -> V {
    use Builtin::*;
    let (a, b) = z;
    let parts = |g: &dyn Fn(Interval) -> V| match (g(a), g(b)) {
        (V::F(x), V::F(y)) => V::C(x, y),
        (V::Bad(w), _) | (_, V::Bad(w)) => V::Bad(w),
        _ => V::Bad(Refusal::Shape("a part")),
    };
    match f {
        Sqr => V::C(square(a).sub(square(b)), a.mul(b).mul(pt(2.0))),
        Sqrt => csqrt(z),
        Log => clog(z),
        Exp => cexp(z),
        Abs => V::C(a.abs(), b.abs()),
        Recip => cdiv((pt(1.0), pt(0.0)), z),
        Ident => V::C(a, b),
        Zero => V::C(pt(0.0), pt(0.0)),
        Conj => V::C(a, b.neg()),
        Flip => V::C(b, a),
        Real => V::C(a, pt(0.0)),
        Imag => V::C(b, pt(0.0)),
        Sin => V::C(sin_iv(a).mul(cosh_iv(b)), cos_iv(a).mul(sinh_iv(b))),
        Cos => V::C(cos_iv(a).mul(cosh_iv(b)), sin_iv(a).mul(sinh_iv(b)).neg()),
        Sinh => V::C(sinh_iv(a).mul(cos_iv(b)), cosh_iv(a).mul(sin_iv(b))),
        Cosh => V::C(cosh_iv(a).mul(cos_iv(b)), sinh_iv(a).mul(sin_iv(b))),
        Round | RoundEven | Trunc | Ceil | Floor | Fract | F32 => parts(&|x| real_fn(f, x)),
        other => V::Bad(Refusal::Intrinsic(other.name().to_string())),
    }
}

// ----------------------------------------------------------- evaluator

struct Ev<'a> {
    p: &'a Program,
    params: &'a ParamValues,
    weight: f64,
    rng: Interval,
}

impl Ev<'_> {
    fn exec(&self, stmts: &[IStmt], mut st: St) -> Result<Exit, Refusal> {
        // The ways out taken so far; the state going on is `st`.
        let mut out = Exit::default();
        for s in stmts {
            match s {
                IStmt::Assign(place, e) => {
                    let v = self.eval(e, &st)?;
                    match place {
                        Place::Var(id) => st.vars[*id] = v,
                        Place::Pixel => st.pixel = v.complex().map_or_else(V::Bad, |(x, y)| V::C(x, y)),
                        Place::Color => st.color = v.real().map_or_else(V::Bad, V::F),
                        // Nothing the point depends on.
                        Place::Hide | Place::RgbR | Place::RgbG | Place::RgbB | Place::WOut | Place::PixelZ => {}
                        _ => return Err(Refusal::Construct("a place a variation does not write")),
                    }
                }
                IStmt::If { arms, otherwise } => {
                    let mut open = Some(st);
                    let mut arms_out = Exit::default();
                    for (c, body) in arms {
                        let Some(cur) = open.take() else { break };
                        match self.eval(c, &cur)?.truth() {
                            Some(true) => arms_out = arms_out.and(self.exec(body, cur)?),
                            Some(false) => open = Some(cur),
                            // Each arm with what its test says of the
                            // variables it compares: `max(a, b)`'s `if b > t`
                            // then gives the max, not the hull of both.
                            None => {
                                arms_out = arms_out.and(self.exec(body, self.refine(c, cur.clone(), true)?)?);
                                open = Some(self.refine(c, cur, false)?);
                            }
                        }
                    }
                    if let Some(cur) = open {
                        arms_out = arms_out.and(self.exec(otherwise, cur)?);
                    }
                    let fall = arms_out.fall.take();
                    out = out.and(arms_out);
                    match fall {
                        Some(s) => st = s,
                        None => return Ok(out),
                    }
                }
                IStmt::While { cond, body } => {
                    let (mut exits, mut cur) = (None, st);
                    let mut settled = false;
                    for _ in 0..LOOP_CAP {
                        match self.eval(cond, &cur)?.truth() {
                            Some(false) => {
                                exits = join_opt(exits, Some(cur.clone()));
                                settled = true;
                                break;
                            }
                            None => exits = join_opt(exits, Some(cur.clone())),
                            Some(true) => {}
                        }
                        let e = self.exec(body, cur)?;
                        out.ret = join_opt(out.ret.take(), e.ret);
                        exits = join_opt(exits, e.brk);
                        match join_opt(e.fall, e.cont) {
                            Some(s) => cur = s,
                            None => {
                                settled = true;
                                break;
                            }
                        }
                    }
                    if !settled {
                        return Err(Refusal::LoopUnsettled);
                    }
                    match exits {
                        Some(s) => st = s,
                        None => return Ok(out),
                    }
                }
                IStmt::Repeat { body, cond } => {
                    let (mut exits, mut cur) = (None, st);
                    let mut settled = false;
                    for _ in 0..LOOP_CAP {
                        let e = self.exec(body, cur)?;
                        out.ret = join_opt(out.ret.take(), e.ret);
                        exits = join_opt(exits, e.brk);
                        // A `continue` goes on past the test, as the GPU's.
                        let mut next = e.cont;
                        if let Some(s) = e.fall {
                            // Until the test holds.
                            match self.eval(cond, &s)?.truth() {
                                Some(true) => exits = join_opt(exits, Some(s)),
                                None => {
                                    exits = join_opt(exits, Some(s.clone()));
                                    next = join_opt(next, Some(s));
                                }
                                Some(false) => next = join_opt(next, Some(s)),
                            }
                        }
                        match next {
                            Some(s) => cur = s,
                            None => {
                                settled = true;
                                break;
                            }
                        }
                    }
                    if !settled {
                        return Err(Refusal::LoopUnsettled);
                    }
                    match exits {
                        Some(s) => st = s,
                        None => return Ok(out),
                    }
                }
                IStmt::Return => {
                    out.ret = join_opt(out.ret.take(), Some(st));
                    return Ok(out);
                }
                IStmt::Break => {
                    out.brk = join_opt(out.brk.take(), Some(st));
                    return Ok(out);
                }
                IStmt::Continue => {
                    out.cont = join_opt(out.cont.take(), Some(st));
                    return Ok(out);
                }
                // An inlined helper: its return leaves it alone.
                IStmt::Scope(body) => {
                    let mut e = self.exec(body, st)?;
                    let on = join_opt(e.fall.take(), e.ret.take());
                    out = out.and(e);
                    match on {
                        Some(s) => st = s,
                        None => return Ok(out),
                    }
                }
                // A palette read or an emission: nothing the point depends on.
                IStmt::Host(h, _) if h.gpu_read() || h.emit() => {}
                IStmt::Host(..) => return Err(Refusal::Construct("a host call")),
                // One element where the index is one number; else each it
                // can reach may now hold the value too.
                IStmt::SetElem { var, index, value } => {
                    let v = self.eval(value, &st)?;
                    let range = self.elements(*var, index, &st)?;
                    let one = range.start() == range.end();
                    for k in range {
                        let a = &mut st.arrays[*var][k];
                        *a = if one { v.clone() } else { join(a, &v) };
                    }
                }
                IStmt::Map { .. } => return Err(Refusal::Construct("a built-in variation's call")),
            }
        }
        out.fall = Some(st);
        Ok(out)
    }

    /// The elements of array `var` an index can reach, clamped to the
    /// array as `IExprKind::Elem` and `IStmt::SetElem` clamp it.
    fn elements(&self, var: VarId, index: &IExpr, st: &St) -> Result<std::ops::RangeInclusive<usize>, Refusal> {
        let n = st.arrays[var].len();
        if n == 0 {
            return Err(Refusal::Shape("an array with no elements"));
        }
        let last = (n - 1) as f64;
        let r = match self.eval(index, st)?.real() {
            Ok(r) if r.lo.is_finite() && r.hi.is_finite() => r,
            _ => Interval::new(0.0, last),
        };
        Ok((r.lo.clamp(0.0, last) as usize)..=(r.hi.clamp(0.0, last) as usize))
    }

    /// `st` where the test `c` comes out `holds`: each variable a
    /// comparison sets against a value narrowed to the side the outcome
    /// leaves it (`t < b` holding: `t` no higher than `b` can be, `b` no
    /// lower than `t` can be). Sound for any outcome the ranges allow; a
    /// strict comparison is taken as its non-strict one.
    fn refine(&self, c: &IExpr, mut st: St, holds: bool) -> Result<St, Refusal> {
        use BinOp::*;
        match &c.kind {
            IExprKind::Unary(UnOp::Not, a) => self.refine(a, st, !holds),
            IExprKind::Binary(And, a, b) if holds => {
                let st = self.refine(a, st, true)?;
                self.refine(b, st, true)
            }
            IExprKind::Binary(Or, a, b) if !holds => {
                let st = self.refine(a, st, false)?;
                self.refine(b, st, false)
            }
            IExprKind::Binary(op @ (Lt | Le | Gt | Ge), a, b) if a.ty != Ty::Complex => {
                // Which side is the lesser, as the outcome says.
                let a_less = matches!(op, Lt | Le) == holds;
                let (lesser, greater) = if a_less { (a, b) } else { (b, a) };
                let (Ok(l), Ok(g)) = (self.eval(lesser, &st)?.real(), self.eval(greater, &st)?.real()) else { return Ok(st) };
                let mut narrow = |e: &IExpr, f: &dyn Fn(Interval) -> Interval| {
                    if let IExprKind::Var(id) = e.kind {
                        st.vars[id] = match &st.vars[id] {
                            V::F(x) => V::F(f(*x)),
                            V::I(x) => V::I(f(*x)),
                            other => other.clone(),
                        };
                    }
                };
                narrow(lesser, &|x| Interval::new(x.lo.min(g.hi), x.hi.min(g.hi)));
                narrow(greater, &|x| Interval::new(x.lo.max(l.lo), x.hi.max(l.lo)));
                Ok(st)
            }
            _ => Ok(st),
        }
    }

    fn eval(&self, e: &IExpr, st: &St) -> Result<V, Refusal> {
        Ok(match &e.kind {
            IExprKind::Bool(b) => V::B(Some(*b)),
            IExprKind::Int(i) => V::I(pt(*i as f64)),
            IExprKind::Float(x) => V::F(pt(*x)),
            IExprKind::Complex(a, b) => V::C(pt(*a), pt(*b)),
            IExprKind::Var(id) => st.vars[*id].clone(),
            IExprKind::Param(id) => {
                let (re, im) = self.params.values[*id];
                match self.p.params[*id].ty() {
                    Ty::Bool => V::B(Some(re != 0.0)),
                    Ty::Int | Ty::UInt => V::I(pt(re)),
                    Ty::Float => V::F(pt(re)),
                    Ty::Complex => V::C(pt(re), pt(im)),
                }
            }
            IExprKind::Predef(p) => self.predef(*p, st),
            IExprKind::Convert(a) => convert(self.eval(a, st)?, e.ty),
            IExprKind::MakeComplex(a, b) => match (self.eval(a, st)?.real(), self.eval(b, st)?.real()) {
                (Ok(x), Ok(y)) => V::C(x, y),
                (Err(w), _) | (_, Err(w)) => V::Bad(w),
            },
            IExprKind::Unary(UnOp::Neg, a) => match self.eval(a, st)? {
                V::I(x) => V::I(int_range(x.neg())),
                V::F(x) => V::F(x.neg()),
                V::C(x, y) => V::C(x.neg(), y.neg()),
                other => other,
            },
            IExprKind::Unary(UnOp::Not, a) => V::B(self.eval(a, st)?.truth().map(|b| !b)),
            // A value times itself is its square, which is never negative
            // and, of a complex, the square's own formula.
            IExprKind::Binary(BinOp::Mul, a, b) if same(a, b) && matches!(e.ty, Ty::Float | Ty::Complex) => match self.eval(a, st)? {
                V::C(x, y) => complex_fn(Builtin::Sqr, (x, y)),
                V::Bad(w) => V::Bad(w),
                other => real(other.real().map(square)),
            },
            IExprKind::Binary(op, a, b) => binary(*op, self.eval(a, st)?, self.eval(b, st)?, e.ty),
            IExprKind::PowInt(a, k) => match self.eval(a, st)? {
                V::C(x, y) => cpow_int((x, y), *k),
                V::Bad(w) => V::Bad(w),
                other => real(other.real().and_then(|x| pow_iv(x, pt(*k as f64)))),
            },
            IExprKind::Modulus(a) => match self.eval(a, st)? {
                V::C(x, y) => V::F(square(x).add(square(y))),
                V::Bad(w) => V::Bad(w),
                other => real(other.real().map(square)),
            },
            IExprKind::Call(f, a) => call(*f, self.eval(a, st)?, e.ty),
            IExprKind::UserCall(..) | IExprKind::UserDeriv(..) | IExprKind::UserJac(..) | IExprKind::UserDiff(..) | IExprKind::FuncChoice(_) => {
                V::Bad(Refusal::Intrinsic("a user function".to_string()))
            }
            IExprKind::Elem(var, index) => {
                let range = self.elements(*var, index, st)?;
                let xs = &st.arrays[*var];
                range.clone().skip(1).fold(xs[*range.start()].clone(), |a, k| join(&a, &xs[k]))
            }
            IExprKind::Host(Host::GridParam, args) => {
                let row = self.eval(&args[3], st)?;
                let Ok(r) = row.real() else { return Ok(row) };
                // Each row the index can take, clamped as `grid_cell` does.
                let IExprKind::Int(last) = args[2].kind else { return Err(Refusal::Shape("a grid's rows")) };
                let (lo, hi) = (r.lo.max(0.0).min(last as f64) as i32, r.hi.max(0.0).min(last as f64) as i32);
                let mut acc: Option<Interval> = None;
                for row in lo..=hi {
                    let v = pt(self.params.values[crate::lang::interp::grid_cell(args, row)].0);
                    acc = Some(acc.map_or(v, |a| a.union(v)));
                }
                V::F(acc.unwrap_or(pt(0.0)))
            }
            IExprKind::Host(..) => V::Bad(Refusal::Construct("a host read")),
        })
    }

    fn predef(&self, p: Predef, st: &St) -> V {
        match p {
            Predef::Pixel => st.pixel.clone(),
            Predef::VarColor => st.color.clone(),
            Predef::Draw => V::F(self.rng),
            Predef::DrawU32 => V::I(U32),
            Predef::Weight => V::F(pt(self.weight)),
            Predef::Pi => V::F(pt(PI)),
            Predef::E => V::F(pt(std::f64::consts::E)),
            // The plane has no z.
            Predef::PixelZ => V::F(pt(0.0)),
            Predef::VarHide => V::B(Some(false)),
            Predef::Accum | Predef::AccumZ => V::Bad(Refusal::Opaque("the running variation sum")),
            Predef::Coef(_) => V::Bad(Refusal::Opaque("the transform's coefficients")),
            _ => V::Bad(Refusal::Construct("a value a variation's bound does not model")),
        }
    }
}

/// Whether two expressions are one value: the same reads through the same
/// operations (`z.re * z.re`). An expression stores nothing, so two reads
/// of a variable in one see one value; two draws are two values.
fn same(a: &IExpr, b: &IExpr) -> bool {
    use IExprKind as K;
    match (&a.kind, &b.kind) {
        (K::Var(x), K::Var(y)) => x == y,
        (K::Param(x), K::Param(y)) => x == y,
        (K::Predef(x), K::Predef(y)) => x == y && !matches!(x, Predef::Draw | Predef::DrawU32),
        (K::Float(x), K::Float(y)) => x == y,
        (K::Int(x), K::Int(y)) => x == y,
        (K::Convert(x), K::Convert(y)) | (K::Modulus(x), K::Modulus(y)) => a.ty == b.ty && same(x, y),
        (K::Unary(o, x), K::Unary(p, y)) => o == p && same(x, y),
        (K::Call(f, x), K::Call(g, y)) => f == g && a.ty == b.ty && same(x, y),
        (K::PowInt(x, m), K::PowInt(y, n)) => m == n && same(x, y),
        (K::Binary(o, x1, x2), K::Binary(p, y1, y2)) => o == p && a.ty == b.ty && same(x1, y1) && same(x2, y2),
        (K::MakeComplex(x1, x2), K::MakeComplex(y1, y2)) => same(x1, y1) && same(x2, y2),
        _ => false,
    }
}

/// A conversion to `ty`, as the interpreter's `convert`.
fn convert(v: V, ty: Ty) -> V {
    if let V::Bad(_) = v {
        return v;
    }
    match ty {
        Ty::Bool => V::B(v.truth()),
        Ty::Float => v.real().map_or_else(V::Bad, V::F),
        Ty::Complex => v.complex().map_or_else(V::Bad, |(x, y)| V::C(x, y)),
        Ty::Int => match v {
            // A u32's bits.
            V::I(x) => V::I(int_range(x)),
            other => match other.real() {
                Ok(x) if x.finite() => V::I(int_range(x.monotone_exact(f64::trunc))),
                Ok(_) => V::I(I32),
                Err(w) => V::Bad(w),
            },
        },
        Ty::UInt => match v {
            V::I(x) => V::I(uint_range(x)),
            other => match other.real() {
                // `as u32` of a float clamps.
                Ok(x) if x.finite() => V::I(Interval::new(x.lo.clamp(0.0, 4_294_967_040.0).trunc(), x.hi.clamp(0.0, 4_294_967_040.0).trunc())),
                Ok(_) => V::I(U32),
                Err(w) => V::Bad(w),
            },
        },
    }
}

fn cpow_int(z: Cx, k: i64) -> V {
    if k.unsigned_abs() > 64 {
        return V::Bad(Refusal::Domain("a power past 64"));
    }
    let mut acc = (pt(1.0), pt(0.0));
    let mut base = z;
    let mut n = k.unsigned_abs();
    while n > 0 {
        if n & 1 == 1 {
            acc = cmul(acc, base);
        }
        base = cmul(base, base);
        n >>= 1;
    }
    if k < 0 {
        cdiv((pt(1.0), pt(0.0)), acc)
    } else {
        V::C(acc.0, acc.1)
    }
}

fn eq_iv(a: Interval, b: Interval) -> Option<bool> {
    if is_point(a) && is_point(b) && a.lo == b.lo {
        Some(true)
    } else if a.hi < b.lo || b.hi < a.lo {
        Some(false)
    } else {
        None
    }
}

/// `op` on two values, operands of the operation's type (comparisons:
/// their common one), as the interpreter's `binary`.
fn binary(op: BinOp, x: V, y: V, ty: Ty) -> V {
    use BinOp::*;
    match op {
        And => V::B(match (x.truth(), y.truth()) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        }),
        Or => V::B(match (x.truth(), y.truth()) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        }),
        Eq | Ne | Lt | Le | Gt | Ge => {
            if x.bad().is_some() || y.bad().is_some() {
                return V::B(None);
            }
            let r = match (op, &x, &y) {
                (Eq | Ne, V::C(a, b), V::C(c, d)) => match (eq_iv(*a, *c), eq_iv(*b, *d)) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                },
                (Eq | Ne, V::B(a), V::B(b)) => match (a, b) {
                    (Some(a), Some(b)) => Some(a == b),
                    _ => None,
                },
                _ => {
                    let (Ok(a), Ok(b)) = (x.real(), y.real()) else { return V::B(None) };
                    match op {
                        Eq | Ne => eq_iv(a, b),
                        Lt => cmp_lt(a, b),
                        Le => cmp_le(a, b),
                        Gt => cmp_lt(b, a),
                        _ => cmp_le(b, a),
                    }
                }
            };
            V::B(if op == Ne { r.map(|b| !b) } else { r })
        }
        _ => {
            if let Some(w) = x.bad().or(y.bad()) {
                return V::Bad(w.clone());
            }
            arith(op, x, y, ty)
        }
    }
}

fn arith(op: BinOp, x: V, y: V, ty: Ty) -> V {
    use BinOp::*;
    let (Ok(a), Ok(b)) = (x.real(), y.real()) else { return V::Bad(Refusal::Shape("an operand")) };
    match (ty, op) {
        (Ty::Int | Ty::UInt, BitAnd | BitOr | BitXor | Shl | Shr | IDiv) => {
            let full = if ty == Ty::UInt { U32 } else { I32 };
            if is_point(a) && is_point(b) {
                let v = if ty == Ty::UInt {
                    f64::from(crate::lang::interp::ubits(op, a.lo as i64 as u32, b.lo as i64 as u32))
                } else {
                    f64::from(crate::lang::interp::bits(op, a.lo as i64 as i32, b.lo as i64 as i32))
                };
                return V::I(pt(v));
            }
            if op == IDiv && !b.contains(0.0) {
                return V::I(corners(a, b, |p, q| (p / q).trunc()));
            }
            V::I(full)
        }
        (Ty::Int, Add) => V::I(int_range(Interval::new(a.lo + b.lo, a.hi + b.hi))),
        (Ty::Int, Sub) => V::I(int_range(Interval::new(a.lo - b.hi, a.hi - b.lo))),
        (Ty::Int, Mul) => V::I(int_range(corners(a, b, |p, q| p * q))),
        (Ty::UInt, Add) => V::I(uint_range(Interval::new(a.lo + b.lo, a.hi + b.hi))),
        (Ty::UInt, Sub) => V::I(uint_range(Interval::new(a.lo - b.hi, a.hi - b.lo))),
        (Ty::UInt, Mul) => V::I(uint_range(corners(a, b, |p, q| p * q))),
        (Ty::Int | Ty::UInt, Mod) => {
            if b.contains(0.0) {
                // A zero divisor gives 0.
                return match rem(a, Interval::new(b.lo.min(-1.0).max(b.lo), b.hi.max(1.0)), true) {
                    V::I(r) => V::I(r.union(pt(0.0))),
                    other => other,
                };
            }
            rem(a, b, true)
        }
        (Ty::Float, Add) => V::F(a.add(b)),
        (Ty::Float, Sub) => V::F(a.sub(b)),
        (Ty::Float, Mul) => V::F(a.mul(b)),
        (Ty::Float, Div) if b.contains(0.0) => pole(),
        (Ty::Float, Div) => V::F(a.div(b)),
        (Ty::Float, Mod) => rem(a, b, false),
        (Ty::Float, Pow) => real(powf_iv(a, b)),
        (Ty::Complex, _) => {
            let (Ok(z), Ok(w)) = (x.complex(), y.complex()) else { return V::Bad(Refusal::Shape("an operand")) };
            match op {
                Add => V::C(z.0.add(w.0), z.1.add(w.1)),
                Sub => V::C(z.0.sub(w.0), z.1.sub(w.1)),
                Mul => {
                    let (re, im) = cmul(z, w);
                    V::C(re, im)
                }
                Div => cdiv(z, w),
                Pow => cpow(z, w),
                _ => V::Bad(Refusal::Construct("a complex operation")),
            }
        }
        _ => V::Bad(Refusal::Construct("an operation a variation's bound does not model")),
    }
}

/// A built-in, as the interpreter's `call`: on the argument's type, to
/// the result's.
fn call(f: Builtin, x: V, ty: Ty) -> V {
    if let V::Bad(w) = &x {
        return if ty == Ty::Bool { V::B(None) } else { V::Bad(w.clone()) };
    }
    match ty {
        // `is_inf`, `is_nan`: false of a finite range well inside f32's.
        Ty::Bool => {
            let fine = |i: &Interval| i.lo > -1e37 && i.hi < 1e37;
            V::B(match &x {
                V::C(a, b) if fine(a) && fine(b) => Some(false),
                V::F(a) | V::I(a) if fine(a) => Some(false),
                _ => None,
            })
        }
        Ty::Int | Ty::UInt => match (&x, f) {
            (V::I(a), Builtin::Sqr) => V::I(int_range(square(*a).monotone_exact(f64::round))),
            (V::I(a), Builtin::Abs) => V::I(int_range(a.abs())),
            (V::I(_), Builtin::Zero) => V::I(pt(0.0)),
            (V::I(_), _) => x,
            _ => match real_fn(f, x.real().expect("not bad")) {
                V::F(r) if r.finite() => V::I(int_range(r)),
                V::F(_) => V::I(I32),
                other => other,
            },
        },
        Ty::Float => match &x {
            V::C(a, b) => match f {
                Builtin::Cabs => real(sqrt_iv(square(*a).add(square(*b)))),
                Builtin::Real => V::F(*a),
                Builtin::Imag => V::F(*b),
                Builtin::Atan2 => V::F(atan2_iv(*b, *a)),
                other => V::Bad(Refusal::Intrinsic(other.name().to_string())),
            },
            _ => real_fn(f, x.real().expect("not bad")),
        },
        Ty::Complex => complex_fn(f, x.complex().expect("not bad")),
    }
}

/// **The bound of a variation's body over an input box**: its point's
/// range, `[re, im]`. `params` the flame's values; `weight` what the body
/// reads as `ctx.weight` (the caller scales the result by it, as for
/// `derive`); `rng` the range of every draw.
pub fn bound_program(p: &Program, params: &ParamValues, weight: f64, input: [Interval; 2], rng: Interval) -> Result<[Interval; 2], Refusal> {
    let (start, start_arrays, state) = crate::lang::interp::call_start(p, params);
    let exact = |x: f64| pt(x);
    let ev = Ev { p, params, weight, rng };
    // `prepare` through the same rules, from zero at the flame's
    // parameters: a value it picks (`if power < 0.0 { -1.0 } else { 1.0 }`)
    // stays the one number, so a body's `sign == 1.0` is decided, and one
    // it computes carries the f32 slack its operations earn. Where the
    // rules refuse it, the interpreter's f64 numbers, widened.
    let zero = St {
        vars: start.iter().map(|x| number(x.zero_like(), &exact)).collect(),
        arrays: start_arrays.iter().map(|xs| xs.iter().map(|x| number(x.zero_like(), &exact)).collect()).collect(),
        pixel: V::C(pt(0.0), pt(0.0)),
        color: V::F(Interval::new(0.0, 1.0)),
    };
    let prepared = ev.exec(&p.global_body, zero).ok().and_then(|e| join_opt(e.fall, e.ret));
    let widened = |x: f64| Interval::new(x - x.abs() * PREPARED, x + x.abs() * PREPARED);
    let global = |from_rules: Option<&V>, number_of: Value| match from_rules {
        Some(x) if x.bad().is_none() => x.clone(),
        _ => number(number_of, &widened),
    };
    let vars = (0..p.vars.len())
        .map(|v| {
            let var = &p.vars[v];
            if var.is_array() {
                V::Bad(Refusal::Shape("an array read as a scalar"))
            } else if state[v] {
                V::Bad(Refusal::State)
            } else if var.global {
                global(prepared.as_ref().map(|s| &s.vars[v]), start[v])
            } else {
                number(start[v].zero_like(), &exact)
            }
        })
        .collect();
    // A table `prepare` fills is its values; any other array starts at
    // zero each call, and `init`'s state is unknown.
    let arrays = (0..p.vars.len())
        .map(|v| {
            start_arrays[v]
                .iter()
                .enumerate()
                .map(|(k, &x)| {
                    if state[v] {
                        V::Bad(Refusal::State)
                    } else if p.vars[v].global {
                        global(prepared.as_ref().and_then(|s| s.arrays[v].get(k)), x)
                    } else {
                        number(x.zero_like(), &exact)
                    }
                })
                .collect()
        })
        .collect();
    // The colour a call is given is the flame's, whatever it is: a point
    // that depends on it refuses, as `derive`'s opaque colour pointer does.
    // One the body writes and reads back is what it wrote.
    let st = St { vars, arrays, pixel: V::C(input[0], input[1]), color: V::Bad(Refusal::Opaque("the colour the call is given")) };
    let exit = ev.exec(&p.transform, st)?;
    let end = join_opt(exit.fall, exit.ret).ok_or(Refusal::Shape("no way out of the body"))?;
    match end.pixel {
        V::C(x, y) if x.finite() && y.finite() => Ok([x, y]),
        V::C(..) => Err(Refusal::Unbounded),
        V::Bad(w) => Err(w),
        _ => Err(Refusal::Shape("the point is not a complex")),
    }
}

/// A number the interpreter computed, as a value: a float widened by
/// `widened` (an exact zero stays one).
fn number(x: Value, widened: &dyn Fn(f64) -> Interval) -> V {
    match x {
        Value::Bool(b) => V::B(Some(b)),
        Value::Int(i) => V::I(pt(f64::from(i))),
        Value::UInt(u) => V::I(pt(f64::from(u))),
        Value::Float(x) if x.is_finite() => V::F(widened(x)),
        Value::Complex(c) if c.re.is_finite() && c.im.is_finite() => V::C(widened(c.re), widened(c.im)),
        _ => V::Bad(Refusal::Unbounded),
    }
}

/// The program a variation's bound comes from: a Rhai variation's own
/// source, a built-in's twin's -- each compiled once a source.
fn program_for(registry: &super::VariationRegistry, name: &str) -> Option<Arc<Program>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<Arc<Program>>>>> = std::sync::OnceLock::new();
    let name = super::twins::planner_name(name);
    let builtin = registry.get(name).map(|i| i.provenance.is_builtin())?;
    let source = if builtin { super::twins::source(name)?.to_string() } else { super::custom::source_of(name)? };
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok()?.get(&source) {
        return hit.clone();
    }
    let built = crate::lang::rhai::transformation(&source).ok().map(Arc::new);
    if let Ok(mut c) = cache.lock() {
        c.insert(source, built.clone());
    }
    built
}

/// A program's parameter values from the flame's, by name: a complex one
/// as its two parts (`name_re`, `name_im`, as the panel shows them).
fn values(p: &Program, params: ParamFn) -> ParamValues {
    let values = p
        .params
        .iter()
        .map(|q| if q.ty() == Ty::Complex { (params(&format!("{}_re", q.name)), params(&format!("{}_im", q.name))) } else { (params(&q.name), 0.0) })
        .collect();
    let funcs = p.funcs.iter().map(|f| params(&f.name).max(0.0).round() as usize).collect();
    ParamValues { values, funcs }
}

fn evaluate(registry: &super::VariationRegistry, name: &str, params: ParamFn, weight: f64, input: [Interval; 2], branch: Option<Branch>) -> Result<[Interval; 2], Refusal> {
    let p = program_for(registry, name).ok_or(Refusal::NoBody)?;
    bound_program(&p, &values(&p, params), weight, input, rng_for(branch))
}

fn cell(ball: Ball) -> [Interval; 2] {
    [Interval::new(ball.c[0] - ball.r, ball.c[0] + ball.r), Interval::new(ball.c[1] - ball.r, ball.c[1] + ball.r)]
}

/// **The disc the variation `name` sends `ball` into**, from its Rhai
/// code: a Rhai variation's, or a built-in's twin's (`registry` the one
/// the caller holds). As `derive::derive`.
pub fn derive(registry: &super::VariationRegistry, name: &str, params: ParamFn, weight: f64, ball: Ball) -> Result<Ball, Refusal> {
    box_to_disc(evaluate(registry, name, params, weight, cell(ball), None)?)
}

/// [`derive`] with the draw pinned to one arm's slice (`derive::Branch`).
pub fn derive_branch(registry: &super::VariationRegistry, name: &str, params: ParamFn, weight: f64, ball: Ball, branch: Branch) -> Result<Ball, Refusal> {
    box_to_disc(evaluate(registry, name, params, weight, cell(ball), Some(branch))?)
}

/// [`derive`] over a `k x k` grid of the ball's box, the cells' union: as
/// `derive::derive_subdivided`, a refusal of any cell refusing.
pub fn derive_subdivided(registry: &super::VariationRegistry, name: &str, params: ParamFn, weight: f64, ball: Ball, k: usize) -> Result<Ball, Refusal> {
    let k = k.max(1);
    let step = 2.0 * ball.r / k as f64;
    let mut acc: Option<[Interval; 2]> = None;
    for iy in 0..k {
        for ix in 0..k {
            let (x, y) = (ball.c[0] - ball.r + step * ix as f64, ball.c[1] - ball.r + step * iy as f64);
            let out = evaluate(registry, name, params, weight, [Interval::new(x, x + step), Interval::new(y, y + step)], None)?;
            acc = Some(match acc {
                None => out,
                Some(a) => [a[0].union(out[0]), a[1].union(out[1])],
            });
        }
    }
    box_to_disc(acc.ok_or(Refusal::Shape("no cells"))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::builtins::C64;
    use crate::lang::interp::VariationRun;

    /// Each twin's program at its defaults, or why not.
    fn twin(name: &str) -> Option<(Program, ParamValues)> {
        let p = crate::lang::rhai::transformation(crate::variations::twins::source(name)?).ok()?;
        let v = ParamValues::defaults(&p);
        Some((p, v))
    }

    /// The points of a box the soundness checks run: a grid with its
    /// edges, 9 x 9.
    fn grid(input: [Interval; 2]) -> Vec<C64> {
        let mut out = Vec::new();
        for i in 0..9 {
            for j in 0..9 {
                let t = |iv: Interval, k: usize| iv.lo + (iv.hi - iv.lo) * k as f64 / 8.0;
                out.push(C64::new(t(input[0], i), t(input[1], j)));
            }
        }
        out
    }

    /// Where the bound misses a point the interpreter maps: the first
    /// one, or none. Every draw on a grid of 16 in the slice `rng`.
    fn misses(p: &Program, v: &ParamValues, input: [Interval; 2], rng: Interval, out: [Interval; 2]) -> Option<String> {
        for k in 0..16 {
            let u = rng.lo + (rng.hi - rng.lo) * (k as f64 + 0.5) / 16.0;
            let mut run = VariationRun::new(p, v, 1).with_draw(u).with_color(0.5);
            for z in grid(input) {
                run.reset_locals();
                let w = run.call(z, C64::ZERO, 1.0).0;
                if !(w.re.is_finite() && w.im.is_finite()) {
                    continue;
                }
                if !out[0].contains(w.re) || !out[1].contains(w.im) {
                    return Some(format!("{z:?} at draw {u:.3} -> {w:?}, outside {out:?}"));
                }
            }
        }
        None
    }

    const DISCS: [([f64; 2], f64); 4] = [([0.6, -0.4], 0.25), ([0.0, 0.0], 0.5), ([-1.5, 1.0], 0.1), ([2.0, 0.3], 1.0)];

    /// **A bound holds every point it bounds**: a few twins with the
    /// shapes the rules have to get right -- roots and powers (julian,
    /// juliascope), divisions (spherical, bubble), angles (swirl, polar,
    /// disc, hyperbolic), trig (sinusoidal), branches (splits) -- each
    /// derives at the discs it can, and the interpreter maps no grid
    /// point at any draw outside it.
    #[test]
    fn a_bound_holds_every_point_it_bounds() {
        let mut derived = 0;
        for name in ["julian", "juliascope", "spherical", "bubble", "swirl", "polar", "disc", "hyperbolic", "sinusoidal", "splits", "linear", "exponential"] {
            let (p, v) = twin(name).unwrap_or_else(|| panic!("{name}'s twin compiles"));
            for (c, r) in DISCS {
                let ball = Ball::new(c, r);
                let input = cell(ball);
                let rng = Interval::new(0.0, 1.0);
                match bound_program(&p, &v, 1.0, input, rng) {
                    Ok(out) => {
                        derived += 1;
                        if let Some(m) = misses(&p, &v, input, rng, out) {
                            panic!("{name} at {c:?} r {r}: {m}");
                        }
                    }
                    Err(e) => println!("  {name} at {c:?} r {r}: {e}"),
                }
            }
        }
        assert!(derived >= 36, "only {derived} of 48 derive");
    }

    /// **What the twins bound** (rhai-one-form step 2): every twin's bound
    /// from its own code at its built-in's defaults, against the
    /// built-in's from its WGSL (`derive`, naga), at the four discs -- how
    /// many of each derive, the tree's disc over naga's where both do, what
    /// refuses the tree where naga derives, and every tree bound checked
    /// against the interpreter's points at 16 draws (a miss fails).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "a census of 640 twins"]
    fn what_the_twins_bound() {
        use std::collections::BTreeMap;
        let reg = crate::variations::global_registry().clone();
        let (mut both, mut tree_only, mut naga_only, mut neither, mut uncompiled) = (0usize, 0usize, 0usize, 0usize, 0usize);
        let mut ratios: Vec<(f64, String)> = Vec::new();
        let mut why: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut missed = Vec::new();
        let (mut t_tree, mut n_tree) = (0.0f64, 0usize);
        for (name, src) in crate::variations::twins::TWINS.iter() {
            let Some(info) = reg.get(name) else { continue };
            let Ok(p) = crate::lang::rhai::transformation(src) else {
                uncompiled += 1;
                continue;
            };
            let pf = |q: &str| info.parameters.iter().find(|d| d.name == q).map(|d| d.slot_default() as f64).unwrap_or(0.0);
            let v = values(&p, &pf);
            for (c, r) in DISCS {
                let ball = Ball::new(c, r);
                let t0 = std::time::Instant::now();
                let tree = bound_program(&p, &v, 1.0, cell(ball), Interval::new(0.0, 1.0));
                t_tree += t0.elapsed().as_secs_f64();
                n_tree += 1;
                if let Ok(out) = tree {
                    if let Some(m) = misses(&p, &v, cell(ball), Interval::new(0.0, 1.0), out) {
                        missed.push(format!("{name} at {c:?} r {r}: {m}"));
                    }
                }
                let naga = crate::variations::derive::derive(name, &pf, 1.0, ball);
                match (tree.and_then(box_to_disc), naga) {
                    (Ok(a), Ok(b)) => {
                        both += 1;
                        let dc = (a.c[0] - b.c[0]).hypot(a.c[1] - b.c[1]);
                        ratios.push(((a.r + dc) / b.r.max(1e-30), format!("{name} at {c:?} r {r}")));
                    }
                    (Ok(_), Err(_)) => tree_only += 1,
                    (Err(e), Ok(_)) => {
                        naga_only += 1;
                        let key = match e {
                            Refusal::Intrinsic(f) => format!("no rule for `{f}`"),
                            other => other.to_string(),
                        };
                        why.entry(key).or_default().push(format!("{name}@{r}"));
                    }
                    (Err(_), Err(_)) => neither += 1,
                }
            }
        }
        ratios.sort_by(|a, b| a.0.total_cmp(&b.0));
        let q = |f: f64| ratios.get(((ratios.len() as f64 - 1.0) * f) as usize).map_or(f64::NAN, |r| r.0);
        println!();
        println!("  {} discs ({uncompiled} twins did not compile): both derive {both}; the tree alone {tree_only}; naga alone {naga_only}; neither {neither}", n_tree);
        println!("  the tree's disc over naga's: median {:.3}, 90% {:.3}, 99% {:.3}, worst {:.3}; {} of {both} as tight or tighter", q(0.5), q(0.9), q(0.99), q(1.0), ratios.iter().filter(|r| r.0 <= 1.0 + 1e-9).count());
        println!("  the widest: {:?}", ratios.iter().rev().take(8).map(|(r, n)| format!("{n} {r:.2}")).collect::<Vec<_>>());
        println!("  a tree bound: {:.3} ms", t_tree * 1e3 / n_tree.max(1) as f64);
        println!("  where naga derives and the tree does not:");
        let mut rows: Vec<_> = why.iter().collect();
        rows.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (k, v) in rows {
            println!("    {:>4}  {k}: {}", v.len(), v.iter().take(6).cloned().collect::<Vec<_>>().join(", "));
        }
        assert!(missed.is_empty(), "{} bounds miss a point:
{}", missed.len(), missed.join("
"));
    }

    /// TIMING: one bound of julian, swirl and spherical through each way
    /// in -- naga's `derive` (warm), this module's `derive` by name (its
    /// program found and its parameters read each call, as the planner
    /// calls it), and `bound_program` alone on a compiled program.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "timing; run with --ignored --nocapture"]
    fn time_a_bound() {
        let reg = crate::variations::global_registry();
        let ball = Ball::new([0.6, -0.4], 0.25);
        for name in ["julian", "swirl", "spherical", "juliascope", "jac_asn", "minkowskope", "truchet2"] {
            let info = reg.get(name).expect("a built-in").clone();
            let pf = |q: &str| info.parameters.iter().find(|d| d.name == q).map(|d| d.slot_default() as f64).unwrap_or(0.0);
            let each = |f: &dyn Fn() -> bool| {
                let _ = f();
                let n = 2000;
                let t0 = std::time::Instant::now();
                let mut ok = 0;
                for _ in 0..n {
                    ok += usize::from(f());
                }
                (t0.elapsed().as_secs_f64() * 1e6 / n as f64, ok)
            };
            let (naga, naga_ok) = each(&|| crate::variations::derive::derive(name, &pf, 1.0, ball).is_ok());
            let (by_name, tree_ok) = each(&|| derive(&reg, name, &pf, 1.0, ball).is_ok());
            let p = program_for(&reg, name).expect("a twin");
            let v = values(&p, &pf);
            let (alone, _) = each(&|| bound_program(&p, &v, 1.0, cell(ball), Interval::new(0.0, 1.0)).is_ok());
            println!("  {name:<12} naga {naga:9.1} us ({}) | by name {by_name:9.1} us ({}) | bound_program {alone:9.1} us", if naga_ok > 0 { "bounds" } else { "refuses" }, if tree_ok > 0 { "bounds" } else { "refuses" });
        }
    }

    /// Pinning julian's draw to one arm's slice shrinks its bound, as
    /// `derive`'s `pinning_the_arm_makes_julian_shrink` measures of the
    /// WGSL.
    #[test]
    fn pinning_an_arm_shrinks_julians_bound() {
        let (p, mut v) = twin("julian").expect("julian");
        v.values[0].0 = 5.0;
        let input = cell(Ball::new([0.6, -0.4], 0.05));
        let all = box_to_disc(bound_program(&p, &v, 1.0, input, Interval::new(0.0, 1.0)).expect("free")).expect("a disc");
        for k in 0..5 {
            let rng = rng_for(Some((k, 5)));
            let out = bound_program(&p, &v, 1.0, input, rng).expect("pinned");
            assert!(misses(&p, &v, input, rng, out).is_none(), "arm {k}");
            let one = box_to_disc(out).expect("a disc");
            assert!(one.r < 0.5 * all.r, "arm {k}: {} against the free {}", one.r, all.r);
        }
    }

    /// A pole the point never reads leaves the bound: a division by a
    /// range through zero feeding the colour alone.
    #[test]
    fn a_pole_only_the_colour_reads_is_not_a_refusal() {
        let src = r#"
const INFO = #{ title: "Colour pole", name: "colour_pole", category: "advanced_2d" };
fn variation(z, p) {
    let c = 1.0 / z.re;
    #{ z: z * 0.5, color: c }
}
"#;
        let p = crate::lang::rhai::transformation(src).expect("compiles");
        let v = ParamValues::defaults(&p);
        let input = [Interval::new(-1.0, 1.0), Interval::new(-1.0, 1.0)];
        let out = bound_program(&p, &v, 1.0, input, Interval::new(0.0, 1.0)).expect("bounded");
        assert!(out[0].lo >= -0.51 && out[0].hi <= 0.51, "{out:?}");
        let src2 = src.replace("z * 0.5", "z * c");
        let p2 = crate::lang::rhai::transformation(&src2).expect("compiles");
        assert!(bound_program(&p2, &ParamValues::defaults(&p2), 1.0, input, Interval::new(0.0, 1.0)).is_err(), "the pole reaches the point");
    }
}
