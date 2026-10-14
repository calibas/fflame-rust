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
/// `vec4`, which the perturbed path and the recolour records store.
pub const STATE_CAP_FLOATS: usize = 4;
/// Floats of parameters each of a formula and a colouring may have
/// (the uniform's `fparams` / `cparams`, 4 vec4s).
pub const PARAM_SLOTS: usize = 16;
/// Floats of `global:` values a formula may have: the uniform's
/// `fdata` (64 vec4s), which nothing else of a custom formula's uses. A
/// colouring's share its parameter block, [`PARAM_SLOTS`] in all.
pub const FDATA_FLOATS: usize = 256;
/// The most times one `while` or `repeat` runs per evaluation; past it
/// the loop ends, so no formula can hang the GPU.
pub const LOOP_CAP: u32 = 4096;

/// Fractint's own predefined names, read in Fractint's dialect only.
const FRACTINT_NAMES: &[&str] = &["lastsqr", "whitesq", "scrnpix", "scrnmax", "ismand", "center", "magxmag", "rotskew"];

/// Check one entry as a formula or a colouring.
pub fn check(entry: &Entry, kind: EntryKind, dialect: Dialect) -> (Option<Program>, Vec<Diag>) {
    let mut c = Checker {
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
    };
    let program = c.run(entry);
    let failed = super::has_errors(&c.diags);
    (if failed { None } else { program }, c.diags)
}

struct Checker {
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
}

/// What a written parameter or predefined name copies.
#[derive(Clone, Debug)]
enum Shadowed {
    Param(String),
    Predef(String),
}

impl Checker {
    fn error(&mut self, span: Span, msg: impl Into<String>) {
        self.diags.push(Diag::error(span, msg));
    }

    fn warn(&mut self, span: Span, msg: impl Into<String>) {
        self.diags.push(Diag::warning(span, msg));
    }

