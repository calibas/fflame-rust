//! UF's `switch:` as the Julia toggle.
//!
//! A formula's `switch:` names another formula of the same file and
//! copies parameters into it (UF's help, "Switch feature"):
//! `type = "Julia"`, then `destination = source` lines, where the source
//! is one of the formula's own parameters or `#pixel`, the point the user
//! clicked. Parameters not copied take their defaults, and a copy between
//! different types is ignored. UF switches on a click; here the pair
//! becomes ONE program, and the Julia toggle is the switch: off, the
//! formula runs; on, the target does, and what `#pixel` gave it is read
//! from the Julia constant. A switch that passes no `#pixel` (a Julia's
//! back to its Mandelbrot) is no toggle and is ignored with a warning.
//!
//! The pair is spliced at the IR: the target's variables follow the
//! formula's, its parameters are the formula's where copied and its own
//! otherwise (renamed when a name is taken), and each section becomes
//! `if (ismand) formula else target` -- Fractint's own toggle, which the
//! engine already drives.

use super::ast::{Entry, ExprKind, File, SettingValue};
use super::check::{check, FDATA_FLOATS, PARAM_SLOTS};
use super::ir::*;
use super::{Diag, Dialect, EntryKind};

/// Check a formula entry, and when its `switch:` names a formula of
/// `file` that takes `#pixel`, the pair as one program.
pub fn check_formula(file: &File, entry: &Entry, dialect: Dialect) -> (Option<Program>, Vec<Diag>) {
    let (program, mut diags) = check(entry, EntryKind::Formula, dialect);
    let Some(program) = program else {
        return (None, diags);
    };
    let Some(ty) = entry.switch.get("type") else {
        return (Some(program), diags);
    };
    let span = ty.span;
    let target_name = match &ty.value {
        SettingValue::Strings(v) if !v.is_empty() => v[0].clone(),
        _ => {
            diags.push(Diag::warning(span, "`type` names the formula to switch to, in quotes"));
            return (Some(program), diags);
        }
    };
    let ignored = |why: String| Diag::warning(span, format!("{why}; the Julia toggle is not offered"));
    let Some(target) = file.entries.iter().find(|e| e.name.eq_ignore_ascii_case(&target_name)) else {
        diags.push(ignored(format!("`switch:` names `{target_name}`, which is not in this file")));
        return (Some(program), diags);
    };
    if target.span == entry.span {
        diags.push(ignored("`switch:` names this formula itself".to_string()));
        return (Some(program), diags);
    }
    let (tgt, tdiags) = check(target, EntryKind::Formula, dialect);
    let Some(tgt) = tgt.filter(|_| !super::has_errors(&tdiags)) else {
        let first = tdiags.iter().find(|d| d.severity == super::Severity::Error).map_or(String::new(), |d| d.message.clone());
        diags.push(ignored(format!("the switch target `{}` does not compile ({first})", target.name)));
        return (Some(program), diags);
    };

    // The copies: the target's parameter or function, from the
    // formula's or from `#pixel`.
    let mut params: Vec<Option<Source>> = vec![None; tgt.params.len()];
    let mut funcs: Vec<Option<FuncId>> = vec![None; tgt.funcs.len()];
    for item in &entry.switch.items {
        if item.name == "type" {
            continue;
        }
        let dest = item.name.trim_start_matches('@').to_ascii_lowercase();
        let source = match &item.value {
            SettingValue::Expr(e) => match &e.kind {
                ExprKind::Predef(n) if n == "pixel" => Some(None),
                ExprKind::Name(n) | ExprKind::Param(n) => Some(Some(n.to_ascii_lowercase())),
                _ => None,
            },
            SettingValue::Func(n) => Some(Some(n.to_ascii_lowercase())),
            SettingValue::Strings(_) => None,
        };
        let Some(source) = source else {
            diags.push(Diag::warning(item.span, format!("`{}`: copy a parameter or `#pixel`", item.name)));
            continue;
        };
        if let Some(d) = tgt.params.iter().position(|p| p.name == dest) {
            // UF ignores a copy between different types, and `#pixel`
            // goes to a complex parameter only.
            let kind = tgt.params[d].kind;
            let copied = match &source {
                None if kind == ParamKind::Complex => Ok(Source::JuliaC),
                None => Err("`#pixel` goes to a complex parameter only".to_string()),
                Some(n) => program
                    .params
                    .iter()
                    .position(|p| p.name == *n && p.kind == kind)
                    .map(Source::Param)
                    .ok_or_else(|| format!("this formula has no {} parameter `{n}`", kind_name(kind))),
            };
            match copied {
                Ok(s) => params[d] = Some(s),
                Err(why) => diags.push(Diag::warning(item.span, format!("`{}`: {why}; ignored, as UF does", item.name))),
            }
        } else if let Some(d) = tgt.funcs.iter().position(|f| f.name == dest) {
            match source.and_then(|n| program.funcs.iter().position(|f| f.name == n)) {
                Some(s) => funcs[d] = Some(s),
                None => diags.push(Diag::warning(item.span, format!("`{}`: no such function to copy; ignored", item.name))),
            }
        } else {
            diags.push(Diag::warning(item.span, format!("`{}` has no parameter `{dest}`; ignored", target.name)));
        }
    }
    if !params.iter().any(|s| matches!(s, Some(Source::JuliaC))) {
        diags.push(ignored("`switch:` passes no `#pixel`".to_string()));
        return (Some(program), diags);
    }

    let pair = splice(program, tgt, &params, &funcs, &target.name.to_ascii_lowercase());
    if pair.param_slots() > PARAM_SLOTS {
        diags.push(Diag::error(
            entry.span,
            format!(
                "with its switch target, the parameters take {} floats; at most {PARAM_SLOTS} fit",
                pair.param_slots()
            ),
        ));
        return (None, diags);
    }
    if pair.global_floats() > FDATA_FLOATS {
        diags.push(Diag::error(
            entry.span,
            format!("with its switch target, the `global:` values take {} floats; at most {FDATA_FLOATS} fit", pair.global_floats()),
        ));
        return (None, diags);
    }
    (Some(pair), diags)
}

