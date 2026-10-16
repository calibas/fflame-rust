//! Whether a formula can take the perturbed path, and how (plan,
//! phase 3).
//!
//! The perturbed path keeps the reference orbit's iterate and nothing
//! else (UF: "all intermediate values for the main Z variable"), and
//! runs each pixel by the formula's perturbation equations
//! (`perturbinit:`, `perturbloop:`). So a formula perturbs when it has
//! equations, when its test reads the iterate and nothing else that
//! changes, and when nothing it computes depends on where the pixel is
//! on the screen (the reference point has no place there).
//!
//! Rebasing (restarting a pixel against the orbit's start when its
//! deviation would be smaller, Zhuoran) treats the pixel's full iterate
//! as a fresh start of the same map. That is the map only when the
//! loop carries nothing from one iteration to the next but the iterate:
//! a variable the loop both reads before writing and writes (`zold`)
//! would restart with the reference's value, not the pixel's. Such a
//! formula still perturbs, rebasing only where the orbit ends, as the
//! built-in Ducks tier does.

use super::ir::*;
use super::wgsl::reads_a_variable;

/// How a formula takes the perturbed path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perturbable {
    /// The loop carries only the iterate: rebase on the smaller
    /// deviation as well as at the orbit's end.
    pub rebase: bool,
    /// A loop that branches on its pass (a hybrid): one reference orbit
    /// per phase of its cycle besides the main one, and a pixel rebases
    /// only onto the reference of its own phase.
    pub cycle: Option<Cycle>,
}

/// The most phases a cycle may have and still perturb: the references
/// are one orbit each, interleaved in the GPU's orbit buffer.
pub const MAX_PHASES: u32 = 31;

impl Perturbable {
    /// The reference orbits: the main one, and one per phase.
    pub fn references(&self) -> u32 {
        1 + self.cycle.map_or(0, |c| c.period)
    }
}

/// Whether `p` perturbs, or why not.
pub fn perturbation(p: &Program) -> Result<Perturbable, String> {
    if p.perturb_loop.is_empty() {
        return Err("it has no perturbation equations (`perturbloop:`)".into());
    }
    if p.bailout.is_none() {
        return Err("it has no test".into());
    }
    if perturb_test(p).is_none() {
        return Err("its test reads more than `#z`".into());
    }
    // Derived equations may carry one value besides z (phase 4, step
    // 5): the GPU keeps its deviation where Phoenix keeps its history.
    if p.carried.len() > 1 {
        return Err(format!("its equations carry {} values besides z; deep zoom carries one", p.carried.len()));
    }
    for place in [Predef::X, Predef::Y, Predef::ScreenPixel, Predef::Random, Predef::WhiteSq] {
        if p.reads(place) {
            return Err("it reads the pixel's place on the screen, which the reference point has none of".into());
        }
    }
    // The reference orbit is computed apart from any render's size.
    for size in [Predef::Width, Predef::Height, Predef::ScrnMax] {
        if p.reads(size) {
            return Err("it reads the image's size".into());
        }
    }
    // What the loop carries across iterations: read before written in
    // it, and written in it.
    let carried = carried_vars(&p.loop_body);
    // A loop that reads its pass needs its cycle: the pixel follows
    // only a reference at its own phase.
    let cycle = if p.reads(Predef::Pass) {
        match p.cycle {
            None => return Err("it reads the pass with no cycle to it".into()),
            Some(c) if c.period == 0 || c.period > MAX_PHASES => {
                return Err(format!("its cycle has {} passes; deep zoom takes at most {MAX_PHASES}", c.period))
            }
            Some(c) => Some(c),
        }
    } else {
        None
    };
    // What the derived equations carry (`Program::carried`) rebases with
    // z; anything else carried restarts wrong, so only at the orbit's end.
    let rebase = carried.iter().all(|v| p.carried.iter().any(|c| c.var == *v));
    Ok(Perturbable { rebase, cycle })
}

/// The formula's test as the perturbed path evaluates it, on the pixel's
/// full z: as written when it reads no variable, else with the init's
/// constants put in by value -- variables the init sets from the pixel,
/// the parameters and the constants, which the loop never writes (a
/// `bailout = 4` in `init:`, common in UF-format conversions of Fractint
/// formulas). `None` when it reads anything else.
pub fn perturb_test(p: &Program) -> Option<IExpr> {
    let test = p.bailout.as_ref()?;
    if !reads_a_variable(test, p) {
        return Some(test.clone());
    }
    // A carried variable (`Program::carried`) is read at its full value,
    // which the perturbed path keeps; the init's constants by value.
    let carried = |v: VarId| p.carried.iter().any(|c| c.var == v);
    let mut values = super::delta::init_values(p).unwrap_or_else(|| vec![None; p.vars.len()]);
    for c in &p.carried {
        values[c.var] = Some(IExpr { kind: IExprKind::Var(c.var), ty: p.vars[c.var].ty });
    }
    fn constants_only(e: &IExpr, p: &Program, values: &[Option<IExpr>]) -> bool {
        match &e.kind {
            IExprKind::Var(v) => {
                p.vars[*v].global
                    || p.carried.iter().any(|c| c.var == *v)
                    || (values[*v].is_some() && !super::delta::writes_var(&p.loop_body, *v))
            }
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => {
                constants_only(a, p, values)
            }
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => constants_only(a, p, values) && constants_only(b, p, values),
            _ => true,
        }
    }
    if !constants_only(test, p, &values) {
        return None;
    }
    let mut ok = true;
    let poisoned = vec![false; p.vars.len()];
    let t = super::delta::subst(test, &values, &poisoned, p, &mut ok);
    (ok && !reads_other_than(&t, p, &carried)).then_some(t)
}

