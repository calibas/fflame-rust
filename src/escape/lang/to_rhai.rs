//! A checked UF or Fractint formula printed as a Rhai formula (plan,
//! phase 4 step 3): what the Formula Editor converts a formula it opens
//! to. The printed formula compiles back ([`super::rhai`]) to a program
//! that iterates as the original does; the tests hold that over the test
//! files, the measurement over UF's public library.
//!
//! - **The state**: `z`, every variable a pass reads before that pass
//!   has certainly written it (carried from `init:` or the pass before),
//!   every variable the bailout reads, and `#pixel` when the loop reads
//!   it. `start` computes them as locals and returns the map; `step`
//!   reads and writes `s.name`.
//! - Every other variable is a `let` of the function that uses it: at
//!   its first assignment when that comes first and stands at the top
//!   level, else declared at the top with its type's zero (UF's
//!   variables start at zero).
//! - `global:` becomes `prepare`, the values the other sections read
//!   returned in its map and read as `p.name`.
//! - The conversions the checker makes on its own are written where
//!   Rhai needs them: a `let` takes its variable's type from its value
//!   (`complex(0.0, 0.0)`), an int or a bool becomes a float by
//!   `to_float`, a number a bool by a comparison. Where the checker would
//!   widen again (an operand beside a wider one, a value assigned) they
//!   are left out, as the original wrote them.
//! - Fractint's conventions are written out: its tests read the real
//!   part (`x.re != 0.0`, `a.re == b.re`), its `round` is
//!   `floor(x + 0.5)`.
//! - The bailout is the continue test negated, `!(...)`, which a NaN
//!   leaves as UF does.
//! - UF's own perturbation equations (`perturbinit:`, `perturbloop:`,
//!   the `perturb` setting) become `perturb_start`, `perturb_step` and
//!   `perturb_when`, the deviation `d` their state (`d.z` the
//!   reference's value, `d.dz` the deviation, `d.dpixel` the pixel's
//!   offset).

use super::ast::{BinOp, UnOp};
use super::builtins::{Builtin, USER_FUNCTIONS};
use super::check::{flow, reads, Flow};
use super::ir::*;
use super::{Dialect, EntryKind};
use std::collections::HashSet;

type R<T> = Result<T, String>;

// Rhai's binding strengths, as its parser has them.
const OR: u8 = 30;
const AND: u8 = 60;
const EQ: u8 = 90;
const CMP: u8 = 130;
const ADD: u8 = 150;
const MUL: u8 = 180;
const POW: u8 = 190;
/// A prefix operator, or a negative literal.
const UNARY: u8 = 250;
/// A name, a literal, a call, a field.
const ATOM: u8 = 255;

/// A printed expression and how tightly it binds.
struct P(String, u8);

impl P {
    fn atom(s: impl Into<String>) -> P {
        P(s.into(), ATOM)
    }

    /// As an operand that must bind at least as tightly as `min`.
    fn at(self, min: u8) -> String {
        if self.1 < min {
            format!("({})", self.0)
        } else {
            self.0
        }
    }
}

/// The Rhai formula for a checked formula; `header` lines (without the
/// `//`) open the file as comments.
pub fn formula(p: &Program, header: &[String]) -> R<String> {
    if p.kind != EntryKind::Formula {
        return Err("a colouring is not a formula".into());
    }
    Printer::new(p).file(header)
}

/// The Rhai colouring for a checked colouring: `start(z, p)` its
/// `init:`, `step(s, z, p)` its `loop:` (both left out when it keeps no
/// state, as nothing they compute would reach the result), `finish(s, z,
/// p)` its `final:`, which returns `#index` (with `#solid`, a map).
pub fn coloring(p: &Program, header: &[String]) -> R<String> {
    if p.kind != EntryKind::Coloring {
        return Err("a formula is not a colouring".into());
    }
    Printer::new(p).file(header)
}

/// The `;` comments of a UF or Fractint text, in order, without the
/// `;` (a `;` inside a string is not one).
pub fn comments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut quoted = false;
        for (i, ch) in line.char_indices() {
            match ch {
                '"' => quoted = !quoted,
                ';' if !quoted => {
                    let c = &line[i + 1..];
                    out.push(c.strip_prefix(' ').unwrap_or(c).trim_end().to_string());
                    break;
                }
                _ => {}
            }
        }
    }
    out
}

/// Names a converted formula's variables leave to its functions'
/// arguments and the names Rhai's formulas give.
const KEPT: &[&str] = &["s", "p", "ctx", "pixel", "z", "start", "step", "escaped", "finish", "prepare", "PARAMS", "INFO", "PI", "E"];

/// What the perturbation equations' variables leave besides: their
/// deviation, `d`.
const KEPT_IN_PERTURB: &[&str] = &["s", "p", "d", "ctx", "pixel", "z", "start", "step", "escaped", "finish", "prepare", "PARAMS", "INFO", "PI", "E"];

/// A name Rhai takes as a variable or a field: not a keyword, nor a word
/// it reserves.
fn rhai_name(n: &str) -> bool {
    ::rhai::is_valid_function_name(n) && ::rhai::Token::lookup_symbol_from_syntax(n).is_none()
}

/// `want` as a Rhai name not yet in `used`, nor one of `kept`: as
/// written where it can be, else with `_` or a number after it.
fn unique_among(want: &str, used: &mut HashSet<String>, kept: &[&str]) -> String {
    let usable = |n: &str| rhai_name(n) && !kept.contains(&n);
    let mut base: String = want.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if base.is_empty() || base.starts_with(|c: char| c.is_ascii_digit()) {
        base.insert(0, 'v');
    }
    let mut name = base.clone();
    let mut n = 1;
    while !usable(&name) || used.contains(&name) {
        name = if n == 1 { format!("{base}_") } else { format!("{base}_{n}") };
        n += 1;
    }
    used.insert(name.clone());
    name
}

/// A variable's name: [`unique_among`] what the formula keeps.
fn unique(want: &str, used: &mut HashSet<String>) -> String {
    unique_among(want, used, KEPT)
}

/// A float as Rhai reads it back exactly: the shorter of its plain and
/// exponent forms (both the shortest that round-trip), with the `.` Rhai
/// wants before an exponent.
fn float(x: f64) -> R<String> {
    // An infinity: a literal past the largest float reads as one.
    if x.is_infinite() {
        return Ok(if x > 0.0 { "1.0e999".into() } else { "-1.0e999".into() });
    }
    if x.is_nan() {
        return Err("a NaN constant has no Rhai literal".into());
    }
    let dotted = |s: String| match s.find('e') {
        Some(i) if !s[..i].contains('.') => format!("{}.0{}", &s[..i], &s[i..]),
        _ => s,
    };
    let (plain, exp) = (dotted(format!("{x:?}")), dotted(format!("{x:e}")));
    Ok(if exp.len() < plain.len() { exp } else { plain })
}

fn float_p(x: f64) -> R<P> {
    let s = float(x)?;
    let b = if s.starts_with('-') { UNARY } else { ATOM };
    Ok(P(s, b))
}

/// A Rhai string literal.
fn string(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn zero(ty: Ty) -> &'static str {
    match ty {
        Ty::Bool => "false",
        Ty::Int => "0",
        Ty::Float => "0.0",
        Ty::Complex => "complex(0.0, 0.0)",
    }
}

/// A user function's choice by the name Rhai's formulas give it.
fn function_name(b: Builtin) -> &'static str {
    match b {
        Builtin::Log => "ln",
        Builtin::Cotan => "cot",
        Builtin::Cotanh => "coth",
        Builtin::Ceil => "ceiling",
        Builtin::Real => "re",
        Builtin::Imag => "im",
        other => other.name(),
    }
}

