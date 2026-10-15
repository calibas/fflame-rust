//! Formulas written in Rhai (plan, phase 4): the GPU subset of a Rhai
//! script lowered to the formula language's parse tree, which the
//! checker types and the rest of the pipeline takes as it takes UF
//! text -- the interpreter, the big-float reference, the WGSL back
//! ends, the derivative and the derived perturbation equations.
//!
//! A formula is three functions and its parameters' metadata:
//!
//! ```rhai
//! const PARAMS = #{
//!     bailout: #{ value: 4.0, min: 1.0, caption: "Bailout" },
//! };
//!
//! fn start(pixel, p) {
//!     #{ z: complex(0.0, 0.0), c: pixel }
//! }
//!
//! fn step(s, p) {
//!     s.z = s.z * s.z + s.c;
//!     s
//! }
//!
//! fn escaped(s, p) {
//!     s.z.norm_sqr() > p.bailout
//! }
//! ```
//!
//! - **The state** is the map `start` returns (its fields the per-pixel
//!   state, `z` the iterate), built as a map literal or a variable whose
//!   fields are set; `step` returns it with the same fields, as the same
//!   variable or a new map (every value read before any is set). Locals
//!   (`let`) are temporaries.
//! - **`p`**, the parameters, declared in `PARAMS`: a value alone, or a
//!   map with `value` and any of `min`, `max`, `caption`, `hint`,
//!   `choices` (an int's names), `function` (a user function's default,
//!   called as `p.f(z)`).
//! - `start` may take the plane as a third argument (`plane.julia`,
//!   `plane.c`, the Julia constant); `step` the pass as a third (0 on
//!   the first), with `INFO.cycle` (`#{ prefix, period }`) when deep
//!   zoom should follow it.
//! - `INFO`: `title`, `center` (a complex), `magn`, `maxiter`, `angle`,
//!   `cycle`.
//! - Other functions are helpers, inlined where they are called; they
//!   read only their arguments, as Rhai's functions do.
//! - `if` is a value as well as a statement (`let d = if c { a } else
//!   { b };`), in `start`, `step` and helpers. `escaped` is one test:
//!   its `let`s and its helpers' are substituted, so there an `if` value
//!   is refused.
//!
//! Values are int, float, bool and complex (`complex(re, im)`; Rhai has
//! no complex literal); a bool and a number do not mix. Operators:
//! `+ - * / %`, `**` the power (`^` is Rhai's XOR), comparisons,
//! `&& || !`. Two ints divided is refused -- Rhai would truncate where a
//! formula wants the quotient. Functions (and as methods, `z.sqr()`):
//! `sqr sqrt exp ln sin cos tan cot sinh cosh tanh coth asin acos atan
//! asinh acosh atanh conj flip recip round floor ceiling trunc ident
//! zero cosxx isinf isnan`; `abs` (of a complex, its modulus), `fold`
//! (each part's absolute value: the Burning Ship), `norm_sqr` (`|z|^2`),
//! `arg` (the angle), `re` and `im` (or `.re`, `.im`), `log` (base 10,
//! as in Rhai), `pow(a, b)`, `atan2(y, x)`. Loops: `while`, `do ...
//! while/until`, `for i in a..b`; no `break`. What does not compile is
//! refused with the script's own positions.

use super::ast::{self, BinOp, Entry, ExprKind, FuncBlock, ParamBlock, Section, SectionKind, Setting, SettingValue, Settings, TypeName, UnOp};
use super::builtins::Builtin;
use super::ir::{Cycle, Program, Ty};
use super::{Diag, EntryKind, Span};
use ::rhai::{ASTFlags, Engine, Expr, FlowControl, FnCallExpr, OptimizationLevel, Position, ScriptFuncDef, Stmt, StmtBlock};
use std::collections::HashMap;

type R<T> = Result<T, Diag>;

/// The program a Rhai formula compiles to.
pub fn program(source: &str) -> Result<Program, Vec<Diag>> {
    let lowered = lower(source)?;
    let (program, diags) = super::check::check_rhai(&lowered.entry, EntryKind::Formula);
    let diags: Vec<Diag> = diags.into_iter().map(|d| lowered.rename(d)).collect();
    match program {
        Some(mut p) if !super::has_errors(&diags) => {
            if p.reads(super::ir::Predef::Pass) {
                p.cycle = lowered.cycle;
            }
            Ok(p)
        }
        _ => Err(diags),
    }
}

/// The title a Rhai formula gives itself (`INFO.title`), for the panel.
pub fn title(source: &str) -> Option<String> {
    lower(source).ok().map(|l| l.entry.name)
}

/// A formula lowered to the parse tree, with what the checker cannot
/// take from it.
pub struct Lowered {
    pub entry: Entry,
    pub cycle: Option<Cycle>,
    /// Lowered names to the script's, for messages.
    names: Vec<(String, String)>,
}

impl Lowered {
    /// A checker message, in the script's names.
    fn rename(&self, mut d: Diag) -> Diag {
        for (lowered, written) in &self.names {
            d.message = d.message.replace(&format!("`{lowered}`"), &format!("`{written}`"));
        }
        d
    }
}

/// Lower a Rhai formula to the parse tree.
pub fn lower(source: &str) -> Result<Lowered, Vec<Diag>> {
    let mut engine = Engine::new_raw();
    engine.set_optimization_level(OptimizationLevel::None);
    let pos = Positions::new(source);
    let ast = engine.compile(source).map_err(|e| vec![Diag::error(pos.span(e.position()), format!("{}", e.err_type()))])?;
    let mut l = Lowerer {
        pos,
        source,
        fns: HashMap::new(),
        depth: 0,
        pending: Vec::new(),
        pure: false,
        types: HashMap::new(),
        param_tys: HashMap::new(),
        names: Vec::new(),
        fresh: 0,
    };
    let mut out = l.run(&ast).map_err(|d| vec![d])?;
    out.names = l.names;
    Ok(out)
}

// ---- positions --------------------------------------------------------

struct Positions {
    line_starts: Vec<usize>,
    len: usize,
    text: Vec<u8>,
}

impl Positions {
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Positions { line_starts, len: source.len(), text: source.as_bytes().to_vec() }
    }

    /// A Rhai position (1-based line and column) as a byte span: the
    /// token there, a word or a single character.
    fn span(&self, p: Position) -> Span {
        let (Some(line), Some(col)) = (p.line(), p.position()) else { return Span::default() };
        let start = self.line_starts.get(line.saturating_sub(1)).copied().unwrap_or(0) + col.saturating_sub(1);
        let start = start.min(self.len);
        let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'.';
        let mut end = start;
        while end < self.len && word(self.text[end]) {
            end += 1;
        }
        Span { start, end: end.max((start + 1).min(self.len)) }
    }
}

// ---- lowering ---------------------------------------------------------

/// What a Rhai name stands for in a function being lowered.
#[derive(Clone)]
enum Binding {
    /// The state map (`s` in `step`, a map built in `start`).
    State,
    /// The parameters (`p`).
    Params,
    /// `start`'s pixel.
    Pixel,
    /// `start`'s plane.
    Plane,
    /// `step`'s pass.
    Pass,
    /// A local, by its lowered name.
    Local(String),
    /// A value substituted where read: a helper's argument, a `let` in
    /// `escaped`.
    Value(ast::Expr),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fn3 {
    Start,
    Step,
    Escaped,
    Helper,
}

struct Lowerer<'s> {
    pos: Positions,
    source: &'s str,
    fns: HashMap<String, ScriptFuncDef>,
    /// Helpers inlined within one another, against recursion.
    depth: usize,
    /// Statements an expression needs before the statement holding it
    /// (an `if` value, a helper's body).
    pending: Vec<ast::Stmt>,
    /// In `escaped`: one test, nothing pending.
    pure: bool,
    /// Lowered variable names (fields and locals) to their types.
    types: HashMap<String, TypeName>,
    param_tys: HashMap<String, TypeName>,
    names: Vec<(String, String)>,
    fresh: usize,
}

