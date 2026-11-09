//! **A variation's call over many points at once**
//! (docs/projects/rhai-planners.md, E4).
//!
//! The walk's CPU evaluator replays each word over a few hundred points,
//! and a Rhai kernel's forward map was a tree walk a point -- 80% of a
//! twin's plan. Here each node of the body is dispatched once for all the
//! points. A value is a column of them, or one value where every lane
//! holds it (a parameter, a constant, a forced draw, what is made of
//! those); a branch runs its arms under per-lane masks, and an early
//! return masks its lanes out for the rest of the call. Each lane's
//! arithmetic is the scalar run's own functions (`binary`, `call`,
//! `convert`, ...), so every lane is [`VariationRun::call`] bit for bit
//! (`a_batch_is_its_scalar_calls`).
//!
//! A body outside the subset -- a loop, an array, a draw not forced, a
//! write to anything but its own locals, its point and its colour -- runs
//! as scalar calls, one a point.

use super::*;
use std::rc::Rc;

/// A value across a batch's lanes: one every lane holds, or a column --
/// of floats and of complexes as such, the scalar run's `Value::Float`
/// and `Value::Complex` lane for lane, and of anything else as values.
#[derive(Clone, Debug)]
enum Col {
    One(Value),
    F(Rc<Vec<f64>>),
    C(Rc<Vec<C64>>),
    V(Rc<Vec<Value>>),
}

impl Col {
    /// Lane `i`'s value, as the scalar run holds it.
    fn at(&self, i: usize) -> Value {
        match self {
            Col::One(v) => *v,
            Col::F(v) => Value::Float(v[i]),
            Col::C(v) => Value::Complex(v[i]),
            Col::V(v) => v[i],
        }
    }

    /// `self.at(i).as_f64()`.
    fn f(&self, i: usize) -> f64 {
        match self {
            Col::F(v) => v[i],
            Col::C(v) => v[i].re,
            other => other.at(i).as_f64(),
        }
    }

    /// `self.at(i).as_c64()`.
    fn c(&self, i: usize) -> C64 {
        match self {
            Col::C(v) => v[i],
            Col::F(v) => C64::real(v[i]),
            other => other.at(i).as_c64(),
        }
    }

    fn is_one(&self) -> bool {
        matches!(self, Col::One(_))
    }

    /// Values as a column, typed where they all are one type.
    fn pack(vals: Vec<Value>) -> Col {
        if vals.iter().all(|v| matches!(v, Value::Float(_))) {
            Col::F(Rc::new(vals.iter().map(|v| v.as_f64()).collect()))
        } else if vals.iter().all(|v| matches!(v, Value::Complex(_))) {
            Col::C(Rc::new(vals.iter().map(|v| v.as_c64()).collect()))
        } else {
            Col::V(Rc::new(vals))
        }
    }

    /// `f` of each of the `n` lanes, once where every lane holds the same.
    fn map(&self, n: usize, f: impl Fn(Value) -> Value) -> Col {
        match self {
            Col::One(v) => Col::One(f(*v)),
            _ => Col::pack((0..n).map(|i| f(self.at(i))).collect()),
        }
    }

    /// `f` of each lane of two columns.
    fn zip(&self, other: &Col, n: usize, f: impl Fn(Value, Value) -> Value) -> Col {
        match (self, other) {
            (Col::One(a), Col::One(b)) => Col::One(f(*a, *b)),
            _ => Col::pack((0..n).map(|i| f(self.at(i), other.at(i))).collect()),
        }
    }

    /// `v` into this column on the `active` lanes.
    fn assign(&mut self, v: Col, active: &[bool]) {
        if active.iter().all(|&a| a) {
            *self = v;
            return;
        }
        let vals = (0..active.len()).map(|i| if active[i] { v.at(i) } else { self.at(i) }).collect();
        *self = Col::pack(vals);
    }
}

