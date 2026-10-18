//! A formula's derivative with respect to its pixel, by forward-mode
//! differentiation of the program.
//!
//! Every complex variable gets a companion holding its derivative, and
//! every assignment to one is preceded by its derivative's assignment
//! (both read the values from before it). Differentiating with respect
//! to `#pixel` is both planes' rule at once: a Mandelbrot-style program
//! gets `2 z dz + 1`, a Julia-style one (`z = #pixel` first) `2 z dz`.
//!
//! The result is an ordinary program, so the interpreter and the WGSL
//! back end run it unchanged; [`Program::dz_var`] names the iterate's
//! derivative. Only the variables the iterate depends on get one. A
//! user function on the pixel's path is chosen at run time, and the
//! choice may be a fold: it takes the Jacobian, below.
//!
//! **The Jacobian** (plan, phase 4, step 9d). A formula whose iterate
//! depends on its pixel through something not complex-differentiable
//! (`abs`, `conj`, `real`, `|x|`, a real variable that follows the
//! pixel) has no complex derivative, but it has a real one: a 2x2 real
//! matrix, written as the real-linear map `d -> P d + Q conj(d)`, as the
//! real-linear BLA tables are (Q zero where the map is holomorphic, P
//! zero where it is anti-holomorphic). [`with_jacobian`] carries a pair
//! of companions, P and Q, for every variable the iterate depends on,
//! real ones too (a real value's pair has `Q = conj(P)`), and names the
//! iterate's Q [`Program::dzq_var`].
//!
//! What a colouring reads from it is one complex number: the gradient of
//! the escape potential is `J^T u` for `u = z/|z|`, which is
//! `conj(P) u + Q conj(u)`, and the derivative that reports it as `dz`
//! does for a complex one (`|dz|` its length, `z / dz` its direction) is
//! `dz = P + conj(Q) u^2` -- `P` when the map is holomorphic. So
//! distance estimation, the normal map and analytic relief run unchanged
//! on a folding formula ([`effective_dz`]).

use super::ast::{BinOp, UnOp};
use super::builtins::{Builtin, C64, USER_FUNCTIONS};
use super::ir::*;
use std::cell::RefCell;

/// The most variables a derivative may add. Over UF's public library
/// (9,250 formulas) half add 15 or fewer, 99% at most 724; the few past
/// a few thousand are giants of thousands of lines (`as.ufm`'s Insect3,
/// 53,108, a derivative of 20 MB of WGSL that validating took minutes
/// over and overflowed the stack), whose state no GPU pixel holds well:
/// they go without, as a formula without a derivative does.
pub const MAX_DERIVATIVE_VARIABLES: usize = 2048;

/// The program extended with its derivative: the complex one where the
/// iterate has it, the Jacobian ([`with_jacobian`]) where it folds;
/// `None` when it has neither, or past [`MAX_DERIVATIVE_VARIABLES`].
pub fn with_derivative(p: &Program) -> Option<Program> {
    complex_derivative(p).or_else(|| with_jacobian(p)).filter(|d| d.vars.len() - p.vars.len() <= MAX_DERIVATIVE_VARIABLES)
}

/// The derivative a colouring reads from a Jacobian `(P, Q)` at the
/// iterate `z`: `P + conj(Q) (z/|z|)^2`, whose length is the escape
/// potential's gradient's and whose direction `z / dz` is its direction
/// (the module's docs). `P` where `z` is zero.
pub fn effective_dz(z: C64, p: C64, q: C64) -> C64 {
    let r = z.abs();
    if !(r > 0.0) || !r.is_finite() {
        return p;
    }
    let u = z.scale(1.0 / r);
    p.add(q.conj().mul(u.mul(u)))
}

/// The program extended with its complex derivative, or `None` when the
/// iterate has none.
fn complex_derivative(p: &Program) -> Option<Program> {
    let taint = tainted(p);
    let need = needed(p);
    let mut out = p.clone();
    // The derivative variables: one per complex variable the iterate
    // depends on, then the iterate's.
    let mut dvar: Vec<Option<VarId>> = vec![None; p.vars.len()];
    for (i, v) in p.vars.iter().enumerate() {
        // A global is one value per image: constant in the pixel.
        if v.ty == Ty::Complex && need[i] && !v.global && !v.is_array() {
            dvar[i] = Some(out.vars.len());
            out.vars.push(Var {
                name: format!("d_{}", v.name),
                ty: Ty::Complex,
                span: v.span,
                stored_real: false,
                global: false,
                perturb: false,
                dims: Vec::new(),
            });
        }
    }
    let dz = out.vars.len();
    out.vars.push(Var {
        name: "d_z".to_string(),
        ty: Ty::Complex,
        span: Default::default(),
        stored_real: false,
        global: false,
        perturb: false,
        dims: Vec::new(),
    });
    let dpix = pixel_unit(&mut out);
    let d = D { taint: &taint, dvar: &dvar, dz, dpix };
    out.init = [vec![set_unit(dpix)], d.stmts(&p.init)?].concat();
    out.loop_body = d.stmts(&p.loop_body)?;
    out.dz_var = Some(dz);
    out.dpixel_var = Some(dpix);
    Some(out)
}

/// The pixel's own derivative as a variable, `d_pixel`, 1 from the
/// init's first statement: what a derivative seeds from. The perturbed
/// path's floatexp rung holds the derivative as a mantissa and an
/// exponent `e`, and sets it to `2^-e` before each pass, so the
/// derivative's inhomogeneous term comes out in the mantissa's units
/// (plan, phase 4, step 9d).
fn pixel_unit(out: &mut Program) -> VarId {
    out.vars.push(Var { name: "d_pixel".to_string(), ty: Ty::Complex, span: Default::default(), stored_real: false, global: false, perturb: false, dims: Vec::new() });
    out.vars.len() - 1
}

fn set_unit(dpix: VarId) -> IStmt {
    IStmt::Assign(Place::Var(dpix), konst(1.0, 0.0))
}

