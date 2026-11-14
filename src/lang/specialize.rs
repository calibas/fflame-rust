//! Specializing a program on its structural parameters: the ones that pick
//! a case -- a branch whose test is the same at every pixel (Newton's
//! function and scheme, a variant) -- or set a power's exponent (the
//! Multibrot's power, McMullen's `n` and `m`). With their values written
//! into the program as literals, the GPU's compiler folds the branches and
//! unrolls the powers, as a built-in's native tier is compiled for one
//! case: Newton's twin, kept general, took 12 ms at a deep view where the
//! built-in took 5, and written for `z^3 - 1` alone, 5 (2026-11-13).
//!
//! The cost is a compile per value: such a parameter is set once, not
//! animated. `escape::custom::resolve` compiles the specialized program
//! under its own name, beside the damped and biomorph variants.

use super::ast::BinOp;
use super::delta::per_image;
use super::interp::ParamValues;
use super::ir::*;

/// The parameters that pick a case or set an exponent, by index: read in a
/// test the same at every pixel (`delta::per_image`), in a power's
/// exponent the same at every pixel, or through a variable every
/// assignment of which is such an expression (`let k = p.power; z ** k`).
pub fn structural_params(p: &Program) -> Vec<ParamId> {
    fn stmts<'a>(st: &'a [IStmt], f: &mut dyn FnMut(&'a IStmt)) {
        for s in st {
            f(s);
            match s {
                IStmt::Scope(b) | IStmt::While { body: b, .. } | IStmt::Repeat { body: b, .. } => stmts(b, f),
                IStmt::If { arms, otherwise } => {
                    for (_, b) in arms {
                        stmts(b, f);
                    }
                    stmts(otherwise, f);
                }
                _ => {}
            }
        }
    }
    fn exprs<'a>(e: &'a IExpr, f: &mut dyn FnMut(&'a IExpr)) {
        f(e);
        match &e.kind {
            IExprKind::Elem(_, a) | IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => exprs(a, f),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => {
                exprs(a, f);
                exprs(b, f);
            }
            IExprKind::Host(_, args) => args.iter().for_each(|a| exprs(a, f)),
            _ => {}
        }
    }
    fn stmt_exprs<'a>(s: &'a IStmt, f: &mut dyn FnMut(&'a IExpr)) {
        match s {
            IStmt::Assign(_, e) => exprs(e, f),
            IStmt::SetElem { index, value, .. } => {
                exprs(index, f);
                exprs(value, f);
            }
            IStmt::If { arms, .. } => arms.iter().for_each(|(c, _)| exprs(c, f)),
            IStmt::While { cond, .. } | IStmt::Repeat { cond, .. } => exprs(cond, f),
            IStmt::Map { input, .. } => input.iter().for_each(|e| exprs(e, f)),
            IStmt::Host(_, args) => args.iter().for_each(|e| exprs(e, f)),
            IStmt::Scope(_) | IStmt::Break | IStmt::Continue | IStmt::Return => {}
        }
    }
    // A colouring's `final` and `color` (`transform`) as well.
    let sections: [&[IStmt]; 5] = [&p.global_body, &p.init, &p.loop_body, &p.final_body, &p.transform];
    // The parameters an expression reads when it reads nothing else that
    // varies -- constants, the image's values, and variables every
    // assignment of which is such an expression -- or `None`.
    fn uniform(e: &IExpr, alias: &[Option<Vec<ParamId>>], out: &mut Vec<ParamId>) -> bool {
        match &e.kind {
            IExprKind::Param(i) => {
                out.push(*i);
                true
            }
            IExprKind::Var(v) => match &alias[*v] {
                Some(ps) => {
                    out.extend(ps.iter().copied());
                    true
                }
                None => false,
            },
            IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) => true,
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) => uniform(a, alias, out),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => uniform(a, alias, out) && uniform(b, alias, out),
            _ => per_image(e),
        }
    }
    // Optimistic, then each variable some assignment of which is not
    // uniform is struck off, until none changes.
    let mut alias: Vec<Option<Vec<ParamId>>> = vec![Some(Vec::new()); p.vars.len()];
    loop {
        let mut next: Vec<Option<Vec<ParamId>>> = vec![Some(Vec::new()); p.vars.len()];
        for sec in sections {
            stmts(sec, &mut |s| {
                if let IStmt::Assign(Place::Var(v), e) = s {
                    let mut ps = Vec::new();
                    let ok = uniform(e, &alias, &mut ps);
                    match (&mut next[*v], ok) {
                        (Some(acc), true) => acc.extend(ps),
                        (slot, _) => *slot = None,
                    }
                }
            });
        }
        let settled = next.iter().zip(&alias).all(|(a, b)| a.is_some() == b.is_some());
        alias = next;
        if settled {
            break;
        }
    }
    let mut out = Vec::new();
    let mark = |e: &IExpr, out: &mut Vec<ParamId>| {
        let mut ps = Vec::new();
        if uniform(e, &alias, &mut ps) {
            out.extend(ps);
        }
    };
    for sec in sections {
        stmts(sec, &mut |s| {
            if let IStmt::If { arms, .. } = s {
                for (c, _) in arms {
                    mark(c, &mut out);
                }
            }
            stmt_exprs(s, &mut |e| {
                if let IExprKind::Binary(BinOp::Pow, _, x) = &e.kind {
                    mark(x, &mut out);
                }
            });
        });
    }
    if let Some(t) = &p.bailout {
        exprs(t, &mut |e| {
            if let IExprKind::Binary(BinOp::Pow, _, x) = &e.kind {
                mark(x, &mut out);
            }
        });
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The structural parameters (`structural_params`) worth specializing at
/// these values: a choice, an int or a bool at any value; a float or
/// complex only at a whole real value up to 64 in size, so a slider moved
/// through the values between keeps the general program and no compile
/// follows each step.
pub fn bake_values(p: &Program, structural: &[ParamId], values: &ParamValues) -> Vec<(ParamId, (f64, f64))> {
    structural
        .iter()
        .filter_map(|&i| {
            let v = *values.values.get(i)?;
            let whole = v.1 == 0.0 && v.0 == v.0.round() && v.0.abs() <= 64.0;
            match p.params.get(i)?.kind {
                ParamKind::Bool | ParamKind::Int | ParamKind::Enum => Some((i, v)),
                ParamKind::Float | ParamKind::Complex if whole => Some((i, v)),
                _ => None,
            }
        })
        .collect()
}

/// Write `values` into the program as literals wherever it reads those
/// parameters, as the GPU would read them (an int rounded, a bool by
/// whether it is zero). The parameters stay declared, so the block the
/// GPU is handed keeps its layout.
pub fn bake(p: &mut Program, values: &[(ParamId, (f64, f64))]) {
    fn expr(e: &mut IExpr, p: &[Param], values: &[(ParamId, (f64, f64))]) {
        if let IExprKind::Param(i) = e.kind {
            if let Some(&(_, (re, im))) = values.iter().find(|(j, _)| *j == i) {
                e.kind = match p[i].ty() {
                    Ty::Bool => IExprKind::Bool(re != 0.0),
                    Ty::Int | Ty::UInt => IExprKind::Int(re.round() as i64),
                    Ty::Float => IExprKind::Float(re),
                    Ty::Complex => IExprKind::Complex(re, im),
                };
            }
            return;
        }
        match &mut e.kind {
            IExprKind::Elem(_, a) | IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => expr(a, p, values),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => {
                expr(a, p, values);
                expr(b, p, values);
            }
            IExprKind::Host(_, args) => args.iter_mut().for_each(|a| expr(a, p, values)),
            _ => {}
        }
    }
    fn stmts(st: &mut [IStmt], p: &[Param], values: &[(ParamId, (f64, f64))]) {
        for s in st {
            match s {
                IStmt::Assign(_, e) => expr(e, p, values),
                IStmt::SetElem { index, value, .. } => {
                    expr(index, p, values);
                    expr(value, p, values);
                }
                IStmt::If { arms, otherwise } => {
                    for (c, b) in arms {
                        expr(c, p, values);
                        stmts(b, p, values);
                    }
                    stmts(otherwise, p, values);
                }
                IStmt::While { cond, body } | IStmt::Repeat { body, cond } => {
                    expr(cond, p, values);
                    stmts(body, p, values);
                }
                IStmt::Map { input, .. } => input.iter_mut().for_each(|e| expr(e, p, values)),
                IStmt::Host(_, args) => args.iter_mut().for_each(|e| expr(e, p, values)),
                IStmt::Scope(b) => stmts(b, p, values),
                IStmt::Break | IStmt::Continue | IStmt::Return => {}
            }
        }
    }
    if values.is_empty() {
        return;
    }
    let params = p.params.clone();
    for sec in [&mut p.global_body, &mut p.init, &mut p.loop_body, &mut p.final_body, &mut p.perturb_init, &mut p.perturb_loop, &mut p.creases, &mut p.transform] {
        stmts(sec, &params, values);
    }
    for e in [&mut p.bailout, &mut p.perturb_when, &mut p.degree].into_iter().flatten() {
        expr(e, &params, values);
    }
    // Until neither finds more: the literals carried through, and each
    // branch whose test is now a literal replaced by the arm it takes.
    loop {
        propagate(p);
        let mut changed = false;
        for sec in [&mut p.global_body, &mut p.init, &mut p.loop_body, &mut p.final_body, &mut p.creases, &mut p.transform] {
            changed |= fold_branches(sec);
        }
        if !changed {
            break;
        }
    }
}

/// A test of literals' value: comparisons of ints, floats and bools, and
/// `!`, `&&`, `||` of them; `None` for any other.
fn constant_test(e: &IExpr) -> Option<bool> {
    fn num(e: &IExpr) -> Option<f64> {
        match &e.kind {
            IExprKind::Int(i) => Some(*i as f64),
            IExprKind::Float(f) => Some(*f as f32 as f64),
            IExprKind::Convert(a) => num(a),
            _ => None,
        }
    }
    match &e.kind {
        IExprKind::Bool(b) => Some(*b),
        IExprKind::Unary(super::ast::UnOp::Not, a) => constant_test(a).map(|b| !b),
        IExprKind::Binary(BinOp::And, a, b) => match (constant_test(a), constant_test(b)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        IExprKind::Binary(BinOp::Or, a, b) => match (constant_test(a), constant_test(b)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        IExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge), a, b) => {
            let (x, y) = (num(a)?, num(b)?);
            Some(match op {
                BinOp::Eq => x == y,
                BinOp::Ne => x != y,
                BinOp::Lt => x < y,
                BinOp::Le => x <= y,
                BinOp::Gt => x > y,
                _ => x >= y,
            })
        }
        _ => None,
    }
}

/// Replace each branch whose tests are literals by the arm it takes, its
/// statements in the branch's place; true when one was.
fn fold_branches(st: &mut Vec<IStmt>) -> bool {
    let mut changed = false;
    let mut out = Vec::with_capacity(st.len());
    for s in std::mem::take(st) {
        match s {
            IStmt::If { arms, otherwise } => {
                // Arms before the first whose test is not a literal: drop
                // the false ones, take a true one.
                let mut kept = Vec::new();
                let mut taken: Option<Vec<IStmt>> = None;
                let mut undecided = false;
                for (c, b) in arms {
                    if undecided {
                        kept.push((c, b));
                        continue;
                    }
                    match constant_test(&c) {
                        Some(true) => {
                            taken = Some(b);
                            changed = true;
                            break;
                        }
                        Some(false) => changed = true,
                        None => {
                            undecided = true;
                            kept.push((c, b));
                        }
                    }
                }
                match (taken, kept.is_empty()) {
                    // Every arm before it decided: its statements.
                    (Some(mut b), true) => {
                        fold_branches(&mut b);
                        out.extend(b);
                    }
                    // An undecided arm before the true one: that one ends
                    // the chain as its `else`.
                    (Some(mut b), false) => {
                        let mut kept = kept;
                        for (_, body) in &mut kept {
                            fold_branches(body);
                        }
                        fold_branches(&mut b);
                        out.push(IStmt::If { arms: kept, otherwise: b });
                    }
                    (None, true) => {
                        let mut o = otherwise;
                        fold_branches(&mut o);
                        out.extend(o);
                    }
                    (None, false) => {
                        let mut kept = kept;
                        for (_, body) in &mut kept {
                            changed |= fold_branches(body);
                        }
                        let mut o = otherwise;
                        changed |= fold_branches(&mut o);
                        out.push(IStmt::If { arms: kept, otherwise: o });
                    }
                }
            }
            IStmt::While { cond, mut body } => {
                changed |= fold_branches(&mut body);
                out.push(IStmt::While { cond, body });
            }
            IStmt::Repeat { mut body, cond } => {
                changed |= fold_branches(&mut body);
                out.push(IStmt::Repeat { body, cond });
            }
            IStmt::Scope(mut body) => {
                changed |= fold_branches(&mut body);
                out.push(IStmt::Scope(body));
            }
            other => out.push(other),
        }
    }
    *st = out;
    changed
}

/// Carry the literals baking wrote through the variables holding them and
/// fold the arithmetic on them: `let k = p.power; z ** (k - 1.0)` reads
/// `z ** 2.0` once `power` is 3, which `lang::delta` derives as a whole
/// power. A variable is replaced only where it is assigned once, at a
/// section's top level, to a literal, and read nowhere before; arithmetic
/// folds only by `+`, `-` and `*`, in f64 and rounded once to f32, which is
/// f32's own correctly rounded result.
fn propagate(p: &mut Program) {
    fn literal(e: &IExpr) -> bool {
        matches!(e.kind, IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..))
    }
    fn reads(e: &IExpr, v: VarId) -> bool {
        match &e.kind {
            IExprKind::Var(x) => *x == v,
            IExprKind::Elem(x, a) => *x == v || reads(a, v),
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => reads(a, v),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => reads(a, v) || reads(b, v),
            IExprKind::Host(_, args) => args.iter().any(|a| reads(a, v)),
            _ => false,
        }
    }
    fn stmt_reads(s: &IStmt, v: VarId) -> bool {
        match s {
            IStmt::Assign(_, e) => reads(e, v),
            IStmt::SetElem { index, value, .. } => reads(index, v) || reads(value, v),
            IStmt::If { arms, otherwise } => arms.iter().any(|(c, b)| reads(c, v) || b.iter().any(|s| stmt_reads(s, v))) || otherwise.iter().any(|s| stmt_reads(s, v)),
            IStmt::While { cond, body } | IStmt::Repeat { body, cond } => reads(cond, v) || body.iter().any(|s| stmt_reads(s, v)),
            IStmt::Map { input, .. } => input.iter().any(|e| reads(e, v)),
            IStmt::Host(_, args) => args.iter().any(|e| reads(e, v)),
            IStmt::Scope(b) => b.iter().any(|s| stmt_reads(s, v)),
            IStmt::Break | IStmt::Continue | IStmt::Return => false,
        }
    }
    fn count_writes(st: &[IStmt], v: VarId) -> usize {
        st.iter()
            .map(|s| match s {
                IStmt::Assign(Place::Var(x), _) => usize::from(*x == v),
                IStmt::SetElem { var, .. } => usize::from(*var == v),
                IStmt::Map { out, .. } => out.iter().filter(|x| **x == v).count(),
                IStmt::If { arms, otherwise } => arms.iter().map(|(_, b)| count_writes(b, v)).sum::<usize>() + count_writes(otherwise, v),
                IStmt::While { body, .. } | IStmt::Repeat { body, .. } | IStmt::Scope(body) => count_writes(body, v),
                _ => 0,
            })
            .sum()
    }
    // Replace the variable's reads by the literal, folding as it goes.
    fn subst(e: &mut IExpr, v: VarId, lit: &IExpr) {
        match &mut e.kind {
            IExprKind::Var(x) if *x == v => {
                *e = IExpr { kind: lit.kind.clone(), ty: e.ty };
                return;
            }
            IExprKind::Elem(_, a) | IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => subst(a, v, lit),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => {
                subst(a, v, lit);
                subst(b, v, lit);
            }
            IExprKind::Host(_, args) => args.iter_mut().for_each(|a| subst(a, v, lit)),
            _ => {}
        }
        fold(e);
    }
    fn subst_stmts(st: &mut [IStmt], v: VarId, lit: &IExpr) {
        for s in st {
            match s {
                IStmt::Assign(_, e) => subst(e, v, lit),
                IStmt::SetElem { index, value, .. } => {
                    subst(index, v, lit);
                    subst(value, v, lit);
                }
                IStmt::If { arms, otherwise } => {
                    for (c, b) in arms {
                        subst(c, v, lit);
                        subst_stmts(b, v, lit);
                    }
                    subst_stmts(otherwise, v, lit);
                }
                IStmt::While { cond, body } | IStmt::Repeat { body, cond } => {
                    subst(cond, v, lit);
                    subst_stmts(body, v, lit);
                }
                IStmt::Map { input, .. } => input.iter_mut().for_each(|e| subst(e, v, lit)),
                IStmt::Host(_, args) => args.iter_mut().for_each(|e| subst(e, v, lit)),
                IStmt::Scope(b) => subst_stmts(b, v, lit),
                IStmt::Break | IStmt::Continue | IStmt::Return => {}
            }
        }
    }
    // A float literal's f32 value, as the GPU holds it.
    fn f32v(x: f64) -> f64 {
        x as f32 as f64
    }
    fn fold(e: &mut IExpr) {
        let float = |x: &IExpr| match x.kind {
            IExprKind::Float(f) => Some(f),
            IExprKind::Int(i) if x.ty == Ty::Float => Some(i as f64),
            _ => None,
        };
        let folded = match &e.kind {
            IExprKind::Unary(super::ast::UnOp::Neg, a) if e.ty == Ty::Float => float(a).map(|x| -x),
            IExprKind::Convert(a) if e.ty == Ty::Float => match a.kind {
                IExprKind::Int(i) => Some(i as f64),
                IExprKind::Float(f) => Some(f),
                _ => None,
            },
            IExprKind::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), a, b) if e.ty == Ty::Float => match (float(a), float(b)) {
                (Some(x), Some(y)) => Some(f32v(match op {
                    BinOp::Add => f32v(x) + f32v(y),
                    BinOp::Sub => f32v(x) - f32v(y),
                    _ => f32v(x) * f32v(y),
                })),
                _ => None,
            },
            _ => None,
        };
        if let Some(f) = folded {
            e.kind = IExprKind::Float(f);
        }
    }
    let nvars = p.vars.len();
    loop {
        let mut changed = false;
        for v in 0..nvars {
            if p.vars[v].global {
                continue;
            }
            let sections = [&p.global_body, &p.init, &p.loop_body];
            let writes: usize = sections.iter().map(|s| count_writes(s, v)).sum();
            if writes != 1 {
                continue;
            }
            // Its one assignment, at a section's top level, to a literal,
            // with no read before it.
            let mut found: Option<IExpr> = None;
            let mut read_before = false;
            'find: for sec in sections {
                for st in sec.iter() {
                    if let IStmt::Assign(Place::Var(x), e) = st {
                        if *x == v {
                            if literal(e) {
                                found = Some(e.clone());
                            }
                            break 'find;
                        }
                    }
                    if stmt_reads(st, v) {
                        read_before = true;
                        break 'find;
                    }
                }
            }
            let Some(lit) = found.filter(|_| !read_before) else { continue };
            let still_read = [&p.global_body, &p.init, &p.loop_body, &p.final_body, &p.creases, &p.transform].iter().any(|sec| sec.iter().any(|s| stmt_reads(s, v)))
                || [&p.bailout, &p.perturb_when, &p.degree].iter().any(|e| e.as_ref().is_some_and(|e| reads(e, v)));
            if !still_read {
                continue;
            }
            for sec in [&mut p.global_body, &mut p.init, &mut p.loop_body, &mut p.final_body, &mut p.creases, &mut p.transform] {
                subst_stmts(sec, v, &lit);
            }
            for e in [&mut p.bailout, &mut p.perturb_when, &mut p.degree].into_iter().flatten() {
                subst(e, v, &lit);
            }
            changed = true;
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newtons_choices_and_exponent_are_structural_its_relaxation_is_not() {
        let src = crate::escape::twins::source("newton").unwrap();
        let p = crate::lang::rhai::program(src).unwrap();
        let names: Vec<&str> = structural_params(&p).iter().map(|&i| p.params[i].name.as_str()).collect();
        for want in ["power", "scheme", "func"] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        for not in ["relax_re", "relax_im", "bailout"] {
            assert!(!names.contains(&not), "{not} in {names:?}");
        }
    }

    #[test]
    fn a_float_is_baked_only_at_a_whole_value() {
        let src = crate::escape::twins::source("multibrot").unwrap();
        let p = crate::lang::rhai::program(src).unwrap();
        let s = structural_params(&p);
        let power = p.params.iter().position(|x| x.name == "power").unwrap();
        assert_eq!(s, vec![power]);
        let mut v = ParamValues::defaults(&p);
        v.values[power] = (4.0, 0.0);
        assert_eq!(bake_values(&p, &s, &v), vec![(power, (4.0, 0.0))]);
        v.values[power] = (3.5, 0.0);
        assert!(bake_values(&p, &s, &v).is_empty());
        let mut baked = p.clone();
        bake(&mut baked, &[(power, (4.0, 0.0))]);
        assert!(!format!("{:?}", baked.loop_body).contains(&format!("Param({power})")), "the read is a literal now");
    }
}