/// A real value lifted to the wider type of the operation it stands in:
/// a widening conversion, or a complex literal with a zero imaginary
/// part. Beside an operand of the wider type, the checker lifts it again.
fn lifted(e: &IExpr) -> bool {
    match &e.kind {
        IExprKind::Convert(a) => a.ty != Ty::Bool && e.ty != Ty::Bool && a.ty.rank() < e.ty.rank(),
        // A negative zero would lose its sign.
        IExprKind::Complex(_, im) => im.to_bits() == 0,
        _ => false,
    }
}

fn is_literal(e: &IExpr) -> bool {
    matches!(e.kind, IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..))
}

/// What an assignment writes, or a read names: `z` or a variable.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Slot {
    Z,
    Var(VarId),
    /// `#dz`, in the perturbation equations.
    DZ,
    /// A colouring's `#index` and `#solid`, in `final:`.
    Index,
    Solid,
}

fn slots_of_expr(e: &IExpr, out: &mut Vec<Slot>) {
    match &e.kind {
        IExprKind::Var(v) => out.push(Slot::Var(*v)),
        IExprKind::Elem(v, a) => {
            out.push(Slot::Var(*v));
            slots_of_expr(a, out);
        }
        IExprKind::Predef(Predef::Z) => out.push(Slot::Z),
        IExprKind::Predef(Predef::Index) => out.push(Slot::Index),
        IExprKind::Predef(Predef::Solid) => out.push(Slot::Solid),
        IExprKind::Convert(a)
        | IExprKind::Unary(_, a)
        | IExprKind::PowInt(a, _)
        | IExprKind::Modulus(a)
        | IExprKind::Call(_, a)
        | IExprKind::UserCall(_, a)
        | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => slots_of_expr(a, out),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => {
            slots_of_expr(a, out);
            slots_of_expr(b, out);
        }
        _ => {}
    }
}

fn place_slot(p: Place) -> Option<Slot> {
    match p {
        Place::Z => Some(Slot::Z),
        Place::Var(v) => Some(Slot::Var(v)),
        Place::DZ => Some(Slot::DZ),
        Place::Index => Some(Slot::Index),
        Place::Solid => Some(Slot::Solid),
        // A Rhai formula's `creases`, which the converter does not write.
        Place::CreaseHeight | Place::CreaseSlope => None,
    }
}

/// Every slot a statement reads or writes, in order.
fn slots_of_stmt(s: &IStmt, out: &mut Vec<Slot>) {
    match s {
        IStmt::Assign(place, e) => {
            slots_of_expr(e, out);
            out.extend(place_slot(*place));
        }
        IStmt::SetElem { var, index, value } => {
            slots_of_expr(index, out);
            slots_of_expr(value, out);
            out.push(Slot::Var(*var));
        }
        IStmt::If { arms, otherwise } => {
            for (c, b) in arms {
                slots_of_expr(c, out);
                b.iter().for_each(|x| slots_of_stmt(x, out));
            }
            otherwise.iter().for_each(|x| slots_of_stmt(x, out));
        }
        IStmt::While { cond, body } | IStmt::Repeat { body, cond } => {
            slots_of_expr(cond, out);
            body.iter().for_each(|x| slots_of_stmt(x, out));
        }
    }
}

/// Why a program with creases does not convert (a Rhai formula's own:
/// UF has none).
const CREASES: &str = "its creases do not convert to Rhai yet";

/// Which of the formula's functions is being printed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fun {
    Prepare,
    Start,
    Step,
    Escaped,
    /// `perturb_start` or `perturb_step`.
    Perturb,
    /// `perturb_when`.
    When,
    /// A colouring's `finish`.
    Finish,
}

struct Printer<'p> {
    p: &'p Program,
    /// Per variable: its name as a local, and as a state field.
    var: Vec<String>,
    /// Per global the other sections read: its name under `p`.
    global: Vec<Option<String>>,
    param: Vec<String>,
    func: Vec<String>,
    /// Per variable: a field of the state.
    state: Vec<bool>,
    /// A colouring: its functions take the formula's `z`.
    coloring: bool,
    /// A colouring's `#index` and `#solid` as `finish`'s locals.
    result: (String, String),
    fun: Fun,
    /// The function being printed reads the context.
    ctx: bool,
    /// Its locals declared so far.
    declared: HashSet<Slot>,
    out: String,
    depth: usize,
}

impl<'p> Printer<'p> {
    fn new(p: &'p Program) -> Self {
        // Names under `p`: the parameters as written where they can be
        // (a config keys their values by name), then the functions and
        // the globals.
        let mut under_p = HashSet::new();
        let param = p.params.iter().map(|x| unique_among(&x.name, &mut under_p, &[])).collect();
        let func = p.funcs.iter().map(|f| unique_among(&f.name, &mut under_p, &[])).collect();
        let mut global = vec![None; p.vars.len()];
        for &g in &p.globals {
            global[g] = Some(unique_among(&p.vars[g].name, &mut under_p, &[]));
        }
        let mut locals = HashSet::new();
        let var = p.vars.iter().map(|v| unique_among(&v.name, &mut locals, if v.perturb { KEPT_IN_PERTURB } else { KEPT })).collect();
        let result = (unique("index", &mut locals), unique("solid", &mut locals));
        let coloring = p.kind == EntryKind::Coloring;
        // A colouring's state is what the checker packs; a formula's,
        // what a pass or the bailout reads before the pass writes it.
        let state = if coloring {
            (0..p.vars.len()).map(|v| p.state.contains(&v)).collect()
        } else {
            let mut in_loop = Flow::default();
            flow(&p.loop_body, &mut in_loop);
            let mut in_test = Flow::default();
            if let Some(b) = &p.bailout {
                reads(b, &mut in_test);
            }
            (0..p.vars.len()).map(|v| !p.vars[v].global && (in_loop.exposed.contains(&v) || in_test.exposed.contains(&v))).collect()
        };
        Printer {
            p,
            var,
            global,
            param,
            func,
            state,
            coloring,
            result,
            fun: Fun::Start,
            ctx: false,
            declared: HashSet::new(),
            out: String::new(),
            depth: 1,
        }
    }

    fn file(&mut self, header: &[String]) -> R<String> {
        let p = self.p;
        let mut out = String::new();
        for h in header {
            if h.is_empty() {
                out.push_str("//\n");
            } else {
                out.push_str(&format!("// {h}\n"));
            }
        }
        if !header.is_empty() {
            out.push('\n');
        }
        if !p.params.is_empty() || !p.funcs.is_empty() {
            out.push_str("const PARAMS = #{\n");
            for (i, x) in p.params.iter().enumerate() {
                out.push_str(&entry(&self.param[i], self.param_meta(x)?));
            }
            for (i, f) in p.funcs.iter().enumerate() {
                let choice = USER_FUNCTIONS.get(f.default).copied().unwrap_or(Builtin::Ident);
                let mut parts = vec![format!("function: {}", string(function_name(choice)))];
                if f.caption != f.name {
                    parts.push(format!("caption: {}", string(&f.caption)));
                }
                if !f.hint.is_empty() {
                    parts.push(format!("hint: {}", string(&f.hint)));
                }
                out.push_str(&entry(&self.func[i], parts));
            }
            out.push_str("};\n\n");
        }
        out.push_str(&self.info()?);
        out.push('\n');
        if let Some(prep) = self.prepare()? {
            out.push_str(&prep);
            out.push('\n');
        }
        if self.coloring {
            // Without state, nothing `init:` or `loop:` computes reaches
            // the result.
            if !p.state.is_empty() {
                out.push_str(&self.start()?);
                out.push('\n');
                if !p.loop_body.is_empty() {
                    out.push_str(&self.step()?);
                    out.push('\n');
                }
            }
            out.push_str(&self.finish()?);
            return Ok(out);
        }
        let bailout = p.bailout.as_ref().ok_or("the formula has no bailout test")?;
        out.push_str(&self.start()?);
        out.push('\n');
        out.push_str(&self.step()?);
        out.push('\n');
        out.push_str(&self.escaped(bailout)?);
        for (name, body) in [("perturb_start", &p.perturb_init), ("perturb_step", &p.perturb_loop)] {
            if !body.is_empty() {
                out.push('\n');
                out.push_str(&self.perturb(name, body)?);
            }
        }
        if let Some(w) = &p.perturb_when {
            self.begin(Fun::When);
            let t = self.expr(w)?.0;
            out.push_str(&format!("\nfn perturb_when(p{}) {{\n    {t}\n}}\n", self.ctx_arg()));
        }
        if let Some(w) = &p.degree {
            self.begin(Fun::When);
            let t = self.expr(w)?.0;
            out.push_str(&format!("\nfn degree(p{}) {{\n    {t}\n}}\n", self.ctx_arg()));
        }
        Ok(out)
    }