    fn run(&mut self, entry: &Entry) -> Option<Program> {
        let sections = self.arrange(entry)?;
        self.read_blocks(&entry.default);
        let view = self.view_settings(&entry.default);
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
        let mut init = Vec::new();
        let mut loop_body = Vec::new();
        let mut final_body = Vec::new();
        let mut bailout = None;
        for (kind, body) in sections {
            self.section = kind.0;
            match kind {
                (SectionKind::Global, _) => global_body = self.stmts(body),
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
            bailout,
            final_body,
            region,
            view,
            state: Vec::new(),
            dz_var: None,
        };
        // The globals the other sections read are what the GPU needs.
        let mut f = Flow::default();
        flow(&program.init, &mut f);
        flow(&program.loop_body, &mut f);
        flow(&program.final_body, &mut f);
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
                    "the parameters and the `global:` values take {} floats; a colouring has {PARAM_SLOTS}                      (a complex takes two)",
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
                }
            }
            let allowed = match (self.kind, kind) {
                (_, SectionKind::Global) => true,
                (EntryKind::Formula, SectionKind::PerturbInit | SectionKind::PerturbLoop) => {
                    self.warn(s.span, format!("`{}:` is ignored: custom formulas do not perturb yet", kind.label()));
                    continue;
                }
                (EntryKind::Formula, SectionKind::Init | SectionKind::Loop | SectionKind::Bailout) => true,
                (EntryKind::Coloring, SectionKind::Init | SectionKind::Loop | SectionKind::Final) => true,
                _ => false,
            };
            if !allowed {
                let what = match self.kind {
                    EntryKind::Formula => "a formula",
                    EntryKind::Coloring => "a colouring",
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
        }
        // Execution order, whatever order they were written in.
        let rank = |k: SectionKind| match k {
            SectionKind::Global => 0,
            SectionKind::Init => 1,
            SectionKind::Loop => 2,
            SectionKind::Bailout => 3,
            _ => 4,
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
                Stmt::Decl { ty, name, span, init } => {
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
                    match self.var_ids.get(name) {
                        Some(&id) if self.vars[id].ty != ty => {
                            let was = self.vars[id].ty;
                            self.error(*span, format!("`{name}` is already a {}", was.name()));
                        }
                        Some(_) => {}
                        None => self.new_var(name, ty, *span),
                    }
                    if let Some(e) = init {
                        self.declare_in_expr(e);
                    }
                }
                Stmt::Expr(e) => self.declare_in_expr(e),
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

    fn new_var(&mut self, name: &str, ty: Ty, span: Span) {
        self.var_ids.insert(name.to_string(), self.vars.len());
        self.vars.push(Var { name: name.to_string(), ty, span, stored_real: false, global: false });
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
            "dz" | "dpixel" => {
                self.error(span, format!("`#{name}` belongs to the perturbation sections, which are not supported yet"));
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
        let colouring = self.kind == EntryKind::Coloring;
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
            return Some(IExpr { kind: IExprKind::Var(id), ty: self.vars[id].ty });
        }
        if self.kind == EntryKind::Formula && n == "z" {
            if !self.per_image_ok(Predef::Z, n, span) {
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
        if self.dialect == Dialect::Fractint {
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
        }
    }

    /// `target = value`, with `a = b = c` chains unwound innermost first.
    /// Returns the place written.
    fn assign(&mut self, target: &Expr, value: &Expr, out: &mut Vec<IStmt>) -> Option<(Place, Ty)> {
        let v = match &value.kind {
            ExprKind::Assign(t2, v2) => {
                let (place, ty) = self.assign(t2, v2, out)?;
                IExpr { kind: place_read(place)?, ty }
            }
            _ => self.expr(value)?,
        };
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
        Some((place, ty))
    }

    fn place(&mut self, target: &Expr) -> Option<(Place, Ty)> {
        match &target.kind {
            ExprKind::Name(n) => {
                if let Some(&id) = self.var_ids.get(n) {
                    if self.vars[id].global && self.section != SectionKind::Global {
                        self.error(target.span, format!("`{n}` is set in `global:` and is read-only elsewhere"));
                        return None;
                    }
                    return Some((Place::Var(id), self.vars[id].ty));
                }
                if self.kind == EntryKind::Formula && n == "z" {
                    return Some((Place::Z, Ty::Complex));
                }
                self.error(target.span, format!("`{n}` cannot be written"));
                None
            }
            ExprKind::Predef(n) => {
                let p = self.predef(n, target.span)?;
                let ok = match p {
                    Predef::Z => self.kind == EntryKind::Formula,
                    Predef::Index | Predef::Solid => self.section == SectionKind::Final,
                    _ => false,
                };
                if !ok {
                    let why = match p {
                        Predef::Index | Predef::Solid => format!("`#{n}` is written in `final:`"),
                        _ => format!("`#{n}` cannot be written here"),
                    };
                    self.error(target.span, why);
                    return None;
                }
                Some(match p {
                    Predef::Z => (Place::Z, Ty::Complex),
                    Predef::Index => (Place::Index, Ty::Float),
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
                    let placed = self.assign(t, v, &mut out);
                    placed.and_then(|(p, ty)| {
                        let read = IExpr { kind: place_read(p)?, ty };
                        self.to_bool(read, e.span)
                    })
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

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr, span: Span) -> Option<IExpr> {
        let l = self.expr(a)?;
        let r = self.expr(b)?;
        match op {
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
            self.bool_warning(span);
            return Some(IExpr { kind: IExprKind::Convert(Box::new(v)), ty: Ty::Int });
        }
        Some(v)
    }

    fn to_bool(&mut self, v: IExpr, span: Span) -> Option<IExpr> {
        if v.ty == Ty::Bool {
            return Some(v);
        }
        self.bool_warning(span);
        Some(IExpr { kind: IExprKind::Convert(Box::new(v)), ty: Ty::Bool })
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
            self.bool_warning(span);
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
        let mut real: HashSet<VarId> = (0..p.vars.len()).filter(|&v| p.vars[v].ty == Ty::Complex).collect();
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
                    let n = if var.stored_real { 1 } else { var.ty.floats() };
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
            _ => false,
        }
    }
    body.iter().any(|s| match s {
        Stmt::Decl { name, init, .. } => name == n || init.as_ref().is_some_and(|i| e(i, n)),
        Stmt::Expr(x) => e(x, n),
        Stmt::If { arms, otherwise, .. } => {
            arms.iter().any(|(c, b)| e(c, n) || mentions(b, n)) || otherwise.as_ref().is_some_and(|b| mentions(b, n))
        }
        Stmt::While { cond, body, .. } | Stmt::Repeat { body, cond, .. } => e(cond, n) || mentions(body, n),
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
        ExprKind::Call { args, .. } => args.iter().rev().find_map(last_sqr_argument),
        _ => None,
    };
    match &x.kind {
        // This call comes after its own argument's calls.
        ExprKind::Call { name, args, user: false } if name == "sqr" && args.len() == 1 => Some(&args[0]),
        _ => inner,
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
    })
}

fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Decl { span, .. } | Stmt::If { span, .. } | Stmt::While { span, .. } | Stmt::Repeat { span, .. } => *span,
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
struct Flow {
    /// Definitely written so far.
    written: HashSet<VarId>,
    /// Written on any path.
    any_write: HashSet<VarId>,
    /// Read where an earlier write is not certain.
    exposed: HashSet<VarId>,
}

fn flow(stmts: &[IStmt], f: &mut Flow) {
    for s in stmts {
        match s {
            IStmt::Assign(place, e) => {
                reads(e, f);
                if let Place::Var(v) = place {
                    f.written.insert(*v);
                    f.any_write.insert(*v);
                }
            }
            IStmt::If { arms, otherwise } => {
                let start = f.written.clone();
                let mut after: Option<HashSet<VarId>> = None;
                for (cond, body) in arms {
                    // Each test runs with only what came before the
                    // `if`: an earlier arm's body did not run.
                    f.written = start.clone();
                    reads(cond, f);
                    flow(body, f);
                    after = Some(match after {
                        None => f.written.clone(),
                        Some(a) => a.intersection(&f.written).copied().collect(),
                    });
                }
                f.written = start.clone();
                flow(otherwise, f);
                let w = f.written.clone();
                f.written = match after {
                    Some(a) => a.intersection(&w).copied().collect(),
                    None => w,
                };
            }
            IStmt::While { cond, body } => {
                reads(cond, f);
                let start = f.written.clone();
                flow(body, f);
                reads(cond, f);
                // The body may not run at all.
                f.written = start;
            }
            IStmt::Repeat { body, cond } => {
                flow(body, f);
                reads(cond, f);
            }
        }
    }
}

fn reads(e: &IExpr, f: &mut Flow) {
    match &e.kind {
        IExprKind::Var(v) => {
            if !f.written.contains(v) {
                f.exposed.insert(*v);
            }
        }
        IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => reads(a, f),
        IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) => {
            reads(a, f);
            reads(b, f);
        }
        IExprKind::Bool(_) | IExprKind::Int(_) | IExprKind::Float(_) | IExprKind::Complex(..) | IExprKind::Param(_) | IExprKind::Predef(_) => {}
    }
}

fn collect_assigns(stmts: &[IStmt], out: &mut Vec<(VarId, IExpr)>) {
    for s in stmts {
        match s {
            IStmt::Assign(Place::Var(v), e) => out.push((*v, e.clone())),
            IStmt::Assign(..) => {}
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
                Sqr | Exp | Sin | Cos | Tan | Cotan | Sinh | Cosh | Tanh | Cotanh | Atan | Asinh | Abs | Ident | Recip | Conj | Round | Trunc | Ceil | Floor | Cosxx => {
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
        // A colouring's globals share its 16-float parameter block.
        let e = errors(
            "C {\nglobal:\n a = 1\n b = 2\n c = 3\nfinal:\n #index = real(a + b + c + @p1 + @p2 + @p3 + @p4 + @p5 + @p6)\n}",
            EntryKind::Coloring,
        );
        assert!(e.iter().any(|m| m.contains("the parameters and the `global:` values take 18")), "{e:?}");
        // And are never the pixel's state.
        let p = ok(
            "C {\nglobal:\n float k = 3\ninit:\n float s = 0\nloop:\n s = s + k\nfinal:\n #index = s\n}",
            EntryKind::Coloring,
        );
        let state: Vec<_> = p.state.iter().map(|&v| p.vars[v].name.as_str()).collect();
        assert_eq!(state, ["s"]);
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
        // Past the cap: named, with the way out.
        let e = errors(
            "Big {\nloop:\n a = #z\n b = #z * 2\n c = #z * 3\nfinal:\n #index = real(a + b + c)\n}",
            EntryKind::Coloring,
        );
        assert!(e.iter().any(|m| m.contains("keeps 6 floats") && m.contains("`a` (2)")), "{e:?}");
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
}