fn kind_name(k: ParamKind) -> &'static str {
    match k {
        ParamKind::Bool => "bool",
        ParamKind::Int => "int",
        ParamKind::Float => "float",
        ParamKind::Complex => "complex",
        ParamKind::Enum => "enum",
    }
}

/// What a copied target parameter reads instead of its own value.
#[derive(Clone, Copy, Debug)]
enum Source {
    Param(ParamId),
    JuliaC,
}

/// How the target's ids map into the pair.
struct Remap<'a> {
    var_offset: usize,
    params: &'a [ParamTo],
    funcs: &'a [FuncId],
}

#[derive(Clone, Copy)]
enum ParamTo {
    Param(ParamId),
    JuliaC,
}

fn splice(src: Program, tgt: Program, copies: &[Option<Source>], func_copies: &[Option<FuncId>], tag: &str) -> Program {
    let mut pair = src;
    let var_offset = pair.vars.len();
    pair.vars.extend(tgt.vars.iter().cloned());
    // The target's own parameters follow the formula's; a name already
    // taken gets the target's name in front, so each keeps its value in
    // the config.
    let mut param_to = Vec::with_capacity(tgt.params.len());
    for (d, p) in tgt.params.iter().enumerate() {
        param_to.push(match copies[d] {
            Some(Source::Param(s)) => ParamTo::Param(s),
            Some(Source::JuliaC) => ParamTo::JuliaC,
            None => {
                let mut own = p.clone();
                if pair.params.iter().any(|q| q.name == own.name) {
                    own.name = format!("{tag}_{}", own.name);
                }
                own.caption = format!("{} ({tag})", own.caption);
                pair.params.push(own);
                ParamTo::Param(pair.params.len() - 1)
            }
        });
    }
    let mut func_to = Vec::with_capacity(tgt.funcs.len());
    for (d, f) in tgt.funcs.iter().enumerate() {
        func_to.push(match func_copies[d] {
            Some(s) => s,
            None => {
                let mut own = f.clone();
                if pair.funcs.iter().any(|g| g.name == own.name) {
                    own.name = format!("{tag}_{}", own.name);
                }
                own.caption = format!("{} ({tag})", own.caption);
                pair.funcs.push(own);
                pair.funcs.len() - 1
            }
        });
    }
    let r = Remap { var_offset, params: &param_to, funcs: &func_to };

    let on_parameter_plane = IExpr {
        kind: IExprKind::Convert(Box::new(IExpr { kind: IExprKind::Predef(Predef::IsMand), ty: Ty::Complex })),
        ty: Ty::Bool,
    };
    let branch = |mine: Vec<IStmt>, theirs: Vec<IStmt>| -> Vec<IStmt> {
        if mine.is_empty() && theirs.is_empty() {
            return Vec::new();
        }
        vec![IStmt::If { arms: vec![(on_parameter_plane.clone(), mine)], otherwise: theirs }]
    };
    pair.global_body.extend(r.stmts(&tgt.global_body));
    pair.globals.extend(tgt.globals.iter().map(|&v| v + var_offset));
    pair.init = branch(std::mem::take(&mut pair.init), r.stmts(&tgt.init));
    pair.loop_body = branch(std::mem::take(&mut pair.loop_body), r.stmts(&tgt.loop_body));
    let truth = || IExpr { kind: IExprKind::Bool(true), ty: Ty::Bool };
    let mine = pair.bailout.take().unwrap_or_else(truth);
    let theirs = tgt.bailout.as_ref().map_or_else(truth, |b| r.expr(b));
    let both = |op, a, b| IExpr { kind: IExprKind::Binary(op, Box::new(a), Box::new(b)), ty: Ty::Bool };
    let not = |a| IExpr { kind: IExprKind::Unary(super::ast::UnOp::Not, Box::new(a)), ty: Ty::Bool };
    use super::ast::BinOp::{And, Or};
    pair.bailout = Some(both(
        Or,
        both(And, on_parameter_plane.clone(), mine),
        both(And, not(on_parameter_plane), theirs),
    ));
    pair
}

