//! The checked program evaluated on the CPU in f64: the reference the
//! WGSL is tested against, and (phase 3) the reference orbit.
//!
//! Order per pixel, as UF runs it: the formula's `init:` and the
//! colouring's `init:`, then each iteration the formula's `loop:`, the
//! colouring's `loop:` and the bailout test, until the test fails
//! (the pixel is outside) or `maxiter` passes (inside); then the
//! colouring's `final:`.

use super::ast::{BinOp, UnOp};
use super::builtins::{Builtin, C64, USER_FUNCTIONS};
use super::check::LOOP_CAP;
use super::ir::*;
use super::Dialect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Float(f64),
    Complex(C64),
}

impl Value {
    fn zero(ty: Ty) -> Value {
        match ty {
            Ty::Bool => Value::Bool(false),
            Ty::Int => Value::Int(0),
            Ty::Float => Value::Float(0.0),
            Ty::Complex => Value::Complex(C64::ZERO),
        }
    }

    pub fn as_bool(self) -> bool {
        match self {
            Value::Bool(b) => b,
            Value::Int(i) => i != 0,
            Value::Float(f) => f != 0.0,
            Value::Complex(c) => c.re != 0.0 || c.im != 0.0,
        }
    }

    pub fn as_f64(self) -> f64 {
        match self {
            Value::Bool(b) => f64::from(u8::from(b)),
            Value::Int(i) => f64::from(i),
            Value::Float(f) => f,
            Value::Complex(c) => c.re,
        }
    }

    pub fn as_c64(self) -> C64 {
        match self {
            Value::Complex(c) => c,
            other => C64::real(other.as_f64()),
        }
    }

    fn as_int(self) -> i32 {
        match self {
            Value::Int(i) => i,
            Value::Bool(b) => i32::from(b),
            other => other.as_f64() as i32,
        }
    }
}

/// Per-pixel inputs: what the predefined symbols read.
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub pixel: C64,
    pub maxiter: u32,
    pub center: C64,
    pub magn: f64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub random: C64,
    /// The Julia toggle (Fractint's `ismand` is its opposite).
    pub julia: bool,
    /// The Julia constant, which a `switch:` target reads.
    pub julia_c: C64,
    /// The view's rotation, radians.
    pub rotation: f64,
}

impl Env {
    /// A pixel with nothing else set: tests that only need `#pixel`.
    pub fn at(pixel: C64, maxiter: u32) -> Self {
        Env { pixel, maxiter, center: C64::ZERO, magn: 1.0, x: 0, y: 0, width: 1, height: 1, random: C64::ZERO, julia: false, julia_c: C64::ZERO, rotation: 0.0 }
    }
}

/// A program's parameter values and user-function choices, by id.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamValues {
    pub values: Vec<(f64, f64)>,
    pub funcs: Vec<usize>,
}

impl ParamValues {
    pub fn defaults(p: &Program) -> Self {
        ParamValues {
            values: p.params.iter().map(|x| x.default).collect(),
            funcs: p.funcs.iter().map(|f| f.default).collect(),
        }
    }

    /// The values a parameter block holds, as the GPU reads them: one
    /// slot each, two for a complex, then one per user function.
    pub fn from_slots(p: &Program, slots: &[f32]) -> Self {
        let mut at = 0;
        let mut next = || {
            let v = slots.get(at).copied().unwrap_or(0.0) as f64;
            at += 1;
            v
        };
        let values = p
            .params
            .iter()
            .map(|x| if x.ty() == Ty::Complex { (next(), next()) } else { (next(), 0.0) })
            .collect();
        let funcs = p.funcs.iter().map(|_| next().round().max(0.0) as usize).collect();
        ParamValues { values, funcs }
    }
}

/// A program's `global:` values, packed as the GPU reads them
/// ([`Program::globals`]: two floats for a complex, one otherwise; an
/// int and a bool as numbers). Runs `global:` once, as UF does per
/// image; `env`'s per-pixel fields are not read there.
pub fn globals(p: &Program, params: &ParamValues, env: &Env) -> Vec<f32> {
    let mut m = Machine::new(p, params);
    let mut sh = Shared::new(env);
    m.exec(&p.global_body, &mut sh);
    let mut out = Vec::with_capacity(p.global_floats());
    for &v in &p.globals {
        match m.vars[v] {
            Value::Complex(c) => out.extend([c.re as f32, c.im as f32]),
            other => out.push(other.as_f64() as f32),
        }
    }
    out
}

/// One program's running state.
struct Machine<'p> {
    prog: &'p Program,
    params: &'p ParamValues,
    vars: Vec<Value>,
}

/// What the shared symbols hold at the moment.
struct Shared<'e> {
    env: &'e Env,
    z: C64,
    numiter: u32,
    /// The formula's pass index ([`Predef::Pass`]).
    pass: u32,
    index: f64,
    solid: bool,
    /// The perturbation equations' `#dz` and `#dpixel`.
    dz: C64,
    dpixel: C64,
}

