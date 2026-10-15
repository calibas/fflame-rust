//! A formula's perturbation equations derived from its own loop (plan,
//! phase 3, step 4), for the formulas that carry no `perturbinit:` /
//! `perturbloop:` -- most of UF's and every Fractint one.
//!
//! Every value of the loop becomes a pair: its value at the reference
//! (Full, computed from the reference's `#z` and the reference point
//! `#pixel - #dpixel`) and the pixel's deviation from it (Small, from
//! `#dz` and `#dpixel`). The deviations follow the rules the built-in
//! tiers' hand derivations use, each free of the cancellation a direct
//! difference `f(Z + d) - f(Z)` would suffer:
//!
//! - `d(a + b) = da + db`, `d(-a) = -da`
//! - `d(a b) = da B + A db + da db`
//! - `d(a / b) = (da - (A/B) db) / (B + db)` (the quotient form)
//! - `d(a^n)` the binomial sum, `d(sqr a) = 2 A da + da^2`
//! - `d(1/a) = -(da / A) / (A + da)`
//! - `d|a|^2 = 2 (re A re da + im A im da) + |da|^2`
//! - `conj`, `flip`, `real`, `imag`: the same of the deviation.
//! - `d|a| = diffabs(A, da)`, `|A + da| - |A|` by sign case (laser
//!   blaster's, the built-in Ship tiers'), per part for a complex.
//! - The transcendental functions by their difference forms (phase 4,
//!   step 6), over `expm1` and `log1p`, which are exact near zero:
//!   `d exp a = exp A expm1 da`, `d log a = log1p(da / A)`,
//!   `d sqrt a = da / (sqrt(A + da) + sqrt A)`, `d sin a = 2 cos(A +
//!   da/2) sin(da/2)` (cos, sinh, cosh alike), `d tan a = sin da / (cos A
//!   cos(A + da))` (cotan, tanh, cotanh alike), `d cabs a = (2 Re(conj A
//!   da) + |da|^2) / (|A + da| + |A|)`, and a power the same for every
//!   pixel `d a^p = A^p expm1(p log1p(da / A))`.
//!
//! The result is ordinary perturbation sections over new variables of
//! the equations (a pair per loop temporary, and one for z once a pass
//! may have written it), which the scaled and floatexp code generators
//! compile as they do a formula's own. A branch in the loop derives
//! when its conditions are the same for every pixel -- a parameter's
//! choice, a constant -- so the reference and the pixel take the same
//! arm. So does one in the init (phase 4, step 4: the plane, which every
//! plane-aware formula branches on): each way through the init's
//! branches is a case, with its own values for what the loop reads, and
//! the equations branch on the cases' tests, one copy of the loop's
//! each.
//!
//! **State carried besides z** (phase 4, step 5): a float or complex
//! the loop writes, and reads in a pass before that pass has certainly
//! written it (a Phoenix's previous z, a Spider's moving c) or the test
//! reads (a convergent formula's previous z), is followed as z is. The
//! reference records it each pass (`Program::carried`), the equations
//! read that record and keep the pixel's deviation from it from pass to
//! pass, and a rebase moves the deviation as it moves `#dz`. The GPU
//! carries one such value (`lang::perturb` refuses more).
//!
//! A form that divides by something zero where the reference value is
//! (the power's `log1p(da / A)`, sqrt's and cabs's sums) is **guarded**:
//! the plain difference where `|da| >= |A|`, which has nothing to cancel
//! there, the form otherwise, chosen per pixel ([`Deriver::guarded`]).
//!
//! What has no such form -- an inverse function, a power of a varying
//! exponent, a user function of the iterate, a branch on the iterate or
//! the pixel, a loop inside the formula's loop or in its init, an int or
//! bool carried -- leaves the formula without equations, on the direct
//! path as before.
//! Unless it stands in an arm (or an init's case) a parameter's choice
//! picks: then the equations are derived without it, and hold when that
//! arm is not taken, which the derived `perturb` setting says. In a
//! chain of tests, one that varies after tests that do not makes the
//! rest of the chain such an arm, as `else` + `if` writes it.

use super::ast::{BinOp, UnOp};
use super::builtins::Builtin;
use super::ir::*;

/// The formula with perturbation equations derived from its loop, or
/// `None` when it has its own or they cannot be derived.
pub fn with_derived_equations(p: &Program) -> Option<Program> {
    if !p.perturb_loop.is_empty() || !p.perturb_init.is_empty() {
        return None;
    }
    let mut d = Deriver {
        p,
        out: p.clone(),
        pairs: vec![None; p.vars.len()],
        inits: Vec::new(),
        z_ref: None,
        z_written: false,
        uniforms: vec![None; p.vars.len()],
        path: Vec::new(),
        excluded: Vec::new(),
        carried: vec![None; p.vars.len()],
        pending: Vec::new(),
        guard_vars: Vec::new(),
    };
    // What the loop carries besides z: a float or complex written in it
    // and read in a pass before that pass has certainly written it, or
    // read by the test (a convergent formula's previous z). The reference
    // records it each pass; the pixel keeps its deviation.
    let mut flow_of_loop = super::check::Flow::default();
    super::check::flow(&p.loop_body, &mut flow_of_loop);
    let mut flow_of_test = super::check::Flow::default();
    if let Some(t) = &p.bailout {
        super::check::reads(t, &mut flow_of_test);
    }
    let mut carried: Vec<VarId> = flow_of_loop
        .exposed
        .union(&flow_of_test.exposed)
        .copied()
        .filter(|v| flow_of_loop.any_write.contains(v) && !p.vars[*v].global)
        .collect();
    carried.sort_unstable();
    for v in carried {
        let ty = p.vars[v].ty;
        if !matches!(ty, Ty::Float | Ty::Complex) {
            return None;
        }
        let name = p.vars[v].name.clone();
        let reference = d.new_var(format!("{name}_record"), ty);
        let current = d.new_var(format!("{name}_cur"), ty);
        let deviation = d.new_var(format!("{name}_carried"), ty);
        d.carried[v] = Some((reference, current, deviation));
        d.out.carried.push(Carried { var: v, reference, deviation });
    }
    // Each way through the init: its variables' values (constant per
    // pixel, so the loop's reads of them are re-derived in each pass),
    // z's start, and the tests that pick it.
    let cases = init_cases(p)?;
    let (mut tests, mut inits, mut loops) = (Vec::new(), Vec::new(), Vec::new());
    let mut derived_any = false;
    for case in &cases {
        // The same for every pixel, so the reference and the pixel take
        // the same way.
        let mut test: Option<IExpr> = None;
        for c in &case.conds {
            let (r, dev) = d.pair(c, &[], false)?;
            if dev.is_some() || r.ty != Ty::Bool {
                return None;
            }
            test = Some(and(test, r));
        }
        let test = test.unwrap_or_else(|| mk(IExprKind::Bool(true), Ty::Bool));
        d.inits = case.values.clone();
        d.path = vec![test.clone()];
        let derived = d.block(&p.loop_body, true).and_then(|l| Some((l, d.init_equations(case)?)));
        match derived {
            Some((l, i)) => {
                loops.push(l);
                inits.push(i);
                derived_any = true;
            }
            // A case a parameter picks may go without: the equations
            // hold when it is not taken.
            None if per_image(&test) && cases.len() > 1 => {
                d.excluded.push(test.clone());
                loops.push(Vec::new());
                inits.push(Vec::new());
            }
            None => return None,
        }
        tests.push(test);
    }
    if !derived_any {
        return None;
    }
    let excluded = std::mem::take(&mut d.excluded);
    let mut out = d.out;
    out.perturb_init = by_case(&tests, inits);
    out.perturb_loop = by_case(&tests, loops);
    // Where an arm went without equations, they hold when it is not
    // taken.
    if let Some(any) = excluded.into_iter().reduce(|a, b| mk(IExprKind::Binary(BinOp::Or, Box::new(a), Box::new(b)), Ty::Bool)) {
        let holds = mk(IExprKind::Unary(UnOp::Not, Box::new(any)), Ty::Bool);
        out.perturb_when = Some(match out.perturb_when.take() {
            Some(w) => and(Some(w), holds),
            None => holds,
        });
    }
    // Belt and braces: the equations read only their own variables and
    // globals, as the checker holds a formula's own to.
    let foreign = |stmts: &[IStmt]| stmts.iter().any(|s| stmt_reads_foreign(s, &out));
    if foreign(&out.perturb_init) || foreign(&out.perturb_loop) {
        return None;
    }
    Some(out)
}

fn stmt_reads_foreign(s: &IStmt, p: &Program) -> bool {
    fn e(x: &IExpr, p: &Program) -> bool {
        match &x.kind {
            IExprKind::Var(v) => !(p.vars[*v].perturb || p.vars[*v].global),
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => e(a, p),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => e(a, p) || e(b, p),
            _ => false,
        }
    }
    match s {
        IStmt::Assign(Place::Var(v), x) => !(p.vars[*v].perturb) || e(x, p),
        IStmt::Assign(_, x) => e(x, p),
        IStmt::If { arms, otherwise } => {
            arms.iter().any(|(c, b)| e(c, p) || b.iter().any(|s| stmt_reads_foreign(s, p))) || otherwise.iter().any(|s| stmt_reads_foreign(s, p))
        }
        IStmt::While { cond, body } | IStmt::Repeat { body, cond } => e(cond, p) || body.iter().any(|s| stmt_reads_foreign(s, p)),
    }
}