    fn param_meta(&self, x: &Param) -> R<Vec<String>> {
        let int = matches!(x.kind, ParamKind::Int | ParamKind::Enum);
        let num = |v: f64| if int { Ok(format!("{}", v.round() as i64)) } else { float(v) };
        let value = match x.kind {
            ParamKind::Bool => (x.default.0 != 0.0).to_string(),
            ParamKind::Int | ParamKind::Enum => format!("{}", x.default.0.round() as i64),
            ParamKind::Float => float(x.default.0)?,
            ParamKind::Complex => format!("complex({}, {})", float(x.default.0)?, float(x.default.1)?),
        };
        let mut parts = vec![format!("value: {value}")];
        if let Some(m) = x.min {
            parts.push(format!("min: {}", num(m)?));
        }
        if let Some(m) = x.max {
            parts.push(format!("max: {}", num(m)?));
        }
        if x.caption != x.name {
            parts.push(format!("caption: {}", string(&x.caption)));
        }
        if !x.hint.is_empty() {
            parts.push(format!("hint: {}", string(&x.hint)));
        }
        if !x.choices.is_empty() {
            let c: Vec<String> = x.choices.iter().map(|c| string(c)).collect();
            parts.push(format!("choices: [{}]", c.join(", ")));
        }
        if x.exponential {
            parts.push("exponential: true".into());
        }
        Ok(parts)
    }

    fn info(&self) -> R<String> {
        let v = &self.p.view;
        let title = v.title.clone().unwrap_or_else(|| self.p.name.clone());
        let mut parts = vec![format!("title: {}", string(&title))];
        if self.coloring {
            match self.p.region {
                Region::Inside => parts.push("region: \"inside\"".into()),
                Region::Outside => parts.push("region: \"outside\"".into()),
                Region::Both => {}
            }
            return Ok(format!("const INFO = {};\n", map_literal(&parts, 0)));
        }
        if let Some((re, im)) = v.center {
            parts.push(format!("center: complex({}, {})", float(re)?, float(im)?));
        }
        if let Some(m) = v.magn {
            parts.push(format!("magn: {}", float(m)?));
        }
        if let Some(m) = v.maxiter {
            parts.push(format!("maxiter: {m}"));
        }
        if let Some(a) = v.angle {
            parts.push(format!("angle: {}", float(a)?));
        }
        if let Some(c) = self.p.cycle {
            parts.push(format!("cycle: #{{ prefix: {}, period: {} }}", c.prefix, c.period));
        }
        Ok(format!("const INFO = {};\n", map_literal(&parts, 0)))
    }

    // ---- the functions -------------------------------------------------

    fn begin(&mut self, fun: Fun) {
        self.fun = fun;
        self.ctx = false;
        self.declared.clear();
        self.out.clear();
        self.depth = 1;
    }

