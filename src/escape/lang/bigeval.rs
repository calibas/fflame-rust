//! A program run in big floats: the reference orbit of the perturbed
//! path (plan, phase 3).
//!
//! The f64 interpreter's semantics at the reference's precision: every
//! float and complex value is a [`BigFloat`] / [`BigComplex`] of the
//! orbit's limb count, ints and bools stay exact. The perturbed path
//! keeps only the iterate of this run, each iteration (UF: "all
//! intermediate values for the main Z variable"); the pixels follow it
//! by the formula's own perturbation equations.
//!
//! What a big float cannot answer ends the orbit ([`Stop`]): a division
//! by zero, a real `sqrt` or `log` off its domain, a value past the
//! format. The perturbed path rebases its pixels onto the orbit's start
//! there, as it does past any orbit's end.

use super::ast::{BinOp, UnOp};
use super::builtins::{Builtin, USER_FUNCTIONS};
use super::check::LOOP_CAP;
use super::interp::{Env, ParamValues};
use super::ir::*;
use super::Dialect;
use crate::escape::bigfloat::{BigComplex, BigFloat};

/// The orbit store's encoding of a big float: sign, exponent, limbs.
pub(crate) fn put_float(out: &mut Vec<u8>, x: &BigFloat) {
    out.push(u8::from(x.neg));
    out.extend_from_slice(&x.exp.to_le_bytes());
    out.extend_from_slice(&(x.limbs.len() as u32).to_le_bytes());
    for l in &x.limbs {
        out.extend_from_slice(&l.to_le_bytes());
    }
}

pub(crate) fn put_complex(out: &mut Vec<u8>, z: &BigComplex) {
    put_float(out, &z.re);
    put_float(out, &z.im);
}

fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    if bytes.len() < n {
        return None;
    }
    let (head, tail) = bytes.split_at(n);
    *bytes = tail;
    Some(head)
}

pub(crate) fn take_u32(bytes: &mut &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(take(bytes, 4)?.try_into().ok()?))
}

pub(crate) fn take_float(bytes: &mut &[u8]) -> Option<BigFloat> {
    let neg = take(bytes, 1)?[0] != 0;
    let exp = i64::from_le_bytes(take(bytes, 8)?.try_into().ok()?);
    let n = take_u32(bytes)? as usize;
    if n == 0 || n > 4096 {
        return None;
    }
    let mut limbs = crate::escape::bigfloat::Limbs::with_capacity(n);
    for _ in 0..n {
        limbs.push(u64::from_le_bytes(take(bytes, 8)?.try_into().ok()?));
    }
    Some(BigFloat { neg, exp, limbs })
}

pub(crate) fn take_complex(bytes: &mut &[u8]) -> Option<BigComplex> {
    Some(BigComplex { re: take_float(bytes)?, im: take_float(bytes)? })
}

/// A variable's value, tagged by its type.
fn put_value(out: &mut Vec<u8>, v: &BigValue) {
    match v {
        BigValue::Bool(b) => {
            out.push(0);
            out.push(u8::from(*b));
        }
        BigValue::Int(i) => {
            out.push(1);
            out.extend_from_slice(&i.to_le_bytes());
        }
        BigValue::Float(x) => {
            out.push(2);
            put_float(out, x);
        }
        BigValue::Complex(c) => {
            out.push(3);
            put_complex(out, c);
        }
    }
}

fn take_value(bytes: &mut &[u8]) -> Option<BigValue> {
    let tag = take(bytes, 1)?[0];
    Some(match tag {
        0 => BigValue::Bool(take(bytes, 1)?[0] != 0),
        1 => BigValue::Int(i32::from_le_bytes(take(bytes, 4)?.try_into().ok()?)),
        2 => BigValue::Float(take_float(bytes)?),
        3 => BigValue::Complex(take_complex(bytes)?),
        _ => return None,
    })
}

/// A real, whole `w` of modest size, as an integer.
fn whole(w: &BigComplex) -> Option<i64> {
    if !w.im.is_zero() {
        return None;
    }
    let x = w.re.to_f64();
    if !(x.abs() <= 65_536.0) || x != x.round() {
        return None;
    }
    let n = x as i64;
    w.re.sub(&BigFloat::from_f64(x, w.re.n_limbs())).is_zero().then_some(n)
}

/// Why the orbit cannot go on: a value a big float has no answer for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stop;

type R<T> = Result<T, Stop>;

#[derive(Clone, Debug)]
pub enum BigValue {
    Bool(bool),
    Int(i32),
    Float(BigFloat),
    Complex(BigComplex),
}

/// A program's state, run at the reference point.
#[derive(Clone, Debug)]
pub struct BigMachine {
    prog: std::sync::Arc<Program>,
    params: ParamValues,
    env: Env,
    n: usize,
    /// The pass index ([`Predef::Pass`]): passes run.
    pass: u32,
    pixel: BigComplex,
    vars: Vec<BigValue>,
    /// Each array variable's elements; empty for a scalar.
    arrays: Vec<Vec<BigValue>>,
    z: BigComplex,
    pi: BigFloat,
}