impl<'e> Shared<'e> {
    fn new(env: &'e Env) -> Self {
        Shared { env, z: C64::ZERO, numiter: 0, pass: 0, index: 0.0, solid: false, dz: C64::ZERO, dpixel: C64::ZERO }
    }
}

/// The result of running a formula, with or without a colouring.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub z: C64,
    /// Loop executions: `#numiter`.
    pub n: u32,
    /// The bailout test failed before `maxiter`: the pixel is outside.
    pub escaped: bool,
    /// The colouring's `#index` and `#solid`, when one ran.
    pub index: f64,
    pub solid: bool,
    /// The iterate's derivative with respect to the pixel, for a
    /// program the derivative pass extended (zero otherwise).
    pub dz: C64,
}

/// The `perturb` setting's answer for these parameters and this image
/// (absent: the equations always hold).
pub fn perturb_holds(p: &Program, params: &ParamValues, env: &Env) -> bool {
    let Some(e) = &p.perturb_when else { return true };
    let m = Machine::new(p, params);
    let sh = Shared::new(env);
    m.eval(e, &sh).as_bool()
}

/// A formula's declared degree at infinity ([`Program::degree`]) for
/// these parameters, or `None` when it declares none.
pub fn degree_of(p: &Program, params: &ParamValues, env: &Env) -> Option<f64> {
    let e = p.degree.as_ref()?;
    let m = Machine::new(p, params);
    let sh = Shared::new(env);
    Some(m.eval(e, &sh).as_f64())
}

/// A pixel as the perturbed path runs it, in f64: the formula's own
/// orbit at `reference` (its init and loop, `#z` kept each iteration,
/// until its test fails or `maxiter`), then the pixel as a deviation
/// from it by the formula's perturbation equations (`#z` the
/// reference's value, `#dpixel` the pixel's offset), tested on the
/// reference's `z` plus the deviation, and rebased onto the orbit's
/// start (Zhuoran) when the orbit ends, and with `rebase` on also when
/// that deviation is the smaller, as the GPU's. The model the GPU path is held to,
/// and the check that a formula's equations agree with its loop.
/// `None` when the formula has no equations.
pub fn run_perturbed(formula: &Program, params: &ParamValues, env: &Env, reference: C64, rebase: bool) -> Option<Run> {
    if formula.perturb_loop.is_empty() {
        return None;
    }
    let test = super::perturb::perturb_test(formula);
    let renv = Env { pixel: reference, ..*env };
    // The references: the main one from pass 0, and for a loop with a
    // cycle one per phase, from the pass that phase first runs
    // (`prefix + q`), each from the init's start.
    let cycle = formula.cycle.filter(|_| formula.reads(Predef::Pass));
    let starts: Vec<u32> = std::iter::once(0).chain(cycle.iter().flat_map(|c| (0..c.period).map(move |q| c.prefix + q))).collect();
    // Each reference keeps z at every pass's start, and what the equations
    // carry besides it (`Program::carried`), recorded beside it.
    let record = |r: &Machine| -> Vec<Value> { formula.carried.iter().map(|c| r.vars[c.var]).collect() };
    let mut records: Vec<Vec<Vec<Value>>> = Vec::new();
    let orbits: Vec<Vec<C64>> = starts
        .iter()
        .map(|&start| {
            let mut r = Machine::new(formula, params);
            let mut rsh = Shared::new(&renv);
            r.exec(&formula.global_body, &mut rsh);
            r.exec(&formula.init, &mut rsh);
            let mut orbit = vec![rsh.z];
            let mut carried = vec![record(&r)];
            for i in start..start + env.maxiter {
                rsh.pass = i;
                r.exec(&formula.loop_body, &mut rsh);
                rsh.pass = i + 1;
                orbit.push(rsh.z);
                carried.push(record(&r));
                if !formula.bailout.as_ref().map_or(true, |t| r.eval(t, &rsh).as_bool()) {
                    break;
                }
            }
            records.push(carried);
            orbit
        })
        .collect();
    // The pixel: the equations' own temporaries, the test on z alone.
    let mut p = Machine::new(formula, params);
    let mut sh = Shared::new(env);
    p.exec(&formula.global_body, &mut sh);
    sh.dpixel = env.pixel.sub(reference);
    sh.z = orbits[0][0];
    p.exec(&formula.perturb_init, &mut sh);
    // Following reference `r` at its index `m`.
    let (mut r, mut m) = (0usize, 0usize);
    let mut escaped = false;
    let mut n = env.maxiter;
    let mut z_full = orbits[0][0].add(sh.dz);
    for i in 0..env.maxiter {
        let orbit = &orbits[r];
        sh.z = orbit[m];
        sh.pass = starts[r] + m as u32;
        for (k, c) in formula.carried.iter().enumerate() {
            p.vars[c.reference] = records[r][m][k];
        }
        p.exec(&formula.perturb_loop, &mut sh);
        m += 1;
        let at = m.min(orbit.len() - 1);
        z_full = orbit[at].add(sh.dz);
        sh.z = z_full;
        // The test reads the carried variables' full values.
        for (k, c) in formula.carried.iter().enumerate() {
            p.vars[c.var] = plus(records[r][at][k], p.vars[c.deviation]);
        }
        if !test.as_ref().map_or(true, |t| p.eval(t, &sh).as_bool()) {
            escaped = true;
            n = i + 1;
            break;
        }
        // Past the orbit's end there is nothing to follow but a start;
        // `rebase` adds the smaller-deviation test. The start is the
        // reference's at the pixel's next phase: the main one's without
        // a cycle, none during the prefix.
        let ended = m + 1 >= orbit.len();
        let next = i + 1;
        let target = match cycle {
            None => Some(0),
            Some(c) if next >= c.prefix => Some(1 + ((next - c.prefix) % c.period) as usize),
            Some(_) => None,
        };
        match target {
            Some(t) => {
                let rebased = z_full.sub(orbits[t][0]);
                // The test weighs what is carried with z, as the GPU's.
                let (mut now, mut then) = (sh.dz.norm_sqr(), rebased.norm_sqr());
                for (k, c) in formula.carried.iter().enumerate() {
                    let dev = p.vars[c.deviation].as_c64();
                    now += dev.norm_sqr();
                    then += plus(records[r][at][k], p.vars[c.deviation]).as_c64().sub(records[t][0][k].as_c64()).norm_sqr();
                }
                if ended || (rebase && then < now) {
                    sh.dz = rebased;
                    // Each carried deviation moves to the new start as
                    // z's does: its full value less the start's record.
                    for (k, c) in formula.carried.iter().enumerate() {
                        let full = plus(records[r][at][k], p.vars[c.deviation]);
                        p.vars[c.deviation] = minus(full, records[t][0][k]);
                    }
                    r = t;
                    m = 0;
                }
            }
            // The main reference ended within the prefix: none other
            // has the pixel's phase.
            None if ended => {
                escaped = true;
                n = i + 1;
                break;
            }
            None => {}
        }
    }
    Some(Run { z: z_full, n, escaped, index: 0.0, solid: false, dz: C64::ZERO })
}