/// Whether each variable follows the pixel: assigned from the pixel, the
/// iterate, or a variable that does.
fn tainted(p: &Program) -> Vec<bool> {
    let mut t = vec![false; p.vars.len()];
    loop {
        let mut changed = false;
        let mut visit = |stmts: &[IStmt], t: &mut Vec<bool>| {
            fn walk(stmts: &[IStmt], t: &mut Vec<bool>, changed: &mut bool) {
                for s in stmts {
                    match s {
                        IStmt::Assign(Place::Var(v), e) => {
                            if !t[*v] && follows(e, t) {
                                t[*v] = true;
                                *changed = true;
                            }
                        }
                        // An array follows the pixel once an element does.
                        IStmt::SetElem { var, index, value } => {
                            if !t[*var] && (follows(value, t) || follows(index, t)) {
                                t[*var] = true;
                                *changed = true;
                            }
                        }
                        IStmt::Assign(..) => {}
                        IStmt::If { arms, otherwise } => {
                            for (_, b) in arms {
                                walk(b, t, changed);
                            }
                            walk(otherwise, t, changed);
                        }
                        IStmt::While { body, .. } | IStmt::Repeat { body, .. } => walk(body, t, changed),
                    }
                }
            }
            walk(stmts, t, &mut changed);
        };
        visit(&p.init, &mut t);
        visit(&p.loop_body, &mut t);
        if !changed {
            return t;
        }
    }
}

/// Whether the iterate depends on each variable: read by an assignment
/// to the iterate, or to a variable it depends on. (A test only picks
/// which assignment runs, which a derivative does not see.)
fn needed(p: &Program) -> Vec<bool> {
    fn mark(e: &IExpr, need: &mut [bool], changed: &mut bool) {
        match &e.kind {
            IExprKind::Var(v) => {
                if !need[*v] {
                    need[*v] = true;
                    *changed = true;
                }
            }
            IExprKind::Elem(v, a) => {
                if !need[*v] {
                    need[*v] = true;
                    *changed = true;
                }
                mark(a, need, changed);
            }
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => mark(a, need, changed),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => {
                mark(a, need, changed);
                mark(b, need, changed);
            }
            _ => {}
        }
    }
    fn walk(stmts: &[IStmt], need: &mut Vec<bool>, changed: &mut bool) {
        for s in stmts {
            match s {
                IStmt::Assign(Place::Z, e) => mark(e, need, changed),
                IStmt::Assign(Place::Var(v), e) if need[*v] => mark(e, need, changed),
                IStmt::SetElem { var, index, value } if need[*var] => {
                    mark(index, need, changed);
                    mark(value, need, changed);
                }
                IStmt::Assign(..) | IStmt::SetElem { .. } => {}
                IStmt::If { arms, otherwise } => {
                    for (_, b) in arms {
                        walk(b, need, changed);
                    }
                    walk(otherwise, need, changed);
                }
                IStmt::While { body, .. } | IStmt::Repeat { body, .. } => walk(body, need, changed),
            }
        }
    }
    let mut need = vec![false; p.vars.len()];
    loop {
        let mut changed = false;
        walk(&p.init, &mut need, &mut changed);
        walk(&p.loop_body, &mut need, &mut changed);
        if !changed {
            return need;
        }
    }
}

/// Whether an expression's value follows the pixel.
fn follows(e: &IExpr, t: &[bool]) -> bool {
    match &e.kind {
        IExprKind::Var(v) => t[*v],
        IExprKind::Elem(v, a) => t[*v] || follows(a, t),
        IExprKind::Predef(Predef::Pixel | Predef::Z) => true,
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => follows(a, t),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => follows(a, t) || follows(b, t),
        _ => false,
    }
}

struct D<'a> {
    taint: &'a [bool],
    dvar: &'a [Option<VarId>],
    dz: VarId,
    dpix: VarId,
}

/// The complex type, for building derivative expressions.
fn cx(kind: IExprKind) -> IExpr {
    IExpr { kind, ty: Ty::Complex }
}

fn konst(re: f64, im: f64) -> IExpr {
    cx(IExprKind::Complex(re, im))
}

fn bin(op: BinOp, a: IExpr, b: IExpr) -> IExpr {
    cx(IExprKind::Binary(op, Box::new(a), Box::new(b)))
}

fn call(f: Builtin, a: IExpr) -> IExpr {
    cx(IExprKind::Call(f, Box::new(a)))
}

fn neg(a: IExpr) -> IExpr {
    cx(IExprKind::Unary(UnOp::Neg, Box::new(a)))
}

/// `factor * da`, the chain rule's shape.
fn times(factor: IExpr, da: IExpr) -> IExpr {
    bin(BinOp::Mul, factor, da)
}

/// `a` as a complex expression (a real constant lifted).
fn as_cx(a: &IExpr) -> IExpr {
    if a.ty == Ty::Complex {
        a.clone()
    } else {
        cx(IExprKind::Convert(Box::new(a.clone())))
    }
}

