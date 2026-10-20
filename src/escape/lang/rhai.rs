//! Formulas and colourings written in Rhai (plan, phase 4): the GPU
//! subset of a Rhai script lowered to the formula language's parse tree,
//! which the checker types and the rest of the pipeline takes as it
//! takes UF text -- the interpreter, the big-float reference, the WGSL
//! back ends, the derivative and the derived perturbation equations.
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
//!   called as `p.f(z)`), `exponential` (the slider's scale).
//! - **The context**, every function's optional last argument (`ctx`
//!   here; the name is the script's): `ctx.julia` (the panel's Julia
//!   toggle), `ctx.c` (the Julia constant), `ctx.pass` (the pass, 0 on
//!   the first; with `INFO.cycle`, `#{ prefix, period }`, when deep zoom
//!   should follow it), `ctx.maxiter`, `ctx.center`, `ctx.magn`,
//!   `ctx.angle` (the view's rotation, degrees), `ctx.x`, `ctx.y`,
//!   `ctx.width`, `ctx.height` (the pixel on the screen),
//!   `ctx.random` and `ctx.pixel` (in any function; `start` also takes
//!   it).
//! - **`prepare(p, ctx)`**, optional: values computed once per image (UF's
//!   `global:`), returned as a map and read as `p.name`.
//! - **UF's own perturbation equations**, optional: `perturb_start(d, p)`
//!   and `perturb_step(d, p)` set `d.dz`, the pixel's deviation from the
//!   reference orbit, from `d.z` (the reference's value) and `d.dpixel`
//!   (the pixel's offset), and return `d`; `perturb_when(p)` says when
//!   they hold. Without them deep zoom derives its own.
//! - **`degree(p)`**, optional: the formula's degree at infinity (the
//!   `p` of `|z_{n+1}| ~ |z_n|^p`), which the smooth colourings divide by
//!   and which offers their recommended bailout; without it, 2.
//! - `INFO`: `title`, `center` (a complex), `magn`, `maxiter`, `angle`,
//!   `cycle`.
//! - Other functions are helpers, inlined where they are called; they
//!   read only their arguments, as Rhai's functions do.
//! - `if` is a value as well as a statement (`let d = if c { a } else
//!   { b };`), in `start`, `step` and helpers. `escaped` is one test:
//!   its `let`s and its helpers' are substituted, so there an `if` value
//!   is refused.
//!
//! Arrays (plan, item 3, step 3b), of one dimension: a literal `[a, b,
//! c]`, `array(n, v)` (n a constant int, each element `v`), `a[i]` read
//! and written (an int index, clamped to the array), `a.len()`; a state
//! field or a `prepare` value may be one (`s.hist[i]`, `p.k[i]`), `let b
//! = a` copies one. An array of arrays is refused: keep one, `a[i * n +
//! j]`.
//!
//! Values are int, float, bool and complex (`complex(re, im)`; Rhai has
//! no complex literal); a bool and a number do not mix, except by
//! `to_float` and `to_int` (1 for true), Rhai's conversions. Operators:
//! `+ - * / %`, `**` the power, comparisons, `&& || !`; on ints, 32-bit
//! as UF's (a literal past `i32::MAX` its pattern), the bit operations
//! `^ & | << >>`, `>>` logical (on bools `& |` are and and or, `^` xor).
//! Two ints divided is refused -- Rhai would truncate where a formula
//! wants the quotient. Functions (and as methods, `z.sqr()`):
//! `sqr sqrt exp ln sin cos tan cot sinh cosh tanh coth asin acos atan
//! asinh acosh atanh conj flip recip round floor ceiling trunc ident
//! zero cosxx isinf isnan`; `abs` (of a complex, its modulus), `fold`
//! (each part's absolute value: the Burning Ship), `norm_sqr` (`|z|^2`),
//! `arg` (the angle), `re` and `im` (or `.re`, `.im`), `log` (base 10,
//! as in Rhai), `pow(a, b)`, `atan2(y, x)`, `to_float`, `to_int` (a
//! float truncated); `PI()` and `E()`. Loops: `while`, `do ...
//! while/until`, `for i in a..b`; no `break`. What does not compile is
//! refused with the script's own positions.
//!
//! **A colouring** has `finish` where a formula has `escaped` (which a
//! file is, [`kind_of`] says by that). It runs beside the formula, each
//! function taking the formula's `z`: `start(z, p)` returns the state
//! the pixel keeps (optional, as UF's `init:` is), `step(s, z, p)`
//! updates it after each of the formula's passes (optional), and
//! `finish(s, z, p)` returns the palette position -- the index -- or
//! `#{ index, solid }` (solid: the pixel left to the solid colour). The
//! context adds `ctx.numiter` (the passes run, in `finish`); `INFO.region`
//! is `"inside"`, `"outside"` or `"both"`. A state field may be called
//! `z`: in a colouring that name is the colouring's own.
//!
//! UF and Fractint formulas and colourings convert to these shapes
//! ([`super::to_rhai`]).

use super::ast::{self, BinOp, Entry, ExprKind, FuncBlock, ParamBlock, Section, SectionKind, Setting, SettingValue, Settings, TypeName, UnOp};
use super::builtins::Builtin;
use super::ir::{Cycle, Program, Ty};
use super::{Diag, EntryKind, Span};
use ::rhai::{ASTFlags, Engine, Expr, FlowControl, FnCallExpr, OptimizationLevel, Position, ScriptFuncDef, Stmt, StmtBlock};
use std::collections::{HashMap, HashSet};

/// Vectors (the KIFS plan, B1), split into scalars here.
mod vec;
use vec::Val;

type R<T> = Result<T, Diag>;

/// The program a Rhai formula compiles to.
pub fn program(source: &str) -> Result<Program, Vec<Diag>> {
    compile(source, EntryKind::Formula)
}

/// The program a Rhai colouring compiles to.
pub fn coloring(source: &str) -> Result<Program, Vec<Diag>> {
    compile(source, EntryKind::Coloring)
}

/// The program a Rhai transformation compiles to (`fn variation(z, p)`;
/// the KIFS plan, C1): UF's `transform:` section, a flame variation's
/// shape.
pub fn transformation(source: &str) -> Result<Program, Vec<Diag>> {
    compile(source, EntryKind::Transform)
}

/// The program a Rhai solid compiles to (`fn start(pos, p)`, `step`,
/// `escaped`, `distance`; the KIFS plan, B3).
pub fn solid(source: &str) -> Result<Program, Vec<Diag>> {
    compile(source, EntryKind::Solid)
}

/// A number field of a script's `INFO` (a solid's `radius`).
pub fn info_number(source: &str, key: &str) -> Option<f64> {
    let (ast, _) = parse(source).ok()?;
    ast.statements().iter().find_map(|st| {
        let Stmt::Var(x, _, _) = st else { return None };
        let (ident, value, _) = &**x;
        let Expr::Map(m, _) = value else { return None };
        (ident.name.as_str() == "INFO").then_some(())?;
        m.0.iter().find(|(k, _)| k.name.as_str() == key).and_then(|(_, v)| match v {
            Expr::FloatConstant(f, _) => Some(**f),
            Expr::IntegerConstant(i, _) => Some(*i as f64),
            _ => None,
        })
    })
}

/// The program a transformation's 3D function compiles to (`fn
/// variation3(v, p[, ctx])`, the KIFS plan, B2): its point a vector, read
/// from and written to `#pixel` and `#pixelz`.
pub fn transformation3(source: &str) -> Result<Program, Vec<Diag>> {
    let lowered = lower_full(source, EntryKind::Transform, &[], true)?;
    checked(lowered, EntryKind::Transform)
}

/// A transformation's 2D program, or its 3D one when it has only that:
/// what checking it as written means.
pub fn transformation_any(source: &str) -> Result<Program, Vec<Diag>> {
    if has_function(source, "variation") || !has_function(source, "variation3") {
        transformation(source)
    } else {
        transformation3(source)
    }
}

/// Whether the script defines a function of that name.
pub fn has_function(source: &str, name: &str) -> bool {
    parse(source).is_ok_and(|(ast, _)| ast.iter_fn_def().any(|f| f.name == name))
}

/// The registry name a transformation gives itself (`INFO.name`), when
/// it gives one.
pub fn variation_name(source: &str) -> Option<String> {
    info_string(source, "name")
}

// ---- variations called inside a formula (the KIFS plan, C1e) ---------------