impl Remap<'_> {
    fn stmts(&self, s: &[IStmt]) -> Vec<IStmt> {
        s.iter().map(|x| self.stmt(x)).collect()
    }

    fn stmt(&self, s: &IStmt) -> IStmt {
        match s {
            IStmt::Assign(place, e) => {
                let place = match place {
                    Place::Var(v) => Place::Var(v + self.var_offset),
                    other => *other,
                };
                IStmt::Assign(place, self.expr(e))
            }
            IStmt::If { arms, otherwise } => IStmt::If {
                arms: arms.iter().map(|(c, b)| (self.expr(c), self.stmts(b))).collect(),
                otherwise: self.stmts(otherwise),
            },
            IStmt::While { cond, body } => IStmt::While { cond: self.expr(cond), body: self.stmts(body) },
            IStmt::Repeat { body, cond } => IStmt::Repeat { body: self.stmts(body), cond: self.expr(cond) },
        }
    }

    fn expr(&self, e: &IExpr) -> IExpr {
        let b = |x: &IExpr| Box::new(self.expr(x));
        let kind = match &e.kind {
            IExprKind::Var(v) => IExprKind::Var(v + self.var_offset),
            IExprKind::Param(p) => match self.params[*p] {
                ParamTo::Param(q) => IExprKind::Param(q),
                ParamTo::JuliaC => IExprKind::Predef(Predef::JuliaC),
            },
            IExprKind::Convert(a) => IExprKind::Convert(b(a)),
            IExprKind::MakeComplex(x, y) => IExprKind::MakeComplex(b(x), b(y)),
            IExprKind::Unary(op, a) => IExprKind::Unary(*op, b(a)),
            IExprKind::Binary(op, x, y) => IExprKind::Binary(*op, b(x), b(y)),
            IExprKind::PowInt(a, n) => IExprKind::PowInt(b(a), *n),
            IExprKind::Modulus(a) => IExprKind::Modulus(b(a)),
            IExprKind::Call(f, a) => IExprKind::Call(*f, b(a)),
            IExprKind::UserCall(f, a) => IExprKind::UserCall(self.funcs[*f], b(a)),
            IExprKind::UserDeriv(f, a) => IExprKind::UserDeriv(self.funcs[*f], b(a)),
            IExprKind::UserJac(f, a, w) => IExprKind::UserJac(self.funcs[*f], b(a), *w),
            other => other.clone(),
        };
        IExpr { kind, ty: e.ty }
    }
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::interp::{run, Env, ParamValues};
    use super::super::parser::parse_file;
    use super::*;

    /// UF's own example pair, from its help.
    const PAIR: &str = "Mandelbrot {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < @bailout\n\
                        default:\n float param bailout\n  default = 4\n endparam\nswitch:\n type = \"Julia\"\n\
                        seed = #pixel\n bailout = bailout\n}\n\
                        Julia {\ninit:\n z = #pixel\nloop:\n z = sqr(z) + @seed\nbailout:\n |z| < @bailout\n\
                        default:\n param seed\n  default = (-0.75, 0.2)\n endparam\n float param bailout\n  default = 4\n\
                        endparam\nswitch:\n type = \"Mandelbrot\"\n bailout = bailout\n}\n";

    fn checked(src: &str, name: &str) -> (Option<Program>, Vec<String>) {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let entry = file.entries.iter().find(|e| e.name == name).unwrap();
        let (p, d) = check_formula(&file, entry, Dialect::UltraFractal);
        (p, d.iter().map(|x| x.render(src)).collect())
    }

    /// Off, the pair is the Mandelbrot; on, the Julia, its seed the
    /// Julia constant and its bailout the Mandelbrot's.
    #[test]
    fn the_julia_toggle_switches_to_the_target() {
        let (pair, d) = checked(PAIR, "Mandelbrot");
        assert!(d.is_empty(), "{d:?}");
        let pair = pair.unwrap();
        assert!(pair.reads(Predef::IsMand), "the toggle drives it");
        let names: Vec<_> = pair.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["bailout"], "both copies are the Mandelbrot's own");
        let (alone_m, _) = checked(&PAIR.replace("switch:\n type = \"Julia\"", "switch:\n stype = \"Julia\""), "Mandelbrot");
        let (alone_j, _) = checked(PAIR, "Julia");
        let (alone_m, alone_j) = (alone_m.unwrap(), alone_j.unwrap());
        let seed = C64::new(-0.12, 0.74);
        for px in [C64::new(-0.6, 0.3), C64::new(0.2, -0.5), C64::new(0.35, 0.1)] {
            let env = Env::at(px, 200);
            let pv = ParamValues::defaults(&pair);
            assert_eq!(run(&pair, &pv, None, &env), run(&alone_m, &ParamValues::defaults(&alone_m), None, &env), "off");
            let jenv = Env { julia: true, julia_c: seed, ..env };
            let mut jv = ParamValues::defaults(&alone_j);
            jv.values[0] = (seed.re, seed.im);
            assert_eq!(run(&pair, &pv, None, &jenv), run(&alone_j, &jv, None, &jenv), "on");
        }
        // The Julia's own switch passes no #pixel: no toggle.
        let (j, d) = checked(PAIR, "Julia");
        assert!(!j.unwrap().reads(Predef::IsMand));
        assert!(d.iter().any(|m| m.contains("passes no `#pixel`")), "{d:?}");
    }

    /// A target parameter not copied keeps its own value, renamed when
    /// the formula has one of the name; what cannot be copied warns.
    #[test]
    fn uncopied_and_mismatched_parameters() {
        let src = "M {\ninit:\n z = 0\nloop:\n z = z^@power + #pixel\nbailout:\n |z| < 4\n\
                   default:\n float param power\n  default = 2\n endparam\nswitch:\n type = \"J\"\n seed = #pixel\n\
                   power = nothing\n k = #pixel\n}\n\
                   J {\ninit:\n z = #pixel\nloop:\n z = z^@power + @seed * @k\nbailout:\n |z| < 4\n\
                   default:\n float param power\n  default = 3\n endparam\n float param k\n  default = 1\n endparam\n}\n";
        let (pair, d) = checked(src, "M");
        let pair = pair.unwrap();
        let names: Vec<_> = pair.params.iter().map(|p| (p.name.as_str(), p.default.0)).collect();
        assert_eq!(names, [("power", 2.0), ("j_power", 3.0), ("k", 1.0)]);
        assert!(d.iter().any(|m| m.contains("`power`: this formula has no float parameter `nothing`")), "{d:?}");
        assert!(d.iter().any(|m| m.contains("`k`: `#pixel` goes to a complex parameter only")), "{d:?}");
        // A missing target or a broken one: the formula alone, warned.
        let (p, d) = checked(&src.replace("type = \"J\"", "type = \"Nope\""), "M");
        assert!(!p.unwrap().reads(Predef::IsMand));
        assert!(d.iter().any(|m| m.contains("not in this file")), "{d:?}");
        let (p, d) = checked(&src.replace("@seed * @k", "@seed * q"), "M");
        assert!(!p.unwrap().reads(Predef::IsMand));
        assert!(d.iter().any(|m| m.contains("does not compile")), "{d:?}");
    }
}