impl D<'_> {
    fn stmts(&self, stmts: &[IStmt]) -> Option<Vec<IStmt>> {
        let mut out = Vec::new();
        for s in stmts {
            match s {
                IStmt::Assign(place, e) => {
                    let target = match place {
                        Place::Z => Some(self.dz),
                        Place::Var(v) => self.dvar[*v],
                        _ => None,
                    };
                    if let Some(dv) = target {
                        // A complex place the iterate depends on: its
                        // derivative first, from the values before this
                        // assignment.
                        let de = self.d(e).ok()?.unwrap_or_else(|| konst(0.0, 0.0));
                        out.push(IStmt::Assign(Place::Var(dv), de));
                    }
                    out.push(s.clone());
                }
                // An array keeps no derivatives: one that follows the
                // pixel refuses where it is read (`d`).
                IStmt::SetElem { .. } => out.push(s.clone()),
                IStmt::If { arms, otherwise } => {
                    let mut a = Vec::new();
                    for (c, b) in arms {
                        a.push((c.clone(), self.stmts(b)?));
                    }
                    out.push(IStmt::If { arms: a, otherwise: self.stmts(otherwise)? });
                }
                IStmt::While { cond, body } => out.push(IStmt::While { cond: cond.clone(), body: self.stmts(body)? }),
                IStmt::Repeat { body, cond } => out.push(IStmt::Repeat { body: self.stmts(body)?, cond: cond.clone() }),
            }
        }
        Some(out)
    }

    /// The derivative of a complex-valued expression: `Ok(None)` when it
    /// is zero, `Err` when it does not exist.
    fn d(&self, e: &IExpr) -> Result<Option<IExpr>, ()> {
        use Builtin::*;
        // A real value (a real variable, `|x|`, `real(z)`) has no complex
        // derivative unless it is constant in the pixel.
        if e.ty != Ty::Complex {
            return if follows(e, self.taint) { Err(()) } else { Ok(None) };
        }
        let f = |x: &IExpr| follows(x, self.taint);
        Ok(match &e.kind {
            IExprKind::Complex(..) | IExprKind::Param(_) | IExprKind::FuncChoice(_) => None,
            // The perturbation equations', which are not differentiated.
            IExprKind::UserDiff(..) => return Err(()),
            IExprKind::Predef(Predef::Pixel) => Some(cx(IExprKind::Var(self.dpix))),
            IExprKind::Predef(Predef::Z) => Some(cx(IExprKind::Var(self.dz))),
            IExprKind::Predef(_) => None,
            IExprKind::Var(v) => self.dvar[*v].map(|dv| cx(IExprKind::Var(dv))),
            IExprKind::Elem(..) if f(e) => return Err(()),
            IExprKind::Elem(..) => None,
            // A real number made complex, or a complex built from parts:
            // differentiable only where it does not follow the pixel.
            IExprKind::Convert(a) | IExprKind::Modulus(a) => {
                if f(a) {
                    return Err(());
                }
                None
            }
            IExprKind::MakeComplex(a, b) => {
                if f(a) || f(b) {
                    return Err(());
                }
                None
            }
            IExprKind::Unary(UnOp::Neg, a) => self.d(a)?.map(neg),
            IExprKind::Unary(UnOp::Not, _) => None,
            IExprKind::Binary(op, a, b) => {
                let (da, db) = (self.d(a)?, self.d(b)?);
                match op {
                    BinOp::Add => match (da, db) {
                        (None, None) => None,
                        (Some(x), None) | (None, Some(x)) => Some(x),
                        (Some(x), Some(y)) => Some(bin(BinOp::Add, x, y)),
                    },
                    BinOp::Sub => match (da, db) {
                        (None, None) => None,
                        (Some(x), None) => Some(x),
                        (None, Some(y)) => Some(neg(y)),
                        (Some(x), Some(y)) => Some(bin(BinOp::Sub, x, y)),
                    },
                    BinOp::Mul => match (da, db) {
                        (None, None) => None,
                        (Some(x), None) => Some(bin(BinOp::Mul, x, as_cx(b))),
                        (None, Some(y)) => Some(bin(BinOp::Mul, as_cx(a), y)),
                        (Some(x), Some(y)) => Some(bin(BinOp::Add, bin(BinOp::Mul, x, as_cx(b)), bin(BinOp::Mul, as_cx(a), y))),
                    },
                    // (da b - a db) / b^2
                    BinOp::Div => match (da, db) {
                        (None, None) => None,
                        (Some(x), None) => Some(bin(BinOp::Div, x, as_cx(b))),
                        (dx, Some(y)) => {
                            let top = match dx {
                                Some(x) => bin(BinOp::Sub, bin(BinOp::Mul, x, as_cx(b)), bin(BinOp::Mul, as_cx(a), y)),
                                None => neg(bin(BinOp::Mul, as_cx(a), y)),
                            };
                            Some(bin(BinOp::Div, top, call(Sqr, as_cx(b))))
                        }
                    },
                    // a^b = exp(b log a): b a^(b-1) da + a^b log(a) db.
                    BinOp::Pow => {
                        let ab = bin(BinOp::Pow, as_cx(a), as_cx(b));
                        let first = da.map(|x| {
                            let pow1 = bin(BinOp::Pow, as_cx(a), bin(BinOp::Sub, as_cx(b), konst(1.0, 0.0)));
                            bin(BinOp::Mul, bin(BinOp::Mul, as_cx(b), pow1), x)
                        });
                        let second = db.map(|y| bin(BinOp::Mul, bin(BinOp::Mul, ab, call(Log, as_cx(a))), y));
                        match (first, second) {
                            (None, None) => None,
                            (Some(x), None) | (None, Some(x)) => Some(x),
                            (Some(x), Some(y)) => Some(bin(BinOp::Add, x, y)),
                        }
                    }
                    // Comparisons and logic are bools, never here.
                    _ => None,
                }
            }
            IExprKind::PowInt(a, n) => match (self.d(a)?, *n) {
                (None, _) | (_, 0) => None,
                (Some(x), 1) => Some(x),
                // The square's derivative as the built-in Mandelbrot
                // writes it, 2 z dz, so the two agree to the bit.
                (Some(x), 2) => Some(bin(BinOp::Mul, konst(2.0, 0.0), times(as_cx(a), x))),
                (Some(x), n) => {
                    let pow = cx(IExprKind::PowInt(Box::new(as_cx(a)), n - 1));
                    Some(times(bin(BinOp::Mul, konst(n as f64, 0.0), pow), x))
                }
            },
            IExprKind::Call(fun, a) => {
                let Some(da) = self.d(a)? else {
                    // Constant in the pixel: any function of it is too.
                    return Ok(None);
                };
                let a = as_cx(a);
                let one = konst(1.0, 0.0);
                let sq = |x: IExpr| call(Sqr, x);
                let factor = match fun {
                    Sqr => return Ok(Some(bin(BinOp::Mul, konst(2.0, 0.0), times(a, da)))),
                    Ident => return Ok(Some(da)),
                    Zero => return Ok(None),
                    Sqrt => bin(BinOp::Div, konst(0.5, 0.0), call(Sqrt, a)),
                    Log => bin(BinOp::Div, one, a),
                    Exp => call(Exp, a),
                    Sin => call(Cos, a),
                    Cos => neg(call(Sin, a)),
                    Tan => bin(BinOp::Div, one, sq(call(Cos, a))),
                    Cotan => neg(bin(BinOp::Div, one, sq(call(Sin, a)))),
                    Sinh => call(Cosh, a),
                    Cosh => call(Sinh, a),
                    Tanh => bin(BinOp::Div, one, sq(call(Cosh, a))),
                    Cotanh => neg(bin(BinOp::Div, one, sq(call(Sinh, a)))),
                    Recip => neg(bin(BinOp::Div, one, sq(a))),
                    Asin => bin(BinOp::Div, one.clone(), call(Sqrt, bin(BinOp::Sub, one, sq(a)))),
                    Acos => neg(bin(BinOp::Div, one.clone(), call(Sqrt, bin(BinOp::Sub, one, sq(a))))),
                    Atan => bin(BinOp::Div, one.clone(), bin(BinOp::Add, one, sq(a))),
                    Asinh => bin(BinOp::Div, one.clone(), call(Sqrt, bin(BinOp::Add, sq(a), one))),
                    Acosh => bin(
                        BinOp::Div,
                        one.clone(),
                        bin(BinOp::Mul, call(Sqrt, bin(BinOp::Sub, a.clone(), one.clone())), call(Sqrt, bin(BinOp::Add, a, one))),
                    ),
                    Atanh => bin(BinOp::Div, one.clone(), bin(BinOp::Sub, one, sq(a))),
                    Expm1 => call(Exp, a),
                    Log1p => bin(BinOp::Div, one.clone(), bin(BinOp::Add, one, a)),
                    // Folds, parts, roundings: not complex-differentiable.
                    Abs | Cabs | Real | Imag | Conj | Flip | Atan2 | Round | Trunc | Ceil | Floor | Cosxx | IsInf | IsNan => {
                        return Err(())
                    }
                };
                Some(times(factor, da))
            }
            // The user's choice decides, at run time, and it may be a fold
            // with no complex derivative: one on the pixel's path takes the
            // Jacobian, which has the right one for every choice (step 9d;
            // this pass gave a fold zero, and distance estimation went
            // flat).
            IExprKind::UserCall(_, a) => {
                if f(a) {
                    return Err(());
                }
                None
            }
            IExprKind::UserDeriv(..) | IExprKind::UserJac(..) | IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) => None,
        })
    }
}