fn floats(n: usize, f: impl Fn(usize) -> f64) -> Col {
    Col::F(Rc::new((0..n).map(f).collect()))
}

fn complexes(n: usize, f: impl Fn(usize) -> C64) -> Col {
    Col::C(Rc::new((0..n).map(f).collect()))
}

/// A batch's state: each variable's column, the point and the colour,
/// and the call's shared values every lane reads alike.
struct Lanes<'s, 'e> {
    n: usize,
    vars: Vec<Col>,
    pixel: Col,
    color: Col,
    sh: &'s Shared<'e>,
}

/// Whether a transform section runs batched, and whether it draws (a draw
/// is one value for every lane only where the run's draws are forced).
pub(super) fn shape(p: &Program, locals: &[VarId]) -> (bool, bool) {
    let mut draws = false;
    let ok = stmts_batch(&p.transform, p, locals, &mut draws);
    (ok, draws)
}

fn stmts_batch(stmts: &[IStmt], p: &Program, locals: &[VarId], draws: &mut bool) -> bool {
    for s in stmts {
        let ok = match s {
            IStmt::Scope(body) => stmts_batch(body, p, locals, draws),
            IStmt::Host(h, _) => h.gpu_read() || h.emit(),
            IStmt::Return => true,
            IStmt::Assign(place, e) => {
                let to = match place {
                    // Its own locals: a variable it keeps between calls (a
                    // state) the calls would share, in order.
                    Place::Var(v) => !p.vars[*v].is_array() && locals.contains(v),
                    Place::Pixel | Place::Color => true,
                    _ => false,
                };
                to && expr_batch(e, draws)
            }
            IStmt::If { arms, otherwise } => {
                let mut ok = true;
                for (c, b) in arms {
                    ok = ok && expr_batch(c, draws) && stmts_batch(b, p, locals, draws);
                }
                ok && stmts_batch(otherwise, p, locals, draws)
            }
            IStmt::Break | IStmt::Continue | IStmt::SetElem { .. } | IStmt::While { .. } | IStmt::Repeat { .. } | IStmt::Map { .. } => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

fn expr_batch(e: &IExpr, draws: &mut bool) -> bool {
    match &e.kind {
        IExprKind::Elem(..) => false,
        IExprKind::Predef(Predef::Draw | Predef::DrawU32) => {
            *draws = true;
            true
        }
        IExprKind::Host(Host::GridParam, args) => {
            let mut ok = true;
            for a in args {
                ok = ok && expr_batch(a, draws);
            }
            ok
        }
        IExprKind::Host(h, _) => h.gpu_read(),
        IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) | IExprKind::Var(_) | IExprKind::Param(_) | IExprKind::Predef(_) | IExprKind::FuncChoice(_) => true,
        IExprKind::Convert(a)
        | IExprKind::Unary(_, a)
        | IExprKind::PowInt(a, _)
        | IExprKind::Modulus(a)
        | IExprKind::Call(_, a)
        | IExprKind::UserCall(_, a)
        | IExprKind::UserDeriv(_, a)
        | IExprKind::UserJac(_, a, _) => expr_batch(a, draws),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => expr_batch(a, draws) && expr_batch(b, draws),
    }
}

impl Machine<'_> {
    /// [`Machine::exec`] over every lane `active` holds: a lane that
    /// returns leaves it.
    fn exec_batch(&self, stmts: &[IStmt], l: &mut Lanes, active: &mut [bool]) {
        for s in stmts {
            if !active.iter().any(|&a| a) {
                return;
            }
            match s {
                // Its `return` leaves it alone.
                IStmt::Scope(body) => {
                    let mut inner = active.to_vec();
                    self.exec_batch(body, l, &mut inner);
                }
                // A palette read or an emission: nothing the point depends on.
                IStmt::Host(..) => {}
                IStmt::Return => active.iter_mut().for_each(|a| *a = false),
                IStmt::Assign(place, e) => {
                    let v = self.eval_batch(e, l);
                    match place {
                        Place::Var(id) => l.vars[*id].assign(v, active),
                        // As the scalar run's `as_c64` and `as_f64`.
                        Place::Pixel => {
                            let v = if matches!(v, Col::C(_)) { v } else { complexes(l.n, |i| v.c(i)) };
                            l.pixel.assign(v, active)
                        }
                        Place::Color => {
                            let v = if matches!(v, Col::F(_)) { v } else { floats(l.n, |i| v.f(i)) };
                            l.color.assign(v, active)
                        }
                        _ => unreachable!("`shape` admits no other place"),
                    }
                }
                // Each lane takes the first arm whose test holds, as the
                // scalar run's; one that returns there leaves the call.
                IStmt::If { arms, otherwise } => {
                    let mut open = active.to_vec();
                    let bodies = arms.iter().map(|(c, b)| (Some(c), b)).chain(std::iter::once((None, otherwise)));
                    for (cond, body) in bodies {
                        let mut arm = match cond {
                            Some(c) => {
                                let c = self.eval_batch(c, l);
                                let arm: Vec<bool> = (0..l.n).map(|i| open[i] && c.at(i).as_bool()).collect();
                                for (o, &a) in open.iter_mut().zip(&arm) {
                                    *o &= !a;
                                }
                                arm
                            }
                            None => std::mem::take(&mut open),
                        };
                        if arm.iter().any(|&a| a) {
                            let taken = arm.clone();
                            self.exec_batch(body, l, &mut arm);
                            for i in 0..l.n {
                                if taken[i] && !arm[i] {
                                    active[i] = false;
                                }
                            }
                        }
                        if open.is_empty() || !open.iter().any(|&o| o) {
                            break;
                        }
                    }
                }
                _ => unreachable!("`shape` admits no other statement"),
            }
        }
    }

    /// [`Machine::eval`] over every lane, each by the same function: a
    /// float's and a complex's arithmetic, calls and conversions on their
    /// own columns, exactly as `binary`, `call` and `convert` take a lane
    /// of that type; anything else lane by lane through them.
    fn eval_batch(&self, e: &IExpr, l: &Lanes) -> Col {
        let (n, dialect) = (l.n, self.prog.dialect);
        match &e.kind {
            IExprKind::Host(Host::GridParam, args) => self.eval_batch(&args[3], l).map(n, |r| Value::Float(self.params.values[grid_cell(args, r.as_int())].0)),
            IExprKind::Var(id) => l.vars[*id].clone(),
            IExprKind::Predef(Predef::Pixel) => l.pixel.clone(),
            IExprKind::Predef(Predef::VarColor) => l.color.clone(),
            // A literal, a parameter, a forced draw, the call's and the
            // image's values: every lane's, as the scalar run gives it.
            IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) | IExprKind::Param(_) | IExprKind::Predef(_) | IExprKind::FuncChoice(_) | IExprKind::Host(..) => {
                Col::One(self.eval(e, l.sh))
            }
            IExprKind::Convert(a)
                if (a.ty, e.ty) == (Ty::Int, Ty::Float)
                    && matches!(&a.kind, IExprKind::Call(Builtin::Floor | Builtin::Trunc | Builtin::Ceil, x) if x.ty == Ty::Float) =>
            {
                let IExprKind::Call(f, x) = &a.kind else { unreachable!() };
                let x = self.eval_batch(x, l);
                if x.is_one() {
                    return x.map(n, |v| Value::Float(f.eval_f(v.as_f64())));
                }
                floats(n, |i| f.eval_f(x.f(i)))
            }
            IExprKind::Convert(a) => {
                let x = self.eval_batch(a, l);
                match (&x, e.ty) {
                    (Col::F(_), Ty::Float) | (Col::C(_), Ty::Complex) => x,
                    (Col::C(_), Ty::Float) => floats(n, |i| x.f(i)),
                    (Col::F(_), Ty::Complex) => complexes(n, |i| x.c(i)),
                    _ => x.map(n, |v| convert(v, e.ty, dialect)),
                }
            }
            IExprKind::MakeComplex(a, b) => {
                let (x, y) = (self.eval_batch(a, l), self.eval_batch(b, l));
                if x.is_one() && y.is_one() {
                    return x.zip(&y, n, make_complex);
                }
                complexes(n, |i| C64::new(x.f(i), y.f(i)))
            }
            IExprKind::Unary(UnOp::Neg, a) => {
                let x = self.eval_batch(a, l);
                match &x {
                    Col::F(v) => Col::F(Rc::new(v.iter().map(|t| -t).collect())),
                    Col::C(v) => Col::C(Rc::new(v.iter().map(|c| c.neg()).collect())),
                    _ => x.map(n, negate),
                }
            }
            IExprKind::Unary(UnOp::Not, a) => self.eval_batch(a, l).map(n, |v| Value::Bool(!v.as_bool())),
            IExprKind::Binary(BinOp::Mul, a, b) if e.ty == Ty::Complex && (a.is_lifted_real() || b.is_lifted_real()) => {
                let (k, z) = if a.is_lifted_real() { (a, b) } else { (b, a) };
                let (z, k) = (self.eval_batch(z, l), self.eval_batch(k, l));
                if z.is_one() && k.is_one() {
                    return z.zip(&k, n, scale_by_real);
                }
                complexes(n, |i| z.c(i).scale(k.c(i).re))
            }
            IExprKind::Binary(op, a, b) => {
                let (x, y) = (self.eval_batch(a, l), self.eval_batch(b, l));
                if x.is_one() && y.is_one() {
                    return x.zip(&y, n, |p, q| binary(*op, p, q, e.ty, dialect));
                }
                match (e.ty, op) {
                    (Ty::Float, BinOp::Add) => floats(n, |i| x.f(i) + y.f(i)),
                    (Ty::Float, BinOp::Sub) => floats(n, |i| x.f(i) - y.f(i)),
                    (Ty::Float, BinOp::Mul) => floats(n, |i| x.f(i) * y.f(i)),
                    (Ty::Float, BinOp::Div) => floats(n, |i| x.f(i) / y.f(i)),
                    (Ty::Float, BinOp::Mod) => floats(n, |i| x.f(i) % y.f(i)),
                    (Ty::Float, BinOp::Pow) => floats(n, |i| x.f(i).powf(y.f(i))),
                    (Ty::Complex, BinOp::Add) => complexes(n, |i| x.c(i).add(y.c(i))),
                    (Ty::Complex, BinOp::Sub) => complexes(n, |i| x.c(i).sub(y.c(i))),
                    (Ty::Complex, BinOp::Mul) => complexes(n, |i| x.c(i).mul(y.c(i))),
                    (Ty::Complex, BinOp::Div) => complexes(n, |i| x.c(i).div(y.c(i))),
                    (Ty::Complex, BinOp::Pow) => complexes(n, |i| x.c(i).powc(y.c(i))),
                    _ => x.zip(&y, n, |p, q| binary(*op, p, q, e.ty, dialect)),
                }
            }
            IExprKind::PowInt(a, k) => {
                let x = self.eval_batch(a, l);
                match &x {
                    Col::C(v) => Col::C(Rc::new(v.iter().map(|c| c.powi(*k)).collect())),
                    Col::F(v) => Col::F(Rc::new(v.iter().map(|t| t.powi(*k as i32)).collect())),
                    _ => x.map(n, |v| pow_int(v, *k)),
                }
            }
            IExprKind::Modulus(a) => {
                let x = self.eval_batch(a, l);
                match &x {
                    Col::C(v) => Col::F(Rc::new(v.iter().map(|c| c.norm_sqr()).collect())),
                    Col::F(v) => Col::F(Rc::new(v.iter().map(|t| t * t).collect())),
                    _ => x.map(n, modulus),
                }
            }
            IExprKind::Call(f, a) => {
                let x = self.eval_batch(a, l);
                // Fractint's round, and an int's or a bool's call: lane by lane.
                if x.is_one() || (*f == Builtin::Round && dialect == Dialect::Fractint) {
                    return x.map(n, |v| call(*f, v, e.ty, dialect));
                }
                match (&x, e.ty) {
                    (Col::F(v), Ty::Float) => Col::F(Rc::new(v.iter().map(|&t| f.eval_f(t)).collect())),
                    (Col::C(v), Ty::Float) => Col::F(Rc::new(v.iter().map(|&c| f.eval_c(c).re).collect())),
                    (Col::F(_) | Col::C(_), Ty::Complex) => complexes(n, |i| f.eval_c(x.c(i))),
                    _ => x.map(n, |v| call(*f, v, e.ty, dialect)),
                }
            }
            IExprKind::UserCall(id, a) => {
                let c = self.params.funcs[*id];
                self.eval_batch(a, l).map(n, |v| user_call(c, v))
            }
            IExprKind::UserDiff(id, a, d) => {
                let c = self.params.funcs[*id];
                self.eval_batch(a, l).zip(&self.eval_batch(d, l), n, |x, y| Value::Complex(user_diff(c, x.as_c64(), y.as_c64())))
            }
            IExprKind::UserDeriv(id, a) => {
                let c = self.params.funcs[*id];
                self.eval_batch(a, l).map(n, |v| user_deriv(c, v))
            }
            IExprKind::UserJac(id, a, b) => {
                let c = self.params.funcs[*id];
                self.eval_batch(a, l).map(n, |v| Value::Complex(user_jac(c, v.as_c64(), *b)))
            }
            IExprKind::Elem(..) => unreachable!("`shape` admits no array"),
        }
    }
}