/// A value and its deviation: `None` when the deviation is zero.
type Pair = (IExpr, Option<IExpr>);

struct Deriver<'p> {
    p: &'p Program,
    out: Program,
    /// The pair of variables standing for each loop temporary.
    pairs: Vec<Option<(VarId, VarId)>>,
    /// Each init-only variable's value expression.
    inits: Vec<Option<IExpr>>,
    /// The loop's z at the reference once a pass may have written it
    /// (`#z` is the value before the step); its deviation is `#dz`.
    z_ref: Option<VarId>,
    /// Whether the statement being derived reads z after a write.
    z_written: bool,
    /// The variable standing for each int or bool temporary of the
    /// loop: its value is the same for every pixel (it has no
    /// deviation), so the equations keep it once.
    uniforms: Vec<Option<VarId>>,
    /// The tests of the arms (and the init's case) the statement being
    /// derived stands in.
    path: Vec<IExpr>,
    /// The tests under which the equations do not hold: an arm, picked
    /// by the parameters, that went without.
    excluded: Vec<IExpr>,
    /// Each carried variable's equation variables: the reference's
    /// record of it at the pass's start, its current value at the
    /// reference through the pass, and the pixel's deviation.
    carried: Vec<Option<(VarId, VarId, VarId)>>,
    /// Statements an expression's deviation needs before the statement
    /// holding it ([`Deriver::guarded`]).
    pending: Vec<IStmt>,
    /// The guards' temporaries by type, and how many of each the
    /// statement being derived has taken: a statement's guards are
    /// spent by it, so the next reuses them.
    guard_vars: Vec<(Ty, Vec<VarId>, usize)>,
}

/// `a && b`, `b` alone when there is no `a`.
fn and(a: Option<IExpr>, b: IExpr) -> IExpr {
    match a {
        None => b,
        Some(a) => mk(IExprKind::Binary(BinOp::And, Box::new(a), Box::new(b)), Ty::Bool),
    }
}

/// Whether a test reads only what the `perturb` setting can: the
/// parameters, the constants and the image's values -- no variable, no
/// pass.
fn per_image(e: &IExpr) -> bool {
    match &e.kind {
        IExprKind::Var(_) | IExprKind::Predef(Predef::Pass) => false,
        IExprKind::Predef(q) => matches!(
            q,
            Predef::Pi | Predef::E | Predef::Center | Predef::Magn | Predef::MaxIter | Predef::JuliaC | Predef::IsMand | Predef::MagXMag | Predef::RotSkew
        ),
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => per_image(a),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => per_image(a) && per_image(b),
        _ => true,
    }
}

/// One code per case of the init: the first's when they are alike, else
/// a branch on the cases' tests, the last case the `else`.
fn by_case(tests: &[IExpr], codes: Vec<Vec<IStmt>>) -> Vec<IStmt> {
    let alike = codes.windows(2).all(|w| format!("{:?}", w[0]) == format!("{:?}", w[1]));
    let n = codes.len();
    if alike || n < 2 {
        return codes.into_iter().next().unwrap_or_default();
    }
    let mut arms = Vec::new();
    let mut otherwise = Vec::new();
    for (k, code) in codes.into_iter().enumerate() {
        if k + 1 < n {
            arms.push((tests[k].clone(), code));
        } else {
            otherwise = code;
        }
    }
    vec![IStmt::If { arms, otherwise }]
}

/// The most ways through an init's branches the equations take: a copy
/// of the loop's equations each.
const MAX_INIT_CASES: usize = 16;

/// Past this many nodes an expression has stopped being a formula's
/// constant and started being a substitution's blowup.
const MAX_NODES: usize = 2000;

/// One way through the init: the arm it takes at each branch, chosen by
/// conditions the same for every pixel (the plane, a parameter), and
/// what the init leaves behind that way.
#[derive(Clone)]
pub(crate) struct InitCase {
    /// What picks this way: tests, all true, as the init has its values
    /// at each branch.
    pub conds: Vec<IExpr>,
    /// Each variable's value after the init (as [`init_values`]).
    pub values: Vec<Option<IExpr>>,
    poisoned: Vec<bool>,
    /// z's start; `None` while unset (zero).
    pub z: Option<IExpr>,
    /// z's start reads what has no value.
    z_poisoned: bool,
}

impl InitCase {
    /// `e` as this way has it so far: the variables and z by their
    /// values.
    fn value_of(&self, e: &IExpr, p: &Program, ok: &mut bool) -> IExpr {
        let v = subst(e, &self.values, &self.poisoned, p, ok);
        if !reads_z(&v) {
            return v;
        }
        if self.z_poisoned {
            *ok = false;
            return v;
        }
        subst_z(&v, &self.z.clone().unwrap_or_else(|| zero_of(Ty::Complex)))
    }
}

/// The init's ways through its branches, one when it has none. `None`
/// when it loops, or has more ways than [`MAX_INIT_CASES`].
pub(crate) fn init_cases(p: &Program) -> Option<Vec<InitCase>> {
    let start = InitCase { conds: Vec::new(), values: vec![None; p.vars.len()], poisoned: vec![false; p.vars.len()], z: None, z_poisoned: false };
    walk_init(&p.init, vec![start], p)
}

fn walk_init(stmts: &[IStmt], mut cases: Vec<InitCase>, p: &Program) -> Option<Vec<InitCase>> {
    for s in stmts {
        match s {
            IStmt::Assign(place, e) => {
                for c in &mut cases {
                    let mut ok = true;
                    let value = c.value_of(e, p, &mut ok);
                    let good = ok && nodes(&value) <= MAX_NODES;
                    match place {
                        Place::Var(v) => {
                            c.values[*v] = good.then_some(value);
                            c.poisoned[*v] = !good;
                        }
                        Place::Z => {
                            c.z = good.then_some(value);
                            c.z_poisoned = !good;
                        }
                        _ => {}
                    }
                }
            }
            IStmt::If { arms, otherwise } => {
                let mut next = Vec::new();
                for c in cases {
                    // Each arm: the earlier tests false, its own true.
                    let mut earlier: Vec<IExpr> = Vec::new();
                    for (cond, body) in arms {
                        let mut ok = true;
                        let t = c.value_of(cond, p, &mut ok);
                        if !ok {
                            return None;
                        }
                        let mut case = c.clone();
                        case.conds.extend(earlier.iter().cloned());
                        case.conds.push(t.clone());
                        next.extend(walk_init(body, vec![case], p)?);
                        earlier.push(mk(IExprKind::Unary(UnOp::Not, Box::new(t)), Ty::Bool));
                    }
                    let mut case = c;
                    case.conds.extend(earlier);
                    next.extend(walk_init(otherwise, vec![case], p)?);
                    if next.len() > MAX_INIT_CASES {
                        return None;
                    }
                }
                cases = next;
            }
            IStmt::While { .. } | IStmt::Repeat { .. } => return None,
        }
    }
    Some(cases)
}

/// `e` with z read as `z`.
fn subst_z(e: &IExpr, z: &IExpr) -> IExpr {
    let b = |a: &IExpr| Box::new(subst_z(a, z));
    let kind = match &e.kind {
        IExprKind::Predef(Predef::Z) => return z.clone(),
        IExprKind::Convert(a) => IExprKind::Convert(b(a)),
        IExprKind::MakeComplex(x, y) => IExprKind::MakeComplex(b(x), b(y)),
        IExprKind::Unary(op, a) => IExprKind::Unary(*op, b(a)),
        IExprKind::Binary(op, x, y) => IExprKind::Binary(*op, b(x), b(y)),
        IExprKind::PowInt(a, n) => IExprKind::PowInt(b(a), *n),
        IExprKind::Modulus(a) => IExprKind::Modulus(b(a)),
        IExprKind::Call(f, a) => IExprKind::Call(*f, b(a)),
        IExprKind::UserCall(f, a) => IExprKind::UserCall(*f, b(a)),
        IExprKind::UserDeriv(f, a) => IExprKind::UserDeriv(*f, b(a)),
        other => other.clone(),
    };
    mk(kind, e.ty)
}

/// Each variable's value after the init, as an expression of the
/// pixel, the parameters and the constants: the init's straight-line
/// assignments substituted in order, so `c = #pixel` then `c = c * 2`
/// gives `#pixel * 2`. A variable read before it is assigned holds
/// zero (UF's); one whose value reads z, or grows past a bound, has
/// none. `None` when the init has a branch or a loop.
pub(crate) fn init_values(p: &Program) -> Option<Vec<Option<IExpr>>> {
    let mut out: Vec<Option<IExpr>> = vec![None; p.vars.len()];
    let mut poisoned = vec![false; p.vars.len()];
    for s in &p.init {
        match s {
            IStmt::Assign(Place::Var(v), e) => {
                let mut ok = true;
                let value = subst(e, &out, &poisoned, p, &mut ok);
                if ok && !reads_z(&value) && nodes(&value) <= MAX_NODES {
                    out[*v] = Some(value);
                    poisoned[*v] = false;
                } else {
                    out[*v] = None;
                    poisoned[*v] = true;
                }
            }
            IStmt::Assign(..) => {}
            _ => return None,
        }
    }
    Some(out)
}

