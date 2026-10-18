//! The panel's iteration modifiers on a compiled formula (plan, phase 4,
//! step 9e): Mann damping and the biomorph test, which the built-ins take
//! in their templates (`assembler::step_lines`, `escape_metric`), as
//! rewrites of the program. Every form derived from a program -- its
//! derivative, its perturbation equations, its BLA steps, its nucleus
//! search -- then comes from the rewritten one, so a damped or biomorph
//! custom formula deep-zooms, which the built-ins' cannot.
//!
//! - **Damping** (Mann iteration): `z <- z + alpha (f(z) - z)`, alpha
//!   complex, the formula's pass `f`. The rewrite keeps z before the
//!   pass and blends after it, alpha a complex parameter the renderer
//!   binds to the panel's (`custom::resolve`). A formula carrying the
//!   previous z keeps the one before the pass, the damped iterate, as
//!   the built-ins' history does. Equations a formula wrote for itself
//!   describe the undamped map, so they go, and the loop's are derived.
//! - **Biomorph**: the escape test reads `(Re z)^2` or `(Im z)^2` where
//!   it read `|z|`, as the built-ins' metric is replaced. Only a test
//!   that reads `|z|` has one.

use super::ast::BinOp;
use super::builtins::Builtin;
use super::ir::*;

/// The biomorph axis: the part of z the escape test reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    Re,
    Im,
}

/// The name of the damping parameter a damped program adds: `alpha`,
/// unless the formula has a parameter of that name already.
pub fn alpha_name(p: &Program) -> String {
    let taken = |n: &str| p.params.iter().any(|x| x.name == n);
    let mut name = "alpha".to_string();
    let mut k = 1;
    while taken(&name) {
        k += 1;
        name = format!("alpha{k}");
    }
    name
}

fn cx(kind: IExprKind) -> IExpr {
    IExpr { kind, ty: Ty::Complex }
}

fn bin(op: BinOp, a: IExpr, b: IExpr) -> IExpr {
    cx(IExprKind::Binary(op, Box::new(a), Box::new(b)))
}

/// The program damped (the module's docs): its parameter `alpha_name`
/// last, `(1, 0)` by default, the undamped map.
pub fn damped(p: &Program) -> Program {
    let mut out = p.clone();
    out.params.push(Param {
        name: alpha_name(p),
        caption: "Damping".to_string(),
        kind: ParamKind::Complex,
        default: (1.0, 0.0),
        min: None,
        max: None,
        hint: "Mann iteration: z + alpha (f(z) - z) a pass (the panel's Damping)".to_string(),
        choices: Vec::new(),
        exponential: false,
        span: Default::default(),
    });
    let alpha = out.params.len() - 1;
    out.vars.push(Var { name: "z_undamped".to_string(), ty: Ty::Complex, span: Default::default(), stored_real: false, global: false, perturb: false, dims: Vec::new() });
    let before = out.vars.len() - 1;
    let z = cx(IExprKind::Predef(Predef::Z));
    let kept = cx(IExprKind::Var(before));
    let step = bin(BinOp::Mul, cx(IExprKind::Param(alpha)), bin(BinOp::Sub, z.clone(), kept.clone()));
    out.loop_body = [vec![IStmt::Assign(Place::Var(before), z)], p.loop_body.clone(), vec![IStmt::Assign(Place::Z, bin(BinOp::Add, kept, step))]].concat();
    // The formula's own equations are the undamped map's.
    out.perturb_init.clear();
    out.perturb_loop.clear();
    out.perturb_when = None;
    out.carried.clear();
    out
}

/// Whether an expression is an escape test's measure of z: `|z|`, or
/// the exponential and trig families' `real(z)` and `abs(imag(z))` --
/// whatever the built-ins' metric is, their biomorph replaces it.
fn is_metric_of_z(e: &IExpr) -> bool {
    let z = |x: &IExpr| matches!(x.kind, IExprKind::Predef(Predef::Z));
    match &e.kind {
        IExprKind::Modulus(a) => z(a),
        IExprKind::Call(Builtin::Real, a) => z(a),
        IExprKind::Call(Builtin::Abs, a) if e.ty == Ty::Float => matches!(&a.kind, IExprKind::Call(Builtin::Imag, b) if z(b)),
        IExprKind::Convert(a) => is_metric_of_z(a),
        _ => false,
    }
}

/// `e` with each measure of z replaced by the axis's square; whether
/// any was.
fn rewrite(e: &IExpr, axis: Axis, found: &mut bool) -> IExpr {
    if is_metric_of_z(e) {
        *found = true;
        let part = if axis == Axis::Re { Builtin::Real } else { Builtin::Imag };
        let z = cx(IExprKind::Predef(Predef::Z));
        let p = IExpr { kind: IExprKind::Call(part, Box::new(z)), ty: Ty::Float };
        return IExpr { kind: IExprKind::Call(Builtin::Sqr, Box::new(p)), ty: Ty::Float };
    }
    let r = |x: &IExpr, found: &mut bool| Box::new(rewrite(x, axis, found));
    let kind = match &e.kind {
        IExprKind::Convert(a) => IExprKind::Convert(r(a, found)),
        IExprKind::Unary(op, a) => IExprKind::Unary(*op, r(a, found)),
        IExprKind::PowInt(a, n) => IExprKind::PowInt(r(a, found), *n),
        IExprKind::Modulus(a) => IExprKind::Modulus(r(a, found)),
        IExprKind::Call(f, a) => IExprKind::Call(*f, r(a, found)),
        IExprKind::UserCall(f, a) => IExprKind::UserCall(*f, r(a, found)),
        IExprKind::MakeComplex(a, b) => IExprKind::MakeComplex(r(a, found), r(b, found)),
        IExprKind::Binary(op, a, b) => IExprKind::Binary(*op, r(a, found), r(b, found)),
        other => other.clone(),
    };
    IExpr { kind, ty: e.ty }
}