impl VariationRun<'_> {
    /// Each of `zs` through the body as [`Self::call`] gives its point from
    /// fresh locals, in one pass (the module note). The colours, hidings
    /// and direct colours the calls write are not kept.
    pub fn call_batch(&mut self, zs: &[C64], accum: C64, weight: f64) -> Vec<C64> {
        let prog = self.m.prog;
        let (ok, draws) = *self.batch_shape.get_or_insert_with(|| shape(prog, &self.locals));
        if !ok || (draws && self.m.fixed_draw.get().is_none()) || zs.len() < 2 {
            return zs
                .iter()
                .map(|&z| {
                    self.reset_locals();
                    self.call(z, accum, weight).0
                })
                .collect();
        }
        // Every lane starts from fresh locals and the run's kept values.
        self.reset_locals();
        let mut sh = Shared::new(&self.env);
        sh.accum = accum;
        sh.weight = weight;
        sh.vcolor_in = self.color;
        sh.coefs = self.coefs;
        let n = zs.len();
        let mut l = Lanes {
            n,
            vars: self.m.vars.iter().map(|&v| Col::One(v)).collect(),
            pixel: Col::C(Rc::new(zs.to_vec())),
            color: Col::One(Value::Float(self.color)),
            sh: &sh,
        };
        let mut active = vec![true; n];
        self.m.exec_batch(&prog.transform, &mut l, &mut active);
        (0..n).map(|i| l.pixel.at(i).as_c64()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same value, a NaN the same as a NaN.
    fn same(a: C64, b: C64) -> bool {
        let s = |x: f64, y: f64| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan());
        s(a.re, b.re) && s(a.im, b.im)
    }

    /// The points every twin is called at: rings out to 4, the axes, the
    /// origin and a huge and a tiny one.
    fn points() -> Vec<C64> {
        let mut zs = vec![C64::ZERO, C64::new(1e-300, 0.0), C64::new(0.0, -1e12), C64::new(-0.0, 0.5), C64::new(1.0, 0.0), C64::new(0.0, 1.0)];
        for ring in 1..=8 {
            let r = 0.5 * ring as f64;
            for k in 0..24 {
                let a = std::f64::consts::TAU * (k as f64 + 0.37) / 24.0;
                zs.push(C64::new(r * a.cos(), r * a.sin()));
            }
        }
        zs
    }

    /// **A batch is its scalar calls** (rhai-planners E4): every twin's
    /// forward map at its defaults, at 198 points and three forced draws,
    /// one batch against a scalar call a point from fresh locals, bit for
    /// bit -- the batched twins and those that fall back alike.
    #[test]
    fn a_batch_is_its_scalar_calls() {
        let zs = points();
        let (mut batched, mut fell_back, mut skipped) = (0usize, Vec::new(), 0usize);
        let mut failures = Vec::new();
        for (name, src) in crate::variations::twins::TWINS.iter() {
            let Ok(p) = crate::lang::rhai::transformation(src) else {
                skipped += 1;
                continue;
            };
            let params = ParamValues::defaults(&p);
            let (ok, _) = shape(&p, &VariationRun::new(&p, &params, 1).locals);
            if ok {
                batched += 1;
            } else {
                fell_back.push(*name);
            }
            for u in [0.0, 0.37, 0.999] {
                let got = VariationRun::new(&p, &params, 1).with_draw(u).with_color(0.4).call_batch(&zs, C64::ZERO, 1.0);
                // A fresh run: a twin that keeps a state between calls (one
                // that falls back) gives each call in its order.
                let mut run = VariationRun::new(&p, &params, 1).with_draw(u).with_color(0.4);
                for (i, &z) in zs.iter().enumerate() {
                    run.reset_locals();
                    let want = run.call(z, C64::ZERO, 1.0).0;
                    if !same(got[i], want) {
                        failures.push(format!("{name} at {z:?}, draw {u}: batch {:?}, scalar {want:?}", got[i]));
                        break;
                    }
                }
            }
        }
        println!("{batched} twins batch; {} fall back ({}); {skipped} not a 2D transformation", fell_back.len(), fell_back.join(", "));
        assert!(batched > 500, "only {batched} batch");
        assert!(failures.is_empty(), "{} differ:\n{}", failures.len(), failures.join("\n"));
    }

    /// TIMING: julian's twin's forward map, a scalar call a point against
    /// batches of 64 to 1024.
    #[test]
    #[ignore = "timing; run with --ignored --nocapture"]
    fn time_a_batch() {
        let src = crate::variations::twins::source("julian").expect("a twin");
        let p = crate::lang::rhai::transformation(src).expect("compiles");
        let params = ParamValues::defaults(&p);
        let mut run = VariationRun::new(&p, &params, 1).with_draw(0.3);
        let zs: Vec<C64> = (0..1024).map(|i| C64::new(0.3 + 0.001 * i as f64, -0.2 + 0.0007 * i as f64)).collect();
        let reps = 2000;
        let t = std::time::Instant::now();
        let mut sink = 0.0;
        for _ in 0..reps / 4 {
            for &z in &zs {
                run.reset_locals();
                sink += run.call(z, C64::ZERO, 1.0).0.re;
            }
        }
        let scalar = t.elapsed().as_secs_f64() / (reps / 4 * zs.len()) as f64;
        println!("scalar: {:.3} us a call", scalar * 1e6);
        for size in [16usize, 64, 256, 1024] {
            let t = std::time::Instant::now();
            let mut done = 0usize;
            while done < reps * 256 {
                for chunk in zs.chunks(size) {
                    sink += run.call_batch(chunk, C64::ZERO, 1.0)[0].re;
                    done += chunk.len();
                }
            }
            let each = t.elapsed().as_secs_f64() / done as f64;
            println!("batch of {size}: {:.3} us a point, {:.1}x", each * 1e6, scalar / each);
        }
        assert!(sink.is_finite() || sink.is_nan());
    }
}
