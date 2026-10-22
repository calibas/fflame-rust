//! Names, types and sections: an entry's parse tree to a [`Program`].
//!
//! UF's rules (Language basics: Variables, Types, Type compatibility,
//! Parameters): a variable is created by its first assignment and is
//! complex unless declared; conversion only widens (int, float,
//! complex); bool converts to and from numbers with a warning; `/`
//! never yields an int. Parameters are read-only and prefixed `@`
//! except Fractint's `p1..p6`; user functions are `@f(x)` or
//! Fractint's `fn1..fn4`.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::ast::*;
use super::builtins::{Builtin, USER_FUNCTIONS};
use super::ir::*;
use super::{Diag, Dialect, EntryKind, Span};

/// Floats a colouring may keep per pixel: the engine's accumulator, one
/// `vec4`, and seven more (`AccumWidth::Extended`), which the perturbed
/// path and the recolour records store. 32 admits 96% of UF's public
/// colourings that compile (measured, plan phase 2).
pub const STATE_CAP_FLOATS: usize = 32;
/// Floats of parameters each of a formula and a colouring may have
/// (the uniform's `fparams` / `cparams`, `assembler::PARAM_VEC4S`
/// vec4s).
pub const PARAM_SLOTS: usize = 64;
/// Floats of `global:` values a formula may have: the uniform's
/// `fdata` (64 vec4s), which nothing else of a custom formula's uses. A
/// colouring's share its parameter block, [`PARAM_SLOTS`] in all.
pub const FDATA_FLOATS: usize = 256;
/// The most times one `while` or `repeat` runs per evaluation; past it
/// the loop ends, so no formula can hang the GPU.
pub const LOOP_CAP: u32 = 4096;
/// The most floats one array may take: a global's must fit its block
/// anyway, and a per-pixel one lives in each GPU thread's registers and
/// private memory (the uniform's `fdata` is as large).
pub const ARRAY_CAP_FLOATS: usize = 256;

/// The settings an entry is checked at: an array's size that reads a
/// parameter or `#maxiter` folds with these values (plan, item 3, step
/// 3f), each parameter's real part by its name; one not named, and
/// every one outside [`at_settings`], at its default, and `#maxiter`
/// at the entry's own (else the panel's default).
#[derive(Clone, Debug, Default)]
pub struct SizeSettings {
    pub params: HashMap<String, f64>,
    pub maxiter: Option<u32>,
}

thread_local! {
    static SETTINGS: std::cell::RefCell<Option<SizeSettings>> = const { std::cell::RefCell::new(None) };
}

/// The settings checks run at now: [`at_settings`]'s, else `None`.
pub fn current_settings() -> Option<SizeSettings> {
    SETTINGS.with(|s| s.borrow().clone())
}

/// Run `f` (checks, a compile) with every entry it checks at `settings`.
pub fn at_settings<T>(settings: SizeSettings, f: impl FnOnce() -> T) -> T {
    let before = SETTINGS.with(|s| s.replace(Some(settings)));
    let out = f();
    SETTINGS.with(|s| *s.borrow_mut() = before);
    out
}

/// Fractint's own predefined names, read in Fractint's dialect only.
const FRACTINT_NAMES: &[&str] = &["lastsqr", "whitesq", "scrnpix", "scrnmax", "ismand", "center", "magxmag", "rotskew"];

/// Check one entry as a formula or a colouring.
pub fn check(entry: &Entry, kind: EntryKind, dialect: Dialect) -> (Option<Program>, Vec<Diag>) {
    check_with(entry, kind, dialect, false, false)
}

/// [`check`], reading the names only generated text may use: `#pass`
/// ([`Predef::Pass`]) and `#juliac` ([`Predef::JuliaC`]), for a hybrid's
/// program ([`super::hybrid`]).
pub(crate) fn check_internal(entry: &Entry, kind: EntryKind, dialect: Dialect) -> (Option<Program>, Vec<Diag>) {
    check_with(entry, kind, dialect, true, false)
}

/// [`check_internal`] for an entry lowered from a Rhai formula
/// ([`super::rhai`]), with the few names whose meaning follows Rhai's
/// rather than UF's: `abs` of a complex is its modulus.
pub(crate) fn check_rhai(entry: &Entry, kind: EntryKind) -> (Option<Program>, Vec<Diag>) {
    check_with(entry, kind, Dialect::UltraFractal, true, true)
}

fn check_with(entry: &Entry, kind: EntryKind, dialect: Dialect, internal: bool, rhai: bool) -> (Option<Program>, Vec<Diag>) {
    let mut c = Checker {
        internal,
        rhai,
        kind,
        dialect,
        diags: Vec::new(),
        vars: Vec::new(),
        var_ids: HashMap::new(),
        params: Vec::new(),
        param_ids: HashMap::new(),
        funcs: Vec::new(),
        func_ids: HashMap::new(),
        section: SectionKind::Init,
        warned_bool: false,
        shadows: Vec::new(),
        lastsqr: None,
        settings: SETTINGS.with(|s| s.borrow().clone()),
        maxiter: 0,
    };
    let program = c.run(entry);
    let failed = super::has_errors(&c.diags);
    (if failed { None } else { program }, c.diags)
}

struct Checker {
    /// Generated text: the internal names are readable.
    internal: bool,
    /// Lowered from Rhai: `abs` of a complex is its modulus.
    rhai: bool,
    kind: EntryKind,
    dialect: Dialect,
    diags: Vec<Diag>,
    vars: Vec<Var>,
    var_ids: HashMap<String, VarId>,
    params: Vec<Param>,
    param_ids: HashMap<String, ParamId>,
    funcs: Vec<UserFunc>,
    func_ids: HashMap<String, FuncId>,
    /// The section being lowered, for what may be read and written.
    section: SectionKind,
    warned_bool: bool,
    /// Parameters and predefined names a formula writes (Fractint's
    /// habit): each a per-pixel copy, set from the original first.
    shadows: Vec<(VarId, Shadowed)>,
    /// Fractint's `LastSqr`: the variable `sqr()` writes, when read.
    lastsqr: Option<VarId>,
    /// What an array's size reading the settings folds with
    /// ([`at_settings`]); `None`, the defaults.
    settings: Option<SizeSettings>,
    /// `#maxiter` in a size: the settings', else the entry's, else the
    /// panel's default.
    maxiter: u32,
}

/// What a written parameter or predefined name copies.
#[derive(Clone, Debug)]
enum Shadowed {
    Param(String),
    Predef(String),
}

impl Checker {
    fn error(&mut self, span: Span, msg: impl Into<String>) {
        // Once per place: a lowered Rhai array is declared twice from one
        // `array(n, v)` (its constructor's and the name's).
        let d = Diag::error(span, msg);
        if !self.diags.iter().any(|x| x.span == d.span && x.message == d.message) {
            self.diags.push(d);
        }
    }

    fn warn(&mut self, span: Span, msg: impl Into<String>) {
        self.diags.push(Diag::warning(span, msg));
    }

    fn run(&mut self, entry: &Entry) -> Option<Program> {
        let sections = self.arrange(entry)?;
        self.read_blocks(&entry.default);
        let view = self.view_settings(&entry.default);
        self.maxiter = self
            .settings
            .as_ref()
            .and_then(|s| s.maxiter)
            .or(view.maxiter)
            .unwrap_or_else(|| crate::config::escape::EscapeConfig::default().max_iter)
            .max(1);
        let region = match (self.kind, entry.setting.as_deref().map(str::to_ascii_lowercase).as_deref()) {
            (EntryKind::Coloring, Some("inside")) => Region::Inside,
            (EntryKind::Coloring, Some("outside")) => Region::Outside,
            _ => Region::Both,
        };

        // Declarations and assignments make the variables, before any
        // expression is resolved: a name means the same thing on every
        // line. `global:` comes first, and what it makes is global.
        for (kind, body) in sections.iter() {
            self.section = kind.0;
            let before = self.vars.len();
            self.declare(body);
            if kind.0 == SectionKind::Global {
                for v in &mut self.vars[before..] {
                    v.global = true;
                }
            }
        }
        // Fractint's `LastSqr` is a variable `sqr()` writes; read anywhere,
        // it exists.
        if self.dialect == Dialect::Fractint && sections.iter().any(|(_, b)| mentions(b, "lastsqr")) {
            let id = match self.var_ids.get("lastsqr") {
                Some(&id) => id,
                None => {
                    self.new_var("lastsqr", Ty::Complex, entry.span);
                    self.vars.len() - 1
                }
            };
            self.lastsqr = Some(id);
        }

        let mut global_body = Vec::new();
        let mut perturb_init = Vec::new();
        let mut perturb_loop = Vec::new();
        let mut init = Vec::new();
        let mut loop_body = Vec::new();
        let mut final_body = Vec::new();
        let mut creases = Vec::new();
        let mut transform = Vec::new();
        let mut bailout = None;
        for (kind, body) in sections {
            self.section = kind.0;
            match kind {
                (SectionKind::Global, _) => global_body = self.stmts(body),
                (SectionKind::PerturbInit, _) => perturb_init = self.stmts(body),
                (SectionKind::PerturbLoop, _) => perturb_loop = self.stmts(body),
                (SectionKind::Init, _) => init = self.stmts(body),
                (SectionKind::Loop, last_is_bailout) => {
                    if last_is_bailout {
                        let (stmts, test) = self.fractint_loop(body);
                        loop_body = stmts;
                        bailout = test;
                    } else {
                        loop_body = self.stmts(body);
                    }
                }
                (SectionKind::Bailout, _) => {
                    self.section = SectionKind::Bailout;
                    bailout = self.bailout_section(body);
                }
                (SectionKind::Final, _) => final_body = self.stmts(body),
                (SectionKind::Creases, _) => creases = self.stmts(body),
                (SectionKind::Transform, _) => transform = self.stmts(body),
                _ => {}
            }
        }

        // A written parameter or predefined name starts each pixel as the
        // original.
        let mut first = Vec::new();
        for (id, of) in self.shadows.clone() {
            let v = match of {
                Shadowed::Param(n) => {
                    let p = self.param(&n, entry.span);
                    Some(IExpr { kind: IExprKind::Param(p), ty: self.params[p].ty() })
                }
                Shadowed::Predef(n) => self.predef(alias_target(&n), entry.span).map(predef_expr),
            };
            if let Some(v) = v {
                if let Some(v) = self.convert(v, Ty::Complex, entry.span) {
                    first.push(IStmt::Assign(Place::Var(id), v));
                }
            }
        }
        first.extend(init);
        let init = first;
        let perturb_when = self.perturb_setting(&entry.default);
        let degree = self.degree_setting(&entry.default);

        let mut program = Program {
            kind: self.kind,
            dialect: self.dialect,
            name: entry.name.clone(),
            vars: std::mem::take(&mut self.vars),
            params: std::mem::take(&mut self.params),
            funcs: std::mem::take(&mut self.funcs),
            global_body,
            globals: Vec::new(),
            init,
            loop_body,
            maps: Vec::new(),
            bailout,
            final_body,
            perturb_init,
            perturb_loop,
            perturb_when,
            degree,
            region,
            view,
            state: Vec::new(),
            dz_var: None,
            dzq_var: None,
            dpixel_var: None,
            cycle: None,
            carried: Vec::new(),
            creases,
            transform,
        };
        // The creases read their own variables, the parameters and the
        // globals: they run apart from the orbit.
        let mut cf = Flow::default();
        flow(&program.creases, &mut cf);
        let mut foreign: Vec<&str> = cf.exposed.iter().filter(|&&v| !program.vars[v].global).map(|&v| program.vars[v].name.as_str()).collect();
        foreign.sort_unstable();
        if let Some(n) = foreign.first() {
            self.error(entry.span, format!("`creases` reads `{n}` before setting it: it reads its own variables, the parameters and the globals"));
        }
        // The globals the other sections read are what the GPU needs.
        let mut f = Flow::default();
        flow(&program.init, &mut f);
        flow(&program.loop_body, &mut f);
        flow(&program.final_body, &mut f);
        flow(&program.creases, &mut f);
        if let Some(b) = &program.bailout {
            reads(b, &mut f);
        }
        let mut globals: Vec<VarId> = f.exposed.into_iter().filter(|&v| program.vars[v].global).collect();
        globals.sort_unstable();
        program.globals = globals;
        if program.param_slots() > PARAM_SLOTS {
            self.error(
                entry.span,
                format!(
                    "the parameters take {} floats; at most {PARAM_SLOTS} fit (a complex takes two)",
                    program.param_slots()
                ),
            );
        } else if self.kind == EntryKind::Coloring && program.param_slots() + program.global_floats() > PARAM_SLOTS {
            self.error(
                entry.span,
                format!(
                    "the parameters and the `global:` values take {} floats; a colouring has {PARAM_SLOTS} (a complex takes two)",
                    program.param_slots() + program.global_floats()
                ),
            );
        }
        if self.kind == EntryKind::Formula && program.global_floats() > FDATA_FLOATS {
            self.error(
                entry.span,
                format!("the `global:` values take {} floats; at most {FDATA_FLOATS} fit", program.global_floats()),
            );
        }
        if self.kind == EntryKind::Coloring {
            self.pack_state(&mut program, entry.span);
        }
        Some(program)
    }