/// The program with its escape test reading the axis's part (the
/// module's docs); `None` when its test reads no measure of z.
pub fn biomorph(p: &Program, axis: Axis) -> Option<Program> {
    let mut found = false;
    let test = rewrite(p.bailout.as_ref()?, axis, &mut found);
    found.then(|| {
        let mut out = p.clone();
        out.bailout = Some(test);
        out
    })
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::interp::{run, Env, ParamValues};
    use super::*;

    fn program(src: &str) -> Program {
        let (file, d) = super::super::parser::parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = super::super::check::check(&file.entries[0], super::super::EntryKind::Formula, super::super::Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        p.unwrap()
    }

    /// Damped, a program iterates Mann's `z + alpha (f(z) - z)`: the
    /// Mandelbrot from zero and Phoenix's history (the damped iterate),
    /// against the iteration written out; alpha 1 is the formula itself.
    #[test]
    fn a_damped_program_iterates_manns_step() {
        let alpha = C64::new(0.7, 0.2);
        let c = C64::new(-0.4, 0.3);
        let m = program("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 1e6\n}");
        let d = damped(&m);
        let mut params = ParamValues::defaults(&d);
        assert_eq!(d.params.last().unwrap().name, "alpha");
        let a = params.values.len() - 1;
        assert_eq!(run(&d, &params, None, &Env::at(c, 12)).z, run(&m, &ParamValues::defaults(&m), None, &Env::at(c, 12)).z, "alpha 1");
        params.values[a] = (alpha.re, alpha.im);
        let mut z = C64::ZERO;
        for _ in 0..12 {
            z = z.add(alpha.mul(z.mul(z).add(c).sub(z)));
        }
        let got = run(&d, &params, None, &Env::at(c, 12)).z;
        assert!(got.sub(z).abs() < 1e-12, "{got:?} against {z:?}");
        // The previous iterate a formula keeps is the damped one, as the
        // built-ins' history.
        let ph = program("P {\ninit:\n z = 0\n y = 0\nloop:\n t = z\n z = sqr(z) + #pixel - 0.5 * y\n y = t\nbailout:\n |z| < 1e6\n}");
        let d = damped(&ph);
        let mut params = ParamValues::defaults(&d);
        params.values[a] = (alpha.re, alpha.im);
        let (mut z, mut y) = (C64::ZERO, C64::ZERO);
        for _ in 0..12 {
            let raw = z.mul(z).add(c).sub(y.scale(0.5));
            y = z;
            z = z.add(alpha.mul(raw.sub(z)));
        }
        let got = run(&d, &params, None, &Env::at(c, 12)).z;
        assert!(got.sub(z).abs() < 1e-12, "{got:?} against {z:?}");
        // A formula with a parameter of the name gets another.
        let taken = program("A {\ninit:\n z = 0\nloop:\n z = sqr(z) + @alpha\nbailout:\n |z| < 4\ndefault:\n param alpha\n  default = (0.1, 0.2)\n endparam\n}");
        assert_eq!(alpha_name(&taken), "alpha2");
    }

    /// The biomorph tests a part of z where the test read `|z|`, as the
    /// built-ins' metric: escaping on `(Re z)^2` alone; a test without
    /// `|z|` has none; the twins' tests (`norm_sqr` in Rhai) have one.
    #[test]
    fn the_biomorph_tests_a_part_of_z() {
        let m = program("M {\ninit:\n z = 0\nloop:\n z = z^3 + #pixel\nbailout:\n |z| < 4\n}");
        let b = biomorph(&m, Axis::Re).expect("|z| in the test");
        let c = C64::new(0.1, 0.9);
        let r = run(&b, &ParamValues::defaults(&b), None, &Env::at(c, 200));
        let mut z = C64::ZERO;
        let mut n = 200;
        for i in 0..200 {
            z = z.mul(z).mul(z).add(c);
            if z.re * z.re >= 4.0 {
                n = i + 1;
                break;
            }
        }
        assert_eq!((r.n, r.z), (n, z));
        // The exponential and trig families' measures, as the built-ins'
        // biomorph replaces any metric: `real(z)` and `abs(imag(z))`.
        for test in ["real(z) < 50", "abs(imag(z)) < 50"] {
            let e = program(&format!("E {{\ninit:\n z = 0\nloop:\n z = exp(z) + #pixel\nbailout:\n {test}\n}}"));
            let b = biomorph(&e, Axis::Im).unwrap_or_else(|| panic!("{test}"));
            let c = C64::new(0.3, 2.1);
            let r = run(&b, &ParamValues::defaults(&b), None, &Env::at(c, 50));
            let mut z = C64::ZERO;
            let mut n = 50;
            for i in 0..50 {
                z = z.exp().add(c);
                if z.im * z.im >= 50.0 {
                    n = i + 1;
                    break;
                }
            }
            assert_eq!(r.n, n, "{test}");
        }
        // A distance is not a measure of z.
        let other = program("N {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z - #pixel| < 4\n}");
        assert!(biomorph(&other, Axis::Im).is_none());
        for name in ["mandelbrot", "multibrot", "burning_ship", "tricorn", "lambda", "cactus"] {
            let src = crate::escape::twins::custom_source(name).unwrap();
            let p = crate::escape::custom::check_entry(super::super::EntryKind::Formula, &src).unwrap();
            assert!(biomorph(&p, Axis::Re).is_some(), "{name}");
        }
    }
}