    fn line(&mut self, s: &str) {
        for _ in 0..self.depth {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn ctx_arg(&self) -> &'static str {
        if self.ctx {
            ", ctx"
        } else {
            ""
        }
    }

    /// `prepare`, when the other functions read what `global:` makes.
    fn prepare(&mut self) -> R<Option<String>> {
        let p = self.p;
        if p.globals.is_empty() {
            return Ok(None);
        }
        self.begin(Fun::Prepare);
        self.body(&p.global_body)?;
        let mut fields = Vec::new();
        for &g in &p.globals {
            let name = self.global[g].clone().unwrap_or_default();
            let value = if self.declared.contains(&Slot::Var(g)) { self.var[g].clone() } else { self.slot_zero(Slot::Var(g))? };
            fields.push(field(&name, value));
        }
        let map = map_literal(&fields, self.depth);
        self.line(&map);
        Ok(Some(format!("fn prepare(p{}) {{\n{}}}\n", self.ctx_arg(), self.out)))
    }

    fn start(&mut self) -> R<String> {
        let p = self.p;
        self.begin(Fun::Start);
        self.body(&p.init)?;
        let mut fields = Vec::new();
        if !self.coloring {
            fields.push(field("z", if self.declared.contains(&Slot::Z) { "z".into() } else { zero(Ty::Complex).into() }));
        }
        for v in 0..p.vars.len() {
            if self.state[v] {
                let value = if self.declared.contains(&Slot::Var(v)) { self.var[v].clone() } else { self.slot_zero(Slot::Var(v))? };
                fields.push(field(&self.var[v], value));
            }
        }
        let map = map_literal(&fields, self.depth);
        self.line(&map);
        let first = if self.coloring { "z" } else { "pixel" };
        Ok(format!("fn start({first}, p{}) {{\n{}}}\n", self.ctx_arg(), self.out))
    }

    fn step(&mut self) -> R<String> {
        let p = self.p;
        self.begin(Fun::Step);
        self.body(&p.loop_body)?;
        self.line("s");
        let args = if self.coloring { "s, z" } else { "s" };
        Ok(format!("fn step({args}, p{}) {{\n{}}}\n", self.ctx_arg(), self.out))
    }

    /// A colouring's `final:`: its result, `#index` (a map with `#solid`
    /// when that is set), its own locals. When `final:` ends by setting
    /// `#index` and touches neither elsewhere, that value is the result.
    fn finish(&mut self) -> R<String> {
        let p = self.p;
        self.begin(Fun::Finish);
        let last = match p.final_body.split_last() {
            Some((IStmt::Assign(Place::Index, e), rest)) => {
                let mut used = Vec::new();
                rest.iter().for_each(|x| slots_of_stmt(x, &mut used));
                slots_of_expr(e, &mut used);
                (!used.iter().any(|x| matches!(x, Slot::Index | Slot::Solid))).then_some((rest, e))
            }
            _ => None,
        };
        if let Some((rest, e)) = last {
            self.body(rest)?;
            let v = self.loose(e)?;
            self.line(&v);
        } else {
            self.body(&p.final_body)?;
            let (index, solid) = self.result.clone();
            let result = match (self.declared.contains(&Slot::Index), self.declared.contains(&Slot::Solid)) {
                (true, false) => index,
                (false, true) => format!("#{{ solid: {solid} }}"),
                (true, true) => format!("#{{ index: {index}, solid: {solid} }}"),
                (false, false) => "0.0".into(),
            };
            self.line(&result);
        }
        Ok(format!("fn finish(s, z, p{}) {{\n{}}}\n", self.ctx_arg(), self.out))
    }

    fn escaped(&mut self, test: &IExpr) -> R<String> {
        self.begin(Fun::Escaped);
        let t = self.expr(test)?;
        Ok(format!("fn escaped(s, p{}) {{\n    !{}\n}}\n", self.ctx_arg(), t.at(ATOM)))
    }

    /// UF's own perturbation equations: the deviation's start or step.
    fn perturb(&mut self, name: &str, body: &[IStmt]) -> R<String> {
        self.begin(Fun::Perturb);
        self.body(body)?;
        self.line("d");
        Ok(format!("fn {name}(d, p{}) {{\n{}}}\n", self.ctx_arg(), self.out))
    }

    /// Whether a slot is a local of the function being printed.
    fn is_local(&self, slot: Slot) -> bool {
        let global = |v: VarId| self.p.vars[v].global;
        match (self.fun, slot) {
            (Fun::Perturb, Slot::Var(v)) => self.p.vars[v].perturb,
            (Fun::Start, Slot::Z) => !self.coloring,
            (Fun::Start, Slot::Var(v)) => !global(v),
            (Fun::Step | Fun::Finish, Slot::Var(v)) => !global(v) && !self.state[v],
            (Fun::Finish, Slot::Index | Slot::Solid) => true,
            (Fun::Prepare, Slot::Var(v)) => global(v),
            _ => false,
        }
    }

    /// A slot's value before anything is written: its type's zero, an
    /// array's elements' (`array(n, 0.0)`; UF's arrays flattened to one
    /// dimension, as their indices are), a size the settings fix as
    /// written (`array(p.n + 1, 0.0)`).
    fn slot_zero(&mut self, s: Slot) -> R<String> {
        match s {
            Slot::Var(v) if self.p.vars[v].is_array() => {
                let p = self.p;
                let var = &p.vars[v];
                let n = if var.size.iter().all(|e| matches!(e.kind, IExprKind::Int(_))) {
                    var.len().to_string()
                } else {
                    let mut parts = Vec::new();
                    for e in &var.size {
                        let P(text, prec) = self.expr(e)?;
                        parts.push(if var.size.len() > 1 && prec < MUL { format!("({text})") } else { text });
                    }
                    parts.join(" * ")
                };
                Ok(format!("array({n}, {})", zero(var.ty)))
            }
            other => Ok(zero(self.slot_decl(other).1).to_string()),
        }
    }

    fn slot_decl(&self, s: Slot) -> (String, Ty) {
        match s {
            Slot::Z => ("z".into(), Ty::Complex),
            Slot::DZ => ("dz".into(), Ty::Complex),
            Slot::Index => (self.result.0.clone(), Ty::Float),
            Slot::Solid => (self.result.1.clone(), Ty::Bool),
            Slot::Var(v) => (self.var[v].clone(), self.p.vars[v].ty),
        }
    }

    /// A function's statements: first the locals whose first use is not
    /// a top-level assignment, at their type's zero.
    fn body(&mut self, stmts: &[IStmt]) -> R<()> {
        let mut seen = HashSet::new();
        let mut hoisted = Vec::new();
        for st in stmts {
            match st {
                IStmt::Assign(place, e) => {
                    let mut read = Vec::new();
                    slots_of_expr(e, &mut read);
                    for s in read {
                        if self.is_local(s) && seen.insert(s) {
                            hoisted.push(s);
                        }
                    }
                    if let Some(s) = place_slot(*place) {
                        seen.insert(s);
                    }
                }
                other => {
                    let mut all = Vec::new();
                    slots_of_stmt(other, &mut all);
                    for s in all {
                        if self.is_local(s) && seen.insert(s) {
                            hoisted.push(s);
                        }
                    }
                }
            }
        }
        for s in hoisted {
            let name = self.slot_decl(s).0;
            let value = self.slot_zero(s)?;
            self.line(&format!("let {name} = {value};"));
            self.declared.insert(s);
        }
        self.stmts(stmts)
    }

    /// Statements in order, a whole-array copy as Rhai writes it.
    fn stmts(&mut self, stmts: &[IStmt]) -> R<()> {
        let mut i = 0;
        while i < stmts.len() {
            if let Some((dst, src)) = stmts.get(i + 1).and_then(|next| self.copy_of(&stmts[i], next)) {
                let target = self.target(Slot::Var(dst))?;
                let from = self.var_ref(src)?;
                self.line(&format!("{target} = {from};"));
                i += 2;
            } else {
                self.stmt(&stmts[i])?;
                i += 1;
            }
        }
        Ok(())
    }

    /// A whole-array assignment, `dst = src`, as the checker writes it
    /// (`Checker::copy_array`): its counter (named `copy N`, a name UF
    /// cannot give) set to 0, then the loop over the elements.
    fn copy_of(&self, first: &IStmt, then: &IStmt) -> Option<(VarId, VarId)> {
        use IExprKind as K;
        let IStmt::Assign(Place::Var(k), IExpr { kind: K::Int(0), .. }) = first else { return None };
        let IStmt::While { cond, body } = then else { return None };
        let is_k = |e: &IExpr| matches!(e.kind, K::Var(v) if v == *k);
        let K::Binary(BinOp::Lt, a, n) = &cond.kind else { return None };
        let [IStmt::SetElem { var: dst, index, value }, IStmt::Assign(Place::Var(k2), step)] = body.as_slice() else { return None };
        let K::Elem(src, at) = &value.kind else { return None };
        let K::Binary(BinOp::Add, b, one) = &step.kind else { return None };
        let fits = self.p.vars[*k].name.starts_with("copy ")
            && is_k(a)
            && matches!(n.kind, K::Int(len) if len == self.p.vars[*dst].len() as i64)
            && is_k(index)
            && is_k(at)
            && k2 == k
            && is_k(b)
            && matches!(one.kind, K::Int(1));
        fits.then_some((*dst, *src))
    }

    fn stmt(&mut self, st: &IStmt) -> R<()> {
        match st {
            IStmt::SetElem { var, index, value } => {
                // Declared by `body` before its first statement.
                let target = self.target(Slot::Var(*var))?;
                let i = self.expr(index)?.0;
                let v = self.loose(value)?;
                self.line(&format!("{target}[{i}] = {v};"));
            }
            IStmt::Assign(place, e) => {
                let slot = place_slot(*place).ok_or_else(|| format!("a formula writes no {place:?}"))?;
                if self.is_local(slot) && !self.declared.contains(&slot) {
                    // Its first use, at the top level: its declaration,
                    // the value of its type.
                    let v = self.expr(e)?.0;
                    let (name, _) = self.slot_decl(slot);
                    self.line(&format!("let {name} = {v};"));
                    self.declared.insert(slot);
                } else {
                    let target = self.target(slot)?;
                    let v = self.loose(e)?;
                    self.line(&format!("{target} = {v};"));
                }
            }
            IStmt::If { arms, otherwise } => {
                for (i, (c, b)) in arms.iter().enumerate() {
                    let c = self.expr(c)?.0;
                    self.line(&if i == 0 { format!("if {c} {{") } else { format!("}} else if {c} {{") });
                    self.block(b)?;
                }
                if !otherwise.is_empty() {
                    self.line("} else {");
                    self.block(otherwise)?;
                }
                self.line("}");
            }
            IStmt::While { cond, body } => {
                let c = self.expr(cond)?.0;
                self.line(&format!("while {c} {{"));
                self.block(body)?;
                self.line("}");
            }
            IStmt::Repeat { body, cond } => {
                self.line("do {");
                self.block(body)?;
                let c = self.expr(cond)?.0;
                self.line(&format!("}} until {c};"));
            }
        }
        Ok(())
    }

    fn block(&mut self, b: &[IStmt]) -> R<()> {
        self.depth += 1;
        let r = self.stmts(b);
        self.depth -= 1;
        r
    }

    /// What an assignment that declares nothing writes.
    fn target(&self, slot: Slot) -> R<String> {
        match (self.fun, slot) {
            (Fun::Start, Slot::Z) => Ok("z".into()),
            (Fun::Step, Slot::Z) => Ok("s.z".into()),
            (Fun::Perturb, Slot::DZ) => Ok("d.dz".into()),
            _ if self.is_local(slot) => Ok(self.slot_decl(slot).0),
            (Fun::Step | Fun::Finish, Slot::Var(v)) if self.state[v] => Ok(format!("s.{}", self.var[v])),
            _ => Err(format!("`{}` is written where the converter keeps no place for it", self.slot_decl(slot).0)),
        }
    }

    /// An assigned value: a widening the assignment makes again is left
    /// out.
    fn loose(&mut self, e: &IExpr) -> R<String> {
        if lifted(e) {
            return Ok(self.narrow(e)?.0);
        }
        Ok(self.expr(e)?.0)
    }

    /// A lifted value as it was before the lift.
    fn narrow(&mut self, e: &IExpr) -> R<P> {
        match &e.kind {
            IExprKind::Convert(a) => self.expr(a),
            IExprKind::Complex(re, _) => float_p(*re),
            _ => self.expr(e),
        }
    }

    // ---- expressions -----------------------------------------------------

    fn expr(&mut self, e: &IExpr) -> R<P> {
        use IExprKind as K;
        Ok(match &e.kind {
            K::Bool(b) => P::atom(b.to_string()),
            K::Int(i) => P(i.to_string(), if *i < 0 { UNARY } else { ATOM }),
            K::Float(x) => float_p(*x)?,
            K::Complex(re, im) => P::atom(format!("complex({}, {})", float(*re)?, float(*im)?)),
            K::Var(v) => P::atom(self.var_ref(*v)?),
            K::Elem(v, index) => {
                let i = self.expr(index)?.0;
                P::atom(format!("{}[{i}]", self.var_ref(*v)?))
            }
            K::Param(i) => P::atom(format!("p.{}", self.param[*i])),
            K::Predef(q) => self.predef(*q)?,
            K::Convert(a) => self.convert(a, e.ty)?,
            K::MakeComplex(a, b) => {
                let (a, b) = (self.expr(a)?.0, self.expr(b)?.0);
                P::atom(format!("complex({a}, {b})"))
            }
            K::Unary(UnOp::Neg, a) => P(format!("-{}", self.expr(a)?.at(ATOM)), UNARY),
            // Not the parameter plane: the Julia plane.
            K::Unary(UnOp::Not, a) if matches!(&a.kind, K::Convert(x) if matches!(x.kind, K::Predef(Predef::IsMand))) => {
                self.ctx = true;
                P::atom("ctx.julia")
            }
            K::Unary(UnOp::Not, a) => P(format!("!{}", self.expr(a)?.at(ATOM)), UNARY),
            K::Binary(op, a, b) => self.binary(*op, a, b)?,
            K::PowInt(a, n) => {
                // An int base was widened for the power; Rhai's `**`
                // widens it again.
                let base = match &a.kind {
                    K::Convert(x) if x.ty == Ty::Int => x,
                    _ => a,
                };
                P(format!("{} ** {n}", self.expr(base)?.at(ATOM)), POW)
            }
            K::Modulus(a) => {
                let x = self.expr(a)?;
                if x.1 == ATOM && !is_literal(a) {
                    P::atom(format!("{}.norm_sqr()", x.0))
                } else {
                    P::atom(format!("norm_sqr({})", x.0))
                }
            }
            K::Call(b, a) => self.call(*b, a)?,
            K::UserCall(f, a) => {
                let a = self.expr(a)?.0;
                P::atom(format!("p.{}({a})", self.func[*f]))
            }
            K::UserDeriv(..) | K::UserJac(..) => return Err("a derivative is the compiler's own".into()),
            K::FuncChoice(..) | K::UserDiff(..) => return Err("a function's choice is the compiler's own".into()),
        })
    }

    fn var_ref(&self, v: VarId) -> R<String> {
        let var = &self.p.vars[v];
        if var.global {
            return match (self.fun, &self.global[v]) {
                (Fun::Prepare, _) => Ok(self.var[v].clone()),
                (_, Some(g)) => Ok(format!("p.{g}")),
                (_, None) => Err(format!("`{}` is a global the converter did not find read", var.name)),
            };
        }
        match self.fun {
            Fun::Start => Ok(self.var[v].clone()),
            Fun::Step if !self.state[v] => Ok(self.var[v].clone()),
            Fun::Finish if !self.state[v] => Ok(self.var[v].clone()),
            Fun::Step | Fun::Escaped | Fun::Finish => Ok(format!("s.{}", self.var[v])),
            Fun::Perturb if var.perturb => Ok(self.var[v].clone()),
            Fun::Prepare | Fun::Perturb | Fun::When => Err(format!("`{}` is read where the converter keeps no place for it", var.name)),
        }
    }

    fn ctx_field(&mut self, f: &str) -> P {
        self.ctx = true;
        P::atom(format!("ctx.{f}"))
    }

    fn predef(&mut self, q: Predef) -> R<P> {
        Ok(match q {
            // A formula's `start` takes the pixel; elsewhere the context
            // holds it.
            Predef::Pixel => match self.fun {
                Fun::Start if !self.coloring => P::atom("pixel"),
                Fun::Prepare | Fun::When => return Err("`#pixel` is read where it has no value".into()),
                _ => self.ctx_field("pixel"),
            },
            // A colouring's functions take the formula's `z`.
            Predef::Z if self.coloring => match self.fun {
                Fun::Start | Fun::Step | Fun::Finish => P::atom("z"),
                _ => return Err("`#z` is read where it has no value".into()),
            },
            Predef::Z => match self.fun {
                Fun::Start => P::atom("z"),
                Fun::Step | Fun::Escaped => P::atom("s.z"),
                Fun::Perturb => P::atom("d.z"),
                _ => return Err("`#z` is read where it has no value".into()),
            },
            Predef::NumIter => self.ctx_field("numiter"),
            Predef::Index | Predef::Solid if self.fun == Fun::Finish => {
                P::atom(self.slot_decl(if q == Predef::Index { Slot::Index } else { Slot::Solid }).0)
            }
            Predef::Pi => P::atom("PI()"),
            Predef::E => P::atom("E()"),
            Predef::MaxIter => self.ctx_field("maxiter"),
            Predef::Center => self.ctx_field("center"),
            Predef::Magn => self.ctx_field("magn"),
            Predef::X => self.ctx_field("x"),
            Predef::Y => self.ctx_field("y"),
            Predef::Width => self.ctx_field("width"),
            Predef::Height => self.ctx_field("height"),
            Predef::Random => self.ctx_field("random"),
            Predef::Pass => self.ctx_field("pass"),
            Predef::JuliaC => self.ctx_field("c"),
            Predef::CreaseDecay | Predef::CreasePattern | Predef::CreaseSign | Predef::CreaseReach | Predef::CreaseHeight | Predef::CreaseSlope => {
                return Err(CREASES.into())
            }
            Predef::ScreenPixel => {
                self.ctx = true;
                P::atom("complex(ctx.x, ctx.y)")
            }
            // Fractint's, written out.
            Predef::WhiteSq => {
                self.ctx = true;
                P::atom("complex(to_float((ctx.x + ctx.y) % 2), 0.0)")
            }
            Predef::ScrnMax => {
                self.ctx = true;
                P::atom("complex(ctx.width, ctx.height)")
            }
            Predef::MagXMag => {
                self.ctx = true;
                P::atom("complex(ctx.magn, 1.0)")
            }
            Predef::IsMand => {
                self.ctx = true;
                P::atom("complex(to_float(!ctx.julia), 0.0)")
            }
            Predef::RotSkew => {
                self.ctx = true;
                P::atom("complex(ctx.angle, 0.0)")
            }
            Predef::Index | Predef::Solid => return Err(format!("{q:?} is a colouring's, in `final:`")),
            Predef::DZ if self.fun == Fun::Perturb => P::atom("d.dz"),
            Predef::DPixel if self.fun == Fun::Perturb => P::atom("d.dpixel"),
            Predef::DZ | Predef::DPixel => return Err("`#dz` and `#dpixel` belong to the perturbation equations".into()),
        })
    }

    /// A conversion the checker made, written out.
    fn convert(&mut self, a: &IExpr, to: Ty) -> R<P> {
        use Ty::*;
        let fractint = self.p.dialect == Dialect::Fractint;
        // `ismand` as a test: the parameter plane.
        if to == Bool && matches!(a.kind, IExprKind::Predef(Predef::IsMand)) {
            self.ctx = true;
            return Ok(P("!ctx.julia".into(), UNARY));
        }
        Ok(match (a.ty, to) {
            (x, y) if x == y => self.expr(a)?,
            (Int | Bool, Float) => P::atom(format!("to_float({})", self.expr(a)?.0)),
            (Bool, Int) => P::atom(format!("to_int({})", self.expr(a)?.0)),
            (Bool, Complex) => P::atom(format!("complex(to_float({}), 0.0)", self.expr(a)?.0)),
            (Int | Float, Complex) => P::atom(format!("complex({}, 0.0)", self.expr(a)?.0)),
            (Int, Bool) => P(format!("{} != 0", self.expr(a)?.at(EQ + 1)), EQ),
            (Float, Bool) => P(format!("{} != 0.0", self.expr(a)?.at(EQ + 1)), EQ),
            // Fractint tests the real part alone; UF either part.
            (Complex, Bool) if fractint => P(format!("{} != 0.0", self.real_part(a)?.at(EQ + 1)), EQ),
            (Complex, Bool) => P(format!("{} != complex(0.0, 0.0)", self.expr(a)?.at(EQ + 1)), EQ),
            (x, y) => return Err(format!("a {} converted to a {}", x.name(), y.name())),
        })
    }

    /// The real part of a complex value, for Fractint's tests.
    fn real_part(&mut self, e: &IExpr) -> R<P> {
        match &e.kind {
            IExprKind::Complex(re, _) => float_p(*re),
            IExprKind::Convert(a) if a.ty != Ty::Bool => self.expr(a),
            _ => Ok(P::atom(format!("{}.re", self.expr(e)?.at(ATOM)))),
        }
    }

    fn binary(&mut self, op: BinOp, a: &IExpr, b: &IExpr) -> R<P> {
        let (prec, sym) = match op {
            BinOp::Or => (OR, "||"),
            BinOp::And => (AND, "&&"),
            BinOp::Eq => (EQ, "=="),
            BinOp::Ne => (EQ, "!="),
            BinOp::Lt => (CMP, "<"),
            BinOp::Le => (CMP, "<="),
            BinOp::Gt => (CMP, ">"),
            BinOp::Ge => (CMP, ">="),
            BinOp::Add => (ADD, "+"),
            BinOp::Sub => (ADD, "-"),
            BinOp::Mul => (MUL, "*"),
            BinOp::Div => (MUL, "/"),
            BinOp::Mod => (MUL, "%"),
            BinOp::Pow => (POW, "**"),
            BinOp::DiffAbs => return Err("`diffabs` is the compiler's own".into()),
            // Rhai binds these otherwise than the arithmetic (shifts above
            // `**`, `|` and `^` with `||`): written in parentheses.
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                let (l, r) = (self.expr(a)?, self.expr(b)?);
                return Ok(P::atom(format!("({} {} {})", l.at(ATOM), op.symbol(), r.at(ATOM))));
            }
        };
        // Fractint compares complex values by their real parts, equality
        // too.
        if matches!(op, BinOp::Eq | BinOp::Ne) && a.ty == Ty::Complex && self.p.dialect == Dialect::Fractint {
            let (l, r) = (self.real_part(a)?, self.real_part(b)?);
            return Ok(P(format!("{} {sym} {}", l.at(prec), r.at(prec + 1)), prec));
        }
        let (la, lb) = (lifted(a), lifted(b));
        let l = if la && !lb { self.narrow(a)? } else { self.expr(a)? };
        let r = if lb && !la { self.narrow(b)? } else { self.expr(b)? };
        // `**` binds to the right; its base is kept plain, as `-x ** 2`
        // reads ambiguously.
        let (lmin, rmin) = if op == BinOp::Pow { (ATOM, POW) } else { (prec, prec + 1) };
        Ok(P(format!("{} {sym} {}", l.at(lmin), r.at(rmin)), prec))
    }