/// `e` with each non-global variable replaced by its value so far.
pub(crate) fn subst(e: &IExpr, values: &[Option<IExpr>], poisoned: &[bool], p: &Program, ok: &mut bool) -> IExpr {
    let b = |a: &IExpr, ok: &mut bool| Box::new(subst(a, values, poisoned, p, ok));
    let kind = match &e.kind {
        IExprKind::Var(v) if !p.vars[*v].global => {
            if poisoned[*v] {
                *ok = false;
                return e.clone();
            }
            return values[*v].clone().unwrap_or_else(|| match e.ty {
                Ty::Bool => mk(IExprKind::Bool(false), Ty::Bool),
                Ty::Int => mk(IExprKind::Int(0), Ty::Int),
                t => zero_of(t),
            });
        }
        IExprKind::Convert(a) => IExprKind::Convert(b(a, ok)),
        IExprKind::MakeComplex(x, y) => IExprKind::MakeComplex(b(x, ok), b(y, ok)),
        IExprKind::Unary(op, a) => IExprKind::Unary(*op, b(a, ok)),
        IExprKind::Binary(op, x, y) => IExprKind::Binary(*op, b(x, ok), b(y, ok)),
        IExprKind::PowInt(a, n) => IExprKind::PowInt(b(a, ok), *n),
        IExprKind::Modulus(a) => IExprKind::Modulus(b(a, ok)),
        IExprKind::Call(f, a) => IExprKind::Call(*f, b(a, ok)),
        IExprKind::UserCall(f, a) => IExprKind::UserCall(*f, b(a, ok)),
        IExprKind::UserDeriv(f, a) => IExprKind::UserDeriv(*f, b(a, ok)),
        other => other.clone(),
    };
    mk(kind, e.ty)
}

fn nodes(e: &IExpr) -> usize {
    1 + match &e.kind {
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => nodes(a),
        IExprKind::MakeComplex(a, c) | IExprKind::Binary(_, a, c) => nodes(a) + nodes(c),
        _ => 0,
    }
}

fn mk(kind: IExprKind, ty: Ty) -> IExpr {
    IExpr { kind, ty }
}

fn bin(op: BinOp, a: IExpr, b: IExpr) -> IExpr {
    let ty = a.ty.join(b.ty);
    mk(IExprKind::Binary(op, Box::new(a), Box::new(b)), ty)
}

fn neg(a: IExpr) -> IExpr {
    let ty = a.ty;
    mk(IExprKind::Unary(UnOp::Neg, Box::new(a)), ty)
}

fn call(f: Builtin, a: IExpr, ty: Ty) -> IExpr {
    mk(IExprKind::Call(f, Box::new(a)), ty)
}

fn konst(v: f64, ty: Ty) -> IExpr {
    match ty {
        Ty::Complex => mk(IExprKind::Complex(v, 0.0), Ty::Complex),
        _ => mk(IExprKind::Float(v), Ty::Float),
    }
}

/// `a` widened to `ty` (complex from float), as the checker writes it.
fn widen(a: IExpr, ty: Ty) -> IExpr {
    if a.ty == ty || ty != Ty::Complex {
        a
    } else {
        mk(IExprKind::Convert(Box::new(a)), Ty::Complex)
    }
}

fn add(a: Option<IExpr>, b: Option<IExpr>, ty: Ty) -> Option<IExpr> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(widen(x, ty)),
        (Some(x), Some(y)) => Some(bin(BinOp::Add, widen(x, ty), widen(y, ty))),
    }
}

fn zero_of(ty: Ty) -> IExpr {
    konst(0.0, ty)
}

fn binomial(n: i64, k: i64) -> f64 {
    let mut acc = 1.0f64;
    for i in 0..k {
        acc = acc * (n - i) as f64 / (i + 1) as f64;
    }
    acc
}