impl BigMachine {
    /// The program at `pixel`, `n_limbs` wide: its `global:` and
    /// `init:` run, so [`BigMachine::z`] is the orbit's start.
    pub fn new(prog: std::sync::Arc<Program>, params: ParamValues, env: Env, pixel: BigComplex, n_limbs: usize) -> R<Self> {
        let n = n_limbs.max(1);
        let pi = BigFloat::atan2(&BigFloat::zero(n), &BigFloat::from_f64(-1.0, n));
        let vars = prog.vars.iter().map(|v| zero_of(v.ty, n)).collect();
        let arrays = prog.vars.iter().map(|v| if v.is_array() { vec![zero_of(v.ty, n); v.len()] } else { Vec::new() }).collect();
        let mut m = BigMachine { params, env, n, pass: 0, pixel, vars, arrays, z: BigComplex::zero(n), pi, prog };
        let prog = m.prog.clone();
        m.exec(&prog.global_body)?;
        m.exec(&prog.init)?;
        Ok(m)
    }

    /// Start the passes at `pass` (a hybrid's reference for one phase
    /// of its cycle, step 5b): the init has run, the loop has not.
    pub fn set_pass(&mut self, pass: u32) {
        self.pass = pass;
    }

    /// The iterate.
    /// The values the derived equations carry besides z
    /// (`Program::carried`), as they stand: what the reference orbit
    /// records beside z each pass. A float is a complex with a zero
    /// imaginary part.
    pub fn carried(&self) -> Vec<BigComplex> {
        self.prog
            .carried
            .iter()
            .map(|c| match &self.vars[c.var] {
                BigValue::Complex(z) => z.clone(),
                BigValue::Float(x) => BigComplex { re: x.clone(), im: x.sub(x) },
                _ => self.z.sub(&self.z),
            })
            .collect()
    }

    pub fn z(&self) -> &BigComplex {
        &self.z
    }