/// The program extended with its Jacobian (the module's docs): `None`
/// when the iterate depends on the pixel through something with no
/// derivative at all (another formula's perturbation equations).
pub fn with_jacobian(p: &Program) -> Option<Program> {
    let taint = tainted(p);
    let need = needed(p);
    // An array keeps no derivatives: one the iterate needs that follows
    // the pixel has none.
    if p.vars.iter().enumerate().any(|(i, v)| v.is_array() && taint[i] && need[i]) {
        return None;
    }
    let mut out = p.clone();
    let mut dvar: Vec<Option<(VarId, VarId)>> = vec![None; p.vars.len()];
    let companion = |out: &mut Program, name: String| {
        out.vars.push(Var { name, ty: Ty::Complex, span: Default::default(), stored_real: false, global: false, perturb: false, dims: Vec::new() });
        out.vars.len() - 1
    };
    for (i, v) in p.vars.iter().enumerate() {
        // A global is one value per image: constant in the pixel. So is a
        // variable nothing pixel-dependent is ever assigned to.
        if matches!(v.ty, Ty::Complex | Ty::Float) && need[i] && taint[i] && !v.global && !v.is_array() {
            let pv = companion(&mut out, format!("d_{}", v.name));
            let qv = companion(&mut out, format!("dq_{}", v.name));
            dvar[i] = Some((pv, qv));
        }
    }
    let dz = (companion(&mut out, "d_z".to_string()), companion(&mut out, "dq_z".to_string()));
    let dpix = pixel_unit(&mut out);
    let j = J { dvar: &dvar, dz, dpix, base: out.vars.len(), temps: RefCell::new(Vec::new()), prelude: RefCell::new(Vec::new()) };
    let init = [vec![set_unit(dpix)], j.stmts(&p.init)?].concat();
    let loop_body = j.stmts(&p.loop_body)?;
    out.vars.extend(j.temps.into_inner());
    out.init = init;
    out.loop_body = loop_body;
    out.dz_var = Some(dz.0);
    out.dzq_var = Some(dz.1);
    out.dpixel_var = Some(dpix);
    Some(out)
}

/// A derivative pair `(P, Q)`, `None` for zero.
type Pair = Option<(IExpr, IExpr)>;

/// Each user-function choice's derivative at `a` as `A d + B conj(d)`,
/// statements writing `ra` and `rb` from `a` on a program of their own:
/// the rules the Jacobian takes for the function itself, applied to the
/// unit pair (1, 0), whose image is (A, B). The GPU's helper
/// (`uf_user_jac`) and the f64 model (`interp::user_jac`) run them, so
/// a user call in a Jacobian is [`IExprKind::UserJac`], not an arm per
/// choice.
pub(crate) struct UserJacForms {
    pub prog: Program,
    pub a: VarId,
    pub ra: VarId,
    pub rb: VarId,
    pub choices: Vec<Vec<IStmt>>,
}

pub(crate) fn user_jac_forms() -> &'static UserJacForms {
    static FORMS: std::sync::OnceLock<UserJacForms> = std::sync::OnceLock::new();
    FORMS.get_or_init(|| {
        // A program to hold the forms' variables, from the smallest
        // formula there is.
        let (file, _) = super::parser::parse_file("J {\nloop:\n z = z\nbailout:\n false\n}");
        let (base, _) = super::check::check(&file.entries[0], super::EntryKind::Formula, super::Dialect::UltraFractal);
        let mut prog = base.expect("the forms' program");
        prog.vars.clear();
        prog.loop_body.clear();
        let var_of = |prog: &mut Program, name: &str| {
            prog.vars.push(Var { name: name.into(), ty: Ty::Complex, span: Default::default(), stored_real: false, global: false, perturb: false, dims: Vec::new() });
            prog.vars.len() - 1
        };
        let (a, ra, rb) = (var_of(&mut prog, "a"), var_of(&mut prog, "ra"), var_of(&mut prog, "rb"));
        let none: Vec<Option<(VarId, VarId)>> = Vec::new();
        let j = J { dvar: &none, dz: (ra, rb), dpix: ra, base: prog.vars.len(), temps: RefCell::new(Vec::new()), prelude: RefCell::new(Vec::new()) };
        let unit = (konst(1.0, 0.0), konst(0.0, 0.0));
        let choices = USER_FUNCTIONS
            .iter()
            .map(|&fun| {
                let (pa, pb) = j.call_pair(fun, var(a), unit.clone()).unwrap_or_else(|| (konst(0.0, 0.0), konst(0.0, 0.0)));
                let mut code = std::mem::take(&mut *j.prelude.borrow_mut());
                code.push(IStmt::Assign(Place::Var(ra), pa));
                code.push(IStmt::Assign(Place::Var(rb), pb));
                code
            })
            .collect();
        prog.vars.extend(j.temps.into_inner());
        UserJacForms { prog, a, ra, rb, choices }
    })
}

struct J<'a> {
    dvar: &'a [Option<(VarId, VarId)>],
    dz: (VarId, VarId),
    dpix: VarId,
    /// The first temporary's variable id.
    base: usize,
    temps: RefCell<Vec<Var>>,
    /// Statements an assignment's derivative needs before it: temporaries
    /// for values read more than once, a user function's choice.
    prelude: RefCell<Vec<IStmt>>,
}

fn var(v: VarId) -> IExpr {
    cx(IExprKind::Var(v))
}

fn conj(a: IExpr) -> IExpr {
    call(Builtin::Conj, a)
}

/// Whether an expression reads a variable.
fn reads(e: &IExpr, v: VarId) -> bool {
    match &e.kind {
        IExprKind::Var(w) => *w == v,
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => reads(a, v),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => reads(a, v) || reads(b, v),
        _ => false,
    }
}