impl Deriver<'_> {
    fn new_var(&mut self, name: String, ty: Ty) -> VarId {
        self.out.vars.push(Var { name, ty, span: Default::default(), stored_real: false, global: false, perturb: true });
        self.out.vars.len() - 1
    }

    /// A deviation by `form` where the reference value `a` is large
    /// against the deviation `d`, else the plain difference `plain`,
    /// chosen per pixel: for a form that divides by something zero where
    /// `a` is (a real power's `log1p(d / A)`, sqrt's sum), where the plain
    /// difference has nothing to cancel. Left pending, before the
    /// statement holding it.
    fn guarded(&mut self, a: &IExpr, d: &IExpr, plain: IExpr, form: IExpr, ty: Ty) -> IExpr {
        let t = self.guard_var(ty);
        let big = mk(
            IExprKind::Binary(BinOp::Ge, Box::new(mk(IExprKind::Modulus(Box::new(d.clone())), Ty::Float)), Box::new(mk(IExprKind::Modulus(Box::new(a.clone())), Ty::Float))),
            Ty::Bool,
        );
        self.pending.push(IStmt::If { arms: vec![(big, vec![IStmt::Assign(Place::Var(t), plain)])], otherwise: vec![IStmt::Assign(Place::Var(t), form)] });
        mk(IExprKind::Var(t), ty)
    }

    /// A temporary for a guard of the statement being derived.
    fn guard_var(&mut self, ty: Ty) -> VarId {
        let k = match self.guard_vars.iter().position(|(t, ..)| *t == ty) {
            Some(k) => k,
            None => {
                self.guard_vars.push((ty, Vec::new(), 0));
                self.guard_vars.len() - 1
            }
        };
        let used = self.guard_vars[k].2;
        let v = match self.guard_vars[k].1.get(used) {
            Some(&v) => v,
            None => {
                let v = self.new_var("diff".into(), ty);
                self.guard_vars[k].1.push(v);
                v
            }
        };
        self.guard_vars[k].2 += 1;
        v
    }

    /// A statement's derivation starts: no guards pending, every
    /// temporary free.
    fn begin_statement(&mut self) {
        self.pending.clear();
        for g in &mut self.guard_vars {
            g.2 = 0;
        }
    }

    /// A case's init equations: the deviation z starts with, and each
    /// carried variable's.
    fn init_equations(&mut self, case: &InitCase) -> Option<Vec<IStmt>> {
        if case.z_poisoned {
            return None;
        }
        let mut out = Vec::new();
        if let Some(z) = &case.z {
            self.begin_statement();
            let (_, dz) = self.pair(z, &[], false)?;
            out.append(&mut self.pending);
            out.push(IStmt::Assign(Place::DZ, dz.map_or_else(|| zero_of(Ty::Complex), |d| widen(d, Ty::Complex))));
        }
        for v in 0..self.carried.len() {
            let Some((_, _, dev)) = self.carried[v] else { continue };
            let ty = self.p.vars[v].ty;
            if case.poisoned[v] {
                return None;
            }
            self.begin_statement();
            let d = match &case.values[v] {
                Some(value) => self.pair(value, &[], false)?.1,
                None => None,
            };
            out.append(&mut self.pending);
            out.push(IStmt::Assign(Place::Var(dev), d.map_or_else(|| zero_of(ty), |d| widen(d, ty))));
        }
        Some(out)
    }

    fn z_ref_var(&mut self) -> VarId {
        match self.z_ref {
            Some(v) => v,
            None => {
                let v = self.new_var("z_ref".into(), Ty::Complex);
                self.z_ref = Some(v);
                v
            }
        }
    }

    /// The equations for a block: the loop (`is_loop`) or the init.
    /// Each temporary becomes a pair of the equations' variables; the
    /// write to z becomes the write to `#dz`.
    fn block(&mut self, body: &[IStmt], is_loop: bool) -> Option<Vec<IStmt>> {
        let mut written = Vec::new();
        let mut wrote_z = false;
        let mut out = self.stmts(body, is_loop, &mut written, &mut wrote_z)?;
        if is_loop && !wrote_z {
            return None;
        }
        if let Some(zr) = self.z_ref.filter(|_| is_loop) {
            if out.iter().any(|s| stmt_reads_var(s, zr)) {
                // Where no arm wrote z, its value is the step's start.
                out.insert(0, IStmt::Assign(Place::Var(zr), mk(IExprKind::Predef(Predef::Z), Ty::Complex)));
            } else {
                strip_assigns(&mut out, zr);
            }
        }
        if is_loop {
            // Each carried variable's current value at the reference
            // starts as the record of it, where anything reads it.
            for v in (0..self.carried.len()).rev() {
                let Some((record, current, _)) = self.carried[v] else { continue };
                if out.iter().any(|s| stmt_reads_var(s, current)) {
                    out.insert(0, IStmt::Assign(Place::Var(current), mk(IExprKind::Var(record), self.p.vars[v].ty)));
                } else {
                    strip_assigns(&mut out, current);
                }
            }
        }
        Some(out)
    }

    /// `body`'s equations, after `written` (the variables this pass has
    /// written, whose pairs hold their values) and `wrote_z`. A branch
    /// is the same branch at the reference and the pixel when its
    /// conditions are the same for every pixel (a parameter's choice, a
    /// constant): its arms are derived each in turn, and a variable is
    /// written after it when written before or in every arm.
    fn stmts(&mut self, body: &[IStmt], is_loop: bool, written: &mut Vec<VarId>, wrote_z: &mut bool) -> Option<Vec<IStmt>> {
        let mut out = Vec::new();
        for s in body {
            match s {
                IStmt::Assign(place, e) => {
                    // The init's equations have z only as it ends.
                    if !is_loop && *wrote_z && reads_z(e) {
                        return None;
                    }
                    self.z_written = is_loop && *wrote_z;
                    self.begin_statement();
                    match place {
                        Place::Z => {
                            if !is_loop && *wrote_z {
                                return None;
                            }
                            let (r, dz) = self.pair(e, written, is_loop)?;
                            out.append(&mut self.pending);
                            out.push(IStmt::Assign(Place::DZ, dz.map_or_else(|| zero_of(Ty::Complex), |d| widen(d, Ty::Complex))));
                            if is_loop {
                                // Its reference value, for what reads z after
                                // this (dropped when nothing does).
                                let zr = self.z_ref_var();
                                out.push(IStmt::Assign(Place::Var(zr), widen(r, Ty::Complex)));
                            }
                            *wrote_z = true;
                        }
                        Place::Var(v) => {
                            if !is_loop {
                                // An init variable: the loop re-derives it
                                // from its value; the init's equations need
                                // it only if z's start reads it, which
                                // `pair` does through `inits` too.
                                continue;
                            }
                            let (r, d) = self.pair(e, written, is_loop)?;
                            out.append(&mut self.pending);
                            let ty = self.p.vars[*v].ty;
                            if matches!(ty, Ty::Int | Ty::Bool) {
                                // The same for every pixel (a pass's phase,
                                // a count): one variable, no pair.
                                if d.is_some() {
                                    return None;
                                }
                                let u = match self.uniforms[*v] {
                                    Some(u) => u,
                                    None => {
                                        let name = format!("{}_u", self.p.vars[*v].name);
                                        let u = self.new_var(name, ty);
                                        self.uniforms[*v] = Some(u);
                                        u
                                    }
                                };
                                out.push(IStmt::Assign(Place::Var(u), r));
                                if !written.contains(v) {
                                    written.push(*v);
                                }
                                continue;
                            }
                            if !matches!(ty, Ty::Float | Ty::Complex) {
                                return None;
                            }
                            // A carried variable: its deviation, kept to the
                            // next pass, and its current value at the
                            // reference.
                            if let Some((_, current, deviation)) = self.carried[*v] {
                                out.push(IStmt::Assign(Place::Var(deviation), d.map_or_else(|| zero_of(ty), |d| widen(d, ty))));
                                out.push(IStmt::Assign(Place::Var(current), widen(r, ty)));
                                if !written.contains(v) {
                                    written.push(*v);
                                }
                                continue;
                            }
                            // One pair per variable, whichever arm writes
                            // it. The deviation first: it reads the pair's
                            // old values, and a reference value reads no
                            // deviation.
                            let (rv, dv) = match self.pairs[*v] {
                                Some(pair) => pair,
                                None => {
                                    let name = self.p.vars[*v].name.clone();
                                    let pair = (self.new_var(format!("{name}_ref"), ty), self.new_var(format!("{name}_dev"), ty));
                                    self.pairs[*v] = Some(pair);
                                    pair
                                }
                            };
                            out.push(IStmt::Assign(Place::Var(dv), d.map_or_else(|| zero_of(ty), |d| widen(d, ty))));
                            out.push(IStmt::Assign(Place::Var(rv), widen(r, ty)));
                            if !written.contains(v) {
                                written.push(*v);
                            }
                        }
                        _ => return None,
                    }
                }
                IStmt::If { arms, otherwise } if is_loop => {
                    let mut derived = Vec::new();
                    let mut after: Option<Vec<VarId>> = None;
                    let mut any_z = *wrote_z;
                    // An arm's equations, under `taken` (the test that
                    // picks it). One that has none, picked by the
                    // parameters, is left empty and excluded: the
                    // equations hold when it is not taken. When every arm
                    // is, the branch has none.
                    let mark = self.excluded.len();
                    let mut kept = 0usize;
                    let mut arm = |this: &mut Self, b: &[IStmt], taken: IExpr| -> Option<Vec<IStmt>> {
                        let (mut w, mut z) = (written.clone(), *wrote_z);
                        this.path.push(taken.clone());
                        let code = this.stmts(b, is_loop, &mut w, &mut z);
                        this.path.pop();
                        let Some(code) = code else {
                            let whole = this.path.iter().cloned().fold(None, |acc, t| Some(and(acc, t)));
                            let whole = and(whole, taken);
                            if !per_image(&whole) {
                                return None;
                            }
                            this.excluded.push(whole);
                            return Some(Vec::new());
                        };
                        kept += 1;
                        any_z |= z;
                        after = Some(match after.take() {
                            None => w,
                            Some(a) => a.into_iter().filter(|v| w.contains(v)).collect(),
                        });
                        Some(code)
                    };
                    let mut earlier: Option<IExpr> = None;
                    let mut rest: Option<IStmt> = None;
                    for (k, (c, b)) in arms.iter().enumerate() {
                        self.z_written = *wrote_z;
                        let paired = self.pair(c, written, is_loop).filter(|(rc, dc)| dc.is_none() && rc.ty == Ty::Bool);
                        let Some((rc, _)) = paired else {
                            if k == 0 {
                                return None;
                            }
                            // A test that varies from pixel to pixel after
                            // tests that do not: the rest of the chain is
                            // their otherwise, as `else { if .. }` writes
                            // it, to be left out whole (a Rhai `else if`
                            // lowers to one chain).
                            rest = Some(IStmt::If { arms: arms[k..].to_vec(), otherwise: otherwise.clone() });
                            break;
                        };
                        let taken = and(earlier.clone(), rc.clone());
                        derived.push((rc.clone(), arm(self, b, taken)?));
                        earlier = Some(and(earlier, mk(IExprKind::Unary(UnOp::Not, Box::new(rc)), Ty::Bool)));
                    }
                    let taken = earlier.unwrap_or_else(|| mk(IExprKind::Bool(true), Ty::Bool));
                    let otherwise = match &rest {
                        Some(r) => arm(self, std::slice::from_ref(r), taken)?,
                        None => arm(self, otherwise, taken)?,
                    };
                    if kept == 0 {
                        self.excluded.truncate(mark);
                        return None;
                    }
                    *written = after.unwrap_or_default();
                    *wrote_z = any_z;
                    out.push(IStmt::If { arms: derived, otherwise });
                }
                _ => return None,
            }
        }
        Some(out)
    }

    /// An expression's value at the reference and the pixel's deviation.
    fn pair(&mut self, e: &IExpr, written: &[VarId], is_loop: bool) -> Option<Pair> {
        let ty = e.ty;
        Some(match &e.kind {
            IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) | IExprKind::Param(_) => {
                (e.clone(), None)
            }
            IExprKind::Predef(Predef::Z) => {
                let dz = mk(IExprKind::Predef(Predef::DZ), Ty::Complex);
                if self.z_written {
                    (mk(IExprKind::Var(self.z_ref_var()), Ty::Complex), Some(dz))
                } else {
                    (e.clone(), Some(dz))
                }
            }
            IExprKind::Predef(Predef::Pixel) => {
                let pixel = mk(IExprKind::Predef(Predef::Pixel), Ty::Complex);
                let dpixel = mk(IExprKind::Predef(Predef::DPixel), Ty::Complex);
                (bin(BinOp::Sub, pixel, dpixel.clone()), Some(dpixel))
            }
            // The image's and the run's values, the same for every pixel.
            IExprKind::Predef(
                Predef::Pi
                | Predef::E
                | Predef::Center
                | Predef::Magn
                | Predef::MaxIter
                | Predef::JuliaC
                | Predef::IsMand
                | Predef::MagXMag
                | Predef::RotSkew
                // The pass the reference is on, which the pixel's phase
                // matches (step 5b's references, one a phase).
                | Predef::Pass,
            ) => (e.clone(), None),
            IExprKind::Predef(_) => return None,
            IExprKind::Var(v) => {
                if self.p.vars[*v].global {
                    (e.clone(), None)
                } else if let Some((_, current, deviation)) = self.carried[*v].filter(|_| is_loop) {
                    let t = self.p.vars[*v].ty;
                    (mk(IExprKind::Var(current), t), Some(mk(IExprKind::Var(deviation), t)))
                } else if let Some(u) = self.uniforms[*v].filter(|_| written.contains(v)) {
                    (mk(IExprKind::Var(u), self.p.vars[*v].ty), None)
                } else if let Some((rv, dv)) = self.pairs[*v].filter(|_| written.contains(v)) {
                    let t = self.p.vars[*v].ty;
                    (mk(IExprKind::Var(rv), t), Some(mk(IExprKind::Var(dv), t)))
                } else if let Some(init) = self.inits[*v].clone() {
                    // Constant per pixel: its init's value (variable-free,
                    // `init_values`), unless the loop changes it.
                    if is_loop && writes_var(&self.p.loop_body, *v) {
                        return None;
                    }
                    self.pair(&init, &[], false)?
                } else {
                    return None;
                }
            }
            IExprKind::Convert(a) => {
                let (r, d) = self.pair(a, written, is_loop)?;
                // From an int or a bool, or to one: the same for every
                // pixel, the conversion kept (`widen` lifts a real to a
                // complex alone).
                if !matches!(a.ty, Ty::Float | Ty::Complex) || !matches!(ty, Ty::Float | Ty::Complex) {
                    if d.is_some() {
                        return None;
                    }
                    return Some((mk(IExprKind::Convert(Box::new(r)), ty), None));
                }
                (widen(r, ty), d.map(|d| widen(d, ty)))
            }
            IExprKind::MakeComplex(a, b) => {
                let (ra, da) = self.pair(a, written, is_loop)?;
                let (rb, db) = self.pair(b, written, is_loop)?;
                let r = mk(IExprKind::MakeComplex(Box::new(ra), Box::new(rb)), Ty::Complex);
                let d = match (da, db) {
                    (None, None) => None,
                    (da, db) => Some(mk(
                        IExprKind::MakeComplex(
                            Box::new(da.unwrap_or_else(|| zero_of(Ty::Float))),
                            Box::new(db.unwrap_or_else(|| zero_of(Ty::Float))),
                        ),
                        Ty::Complex,
                    )),
                };
                (r, d)
            }
            IExprKind::Unary(UnOp::Neg, a) => {
                let (r, d) = self.pair(a, written, is_loop)?;
                (neg(r), d.map(neg))
            }
            IExprKind::Unary(UnOp::Not, a) => {
                let (r, d) = self.pair(a, written, is_loop)?;
                if d.is_some() {
                    return None;
                }
                (mk(IExprKind::Unary(UnOp::Not, Box::new(r)), ty), None)
            }
            IExprKind::Binary(op, a, b) => {
                if ty == Ty::Bool || ty == Ty::Int {
                    // A test or an int: fine while it is the same for
                    // every pixel.
                    let (ra, da) = self.pair(a, written, is_loop)?;
                    let (rb, db) = self.pair(b, written, is_loop)?;
                    if da.is_some() || db.is_some() {
                        return None;
                    }
                    return Some((mk(IExprKind::Binary(*op, Box::new(ra), Box::new(rb)), ty), None));
                }
                let (ra, da) = self.pair(a, written, is_loop)?;
                let (rb, db) = self.pair(b, written, is_loop)?;
                match op {
                    BinOp::Add | BinOp::Sub => {
                        let r = bin(*op, ra, rb);
                        let d = match (da, db) {
                            (None, None) => None,
                            (Some(x), None) => Some(widen(x, ty)),
                            (None, Some(y)) => Some(if *op == BinOp::Sub { neg(widen(y, ty)) } else { widen(y, ty) }),
                            (Some(x), Some(y)) => Some(bin(*op, widen(x, ty), widen(y, ty))),
                        };
                        (r, d)
                    }
                    BinOp::Mul => {
                        let r = bin(BinOp::Mul, ra.clone(), rb.clone());
                        let d = match (da, db) {
                            (None, None) => None,
                            (Some(x), None) => Some(bin(BinOp::Mul, widen(x, ty), widen(rb, ty))),
                            (None, Some(y)) => Some(bin(BinOp::Mul, widen(ra, ty), widen(y, ty))),
                            (Some(x), Some(y)) => {
                                let xb = bin(BinOp::Mul, widen(x.clone(), ty), widen(rb, ty));
                                let ay = bin(BinOp::Mul, widen(ra, ty), widen(y.clone(), ty));
                                let xy = bin(BinOp::Mul, widen(x, ty), widen(y, ty));
                                Some(bin(BinOp::Add, bin(BinOp::Add, xb, ay), xy))
                            }
                        };
                        (r, d)
                    }
                    BinOp::Div => {
                        let q = bin(BinOp::Div, widen(ra, ty), widen(rb.clone(), ty));
                        let d = match (da, db) {
                            (None, None) => None,
                            (Some(x), None) => Some(bin(BinOp::Div, widen(x, ty), widen(rb, ty))),
                            (dx, Some(y)) => {
                                // (da - q db) / (B + db)
                                let qdb = bin(BinOp::Mul, q.clone(), widen(y.clone(), ty));
                                let top = match dx {
                                    Some(x) => bin(BinOp::Sub, widen(x, ty), qdb),
                                    None => neg(qdb),
                                };
                                Some(bin(BinOp::Div, top, bin(BinOp::Add, widen(rb, ty), widen(y, ty))))
                            }
                        };
                        (q, d)
                    }
                    // A power whose exponent is the same for every pixel:
                    // A^p expm1(p log1p(da / A)).
                    BinOp::Pow if db.is_none() && matches!(ty, Ty::Float | Ty::Complex) => {
                        let r = mk(IExprKind::Binary(BinOp::Pow, Box::new(widen(ra.clone(), ty)), Box::new(widen(rb.clone(), ty))), ty);
                        let Some(x) = da else { return Some((r, None)) };
                        let ratio = bin(BinOp::Div, widen(x.clone(), ty), widen(ra.clone(), ty));
                        let exponent = bin(BinOp::Mul, widen(rb.clone(), ty), call(Builtin::Log1p, ratio, ty));
                        let form = bin(BinOp::Mul, r.clone(), call(Builtin::Expm1, exponent, ty));
                        let end = mk(IExprKind::Binary(BinOp::Pow, Box::new(bin(BinOp::Add, widen(ra.clone(), ty), widen(x.clone(), ty))), Box::new(widen(rb, ty))), ty);
                        let plain = bin(BinOp::Sub, end, r.clone());
                        let d = self.guarded(&ra, &x, plain, form, ty);
                        (r, Some(d))
                    }
                    // A remainder, a power of a varying exponent: no
                    // cancellation-free form.
                    _ => {
                        if da.is_some() || db.is_some() {
                            return None;
                        }
                        (mk(IExprKind::Binary(*op, Box::new(ra), Box::new(rb)), ty), None)
                    }
                }
            }
            IExprKind::PowInt(a, n) => {
                let (ra, da) = self.pair(a, written, is_loop)?;
                let r = mk(IExprKind::PowInt(Box::new(ra.clone()), *n), ty);
                let Some(x) = da else { return Some((r, None)) };
                match *n {
                    0 => (r, None),
                    1 => (r, Some(x)),
                    n if n >= 2 && n <= 12 => {
                        // sum_k C(n,k) A^(n-k) da^k
                        let mut terms: Option<IExpr> = None;
                        for k in 1..=n {
                            let c = konst(binomial(n, k), ty);
                            let ak = if n - k == 0 {
                                None
                            } else {
                                Some(mk(IExprKind::PowInt(Box::new(ra.clone()), n - k), ty))
                            };
                            let xk = if k == 1 { x.clone() } else { mk(IExprKind::PowInt(Box::new(x.clone()), k), ty) };
                            let mut term = bin(BinOp::Mul, c, xk);
                            if let Some(ak) = ak {
                                term = bin(BinOp::Mul, ak, term);
                            }
                            terms = add(terms, Some(term), ty);
                        }
                        (r, terms)
                    }
                    _ => return None,
                }
            }
            IExprKind::Modulus(a) => {
                let (ra, da) = self.pair(a, written, is_loop)?;
                let r = mk(IExprKind::Modulus(Box::new(ra.clone())), Ty::Float);
                let Some(x) = da else { return Some((r, None)) };
                if a.ty != Ty::Complex {
                    // d(a^2) = 2 A da + da^2
                    let two_ax = bin(BinOp::Mul, konst(2.0, Ty::Float), bin(BinOp::Mul, ra, x.clone()));
                    return Some((r, Some(bin(BinOp::Add, two_ax, mk(IExprKind::Modulus(Box::new(x)), Ty::Float)))));
                }
                let re = |v: IExpr| call(Builtin::Real, v, Ty::Float);
                let im = |v: IExpr| call(Builtin::Imag, v, Ty::Float);
                let cross = bin(
                    BinOp::Add,
                    bin(BinOp::Mul, re(ra.clone()), re(x.clone())),
                    bin(BinOp::Mul, im(ra), im(x.clone())),
                );
                let d = bin(BinOp::Add, bin(BinOp::Mul, konst(2.0, Ty::Float), cross), mk(IExprKind::Modulus(Box::new(x)), Ty::Float));
                (r, Some(d))
            }
            IExprKind::Call(f, a) => {
                let (ra, da) = self.pair(a, written, is_loop)?;
                let r = call(*f, ra.clone(), ty);
                let Some(x) = da else { return Some((r, None)) };
                let complex = a.ty == Ty::Complex;
                let d = match f {
                    Builtin::Sqr => {
                        let two_ax = bin(BinOp::Mul, konst(2.0, ty), bin(BinOp::Mul, widen(ra, ty), widen(x.clone(), ty)));
                        bin(BinOp::Add, two_ax, call(Builtin::Sqr, x, ty))
                    }
                    Builtin::Ident => x,
                    Builtin::Zero => return Some((r, None)),
                    Builtin::Conj | Builtin::Flip if complex => call(*f, x, ty),
                    Builtin::Real | Builtin::Imag if complex => call(*f, x, ty),
                    // |A + da| - |A| by sign case, per part for a
                    // complex (UF's abs folds each).
                    Builtin::Abs if complex => {
                        let re = |v: IExpr| call(Builtin::Real, v, Ty::Float);
                        let im = |v: IExpr| call(Builtin::Imag, v, Ty::Float);
                        mk(
                            IExprKind::MakeComplex(
                                Box::new(bin(BinOp::DiffAbs, re(ra.clone()), re(x.clone()))),
                                Box::new(bin(BinOp::DiffAbs, im(ra), im(x))),
                            ),
                            Ty::Complex,
                        )
                    }
                    Builtin::Abs if ty == Ty::Float => bin(BinOp::DiffAbs, ra, x),
                    Builtin::Recip => {
                        // -(da / A) / (A + da)
                        let t = bin(BinOp::Div, widen(x.clone(), ty), widen(ra.clone(), ty));
                        neg(bin(BinOp::Div, t, bin(BinOp::Add, widen(ra, ty), widen(x, ty))))
                    }
                    // sqrt's form divides by sqrt A + sqrt(A + d), zero where
                    // both are.
                    Builtin::Sqrt if ty == a.ty => {
                        let form = difference_form(Builtin::Sqrt, ra.clone(), x.clone(), ty)?;
                        let plain = bin(BinOp::Sub, call(Builtin::Sqrt, bin(BinOp::Add, widen(ra.clone(), ty), widen(x.clone(), ty)), ty), call(Builtin::Sqrt, widen(ra.clone(), ty), ty));
                        self.guarded(&ra, &x, plain, form, ty)
                    }
                    f if ty == a.ty && matches!(ty, Ty::Float | Ty::Complex) => match difference_form(*f, ra, x, ty) {
                        Some(d) => d,
                        None => return None,
                    },
                    // |a| of a complex, a float: (2 Re(conj A da) + |da|^2)
                    // / (|A + da| + |A|).
                    Builtin::Cabs if complex => {
                        let re = |v: IExpr| call(Builtin::Real, v, Ty::Float);
                        let im = |v: IExpr| call(Builtin::Imag, v, Ty::Float);
                        let cross = bin(BinOp::Add, bin(BinOp::Mul, re(ra.clone()), re(x.clone())), bin(BinOp::Mul, im(ra.clone()), im(x.clone())));
                        let top = bin(BinOp::Add, bin(BinOp::Mul, konst(2.0, Ty::Float), cross), mk(IExprKind::Modulus(Box::new(x.clone())), Ty::Float));
                        let full = call(Builtin::Cabs, bin(BinOp::Add, ra.clone(), x.clone()), Ty::Float);
                        let form = bin(BinOp::Div, top, bin(BinOp::Add, full.clone(), call(Builtin::Cabs, ra.clone(), Ty::Float)));
                        // Zero over zero where A and da are: guarded.
                        let plain = bin(BinOp::Sub, full, call(Builtin::Cabs, ra.clone(), Ty::Float));
                        self.guarded(&ra, &x, plain, form, Ty::Float)
                    }
                    _ => return None,
                };
                (r, Some(d))
            }
            IExprKind::UserCall(f, a) => {
                let (r, d) = self.pair(a, written, is_loop)?;
                if d.is_some() {
                    return None;
                }
                (mk(IExprKind::UserCall(*f, Box::new(r)), ty), None)
            }
            _ => return None,
        })
    }
}