/// One function's scope: Rhai names to what they stand for, innermost
/// block last.
struct Scope {
    blocks: Vec<HashMap<String, Binding>>,
    which: Fn3,
    /// The state's fields so far (`start` adds them).
    fields: Vec<String>,
}

impl Scope {
    fn new(which: Fn3, fields: Vec<String>) -> Self {
        Scope { blocks: vec![HashMap::new()], which, fields }
    }

    fn get(&self, name: &str) -> Option<&Binding> {
        self.blocks.iter().rev().find_map(|b| b.get(name))
    }

    fn bind(&mut self, name: &str, b: Binding) {
        self.blocks.last_mut().expect("a block").insert(name.to_string(), b);
    }
}

/// The value a block's last statement gives: an expression, a call
/// (rhai keeps a call standing alone as its own statement), `return` of
/// either, or an `if` whose arms end in values.
#[derive(Clone, Copy)]
enum Final<'a> {
    Expr(&'a Expr),
    Call(&'a FnCallExpr, Position),
    If(&'a FlowControl, Position),
}

fn final_value(st: &Stmt) -> Option<Final<'_>> {
    match st {
        Stmt::Expr(e) => match &**e {
            // `if` as a value, written as the block's last expression.
            Expr::Stmt(b) => match b.statements() {
                [Stmt::If(fc, p)] => Some(Final::If(fc, *p)),
                _ => Some(Final::Expr(e)),
            },
            _ => Some(Final::Expr(e)),
        },
        Stmt::FnCall(f, p) => Some(Final::Call(f, *p)),
        Stmt::Return(Some(e), ..) => Some(Final::Expr(e)),
        Stmt::If(fc, p) if !fc.branch.is_empty() => Some(Final::If(fc, *p)),
        _ => None,
    }
}

fn mk(kind: ExprKind, span: Span) -> ast::Expr {
    ast::Expr { kind, span }
}

fn assign(target: ast::Expr, value: ast::Expr, span: Span) -> ast::Stmt {
    ast::Stmt::Expr(mk(ExprKind::Assign(Box::new(target), Box::new(value)), span))
}

fn field_name(f: &str) -> String {
    format!("s_{f}")
}

impl<'s> Lowerer<'s> {
    fn err<T>(&self, p: Position, msg: impl Into<String>) -> R<T> {
        Err(Diag::error(self.pos.span(p), msg))
    }

    fn run(&mut self, ast: &::rhai::AST) -> R<Lowered> {
        for f in ast.iter_fn_def() {
            self.fns.insert(f.name.to_string(), (**f).clone());
        }
        // The top level: PARAMS, INFO, nothing else.
        let mut params: Vec<(String, Expr, Position)> = Vec::new();
        let mut info: Vec<(String, Expr, Position)> = Vec::new();
        for st in ast.statements() {
            match st {
                Stmt::Var(x, _, pos) => {
                    let (ident, value, _) = &**x;
                    let list = match ident.name.as_str() {
                        "PARAMS" => &mut params,
                        "INFO" => &mut info,
                        other => return self.err(*pos, format!("a formula's top level holds `PARAMS` and `INFO`, not `{other}`")),
                    };
                    let Expr::Map(m, _) = value else {
                        return self.err(*pos, format!("`{}` is a map, `#{{ ... }}`", ident.name));
                    };
                    for (k, v) in &m.0 {
                        list.push((k.name.to_string(), v.clone(), k.pos));
                    }
                }
                Stmt::Noop(_) => {}
                other => return self.err(other.position(), "a formula's top level holds `PARAMS`, `INFO` and functions"),
            }
        }
        let mut default = Settings::default();
        for (name, v, p) in &params {
            self.param(name, v, *p, &mut default)?;
        }
        let mut title = "Formula".to_string();
        let mut cycle = None;
        for (name, v, p) in &info {
            match name.as_str() {
                "title" => match v {
                    Expr::StringConstant(s, _) => title = s.to_string(),
                    _ => return self.err(*p, "`title` is a string"),
                },
                "center" | "magn" | "maxiter" | "angle" => {
                    let value = self.literal(v)?;
                    default.items.push(Setting { name: name.clone(), value: SettingValue::Expr(value), span: self.pos.span(*p) });
                }
                "cycle" => {
                    let Expr::Map(m, _) = v else { return self.err(*p, "`cycle` is `#{ prefix, period }`") };
                    let mut c = Cycle { prefix: 0, period: 0 };
                    for (k, x) in &m.0 {
                        let Expr::IntegerConstant(n, _) = x else { return self.err(k.pos, "an int") };
                        match k.name.as_str() {
                            "prefix" => c.prefix = (*n).max(0) as u32,
                            "period" => c.period = (*n).max(0) as u32,
                            o => return self.err(k.pos, format!("`cycle` has `prefix` and `period`, not `{o}`")),
                        }
                    }
                    if c.period == 0 {
                        return self.err(*p, "a cycle's period is at least 1");
                    }
                    cycle = Some(c);
                }
                o => return self.err(*p, format!("`INFO` has `title`, `center`, `magn`, `maxiter`, `angle` and `cycle`, not `{o}`")),
            }
        }
        let get = |name: &str| self.fns.get(name).cloned();
        let (Some(start), Some(step), Some(escaped)) = (get("start"), get("step"), get("escaped")) else {
            let missing: Vec<&str> = ["start", "step", "escaped"].into_iter().filter(|n| !self.fns.contains_key(*n)).collect();
            return Err(Diag::error(Span::default(), format!("a formula needs `fn {}`", missing.join("`, `fn "))));
        };
        let span = Span { start: 0, end: self.source.len() };
        // start: its map's fields become the state.
        let mut scope = Scope::new(Fn3::Start, Vec::new());
        if start.params.len() > 3 || start.params.len() < 2 {
            return self.err(start.body.position(), "`start(pixel, p)` or `start(pixel, p, plane)`");
        }
        for (name, role) in start.params.iter().zip([Binding::Pixel, Binding::Params, Binding::Plane]) {
            scope.bind(name, role);
        }
        let mut init = Vec::new();
        self.state_body(&start.body, &mut scope, &mut init)?;
        let fields = scope.fields.clone();
        if !fields.iter().any(|f| f == "z") {
            return self.err(start.body.position(), "the state needs `z`, the iterate");
        }
        // step: the same fields.
        let mut scope = Scope::new(Fn3::Step, fields.clone());
        if step.params.len() > 3 || step.params.len() < 2 {
            return self.err(step.body.position(), "`step(s, p)` or `step(s, p, pass)`");
        }
        for (name, role) in step.params.iter().zip([Binding::State, Binding::Params, Binding::Pass]) {
            scope.bind(name, role);
        }
        let mut lp = Vec::new();
        self.state_body(&step.body, &mut scope, &mut lp)?;
        // escaped: one test, its lets substituted.
        let mut scope = Scope::new(Fn3::Escaped, fields);
        if escaped.params.len() != 2 {
            return self.err(escaped.body.position(), "`escaped(s, p)`");
        }
        for (name, role) in escaped.params.iter().zip([Binding::State, Binding::Params]) {
            scope.bind(name, role);
        }
        self.pure = true;
        let test = self.pure_body(&escaped.body, &mut scope)?;
        self.pure = false;
        let tspan = test.span;
        let go = mk(ExprKind::Unary(UnOp::Not, Box::new(test)), tspan);
        let sections = vec![
            Section { kind: SectionKind::Init, span, body: init, last_is_bailout: false, labelled: true },
            Section { kind: SectionKind::Loop, span, body: lp, last_is_bailout: false, labelled: true },
            Section { kind: SectionKind::Bailout, span, body: vec![ast::Stmt::Expr(go)], last_is_bailout: false, labelled: true },
        ];
        default.items.push(Setting { name: "title".into(), value: SettingValue::Strings(vec![title.clone()]), span });
        let entry = Entry { name: title, setting: None, span, body: span, sections, default, switch: Settings::default() };
        Ok(Lowered { entry, cycle, names: Vec::new() })
    }

    // ---- parameters ---------------------------------------------------

    /// One `PARAMS` entry as a parameter (or user function) block.
    fn param(&mut self, name: &str, v: &Expr, p: Position, out: &mut Settings) -> R<()> {
        let span = self.pos.span(p);
        let mut value: Option<&Expr> = None;
        let mut settings = Vec::new();
        let mut function: Option<String> = None;
        let mut choices: Vec<String> = Vec::new();
        match v {
            Expr::Map(m, _) => {
                for (k, x) in &m.0 {
                    let kspan = self.pos.span(k.pos);
                    match k.name.as_str() {
                        "value" => value = Some(x),
                        "min" | "max" => settings.push(Setting { name: k.name.to_string(), value: SettingValue::Expr(self.literal(x)?), span: kspan }),
                        "caption" | "hint" => match x {
                            Expr::StringConstant(s, _) => settings.push(Setting { name: k.name.to_string(), value: SettingValue::Strings(vec![s.to_string()]), span: kspan }),
                            _ => return self.err(k.pos, format!("`{}` is a string", k.name)),
                        },
                        "choices" => match x {
                            Expr::Array(items, _) => {
                                for it in items.iter() {
                                    match it {
                                        Expr::StringConstant(s, _) => choices.push(s.to_string()),
                                        _ => return self.err(it.position(), "a choice is a string"),
                                    }
                                }
                            }
                            _ => return self.err(k.pos, "`choices` is an array of strings"),
                        },
                        "function" => match x {
                            Expr::StringConstant(s, _) => function = Some(s.to_string()),
                            _ => return self.err(k.pos, "`function` names one, `\"sin\"`"),
                        },
                        o => return self.err(k.pos, format!("a parameter has `value`, `min`, `max`, `caption`, `hint`, `choices`, `function`; not `{o}`")),
                    }
                }
            }
            other => value = Some(other),
        }
        if let Some(f) = function {
            let uf = rename_function(&f).unwrap_or(f.as_str()).to_string();
            settings.push(Setting { name: "default".into(), value: SettingValue::Func(uf), span });
            out.funcs.push(FuncBlock { ty: None, name: name.to_string(), settings, span });
            return Ok(());
        }
        let Some(value) = value else { return self.err(p, "a parameter needs its `value`") };
        let lit = self.literal(value)?;
        let ty = match &lit.kind {
            ExprKind::Int(_) => TypeName::Int,
            ExprKind::Float(_) => TypeName::Float,
            ExprKind::Bool(_) => TypeName::Bool,
            _ => TypeName::Complex,
        };
        if !choices.is_empty() {
            if ty != TypeName::Int {
                return self.err(p, "`choices` names an int's values");
            }
            settings.push(Setting { name: "enum".into(), value: SettingValue::Strings(choices), span });
        }
        settings.push(Setting { name: "default".into(), value: SettingValue::Expr(lit), span });
        self.param_tys.insert(name.to_string(), ty);
        out.params.push(ParamBlock { ty: Some(ty), name: name.to_string(), settings, span });
        Ok(())
    }

    /// A constant: a number, a bool, `complex(a, b)` of numbers.
    fn literal(&self, e: &Expr) -> R<ast::Expr> {
        let span = self.pos.span(e.position());
        Ok(match e {
            Expr::IntegerConstant(v, _) => mk(ExprKind::Int(*v), span),
            Expr::FloatConstant(v, _) => mk(ExprKind::Float(**v), span),
            Expr::BoolConstant(v, _) => mk(ExprKind::Bool(*v), span),
            Expr::FnCall(f, _) if f.name == "-" && f.args.len() == 1 => {
                let inner = self.literal(&f.args[0])?;
                match inner.kind {
                    ExprKind::Int(v) => mk(ExprKind::Int(-v), span),
                    ExprKind::Float(v) => mk(ExprKind::Float(-v), span),
                    _ => return self.err(e.position(), "a constant here"),
                }
            }
            Expr::FnCall(f, _) if f.name == "complex" && f.args.len() == 2 => {
                let (a, b) = (self.literal(&f.args[0])?, self.literal(&f.args[1])?);
                mk(ExprKind::Complex(Box::new(a), Box::new(b)), span)
            }
            _ => return self.err(e.position(), "a constant here: a number, a bool, or `complex(re, im)` of numbers"),
        })
    }

    // ---- statements ---------------------------------------------------

    fn local(&mut self, written: &str, ty: TypeName) -> String {
        self.fresh += 1;
        let lowered = format!("l{}_{written}", self.fresh);
        self.types.insert(lowered.clone(), ty);
        self.names.push((lowered.clone(), written.to_string()));
        lowered
    }

    /// Statements an expression left pending, before what holds it.
    fn flush(&mut self, out: &mut Vec<ast::Stmt>) {
        out.append(&mut self.pending);
    }

    /// `start`'s or `step`'s body: statements, then the state.
    fn state_body(&mut self, b: &StmtBlock, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let stmts = b.statements();
        let Some((last, rest)) = stmts.split_last() else {
            return self.err(b.position(), "the function ends with the state: `s`, or a map `#{ z: ... }`");
        };
        for st in rest {
            self.stmt(st, scope, out)?;
        }
        match final_value(last) {
            Some(Final::Expr(value)) => self.state_value(value, scope, out),
            Some(Final::If(fc, p)) => self.state_if(fc, p, scope, out),
            _ => self.err(last.position(), "the function ends with the state: `s`, or a map `#{ z: ... }`"),
        }
    }

    /// An `if` that ends `start` or `step`, each arm ending with the
    /// state; in `start` every arm gives the same fields.
    fn state_if(&mut self, fc: &FlowControl, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let span = self.pos.span(p);
        let mut arms = Vec::new();
        let mut keys: Vec<Vec<String>> = Vec::new();
        let mut cur = fc;
        let otherwise;
        loop {
            let cond = self.expr(&cur.expr, scope)?;
            if !arms.is_empty() && !self.pending.is_empty() {
                return self.err(cur.expr.position(), "an `else if` condition holds no `if` value or helper with statements");
            }
            self.flush(out);
            let body = self.state_arm(&cur.body, scope, &mut keys)?;
            arms.push((cond, body));
            match cur.branch.statements() {
                [Stmt::If(inner, _)] => cur = &**inner,
                [] => return self.err(p, "the `if` that ends the function needs its `else`"),
                _ => {
                    otherwise = Some(self.state_arm(&cur.branch, scope, &mut keys)?);
                    break;
                }
            }
        }
        if scope.which == Fn3::Start {
            for k in keys.iter_mut() {
                k.sort();
            }
            if keys.windows(2).any(|w| w[0] != w[1]) {
                return self.err(p, "each arm's state has the same fields");
            }
        }
        out.push(ast::Stmt::If { arms, otherwise, span });
        Ok(())
    }

    /// One arm of [`Self::state_if`], in its own scope; the fields its
    /// map names are pushed to `keys` (none for the state's variable).
    fn state_arm(&mut self, b: &StmtBlock, scope: &mut Scope, keys: &mut Vec<Vec<String>>) -> R<Vec<ast::Stmt>> {
        let outer = std::mem::take(&mut self.pending);
        scope.blocks.push(HashMap::new());
        let mut body = Vec::new();
        let r = self.state_body(b, scope, &mut body);
        scope.blocks.pop();
        self.pending = outer;
        r?;
        let named = match b.statements().last().and_then(final_value) {
            Some(Final::Expr(Expr::Map(m, _))) => m.0.iter().map(|(k, _)| k.name.to_string()).collect(),
            _ => scope.fields.clone(),
        };
        keys.push(named);
        Ok(body)
    }

    /// The value a `start` or `step` returns.
    fn state_value(&mut self, e: &Expr, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        match e {
            Expr::Variable(v, _, p) => match scope.get(v.1.as_str()) {
                Some(Binding::State) => Ok(()),
                _ => self.err(*p, "the function ends with the state"),
            },
            Expr::Map(m, _) => {
                // Every value first, then the fields: a map's entries
                // read the state before any of them is set.
                let mut values = Vec::new();
                for (k, x) in &m.0 {
                    let v = self.expr(x, scope)?;
                    values.push((k.name.to_string(), k.pos, v));
                }
                self.flush(out);
                let mut temps = Vec::new();
                for (f, p, v) in values {
                    let ty = self.ty_of(&v);
                    if scope.which == Fn3::Start {
                        self.declare_field(&f, ty, v, p, scope, out);
                    } else {
                        if !scope.fields.contains(&f) {
                            return self.err(p, format!("`{f}` is not in the state `start` returns"));
                        }
                        let t = self.local(&format!("{f}_new"), ty);
                        let span = v.span;
                        out.push(ast::Stmt::Decl { ty, name: t.clone(), init: Some(v), span });
                        temps.push((f, t, span));
                    }
                }
                for (f, t, span) in temps {
                    out.push(assign(self.field_target(&f, span), mk(ExprKind::Name(t), span), span));
                }
                Ok(())
            }
            other => self.err(other.position(), "the function ends with the state: `s`, or a map `#{ z: ... }`"),
        }
    }

    /// A field of the state in `start`: `z` the iterate, any other a
    /// variable.
    fn declare_field(&mut self, f: &str, ty: TypeName, v: ast::Expr, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) {
        let span = self.pos.span(p);
        if scope.fields.iter().any(|x| x == f) {
            out.push(assign(self.field_target(f, span), v, span));
            return;
        }
        scope.fields.push(f.to_string());
        if f == "z" {
            out.push(assign(mk(ExprKind::Predef("z".into()), span), v, span));
        } else {
            let name = field_name(f);
            self.types.insert(name.clone(), ty);
            self.names.push((name.clone(), format!("s.{f}")));
            out.push(ast::Stmt::Decl { ty, name, init: Some(v), span });
        }
    }

    fn field_target(&self, f: &str, span: Span) -> ast::Expr {
        if f == "z" {
            mk(ExprKind::Predef("z".into()), span)
        } else {
            mk(ExprKind::Name(field_name(f)), span)
        }
    }

    fn stmt(&mut self, st: &Stmt, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        match st {
            Stmt::Noop(_) => Ok(()),
            Stmt::Var(x, _, p) => {
                let (ident, value, _) = &**x;
                let span = self.pos.span(*p);
                // `let s = #{ ... }` in `start`: the state, built.
                if scope.which == Fn3::Start {
                    if let Expr::Map(..) = value {
                        scope.bind(&ident.name, Binding::State);
                        return self.state_value(value, scope, out);
                    }
                }
                let v = self.expr(value, scope)?;
                self.flush(out);
                let ty = self.ty_of(&v);
                let name = self.local(&ident.name, ty);
                out.push(ast::Stmt::Decl { ty, name: name.clone(), init: Some(v), span });
                scope.bind(&ident.name, Binding::Local(name));
                Ok(())
            }
            Stmt::Assignment(x) => {
                let (op, be) = &**x;
                if let Some((f, fp)) = self.new_field(&be.lhs, scope) {
                    if op.get_op_assignment_info().is_some() {
                        return self.err(fp, format!("`{f}` is not in the state yet: set it with `=`"));
                    }
                    let v = self.expr(&be.rhs, scope)?;
                    self.flush(out);
                    let ty = self.ty_of(&v);
                    self.declare_field(&f, ty, v, fp, scope, out);
                    return Ok(());
                }
                let target = self.target(&be.lhs, scope)?;
                let span = target.span;
                let mut v = self.expr(&be.rhs, scope)?;
                if let Some((.., syntax)) = op.get_op_assignment_info() {
                    let bop = binop(syntax).ok_or_else(|| Diag::error(span, format!("`{syntax}=` is not a formula's")))?;
                    v = self.binary(bop, target.clone(), v, op.position())?;
                }
                self.flush(out);
                out.push(assign(target, v, span));
                Ok(())
            }
            Stmt::If(fc, p) => {
                let span = self.pos.span(*p);
                let mut arms = Vec::new();
                let mut fc = &**fc;
                let otherwise;
                loop {
                    let cond = self.expr(&fc.expr, scope)?;
                    if !self.pending.is_empty() && !arms.is_empty() {
                        return self.err(fc.expr.position(), "an `else if` condition holds no `if` value or helper with statements");
                    }
                    self.flush(out);
                    let body = self.block(&fc.body, scope)?;
                    arms.push((cond, body));
                    // `else if`: one nested `if` alone in the branch.
                    match fc.branch.statements() {
                        [Stmt::If(inner, _)] => fc = &**inner,
                        [] => {
                            otherwise = None;
                            break;
                        }
                        _ => {
                            otherwise = Some(self.block(&fc.branch, scope)?);
                            break;
                        }
                    }
                }
                out.push(ast::Stmt::If { arms, otherwise, span });
                Ok(())
            }
            Stmt::While(fc, p) => {
                if matches!(fc.expr, Expr::Unit(_)) {
                    return self.err(*p, "`loop` needs `break`, which a formula has not: write `while`");
                }
                let cond = self.expr(&fc.expr, scope)?;
                // What the condition needs runs before each test: before
                // the loop and at the end of each pass.
                let pre = std::mem::take(&mut self.pending);
                out.extend(pre.iter().cloned());
                let mut body = self.block(&fc.body, scope)?;
                body.extend(pre);
                out.push(ast::Stmt::While { cond, body, span: self.pos.span(*p) });
                Ok(())
            }
            Stmt::Do(fc, flags, p) => {
                let mut body = self.block(&fc.body, scope)?;
                let c = self.expr(&fc.expr, scope)?;
                self.flush(&mut body);
                let span = c.span;
                // UF's repeat runs until its condition: `until` as is,
                // `while` negated.
                let cond = if flags.intersects(ASTFlags::NEGATED) { c } else { mk(ExprKind::Unary(UnOp::Not, Box::new(c)), span) };
                out.push(ast::Stmt::Repeat { body, cond, span: self.pos.span(*p) });
                Ok(())
            }
            Stmt::For(x, p) => {
                let (var, counter, fc) = &**x;
                if counter.is_some() {
                    return self.err(*p, "`for (x, i) in ...`: a formula's `for` takes one variable");
                }
                let range = match &fc.expr {
                    Expr::FnCall(range, _) if matches!(range.name.as_str(), ".." | "..=") && range.args.len() == 2 => range,
                    _ => return self.err(fc.expr.position(), "a formula's `for` runs over a range, `a..b`"),
                };
                let inclusive = range.name.as_str() == "..=";
                let span = self.pos.span(*p);
                let from = self.expr(&range.args[0], scope)?;
                let to = self.expr(&range.args[1], scope)?;
                self.flush(out);
                let name = self.local(&var.name, TypeName::Int);
                out.push(ast::Stmt::Decl { ty: TypeName::Int, name: name.clone(), init: Some(from), span });
                // The bound is read once, as Rhai reads its range.
                let bound = self.local(&format!("{}_end", var.name), TypeName::Int);
                out.push(ast::Stmt::Decl { ty: TypeName::Int, name: bound.clone(), init: Some(to), span });
                scope.blocks.push(HashMap::new());
                scope.bind(&var.name, Binding::Local(name.clone()));
                let body = self.block(&fc.body, scope);
                scope.blocks.pop();
                let mut body = body?;
                let i = || mk(ExprKind::Name(name.clone()), span);
                body.push(assign(i(), mk(ExprKind::Binary(BinOp::Add, Box::new(i()), Box::new(mk(ExprKind::Int(1), span))), span), span));
                let op = if inclusive { BinOp::Le } else { BinOp::Lt };
                out.push(ast::Stmt::While { cond: mk(ExprKind::Binary(op, Box::new(i()), Box::new(mk(ExprKind::Name(bound), span))), span), body, span });
                Ok(())
            }
            Stmt::Block(b) => {
                let body = self.block(b, scope)?;
                out.extend(body);
                Ok(())
            }
            Stmt::BreakLoop(_, _, p) => self.err(*p, "a formula's loops have no `break` or `continue`"),
            Stmt::Return(_, _, p) => self.err(*p, "`return` ends a function, as its last statement"),
            Stmt::Expr(e) => self.err(e.position(), "a value alone does nothing here"),
            Stmt::FnCall(_, p) => self.err(*p, "a call alone does nothing here"),
            other => self.err(other.position(), "not in a formula"),
        }
    }

    /// A block of statements, its own scope; its pending statements its
    /// own.
    fn block(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<Vec<ast::Stmt>> {
        let outer = std::mem::take(&mut self.pending);
        scope.blocks.push(HashMap::new());
        let mut out = Vec::new();
        let r = b.statements().iter().try_for_each(|st| self.stmt(st, scope, &mut out));
        scope.blocks.pop();
        self.pending = outer;
        r.map(|_| out)
    }

    /// An `if` as a value: a local set in each arm, its statements left
    /// pending.
    fn if_value(&mut self, fc: &FlowControl, p: Position, scope: &mut Scope) -> R<ast::Expr> {
        if self.pure {
            return self.err(p, "`escaped` is one test: write the `if` as a comparison, or move it to `step`");
        }
        let span = self.pos.span(p);
        // Each arm: (condition, statements, value), in its own scope.
        let mut arms: Vec<(Option<ast::Expr>, Vec<ast::Stmt>, ast::Expr)> = Vec::new();
        let mut cur = fc;
        let mut conds_pending = Vec::new();
        loop {
            let cond = self.expr(&cur.expr, scope)?;
            if !arms.is_empty() && !self.pending.is_empty() {
                return self.err(cur.expr.position(), "an `else if` condition holds no `if` value or helper with statements");
            }
            conds_pending.append(&mut self.pending);
            let (body, value) = self.value_block(&cur.body, scope)?;
            arms.push((Some(cond), body, value));
            match cur.branch.statements() {
                [Stmt::If(inner, _)] => cur = &**inner,
                [] => return self.err(p, "an `if` used as a value needs its `else`"),
                _ => {
                    let (body, value) = self.value_block(&cur.branch, scope)?;
                    arms.push((None, body, value));
                    break;
                }
            }
        }
        let ty = arms.iter().map(|(_, _, v)| self.ty_of(v)).reduce(join).unwrap_or(TypeName::Float);
        let name = self.local("if", ty);
        self.pending.extend(conds_pending);
        self.pending.push(ast::Stmt::Decl { ty, name: name.clone(), init: None, span });
        let mut lowered = Vec::new();
        let mut otherwise = None;
        for (cond, mut body, value) in arms {
            body.push(assign(mk(ExprKind::Name(name.clone()), span), value, span));
            match cond {
                Some(c) => lowered.push((c, body)),
                None => otherwise = Some(body),
            }
        }
        self.pending.push(ast::Stmt::If { arms: lowered, otherwise, span });
        Ok(mk(ExprKind::Name(name), span))
    }

    /// A block that ends in a value: its statements (with what they left
    /// pending) and the value.
    fn value_block(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<(Vec<ast::Stmt>, ast::Expr)> {
        let outer = std::mem::take(&mut self.pending);
        scope.blocks.push(HashMap::new());
        let r = (|| {
            let stmts = b.statements();
            let Some((last, rest)) = stmts.split_last() else { return self.err(b.position(), "each arm ends with its value") };
            let mut out = Vec::new();
            for st in rest {
                self.stmt(st, scope, &mut out)?;
            }
            let Some(f) = final_value(last) else { return self.err(last.position(), "each arm ends with its value") };
            let v = self.final_expr(f, scope)?;
            self.flush(&mut out);
            Ok((out, v))
        })();
        scope.blocks.pop();
        self.pending = outer;
        r
    }

    /// What an assignment writes: a local, or a field of the state.
    fn target(&mut self, e: &Expr, scope: &mut Scope) -> R<ast::Expr> {
        let span = self.pos.span(e.position());
        match e {
            Expr::Variable(v, _, p) => match scope.get(v.1.as_str()) {
                Some(Binding::Local(n)) => Ok(mk(ExprKind::Name(n.clone()), span)),
                Some(_) => self.err(*p, format!("`{}` is not a variable to set", v.1)),
                None => self.err(*p, format!("`{}` is not declared: `let {} = ...`", v.1, v.1)),
            },
            Expr::Dot(be, _, _) => {
                let Expr::Variable(v, _, _) = &be.lhs else { return self.err(e.position(), "set a field of the state, `s.z = ...`") };
                let Some(Binding::State) = scope.get(v.1.as_str()) else {
                    return self.err(e.position(), "set a field of the state, `s.z = ...`");
                };
                let Expr::Property(prop, pp) = &be.rhs else { return self.err(e.position(), "set a field of the state, `s.z = ...`") };
                let f = prop.2.to_string();
                if !scope.fields.contains(&f) {
                    return self.err(*pp, format!("`{f}` is not in the state `start` returns"));
                }
                Ok(self.field_target(&f, self.pos.span(*pp)))
            }
            _ => self.err(e.position(), "set a variable or a field of the state"),
        }
    }

    /// In `start`, `s.f` for a field the state has not got yet.
    fn new_field(&self, e: &Expr, scope: &Scope) -> Option<(String, Position)> {
        if scope.which != Fn3::Start {
            return None;
        }
        let Expr::Dot(be, _, _) = e else { return None };
        let Expr::Variable(v, _, _) = &be.lhs else { return None };
        let Some(Binding::State) = scope.get(v.1.as_str()) else { return None };
        let Expr::Property(prop, pp) = &be.rhs else { return None };
        let f = prop.2.to_string();
        (!scope.fields.contains(&f)).then_some((f, *pp))
    }

    /// `escaped`'s body, or a helper's inside it: `let`s substituted,
    /// then a value.
    fn pure_body(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<ast::Expr> {
        let stmts = b.statements();
        let Some((last, lets)) = stmts.split_last() else { return self.err(b.position(), "the function ends with its value") };
        scope.blocks.push(HashMap::new());
        let r = (|| {
            for st in lets {
                match st {
                    Stmt::Var(x, _, _) => {
                        let (ident, value, _) = &**x;
                        let v = self.expr(value, scope)?;
                        scope.bind(&ident.name, Binding::Value(v));
                    }
                    Stmt::Noop(_) => {}
                    other => return self.err(other.position(), "`escaped` is one test: here only `let`s, then the value"),
                }
            }
            let Some(v) = final_value(last) else { return self.err(last.position(), "the function ends with its value") };
            self.final_expr(v, scope)
        })();
        scope.blocks.pop();
        r
    }

    // ---- expressions --------------------------------------------------

    fn final_expr(&mut self, f: Final, scope: &mut Scope) -> R<ast::Expr> {
        match f {
            Final::Expr(e) => self.expr(e, scope),
            Final::Call(c, p) => self.call(c, None, p, scope),
            Final::If(fc, p) => self.if_value(fc, p, scope),
        }
    }

    fn expr(&mut self, e: &Expr, scope: &mut Scope) -> R<ast::Expr> {
        let span = self.pos.span(e.position());
        Ok(match e {
            Expr::IntegerConstant(v, _) => mk(ExprKind::Int(*v), span),
            Expr::FloatConstant(v, _) => mk(ExprKind::Float(**v), span),
            Expr::BoolConstant(v, _) => mk(ExprKind::Bool(*v), span),
            Expr::Variable(v, _, p) => match scope.get(v.1.as_str()).cloned() {
                Some(Binding::Local(n)) => mk(ExprKind::Name(n), span),
                Some(Binding::Value(x)) => x,
                Some(Binding::Pixel) => mk(ExprKind::Predef("pixel".into()), span),
                Some(Binding::Pass) => mk(ExprKind::Predef("pass".into()), span),
                Some(Binding::State) => return self.err(*p, "the state is read by its fields, `s.z`"),
                Some(Binding::Params) => return self.err(*p, "the parameters are read by name, `p.power`"),
                Some(Binding::Plane) => return self.err(*p, "the plane is read as `plane.julia` or `plane.c`"),
                None => return self.err(*p, format!("`{}` is not declared", v.1)),
            },
            Expr::Dot(be, _, _) => {
                let base = self.base(&be.lhs, scope)?;
                self.chain(base, &be.rhs, scope)?
            }
            Expr::And(xs, _) | Expr::Or(xs, _) => {
                let op = if matches!(e, Expr::And(..)) { BinOp::And } else { BinOp::Or };
                let mut it = xs.iter();
                let first = it.next().ok_or_else(|| Diag::error(span, "an operand"))?;
                let mut acc = self.expr(first, scope)?;
                for x in it {
                    let r = self.expr(x, scope)?;
                    acc = mk(ExprKind::Binary(op, Box::new(acc), Box::new(r)), span);
                }
                acc
            }
            Expr::FnCall(f, p) => self.call(f, None, *p, scope)?,
            Expr::Stmt(b) => match b.statements() {
                [Stmt::If(fc, p)] => self.if_value(fc, *p, scope)?,
                _ => return self.err(e.position(), "a block is not a value here"),
            },
            Expr::Map(..) => return self.err(e.position(), "a map is the state, at the end of `start` or `step`"),
            Expr::StringConstant(..) | Expr::InterpolatedString(..) | Expr::CharConstant(..) => {
                return self.err(e.position(), "a string is metadata, not a value")
            }
            Expr::Array(..) => return self.err(e.position(), "a formula has no arrays (yet)"),
            _ => return self.err(e.position(), "not in a formula"),
        })
    }

    /// The left of a `.`: a binding read by its fields, or a value.
    fn base(&mut self, e: &Expr, scope: &mut Scope) -> R<Base> {
        if let Expr::Variable(v, _, _) = e {
            match scope.get(v.1.as_str()) {
                Some(Binding::State) => return Ok(Base::State),
                Some(Binding::Params) => return Ok(Base::Params),
                Some(Binding::Plane) => return Ok(Base::Plane),
                _ => {}
            }
        }
        Ok(Base::Value(self.expr(e, scope)?))
    }

    /// `base.rhs`, where `rhs` is a property, a method call or a further
    /// chain.
    fn chain(&mut self, base: Base, rhs: &Expr, scope: &mut Scope) -> R<ast::Expr> {
        match rhs {
            Expr::Dot(be, _, _) => {
                let first = self.chain(base, &be.lhs, scope)?;
                self.chain(Base::Value(first), &be.rhs, scope)
            }
            Expr::Property(prop, p) => {
                let name = prop.2.as_str();
                let span = self.pos.span(*p);
                match base {
                    Base::State => {
                        if !scope.fields.iter().any(|f| f == name) {
                            let when = if scope.which == Fn3::Start { "yet" } else { "that `start` returns" };
                            return self.err(*p, format!("`{name}` is not in the state {when}"));
                        }
                        Ok(self.field_target(name, span))
                    }
                    Base::Params => {
                        if !self.param_tys.contains_key(name) {
                            return self.err(*p, format!("`{name}` is not in `PARAMS`"));
                        }
                        Ok(mk(ExprKind::Param(name.to_string()), span))
                    }
                    Base::Plane => match name {
                        // `ismand` is 1 on the parameter plane.
                        "julia" => Ok(mk(
                            ExprKind::Binary(BinOp::Eq, Box::new(mk(ExprKind::Name("ismand".into()), span)), Box::new(mk(ExprKind::Int(0), span))),
                            span,
                        )),
                        "c" => Ok(mk(ExprKind::Predef("juliac".into()), span)),
                        o => self.err(*p, format!("the plane has `julia` and `c`, not `{o}`")),
                    },
                    Base::Value(v) => match name {
                        "re" => Ok(mk(ExprKind::Call { name: "real".into(), args: vec![v], user: false }, span)),
                        "im" => Ok(mk(ExprKind::Call { name: "imag".into(), args: vec![v], user: false }, span)),
                        o => self.err(*p, format!("a value's parts are `.re` and `.im`, not `.{o}`")),
                    },
                }
            }
            Expr::MethodCall(f, p) => match base {
                Base::Params => {
                    // `p.f(z)`: a user function.
                    let name = f.name.to_string();
                    if f.args.len() != 1 {
                        return self.err(*p, format!("`p.{name}` takes one argument"));
                    }
                    let arg = self.expr(&f.args[0], scope)?;
                    Ok(mk(ExprKind::Call { name, args: vec![arg], user: true }, self.pos.span(*p)))
                }
                Base::Value(v) => self.call(f, Some(v), *p, scope),
                _ => self.err(*p, "call a function of a value, `s.z.sqr()`"),
            },
            other => self.err(other.position(), "not in a formula"),
        }
    }

    /// A function call, or an operator; `this` is a method call's value.
    fn call(&mut self, f: &FnCallExpr, this: Option<ast::Expr>, p: Position, scope: &mut Scope) -> R<ast::Expr> {
        let span = self.pos.span(p);
        let mut args = Vec::new();
        if let Some(t) = this {
            args.push(t);
        }
        let name = f.name.as_str();
        // Operators.
        if f.op_token.is_some() || is_operator(name) {
            for a in f.args.iter() {
                args.push(self.expr(a, scope)?);
            }
            return match (name, args.len()) {
                ("-", 1) => Ok(mk(ExprKind::Unary(UnOp::Neg, Box::new(args.remove(0))), span)),
                ("+", 1) => Ok(args.remove(0)),
                ("!", 1) => Ok(mk(ExprKind::Unary(UnOp::Not, Box::new(args.remove(0))), span)),
                (op, 2) => {
                    let bop = binop(op).ok_or_else(|| Diag::error(span, format!("`{op}` is not a formula's operator")))?;
                    let b = args.pop().unwrap();
                    let a = args.pop().unwrap();
                    self.binary(bop, a, b, p)
                }
                (op, _) => self.err(p, format!("`{op}` is not a formula's operator")),
            };
        }
        // A helper of the script's own, inlined; it reads only its
        // arguments.
        if let Some(def) = self.fns.get(name).cloned() {
            if !args.is_empty() {
                // Rhai binds a method call's value to `this`, not to the
                // first parameter.
                return self.err(p, format!("call a helper as a function, `{name}(x)`"));
            }
            if matches!(name, "start" | "step" | "escaped") {
                return self.err(p, format!("`{name}` is the formula's own, not a helper to call"));
            }
            for a in f.args.iter() {
                args.push(self.expr(a, scope)?);
            }
            if def.params.len() != args.len() {
                return self.err(p, format!("`{name}` takes {}", def.params.len()));
            }
            if self.depth > 32 {
                return self.err(p, "helpers call each other too deep (recursion?)");
            }
            let mut inner = Scope::new(Fn3::Helper, Vec::new());
            for (param, a) in def.params.iter().zip(args) {
                inner.bind(param, Binding::Value(a));
            }
            self.depth += 1;
            let r = if self.pure { self.pure_body(&def.body, &mut inner) } else { self.helper_body(&def.body, &mut inner) };
            self.depth -= 1;
            return r;
        }
        for a in f.args.iter() {
            args.push(self.expr(a, scope)?);
        }
        let one = |args: &mut Vec<ast::Expr>, this: &Self| -> R<ast::Expr> {
            if args.len() != 1 {
                return this.err(p, format!("`{name}` takes one argument"));
            }
            Ok(args.remove(0))
        };
        let call = |n: &str, a: ast::Expr| mk(ExprKind::Call { name: n.to_string(), args: vec![a], user: false }, span);
        Ok(match name {
            "complex" => {
                if args.len() != 2 {
                    return self.err(p, "`complex(re, im)`");
                }
                let b = args.pop().unwrap();
                let a = args.pop().unwrap();
                mk(ExprKind::Complex(Box::new(a), Box::new(b)), span)
            }
            "pow" => {
                if args.len() != 2 {
                    return self.err(p, "`pow(base, exponent)`");
                }
                let b = args.pop().unwrap();
                let a = args.pop().unwrap();
                self.binary(BinOp::Pow, a, b, p)?
            }
            "atan2" => {
                if args.len() != 2 {
                    return self.err(p, "`atan2(y, x)`");
                }
                let x = args.pop().unwrap();
                let y = args.pop().unwrap();
                call("atan2", mk(ExprKind::Complex(Box::new(x), Box::new(y)), span))
            }
            "norm_sqr" => mk(ExprKind::Modulus(Box::new(one(&mut args, self)?)), span),
            "fold" => {
                let a = one(&mut args, self)?;
                let part = |n: &str| call("abs", call(n, a.clone()));
                mk(ExprKind::Complex(Box::new(part("real")), Box::new(part("imag"))), span)
            }
            "log" => {
                // Rhai's `log` is base 10.
                let a = one(&mut args, self)?;
                mk(ExprKind::Binary(BinOp::Div, Box::new(call("log", a)), Box::new(mk(ExprKind::Float(std::f64::consts::LN_10), span))), span)
            }
            other => {
                let uf = rename_function(other).ok_or_else(|| Diag::error(span, format!("unknown function `{other}`")))?;
                call(uf, one(&mut args, self)?)
            }
        })
    }

    /// A helper's body inlined into `start` or `step`: its statements
    /// pending, its value returned.
    fn helper_body(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<ast::Expr> {
        let (body, value) = self.value_block(b, scope)?;
        self.pending.extend(body);
        Ok(value)
    }

    /// `a op b`, with Rhai's rules where they differ from the checker's.
    fn binary(&mut self, op: BinOp, a: ast::Expr, b: ast::Expr, p: Position) -> R<ast::Expr> {
        let span = self.pos.span(p);
        if op == BinOp::Div && self.ty_of(&a) == TypeName::Int && self.ty_of(&b) == TypeName::Int {
            return self.err(p, "two ints divided: Rhai would truncate; write a float (`1.0 / 2`) for the quotient");
        }
        Ok(mk(ExprKind::Binary(op, Box::new(a), Box::new(b)), span))
    }

    /// A lowered expression's type, as far as the declarations need it
    /// (the checker types it fully).
    fn ty_of(&self, e: &ast::Expr) -> TypeName {
        match &e.kind {
            ExprKind::Int(_) => TypeName::Int,
            ExprKind::Float(_) => TypeName::Float,
            ExprKind::Bool(_) => TypeName::Bool,
            ExprKind::Imag(_) | ExprKind::Complex(..) | ExprKind::Str(_) => TypeName::Complex,
            ExprKind::Name(n) if n == "ismand" => TypeName::Complex,
            ExprKind::Name(n) => self.types.get(n).copied().unwrap_or(TypeName::Complex),
            ExprKind::Predef(n) => match n.as_str() {
                "pass" => TypeName::Int,
                _ => TypeName::Complex,
            },
            ExprKind::Param(n) => self.param_tys.get(n).copied().unwrap_or(TypeName::Complex),
            ExprKind::Call { name, args, user } => {
                if *user {
                    return TypeName::Complex;
                }
                let arg = args.first().map(|a| self.ty_of(a)).unwrap_or(TypeName::Complex);
                let b = match Builtin::from_name(name) {
                    Some(Builtin::Abs) if arg == TypeName::Complex => Some(Builtin::Cabs),
                    other => other,
                };
                match b.map(|b| b.result_ty(to_ty(arg))) {
                    Some(Ok(t)) => from_ty(t),
                    _ => TypeName::Complex,
                }
            }
            ExprKind::Unary(UnOp::Neg, a) => self.ty_of(a),
            ExprKind::Unary(UnOp::Not, _) => TypeName::Bool,
            ExprKind::Modulus(_) => TypeName::Float,
            ExprKind::Assign(_, v) => self.ty_of(v),
            ExprKind::Binary(op, a, b) => match op {
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::And | BinOp::Or => TypeName::Bool,
                BinOp::Pow => match (self.ty_of(a), self.ty_of(b)) {
                    (TypeName::Complex, _) | (_, TypeName::Complex) => TypeName::Complex,
                    _ => TypeName::Float,
                },
                BinOp::Div => match join(self.ty_of(a), self.ty_of(b)) {
                    TypeName::Int => TypeName::Float,
                    t => t,
                },
                _ => join(self.ty_of(a), self.ty_of(b)),
            },
        }
    }
}

enum Base {
    State,
    Params,
    Plane,
    Value(ast::Expr),
}

fn is_operator(name: &str) -> bool {
    matches!(name, "+" | "-" | "*" | "/" | "%" | "**" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "!" | "&" | "|" | "^" | "<<" | ">>")
}

fn binop(op: &str) -> Option<BinOp> {
    Some(match op {
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        "%" => BinOp::Mod,
        "**" => BinOp::Pow,
        "==" => BinOp::Eq,
        "!=" => BinOp::Ne,
        "<" => BinOp::Lt,
        "<=" => BinOp::Le,
        ">" => BinOp::Gt,
        ">=" => BinOp::Ge,
        _ => return None,
    })
}

/// A Rhai formula's function name as the language's built-in.
fn rename_function(name: &str) -> Option<&'static str> {
    Some(match name {
        "ln" => "log",
        "cot" => "cotan",
        "coth" => "cotanh",
        "ceiling" | "ceil" => "ceil",
        "re" => "real",
        "im" => "imag",
        "arg" => "atan2",
        "abs" => "abs",
        other => Builtin::from_name(other)
            .filter(|b| !matches!(b, Builtin::Log | Builtin::Cabs | Builtin::Real | Builtin::Imag | Builtin::Atan2 | Builtin::Cotan | Builtin::Cotanh))
            .map(|b| b.name())?,
    })
}

fn join(a: TypeName, b: TypeName) -> TypeName {
    use TypeName::*;
    match (a, b) {
        (Complex, _) | (_, Complex) => Complex,
        (Float, _) | (_, Float) => Float,
        (Int, Int) => Int,
        (Bool, Bool) => Bool,
        _ => Float,
    }
}

fn to_ty(t: TypeName) -> Ty {
    match t {
        TypeName::Bool => Ty::Bool,
        TypeName::Int => Ty::Int,
        TypeName::Float => Ty::Float,
        _ => Ty::Complex,
    }
}

fn from_ty(t: Ty) -> TypeName {
    match t {
        Ty::Bool => TypeName::Bool,
        Ty::Int => TypeName::Int,
        Ty::Float => TypeName::Float,
        Ty::Complex => TypeName::Complex,
    }
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::interp::{run, Env, ParamValues};
    use super::super::{check::check, parser::parse_file, Dialect};
    use super::*;

    fn uf(src: &str) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        p.unwrap()
    }

    fn rh(src: &str) -> Program {
        program(src).unwrap_or_else(|d| panic!("{}", d.iter().map(|x| x.render(src)).collect::<Vec<_>>().join("\n")))
    }

    /// The two run alike: the same count, escape and iterate, at points
    /// inside and outside.
    fn same(a: &Program, b: &Program, julia: bool) {
        for (x, y) in [(-0.75, 0.1), (0.3, 0.5), (-1.76, -0.02), (0.25, 0.0), (-0.1, 0.65), (0.4, -0.3)] {
            let mut env = Env::at(C64::new(x, y), 200);
            env.julia = julia;
            env.julia_c = C64::new(-0.8, 0.156);
            let (ra, rb) = (run(a, &ParamValues::defaults(a), None, &env), run(b, &ParamValues::defaults(b), None, &env));
            assert_eq!((ra.n, ra.escaped), (rb.n, rb.escaped), "at ({x}, {y})");
            assert!(ra.z.sub(rb.z).abs() <= 1e-12 * (1.0 + ra.z.abs()), "at ({x}, {y}): {:?} against {:?}", ra.z, rb.z);
        }
    }

    const MANDELBROT: &str = r#"
const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout" },
};
const INFO = #{ title: "Mandelbrot", center: complex(-0.5, 0.0), maxiter: 300 };

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}

fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
"#;

    #[test]
    fn a_rhai_formula_runs_as_its_uf_twin() {
        let r = rh(MANDELBROT);
        assert_eq!(r.name, "Mandelbrot");
        assert_eq!(r.view.maxiter, Some(300));
        assert_eq!(r.view.center, Some((-0.5, 0.0)));
        assert_eq!(r.params[0].name, "bailout");
        let m = uf("M {\ninit:\n z = 0\n c = #pixel\nloop:\n z = z * z + c\nbailout:\n |z| <= @bailout\ndefault:\n float param bailout\n  default = 4\n endparam\n}");
        same(&r, &m, false);
        // Deep zoom: equations derived, as for the UF twin.
        let d = super::super::delta::with_derived_equations(&r).expect("equations");
        assert_eq!(super::super::perturb::perturbation(&d).map(|h| h.rebase), Ok(true));
    }

    #[test]
    fn every_construct_lowers_and_runs_as_written() {
        // The Burning Ship by `fold`, a map returned, a helper, a
        // power, the plane.
        let ship = rh(r#"
const PARAMS = #{ bailout: 4.0 };
fn sq(w) { w ** 2 }
fn start(pixel, p, plane) {
    let z0 = if plane.julia { pixel } else { complex(0.0, 0.0) };
    let c = if plane.julia { plane.c } else { pixel };
    #{ z: z0, c: c }
}
fn step(s, p) {
    #{ z: sq(fold(s.z)) + s.c }
}
fn escaped(s, p) { s.z.norm_sqr() > p.bailout }
"#);
        let u = uf("B {\ninit:\n if @j\n  z = #pixel\n  c = (-0.8, 0.156)\n else\n  z = 0\n  c = #pixel\n endif\nloop:\n z = sqr(abs(z)) + c\nbailout:\n |z| <= 4\ndefault:\n bool param j\n  default = false\n endparam\n}");
        same(&ship, &u, false);
        // Carried state (a previous z), op-assignment, `for`, `do`, an
        // int, a complex parameter, a user function, `let` from `if`.
        let phoenix = rh(r#"
const PARAMS = #{
    k: #{ value: complex(0.5667, 0.0), caption: "Feedback" },
    f: #{ function: "ident" },
    mode: #{ value: 1, choices: ["Plain", "Damped"] },
};
fn start(pixel, p) {
    let s = #{ z: complex(0.0, 0.0), prev: complex(0.0, 0.0) };
    s.c = pixel;
    s
}
fn step(s, p) {
    let t = s.z;
    let damp = if p.mode == 1 { 1.0 } else { 0.5 };
    let acc = complex(0.0, 0.0);
    for i in 0..2 {
        acc += s.prev * 0.5;
    }
    let j = 0;
    do {
        j += 1;
    } until j >= 3;
    s.z = p.f(s.z * s.z) + s.c.re + acc * p.k * damp;
    s.z += complex(0.0, s.c.im);
    s.prev = t;
    s
}
fn escaped(s, p) { s.z.abs() > 2.0 }
"#);
        let u = uf("P {\ninit:\n z = 0\n prev = 0\n c = #pixel\nloop:\n t = z\n z = z * z + real(c) + prev * (0.5667, 0) + flip(imag(c))\n prev = t\nbailout:\n cabs(z) <= 2\n}");
        same(&phoenix, &u, false);
        // A pass-dependent step: its cycle recorded.
        let h = rh(r#"
const INFO = #{ title: "Alternating", cycle: #{ prefix: 0, period: 2 } };
fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }
fn step(s, p, pass) {
    if pass % 2 == 0 { s.z = s.z ** 2 + s.c; } else { s.z = fold(s.z) ** 2 + s.c; }
    s
}
fn escaped(s, p) { s.z.norm_sqr() > 4.0 }
"#);
        assert_eq!(h.cycle, Some(Cycle { prefix: 0, period: 2 }));
        let (_, alt) = super::super::hybrid::presets().into_iter().find(|(n, _)| *n == "Mandelbrot / Burning Ship").unwrap();
        same(&h, &alt.program().unwrap(), false);
        // The functions by Rhai's names: ln, log (base 10), abs of a
        // complex its modulus, arg, atan2(y, x).
        let f = rh(r#"
fn start(pixel, p) { #{ z: pixel } }
fn step(s, p) {
    s.z = complex(ln(s.z.abs() + 1.0) + log(10.0) - 1.0, atan2(s.z.im, s.z.re) - s.z.arg());
    s
}
fn escaped(s, p) { s.z.re > 1e9 }
"#);
        let r = run(&f, &ParamValues::defaults(&f), None, &Env::at(C64::new(3.0, 4.0), 1));
        assert!((r.z.re - (6.0f64).ln()).abs() < 1e-12 && r.z.im.abs() < 1e-12, "{:?}", r.z);
    }

    /// The examples the editor offers compile, and run as their UF twins
    /// (on both planes where they have them).
    #[test]
    fn the_examples_run_as_their_uf_twins() {
        let m = rh(include_str!("../../../assets/formulas/mandelbrot.rhai"));
        let um = uf("M {\ninit:\n if ismand == 0\n  z = #pixel\n  c = (-0.8, 0.156)\n else\n  z = 0\n  c = #pixel\n endif\nloop:\n z = z * z + c\nbailout:\n |z| <= @bailout\ndefault:\n float param bailout\n  default = 4\n endparam\n}");
        same(&m, &um, false);
        same(&m, &um, true);
        let ship = rh(include_str!("../../../assets/formulas/burning-ship.rhai"));
        let us = uf("B {\ninit:\n z = 0\n c = #pixel\nloop:\n z = sqr(abs(z)) + c\nbailout:\n |z| <= 4\n}");
        same(&ship, &us, false);
        let ph = rh(include_str!("../../../assets/formulas/phoenix.rhai"));
        let up = uf("P {\ninit:\n z = 0\n prev = 0\n c = #pixel\nloop:\n t = z\n z = z * z + c + (-0.5, 0) * prev\n prev = t\nbailout:\n |z| <= 4\n}");
        same(&ph, &up, false);
        // A `start` whose arms disagree on the state is refused.
        let bad = "fn start(pixel, p, plane) { if plane.julia { #{ z: pixel, c: pixel } } else { #{ z: pixel } } }\n\
                   fn step(s, p) { s }\nfn escaped(s, p) { true }";
        assert!(program(bad).unwrap_err()[0].message.contains("same fields"));
    }

    #[test]
    fn what_a_formula_refuses_and_where() {
        let base = |step: &str| format!("fn start(pixel, p) {{ #{{ z: pixel }} }}\nfn step(s, p) {{\n{step}\n}}\nfn escaped(s, p) {{ s.z.norm_sqr() > 4.0 }}");
        for (step, says) in [
            ("let q = 1 / 2; s.z = s.z * q; s", "two ints divided"),
            ("s.w = s.z; s", "`w` is not in the state"),
            ("s.z = s.z * p.power; s", "`power` is not in `PARAMS`"),
            ("s.z = gamma(s.z); s", "unknown function `gamma`"),
            ("while true { break; } s", "no `break`"),
            ("let a = [1, 2]; s", "no arrays"),
            ("s.z = \"x\"; s", "a string is metadata"),
            ("s.z = s.z + 1.0;", "ends with the state"),
            ("s.z = q; s", "`q` is not declared"),
        ] {
            let src = base(step);
            let err = program(&src).err().unwrap_or_else(|| panic!("{step}: compiled"));
            let text: Vec<String> = err.iter().map(|d| d.message.clone()).collect();
            assert!(text.iter().any(|m| m.contains(says)), "{step}: {text:?}");
            assert!(err.iter().all(|d| d.span.end <= src.len()), "{step}: a span past the source");
        }
        // A missing function, a stray top-level statement.
        assert!(program("fn start(pixel, p) { #{ z: pixel } }").unwrap_err()[0].message.contains("`fn step`"));
        assert!(program("let x = 1;").unwrap_err()[0].message.contains("top level"));
        // A message about a local names it as the script does.
        let src = base("let t = s.z; let t2 = t + true; s");
        let msgs: Vec<String> = program(&src).unwrap_err().iter().map(|d| d.message.clone()).collect();
        assert!(msgs.iter().all(|m| !m.contains("l1_") && !m.contains("l2_")), "{msgs:?}");
    }
}