    /// Which statements are which section, by entry kind. Returns
    /// `((kind, last_is_bailout), body)` in execution order.
    fn arrange<'e>(&mut self, entry: &'e Entry) -> Option<Vec<((SectionKind, bool), &'e [Stmt])>> {
        let mut out = Vec::new();
        let only_unlabelled = entry.sections.len() == 1 && !entry.sections[0].labelled;
        for s in &entry.sections {
            let mut kind = s.kind;
            let mut last_is_bailout = s.last_is_bailout;
            if !s.labelled {
                // Before any label: init in a formula, final in a
                // colouring. A formula that is nothing BUT unlabelled
                // statements is Fractint's without a `:`: all loop,
                // its last statement the test.
                match self.kind {
                    EntryKind::Formula if only_unlabelled => {
                        kind = SectionKind::Loop;
                        last_is_bailout = true;
                    }
                    EntryKind::Formula => kind = SectionKind::Init,
                    EntryKind::Coloring => kind = SectionKind::Final,
                    EntryKind::Transform => kind = SectionKind::Transform,
                    EntryKind::Solid => kind = SectionKind::Init,
                }
            }
            let allowed = match (self.kind, kind) {
                // A transformation's derived values would be its
                // variation's `wgsl_init`: a later step.
                (EntryKind::Formula | EntryKind::Coloring, SectionKind::Global) => true,
                // A variation's `init(p)`: the state a thread starts with
                // (C2c).
                (EntryKind::Transform, SectionKind::Transform | SectionKind::Init) => true,
                (EntryKind::Formula, SectionKind::Init | SectionKind::Loop | SectionKind::Bailout) => true,
                (EntryKind::Formula, SectionKind::PerturbInit | SectionKind::PerturbLoop) => true,
                (EntryKind::Formula, SectionKind::Creases) => true,
                (EntryKind::Coloring, SectionKind::Init | SectionKind::Loop | SectionKind::Final) => true,
                // A solid: its start, step, bail and distance (B3), and
                // what `prepare` computes once (B's leftovers).
                (EntryKind::Solid, SectionKind::Global | SectionKind::Init | SectionKind::Loop | SectionKind::Bailout | SectionKind::Final) => true,
                _ => false,
            };
            if !allowed {
                let what = match self.kind {
                    EntryKind::Formula => "a formula",
                    EntryKind::Coloring => "a colouring",
                    EntryKind::Transform => "a transformation",
                    EntryKind::Solid => "a solid",
                };
                self.error(s.span, format!("{what} has no `{}:` section", kind.label()));
                continue;
            }
            out.push(((kind, last_is_bailout), s.body.as_slice()));
        }
        let has = |k: SectionKind| out.iter().any(|((kind, _), _)| *kind == k);
        match self.kind {
            EntryKind::Formula => {
                if !has(SectionKind::Loop) {
                    self.error(entry.span, "a formula needs a `loop:` section");
                    return None;
                }
                let fractint = out.iter().any(|((k, l), _)| *k == SectionKind::Loop && *l);
                if !fractint && !has(SectionKind::Bailout) {
                    self.error(entry.span, "a formula needs a `bailout:` section");
                    return None;
                }
            }
            EntryKind::Coloring => {
                if !has(SectionKind::Final) {
                    self.error(entry.span, "a colouring needs a `final:` section");
                    return None;
                }
            }
            EntryKind::Transform => {
                if !has(SectionKind::Transform) {
                    self.error(entry.span, "a transformation needs a `transform:` section");
                    return None;
                }
            }
            EntryKind::Solid => {
                if !has(SectionKind::Loop) || !has(SectionKind::Final) {
                    self.error(entry.span, "a solid needs `step` and `distance`");
                    return None;
                }
            }
        }
        // Execution order, whatever order they were written in.
        let rank = |k: SectionKind| match k {
            SectionKind::Global => 0,
            SectionKind::Init => 1,
            SectionKind::Loop => 2,
            SectionKind::Bailout => 3,
            SectionKind::PerturbInit => 4,
            SectionKind::PerturbLoop => 5,
            _ => 6,
        };
        out.sort_by_key(|((k, _), _)| rank(*k));
        Some(out)
    }

    // ---- parameters and settings ----------------------------------------

    fn read_blocks(&mut self, d: &Settings) {
        for block in &d.params {
            let default_expr = block.get("default").and_then(|s| match &s.value {
                SettingValue::Expr(e) => Some(e.clone()),
                _ => None,
            });
            let default = default_expr.as_ref().and_then(|e| self.const_value(e));
            let choices: Vec<String> = match block.get("enum").map(|s| &s.value) {
                Some(SettingValue::Strings(v)) => v.clone(),
                _ => Vec::new(),
            };
            let kind = match (block.ty, &default, choices.is_empty()) {
                (Some(TypeName::Color), ..) => {
                    self.error(block.span, "colour parameters are not supported yet");
                    continue;
                }
                (_, _, false) => ParamKind::Enum,
                (Some(TypeName::Bool), ..) => ParamKind::Bool,
                (Some(TypeName::Int), ..) => ParamKind::Int,
                (Some(TypeName::Float), ..) => ParamKind::Float,
                (Some(TypeName::Complex), ..) => ParamKind::Complex,
                (None, Some(ConstVal::Bool(_)), _) => ParamKind::Bool,
                (None, Some(ConstVal::Int(_)), _) => ParamKind::Int,
                (None, Some(ConstVal::Float(_)), _) => ParamKind::Float,
                (None, _, _) => ParamKind::Complex,
            };
            // An enum's default may name its item.
            let enum_default = match block.get("default").map(|s| &s.value) {
                Some(SettingValue::Strings(v)) if !choices.is_empty() => {
                    v.first().and_then(|n| choices.iter().position(|c| c == n)).map(|i| (i as f64, 0.0))
                }
                _ => None,
            };
            let default = enum_default.unwrap_or_else(|| match default {
                Some(ConstVal::Bool(b)) => (if b { 1.0 } else { 0.0 }, 0.0),
                Some(ConstVal::Int(v)) => (v as f64, 0.0),
                Some(ConstVal::Float(v)) => (v, 0.0),
                Some(ConstVal::Complex(re, im)) => (re, im),
                None => (0.0, 0.0),
            });
            if kind != ParamKind::Complex && default.1 != 0.0 {
                self.error(block.span, format!("`{}` is not complex, but its default is", block.name));
            }
            let num = |c: &mut Self, key: &str| {
                block.get(key).and_then(|s| match &s.value {
                    SettingValue::Expr(e) => match c.const_value(e) {
                        Some(ConstVal::Int(v)) => Some(v as f64),
                        Some(ConstVal::Float(v)) => Some(v),
                        _ => None,
                    },
                    _ => None,
                })
            };
            let min = num(self, "min");
            let max = num(self, "max");
            let exponential = block.get("exponential").is_some_and(|s| {
                matches!(&s.value, SettingValue::Expr(Expr { kind: ExprKind::Bool(true), .. }))
            });
            let param = Param {
                name: block.name.clone(),
                caption: string_setting(block.get("caption")).unwrap_or_else(|| block.name.clone()),
                kind,
                default,
                min,
                max,
                hint: string_setting(block.get("hint")).unwrap_or_default(),
                choices,
                exponential,
                span: block.span,
            };
            // The last block for a name wins (UF's rule).
            match self.param_ids.get(&block.name) {
                Some(&id) => self.params[id] = param,
                None => {
                    self.param_ids.insert(block.name.clone(), self.params.len());
                    self.params.push(param);
                }
            }
        }
        for block in &d.funcs {
            if block.ty == Some(TypeName::Color) {
                self.error(block.span, "colour functions are not supported yet");
                continue;
            }
            let default = match block.get("default").map(|s| &s.value) {
                Some(SettingValue::Func(f)) => match Builtin::from_name(f).and_then(|b| USER_FUNCTIONS.iter().position(|u| *u == b)) {
                    Some(i) => i,
                    None => {
                        self.error(block.span, format!("`{f}` cannot be a user function's default"));
                        0
                    }
                },
                _ => user_function_index(Builtin::Ident),
            };
            let f = UserFunc {
                name: block.name.clone(),
                caption: string_setting(block.get("caption")).unwrap_or_else(|| block.name.clone()),
                default,
                hint: string_setting(block.get("hint")).unwrap_or_default(),
                span: block.span,
            };
            match self.func_ids.get(&block.name) {
                Some(&id) => self.funcs[id] = f,
                None => {
                    self.func_ids.insert(block.name.clone(), self.funcs.len());
                    self.funcs.push(f);
                }
            }
        }
    }

    fn view_settings(&mut self, d: &Settings) -> ViewSettings {
        let mut v = ViewSettings { title: string_setting(d.get("title")), ..Default::default() };
        let get = |c: &mut Self, name: &str| -> Option<ConstVal> {
            match &d.get(name)?.value {
                SettingValue::Expr(e) => c.const_value(e),
                _ => None,
            }
        };
        v.center = match get(self, "center") {
            Some(ConstVal::Complex(re, im)) => Some((re, im)),
            Some(ConstVal::Float(re)) => Some((re, 0.0)),
            Some(ConstVal::Int(re)) => Some((re as f64, 0.0)),
            _ => None,
        };
        let real = |c: Option<ConstVal>| match c {
            Some(ConstVal::Float(v)) => Some(v),
            Some(ConstVal::Int(v)) => Some(v as f64),
            _ => None,
        };
        v.magn = real(get(self, "magn"));
        v.angle = real(get(self, "angle"));
        v.maxiter = real(get(self, "maxiter")).filter(|m| *m >= 1.0).map(|m| m as u32);
        v
    }

    /// A constant: literals, negation, `(re, im)`.
    fn const_value(&mut self, e: &Expr) -> Option<ConstVal> {
        Some(match &e.kind {
            ExprKind::Int(v) => ConstVal::Int(*v),
            ExprKind::Float(v) => ConstVal::Float(*v),
            ExprKind::Imag(v) => ConstVal::Complex(0.0, *v),
            ExprKind::Bool(b) => ConstVal::Bool(*b),
            ExprKind::Unary(UnOp::Neg, inner) => match self.const_value(inner)? {
                ConstVal::Int(v) => ConstVal::Int(-v),
                ConstVal::Float(v) => ConstVal::Float(-v),
                ConstVal::Complex(re, im) => ConstVal::Complex(-re, -im),
                ConstVal::Bool(_) => return None,
            },
            ExprKind::Complex(a, b) => {
                let re = self.const_value(a)?.as_f64()?;
                let im = self.const_value(b)?.as_f64()?;
                ConstVal::Complex(re, im)
            }
            ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), a, b) => {
                // `2.8 + 4i`
                let (ar, ai) = self.const_value(a)?.as_complex()?;
                let (br, bi) = self.const_value(b)?.as_complex()?;
                let s = if *op == BinOp::Add { 1.0 } else { -1.0 };
                let (re, im) = (ar + s * br, ai + s * bi);
                if im == 0.0 && !matches!(self.const_value(a)?, ConstVal::Complex(..)) {
                    ConstVal::Float(re)
                } else {
                    ConstVal::Complex(re, im)
                }
            }
            _ => {
                self.error(e.span, "a setting's value must be a constant");
                return None;
            }
        })
    }

    // ---- names ------------------------------------------------------------

    /// Make every declared or assigned variable.
    fn declare(&mut self, body: &[Stmt]) {
        for s in body {
            match s {
                Stmt::Decl { ty, name, span, init, dims } => {
                    let ty = match ty {
                        TypeName::Bool => Ty::Bool,
                        TypeName::Int => Ty::Int,
                        TypeName::Float => Ty::Float,
                        TypeName::Complex => Ty::Complex,
                        TypeName::Color => {
                            self.error(*span, "colour variables are not supported yet");
                            continue;
                        }
                    };
                    let (dims, size) = match dims {
                        None => (Vec::new(), Vec::new()),
                        Some(d) => match self.array_dims(d, ty, *span) {
                            Some(d) => d,
                            None => continue,
                        },
                    };
                    match self.var_ids.get(name) {
                        Some(&id) if self.vars[id].ty != ty => {
                            let was = self.vars[id].ty;
                            self.error(*span, format!("`{name}` is already a {}", was.name()));
                        }
                        Some(&id) if self.vars[id].dims != dims => {
                            self.error(*span, format!("`{name}` is already declared with other dimensions"));
                        }
                        Some(_) => {}
                        None => {
                            self.new_var(name, ty, *span);
                            let var = self.vars.last_mut().expect("the new variable");
                            var.dims = dims.clone();
                            var.size = size;
                        }
                    }
                    if let Some(e) = init {
                        if !dims.is_empty() {
                            self.error(e.span, "an array is not set in its declaration: assign its elements");
                        }
                        self.declare_in_expr(e);
                    }
                }
                Stmt::Expr(e) => self.declare_in_expr(e),
                // Its parts are declared before it (`vary` in a solid).
                Stmt::Map { input, .. } => {
                    for e in input.iter() {
                        self.declare_in_expr(e);
                    }
                }
                Stmt::If { arms, otherwise, .. } => {
                    for (c, b) in arms {
                        self.declare_in_expr(c);
                        self.declare(b);
                    }
                    if let Some(b) = otherwise {
                        self.declare(b);
                    }
                }
                Stmt::While { cond, body, .. } | Stmt::Repeat { body, cond, .. } => {
                    self.declare_in_expr(cond);
                    self.declare(body);
                }
            }
        }
    }

    fn declare_in_expr(&mut self, e: &Expr) {
        if let ExprKind::Assign(target, value) = &e.kind {
            // An element is written: its array is declared already.
            if let ExprKind::Index(..) = &target.kind {
                self.declare_in_expr(value);
                return;
            }
            if let ExprKind::Name(n) = &target.kind {
                let writes_z = self.kind == EntryKind::Formula && n == "z";
                if !writes_z && !self.var_ids.contains_key(n) {
                    // Writing a parameter (`p1`, in either dialect: UF
                    // allows it for Fractint's sake) or, in Fractint, a
                    // predefined name makes a per-pixel copy of it.
                    let shadow = if is_fractint_param(n) {
                        Some(Shadowed::Param(n.clone()))
                    } else if is_alias(n) && self.dialect == Dialect::Fractint {
                        Some(Shadowed::Predef(n.clone()))
                    } else if is_alias(n) {
                        None
                    } else {
                        self.new_var(n, Ty::Complex, target.span);
                        None
                    };
                    if let Some(of) = shadow {
                        if self.section == SectionKind::Global {
                            self.error(target.span, format!("`{n}` cannot be written in `global:`"));
                        } else {
                            self.new_var(n, Ty::Complex, target.span);
                            self.shadows.push((self.vars.len() - 1, of));
                        }
                    }
                }
            }
            self.declare_in_expr(value);
        }
    }

    /// An array declaration's sizes, folded and as written: constant
    /// integer expressions (UF's rule, which counts the parameters and
    /// `#maxiter` constants: those fold at the settings, [`SizeSettings`]),
    /// each at least one, the whole within [`ARRAY_CAP_FLOATS`].
    fn array_dims(&mut self, dims: &[Expr], ty: Ty, span: Span) -> Option<(Vec<u32>, Vec<IExpr>)> {
        if dims.is_empty() {
            self.error(span, "dynamic arrays (`[]`, `setLength`) are not supported yet");
            return None;
        }
        let mut out = Vec::new();
        let mut size = Vec::new();
        for d in dims {
            let n = match const_size(d) {
                Ok(n) => {
                    size.push(IExpr { kind: IExprKind::Int(n), ty: Ty::Int });
                    n
                }
                Err(_) if settings_size(d).is_empty() => {
                    let e = self.expr(d)?;
                    if e.ty != Ty::Int {
                        self.error(d.span, format!("an array's size is an int, not a {}: `round` or `trunc` it", e.ty.name()));
                        return None;
                    }
                    let settings = self.settings.clone();
                    let params = &self.params;
                    let param = |id: ParamId| {
                        let p = &params[id];
                        let x = settings.as_ref().and_then(|s| s.params.get(&p.name).copied()).unwrap_or(p.default.0);
                        if p.ty() == Ty::Int { x.round() } else { x }
                    };
                    let Some(v) = size_value(&e, &param, self.maxiter).filter(|v| v.is_finite()) else {
                        self.error(d.span, "this size has no value at the parameters' values");
                        return None;
                    };
                    size.push(e);
                    v.round().clamp(-1.0e12, 1.0e12) as i64
                }
                Err(_) => {
                    self.error(d.span, settings_size(d));
                    return None;
                }
            };
            if n < 1 {
                self.error(d.span, format!("an array's size is at least 1, not {n}"));
                return None;
            }
            out.push(n.min(i64::from(u32::MAX)) as u32);
        }
        let floats = out.iter().map(|&d| d as usize).fold(1usize, |a, d| a.saturating_mul(d)).saturating_mul(ty.floats());
        if floats > ARRAY_CAP_FLOATS {
            self.error(span, format!("this array takes {floats} floats; at most {ARRAY_CAP_FLOATS} fit (a complex takes two)"));
            return None;
        }
        Some((out, size))
    }

    /// An array element: `name[i, j]` read at its flattened index, each
    /// index an int, a constant one within its dimension (UF refuses it
    /// otherwise; one computed is clamped where it is read).
    fn element(&mut self, name: &str, index: &[Expr], span: Span) -> Option<(VarId, IExpr)> {
        let Some(&id) = self.var_ids.get(name) else {
            self.error(span, format!("`{name}` is not an array"));
            return None;
        };
        let dims = self.vars[id].dims.clone();
        let size = self.vars[id].size.clone();
        if dims.is_empty() {
            self.error(span, format!("`{name}` is not an array"));
            return None;
        }
        if index.len() != dims.len() {
            self.error(span, format!("`{name}` has {} dimensions: write {} indices", dims.len(), dims.len()));
            return None;
        }
        let int = |kind: IExprKind| IExpr { kind, ty: Ty::Int };
        let mut flat: Option<IExpr> = None;
        for ((x, &d), written) in index.iter().zip(&dims).zip(&size) {
            let i = self.expr(x)?;
            if i.ty != Ty::Int {
                self.error(x.span, format!("an array index is an int, not a {}", i.ty.name()));
                return None;
            }
            // A dimension the settings fix is read as written, so the
            // index means the same at every size (a converted formula's
            // too); a constant index past it is clamped as a computed one.
            let fixed = matches!(written.kind, IExprKind::Int(_));
            if let (IExprKind::Int(k), true) = (&i.kind, fixed) {
                if *k < 0 || *k >= i64::from(d) {
                    self.error(x.span, format!("index {k} is outside `{name}`, whose dimension has {d}"));
                    return None;
                }
            }
            flat = Some(match (flat, &i.kind) {
                (None, _) => i,
                // Constant indices fold: the element is known.
                (Some(IExpr { kind: IExprKind::Int(f), .. }), IExprKind::Int(k)) if fixed => int(IExprKind::Int(f * i64::from(d) + k)),
                (Some(f), _) => {
                    let scaled = int(IExprKind::Binary(BinOp::Mul, Box::new(f), Box::new(written.clone())));
                    int(IExprKind::Binary(BinOp::Add, Box::new(scaled), Box::new(i)))
                }
            });
        }
        Some((id, flat.expect("one index at least")))
    }

    fn new_var(&mut self, name: &str, ty: Ty, span: Span) {
        self.var_ids.insert(name.to_string(), self.vars.len());
        let perturb = matches!(self.section, SectionKind::PerturbInit | SectionKind::PerturbLoop);
        self.vars.push(Var { name: name.to_string(), ty, span, stored_real: false, global: false, perturb, dims: Vec::new(), size: Vec::new() });
    }

    fn param(&mut self, name: &str, span: Span) -> ParamId {
        if let Some(&id) = self.param_ids.get(name) {
            return id;
        }
        // Undeclared: complex, defaulting to zero (UF's rule).
        let id = self.params.len();
        self.param_ids.insert(name.to_string(), id);
        self.params.push(Param {
            name: name.to_string(),
            caption: name.to_string(),
            kind: ParamKind::Complex,
            default: (0.0, 0.0),
            min: None,
            max: None,
            hint: String::new(),
            choices: Vec::new(),
            exponential: false,
            span,
        });
        id
    }

    fn func(&mut self, name: &str, span: Span) -> FuncId {
        if let Some(&id) = self.func_ids.get(name) {
            return id;
        }
        // Fractint's fn1..fn4 start as sin; a UF user function with no
        // block as the identity.
        let default = if is_fractint_fn(name) { Builtin::Sin } else { Builtin::Ident };
        let id = self.funcs.len();
        self.func_ids.insert(name.to_string(), id);
        self.funcs.push(UserFunc {
            name: name.to_string(),
            caption: name.to_string(),
            default: user_function_index(default),
            hint: String::new(),
            span,
        });
        id
    }

    fn predef(&mut self, name: &str, span: Span) -> Option<Predef> {
        let p = match name {
            "pixel" => Predef::Pixel,
            "z" => Predef::Z,
            "numiter" => Predef::NumIter,
            "maxiter" | "maxit" => Predef::MaxIter,
            "pi" => Predef::Pi,
            "e" => Predef::E,
            "random" | "rand" => Predef::Random,
            "center" => Predef::Center,
            "magn" => Predef::Magn,
            "x" => Predef::X,
            "y" => Predef::Y,
            "width" => Predef::Width,
            "height" => Predef::Height,
            "screenpixel" => Predef::ScreenPixel,
            "index" => Predef::Index,
            "solid" => Predef::Solid,
            "pass" if self.internal => Predef::Pass,
            // Rhai's `rand()` (C2a), and a variation's context and colour
            // (C2b, C2d).
            "draw" if self.internal => Predef::Draw,
            // A solid's position's third part (B3).
            "pixelz" if self.internal && self.kind == EntryKind::Solid => Predef::PixelZ,
            "accum" | "weight" | "vcolor" | "pixelz" | "accumz" if self.internal => {
                if self.kind != EntryKind::Transform {
                    self.error(span, format!("`{name}` is a flame variation's"));
                    return None;
                }
                match name {
                    "accum" => Predef::Accum,
                    "weight" => Predef::Weight,
                    "pixelz" => Predef::PixelZ,
                    "accumz" => Predef::AccumZ,
                    _ => Predef::VarColor,
                }
            }
            "juliac" if self.internal => Predef::JuliaC,
            "crease_decay" | "crease_pattern" | "crease_sign" | "crease_reach" | "crease_height" | "crease_slope"
                if self.internal =>
            {
                if self.section != SectionKind::Creases {
                    self.error(span, format!("`{name}` belongs to `creases`"));
                    return None;
                }
                match name {
                    "crease_decay" => Predef::CreaseDecay,
                    "crease_pattern" => Predef::CreasePattern,
                    "crease_sign" => Predef::CreaseSign,
                    "crease_reach" => Predef::CreaseReach,
                    "crease_height" => Predef::CreaseHeight,
                    _ => Predef::CreaseSlope,
                }
            }
            "dz" | "dpixel" if self.in_perturb() => {
                if name == "dz" {
                    Predef::DZ
                } else {
                    Predef::DPixel
                }
            }
            "dz" | "dpixel" => {
                self.error(span, format!("`#{name}` belongs to `perturbinit:` and `perturbloop:`"));
                return None;
            }
            "color" => {
                self.error(span, "direct colouring (`#color`) is not supported yet");
                return None;
            }
            _ => {
                self.error(span, format!("unknown symbol `#{name}`"));
                return None;
            }
        };
        if !self.per_image_ok(p, &format!("#{name}"), span) {
            return None;
        }
        if p == Predef::Z && self.section == SectionKind::Creases {
            self.error(span, "`creases` has the starting point, `#pixel`, not the iterate");
            return None;
        }
        if p == Predef::Pass && self.section == SectionKind::Creases {
            self.error(span, "`creases` runs once, at the starting point: it has no pass");
            return None;
        }
        // A variation sees its point and nothing of an image or an orbit.
        // A solid sees its position and its own state (B3); the
        // estimate it gives is `#index`, in `final:`.
        if self.kind == EntryKind::Solid && !matches!(p, Predef::Pixel | Predef::PixelZ | Predef::Pi | Predef::E | Predef::Index) {
            self.error(span, format!("a solid reads its position, `#pi` and `#e`, not `#{name}`"));
            return None;
        }
        if self.kind == EntryKind::Transform && !matches!(p, Predef::Pixel | Predef::Pi | Predef::E | Predef::Draw | Predef::Accum | Predef::Weight | Predef::VarColor | Predef::PixelZ | Predef::AccumZ) {
            self.error(span, format!("a transformation reads `#pixel`, `#pi` and `#e`, not `#{name}`"));
            return None;
        }
        let colouring = self.kind == EntryKind::Coloring || (self.kind == EntryKind::Solid && p == Predef::Index);
        if matches!(p, Predef::Index | Predef::Solid | Predef::NumIter) {
            if !colouring {
                self.error(span, format!("`#{name}` is a colouring's"));
                return None;
            }
            // UF reads `#numiter` in `final:`, and there alone does the
            // engine have the orbit's summary.
            if self.section != SectionKind::Final {
                self.error(span, format!("`#{name}` belongs to `final:`"));
                return None;
            }
        }
        Some(p)
    }

    /// In `perturbinit:` or `perturbloop:`.
    fn in_perturb(&self) -> bool {
        matches!(self.section, SectionKind::PerturbInit | SectionKind::PerturbLoop)
    }

    /// The `perturb` setting: when the perturbation equations hold, a
    /// bool of parameters and constants (UF's; absent, always).
    fn perturb_setting(&mut self, d: &Settings) -> Option<IExpr> {
        let s = d.get("perturb")?;
        let SettingValue::Expr(e) = &s.value else {
            self.error(s.span, "`perturb` is a bool expression");
            return None;
        };
        // Parameters and constants only: what `global:` may read.
        let saved = self.section;
        self.section = SectionKind::Global;
        let v = self.expr(e).and_then(|v| self.to_bool(v, e.span));
        self.section = saved;
        v
    }

    /// The `degree` setting: the formula's degree at infinity, a float of
    /// parameters and constants (ours; [`Program::degree`]).
    fn degree_setting(&mut self, d: &Settings) -> Option<IExpr> {
        let s = d.get("degree")?;
        let SettingValue::Expr(e) = &s.value else {
            self.error(s.span, "`degree` is a number, or an expression of the parameters");
            return None;
        };
        let saved = self.section;
        self.section = SectionKind::Global;
        let v = self.expr(e).and_then(|v| {
            if matches!(v.ty, Ty::Int | Ty::Float) {
                self.convert(v, Ty::Float, e.span)
            } else {
                self.error(e.span, "`degree` is a real number");
                None
            }
        });
        self.section = saved;
        v
    }

    /// `global:` runs once per image, before any pixel: what belongs to
    /// a pixel has no value there.
    fn per_image_ok(&mut self, p: Predef, name: &str, span: Span) -> bool {
        let per_pixel = matches!(
            p,
            Predef::Pixel
                | Predef::Z
                | Predef::X
                | Predef::Y
                | Predef::ScreenPixel
                | Predef::Random
                | Predef::WhiteSq
                | Predef::Index
                | Predef::Solid
                | Predef::NumIter
                | Predef::Pass
        );
        if self.section == SectionKind::Global && per_pixel {
            self.error(span, format!("`{name}` belongs to a pixel; `global:` runs once per image"));
            return false;
        }
        true
    }

    /// A bare name read: a variable, `z` in a formula, or a predefined
    /// symbol or parameter written without its prefix.
    fn name(&mut self, n: &str, span: Span) -> Option<IExpr> {
        if let Some(&id) = self.var_ids.get(n) {
            if self.vars[id].is_array() {
                self.error(span, format!("`{n}` is an array: read an element, `{n}[i]`"));
                return None;
            }
            // The perturbation equations and the formula's own sections
            // share no variables: the reference orbit keeps only `#z`.
            if !self.readable(id, n, span) {
                return None;
            }
            return Some(IExpr { kind: IExprKind::Var(id), ty: self.vars[id].ty });
        }
        self.unresolved(n, span)
    }

    /// Whether variable `id` may be read in this section: the
    /// perturbation equations and the formula's own sections share none
    /// (the reference orbit keeps only `#z`).
    fn readable(&mut self, id: VarId, n: &str, span: Span) -> bool {
        {
            let var = &self.vars[id];
            if !var.global && var.perturb != self.in_perturb() {
                let why = if self.in_perturb() {
                    "the perturbation equations read `#z`, `#dz`, `#dpixel`, parameters and their own variables"
                } else {
                    "it belongs to the perturbation equations"
                };
                self.error(span, format!("`{n}` cannot be read here: {why}"));
                return false;
            }
        }
        true
    }

    /// A name that is no variable: `z`, a predefined name or parameter
    /// written bare.
    fn unresolved(&mut self, n: &str, span: Span) -> Option<IExpr> {
        if self.kind == EntryKind::Formula && n == "z" {
            if !self.per_image_ok(Predef::Z, n, span) {
                return None;
            }
            if self.section == SectionKind::Creases {
                self.error(span, "`creases` has the starting point, `#pixel`, not the iterate");
                return None;
            }
            return Some(predef_expr(Predef::Z));
        }
        if is_alias(n) {
            let p = self.predef(alias_target(n), span)?;
            return Some(predef_expr(p));
        }
        if is_fractint_param(n) {
            let id = self.param(n, span);
            return Some(IExpr { kind: IExprKind::Param(id), ty: self.params[id].ty() });
        }
        // Fractint's screen and view names: UF reads them too (UF-format
        // conversions of Fractint formulas test `whitesq`).
        {
            let p = match n {
                "whitesq" => Some(Predef::WhiteSq),
                "scrnpix" => Some(Predef::ScreenPixel),
                "scrnmax" => Some(Predef::ScrnMax),
                "ismand" => Some(Predef::IsMand),
                "center" => Some(Predef::Center),
                "magxmag" => Some(Predef::MagXMag),
                "rotskew" => Some(Predef::RotSkew),
                _ => None,
            };
            if let Some(p) = p {
                if !self.per_image_ok(p, n, span) {
                    return None;
                }
                return Some(predef_expr(p));
            }
        }
        // `LastSqr` changes what `sqr()` does, so it stays Fractint's.
        if FRACTINT_NAMES.contains(&n) {
            self.error(span, format!("`{n}` is Fractint's: set the language to Fractint"));
            return None;
        }
        self.error(span, format!("`{n}` is used before anything is assigned to it"));
        None
    }

    // ---- statements -------------------------------------------------------

    fn stmts(&mut self, body: &[Stmt]) -> Vec<IStmt> {
        let mut out = Vec::new();
        for s in body {
            self.stmt(s, &mut out);
        }
        out
    }

    fn stmt(&mut self, s: &Stmt, out: &mut Vec<IStmt>) {
        match s {
            Stmt::Decl { name, init, .. } => {
                if let Some(e) = init {
                    let Some(&id) = self.var_ids.get(name) else { return };
                    if self.vars[id].is_array() {
                        return;
                    }
                    if let Some(v) = self.expr(e) {
                        let ty = self.vars[id].ty;
                        if let Some(v) = self.convert(v, ty, e.span) {
                            out.push(IStmt::Assign(Place::Var(id), v));
                        }
                    }
                }
            }
            Stmt::Expr(e) => match &e.kind {
                ExprKind::Assign(target, value) => {
                    self.assign(target, value, out);
                }
                _ => self.warn(e.span, "this value is not used"),
            },
            Stmt::If { arms, otherwise, .. } => {
                let mut iarms = Vec::new();
                for (c, b) in arms {
                    let cond = self.expr(c).and_then(|v| self.to_bool(v, c.span));
                    let body = self.stmts(b);
                    if let Some(cond) = cond {
                        iarms.push((cond, body));
                    }
                }
                let otherwise = otherwise.as_ref().map(|b| self.stmts(b)).unwrap_or_default();
                out.push(IStmt::If { arms: iarms, otherwise });
            }
            Stmt::While { cond, body, .. } => {
                let c = self.expr(cond).and_then(|v| self.to_bool(v, cond.span));
                let body = self.stmts(body);
                if let Some(cond) = c {
                    out.push(IStmt::While { cond, body });
                }
            }
            Stmt::Repeat { body, cond, .. } => {
                let body = self.stmts(body);
                let c = self.expr(cond).and_then(|v| self.to_bool(v, cond.span));
                if let Some(cond) = c {
                    out.push(IStmt::Repeat { body, cond });
                }
            }
            Stmt::Map { call, input, out: names, span } => {
                // A solid's, outside `prepare`: the GPU's alone.
                if self.kind != EntryKind::Solid || self.section == SectionKind::Global {
                    self.error(*span, "a built-in variation runs on the GPU, per pixel: a solid calls it in `start`, `step` or `distance`, not in `prepare`");
                    return;
                }
                let mut parts = Vec::new();
                for e in input.iter() {
                    let Some(v) = self.expr(e) else { return };
                    let Some(v) = self.convert(v, Ty::Float, e.span) else { return };
                    parts.push(v);
                }
                let mut ids = [0usize; 3];
                for (k, n) in names.iter().enumerate() {
                    let Some(&id) = self.var_ids.get(n) else { return };
                    ids[k] = id;
                }
                let [x, y, z]: [IExpr; 3] = parts.try_into().expect("three parts");
                out.push(IStmt::Map { call: *call, input: Box::new([x, y, z]), out: ids });
            }
        }
    }

    /// `target = value`, with `a = b = c` chains unwound innermost first.
    /// Returns what reads the target back.
    fn assign(&mut self, target: &Expr, value: &Expr, out: &mut Vec<IStmt>) -> Option<IExpr> {
        // A whole array: UF's `x = y` of same-size arrays, element by
        // element.
        if let (ExprKind::Name(t), ExprKind::Name(v)) = (&target.kind, &value.kind) {
            if let (Some(&dst), Some(&src)) = (self.var_ids.get(t), self.var_ids.get(v)) {
                if self.vars[dst].is_array() || self.vars[src].is_array() {
                    self.copy_array(dst, src, target.span, out);
                    return None;
                }
            }
        }
        let v = match &value.kind {
            ExprKind::Assign(t2, v2) => self.assign(t2, v2, out)?,
            _ => self.expr(value)?,
        };
        if let ExprKind::Index(n, index) = &target.kind {
            let (id, flat) = self.element(n, index, target.span)?;
            if self.vars[id].global && self.section != SectionKind::Global {
                self.error(target.span, format!("`{n}` is set in `global:` and is read-only elsewhere"));
                return None;
            }
            if self.vars[id].perturb != self.in_perturb() {
                self.error(target.span, format!("`{n}` cannot be written here: the perturbation equations have their own variables"));
                return None;
            }
            let ty = self.vars[id].ty;
            let v = self.convert(v, ty, value.span)?;
            out.push(IStmt::SetElem { var: id, index: flat.clone(), value: v });
            return Some(IExpr { kind: IExprKind::Elem(id, Box::new(flat)), ty });
        }
        // Fractint's `sqr()` leaves its argument's squared modulus in
        // `LastSqr`: the last call evaluated, read from the values before
        // this assignment, so it is set first.
        if let (Some(ls), Some(arg)) = (self.lastsqr, last_sqr_argument(value)) {
            if let Some(a) = self.expr(arg) {
                let a = if a.ty == Ty::Int { self.convert(a, Ty::Float, arg.span) } else { Some(a) };
                if let Some(a) = a.filter(|a| a.ty != Ty::Bool) {
                    let m = IExpr { kind: IExprKind::Modulus(Box::new(a)), ty: Ty::Float };
                    if let Some(m) = self.convert(m, Ty::Complex, arg.span) {
                        out.push(IStmt::Assign(Place::Var(ls), m));
                    }
                }
            }
        }
        let (place, ty) = self.place(target)?;
        let v = self.convert(v, ty, value.span)?;
        out.push(IStmt::Assign(place, v));
        Some(IExpr { kind: place_read(place)?, ty })
    }

    /// `dst = src` for two arrays of the same dimensions and type: a loop
    /// over the elements, its counter a variable of its own.
    fn copy_array(&mut self, dst: VarId, src: VarId, span: Span, out: &mut Vec<IStmt>) {
        let (d, s) = (&self.vars[dst], &self.vars[src]);
        if d.dims != s.dims || d.ty != s.ty || !d.is_array() {
            let (dn, sn) = (d.name.clone(), s.name.clone());
            self.error(span, format!("`{dn} = {sn}`: an array is assigned whole from one of the same size and type, else by element"));
            return;
        }
        if self.vars[dst].global && self.section != SectionKind::Global {
            let n = self.vars[dst].name.clone();
            self.error(span, format!("`{n}` is set in `global:` and is read-only elsewhere"));
            return;
        }
        let (name, len, ty) = (format!("copy {}", self.vars.len()), self.vars[dst].len(), self.vars[dst].ty);
        self.new_var(&name, Ty::Int, span);
        let k = self.vars.len() - 1;
        let int = |kind: IExprKind| IExpr { kind, ty: Ty::Int };
        let i = || int(IExprKind::Var(k));
        let element = IExpr { kind: IExprKind::Elem(src, Box::new(i())), ty };
        out.push(IStmt::Assign(Place::Var(k), int(IExprKind::Int(0))));
        out.push(IStmt::While {
            cond: IExpr { kind: IExprKind::Binary(BinOp::Lt, Box::new(i()), Box::new(int(IExprKind::Int(len as i64)))), ty: Ty::Bool },
            body: vec![
                IStmt::SetElem { var: dst, index: i(), value: element },
                IStmt::Assign(Place::Var(k), int(IExprKind::Binary(BinOp::Add, Box::new(i()), Box::new(int(IExprKind::Int(1)))))),
            ],
        });
    }

    fn place(&mut self, target: &Expr) -> Option<(Place, Ty)> {
        match &target.kind {
            ExprKind::Name(n) => {
                if let Some(&id) = self.var_ids.get(n) {
                    if self.vars[id].is_array() {
                        self.error(target.span, format!("`{n}` is an array: assign its elements, `{n}[i] = ...`"));
                        return None;
                    }
                    if self.vars[id].global && self.section != SectionKind::Global {
                        self.error(target.span, format!("`{n}` is set in `global:` and is read-only elsewhere"));
                        return None;
                    }
                    if self.vars[id].perturb != self.in_perturb() {
                        self.error(target.span, format!("`{n}` cannot be written here: the perturbation equations have their own variables"));
                        return None;
                    }
                    return Some((Place::Var(id), self.vars[id].ty));
                }
                if self.kind == EntryKind::Formula && n == "z" {
                    if self.in_perturb() {
                        self.error(target.span, "`#z` is the reference orbit's here: write `#dz`");
                        return None;
                    }
                    return Some((Place::Z, Ty::Complex));
                }
                self.error(target.span, format!("`{n}` cannot be written"));
                None
            }
            ExprKind::Predef(n) => {
                let p = self.predef(n, target.span)?;
                let ok = match p {
                    Predef::Z => self.kind == EntryKind::Formula && !self.in_perturb() && self.section != SectionKind::Creases,
                    Predef::Index | Predef::Solid => self.section == SectionKind::Final,
                    Predef::DZ => true,
                    Predef::CreaseHeight | Predef::CreaseSlope => true,
                    Predef::Pixel | Predef::VarColor | Predef::PixelZ => self.section == SectionKind::Transform,
                    _ => false,
                };
                if !ok {
                    let why = match p {
                        Predef::Index | Predef::Solid => format!("`#{n}` is written in `final:`"),
                        Predef::Z if self.in_perturb() => "`#z` is the reference orbit's here: write `#dz`".to_string(),
                        _ => format!("`#{n}` cannot be written here"),
                    };
                    self.error(target.span, why);
                    return None;
                }
                Some(match p {
                    Predef::Z => (Place::Z, Ty::Complex),
                    Predef::Index => (Place::Index, Ty::Float),
                    Predef::DZ => (Place::DZ, Ty::Complex),
                    Predef::CreaseHeight => (Place::CreaseHeight, Ty::Float),
                    Predef::CreaseSlope => (Place::CreaseSlope, Ty::Complex),
                    Predef::Pixel => (Place::Pixel, Ty::Complex),
                    Predef::VarColor => (Place::Color, Ty::Float),
                    Predef::PixelZ => (Place::PixelZ, Ty::Float),
                    _ => (Place::Solid, Ty::Bool),
                })
            }
            ExprKind::Param(n) => {
                self.error(target.span, format!("`@{n}` is a parameter and cannot be written"));
                None
            }
            _ => {
                self.error(target.span, "only a variable can be assigned to");
                None
            }
        }
    }

    /// Fractint's loop: every statement but the last, and the last as
    /// the test (an assignment's value when it is one).
    fn fractint_loop(&mut self, body: &[Stmt]) -> (Vec<IStmt>, Option<IExpr>) {
        let Some((last, rest)) = body.split_last() else {
            return (Vec::new(), None);
        };
        let mut out = self.stmts(rest);
        let test = match last {
            Stmt::Expr(e) => match &e.kind {
                ExprKind::Assign(t, v) => {
                    let read = self.assign(t, v, &mut out);
                    read.and_then(|read| self.to_bool(read, e.span))
                }
                _ => self.expr(e).and_then(|v| self.to_bool(v, e.span)),
            },
            other => {
                let span = stmt_span(other);
                self.error(span, "the loop's last statement must be its bailout test");
                None
            }
        };
        (out, test)
    }

    fn bailout_section(&mut self, body: &[Stmt]) -> Option<IExpr> {
        match body {
            [Stmt::Expr(e)] if !matches!(e.kind, ExprKind::Assign(..)) => {
                let v = self.expr(e)?;
                self.to_bool(v, e.span)
            }
            [] => {
                self.error(Span::default(), "`bailout:` is empty");
                None
            }
            _ => {
                let span = stmt_span(&body[0]);
                self.error(span, "`bailout:` holds one test, nothing else");
                None
            }
        }
    }

    // ---- expressions ------------------------------------------------------

    fn expr(&mut self, e: &Expr) -> Option<IExpr> {
        let lit = |kind: IExprKind, ty: Ty| Some(IExpr { kind, ty });
        match &e.kind {
            ExprKind::Int(v) => lit(IExprKind::Int(*v), Ty::Int),
            ExprKind::Float(v) => lit(IExprKind::Float(*v), Ty::Float),
            ExprKind::Imag(v) => lit(IExprKind::Complex(0.0, *v), Ty::Complex),
            ExprKind::Bool(b) => lit(IExprKind::Bool(*b), Ty::Bool),
            // Compared with an enum parameter, a string is its item (see
            // `binary`); anywhere else it has no value.
            ExprKind::Str(_) => {
                self.error(e.span, "a string is not a value here");
                None
            }
            ExprKind::Complex(a, b) => {
                let re = self.expr(a)?;
                let im = self.expr(b)?;
                if re.ty == Ty::Complex || im.ty == Ty::Complex || re.ty == Ty::Bool || im.ty == Ty::Bool {
                    self.error(e.span, "`(re, im)` takes two real numbers");
                    return None;
                }
                if let (Some(r), Some(i)) = (const_real(&re), const_real(&im)) {
                    return lit(IExprKind::Complex(r, i), Ty::Complex);
                }
                let re = self.convert(re, Ty::Float, a.span)?;
                let im = self.convert(im, Ty::Float, b.span)?;
                lit(IExprKind::MakeComplex(Box::new(re), Box::new(im)), Ty::Complex)
            }
            ExprKind::Name(n) => self.name(n, e.span),
            ExprKind::Index(n, index) => {
                let (id, flat) = self.element(n, index, e.span)?;
                if !self.readable(id, n, e.span) {
                    return None;
                }
                lit(IExprKind::Elem(id, Box::new(flat)), self.vars[id].ty)
            }
            ExprKind::Predef(n) => {
                let p = self.predef(n, e.span)?;
                Some(predef_expr(p))
            }
            ExprKind::Param(n) => {
                if self.func_ids.contains_key(n) {
                    self.error(e.span, format!("`@{n}` is a function: call it, `@{n}(z)`"));
                    return None;
                }
                let id = self.param(n, e.span);
                lit(IExprKind::Param(id), self.params[id].ty())
            }
            ExprKind::Call { name, args, user } => self.call(name, args, *user, e.span),
            ExprKind::Unary(op, inner) => {
                let v = self.expr(inner)?;
                match op {
                    UnOp::Neg => {
                        if v.ty == Ty::Bool {
                            self.error(e.span, "a bool cannot be negated with `-`; use `!`");
                            return None;
                        }
                        // Fold a negated literal: `2^-1` must stay a
                        // constant exponent.
                        let ty = v.ty;
                        Some(match v.kind {
                            IExprKind::Int(i) => IExpr { kind: IExprKind::Int(-i), ty },
                            IExprKind::Float(f) => IExpr { kind: IExprKind::Float(-f), ty },
                            IExprKind::Complex(re, im) => IExpr { kind: IExprKind::Complex(-re, -im), ty },
                            kind => IExpr { kind: IExprKind::Unary(UnOp::Neg, Box::new(IExpr { kind, ty })), ty },
                        })
                    }
                    UnOp::Not => {
                        let b = self.to_bool(v, inner.span)?;
                        lit(IExprKind::Unary(UnOp::Not, Box::new(b)), Ty::Bool)
                    }
                }
            }
            ExprKind::Binary(op, a, b) => self.binary(*op, a, b, e.span),
            ExprKind::Modulus(inner) => {
                let v = self.expr(inner)?;
                if v.ty == Ty::Bool {
                    self.error(e.span, "`|...|` takes a number");
                    return None;
                }
                let v = if v.ty == Ty::Int { self.convert(v, Ty::Float, inner.span)? } else { v };
                lit(IExprKind::Modulus(Box::new(v)), Ty::Float)
            }
            ExprKind::Assign(..) => {
                self.error(e.span, "an assignment inside an expression is not supported yet");
                None
            }
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], user: bool, span: Span) -> Option<IExpr> {
        if self.rhai && !user && matches!(name, "to_float" | "to_int") {
            return self.rhai_conversion(name, args, span);
        }
        let known = user || is_fractint_fn(name) || Builtin::from_name(name).is_some();
        if known && args.len() != 1 {
            self.error(span, format!("`{name}` takes one argument"));
            return None;
        }
        if user || is_fractint_fn(name) {
            let arg = self.expr(&args[0])?;
            if self.param_ids.contains_key(name) {
                self.error(span, format!("`@{name}` is a parameter, not a function"));
                return None;
            }
            let id = self.func(name, span);
            let arg = self.convert(arg, Ty::Complex, args[0].span)?;
            return Some(IExpr { kind: IExprKind::UserCall(id, Box::new(arg)), ty: Ty::Complex });
        }
        let Some(b) = Builtin::from_name(name) else {
            let hint = match name {
                "rgb" | "rgba" | "hsl" | "hsla" | "red" | "green" | "blue" | "hue" | "sat" | "lum" | "alpha" | "gradient" | "blend" | "compose" => {
                    " (colour functions are not supported yet)"
                }
                "srand" => " (Fractint's `srand` is not supported yet)",
                _ => "",
            };
            self.error(span, format!("unknown function `{name}`{hint}"));
            return None;
        };
        let arg = self.expr(&args[0])?;
        // Rhai's `abs` of a complex is its modulus (UF's folds each
        // part; Rhai formulas spell that `fold`).
        let b = if self.rhai && b == Builtin::Abs && arg.ty == Ty::Complex { Builtin::Cabs } else { b };
        let ty = match b.result_ty(arg.ty) {
            Ok(t) => t,
            Err(msg) => {
                self.error(span, msg);
                return None;
            }
        };
        // An int argument becomes a float unless the result is an int.
        let arg = if arg.ty == Ty::Int && ty != Ty::Int { self.convert(arg, Ty::Float, args[0].span)? } else { arg };
        Some(IExpr { kind: IExprKind::Call(b, Box::new(arg)), ty })
    }

    /// Rhai's conversions, the explicit way a Rhai formula turns a bool
    /// into a number: `to_float` of a bool (1 for true) or an int,
    /// `to_int` of a bool or of a float (truncated).
    fn rhai_conversion(&mut self, name: &str, args: &[Expr], span: Span) -> Option<IExpr> {
        if args.len() != 1 {
            self.error(span, format!("`{name}` takes one argument"));
            return None;
        }
        let arg = self.expr(&args[0])?;
        let to = if name == "to_float" { Ty::Float } else { Ty::Int };
        match (arg.ty, to) {
            (t, to) if t == to => Some(arg),
            (Ty::Bool, _) => Some(IExpr { kind: IExprKind::Convert(Box::new(arg)), ty: to }),
            (Ty::Int, Ty::Float) => self.convert(arg, Ty::Float, span),
            (Ty::Float, Ty::Int) => Some(IExpr { kind: IExprKind::Call(Builtin::Trunc, Box::new(arg)), ty: Ty::Int }),
            (t, _) => {
                self.error(span, format!("`{name}` takes a bool, an int or a float, not a {}", t.name()));
                None
            }
        }
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr, span: Span) -> Option<IExpr> {
        // UF compares an enum parameter with an item's name.
        if matches!(op, BinOp::Eq | BinOp::Ne) {
            let named = match (&a.kind, &b.kind) {
                (ExprKind::Param(p), ExprKind::Str(s)) | (ExprKind::Str(s), ExprKind::Param(p)) => Some((p, s)),
                _ => None,
            };
            if let Some((p, s)) = named {
                let Some(&id) = self.param_ids.get(p.as_str()).filter(|&&id| self.params[id].kind == ParamKind::Enum) else {
                    self.error(span, format!("only an enum parameter compares with a string, and `@{p}` is not one"));
                    return None;
                };
                let choices = &self.params[id].choices;
                let item = choices.iter().position(|c| c == s).or_else(|| choices.iter().position(|c| c.eq_ignore_ascii_case(s)));
                let Some(item) = item else {
                    self.error(span, format!("\"{s}\" is not one of `@{p}`'s items"));
                    return None;
                };
                let l = IExpr { kind: IExprKind::Param(id), ty: Ty::Int };
                let r = IExpr { kind: IExprKind::Int(item as i64), ty: Ty::Int };
                return Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: Ty::Bool });
            }
        }
        let l = self.expr(a)?;
        let r = self.expr(b)?;
        match op {
            BinOp::DiffAbs => {
                self.error(span, "`diffabs` is the compiler's own".to_string());
                None
            }
            // Rhai's: on bools logic, on ints bits.
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor if l.ty == Ty::Bool && r.ty == Ty::Bool => {
                let op = match op {
                    BinOp::BitAnd => BinOp::And,
                    BinOp::BitOr => BinOp::Or,
                    _ => BinOp::Ne,
                };
                Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: Ty::Bool })
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if l.ty != Ty::Int || r.ty != Ty::Int {
                    self.error(span, format!("`{}` takes two ints (a float's bits are not a formula's)", op.symbol()));
                    return None;
                }
                Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: Ty::Int })
            }
            BinOp::And | BinOp::Or => {
                let l = self.to_bool(l, a.span)?;
                let r = self.to_bool(r, b.span)?;
                Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: Ty::Bool })
            }
            BinOp::Eq | BinOp::Ne if l.ty == Ty::Bool && r.ty == Ty::Bool => {
                Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: Ty::Bool })
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let l = self.numeric(l, a.span)?;
                let r = self.numeric(r, b.span)?;
                let t = l.ty.join(r.ty);
                let l = self.convert(l, t, a.span)?;
                let r = self.convert(r, t, b.span)?;
                Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: Ty::Bool })
            }
            BinOp::Pow => {
                let l = self.numeric(l, a.span)?;
                let r = self.numeric(r, b.span)?;
                // A constant integer exponent multiplies out.
                if let Some(n) = const_int(&r).filter(|n| n.abs() <= 64) {
                    let t = l.ty.join(Ty::Float);
                    let l = self.convert(l, t, a.span)?;
                    return Some(IExpr { kind: IExprKind::PowInt(Box::new(l), n), ty: t });
                }
                let t = l.ty.join(r.ty).join(Ty::Float);
                let l = self.convert(l, t, a.span)?;
                let r = self.convert(r, t, b.span)?;
                Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: t })
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                let l = self.numeric(l, a.span)?;
                let r = self.numeric(r, b.span)?;
                let mut t = l.ty.join(r.ty);
                if op == BinOp::Div {
                    t = t.join(Ty::Float);
                }
                if op == BinOp::Mod && t == Ty::Complex {
                    self.error(span, "`%` takes real numbers");
                    return None;
                }
                let l = self.convert(l, t, a.span)?;
                let r = self.convert(r, t, b.span)?;
                Some(IExpr { kind: IExprKind::Binary(op, Box::new(l), Box::new(r)), ty: t })
            }
        }
    }

    /// A bool used as a number becomes 1 or 0, with a warning.
    fn numeric(&mut self, v: IExpr, span: Span) -> Option<IExpr> {
        if v.ty == Ty::Bool {
            if !self.bool_mix(span) {
                return None;
            }
            return Some(IExpr { kind: IExprKind::Convert(Box::new(v)), ty: Ty::Int });
        }
        Some(v)
    }

    fn to_bool(&mut self, v: IExpr, span: Span) -> Option<IExpr> {
        if v.ty == Ty::Bool {
            return Some(v);
        }
        if !self.bool_mix(span) {
            return None;
        }
        Some(IExpr { kind: IExprKind::Convert(Box::new(v)), ty: Ty::Bool })
    }

    /// A bool used as a number or the reverse: UF's and Fractint's
    /// numbers are their booleans (a warning in UF files); Rhai's are
    /// not (an error).
    fn bool_mix(&mut self, span: Span) -> bool {
        if self.rhai {
            self.error(span, "a bool and a number do not mix: compare (`x != 0`) or choose (`if`)");
            return false;
        }
        self.bool_warning(span);
        true
    }

    fn bool_warning(&mut self, span: Span) {
        // Fractint's numbers ARE its booleans; say so once, in UF files.
        if self.dialect == Dialect::UltraFractal && !self.warned_bool {
            self.warned_bool = true;
            self.warn(span, "a number used as a bool (or the reverse); nonzero is true");
        }
    }

    /// Convert to `to`: widening, or to and from bool. Narrowing is an
    /// error, as in UF.
    fn convert(&mut self, v: IExpr, to: Ty, span: Span) -> Option<IExpr> {
        if v.ty == to {
            return Some(v);
        }
        if v.ty == Ty::Bool || to == Ty::Bool {
            if !self.bool_mix(span) {
                return None;
            }
            return Some(IExpr { kind: IExprKind::Convert(Box::new(v)), ty: to });
        }
        if v.ty.rank() > to.rank() {
            self.error(span, format!("cannot convert {} to {} (use `real`, `round` and the like)", v.ty.name(), to.name()));
            return None;
        }
        // Constants convert here, so the back ends see literals.
        Some(match v.kind {
            IExprKind::Int(i) if to == Ty::Float => IExpr { kind: IExprKind::Float(i as f64), ty: to },
            IExprKind::Int(i) => IExpr { kind: IExprKind::Complex(i as f64, 0.0), ty: to },
            IExprKind::Float(f) if to == Ty::Complex => IExpr { kind: IExprKind::Complex(f, 0.0), ty: to },
            kind => IExpr { kind: IExprKind::Convert(Box::new(IExpr { kind, ty: v.ty })), ty: to },
        })
    }

    // ---- a colouring's state ------------------------------------------------

    /// Which variables the colouring keeps per pixel, packed small, and
    /// whether they fit.
    fn pack_state(&mut self, p: &mut Program, span: Span) {
        let mut state: BTreeSet<VarId> = BTreeSet::new();
        // Read in an iteration before being written in it: carried from
        // the last iteration, or from init. A global is the image's, not
        // the pixel's.
        let mut loop_flow = Flow::default();
        flow(&p.loop_body, &mut loop_flow);
        state.extend(loop_flow.exposed.iter().copied().filter(|&v| !p.vars[v].global));
        // Read in final, written before it.
        let mut written: HashSet<VarId> = HashSet::new();
        let mut init_flow = Flow::default();
        flow(&p.init, &mut init_flow);
        written.extend(init_flow.any_write.iter().copied());
        written.extend(loop_flow.any_write.iter().copied());
        let mut final_flow = Flow::default();
        flow(&p.final_body, &mut final_flow);
        for v in &final_flow.exposed {
            if written.contains(v) {
                state.insert(*v);
            }
        }
        // A complex whose every assignment is real keeps one float. The
        // fixpoint runs over every complex variable, temporaries too: a
        // state variable is only as real as what is added to it.
        let mut real: HashSet<VarId> = (0..p.vars.len()).filter(|&v| p.vars[v].ty == Ty::Complex && !p.vars[v].is_array()).collect();
        let mut assigns: Vec<(VarId, IExpr)> = Vec::new();
        collect_assigns(&p.global_body, &mut assigns);
        collect_assigns(&p.init, &mut assigns);
        collect_assigns(&p.loop_body, &mut assigns);
        collect_assigns(&p.final_body, &mut assigns);
        loop {
            let before = real.len();
            for (v, e) in &assigns {
                if real.contains(v) && !real_valued(e, &real) {
                    real.remove(v);
                }
            }
            if real.len() == before {
                break;
            }
        }
        for v in &state {
            if real.contains(v) {
                p.vars[*v].stored_real = true;
            }
        }
        p.state = state.into_iter().collect();
        if p.state_floats() > STATE_CAP_FLOATS {
            let list: Vec<String> = p
                .state
                .iter()
                .map(|&v| {
                    let var = &p.vars[v];
                    let n = if var.stored_real { 1 } else { var.floats() };
                    format!("`{}` ({n})", var.name)
                })
                .collect();
            self.error(
                span,
                format!(
                    "this colouring keeps {} floats per pixel and {STATE_CAP_FLOATS} fit: {}. \
                     Declare real variables `float` (an undeclared one is complex, two floats)",
                    p.state_floats(),
                    list.join(", ")
                ),
            );
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ConstVal {
    Bool(bool),
    Int(i64),
    Float(f64),
    Complex(f64, f64),
}

impl ConstVal {
    fn as_f64(self) -> Option<f64> {
        match self {
            ConstVal::Int(v) => Some(v as f64),
            ConstVal::Float(v) => Some(v),
            _ => None,
        }
    }

    fn as_complex(self) -> Option<(f64, f64)> {
        match self {
            ConstVal::Int(v) => Some((v as f64, 0.0)),
            ConstVal::Float(v) => Some((v, 0.0)),
            ConstVal::Complex(re, im) => Some((re, im)),
            ConstVal::Bool(_) => None,
        }
    }
}

fn string_setting(s: Option<&Setting>) -> Option<String> {
    match &s?.value {
        SettingValue::Strings(v) => Some(v.join(" ")),
        _ => None,
    }
}

fn user_function_index(b: Builtin) -> usize {
    USER_FUNCTIONS.iter().position(|u| *u == b).expect("a user function")
}

/// Names UF accepts without `#` for Fractint's sake.
/// Whether any statement reads or writes the bare name `n`.
fn mentions(body: &[Stmt], n: &str) -> bool {
    fn e(x: &Expr, n: &str) -> bool {
        match &x.kind {
            ExprKind::Name(m) => m == n,
            ExprKind::Complex(a, b) | ExprKind::Binary(_, a, b) | ExprKind::Assign(a, b) => e(a, n) || e(b, n),
            ExprKind::Unary(_, a) | ExprKind::Modulus(a) => e(a, n),
            ExprKind::Call { args, .. } => args.iter().any(|a| e(a, n)),
            ExprKind::Index(m, index) => m == n || index.iter().any(|a| e(a, n)),
            _ => false,
        }
    }
    body.iter().any(|s| match s {
        Stmt::Decl { name, init, dims, .. } => {
            name == n || init.as_ref().is_some_and(|i| e(i, n)) || dims.as_ref().is_some_and(|d| d.iter().any(|x| e(x, n)))
        }
        Stmt::Expr(x) => e(x, n),
        Stmt::If { arms, otherwise, .. } => {
            arms.iter().any(|(c, b)| e(c, n) || mentions(b, n)) || otherwise.as_ref().is_some_and(|b| mentions(b, n))
        }
        Stmt::While { cond, body, .. } | Stmt::Repeat { body, cond, .. } => e(cond, n) || mentions(body, n),
        Stmt::Map { input, out, .. } => input.iter().any(|x| e(x, n)) || out.iter().any(|o| o == n),
    })
}

/// The argument of the last `sqr()` call an expression evaluates (its
/// arguments first, left to right).
fn last_sqr_argument(x: &Expr) -> Option<&Expr> {
    let inner = match &x.kind {
        ExprKind::Complex(a, b) | ExprKind::Binary(_, a, b) | ExprKind::Assign(a, b) => {
            last_sqr_argument(b).or_else(|| last_sqr_argument(a))
        }
        ExprKind::Unary(_, a) | ExprKind::Modulus(a) => last_sqr_argument(a),
        ExprKind::Call { args, .. } | ExprKind::Index(_, args) => args.iter().rev().find_map(last_sqr_argument),
        _ => None,
    };
    match &x.kind {
        // This call comes after its own argument's calls.
        ExprKind::Call { name, args, user: false } if name == "sqr" && args.len() == 1 => Some(&args[0]),
        _ => inner,
    }
}

/// An array size: a constant integer expression, its literals and
/// arithmetic folded; otherwise why not ([`settings_size`]: empty for a
/// size the settings fix, which `Checker::array_dims` folds).
fn const_size(e: &Expr) -> Result<i64, String> {
    let fold = |a: &Expr, b: &Expr, f: fn(i64, i64) -> Option<i64>| -> Result<i64, String> {
        f(const_size(a)?, const_size(b)?).ok_or_else(|| "this size does not fold to an integer".to_string())
    };
    match &e.kind {
        ExprKind::Int(v) => Ok(*v),
        ExprKind::Unary(UnOp::Neg, a) => Ok(-const_size(a)?),
        ExprKind::Binary(BinOp::Add, a, b) => fold(a, b, i64::checked_add),
        ExprKind::Binary(BinOp::Sub, a, b) => fold(a, b, i64::checked_sub),
        ExprKind::Binary(BinOp::Mul, a, b) => fold(a, b, i64::checked_mul),
        _ => Err(settings_size(e)),
    }
}

/// Why a size cannot fold at the settings, or empty when it can: it
/// reads only what they fix before the image -- numbers, parameters,
/// `#maxiter`, `#pi` and `#e`, arithmetic and the built-ins.
fn settings_size(e: &Expr) -> String {
    let first = |xs: &[&Expr]| xs.iter().map(|x| settings_size(x)).find(|w| !w.is_empty()).unwrap_or_default();
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Param(_) => String::new(),
        ExprKind::Predef(n) if matches!(n.as_str(), "maxiter" | "maxit" | "pi" | "e") => String::new(),
        ExprKind::Predef(n) if matches!(n.as_str(), "width" | "height") => {
            format!("an array sized by `#{n}` is not supported: the picture's size differs between the view, an export and its tiles")
        }
        ExprKind::Predef(n) => format!("an array's size reads numbers, parameters and `#maxiter`, not `#{n}`"),
        ExprKind::Unary(UnOp::Neg, a) => settings_size(a),
        ExprKind::Binary(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow, a, b) => first(&[a, b]),
        ExprKind::Call { args, user: false, .. } if !args.is_empty() => first(&args.iter().collect::<Vec<_>>()),
        _ => "an array's size is a constant integer expression: numbers, parameters and `#maxiter`".to_string(),
    }
}

fn is_alias(n: &str) -> bool {
    matches!(n, "pixel" | "maxit" | "rand" | "pi" | "e")
}

fn alias_target(n: &str) -> &str {
    match n {
        "maxit" => "maxiter",
        "rand" => "random",
        other => other,
    }
}

fn is_fractint_param(n: &str) -> bool {
    matches!(n, "p1" | "p2" | "p3" | "p4" | "p5" | "p6")
}

fn is_fractint_fn(n: &str) -> bool {
    matches!(n, "fn1" | "fn2" | "fn3" | "fn4")
}

fn predef_expr(p: Predef) -> IExpr {
    IExpr { kind: IExprKind::Predef(p), ty: p.ty() }
}

fn place_read(p: Place) -> Option<IExprKind> {
    Some(match p {
        Place::Var(v) => IExprKind::Var(v),
        Place::Z => IExprKind::Predef(Predef::Z),
        Place::Index => IExprKind::Predef(Predef::Index),
        Place::Solid => IExprKind::Predef(Predef::Solid),
        Place::DZ => IExprKind::Predef(Predef::DZ),
        Place::CreaseHeight => IExprKind::Predef(Predef::CreaseHeight),
        Place::CreaseSlope => IExprKind::Predef(Predef::CreaseSlope),
        Place::Pixel => IExprKind::Predef(Predef::Pixel),
        Place::Color => IExprKind::Predef(Predef::VarColor),
        Place::PixelZ => IExprKind::Predef(Predef::PixelZ),
    })
}

fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Decl { span, .. } | Stmt::If { span, .. } | Stmt::While { span, .. } | Stmt::Repeat { span, .. } | Stmt::Map { span, .. } => *span,
        Stmt::Expr(e) => e.span,
    }
}

