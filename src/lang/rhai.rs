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
//! - In the effect-like functions (an effect's, a simulation's colouring
//!   and model), whose code is the GPU's alone: `break` and `continue`
//!   in loops (`continue` runs a `for`'s step first; not in a `do` loop)
//!   and an early `return x;`, which sets the result as the last value
//!   does (`put_result`). A helper or an `if` value is a function of its
//!   own: no loop of its caller's to leave, nothing to return from.
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
//! Two ints divided truncate, as Rhai's do. `u32(x)` is an unsigned
//! 32-bit int, as WGSL's (an int's bits, a float truncated and clamped):
//! `+ - *` wrap, `/ %` and comparisons unsigned, `to_float` 0..2^32-1;
//! one joins an int to a u32, and a u32 is an int again only by
//! `to_int`. Functions (and as methods, `z.sqr()`):
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
//! is `"inside"`, `"outside"` or `"both"`, `INFO.bounded` clamps the
//! value rather than wrapping it, and `INFO.bailout` and `INFO.pick` are
//! what picking it sets. A state field may be called
//! `z`: in a colouring that name is the colouring's own.
//!
//! UF and Fractint formulas and colourings convert to these shapes
//! ([`super::to_rhai`]).

use super::ast::{self, BinOp, Entry, ExprKind, FuncBlock, ParamBlock, Section, SectionKind, Setting, SettingValue, Settings, TypeName, UnOp};
use super::builtins::Builtin;
use super::ir::{Cycle, Program, Ty};
use super::{Diag, EntryKind, Span};
use ::rhai::{ASTFlags, Engine, Expr, FlowControl, FnCallExpr, OptimizationLevel, Position, ScriptFuncDef, Stmt, StmtBlock};
use std::collections::HashMap;

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

/// An effect (P2): `fn effect(c, p, ctx)`, per pixel of an image.
pub fn effect(source: &str) -> Result<Program, Vec<Diag>> {
    compile(source, EntryKind::Effect)
}

/// Which function of a simulation model a lowering takes (P4): its seed,
/// or pass `n` of its step (0 for `cell_step`, 1 for `cell_step2` ...);
/// an agent's seed, or pass `n` of its move (P4 step 9d: `agent_seed`,
/// `agent_step`, `agent_step2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimFn {
    Seed,
    Step(u8),
    AgentSeed,
    Agent(u8),
}

impl SimFn {
    /// The Rhai function it lowers.
    pub fn function(self) -> String {
        match self {
            SimFn::Seed => "cell_seed".to_string(),
            SimFn::Step(0) => "cell_step".to_string(),
            SimFn::Step(n) => format!("cell_step{}", n + 1),
            SimFn::AgentSeed => "agent_seed".to_string(),
            SimFn::Agent(0) => "agent_step".to_string(),
            SimFn::Agent(n) => format!("agent_step{}", n + 1),
        }
    }

    /// A pass of the step, run per cell.
    pub fn cell_step(self) -> bool {
        matches!(self, SimFn::Step(_))
    }

    /// An agent's function, run per agent.
    pub fn agent(self) -> bool {
        matches!(self, SimFn::AgentSeed | SimFn::Agent(_))
    }
}

/// A simulation model's function (P4): its seed, or a pass of its step.
pub fn sim_model(source: &str, f: SimFn) -> Result<Program, Vec<Diag>> {
    let lowered = lower_full(source, EntryKind::SimModel, &[], false, f)?;
    checked(lowered, EntryKind::SimModel)
}

/// How many passes a model's step has: `cell_step`, and each of
/// `cell_step2` ... `cell_step4` it defines in a row.
pub fn sim_model_passes(source: &str) -> u8 {
    (1..4).take_while(|n| has_function(source, &SimFn::Step(*n).function())).count() as u8 + 1
}

/// How many passes a model's agents make (P4 step 9d): `agent_step` and
/// `agent_step2`, in a row; none without `agent_step`.
pub fn sim_agent_passes(source: &str) -> u8 {
    (0..2).take_while(|n| has_function(source, &SimFn::Agent(*n).function())).count() as u8
}

/// A simulation's colouring (P4): `fn cell_color(cell, p, ctx)`, per
/// sample of the field.
pub fn sim_coloring(source: &str) -> Result<Program, Vec<Diag>> {
    compile(source, EntryKind::SimColoring)
}

/// A number field of a script's `INFO` (a solid's `radius`, `depth`).
pub fn info_number(source: &str, key: &str) -> Option<f64> {
    info_field(source, key, |v| match v {
        Expr::FloatConstant(f, _) => Some(**f),
        Expr::IntegerConstant(i, _) => Some(*i as f64),
        _ => None,
    })
}

/// A true-or-false field of a script's `INFO` (a transformation's
/// `resets_orbits`).
pub fn info_bool(source: &str, key: &str) -> Option<bool> {
    info_field(source, key, |v| match v {
        Expr::BoolConstant(b, _) => Some(*b),
        _ => None,
    })
}

/// A string field of a script's `INFO` (a solid's `march`, a
/// transformation's `name`).
pub fn info_string(source: &str, key: &str) -> Option<String> {
    info_field(source, key, |v| match v {
        Expr::StringConstant(s, _) => Some(s.to_string()),
        _ => None,
    })
}

/// A list of strings in a script's `INFO` (a simulation model's
/// `diffusion`); empty when it has none.
pub fn info_strings(source: &str, key: &str) -> Vec<String> {
    info_field(source, key, |v| match v {
        Expr::Array(items, _) => Some(items.iter().filter_map(|i| if let Expr::StringConstant(s, _) = i { Some(s.to_string()) } else { None }).collect()),
        _ => None,
    })
    .unwrap_or_default()
}

/// Whether the script's `INFO` has `key`, whatever its value.
pub fn info_has(source: &str, key: &str) -> bool {
    info_field(source, key, |_| Some(())).is_some()
}

/// A field of the script's `INFO` as JSON (a simulation model's
/// `presets`, P4 step 6): maps, arrays, strings, numbers (an int an int),
/// bools and `()` as null, a literal's negation folded. `None` when
/// absent or holding anything else.
pub fn info_json(source: &str, key: &str) -> Option<serde_json::Value> {
    fn json(e: &Expr) -> Option<serde_json::Value> {
        use serde_json::Value;
        Some(match e {
            Expr::IntegerConstant(i, _) => Value::from(*i),
            Expr::FloatConstant(f, _) => Value::from(**f),
            Expr::StringConstant(s, _) => Value::from(s.as_str()),
            Expr::BoolConstant(b, _) => Value::from(*b),
            Expr::Unit(_) => Value::Null,
            Expr::Array(items, _) => Value::Array(items.iter().map(json).collect::<Option<_>>()?),
            Expr::Map(m, _) => Value::Object(m.0.iter().map(|(k, v)| Some((k.name.to_string(), json(v)?))).collect::<Option<_>>()?),
            Expr::FnCall(f, _) if f.name == "-" && f.args.len() == 1 => match json(&f.args[0])? {
                Value::Number(n) if n.is_i64() => Value::from(-n.as_i64()?),
                Value::Number(n) => Value::from(-n.as_f64()?),
                _ => return None,
            },
            _ => return None,
        })
    }
    info_field(source, key, json)
}

/// A label as written (design rule 8): a string, every language's, or a
/// map of locale to string, `#{ en: "Spiral", ja: "螺旋" }` (a key may be
/// quoted, `"zh-CN"`). `None` for anything else.
pub fn label_of(v: &Expr) -> Option<crate::scripted::Label> {
    match v {
        Expr::StringConstant(s, _) => Some(crate::scripted::Label::text(s.to_string())),
        Expr::Map(m, _) => {
            let mut out = Vec::new();
            for (k, x) in &m.0 {
                let Expr::StringConstant(s, _) = x else { return None };
                out.push((k.name.to_string(), s.to_string()));
            }
            Some(crate::scripted::Label::locales(out))
        }
        _ => None,
    }
}

