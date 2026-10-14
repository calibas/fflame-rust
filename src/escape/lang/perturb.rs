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
}

/// Whether `p` perturbs, or why not.
pub fn perturbation(p: &Program) -> Result<Perturbable, String> {
    if p.perturb_loop.is_empty() {
        return Err("it has no perturbation equations (`perturbloop:`)".into());
    }
    let Some(test) = &p.bailout else {
        return Err("it has no test".into());
    };
    if reads_a_variable(test, p) {
        return Err("its test reads more than `#z`".into());
    }
    for place in [Predef::X, Predef::Y, Predef::ScreenPixel, Predef::Random, Predef::WhiteSq] {
        if p.reads(place) {
            return Err("it reads the pixel's place on the screen, which the reference point has none of".into());
        }
    }
    // What the loop carries across iterations: read before written in
    // it, and written in it.
    let carried = carried_vars(&p.loop_body);
    Ok(Perturbable { rebase: carried.is_empty() })
}

/// Variables the loop reads before writing in an iteration and also
/// writes: state that changes from one iteration to the next.
fn carried_vars(body: &[IStmt]) -> Vec<VarId> {
    fn reads(e: &IExpr, written: &[VarId], exposed: &mut Vec<VarId>) {
        match &e.kind {
            IExprKind::Var(v) => {
                if !written.contains(v) && !exposed.contains(v) {
                    exposed.push(*v);
                }
            }
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => {
                reads(a, written, exposed)
            }
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => {
                reads(a, written, exposed);
                reads(b, written, exposed);
            }
            _ => {}
        }
    }
    // Conservative: a branch's writes count as made, a read anywhere
    // as exposed when nothing before it wrote the variable.
    fn walk(stmts: &[IStmt], written: &mut Vec<VarId>, exposed: &mut Vec<VarId>, all_writes: &mut Vec<VarId>) {
        for s in stmts {
            match s {
                IStmt::Assign(place, e) => {
                    reads(e, written, exposed);
                    if let Place::Var(v) = place {
                        written.push(*v);
                        all_writes.push(*v);
                    }
                }
                IStmt::If { arms, otherwise } => {
                    let before = written.clone();
                    for (c, b) in arms {
                        reads(c, written, exposed);
                        let mut w = before.clone();
                        walk(b, &mut w, exposed, all_writes);
                    }
                    let mut w = before.clone();
                    walk(otherwise, &mut w, exposed, all_writes);
                    *written = before;
                }
                IStmt::While { cond, body } | IStmt::Repeat { body, cond } => {
                    reads(cond, written, exposed);
                    let mut w = written.clone();
                    walk(body, &mut w, exposed, all_writes);
                    reads(cond, &w, exposed);
                }
            }
        }
    }
    let (mut written, mut exposed, mut all_writes) = (Vec::new(), Vec::new(), Vec::new());
    walk(body, &mut written, &mut exposed, &mut all_writes);
    exposed.into_iter().filter(|v| all_writes.contains(v)).collect()
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
        assert_eq!(perturbation(&m), Ok(Perturbable { rebase: true }));
        // A history: perturbs, but rebases only at the orbit's end.
        let h = program(&format!(
            "H {{\ninit:\n z = 0\n zold = 0\nloop:\n t = z\n z = sqr(z) + #pixel + zold * 0.1\n zold = t\nbailout:\n |z| < 4\n{EQ}}}"
        ));
        assert_eq!(perturbation(&h), Ok(Perturbable { rebase: false }));
        // No equations, a test of more than z, a screen position: no.
        let none = program("N {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}");
        assert!(perturbation(&none).unwrap_err().contains("no perturbation equations"));
        let test = program(&format!("T {{\ninit:\n z = 0\nloop:\n w = z\n z = sqr(z) + #pixel\nbailout:\n |z - w| < 4\n{EQ}}}"));
        assert!(perturbation(&test).unwrap_err().contains("test reads more"));
        let screen = program(&format!("S {{\ninit:\n z = #x * 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n{EQ}}}"));
        assert!(perturbation(&screen).unwrap_err().contains("place on the screen"));
    }
}