    /// The machine's state as the orbit store keeps it (plan, phase 4,
    /// the speed work): its pass, its iterate and its variables, so a
    /// stored custom reference deepens as the one that was saved. The
    /// program, parameters and image come back from the reference's
    /// identity, which names them.
    pub fn save_state(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.pass.to_le_bytes());
        put_complex(out, &self.z);
        out.extend_from_slice(&(self.vars.len() as u32).to_le_bytes());
        for v in &self.vars {
            put_value(out, v);
        }
        // The arrays' elements, when the program has any (a stored state
        // of one without reads as it was written).
        if self.prog.vars.iter().any(|v| v.is_array()) {
            for a in &self.arrays {
                for v in a {
                    put_value(out, v);
                }
            }
        }
    }

    /// Restore what [`Self::save_state`] wrote, advancing `bytes` past it;
    /// `None` when it does not fit this program (a miss for the store).
    pub fn load_state(&mut self, bytes: &mut &[u8]) -> Option<()> {
        self.pass = take_u32(bytes)?;
        self.z = take_complex(bytes)?;
        let n = take_u32(bytes)? as usize;
        if n != self.vars.len() {
            return None;
        }
        for v in &mut self.vars {
            *v = take_value(bytes)?;
        }
        for a in &mut self.arrays {
            for v in a {
                *v = take_value(bytes)?;
            }
        }
        Some(())
    }

    /// A variable's value as a complex (a float's with a zero imaginary
    /// part): a derivative program's `d_z` for the nucleus search
    /// (`lang::periodic`).
    pub fn complex_var(&self, v: VarId) -> BigComplex {
        match &self.vars[v] {
            BigValue::Complex(z) => z.clone(),
            BigValue::Float(x) => BigComplex { re: x.clone(), im: BigFloat::zero(self.n) },
            _ => BigComplex::zero(self.n),
        }
    }

    /// One loop pass; then whether the test holds (iterate again).
    pub fn step(&mut self) -> R<bool> {
        let prog = self.prog.clone();
        self.exec(&prog.loop_body)?;
        self.pass += 1;
        match &prog.bailout {
            Some(t) => Ok(self.eval(t)?.truth(prog.dialect)),
            None => Ok(true),
        }
    }

    fn exec(&mut self, stmts: &[IStmt]) -> R<()> {
        for s in stmts {
            match s {
                IStmt::Assign(place, e) => {
                    let v = self.eval(e)?;
                    match place {
                        Place::Var(id) => self.vars[*id] = v,
                        Place::Z => self.z = self.to_c(v),
                        // A formula writes neither; the checker keeps it so.
                        Place::Index | Place::Solid | Place::DZ | Place::CreaseHeight | Place::CreaseSlope | Place::Pixel | Place::Color | Place::PixelZ => {}
                    }
                }
                IStmt::SetElem { var, index, value } => {
                    let v = self.eval(value)?;
                    let i = self.element(*var, index)?;
                    self.arrays[*var][i] = v;
                }
                IStmt::If { arms, otherwise } => {
                    let mut taken = false;
                    for (c, b) in arms {
                        if self.eval(c)?.truth(self.prog.dialect) {
                            self.exec(b)?;
                            taken = true;
                            break;
                        }
                    }
                    if !taken {
                        self.exec(otherwise)?;
                    }
                }
                IStmt::While { cond, body } => {
                    let mut k = 0;
                    while k < LOOP_CAP && self.eval(cond)?.truth(self.prog.dialect) {
                        self.exec(body)?;
                        k += 1;
                    }
                }
                IStmt::Repeat { body, cond } => {
                    let mut k = 0;
                    loop {
                        self.exec(body)?;
                        k += 1;
                        if k >= LOOP_CAP || self.eval(cond)?.truth(self.prog.dialect) {
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// An element's place: the flattened index clamped to the array, as
    /// the GPU's.
    fn element(&self, v: VarId, index: &IExpr) -> R<usize> {
        let i = match self.eval(index)? {
            BigValue::Int(i) => i,
            other => self.to_f(other).to_f64() as i32,
        };
        Ok((i.max(0) as usize).min(self.arrays[v].len().saturating_sub(1)))
    }

    fn f(&self, v: f64) -> BigFloat {
        BigFloat::from_f64(v, self.n)
    }

    fn c(&self, re: f64, im: f64) -> BigComplex {
        BigComplex::from_f64(re, im, self.n)
    }

    fn to_c(&self, v: BigValue) -> BigComplex {
        match v {
            BigValue::Complex(c) => c,
            BigValue::Float(f) => BigComplex { re: f, im: BigFloat::zero(self.n) },
            BigValue::Int(i) => self.c(i as f64, 0.0),
            BigValue::Bool(b) => self.c(if b { 1.0 } else { 0.0 }, 0.0),
        }
    }

    fn to_f(&self, v: BigValue) -> BigFloat {
        match v {
            BigValue::Complex(c) => c.re,
            BigValue::Float(f) => f,
            BigValue::Int(i) => self.f(i as f64),
            BigValue::Bool(b) => self.f(if b { 1.0 } else { 0.0 }),
        }
    }

    fn eval(&self, e: &IExpr) -> R<BigValue> {
        Ok(match &e.kind {
            IExprKind::Bool(b) => BigValue::Bool(*b),
            IExprKind::Int(i) => BigValue::Int(*i as i32),
            IExprKind::Float(f) => BigValue::Float(self.f(*f)),
            IExprKind::Complex(re, im) => BigValue::Complex(self.c(*re, *im)),
            IExprKind::Var(v) => self.vars[*v].clone(),
            IExprKind::Elem(v, index) => self.arrays[*v][self.element(*v, index)?].clone(),
            IExprKind::Param(id) => {
                let (re, im) = self.params.values[*id];
                match self.prog.params[*id].ty() {
                    Ty::Bool => BigValue::Bool(re != 0.0),
                    Ty::Int => BigValue::Int(re.round() as i32),
                    Ty::Float => BigValue::Float(self.f(re)),
                    Ty::Complex => BigValue::Complex(self.c(re, im)),
                }
            }
            IExprKind::Predef(p) => self.predef(*p),
            IExprKind::Convert(a) => {
                let v = self.eval(a)?;
                match e.ty {
                    Ty::Bool => BigValue::Bool(v.truth(self.prog.dialect)),
                    Ty::Int => BigValue::Int(match v {
                        BigValue::Int(i) => i,
                        BigValue::Bool(b) => i32::from(b),
                        other => self.to_f(other).to_f64() as i32,
                    }),
                    Ty::Float => BigValue::Float(self.to_f(v)),
                    Ty::Complex => BigValue::Complex(self.to_c(v)),
                }
            }
            IExprKind::MakeComplex(a, b) => {
                let re = self.to_f(self.eval(a)?);
                let im = self.to_f(self.eval(b)?);
                BigValue::Complex(BigComplex { re, im })
            }
            IExprKind::Unary(UnOp::Neg, a) => match self.eval(a)? {
                BigValue::Int(i) => BigValue::Int(i.wrapping_neg()),
                BigValue::Float(f) => BigValue::Float(f.neg()),
                BigValue::Complex(c) => BigValue::Complex(cneg(&c)),
                b => b,
            },
            IExprKind::Unary(UnOp::Not, a) => BigValue::Bool(!self.eval(a)?.truth(self.prog.dialect)),
            // A real factor scales, as both other back ends compute it.
            IExprKind::Binary(BinOp::Mul, a, b) if e.ty == Ty::Complex && (a.is_lifted_real() || b.is_lifted_real()) => {
                let (k, z) = if a.is_lifted_real() { (a, b) } else { (b, a) };
                let k = self.to_f(self.eval(k)?);
                let z = self.to_c(self.eval(z)?);
                BigValue::Complex(BigComplex { re: z.re.mul(&k), im: z.im.mul(&k) })
            }
            IExprKind::Binary(op, a, b) => self.binary(*op, self.eval(a)?, self.eval(b)?, e.ty)?,
            IExprKind::PowInt(a, n) => match self.eval(a)? {
                BigValue::Complex(c) => BigValue::Complex(self.cpowi(&c, *n)?),
                v => {
                    let x = self.to_f(v);
                    let p = self.cpowi(&BigComplex { re: x, im: BigFloat::zero(self.n) }, *n)?;
                    BigValue::Float(p.re)
                }
            },
            IExprKind::Modulus(a) => match self.eval(a)? {
                BigValue::Complex(c) => BigValue::Float(c.norm_sqr()),
                v => {
                    let x = self.to_f(v);
                    BigValue::Float(x.mul(&x))
                }
            },
            IExprKind::Call(f, a) => self.call(*f, self.eval(a)?, e.ty)?,
            IExprKind::UserCall(id, a) => {
                let f = USER_FUNCTIONS[self.params.funcs[*id].min(USER_FUNCTIONS.len() - 1)];
                let z = self.to_c(self.eval(a)?);
                BigValue::Complex(self.ccall(f, &z)?)
            }
            IExprKind::FuncChoice(id) => BigValue::Int(self.params.funcs[*id] as i32),
            // The derived equations', never in a reference program.
            IExprKind::UserDiff(..) => return Err(Stop),
            // The derivative pass's, never in a reference program.
            IExprKind::UserDeriv(..) | IExprKind::UserJac(..) => return Err(Stop),
        })
    }

    fn predef(&self, p: Predef) -> BigValue {
        let env = &self.env;
        match p {
            Predef::Pixel => BigValue::Complex(self.pixel.clone()),
            Predef::Z => BigValue::Complex(self.z.clone()),
            Predef::MaxIter => BigValue::Int(env.maxiter as i32),
            Predef::Pass => BigValue::Int(self.pass as i32),
            Predef::Pi => BigValue::Float(self.pi.clone()),
            Predef::E => BigValue::Float(self.f(1.0).exp()),
            Predef::Center => BigValue::Complex(self.c(env.center.re, env.center.im)),
            Predef::Magn => BigValue::Float(self.f(env.magn)),
            Predef::Width => BigValue::Int(env.width),
            Predef::Height => BigValue::Int(env.height),
            Predef::ScrnMax => BigValue::Complex(self.c(env.width as f64, env.height as f64)),
            Predef::IsMand => BigValue::Complex(self.c(if env.julia { 0.0 } else { 1.0 }, 0.0)),
            Predef::MagXMag => BigValue::Complex(self.c(env.magn, 1.0)),
            Predef::RotSkew => BigValue::Complex(self.c(env.rotation.to_degrees(), 0.0)),
            Predef::JuliaC => BigValue::Complex(self.c(env.julia_c.re, env.julia_c.im)),
            // A pixel's own place on the screen has no meaning at the
            // reference point; `lang::perturb` refuses formulas that
            // read them. Zero keeps this total.
            Predef::X | Predef::Y => BigValue::Int(0),
            Predef::ScreenPixel | Predef::Random | Predef::WhiteSq | Predef::DZ | Predef::DPixel => {
                BigValue::Complex(BigComplex::zero(self.n))
            }
            // A colouring's.
            Predef::NumIter => BigValue::Int(0),
            Predef::Index => BigValue::Float(BigFloat::zero(self.n)),
            Predef::Solid => BigValue::Bool(false),
            // The creases section's, which no orbit runs.
            Predef::CreasePattern => BigValue::Int(0),
            Predef::CreaseDecay | Predef::CreaseSign | Predef::CreaseReach | Predef::CreaseHeight => BigValue::Float(BigFloat::zero(self.n)),
            Predef::CreaseSlope => BigValue::Complex(BigComplex::zero(self.n)),
            // A draw has no reference value (`lang::perturb` refuses a
            // program that draws); a flame variation's, no orbit.
            Predef::Draw | Predef::Weight | Predef::VarColor | Predef::PixelZ | Predef::AccumZ => BigValue::Float(BigFloat::zero(self.n)),
            Predef::Accum => BigValue::Complex(BigComplex::zero(self.n)),
        }
    }

    fn binary(&self, op: BinOp, l: BigValue, r: BigValue, ty: Ty) -> R<BigValue> {
        use BinOp::*;
        let dialect = self.prog.dialect;
        Ok(match op {
            And => BigValue::Bool(l.truth(dialect) && r.truth(dialect)),
            Or => BigValue::Bool(l.truth(dialect) || r.truth(dialect)),
            BitAnd | BitOr | BitXor | Shl | Shr => match (l, r) {
                (BigValue::Int(a), BigValue::Int(b)) => BigValue::Int(super::interp::bits(op, a, b)),
                _ => return Err(Stop),
            },
            Eq | Ne => {
                let eq = match (&l, &r) {
                    (BigValue::Complex(a), BigValue::Complex(b)) if dialect == Dialect::Fractint => sign(&a.re.sub(&b.re)) == 0,
                    (BigValue::Complex(a), BigValue::Complex(b)) => sign(&a.re.sub(&b.re)) == 0 && sign(&a.im.sub(&b.im)) == 0,
                    (BigValue::Bool(a), BigValue::Bool(b)) => a == b,
                    _ => sign(&self.to_f(l.clone()).sub(&self.to_f(r.clone()))) == 0,
                };
                BigValue::Bool(eq == (op == Eq))
            }
            // Complex values order by their real parts.
            Lt | Le | Gt | Ge => {
                let d = sign(&self.to_f(l).sub(&self.to_f(r)));
                BigValue::Bool(match op {
                    Lt => d < 0,
                    Le => d <= 0,
                    Gt => d > 0,
                    _ => d >= 0,
                })
            }
            // Only in derived perturbation equations, which the reference
            // never runs; |c + d| - |c| all the same.
            DiffAbs => {
                let (c, d) = (self.to_f(l), self.to_f(r));
                BigValue::Float(c.add(&d).abs().sub(&c.abs()))
            }
            Add | Sub | Mul | Div | Mod | Pow => match ty {
                Ty::Int => {
                    let (BigValue::Int(a), BigValue::Int(b)) = (&l, &r) else { return Err(Stop) };
                    let (a, b) = (*a, *b);
                    BigValue::Int(match op {
                        Add => a.wrapping_add(b),
                        Sub => a.wrapping_sub(b),
                        Mul => a.wrapping_mul(b),
                        Mod => {
                            if b == 0 {
                                0
                            } else {
                                a.wrapping_rem(b)
                            }
                        }
                        _ => return Err(Stop),
                    })
                }
                Ty::Float => {
                    let (a, b) = (self.to_f(l), self.to_f(r));
                    BigValue::Float(match op {
                        Add => a.add(&b),
                        Sub => a.sub(&b),
                        Mul => a.mul(&b),
                        Div => a.mul(&recip(&b)?),
                        // Float `%`: the f64 one on the values (rare in
                        // an iterate, and the remainder of big numbers
                        // has no use here).
                        Mod => self.f(a.to_f64() % b.to_f64()),
                        Pow => self.fpow(&a, &b)?,
                        _ => return Err(Stop),
                    })
                }
                _ => {
                    let (a, b) = (self.to_c(l), self.to_c(r));
                    BigValue::Complex(match op {
                        Add => a.add(&b),
                        Sub => a.sub(&b),
                        Mul => a.mul(&b),
                        Div => cdiv(&a, &b)?,
                        Pow => self.cpow(&a, &b)?,
                        _ => return Err(Stop),
                    })
                }
            },
        })
    }

    /// `a^b` for floats: the real power, defined for a negative base
    /// at a whole exponent, as the other back ends compute it.
    fn fpow(&self, a: &BigFloat, b: &BigFloat) -> R<BigFloat> {
        if b.is_zero() {
            return Ok(self.f(1.0));
        }
        // A whole exponent by repeated squaring, as `cpow`.
        if let Some(n) = whole(&BigComplex { re: b.clone(), im: BigFloat::zero(self.n) }) {
            let z = self.cpowi(&BigComplex { re: a.clone(), im: BigFloat::zero(self.n) }, n)?;
            return Ok(z.re);
        }
        if a.is_zero() {
            return Ok(BigFloat::zero(self.n));
        }
        if a.neg {
            let bf = b.to_f64();
            if bf != bf.floor() || bf.abs() >= 16_777_216.0 {
                return Err(Stop);
            }
            let m = a.abs().powf(b);
            return Ok(if (bf as i64) & 1 == 1 { m.neg() } else { m });
        }
        Ok(a.powf(b))
    }

    /// `z^n` by repeated squaring; negative powers invert.
    fn cpowi(&self, z: &BigComplex, n: i64) -> R<BigComplex> {
        let mut base = if n < 0 { cdiv(&self.c(1.0, 0.0), z)? } else { z.clone() };
        let mut e = n.unsigned_abs();
        let mut acc = self.c(1.0, 0.0);
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.mul(&base);
            }
            e >>= 1;
            if e > 0 {
                base = base.mul(&base);
            }
        }
        Ok(acc)
    }

    /// `z^w = exp(w log z)`, and 0 at the origin; a whole real `w` (a
    /// power given as a parameter, the Multibrot's) by repeated squaring,
    /// the same value without the exponential's cost -- a pass of the
    /// Multibrot's twin took 79 us through `exp` and `log`.
    fn cpow(&self, z: &BigComplex, w: &BigComplex) -> R<BigComplex> {
        if let Some(n) = whole(w) {
            return self.cpowi(z, n);
        }
        if z.is_zero() {
            return Ok(BigComplex::zero(self.n));
        }
        Ok(self.cexp(&w.mul(&clog(z)?)))
    }

    fn cexp(&self, z: &BigComplex) -> BigComplex {
        let e = z.re.exp();
        let (s, c) = z.im.sin_cos();
        BigComplex { re: e.mul(&c), im: e.mul(&s) }
    }

    /// `(cosh y, sinh y)`.
    fn cosh_sinh(&self, y: &BigFloat) -> R<(BigFloat, BigFloat)> {
        let ey = y.exp();
        let eny = recip(&ey)?;
        Ok((ey.add(&eny).mul_pow2(-1), ey.sub(&eny).mul_pow2(-1)))
    }

    fn csqrt(&self, z: &BigComplex) -> R<BigComplex> {
        let r = z.norm_sqr().sqrt();
        if r.is_zero() {
            return Ok(BigComplex::zero(self.n));
        }
        let t = r.add(&z.re.abs()).mul_pow2(-1).sqrt();
        let half_over_t = recip(&t)?.mul_pow2(-1);
        Ok(if !z.re.neg {
            BigComplex { re: t, im: z.im.mul(&half_over_t) }
        } else {
            let t = if z.im.neg { t.neg() } else { t };
            BigComplex { re: z.im.abs().mul(&half_over_t), im: t }
        })
    }

    fn ccall(&self, f: Builtin, z: &BigComplex) -> R<BigComplex> {
        use Builtin::*;
        let n = self.n;
        let one = self.c(1.0, 0.0);
        let i = self.c(0.0, 1.0);
        let real = |v: BigFloat| BigComplex { re: v, im: BigFloat::zero(n) };
        Ok(match f {
            Sqr => z.mul(z),
            Sqrt => self.csqrt(z)?,
            Log => clog(z)?,
            Exp => self.cexp(z),
            Abs => BigComplex { re: z.re.abs(), im: z.im.abs() },
            Cabs => real(z.norm_sqr().sqrt()),
            Recip => cdiv(&one, z)?,
            Ident => z.clone(),
            Zero => BigComplex::zero(n),
            F32 => self.c(z.re.to_f64() as f32 as f64, z.im.to_f64() as f32 as f64),
            Sin => {
                let (s, c) = z.re.sin_cos();
                let (ch, sh) = self.cosh_sinh(&z.im)?;
                BigComplex { re: s.mul(&ch), im: c.mul(&sh) }
            }
            Cos => {
                let (s, c) = z.re.sin_cos();
                let (ch, sh) = self.cosh_sinh(&z.im)?;
                BigComplex { re: c.mul(&ch), im: s.mul(&sh).neg() }
            }
            Sinh => {
                let (s, c) = z.im.sin_cos();
                let (ch, sh) = self.cosh_sinh(&z.re)?;
                BigComplex { re: sh.mul(&c), im: ch.mul(&s) }
            }
            Cosh => {
                let (s, c) = z.im.sin_cos();
                let (ch, sh) = self.cosh_sinh(&z.re)?;
                BigComplex { re: ch.mul(&c), im: sh.mul(&s) }
            }
            Tan => cdiv(&self.ccall(Sin, z)?, &self.ccall(Cos, z)?)?,
            Cotan => cdiv(&self.ccall(Cos, z)?, &self.ccall(Sin, z)?)?,
            Tanh => cdiv(&self.ccall(Sinh, z)?, &self.ccall(Cosh, z)?)?,
            Cotanh => cdiv(&self.ccall(Cosh, z)?, &self.ccall(Sinh, z)?)?,
            // The f64 interpreter's forms (builtins.rs), principal cuts.
            Asin => cneg(&i).mul(&clog(&i.mul(z).add(&self.csqrt(&one.sub(&z.mul(z)))?))?),
            Acos => real(self.pi.mul_pow2(-1)).sub(&self.ccall(Asin, z)?),
            Atan => {
                let half_i = BigComplex { re: BigFloat::zero(n), im: self.f(0.5) };
                half_i.mul(&clog(&cdiv(&i.add(z), &i.sub(z))?)?)
            }
            Asinh => clog(&z.add(&self.csqrt(&z.mul(z).add(&one))?))?,
            Acosh => clog(&z.add(&self.csqrt(&z.add(&one))?.mul(&self.csqrt(&z.sub(&one))?)))?,
            Atanh => {
                let l = clog(&cdiv(&one.add(z), &one.sub(z))?)?;
                BigComplex { re: l.re.mul_pow2(-1), im: l.im.mul_pow2(-1) }
            }
            Atan2 => real(BigFloat::atan2(&z.im, &z.re)),
            Round => self.round_each(z, |x| x.round()),
            Trunc => self.round_each(z, f64::trunc),
            Ceil => self.round_each(z, f64::ceil),
            Floor => self.round_each(z, f64::floor),
            Real => real(z.re.clone()),
            Imag => real(z.im.clone()),
            Conj => BigComplex { re: z.re.clone(), im: z.im.neg() },
            Flip => BigComplex { re: z.im.clone(), im: z.re.clone() },
            Cosxx => {
                let c = self.ccall(Cos, z)?;
                BigComplex { re: c.re, im: c.im.neg() }
            }
            IsInf | IsNan => BigComplex::zero(n),
            // Exact enough in big floats as the plain forms.
            Expm1 => self.cexp(z).sub(&one),
            Log1p => clog(&one.add(z))?,
        })
    }

    /// A rounding of each part, through f64: exact while the part's
    /// whole number fits f64, which is every value an iterate rounds.
    fn round_each(&self, z: &BigComplex, f: impl Fn(f64) -> f64) -> BigComplex {
        self.c(f(z.re.to_f64()), f(z.im.to_f64()))
    }

    fn call(&self, f: Builtin, arg: BigValue, ty: Ty) -> R<BigValue> {
        use Builtin::*;
        let dialect = self.prog.dialect;
        if f == Round && dialect == Dialect::Fractint {
            let r = |x: f64| (x + 0.5).floor();
            return Ok(match (arg, ty) {
                (BigValue::Int(i), Ty::Int) => BigValue::Int(i),
                (v, Ty::Int) => BigValue::Int(r(self.to_f(v).to_f64()) as i32),
                (v, _) => {
                    let c = self.to_c(v);
                    BigValue::Complex(self.c(r(c.re.to_f64()), r(c.im.to_f64())))
                }
            });
        }
        Ok(match (arg, ty) {
            (_, Ty::Bool) => BigValue::Bool(false),
            (BigValue::Int(i), Ty::Int) => BigValue::Int(match f {
                Sqr => i.wrapping_mul(i),
                Abs => i.wrapping_abs(),
                Zero => 0,
                _ => i,
            }),
            (v, Ty::Int) => BigValue::Int(f.eval_f(self.to_f(v).to_f64()) as i32),
            (BigValue::Complex(c), Ty::Float) => BigValue::Float(self.ccall(f, &c)?.re),
            (v, Ty::Float) => {
                let x = self.to_f(v);
                // The real functions off their domain have no value
                // (the complex ones would answer with another).
                let past_one = x.cmp_abs(&self.f(1.0));
                let off = match f {
                    Sqrt => x.neg,
                    Log => x.neg || x.is_zero(),
                    Asin | Acos => past_one == std::cmp::Ordering::Greater,
                    Atanh => past_one != std::cmp::Ordering::Less,
                    Acosh => sign(&x.sub(&self.f(1.0))) < 0,
                    _ => false,
                };
                if off {
                    return Err(Stop);
                }
                // A real log is the real one (the lifted complex one
                // would square its argument for its test of zero).
                if f == Log {
                    return Ok(BigValue::Float(x.ln()));
                }
                let lifted = BigComplex { re: x, im: BigFloat::zero(self.n) };
                BigValue::Float(self.ccall(f, &lifted)?.re)
            }
            (v, _) => BigValue::Complex(self.ccall(f, &self.to_c(v))?),
        })
    }
}

impl BigValue {
    /// Whether a value tests true: UF any nonzero part, Fractint the
    /// real part alone.
    fn truth(&self, dialect: Dialect) -> bool {
        match self {
            BigValue::Bool(b) => *b,
            BigValue::Int(i) => *i != 0,
            BigValue::Float(f) => !f.is_zero(),
            BigValue::Complex(c) if dialect == Dialect::Fractint => !c.re.is_zero(),
            BigValue::Complex(c) => !(c.re.is_zero() && c.im.is_zero()),
        }
    }
}

fn zero_of(t: Ty, n: usize) -> BigValue {
    match t {
        Ty::Bool => BigValue::Bool(false),
        Ty::Int => BigValue::Int(0),
        Ty::Float => BigValue::Float(BigFloat::zero(n)),
        Ty::Complex => BigValue::Complex(BigComplex::zero(n)),
    }
}

/// -1, 0 or 1.
fn sign(x: &BigFloat) -> i32 {
    if x.is_zero() {
        0
    } else if x.neg {
        -1
    } else {
        1
    }
}

fn recip(x: &BigFloat) -> R<BigFloat> {
    if x.is_zero() {
        Err(Stop)
    } else {
        Ok(x.recip())
    }
}

fn cneg(z: &BigComplex) -> BigComplex {
    BigComplex { re: z.re.neg(), im: z.im.neg() }
}

fn cdiv(a: &BigComplex, b: &BigComplex) -> R<BigComplex> {
    if b.is_zero() {
        return Err(Stop);
    }
    Ok(a.div(b))
}

/// The principal log; the origin, which has none, ends the orbit.
/// The language's complex log, as the GPU's `uf_clog` takes it: no
/// sentinel near zero (`BigComplex::ln`'s is the built-in Ducks
/// shader's, -34.5 under |z|^2 1e-30, which for a real `ln(x)` lifted
/// to complex fired at x 1e-15 and parted the reference from the pixels
/// wherever an orbit grazed zero); none at zero itself.
fn clog(z: &BigComplex) -> R<BigComplex> {
    if z.is_zero() {
        return Err(Stop);
    }
    Ok(BigComplex { re: z.norm_sqr().ln().mul_pow2(-1), im: BigFloat::atan2(&z.im, &z.re) })
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::check::check;
    use super::super::interp::run;
    use super::super::parser::parse_file;
    use super::super::EntryKind;
    use super::*;

    fn program(src: &str) -> std::sync::Arc<Program> {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        std::sync::Arc::new(p.unwrap())
    }

    /// The big run is the f64 run, to f64's reach: every function and
    /// operator the language has, over a few iterations.
    #[test]
    fn a_big_run_is_the_f64_run() {
        let srcs = [
            "M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}",
            "F {\ninit:\n z = #pixel\nloop:\n complex w = z * 0.5 + (0.3, 0.2)\n float x = real(w) + 1.5\n\
             z = sin(w) + cos(w) + tan(w) * 0.1 + cotan(w) * 0.01 + sinh(w) + cosh(w) * 0.5 + tanh(w) \
             + cotanh(w) * 0.01 + asin(w) + acos(w) * 0.5 + atan(w) + asinh(w) + acosh(w) * 0.5 \
             + atanh(w) * 0.5 + sqrt(w) + log(w) * 0.5 + exp(w) * 0.1 + abs(w) + cabs(w) + recip(w) * 0.01 \
             + conj(w) + flip(w) + atan2(w) + cosxx(w) * 0.1 + w^(0.5, 0.25) * 0.1 + w^3 * 0.2 + (1.5)^w * 0.01\n\
             z = z * 0.01 + sqrt(x) + log(x) + exp(-x) + sin(x) + atan(x) + x^2.5 * 0.01 + #pixel\n\
             int k = 7 % 3\n if k == 1 && |z| > 0\n  z = z / (2, 1)\n endif\nbailout:\n |z| < 1e6\n}",
        ];
        for src in srcs {
            let p = program(src);
            let params = ParamValues::defaults(&p);
            for &(x, y) in &[(0.3, -0.2), (-0.4, 0.1), (0.1, 0.45)] {
                let px = C64::new(x, y);
                let steps = 6;
                let env = Env::at(px, steps);
                let mut m = BigMachine::new(p.clone(), params.clone(), env, BigComplex::from_f64(x, y, 3), 3).expect("starts");
                for _ in 0..steps {
                    if !m.step().expect("steps") {
                        break;
                    }
                }
                let want = run(&p, &params, None, &env).z;
                let got = C64::new(m.z().re.to_f64(), m.z().im.to_f64());
                let err = got.sub(want).abs() / want.abs().max(1.0);
                assert!(err < 1e-12, "{} at ({x}, {y}): big {got:?}, f64 {want:?}", p.name);
            }
        }
    }

    /// Past f64: the Mandelbrot iterate at a point given to 60 digits
    /// keeps them -- inside the main cardioid, the orbits at c and at
    /// c + 1e-40 part and stay close, where f64 sees one point.
    #[test]
    fn a_big_run_keeps_the_digits_f64_drops() {
        let p = program("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}");
        let params = ParamValues::defaults(&p);
        let n = 4;
        let c = BigComplex::from_f64(-0.2, 0.1, n);
        let tiny = BigComplex::from_f64(1e-40, 0.0, n);
        let env = Env::at(C64::new(-0.2, 0.1), 200);
        let mut a = BigMachine::new(p.clone(), params.clone(), env, c.clone(), n).unwrap();
        let mut b = BigMachine::new(p.clone(), params, env, c.add(&tiny), n).unwrap();
        for _ in 0..40 {
            a.step().unwrap();
            b.step().unwrap();
        }
        let d = a.z().sub(b.z());
        assert!(!d.is_zero(), "the two orbits are one: the digits were lost");
        assert!(d.norm_sqr().to_f64() < 1e-30, "and they stay close");
    }
}

#[cfg(test)]
mod speed {
    use super::*;
    use super::super::builtins::C64;

    /// MEASUREMENT: a pass of each twin in big floats, 3 limbs.
    #[test]
    #[ignore = "measurement"]
    fn measure_a_pass_of_each_twin() {
        for (name, params) in [("mandelbrot", vec![]), ("multibrot", vec![("power", 4.0)]), ("tricorn", vec![("power", 2.0)]), ("burning_ship", vec![("variant", 0.0)]), ("lambda", vec![]), ("phoenix", vec![]), ("manowar", vec![])] {
            let src = crate::escape::twins::custom_source(name).unwrap();
            let p = std::sync::Arc::new(crate::escape::custom::check_entry(super::super::EntryKind::Formula, &src).unwrap());
            let mut pv = ParamValues::defaults(&p);
            for (k, v) in &params {
                if let Some(i) = p.params.iter().position(|x| x.name == *k) {
                    pv.values[i].0 = *v;
                }
            }
            let n = 3;
            let c = BigComplex::from_f64(-0.5, -0.5, n);
            let mut m = BigMachine::new(p.clone(), pv, Env::at(C64::new(-0.5, -0.5), 20000), c, n).unwrap();
            let steps = 2000;
            let t = std::time::Instant::now();
            let mut zs = Vec::with_capacity(steps);
            for _ in 0..steps {
                zs.push(m.z().clone());
                let _ = m.step();
            }
            let big = t.elapsed().as_secs_f64() * 1e6 / steps as f64;
            // The nucleus search's watch beside it (`periodic::SlopeWatch`):
            // the f64 derivative pass and the test, along the same orbit.
            let search = super::super::periodic::NucleusSearch::new(&p, ParamValues::defaults(&p), Env::at(C64::new(-0.5, -0.5), 20000));
            let watch = search.and_then(|s| s.watch(C64::new(-0.5, -0.5), zs[0].clone(), -30.0));
            let per_watch = watch.map(|mut w| {
                let t = std::time::Instant::now();
                for pair in zs.windows(2) {
                    let prev = C64::new(pair[0].re.to_f64(), pair[0].im.to_f64());
                    let _ = w.observe(prev, &pair[1]);
                }
                t.elapsed().as_secs_f64() * 1e6 / (steps - 1) as f64
            });
            println!("{name}: {big:.2} us a pass; the watch {per_watch:.2?} us");
        }
        let n = 3;
        let a = BigFloat::from_f64(0.7, n);
        let b = BigFloat::from_f64(-0.3, n);
        let t = std::time::Instant::now();
        let mut x = a.clone();
        for _ in 0..100_000 {
            x = x.mul(&b).add(&a);
        }
        println!("BigFloat mul+add: {:.1} ns ({})", t.elapsed().as_secs_f64() * 1e9 / 100_000.0, x.to_f64());
        // The built-ins' fixed point, the same width.
        use crate::escape::fixedpoint::FixedPoint;
        let (fa, fb) = (FixedPoint::from_f64(0.7, n), FixedPoint::from_f64(-0.3, n));
        let t = std::time::Instant::now();
        let mut y = fa.clone();
        for _ in 0..100_000 {
            y = y.mul(&fb).add(&fa);
        }
        println!("FixedPoint mul+add: {:.1} ns ({})", t.elapsed().as_secs_f64() * 1e9 / 100_000.0, y.to_f64());
    }
}