/// `a + b` of two floats or two complex values (a carried variable's
/// record and its deviation).
fn plus(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => Value::Float(x + y),
        (a, b) => Value::Complex(a.as_c64().add(b.as_c64())),
    }
}

fn minus(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => Value::Float(x - y),
        (a, b) => Value::Complex(a.as_c64().sub(b.as_c64())),
    }
}

/// Run a formula, and a colouring alongside it when given.
pub fn run(
    formula: &Program,
    fparams: &ParamValues,
    coloring: Option<(&Program, &ParamValues)>,
    env: &Env,
) -> Run {
    let mut f = Machine::new(formula, fparams);
    let mut c = coloring.map(|(p, v)| Machine::new(p, v));
    let mut sh = Shared::new(env);
    // `global:` first: per image on the GPU, per call here.
    f.exec(&formula.global_body, &mut sh);
    if let Some(c) = c.as_mut() {
        let prog = c.prog;
        c.exec(&prog.global_body, &mut sh);
    }
    f.exec(&formula.init, &mut sh);
    if let Some(c) = c.as_mut() {
        let prog = c.prog;
        c.exec(&prog.init, &mut sh);
    }
    let mut escaped = false;
    for i in 0..env.maxiter {
        sh.pass = i;
        f.exec(&formula.loop_body, &mut sh);
        sh.pass = i + 1;
        sh.numiter = i + 1;
        if let Some(c) = c.as_mut() {
            let prog = c.prog;
        c.exec(&prog.loop_body, &mut sh);
        }
        let go = formula.bailout.as_ref().map_or(true, |t| f.eval(t, &sh).as_bool());
        if !go {
            escaped = true;
            break;
        }
    }
    if !escaped {
        sh.numiter = env.maxiter;
    }
    if let Some(c) = c.as_mut() {
        let prog = c.prog;
        c.exec(&prog.final_body, &mut sh);
        // An INSIDE colouring leaves the outside to the background, as
        // its WGSL does.
        if prog.region == Region::Inside && escaped {
            sh.solid = true;
        }
    }
    let dz = formula.dz_var.map_or(C64::ZERO, |v| f.vars[v].as_c64());
    Run { z: sh.z, n: sh.numiter, escaped, index: sh.index, solid: sh.solid, dz }
}

impl<'p> Machine<'p> {
    fn new(prog: &'p Program, params: &'p ParamValues) -> Self {
        Machine { prog, params, vars: prog.vars.iter().map(|v| Value::zero(v.ty)).collect() }
    }