/// `sign(x)` of a real expression, +1 at zero: a fold's slope.
fn sign(x: IExpr) -> IExpr {
    let real = |kind| IExpr { kind, ty: Ty::Float };
    let ge = IExpr { kind: IExprKind::Binary(BinOp::Ge, Box::new(x), Box::new(real(IExprKind::Float(0.0)))), ty: Ty::Bool };
    let unit = real(IExprKind::Convert(Box::new(ge)));
    let twice = real(IExprKind::Binary(BinOp::Mul, Box::new(real(IExprKind::Float(2.0))), Box::new(unit)));
    as_cx(&real(IExprKind::Binary(BinOp::Sub, Box::new(twice), Box::new(real(IExprKind::Float(1.0))))))
}

fn pair_sum(a: Pair, b: Pair) -> Pair {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        (Some((p1, q1)), Some((p2, q2))) => Some((bin(BinOp::Add, p1, p2), bin(BinOp::Add, q1, q2))),
    }
}

fn pair_neg(a: Pair) -> Pair {
    a.map(|(p, q)| (neg(p), neg(q)))
}

/// `factor` times a pair: the chain rule through a holomorphic step.
fn scaled(factor: IExpr, a: Pair) -> Pair {
    a.map(|(p, q)| (times(factor.clone(), p), times(factor, q)))
}