    fn call(&mut self, b: Builtin, a: &IExpr) -> R<P> {
        use Builtin::*;
        let named = |s: &mut Self, name: &str| -> R<P> { Ok(P::atom(format!("{name}({})", s.expr(a)?.0))) };
        match b {
            // UF's `abs` of a complex takes each part's; Rhai's is the
            // modulus, UF's `cabs`.
            Abs if a.ty == Ty::Complex => named(self, "fold"),
            Cabs if a.ty == Ty::Int => Ok(P::atom(format!("abs(to_float({}))", self.expr(a)?.0))),
            Cabs => named(self, "abs"),
            Log => named(self, "ln"),
            Atan2 => named(self, "arg"),
            Cotan => named(self, "cot"),
            Cotanh => named(self, "coth"),
            Ceil => named(self, "ceiling"),
            F32 => named(self, "to_f32"),
            Real | Imag => {
                let part = if b == Real { "re" } else { "im" };
                let x = self.expr(a)?;
                if x.1 == ATOM && !is_literal(a) && a.ty == Ty::Complex {
                    Ok(P::atom(format!("{}.{part}", x.0)))
                } else {
                    Ok(P::atom(format!("{part}({})", x.0)))
                }
            }
            // Fractint's round: halves up.
            Round if self.p.dialect == Dialect::Fractint => match a.ty {
                Ty::Int => self.expr(a),
                Ty::Float => Ok(P::atom(format!("floor({} + 0.5)", self.expr(a)?.0))),
                _ => Ok(P::atom(format!("floor({} + complex(0.5, 0.5))", self.expr(a)?.0))),
            },
            _ => named(self, b.name()),
        }
    }
}

