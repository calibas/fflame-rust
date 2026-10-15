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
//! formula whose iterate depends on its pixel through something not
//! complex-differentiable (`abs`, `conj`, `real`, `|x|`, a real variable
//! that follows the pixel) has no derivative, as the built-ins without
//! one: distance estimation then reports the gap. A user function is
//! chosen at run time, so its derivative is too, and a choice with none
//! (`abs`, `conj` ...) differentiates as zero.

use super::ast::{BinOp, UnOp};
use super::builtins::Builtin;
use super::ir::*;

/// The program extended with its derivative, or `None` when it has
/// none.
pub fn with_derivative(p: &Program) -> Option<Program> {
    let taint = tainted(p);
    let need = needed(p);
    let mut out = p.clone();
    // The derivative variables: one per complex variable the iterate
    // depends on, then the iterate's.
    let mut dvar: Vec<Option<VarId>> = vec![None; p.vars.len()];
    for (i, v) in p.vars.iter().enumerate() {
        // A global is one value per image: constant in the pixel.
        if v.ty == Ty::Complex && need[i] && !v.global {
            dvar[i] = Some(out.vars.len());
            out.vars.push(Var {
                name: format!("d_{}", v.name),
                ty: Ty::Complex,
                span: v.span,
                stored_real: false,
                global: false,
                perturb: false,
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
    });
    let d = D { taint: &taint, dvar: &dvar, dz };
    out.init = d.stmts(&p.init)?;
    out.loop_body = d.stmts(&p.loop_body)?;
    out.dz_var = Some(dz);
    Some(out)
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
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => mark(a, need, changed),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => {
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
                IStmt::Assign(..) => {}
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
        IExprKind::Predef(Predef::Pixel | Predef::Z) => true,
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => follows(a, t),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => follows(a, t) || follows(b, t),
        _ => false,
    }
}

struct D<'a> {
    taint: &'a [bool],
    dvar: &'a [Option<VarId>],
    dz: VarId,
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
            IExprKind::Complex(..) | IExprKind::Param(_) => None,
            IExprKind::Predef(Predef::Pixel) => Some(konst(1.0, 0.0)),
            IExprKind::Predef(Predef::Z) => Some(cx(IExprKind::Var(self.dz))),
            IExprKind::Predef(_) => None,
            IExprKind::Var(v) => self.dvar[*v].map(|dv| cx(IExprKind::Var(dv))),
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
            // The user's choice decides; its derivative is chosen with it
            // (zero for a function that has none).
            IExprKind::UserCall(id, a) => self.d(a)?.map(|da| times(cx(IExprKind::UserDeriv(*id, Box::new(as_cx(a)))), da)),
            IExprKind::UserDeriv(..) | IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) => None,
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

    /// The folds have no complex derivative: a formula iterating through
    /// one has none, as the built-in Burning Ship.
    #[test]
    fn folds_in_the_iterate_have_no_derivative() {
        for src in [
            "B {\ninit:\n z = 0\nloop:\n z = sqr(abs(z)) + #pixel\nbailout:\n true\n}",
            "R {\ninit:\n z = 0\nloop:\n z = sqr(z) + real(#pixel)\nbailout:\n true\n}",
            "T {\ninit:\n z = 0\nloop:\n z = sqr(conj(z)) + #pixel\nbailout:\n true\n}",
            // A real variable that follows the pixel.
            "V {\ninit:\n z = 0\nloop:\n float r = |z|\n z = sqr(z) + #pixel * r\nbailout:\n true\n}",
        ] {
            assert!(with_derivative(&program(src, Dialect::UltraFractal)).is_none(), "{src}");
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
}