fn const_real(e: &IExpr) -> Option<f64> {
    match e.kind {
        IExprKind::Int(v) => Some(v as f64),
        IExprKind::Float(v) => Some(v),
        _ => None,
    }
}

fn const_int(e: &IExpr) -> Option<i64> {
    match e.kind {
        IExprKind::Int(v) => Some(v),
        IExprKind::Float(v) if v.fract() == 0.0 && v.abs() < 1e9 => Some(v as i64),
        IExprKind::Complex(re, 0.0) if re.fract() == 0.0 && re.abs() < 1e9 => Some(re as i64),
        _ => None,
    }
}

/// Reads and writes of a statement list, in order.
#[derive(Default)]
pub(crate) struct Flow {
    /// Definitely written so far.
    pub(crate) written: HashSet<VarId>,
    /// Array elements definitely written so far, at constant indices: a
    /// later read of one reads this pass's value (a scratch array is no
    /// state).
    pub(crate) written_elems: HashSet<(VarId, i64)>,
    /// Written on any path.
    pub(crate) any_write: HashSet<VarId>,
    /// Read where an earlier write is not certain.
    pub(crate) exposed: HashSet<VarId>,
}

pub(crate) fn flow(stmts: &[IStmt], f: &mut Flow) {
    for s in stmts {
        match s {
            IStmt::Assign(place, e) => {
                reads(e, f);
                if let Place::Var(v) = place {
                    f.written.insert(*v);
                    f.any_write.insert(*v);
                }
            }
            // One element written: the array is not, so a later read of it
            // still reads what came before.
            IStmt::SetElem { var, index, value } => {
                reads(index, f);
                reads(value, f);
                f.any_write.insert(*var);
                if let IExprKind::Int(k) = index.kind {
                    f.written_elems.insert((*var, k));
                }
            }
            IStmt::If { arms, otherwise } => {
                let start = (f.written.clone(), f.written_elems.clone());
                let mut after: Option<(HashSet<VarId>, HashSet<(VarId, i64)>)> = None;
                for (cond, body) in arms {
                    // Each test runs with only what came before the
                    // `if`: an earlier arm's body did not run.
                    (f.written, f.written_elems) = start.clone();
                    reads(cond, f);
                    flow(body, f);
                    after = Some(match after {
                        None => (f.written.clone(), f.written_elems.clone()),
                        Some((a, e)) => (a.intersection(&f.written).copied().collect(), e.intersection(&f.written_elems).copied().collect()),
                    });
                }
                (f.written, f.written_elems) = start.clone();
                flow(otherwise, f);
                let (w, we) = (f.written.clone(), f.written_elems.clone());
                (f.written, f.written_elems) = match after {
                    Some((a, e)) => (a.intersection(&w).copied().collect(), e.intersection(&we).copied().collect()),
                    None => (w, we),
                };
            }
            IStmt::While { cond, body } => {
                reads(cond, f);
                let start = (f.written.clone(), f.written_elems.clone());
                flow(body, f);
                reads(cond, f);
                // The body may not run at all.
                (f.written, f.written_elems) = start;
            }
            IStmt::Repeat { body, cond } => {
                flow(body, f);
                reads(cond, f);
            }
            IStmt::Map { input, out, .. } => {
                for x in input.iter() {
                    reads(x, f);
                }
                for v in out {
                    f.written.insert(*v);
                    f.any_write.insert(*v);
                }
            }
        }
    }
}