/// The deviation of `f(A + d) - f(A)` for a function with a
/// cancellation-free difference form, its argument and value of type
/// `ty`; `None` for one without.
fn difference_form(f: Builtin, a: IExpr, d: IExpr, ty: Ty) -> Option<IExpr> {
    use Builtin::*;
    let (a, d) = (widen(a, ty), widen(d, ty));
    let half = || bin(BinOp::Mul, konst(0.5, ty), d.clone());
    let mid = || bin(BinOp::Add, a.clone(), half());
    let end = || bin(BinOp::Add, a.clone(), d.clone());
    let two = || konst(2.0, ty);
    let c = |f: Builtin, x: IExpr| call(f, x, ty);
    Some(match f {
        // exp A (exp d - 1); expm1's difference is exp's.
        Exp | Expm1 => bin(BinOp::Mul, c(Exp, a.clone()), c(Expm1, d.clone())),
        // log(1 + d / A); log1p's over 1 + A.
        Log => c(Log1p, bin(BinOp::Div, d.clone(), a.clone())),
        Log1p => c(Log1p, bin(BinOp::Div, d.clone(), bin(BinOp::Add, konst(1.0, ty), a.clone()))),
        // d / (sqrt(A + d) + sqrt A)
        Sqrt => bin(BinOp::Div, d.clone(), bin(BinOp::Add, c(Sqrt, end()), c(Sqrt, a.clone()))),
        // Sums to products, the half-difference small.
        Sin => bin(BinOp::Mul, bin(BinOp::Mul, two(), c(Cos, mid())), c(Sin, half())),
        Cos => neg(bin(BinOp::Mul, bin(BinOp::Mul, two(), c(Sin, mid())), c(Sin, half()))),
        Sinh => bin(BinOp::Mul, bin(BinOp::Mul, two(), c(Cosh, mid())), c(Sinh, half())),
        Cosh => bin(BinOp::Mul, bin(BinOp::Mul, two(), c(Sinh, mid())), c(Sinh, half())),
        // tan x - tan y = sin(x - y) / (cos x cos y), and alike.
        Tan => bin(BinOp::Div, c(Sin, d.clone()), bin(BinOp::Mul, c(Cos, a.clone()), c(Cos, end()))),
        Cotan => neg(bin(BinOp::Div, c(Sin, d.clone()), bin(BinOp::Mul, c(Sin, a.clone()), c(Sin, end())))),
        Tanh => bin(BinOp::Div, c(Sinh, d.clone()), bin(BinOp::Mul, c(Cosh, a.clone()), c(Cosh, end()))),
        Cotanh => neg(bin(BinOp::Div, c(Sinh, d.clone()), bin(BinOp::Mul, c(Sinh, a.clone()), c(Sinh, end())))),
        _ => return None,
    })
}