thread_local! {
    /// The variation sources in scope for `vary`, by name: what the entry
    /// being compiled carries (`CustomSource::variations`).
    static VARIATIONS: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `f` with `sources` -- `(name, source)` of transformations -- the
/// first place `vary` looks a variation up, then any scope outside this
/// one, then the user's registered ones and their library
/// (`variations::custom::source_of`).
pub fn with_variations<T>(sources: &[(String, String)], f: impl FnOnce() -> T) -> T {
    let outer = VARIATIONS.with(|v| {
        let outer = v.borrow().clone();
        let mut scope = sources.to_vec();
        scope.extend(outer.iter().cloned());
        std::mem::replace(&mut *v.borrow_mut(), scope)
    });
    let r = f();
    VARIATIONS.with(|v| *v.borrow_mut() = outer);
    r
}

/// The source of the variation `name`, for `vary`.
fn variation_source(name: &str) -> Option<String> {
    let carried = VARIATIONS.with(|v| v.borrow().iter().find(|(n, _)| n == name).map(|(_, s)| s.clone()));
    carried.or_else(|| crate::variations::custom::source_of(name))
}

/// The variations an entry calls with `vary`, each with the source it
/// compiled against (`kind` as the entry is): what a config carries
/// beside it. Empty when it calls none or does not compile.
pub fn variations_called(source: &str, kind: EntryKind) -> Vec<(String, String)> {
    lower_as(source, kind).map(|l| l.variations).unwrap_or_default()
}

/// A string field of a script's `INFO`.
fn info_string(source: &str, key: &str) -> Option<String> {
    let (ast, _) = parse(source).ok()?;
    ast.statements().iter().find_map(|st| {
        let Stmt::Var(x, _, _) = st else { return None };
        let (ident, value, _) = &**x;
        let Expr::Map(m, _) = value else { return None };
        (ident.name.as_str() == "INFO").then_some(())?;
        m.0.iter().find(|(k, _)| k.name.as_str() == key).and_then(|(_, v)| match v {
            Expr::StringConstant(s, _) => Some(s.to_string()),
            _ => None,
        })
    })
}

/// A formula from a script's own function `f(z, c)` (phase 4, step 8):
/// the Mandelbrot's shape on both planes, `f` its step, the panel's
/// bailout its test. `fns` are the script's functions, by name, which `f`
/// may call as helpers; each reads only its arguments, as a formula's
/// helper does.
pub fn program_from_fn(name: &str, fns: &[(String, ScriptFuncDef)]) -> Result<Program, Vec<Diag>> {
    let source = format!(
        "const PARAMS = #{{ bailout: #{{ value: 4.0, min: 1.0, caption: \"Bailout\", hint: \"The squared radius past which a pixel has escaped\" }} }};\n\
         const INFO = #{{ title: \"{name}\" }};\n\
         fn start(pixel, p, ctx) {{ if ctx.julia {{ #{{ z: pixel, c: ctx.c }} }} else {{ #{{ z: complex(0.0, 0.0), c: pixel }} }} }}\n\
         fn step(s, p) {{ s.z = {name}(s.z, s.c); s }}\n\
         fn escaped(s, p) {{ s.z.norm_sqr() > p.bailout }}\n"
    );
    let lowered = lower_with(&source, EntryKind::Formula, fns)?;
    checked(lowered, EntryKind::Formula)
}

fn compile(source: &str, kind: EntryKind) -> Result<Program, Vec<Diag>> {
    let lowered = lower_as(source, kind)?;
    checked(lowered, kind)
}

fn checked(lowered: Lowered, kind: EntryKind) -> Result<Program, Vec<Diag>> {
    let (program, diags) = super::check::check_rhai(&lowered.entry, kind);
    let diags: Vec<Diag> = diags.into_iter().map(|d| lowered.rename(d)).collect();
    match program {
        Some(mut p) if !super::has_errors(&diags) => {
            if p.reads(super::ir::Predef::Pass) {
                p.cycle = lowered.cycle;
            }
            p.maps = lowered.maps;
            Ok(p)
        }
        _ => Err(diags),
    }
}

/// What a Rhai file holds, by its functions: a colouring has `finish`,
/// a formula `escaped` or, never escaping, only its `step`. `None` when
/// it does not parse or has neither.
pub fn kind_of(source: &str) -> Option<EntryKind> {
    let (ast, _) = parse(source).ok()?;
    let has = |n: &str| ast.iter_fn_def().any(|f| f.name == n);
    if has("variation") || has("variation3") {
        Some(EntryKind::Transform)
    } else if has("distance") {
        Some(EntryKind::Solid)
    } else if has("finish") {
        Some(EntryKind::Coloring)
    } else if has("escaped") || has("step") {
        Some(EntryKind::Formula)
    } else {
        None
    }
}

/// The title a Rhai formula or colouring gives itself (`INFO.title`),
/// for the panel.
pub fn title(source: &str) -> Option<String> {
    lower_as(source, kind_of(source).unwrap_or(EntryKind::Formula)).ok().map(|l| l.entry.name)
}

/// A formula lowered to the parse tree, with what the checker cannot
/// take from it.
pub struct Lowered {
    pub entry: Entry,
    pub cycle: Option<Cycle>,
    /// Lowered names to the script's, for messages.
    names: Vec<(String, String)>,
    /// The variations `vary` inlined, `(name, source)`, first call first.
    pub variations: Vec<(String, String)>,
    /// A solid's calls of built-in variations, by their `Stmt::Map`'s
    /// `call` (3d-look-and-tools P5).
    pub maps: Vec<super::ir::MapCall>,
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

/// Rhai's own parse of a script, optimisation off so the tree is the
/// source's.
fn parse(source: &str) -> Result<(::rhai::AST, Positions), Vec<Diag>> {
    let mut engine = Engine::new_raw();
    engine.set_optimization_level(OptimizationLevel::None);
    // Rhai counts each link of a chain (`a + b + c`) as a level of depth,
    // without recursing: its default, 64 in a function, refuses a long sum.
    engine.set_max_expr_depths(2048, 2048);
    let pos = Positions::new(source);
    let ast = engine.compile(source).map_err(|e| vec![Diag::error(pos.span(e.position()), format!("{}", e.err_type()))])?;
    Ok((ast, pos))
}

/// Lower a Rhai formula to the parse tree.
pub fn lower(source: &str) -> Result<Lowered, Vec<Diag>> {
    lower_as(source, EntryKind::Formula)
}

/// Lower a Rhai formula or colouring to the parse tree.
pub fn lower_as(source: &str, kind: EntryKind) -> Result<Lowered, Vec<Diag>> {
    lower_with(source, kind, &[])
}

/// [`lower_as`], with functions besides the source's own to call as
/// helpers (a script's, `program_from_fn`).
fn lower_with(source: &str, kind: EntryKind, extra: &[(String, ScriptFuncDef)]) -> Result<Lowered, Vec<Diag>> {
    lower_full(source, kind, extra, false)
}

/// [`lower_with`]; `three` lowers a transformation's 3D function.
fn lower_full(source: &str, kind: EntryKind, extra: &[(String, ScriptFuncDef)], three: bool) -> Result<Lowered, Vec<Diag>> {
    let (ast, pos) = parse(source)?;
    let mut l = Lowerer {
        coloring: kind == EntryKind::Coloring,
        transform: kind == EntryKind::Transform,
        pos,
        source,
        fns: HashMap::new(),
        depth: 0,
        pending: Vec::new(),
        pure: false,
        types: HashMap::new(),
        param_tys: HashMap::new(),
        prepared: HashMap::new(),
        arrays: HashMap::new(),
        names: Vec::new(),
        fresh: 0,
        variations: Vec::new(),
        maps: Vec::new(),
        param_vecs: HashSet::new(),
        three,
        solid: kind == EntryKind::Solid,
        probe: std::cell::Cell::new(0),
    };
    for (name, def) in extra {
        l.fns.insert(name.clone(), def.clone());
    }
    let mut out = l.run(&ast, kind).map_err(|d| vec![d])?;
    out.names = l.names;
    out.variations = l.variations;
    out.maps = l.maps;
    // The checker's names for what the context reads.
    for (uf, rhai) in CTX_FIELDS {
        out.names.push((format!("#{uf}"), format!("ctx.{rhai}")));
    }
    out.names.push(("global:".into(), "prepare".into()));
    Ok(out)
}

/// The context's fields: the checker's predefined name, the Rhai one.
/// `julia` (`ismand == 0`) and `angle` (the real part of `rotskew`) are
/// the context's too.
const CTX_FIELDS: &[(&str, &str)] = &[
    ("juliac", "c"),
    ("pass", "pass"),
    ("maxiter", "maxiter"),
    ("center", "center"),
    ("magn", "magn"),
    ("x", "x"),
    ("y", "y"),
    ("width", "width"),
    ("height", "height"),
    ("random", "random"),
    ("pixel", "pixel"),
    // A colouring's, in `finish`.
    ("numiter", "numiter"),
    // The panel's crease settings, in `creases`.
    ("crease_decay", "crease_decay"),
    ("crease_pattern", "crease_pattern"),
    ("crease_sign", "crease_sign"),
    ("crease_reach", "crease_reach"),
];

// ---- positions --------------------------------------------------------

struct Positions {
    line_starts: Vec<usize>,
    len: usize,
    text: Vec<u8>,
    /// Every position at this span: a variation's body inlined by
    /// `vary`, whose positions are in its own source, placed at the call.
    fixed: Option<Span>,
}

impl Positions {
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Positions { line_starts, len: source.len(), text: source.as_bytes().to_vec(), fixed: None }
    }

    fn fixed(at: Span) -> Self {
        Positions { line_starts: vec![0], len: 0, text: Vec::new(), fixed: Some(at) }
    }

    /// A Rhai position (1-based line and column) as a byte span: the
    /// token there, a word or a single character.
    fn span(&self, p: Position) -> Span {
        if let Some(at) = self.fixed {
            return at;
        }
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
    /// A colouring's `z`: the formula's iterate.
    Z,
    /// The context: every function's optional last argument.
    Ctx,
    /// A local, by its lowered name.
    Local(String),
    /// A value substituted where read: a helper's argument, a `let` in
    /// `escaped`.
    Value(ast::Expr),
    /// An inlined variation's parameters, or its context (`vary`): each
    /// read as the value the call gives it, or its default; the label
    /// says what it is, for a name it lacks.
    ParamMap(HashMap<String, ast::Expr>, &'static str),
    /// A vector local, its three float locals (B1).
    VecLocal([String; 3]),
    /// A vector substituted where read: a helper's vector argument.
    VecValue(Box<vec::V3>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fn3 {
    /// `prepare`: the per-image values, UF's `global:`.
    Prepare,
    Start,
    Step,
    Escaped,
    Helper,
    /// `perturb_start` or `perturb_step`: UF's own perturbation
    /// equations, the deviation `d` their state.
    Perturb,
    /// A colouring's `finish`.
    Finish,
    /// A formula's `creases`: the paper's height and gradient.
    Creases,
    /// A transformation's `variation`: the new point.
    Variation,
}

impl Fn3 {
    /// Builds its map at the end, its fields new (`start`, `prepare`).
    fn builds(self) -> bool {
        matches!(self, Fn3::Start | Fn3::Prepare)
    }
}

struct Lowerer<'s> {
    /// A colouring: `z` is the formula's, and a state field of that name
    /// is the colouring's own.
    coloring: bool,
    /// A transformation: `fn variation`, its top level as a formula's.
    transform: bool,
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
    /// `prepare`'s fields, read as `p.name`.
    prepared: HashMap<String, TypeName>,
    /// The arrays' lengths, by lowered name (their elements' type is in
    /// `types`): a constant, or an int of the parameters and
    /// `ctx.maxiter`, which the settings fix (plan, item 3, step 3f).
    arrays: HashMap<String, ast::Expr>,
    names: Vec<(String, String)>,
    fresh: usize,
    /// The variations `vary` inlined, `(name, source)`.
    variations: Vec<(String, String)>,
    /// A solid's calls of built-in variations (P5), `call` their index.
    maps: Vec<super::ir::MapCall>,
    /// The vector parameters (B1), each three float parameters
    /// `name_x`, `name_y`, `name_z`.
    param_vecs: HashSet<String>,
    /// Lowering a transformation's 3D function, `variation3` (B2).
    three: bool,
    /// Lowering a solid (B3): its `z` is a vector of its own, not the
    /// formula's iterate.
    solid: bool,
    /// How deep [`Lowerer::is_vec`] has looked into helpers, against a
    /// helper that calls itself.
    probe: std::cell::Cell<usize>,
}

/// One function's scope: Rhai names to what they stand for, innermost
/// block last.
#[derive(Clone)]
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

fn prepared_name(f: &str) -> String {
    format!("g_{f}")
}

/// The deviation's fields in the perturbation equations: the reference's
/// value, the pixel's deviation from it, the pixel's offset.
const DEVIATION: &[&str] = &["z", "dz", "dpixel"];

impl<'s> Lowerer<'s> {
    fn err<T>(&self, p: Position, msg: impl Into<String>) -> R<T> {
        Err(Diag::error(self.pos.span(p), msg))
    }

    fn run(&mut self, ast: &::rhai::AST, kind: EntryKind) -> R<Lowered> {
        for f in ast.iter_fn_def() {
            self.fns.insert(f.name.to_string(), (**f).clone());
        }
        let what = if self.coloring {
            "a colouring"
        } else if self.transform {
            "a transformation"
        } else if kind == EntryKind::Solid {
            "a solid"
        } else {
            "a formula"
        };
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
                        other => return self.err(*pos, format!("{what}'s top level holds `PARAMS` and `INFO`, not `{other}`")),
                    };
                    let Expr::Map(m, _) = value else {
                        return self.err(*pos, format!("`{}` is a map, `#{{ ... }}`", ident.name));
                    };
                    for (k, v) in &m.0 {
                        list.push((k.name.to_string(), v.clone(), k.pos));
                    }
                }
                Stmt::Noop(_) => {}
                other => return self.err(other.position(), format!("{what}'s top level holds `PARAMS`, `INFO` and functions")),
            }
        }
        let mut default = Settings::default();
        for (name, v, p) in &params {
            self.param(name, v, *p, &mut default)?;
        }
        let mut title = if self.coloring {
            "Colouring"
        } else if self.transform {
            "Variation"
        } else if kind == EntryKind::Solid {
            "Solid"
        } else {
            "Formula"
        }
        .to_string();
        let mut cycle = None;
        let mut region = None;
        for (name, v, p) in &info {
            match name.as_str() {
                "title" => match v {
                    Expr::StringConstant(s, _) => title = s.to_string(),
                    _ => return self.err(*p, "`title` is a string"),
                },
                // The registry name (`variation_name`), read apart.
                "name" if self.transform => match v {
                    Expr::StringConstant(..) => {}
                    _ => return self.err(*p, "`name` is a string"),
                },
                o if self.transform => return self.err(*p, format!("a transformation's `INFO` has `title` and `name`, not `{o}`")),
                // A solid's ball: the set lies within this radius
                // (`info_number`, read apart).
                "radius" if kind == EntryKind::Solid => match v {
                    Expr::FloatConstant(..) | Expr::IntegerConstant(..) => {}
                    _ => return self.err(*p, "`radius` is a number"),
                },
                o if kind == EntryKind::Solid => return self.err(*p, format!("a solid's `INFO` has `title` and `radius`, not `{o}`")),
                "center" | "magn" | "maxiter" | "angle" if !self.coloring => {
                    let value = self.literal(v)?;
                    default.items.push(Setting { name: name.clone(), value: SettingValue::Expr(value), span: self.pos.span(*p) });
                }
                "cycle" if !self.coloring => {
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
                // Which pixels a colouring colours: UF's `(INSIDE)`.
                "region" if self.coloring => match v {
                    Expr::StringConstant(r, _) if matches!(r.as_str(), "inside" | "outside" | "both") => region = Some(r.to_string()),
                    _ => return self.err(*p, "`region` is \"inside\", \"outside\" or \"both\""),
                },
                o if self.coloring => return self.err(*p, format!("a colouring's `INFO` has `title` and `region`, not `{o}`")),
                o => return self.err(*p, format!("`INFO` has `title`, `center`, `magn`, `maxiter`, `angle` and `cycle`, not `{o}`")),
            }
        }
        let span = Span { start: 0, end: self.source.len() };
        let mut sections = Vec::new();
        if self.transform && self.fns.contains_key("prepare") {
            let f = &self.fns["prepare"];
            return self.err(f.body.position(), "a transformation has no `prepare` yet: its values are computed in `variation`");
        }
        // prepare: once per image, its map's fields read as `p.name`.
        if let Some(prep) = self.fns.get("prepare").cloned() {
            let mut scope = Scope::new(Fn3::Prepare, Vec::new());
            if prep.params.is_empty() || prep.params.len() > 2 {
                return self.err(prep.body.position(), "`prepare(p)` or `prepare(p, ctx)`");
            }
            for (name, role) in prep.params.iter().zip([Binding::Params, Binding::Ctx]) {
                scope.bind(name, role);
            }
            let mut body = Vec::new();
            self.state_body(&prep.body, &mut scope, &mut body)?;
            sections.push(Section { kind: SectionKind::Global, span, body, last_is_bailout: false, labelled: true });
        }
        // A variation's state (C2c): `init(p)` returns the map each
        // thread starts with, its fields `this.name` in `variation`.
        let mut state_fields = Vec::new();
        if self.transform {
            if let Some(init) = self.fns.get("init").cloned() {
                if init.params.len() != 1 {
                    return self.err(init.body.position(), "`init(p)`: the state each thread starts with");
                }
                let mut scope = Scope::new(Fn3::Start, Vec::new());
                scope.bind(&init.params[0], Binding::Params);
                let mut body = Vec::new();
                self.state_body(&init.body, &mut scope, &mut body)?;
                state_fields = scope.fields.clone();
                sections.push(Section { kind: SectionKind::Init, span, body, last_is_bailout: false, labelled: true });
            }
        }
        match kind {
            EntryKind::Formula => sections.extend(self.formula_sections(span, &mut default)?),
            EntryKind::Coloring => sections.extend(self.coloring_sections(span)?),
            EntryKind::Transform => sections.push(self.transform_section(span, state_fields)?),
            EntryKind::Solid => sections.extend(self.solid_sections(span)?),
        }
        default.items.push(Setting { name: "title".into(), value: SettingValue::Strings(vec![title.clone()]), span });
        let setting = region.filter(|r| r != "both");
        let entry = Entry { name: title, setting, span, body: span, sections, default, switch: Settings::default() };
        Ok(Lowered { entry, cycle, names: Vec::new(), variations: Vec::new(), maps: Vec::new() })
    }

    /// A transformation's function as its section (UF's `transform:`):
    /// `variation(z, p[, ctx])`, the point `z` a local the body may
    /// reassign, set from `#pixel`, and the last value written back to
    /// `#pixel` -- or a map `#{ z, color }`, the colour the palette index
    /// a flame blends in (C2d). `this.name` reads and sets the state
    /// `init` returns (`fields`).
    fn transform_section(&mut self, span: Span, fields: Vec<String>) -> R<Section> {
        if self.three {
            return self.transform_section3(span, fields);
        }
        let Some(f) = self.fns.get("variation").cloned() else {
            if self.fns.contains_key("variation3") {
                return self.err(Position::NONE, "a 3D variation only: it has `fn variation3`, not `fn variation(z, p)`");
            }
            return self.err(Position::NONE, "a transformation has `fn variation(z, p)`");
        };
        if f.params.is_empty() || f.params.len() > 3 {
            return self.err(f.body.position(), "`variation(z)`, `variation(z, p)` or `variation(z, p, ctx)`");
        }
        let mut scope = Scope::new(Fn3::Variation, fields);
        let point = self.local(&f.params[0], TypeName::Complex);
        let mut body = vec![ast::Stmt::Decl {
            ty: TypeName::Complex,
            name: point.clone(),
            init: Some(mk(ExprKind::Predef("pixel".into()), span)),
            dims: None,
            span,
        }];
        scope.bind(&f.params[0], Binding::Local(point));
        if let Some(p) = f.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        if let Some(c) = f.params.get(2) {
            scope.bind(c, Binding::Ctx);
        }
        const RESULT: &str = "`variation` ends with the new point, or `#{ z: point, color: index }`";
        let stmts = f.body.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(f.body.position(), RESULT) };
        for st in rest {
            self.stmt(st, &mut scope, &mut body)?;
        }
        let mut color = None;
        let value = match final_value(last) {
            Some(Final::Expr(Expr::Map(m, mp))) => {
                let mut point = None;
                for (k, v) in &m.0 {
                    match k.name.as_str() {
                        "z" => point = Some(self.expr(v, &mut scope)?),
                        "color" => color = Some(self.expr(v, &mut scope)?),
                        o => return self.err(k.pos, format!("a variation's result has `z` and `color`, not `{o}`")),
                    }
                }
                match point {
                    Some(v) => v,
                    None => return self.err(*mp, "the result needs `z`, the new point"),
                }
            }
            Some(Final::Expr(x)) => self.expr(x, &mut scope)?,
            Some(Final::If(fc, p)) => self.if_value(fc, p, &mut scope)?,
            _ => return self.err(last.position(), RESULT),
        };
        self.flush(&mut body);
        let at = self.pos.span(last.position());
        if let Some(c) = color {
            body.push(assign(mk(ExprKind::Predef("vcolor".into()), at), c, at));
        }
        body.push(assign(mk(ExprKind::Predef("pixel".into()), at), value, at));
        Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true })
    }

    /// A solid's functions as its sections (B3): `start(pos, p)` the init,
    /// its map the state (`z` a vector, the iterate; `dr` the running
    /// derivative, read by the colourings if present), `step(s, p)` the
    /// loop, `escaped(s, p)` the bail negated (without it the walk runs
    /// every iteration), `distance(s, p)` the estimate, `#index` in
    /// `final:`.
    fn solid_sections(&mut self, span: Span) -> R<Vec<Section>> {
        let get = |l: &Self, name: &str| l.fns.get(name).cloned();
        let (Some(start), Some(step), Some(distance)) = (get(self, "start"), get(self, "step"), get(self, "distance")) else {
            return Err(Diag::error(Span::default(), "a solid has `fn start(pos, p)`, `fn step(s, p)` and `fn distance(s, p)`"));
        };
        // start: the position a vector, its map the state.
        if start.params.is_empty() || start.params.len() > 2 {
            return self.err(start.body.position(), "`start(pos, p)`");
        }
        let mut scope = Scope::new(Fn3::Start, Vec::new());
        let pixel = || mk(ExprKind::Predef("pixel".into()), span);
        let part = |name: &str| mk(ExprKind::Call { name: name.into(), args: vec![pixel()], user: false }, span);
        let pos = [part("real"), part("imag"), mk(ExprKind::Predef("pixelz".into()), span)];
        scope.bind(&start.params[0], Binding::VecValue(Box::new(pos)));
        if let Some(p) = start.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        let mut init = Vec::new();
        self.state_body(&start.body, &mut scope, &mut init)?;
        let fields = scope.fields.clone();
        if !fields.iter().any(|f| *f == vec::part("z", 0)) {
            return self.err(start.body.position(), "the state needs `z`, the iterate, a vector: `#{ z: pos, dr: 1.0 }`");
        }
        let with = |l: &Self, f: &ScriptFuncDef, what: &str| -> R<Scope> {
            if f.params.is_empty() || f.params.len() > 2 {
                return l.err(f.body.position(), format!("`{what}(s, p)`"));
            }
            let mut scope = Scope::new(if what == "step" { Fn3::Step } else { Fn3::Finish }, fields.clone());
            scope.bind(&f.params[0], Binding::State);
            if let Some(p) = f.params.get(1) {
                scope.bind(p, Binding::Params);
            }
            Ok(scope)
        };
        let mut scope = with(self, &step, "step")?;
        let mut lp = Vec::new();
        self.state_body(&step.body, &mut scope, &mut lp)?;
        let go = match get(self, "escaped") {
            Some(escaped) => {
                let mut scope = with(self, &escaped, "escaped")?;
                scope.which = Fn3::Escaped;
                self.pure = true;
                let test = self.pure_body(&escaped.body, &mut scope);
                self.pure = false;
                let test = test?;
                let tspan = test.span;
                mk(ExprKind::Unary(UnOp::Not, Box::new(test)), tspan)
            }
            None => mk(ExprKind::Bool(true), span),
        };
        let mut scope = with(self, &distance, "distance")?;
        let mut fin = Vec::new();
        self.result_body(&distance.body, &mut scope, &mut fin)?;
        Ok(vec![
            Section { kind: SectionKind::Init, span, body: init, last_is_bailout: false, labelled: true },
            Section { kind: SectionKind::Loop, span, body: lp, last_is_bailout: false, labelled: true },
            Section { kind: SectionKind::Bailout, span, body: vec![ast::Stmt::Expr(go)], last_is_bailout: false, labelled: true },
            Section { kind: SectionKind::Final, span, body: fin, last_is_bailout: false, labelled: true },
        ])
    }

    /// A transformation's 3D function as its section (B2): `variation3(v,
    /// p[, ctx])`, the point `v` a vector local set from `#pixel` and
    /// `#pixelz`, the last value -- a vector, or `#{ z: v, color }` --
    /// written back to them; `ctx.accum` a vector.
    fn transform_section3(&mut self, span: Span, fields: Vec<String>) -> R<Section> {
        let Some(f) = self.fns.get("variation3").cloned() else {
            return self.err(Position::NONE, "a 3D transformation has `fn variation3(v, p)`");
        };
        if f.params.is_empty() || f.params.len() > 3 {
            return self.err(f.body.position(), "`variation3(v)`, `variation3(v, p)` or `variation3(v, p, ctx)`");
        }
        let mut scope = Scope::new(Fn3::Variation, fields);
        let names = self.vec_local(&f.params[0]);
        let pixel = || mk(ExprKind::Predef("pixel".into()), span);
        let part = |name: &str| mk(ExprKind::Call { name: name.into(), args: vec![pixel()], user: false }, span);
        let inits = [part("real"), part("imag"), mk(ExprKind::Predef("pixelz".into()), span)];
        let mut body: Vec<ast::Stmt> = names
            .iter()
            .zip(inits)
            .map(|(n, init)| ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: Some(init), dims: None, span })
            .collect();
        scope.bind(&f.params[0], Binding::VecLocal(names));
        if let Some(p) = f.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        if let Some(c) = f.params.get(2) {
            scope.bind(c, Binding::Ctx);
        }
        const RESULT: &str = "`variation3` ends with the new point, a vector, or `#{ z: v, color: index }`";
        let stmts = f.body.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(f.body.position(), RESULT) };
        for st in rest {
            self.stmt(st, &mut scope, &mut body)?;
        }
        let mut color = None;
        let value = match final_value(last) {
            Some(Final::Expr(Expr::Map(m, mp))) => {
                let mut point = None;
                for (k, v) in &m.0 {
                    match k.name.as_str() {
                        "z" => point = Some(self.vexpr(v, &mut scope)?),
                        "color" => color = Some(self.expr(v, &mut scope)?),
                        o => return self.err(k.pos, format!("a variation's result has `z` and `color`, not `{o}`")),
                    }
                }
                match point {
                    Some(v) => v,
                    None => return self.err(*mp, "the result needs `z`, the new point"),
                }
            }
            Some(Final::Expr(x)) => self.vexpr(x, &mut scope)?,
            _ => return self.err(last.position(), RESULT),
        };
        self.flush(&mut body);
        let at = self.pos.span(last.position());
        if let Some(c) = color {
            body.push(assign(mk(ExprKind::Predef("vcolor".into()), at), c, at));
        }
        let [x, y, z] = value;
        body.push(assign(mk(ExprKind::Predef("pixel".into()), at), mk(ExprKind::Complex(Box::new(x), Box::new(y)), at), at));
        body.push(assign(mk(ExprKind::Predef("pixelz".into()), at), z, at));
        Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true })
    }

    /// A formula's functions as its sections: `start` the init, `step`
    /// the loop, `escaped` the bailout negated, and UF's own
    /// perturbation equations when it brings them.
    fn formula_sections(&mut self, span: Span, default: &mut Settings) -> R<Vec<Section>> {
        let get = |l: &Self, name: &str| l.fns.get(name).cloned();
        let (Some(start), Some(step)) = (get(self, "start"), get(self, "step")) else {
            let missing: Vec<&str> = ["start", "step"].into_iter().filter(|n| !self.fns.contains_key(*n)).collect();
            return Err(Diag::error(Span::default(), format!("a formula needs `fn {}`", missing.join("`, `fn "))));
        };
        // start: its map's fields become the state.
        let mut scope = Scope::new(Fn3::Start, Vec::new());
        if start.params.len() > 3 || start.params.len() < 2 {
            return self.err(start.body.position(), "`start(pixel, p)` or `start(pixel, p, ctx)`");
        }
        for (name, role) in start.params.iter().zip([Binding::Pixel, Binding::Params, Binding::Ctx]) {
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
            return self.err(step.body.position(), "`step(s, p)` or `step(s, p, ctx)`");
        }
        for (name, role) in step.params.iter().zip([Binding::State, Binding::Params, Binding::Ctx]) {
            scope.bind(name, role);
        }
        let mut lp = Vec::new();
        self.state_body(&step.body, &mut scope, &mut lp)?;
        // escaped: one test, its lets substituted; without it the formula
        // never escapes (UF's `bailout: true`).
        let go = match get(self, "escaped") {
            Some(escaped) => {
                let mut scope = Scope::new(Fn3::Escaped, fields);
                if escaped.params.len() > 3 || escaped.params.len() < 2 {
                    return self.err(escaped.body.position(), "`escaped(s, p)` or `escaped(s, p, ctx)`");
                }
                for (name, role) in escaped.params.iter().zip([Binding::State, Binding::Params, Binding::Ctx]) {
                    scope.bind(name, role);
                }
                self.pure = true;
                let test = self.pure_body(&escaped.body, &mut scope)?;
                self.pure = false;
                let tspan = test.span;
                mk(ExprKind::Unary(UnOp::Not, Box::new(test)), tspan)
            }
            None => mk(ExprKind::Bool(true), span),
        };
        let mut sections = vec![
            Section { kind: SectionKind::Init, span, body: init, last_is_bailout: false, labelled: true },
            Section { kind: SectionKind::Loop, span, body: lp, last_is_bailout: false, labelled: true },
            Section { kind: SectionKind::Bailout, span, body: vec![ast::Stmt::Expr(go)], last_is_bailout: false, labelled: true },
        ];
        // UF's own perturbation equations, when the formula brings them:
        // the deviation's start and step, and when they hold.
        for (name, kind) in [("perturb_start", SectionKind::PerturbInit), ("perturb_step", SectionKind::PerturbLoop)] {
            let Some(f) = get(self, name) else { continue };
            let mut scope = Scope::new(Fn3::Perturb, DEVIATION.iter().map(|f| f.to_string()).collect());
            if f.params.len() < 2 || f.params.len() > 3 {
                return self.err(f.body.position(), format!("`{name}(d, p)` or `{name}(d, p, ctx)`"));
            }
            for (n, role) in f.params.iter().zip([Binding::State, Binding::Params, Binding::Ctx]) {
                scope.bind(n, role);
            }
            let mut body = Vec::new();
            self.state_body(&f.body, &mut scope, &mut body)?;
            sections.push(Section { kind, span, body, last_is_bailout: false, labelled: true });
        }
        // Its degree at infinity, for the smooth colourings.
        if let Some(f) = get(self, "degree") {
            let mut scope = Scope::new(Fn3::Escaped, Vec::new());
            if f.params.is_empty() || f.params.len() > 2 {
                return self.err(f.body.position(), "`degree(p)` or `degree(p, ctx)`");
            }
            for (n, role) in f.params.iter().zip([Binding::Params, Binding::Ctx]) {
                scope.bind(n, role);
            }
            self.pure = true;
            let value = self.pure_body(&f.body, &mut scope);
            self.pure = false;
            default.items.push(Setting { name: "degree".into(), value: SettingValue::Expr(value?), span });
        }
        // A folding formula's paper: `creases(p0, p, ctx)` ends with
        // `#{ height, slope }`.
        if let Some(f) = get(self, "creases") {
            let mut scope = Scope::new(Fn3::Creases, Vec::new());
            if f.params.len() < 2 || f.params.len() > 3 {
                return self.err(f.body.position(), "`creases(p0, p)` or `creases(p0, p, ctx)`");
            }
            for (n, role) in f.params.iter().zip([Binding::Pixel, Binding::Params, Binding::Ctx]) {
                scope.bind(n, role);
            }
            let mut body = Vec::new();
            self.creases_body(&f.body, &mut scope, &mut body)?;
            sections.push(Section { kind: SectionKind::Creases, span, body, last_is_bailout: false, labelled: true });
        }
        if let Some(f) = get(self, "perturb_when") {
            let mut scope = Scope::new(Fn3::Escaped, Vec::new());
            if f.params.is_empty() || f.params.len() > 2 {
                return self.err(f.body.position(), "`perturb_when(p)` or `perturb_when(p, ctx)`");
            }
            for (n, role) in f.params.iter().zip([Binding::Params, Binding::Ctx]) {
                scope.bind(n, role);
            }
            self.pure = true;
            let test = self.pure_body(&f.body, &mut scope);
            self.pure = false;
            default.items.push(Setting { name: "perturb".into(), value: SettingValue::Expr(test?), span });
        }
        Ok(sections)
    }

    /// A colouring's functions as its sections: `start(z, p)` the init,
    /// `step(s, z, p)` the loop (both optional, as UF's are), `finish(s,
    /// z, p)` the final, its result `#index` (and `#solid`).
    fn coloring_sections(&mut self, span: Span) -> R<Vec<Section>> {
        let get = |l: &Self, name: &str| l.fns.get(name).cloned();
        let Some(finish) = get(self, "finish") else {
            return Err(Diag::error(Span::default(), "a colouring needs `fn finish`"));
        };
        let mut sections = Vec::new();
        let mut fields = Vec::new();
        if let Some(start) = get(self, "start") {
            let mut scope = Scope::new(Fn3::Start, Vec::new());
            if start.params.len() < 2 || start.params.len() > 3 {
                return self.err(start.body.position(), "a colouring's `start(z, p)` or `start(z, p, ctx)`");
            }
            for (name, role) in start.params.iter().zip([Binding::Z, Binding::Params, Binding::Ctx]) {
                scope.bind(name, role);
            }
            let mut body = Vec::new();
            self.state_body(&start.body, &mut scope, &mut body)?;
            fields = scope.fields.clone();
            sections.push(Section { kind: SectionKind::Init, span, body, last_is_bailout: false, labelled: true });
        }
        if let Some(step) = get(self, "step") {
            let mut scope = Scope::new(Fn3::Step, fields.clone());
            if step.params.len() < 3 || step.params.len() > 4 {
                return self.err(step.body.position(), "a colouring's `step(s, z, p)` or `step(s, z, p, ctx)`");
            }
            for (name, role) in step.params.iter().zip([Binding::State, Binding::Z, Binding::Params, Binding::Ctx]) {
                scope.bind(name, role);
            }
            let mut body = Vec::new();
            self.state_body(&step.body, &mut scope, &mut body)?;
            sections.push(Section { kind: SectionKind::Loop, span, body, last_is_bailout: false, labelled: true });
        }
        let mut scope = Scope::new(Fn3::Finish, fields);
        if finish.params.len() < 3 || finish.params.len() > 4 {
            return self.err(finish.body.position(), "`finish(s, z, p)` or `finish(s, z, p, ctx)`");
        }
        for (name, role) in finish.params.iter().zip([Binding::State, Binding::Z, Binding::Params, Binding::Ctx]) {
            scope.bind(name, role);
        }
        let mut body = Vec::new();
        self.result_body(&finish.body, &mut scope, &mut body)?;
        sections.push(Section { kind: SectionKind::Final, span, body, last_is_bailout: false, labelled: true });
        Ok(sections)
    }

    /// `finish`'s body: statements, then its result -- the index, or
    /// `#{ index, solid }` -- or an `if` whose arms end in results.
    fn result_body(&mut self, b: &StmtBlock, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        const RESULT: &str = "`finish` ends with the index, or `#{ index: ..., solid: ... }`";
        let stmts = b.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(b.position(), RESULT) };
        for st in rest {
            self.stmt(st, scope, out)?;
        }
        match final_value(last) {
            Some(Final::If(fc, p)) => self.result_if(fc, p, scope, out),
            Some(Final::Expr(Expr::Map(m, _))) => {
                let mut values = Vec::new();
                for (k, x) in &m.0 {
                    if !matches!(k.name.as_str(), "index" | "solid") {
                        return self.err(k.pos, format!("`finish` gives `index` and `solid`, not `{}`", k.name));
                    }
                    let v = self.expr(x, scope)?;
                    values.push((k.name.to_string(), v, self.pos.span(k.pos)));
                }
                self.flush(out);
                for (place, v, span) in values {
                    out.push(assign(mk(ExprKind::Predef(place), span), v, span));
                }
                Ok(())
            }
            Some(f) => {
                let v = self.final_expr(f, scope)?;
                self.flush(out);
                let span = v.span;
                out.push(assign(mk(ExprKind::Predef("index".into()), span), v, span));
                Ok(())
            }
            None => self.err(last.position(), RESULT),
        }
    }

    /// `creases`'s body: statements, then `#{ height: h, slope: g }`, the
    /// paper's height and gradient at the starting point.
    fn creases_body(&mut self, b: &StmtBlock, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        const RESULT: &str = "`creases` ends with `#{ height: ..., slope: ... }`";
        let stmts = b.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(b.position(), RESULT) };
        for st in rest {
            self.stmt(st, scope, out)?;
        }
        let Some(Final::Expr(Expr::Map(m, p))) = final_value(last) else { return self.err(last.position(), RESULT) };
        let mut values = Vec::new();
        for (k, x) in &m.0 {
            let place = match k.name.as_str() {
                "height" => "crease_height",
                "slope" => "crease_slope",
                o => return self.err(k.pos, format!("`creases` gives `height` and `slope`, not `{o}`")),
            };
            let v = self.expr(x, scope)?;
            values.push((place, v, self.pos.span(k.pos)));
        }
        if values.len() != 2 {
            return self.err(*p, RESULT);
        }
        self.flush(out);
        for (place, v, span) in values {
            out.push(assign(mk(ExprKind::Predef(place.into()), span), v, span));
        }
        Ok(())
    }

    /// An `if` that ends `finish`, each arm ending in a result.
    fn result_if(&mut self, fc: &FlowControl, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let span = self.pos.span(p);
        let mut arms = Vec::new();
        let mut cur = fc;
        let otherwise;
        loop {
            let cond = self.expr(&cur.expr, scope)?;
            if !arms.is_empty() && !self.pending.is_empty() {
                return self.err(cur.expr.position(), "an `else if` condition holds no `if` value or helper with statements");
            }
            self.flush(out);
            let body = self.result_arm(&cur.body, scope)?;
            arms.push((cond, body));
            match cur.branch.statements() {
                [Stmt::If(inner, _)] => cur = &**inner,
                [] => return self.err(p, "the `if` that ends `finish` needs its `else`"),
                _ => {
                    otherwise = Some(self.result_arm(&cur.branch, scope)?);
                    break;
                }
            }
        }
        out.push(ast::Stmt::If { arms, otherwise, span });
        Ok(())
    }

    fn result_arm(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<Vec<ast::Stmt>> {
        let outer = std::mem::take(&mut self.pending);
        scope.blocks.push(HashMap::new());
        let mut body = Vec::new();
        let r = self.result_body(b, scope, &mut body);
        scope.blocks.pop();
        self.pending = outer;
        r.map(|_| body)
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
                        "exponential" => match x {
                            Expr::BoolConstant(..) => settings.push(Setting { name: k.name.to_string(), value: SettingValue::Expr(self.literal(x)?), span: kspan }),
                            _ => return self.err(k.pos, "`exponential` is `true` or `false`"),
                        },
                        o => {
                            return self.err(
                                k.pos,
                                format!("a parameter has `value`, `min`, `max`, `caption`, `hint`, `choices`, `function`, `exponential`; not `{o}`"),
                            )
                        }
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
        // A vector: three float parameters (B1).
        if let Expr::FnCall(fc, _) = value {
            if fc.name.as_str() == "vec3" {
                return self.vec_param(name, fc, settings, span, out);
            }
        }
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

    /// `name`'s declaration with its value: an array's (a value naming an
    /// array) declared at its length and copied whole.
    fn decl(&mut self, ty: TypeName, name: String, init: ast::Expr, span: Span, out: &mut Vec<ast::Stmt>) {
        if let ExprKind::Name(from) = &init.kind {
            if let Some(len) = self.arrays.get(from).cloned() {
                self.arrays.insert(name.clone(), len.clone());
                self.types.insert(name.clone(), ty);
                let dims = Some(vec![len]);
                out.push(ast::Stmt::Decl { ty, name: name.clone(), init: None, dims, span });
                out.push(assign(mk(ExprKind::Name(name), span), init, span));
                return;
            }
        }
        out.push(ast::Stmt::Decl { ty, name, init: Some(init), dims: None, span });
    }

    /// A new local array of `len` elements, its declaration pending.
    fn new_array(&mut self, ty: TypeName, len: ast::Expr, span: Span) -> String {
        let name = self.local("array", ty);
        self.arrays.insert(name.clone(), len.clone());
        self.pending.push(ast::Stmt::Decl { ty, name: name.clone(), init: None, dims: Some(vec![len]), span });
        name
    }

    /// The array a lowered value names, and its length.
    fn array_of(&self, e: &ast::Expr) -> Option<(String, ast::Expr)> {
        match &e.kind {
            ExprKind::Name(n) => self.arrays.get(n).map(|l| (n.clone(), l.clone())),
            _ => None,
        }
    }

    /// `array[index]`, the array a lowered value. Rhai's tree keeps what
    /// follows an index inside it (`a[i].re` is `a[(i).re]` there, the
    /// index's node without `BREAK`, which ends a chain; `a[s.i]` has
    /// it): a value's chain goes on from the element; a target's ends at
    /// it.
    fn index(&mut self, array: ast::Expr, index: &Expr, flags: ASTFlags, scope: &mut Scope, p: Position, value: bool) -> R<ast::Expr> {
        let span = self.pos.span(p);
        if let ExprKind::Index(..) = array.kind {
            return self.err(p, "an array of arrays: index one array, `a[i * n + j]`");
        }
        let Some((name, _)) = self.array_of(&array) else { return self.err(p, "only an array is indexed") };
        let (index, rest) = match index {
            Expr::Dot(be, _, _) if !flags.contains(ASTFlags::BREAK) => (&be.lhs, Some(&be.rhs)),
            other => (other, None),
        };
        let i = self.expr(index, scope)?;
        let element = mk(ExprKind::Index(name, vec![i]), span);
        match rest {
            None => Ok(element),
            Some(_) if !value => self.err(p, "set an element whole, `a[i] = ...`"),
            Some(r) => self.chain(Base::Value(element), r, scope),
        }
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
        if scope.which.builds() {
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
                    // A vector: three fields, its parts (B1).
                    if self.is_vec(x, scope) {
                        let parts = self.vexpr(x, scope)?;
                        for (i, c) in parts.into_iter().enumerate() {
                            values.push((vec::part(k.name.as_str(), i), k.pos, c, true));
                        }
                        continue;
                    }
                    let v = self.expr(x, scope)?;
                    values.push((k.name.to_string(), k.pos, v, false));
                }
                self.flush(out);
                let mut temps = Vec::new();
                for (f, p, v, is_part) in values {
                    let ty = if is_part { TypeName::Float } else { self.ty_of(&v) };
                    if scope.which.builds() {
                        self.declare_field(&f, ty, v, p, scope, out)?;
                    } else {
                        if !scope.fields.contains(&f) {
                            return self.err(p, format!("`{f}` is not in the state `start` returns"));
                        }
                        let t = self.local(&format!("{f}_new"), ty);
                        let span = v.span;
                        self.decl(ty, t.clone(), v, span, out);
                        temps.push((f, t, span));
                    }
                }
                for (f, t, span) in temps {
                    out.push(assign(self.field_target(scope.which, &f, span), mk(ExprKind::Name(t), span), span));
                }
                Ok(())
            }
            other => self.err(other.position(), "the function ends with the state: `s`, or a map `#{ z: ... }`"),
        }
    }

    /// A field of the state in `start` (`z` the iterate, any other a
    /// variable), or of `prepare`'s map (a global, read as `p.name`).
    fn declare_field(&mut self, f: &str, ty: TypeName, v: ast::Expr, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let span = self.pos.span(p);
        if scope.fields.iter().any(|x| x == f) {
            out.push(assign(self.field_target(scope.which, f, span), v, span));
            return Ok(());
        }
        if scope.which == Fn3::Prepare {
            if self.param_tys.contains_key(f) {
                return self.err(p, format!("`{f}` is a parameter's name; `prepare` gives another"));
            }
            self.prepared.insert(f.to_string(), ty);
        }
        scope.fields.push(f.to_string());
        let target = self.field_target(scope.which, f, span);
        match target.kind {
            ExprKind::Name(name) => {
                self.types.insert(name.clone(), ty);
                let f = f.replace("__", ".");
                let written = if scope.which == Fn3::Prepare { format!("p.{f}") } else { format!("s.{f}") };
                self.names.push((name.clone(), written));
                self.decl(ty, name, v, span, out);
            }
            _ => out.push(assign(target, v, span)),
        }
        Ok(())
    }

    /// What a field is lowered to: `z` the iterate, a state field a
    /// variable, a `prepare` field a global; in the perturbation
    /// equations the deviation's fields (`z` there the reference's).
    fn field_target(&self, which: Fn3, f: &str, span: Span) -> ast::Expr {
        if which == Fn3::Perturb {
            mk(ExprKind::Predef(f.into()), span)
        } else if which == Fn3::Prepare {
            mk(ExprKind::Name(prepared_name(f)), span)
        } else if f == "z" && !self.coloring && !self.transform && !self.solid {
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
                // `let s = #{ ... }` in `start`: the state, built (and
                // `prepare`'s map).
                if scope.which.builds() {
                    if let Expr::Map(..) = value {
                        scope.bind(&ident.name, Binding::State);
                        return self.state_value(value, scope, out);
                    }
                }
                // A vector: three float locals (B1).
                if self.is_vec(value, scope) {
                    let parts = self.vexpr(value, scope)?;
                    self.flush(out);
                    let names = self.vec_local(&ident.name);
                    for (n, c) in names.iter().zip(parts) {
                        out.push(ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: Some(c), dims: None, span });
                    }
                    scope.bind(&ident.name, Binding::VecLocal(names));
                    return Ok(());
                }
                let v = self.expr(value, scope)?;
                self.flush(out);
                let ty = self.ty_of(&v);
                let name = self.local(&ident.name, ty);
                self.decl(ty, name.clone(), v, span, out);
                scope.bind(&ident.name, Binding::Local(name));
                Ok(())
            }
            Stmt::Assignment(x) => {
                let (op, be) = &**x;
                let syntax = op.get_op_assignment_info().map(|(.., syntax)| syntax);
                if self.vec_assign(&be.lhs, &be.rhs, syntax, scope, out)? {
                    return Ok(());
                }
                if let Some((f, fp)) = self.new_field(&be.lhs, scope) {
                    if op.get_op_assignment_info().is_some() {
                        return self.err(fp, format!("`{f}` is not in the state yet: set it with `=`"));
                    }
                    let v = self.expr(&be.rhs, scope)?;
                    self.flush(out);
                    let ty = self.ty_of(&v);
                    return self.declare_field(&f, ty, v, fp, scope, out);
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
            Stmt::If(fc, p) => self.if_chain(fc, self.pos.span(*p), scope, out),
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
                out.push(ast::Stmt::Decl { ty: TypeName::Int, name: name.clone(), init: Some(from), dims: None, span });
                // The bound is read once, as Rhai reads its range.
                let bound = self.local(&format!("{}_end", var.name), TypeName::Int);
                out.push(ast::Stmt::Decl { ty: TypeName::Int, name: bound.clone(), init: Some(to), dims: None, span });
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

    /// An `if` and its `else if`s. A later condition that needs
    /// statements first (an `if` value, a helper's body) cannot be an
    /// arm, as they would run before the earlier tests: the chain from
    /// there nests in the `else`, after them.
    fn if_chain(&mut self, first: &FlowControl, span: Span, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let mut arms = Vec::new();
        let mut fc = first;
        let otherwise;
        loop {
            let cond = self.expr(&fc.expr, scope)?;
            if !arms.is_empty() && !self.pending.is_empty() {
                let mut rest = std::mem::take(&mut self.pending);
                let body = self.block(&fc.body, scope)?;
                let tail = self.else_of(fc, span, scope)?;
                rest.push(ast::Stmt::If { arms: vec![(cond, body)], otherwise: tail, span });
                otherwise = Some(rest);
                break;
            }
            self.flush(out);
            let body = self.block(&fc.body, scope)?;
            arms.push((cond, body));
            // `else if`: one nested `if` alone in the branch.
            match fc.branch.statements() {
                [Stmt::If(inner, _)] => fc = &**inner,
                _ => {
                    otherwise = self.else_of(fc, span, scope)?;
                    break;
                }
            }
        }
        out.push(ast::Stmt::If { arms, otherwise, span });
        Ok(())
    }

    /// What follows an arm's body: nothing, its `else`, or its `else if`
    /// chain, lowered on its own.
    fn else_of(&mut self, fc: &FlowControl, span: Span, scope: &mut Scope) -> R<Option<Vec<ast::Stmt>>> {
        Ok(match fc.branch.statements() {
            [] => None,
            [Stmt::If(inner, _)] => {
                let outer = std::mem::take(&mut self.pending);
                let mut o = Vec::new();
                let r = self.if_chain(inner, span, scope, &mut o);
                self.pending = outer;
                r?;
                Some(o)
            }
            _ => Some(self.block(&fc.branch, scope)?),
        })
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
        self.pending.push(ast::Stmt::Decl { ty, name: name.clone(), init: None, dims: None, span });
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
            Expr::Index(be, flags, p) => {
                let array = self.target(&be.lhs, scope)?;
                self.index(array, &be.rhs, *flags, scope, *p, false)
            }
            Expr::Dot(be, _, _) => {
                let state = match &be.lhs {
                    Expr::Variable(v, _, _) => matches!(scope.get(v.1.as_str()), Some(Binding::State)),
                    // A variation's state (C2c).
                    Expr::ThisPtr(tp) => {
                        self.this_state(*tp, scope)?;
                        true
                    }
                    _ => false,
                };
                if !state {
                    return self.err(e.position(), "set a field of the state, `s.z = ...`");
                }
                let owner = if scope.which == Fn3::Variation { "init" } else { "start" };
                // `s.hist[i] = ...`: an element of a field.
                if let Expr::Index(ibe, flags, ip) = &be.rhs {
                    let Expr::Property(prop, pp) = &ibe.lhs else { return self.err(e.position(), "set a field of the state, `s.z = ...`") };
                    let f = prop.2.to_string();
                    if !scope.fields.contains(&f) {
                        return self.err(*pp, format!("`{f}` is not in the state `{owner}` returns"));
                    }
                    let array = self.field_target(scope.which, &f, self.pos.span(*pp));
                    return self.index(array, &ibe.rhs, *flags, scope, *ip, false);
                }
                let Expr::Property(prop, pp) = &be.rhs else { return self.err(e.position(), "set a field of the state, `s.z = ...`") };
                let f = prop.2.to_string();
                if scope.which == Fn3::Perturb && f != "dz" {
                    return self.err(*pp, format!("`{f}` is read here; the equations set `dz`"));
                }
                if !scope.fields.contains(&f) {
                    return self.err(*pp, format!("`{f}` is not in the state `{owner}` returns"));
                }
                Ok(self.field_target(scope.which, &f, self.pos.span(*pp)))
            }
            _ => self.err(e.position(), "set a variable or a field of the state"),
        }
    }

    /// `this` at `p`: the variation's state, which `init` must give.
    fn this_state(&self, p: Position, scope: &Scope) -> R<()> {
        if scope.which != Fn3::Variation {
            return self.err(p, "`this` is a variation's state, read in `variation`");
        }
        if scope.fields.is_empty() {
            return self.err(p, "`this` is the state `init(p)` returns; this variation has no `init`");
        }
        Ok(())
    }

    /// In `start` (or `prepare`), `s.f` for a field the map has not got
    /// yet.
    fn new_field(&self, e: &Expr, scope: &Scope) -> Option<(String, Position)> {
        if !scope.which.builds() {
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
                        if self.is_vec(value, scope) {
                            let v = self.vexpr(value, scope)?;
                            scope.bind(&ident.name, Binding::VecValue(Box::new(v)));
                        } else {
                            let v = self.expr(value, scope)?;
                            scope.bind(&ident.name, Binding::Value(v));
                        }
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
        if self.is_vec(e, scope) {
            return self.err(e.position(), vec::NUMBER_WANTED);
        }
        Ok(match e {
            Expr::IntegerConstant(v, _) => mk(ExprKind::Int(*v), span),
            Expr::FloatConstant(v, _) => mk(ExprKind::Float(**v), span),
            Expr::BoolConstant(v, _) => mk(ExprKind::Bool(*v), span),
            Expr::Variable(v, _, p) => match scope.get(v.1.as_str()).cloned() {
                Some(Binding::Local(n)) => mk(ExprKind::Name(n), span),
                Some(Binding::Value(x)) => x,
                Some(Binding::Pixel) => mk(ExprKind::Predef("pixel".into()), span),
                Some(Binding::Z) => mk(ExprKind::Predef("z".into()), span),
                Some(Binding::State) => return self.err(*p, "the state is read by its fields, `s.z`"),
                Some(Binding::Params) | Some(Binding::ParamMap(..)) => return self.err(*p, "the parameters are read by name, `p.power`"),
                Some(Binding::Ctx) => return self.err(*p, "the context is read by its fields, `ctx.julia`"),
                Some(Binding::VecLocal(_) | Binding::VecValue(_)) => return self.err(*p, vec::NUMBER_WANTED),
                None => return self.err(*p, format!("`{}` is not declared", v.1)),
            },
            // A part of a vector, or a number of it: `v.x`, `v.length()`.
            Expr::Dot(be, _, _) if self.is_vec(&be.lhs, scope) => {
                let v = self.vexpr(&be.lhs, scope)?;
                match self.member(v, &be.rhs, scope)? {
                    Val::S(s) => s,
                    Val::V(_) => return self.err(e.position(), vec::NUMBER_WANTED),
                }
            }
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
            // `[a, b, c]`: a local array, its elements set.
            Expr::Array(xs, p) => {
                if self.pure {
                    return self.err(*p, "`escaped` is one test: build arrays in `step`");
                }
                if xs.is_empty() {
                    return self.err(*p, "an empty array has no length: `array(n, 0.0)`");
                }
                let mut values = Vec::new();
                for x in xs.iter() {
                    let v = self.expr(x, scope)?;
                    if self.array_of(&v).is_some() {
                        return self.err(x.position(), "an array of arrays: keep one array, `a[i * n + j]`");
                    }
                    values.push(v);
                }
                let ty = values.iter().map(|v| self.ty_of(v)).reduce(join).unwrap_or(TypeName::Float);
                let name = self.new_array(ty, mk(ExprKind::Int(values.len() as i64), span), span);
                for (k, v) in values.into_iter().enumerate() {
                    let at = mk(ExprKind::Index(name.clone(), vec![mk(ExprKind::Int(k as i64), span)]), span);
                    self.pending.push(assign(at, v, span));
                }
                mk(ExprKind::Name(name), span)
            }
            Expr::Index(be, flags, p) => {
                let array = self.expr(&be.lhs, scope)?;
                self.index(array, &be.rhs, *flags, scope, *p, true)?
            }
            _ => return self.err(e.position(), "not in a formula"),
        })
    }

    /// The left of a `.`: a binding read by its fields, or a value.
    fn base(&mut self, e: &Expr, scope: &mut Scope) -> R<Base> {
        // `this`: a variation's state (C2c), as Rhai binds the object a
        // function is called on.
        if let Expr::ThisPtr(p) = e {
            return self.this_state(*p, scope).map(|_| Base::State);
        }
        if let Expr::Variable(v, _, _) = e {
            match scope.get(v.1.as_str()) {
                Some(Binding::State) => return Ok(Base::State),
                Some(Binding::Params) => return Ok(Base::Params),
                Some(Binding::ParamMap(m, what)) => return Ok(Base::Map(m.clone(), what)),
                Some(Binding::Ctx) => return Ok(Base::Ctx),
                _ => {}
            }
        }
        Ok(Base::Value(self.expr(e, scope)?))
    }

    /// `base.rhs`, where `rhs` is a property, a method call or a further
    /// chain.
    fn chain(&mut self, base: Base, rhs: &Expr, scope: &mut Scope) -> R<ast::Expr> {
        // A vector field or parameter, then a part of it or a number of
        // it: `s.z.x`, `p.c.length()` (B1).
        let owner = match &base {
            Base::State => Some(vec::Owner::State),
            Base::Params => Some(vec::Owner::Params),
            Base::Ctx => Some(vec::Owner::Ctx),
            _ => None,
        };
        if let Some(owner) = owner {
            let (first, rest) = match rhs {
                Expr::Dot(be, _, _) => (&be.lhs, Some(&be.rhs)),
                r => (r, None),
            };
            if let Expr::Property(prop, pp) = first {
                if let Some(v) = self.named_vector(&owner, prop.2.as_str(), scope, self.pos.span(*pp)) {
                    let Some(rest) = rest else { return self.err(*pp, vec::NUMBER_WANTED) };
                    return match self.member(v, rest, scope)? {
                        Val::S(s) => Ok(s),
                        Val::V(_) => self.err(rhs.position(), vec::NUMBER_WANTED),
                    };
                }
            }
        }
        match rhs {
            Expr::Dot(be, _, _) => {
                let first = self.chain(base, &be.lhs, scope)?;
                self.chain(Base::Value(first), &be.rhs, scope)
            }
            // A field's element: `p.lines[i]`, `s.hist[0]`.
            Expr::Index(be, flags, p) => {
                let array = self.chain(base, &be.lhs, scope)?;
                self.index(array, &be.rhs, *flags, scope, *p, true)
            }
            Expr::Property(prop, p) => {
                let name = prop.2.as_str();
                let span = self.pos.span(*p);
                match base {
                    Base::State => {
                        if !scope.fields.iter().any(|f| f == name) {
                            let when = if scope.which.builds() {
                                "yet"
                            } else if scope.which == Fn3::Variation {
                                "that `init` returns"
                            } else {
                                "that `start` returns"
                            };
                            return self.err(*p, format!("`{name}` is not in the state {when}"));
                        }
                        Ok(self.field_target(scope.which, name, span))
                    }
                    Base::Params => {
                        if self.prepared.contains_key(name) {
                            return Ok(mk(ExprKind::Name(prepared_name(name)), span));
                        }
                        if !self.param_tys.contains_key(name) {
                            return self.err(*p, format!("`{name}` is not in `PARAMS`, nor made by `prepare`"));
                        }
                        Ok(mk(ExprKind::Param(name.to_string()), span))
                    }
                    // A variation's (C2b): what the flame hands it.
                    Base::Ctx if self.transform => match name {
                        "accum" => Ok(mk(ExprKind::Predef("accum".into()), span)),
                        "weight" => Ok(mk(ExprKind::Predef("weight".into()), span)),
                        o => self.err(*p, format!("a variation's context has `accum` and `weight`; not `{o}`")),
                    },
                    Base::Ctx => match name {
                        // `ismand` is 1 on the parameter plane.
                        "julia" => Ok(mk(
                            ExprKind::Binary(BinOp::Eq, Box::new(mk(ExprKind::Name("ismand".into()), span)), Box::new(mk(ExprKind::Int(0), span))),
                            span,
                        )),
                        // The view's rotation in degrees: Fractint's
                        // `rotskew` holds it.
                        "angle" => Ok(mk(ExprKind::Call { name: "real".into(), args: vec![mk(ExprKind::Name("rotskew".into()), span)], user: false }, span)),
                        // A colouring runs after the formula's pass and
                        // counts its own.
                        "pass" if self.coloring => self.err(*p, "a colouring counts its own passes, in its state"),
                        o => match CTX_FIELDS.iter().find(|(_, r)| *r == o) {
                            Some((uf, _)) => Ok(mk(ExprKind::Predef((*uf).into()), span)),
                            None => {
                                let all: Vec<String> =
                                    ["julia", "angle"].into_iter().chain(CTX_FIELDS.iter().map(|(_, r)| *r)).map(|r| format!("`{r}`")).collect();
                                self.err(*p, format!("the context has {}; not `{o}`", all.join(", ")))
                            }
                        },
                    },
                    Base::Map(m, what) => match m.get(name) {
                        Some(v) => Ok(v.clone()),
                        None => self.err(*p, format!("`{name}` is not {what}")),
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
                Base::Map(..) => self.err(*p, "a variation's parameters hold no functions"),
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
        // A variation of the user's, inlined (the KIFS plan, C1e).
        if name == "vary" && args.is_empty() {
            return self.vary(f, p, scope);
        }
        // A fresh draw in [0, 1) (C2a), unless the script has its own.
        if name == "rand" && args.is_empty() && !self.fns.contains_key("rand") {
            if !f.args.is_empty() {
                return self.err(p, "`rand()` takes no argument: a draw in [0, 1)");
            }
            return Ok(mk(ExprKind::Predef("draw".into()), span));
        }
        // A helper of the script's own, inlined; it reads only its
        // arguments.
        if let Some(def) = self.fns.get(name).cloned() {
            if !args.is_empty() {
                // Rhai binds a method call's value to `this`, not to the
                // first parameter.
                return self.err(p, format!("call a helper as a function, `{name}(x)`"));
            }
            if matches!(name, "start" | "step" | "escaped" | "finish" | "prepare" | "perturb_start" | "perturb_step" | "perturb_when" | "degree" | "creases" | "variation" | "vary") {
                return self.err(p, format!("`{name}` is the formula's own, not a helper to call"));
            }
            // A vector argument binds as one (B1).
            let mut vals = Vec::new();
            for a in f.args.iter() {
                vals.push(self.arg(a, scope)?);
            }
            if def.params.len() != vals.len() {
                return self.err(p, format!("`{name}` takes {}", def.params.len()));
            }
            if self.depth > 32 {
                return self.err(p, "helpers call each other too deep (recursion?)");
            }
            let mut inner = Scope::new(Fn3::Helper, Vec::new());
            for (param, a) in def.params.iter().zip(vals) {
                match a {
                    Val::S(a) => inner.bind(param, Binding::Value(a)),
                    Val::V(v) => inner.bind(param, Binding::VecValue(Box::new(v))),
                }
            }
            // A helper giving a vector, where a number is wanted.
            if self.block_gives_vec(&def.body, &inner) {
                return self.err(p, format!("`{name}` gives a vector where a number is wanted: take a part (`.x`), `length` or `dot`"));
            }
            self.depth += 1;
            let r = if self.pure { self.pure_body(&def.body, &mut inner) } else { self.helper_body(&def.body, &mut inner) };
            self.depth -= 1;
            return r;
        }
        // Numbers of vectors (`dot`, `length`), and the functions that came
        // with them (`min`, `max`, `clamp` of numbers) (B1).
        if matches!(name, "vec3" | "dot" | "cross" | "length" | "normalize" | "min" | "max" | "clamp") || f.args.iter().any(|a| self.is_vec(a, scope)) {
            let mut vals: Vec<Val> = args.drain(..).map(Val::S).collect();
            for a in f.args.iter() {
                vals.push(self.arg(a, scope)?);
            }
            return match self.vfunc(name, vals, p)? {
                Val::S(s) => Ok(s),
                Val::V(_) => self.err(p, vec::NUMBER_WANTED),
            };
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
        // An array's length: a constant, or what the settings fix.
        if name == "len" {
            return match args.as_slice() {
                [a] => match self.array_of(a) {
                    Some((_, len)) => Ok(len),
                    None => self.err(p, "`len` is an array's length"),
                },
                _ => self.err(p, "`a.len()`, an array's length"),
            };
        }
        // `array(n, v)`: n elements, each `v`; n a constant int, or one of
        // the parameters and `ctx.maxiter` (the checker folds it at the
        // settings).
        if name == "array" {
            if self.pure {
                return self.err(p, "`escaped` is one test: build arrays in `step`");
            }
            if args.len() != 2 {
                return self.err(p, "`array(n, value)`: n elements of the value");
            }
            let v = args.pop().unwrap();
            let n = args.pop().unwrap();
            if const_int(&n).is_some_and(|n| n < 1) {
                return self.err(p, "`array(n, value)`: n is at least 1");
            }
            let ty = self.ty_of(&v);
            let name = self.new_array(ty, n.clone(), span);
            fn is_zero(e: &ast::Expr) -> bool {
                match &e.kind {
                    ExprKind::Int(0) | ExprKind::Bool(false) => true,
                    ExprKind::Float(x) => *x == 0.0,
                    ExprKind::Complex(a, b) => is_zero(a) && is_zero(b),
                    _ => false,
                }
            }
            // What every element starts as already (a converted UF
            // array's `array(n, complex(0.0, 0.0))` among them).
            let zero = is_zero(&v);
            if !zero {
                // Each element set, by a counter of its own.
                let i = self.local("i", TypeName::Int);
                let at = |k: &str| mk(ExprKind::Name(k.to_string()), span);
                self.pending.push(ast::Stmt::Decl { ty: TypeName::Int, name: i.clone(), init: Some(mk(ExprKind::Int(0), span)), dims: None, span });
                let body = vec![
                    assign(mk(ExprKind::Index(name.clone(), vec![at(&i)]), span), v, span),
                    assign(at(&i), mk(ExprKind::Binary(BinOp::Add, Box::new(at(&i)), Box::new(mk(ExprKind::Int(1), span))), span), span),
                ];
                let cond = mk(ExprKind::Binary(BinOp::Lt, Box::new(at(&i)), Box::new(n)), span);
                self.pending.push(ast::Stmt::While { cond, body, span });
            }
            return Ok(mk(ExprKind::Name(name), span));
        }
        Ok(match name {
            // Rhai's constants, `PI()` and `E()`.
            "PI" | "E" => {
                if !args.is_empty() {
                    return self.err(p, format!("`{name}()` takes no argument"));
                }
                mk(ExprKind::Predef(name.to_ascii_lowercase()), span)
            }
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
            // a + (b - a) t: linear, so deep zoom derives through it (a
            // continuous morph between two expressions, phase 4, step 8).
            "mix" => {
                if args.len() != 3 {
                    return self.err(p, "`mix(a, b, t)`");
                }
                let t = args.pop().unwrap();
                let b = args.pop().unwrap();
                let a = args.pop().unwrap();
                let d = self.binary(BinOp::Sub, b, a.clone(), p)?;
                let m = self.binary(BinOp::Mul, d, t, p)?;
                self.binary(BinOp::Add, a, m, p)?
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

    /// `vary("name", z)` or `vary("name", z, #{ param: value, ... })`: the
    /// user's variation `name` applied to `z`, its body inlined as a
    /// helper's is -- so deep zoom and the derivative see through it --
    /// its parameters the map's values, or their defaults. The source
    /// comes from what the entry carries, or the user's registered
    /// variations and library (`variation_source`).
    fn vary(&mut self, f: &FnCallExpr, p: Position, scope: &mut Scope) -> R<ast::Expr> {
        const FORM: &str = "`vary(\"name\", z)` or `vary(\"name\", z, #{ param: value })`: a variation of yours applied to a point";
        if !(2..=3).contains(&f.args.len()) {
            return self.err(p, FORM);
        }
        let Expr::StringConstant(vname, _) = &f.args[0] else { return self.err(p, FORM) };
        let vname = vname.to_string();
        let Some(source) = variation_source(&vname) else {
            let builtin = crate::variations::global_registry().get(&vname).is_some_and(|v| v.provenance != crate::provenance::Provenance::Custom);
            return self.err(
                p,
                if builtin {
                    format!("`{vname}` is a built-in variation: a formula calls the variations you write in Rhai (a built-in's is a later step)")
                } else {
                    format!("no variation of yours is named `{vname}`: use one as a variation, or keep its file in your library")
                },
            );
        };
        // Checked whole first: an error inside it is reported as its own.
        let prog = match transformation(&source) {
            Ok(prog) => prog,
            Err(d) => {
                let first = d.iter().find(|x| x.severity == super::Severity::Error).map(|x| x.render(&source)).unwrap_or_default();
                return self.err(p, format!("the variation `{vname}` does not compile: {first}"));
            }
        };
        let Ok((ast, _)) = parse(&source) else { return self.err(p, format!("the variation `{vname}` does not parse")) };
        let fns: HashMap<String, ScriptFuncDef> = ast.iter_fn_def().map(|d| (d.name.to_string(), (**d).clone())).collect();
        let Some(def) = fns.get("variation").cloned() else { return self.err(p, format!("`{vname}` is not a variation")) };
        if fns.contains_key("init") {
            return self.err(p, format!("`{vname}` keeps state (`init`): a flame's; inside a formula, a later step"));
        }
        // A body ending `#{ z, color }` gives its point here: the colour
        // is a flame's.
        let def = match def.body.statements().last() {
            Some(Stmt::Expr(e)) if matches!(&**e, Expr::Map(..)) => {
                let Expr::Map(m, _) = &**e else { unreachable!() };
                let Some((_, z)) = m.0.iter().find(|(k, _)| k.name.as_str() == "z") else {
                    return self.err(p, format!("`{vname}` ends with a map without `z`"));
                };
                let mut stmts: Vec<Stmt> = def.body.statements().to_vec();
                *stmts.last_mut().expect("a last statement") = Stmt::Expr(Box::new(z.clone()));
                let mut d = def.clone();
                d.body = StmtBlock::new(stmts, def.body.position(), def.body.end_position());
                d
            }
            _ => def,
        };
        // Its parameters: the map's values (lowered here, in the caller's
        // terms), the rest their defaults.
        let span = self.pos.span(p);
        let mut values: HashMap<String, ast::Expr> = HashMap::new();
        if let Some(arg) = f.args.get(2) {
            let Expr::Map(m, _) = arg else { return self.err(arg.position(), "the parameters are a map, `#{ name: value }`") };
            for (k, v) in &m.0 {
                if !prog.params.iter().any(|x| x.name == k.name.as_str()) {
                    let all: Vec<String> = prog.params.iter().map(|x| format!("`{}`", x.name)).collect();
                    let has = if all.is_empty() { "none".to_string() } else { all.join(", ") };
                    return self.err(k.pos, format!("`{vname}` has no parameter `{}`; it has {has}", k.name));
                }
                let value = self.expr(v, scope)?;
                values.insert(k.name.to_string(), value);
            }
        }
        for x in &prog.params {
            values.entry(x.name.clone()).or_insert_with(|| {
                let (re, im) = x.default;
                let kind = match x.kind {
                    super::ir::ParamKind::Complex => ExprKind::Complex(Box::new(mk(ExprKind::Float(re), span)), Box::new(mk(ExprKind::Float(im), span))),
                    super::ir::ParamKind::Float => ExprKind::Float(re),
                    super::ir::ParamKind::Int | super::ir::ParamKind::Enum => ExprKind::Int(re as i64),
                    super::ir::ParamKind::Bool => ExprKind::Bool(re != 0.0),
                };
                mk(kind, span)
            });
        }
        let point = self.expr(&f.args[1], scope)?;
        if self.depth > 32 {
            return self.err(p, "variations call each other too deep (recursion?)");
        }
        // Its body in its own functions, every position at the call.
        let mut inner = Scope::new(Fn3::Helper, Vec::new());
        let point = if self.pure {
            Binding::Value(point)
        } else {
            // A local, so the body may reassign its point.
            let name = self.local(&def.params[0], TypeName::Complex);
            self.pending.push(ast::Stmt::Decl { ty: TypeName::Complex, name: name.clone(), init: Some(point), dims: None, span });
            Binding::Local(name)
        };
        inner.bind(&def.params[0], point);
        if let Some(pp) = def.params.get(1) {
            inner.bind(pp, Binding::ParamMap(values, "one of the variation's parameters"));
        }
        // Its context inside a formula: no earlier variations, weight 1.
        if let Some(cp) = def.params.get(2) {
            let zero = mk(ExprKind::Complex(Box::new(mk(ExprKind::Float(0.0), span)), Box::new(mk(ExprKind::Float(0.0), span))), span);
            let ctx: HashMap<String, ast::Expr> = [("accum".to_string(), zero), ("weight".to_string(), mk(ExprKind::Float(1.0), span))].into_iter().collect();
            inner.bind(cp, Binding::ParamMap(ctx, "in a variation's context (`accum`, `weight`)"));
        }
        let fns = std::mem::replace(&mut self.fns, fns);
        let pos = std::mem::replace(&mut self.pos, Positions::fixed(span));
        let transform = std::mem::replace(&mut self.transform, false);
        let coloring = std::mem::replace(&mut self.coloring, false);
        self.depth += 1;
        let r = if self.pure { self.pure_body(&def.body, &mut inner) } else { self.helper_body(&def.body, &mut inner) };
        self.depth -= 1;
        self.fns = fns;
        self.pos = pos;
        self.transform = transform;
        self.coloring = coloring;
        if !self.variations.iter().any(|(n, _)| *n == vname) {
            self.variations.push((vname, source));
        }
        r.map_err(|mut d| {
            d.message = format!("in the variation: {}", d.message);
            d
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
                "pass" | "maxiter" | "x" | "y" | "width" | "height" | "numiter" | "crease_pattern" => TypeName::Int,
                "magn" | "pi" | "e" | "crease_decay" | "crease_sign" | "crease_reach" | "crease_height" | "draw" | "weight" | "vcolor" | "pixelz" | "accumz" => TypeName::Float,
                _ => TypeName::Complex,
            },
            ExprKind::Param(n) => self.param_tys.get(n).copied().unwrap_or(TypeName::Complex),
            ExprKind::Call { name, args, user } => {
                if *user {
                    return TypeName::Complex;
                }
                match name.as_str() {
                    "to_float" => return TypeName::Float,
                    "to_int" => return TypeName::Int,
                    _ => {}
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
            // An array's element: its elements' type.
            ExprKind::Index(n, _) => self.types.get(n).copied().unwrap_or(TypeName::Complex),
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

/// A lowered constant integer expression's value (an array's length).
fn const_int(e: &ast::Expr) -> Option<i64> {
    match &e.kind {
        ExprKind::Int(v) => Some(*v),
        ExprKind::Binary(BinOp::Add, a, b) => const_int(a)?.checked_add(const_int(b)?),
        ExprKind::Binary(BinOp::Sub, a, b) => const_int(a)?.checked_sub(const_int(b)?),
        ExprKind::Binary(BinOp::Mul, a, b) => const_int(a)?.checked_mul(const_int(b)?),
        ExprKind::Unary(UnOp::Neg, a) => const_int(a).map(|v| -v),
        _ => None,
    }
}

enum Base {
    State,
    Params,
    /// An inlined variation's parameters, or its context.
    Map(HashMap<String, ast::Expr>, &'static str),
    Ctx,
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
        "&" => BinOp::BitAnd,
        "|" => BinOp::BitOr,
        "^" => BinOp::BitXor,
        "<<" => BinOp::Shl,
        ">>" => BinOp::Shr,
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
        // Rhai's conversions, which the checker makes (a bool to a
        // number among them).
        "to_float" => "to_float",
        "to_int" => "to_int",
        "abs" => "abs",
        // A value at the GPU's precision (UF-side `f32`).
        "to_f32" => "f32",
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

    /// Arrays (plan, item 3, step 3b): `prepare`'s tables read as
    /// `p.k[i]`, a literal, `array(n, v)`, a state field indexed and
    /// written, `len`, an element's part (`a[i].re`) and an index that is
    /// a field (`a[s.u]`) -- running as the same formula in UF with
    /// arrays.
    #[test]
    fn rhai_arrays_run_as_their_uf_twin() {
        let r = rh(r#"
const PARAMS = #{ bailout: #{ value: 4.0 }, w: #{ value: 0.01 } };

fn prepare(p) {
    let k = array(4, 0.0);
    for i in 0..4 {
        k[i] = to_float(i) + 1.0;
    }
    #{ k: k, unit: [1.0, 0.0] }
}

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel, hist: array(3, complex(0.0, 0.0)), t: 0, u: 1 }
}

fn step(s, p) {
    s.hist[s.t % 3] = s.z;
    s.t += 1;
    let picks = [p.k[2] / 3.0, p.unit[0]];
    s.z = s.z * s.z + s.c * picks[0] * picks[1] + p.w * s.hist[(s.t + 1) % 3] + 0.001 * p.k[p.k.len() - 1]
        + 0.0001 * (s.hist[s.u].re + p.k[s.u] + s.hist[2].norm_sqr());
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
"#);
        let u = uf("A {\nglobal:\n float k[4]\n int i = 0\n while i < 4\n  k[i] = i + 1\n  i = i + 1\n endwhile\n float unit[2]\n unit[0] = 1\n\
                    init:\n z = 0\n c = #pixel\n complex hist[3]\n int t = 0\n int u = 1\nloop:\n hist[t % 3] = z\n t = t + 1\n float picks[2]\n\
                    picks[0] = k[2] / 3\n picks[1] = unit[0]\n z = z * z + c * picks[0] * picks[1] + @w * hist[(t + 1) % 3] + 0.001 * k[3]\n\
                    z = z + 0.0001 * (real(hist[u]) + k[u] + |hist[2]|)\n\
                    bailout:\n |z| <= @bailout\ndefault:\n float param bailout\n  default = 4\n endparam\n float param w\n  default = 0.01\n endparam\n}");
        same(&r, &u, false);
        let refused = |body: &str, want: &str| {
            let src = format!("fn start(pixel, p) {{ #{{ z: pixel }} }}\nfn step(s, p) {{\n{body}\n s\n}}\nfn escaped(s, p) {{ s.z.norm_sqr() > 4.0 }}");
            let d = program(&src).err().unwrap_or_else(|| panic!("{body}: compiled"));
            let all: Vec<String> = d.iter().map(|x| x.render(&src)).collect();
            assert!(all.iter().any(|m| m.contains(want)), "{body}: {all:?}");
        };
        refused(" let a = [];", "an empty array");
        refused(" let n = 3;\n let a = array(n, 0.0);", "numbers, parameters and `ctx.maxiter`");
        refused(" let a = [[1.0], [2.0]];\n s.z = s.z * a[0][0];", "an array of arrays");
        refused(" let x = 2.0;\n s.z = s.z * x.len();", "an array's length");
        refused(" let x = 2.0;\n s.z = s.z * x[0];", "only an array is indexed");
    }

    /// Rhai's bit operations (plan, item 3, step 3c) on ints as 32-bit
    /// patterns: Origami's hash written in a formula gives the built-in's
    /// (`formulas::origami_hash`) bit for bit, its constants past
    /// `i32::MAX` read as their patterns, `>>` logical; and the masks and
    /// shifts beside it as u32 arithmetic has them.
    #[test]
    fn bit_operations_hash_as_the_builtin() {
        let r = rh(BIT_HASH);
        for x in [0i64, 1, 7, 255, 4096, 65_535, 123_456_789, 2_000_000_000, -1, -77] {
            for y in [0i64, 3, 911] {
                let env = Env::at(C64::new(x as f64, y as f64), 1);
                let got = run(&r, &ParamValues::defaults(&r), None, &env).z;
                let u = (x as u32).wrapping_mul(2_654_435_761).wrapping_add(y as u32);
                let h = crate::escape::formulas::origami_hash(u);
                assert_eq!(got.re, f64::from(h >> 8) / 16_777_216.0, "hash at ({x}, {y})");
                let bits = ((u << 3) | (u & 7)) ^ ((u >> 29) & 0xff);
                assert_eq!(got.im, f64::from(bits & 0xffff), "bits at ({x}, {y})");
            }
        }
        // Bools: `&` and `|` are and and or, `^` xor; a float's bits none.
        let d = program("fn start(pixel, p) { #{ z: pixel } }\nfn step(s, p) { s.z = s.z * to_float(2.5 & 1); s }").err().expect("refused");
        assert!(d.iter().any(|x| x.message.contains("takes two ints")), "{d:?}");
    }

    const BIT_HASH: &str = r#"
fn hash(x) {
    let h = x;
    h = h ^ (h >> 16);
    h = h * 0x7feb352d;
    h = h ^ (h >> 15);
    h = h * 0x846ca68b;
    h ^ (h >> 16)
}

fn start(pixel, p) {
    let x = to_int(pixel.re) * 2654435761 + to_int(pixel.im);
    let bits = ((x << 3) | (x & 7)) ^ ((x >> 29) & 0xff);
    let odd = (hash(x) >> 31) == 1;
    #{ z: complex(to_float(hash(x) >> 8) / 16777216.0, to_float(bits & 0xffff)), flag: odd ^ (x < 0) }
}

fn step(s, p) {
    s
}
"#;

    /// An array sized by a parameter or `ctx.maxiter` (plan, item 3,
    /// step 3f): `array(n, v)` fills it at the size the settings fix, and
    /// `len` is that size (the size as written, so a run at other
    /// settings than its check's reads those).
    #[test]
    fn rhai_arrays_sized_by_the_settings() {
        use super::super::check::{at_settings, SizeSettings};
        let text = "const PARAMS = #{ n: #{ value: 3 } };\nfn start(pixel, p, ctx) {\n let a = array(p.n * 2, 1.5);\n let b = array(ctx.maxiter - 1, 0.0);\n\
                    #{ z: complex(to_float(a.len() + b.len()), a[p.n * 2 - 1]) }\n}\nfn step(s, p) { s }\nfn escaped(s, p) { true }";
        // At the defaults: the panel's 256 iterations, as a render's.
        let z = |p: &Program| run(p, &ParamValues::defaults(p), None, &Env::at(C64::ZERO, 256)).z;
        assert_eq!(z(&rh(text)), C64::new(6.0 + 255.0, 1.5));
        let settings = SizeSettings { params: [("n".to_string(), 5.0)].into_iter().collect(), maxiter: Some(20) };
        let p = at_settings(settings, || rh(text));
        // The values the sizes fold at are the run's too.
        let mut values = ParamValues::defaults(&p);
        values.values[0].0 = 5.0;
        assert_eq!(run(&p, &values, None, &Env::at(C64::ZERO, 20)).z, C64::new(10.0 + 19.0, 1.5));
    }

    /// The crease relief as a formula's function (plan, item 3, step 3d):
    /// `creases(p0, p, ctx)` gives the paper's height and gradient from
    /// the starting point and the panel's settings, as the interpreter
    /// computes them; what it may not read or give is refused.
    #[test]
    fn a_formulas_creases_give_height_and_slope() {
        let base = |creases: &str| {
            format!(
                "const PARAMS = #{{ k: #{{ value: 2.0 }} }};\nfn start(pixel, p) {{ #{{ z: pixel }} }}\nfn step(s, p) {{ s.z = s.z * 0.5; s }}\n{creases}"
            )
        };
        let p = rh(&base(
            "fn creases(p0, p, ctx) {\n let h = p0.re * p.k * ctx.crease_sign + ctx.crease_decay + ctx.crease_reach;\n\
             let g = complex(p.k, to_float(ctx.crease_pattern));\n #{ height: h, slope: g }\n}",
        ));
        assert!(!p.creases.is_empty());
        let (h, g) = super::super::interp::creases(&p, &ParamValues::defaults(&p), &Env::at(C64::ZERO, 10), C64::new(1.5, 0.0), [0.6, 2.0, -1.0, 0.25]);
        assert_eq!((h, g), (1.5 * 2.0 * -1.0 + 0.6 + 0.25, C64::new(2.0, 2.0)));
        let refused = |creases: &str, want: &str| {
            let src = base(creases);
            let d = program(&src).err().unwrap_or_else(|| panic!("{creases}: compiled"));
            let all: Vec<String> = d.iter().map(|x| x.render(&src)).collect();
            assert!(all.iter().any(|m| m.contains(want)), "{creases}: {all:?}");
        };
        refused("fn creases(p0, p) { #{ height: 1.0, tilt: complex(0.0, 0.0) } }", "gives `height` and `slope`");
        refused("fn creases(p0, p) { #{ height: 1.0 } }", "ends with `#{ height");
        refused("fn creases(p0, p, ctx) { #{ height: to_float(ctx.pass), slope: complex(0.0, 0.0) } }", "it has no pass");
        let in_step = "const PARAMS = #{ k: #{ value: 2.0 } };\nfn start(pixel, p) { #{ z: pixel } }\nfn step(s, p, ctx) { s.z = s.z * ctx.crease_decay; s }";
        let d = program(in_step).err().expect("refused");
        assert!(d.iter().any(|x| x.message.contains("belongs to `creases`")), "{d:?}");
    }

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
fn start(pixel, p, ctx) {
    let z0 = if ctx.julia { pixel } else { complex(0.0, 0.0) };
    let c = if ctx.julia { ctx.c } else { pixel };
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
fn step(s, p, ctx) {
    if ctx.pass % 2 == 0 { s.z = s.z ** 2 + s.c; } else { s.z = fold(s.z) ** 2 + s.c; }
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
        let bad = "fn start(pixel, p, ctx) { if ctx.julia { #{ z: pixel, c: pixel } } else { #{ z: pixel } } }\n\
                   fn step(s, p) { s }\nfn escaped(s, p) { true }";
        assert!(program(bad).unwrap_err()[0].message.contains("same fields"));
    }

    fn ucl(src: &str) -> Program {
        let (file, d) = parse_file(src);
        assert!(d.is_empty(), "{d:?}");
        let (p, d) = check(&file.entries[0], EntryKind::Coloring, Dialect::UltraFractal);
        assert!(!super::super::has_errors(&d), "{d:?}");
        p.unwrap()
    }

    /// A Rhai colouring colours as its UF twin beside a formula: the
    /// examples, with state, a solid result and the region.
    #[test]
    fn a_rhai_colouring_colours_as_its_uf_twin() {
        use super::super::ir::Region;
        use super::super::to_rhai::parting_coloring;
        let m = uf("M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| <= 4\n}");
        let text = include_str!("../../../assets/formulas/orbit-trap.rhai");
        assert_eq!(kind_of(text), Some(EntryKind::Coloring));
        let trap = coloring(text).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>()));
        let utrap = ucl(
            "T {\ninit:\n float c = cabs(#z - @point)\nloop:\n float d = cabs(#z - @point)\n if d < c\n  c = d\n endif\n\
             final:\n if c < @width\n  #solid = true\n else\n  #index = sqrt(c) * @scale\n endif\n\
             default:\n param point\n  default = (0, 0)\n endparam\n float param width\n  default = 0\n endparam\n\
             float param scale\n  default = 2\n endparam\n}",
        );
        assert_eq!(parting_coloring(&m, &utrap, &trap, 100), None);
        assert_eq!(trap.state_floats(), 1, "the closest distance alone");
        let text = include_str!("../../../assets/formulas/smooth.rhai");
        let smooth = coloring(text).unwrap();
        assert_eq!(smooth.region, Region::Outside);
        let usmooth = ucl(
            "S(OUTSIDE) {\nfinal:\n float r = cabs(#z)\n #index = (#numiter + 1 - log(log(r)) / log(2)) * @scale\n\
             default:\n float param scale\n  default = 0.02\n endparam\n}",
        );
        assert_eq!(parting_coloring(&m, &usmooth, &smooth, 100), None);
        assert_eq!(kind_of(MANDELBROT), Some(EntryKind::Formula));
        // What a colouring refuses, and where.
        for (src, says) in [
            ("fn finish(s, z, p) { #{ hue: 1.0 } }", "`finish` gives `index` and `solid`"),
            ("fn finish(s, z, p, ctx) { to_float(ctx.pass) }", "counts its own passes"),
            ("fn step(s, z, p) { s }", "needs `fn finish`"),
            ("fn finish(s, z, p) { z = complex(1.0, 0.0); 0.0 }", "not a variable to set"),
            ("const INFO = #{ center: complex(0.0, 0.0) };\nfn finish(s, z, p) { 0.0 }", "`title` and `region`"),
            ("const INFO = #{ region: \"left\" };\nfn finish(s, z, p) { 0.0 }", "`region` is"),
            ("fn finish(s, z, p) { let t = 1.0; }", "ends with the index"),
        ] {
            let err = coloring(src).err().unwrap_or_else(|| panic!("{src}: compiled"));
            let text: Vec<String> = err.iter().map(|d| d.message.clone()).collect();
            assert!(text.iter().any(|m| m.contains(says)), "{src}: {text:?}");
        }
    }

    #[test]
    fn what_a_formula_refuses_and_where() {
        let base = |step: &str| format!("fn start(pixel, p) {{ #{{ z: pixel }} }}\nfn step(s, p, ctx) {{\n{step}\n}}\nfn escaped(s, p) {{ s.z.norm_sqr() > 4.0 }}");
        for (step, says) in [
            ("let q = 1 / 2; s.z = s.z * q; s", "two ints divided"),
            ("s.w = s.z; s", "`w` is not in the state"),
            ("s.z = s.z * p.power; s", "`power` is not in `PARAMS`"),
            ("s.z = gamma(s.z); s", "unknown function `gamma`"),
            ("while true { break; } s", "no `break`"),
            ("let a = []; s", "an empty array has no length"),
            ("s.z = \"x\"; s", "a string is metadata"),
            ("s.z = s.z + 1.0;", "ends with the state"),
            ("s.z = q; s", "`q` is not declared"),
            ("s.z = s.z * to_float(ctx.numiter); s", "`ctx.numiter` is a colouring's"),
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