/// `name: value`, a map's field.
fn field(name: &str, value: String) -> String {
    format!("{name}: {value}")
}

/// One `PARAMS` entry: its line, or its lines when long.
fn entry(name: &str, parts: Vec<String>) -> String {
    format!("    {name}: {},\n", map_literal(&parts, 1))
}

/// A map literal: on one line when it is short, else a field a line at
/// `depth` (in four-space steps).
fn map_literal(fields: &[String], depth: usize) -> String {
    if fields.is_empty() {
        return "#{}".into();
    }
    let one = format!("#{{ {} }}", fields.join(", "));
    if one.len() + depth * 4 <= 96 {
        return one;
    }
    let pad = "    ".repeat(depth + 1);
    let close = "    ".repeat(depth);
    let mut out = String::from("#{\n");
    for f in fields {
        out.push_str(&format!("{pad}{f},\n"));
    }
    out.push_str(&format!("{close}}}"));
    out
}

/// The points the comparisons run at, on both planes, with the screen
/// and view set to something other than their defaults.
#[cfg(test)]
fn sample_envs(maxiter: u32) -> Vec<(String, super::interp::Env)> {
    use super::builtins::C64;
    let mut out = Vec::new();
    for julia in [false, true] {
        for (x, y) in [(-0.75, 0.1), (0.3, 0.5), (-1.76, -0.02), (0.25, 0.0), (-0.1, 0.65), (0.4, -0.3), (1.2, 0.9)] {
            let mut env = super::interp::Env::at(C64::new(x, y), maxiter);
            env.julia = julia;
            env.julia_c = C64::new(-0.8, 0.156);
            env.x = 7;
            env.y = 4;
            env.width = 640;
            env.height = 480;
            env.magn = 2.0;
            env.center = C64::new(-0.5, 0.25);
            env.random = C64::new(0.3, -0.6);
            out.push((format!("at ({x}, {y}){}", if julia { " on the Julia plane" } else { "" }), env));
        }
    }
    out
}