fn stmt_reads_var(s: &IStmt, v: VarId) -> bool {
    fn e(x: &IExpr, v: VarId) -> bool {
        match &x.kind {
            IExprKind::Var(w) => *w == v,
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => e(a, v),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => e(a, v) || e(b, v),
            _ => false,
        }
    }
    match s {
        IStmt::Assign(_, x) => e(x, v),
        IStmt::If { arms, otherwise } => {
            arms.iter().any(|(c, b)| e(c, v) || b.iter().any(|s| stmt_reads_var(s, v))) || otherwise.iter().any(|s| stmt_reads_var(s, v))
        }
        IStmt::While { cond, body } | IStmt::Repeat { body, cond } => e(cond, v) || body.iter().any(|s| stmt_reads_var(s, v)),
    }
}

/// `stmts` without their assignments to `v`.
fn strip_assigns(stmts: &mut Vec<IStmt>, v: VarId) {
    stmts.retain(|s| !matches!(s, IStmt::Assign(Place::Var(w), _) if *w == v));
    for s in stmts {
        match s {
            IStmt::If { arms, otherwise } => {
                for (_, b) in arms {
                    strip_assigns(b, v);
                }
                strip_assigns(otherwise, v);
            }
            IStmt::While { body, .. } | IStmt::Repeat { body, .. } => strip_assigns(body, v),
            IStmt::Assign(..) => {}
        }
    }
}

fn reads_z(e: &IExpr) -> bool {
    match &e.kind {
        IExprKind::Predef(Predef::Z) => true,
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => reads_z(a),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => reads_z(a) || reads_z(b),
        _ => false,
    }
}