    fn exec(&mut self, stmts: &[IStmt], sh: &mut Shared) {
        for s in stmts {
            match s {
                IStmt::Assign(place, e) => {
                    let v = self.eval(e, sh);
                    match place {
                        Place::Var(id) => self.vars[*id] = v,
                        Place::Z => sh.z = v.as_c64(),
                        Place::Index => sh.index = v.as_f64(),
                        Place::Solid => sh.solid = v.as_bool(),
                        Place::DZ => sh.dz = v.as_c64(),
                    }
                }
                IStmt::If { arms, otherwise } => {
                    let mut taken = false;
                    for (cond, body) in arms {
                        if self.eval(cond, sh).as_bool() {
                            self.exec(body, sh);
                            taken = true;
                            break;
                        }
                    }
                    if !taken {
                        self.exec(otherwise, sh);
                    }
                }
                IStmt::While { cond, body } => {
                    let mut k = 0;
                    while k < LOOP_CAP && self.eval(cond, sh).as_bool() {
                        self.exec(body, sh);
                        k += 1;
                    }
                }
                IStmt::Repeat { body, cond } => {
                    let mut k = 0;
                    loop {
                        self.exec(body, sh);
                        k += 1;
                        if k >= LOOP_CAP || self.eval(cond, sh).as_bool() {
                            break;
                        }
                    }
                }
            }
        }
    }

    fn eval(&self, e: &IExpr, sh: &Shared) -> Value {
        match &e.kind {
            IExprKind::Bool(b) => Value::Bool(*b),
            IExprKind::Int(i) => Value::Int(*i as i32),
            IExprKind::Float(f) => Value::Float(*f),
            IExprKind::Complex(re, im) => Value::Complex(C64::new(*re, *im)),
            IExprKind::Var(id) => self.vars[*id],
            IExprKind::Param(id) => {
                let (re, im) = self.params.values[*id];
                match self.prog.params[*id].ty() {
                    Ty::Bool => Value::Bool(re != 0.0),
                    Ty::Int => Value::Int(re.round() as i32),
                    Ty::Float => Value::Float(re),
                    Ty::Complex => Value::Complex(C64::new(re, im)),
                }
            }
            IExprKind::Predef(p) => predef(*p, sh),
            IExprKind::Convert(a) => {
                let v = self.eval(a, sh);
                match e.ty {
                    // Fractint tests the real part alone.
                    Ty::Bool if self.prog.dialect == Dialect::Fractint => Value::Bool(v.as_f64() != 0.0),
                    Ty::Bool => Value::Bool(v.as_bool()),
                    Ty::Int => Value::Int(v.as_int()),
                    Ty::Float => Value::Float(v.as_f64()),
                    Ty::Complex => Value::Complex(v.as_c64()),
                }
            }
            IExprKind::MakeComplex(a, b) => Value::Complex(C64::new(self.eval(a, sh).as_f64(), self.eval(b, sh).as_f64())),
            IExprKind::Unary(UnOp::Neg, a) => match self.eval(a, sh) {
                Value::Int(i) => Value::Int(i.wrapping_neg()),
                Value::Float(f) => Value::Float(-f),
                Value::Complex(c) => Value::Complex(c.neg()),
                Value::Bool(b) => Value::Bool(b),
            },
            IExprKind::Unary(UnOp::Not, a) => Value::Bool(!self.eval(a, sh).as_bool()),
            // A real factor scales (IExpr::is_lifted_real).
            IExprKind::Binary(BinOp::Mul, a, b) if e.ty == Ty::Complex && (a.is_lifted_real() || b.is_lifted_real()) => {
                let (k, z) = if a.is_lifted_real() { (a, b) } else { (b, a) };
                Value::Complex(self.eval(z, sh).as_c64().scale(self.eval(k, sh).as_c64().re))
            }
            IExprKind::Binary(op, a, b) => binary(*op, self.eval(a, sh), self.eval(b, sh), e.ty, self.prog.dialect),
            IExprKind::PowInt(a, n) => match self.eval(a, sh) {
                Value::Complex(c) => Value::Complex(c.powi(*n)),
                v => Value::Float(v.as_f64().powi(*n as i32)),
            },
            IExprKind::Modulus(a) => match self.eval(a, sh) {
                Value::Complex(c) => Value::Float(c.norm_sqr()),
                v => {
                    let x = v.as_f64();
                    Value::Float(x * x)
                }
            },
            IExprKind::Call(f, a) => call(*f, self.eval(a, sh), e.ty, self.prog.dialect),
            IExprKind::UserCall(id, a) => {
                let f = USER_FUNCTIONS[self.params.funcs[*id].min(USER_FUNCTIONS.len() - 1)];
                Value::Complex(f.eval_c(self.eval(a, sh).as_c64()))
            }
            IExprKind::UserDeriv(id, a) => {
                let f = USER_FUNCTIONS[self.params.funcs[*id].min(USER_FUNCTIONS.len() - 1)];
                Value::Complex(f.deriv_c(self.eval(a, sh).as_c64()))
            }
        }
    }
}