/// Alike: equal (infinities too), both NaN, or within rounding.
#[cfg(test)]
fn alike(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-9 * (1.0 + a.abs())
}

/// Where two formulas part: the first point at which they do not run
/// alike (iterations, escape, the last z).
#[cfg(test)]
pub(crate) fn parting(a: &Program, b: &Program, maxiter: u32) -> Option<String> {
    use super::interp::{run, ParamValues};
    for (at, env) in sample_envs(maxiter) {
        let (ra, rb) = (run(a, &ParamValues::defaults(a), None, &env), run(b, &ParamValues::defaults(b), None, &env));
        if (ra.n, ra.escaped) != (rb.n, rb.escaped) || !alike(ra.z.re, rb.z.re) || !alike(ra.z.im, rb.z.im) {
            return Some(format!(
                "{at}: {} iterations, escaped {}, z {:?}; converted {} iterations, escaped {}, z {:?}",
                ra.n, ra.escaped, ra.z, rb.n, rb.escaped, rb.z
            ));
        }
    }
    None
}

/// Where two colourings part, each run beside `formula`: the first point
/// at which their index or solid differ.
#[cfg(test)]
pub(crate) fn parting_coloring(formula: &Program, a: &Program, b: &Program, maxiter: u32) -> Option<String> {
    use super::interp::{run, ParamValues};
    let fp = ParamValues::defaults(formula);
    let (pa, pb) = (ParamValues::defaults(a), ParamValues::defaults(b));
    for (at, env) in sample_envs(maxiter) {
        let (ra, rb) = (run(formula, &fp, Some((a, &pa)), &env), run(formula, &fp, Some((b, &pb)), &env));
        if !alike(ra.index, rb.index) || ra.solid != rb.solid {
            return Some(format!("{at}: index {}, solid {}; converted index {}, solid {}", ra.index, ra.solid, rb.index, rb.solid));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::builtins::C64;
    use super::super::interp::{run, Env, ParamValues};
    use super::super::{check::check, parser::parse_file, switch::check_formula};
    use super::*;

    /// Every formula entry of a UF or Fractint text, checked.
    fn programs(text: &str, dialect: Dialect) -> Vec<Program> {
        let (file, _) = parse_file(text);
        file.entries
            .iter()
            .filter_map(|e| {
                let (p, d) = check_formula(&file, e, dialect);
                p.filter(|_| !super::super::has_errors(&d))
            })
            .collect()
    }

    fn round_trip(p: &Program) -> String {
        let text = formula(p, &[]).unwrap_or_else(|e| panic!("{}: {e}", p.name));
        let back = super::super::rhai::program(&text)
            .unwrap_or_else(|d| panic!("{} does not compile back:\n{}\n{text}", p.name, d.iter().map(|x| x.render(&text)).collect::<Vec<_>>().join("\n")));
        if let Some(why) = parting(p, &back, 200) {
            panic!("{} runs otherwise once converted, {why}\n{text}", p.name);
        }
        text
    }

    /// Every formula of the test files converts, compiles back, and runs
    /// as the original.
    #[test]
    fn the_test_files_convert_and_run_as_written() {
        let mut n = 0;
        for (text, dialect) in [
            (include_str!("../../../tests/formulas/language.ufm"), Dialect::UltraFractal),
            (include_str!("../../../tests/formulas/fractint.frm"), Dialect::Fractint),
        ] {
            for p in programs(text, dialect) {
                round_trip(&p);
                n += 1;
            }
        }
        assert!(n >= 10, "only {n} formulas");
    }

    /// Every colouring of the test file converts, compiles back, keeps
    /// its state's size, and colours as the original beside a formula.
    #[test]
    fn the_test_colourings_convert_and_colour_as_written() {
        let formula = programs("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| <= 4\n}", Dialect::UltraFractal).remove(0);
        let text = include_str!("../../../tests/formulas/language.ucl");
        let (file, _) = parse_file(text);
        let mut n = 0;
        for e in &file.entries {
            let (p, d) = check(e, EntryKind::Coloring, Dialect::UltraFractal);
            let Some(p) = p.filter(|_| !super::super::has_errors(&d)) else { continue };
            let text = coloring(&p, &[]).unwrap_or_else(|e| panic!("{}: {e}", p.name));
            let back = super::super::rhai::coloring(&text)
                .unwrap_or_else(|d| panic!("{} does not compile back:\n{}\n{text}", p.name, d.iter().map(|x| x.render(&text)).collect::<Vec<_>>().join("\n")));
            assert_eq!(back.state_floats(), p.state_floats(), "{}: its state\n{text}", p.name);
            assert_eq!(back.region, p.region, "{}", p.name);
            if let Some(why) = parting_coloring(&formula, &p, &back, 100) {
                panic!("{} colours otherwise once converted, {why}\n{text}", p.name);
            }
            n += 1;
        }
        assert!(n >= 5, "only {n} colourings");
    }

    /// A colouring's shape: the formula's `z` taken, the state from what
    /// is carried, the result returned.
    #[test]
    fn a_colouring_converts_to_the_shape_a_person_writes() {
        let (file, _) = parse_file(
            "Trap(OUTSIDE) {\ninit:\n float closest = 1e10\nloop:\n float d = cabs(#z - @point)\n if d < closest\n  closest = d\n endif\n\
             final:\n if closest < @width\n  #solid = true\n else\n  #index = closest * 0.5 + #numiter * 0.01\n endif\n\
             default:\n param point\n  default = (0, 0)\n endparam\n float param width\n  default = 0.01\n endparam\n}",
        );
        let (p, d) = check(&file.entries[0], EntryKind::Coloring, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        let p = p.unwrap();
        let text = coloring(&p, &[]).unwrap();
        for line in [
            "const INFO = #{ title: \"Trap\", region: \"outside\" };",
            "fn start(z, p) {\n    let closest = 1.0e10;\n    #{ closest: closest }\n}",
            "fn step(s, z, p) {\n    let d = abs(z - p.point);\n    if d < s.closest {\n        s.closest = d;\n    }\n    s\n}",
            "fn finish(s, z, p, ctx) {",
            "let index = 0.0;",
            "solid = true;",
            "index = s.closest * 0.5 + ctx.numiter * 0.01;",
            "#{ index: index, solid: solid }",
        ] {
            assert!(text.contains(line), "no `{line}` in\n{text}");
        }
        let back = super::super::rhai::coloring(&text).unwrap();
        let formula = programs("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| <= 4\n}", Dialect::UltraFractal).remove(0);
        assert_eq!(parting_coloring(&formula, &p, &back, 100), None, "{text}");
        // A `final:` that ends by setting `#index` returns the value.
        let (file, _) = parse_file("N {\nfinal:\n float t = #numiter\n #index = t * 0.1\n}");
        let (p, _) = check(&file.entries[0], EntryKind::Coloring, Dialect::UltraFractal);
        let text = coloring(&p.unwrap(), &[]).unwrap();
        assert!(text.contains("fn finish(s, z, p, ctx) {\n    let t = to_float(ctx.numiter);\n    t * 0.1\n}"), "{text}");
        assert!(!text.contains("fn start"), "no state, no start:\n{text}");
    }

    /// The shape: the state from what is carried, `let`s for the rest,
    /// the parameters and view, the bailout negated.
    #[test]
    fn a_formula_converts_to_the_shape_a_person_writes() {
        let (file, _) = parse_file(
            "Phoenix {\ninit:\n z = #pixel\n y = 0\n c = @seed\nloop:\n t = z\n z = sqr(z) + c + @k * y\n y = t\nbailout:\n |z| <= @bailout\n\
             default:\n title = \"Phoenix\"\n maxiter = 300\n complex param seed\n  default = (0.56667, 0)\n endparam\n\
             float param k\n  caption = \"Feedback\"\n  default = -0.5\n endparam\n float param bailout\n  default = 4\n  min = 1\n endparam\n}",
        );
        let (p, d) = check(&file.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        let text = round_trip(&p.unwrap());
        for line in [
            "const PARAMS = #{",
            "    seed: #{ value: complex(0.56667, 0.0) },",
            "    k: #{ value: -0.5, caption: \"Feedback\" },",
            "    bailout: #{ value: 4.0, min: 1.0 },",
            "const INFO = #{ title: \"Phoenix\", maxiter: 300 };",
            "fn start(pixel, p) {",
            "    let z = pixel;",
            "    let y = complex(0.0, 0.0);",
            "    let c = p.seed;",
            "    #{ z: z, y: y, c: c }",
            "fn step(s, p) {",
            "    let t = s.z;",
            "    s.z = sqr(s.z) + s.c + p.k * s.y;",
            "    s.y = t;",
            "    s\n",
            "fn escaped(s, p) {\n    !(s.z.norm_sqr() <= p.bailout)\n}",
        ] {
            assert!(text.contains(line), "no `{line}` in\n{text}");
        }
    }

    /// `global:` becomes `prepare`; the plane, the screen and Fractint's
    /// conventions are written out.
    #[test]
    fn globals_the_plane_and_fractint_convert() {
        let (file, _) = parse_file(
            "G {\nglobal:\n float k = cos(@a * #pi)\n complex w = (k, 1)\ninit:\n z = 0\n if #width > 100\n  z = w\n endif\nloop:\n z = z * z + #pixel * k\nbailout:\n |z| <= 4\n\
             default:\n float param a\n  default = 0.25\n endparam\n}",
        );
        let (p, d) = check(&file.entries[0], EntryKind::Formula, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        let text = round_trip(&p.unwrap());
        for line in ["fn prepare(p) {", "let k = cos(p.a * PI());", "#{ k: k, w: w }", "fn start(pixel, p, ctx) {", "if ctx.width > 100 {", "ctx.pixel * p.k"] {
            assert!(text.contains(line), "no `{line}` in\n{text}");
        }
        // Fractint: real-part tests and comparisons, its round, the
        // plane.
        let progs = programs(
            "F {\n z = pixel, c = round(p1 * 2.5):\n z = sqr(z) + c\n if (z == c)\n  z = 0\n endif\n if (ismand)\n  z = z + whitesq\n endif\n |z| <= 4 && z\n}",
            Dialect::Fractint,
        );
        assert_eq!(progs.len(), 1);
        let text = round_trip(&progs[0]);
        for line in ["floor(p.p1 * 2.5 + complex(0.5, 0.5))", "s.z.re == s.c.re", "s.z.re != 0.0", "if !ctx.julia {", "fn step(s, p, ctx) {"] {
            assert!(text.contains(line), "no `{line}` in\n{text}");
        }
    }

    /// Arrays (plan, item 3, step 3b): declared as `array(n, zero)`, a
    /// global one built in `prepare` and read as the parameters', a
    /// carried one a field of the state, a two-dimensional index
    /// flattened as the checker flattens it, a whole-array copy as Rhai
    /// writes it.
    #[test]
    fn arrays_convert() {
        let progs = programs(
            "A {\nglobal:\n float k[2]\n complex m[2, 3]\n k[1] = 0.5\n m[1, 2] = (0, 1)\ninit:\n z = 0\n complex last[2]\n complex prev[2]\n\
             loop:\n prev = last\n last[1] = last[0]\n last[0] = z\n z = sqr(z) + #pixel + (prev[1] + m[1, 2]) * k[1] * 0.1\nbailout:\n |z| < 4\n}",
            Dialect::UltraFractal,
        );
        let text = round_trip(&progs[0]);
        for line in [
            "let k = array(2, 0.0);",
            "let m = array(6, complex(0.0, 0.0));",
            "m[5] = complex(0.0, 1.0);",
            "#{ k: k, m: m }",
            "#{ z: z, last: array(2, complex(0.0, 0.0)), prev: array(2, complex(0.0, 0.0)) }",
            "    s.prev = s.last;\n    s.last[1] = s.last[0];",
            "(s.prev[1] + p.m[5]) * p.k[1] * 0.1",
        ] {
            assert!(text.contains(line), "no `{line}` in\n{text}");
        }
    }

    /// Sizes the settings fix (step 3f) convert as written: the Rhai,
    /// checked at other settings, has the UF's sizes there and runs as it.
    #[test]
    fn sizes_the_settings_fix_convert_as_written() {
        use super::super::check::{at_settings, SizeSettings};
        let src = "S {\nglobal:\n float t[@n + 1]\n int i = 0\n while i <= @n\n  t[i] = i * 0.5\n  i = i + 1\n endwhile\ninit:\n z = 0\n complex m[2, @n]\n\
                   loop:\n m[1, @n - 1] = z\n z = sqr(z) + #pixel + t[@n] * 0.01 + m[1, @n - 1] * 0.001 + #maxiter * 0.0\nbailout:\n |z| < 4\n\
                   default:\n int param n\n  default = 3\n endparam\n}";
        let text = round_trip(&programs(src, Dialect::UltraFractal).remove(0));
        for line in ["let t = array(p.n + 1, 0.0);", "array(2 * p.n, complex(0.0, 0.0))"] {
            assert!(text.contains(line), "no `{line}` in\n{text}");
        }
        let settings = SizeSettings { params: [("n".to_string(), 6.0)].into_iter().collect(), maxiter: Some(90) };
        let uf = at_settings(settings.clone(), || programs(src, Dialect::UltraFractal).remove(0));
        let back = at_settings(settings, || super::super::rhai::program(&text)).unwrap_or_else(|d| panic!("{:?}\n{text}", d[0].message));
        // The lengths (Rhai's are one-dimensional, and its constructors
        // make temporaries).
        let lens = |p: &Program| -> std::collections::BTreeSet<usize> { p.vars.iter().filter(|v| v.is_array()).map(|v| v.len()).collect() };
        assert_eq!(lens(&uf), [7, 12].into_iter().collect());
        assert_eq!(lens(&back), lens(&uf), "{text}");
        assert_eq!(parting(&uf, &back, 200), None, "{text}");
    }

    /// Names Rhai keeps are renamed, and the literal forms read back
    /// exactly.
    #[test]
    fn names_and_numbers_read_back() {
        let mut used = HashSet::new();
        assert_eq!(unique("fn", &mut used), "fn_");
        assert_eq!(unique("let", &mut used), "let_");
        assert_eq!(unique("print", &mut used), "print_");
        assert_eq!(unique("s", &mut used), "s_");
        assert_eq!(unique("s", &mut used), "s_2");
        assert_eq!(unique("c", &mut used), "c");
        for x in [1e-7, 1e300, -2.5e-12, 0.1, 4.0, -0.0, 123456789.123, f64::INFINITY, f64::NEG_INFINITY] {
            let text = format!("fn start(pixel, p) {{ #{{ z: complex({}, 0.0) }} }}\nfn step(s, p) {{ s }}\nfn escaped(s, p) {{ true }}", float(x).unwrap());
            let back = super::super::rhai::program(&text).unwrap_or_else(|d| panic!("{x}: {:?}", d[0].message));
            let r = run(&back, &ParamValues::defaults(&back), None, &Env::at(C64::ZERO, 0));
            assert_eq!(r.z.re.to_bits(), x.to_bits(), "{x} read back as {}", r.z.re);
        }
        assert_eq!(comments("a = 1 ; one\n hint = \"x; y\" ;two\n;three"), vec!["one", "two", "three"]);
    }
}