pub(crate) fn writes_var(stmts: &[IStmt], v: VarId) -> bool {
    stmts.iter().any(|s| match s {
        IStmt::Assign(Place::Var(w), _) => *w == v,
        IStmt::Assign(..) => false,
        IStmt::If { arms, otherwise } => arms.iter().any(|(_, b)| writes_var(b, v)) || writes_var(otherwise, v),
        IStmt::While { body, .. } | IStmt::Repeat { body, .. } => writes_var(body, v),
    })
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::check::check;
    use super::super::interp::{run, run_perturbed, Env, ParamValues};
    use super::super::parser::parse_file;
    use super::super::{Dialect, EntryKind};
    use super::*;

    fn program(src: &str, dialect: Dialect) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Formula, dialect);
        assert!(!super::super::has_errors(&d), "{d:?}");
        p.unwrap()
    }

    /// Derived equations iterate a pixel near the reference as the
    /// formula does: the f64 model of the perturbed path against the
    /// direct run, for the shapes the rules cover.
    #[test]
    fn derived_equations_run_as_the_formula_does() {
        let cases: &[(&str, Dialect, (f64, f64))] = &[
            ("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}", Dialect::UltraFractal, (-0.7436, 0.1318)),
            ("C {\ninit:\n z = 0\n c = #pixel\nloop:\n z = z^3 + c\nbailout:\n |z| < 4\n}", Dialect::UltraFractal, (-0.55, 0.62)),
            ("J {\ninit:\n z = #pixel\nloop:\n z = z * z + (-0.8, 0.156)\nbailout:\n |z| < 4\n}", Dialect::UltraFractal, (0.02, -0.03)),
            ("T {\ninit:\n z = 0\nloop:\n z = sqr(conj(z)) + #pixel\nbailout:\n |z| < 4\n}", Dialect::UltraFractal, (-0.3, 0.6)),
            ("L {\ninit:\n z = 0.5\nloop:\n z = #pixel * z * (1 - z)\nbailout:\n |z| < 1e4\n}", Dialect::UltraFractal, (3.2, 0.4)),
            (
                "G {\ninit:\n z = 0\nloop:\n t = z * z + #pixel\n z = t / (1 + 0.1 * t) + 0.01 * recip(t + 3)\nbailout:\n |z| < 1e4\n}",
                Dialect::UltraFractal,
                (-0.6, 0.3),
            ),
            ("F {\n z = 0, c = pixel:\n z = sqr(z) + c, |z| <= 4\n}", Dialect::Fractint, (-0.75, 0.1)),
            // Branches on a parameter: the same arm at the reference and
            // the pixel. `t` written before and changed in one arm; z
            // written in every arm, by different maps.
            (
                "K {\ninit:\n z = 0\nloop:\n t = sqr(z)\n if @flip\n  t = conj(t)\n endif\n if @mode == 0\n  z = t + #pixel\n elseif @mode == 1\n  z = t * z + #pixel\n else\n  z = t - #pixel\n endif\nbailout:\n |z| < 4\ndefault:\n bool param flip\n  default = true\n endparam\n int param mode\n  default = 1\n endparam\n}",
                Dialect::UltraFractal,
                (-0.3, 0.55),
            ),
            // A branch on a constant of the init, z written in one arm.
            (
                "Q {\ninit:\n z = 0\n k = @power\nloop:\n if k > 2\n  z = z^3 + #pixel\n endif\n if k <= 2\n  z = sqr(z) + #pixel\n endif\nbailout:\n |z| < 4\ndefault:\n float param power\n  default = 2\n endparam\n}",
                Dialect::UltraFractal,
                (-0.7436, 0.1318),
            ),
            // z written in steps, read after each.
            ("S {\ninit:\n z = 0\nloop:\n z = sqr(z)\n z = z + #pixel\n z = z * (1, 0.01)\nbailout:\n |z| < 4\n}", Dialect::UltraFractal, (-0.74, 0.13)),
            // A branch in the init on the plane (the Julia toggle), and
            // one on a parameter: a copy of the loop's equations each.
            (
                "P {\ninit:\n if ismand\n  z = 0\n  c = #pixel\n else\n  z = #pixel\n  c = (-0.8, 0.156)\n endif\nloop:\n z = sqr(z) + c\nbailout:\n |z| < 4\n}",
                Dialect::Fractint,
                (-0.7436, 0.1318),
            ),
            (
                "Q2 {\ninit:\n c = #pixel\n if @cubic\n  z = c * 0.1\n  k = 3\n else\n  z = 0\n  k = 2\n endif\nloop:\n if k == 3\n  z = z^3 + c\n else\n  z = sqr(z) + c\n endif\nbailout:\n |z| < 4\ndefault:\n bool param cubic\n  default = true\n endparam\n}",
                Dialect::UltraFractal,
                (-0.55, 0.62),
            ),
            // abs by diffabs: the Burning Ship (UF's abs folds each
            // part), and a real one.
            ("A {\ninit:\n z = 0\nloop:\n z = sqr(abs(z)) + #pixel\nbailout:\n |z| < 4\n}", Dialect::UltraFractal, (-1.8, 0.0)),
            ("A2 {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel + 0.3 * abs(real(z) - 0.1)\nbailout:\n |z| < 4\n}", Dialect::UltraFractal, (-0.6, 0.45)),
        ];
        for &(src, dialect, (rx, ry)) in cases {
            let p = program(src, dialect);
            let d = with_derived_equations(&p).unwrap_or_else(|| panic!("{}: no equations derived", p.name));
            assert!(!d.perturb_loop.is_empty());
            let params = ParamValues::defaults(&d);
            let reference = C64::new(rx, ry);
            let mut compared = 0;
            for k in 0..10 {
                let t = k as f64 * 0.628;
                let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-5 * (1.0 + k as f64)));
                let env = Env::at(px, 300);
                let direct = run(&p, &ParamValues::defaults(&p), None, &env);
                let pert = run_perturbed(&d, &params, &env, reference, true).unwrap();
                assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "{} at {px:?}", p.name);
                if direct.escaped {
                    let err = pert.z.sub(direct.z).abs() / direct.z.abs();
                    assert!(err < 1e-5, "{} at {px:?}: {:?} vs {:?}", p.name, pert.z, direct.z);
                }
                compared += 1;
            }
            assert_eq!(compared, 10);
        }
    }

    /// A plane-aware formula's equations follow the Julia toggle: on the
    /// Julia plane the pixel is the start, c the constant.
    #[test]
    fn a_branch_on_the_plane_derives_for_both() {
        let p = program(
            "P {\ninit:\n if ismand\n  z = 0\n  c = #pixel\n else\n  z = #pixel\n  c = (-0.8, 0.156)\n endif\nloop:\n z = sqr(z) + c\nbailout:\n |z| < 4\n}",
            Dialect::Fractint,
        );
        let d = with_derived_equations(&p).expect("equations");
        assert!(matches!(d.perturb_loop.as_slice(), [IStmt::If { .. }]), "a copy a plane");
        let params = ParamValues::defaults(&d);
        for (julia, (rx, ry)) in [(false, (-0.7436, 0.1318)), (true, (0.12, -0.31))] {
            let reference = C64::new(rx, ry);
            for k in 0..10 {
                let t = k as f64 * 0.628;
                let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-5 * (1.0 + k as f64)));
                let mut env = Env::at(px, 300);
                env.julia = julia;
                env.julia_c = C64::new(-0.8, 0.156);
                let direct = run(&p, &ParamValues::defaults(&p), None, &env);
                let pert = run_perturbed(&d, &params, &env, reference, true).unwrap();
                assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "julia {julia} at {px:?}");
            }
        }
        // A branch in the init on the pixel stays underived.
        let on_pixel = program("B {\ninit:\n if real(#pixel) > 0\n  z = 0\n else\n  z = 1\n endif\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}", Dialect::UltraFractal);
        assert!(with_derived_equations(&on_pixel).is_none());
    }

    /// Every hybrid preset derives equations, and with a reference per
    /// phase (the f64 model of step 5b) a pixel iterates as the hybrid
    /// does: through the prefix, across rebases at every phase.
    #[test]
    fn hybrids_perturb_with_a_reference_per_phase() {
        let mut rebased_mid_cycle = 0;
        for (name, h) in super::super::hybrid::presets() {
            let p = h.program().unwrap();
            let d = with_derived_equations(&p).unwrap_or_else(|| panic!("{name}: no equations derived"));
            let how = super::super::perturb::perturbation(&d).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(how.cycle.is_some(), h.groups.len() > 1, "{name}");
            assert!(how.rebase, "{name}: the phase is no carried state");
            let params = ParamValues::defaults(&d);
            for (rx, ry) in [(-0.7436, 0.1318), (-1.76, -0.028), (-0.3, 0.6), (0.27, 0.005)] {
                let reference = C64::new(rx, ry);
                for k in 0..8 {
                    let t = k as f64 * 0.785;
                    let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-6 * (1.0 + k as f64)));
                    let env = Env::at(px, 200);
                    let direct = run(&p, &ParamValues::defaults(&p), None, &env);
                    let pert = run_perturbed(&d, &params, &env, reference, true).unwrap();
                    assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "{name} at {px:?}");
                    if direct.escaped {
                        let err = pert.z.sub(direct.z).abs() / direct.z.abs();
                        assert!(err < 1e-4, "{name} at {px:?}: {:?} vs {:?}", pert.z, direct.z);
                        if h.groups.len() > 1 && direct.n > 30 {
                            rebased_mid_cycle += 1;
                        }
                    }
                }
            }
        }
        assert!(rebased_mid_cycle > 5, "the views test little ({rebased_mid_cycle})");
    }

    /// An arm a parameter picks that has no equations is left out, and the
    /// derived `perturb` setting says when the rest hold: a Tricorn whose
    /// power 2 squares and any other takes an asin deep-zooms at 2.
    #[test]
    fn an_arm_without_equations_is_excluded_by_the_parameters() {
        let p = program(
            "T {\ninit:\n z = 0\nloop:\n if @power == 2\n  z = sqr(conj(z)) + #pixel\n else\n  z = asin(conj(z)^@power) + #pixel\n endif\nbailout:\n |z| < 4\n\
             default:\n float param power\n  default = 2\n endparam\n}",
            Dialect::UltraFractal,
        );
        let d = with_derived_equations(&p).expect("equations for power 2");
        let env = Env::at(C64::new(-0.3, 0.6), 100);
        let mut params = ParamValues::defaults(&d);
        assert!(super::super::interp::perturb_holds(&d, &params, &env), "power 2");
        let reference = C64::new(-0.3, 0.6);
        for k in 0..8 {
            let t = k as f64 * 0.785;
            let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-5 * (1.0 + k as f64)));
            let env = Env::at(px, 300);
            let direct = run(&p, &ParamValues::defaults(&p), None, &env);
            let pert = run_perturbed(&d, &params, &env, reference, true).unwrap();
            assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "at {px:?}");
        }
        params.values[0].0 = 2.5;
        assert!(!super::super::interp::perturb_holds(&d, &params, &env), "power 2.5 has none");
        // A branch on the iterate excludes nothing: no equations.
        let b = program(
            "B {\ninit:\n z = 0\nloop:\n if @k == 1\n  if real(z) > 0\n   z = exp(z)\n  endif\n endif\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n\
             default:\n int param k\n  default = 0\n endparam\n}",
            Dialect::UltraFractal,
        );
        assert!(with_derived_equations(&b).is_some(), "the @k == 1 arm is excluded");
        // A test on the iterate after a parameter's, in one chain (as a
        // Rhai `else if` lowers): the rest of the chain is left out whole.
        let c = program(
            "C {\ninit:\n z = 0\nloop:\n if @k == 0\n  z = sqr(z) + #pixel\n elseif real(z) > 0\n  z = exp(z)\n else\n  z = sqr(z) - #pixel\n endif\n\
             bailout:\n |z| < 4\ndefault:\n int param k\n  default = 0\n endparam\n}",
            Dialect::UltraFractal,
        );
        let d = with_derived_equations(&c).expect("the @k == 0 arm derives");
        let mut params = ParamValues::defaults(&d);
        assert!(super::super::interp::perturb_holds(&d, &params, &env), "k = 0");
        params.values[0].0 = 1.0;
        assert!(!super::super::interp::perturb_holds(&d, &params, &env), "k = 1 has none");
    }

    /// State the loop carries besides z (phase 4, step 5): the reference
    /// records it each pass, the pixel keeps its deviation, a rebase
    /// moves it as it moves `#dz`. A Phoenix (its previous z), a carried
    /// sum written in one arm, and a convergent Newton whose test reads
    /// its previous z run perturbed as they run, in the f64 model.
    #[test]
    fn carried_state_runs_as_the_formula_does() {
        let cases: &[(&str, (f64, f64), u32)] = &[
            ("H {\ninit:\n z = 0\n zold = 0\nloop:\n t = z\n z = sqr(z) + #pixel + zold * (-0.5, 0)\n zold = t\nbailout:\n |z| < 4\n}", (0.56667, -0.1), 300),
            (
                "A {\ninit:\n z = 0\n float acc = 0\nloop:\n if @on\n  acc = acc * 0.5 + real(z)\n endif\n z = sqr(z) + #pixel + acc * 0.01\nbailout:\n |z| < 4\n\
                 default:\n bool param on\n  default = true\n endparam\n}",
                (-0.7436, 0.1318),
                300,
            ),
            (
                "N {\ninit:\n z = #pixel\n zold = 0\nloop:\n zold = z\n z = z - (z^3 - 1) / (3 * z^2)\nbailout:\n |z - zold| > 1e-8\n}",
                (0.21, 0.83),
                100,
            ),
            // `t` written in one arm only, read after: in the other it
            // holds last iteration's value, carried.
            (
                "V {\ninit:\n z = 0\nloop:\n if @m == 1\n  t = z * 0.5\n endif\n z = sqr(z) + t + #pixel\nbailout:\n |z| < 4\ndefault:\n int param m\n  default = 1\n endparam\n}",
                (0.3, 0.55),
                300,
            ),
        ];
        for &(src, (rx, ry), maxiter) in cases {
            let p = program(src, Dialect::UltraFractal);
            let d = with_derived_equations(&p).unwrap_or_else(|| panic!("{}: no equations", p.name));
            assert!(!d.carried.is_empty(), "{}: carries", p.name);
            assert!(super::super::perturb::perturb_test(&d).is_some(), "{}: its test", p.name);
            let params = ParamValues::defaults(&d);
            let reference = C64::new(rx, ry);
            let mut escaped = 0;
            for k in 0..12 {
                let t = k as f64 * 0.524;
                let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-6 * (1.0 + k as f64)));
                let env = Env::at(px, maxiter);
                let direct = run(&p, &ParamValues::defaults(&p), None, &env);
                for rebase in [false, true] {
                    let pert = run_perturbed(&d, &params, &env, reference, rebase).unwrap();
                    assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "{} at {px:?}, rebase {rebase}", p.name);
                }
                escaped += direct.escaped as usize;
            }
            assert!(escaped > 0, "{}: nothing escapes at that reference", p.name);
        }
    }

    /// The transcendental functions and real powers by their difference
    /// forms (phase 4, step 6): perturbed as they run, in the f64 model.
    #[test]
    fn transcendental_functions_derive_by_difference_forms() {
        let cases: &[(&str, (f64, f64), u32)] = &[
            // A real power, the Multibrot at 2.5.
            ("P {\ninit:\n z = 0\nloop:\n z = z^2.5 + #pixel\nbailout:\n |z| < 4\n}", (-0.9, 0.3), 300),
            // A power given as a parameter.
            ("Q {\ninit:\n z = 0\nloop:\n z = z^@p + #pixel\nbailout:\n |z| < 4\ndefault:\n float param p\n  default = 3.3\n endparam\n}", (-0.6, 0.6), 300),
            // exp: the exponential map, its test on the real part.
            ("E {\ninit:\n z = 0\nloop:\n z = exp(z) + #pixel\nbailout:\n real(z) < 40\n}", (0.29, 0.05), 200),
            // sin and cos: the classic sine fractal.
            ("S {\ninit:\n z = #pixel\nloop:\n z = sin(z) * #pixel\nbailout:\n abs(imag(z)) < 40\n}", (1.0, 0.5), 200),
            ("C {\ninit:\n z = #pixel\nloop:\n z = cos(z) * #pixel + 0.1 * cosh(z * 0.2)\nbailout:\n abs(imag(z)) < 40\n}", (1.3, 0.2), 200),
            // sqrt, log, tan, cabs, expm1 and log1p in one map.
            (
                "M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel + 0.01 * (sqrt(z + 2) + log(z + 3) + tan(z * 0.1) + cabs(z) + expm1(z * 0.1) + log1p(z * 0.1))\n\
                 bailout:\n |z| < 4\n}",
                (0.3, 0.5),
                300,
            ),
        ];
        for &(src, (rx, ry), maxiter) in cases {
            let p = program(src, Dialect::UltraFractal);
            let d = with_derived_equations(&p).unwrap_or_else(|| panic!("{}: no equations", p.name));
            assert!(super::super::perturb::perturbation(&d).is_ok(), "{}", p.name);
            let params = ParamValues::defaults(&d);
            let reference = C64::new(rx, ry);
            let mut escaped = 0;
            for k in 0..12 {
                let t = k as f64 * 0.524;
                let px = reference.add(C64::new(t.cos(), t.sin()).scale(1e-7 * (1.0 + k as f64)));
                let env = Env::at(px, maxiter);
                let direct = run(&p, &ParamValues::defaults(&p), None, &env);
                let pert = run_perturbed(&d, &params, &env, reference, true).unwrap();
                assert_eq!((pert.n, pert.escaped), (direct.n, direct.escaped), "{} at {px:?}", p.name);
                escaped += direct.escaped as usize;
            }
            assert!(escaped > 0, "{}: nothing escapes there", p.name);
        }
    }

    /// What has no cancellation-free form, or carries state, derives
    /// nothing; a formula with its own equations keeps them.
    #[test]
    fn what_derives_nothing() {
        for src in [
            // A power of a varying exponent, an inverse function.
            "Z {\ninit:\n z = 0\nloop:\n z = z^z + #pixel\nbailout:\n |z| < 4\n}",
            "A {\ninit:\n z = 0\nloop:\n z = asin(z) + #pixel\nbailout:\n |z| < 4\n}",
            "B {\ninit:\n z = 0\nloop:\n if real(z) > 0\n  z = sqr(z) + #pixel\n else\n  z = z + #pixel\n endif\nbailout:\n |z| < 4\n}",
            "U {\ninit:\n z = 0\nloop:\n z = @f(z) + #pixel\nbailout:\n |z| < 4\n}",
            // A carried int (a counter): no deviation to follow it by.
            "I {\ninit:\n z = 0\n int k = 0\nloop:\n k = k + 1\n z = sqr(z) + #pixel * k\nbailout:\n |z| < 4\n}",
        ] {
            assert!(with_derived_equations(&program(src, Dialect::UltraFractal)).is_none(), "{src}");
        }
        let own = program(
            "O {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\nperturbloop:\n #dz = 2 * #z * #dz + sqr(#dz) + #dpixel\n}",
            Dialect::UltraFractal,
        );
        assert!(with_derived_equations(&own).is_none());
    }
}