fn predef(p: Predef, sh: &Shared) -> Value {
    let env = sh.env;
    match p {
        Predef::Pixel => Value::Complex(env.pixel),
        Predef::Z => Value::Complex(sh.z),
        Predef::NumIter => Value::Int(sh.numiter as i32),
        Predef::Pass => Value::Int(sh.pass as i32),
        Predef::MaxIter => Value::Int(env.maxiter as i32),
        Predef::Pi => Value::Float(std::f64::consts::PI),
        Predef::E => Value::Float(std::f64::consts::E),
        Predef::Random => Value::Complex(env.random),
        Predef::Center => Value::Complex(env.center),
        Predef::Magn => Value::Float(env.magn),
        Predef::X => Value::Int(env.x),
        Predef::Y => Value::Int(env.y),
        Predef::Width => Value::Int(env.width),
        Predef::Height => Value::Int(env.height),
        Predef::ScreenPixel => Value::Complex(C64::new(f64::from(env.x), f64::from(env.y))),
        Predef::Index => Value::Float(sh.index),
        Predef::Solid => Value::Bool(sh.solid),
        Predef::WhiteSq => Value::Complex(C64::real(f64::from((env.x + env.y) & 1))),
        Predef::ScrnMax => Value::Complex(C64::new(f64::from(env.width), f64::from(env.height))),
        Predef::IsMand => Value::Complex(C64::real(if env.julia { 0.0 } else { 1.0 })),
        Predef::MagXMag => Value::Complex(C64::new(env.magn, 1.0)),
        Predef::RotSkew => Value::Complex(C64::real(env.rotation.to_degrees())),
        Predef::JuliaC => Value::Complex(env.julia_c),
        Predef::DZ => Value::Complex(sh.dz),
        Predef::DPixel => Value::Complex(sh.dpixel),
    }
}

/// `|c + d| - |c|` by sign case, as the GPU's `uf_diffabs`: exact
/// where the difference would cancel.
pub fn diffabs(c: f64, d: f64) -> f64 {
    if c >= 0.0 {
        if c + d >= 0.0 {
            d
        } else {
            -(2.0 * c + d)
        }
    } else if c + d > 0.0 {
        2.0 * c + d
    } else {
        -d
    }
}

/// Operands arrive converted to the operation's type (comparisons:
/// their common one).
fn binary(op: BinOp, l: Value, r: Value, ty: Ty, dialect: Dialect) -> Value {
    use BinOp::*;
    match op {
        And => Value::Bool(l.as_bool() && r.as_bool()),
        Or => Value::Bool(l.as_bool() || r.as_bool()),
        Eq | Ne => {
            let eq = match (l, r) {
                // Fractint compares real parts, equality too.
                (Value::Complex(a), Value::Complex(b)) if dialect == Dialect::Fractint => a.re == b.re,
                (Value::Complex(a), Value::Complex(b)) => a == b,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (a, b) => a.as_f64() == b.as_f64(),
            };
            Value::Bool(eq == (op == Eq))
        }
        // Complex values order by their real parts.
        Lt => Value::Bool(l.as_f64() < r.as_f64()),
        Le => Value::Bool(l.as_f64() <= r.as_f64()),
        Gt => Value::Bool(l.as_f64() > r.as_f64()),
        Ge => Value::Bool(l.as_f64() >= r.as_f64()),
        DiffAbs => Value::Float(diffabs(l.as_f64(), r.as_f64())),
        Add | Sub | Mul | Div | Mod | Pow => match ty {
            Ty::Int => {
                let (a, b) = (l.as_int(), r.as_int());
                Value::Int(match op {
                    Add => a.wrapping_add(b),
                    Sub => a.wrapping_sub(b),
                    Mul => a.wrapping_mul(b),
                    // A zero divisor gives 0, in WGSL too.
                    Mod => {
                        if b == 0 {
                            0
                        } else {
                            a.wrapping_rem(b)
                        }
                    }
                    _ => unreachable!("int division and powers are float"),
                })
            }
            Ty::Float => {
                let (a, b) = (l.as_f64(), r.as_f64());
                Value::Float(match op {
                    Add => a + b,
                    Sub => a - b,
                    Mul => a * b,
                    Div => a / b,
                    Mod => a % b,
                    Pow => a.powf(b),
                    _ => unreachable!(),
                })
            }
            _ => {
                let (a, b) = (l.as_c64(), r.as_c64());
                Value::Complex(match op {
                    Add => a.add(b),
                    Sub => a.sub(b),
                    Mul => a.mul(b),
                    Div => a.div(b),
                    Pow => a.powc(b),
                    _ => unreachable!("complex % is refused by the checker"),
                })
            }
        },
    }
}

fn call(f: Builtin, arg: Value, ty: Ty, dialect: Dialect) -> Value {
    // Fractint's round is floor(x + 0.5), halves always up.
    if f == Builtin::Round && dialect == Dialect::Fractint {
        let r = |x: f64| (x + 0.5).floor();
        return match (arg, ty) {
            (Value::Int(i), Ty::Int) => Value::Int(i),
            (_, Ty::Int) => Value::Int(r(arg.as_f64()) as i32),
            (v, _) => {
                let c = v.as_c64();
                Value::Complex(C64::new(r(c.re), r(c.im)))
            }
        };
    }
    match (arg, ty) {
        (_, Ty::Bool) => {
            let c = arg.as_c64();
            let hit = match f {
                Builtin::IsInf => c.re.is_infinite() || c.im.is_infinite(),
                _ => c.re.is_nan() || c.im.is_nan(),
            };
            Value::Bool(hit)
        }
        (Value::Int(i), Ty::Int) => Value::Int(match f {
            Builtin::Sqr => i.wrapping_mul(i),
            Builtin::Abs => i.wrapping_abs(),
            Builtin::Zero => 0,
            _ => i,
        }),
        (_, Ty::Int) => Value::Int(f.eval_f(arg.as_f64()) as i32),
        (Value::Complex(c), Ty::Float) => Value::Float(f.eval_c(c).re),
        (_, Ty::Float) => Value::Float(f.eval_f(arg.as_f64())),
        (_, _) => Value::Complex(f.eval_c(arg.as_c64())),
    }
}