/// A script's labels (`crate::scripted::Labels`), read from its `INFO`
/// and `PARAMS` as written, for whatever shows them to resolve in the
/// current language when it shows them -- its description and authors
/// from the doc block it opens with where `INFO` has none. `None` when it
/// does not parse; a label it does not write is empty.
pub fn source_labels(source: &str) -> Option<crate::scripted::Labels> {
    let (ast, _) = parse(source).ok()?;
    let mut out = crate::scripted::Labels::default();
    for st in ast.statements() {
        let Stmt::Var(x, _, _) = st else { continue };
        let (ident, value, _) = &**x;
        let Expr::Map(m, _) = value else { continue };
        match ident.name.as_str() {
            "INFO" => {
                let field = |name: &str| m.0.iter().find(|(k, _)| k.name.as_str() == name).map(|(_, v)| v);
                if let Some(t) = field("title").and_then(label_of) {
                    out.title = t;
                }
                if let Some(d) = field("description").and_then(label_of) {
                    out.description = d;
                }
                if let Some(Expr::StringConstant(a, _)) = field("author") {
                    out.author = a.to_string();
                }
            }
            "PARAMS" => {
                for (k, v) in &m.0 {
                    let mut p = crate::scripted::ParamLabels { key: k.name.to_string(), ..Default::default() };
                    if let Expr::Map(fields, _) = v {
                        for (f, x) in &fields.0 {
                            match f.name.as_str() {
                                "caption" => p.caption = label_of(x).unwrap_or_default(),
                                "hint" => p.hint = label_of(x).unwrap_or_default(),
                                "choices" => {
                                    if let Expr::Array(items, _) = x {
                                        p.choices = items.iter().map(|i| label_of(i).unwrap_or_default()).collect();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    out.params.push(p);
                }
            }
            _ => {}
        }
    }
    // The doc block the source opens with (a def's shape: the prose, then
    // `# Authors` and its `- name` lines), where `INFO` gives no
    // description or author: the text the API reads and writes without
    // evaluating Rhai (`variations::docs::leading_block`).
    if let Some(doc) = crate::variations::docs::leading_block(source) {
        if out.description.resolve().is_empty() {
            out.description = crate::scripted::Label::text(doc.description);
        }
        if out.author.is_empty() {
            out.author = doc.authors.join(", ");
        }
    }
    Some(out)
}

/// `src` with the variations it calls (`vary`) filled in from where they
/// are now -- the user's registered variations and library -- so the
/// config that carries it carries them too (the KIFS plan, C1e). A
/// source that is not Rhai, or calls none, comes back as it was.
pub fn with_called_variations(kind: EntryKind, src: crate::config::escape::CustomSource) -> crate::config::escape::CustomSource {
    use crate::config::escape::{CustomDialect, CustomSource};
    if src.dialect != CustomDialect::Rhai {
        return src;
    }
    let called = with_variations(&carried_variations(&src), || variations_called(&src.source, kind));
    let variations = called
        .into_iter()
        .map(|(entry, source)| CustomSource { dialect: CustomDialect::Rhai, entry, source, variations: Vec::new() })
        .collect();
    CustomSource { variations, ..src }
}

/// What `src` carries of the variations it calls, as the lowering's
/// scope takes them.
pub fn carried_variations(src: &crate::config::escape::CustomSource) -> Vec<(String, String)> {
    src.variations.iter().map(|v| (v.entry.clone(), v.source.clone())).collect()
}

/// `source` with `id` in its `INFO` -- first in the map, or a new `INFO`
/// above the first line of code -- unless it has an id already: how a
/// script gets the identity it carries (design rule 7). Nothing else of
/// the text changes.
pub fn with_info_id(source: &str, id: &str) -> String {
    if info_string(source, "id").is_some() {
        return source.to_string();
    }
    if let Ok((ast, pos)) = parse(source) {
        let info = ast.statements().iter().find_map(|st| match st {
            Stmt::Var(x, _, p) if x.0.name.as_str() == "INFO" && matches!(x.1, Expr::Map(..)) => Some(*p),
            _ => None,
        });
        if let Some(p) = info {
            let from = pos.span(p).start.min(source.len());
            if let Some(at) = source[from..].find("#{").map(|i| from + i + 2) {
                return format!("{} id: \"{id}\",{}", &source[..at], &source[at..]);
            }
        }
    }
    // Above the first line that is not a comment or blank, so a header
    // comment stays the header.
    let mut at = 0;
    for line in source.split_inclusive('\n') {
        let t = line.trim();
        if !(t.is_empty() || t.starts_with("//")) {
            break;
        }
        at += line.len();
    }
    format!("{}const INFO = #{{ id: \"{id}\" }};\n{}", &source[..at], &source[at..])
}

/// `source` with an id of its own: a UUID it has is kept; any other id
/// (a copy of a built-in's, `vignette`, or one typed by hand) is replaced
/// by `id`; none, `id` is added ([`with_info_id`]). What the Formula
/// Editor gives one of the user's scripts on Use (design rule 7: the
/// user's are UUIDs).
pub fn with_own_id(source: &str, id: &str) -> String {
    match info_string(source, "id") {
        None => with_info_id(source, id),
        Some(have) if crate::scripted::is_uuid(&have) => source.to_string(),
        Some(_) => {
            // The literal's span, from the parse: its opening quote, then
            // the next quote not escaped.
            let Ok((ast, pos)) = parse(source) else { return source.to_string() };
            let at = ast.statements().iter().find_map(|st| {
                let Stmt::Var(x, _, _) = st else { return None };
                let Expr::Map(m, _) = &x.1 else { return None };
                (x.0.name.as_str() == "INFO").then_some(())?;
                m.0.iter().find(|(k, _)| k.name.as_str() == "id").and_then(|(_, v)| match v {
                    Expr::StringConstant(_, p) => Some(pos.span(*p).start),
                    _ => None,
                })
            });
            let Some(open) = at.and_then(|s| source[s.min(source.len())..].find('"').map(|i| s + i)) else {
                return source.to_string();
            };
            let mut close = None;
            let mut escaped = false;
            for (i, ch) in source[open + 1..].char_indices() {
                match ch {
                    '\\' if !escaped => escaped = true,
                    '"' if !escaped => {
                        close = Some(open + 1 + i);
                        break;
                    }
                    _ => escaped = false,
                }
            }
            match close {
                Some(close) => format!("{}\"{id}\"{}", &source[..open], &source[close + 1..]),
                None => source.to_string(),
            }
        }
    }
}

fn info_field<T>(source: &str, key: &str, read: impl Fn(&Expr) -> Option<T>) -> Option<T> {
    let (ast, _) = parse(source).ok()?;
    ast.statements().iter().find_map(|st| {
        let Stmt::Var(x, _, _) = st else { return None };
        let (ident, value, _) = &**x;
        let Expr::Map(m, _) = value else { return None };
        (ident.name.as_str() == "INFO").then_some(())?;
        m.0.iter().find(|(k, _)| k.name.as_str() == key).and_then(|(_, v)| read(v))
    })
}

/// The program a transformation's 3D function compiles to (`fn
/// variation3(v, p[, ctx])`, the KIFS plan, B2): its point a vector, read
/// from and written to `#pixel` and `#pixelz`.
pub fn transformation3(source: &str) -> Result<Program, Vec<Diag>> {
    let lowered = lower_full(source, EntryKind::Transform, &[], true, SimFn::Step(0))?;
    checked(lowered, EntryKind::Transform)
}

thread_local! {
    /// Lowering a transformation's INVERSE (`fn inverse(z, p, branch)`;
    /// docs/projects/rhai-planners.md, B1) in place of its `variation`.
    static INVERSE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// The program a transformation's inverse compiles to: `fn inverse(z, p,
/// branch)`, the preimage of `z` under `variation` on the branch `branch`
/// (0 to `INFO.inverse_branches - 1`), or `no_preimage()` where there is
/// none -- what the Flame Attractor walks backwards. Lowered as a
/// transformation whose point is `z`, so it runs wherever a variation's
/// body does: the interpreter, and a flame variation's WGSL. Its `branch`
/// arrives as the colour coordinate (`ctx.color`), which an inverse has no
/// other use for; the callers set it (`variations::user_inverse`).
pub fn inverse_transformation(source: &str) -> Result<Program, Vec<Diag>> {
    planner_function(source, "inverse")
}

/// The program a transformation's image gap compiles to: `fn
/// image_gap(z, p)`, a lower bound on how far `z` lies outside the image
/// of `variation` -- 0 or less inside it -- which the Flame Attractor
/// scores a point by where `inverse` has no preimage, as it does the
/// built-ins' unit discs. Lowered as [`inverse_transformation`] is, its
/// number the real part of the point it gives.
pub fn image_gap_transformation(source: &str) -> Result<Program, Vec<Diag>> {
    planner_function(source, "image_gap")
}

/// The program a transformation's 3D inverse compiles to: `fn
/// inverse3(v, p)`, the preimage of `v` under `variation3` -- one branch,
/// as the solid walk's kernels have (B3) -- lowered as `variation3` is.
pub fn inverse_transformation3(source: &str) -> Result<Program, Vec<Diag>> {
    planner_function_in(source, "inverse3", true)
}

fn planner_function(source: &str, entry: &'static str) -> Result<Program, Vec<Diag>> {
    planner_function_in(source, entry, false)
}

fn planner_function_in(source: &str, entry: &'static str, three: bool) -> Result<Program, Vec<Diag>> {
    let previous = INVERSE.with(|c| c.replace(Some(entry)));
    let lowered = lower_full(source, EntryKind::Transform, &[], three, SimFn::Step(0));
    INVERSE.with(|c| c.set(previous));
    checked(lowered?, EntryKind::Transform)
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
    if has("effect") {
        Some(EntryKind::Effect)
    } else if has("cell_color") {
        Some(EntryKind::SimColoring)
    } else if has("cell_step") {
        Some(EntryKind::SimModel)
    } else if has("variation") || has("variation3") {
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
    lower_full(source, kind, extra, false, SimFn::Step(0))
}

/// [`lower_with`]; `three` lowers a transformation's 3D function, `sim_fn`
/// which of a simulation model's.
fn lower_full(source: &str, kind: EntryKind, extra: &[(String, ScriptFuncDef)], three: bool, sim_fn: SimFn) -> Result<Lowered, Vec<Diag>> {
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
        param_vecs: HashMap::new(),
        three,
        inverse: INVERSE.with(|c| c.get()),
        solid: kind == EntryKind::Solid,
        effect: kind == EntryKind::Effect,
        sim_coloring: kind == EntryKind::SimColoring,
        palette_reads: 0,
        state_this: false,
        stmt_call: false,
        texture_reads: 0,
        sim_model: kind == EntryKind::SimModel,
        sim_fn,
        probe: std::cell::Cell::new(0),
        loops: Vec::new(),
        loop_floor: 0,
        returns: None,
        scope_tys: Vec::new(),
        perturbing: false,
        grids: HashMap::new(),
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
    // A colouring's view of the loop (P7 step 2): anywhere, in `finish`,
    // in `step`.
    ("bailout", "bailout"),
    ("degree", "degree"),
    ("escaped", "escaped"),
    ("converged", "converged"),
    ("period", "period"),
    ("escape_fraction", "escape_fraction"),
    ("prev", "prev"),
    ("step_c", "step_c"),
    // The derivative's (batch 4): `#dz` is the perturbation equations'
    // deviation, so the checker calls the orbit's `orbit_dz`.
    ("orbit_dz", "dz"),
    ("has_derivative", "has_derivative"),
    ("render_px", "render_px"),
    ("supersample", "supersample"),
    // A colour colouring's (batch 5): the value after the transfer, in
    // `color`; the view's rotation, (cos, sin).
    ("value", "value"),
    ("rotation", "rotation"),
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

/// A flame variation's emission, a statement (rhai-plugins P7 step 3).
fn is_emit(f: &FnCallExpr) -> bool {
    matches!(f.name.as_str(), "emit" | "emit_offset" | "suppress_main")
}

/// A simulation model's write, a statement (P4 step 9c).
fn is_sim_write(f: &FnCallExpr) -> bool {
    matches!(f.name.as_str(), "write_scratch" | "write_memory")
}

/// An argument that is the entry's parameters, its context or a
/// colouring's sample, passed whole to a helper (P4 step 9c): the helper
/// reads it by its fields as its caller does.
fn passed_whole(e: &Expr, scope: &Scope) -> Option<Binding> {
    let Expr::Variable(v, _, _) = e else { return None };
    match scope.get(v.1.as_str()) {
        Some(b @ (Binding::Params | Binding::ParamMap(..) | Binding::Ctx | Binding::Cell)) => Some(b.clone()),
        _ => None,
    }
}

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
    /// A simulation colouring's sample (P4): its fields read as the
    /// `__cell` built-in.
    Cell,
    /// A local, by its lowered name.
    Local(String),
    /// A value substituted where read: a helper's argument, a `let` in
    /// `escaped`.
    Value(ast::Expr),
    /// An inlined variation's parameters, or its context (`vary`): each
    /// read as the value the call gives it, or its default; the label
    /// says what it is, for a name it lacks.
    ParamMap(HashMap<String, ast::Expr>, &'static str),
    /// A vector local, a float local per part (B1): three for a vector,
    /// four for a quaternion.
    VecLocal(vec::Kind, Vec<String>),
    /// A vector substituted where read: a helper's vector argument.
    VecValue(Box<vec::Vector>),
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
    /// An effect's `effect`: the pixel's colour (P2).
    Effect,
}

impl Fn3 {
    /// Builds its map at the end, its fields new (`start`, `prepare`).
    fn builds(self) -> bool {
        matches!(self, Fn3::Start | Fn3::Prepare)
    }
}

/// How an effect-like function's value becomes its result (P4 step 5):
/// its last value's way, and an early `return`'s.
#[derive(Clone, Debug)]
enum Returns {
    /// An effect: an `rgba`, a `vec3` keeping the input's alpha (the local
    /// named), or `#{ color, blend, amount }`.
    Effect { alpha: String },
    /// A simulation colouring: an `rgba` (its `a` the coverage), a `vec3`,
    /// or `#{ color, coverage }`.
    SimColoring,
    /// An escape colouring's `color` (batch 5): a `vec3`, linear light.
    Color,
    /// A simulation model's function: a vector of four (`result` says so).
    SimModel { result: String },
    /// An inlined body that returns early (rhai-plugins P7 step 1): its
    /// value into the result's locals -- one, or a vector's parts
    /// (`kind`) -- the one's type the join of every value it is given.
    Scope { into: Vec<String>, kind: Option<vec::Kind> },
    /// A variation's own body: the new point, and its colour, into
    /// `#pixel` (and `#pixelz`, `#vcolor`), as its end sets them.
    Variation { three: bool },
    /// A formula's `step`: the state, as its end leaves it.
    State,
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
    /// The vector parameters (B1), each a float parameter per part
    /// (`name_x` ...; a quaternion's `name_w` first).
    param_vecs: HashMap<String, vec::Kind>,
    /// Lowering a transformation's 3D function, `variation3` (B2).
    three: bool,
    /// Lowering a transformation's inverse, `fn inverse(z, p, branch)`
    /// ([`inverse_transformation`]), or its `fn image_gap(z, p)`
    /// ([`image_gap_transformation`]): the function's name.
    inverse: Option<&'static str>,
    /// Lowering a solid (B3): its `z` is a vector of its own, not the
    /// formula's iterate.
    solid: bool,
    /// Lowering an effect (P2): `fn effect`, its reads of the image.
    effect: bool,
    /// Lowering a simulation's colouring (P4): `fn cell_color`, its reads
    /// of the sample.
    sim_coloring: bool,
    /// An escape colouring's palette reads made so far (batch 5): each
    /// call site's slot, read once.
    palette_reads: i64,
    /// A helper called on a variation's state (`this.f(..)`), until
    /// [`Lowerer::call`] or `vcall` takes it.
    state_this: bool,
    /// A statement calling a helper alone (for what it writes), until
    /// [`Lowerer::call`] takes it.
    stmt_call: bool,
    /// Texture reads made so far (batch 6), likewise.
    texture_reads: i64,
    /// Lowering a simulation's model (P4), and which of its functions.
    sim_model: bool,
    sim_fn: SimFn,
    /// How deep [`Lowerer::is_vec`] has looked into helpers, against a
    /// helper that calls itself.
    probe: std::cell::Cell<usize>,
    /// The loops being lowered, innermost last (P4 step 5): each one's
    /// step, which a `continue` runs before it jumps -- a `for`'s counter,
    /// a `while`'s condition's statements -- or `None` for a `do` loop,
    /// whose test follows its body. `loop_floor` is where the function
    /// being lowered starts: a helper or an `if` value cannot leave a
    /// loop outside it.
    loops: Vec<Option<Vec<ast::Stmt>>>,
    loop_floor: usize,
    /// How an early `return` sets the result, while an effect-like
    /// function's own statements are lowered (`body_returning`), or a
    /// scope's (`scoped`); `None` in an `if` value or anything else.
    returns: Option<Returns>,
    /// The types each open scope's `return`s give, joined into its
    /// result's (rhai-plugins P7 step 1).
    scope_tys: Vec<Vec<TypeName>>,
    /// Lowering the perturbation equations, whose emitters take no scope.
    perturbing: bool,
    /// The grids `PARAMS` declares (rhai-plugins P7), by name: how
    /// `p.grid[row].column` finds its cell.
    grids: HashMap<String, GridCells>,
}

/// A grid's cells as the lowering reads them: their names' template, its
/// rows and its columns' keys.
#[derive(Clone, Debug)]
struct GridCells {
    cell: String,
    rows: usize,
    columns: Vec<String>,
}

impl GridCells {
    fn cell(&self, row: usize, column: &str) -> String {
        self.cell.replace("{row}", &row.to_string()).replace("{column}", column)
    }
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

/// Whether a `PARAMS` entry is a grid: `type: "grid"`.
fn is_grid(entries: &[(::rhai::Ident, Expr)]) -> bool {
    entries.iter().any(|(k, x)| k.name == "type" && matches!(x, Expr::StringConstant(s, _) if s.as_str() == "grid"))
}

/// A grid's parts as `PARAMS` writes them (rhai-plugins P7).
struct GridParts<'a> {
    rows: usize,
    cell: String,
    row: String,
    live_rows: Option<String>,
    caption: Option<crate::scripted::Label>,
    show_if: crate::params::layout::ShowIf,
    fill: Option<(crate::params::layout::ShowIf, String)>,
    warn: Option<String>,
    drawn: bool,
    columns: Vec<ColumnParts<'a>>,
}

struct ColumnParts<'a> {
    key: String,
    pos: Position,
    /// Its map, a parameter's settings each cell takes.
    map: &'a Expr,
    value: &'a Expr,
    header: Option<crate::scripted::Label>,
    caption: Option<crate::scripted::Label>,
    whole: bool,
    show_if: crate::params::layout::ShowIf,
}

impl GridParts<'_> {
    fn cell(&self, row: usize, column: &str) -> String {
        self.cell.replace("{row}", &row.to_string()).replace("{column}", column)
    }
}

/// A grid's parts, or what is wrong with it and where.
fn grid_of<'a>(name: &str, entries: &'a [(::rhai::Ident, Expr)]) -> Result<GridParts<'a>, (Option<Position>, String)> {
    use crate::params::layout::ShowIf;
    let mut g = GridParts {
        rows: 0,
        cell: format!("{name}_{{row}}_{{column}}"),
        row: "Row {n}".into(),
        live_rows: None,
        caption: None,
        show_if: ShowIf::default(),
        fill: None,
        warn: None,
        drawn: true,
        columns: Vec::new(),
    };
    let string = |x: &Expr, k: &::rhai::Ident, what: &str| match x {
        Expr::StringConstant(s, _) => Ok(s.to_string()),
        _ => Err((Some(k.pos), format!("`{}` is {what}", k.name))),
    };
    for (k, x) in entries {
        match k.name.as_str() {
            "type" => {}
            "rows" => match x {
                Expr::IntegerConstant(n, _) if (1..=64).contains(n) => g.rows = *n as usize,
                _ => return Err((Some(k.pos), "`rows` is how many, 1 to 64".into())),
            },
            "cell" => g.cell = string(x, k, "the cells' names, `\"s{row}_{column}\"`")?,
            "row" => g.row = string(x, k, "a row's caption, `\"Scale {n}\"`")?,
            "live_rows" => g.live_rows = Some(string(x, k, "the parameter counting the live rows")?),
            "caption" => g.caption = Some(label_of(x).ok_or((Some(k.pos), "`caption` is a string, or a map of locale to string".to_string()))?),
            "show_if" => g.show_if = show_if_of(x).map_err(|m| (Some(k.pos), m))?,
            "fill" => {
                let Expr::Map(m, _) = x else { return Err((Some(k.pos), "`fill` is `#{ when: #{ layout: 1 }, from: \"ladder\" }`".into())) };
                let (mut when, mut from) = (None, None);
                for (fk, fx) in &m.0 {
                    match fk.name.as_str() {
                        "when" => when = Some(show_if_of(fx).map_err(|m| (Some(fk.pos), m))?),
                        "from" => from = Some(string(fx, fk, "the function giving a row's values, `\"ladder\"`")?),
                        o => return Err((Some(fk.pos), format!("`fill` has `when` and `from`, not `{o}`"))),
                    }
                }
                match (when, from) {
                    (Some(w), Some(f)) => g.fill = Some((w, f)),
                    _ => return Err((Some(k.pos), "`fill` needs `when` and `from`".into())),
                }
            }
            "warn" => g.warn = Some(string(x, k, "the function warning of a row, `\"past_reach\"`")?),
            // Another section draws its cells (a model's per-scale warps).
            "drawn" => match x {
                Expr::BoolConstant(b, _) => g.drawn = *b,
                _ => return Err((Some(k.pos), "`drawn` is `true` or `false`".into())),
            },
            "columns" => {
                let Expr::Map(m, _) = x else { return Err((Some(k.pos), "`columns` is a map of each column's settings".into())) };
                for (ck, cx) in &m.0 {
                    let Expr::Map(cm, _) = cx else { return Err((Some(ck.pos), format!("column `{}` is a map of its settings", ck.name))) };
                    let mut c = ColumnParts { key: ck.name.to_string(), pos: ck.pos, map: cx, value: cx, header: None, caption: None, whole: false, show_if: ShowIf::default() };
                    let mut value = None;
                    for (sk, sx) in &cm.0 {
                        match sk.name.as_str() {
                            "value" => value = Some(sx),
                            "header" => c.header = Some(label_of(sx).ok_or((Some(sk.pos), "`header` is a string, or a map of locale to string".to_string()))?),
                            "caption" => c.caption = label_of(sx),
                            "whole" => c.whole = matches!(sx, Expr::BoolConstant(true, _)),
                            "show_if" => c.show_if = show_if_of(sx).map_err(|m| (Some(sk.pos), m))?,
                            _ => {}
                        }
                    }
                    c.value = value.ok_or((Some(ck.pos), format!("column `{}` needs its `value`, one for every row or a list of one a row", ck.name)))?;
                    g.columns.push(c);
                }
            }
            o => return Err((Some(k.pos), format!("a grid has `type`, `rows`, `columns`, `cell`, `row`, `live_rows`, `caption`, `show_if`, `fill`, `warn`, `drawn`; not `{o}`"))),
        }
    }
    if g.rows == 0 {
        return Err((None, format!("the grid `{name}` needs its `rows`")));
    }
    if g.columns.is_empty() {
        return Err((None, format!("the grid `{name}` needs its `columns`")));
    }
    if !g.cell.contains("{row}") || !g.cell.contains("{column}") {
        return Err((None, format!("the grid `{name}`'s `cell` names each cell by `{{row}}` and `{{column}}`")));
    }
    Ok(g)
}

/// A condition, `#{ layout: 1 }` or `#{ averaging: [1, 2] }`: every
/// parameter named at one of its values.
fn show_if_of(x: &Expr) -> Result<crate::params::layout::ShowIf, String> {
    const WHAT: &str = "`show_if` is `#{ layout: 1 }`, or a list of values, `#{ averaging: [1, 2] }`";
    let Expr::Map(m, _) = x else { return Err(WHAT.into()) };
    let number = |e: &Expr| match e {
        Expr::IntegerConstant(v, _) => Some(*v as f32),
        Expr::FloatConstant(v, _) => Some(**v as f32),
        Expr::BoolConstant(b, _) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    };
    let mut out = Vec::new();
    for (k, v) in &m.0 {
        let values = match v {
            Expr::Array(items, _) => items.iter().map(number).collect::<Option<Vec<_>>>(),
            e => number(e).map(|n| vec![n]),
        };
        out.push((k.name.to_string(), values.ok_or(WHAT.to_string())?));
    }
    Ok(crate::params::layout::ShowIf(out))
}

/// A script's grid as its layout has it (rhai-plugins P7): the grid, and
/// its fill's condition and function and its warning's function by the
/// names the script defines them under, which the CPU runs.
pub struct GridSpec {
    pub grid: crate::params::layout::ParamGrid,
    pub fill: Option<(crate::params::layout::ShowIf, String)>,
    pub warn: Option<String>,
}

/// A script's parameter layout: its parameters' conditions and its grids.
pub struct LayoutSpec {
    pub show_if: Vec<(String, crate::params::layout::ShowIf)>,
    pub grids: Vec<GridSpec>,
}

impl LayoutSpec {
    /// The layout a kind draws without CPU functions: its conditions and
    /// its grids (one parameter system, step 10). A grid's fill and
    /// warning are functions the CPU runs, which a simulation's registry
    /// alone does yet: refused elsewhere.
    pub fn without_cpu(self) -> Result<crate::params::layout::ParamLayout, String> {
        if let Some(g) = self.grids.iter().find(|g| g.fill.is_some() || g.warn.is_some()) {
            return Err(format!("the grid `{}`: a fill and a warning run on the CPU, a simulation's alone for now", g.grid.name));
        }
        Ok(crate::params::layout::ParamLayout { show_if: self.show_if, grids: self.grids.into_iter().map(|g| g.grid).collect() })
    }
}

/// A source's layout as a kind without CPU functions draws it
/// ([`LayoutSpec::without_cpu`]); `None` when it has neither conditions
/// nor grids.
pub fn plain_layout(source: &str) -> Result<Option<std::sync::Arc<crate::params::layout::ParamLayout>>, String> {
    param_layout(source)?.map(LayoutSpec::without_cpu).transpose().map(|l| l.map(std::sync::Arc::new))
}

/// A script's parameter layout from its `PARAMS` (rhai-plugins P7): each
/// `show_if`, and each grid -- its cells, live rows, columns, headers,
/// fill and warning -- every name it reads checked; `None` with neither.
pub fn param_layout(source: &str) -> Result<Option<LayoutSpec>, String> {
    use crate::params::layout::{GridColumn, ParamGrid, Text};
    let Ok((ast, _)) = parse(source) else { return Ok(None) };
    let params = ast.statements().iter().find_map(|st| match st {
        Stmt::Var(x, _, _) if x.0.name.as_str() == "PARAMS" => match &x.1 {
            Expr::Map(m, _) => Some(&m.0),
            _ => None,
        },
        _ => None,
    });
    let Some(params) = params else { return Ok(None) };
    let mut names: Vec<String> = Vec::new();
    let mut spec = LayoutSpec { show_if: Vec::new(), grids: Vec::new() };
    for (k, v) in params.iter() {
        let Expr::Map(m, _) = v else {
            names.push(k.name.to_string());
            continue;
        };
        if is_grid(&m.0) {
            let g = grid_of(k.name.as_str(), &m.0).map_err(|(_, msg)| msg)?;
            for r in 0..g.rows {
                for c in &g.columns {
                    names.push(g.cell(r, &c.key));
                }
            }
            let columns = g
                .columns
                .iter()
                .map(|c| GridColumn {
                    key: c.key.clone(),
                    header: Text::Label(c.header.clone().or_else(|| c.caption.clone()).unwrap_or_else(|| crate::scripted::Label::text(c.key.clone()))),
                    whole: c.whole,
                    show_if: c.show_if.clone(),
                })
                .collect();
            let grid = ParamGrid {
                name: k.name.to_string(),
                cell: g.cell.clone(),
                rows: g.rows,
                live_rows: g.live_rows.clone(),
                columns,
                show_if: g.show_if.clone(),
                fill: None,
                warn: None,
                drawn: g.drawn,
            };
            spec.grids.push(GridSpec { grid, fill: g.fill.clone(), warn: g.warn.clone() });
            continue;
        }
        names.push(k.name.to_string());
        for (sk, sx) in &m.0 {
            if sk.name == "show_if" {
                spec.show_if.push((k.name.to_string(), show_if_of(sx)?));
            }
        }
    }
    // Every name it reads is a parameter, every function it runs defined.
    let known = |n: &str| names.iter().any(|m| m == n);
    let mut reads: Vec<(&str, &str)> = spec.show_if.iter().flat_map(|(p, s)| s.reads().map(move |r| (p.as_str(), r))).collect();
    for g in &spec.grids {
        reads.extend(g.grid.show_if.reads().map(|r| (g.grid.name.as_str(), r)));
        reads.extend(g.grid.columns.iter().flat_map(|c| c.show_if.reads().map(|r| (g.grid.name.as_str(), r))));
        reads.extend(g.fill.iter().flat_map(|(w, _)| w.reads().map(|r| (g.grid.name.as_str(), r))));
        reads.extend(g.grid.live_rows.iter().map(|r| (g.grid.name.as_str(), r.as_str())));
        for f in g.fill.iter().map(|(_, f)| f).chain(g.warn.iter()) {
            if !ast.iter_fn_def().any(|d| d.name == f.as_str()) {
                return Err(format!("the grid `{}` runs `fn {f}`, which the script does not define", g.grid.name));
            }
        }
    }
    if let Some((by, name)) = reads.iter().find(|(_, n)| !known(n)) {
        return Err(format!("`{by}` reads `{name}`, which is not a parameter"));
    }
    Ok((!spec.show_if.is_empty() || !spec.grids.is_empty()).then_some(spec))
}

/// Whether a function's body returns before its end (rhai-plugins P7
/// step 1): a `return` anywhere but as its last statement. Inlined, such a
/// body is a scope its `return` leaves.
fn returns_early(b: &StmtBlock) -> bool {
    let stmts = b.statements();
    let Some((last, rest)) = stmts.split_last() else { return false };
    rest.iter().any(has_return) || !matches!(last, Stmt::Return(..)) && has_return(last)
}

/// Whether a statement holds a `return` (not a `throw`).
fn has_return(s: &Stmt) -> bool {
    let block = |b: &StmtBlock| b.statements().iter().any(has_return);
    match s {
        Stmt::Return(_, flags, _) => !flags.intersects(ASTFlags::BREAK),
        Stmt::If(fc, _) => block(&fc.body) || block(&fc.branch),
        Stmt::While(fc, _) | Stmt::Do(fc, _, _) => block(&fc.body),
        Stmt::For(x, _) => block(&x.2.body),
        Stmt::Block(b) => block(b),
        Stmt::Expr(e) => matches!(&**e, Expr::Stmt(b) if block(b)),
        _ => false,
    }
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

/// A simulation's `ctx.seed_lo`, `ctx.seed_hi`, `ctx.step` or a model's
/// `ctx.layer` (P4 step 5): the run's seed as its two 32-bit words, for
/// `pcg`, the step index and the layer -- ints, as the framework's are,
/// so a parity or a row index is whole at any step and a draw at another
/// cell can be the framework's `sim_rand` there.
fn host_int(name: &str, span: Span) -> ast::Expr {
    let k = match name {
        "seed_lo" => 0,
        "seed_hi" => 1,
        "layer" => 3,
        "kernel_radius" => 4,
        "kernel_taps" => 5,
        "exact_discs" => 6,
        "scale_warped" => 7,
        "scratch_on" => 8,
        "memory_count" => 9,
        "periodic" => 10,
        "agent" => 11,
        _ => 2,
    };
    mk(ExprKind::Call { name: "__host_int".into(), args: vec![mk(ExprKind::Int(k), span)], user: false }, span)
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
            // Rhai tells two functions of one name apart by how many
            // parameters they take; the compiler finds a function by its
            // name alone.
            if self.fns.insert(f.name.to_string(), (**f).clone()).is_some() {
                return self.err(f.body.position(), format!("`{}` is defined twice: give each its own name", f.name));
            }
        }
        let what = if self.coloring {
            "a colouring"
        } else if self.transform {
            "a transformation"
        } else if self.effect {
            "an effect"
        } else if self.sim_coloring {
            "a simulation colouring"
        } else if self.sim_model {
            "a simulation model"
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
        } else if self.effect {
            "Effect"
        } else if self.sim_coloring {
            "Colouring"
        } else if self.sim_model {
            "Model"
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
                // Its identity (design rule 7): a UUID for one of the
                // user's, a built-in's name for a shipped one; read apart
                // (`info_string`).
                "id" => match v {
                    Expr::StringConstant(..) => {}
                    _ => return self.err(*p, "`id` is a string: the script's UUID, or a built-in's name"),
                },
                // A string, or one by locale (design rule 8), in the
                // current language when compiled.
                "title" => match label_of(v) {
                    Some(l) => title = l.resolve(),
                    None => return self.err(*p, "`title` is a string, or a map of locale to string"),
                },
                // What a reader of the library is told about it, and who
                // wrote it (P3's metadata; `source_labels`).
                "description" => match label_of(v) {
                    Some(_) => {}
                    None => return self.err(*p, "`description` is a string, or a map of locale to string"),
                },
                "author" => match v {
                    Expr::StringConstant(..) => {}
                    _ => return self.err(*p, "`author` is a string"),
                },
                // The registry name (`variation_name`), read apart.
                "name" if self.transform => match v {
                    Expr::StringConstant(..) => {}
                    _ => return self.err(*p, "`name` is a string"),
                },
                // A flame using it starts its orbits afresh every
                // dispatch (`Feature::ResetsOrbits`; `info_bool`).
                "resets_orbits" if self.transform => match v {
                    Expr::BoolConstant(..) => {}
                    _ => return self.err(*p, "`resets_orbits` is true or false"),
                },
                // It replaces the working point rather than adding into it
                // (`Feature::Replace`): `variations::custom` reads it apart.
                // The most points one call emits (`Feature::PlotEmits`).
                "emits" if self.transform => match v {
                    Expr::IntegerConstant(n, _) if (1..=16).contains(n) => {}
                    _ => return self.err(*p, "`emits` is the most points one call emits, 1 to 16"),
                },
                "replace" if self.transform => match v {
                    Expr::BoolConstant(..) => {}
                    _ => return self.err(*p, "`replace` is true or false: whether it replaces the point it is given, as JWildfire's `pVarTP.x = ...`"),
                },
                // When it runs among a transform's variations, and where the
                // pickers group it: a built-in's own (`variations::custom`
                // reads them apart; `any` a phase the user may move).
                "phase" if self.transform => match v {
                    Expr::StringConstant(s, _) if matches!(s.as_str(), "normal" | "any" | "pre" | "post") => {}
                    _ => return self.err(*p, "`phase` is \"normal\", \"any\" (the user may move it to pre or post), \"pre\" or \"post\""),
                },
                // The parameters a slider commits when its drag ends (a
                // built-in's `slow`): `variations::custom::param_defs`.
                "slow" if self.transform => match v {
                    Expr::Array(items, _) if items.iter().all(|i| matches!(i, Expr::StringConstant(..))) => {}
                    _ => return self.err(*p, "`slow` is a list of the parameters committed when a drag ends: `[\"depth\"]`"),
                },
                "category" if self.transform => match v {
                    Expr::StringConstant(s, _) if crate::variations::VariationCategory::from_api_str(s).to_api_str() == s.as_str() => {}
                    _ => {
                        return self.err(
                            *p,
                            "`category` is \"basic_2d\", \"advanced_2d\", \"depth_3d\", \"rotation_3d\", \"full_3d\", \"only_3d\" or \"plugin\": where the pickers group it",
                        )
                    }
                },
                // JWildfire's z rule (`Feature::AlwaysZ`, `NeverZ`):
                // `variations::custom` reads it apart.
                "z" if self.transform => match v {
                    Expr::StringConstant(s, _) if matches!(s.as_str(), "always" | "preserve_z" | "never") => {}
                    _ => {
                        return self.err(
                            *p,
                            "`z` is \"always\" (the z it returns is kept), \"preserve_z\" (kept when the flame preserves z, else zero) or \"never\" (zero)",
                        )
                    }
                },
                // How many branches its inverse has (`fn inverse(z, p,
                // branch)`): `variations::user_inverse` reads it apart.
                "inverse_branches" if self.transform => match v {
                    Expr::IntegerConstant(n, _) if (1..=crate::variations::user_inverse::MAX_BRANCHES as i64).contains(n) => {}
                    _ => return self.err(*p, format!("`inverse_branches` is how many preimages `fn inverse` picks among: a whole number from 1 to {}", crate::variations::user_inverse::MAX_BRANCHES)),
                },
                o if self.transform => {
                    return self.err(*p, format!("a transformation's `INFO` has `title`, `id`, `description`, `author`, `name`, `resets_orbits`, `replace`, `slow`, `phase`, `category`, `z` and `inverse_branches`, not `{o}`"))
                }
                // An effect's identity (design rule 7: a UUID, or a
                // built-in's name) and its place in the pipeline, read
                // apart (`info_string`).
                "category" if self.effect => match v {
                    Expr::StringConstant(s, _) if matches!(s.as_str(), "color" | "density") => {}
                    _ => return self.err(*p, "`category` is \"color\" (after the tone map) or \"density\" (before it)"),
                },
                o if self.effect => return self.err(*p, format!("an effect's `INFO` has `title`, `id`, `description`, `author` and `category`, not `{o}`")),
                o if self.sim_coloring => return self.err(*p, format!("a simulation colouring's `INFO` has `title`, `id`, `description` and `author`, not `{o}`")),
                // A model's working values (`sim::custom` reads them apart):
                // its dt and the largest it is stable at, a still's step
                // count, its diffusion rates' parameter names, how a live
                // field moves into another grid.
                "dt" | "max_dt" | "steps" if self.sim_model => match v {
                    Expr::FloatConstant(..) | Expr::IntegerConstant(..) => {}
                    _ => return self.err(*p, format!("`{name}` is a number")),
                },
                "diffusion" if self.sim_model => match v {
                    Expr::Array(items, _) if items.iter().all(|i| matches!(i, Expr::StringConstant(..))) => {}
                    _ => return self.err(*p, "`diffusion` is a list of the parameters that are diffusion rates, `[\"diffusion_a\"]`"),
                },
                // The parameters whose every value is a slow step (a kernel's
                // radius, an agent count): the panel commits them when a
                // drag ends, as a built-in's `.slow()` marks it.
                "slow" if self.sim_model => match v {
                    Expr::Array(items, _) if items.iter().all(|i| matches!(i, Expr::StringConstant(..))) => {}
                    _ => return self.err(*p, "`slow` is a list of the parameters that make a step slow, committed when a drag ends: `[\"radius\"]`"),
                },
                "state" if self.sim_model => match v {
                    Expr::StringConstant(s, _) if matches!(s.as_str(), "continuous" | "discrete") => {}
                    _ => return self.err(*p, "`state` is \"continuous\" (interpolated into a new grid) or \"discrete\" (the nearest cell)"),
                },
                // What the panel and the export say of it (P4 step 5).
                // A measure pass (P4 step 9c), read by `sim::custom`.
                "measure" if self.sim_model => match v {
                    Expr::Map(..) => {}
                    _ => return self.err(*p, "`measure` is `#{ param: \"variation\", slices: 2 }`: the parameter that turns the first pass on, and its scratch"),
                },
                // Per-scale warps (P4 step 9c), read by `sim::custom`.
                "scale_warps" if self.sim_model => match v {
                    Expr::Map(..) => {}
                    _ => return self.err(*p, "`scale_warps` is `#{ rows_param: \"scales\", rows: 6 }`: the parameter counting the live rows, and the rows"),
                },
                "time_step" | "settles" | "publishes_signal" if self.sim_model => match v {
                    Expr::BoolConstant(..) => {}
                    _ => return self.err(*p, format!("`{name}` is `true` or `false`")),
                },
                // A model's presets (P4 step 6): read by `sim::custom`.
                "presets" if self.sim_model => match v {
                    Expr::Array(..) => {}
                    _ => return self.err(*p, "`presets` is a list of maps, `[#{ name: \"spots\", params: #{ feed: 0.03 } }]`"),
                },
                // A pass run several times a step (P4 step 9a), read by
                // `sim::custom`.
                "repeat" if self.sim_model => match v {
                    Expr::Map(..) => {}
                    _ => return self.err(*p, "`repeat` is `#{ pass: 2, param: \"relax\" }`: the pass (`cell_step` is 1) and the parameter that counts its runs"),
                },
                // What the built-ins have and a script does not: a reason.
                "agents" if self.sim_model => {
                    return self.err(
                        *p,
                        format!(
                            "`INFO.{name}`: a model's agents are its functions, `fn agent_step(a, p, ctx)`"
                        ),
                    )
                }
                o if self.sim_model => {
                    return self.err(
                        *p,
                        format!(
                            "a simulation model's `INFO` has `title`, `id`, `description`, `author`, `dt`, `max_dt`, `steps`, `diffusion`, `state`, \
                             `time_step`, `settles`, `presets`, `repeat`, `publishes_signal`, `measure`, `scale_warps` and `slow`, not `{o}`"
                        ),
                    )
                }
                // A solid's ball: the set lies within this radius
                // (`info_number`, read apart).
                "radius" if kind == EntryKind::Solid => match v {
                    Expr::FloatConstant(..) | Expr::IntegerConstant(..) => {}
                    _ => return self.err(*p, "`radius` is a number"),
                },
                // Its colour depth's default (`info_number`).
                "depth" if kind == EntryKind::Solid => match v {
                    Expr::IntegerConstant(n, _) if (1..=12).contains(n) => {}
                    _ => return self.err(*p, "`depth` is a whole number from 1 to 12: how many iterations the colourings read"),
                },
                // How it is marched (`info_string`; `IfsDef::march_inside`).
                "march" if kind == EntryKind::Solid => match v {
                    Expr::StringConstant(s, _) if matches!(s.as_str(), "near" | "inside") => {}
                    _ => return self.err(*p, "`march` is \"near\" (a hit within a pixel of the solid) or \"inside\" (a hit only in it)"),
                },
                o if kind == EntryKind::Solid => return self.err(*p, format!("a solid's `INFO` has `title`, `id`, `description`, `author`, `radius`, `depth` and `march`, not `{o}`")),
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
                // A level, clamped to 0..1 rather than wrapped
                // (`ColoringFeature::Bounded`), and what picking the
                // colouring sets: the squared bailout it is best drawn at
                // and the values a fresh pick takes (`recommended_bailout`,
                // `pick_params`). `custom::compile_coloring` reads them
                // apart.
                "bounded" if self.coloring => match v {
                    Expr::BoolConstant(..) => {}
                    _ => return self.err(*p, "`bounded` is true or false: a value from 0 to 1, clamped rather than wrapped through the palette"),
                },
                // `step` sees only the iterates that did not bail out, as
                // UF runs a colouring's `loop:` (`SkipsEscapingIterate`).
                "skips_escaping" if self.coloring => match v {
                    Expr::BoolConstant(..) => {}
                    _ => return self.err(*p, "`skips_escaping` is true or false: whether `step` skips the iterate that escapes, as Ultra Fractal's loop section does"),
                },
                "bailout" if self.coloring => match v {
                    Expr::FloatConstant(f, _) if **f > 0.0 => {}
                    Expr::IntegerConstant(i, _) if *i > 0 => {}
                    _ => return self.err(*p, "`bailout` is a positive number: the squared bailout the colouring is best drawn at, which picking it sets"),
                },
                "pick" if self.coloring => {
                    let Expr::Map(m, _) = v else {
                        return self.err(*p, "`pick` is a map of parameter to value, `#{ deband: 1 }`: what a fresh pick of the colouring sets");
                    };
                    for (k, x) in &m.0 {
                        if !params.iter().any(|(n, _, _)| n.as_str() == k.name.as_str()) {
                            return self.err(k.pos, format!("`pick` sets `{}`, which is not a parameter", k.name));
                        }
                        let number = |e: &Expr| matches!(e, Expr::FloatConstant(..) | Expr::IntegerConstant(..) | Expr::BoolConstant(..));
                        let negated = matches!(x, Expr::FnCall(f, _) if f.name == "-" && f.args.len() == 1 && number(&f.args[0]));
                        if !number(x) && !negated {
                            return self.err(k.pos, "a value `pick` sets is a number, or true or false");
                        }
                    }
                }
                o if self.coloring => {
                    return self.err(
                        *p,
                        format!("a colouring's `INFO` has `title`, `id`, `description`, `author`, `region`, `bounded`, `skips_escaping`, `bailout` and `pick`, not `{o}`"),
                    )
                }
                o => return self.err(*p, format!("`INFO` has `title`, `id`, `description`, `author`, `center`, `magn`, `maxiter`, `angle` and `cycle`, not `{o}`")),
            }
        }
        let span = Span { start: 0, end: self.source.len() };
        let mut sections = Vec::new();
        // A variation's derived values (a built-in's `wgsl_init`): from its
        // parameters alone, on the GPU once per transform as they change.
        if self.transform && self.fns.get("prepare").is_some_and(|f| f.params.len() != 1) {
            let f = &self.fns["prepare"];
            return self.err(f.body.position(), "`prepare(p)`: a variation's values from its parameters, computed once as they change");
        }
        if self.effect && self.fns.contains_key("prepare") {
            let f = &self.fns["prepare"];
            return self.err(f.body.position(), "an effect has no `prepare` yet: its values are computed in `effect`");
        }
        if self.sim_coloring && self.fns.contains_key("prepare") {
            let f = &self.fns["prepare"];
            return self.err(f.body.position(), "a simulation colouring has no `prepare`: its values are computed in `cell_color`");
        }
        if self.sim_model && self.fns.contains_key("prepare") {
            let f = &self.fns["prepare"];
            return self.err(f.body.position(), "a simulation model has no `prepare`: its values are computed in `cell_step`");
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
            EntryKind::Effect => sections.push(self.effect_section(span)?),
            EntryKind::SimColoring => sections.push(self.sim_coloring_section(span)?),
            EntryKind::SimModel => sections.push(self.sim_model_section(span)?),
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
        if let Some(entry) = self.inverse {
            return self.inverse_section(span, fields, entry);
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
        // One that returns early is a scope its `return` leaves, each
        // setting the point as its end does (P7 step 1).
        if returns_early(&f.body) {
            let how = Returns::Variation { three: false };
            let outer = self.returns.replace(how.clone());
            let inner = self.scope_body(&how, &f.body, &mut scope);
            self.returns = outer;
            body.push(ast::Stmt::Scope { body: inner?, span });
            return Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true });
        }
        let stmts = f.body.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(f.body.position(), RESULT) };
        for st in rest {
            self.stmt(st, &mut scope, &mut body)?;
        }
        let (value, color) = match final_value(last) {
            Some(Final::Expr(x)) => self.point2(x, &mut scope)?,
            Some(Final::If(fc, p)) => (self.if_value(fc, p, &mut scope)?, Vec::new()),
            _ => return self.err(last.position(), RESULT),
        };
        let at = self.pos.span(last.position());
        self.set_point2(value, color, at, &mut body);
        Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true })
    }

    /// An effect's function as its section (P2): `effect(c, p[, ctx])`,
    /// `c` the input's colour at the pixel (an `rgba`, read once), `p` the
    /// parameters, `ctx` the pixel's position (`ctx.uv`, 0 to 1), the
    /// image's size and its density. The last value is the output: an
    /// `rgba`; a `vec3`, the input's alpha kept; or `#{ color, blend,
    /// amount }` -- `color` either -- which the template blends over the
    /// input with the blend library's mode `blend` (an int, the order of
    /// `effects::BLEND_MODE_NAMES`) at `amount` (1 when absent).
    fn effect_section(&mut self, span: Span) -> R<Section> {
        let Some(f) = self.fns.get("effect").cloned() else {
            return self.err(Position::NONE, "an effect has `fn effect(c, p)`");
        };
        if f.params.is_empty() || f.params.len() > 3 {
            return self.err(f.body.position(), "`effect(c)`, `effect(c, p)` or `effect(c, p, ctx)`");
        }
        let mut scope = Scope::new(Fn3::Effect, Vec::new());
        let input = self.vec_local(vec::Kind::Rgba, &f.params[0]);
        let mut body = Vec::new();
        for (n, ch) in input.iter().zip(["r", "g", "b", "a"]) {
            let read = mk(ExprKind::Call { name: format!("__sample_{ch}"), args: vec![mk(ExprKind::Predef("pixel".into()), span)], user: false }, span);
            body.push(ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: Some(read), dims: None, span });
        }
        scope.bind(&f.params[0], Binding::VecLocal(vec::Kind::Rgba, input.clone()));
        if let Some(p) = f.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        if let Some(c) = f.params.get(2) {
            scope.bind(c, Binding::Ctx);
        }
        const RESULT: &str = "`effect` ends with the output: an `rgba`, a `vec3` (the input's alpha kept), or `#{ color, blend, amount }`";
        let stmts = f.body.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(f.body.position(), RESULT) };
        let how = Returns::Effect { alpha: input[3].clone() };
        self.body_returning(rest, &how, &mut scope, &mut body)?;
        let Some(f) = final_value(last) else { return self.err(last.position(), RESULT) };
        self.put_result(&how, f, last.position(), &mut scope, &mut body)?;
        Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true })
    }

    /// A simulation colouring's function as its section (P4):
    /// `cell_color(cell[, p[, ctx]])`, `cell` the sample the template
    /// measured at the point (`cell.state`, `cell.gx`, `cell.gy` vectors of
    /// four, `cell.dist` a number, `cell.tensor` and `cell.relief` vectors
    /// of three), `p` the parameters, `ctx` the cell (`ctx.cell`) and the
    /// grid's size. The last value is the output: an `rgba`, its `a` the
    /// coverage (0 lets the background through); a `vec3`, drawn whole; or
    /// `#{ color, coverage }`.
    fn sim_coloring_section(&mut self, span: Span) -> R<Section> {
        let Some(f) = self.fns.get("cell_color").cloned() else {
            return self.err(Position::NONE, "a simulation colouring has `fn cell_color(cell, p)`");
        };
        if f.params.is_empty() || f.params.len() > 3 {
            return self.err(f.body.position(), "`cell_color(cell)`, `cell_color(cell, p)` or `cell_color(cell, p, ctx)`");
        }
        let mut scope = Scope::new(Fn3::Effect, Vec::new());
        scope.bind(&f.params[0], Binding::Cell);
        if let Some(p) = f.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        if let Some(c) = f.params.get(2) {
            scope.bind(c, Binding::Ctx);
        }
        const RESULT: &str = "`cell_color` ends with the output: an `rgba` (its `a` the coverage), a `vec3`, or `#{ color, coverage }`";
        let stmts = f.body.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(f.body.position(), RESULT) };
        let mut body = Vec::new();
        self.body_returning(rest, &Returns::SimColoring, &mut scope, &mut body)?;
        let Some(f) = final_value(last) else { return self.err(last.position(), RESULT) };
        self.put_result(&Returns::SimColoring, f, last.position(), &mut scope, &mut body)?;
        Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true })
    }

    /// A simulation model's function as its section (P4), the one
    /// `sim_fn` names. A step pass, `cell_step(cell[, p[, ctx]])`: `cell`
    /// the cell's state (a vector of four), its neighbours `read(dx, dy)`,
    /// `ctx.dt`, `ctx.step`, the cell and the grid; its last value the next
    /// state, a vector of four. The seed, `cell_seed(inside, noise[, p[,
    /// ctx]])`: whether the init shape covers the cell (1 or 0) and a
    /// uniform draw in [0, 1) from the config's seed; its last value the
    /// first state.
    fn sim_model_section(&mut self, span: Span) -> R<Section> {
        let name = self.sim_fn.function();
        let Some(f) = self.fns.get(&name).cloned() else {
            return self.err(Position::NONE, format!("a simulation model has `fn {name}`"));
        };
        let mut scope = Scope::new(Fn3::Effect, Vec::new());
        let host = |i: i64| mk(ExprKind::Call { name: "__cell".into(), args: vec![mk(ExprKind::Int(i), span)], user: false }, span);
        let rest = match self.sim_fn {
            SimFn::Seed => {
                if f.params.len() < 2 || f.params.len() > 4 {
                    return self.err(f.body.position(), "`cell_seed(inside, noise)`, `cell_seed(inside, noise, p)` or `cell_seed(inside, noise, p, ctx)`");
                }
                scope.bind(&f.params[0], Binding::Value(host(19)));
                scope.bind(&f.params[1], Binding::Value(host(20)));
                2
            }
            SimFn::Step(_) => {
                if f.params.is_empty() || f.params.len() > 3 {
                    return self.err(f.body.position(), format!("`{name}(cell)`, `{name}(cell, p)` or `{name}(cell, p, ctx)`"));
                }
                let state = vec::Vector::new(vec::Kind::Rgba, (0..4).map(host).collect());
                scope.bind(&f.params[0], Binding::VecValue(Box::new(state)));
                1
            }
            // An agent's first record, from its index alone (P4 step 9d).
            SimFn::AgentSeed => {
                if f.params.len() > 2 {
                    return self.err(f.body.position(), "`agent_seed()`, `agent_seed(p)` or `agent_seed(p, ctx)`: `ctx.agent` is the agent's index");
                }
                0
            }
            // An agent's move: its record, `vec4(x, y, heading, state)`.
            SimFn::Agent(_) => {
                if f.params.is_empty() || f.params.len() > 3 {
                    return self.err(f.body.position(), format!("`{name}(a)`, `{name}(a, p)` or `{name}(a, p, ctx)`"));
                }
                let record = vec::Vector::new(vec::Kind::Rgba, (0..4).map(host).collect());
                scope.bind(&f.params[0], Binding::VecValue(Box::new(record)));
                1
            }
        };
        if let Some(p) = f.params.get(rest) {
            scope.bind(p, Binding::Params);
        }
        if let Some(c) = f.params.get(rest + 1) {
            scope.bind(c, Binding::Ctx);
        }
        let result = if self.sim_fn.agent() {
            format!("`{name}` ends with the agent: a vector of four, `vec4(x, y, heading, state)`")
        } else {
            format!("`{name}` ends with the state: a vector of four, `vec4(x, y, z, w)`")
        };
        let stmts = f.body.statements();
        let Some((last, others)) = stmts.split_last() else { return self.err(f.body.position(), result) };
        let mut body = Vec::new();
        let how = Returns::SimModel { result: result.clone() };
        self.body_returning(others, &how, &mut scope, &mut body)?;
        let Some(f) = final_value(last) else { return self.err(last.position(), result) };
        self.put_result(&how, f, last.position(), &mut scope, &mut body)?;
        Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true })
    }

    /// An inlined body that returns early, as a scope (rhai-plugins P7
    /// step 1): its result's locals declared before it, and each `return`
    /// and its end setting them -- a `return` leaves the scope, not the
    /// function it is inlined into. A helper's or a varied variation's
    /// body: no loop outside it to leave. Gives the declarations and the
    /// scope, and the result's locals.
    fn scoped(&mut self, b: &StmtBlock, scope: &mut Scope, kind: Option<vec::Kind>) -> R<(Vec<ast::Stmt>, Vec<String>)> {
        if self.perturbing {
            return self.err(b.position(), "a helper of the perturbation equations returns early: write it without `return`");
        }
        let span = self.pos.span(b.position());
        let into = match kind {
            None => vec![self.local("result", TypeName::Float)],
            Some(k) => self.vec_local(k, "result"),
        };
        let how = Returns::Scope { into: into.clone(), kind };
        let outer = std::mem::take(&mut self.pending);
        let sealed = self.seal();
        self.returns = Some(how.clone());
        self.scope_tys.push(Vec::new());
        scope.blocks.push(HashMap::new());
        let r = self.scope_body(&how, b, scope);
        scope.blocks.pop();
        let tys = self.scope_tys.pop().unwrap_or_default();
        self.unseal(sealed);
        self.pending = outer;
        let body = r?;
        let mut decls = Vec::new();
        for n in &into {
            let ty = match kind {
                None => tys.iter().copied().reduce(join).unwrap_or(TypeName::Float),
                Some(_) => TypeName::Float,
            };
            self.types.insert(n.clone(), ty);
            decls.push(ast::Stmt::Decl { ty, name: n.clone(), init: None, dims: None, span });
        }
        decls.push(ast::Stmt::Scope { body, span });
        Ok((decls, into))
    }

    /// A function's statements, then its last value set as `how` says (an
    /// early `return` sets it the same way).
    fn scope_body(&mut self, how: &Returns, b: &StmtBlock, scope: &mut Scope) -> R<Vec<ast::Stmt>> {
        let stmts = b.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(b.position(), "the function ends with its value") };
        let mut body = Vec::new();
        for st in rest {
            self.stmt(st, scope, &mut body)?;
        }
        let Some(f) = final_value(last) else { return self.err(last.position(), "the function ends with its value") };
        self.put_result(how, f, last.position(), scope, &mut body)?;
        Ok(body)
    }

    /// A scope's value into its result's locals.
    fn scope_result(&mut self, into: &[String], kind: Option<vec::Kind>, f: Final, p: Position, scope: &mut Scope, body: &mut Vec<ast::Stmt>) -> R<()> {
        // An `if` that ends it: each arm sets the result itself, so an arm
        // may `return` too.
        if let Final::If(fc, ip) = f {
            return self.returns_if(&Returns::Scope { into: into.to_vec(), kind }, fc, ip, scope, body);
        }
        let at = self.pos.span(p);
        match kind {
            None => {
                let v = self.final_expr(f, scope)?;
                self.flush(body);
                let ty = self.ty_of(&v);
                if let Some(t) = self.scope_tys.last_mut() {
                    t.push(ty);
                }
                body.push(assign(mk(ExprKind::Name(into[0].clone()), at), v, at));
            }
            Some(k) => {
                let v = self.final_vec(f, scope)?;
                if v.kind != k {
                    return self.err(p, "each `return` gives the kind of vector the function's end does");
                }
                self.flush(body);
                for (n, part) in into.iter().zip(v.parts) {
                    body.push(assign(mk(ExprKind::Name(n.clone()), at), part, at));
                }
            }
        }
        Ok(())
    }

    /// A variation's own body's value, from its end or a `return`.
    fn variation_result(&mut self, three: bool, f: Final, p: Position, scope: &mut Scope, body: &mut Vec<ast::Stmt>) -> R<()> {
        let at = self.pos.span(p);
        match f {
            Final::If(fc, ip) => self.returns_if(&Returns::Variation { three }, fc, ip, scope, body),
            Final::Expr(e) if three => {
                let (value, color) = self.point3(e, p, scope)?;
                self.set_point3(value, color, p, at, body)
            }
            Final::Expr(e) => {
                let (value, color) = self.point2(e, scope)?;
                self.set_point2(value, color, at, body);
                Ok(())
            }
            Final::Call(..) => self.err(p, "`variation` ends with the new point, or `#{ z: point, color: index }`"),
        }
    }

    /// An `if` that ends a scope's body (P7 step 1): each arm's statements,
    /// then its value set as `how` sets it -- so an arm may `return` too.
    fn returns_if(&mut self, how: &Returns, fc: &FlowControl, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
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
            scope.blocks.push(HashMap::new());
            let body = self.scope_body(how, &cur.body, scope);
            scope.blocks.pop();
            arms.push((cond, body?));
            match cur.branch.statements() {
                [Stmt::If(inner, _)] => cur = &**inner,
                [] => return self.err(p, "the `if` that ends the function needs its `else`"),
                _ => {
                    scope.blocks.push(HashMap::new());
                    let body = self.scope_body(how, &cur.branch, scope);
                    scope.blocks.pop();
                    otherwise = Some(body?);
                    break;
                }
            }
        }
        out.push(ast::Stmt::If { arms, otherwise, span });
        Ok(())
    }

    /// A result map's entries besides its point: `color` (the colour it
    /// writes) and `hide` (whether it hides the point), as the places
    /// they set.
    fn result_extra(&mut self, k: &str, v: &Expr, kp: Position, scope: &mut Scope) -> R<Vec<(&'static str, ast::Expr)>> {
        match k {
            "color" => Ok(vec![("vcolor", self.expr(v, scope)?)]),
            "hide" => Ok(vec![("vhide", self.expr(v, scope)?)]),
            // The 4th coordinate a 3D body gives, raw (`NeedsW`).
            "w" => Ok(vec![("wout", self.expr(v, scope)?)]),
            // A direct colour, a vector (`WritesRgb`): each part.
            "rgb" => {
                let c = self.vexpr(v, scope)?;
                if c.kind != vec::Kind::Vec3 {
                    return self.err(v.position(), "`rgb` is a colour, `vec3(r, g, b)`");
                }
                Ok(["vrgb_r", "vrgb_g", "vrgb_b"].into_iter().zip(c.parts).collect())
            }
            o => self.err(kp, format!("a variation's result has `z`, `color`, `rgb`, `hide` and (in 3D) `w`, not `{o}`")),
        }
    }

    /// A 2D variation's value: the new point, or `#{ z, color, hide }`.
    fn point2(&mut self, e: &Expr, scope: &mut Scope) -> R<(ast::Expr, Vec<(&'static str, ast::Expr)>)> {
        let Expr::Map(m, mp) = e else { return Ok((self.expr(e, scope)?, Vec::new())) };
        let (mut point, mut extra) = (None, Vec::new());
        for (k, v) in &m.0 {
            match k.name.as_str() {
                "z" => point = Some(self.expr(v, scope)?),
                o => extra.extend(self.result_extra(o, v, k.pos, scope)?),
            }
        }
        match point {
            Some(v) => Ok((v, extra)),
            None => self.err(*mp, "the result needs `z`, the new point"),
        }
    }

    /// A 2D variation's point, colour and hide flag, set.
    fn set_point2(&mut self, value: ast::Expr, extra: Vec<(&'static str, ast::Expr)>, at: Span, body: &mut Vec<ast::Stmt>) {
        self.flush(body);
        for (place, x) in extra {
            body.push(assign(mk(ExprKind::Predef(place.into()), at), x, at));
        }
        body.push(assign(mk(ExprKind::Predef("pixel".into()), at), value, at));
    }

    /// A 3D variation's value: the new point, a vector, or `#{ z, color, hide }`.
    fn point3(&mut self, e: &Expr, p: Position, scope: &mut Scope) -> R<(vec::Vector, Vec<(&'static str, ast::Expr)>)> {
        let (value, color) = match e {
            Expr::Map(m, mp) => {
                let (mut point, mut extra) = (None, Vec::new());
                for (k, v) in &m.0 {
                    match k.name.as_str() {
                        "z" => point = Some(self.vexpr(v, scope)?),
                        o => extra.extend(self.result_extra(o, v, k.pos, scope)?),
                    }
                }
                match point {
                    Some(v) => (v, extra),
                    None => return self.err(*mp, "the result needs `z`, the new point"),
                }
            }
            x => (self.vexpr(x, scope)?, Vec::new()),
        };
        if value.kind != vec::Kind::Vec3 {
            return self.err(p, "the new point is a vector: a quaternion's vector part is `q.v`");
        }
        Ok((value, color))
    }

    /// A 3D variation's point and colour, set.
    fn set_point3(&mut self, value: vec::Vector, extra: Vec<(&'static str, ast::Expr)>, p: Position, at: Span, body: &mut Vec<ast::Stmt>) -> R<()> {
        self.flush(body);
        for (place, x) in extra {
            body.push(assign(mk(ExprKind::Predef(place.into()), at), x, at));
        }
        let Ok([x, y, z]) = <[ast::Expr; 3]>::try_from(value.parts) else {
            return self.err(p, "`variation3` ends with the new point, a vector, or `#{ z: v, color: index }`");
        };
        body.push(assign(mk(ExprKind::Predef("pixel".into()), at), mk(ExprKind::Complex(Box::new(x), Box::new(y)), at), at));
        body.push(assign(mk(ExprKind::Predef("pixelz".into()), at), z, at));
        Ok(())
    }

    /// An effect-like function's statements but its last (P4 step 5): an
    /// early `return` in them sets the result as `how` says, and returns.
    fn body_returning(&mut self, stmts: &[Stmt], how: &Returns, scope: &mut Scope, body: &mut Vec<ast::Stmt>) -> R<()> {
        self.returns = Some(how.clone());
        let r = stmts.iter().try_for_each(|st| self.stmt(st, scope, body));
        self.returns = None;
        r
    }

    /// Set an effect-like function's result from its value `f` at `p` --
    /// its last, or an early `return`'s -- into the `effect_*` places the
    /// function's end returns (P4 step 5).
    fn put_result(&mut self, how: &Returns, f: Final, p: Position, scope: &mut Scope, body: &mut Vec<ast::Stmt>) -> R<()> {
        match how {
            Returns::Scope { into, kind } => return self.scope_result(into, *kind, f, p, scope, body),
            Returns::Variation { three } => return self.variation_result(*three, f, p, scope, body),
            Returns::State => {
                return match f {
                    Final::Expr(v) => self.state_value(v, scope, body),
                    Final::If(fc, ip) => self.state_if(fc, ip, scope, body),
                    Final::Call(..) => self.err(p, "the function ends with the state: `s`, or a map `#{ z: ... }`"),
                };
            }
            _ => {}
        }
        let at = self.pos.span(p);
        let mut extra: Vec<(&str, ast::Expr)> = Vec::new();
        let parts = match how {
            Returns::Scope { .. } | Returns::Variation { .. } | Returns::State => unreachable!("set above"),
            Returns::Effect { alpha } => {
                let (mut blend, mut amount) = (None, None);
                let color = match f {
                    Final::Expr(Expr::Map(m, mp)) => {
                        let mut color = None;
                        for (k, v) in &m.0 {
                            match k.name.as_str() {
                                "color" => color = Some(self.vexpr(v, scope)?),
                                "blend" => blend = Some(self.expr(v, scope)?),
                                "amount" => amount = Some(self.expr(v, scope)?),
                                o => return self.err(k.pos, format!("an effect's result has `color`, `blend` and `amount`, not `{o}`")),
                            }
                        }
                        match color {
                            Some(c) => c,
                            None => return self.err(*mp, "the result needs `color`, the effect's colour"),
                        }
                    }
                    f => self.final_vec(f, scope)?,
                };
                let parts = match color.kind {
                    vec::Kind::Rgba => color.parts,
                    vec::Kind::Vec3 => {
                        let mut parts = color.parts;
                        parts.push(mk(ExprKind::Name(alpha.clone()), at));
                        parts
                    }
                    vec::Kind::Quat => return self.err(p, "an effect's output is a colour: an `rgba`, or a `vec3`"),
                };
                extra.push(("effect_blend", blend.unwrap_or_else(|| mk(ExprKind::Int(-1), at))));
                extra.push(("effect_amount", amount.unwrap_or_else(|| mk(ExprKind::Float(1.0), at))));
                parts
            }
            Returns::SimColoring => {
                let mut coverage = None;
                let color = match f {
                    Final::Expr(Expr::Map(m, mp)) => {
                        let mut color = None;
                        for (k, v) in &m.0 {
                            match k.name.as_str() {
                                "color" => color = Some(self.vexpr(v, scope)?),
                                "coverage" => coverage = Some(self.expr(v, scope)?),
                                o => return self.err(k.pos, format!("a colouring's result has `color` and `coverage`, not `{o}`")),
                            }
                        }
                        match color {
                            Some(c) => c,
                            None => return self.err(*mp, "the result needs `color`, the cell's colour"),
                        }
                    }
                    f => self.final_vec(f, scope)?,
                };
                let mut parts = match color.kind {
                    vec::Kind::Rgba => color.parts,
                    vec::Kind::Vec3 => {
                        let mut parts = color.parts;
                        parts.push(mk(ExprKind::Float(1.0), at));
                        parts
                    }
                    vec::Kind::Quat => return self.err(p, "a colouring's output is a colour: an `rgba`, or a `vec3`"),
                };
                if let Some(c) = coverage {
                    parts[3] = c;
                }
                parts
            }
            // A `vec3` leaves the coverage alone (opaque); an `rgba` sets
            // it (`DirectAlpha`).
            Returns::Color => {
                let color = self.final_vec(f, scope)?;
                if color.kind == vec::Kind::Quat {
                    return self.err(p, "`color` ends with the colour: a `vec3` in linear light, or an `rgba`, its `a` the coverage");
                }
                color.parts
            }
            Returns::SimModel { result } => {
                let state = self.final_vec(f, scope)?;
                if state.kind != vec::Kind::Rgba {
                    return self.err(p, result.clone());
                }
                state.parts
            }
        };
        self.flush(body);
        let put = |name: &str, value: ast::Expr| assign(mk(ExprKind::Predef(name.into()), at), value, at);
        for (name, part) in ["effect_r", "effect_g", "effect_b", "effect_a"].into_iter().zip(parts) {
            body.push(put(name, part));
        }
        for (name, value) in extra {
            body.push(put(name, value));
        }
        Ok(())
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
        scope.bind(&start.params[0], Binding::VecValue(Box::new(vec::Vector::new(vec::Kind::Vec3, pos.to_vec()))));
        if let Some(p) = start.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        let mut init = Vec::new();
        self.state_body(&start.body, &mut scope, &mut init)?;
        let fields = scope.fields.clone();
        // A vector's or a quaternion's: both have an `x`.
        if !fields.iter().any(|f| *f == vec::part("z", vec::Kind::Vec3, 0)) {
            return self.err(start.body.position(), "the state needs `z`, the iterate, a vector or a quaternion: `#{ z: pos, dr: 1.0 }`");
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
    /// A transformation's inverse as its section: `inverse(z, p,
    /// branch)`, lowered as `variation(z, p)` is, its `branch` an int local
    /// read from the colour coordinate the callers set it in.
    fn inverse_section(&mut self, span: Span, fields: Vec<String>, entry: &'static str) -> R<Section> {
        let gap = entry == "image_gap";
        let Some(f) = self.fns.get(entry).cloned() else {
            return self.err(Position::NONE, if gap { "an image gap is `fn image_gap(z, p)`" } else { "an inverse is `fn inverse(z, p, branch)`" });
        };
        if gap && (f.params.is_empty() || f.params.len() > 2) {
            return self.err(f.body.position(), "`image_gap(z)` or `image_gap(z, p)`");
        }
        if gap && returns_early(&f.body) {
            return self.err(f.body.position(), "`image_gap` is one value: an `if` of values, not a `return`");
        }
        if f.params.is_empty() || f.params.len() > 3 {
            return self.err(f.body.position(), "`inverse(z)`, `inverse(z, p)` or `inverse(z, p, branch)`");
        }
        if !fields.is_empty() {
            return self.err(f.body.position(), "an inverse keeps no state: a variation with `init` has none for the Flame Attractor");
        }
        let mut scope = Scope::new(Fn3::Variation, fields);
        let point = self.local(&f.params[0], TypeName::Complex);
        let mut body = vec![ast::Stmt::Decl { ty: TypeName::Complex, name: point.clone(), init: Some(mk(ExprKind::Predef("pixel".into()), span)), dims: None, span }];
        scope.bind(&f.params[0], Binding::Local(point));
        if let Some(p) = f.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        if let Some(b) = f.params.get(2) {
            let name = self.local(b, TypeName::Int);
            let color = mk(ExprKind::Predef("vcolor".into()), span);
            let round = mk(ExprKind::Call { name: "round".into(), args: vec![color], user: false }, span);
            let branch = mk(ExprKind::Call { name: "to_int".into(), args: vec![round], user: false }, span);
            body.push(ast::Stmt::Decl { ty: TypeName::Int, name: name.clone(), init: Some(branch), dims: None, span });
            scope.bind(b, Binding::Local(name));
        }
        const RESULT: &str = "`inverse` ends with the preimage, or `no_preimage()`";
        if returns_early(&f.body) {
            let how = Returns::Variation { three: false };
            let outer = self.returns.replace(how.clone());
            let inner = self.scope_body(&how, &f.body, &mut scope);
            self.returns = outer;
            body.push(ast::Stmt::Scope { body: inner?, span });
            return Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true });
        }
        let stmts = f.body.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(f.body.position(), RESULT) };
        for st in rest {
            self.stmt(st, &mut scope, &mut body)?;
        }
        let value = match final_value(last) {
            // The gap, a number: the real part of the point it gives.
            Some(Final::Expr(x)) if gap => {
                let g = self.expr(x, &mut scope)?;
                mk(ExprKind::Complex(Box::new(g), Box::new(mk(ExprKind::Float(0.0), span))), span)
            }
            Some(Final::If(fc, p)) if gap => {
                let g = self.if_value(fc, p, &mut scope)?;
                mk(ExprKind::Complex(Box::new(g), Box::new(mk(ExprKind::Float(0.0), span))), span)
            }
            _ if gap => return self.err(last.position(), "`image_gap` ends with the gap, a number"),
            Some(Final::Expr(x)) => {
                let (v, color) = self.point2(x, &mut scope)?;
                if !color.is_empty() {
                    return self.err(last.position(), "an inverse gives a point, not a colour");
                }
                v
            }
            Some(Final::If(fc, p)) => self.if_value(fc, p, &mut scope)?,
            _ => return self.err(last.position(), RESULT),
        };
        let at = self.pos.span(last.position());
        self.set_point2(value, Vec::new(), at, &mut body);
        Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true })
    }

    fn transform_section3(&mut self, span: Span, fields: Vec<String>) -> R<Section> {
        // A 3D inverse, `inverse3(v, p)`, lowers as `variation3` does
        // ([`inverse_transformation3`]): one branch, no context.
        let entry = self.inverse.unwrap_or("variation3");
        let Some(f) = self.fns.get(entry).cloned() else {
            if entry == "inverse3" {
                return self.err(Position::NONE, "a 3D inverse is `fn inverse3(v, p)`");
            }
            return self.err(Position::NONE, "a 3D transformation has `fn variation3(v, p)`");
        };
        if entry == "inverse3" && (f.params.is_empty() || f.params.len() > 2) {
            return self.err(f.body.position(), "`inverse3(v)` or `inverse3(v, p)`");
        }
        if f.params.is_empty() || f.params.len() > 3 {
            return self.err(f.body.position(), "`variation3(v)`, `variation3(v, p)` or `variation3(v, p, ctx)`");
        }
        let mut scope = Scope::new(Fn3::Variation, fields);
        let names = self.vec_local(vec::Kind::Vec3, &f.params[0]);
        let pixel = || mk(ExprKind::Predef("pixel".into()), span);
        let part = |name: &str| mk(ExprKind::Call { name: name.into(), args: vec![pixel()], user: false }, span);
        let inits = [part("real"), part("imag"), mk(ExprKind::Predef("pixelz".into()), span)];
        let mut body: Vec<ast::Stmt> = names
            .iter()
            .zip(inits)
            .map(|(n, init)| ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: Some(init), dims: None, span })
            .collect();
        scope.bind(&f.params[0], Binding::VecLocal(vec::Kind::Vec3, names));
        if let Some(p) = f.params.get(1) {
            scope.bind(p, Binding::Params);
        }
        if let Some(c) = f.params.get(2) {
            scope.bind(c, Binding::Ctx);
        }
        const RESULT: &str = "`variation3` ends with the new point, a vector, or `#{ z: v, color: index }`";
        if returns_early(&f.body) {
            let how = Returns::Variation { three: true };
            let outer = self.returns.replace(how.clone());
            let inner = self.scope_body(&how, &f.body, &mut scope);
            self.returns = outer;
            body.push(ast::Stmt::Scope { body: inner?, span });
            return Ok(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true });
        }
        let stmts = f.body.statements();
        let Some((last, rest)) = stmts.split_last() else { return self.err(f.body.position(), RESULT) };
        for st in rest {
            self.stmt(st, &mut scope, &mut body)?;
        }
        let (value, color) = match final_value(last) {
            Some(Final::Expr(x)) => self.point3(x, last.position(), &mut scope)?,
            _ => return self.err(last.position(), RESULT),
        };
        let at = self.pos.span(last.position());
        self.set_point3(value, color, last.position(), at, &mut body)?;
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
        // A step that returns early is a scope its `return` leaves, the
        // state as the return finds it (P7 step 1).
        if returns_early(&step.body) {
            let how = Returns::State;
            let outer = self.returns.replace(how.clone());
            let inner = self.scope_body(&how, &step.body, &mut scope);
            self.returns = outer;
            lp.push(ast::Stmt::Scope { body: inner?, span });
        } else {
            self.state_body(&step.body, &mut scope, &mut lp)?;
        }
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
            self.perturbing = true;
            let r = self.state_body(&f.body, &mut scope, &mut body);
            self.perturbing = false;
            r?;
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
        // A colour, not a palette position (batch 5, `DirectColor`):
        // `color(s, z, p, ctx)`, beside `finish`, which still gives the
        // value the relief reads and `ctx.value` is after the transfer.
        if let Some(color) = get(self, "color") {
            let mut scope = Scope::new(Fn3::Finish, scope.fields.clone());
            if color.params.len() < 3 || color.params.len() > 4 {
                return self.err(color.body.position(), "`color(s, z, p)` or `color(s, z, p, ctx)`");
            }
            for (name, role) in color.params.iter().zip([Binding::State, Binding::Z, Binding::Params, Binding::Ctx]) {
                scope.bind(name, role);
            }
            const RESULT: &str = "`color` ends with the colour: a `vec3`, in linear light";
            let stmts = color.body.statements();
            let Some((last, rest)) = stmts.split_last() else { return self.err(color.body.position(), RESULT) };
            let mut body = Vec::new();
            self.body_returning(rest, &Returns::Color, &mut scope, &mut body)?;
            let Some(f) = final_value(last) else { return self.err(last.position(), RESULT) };
            self.put_result(&Returns::Color, f, last.position(), &mut scope, &mut body)?;
            sections.push(Section { kind: SectionKind::Transform, span, body, last_is_bailout: false, labelled: true });
        }
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
        if let Expr::Map(m, _) = v {
            if is_grid(&m.0) {
                return self.grid_param(name, &m.0, p, out);
            }
        }
        self.param_cell(name, v, p, out, None)
    }

    /// A grid of parameters (rhai-plugins P7): `rows` rows by its
    /// `columns`, each cell a parameter of its own -- named by `cell`
    /// (`s{row}_{column}`), captioned by `row` (`Scale {n}`) and its
    /// column's caption -- in row order. Its layout is `param_layout`'s.
    fn grid_param(&mut self, name: &str, entries: &[(::rhai::Ident, Expr)], p: Position, out: &mut Settings) -> R<()> {
        let g = grid_of(name, entries).map_err(|(at, msg)| Diag::error(self.pos.span(at.unwrap_or(p)), msg))?;
        for r in 0..g.rows {
            let row = g.row.replace("{n}", &(r + 1).to_string());
            for c in &g.columns {
                let at = match c.value {
                    Expr::Array(items, ip) => match items.get(r) {
                        Some(x) => x,
                        None => return self.err(*ip, format!("`value` lists one value a row: {}", g.rows)),
                    },
                    x => x,
                };
                self.param_cell(&g.cell(r, &c.key), c.map, c.pos, out, Some((at, row.clone())))?;
            }
        }
        self.grids.insert(name.to_string(), GridCells { cell: g.cell.clone(), rows: g.rows, columns: g.columns.iter().map(|c| c.key.clone()).collect() });
        Ok(())
    }

    /// A parameter, or a grid's cell: its column's settings, its row's
    /// value and its row's caption before its column's.
    fn param_cell(&mut self, name: &str, v: &Expr, p: Position, out: &mut Settings, cell: Option<(&Expr, String)>) -> R<()> {
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
                        "value" => value = Some(cell.as_ref().map_or(x, |c| c.0)),
                        "min" | "max" => settings.push(Setting { name: k.name.to_string(), value: SettingValue::Expr(self.literal(x)?), span: kspan }),
                        "caption" | "hint" => match label_of(x) {
                            Some(l) => {
                                let text = match &cell {
                                    Some((_, row)) if k.name == "caption" => format!("{row} {}", l.resolve()),
                                    _ => l.resolve(),
                                };
                                settings.push(Setting { name: k.name.to_string(), value: SettingValue::Strings(vec![text]), span: kspan })
                            }
                            None => return self.err(k.pos, format!("`{}` is a string, or a map of locale to string", k.name)),
                        },
                        // When it shows, and a column's header and whole
                        // numbers: its layout's (`param_layout`).
                        "show_if" => {}
                        "header" | "whole" if cell.is_some() => {}
                        "choices" => match x {
                            Expr::Array(items, _) => {
                                for it in items.iter() {
                                    match label_of(it) {
                                        Some(l) => choices.push(l.resolve()),
                                        None => return self.err(it.position(), "a choice is a string, or a map of locale to string"),
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
                        // Free beyond the slider's range, both bounds given
                        // (a built-in variation's unlimited number).
                        "free" => match x {
                            Expr::BoolConstant(..) => settings.push(Setting { name: k.name.to_string(), value: SettingValue::Expr(self.literal(x)?), span: kspan }),
                            _ => return self.err(k.pos, "`free` is `true` or `false`: whether a typed value may leave the range"),
                        },
                        // What the control is, beyond what the value says:
                        // an angle, in degrees as stored and shown.
                        "type" => match x {
                            Expr::StringConstant(t, _) if t.as_str() == "angle" => {
                                settings.push(Setting { name: "angle".into(), value: SettingValue::Expr(mk(ExprKind::Bool(true), kspan)), span: kspan })
                            }
                            _ => return self.err(k.pos, "`type` is `\"angle\"` (degrees, as stored and shown); a number's other kinds follow its `value`"),
                        },
                        o => {
                            return self.err(
                                k.pos,
                                format!("a parameter has `value`, `min`, `max`, `caption`, `hint`, `choices`, `function`, `exponential`, `free`, `type`, `show_if`; not `{o}`"),
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
            if matches!(fc.name.as_str(), "vec3" | "quat" | "rgba") {
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
                    // A vector: a field per part (B1).
                    if self.is_vec(x, scope) {
                        let v = self.vexpr(x, scope)?;
                        let kind = v.kind;
                        for (i, c) in v.parts.into_iter().enumerate() {
                            values.push((vec::part(k.name.as_str(), kind, i), k.pos, c, true));
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
                // A vector: a float local per part (B1).
                if self.is_vec(value, scope) {
                    let v = self.vexpr(value, scope)?;
                    self.flush(out);
                    let names = self.vec_local(v.kind, &ident.name);
                    for (n, c) in names.iter().zip(v.parts) {
                        out.push(ast::Stmt::Decl { ty: TypeName::Float, name: n.clone(), init: Some(c), dims: None, span });
                    }
                    scope.bind(&ident.name, Binding::VecLocal(v.kind, names));
                    return Ok(());
                }
                let v = self.expr(value, scope)?;
                self.flush(out);
                // A fresh array (`[a, b]`, `array(n, x)`) is the local
                // itself, not copied into one: no other name holds it.
                let fresh = match value {
                    Expr::Array(..) => true,
                    Expr::FnCall(f, _) => f.name.as_str() == "array" && !self.fns.contains_key("array"),
                    _ => false,
                };
                if let (true, ExprKind::Name(n)) = (fresh, &v.kind) {
                    if self.arrays.contains_key(n) {
                        scope.bind(&ident.name, Binding::Local(n.clone()));
                        return Ok(());
                    }
                }
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
                self.loops.push(Some(pre.clone()));
                let body = self.block(&fc.body, scope);
                self.loops.pop();
                let mut body = body?;
                body.extend(pre);
                out.push(ast::Stmt::While { cond, body, span: self.pos.span(*p) });
                Ok(())
            }
            Stmt::Do(fc, flags, p) => {
                self.loops.push(None);
                let body = self.block(&fc.body, scope);
                self.loops.pop();
                let mut body = body?;
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
                let i = || mk(ExprKind::Name(name.clone()), span);
                let step = assign(i(), mk(ExprKind::Binary(BinOp::Add, Box::new(i()), Box::new(mk(ExprKind::Int(1), span))), span), span);
                self.loops.push(Some(vec![step.clone()]));
                let body = self.block(&fc.body, scope);
                self.loops.pop();
                scope.blocks.pop();
                let mut body = body?;
                body.push(step);
                let op = if inclusive { BinOp::Le } else { BinOp::Lt };
                out.push(ast::Stmt::While { cond: mk(ExprKind::Binary(op, Box::new(i()), Box::new(mk(ExprKind::Name(bound), span))), span), body, span });
                Ok(())
            }
            Stmt::Block(b) => {
                let body = self.block(b, scope)?;
                out.extend(body);
                Ok(())
            }
            // A variation's emissions (`Feature::PlotEmits`): statements.
            Stmt::FnCall(f, p) if self.transform && is_emit(f) && !self.fns.contains_key(f.name.as_str()) => self.emit_call(f, *p, scope, out),
            Stmt::Expr(e) if self.transform && matches!(&**e, Expr::FnCall(f, _) if is_emit(f) && !self.fns.contains_key(f.name.as_str())) => {
                let Expr::FnCall(f, p) = &**e else { unreachable!("matched above") };
                self.emit_call(f, *p, scope, out)
            }
            // A simulation model's writes (P4 step 9c): the measure pass's
            // scratch, the memory -- statements, a vector of four each.
            Stmt::FnCall(f, p) if self.sim_model && is_sim_write(f) => self.sim_write(f, *p, scope, out),
            // An agent's claim and deposit (P4 step 9d): statements.
            Stmt::FnCall(f, p) if self.sim_model && self.is_agent_write(f) => self.agent_write(f, *p, scope, out),
            Stmt::Expr(e) if self.sim_model && matches!(&**e, Expr::FnCall(f, _) if self.is_agent_write(f)) => {
                let Expr::FnCall(f, p) = &**e else { unreachable!("matched above") };
                self.agent_write(f, *p, scope, out)
            }
            Stmt::Expr(e) if self.sim_model && matches!(&**e, Expr::FnCall(f, _) if is_sim_write(f)) => {
                let Expr::FnCall(f, p) = &**e else { unreachable!("matched above") };
                self.sim_write(f, *p, scope, out)
            }
            // A loop's flow: the effect-like functions' since P4 step 5,
            // every kind's since P7 step 1 (the interpreter runs them too).
            Stmt::BreakLoop(value, flags, p) => {
                let brk = flags.intersects(ASTFlags::BREAK);
                let word = if brk { "break" } else { "continue" };
                if value.is_some() {
                    return self.err(*p, format!("`{word}` takes no value here"));
                }
                if self.loops.len() <= self.loop_floor {
                    return self.err(*p, format!("`{word}` outside a loop: in a helper or an `if` value, only the helper's own loops"));
                }
                let span = self.pos.span(*p);
                self.flush(out);
                if brk {
                    out.push(ast::Stmt::Break { span });
                    return Ok(());
                }
                let Some(step) = self.loops.last().cloned().flatten() else {
                    return self.err(*p, "`continue` in a `do` loop, whose test follows its body: write it as a `while`");
                };
                out.extend(step);
                out.push(ast::Stmt::Continue { span });
                Ok(())
            }
            Stmt::Return(value, flags, p) => {
                let Some(how) = self.returns.clone().filter(|_| !flags.intersects(ASTFlags::BREAK)) else {
                    return self.err(*p, "`return` ends a function, as its last statement");
                };
                let Some(value) = value else { return self.err(*p, "`return` gives the function's result: `return x;`") };
                self.put_result(&how, Final::Expr(value), *p, scope, out)?;
                out.push(ast::Stmt::Return { span: self.pos.span(*p) });
                Ok(())
            }
            // A helper called for what it does: what it writes through
            // `this` (a method call), its value, if any, unused.
            Stmt::Expr(e) if self.helper_method(e) => {
                self.stmt_call = true;
                let r = self.expr(e, scope);
                self.stmt_call = false;
                r?;
                self.flush(out);
                Ok(())
            }
            Stmt::FnCall(f, p) if self.fns.contains_key(f.name.as_str()) => {
                self.stmt_call = true;
                let r = self.call(f, None, *p, scope);
                self.stmt_call = false;
                r?;
                self.flush(out);
                Ok(())
            }
            Stmt::Expr(e) if matches!(&**e, Expr::FnCall(f, _) if self.fns.contains_key(f.name.as_str())) => {
                let Expr::FnCall(f, p) = &**e else { unreachable!("matched above") };
                self.stmt_call = true;
                let r = self.call(f, None, *p, scope);
                self.stmt_call = false;
                r?;
                self.flush(out);
                Ok(())
            }
            Stmt::Expr(e) => self.err(e.position(), "a value alone does nothing here"),
            Stmt::FnCall(_, p) => self.err(*p, "a call alone does nothing here"),
            other => self.err(other.position(), "not in a formula"),
        }
    }

    /// An agent's `claim(x, y)` or `deposit(x, y, amount)`, which a script
    /// has not defined for itself.
    fn is_agent_write(&self, f: &FnCallExpr) -> bool {
        matches!(f.name.as_str(), "claim" | "deposit") && !self.fns.contains_key(f.name.as_str())
    }

    /// An agent's claim or deposit, a statement (P4 step 9d).
    fn agent_write(&mut self, f: &FnCallExpr, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let call = self.call(f, None, p, scope)?;
        self.flush(out);
        out.push(ast::Stmt::Expr(call));
        Ok(())
    }

    /// `p.grid[row].column` (rhai-plugins P7): at a constant row the cell
    /// by its name, in every kind; at a computed row, a simulation model's,
    /// the row clamped to the grid's, one read at the slot its first row's
    /// and its next's give (`__grid_param`, the checker's).
    fn grid_read(&mut self, g: &GridCells, rhs: &Expr, flags: ASTFlags, scope: &mut Scope, p: Position) -> R<ast::Expr> {
        let span = self.pos.span(p);
        const WANT: &str = "a grid's cell is `p.grid[row].column`";
        // Rhai keeps what follows an index inside it, unbroken.
        let (index, rest) = match rhs {
            Expr::Dot(be, _, _) if !flags.intersects(ASTFlags::BREAK) => (&be.lhs, &be.rhs),
            _ => return self.err(p, WANT),
        };
        let Expr::Property(col, cp) = rest else { return self.err(p, WANT) };
        let col = col.2.as_str();
        if !g.columns.iter().any(|c| c == col) {
            return self.err(*cp, format!("the grid's columns are `{}`; not `{col}`", g.columns.join("`, `")));
        }
        let row = self.expr(index, scope)?;
        if let ExprKind::Int(r) = row.kind {
            if r < 0 || r as usize >= g.rows {
                return self.err(p, format!("the grid's rows are 0 to {}", g.rows - 1));
            }
            return Ok(mk(ExprKind::Param(g.cell(r as usize, col)), span));
        }
        if g.rows == 1 {
            return Ok(mk(ExprKind::Param(g.cell(0, col)), span));
        }
        let name = |r: usize| mk(ExprKind::Str(g.cell(r, col)), span);
        let last = mk(ExprKind::Int(g.rows as i64 - 1), span);
        Ok(mk(ExprKind::Call { name: "__grid_param".into(), args: vec![name(0), name(1), row, last], user: false }, span))
    }

    /// `write_scratch(k, v)`, `write_memory(k, v)`: the slice and a vector
    /// of four, the host's write.
    /// A variation's emission (`Feature::PlotEmits`, its cap `INFO.emits`):
    /// `emit(z)` or `emit(z, w)` an extra plot point of weight `w` (1
    /// without), `emit_offset(d, w)` one offset from where the iteration
    /// finally plots, `suppress_main()` its own plot left out, the
    /// emissions kept. A point is a complex number or a `vec3`.
    fn emit_call(&mut self, f: &FnCallExpr, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let span = self.pos.span(p);
        let call = |name: &str, args: Vec<ast::Expr>| ast::Stmt::Expr(mk(ExprKind::Call { name: name.into(), args, user: false }, span));
        if f.name == "suppress_main" {
            if !f.args.is_empty() {
                return self.err(p, "`suppress_main()` takes nothing");
            }
            self.flush(out);
            out.push(call("__emit_suppress", Vec::new()));
            return Ok(());
        }
        let (z, w) = match &f.args[..] {
            [z] => (z, None),
            [z, w] => (z, Some(w)),
            _ => return self.err(p, format!("`{0}(z)` or `{0}(z, w)`: a point and its weight", f.name)),
        };
        let mut args = if self.is_vec(z, scope) {
            let v = self.vexpr(z, scope)?;
            if v.kind != vec::Kind::Vec3 {
                return self.err(p, "a point emitted is a complex number or a `vec3`");
            }
            v.parts
        } else {
            let z = self.expr(z, scope)?;
            let z = self.hold(z, "emit");
            let part = |n: &str| mk(ExprKind::Call { name: n.into(), args: vec![z.clone()], user: false }, span);
            vec![part("real"), part("imag")]
        };
        let three = args.len() == 3;
        args.push(match w {
            Some(w) => self.expr(w, scope)?,
            None => mk(ExprKind::Float(1.0), span),
        });
        self.flush(out);
        let name = match (f.name.as_str() == "emit_offset", three) {
            (false, false) => "__emit_plot2",
            (false, true) => "__emit_plot3",
            (true, false) => "__emit_offset2",
            (true, true) => "__emit_offset3",
        };
        out.push(call(name, args));
        Ok(())
    }

    fn sim_write(&mut self, f: &FnCallExpr, p: Position, scope: &mut Scope, out: &mut Vec<ast::Stmt>) -> R<()> {
        let span = self.pos.span(p);
        if !self.sim_fn.cell_step() {
            return self.err(p, format!("`{}` is a step's, not a seed's or an agent's", f.name));
        }
        let [k, v] = &f.args[..] else { return self.err(p, format!("`{}(k, v)`: the slice and a vector of four", f.name)) };
        let k = self.expr(k, scope)?;
        let v = self.vexpr(v, scope)?;
        if v.kind != vec::Kind::Rgba {
            return self.err(p, format!("`{}(k, v)` writes a vector of four", f.name));
        }
        self.flush(out);
        let name = if f.name == "write_scratch" { "__scratch_write" } else { "__mem_write" };
        let mut args = vec![k];
        args.extend(v.parts);
        out.push(ast::Stmt::Expr(mk(ExprKind::Call { name: name.into(), args, user: false }, span)));
        Ok(())
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
        // Arms giving arrays: an array of their length, each arm setting it
        // whole.
        let len = arms.first().and_then(|(_, _, v)| self.array_of(v)).map(|(_, len)| len);
        if let Some(len) = &len {
            self.arrays.insert(name.clone(), len.clone());
        }
        self.pending.push(ast::Stmt::Decl { ty, name: name.clone(), init: None, dims: len.map(|l| vec![l]), span });
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
        let sealed = self.seal();
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
        self.unseal(sealed);
        self.pending = outer;
        r
    }

    /// Lower a helper's or an `if` value's statements as a function of
    /// their own (P4 step 5): no loop outside them to leave, nothing to
    /// return from. Undone by [`Lowerer::unseal`].
    fn seal(&mut self) -> (usize, Option<Returns>) {
        let outer = (self.loop_floor, self.returns.take());
        self.loop_floor = self.loops.len();
        outer
    }

    fn unseal(&mut self, (floor, returns): (usize, Option<Returns>)) {
        self.loop_floor = floor;
        self.returns = returns;
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
            // The value a helper is called on: the caller's variable.
            Expr::ThisPtr(p) if scope.get("this").is_some() => match scope.get("this") {
                Some(Binding::Local(n)) => Ok(mk(ExprKind::Name(n.clone()), span)),
                _ => self.err(*p, "`this` is a value here, not a variable to set"),
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
        if scope.which != Fn3::Variation && !matches!(scope.get("this"), Some(Binding::State)) {
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
            // A helper called as a method: the value it was called on.
            Expr::ThisPtr(p) => match scope.get("this").cloned() {
                Some(Binding::Local(n)) => mk(ExprKind::Name(n), span),
                Some(Binding::Value(x)) => x,
                _ => return self.err(*p, "`this` is a variation's state, read by its fields (`this.n`), or the value a helper is called on"),
            },
            Expr::Variable(v, _, p) => match scope.get(v.1.as_str()).cloned() {
                Some(Binding::Local(n)) => mk(ExprKind::Name(n), span),
                Some(Binding::Value(x)) => x,
                Some(Binding::Pixel) => mk(ExprKind::Predef("pixel".into()), span),
                Some(Binding::Z) => mk(ExprKind::Predef("z".into()), span),
                Some(Binding::State) => return self.err(*p, "the state is read by its fields, `s.z`"),
                Some(Binding::Params) | Some(Binding::ParamMap(..)) => return self.err(*p, "the parameters are read by name, `p.power`"),
                Some(Binding::Ctx) => return self.err(*p, "the context is read by its fields, `ctx.julia`"),
                Some(Binding::Cell) => return self.err(*p, "a cell is read by its fields, `cell.state.x`"),
                Some(Binding::VecLocal(..) | Binding::VecValue(_)) => return self.err(*p, vec::NUMBER_WANTED),
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
            match scope.get("this") {
                Some(Binding::State) => return self.this_state(*p, scope).map(|_| Base::State),
                Some(_) => return Ok(Base::Value(self.expr(e, scope)?)),
                None => {}
            }
            return self.this_state(*p, scope).map(|_| Base::State);
        }
        if let Expr::Variable(v, _, _) = e {
            match scope.get(v.1.as_str()) {
                Some(Binding::State) => return Ok(Base::State),
                Some(Binding::Params) => return Ok(Base::Params),
                Some(Binding::ParamMap(m, what)) => return Ok(Base::Map(m.clone(), what)),
                Some(Binding::Ctx) => return Ok(Base::Ctx),
                Some(Binding::Cell) => return Ok(Base::Cell),
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
            Base::Cell => Some(vec::Owner::Cell),
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
                // A grid's cell: `p.table[i].radius` (rhai-plugins P7).
                if matches!(base, Base::Params) {
                    if let Expr::Property(prop, _) = &be.lhs {
                        if let Some(g) = self.grids.get(prop.2.as_str()).cloned() {
                            return self.grid_read(&g, &be.rhs, *flags, scope, *p);
                        }
                    }
                }
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
                    // A simulation colouring's sample (P4): its one number;
                    // the vectors are `named_vector`'s.
                    Base::Cell => match name {
                        "dist" => Ok(mk(ExprKind::Call { name: "__cell".into(), args: vec![mk(ExprKind::Int(12), span)], user: false }, span)),
                        o => self.err(
                            *p,
                            format!("a cell has `state`, `gx`, `gy`, `m0`, `m1` (vectors of four: read a part), `dist`, `tensor` and `relief` (vectors of three); not `{o}`"),
                        ),
                    },
                    // A simulation model's (P4): its time step and step
                    // index (host values, `__cell` 21 and 22), the cell, the
                    // grid's size.
                    Base::Ctx if self.sim_model => match name {
                        "dt" => Ok(mk(ExprKind::Call { name: "__cell".into(), args: vec![mk(ExprKind::Int(21), span)], user: false }, span)),
                        // The field's range of `.x`, a step ago (P4 step 9a).
                        "field_min" | "field_max" => {
                            let k = if name == "field_min" { 22 } else { 23 };
                            Ok(mk(ExprKind::Call { name: "__cell".into(), args: vec![mk(ExprKind::Int(k), span)], user: false }, span))
                        }
                        "cell" if self.sim_fn.agent() => self.err(*p, "an agent is at no cell: its position is its record's, `a.x`, `a.y`"),
                        "cell" => Ok(mk(ExprKind::Predef("pixel".into()), span)),
                        "width" | "height" => Ok(mk(ExprKind::Predef(name.into()), span)),
                        "step" | "seed_lo" | "seed_hi" | "layer" => Ok(host_int(name, span)),
                        // Whether the boundary wraps, and an agent's index
                        // (P4 step 9d): ints.
                        "periodic" => Ok(host_int(name, span)),
                        "agent" if self.sim_fn.agent() => Ok(host_int(name, span)),
                        "agent" => self.err(*p, "`ctx.agent` is an agent's index: in `agent_seed` and `agent_step`"),
                        // Its kernel table's size and whether the spectral
                        // stage's discs are in (P4 step 9b): ints.
                        "kernel_radius" | "kernel_taps" | "exact_discs" => Ok(host_int(name, span)),
                        // The warp's per-scale maps, the measure pass, the
                        // memory (P4 step 9c): ints.
                        "scale_warped" | "scratch_on" | "memory_count" => Ok(host_int(name, span)),
                        o => self.err(
                            *p,
                            format!(
                                "a simulation model's context has `dt`, `step`, `cell`, `width`, `height`, `seed_lo`, `seed_hi`, `layer`, \
                                 `field_min`, `field_max`, `kernel_radius`, `kernel_taps`, `exact_discs`, `scale_warped`, `scratch_on`, \
                                 `memory_count`, `periodic` and (an agent's) `agent`; not `{o}`"
                            ),
                        ),
                    },
                    // A simulation colouring's (P4): the cell, the grid's
                    // size.
                    Base::Ctx if self.sim_coloring => match name {
                        "cell" => Ok(mk(ExprKind::Predef("pixel".into()), span)),
                        "width" | "height" => Ok(mk(ExprKind::Predef(name.into()), span)),
                        // The step the run is at, and the seed (P4 step 5).
                        "step" | "seed_lo" | "seed_hi" => Ok(host_int(name, span)),
                        o => self.err(*p, format!("a simulation colouring's context has `cell`, `width`, `height`, `step`, `seed_lo` and `seed_hi`; not `{o}`")),
                    },
                    // An effect's (P2): the pixel's position, the image's
                    // size and density.
                    Base::Ctx if self.effect => match name {
                        "uv" => Ok(mk(ExprKind::Predef("pixel".into()), span)),
                        "width" | "height" | "density_mean" | "pixel_scale" => Ok(mk(ExprKind::Predef(name.into()), span)),
                        o => self.err(*p, format!("an effect's context has `uv`, `width`, `height`, `density_mean` and `pixel_scale`; not `{o}`")),
                    },
                    // A variation's (C2b): what the flame hands it.
                    Base::Ctx if self.transform => match name {
                        "accum" => Ok(mk(ExprKind::Predef("accum".into()), span)),
                        "weight" => Ok(mk(ExprKind::Predef("weight".into()), span)),
                        // The colour coordinate as it reaches the variation
                        // (JWildfire's `pVarTP.color`), before any it writes.
                        "color" => Ok(mk(ExprKind::Predef("vcolor".into()), span)),
                        // Whether the point is hidden as it reaches it (an
                        // earlier variation's `hide`).
                        "hide" => Ok(mk(ExprKind::Predef("vhide".into()), span)),
                        // Its transform's affine coefficients, as the flame
                        // stores them (the editor's and a `.flame`'s `coefs`).
                        "a" | "b" | "c" | "d" | "e" | "f" => Ok(mk(ExprKind::Predef(format!("coef_{name}")), span)),
                        // The 4th coordinate a 3D body is given (`NeedsW`).
                        "w" => Ok(mk(ExprKind::Predef("pointw".into()), span)),
                        // The flame's 3D camera, as the shader holds it:
                        // radians, and the perspective strength.
                        "camera_x" | "camera_y" | "camera_z" | "camera_pitch" | "camera_yaw" | "camera_bank" => {
                            Ok(mk(ExprKind::Predef(format!("cam_{}", &name["camera_".len()..])), span))
                        }
                        "perspective" => Ok(mk(ExprKind::Predef("cam_persp".into()), span)),
                        o => self.err(
                            *p,
                            format!(
                                "a variation's context has `accum`, `weight`, `color`, `hide`, `rgb`, its transform's `a` .. `f`, (in 3D) `w`, and the camera's `camera_x` .. `camera_z`, `camera_pitch`, `camera_yaw`, `camera_bank` and `perspective`; not `{o}`"
                            ),
                        ),
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
                // A helper called on a variation's state: its `this` the
                // state, by reference, as Rhai binds the object.
                Base::State if self.fns.contains_key(f.name.as_str()) => {
                    self.state_this = true;
                    self.call(f, None, *p, scope)
                }
                Base::Map(..) => self.err(*p, "a variation's parameters hold no functions"),
                _ => self.err(*p, "call a function of a value, `s.z.sqr()`"),
            },
            other => self.err(other.position(), "not in a formula"),
        }
    }

    /// A function call, or an operator; `this` is a method call's value.
    fn call(&mut self, f: &FnCallExpr, this: Option<ast::Expr>, p: Position, scope: &mut Scope) -> R<ast::Expr> {
        let span = self.pos.span(p);
        // A statement's call of a helper: its body as statements. Taken
        // here, so a call among its arguments is a value as ever.
        let alone = std::mem::take(&mut self.stmt_call);
        let on_state = std::mem::take(&mut self.state_this);
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
        // A fresh draw in [0, 1) (C2a), unless the script has its own; a
        // simulation model's on a stream of its own as well, `rand(k)`,
        // which an edit elsewhere in the script leaves where it was (P4
        // step 5).
        if name == "rand" && args.is_empty() && !self.fns.contains_key("rand") {
            if self.sim_model && f.args.len() == 1 {
                let k = self.expr(&f.args[0], scope)?;
                return Ok(mk(ExprKind::Call { name: "__sim_rand".into(), args: vec![k], user: false }, span));
            }
            if !f.args.is_empty() {
                let what = if self.sim_model { "`rand()` takes no argument, or a stream's number: `rand(k)`" } else { "`rand()` takes no argument: a draw in [0, 1)" };
                return self.err(p, what);
            }
            if self.sim_coloring {
                return self.err(p, "a simulation colouring has no `rand()`: a picture is the same every frame -- hash the cell and the run's seed (`pcg`, `ctx.seed_lo`)");
            }
            if self.effect {
                return self.err(p, "an effect has no `rand()`: a picture is the same every frame -- hash the pixel's position and a seed parameter");
            }
            return Ok(mk(ExprKind::Predef("draw".into()), span));
        }
        // A flame variation's raw draw: the stream's next 32 bits, a u32.
        if name == "rand_u32" && args.is_empty() && f.args.is_empty() && self.transform && !self.fns.contains_key("rand_u32") {
            return Ok(mk(ExprKind::Predef("draw_u32".into()), span));
        }
        // A simulation model's kernel table at a tap, and the spectral
        // stage's disc average at the cell (P4 step 9b).
        if matches!(name, "tap" | "disc") && args.is_empty() && self.sim_model && !self.fns.contains_key(name) {
            if !self.sim_fn.cell_step() {
                return self.err(p, format!("`{name}(i)` is a step's, at its cell"));
            }
            let [i] = &f.args[..] else { return self.err(p, format!("`{name}(i)` takes the index")) };
            let i = self.expr(i, scope)?;
            return Ok(mk(ExprKind::Call { name: format!("__{name}"), args: vec![i], user: false }, span));
        }
        // A simulation model's framework calls (P4 step 9c): the pyramid's
        // averages, the spectral differences, a grid test, a parameter by
        // its slot, the agents' deposit -- a step's, but `param`.
        if self.sim_model && args.is_empty() && !self.fns.contains_key(name) {
            let host = match (name, f.args.len()) {
                ("pyr_sample", 3) => Some("__pyr_sample"),
                ("pyr_level", 1) => Some("__pyr_level"),
                ("pyr_diff", 3) => Some("__pyr_diff"),
                ("outside", 2) => Some("__outside"),
                ("param", 1) => Some("__param"),
                ("take_deposit", 0) => Some("__take_deposit"),
                // An agent's (P4 step 9d): its claim on the cell (x, y),
                // whether it won it, its deposit there.
                ("claim", 2) => Some("__agent_claim"),
                ("claim_check", 2) => Some("__agent_claim_check"),
                ("deposit", 3) => Some("__agent_deposit"),
                _ => None,
            };
            if let Some(h) = host {
                let agents = matches!(h, "__agent_claim" | "__agent_claim_check" | "__agent_deposit");
                if agents && !self.sim_fn.agent() {
                    return self.err(p, format!("`{name}` is an agent's: in `agent_step`"));
                }
                if !agents && h != "__param" && !self.sim_fn.cell_step() {
                    return self.err(p, format!("`{name}` is a step's, at its cell"));
                }
                let args = f.args.iter().map(|a| self.expr(a, scope)).collect::<R<Vec<_>>>()?;
                return Ok(mk(ExprKind::Call { name: h.into(), args, user: false }, span));
            }
        }
        // A simulation's hash of an int (P4 step 5), the framework's PCG.
        if name == "pcg" && args.is_empty() && (self.sim_coloring || self.sim_model) && !self.fns.contains_key("pcg") {
            let [n] = &f.args[..] else { return self.err(p, "`pcg(n)` takes the int to hash") };
            let n = self.expr(n, scope)?;
            return Ok(mk(ExprKind::Call { name: "__pcg".into(), args: vec![n], user: false }, span));
        }
        // A seed's init shape at another cell (P4 step 5): the cell
        // (x, y), not an offset.
        if name == "init_at" && args.is_empty() && !self.fns.contains_key("init_at") {
            if !self.sim_model || self.sim_fn != SimFn::Seed {
                return self.err(p, "`init_at(x, y)` is a seed's: the init shape at the cell (x, y), in `fn cell_seed` alone");
            }
            if f.args.len() != 2 {
                return self.err(p, "`init_at(x, y)` takes the cell's two coordinates");
            }
            let x = self.expr(&f.args[0], scope)?;
            let y = self.expr(&f.args[1], scope)?;
            let at = mk(ExprKind::Complex(Box::new(x), Box::new(y)), span);
            return Ok(mk(ExprKind::Call { name: "__init_at".into(), args: vec![at], user: false }, span));
        }
        // An inverse's "no preimage here" (docs/projects/rhai-planners.md,
        // B1): the point at 1e30, the sentinel the Flame Attractor's own
        // kernels give, which its walk reads as that piece having escaped.
        if name == "no_preimage" {
            if !f.args.is_empty() {
                return self.err(p, "`no_preimage()` takes nothing");
            }
            if self.inverse != Some("inverse") {
                return self.err(p, "`no_preimage()` is an inverse's: `fn inverse(z, p, branch)`");
            }
            let far = || mk(ExprKind::Float(1e30), span);
            return Ok(mk(ExprKind::Complex(Box::new(far()), Box::new(far())), span));
        }
        // A helper of the script's own, inlined; it reads only its
        // arguments.
        if let Some(def) = self.fns.get(name).cloned() {
            // A method call binds its value to `this`, as Rhai does: a
            // variable by reference, so the helper's writes to `this` are
            // its caller's (a WGSL pointer's way); any other value a
            // temporary, its changes lost.
            let this = args.pop().map(|v| self.this_binding(v, span));
            if matches!(name, "start" | "step" | "escaped" | "finish" | "prepare" | "perturb_start" | "perturb_step" | "perturb_when" | "degree" | "creases" | "variation" | "vary") {
                return self.err(p, format!("`{name}` is the formula's own, not a helper to call"));
            }
            // A vector argument binds as one (B1).
            let mut vals = Vec::new();
            for a in f.args.iter() {
                vals.push(self.helper_arg(a, scope)?);
            }
            if def.params.len() != vals.len() {
                return self.err(p, format!("`{name}` takes {}", def.params.len()));
            }
            if self.depth > 32 {
                return self.err(p, "helpers call each other too deep (recursion?)");
            }
            let mut inner = Scope::new(Fn3::Helper, if on_state { scope.fields.clone() } else { Vec::new() });
            for (param, a) in def.params.iter().zip(vals) {
                inner.bind(param, a);
            }
            if on_state {
                inner.bind("this", Binding::State);
            } else if let Some(this) = this {
                inner.bind("this", this);
            }
            // A helper giving a vector, where a number is wanted.
            if self.block_gives_vec(&def.body, &inner).is_some() {
                return self.err(p, format!("`{name}` gives a vector where a number is wanted: take a part (`.x`), `length` or `dot`"));
            }
            self.depth += 1;
            let r = if alone {
                self.helper_stmts(&def.body, &mut inner).map(|_| mk(ExprKind::Int(0), span))
            } else if self.pure {
                self.pure_body(&def.body, &mut inner)
            } else {
                self.helper_body(&def.body, &mut inner)
            };
            self.depth -= 1;
            return r;
        }
        // Numbers of vectors (`dot`, `length`), and the functions that came
        // with them (`min`, `max`, `clamp` of numbers) (B1).
        if matches!(name, "vec3" | "vec4" | "quat" | "rgba" | "input" | "input_at" | "palette" | "palette_srgb" | "palette_table" | "texture" | "texel" | "read" | "read_at" | "drive" | "scratch" | "memory" | "dot" | "cross" | "length" | "normalize" | "min" | "max" | "clamp")
            || f.args.iter().any(|a| self.is_vec(a, scope))
        {
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
            // The texture's size, (width, height) in texels (batch 6).
            "texture_size" => {
                if !self.coloring {
                    return self.err(p, "`texture_size()` is a texture's: an escape colouring's, in `start` and `step`");
                }
                match args.as_slice() {
                    [] => {}
                    [k] => vec::texture_index(k).map_err(|m| Diag::error(span, m))?,
                    _ => return self.err(p, "`texture_size()`, or `texture_size(k)` for texture `k`"),
                }
                let part = |k: i64| mk(ExprKind::Call { name: "__texture_size".into(), args: vec![mk(ExprKind::Int(k), span)], user: false }, span);
                mk(ExprKind::Complex(Box::new(part(0)), Box::new(part(1))), span)
            }
            // The palette mapping's curve (an escape colouring's, batch 5).
            "palette_curve" => {
                if !self.coloring {
                    return self.err(p, "`palette_curve(t)` is an escape colouring's");
                }
                let a = one(&mut args, self)?;
                mk(ExprKind::Call { name: "__esc_palette_curve".into(), args: vec![a], user: false }, span)
            }
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
            let builtin = crate::variations::global_registry().get(&vname).is_some_and(|v| v.provenance != crate::provenance::Provenance::Local);
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
        if fns.contains_key("prepare") {
            return self.err(p, format!("`{vname}` computes values from its parameters (`prepare`): a flame's; inside a formula, a later step"));
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
            // A formula has no colour coordinate: a variation reads 0 there.
            let ctx: HashMap<String, ast::Expr> =
                [("accum".to_string(), zero), ("weight".to_string(), mk(ExprKind::Float(1.0), span)), ("color".to_string(), mk(ExprKind::Float(0.0), span)), ("hide".to_string(), mk(ExprKind::Bool(false), span))]
                    .into_iter()
                    .chain(["a", "b", "c", "d", "e", "f"].into_iter().zip([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]).map(|(n, v)| (n.to_string(), mk(ExprKind::Float(v), span))))
                    .collect();
            inner.bind(cp, Binding::ParamMap(ctx, "in a variation's context (`accum`, `weight`, `color`, `hide`)"));
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
    /// A method call's value as the helper's `this`, as Rhai binds it: a
    /// variable by reference, so the helper's writes to `this` are its
    /// caller's (a WGSL pointer's way); any other value a temporary, its
    /// changes lost.
    fn this_binding(&mut self, v: ast::Expr, span: Span) -> Binding {
        match &v.kind {
            ExprKind::Name(n) => Binding::Local(n.clone()),
            _ if self.pure => Binding::Value(v),
            _ => {
                let ty = self.ty_of(&v);
                let t = self.local("this", ty);
                self.pending.push(ast::Stmt::Decl { ty, name: t.clone(), init: Some(v), dims: None, span });
                Binding::Local(t)
            }
        }
    }

    /// Whether `e` calls a helper of the script's as a method of a
    /// variable or of `this`: a statement for what it writes.
    fn helper_method(&self, e: &Expr) -> bool {
        let Expr::Dot(be, _, _) = e else { return false };
        matches!(&be.lhs, Expr::Variable(..) | Expr::ThisPtr(_)) && matches!(&be.rhs, Expr::MethodCall(f, _) if self.fns.contains_key(f.name.as_str()))
    }

    /// A helper called alone: its statements, a value it ends in unused.
    fn helper_stmts(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<()> {
        if returns_early(b) {
            return self.err(b.position(), "a helper called alone, for what it does, returns at its end");
        }
        let outer = std::mem::take(&mut self.pending);
        let sealed = self.seal();
        scope.blocks.push(HashMap::new());
        let r = (|| {
            let mut out = Vec::new();
            let stmts = b.statements();
            for (i, st) in stmts.iter().enumerate() {
                match final_value(st).filter(|_| i + 1 == stmts.len()) {
                    Some(f) => {
                        let _ = self.final_expr(f, scope)?;
                        self.flush(&mut out);
                    }
                    None => self.stmt(st, scope, &mut out)?,
                }
            }
            Ok(out)
        })();
        scope.blocks.pop();
        self.unseal(sealed);
        self.pending = outer;
        self.pending.extend(r?);
        Ok(())
    }

    fn helper_body(&mut self, b: &StmtBlock, scope: &mut Scope) -> R<ast::Expr> {
        // One that returns early: a scope (P7 step 1).
        if returns_early(b) {
            let (decls, into) = self.scoped(b, scope, None)?;
            self.pending.extend(decls);
            return Ok(mk(ExprKind::Name(into[0].clone()), self.pos.span(b.position())));
        }
        let (body, value) = self.value_block(b, scope)?;
        self.pending.extend(body);
        Ok(value)
    }

    /// `a op b`, with Rhai's rules where they differ from the checker's.
    fn binary(&mut self, op: BinOp, a: ast::Expr, b: ast::Expr, p: Position) -> R<ast::Expr> {
        let span = self.pos.span(p);
        // Two ints divided truncate, as Rhai's do (P4 step 9b): its own
        // operator, not UF's quotient.
        let integer = |t: TypeName| matches!(t, TypeName::Int | TypeName::UInt);
        if op == BinOp::Div && integer(self.ty_of(&a)) && integer(self.ty_of(&b)) {
            return Ok(mk(ExprKind::Binary(BinOp::IDiv, Box::new(a), Box::new(b)), span));
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
                "pass" | "maxiter" | "x" | "y" | "width" | "height" | "numiter" | "crease_pattern" | "effect_blend" | "period" => TypeName::Int,
                // A colouring's view of the loop (P7 step 2); `prev` and
                // `step_c` are complex, the default.
                "bailout" | "degree" | "escape_fraction" | "render_px" | "supersample" | "value" => TypeName::Float,
                "escaped" | "converged" | "has_derivative" | "vhide" => TypeName::Bool,
                "draw_u32" => TypeName::UInt,
                "magn" | "pi" | "e" | "crease_decay" | "crease_sign" | "crease_reach" | "crease_height" | "draw" | "weight" | "vcolor" | "pixelz" | "accumz" | "vrgb_r" | "vrgb_g" | "vrgb_b" | "coef_a" | "coef_b" | "coef_c" | "coef_d"
                | "coef_e" | "coef_f" | "pointw" | "wout" | "cam_x" | "cam_y" | "cam_z" | "cam_pitch" | "cam_yaw" | "cam_bank" | "cam_persp" => TypeName::Float,
                // An effect's (P2).
                "density_mean" | "pixel_scale" | "effect_r" | "effect_g" | "effect_b" | "effect_a" | "effect_amount" => TypeName::Float,
                _ => TypeName::Complex,
            },
            ExprKind::Param(n) => self.param_tys.get(n).copied().unwrap_or(TypeName::Complex),
            ExprKind::Call { name, args, user } => {
                if *user {
                    return TypeName::Complex;
                }
                match name.as_str() {
                    "to_float" | "__grid_param" => return TypeName::Float,
                    "to_int" => return TypeName::Int,
                    "u32" => return TypeName::UInt,
                    _ => {}
                }
                // A simulation's framework call (P4 steps 9c, 9d).
                if let Some(h) = super::builtins::Host::from_name(name) {
                    return h.result().map_or(TypeName::Float, from_ty);
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
                // Two ints divided truncate (`binary`); two u32s, as WGSL's.
                BinOp::Div => join(self.ty_of(a), self.ty_of(b)),
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
    /// A simulation colouring's sample (P4).
    Cell,
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
        // An unsigned 32-bit int (`Ty::UInt`).
        "u32" => "u32",
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
        (UInt, Int | UInt) | (Int, UInt) => UInt,
        (Bool, Bool) => Bool,
        _ => Float,
    }
}

fn to_ty(t: TypeName) -> Ty {
    match t {
        TypeName::Bool => Ty::Bool,
        TypeName::Int => Ty::Int,
        TypeName::UInt => Ty::UInt,
        TypeName::Float => Ty::Float,
        _ => Ty::Complex,
    }
}

fn from_ty(t: Ty) -> TypeName {
    match t {
        Ty::Bool => TypeName::Bool,
        Ty::Int => TypeName::Int,
        Ty::UInt => TypeName::UInt,
        Ty::Float => TypeName::Float,
        Ty::Complex => TypeName::Complex,
    }
}

#[cfg(test)]
pub(crate) mod tests {
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

    /// A loop's `break` and `continue` in every kind (rhai-plugins P7
    /// step 1): a formula's step using both, in a `for` and a `while`,
    /// runs in the interpreter -- the CPU's reference, deep zoom's and
    /// the tests' -- as the same step written without them, and compiles
    /// to WGSL that validates; a variation's too, and its WGSL loops
    /// break and continue.
    #[test]
    fn break_and_continue_run_in_every_kind() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{run, transform, Env, ParamValues};
        let head = "fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}
fn escaped(s, p) {
    s.z.norm_sqr() > 4.0
}
";
        let flow = r#"
fn step(s, p) {
    let w = s.z;
    for k in 0..6 {
        if k == 1 {
            continue;
        }
        if k == 4 {
            break;
        }
        w = w * 0.5 + s.c * 0.25;
    }
    let n = 0;
    while n < 100 {
        n += 1;
        if n == 3 {
            break;
        }
    }
    s.z = w * w + s.c * to_float(n - 2);
    s
}
"#;
        let plain = "fn step(s, p) {
    let w = s.z;
    for k in 0..3 {
        w = w * 0.5 + s.c * 0.25;
    }
    s.z = w * w + s.c;
    s
}
";
        let (a, b) = (program(&format!("{head}{flow}")).unwrap(), program(&format!("{head}{plain}")).unwrap());
        let (va, vb) = (ParamValues::defaults(&a), ParamValues::defaults(&b));
        for (re, im) in [(-0.75, 0.1), (0.3, 0.5), (-1.2, -0.2), (0.25, 0.0)] {
            let env = Env::at(C64::new(re, im), 200);
            let (x, y) = (run(&a, &va, None, &env), run(&b, &vb, None, &env));
            assert_eq!((x.n, x.z.re.to_bits(), x.z.im.to_bits()), (y.n, y.z.re.to_bits(), y.z.im.to_bits()), "at {re} + {im}i");
        }
        let wgsl = crate::lang::wgsl::formula(&a);
        assert!(wgsl.contains("break;") && wgsl.contains("continue;"), "{wgsl}");
        crate::lang::wgsl::validate_formula(&wgsl).unwrap_or_else(|e| panic!("{e}
{wgsl}"));

        let var = "fn variation(z, p) {
    let w = z;
    for k in 0..5 {
        if k == 1 {
            continue;
        }
        if k == 3 {
            break;
        }
        w = w * 0.9;
    }
    w
}
";
        let v = transformation(var).unwrap_or_else(|d| panic!("{d:?}"));
        let z = C64::new(0.7, -0.3);
        let got = transform(&v, &ParamValues::defaults(&v), z);
        let want = C64::new(0.7 * 0.9 * 0.9, -0.3 * 0.9 * 0.9);
        assert!((got.re - want.re).abs() < 1e-12 && (got.im - want.im).abs() < 1e-12, "{got:?}");
        let w = crate::lang::wgsl::variation(Some(&v), None, "flowtest").unwrap();
        let w2 = w.wgsl_2d.expect("a 2D variation");
        assert!(w2.contains("break;") && w2.contains("continue;"), "{w2}");

        // An early `return` in the step: its own scope, since P7 step 1.
        assert!(program(&format!("{head}fn step(s, p) {{
    if s.z.re > 1.0 {{
        return s;
    }}
    s.z = s.z * s.z + s.c;
    s
}}
")).is_ok());
    }

    /// A variation that returns early, varied by a formula, as an early
    /// return in a varied variation for the GPU test beside it.
    pub(crate) const EARLY_VARIATION: &str = r#"
const INFO = #{ name: "rhai_early_test" };
fn variation(z, p) {
    if z.norm_sqr() > 1.0 {
        return z * 0.5;
    }
    let w = z;
    for k in 0..4 {
        if k == 2 {
            return w * 2.0;
        }
        w = w + complex(0.1, 0.0);
    }
    w
}
"#;

    /// The formula that varies it, and the same formula written without
    /// either: the variation's returns leave the variation, and the step
    /// goes on past it.
    pub(crate) const EARLY_FORMULA: &str = r#"
fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}
fn step(s, p) {
    let w = vary("rhai_early_test", s.z);
    s.z = w * w + s.c;
    s
}
fn escaped(s, p) {
    s.z.norm_sqr() > 4.0
}
"#;
    pub(crate) const EARLY_PLAIN: &str = r#"
fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}
fn step(s, p) {
    let w = if s.z.norm_sqr() > 1.0 { s.z * 0.5 } else { (s.z + complex(0.1, 0.0) + complex(0.1, 0.0)) * 2.0 };
    s.z = w * w + s.c;
    s
}
fn escaped(s, p) {
    s.z.norm_sqr() > 4.0
}
"#;

    /// A grid's cell at a row the formula computes (one parameter system,
    /// step 10): one `fparam` read at the slot the row gives, clamped to
    /// the grid; the interpreter reads the same parameter, so the CPU's
    /// orbit is the formula's.
    #[test]
    fn a_formula_reads_a_grid_row_it_computes() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{run, Env, ParamValues};
        let src = "const PARAMS = #{\n    pick: #{ value: 2, min: 0, max: 2 },\n    \
                   table: #{ type: \"grid\", rows: 3, cell: \"t{row}_{column}\", columns: #{ k: #{ value: [0.0, 0.5, 0.25], min: 0.0, max: 1.0 } } },\n};\n\
                   fn start(pixel, p) {\n    #{ z: complex(0.0, 0.0), c: pixel }\n}\n\
                   fn step(s, p) {\n    s.z = s.z * s.z + s.c * p.table[p.pick].k;\n    s\n}\n\
                   fn escaped(s, p) {\n    s.z.norm_sqr() > 4.0\n}\n";
        let p = program(src).unwrap_or_else(|d| panic!("{d:?}"));
        let wgsl = crate::lang::wgsl::formula(&p);
        crate::lang::wgsl::validate_formula(&wgsl).unwrap_or_else(|e| panic!("{e}\n{wgsl}"));
        assert!(wgsl.contains("fparam(u32(1i + clamp("), "{wgsl}");
        // The same orbit as the formula with the row's value written in.
        let plain = program(&src.replace("p.table[p.pick].k", "0.25")).unwrap();
        let env = Env::at(C64::new(1.5, 0.7), 50);
        let (a, b) = (run(&p, &ParamValues::defaults(&p), None, &env), run(&plain, &ParamValues::defaults(&plain), None, &env));
        assert_eq!((a.n, a.z.re.to_bits(), a.z.im.to_bits()), (b.n, b.z.re.to_bits(), b.z.im.to_bits()));
    }

    /// An early `return` leaves its own scope (rhai-plugins P7 step 1): a
    /// varied variation's -- directly, and from inside its own loop --
    /// ends the variation, not the formula's step; a helper's ends the
    /// helper; a step's ends the step. Each formula runs in the interpreter
    /// as the same formula written without `return`, bit for bit, and its
    /// WGSL validates; the variation's own WGSL leaves its scope by a
    /// `break`, and by its flag from inside its loop. A function defined
    /// twice is refused.
    #[test]
    fn an_early_return_leaves_its_own_scope() {
        use crate::lang::builtins::C64;
        use crate::lang::interp::{run, transform, Env, ParamValues};
        let same = |a: &str, b: &str, vars: &[(String, String)]| {
            let (a, b) = with_variations(vars, || (program(a), program(b)));
            let (a, b) = (a.unwrap_or_else(|d| panic!("{d:?}")), b.unwrap_or_else(|d| panic!("{d:?}")));
            let (va, vb) = (ParamValues::defaults(&a), ParamValues::defaults(&b));
            for (re, im) in [(-0.75, 0.1), (0.3, 0.5), (-1.2, -0.2), (0.25, 0.0), (-0.1, 0.9), (0.38, -0.31)] {
                let env = Env::at(C64::new(re, im), 300);
                let (x, y) = (run(&a, &va, None, &env), run(&b, &vb, None, &env));
                assert_eq!((x.n, x.z.re.to_bits(), x.z.im.to_bits()), (y.n, y.z.re.to_bits(), y.z.im.to_bits()), "at {re} + {im}i");
            }
            let wgsl = crate::lang::wgsl::formula(&a);
            crate::lang::wgsl::validate_formula(&wgsl).unwrap_or_else(|e| panic!("{e}
{wgsl}"));
            wgsl
        };
        // A varied variation.
        let vars = [("rhai_early_test".to_string(), EARLY_VARIATION.to_string())];
        let wgsl = same(EARLY_FORMULA, EARLY_PLAIN, &vars);
        if std::env::var("SHOW_SCOPE_WGSL").is_ok() {
            println!("{wgsl}");
        }
        assert!(wgsl.contains("loop {") && wgsl.contains("scope_done"), "{wgsl}");
        // A helper, and the step itself.
        let head = "fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}
fn escaped(s, p) {
    s.z.norm_sqr() > 4.0
}
";
        let helper = "fn capped(x, hi) {
    if x > hi {
        return hi;
    }
    x
}
fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s.z = complex(capped(s.z.re, 1.5), s.z.im);
    s
}
";
        let helper_plain = "fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s.z = complex(if s.z.re > 1.5 { 1.5 } else { s.z.re }, s.z.im);
    s
}
";
        same(&format!("{head}{helper}"), &format!("{head}{helper_plain}"), &[]);
        let step = "fn step(s, p) {
    if s.z.norm_sqr() > 2.0 {
        s.z = s.z * 1.5;
        return s;
    }
    s.z = s.z * s.z + s.c;
    s
}
";
        let step_plain = "fn step(s, p) {
    if s.z.norm_sqr() > 2.0 {
        s.z = s.z * 1.5;
    } else {
        s.z = s.z * s.z + s.c;
    }
    s
}
";
        same(&format!("{head}{step}"), &format!("{head}{step_plain}"), &[]);

        // The variation on its own: the interpreter's point, and its WGSL
        // leaving by the flag from inside its loop.
        let v = transformation(EARLY_VARIATION).unwrap_or_else(|d| panic!("{d:?}"));
        let at = |z: C64| transform(&v, &ParamValues::defaults(&v), z);
        let z = C64::new(0.2, 0.1);
        let w = at(z);
        assert_eq!((w.re.to_bits(), w.im.to_bits()), ((((0.2 + 0.1) + 0.1) * 2.0f64).to_bits(), (0.1 * 2.0f64).to_bits()));
        let far = at(C64::new(1.0, 1.0));
        assert_eq!((far.re, far.im), (0.5, 0.5));
        let w2 = crate::lang::wgsl::variation(Some(&v), None, "rhai_early_test").unwrap().wgsl_2d.expect("2D");
        assert!(w2.contains("scope_done") && w2.contains("loop {"), "{w2}");

        // Rhai's overloads by arity are one name too many here.
        let twice = format!("{head}fn f(x) {{ x }}
fn f(x, y) {{ x + y }}
fn step(s, p) {{
    s.z = s.z * s.z + s.c;
    s
}}
");
        let e = program(&twice).unwrap_err();
        assert!(e.iter().any(|d| d.message.contains("`f` is defined twice")), "{e:?}");
    }

    /// An id goes into the script's own text (design rule 7): first in its
    /// `INFO`, or a new `INFO` under its header comment; one it has stays.
    #[test]
    fn an_id_is_written_into_the_source() {
        let id = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
        let with = with_info_id("const INFO = #{ title: \"Glow\" };\nfn effect(c, p) { c }\n", id);
        assert_eq!(info_string(&with, "id").as_deref(), Some(id));
        assert_eq!(info_string(&with, "title").as_deref(), Some("Glow"));
        let bare = with_info_id("// Glow: a soft bloom.\n\nfn effect(c, p) { c }\n", id);
        assert!(bare.starts_with("// Glow: a soft bloom.\n\nconst INFO = #{ id: "), "{bare}");
        assert_eq!(info_string(&bare, "id").as_deref(), Some(id));
        assert_eq!(with_info_id(&with, "another"), with, "an id it has stays");

        // An id of one's own: a UUID stays; a built-in's or a hand-typed
        // one is replaced, the rest of the text as it was; none is added.
        let fresh = "0e5c6b2a-1d3f-4a7e-9b8c-2f4d6a8c0e1b";
        assert_eq!(with_own_id(&with, fresh), with, "a UUID it has stays");
        let copied = "const INFO = #{ title: \"Vignette\", id: \"vignette\" };\nfn effect(c, p) { c }\n";
        let own = with_own_id(copied, fresh);
        assert_eq!(own, copied.replace("\"vignette\"", &format!("\"{fresh}\"")));
        assert_eq!(info_string(&own, "title").as_deref(), Some("Vignette"));
        let bare = "fn effect(c, p) { c }\n";
        assert_eq!(info_string(&with_own_id(bare, fresh), "id").as_deref(), Some(fresh));
    }

    /// Labels by locale (design rule 8): a title, caption, hint or choice
    /// written as a map compiles, in the current language, and
    /// `source_labels` keeps every language for whatever shows them.
    #[test]
    fn labels_may_be_written_by_locale() {
        let src = r#"
const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: #{ en: "Bailout", ja: "脱出半径" }, hint: "How far" },
    shape: #{ value: 0, min: 0, max: 1, choices: [#{ en: "Round", "zh-CN": "圆" }, "Square"] },
};
const INFO = #{ title: #{ en: "Mandelbrot", ja: "マンデルブロ" }, center: complex(-0.5, 0.0) };

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
        rh(src);
        let labels = source_labels(src).expect("parses");
        assert_eq!(labels.title.resolve_in("ja"), "マンデルブロ");
        assert_eq!(labels.title.resolve_in("es"), "Mandelbrot");
        let bailout = labels.param("bailout").expect("bailout");
        assert_eq!(bailout.caption.resolve_in("ja"), "脱出半径");
        assert_eq!(bailout.hint.resolve_in("ja"), "How far");
        let shape = labels.param("shape").expect("shape");
        assert_eq!(shape.choices.len(), 2);
        assert_eq!(shape.choices[0].resolve_in("zh-CN"), "圆");
        assert_eq!(shape.choices[1].resolve_in("zh-CN"), "Square");
        // Anything else is refused, with what it may be.
        let bad = src.replace("hint: \"How far\"", "hint: 3");
        let e = program(&bad).expect_err("a number is not a label");
        assert!(e.iter().any(|d| d.message.contains("map of locale")), "{e:?}");
    }

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
        let m = rh(include_str!("../../assets/formulas/mandelbrot.rhai"));
        let um = uf("M {\ninit:\n if ismand == 0\n  z = #pixel\n  c = (-0.8, 0.156)\n else\n  z = 0\n  c = #pixel\n endif\nloop:\n z = z * z + c\nbailout:\n |z| <= @bailout\ndefault:\n float param bailout\n  default = 4\n endparam\n}");
        same(&m, &um, false);
        same(&m, &um, true);
        let ship = rh(include_str!("../../assets/formulas/burning-ship.rhai"));
        let us = uf("B {\ninit:\n z = 0\n c = #pixel\nloop:\n z = sqr(abs(z)) + c\nbailout:\n |z| <= 4\n}");
        same(&ship, &us, false);
        let ph = rh(include_str!("../../assets/formulas/phoenix.rhai"));
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
        let text = include_str!("../../assets/formulas/orbit-trap.rhai");
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
        let text = include_str!("../../assets/formulas/smooth.rhai");
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
            ("const INFO = #{ center: complex(0.0, 0.0) };\nfn finish(s, z, p) { 0.0 }", "`bailout` and `pick`"),
            ("const INFO = #{ region: \"left\" };\nfn finish(s, z, p) { 0.0 }", "`region` is"),
            ("const INFO = #{ bounded: 1 };\nfn finish(s, z, p) { 0.0 }", "`bounded` is true or false"),
            // A colour (batch 5): `color` ends with a `vec3`, and its
            // value after the transfer is its alone.
            ("fn finish(s, z, p) { 0.0 }\nfn color(s, z, p) { 1.0 }", "a number where a vector is wanted"),
            ("fn finish(s, z, p) { 0.0 }\nfn color(s, z, p) { quat(1.0, 0.0, 0.0, 0.0) }", "a `vec3` in linear light, or an `rgba`"),
            // A texture (batch 6): read in the loop alone, and by a kind
            // whose template binds one.
            ("fn finish(s, z, p) { texture(complex(0.5, 0.5)).r }", "in `start` and `step`"),
            ("fn finish(s, z, p) { 0.0 }\nfn color(s, z, p) { let t = texel(1, 2); vec3(t.r, t.g, t.b) }", "in `start` and `step`"),
            ("const INFO = #{ skips_escaping: 1 };\nfn finish(s, z, p) { 0.0 }", "`skips_escaping` is true or false"),
            ("fn finish(s, z, p, ctx) { ctx.value }", "`ctx.value` belongs to `color`"),
            ("fn step(s, z, p, ctx) { s }\nfn finish(s, z, p, ctx) { 0.0 }\nfn color(s, z, p, ctx) { vec3(ctx.prev.re, 0.0, 0.0) }", "`ctx.prev` belongs to `step`"),
            ("const INFO = #{ bailout: -4.0 };\nfn finish(s, z, p) { 0.0 }", "`bailout` is a positive number"),
            ("const INFO = #{ pick: #{ deband: 1 } };\nfn finish(s, z, p) { 0.0 }", "`deband`, which is not a parameter"),
            ("const PARAMS = #{ deband: #{ value: 0 } };\nconst INFO = #{ pick: #{ deband: \"on\" } };\nfn finish(s, z, p) { 0.0 }", "a number, or true or false"),
            ("fn finish(s, z, p) { let t = 1.0; }", "ends with the index"),
        ] {
            let err = coloring(src).err().unwrap_or_else(|| panic!("{src}: compiled"));
            let text: Vec<String> = err.iter().map(|d| d.message.clone()).collect();
            assert!(text.iter().any(|m| m.contains(says)), "{src}: {text:?}");
        }
    }

    /// A texture read names its texture by a last argument, 0 by default
    /// (batch 6): one exists yet, so 0 alone, written out -- and a script
    /// written so keeps working when more come.
    #[test]
    fn a_texture_is_named_by_its_index() {
        let with = |k: &str| {
            coloring(&format!(
                "fn start(z, p) {{ #{{ r: 0.0 }} }}\nfn step(s, z, p) {{ let t = texture(z{k}); let u = texel(1, 2{k}); let w = texture_size({}); s.r = t.r + u.g + w.re; s }}\nfn finish(s, z, p) {{ s.r }}",
                k.trim_start_matches(", ")
            ))
        };
        assert!(with("").is_ok(), "{:?}", with("").err().map(|d| d.iter().map(|x| x.message.clone()).collect::<Vec<_>>()));
        assert!(with(", 0").is_ok(), "texture 0, named");
        for (k, says) in [(", 1", "one texture exists yet"), (", 1 - 1", "written out")] {
            let err = with(k).err().unwrap_or_else(|| panic!("texture{k} compiled"));
            assert!(err.iter().any(|d| d.message.contains(says)), "{k}: {:?}", err.iter().map(|d| &d.message).collect::<Vec<_>>());
        }
    }

    /// A texture is read where a kind's template binds one (P7 step 2,
    /// batch 6): an escape colouring's loop, not yet a formula's.
    #[test]
    fn a_formula_reads_no_texture_yet() {
        let src = "fn start(pixel, p) { #{ z: complex(0.0, 0.0), c: pixel } }\n\
                   fn step(s, p) { let t = texture(s.z); s.z = s.z * s.z + s.c + complex(t.r, 0.0); s }\n\
                   fn escaped(s, p) { s.z.norm_sqr() > 4.0 }";
        let err = program(src).err().expect("a formula reading a texture compiled");
        assert!(err.iter().any(|d| d.message.contains("where the template binds one")), "{:?}", err.iter().map(|d| &d.message).collect::<Vec<_>>());
    }

    /// Two ints divided truncate toward zero, as Rhai's do (P4 step 9b),
    /// in the interpreter as on the GPU; by zero the dividend, as WGSL's.
    #[test]
    fn two_ints_divided_truncate_as_rhai_does() {
        let src = "fn start(pixel, p) { #{ z: pixel } }
fn step(s, p) {
    let q = 7 / 2 + (-7) / 2 * 10 + 5 / 0 * 100;
    s.z = complex(to_float(q), 0.0);
    s
}
fn escaped(s, p) { false }";
        let p = program(src).unwrap_or_else(|d| panic!("{d:?}"));
        let params = super::super::interp::ParamValues::defaults(&p);
        let out = super::super::interp::run(&p, &params, None, &super::super::interp::Env::at(super::super::builtins::C64::new(0.0, 0.0), 1));
        assert_eq!(out.z.re, f64::from(3 + (-3) * 10 + 5 * 100), "7 / 2 is 3, -7 / 2 is -3, 5 / 0 is 5");
    }

    #[test]
    fn what_a_formula_refuses_and_where() {
        let base = |step: &str| format!("fn start(pixel, p) {{ #{{ z: pixel }} }}\nfn step(s, p, ctx) {{\n{step}\n}}\nfn escaped(s, p) {{ s.z.norm_sqr() > 4.0 }}");
        for (step, says) in [
            ("s.w = s.z; s", "`w` is not in the state"),
            ("s.z = s.z * p.power; s", "`power` is not in `PARAMS`"),
            ("s.z = gamma(s.z); s", "unknown function `gamma`"),
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