/// Whether `e` reads a variable that is neither global nor `allowed`.
fn reads_other_than(e: &IExpr, p: &Program, allowed: &dyn Fn(VarId) -> bool) -> bool {
    match &e.kind {
        IExprKind::Var(v) => !p.vars[*v].global && !allowed(*v),
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => {
            reads_other_than(a, p, allowed)
        }
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => reads_other_than(a, p, allowed) || reads_other_than(b, p, allowed),
        _ => false,
    }
}

/// Variables the loop reads before writing in an iteration and also
/// writes: state that changes from one iteration to the next.
fn carried_vars(body: &[IStmt]) -> Vec<VarId> {
    // A variable read where a write before it in the pass is not
    // certain -- a branch's writes count when every arm makes them, as
    // the checker and the derivation (`lang::delta`) take them -- and
    // written in the pass.
    let mut f = super::check::Flow::default();
    super::check::flow(body, &mut f);
    let mut out: Vec<VarId> = f.exposed.into_iter().filter(|v| f.any_write.contains(v)).collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::super::check::check;
    use super::super::parser::parse_file;
    use super::super::{Dialect, EntryKind};
    use super::*;

    fn program(src: &str) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        p.unwrap()
    }

    const EQ: &str = "perturbinit:\n #dz = 0\nperturbloop:\n #dz = 2 * #z * #dz + sqr(#dz) + #dpixel\n";

    #[test]
    fn what_perturbs_and_what_rebases() {
        // The Mandelbrot: perturbs and rebases; `c`, written in init and
        // only read in the loop, is no carried state.
        let m = program(&format!("M {{\ninit:\n z = 0\n c = #pixel\nloop:\n z = sqr(z) + c\nbailout:\n |z| < 4\n{EQ}}}"));
        assert_eq!(perturbation(&m), Ok(Perturbable { rebase: true, cycle: None }));
        // A history: perturbs, but rebases only at the orbit's end.
        let h = program(&format!(
            "H {{\ninit:\n z = 0\n zold = 0\nloop:\n t = z\n z = sqr(z) + #pixel + zold * 0.1\n zold = t\nbailout:\n |z| < 4\n{EQ}}}"
        ));
        assert_eq!(perturbation(&h), Ok(Perturbable { rebase: false, cycle: None }));
        // No equations, a test of more than z, a screen position: no.
        let none = program("N {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}");
        assert!(perturbation(&none).unwrap_err().contains("no perturbation equations"));
        let test = program(&format!("T {{\ninit:\n z = 0\nloop:\n w = z\n z = sqr(z) + #pixel\nbailout:\n |z - w| < 4\n{EQ}}}"));
        assert!(perturbation(&test).unwrap_err().contains("test reads more"));
        let screen = program(&format!("S {{\ninit:\n z = #x * 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n{EQ}}}"));
        assert!(perturbation(&screen).unwrap_err().contains("place on the screen"));
    }

    #[test]
    fn a_test_reads_the_inits_constants_by_value() {
        // `b` set once in the init, from a parameter through another
        // variable: the test is `|z| < @bail * 2`.
        let c = program(&format!(
            "C {{\ninit:\n z = 0\n k = @bail\n b = k * 2\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < b\n{EQ}default:\n float param bail\n  default = 2\n endparam\n}}"
        ));
        assert!(perturbation(&c).is_ok());
        let t = perturb_test(&c).unwrap();
        assert!(!reads_a_variable(&t, &c));
        // The interpreter's perturbed run escapes where the direct one
        // does, at the same count.
        use super::super::builtins::C64;
        use super::super::interp::{run, run_perturbed, Env, ParamValues};
        let env = Env::at(C64::new(0.3, 0.6), 200);
        let params = ParamValues::defaults(&c);
        let direct = run(&c, &params, None, &env);
        let perturbed = run_perturbed(&c, &params, &env, C64::new(0.3, 0.6001), true).unwrap();
        assert!(direct.escaped && perturbed.escaped);
        assert_eq!(direct.n, perturbed.n);
        // A variable the loop changes stays out of it.
        let w = program(&format!(
            "W {{\ninit:\n z = 0\n b = 4\nloop:\n z = sqr(z) + #pixel\n b = b + 1\nbailout:\n |z| < b\n{EQ}}}"
        ));
        assert!(perturb_test(&w).is_none());
    }
}