#[cfg(test)]
mod tests {
    use super::super::check::check;
    use super::super::parser::parse_file;
    use super::super::{Dialect, EntryKind};
    use super::*;

    fn program(src: &str, kind: EntryKind) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], kind, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{:?}", d.iter().map(|x| x.render(src)).collect::<Vec<_>>());
        p.unwrap()
    }

    const MANDELBROT: &str = "Mandelbrot {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}";

    /// Escape counts against the textbook loop, for points of every kind.
    #[test]
    fn mandelbrot_counts_match_the_textbook() {
        let p = program(MANDELBROT, EntryKind::Formula);
        let params = ParamValues::defaults(&p);
        for (re, im) in [(0.0, 0.0), (-0.75, 0.1), (0.3, 0.5), (-2.0, 0.0), (0.26, 0.0), (1.0, 1.0)] {
            let c = C64::new(re, im);
            let mut z = C64::ZERO;
            let mut n = 0u32;
            let mut escaped = false;
            while n < 500 {
                z = z.mul(z).add(c);
                n += 1;
                if z.norm_sqr() >= 4.0 {
                    escaped = true;
                    break;
                }
            }
            let r = run(&p, &params, None, &Env::at(c, 500));
            assert_eq!((r.n, r.escaped), (n, escaped), "c = {c:?}");
            assert_eq!(r.z, z);
        }
    }

    /// Magnet I and II as UF and Fractint define them, against the
    /// built-in formula's own algebra.
    #[test]
    fn the_magnet_pair_matches_its_algebra() {
        let m1 = program(
            "Magnet1 {\ninit:\n z = 0\nloop:\n z = sqr((z^2 + #pixel - 1) / (2*z + #pixel - 2))\nbailout:\n |z| < 100 && |z - 1| > 1e-6\n}",
            EntryKind::Formula,
        );
        let m2 = program(
            "Magnet2 {\ninit:\n z = 0\n c = #pixel\nloop:\n \
             z = sqr((z^3 + 3*(c - 1)*z + (c - 1)*(c - 2)) / (3*z^2 + 3*(c - 2)*z + (c - 1)*(c - 2) + 1))\n\
             bailout:\n |z| < 100 && |z - 1| > 1e-6\n}",
            EntryKind::Formula,
        );
        let c = C64::new(1.3, 0.7);
        let one = C64::ONE;
        let two = C64::real(2.0);
        let three = C64::real(3.0);
        let (mut z1, mut z2) = (C64::ZERO, C64::ZERO);
        for _ in 0..5 {
            let q = z1.mul(z1).add(c).sub(one).div(z1.scale(2.0).add(c).sub(two));
            z1 = q.mul(q);
            let cm1 = c.sub(one);
            let cm2 = c.sub(two);
            let num = z2.powi(3).add(three.mul(cm1).mul(z2)).add(cm1.mul(cm2));
            let den = three.mul(z2.mul(z2)).add(three.mul(cm2).mul(z2)).add(cm1.mul(cm2)).add(one);
            let q = num.div(den);
            z2 = q.mul(q);
        }
        let r1 = run(&m1, &ParamValues::defaults(&m1), None, &Env::at(c, 5));
        let r2 = run(&m2, &ParamValues::defaults(&m2), None, &Env::at(c, 5));
        assert!((r1.z.re - z1.re).abs() < 1e-12 && (r1.z.im - z1.im).abs() < 1e-12, "{:?} vs {z1:?}", r1.z);
        assert!((r2.z.re - z2.re).abs() < 1e-12 && (r2.z.im - z2.im).abs() < 1e-12, "{:?} vs {z2:?}", r2.z);
    }

    /// A colouring sees the formula's `#z` every iteration, after the
    /// loop and before the test, and its final sees the count.
    #[test]
    fn a_colouring_runs_alongside_in_uf_order() {
        let f = program(MANDELBROT, EntryKind::Formula);
        let c = program(
            "Avg {\ninit:\n float sum = 0\nloop:\n sum = sum + cabs(#z)\nfinal:\n #index = sum / #numiter\n if #numiter > 3\n  #solid = true\n endif\n}",
            EntryKind::Coloring,
        );
        let px = C64::new(0.3, 0.5);
        let r = run(&f, &ParamValues::defaults(&f), Some((&c, &ParamValues::defaults(&c))), &Env::at(px, 100));
        // Recompute: every iterate including the escaping one.
        let mut z = C64::ZERO;
        let mut sum = 0.0;
        let mut n = 0;
        loop {
            z = z.mul(z).add(px);
            n += 1;
            sum += z.abs();
            if z.norm_sqr() >= 4.0 || n == 100 {
                break;
            }
        }
        assert_eq!(r.n, n);
        assert!((r.index - sum / f64::from(n)).abs() < 1e-12);
        assert_eq!(r.solid, n > 3);
    }

    #[test]
    fn parameters_user_functions_and_loops() {
        let p = program(
            "P {\ninit:\n z = @seed\n int k = 0\n while k < 10\n  k = k + 1\n endwhile\n repeat\n  k = k - 3\n until k < 0\n\
             loop:\n z = @f(z) * k + @scale\nbailout:\n false\ndefault:\n complex param seed\n  default = (0.5, -1)\n endparam\n\
             float param scale\n  default = 2\n endparam\n func f\n  default = conj()\n endfunc\n}",
            EntryKind::Formula,
        );
        let mut params = ParamValues::defaults(&p);
        // k: 10, then 7, 4, 1, -2. One iteration: conj(seed) * -2 + 2.
        let r = run(&p, &params, None, &Env::at(C64::ZERO, 50));
        assert!(r.escaped && r.n == 1);
        assert_eq!(r.z, C64::new(0.5, 1.0).scale(-2.0).add(C64::real(2.0)));
        params.funcs[0] = USER_FUNCTIONS.iter().position(|b| *b == Builtin::Ident).unwrap();
        params.values[1] = (0.0, 0.0);
        let r = run(&p, &params, None, &Env::at(C64::ZERO, 50));
        assert_eq!(r.z, C64::new(0.5, -1.0).scale(-2.0));
    }

    fn program_in(src: &str, kind: EntryKind, dialect: Dialect) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], kind, dialect);
        assert!(!super::super::has_errors(&d), "{:?}", d.iter().map(|x| x.render(src)).collect::<Vec<_>>());
        p.unwrap()
    }

    fn run_in(src: &str, dialect: Dialect, env: &Env) -> Run {
        let p = program_in(src, EntryKind::Formula, dialect);
        run(&p, &ParamValues::defaults(&p), None, env)
    }

    /// Fractint tests the real part alone; UF any part.
    #[test]
    fn fractint_truth_is_the_real_part() {
        let src = "T { z = (0, 1) : z = z, z }";
        let f = run_in(src, Dialect::Fractint, &Env::at(C64::ZERO, 50));
        assert!(f.escaped && f.n == 1, "Fractint stops on a zero real part");
        let u = run_in(src, Dialect::UltraFractal, &Env::at(C64::ZERO, 50));
        assert!(!u.escaped && u.n == 50, "UF reads (0, 1) as true");
        // Equality compares real parts in Fractint.
        let src = "E { z = 0 : z = (1, 5) == (1, 7), 0 }";
        assert_eq!(run_in(src, Dialect::Fractint, &Env::at(C64::ZERO, 5)).z, C64::ONE);
        assert_eq!(run_in(src, Dialect::UltraFractal, &Env::at(C64::ZERO, 5)).z, C64::ZERO);
    }

    /// Fractint rounds halves up, floor(x + 0.5); UF away from zero.
    #[test]
    fn fractint_rounds_halves_up() {
        let src = "R { z = round((-2.5, 2.5)) : z = z, 0 }";
        assert_eq!(run_in(src, Dialect::Fractint, &Env::at(C64::ZERO, 1)).z, C64::new(-2.0, 3.0));
        assert_eq!(run_in(src, Dialect::UltraFractal, &Env::at(C64::ZERO, 1)).z, C64::new(-3.0, 3.0));
    }

    /// `sqr()` leaves its argument's squared modulus in `LastSqr`, read
    /// from before the assignment: the test sees the PREVIOUS iterate.
    #[test]
    fn fractint_lastsqr_is_the_last_squared_argument() {
        let c = C64::new(0.3, 0.5);
        let r = run_in("L { z = 0, c = pixel : z = sqr(z) + c, LastSqr <= 4 }", Dialect::Fractint, &Env::at(c, 500));
        let mut z = C64::ZERO;
        let mut n = 0;
        loop {
            let last = z.norm_sqr();
            z = z.mul(z).add(c);
            n += 1;
            if last > 4.0 || n == 500 {
                break;
            }
        }
        assert_eq!((r.n, r.z), (n, z));
    }

    /// Writing a parameter or a predefined name makes a per-pixel copy
    /// that starts as the original.
    #[test]
    fn fractint_writes_parameters_and_predefined_names_as_copies() {
        let px = C64::new(0.25, -0.5);
        let r = run_in("W { p1 = p1 + 1, pixel = pixel * 2, z = p1 + pixel : z = z, 0 }", Dialect::Fractint, &Env::at(px, 1));
        assert_eq!(r.z, C64::ONE.add(px.scale(2.0)));
    }

    /// Fractint's screen and view names.
    #[test]
    fn fractint_screen_and_view_names() {
        let env = Env { x: 3, y: 4, width: 10, height: 20, magn: 2.0, rotation: std::f64::consts::FRAC_PI_2, ..Env::at(C64::ZERO, 1) };
        let src = "S { z = whitesq + scrnpix + scrnmax * (0, 1) + ismand * 100 + magxmag * 1000 + rotskew : z = z, 0 }";
        let z = run_in(src, Dialect::Fractint, &env).z;
        assert!((z.re - 2174.0).abs() < 1e-9 && (z.im - 1014.0).abs() < 1e-9, "{z:?}");
        let julia = Env { julia: true, ..env };
        let zj = run_in(src, Dialect::Fractint, &julia).z;
        assert!((z.re - zj.re - 100.0).abs() < 1e-9, "ismand follows the Julia toggle");
    }

    /// `global:` values are the program's own constants: a formula
    /// computing its constant there runs as one writing it inline, and
    /// `globals` packs what the GPU reads, the image's names included.
    #[test]
    fn global_values_run_first_and_pack() {
        let src = "G {\nglobal:\n complex c = @seed * 2\n float k = #magn + #width\n int n = #maxiter\n bool b = n > 10\n\
                   init:\n z = #pixel\nloop:\n z = sqr(z) + c\nbailout:\n |z| < 4 && b && k > 0\n\
                   default:\n param seed\n  default = (-0.4, 0.078)\n endparam\n}";
        let inline = "I {\ninit:\n z = #pixel\nloop:\n z = sqr(z) + (-0.8, 0.156)\nbailout:\n |z| < 4\n}";
        let at = C64::new(0.1, -0.2);
        assert_eq!(run_in(src, Dialect::UltraFractal, &Env::at(at, 60)), run_in(inline, Dialect::UltraFractal, &Env::at(at, 60)));

        let p = program_in(src, EntryKind::Formula, Dialect::UltraFractal);
        let env = Env { magn: 2.5, width: 640, ..Env::at(C64::ZERO, 50) };
        let packed = globals(&p, &ParamValues::defaults(&p), &env);
        // `n` is read in `global:` alone, so it is not packed.
        assert_eq!(packed, vec![-0.8, 0.156, 642.5, 1.0]);
        // The parameters as the GPU holds them read back the same.
        let slots = [-0.4f32, 0.078];
        assert_eq!(ParamValues::from_slots(&p, &slots).values, vec![(-0.4f32 as f64, 0.078f32 as f64)]);
    }

    /// UF's perturbation example: run as the perturbed path runs it, a
    /// pixel near the reference iterates as it does directly -- the
    /// same count, the same final z to f64's reach -- on both planes,
    /// with and without rebasing.
    #[test]
    fn perturbation_equations_run_as_the_formula_does() {
        let m = program_in(
            "PM {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\nperturbinit:\n #dz = 0\n\
             perturbloop:\n #dz = 2 * #z * #dz + sqr(#dz) + #dpixel\n}",
            EntryKind::Formula,
            Dialect::UltraFractal,
        );
        let j = program_in(
            "PJ {\ninit:\n z = #pixel\nloop:\n z = sqr(z) + @seed\nbailout:\n |z| < 4\nperturbinit:\n #dz = #dpixel\n\
             perturbloop:\n #dz = 2 * #z * #dz + sqr(#dz)\ndefault:\n param seed\n  default = (-0.123, 0.745)\n endparam\n}",
            EntryKind::Formula,
            Dialect::UltraFractal,
        );
        let mut compared = 0;
        for (p, reference) in [(&m, C64::new(-0.7436, 0.1318)), (&j, C64::new(0.02, -0.03))] {
            let params = ParamValues::defaults(p);
            for k in 0..12 {
                let t = k as f64 * 0.5236;
                let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-4 * (1.0 + k as f64)));
                let env = Env::at(px, 400);
                let direct = run(p, &params, None, &env);
                for rebase in [false, true] {
                    let pert = run_perturbed(p, &params, &env, reference, rebase).expect("it has equations");
                    assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "{} at {px:?}, rebase {rebase}", p.name);
                    if direct.escaped {
                        let err = pert.z.sub(direct.z).abs() / direct.z.abs();
                        assert!(err < 1e-6, "{} at {px:?}: {:?} vs {:?}", p.name, pert.z, direct.z);
                    }
                    compared += 1;
                }
            }
        }
        assert_eq!(compared, 48);
        // No equations: nothing to run.
        let plain = program_in("P {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}", EntryKind::Formula, Dialect::UltraFractal);
        assert!(run_perturbed(&plain, &ParamValues::defaults(&plain), &Env::at(C64::ZERO, 10), C64::ZERO, true).is_none());
    }

    #[test]
    fn a_runaway_loop_stops_at_the_cap() {
        let p = program("R {\ninit:\n int k = 0\n while true\n  k = k + 1\n endwhile\n z = k\nloop:\n z = z\nbailout:\n false\n}", EntryKind::Formula);
        let r = run(&p, &ParamValues::defaults(&p), None, &Env::at(C64::ZERO, 1));
        assert_eq!(r.z, C64::real(f64::from(LOOP_CAP)));
    }
}