pub(crate) fn reads(e: &IExpr, f: &mut Flow) {
    match &e.kind {
        IExprKind::Var(v) => {
            if !f.written.contains(v) {
                f.exposed.insert(*v);
            }
        }
        IExprKind::Elem(v, index) => {
            let known = matches!(index.kind, IExprKind::Int(k) if f.written_elems.contains(&(*v, k)));
            if !f.written.contains(v) && !known {
                f.exposed.insert(*v);
            }
            reads(index, f);
        }
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) | IExprKind::UserJac(_, a, _) => reads(a, f),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => {
            reads(a, f);
            reads(b, f);
        }
        IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) | IExprKind::Param(_) | IExprKind::Predef(_) | IExprKind::FuncChoice(_) => {}
    }
}

fn collect_assigns(stmts: &[IStmt], out: &mut Vec<(VarId, IExpr)>) {
    for s in stmts {
        match s {
            IStmt::Assign(Place::Var(v), e) => out.push((*v, e.clone())),
            IStmt::Assign(..) | IStmt::SetElem { .. } | IStmt::Map { .. } => {}
            IStmt::If { arms, otherwise } => {
                for (_, b) in arms {
                    collect_assigns(b, out);
                }
                collect_assigns(otherwise, out);
            }
            IStmt::While { body, .. } | IStmt::Repeat { body, .. } => collect_assigns(body, out),
        }
    }
}