impl J<'_> {
    /// A temporary holding `e`, assigned in the prelude; `e` itself when
    /// it is a variable or a constant already.
    fn bind(&self, e: IExpr) -> IExpr {
        if matches!(e.kind, IExprKind::Var(_) | IExprKind::Complex(..) | IExprKind::Float(_) | IExprKind::Predef(_)) {
            return e;
        }
        let mut temps = self.temps.borrow_mut();
        let id = self.base + temps.len();
        let name = format!("dj_{}", temps.len());
        temps.push(Var { name, ty: Ty::Complex, span: Default::default(), stored_real: false, global: false, perturb: false, dims: Vec::new() });
        self.prelude.borrow_mut().push(IStmt::Assign(Place::Var(id), e));
        var(id)
    }

    fn bound(&self, a: Pair) -> Pair {
        a.map(|(p, q)| (self.bind(p), self.bind(q)))
    }

    fn stmts(&self, stmts: &[IStmt]) -> Option<Vec<IStmt>> {
        let mut out = Vec::new();
        for s in stmts {
            match s {
                IStmt::Assign(place, e) => {
                    let target = match place {
                        Place::Z => Some(self.dz),
                        Place::Var(v) => self.dvar[*v],
                        _ => None,
                    };
                    if let Some((pv, qv)) = target {
                        // Both read the values from before this
                        // assignment, the companions included: where Q's
                        // reads P's companion (a conjugate swaps them), P
                        // waits in a temporary.
                        let d = self.d(e).ok()?;
                        out.append(&mut self.prelude.borrow_mut());
                        let (p, q) = d.unwrap_or_else(|| (konst(0.0, 0.0), konst(0.0, 0.0)));
                        if reads(&q, pv) {
                            let p = self.bind(p);
                            out.append(&mut self.prelude.borrow_mut());
                            out.push(IStmt::Assign(Place::Var(qv), q));
                            out.push(IStmt::Assign(Place::Var(pv), p));
                        } else {
                            out.push(IStmt::Assign(Place::Var(pv), p));
                            out.push(IStmt::Assign(Place::Var(qv), q));
                        }
                    }
                    out.push(s.clone());
                }
                IStmt::SetElem { .. } => out.push(s.clone()),
                IStmt::If { arms, otherwise } => {
                    let mut a = Vec::new();
                    for (c, b) in arms {
                        a.push((c.clone(), self.stmts(b)?));
                    }
                    out.push(IStmt::If { arms: a, otherwise: self.stmts(otherwise)? });
                }
                IStmt::While { cond, body } => out.push(IStmt::While { cond: cond.clone(), body: self.stmts(body)? }),
                IStmt::Repeat { body, cond } => out.push(IStmt::Repeat { body: self.stmts(body)?, cond: cond.clone() }),
            }
        }
        Some(out)
    }

    /// The real part's pair of a value with pair `(P, Q)`:
    /// `re(a) = (a + conj(a)) / 2`.
    fn re_pair(p: &IExpr, q: &IExpr) -> (IExpr, IExpr) {
        let half = |x| times(konst(0.5, 0.0), x);
        (half(bin(BinOp::Add, p.clone(), conj(q.clone()))), half(bin(BinOp::Add, q.clone(), conj(p.clone()))))
    }

    /// The imaginary part's: `im(a) = (a - conj(a)) / 2i`.
    fn im_pair(p: &IExpr, q: &IExpr) -> (IExpr, IExpr) {
        let by = |x| times(konst(0.0, -0.5), x);
        (by(bin(BinOp::Sub, p.clone(), conj(q.clone()))), by(bin(BinOp::Sub, q.clone(), conj(p.clone()))))
    }

    /// A function's pair at `a` (complex, lifted) from `a`'s: the
    /// holomorphic ones by their factor, the rest by their own rules.
    /// `None` inside: zero.
    fn call_pair(&self, fun: Builtin, a: IExpr, da: (IExpr, IExpr)) -> Pair {
        use Builtin::*;
        let one = konst(1.0, 0.0);
        let sq = |x: IExpr| call(Sqr, x);
        let factor = match fun {
            Sqr => times(konst(2.0, 0.0), a),
            Ident => return Some(da),
            Zero | Round | Trunc | Ceil | Floor | IsInf | IsNan => return None,
            Sqrt => bin(BinOp::Div, konst(0.5, 0.0), call(Sqrt, a)),
            Log => bin(BinOp::Div, one, a),
            Exp | Expm1 => call(Exp, a),
            Sin => call(Cos, a),
            Cos => neg(call(Sin, a)),
            Tan => bin(BinOp::Div, one, sq(call(Cos, a))),
            Cotan => neg(bin(BinOp::Div, one, sq(call(Sin, a)))),
            Sinh => call(Cosh, a),
            Cosh => call(Sinh, a),
            Tanh => bin(BinOp::Div, one, sq(call(Cosh, a))),
            Cotanh => neg(bin(BinOp::Div, one, sq(call(Sinh, a)))),
            Recip => neg(bin(BinOp::Div, one, sq(a))),
            Asin => bin(BinOp::Div, one.clone(), call(Sqrt, bin(BinOp::Sub, one, sq(a)))),
            Acos => neg(bin(BinOp::Div, one.clone(), call(Sqrt, bin(BinOp::Sub, one, sq(a))))),
            Atan => bin(BinOp::Div, one.clone(), bin(BinOp::Add, one, sq(a))),
            Asinh => bin(BinOp::Div, one.clone(), call(Sqrt, bin(BinOp::Add, sq(a), one))),
            Acosh => bin(BinOp::Div, one.clone(), bin(BinOp::Mul, call(Sqrt, bin(BinOp::Sub, a.clone(), one.clone())), call(Sqrt, bin(BinOp::Add, a, one)))),
            Atanh => bin(BinOp::Div, one.clone(), bin(BinOp::Sub, one, sq(a))),
            Log1p => bin(BinOp::Div, one.clone(), bin(BinOp::Add, one, a)),
            // The folds and the parts, by their real-linear rules.
            Conj => return Some((conj(da.1), conj(da.0))),
            // flip(a) = i conj(a).
            Flip => {
                let i = konst(0.0, 1.0);
                return Some((times(i.clone(), conj(da.1)), times(i, conj(da.0))));
            }
            Real => return Some(Self::re_pair(&da.0, &da.1)),
            Imag => return Some(Self::im_pair(&da.0, &da.1)),
            // |re| + i |im|: each part's pair by its sign.
            Abs => {
                let (pr, qr) = Self::re_pair(&da.0, &da.1);
                let (pi, qi) = Self::im_pair(&da.0, &da.1);
                let sx = self.bind(sign(IExpr { kind: IExprKind::Call(Real, Box::new(a.clone())), ty: Ty::Float }));
                let sy = self.bind(times(konst(0.0, 1.0), sign(IExpr { kind: IExprKind::Call(Imag, Box::new(a)), ty: Ty::Float })));
                return Some((bin(BinOp::Add, times(sx.clone(), pr), times(sy.clone(), pi)), bin(BinOp::Add, times(sx, qr), times(sy, qi))));
            }
            // |a| = sqrt(a conj(a)): its pair over 2|a| (a zero modulus,
            // a kink, takes the zero slope).
            Cabs => {
                let ca = self.bind(conj(a.clone()));
                let den = bin(BinOp::Add, times(konst(2.0, 0.0), as_cx(&IExpr { kind: IExprKind::Call(Cabs, Box::new(a.clone())), ty: Ty::Float })), konst(1e-30, 0.0));
                let den = self.bind(den);
                let p = bin(BinOp::Add, times(ca.clone(), da.0.clone()), times(a.clone(), conj(da.1.clone())));
                let q = bin(BinOp::Add, times(ca, da.1), times(a, conj(da.0)));
                return Some((bin(BinOp::Div, p, den.clone()), bin(BinOp::Div, q, den)));
            }
            // arg a = im(log a): the imaginary part of da / a.
            Atan2 => {
                let den = self.bind(bin(BinOp::Add, a, konst(1e-30, 0.0)));
                let w = (bin(BinOp::Div, da.0, den.clone()), bin(BinOp::Div, da.1, den));
                let (p, q) = (self.bind(w.0), self.bind(w.1));
                return Some(Self::im_pair(&p, &q));
            }
            // cosxx = conj(cos a).
            Cosxx => {
                let s = self.bind(neg(call(Sin, a)));
                return Some((conj(times(s.clone(), da.1)), conj(times(s, da.0))));
            }
        };
        let f = self.bind(factor);
        Some((times(f.clone(), da.0), times(f, da.1)))
    }

    /// The pair of an expression: `Ok(None)` when it is zero, `Err` when
    /// it does not exist.
    fn d(&self, e: &IExpr) -> Result<Pair, ()> {
        use Builtin::*;
        if matches!(e.ty, Ty::Bool | Ty::Int) {
            return Ok(None);
        }
        Ok(match &e.kind {
            IExprKind::Complex(..) | IExprKind::Float(_) | IExprKind::Int(_) | IExprKind::Bool(_) | IExprKind::Param(_) | IExprKind::FuncChoice(_) | IExprKind::UserDeriv(..) | IExprKind::UserJac(..) => None,
            // The perturbation equations', which are not differentiated.
            IExprKind::UserDiff(..) => return Err(()),
            IExprKind::Predef(Predef::Pixel) => Some((var(self.dpix), konst(0.0, 0.0))),
            IExprKind::Predef(Predef::Z) => Some((var(self.dz.0), var(self.dz.1))),
            IExprKind::Predef(_) => None,
            IExprKind::Var(v) => self.dvar[*v].map(|(p, q)| (var(p), var(q))),
            // Constant in the pixel (`with_jacobian` refuses an array that
            // is not).
            IExprKind::Elem(..) => None,
            IExprKind::Convert(a) => self.d(a)?,
            // |a|^2 = a conj(a): (conj(a) P + a conj(Q), conj(a) Q + a conj(P)).
            IExprKind::Modulus(a) => {
                let Some((pa, qa)) = self.bound(self.d(a)?) else { return Ok(None) };
                let a = as_cx(a);
                let ca = conj(a.clone());
                Some((
                    bin(BinOp::Add, times(ca.clone(), pa.clone()), times(a.clone(), conj(qa.clone()))),
                    bin(BinOp::Add, times(ca, qa), times(a, conj(pa))),
                ))
            }
            IExprKind::MakeComplex(a, b) => {
                let i = konst(0.0, 1.0);
                pair_sum(self.d(a)?, scaled(i, self.d(b)?))
            }
            IExprKind::Unary(UnOp::Neg, a) => pair_neg(self.d(a)?),
            IExprKind::Unary(UnOp::Not, _) => None,
            IExprKind::Binary(op, a, b) => {
                let (da, db) = (self.d(a)?, self.d(b)?);
                let (ca, cb) = (as_cx(a), as_cx(b));
                match op {
                    BinOp::Add => pair_sum(da, db),
                    BinOp::Sub => pair_sum(da, pair_neg(db)),
                    BinOp::Mul => pair_sum(scaled(cb, da), scaled(ca, db)),
                    // (da b - a db) / b^2
                    BinOp::Div => {
                        let first = scaled(bin(BinOp::Div, konst(1.0, 0.0), cb.clone()), da);
                        let second = db.map(|y| {
                            let k = bin(BinOp::Div, ca, call(Sqr, cb));
                            (neg(times(k.clone(), y.0)), neg(times(k, y.1)))
                        });
                        pair_sum(first, second)
                    }
                    // a^b = exp(b log a): b a^(b-1) da + a^b log(a) db.
                    BinOp::Pow => {
                        let fa = bin(BinOp::Mul, cb.clone(), bin(BinOp::Pow, ca.clone(), bin(BinOp::Sub, cb.clone(), konst(1.0, 0.0))));
                        let fb = bin(BinOp::Mul, bin(BinOp::Pow, ca.clone(), cb), call(Log, ca));
                        pair_sum(scaled(fa, da), scaled(fb, db))
                    }
                    _ => None,
                }
            }
            IExprKind::PowInt(a, n) => match (self.d(a)?, *n) {
                (None, _) | (_, 0) => None,
                (Some(x), 1) => Some(x),
                (Some(x), 2) => scaled(times(konst(2.0, 0.0), as_cx(a)), Some(x)),
                (Some(x), n) => scaled(bin(BinOp::Mul, konst(n as f64, 0.0), cx(IExprKind::PowInt(Box::new(as_cx(a)), n - 1))), Some(x)),
            },
            IExprKind::Call(fun, a) => {
                let Some(da) = self.d(a)? else {
                    return Ok(None);
                };
                // Rules reading a pair more than once read it bound.
                let da = if matches!(fun, Abs | Cabs | Real | Imag | Atan2) { self.bound(Some(da)).unwrap() } else { da };
                self.call_pair(*fun, as_cx(a), da)
            }
            // The user's choice decides, at run time: its derivative is
            // `A d + B conj(d)` (B zero for a holomorphic choice), each
            // choice's A and B by its own rule in one helper
            // (`user_jac_forms`), so the pair is A (P, Q) + B conj(Q, P).
            // An arm per choice here made a formula of a thousand user
            // calls a derivative of tens of megabytes.
            IExprKind::UserCall(id, a) => {
                let Some(da) = self.bound(self.d(a)?) else {
                    return Ok(None);
                };
                let arg = self.bind(as_cx(a));
                let jac = |b: bool| self.bind(cx(IExprKind::UserJac(*id, Box::new(arg.clone()), b)));
                let (ja, jb) = (jac(false), jac(true));
                let p = bin(BinOp::Add, times(ja.clone(), da.0.clone()), times(jb.clone(), conj(da.1.clone())));
                let q = bin(BinOp::Add, times(ja, da.1), times(jb, conj(da.0)));
                Some((p, q))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::check::check;
    use super::super::interp::{run, Env, ParamValues};
    use super::super::parser::parse_file;
    use super::super::{Dialect, EntryKind};
    use super::*;

    fn program(src: &str, dialect: Dialect) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Formula, dialect);
        assert!(!super::super::has_errors(&d), "{:?}", d.iter().map(|x| x.render(src)).collect::<Vec<_>>());
        p.unwrap()
    }

    /// The derived program's `dz` after `n` iterations at `at` against
    /// central differences of the plain program's `z`, along both axes
    /// (agreeing along both is the Cauchy-Riemann test: a complex
    /// derivative, not a real gradient).
    fn matches_differences(p: &Program, at: C64, n: u32) {
        let d = with_derivative(p).expect("differentiable");
        let plain = ParamValues::defaults(p);
        let r = run(&d, &ParamValues::defaults(&d), None, &Env::at(at, n));
        assert_eq!(r.z, run(p, &plain, None, &Env::at(at, n)).z, "the derivative changes nothing else");
        let z = |c: C64| run(p, &plain, None, &Env::at(c, n)).z;
        let h = 1e-6;
        for step in [C64::real(h), C64::new(0.0, h)] {
            let fd = z(at.add(step)).sub(z(at.sub(step))).div(step.scale(2.0));
            let err = r.dz.sub(fd).abs() / fd.abs().max(1.0);
            assert!(err < 1e-6, "{}: dz {:?}, differences {fd:?} along {step:?}", p.name, r.dz);
        }
    }

    #[test]
    fn mandelbrot_and_julia_style_derivatives() {
        let m = program("M {\ninit:\n z = 0\n c = #pixel\nloop:\n z = sqr(z) + c\nbailout:\n true\n}", Dialect::UltraFractal);
        matches_differences(&m, C64::new(-0.4, 0.3), 8);
        // Julia style: dz starts at 1 (z0 is the pixel) and the constant
        // adds nothing.
        let j = program("J {\ninit:\n z = #pixel\nloop:\n z = z^2 + (0.3, 0.5)\nbailout:\n true\n}", Dialect::UltraFractal);
        matches_differences(&j, C64::new(0.1, 0.2), 6);
        let r = run(&with_derivative(&j).unwrap(), &ParamValues::defaults(&j), None, &Env::at(C64::new(0.1, 0.2), 0));
        assert_eq!(r.dz, C64::ONE);
        // A `global:` value is the image's: constant in the pixel, and
        // no derivative variable of its own.
        let g = program(
            "G {\nglobal:\n c = (0.15, 0.25) * 2\ninit:\n z = #pixel\nloop:\n z = z^2 + c\nbailout:\n true\n}",
            Dialect::UltraFractal,
        );
        matches_differences(&g, C64::new(0.1, 0.2), 6);
        assert!(!with_derivative(&g).unwrap().vars.iter().any(|v| v.name == "d_c"));
    }

    /// Every rule at once: products, quotients, both kinds of power,
    /// functions, a user function, a branch and a loop.
    #[test]
    fn a_program_using_every_rule() {
        let p = program(
            "E {\ninit:\n z = #pixel\n w = 0.5 * z\nloop:\n w = sin(w) / (z + 3) + exp(z) * 0.1\n\
             z = z^3 * 0.2 + w - sqrt(z + 2) + log(z + 4) + cosh(z) * 0.1 + atan(z) * 0.1 + (z + 3)^(0.5, 0.1)\n\
             z = z + 2^z * 0.05 + @fn(z) * 0.1 + recip(z + 5) - z / (z^2 + 7)\n\
             if |z| > 100\n  z = z * 0.5\n endif\n\
             int k = 0\n while k < 2\n  z = z - 0.01 * tanh(z)\n  k = k + 1\n endwhile\nbailout:\n true\n\
             default:\n func fn\n  default = sin()\n endfunc\n}",
            Dialect::UltraFractal,
        );
        matches_differences(&p, C64::new(0.21, -0.33), 2);
    }

    /// Fractint's per-pixel copies of a written parameter and of `pixel`
    /// differentiate as the values they copy.
    #[test]
    fn fractint_copies_differentiate() {
        let p = program("F { c = pixel, p1 = p1 + c * 0.5, z = pixel : z = sqr(z) + p1 }", Dialect::Fractint);
        matches_differences(&p, C64::new(-0.2, 0.45), 5);
    }

    /// The folds have no complex derivative, but they have a Jacobian
    /// (phase 4, step 9d): what a colouring reads from it, `dz`, gives
    /// the escape potential's gradient, `conj(dz) z/|z|` equal to the
    /// central differences of `|z_n|` along both axes -- through abs,
    /// real, imag, conj, flip, cabs, |x|, a real variable, an argument
    /// and a user function set to a fold.
    #[test]
    fn folds_in_the_iterate_have_a_jacobian() {
        let gradient_matches = |p: &Program, at: C64, n: u32| {
            assert!(complex_derivative(p).is_none(), "{}: has a complex derivative", p.name);
            let d = with_derivative(p).expect("a Jacobian");
            assert!(d.dzq_var.is_some(), "{}", p.name);
            let plain = ParamValues::defaults(p);
            let r = run(&d, &ParamValues::defaults(&d), None, &Env::at(at, n));
            assert_eq!(r.z, run(p, &plain, None, &Env::at(at, n)).z, "the derivative changes nothing else");
            let m = |c: C64| run(p, &plain, None, &Env::at(c, n)).z.abs();
            let h = 1e-7;
            let g = C64::new((m(at.add(C64::real(h))) - m(at.sub(C64::real(h)))) / (2.0 * h), (m(at.add(C64::new(0.0, h))) - m(at.sub(C64::new(0.0, h)))) / (2.0 * h));
            let ours = r.dz.conj().mul(r.z.scale(1.0 / r.z.abs()));
            let err = ours.sub(g).abs() / g.abs().max(1.0);
            assert!(err < 1e-5, "{}: the gradient {ours:?} against the differences' {g:?}", p.name);
        };
        for (src, at, n) in [
            ("B {\ninit:\n z = 0\nloop:\n z = sqr(abs(z)) + #pixel\nbailout:\n true\n}", C64::new(-0.55, -0.42), 6),
            ("R {\ninit:\n z = 0\nloop:\n z = sqr(z) + real(#pixel) + (0, 0.3)\nbailout:\n true\n}", C64::new(-0.4, 0.2), 6),
            ("T {\ninit:\n z = 0\nloop:\n z = sqr(conj(z)) + #pixel\nbailout:\n true\n}", C64::new(-0.3, 0.4), 6),
            ("F {\ninit:\n z = #pixel\nloop:\n z = flip(z) * z * 0.7 + #pixel\nbailout:\n true\n}", C64::new(0.2, -0.35), 5),
            ("I {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel + (0, 0.2) * imag(z) * cabs(z)\nbailout:\n true\n}", C64::new(-0.6, 0.3), 6),
            // A real variable that follows the pixel.
            ("V {\ninit:\n z = 0\nloop:\n float r = |z|\n z = sqr(z) + #pixel * (1 + 0.1 * r)\nbailout:\n true\n}", C64::new(-0.4, 0.5), 6),
            ("A {\ninit:\n z = #pixel\nloop:\n z = z * z * 0.9 + #pixel * (1 + 0.05 * atan2(z))\nbailout:\n true\n}", C64::new(0.3, 0.4), 4),
            ("U {\ninit:\n z = 0\nloop:\n z = sqr(@fn1(z)) + #pixel\nbailout:\n true\ndefault:\n func fn1\n  default = abs()\n endfunc\n}", C64::new(-0.55, -0.42), 6),
        ] {
            gradient_matches(&program(src, Dialect::UltraFractal), at, n);
        }
        // A fold the iterate does not depend on costs nothing: only the
        // bailout reads it.
        let p = program(
            "U {\ninit:\n z = 0\nloop:\n zold = abs(z)\n z = sqr(z) + #pixel\nbailout:\n |z - zold| < 100\n}",
            Dialect::UltraFractal,
        );
        let d = with_derivative(&p).expect("the fold is beside the iterate");
        assert!(!d.vars.iter().any(|v| v.name == "d_zold"));
        matches_differences(&p, C64::new(-0.6, 0.2), 4);
    }

    /// Each user-function choice's derivative, `A d + B conj(d)`
    /// (`user_jac_forms`), against central differences of the function
    /// itself along 1 and i.
    #[test]
    fn every_user_choice_has_its_jacobian() {
        let a = C64::new(0.37, 0.29);
        let h = 1e-6;
        for (k, f) in USER_FUNCTIONS.iter().enumerate() {
            let (ja, jb) = (super::super::interp::user_jac(k, a, false), super::super::interp::user_jac(k, a, true));
            for d in [C64::real(1.0), C64::new(0.0, 1.0)] {
                let fd = f.eval_c(a.add(d.scale(h))).sub(f.eval_c(a.sub(d.scale(h)))).scale(0.5 / h);
                let ours = ja.mul(d).add(jb.mul(d.conj()));
                assert!(ours.sub(fd).abs() < 1e-5 * fd.abs().max(1.0), "{}: {ours:?} against {fd:?} along {d:?}", f.name());
            }
        }
    }

    /// A formula of parameter branches full of user calls, as UF's
    /// largest are (`as.ufm`'s DaddyBrot: a derivative of 16.7 MB of WGSL
    /// for 435 KB of formula, which validating took minutes over): a user
    /// call costs its derivative what a built-in call does, not an arm
    /// per choice; and past [`MAX_DERIVATIVE_VARIABLES`] the formula goes
    /// without.
    #[test]
    fn a_user_call_costs_the_derivative_what_a_builtin_does() {
        let chained = |blocks: usize, f: [&str; 3]| {
            let mut s = String::from("C {\ninit:\n z = #pixel\n x = z\nloop:\n");
            for _ in 0..blocks {
                s.push_str(&format!(" if @mode == 0\n  x = {}(z^@a) + {}(z^@b)\n", f[0], f[1]));
                for (m, op) in ["-", "*", "/"].iter().enumerate() {
                    s.push_str(&format!(" elseif @mode == {}\n  x = {}(z^@a) {op} {}(z^@b)\n", m + 1, f[0], f[1]));
                }
                s.push_str(&format!(" endif\n z = {}(x) * 0.5 + #pixel\n", f[2]));
            }
            s.push_str(
                "bailout:\n |z| < 4\ndefault:\n int param mode\n  default = 0\n endparam\n complex param a\n  default = (2, 0)\n endparam\n complex param b\n  default = (3, 0)\n endparam\n}",
            );
            program(&s, Dialect::UltraFractal)
        };
        let size = |p: &Program| super::super::wgsl::formula_with_derivative(p).expect("a derivative").len();
        // Against folds, which take the Jacobian as any user call does.
        let (user, builtin) = (size(&chained(10, ["fn1", "@fn2", "fn3"])), size(&chained(10, ["abs", "flip", "abs"])));
        // 1.5 times now (a user call binds its A and B); an arm per choice
        // for each call made it several times the size.
        assert!((user as f64) < 2.0 * builtin as f64, "with user calls {user} B, with folds {builtin} B");
        assert!(with_derivative(&chained(200, ["fn1", "@fn2", "fn3"])).is_none(), "past the cap");
    }
}
