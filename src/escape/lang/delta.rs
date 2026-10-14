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
//!
//! The result is ordinary perturbation sections over new variables of
//! the equations (a pair per loop temporary, and one for z once a pass
//! may have written it), which the scaled and floatexp code generators
//! compile as they do a formula's own. A branch in the loop derives
//! when its conditions are the same for every pixel -- a parameter's
//! choice, a constant -- so the reference and the pixel take the same
//! arm. What has no such form -- a transcendental function, a real or
//! complex power, a user function of
//! the iterate, a branch on the iterate, a loop inside the formula's
//! loop, a branch in the init, or state carried from one iteration to
//! the next besides z -- leaves the formula without equations, on the
//! direct path as before.

use super::ast::{BinOp, UnOp};
use super::builtins::Builtin;
use super::ir::*;

/// The formula with perturbation equations derived from its loop, or
/// `None` when it has its own or they cannot be derived.
pub fn with_derived_equations(p: &Program) -> Option<Program> {
    if !p.perturb_loop.is_empty() || !p.perturb_init.is_empty() {
        return None;
    }
    let mut d = Deriver { p, out: p.clone(), pairs: vec![None; p.vars.len()], inits: Vec::new(), z_ref: None, z_written: false };
    // The init's variables other than z: constant per pixel when their
    // value reads neither z nor anything the loop writes; the loop's
    // reads of them are their init's values, re-derived in each pass.
    d.inits = init_values(p)?;
    let loop_body = d.block(&p.loop_body, true)?;
    let init_body = d.block(&p.init, false)?;
    let mut out = d.out;
    out.perturb_init = init_body;
    out.perturb_loop = loop_body;
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
}

/// Each variable's value after the init, as an expression of the
/// pixel, the parameters and the constants: the init's straight-line
/// assignments substituted in order, so `c = #pixel` then `c = c * 2`
/// gives `#pixel * 2`. A variable read before it is assigned holds
/// zero (UF's); one whose value reads z, or grows past a bound, has
/// none. `None` when the init has a branch or a loop.
pub(crate) fn init_values(p: &Program) -> Option<Vec<Option<IExpr>>> {
    // Past this many nodes an expression has stopped being a formula's
    // constant and started being a substitution's blowup.
    const MAX_NODES: usize = 2000;
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
                    match place {
                        Place::Z => {
                            if !is_loop && *wrote_z {
                                return None;
                            }
                            let (r, dz) = self.pair(e, written, is_loop)?;
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
                            let ty = self.p.vars[*v].ty;
                            if !matches!(ty, Ty::Float | Ty::Complex) {
                                return None;
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
                    let mut arm = |this: &mut Self, b: &[IStmt]| -> Option<Vec<IStmt>> {
                        let (mut w, mut z) = (written.clone(), *wrote_z);
                        let code = this.stmts(b, is_loop, &mut w, &mut z)?;
                        any_z |= z;
                        after = Some(match after.take() {
                            None => w,
                            Some(a) => a.into_iter().filter(|v| w.contains(v)).collect(),
                        });
                        Some(code)
                    };
                    for (c, b) in arms {
                        self.z_written = *wrote_z;
                        let (rc, dc) = self.pair(c, written, is_loop)?;
                        if dc.is_some() || rc.ty != Ty::Bool {
                            return None;
                        }
                        derived.push((rc, arm(self, b)?));
                    }
                    let otherwise = arm(self, otherwise)?;
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
                | Predef::RotSkew,
            ) => (e.clone(), None),
            IExprKind::Predef(_) => return None,
            IExprKind::Var(v) => {
                if self.p.vars[*v].global {
                    (e.clone(), None)
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
                if !matches!(a.ty, Ty::Int | Ty::Float | Ty::Complex) || !matches!(ty, Ty::Float | Ty::Complex) {
                    let (r, d) = self.pair(a, written, is_loop)?;
                    if d.is_some() {
                        return None;
                    }
                    return Some((mk(IExprKind::Convert(Box::new(r)), ty), None));
                }
                let (r, d) = self.pair(a, written, is_loop)?;
                if a.ty == Ty::Int && d.is_some() {
                    return None;
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
                    // A power, a remainder: no cancellation-free form.
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

    /// What has no cancellation-free form, or carries state, derives
    /// nothing; a formula with its own equations keeps them.
    #[test]
    fn what_derives_nothing() {
        for src in [
            "E {\ninit:\n z = 0\nloop:\n z = exp(z) + #pixel\nbailout:\n |z| < 4\n}",
            "P {\ninit:\n z = 0\nloop:\n z = z^2.5 + #pixel\nbailout:\n |z| < 4\n}",
            "B {\ninit:\n z = 0\nloop:\n if real(z) > 0\n  z = sqr(z) + #pixel\n else\n  z = z + #pixel\n endif\nbailout:\n |z| < 4\n}",
            "H {\ninit:\n z = 0\n zold = 0\nloop:\n t = z\n z = sqr(z) + #pixel + zold\n zold = t\nbailout:\n |z| < 4\n}",
            "U {\ninit:\n z = 0\nloop:\n z = @f(z) + #pixel\nbailout:\n |z| < 4\n}",
            // `t` written in one arm only, read after: in the other it
            // holds last iteration's value.
            "V {\ninit:\n z = 0\nloop:\n if @m == 1\n  t = z\n endif\n z = sqr(z) + t + #pixel\nbailout:\n |z| < 4\ndefault:\n int param m\n  default = 1\n endparam\n}",
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