/// Whether a complex expression's imaginary part is zero for every
/// input. Conservative: `sqrt`, `log` and the inverse functions leave
/// the real line, so a real argument does not make them real.
fn real_valued(e: &IExpr, real: &HashSet<VarId>) -> bool {
    if e.ty != Ty::Complex {
        return true;
    }
    match &e.kind {
        IExprKind::Complex(_, im) => *im == 0.0,
        IExprKind::Var(v) => real.contains(v),
        IExprKind::Convert(a) => a.ty != Ty::Complex,
        IExprKind::MakeComplex(_, im) => matches!(im.kind, IExprKind::Float(v) if v == 0.0),
        IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) => real_valued(a, real),
        IExprKind::Binary(op, a, b) => {
            matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) && real_valued(a, real) && real_valued(b, real)
        }
        IExprKind::Call(f, a) => {
            use Builtin::*;
            match f {
                Zero | Real | Imag | Cabs | Atan2 => true,
                Sqr | Exp | Expm1 | Sin | Cos | Tan | Cotan | Sinh | Cosh | Tanh | Cotanh | Atan | Asinh | Abs | Ident | Recip | Conj | Round | Trunc | Ceil | Floor | Cosxx => {
                    real_valued(a, real)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse_file;
    use super::*;

    fn compile(src: &str, kind: EntryKind) -> (Option<Program>, Vec<String>) {
        let (file, mut diags) = parse_file(src);
        let entry = file.entries.first().expect("an entry");
        let (p, d) = check(entry, kind, Dialect::UltraFractal);
        diags.extend(d);
        (p, diags.iter().map(|d| d.render(src)).collect())
    }

    fn ok(src: &str, kind: EntryKind) -> Program {
        let (p, d) = compile(src, kind);
        assert!(d.iter().all(|m| !m.contains("error")), "{d:?}");
        p.expect("a program")
    }

    fn errors(src: &str, kind: EntryKind) -> Vec<String> {
        let (p, d) = compile(src, kind);
        assert!(p.is_none(), "expected errors, got a program");
        d
    }

    #[test]
    fn a_uf_formula_checks_with_its_parameters() {
        let p = ok(
            "M {\ninit:\n z = 0\n complex c = #pixel\nloop:\n z = z^2 + c\nbailout:\n |z| < @bailout\n\
             default:\n title = \"M\"\n center = (-0.5, 0)\n magn = 1.5\n maxiter = 200\n\
             float param bailout\n  caption = \"Bail-out\"\n  default = 4.0\n  min = 1\n endparam\n}",
            EntryKind::Formula,
        );
        assert_eq!(p.params.len(), 1);
        assert_eq!((p.params[0].kind, p.params[0].default.0, p.params[0].min), (ParamKind::Float, 4.0, Some(1.0)));
        assert_eq!(p.vars.len(), 1, "z is the formula's own, c a variable");
        assert_eq!(p.vars[0].ty, Ty::Complex);
        assert!(matches!(p.loop_body[0], IStmt::Assign(Place::Z, _)));
        assert!(matches!(p.loop_body[0], IStmt::Assign(_, IExpr { ty: Ty::Complex, .. })));
        let IStmt::Assign(_, IExpr { kind: IExprKind::Binary(BinOp::Add, ref sq, _), .. }) = p.loop_body[0] else { panic!() };
        assert!(matches!(sq.kind, IExprKind::PowInt(_, 2)), "z^2 multiplies out");
        assert_eq!(p.view.center, Some((-0.5, 0.0)));
        assert_eq!((p.view.magn, p.view.maxiter), (Some(1.5), Some(200)));
        assert!(p.bailout.is_some());
    }

    #[test]
    fn fractint_names_and_shapes_resolve() {
        let p = ok("F { z = pixel, c = p1 : z = fn1(z) * c + p2, |z| <= 4 }", EntryKind::Formula);
        let names: Vec<_> = p.params.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["p1", "p2"]);
        assert_eq!(p.funcs.len(), 1);
        assert_eq!(USER_FUNCTIONS[p.funcs[0].default], Builtin::Sin, "Fractint's fn1 starts as sin");
        assert_eq!(p.init.len(), 2);
        assert_eq!(p.loop_body.len(), 1);
        // No `:` at all: the whole body is the loop.
        let p = ok("G { z = z*z + pixel, |z| < 4 }", EntryKind::Formula);
        assert!(p.init.is_empty());
        assert_eq!(p.loop_body.len(), 1);
    }

    #[test]
    fn types_widen_and_never_narrow() {
        let p = ok("T {\ninit:\n int i = 3\n float f = i + 0.5\n x = f / 2\n z = 0\nloop:\n z = z + x\nbailout:\n i < 4\n}", EntryKind::Formula);
        let ty = |n: &str| p.vars.iter().find(|v| v.name == n).unwrap().ty;
        assert_eq!((ty("i"), ty("f"), ty("x")), (Ty::Int, Ty::Float, Ty::Complex));
        let e = errors("T {\ninit:\n int i = 3.5\nloop:\n z = 1\nbailout:\n true\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("cannot convert float to int")), "{e:?}");
        let e = errors("T {\ninit:\n float f = (1, 2)\nloop:\n z = 1\nbailout:\n true\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("cannot convert complex to float")), "{e:?}");
        let p = ok("D {\ninit:\n x = 1 / 2\nloop:\n z = x\nbailout:\n true\n}", EntryKind::Formula);
        let IStmt::Assign(_, ref v) = p.init[0] else { panic!() };
        assert!(matches!(v.kind, IExprKind::Convert(ref inner) if inner.ty == Ty::Float), "int / int is a float");
    }

    #[test]
    fn what_cannot_be_read_or_written_is_reported() {
        let e = errors("A {\nloop:\n z = q\nbailout:\n true\n}", EntryKind::Formula);
        assert!(e[0].starts_with("3:6:") && e[0].contains("`q` is used before"), "{e:?}");
        let e = errors("A {\nloop:\n #pixel = 1\nbailout:\n true\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("cannot be written")), "{e:?}");
        let e = errors("A {\nloop:\n z = lastsqr\nbailout:\n true\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("`lastsqr` is Fractint's")), "{e:?}");
        let e = errors("A {\nloop:\n z = 1\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("needs a `bailout:`")), "{e:?}");
        let e = errors("A {\nloop:\n z = 1\nbailout:\n true\nfinal:\n #index = 0\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("no `final:` section")), "{e:?}");
        let e = errors("A {\nloop:\n z = rgb(1, 2, 3)\nbailout:\n true\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("colour functions")), "{e:?}");
        let e = errors("C {\nloop:\n #index = 1\nfinal:\n #index = 0\n}", EntryKind::Coloring);
        assert!(e.iter().any(|m| m.contains("belongs to `final:`")), "{e:?}");
    }

    /// `global:` runs first, once per image: what it sets is read-only
    /// elsewhere (UF's rule), what belongs to a pixel has no value in
    /// it, and only the globals the other sections read are packed.
    #[test]
    fn global_sections_set_read_only_values() {
        let p = ok(
            "G {\nglobal:\n float k = 2 * #pi\n int n = #maxiter\n tmp = @a * 3\n complex c = tmp + 1\n\
             init:\n z = 0\nloop:\n z = sqr(z) + #pixel * k + c\nbailout:\n |z| < n\n}",
            EntryKind::Formula,
        );
        let names: Vec<_> = p.globals.iter().map(|&v| p.vars[v].name.as_str()).collect();
        assert_eq!(names, ["k", "n", "c"], "`tmp` is read in `global:` alone");
        assert!(p.vars.iter().filter(|v| v.global).count() == 4);
        assert_eq!(p.global_floats(), 4);
        assert!(!p.global_body.is_empty());

        let e = errors("G {\nglobal:\n k = 1\nloop:\n k = 2\n z = k\nbailout:\n true\n}", EntryKind::Formula);
        assert!(e.iter().any(|m| m.contains("`k` is set in `global:` and is read-only")), "{e:?}");
        for sym in ["#pixel", "#z", "#x", "#random", "z"] {
            let src = format!("G {{\nglobal:\n k = {sym}\nloop:\n z = k\nbailout:\n true\n}}");
            let e = errors(&src, EntryKind::Formula);
            assert!(e.iter().any(|m| m.contains("`global:` runs once per image")), "{sym}: {e:?}");
        }
        // A colouring's globals share its parameter block: 31 complex
        // parameters and two complex globals are 66 floats.
        let sum: String = (1..=31).map(|i| format!(" + @p{i}")).collect();
        let src = format!("C {{\nglobal:\n a = 1\n b = 2\nfinal:\n #index = real(a + b{sum})\n}}");
        let e = errors(&src, EntryKind::Coloring);
        assert!(e.iter().any(|m| m.contains("the parameters and the `global:` values take 66")), "{e:?}");
        // And are never the pixel's state.
        let p = ok(
            "C {\nglobal:\n float k = 3\ninit:\n float s = 0\nloop:\n s = s + k\nfinal:\n #index = s\n}",
            EntryKind::Coloring,
        );
        let state: Vec<_> = p.state.iter().map(|&v| p.vars[v].name.as_str()).collect();
        assert_eq!(state, ["s"]);
    }

    /// The perturbation sections: `#dz` and `#dpixel` theirs alone,
    /// `#z` the reference's and read-only there, their own variables,
    /// and the `perturb` setting a bool of parameters and constants.
    #[test]
    fn perturbation_sections_and_their_symbols() {
        let p = ok(
            "P {\ninit:\n z = 0\nloop:\n z = z^@power + #pixel\nbailout:\n |z| < 4\nperturbinit:\n #dz = 0\n\
             perturbloop:\n t = 2 * #z\n #dz = t * #dz + sqr(#dz) + #dpixel\n\
             default:\n float param power\n  default = 2\n endparam\n perturb = @power == 2\n}",
            EntryKind::Formula,
        );
        assert_eq!((p.perturb_init.len(), p.perturb_loop.len()), (1, 2));
        assert!(p.perturb_when.is_some());
        assert!(p.vars.iter().any(|v| v.name == "t" && v.perturb));
        let cases = [
            ("loop:\n z = sqr(z) + #dz\nbailout:\n |z| < 4", "belongs to `perturbinit:` and `perturbloop:`"),
            ("loop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\nperturbloop:\n #z = #dz", "the reference orbit's here"),
            ("loop:\n w = z\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\nperturbloop:\n #dz = w", "cannot be read here"),
            ("loop:\n z = sqr(z) + #pixel + t\nbailout:\n |z| < 4\nperturbloop:\n t = 1\n #dz = t", "cannot be read here"),
            ("loop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\ndefault:\n perturb = #pixel", "belongs to a pixel"),
        ];
        for (body, want) in cases {
            let e = errors(&format!("E {{\n{body}\n}}"), EntryKind::Formula);
            assert!(e.iter().any(|m| m.contains(want)), "{body}: {e:?}");
        }
    }

    #[test]
    fn a_colouring_keeps_only_what_crosses_iterations_packed_small() {
        // Untyped `sum` and `count` stay real: one float each. `t` is a
        // temporary of one iteration and costs nothing.
        let p = ok(
            "Avg {\ninit:\n sum = 0\n count = 0\nloop:\n t = |#z|\n sum = sum + t\n count = count + 1\n\
             final:\n #index = real(sum / count)\n}",
            EntryKind::Coloring,
        );
        let state: Vec<_> = p.state.iter().map(|&v| p.vars[v].name.as_str()).collect();
        assert_eq!(state, ["sum", "count"]);
        assert!(p.state.iter().all(|&v| p.vars[v].stored_real));
        assert_eq!(p.state_floats(), 2);
        // A complex that really is complex keeps two.
        let p = ok("Last {\nloop:\n w = #z\nfinal:\n #index = cabs(w)\n}", EntryKind::Coloring);
        assert_eq!(p.state_floats(), 2);
        // sqrt leaves the real line: no narrowing.
        let p = ok("Sq {\ninit:\n s = 0\nloop:\n s = sqrt(s - 1)\nfinal:\n #index = real(s)\n}", EntryKind::Coloring);
        assert_eq!(p.state_floats(), 2);
        // Up to the cap, 32 floats: sixteen complex values.
        let vars = |n: usize| -> (String, String) {
            let set: String = (0..n).map(|i| format!(" v{i} = #z * {}\n", i + 1)).collect();
            let sum: String = (0..n).map(|i| format!(" + v{i}")).collect();
            (set, sum)
        };
        let (set, sum) = vars(16);
        let p = ok(&format!("Max {{\nloop:\n{set}final:\n #index = real(0{sum})\n}}"), EntryKind::Coloring);
        assert_eq!(p.state_floats(), 32);
        // Past it: named, with the way out.
        let (set, sum) = vars(17);
        let e = errors(&format!("Big {{\nloop:\n{set}final:\n #index = real(0{sum})\n}}"), EntryKind::Coloring);
        assert!(e.iter().any(|m| m.contains("keeps 34 floats") && m.contains("`v0` (2)")), "{e:?}");
        // An unlabelled colouring is all final.
        let p = ok("N {\n #index = #numiter / 10\n}", EntryKind::Coloring);
        assert!(p.state.is_empty() && p.final_body.len() == 1);
    }

    #[test]
    fn enum_and_function_blocks() {
        let p = ok(
            "E {\nloop:\n z = @f(z) + @mode\nbailout:\n |z| < 4\ndefault:\n\
             int param mode\n  enum = \"one\" \"two\" \"three\"\n  default = 1\n endparam\n\
             func f\n  default = cosh()\n endfunc\n}",
            EntryKind::Formula,
        );
        assert_eq!(p.params[0].kind, ParamKind::Enum);
        assert_eq!(p.params[0].choices, ["one", "two", "three"]);
        assert_eq!(USER_FUNCTIONS[p.funcs[0].default], Builtin::Cosh);
        assert_eq!(p.param_slots(), 2);
    }
    /// A size the settings fix (plan, item 3, step 3f): parameters and
    /// `#maxiter` fold at the defaults (the entry's own `maxiter`), or at
    /// the settings a check runs at; the size stays as written, so a
    /// stride follows it, and a constant index is clamped rather than
    /// refused where the size may change.
    #[test]
    fn sizes_fold_at_the_settings() {
        let src = "S {\nglobal:\n float t[#maxiter]\n complex m[2, @n]\ninit:\n z = 0\n float k[round(@w * 2.5) + 1]\n k[9] = 1\n\
                   loop:\n z = sqr(z) + #pixel + m[1, 2] * t[3] + k[1]\nbailout:\n |z| < 4\n\
                   default:\n maxiter = 50\n int param n\n  default = 3\n endparam\n float param w\n  default = 1\n endparam\n}";
        let dims = |p: &Program| -> Vec<Vec<u32>> { ["t", "m", "k"].iter().map(|n| p.vars.iter().find(|v| v.name == *n).unwrap().dims.clone()).collect() };
        let p = ok(src, EntryKind::Formula);
        assert_eq!(dims(&p), vec![vec![50], vec![2, 3], vec![4]]);
        assert!(p.sized_by_settings());
        assert_eq!(p.sizes_at(&[(5.0, 0.0), (2.0, 0.0)], 80), Some(vec![80, 2, 5, 6]));
        let settings = SizeSettings { params: [("n".to_string(), 5.0), ("w".to_string(), 2.0)].into_iter().collect(), maxiter: Some(80) };
        let q = at_settings(settings, || ok(src, EntryKind::Formula));
        assert_eq!(dims(&q), vec![vec![80], vec![2, 5], vec![6]]);
        // `m[1, 2]` reads `1 * @n + 2`, at any `@n`.
        fn strided(e: &IExpr) -> bool {
            match &e.kind {
                IExprKind::Elem(_, i) => matches!(&i.kind, IExprKind::Binary(BinOp::Add, a, _)
                    if matches!(&a.kind, IExprKind::Binary(BinOp::Mul, _, n) if matches!(n.kind, IExprKind::Param(_)))),
                IExprKind::Binary(_, a, b) => strided(a) || strided(b),
                IExprKind::Call(_, a) | IExprKind::Convert(a) | IExprKind::Unary(_, a) => strided(a),
                _ => false,
            }
        }
        let IStmt::Assign(_, e) = q.loop_body.last().unwrap() else { panic!("{:?}", q.loop_body) };
        assert!(strided(e), "{e:?}");
        // A constant program is not sized by the settings.
        assert!(!ok("C {\ninit:\n z = 0\n float a[3]\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}", EntryKind::Formula).sized_by_settings());
        // Past the cap at these settings, under 1 at those.
        let at = |n: f64| SizeSettings { params: [("n".to_string(), n)].into_iter().collect(), maxiter: None };
        let e = at_settings(at(200.0), || errors(src, EntryKind::Formula));
        assert!(e.iter().any(|m| m.contains("at most 256")), "{e:?}");
        let e = at_settings(at(0.0), || errors(src, EntryKind::Formula));
        assert!(e.iter().any(|m| m.contains("at least 1, not 0")), "{e:?}");
    }

    /// Static arrays (plan, item 3, step 3a): declared with constant
    /// sizes in any section, read and written by element at a flattened
    /// index; a global one packed with the globals, a colouring's kept
    /// whole in its state; and what is refused, and where.
    #[test]
    fn static_arrays_declare_index_and_pack() {
        let p = ok(
            "A {\nglobal:\n float k[3]\n complex m[2, 2 * 2 - 1]\n k[0] = 1\n m[1, 2] = (0, 1)\ninit:\n z = 0\n int h[4]\n\
             loop:\n h[1] = h[0] + 1\n z = sqr(z) * k[0] + #pixel + m[1, 2] * k[2]\nbailout:\n |z| < 4\n}",
            EntryKind::Formula,
        );
        let var = |n: &str| p.vars.iter().position(|v| v.name == n).expect(n);
        let (k, m, h) = (var("k"), var("m"), var("h"));
        assert_eq!((p.vars[k].dims.clone(), p.vars[m].dims.clone(), p.vars[h].dims.clone()), (vec![3], vec![2, 3], vec![4]));
        assert!(p.vars[k].global && p.vars[m].global && !p.vars[h].global);
        // The globals the loop reads, every element packed: 3 + 6 complex.
        assert_eq!(p.globals, vec![k, m]);
        assert_eq!(p.global_floats(), 3 + 12);
        // `m[1, 2]` flattens to 1 * 3 + 2.
        fn flat(e: &IExpr) -> Option<i64> {
            match &e.kind {
                IExprKind::Int(i) => Some(*i),
                IExprKind::Binary(BinOp::Add, a, b) => Some(flat(a)? + flat(b)?),
                IExprKind::Binary(BinOp::Mul, a, b) => Some(flat(a)? * flat(b)?),
                _ => None,
            }
        }
        let IStmt::SetElem { var: v, index, .. } = &p.global_body[1] else { panic!("{:?}", p.global_body[1]) };
        assert_eq!((*v, flat(index)), (m, Some(5)));

        // A colouring keeps an array it carries across iterations whole.
        let c = ok(
            "C {\ninit:\n float last[3]\nloop:\n last[2] = last[1]\n last[1] = last[0]\n last[0] = cabs(#z)\n\
             final:\n #index = last[0] + last[1] + last[2]\n}",
            EntryKind::Coloring,
        );
        assert_eq!(c.state_floats(), 3);

        let refused = |src: &str, want: &str| {
            let e = errors(src, EntryKind::Formula);
            assert!(e.iter().any(|m| m.contains(want)), "{src}: {e:?}");
        };
        let shell = |body: &str| format!("F {{\ninit:\n z = 0\n{body}\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}}");
        refused(&shell(" float a[]"), "dynamic arrays");
        refused(&shell(" float a[@n]"), "an array's size is an int, not a complex");
        refused(&shell(" float a[#width]"), "the picture's size differs");
        refused(&shell(" float a[#numiter]"), "not `#numiter`");
        refused(&shell(" int n = 2\n float a[n]"), "numbers, parameters and `#maxiter`");
        refused(&shell(" float a[0]"), "at least 1");
        refused(&shell(" complex a[200]"), "at most 256");
        refused(&shell(" float a[2] = 1"), "not set in its declaration");
        refused(&shell(" float a[2]\n float b = a"), "is an array: read an element");
        refused(&shell(" float a[2]\n a = 1"), "assign its elements");
        refused(&shell(" float a[2]\n float b[3]\n b = a"), "same size and type");
        refused(&shell(" float a[2]\n int b[2]\n b = a"), "same size and type");
        refused(&shell(" float a[2, 2]\n float b = a[1]"), "2 dimensions");
        refused(&shell(" float a[2]\n float b = a[1.5]"), "an array index is an int");
        refused(&shell(" float a[2]\n float b = a[2]"), "outside `a`");
        refused(&shell(" float b = c[0]"), "`c` is not an array");
        refused(
            "F {\nglobal:\n float k[2]\ninit:\n z = 0\n k[0] = 1\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| < 4\n}",
            "read-only elsewhere",
        );
    }
}
