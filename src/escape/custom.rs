//! Formulas and colourings compiled from source at runtime
//! (`escape::lang`), registered beside the built-ins.
//!
//! Every cache in the escape engine keys on a formula's or colouring's
//! NAME (the pipelines, the records, the terrain's pictures), so a
//! compiled entry is registered as a leaked [`FormulaDef`] /
//! [`ColoringDef`] under a name that changes with its source:
//! `custom-` and a hash of the entry. [`get_formula`](super::get_formula)
//! and [`get_coloring`](super::get_coloring) look here after the
//! built-ins, so the renderer, the panel's sliders and every key work
//! unchanged. A config carries the source (`EscapeConfig::
//! custom_formula`); [`ensure`] compiles it under the config's own name
//! before anything resolves that name.
//!
//! Each distinct source leaks one def (kilobytes); the editor compiles
//! on a pause in typing, not per keystroke.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use super::lang::{self, ir, Diag, Dialect, EntryKind};
use super::{ColoringDef, ColoringFeature, EscapeMetric, EscapeParamDef, FormulaDef, FormulaFeature};
use crate::config::escape::{CustomDialect, CustomSource, EscapeConfig};

/// What a registry name starts with when it names a compiled entry.
pub const PREFIX: &str = "custom-";

#[derive(Default)]
struct Registry {
    formulas: HashMap<String, &'static FormulaDef>,
    colorings: HashMap<String, &'static ColoringDef>,
    /// Names whose embedded source failed to compile: not retried every
    /// frame. A new source is a new name.
    failed: HashSet<String>,
    /// The programs with `global:` values, which the renderer has the
    /// interpreter compute ([`globals`]).
    programs: HashMap<String, Arc<ir::Program>>,
    /// The colourings keeping more than four floats: the `vec4`s of
    /// state past the first ([`accum_ext`]).
    accum_ext: HashMap<String, usize>,
    /// The formulas that take the perturbed path: the program, for the
    /// reference orbit, how it perturbs ([`perturb_tier`]), and its
    /// equations for the floatexp rung ([`perturb_fe_wgsl`]).
    perturbable: HashMap<String, (Arc<ir::Program>, lang::perturb::Perturbable, &'static str)>,
    /// What each reference identity handed out names ([`reference_identity`]).
    references: HashMap<u64, ReferenceInputs>,
    /// The formulas declaring their degree at infinity ([`escape_degree`]).
    degrees: HashMap<String, Arc<ir::Program>>,
    /// Whether each formula has a nucleus search ([`offers_nucleus`]).
    nucleus: HashMap<String, bool>,
}

/// Everything a custom formula's reference orbit is a function of
/// besides its point and precision.
#[derive(Clone)]
struct ReferenceInputs {
    prog: Arc<ir::Program>,
    params: lang::interp::ParamValues,
    env: lang::interp::Env,
}

fn registry() -> &'static RwLock<Registry> {
    static R: OnceLock<RwLock<Registry>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(Registry::default()))
}

pub fn formula(name: &str) -> Option<&'static FormulaDef> {
    if !name.starts_with(PREFIX) {
        return None;
    }
    registry().read().ok()?.formulas.get(name).copied()
}

pub fn coloring(name: &str) -> Option<&'static ColoringDef> {
    if !name.starts_with(PREFIX) {
        return None;
    }
    registry().read().ok()?.colorings.get(name).copied()
}

fn dialect(d: CustomDialect) -> Dialect {
    match d {
        // A hybrid's program is generated in UF's language.
        CustomDialect::UltraFractal | CustomDialect::Hybrid | CustomDialect::Rhai => Dialect::UltraFractal,
        CustomDialect::Fractint => Dialect::Fractint,
    }
}

/// The registry name of a source: stable across runs and builds (FNV-1a,
/// not the std hasher, whose output is not promised), so a file saved
/// today names the same entry tomorrow.
pub fn name_of(kind: EntryKind, src: &CustomSource) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // A separator, so ("ab", "c") and ("a", "bc") differ.
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    eat(match kind {
        EntryKind::Formula => b"formula",
        EntryKind::Coloring => b"coloring",
    });
    eat(match src.dialect {
        CustomDialect::UltraFractal => b"uf",
        CustomDialect::Fractint => b"frm",
        CustomDialect::Hybrid => b"hybrid",
        CustomDialect::Rhai => b"rhai",
    });
    eat(src.entry.as_bytes());
    eat(src.source.as_bytes());
    format!("{PREFIX}{h:016x}")
}

/// Parse and check the named entry of a source. The source may hold
/// other entries; only this one is compiled.
pub fn check_entry(kind: EntryKind, src: &CustomSource) -> Result<ir::Program, Vec<Diag>> {
    if src.dialect == CustomDialect::Rhai {
        return match kind {
            EntryKind::Formula => lang::rhai::program(&src.source),
            EntryKind::Coloring => lang::rhai::coloring(&src.source),
        };
    }
    if src.dialect == CustomDialect::Hybrid {
        if kind != EntryKind::Formula {
            return Err(vec![Diag::error(lang::Span::default(), "a hybrid is a formula")]);
        }
        let hybrid = lang::hybrid::Hybrid::from_json(&src.source).map_err(|e| vec![Diag::error(lang::Span::default(), e)])?;
        return hybrid.program();
    }
    let (file, mut diags) = lang::parser::parse_file(&src.source);
    let entry = file.entries.iter().find(|e| e.name.eq_ignore_ascii_case(&src.entry));
    let Some(entry) = entry else {
        return Err(vec![Diag::error(lang::Span::default(), format!("no entry named `{}`", src.entry))]);
    };
    // Errors elsewhere in the file are not this entry's.
    diags.retain(|d| d.span.start >= entry.span.start && d.span.end <= entry.span.end);
    // A formula's `switch:` target, when the source carries it, makes
    // the pair one program with the Julia toggle (lang::switch).
    let (program, d) = match kind {
        EntryKind::Formula => lang::switch::check_formula(&file, entry, dialect(src.dialect)),
        EntryKind::Coloring => lang::check::check(entry, kind, dialect(src.dialect)),
    };
    diags.extend(d);
    match program {
        Some(p) if !lang::has_errors(&diags) => Ok(p),
        _ => Err(diags),
    }
}

/// The source a config carries for entry `e` of a file (`text`, parsed
/// as `file`): the entry's own text, and the formula its `switch:` names
/// when that is another entry of the file, which the Julia toggle runs
/// (lang::switch).
pub fn carried_source(text: &str, file: &lang::ast::File, e: &lang::ast::Entry) -> String {
    let own = |e: &lang::ast::Entry| text.get(e.span.start..e.span.end.min(text.len())).unwrap_or_default().to_string();
    let mut source = own(e);
    let target = e.switch.get("type").and_then(|t| match &t.value {
        lang::ast::SettingValue::Strings(v) => v.first().cloned(),
        _ => None,
    });
    if let Some(t) = target.and_then(|n| file.entries.iter().find(|x| x.name.eq_ignore_ascii_case(&n))) {
        if t.span != e.span {
            source = format!("{source}\n\n{}", own(t));
        }
    }
    source
}

/// A formula or colouring entry of a UF or Fractint file (`text`) in
/// Rhai (plan, phase 4 step 3): what the Formula Editor opens it as to
/// edit. The entry's comments, and those just above it, come along;
/// `origin` names the file. The result compiles: a conversion that does
/// not is refused, with why.
pub fn convert_to_rhai(text: &str, entry: &str, kind: EntryKind, from: CustomDialect, origin: &str) -> Result<String, String> {
    let (file, _) = lang::parser::parse_file(text);
    let e = file.entries.iter().find(|e| e.name.eq_ignore_ascii_case(entry)).ok_or_else(|| format!("no entry named `{entry}`"))?;
    let d = dialect(from);
    let (program, diags) = match kind {
        EntryKind::Formula => lang::switch::check_formula(&file, e, d),
        EntryKind::Coloring => lang::check::check(e, kind, d),
    };
    let program = match program {
        Some(p) if !lang::has_errors(&diags) => p,
        _ => {
            let first = diags.iter().find(|x| x.severity == lang::Severity::Error).map(|x| x.render(text));
            return Err(first.unwrap_or_else(|| "it does not compile".into()));
        }
    };
    let language = match d {
        Dialect::Fractint => "Fractint",
        Dialect::UltraFractal => "Ultra Fractal",
    };
    let mut header = vec![format!("{}, converted from {language} ({origin}).", e.name)];
    // The comment lines just above the entry (its description, often its
    // author), then those inside it.
    let above: Vec<&str> = text[..e.span.start.min(text.len())].lines().rev().take_while(|l| l.trim().is_empty() || l.trim_start().starts_with(';')).collect();
    let above: String = above.into_iter().rev().collect::<Vec<_>>().join("\n");
    let mut notes = lang::to_rhai::comments(&above);
    notes.extend(lang::to_rhai::comments(&carried_source(text, &file, e)));
    while notes.first().is_some_and(|n| n.trim().is_empty()) {
        notes.remove(0);
    }
    if !notes.is_empty() {
        header.push(String::new());
        header.extend(notes);
    }
    let rhai = match kind {
        EntryKind::Formula => lang::to_rhai::formula(&program, &header)?,
        EntryKind::Coloring => lang::to_rhai::coloring(&program, &header)?,
    };
    // What it printed must compile back: one that does not is the
    // converter's bug, not the user's to fix.
    let back = match kind {
        EntryKind::Formula => lang::rhai::program(&rhai),
        EntryKind::Coloring => lang::rhai::coloring(&rhai),
    };
    back.map_err(|d| format!("the conversion does not compile ({})", d.first().map(|x| x.render(&rhai)).unwrap_or_default()))?;
    Ok(rhai)
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// The panel's view of a program's parameters: one def per float slot,
/// in slot order, as `pack_params` packs them. A complex parameter is
/// two (`name.re`, `name.im`); a user function an enum over the
/// functions it may be.
/// The custom parameters declared `exponential`, by their definitions'
/// addresses (leaked, so fixed for the run): their sliders run
/// logarithmically (`is_exponential`).
static EXPONENTIAL: std::sync::Mutex<std::collections::BTreeSet<usize>> = std::sync::Mutex::new(std::collections::BTreeSet::new());

/// A program's parameter definitions, leaked for the registry, its
/// `exponential` ones noted.
fn leak_params(p: &ir::Program) -> &'static [EscapeParamDef] {
    let defs: &'static [EscapeParamDef] = Box::leak(param_defs(p).into_boxed_slice());
    if let Ok(mut set) = EXPONENTIAL.lock() {
        for d in defs {
            let base = d.name.strip_suffix(".re").or_else(|| d.name.strip_suffix(".im")).unwrap_or(d.name);
            if p.params.iter().any(|q| q.name == base && q.exponential) {
                set.insert(d as *const EscapeParamDef as usize);
            }
        }
    }
    defs
}

/// Whether a parameter was declared `exponential` (UF's `exponential =
/// true`, a Rhai `PARAMS` entry's `exponential: true`): its slider runs
/// logarithmically.
pub fn is_exponential(d: &EscapeParamDef) -> bool {
    EXPONENTIAL.lock().is_ok_and(|set| set.contains(&(d as *const EscapeParamDef as usize)))
}

fn param_defs(p: &ir::Program) -> Vec<EscapeParamDef> {
    let mut out = Vec::new();
    let range = |min: Option<f64>, max: Option<f64>, default: f64| -> (f32, f32) {
        let reach = default.abs().max(1.0) * 4.0;
        let lo = min.unwrap_or_else(|| max.map_or(-reach, |m| m - 2.0 * reach).min(default));
        let hi = max.unwrap_or_else(|| min.map_or(reach, |m| m + 2.0 * reach).max(default));
        (lo as f32, hi as f32)
    };
    for param in &p.params {
        let caption = leak(param.caption.clone());
        let tooltip = leak(param.hint.clone());
        match param.kind {
            ir::ParamKind::Complex => {
                for (part, value, label) in [("re", param.default.0, "re"), ("im", param.default.1, "im")] {
                    let (min, max) = range(param.min, param.max, value);
                    out.push(EscapeParamDef {
                        name: leak(format!("{}.{part}", param.name)),
                        display_name: leak(format!("{} ({label})", param.caption)),
                        default: value as f32,
                        min,
                        max,
                        tooltip,
                        choices: &[],
                    });
                }
            }
            ir::ParamKind::Bool => out.push(EscapeParamDef {
                name: leak(param.name.clone()),
                display_name: caption,
                default: param.default.0 as f32,
                min: 0.0,
                max: 1.0,
                tooltip,
                choices: &["Off", "On"],
            }),
            ir::ParamKind::Enum => {
                let choices: Vec<&'static str> = param.choices.iter().map(|c| leak(c.clone())).collect();
                out.push(EscapeParamDef {
                    name: leak(param.name.clone()),
                    display_name: caption,
                    default: (param.default.0.round() as f32).clamp(0.0, (choices.len().max(1) - 1) as f32),
                    min: 0.0,
                    max: (choices.len().max(1) - 1) as f32,
                    tooltip,
                    choices: Box::leak(choices.into_boxed_slice()),
                });
            }
            ir::ParamKind::Int | ir::ParamKind::Float => {
                let (min, max) = range(param.min, param.max, param.default.0);
                out.push(EscapeParamDef {
                    name: leak(param.name.clone()),
                    display_name: caption,
                    default: param.default.0 as f32,
                    min,
                    max,
                    tooltip,
                    choices: &[],
                });
            }
        }
    }
    let names: &'static [&'static str] =
        Box::leak(lang::builtins::USER_FUNCTIONS.iter().map(|b| b.name()).collect::<Vec<_>>().into_boxed_slice());
    for f in &p.funcs {
        out.push(EscapeParamDef {
            name: leak(f.name.clone()),
            display_name: leak(f.caption.clone()),
            default: f.default as f32,
            min: 0.0,
            max: (names.len() - 1) as f32,
            tooltip: leak(f.hint.clone()),
            choices: names,
        });
    }
    out
}

fn display_name(p: &ir::Program) -> &'static str {
    leak(p.view.title.clone().unwrap_or_else(|| p.name.clone()))
}

/// Compile a formula entry and register it under `name`.
pub fn compile_formula(src: &CustomSource, name: &str) -> Result<&'static FormulaDef, Vec<Diag>> {
    if let Some(def) = formula(name) {
        return Ok(def);
    }
    let p = check_entry(EntryKind::Formula, src)?;
    // Without equations of its own, a formula gets them from its loop
    // when the rules cover it (lang::delta): the perturbed path at depth.
    // Derived equations whose WGSL does not validate (a bug) leave the
    // formula as it was rather than fail it.
    let p = match lang::delta::with_derived_equations(&p) {
        #[cfg(not(target_arch = "wasm32"))]
        Some(d) if lang::wgsl::validate_formula(&lang::wgsl::formula(&d)).is_ok() => d,
        #[cfg(target_arch = "wasm32")]
        Some(d) => d,
        _ => p,
    };
    let wgsl = lang::wgsl::formula(&p);
    // The same program carrying its derivative, for distance estimation
    // and analytic relief; empty when the iterate has none, which those
    // report as they do for a built-in without one.
    let derivative = lang::wgsl::formula_with_derivative(&p).unwrap_or_default();
    #[cfg(not(target_arch = "wasm32"))]
    for w in [&wgsl, &derivative] {
        if w.is_empty() {
            continue;
        }
        if let Err(e) = lang::wgsl::validate_formula(w) {
            return Err(vec![Diag::error(lang::Span::default(), format!("the compiler produced invalid WGSL (a bug): {e}"))]);
        }
    }
    let def: &'static FormulaDef = Box::leak(Box::new(FormulaDef {
        name: leak(name.to_string()),
        display_name: display_name(&p),
        // A program owns its seed, its escape and its plane: the Julia
        // toggle has nothing to swap, unless it reads Fractint's
        // `ismand`, which the toggle sets.
        features: if p.reads(ir::Predef::IsMand) {
            &[FormulaFeature::Program]
        } else {
            &[FormulaFeature::Program, FormulaFeature::DynamicalOnly]
        },
        parameters: leak_params(&p),
        wgsl: leak(wgsl),
        wgsl_param_seed: "",
        wgsl_prev_init: "",
        escape_metric: EscapeMetric::NormSq,
        escape_degree: None,
        derived_data: None,
        wgsl_derivative: leak(derivative),
        presets: &[],
    }));
    let perturbs = lang::perturb::perturbation(&p).ok();
    if let Ok(mut r) = registry().write() {
        r.formulas.insert(name.to_string(), def);
        let p = Arc::new(p);
        if p.degree.is_some() {
            r.degrees.insert(name.to_string(), p.clone());
        }
        if let Some(how) = perturbs {
            let fe = leak(lang::wgsl::formula_perturb_fe(&p).unwrap_or_default());
            r.perturbable.insert(name.to_string(), (p.clone(), how, fe));
        }
        if !p.globals.is_empty() {
            r.programs.insert(name.to_string(), p);
        }
    }
    Ok(def)
}

/// A compiled formula's declared degree at infinity (`fn degree(p)`;
/// `lang::ir::Program::degree`) for its slot-ordered parameters: what a
/// built-in's `escape_degree` gives. `None` when it declares none.
pub fn escape_degree(name: &str, fparams: &[f32]) -> Option<f32> {
    let prog = registry().read().ok()?.degrees.get(name)?.clone();
    let params = lang::interp::ParamValues::from_slots(&prog, fparams);
    let env = lang::interp::Env::at(lang::builtins::C64::ZERO, 1);
    lang::interp::degree_of(&prog, &params, &env).map(|d| d as f32)
}

/// A custom formula's perturbation tier for a view, when it has one:
/// equations a formula of its shape can take (`lang::perturb`), and its
/// `perturb` setting holding for these parameters.
pub fn perturb_tier(escape: &EscapeConfig) -> Option<super::assembler::PerturbTier> {
    if !escape.formula.starts_with(PREFIX) {
        return None;
    }
    ensure(escape);
    let (prog, how, _) = registry().read().ok()?.perturbable.get(&escape.formula).cloned()?;
    let inputs = reference_inputs(escape, &prog)?;
    if !lang::interp::perturb_holds(&prog, &inputs.params, &inputs.env) {
        return None;
    }
    let hash = u64::from_str_radix(&escape.formula[PREFIX.len()..], 16).ok()?;
    Some(super::assembler::PerturbTier::Custom { hash, rebase: how.rebase, stride: how.references() as u8, width: (1 + prog.carried.len()) as u8 })
}

/// The program and inputs a config's custom formula iterates with: its
/// parameters as the GPU packs them (the panel's bailout bound to the
/// program's own), and the image's values it may read.
fn reference_inputs(escape: &EscapeConfig, prog: &Arc<ir::Program>) -> Option<ReferenceInputs> {
    let def = formula(&escape.formula)?;
    let mut slots = super::packed_formula_params(def, &escape.formula_params);
    if let Some(i) = def.parameters.iter().position(|p| p.name == "bailout" || p.name == "bailout.re") {
        slots[i] = escape.bailout;
    }
    let params = lang::interp::ParamValues::from_slots(prog, &slots);
    let (cx, cy) = escape.center_f64();
    let env = lang::interp::Env {
        pixel: lang::builtins::C64::ZERO,
        maxiter: escape.max_iter.max(1),
        center: lang::builtins::C64::new(cx, cy),
        // As the shader's `esc_env_magn`: 4 / span.
        magn: escape.zoom_factor(),
        x: 0,
        y: 0,
        width: 1,
        height: 1,
        random: lang::builtins::C64::ZERO,
        julia: escape.julia,
        julia_c: lang::builtins::C64::new(f64::from(escape.julia_re), f64::from(escape.julia_im)),
        rotation: f64::from(escape.rotation),
    };
    Some(ReferenceInputs { prog: prog.clone(), params, env })
}

/// A custom formula's BLA steps (phase 4, step 9): each reference step's
/// linear part, real-linear in general -- `A = P·δ + Q·conj(δ)` in the
/// deviation, `B` likewise in the pixel's offset, Q zero where the step
/// is holomorphic, a fold or a conjugate making it otherwise -- with the
/// radius below which the step's dropped nonlinear part stays under
/// `bla::BLA_EPS` of its linear one: the single-step entries
/// `bla::BlaTable::build_from_steps` merges.
///
/// All of it is read from the formula's own perturbation equations
/// (derived or its own), stepped in f64 from `Z_n`
/// (`interp::PerturbStepper`): they are free of cancellation, so a
/// deviation of 2^-60 |Z| gives the linear part exactly, folds
/// (`diffabs`) included. The radius: the residual at a small deviation
/// along the axes sizes the quadratic term; the candidate is then
/// checked on a ring of eight directions, with and without the view's
/// largest pixel offset `dc_max`, halving until the residual holds --
/// which finds a fold's kink or a branch cut as well as curvature --
/// and halved once more for directions between the eight.
///
/// `reference` is the reference's pixel (its point on the parameter
/// plane, its start on the Julia plane). `None` where skipping is not
/// sound: a branch on the iterate (a skip could cross it), state carried
/// besides z or phases (a skip advances neither), or a test other than
/// z's modulus exceeding a bound (a skip could cross a convergent
/// formula's settle).
pub fn bla_steps(escape: &EscapeConfig, orbit: &[[f64; 2]], reference: lang::builtins::C64, dc_max: f64) -> Option<Vec<super::bla::BlaEntry>> {
    use ir::{IExpr, IExprKind, IStmt, Predef};
    use lang::ast::{BinOp, UnOp};
    let (prog, how, _) = registry().read().ok()?.perturbable.get(&escape.formula).cloned()?;
    if how.cycle.is_some() || !how.rebase || !prog.carried.is_empty() {
        return None;
    }
    // Reads the iterate or anything that follows it.
    fn varies(e: &IExpr, p: &ir::Program) -> bool {
        match &e.kind {
            IExprKind::Predef(Predef::Z | Predef::Pixel | Predef::NumIter | Predef::Pass) => true,
            IExprKind::Var(v) => !p.vars[*v].global,
            IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => varies(a, p),
            IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => varies(a, p) || varies(b, p),
            _ => false,
        }
    }
    fn branches_on_the_iterate(stmts: &[IStmt], p: &ir::Program) -> bool {
        stmts.iter().any(|s| match s {
            IStmt::Assign(..) => false,
            IStmt::If { arms, otherwise } => {
                arms.iter().any(|(c, b)| varies(c, p) || branches_on_the_iterate(b, p)) || branches_on_the_iterate(otherwise, p)
            }
            IStmt::While { .. } | IStmt::Repeat { .. } => true,
        })
    }
    if branches_on_the_iterate(&prog.loop_body, &prog) {
        return None;
    }
    // The test: z's modulus (or its square) below a bound the same for
    // every pixel, or the same negated.
    fn escapes_on_the_modulus(e: &IExpr, p: &ir::Program) -> bool {
        let modulus = |x: &IExpr| match &x.kind {
            IExprKind::Modulus(a) | IExprKind::Call(lang::builtins::Builtin::Cabs, a) => matches!(a.kind, IExprKind::Predef(Predef::Z)),
            _ => false,
        };
        match &e.kind {
            IExprKind::Binary(BinOp::Lt | BinOp::Le, a, b) => modulus(a) && !varies(b, p),
            IExprKind::Unary(UnOp::Not, x) => match &x.kind {
                IExprKind::Binary(BinOp::Gt | BinOp::Ge, a, b) => modulus(a) && !varies(b, p),
                _ => false,
            },
            _ => false,
        }
    }
    if !prog.bailout.as_ref().is_some_and(|b| escapes_on_the_modulus(b, &prog)) {
        return None;
    }
    let inputs = reference_inputs(escape, &prog)?;
    let env = lang::interp::Env { pixel: reference, ..inputs.env };
    let stepper = lang::interp::PerturbStepper::new(&prog, &inputs.params, &env)?;
    use super::bla::{BlaEntry, Cfe64, MagFe, BLA_EPS};
    use lang::builtins::C64;
    let i = C64::new(0.0, 1.0);
    let dc_max = if dc_max.is_finite() { dc_max } else { 0.0 };
    let one = |n: usize| -> BlaEntry {
        let z = C64::new(orbit[n][0], orbit[n][1]);
        let scale = z.abs().max(1e-100);
        // The linear parts, from deviations far below any nonlinearity.
        let h = scale * 2f64.powi(-60);
        let (j1, ji) = (stepper.step(z, C64::new(h, 0.0), C64::ZERO).scale(1.0 / h), stepper.step(z, C64::new(0.0, h), C64::ZERO).scale(1.0 / h));
        let (p, q) = (j1.sub(i.mul(ji)).scale(0.5), j1.add(i.mul(ji)).scale(0.5));
        let hc = reference.abs().max(1.0) * 2f64.powi(-60);
        let (b1, bi) = (stepper.step(z, C64::ZERO, C64::new(hc, 0.0)).scale(1.0 / hc), stepper.step(z, C64::ZERO, C64::new(0.0, hc)).scale(1.0 / hc));
        let (pb, qb) = (b1.sub(i.mul(bi)).scale(0.5), b1.add(i.mul(bi)).scale(0.5));
        let linear = |d: C64, dc: C64| p.mul(d).add(q.mul(d.conj())).add(pb.mul(dc)).add(qb.mul(dc.conj()));
        let residual = |d: C64, dc: C64| stepper.step(z, d, dc).sub(linear(d, dc)).abs();
        // The smallest stretch of the linear part: what the residual is
        // weighed against.
        let sigma = (p.abs() - q.abs()).abs();
        let mut r = 0.0;
        if sigma > 0.0 && sigma.is_finite() {
            let rho = scale * 2f64.powi(-16);
            let axes = [C64::ONE, i, C64::new(-1.0, 0.0), C64::new(0.0, -1.0)];
            let r0 = axes.iter().map(|&e| residual(e.scale(rho), C64::ZERO)).fold(0.0f64, f64::max);
            // c rho^2 = r0: the residual c r^2 under eps sigma r -- from
            // just inside the bound, which rounding would otherwise fail.
            r = if r0 > 0.0 { 0.9 * BLA_EPS * sigma * rho * rho / r0 } else { scale };
            r = r.min(scale);
            let ring: Vec<C64> = (0..8).map(|k| C64::new((k as f64 * std::f64::consts::FRAC_PI_4).cos(), (k as f64 * std::f64::consts::FRAC_PI_4).sin())).collect();
            let holds = |r: f64| {
                ring.iter().enumerate().all(|(k, &e)| {
                    let d = e.scale(r);
                    let bound = BLA_EPS * sigma * r;
                    residual(d, C64::ZERO) <= bound && (dc_max == 0.0 || residual(d, ring[(k + 3) % 8].scale(dc_max)) <= bound)
                })
            };
            let mut tries = 0;
            while r > 0.0 && !holds(r) {
                r *= 0.5;
                tries += 1;
                if tries > 40 {
                    r = 0.0;
                }
            }
            r *= 0.5;
        }
        let c = |v: C64| Cfe64::from_f64(v.re, v.im);
        BlaEntry { a: c(p), b: c(pb), r: MagFe::from_f64(r), aq: c(q), bq: c(qb) }
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        Some((0..orbit.len()).into_par_iter().map(one).collect())
    }
    #[cfg(target_arch = "wasm32")]
    Some((0..orbit.len()).map(one).collect())
}

/// A built-in tier's BLA steps from its Rhai twin (phase 4, step 9b): the
/// same map, so its equations give the built-in's single steps too (the
/// Ship family, the Tricorn).
pub fn twin_bla_steps(escape: &EscapeConfig, twin: &str, params: &[(&str, f32)], orbit: &[[f64; 2]], reference: lang::builtins::C64, dc_max: f64) -> Option<Vec<super::bla::BlaEntry>> {
    let src = super::twins::custom_source(twin)?;
    let mut t = escape.clone();
    t.formula = name_of(EntryKind::Formula, &src);
    t.custom_formula = Some(src);
    t.formula_params = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    ensure(&t);
    bla_steps(&t, orbit, reference, dc_max)
}

/// A compiled formula's perturbation equations for the floatexp rung
/// (`lang::wgsl::formula_perturb_fe`), by its name.
pub fn perturb_fe_wgsl(name: &str) -> Option<&'static str> {
    registry().read().ok()?.perturbable.get(name).map(|e| e.2).filter(|s| !s.is_empty())
}

/// The reference identity of a config's custom formula: the two map
/// parameters the reference orbit's cache keys already compare
/// (`ReferenceOrbit::map_params`), carrying a 46-bit hash of everything
/// the orbit is a function of besides its point and precision -- the
/// program, its parameters, the plane, and what of the image it reads.
/// Each half is a normal f32 in [1, 2), so equality compares bits.
pub fn reference_identity(escape: &EscapeConfig) -> Option<[f32; 2]> {
    let prog = registry().read().ok()?.perturbable.get(&escape.formula)?.0.clone();
    let inputs = reference_inputs(escape, &prog)?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for b in escape.formula.bytes() {
        eat(u64::from(b));
    }
    for (re, im) in &inputs.params.values {
        eat(re.to_bits());
        eat(im.to_bits());
    }
    for f in &inputs.params.funcs {
        eat(*f as u64);
    }
    let env = &inputs.env;
    eat(u64::from(env.julia));
    eat(env.julia_c.re.to_bits());
    eat(env.julia_c.im.to_bits());
    if prog.reads(ir::Predef::MaxIter) {
        eat(u64::from(env.maxiter));
    }
    if prog.reads(ir::Predef::Magn) || prog.reads(ir::Predef::MagXMag) {
        eat(env.magn.to_bits());
    }
    if prog.reads(ir::Predef::RotSkew) {
        eat(env.rotation.to_bits());
    }
    let id = h & ((1u64 << 46) - 1);
    if let Ok(mut r) = registry().write() {
        // One a parameter set seen; a session that drags a slider
        // through thousands starts over rather than grow without end
        // (a reference already computed keeps its machine).
        if r.references.len() >= 1024 && !r.references.contains_key(&id) {
            r.references.clear();
        }
        r.references.insert(id, inputs);
    }
    let half = |bits: u64| f32::from_bits(0x3F80_0000 | (bits & 0x7F_FFFF) as u32);
    Some([half(id), half(id >> 23)])
}

/// The program a reference identity names, started at the reference
/// point in big floats `n_limbs` wide: the custom family's state in a
/// reference orbit. `None` for an identity this session did not hand
/// out, or a start the big floats cannot make.
pub fn reference_machines(map_params: [f32; 2], center: &crate::escape::bigfloat::BigComplex, n_limbs: usize) -> Option<Vec<lang::bigeval::BigMachine>> {
    let inputs = identified(map_params)?;
    // A loop with a cycle (a hybrid): besides the main reference, one
    // per phase, its passes starting where that phase first runs
    // (plan, phase 3, step 5b). Every one starts from the init's z.
    let cycle = lang::perturb::perturbation(&inputs.prog).ok().and_then(|how| how.cycle);
    let starts: Vec<u32> = std::iter::once(0).chain(cycle.iter().flat_map(|c| (0..c.period).map(move |q| c.prefix + q))).collect();
    starts
        .into_iter()
        .map(|start| {
            let mut m = lang::bigeval::BigMachine::new(inputs.prog.clone(), inputs.params.clone(), inputs.env, center.clone(), n_limbs).ok()?;
            m.set_pass(start);
            Some(m)
        })
        .collect()
}

/// The inputs a reference identity names (`reference_identity`).
fn identified(map_params: [f32; 2]) -> Option<ReferenceInputs> {
    let id = u64::from(map_params[0].to_bits() & 0x7F_FFFF) | (u64::from(map_params[1].to_bits() & 0x7F_FFFF) << 23);
    registry().read().ok()?.references.get(&id).cloned()
}

/// Whether the reference an identity names returns exactly to its
/// start when its iterate does (`lang::periodic::wraps_exactly`): what
/// lets a closing orbit become periodic.
pub fn reference_wraps(map_params: [f32; 2]) -> bool {
    identified(map_params).is_some_and(|i| lang::periodic::wraps_exactly(&i.prog))
}

/// The nucleus search for a reference identity (phase 4, step 9c): the
/// program `reference_machines` starts, through its derivative. `None`
/// for an identity this session did not hand out, or a formula with no
/// exact wrap or no derivative.
pub fn nucleus_search(map_params: [f32; 2]) -> Option<lang::periodic::NucleusSearch> {
    let i = identified(map_params)?;
    lang::periodic::NucleusSearch::new(&i.prog, i.params, i.env)
}

/// The nucleus search for a config's custom formula, as its reference
/// would run it (the panel's Center on minibrot).
pub fn nucleus_search_for(escape: &EscapeConfig) -> Option<lang::periodic::NucleusSearch> {
    if !escape.formula.starts_with(PREFIX) {
        return None;
    }
    ensure(escape);
    let (prog, _, _) = registry().read().ok()?.perturbable.get(&escape.formula).cloned()?;
    let i = reference_inputs(escape, &prog)?;
    lang::periodic::NucleusSearch::new(&i.prog, i.params, i.env)
}

/// Center on minibrot for a config's custom formula (plan, phase 4, step
/// 9c): the nucleus governing its view, through the formula's
/// derivative (`lang::periodic`), its digits those the built-ins' give
/// at this depth (`escape::nucleus::find_nucleus`, zoom + 16 bits): the
/// view's reference closes there or relocates to the nucleus. `None` on
/// the Julia plane, for a formula with no search, or none found.
pub fn locate_nucleus(escape: &EscapeConfig, max_period: u32, newton_periods: u32) -> Option<super::nucleus::Nucleus> {
    if escape.julia {
        return None;
    }
    let mut hit = nucleus_search_for(escape)?.locate(&escape.center_re, &escape.center_im, escape.zoom_log2, max_period, newton_periods)?;
    let n = super::fixedpoint::limbs_for_zoom(escape.zoom_log2 + 16.0) + 1;
    let digits = ((64 * n as i64 - 8) as f64 * 0.301).ceil() as usize;
    let trim = |s: &mut String| {
        if let Some(dot) = s.find('.') {
            s.truncate((dot + 1 + digits).min(s.len()));
        }
    };
    trim(&mut hit.re);
    trim(&mut hit.im);
    Some(hit)
}

/// Detect period for a config's custom formula (plan, phase 4, step 9c):
/// the period its centre's orbit closes at below the view's limit, with
/// its octave, as `escape::nucleus::detect_period_for_zoom` finds the
/// built-ins' -- the hint its reference then takes. `None` on the Julia
/// plane, for a formula whose reference does not wrap exactly, or none
/// within `max_period`.
pub fn detect_period(escape: &EscapeConfig, max_period: u32) -> Option<(u32, i64)> {
    if escape.julia || !escape.formula.starts_with(PREFIX) {
        return None;
    }
    ensure(escape);
    let (prog, _, _) = registry().read().ok()?.perturbable.get(&escape.formula).cloned()?;
    let i = reference_inputs(escape, &prog)?;
    if !lang::periodic::wraps_exactly(&i.prog) {
        return None;
    }
    let n = super::fixedpoint::limbs_for_view(&escape.center_re, &escape.center_im, escape.zoom_log2);
    let at = |s: &str| super::fixedpoint::FixedPoint::from_decimal(s, n).map(|f| super::bigfloat::BigFloat::from_fixed(&f));
    let c = super::bigfloat::BigComplex { re: at(&escape.center_re)?, im: at(&escape.center_im)? };
    let limit = super::reference::closure_limit_for_zoom(escape.zoom_log2);
    lang::periodic::closing_period(i.prog, i.params, i.env, &c, n, limit, max_period)
}

/// Whether the panel offers Center on minibrot for a config's custom
/// formula: it has a nucleus search (`nucleus_search_for`), on the
/// parameter plane. Remembered per formula.
pub fn offers_nucleus(escape: &EscapeConfig) -> bool {
    if escape.julia || !escape.formula.starts_with(PREFIX) {
        return false;
    }
    if let Some(&known) = registry().read().ok().and_then(|r| r.nucleus.get(&escape.formula).copied()).as_ref() {
        return known;
    }
    let found = nucleus_search_for(escape).is_some();
    if let Ok(mut r) = registry().write() {
        r.nucleus.insert(escape.formula.clone(), found);
    }
    found
}

/// The cycle a registered custom formula's references follow, when its
/// loop branches on the pass (a hybrid): the perturbed shader's
/// constants.
pub fn cycle(name: &str) -> Option<lang::ir::Cycle> {
    registry().read().ok()?.perturbable.get(name).and_then(|(_, how, _)| how.cycle)
}

/// Compile a colouring entry and register it under `name`.
pub fn compile_coloring(src: &CustomSource, name: &str) -> Result<&'static ColoringDef, Vec<Diag>> {
    if let Some(def) = coloring(name) {
        return Ok(def);
    }
    let p = check_entry(EntryKind::Coloring, src)?;
    let c = lang::wgsl::coloring(&p);
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = lang::wgsl::validate_coloring(&c) {
        return Err(vec![Diag::error(lang::Span::default(), format!("the compiler produced invalid WGSL (a bug): {e}"))]);
    }
    let mut features = Vec::new();
    if c.needs_accum {
        features.push(ColoringFeature::NeedsOrbitAccum);
    }
    if p.region != ir::Region::Outside {
        features.push(ColoringFeature::ColorsInterior);
    }
    let def: &'static ColoringDef = Box::leak(Box::new(ColoringDef {
        name: leak(name.to_string()),
        display_name: display_name(&p),
        features: Box::leak(features.into_boxed_slice()),
        parameters: leak_params(&p),
        wgsl: leak(c.wgsl),
        accum_init: leak(c.accum_init),
        wgsl_accum: leak(c.wgsl_accum),
        recommended_bailout: None,
        pick_params: &[],
    }));
    if let Ok(mut r) = registry().write() {
        r.colorings.insert(name.to_string(), def);
        if c.ext_vec4s > 0 {
            r.accum_ext.insert(name.to_string(), c.ext_vec4s);
        }
        if !p.globals.is_empty() {
            r.programs.insert(name.to_string(), Arc::new(p));
        }
    }
    Ok(def)
}

/// The `vec4`s of state a compiled colouring keeps past the first four
/// floats (its private `ufc_ext`, which the templates save and restore
/// beside the accumulator); 0 for any other name.
pub fn accum_ext(name: &str) -> usize {
    if !name.starts_with(PREFIX) {
        return 0;
    }
    registry().read().ok().and_then(|r| r.accum_ext.get(name).copied()).unwrap_or(0)
}

/// A compiled entry's `global:` values for one render, from its
/// parameter block as the GPU holds it (`block`) and the image: where
/// they go in their block and the values, or `None` for a name with
/// none. A formula's fill the uniform's `fdata` from the start; a
/// colouring's follow its parameters in its own block.
///
/// UF runs `global:` once per image; the renderer asks every frame, so
/// the last answer per name is kept and reused while nothing it read
/// has changed.
pub fn globals(name: &str, block: &[f32], env: &lang::interp::Env) -> Option<(usize, Vec<f32>)> {
    let p = registry().read().ok()?.programs.get(name)?.clone();
    let at = if p.kind == EntryKind::Coloring { p.param_slots() } else { 0 };
    // Everything `global:` can read: the parameters and the image.
    let mut key: Vec<u64> = block.iter().map(|v| u64::from(v.to_bits())).collect();
    key.extend([
        u64::from(env.maxiter),
        env.center.re.to_bits(),
        env.center.im.to_bits(),
        env.magn.to_bits(),
        env.width as u64,
        env.height as u64,
        u64::from(env.julia),
        env.rotation.to_bits(),
    ]);
    static LAST: OnceLock<Mutex<HashMap<String, (Vec<u64>, Vec<f32>)>>> = OnceLock::new();
    let last = LAST.get_or_init(Default::default);
    if let Some((k, v)) = last.lock().ok()?.get(name) {
        if *k == key {
            return Some((at, v.clone()));
        }
    }
    let params = lang::interp::ParamValues::from_slots(&p, block);
    let values = lang::interp::globals(&p, &params, env);
    if let Ok(mut l) = last.lock() {
        l.insert(name.to_string(), (key, values.clone()));
    }
    Some((at, values))
}

/// Register the config's embedded sources under the names it gives
/// them, before anything resolves those names. Cheap when they are
/// registered already; a source that fails to compile is logged once
/// and its name falls back as any unknown name does.
pub fn ensure(escape: &EscapeConfig) {
    let wanted = [
        (escape.formula.as_str(), escape.custom_formula.as_ref(), EntryKind::Formula),
        (escape.coloring.as_str(), escape.custom_coloring.as_ref(), EntryKind::Coloring),
    ];
    for (name, src, kind) in wanted {
        let Some(src) = src else { continue };
        if !name.starts_with(PREFIX) {
            continue;
        }
        let known = match kind {
            EntryKind::Formula => formula(name).is_some(),
            EntryKind::Coloring => coloring(name).is_some(),
        };
        if known || registry().read().is_ok_and(|r| r.failed.contains(name)) {
            continue;
        }
        let result = match kind {
            EntryKind::Formula => compile_formula(src, name).map(|_| ()),
            EntryKind::Coloring => compile_coloring(src, name).map(|_| ()),
        };
        if let Err(diags) = result {
            let msgs: Vec<String> = diags.iter().map(|d| d.render(&src.source)).collect();
            log::warn!("Custom escape entry `{}` ({name}) did not compile: {}", src.entry, msgs.join("; "));
            if let Ok(mut r) = registry().write() {
                r.failed.insert(name.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(text: &str, entry: &str) -> CustomSource {
        CustomSource { dialect: CustomDialect::UltraFractal, entry: entry.to_string(), source: text.to_string() }
    }

    /// Center on minibrot for custom formulas (phase 4, step 9c): the
    /// Mandelbrot's twin and the cubic Multibrot's find the nuclei the
    /// built-ins' Newton finds at their periods, to the digits the view
    /// needs; the panel offers it where there is a search, not on a fold
    /// or the Julia plane.
    #[test]
    fn a_custom_formula_centres_on_its_minibrot() {
        let config = |name: &str, params: &[(&str, f32)], (cre, cim, zoom): (&str, &str, f64)| {
            let src = crate::escape::twins::custom_source(name).unwrap();
            let mut esc = EscapeConfig { bailout: 4.0, zoom_log2: zoom, ..EscapeConfig::default() };
            esc.center_re = cre.into();
            esc.center_im = cim.into();
            esc.formula = name_of(EntryKind::Formula, &src);
            esc.custom_formula = Some(src);
            for (k, v) in params {
                esc.formula_params.insert(k.to_string(), *v);
            }
            ensure(&esc);
            esc
        };
        // (The seahorse view's period is 998: the built-ins' ball method
        // there says 850, where Newton finds nothing.)
        for (name, params, power, view, period) in [
            ("mandelbrot", &[][..], 2, ("-1.754", "0.0005", 9.0), 3),
            ("mandelbrot", &[], 2, ("-0.7436438870371587", "0.1318259042053120", 30.0), 998),
            ("multibrot", &[("power", 3.0)], 3, ("0.0", "1.0", 8.0), 2),
        ] {
            let esc = config(name, params, view);
            assert!(offers_nucleus(&esc), "{name}");
            let ours = locate_nucleus(&esc, 100_000, 100_000).unwrap_or_else(|| panic!("{name} at {view:?}"));
            assert_eq!(ours.period, period, "{name} at {view:?}");
            // The built-ins' Newton at that period lands there too.
            let theirs = crate::escape::nucleus::find_nucleus(view.0, view.1, period, view.2 + 16.0, power).unwrap();
            let same = |a: &str, b: &str| a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
            let digits = (view.2 * 0.301) as usize + 6;
            assert!(same(&ours.re, &theirs.re) >= digits && same(&ours.im, &theirs.im) >= digits, "{name}: ({}, {}) against ({}, {})", ours.re, ours.im, theirs.re, theirs.im);
        }
        // Detect period: at the nucleus to 60 octaves, the period the
        // built-ins' detection gives, at the octave it closes.
        let nucleus = crate::escape::nucleus::find_nucleus("-1.754", "0.0005", 3, 60.0, 2).unwrap();
        let esc = config("mandelbrot", &[], (&nucleus.re, &nucleus.im, 20.0));
        let found = detect_period(&esc, 1000).expect("closes");
        let theirs = crate::escape::nucleus::detect_period_for_zoom(&nucleus.re, &nucleus.im, 2, 1000, 20.0).unwrap();
        assert_eq!(found.0, theirs.0);
        assert!((found.1 - theirs.1).abs() <= 1, "{found:?} against {theirs:?}");
        // A fold searches through its Jacobian (step 9d): on the real axis
        // the Burning Ship is the Mandelbrot, and finds its nucleus.
        let ship = config("burning_ship", &[], ("-1.754", "0.0005", 9.0));
        assert!(offers_nucleus(&ship));
        assert_eq!(locate_nucleus(&ship, 1000, 1000).map(|n| n.period), Some(3));
        let julia = EscapeConfig { julia: true, ..config("mandelbrot", &[], ("-1.754", "0.0005", 9.0)) };
        assert!(!offers_nucleus(&julia) && locate_nucleus(&julia, 1000, 1000).is_none());
    }

    /// BLA for custom formulas (phase 4, step 9): the Mandelbrot's twin
    /// reads the power table's single steps (A = 2Z, B = 1, Q zero; the
    /// radius 0.45 of the power table's 2 eps |Z|, the probe's margins) from
    /// its own equations; the Burning Ship's twin its real-linear
    /// Jacobian, its radius inside the folds' distance to their axes;
    /// what skipping is not sound for gets none.
    #[test]
    fn custom_bla_steps_match_the_power_tables() {
        use lang::builtins::C64;
        let config = |src: CustomSource, params: &[(&str, f32)]| {
            let mut esc = EscapeConfig { bailout: 4.0, ..EscapeConfig::default() };
            esc.formula = name_of(EntryKind::Formula, &src);
            esc.custom_formula = Some(src);
            for (k, v) in params {
                esc.formula_params.insert(k.to_string(), *v);
            }
            ensure(&esc);
            esc
        };
        let twin = |name: &str| crate::escape::twins::custom_source(name).unwrap();
        let c = C64::new(-0.7436, 0.1318);
        let mut orbit = vec![[0.0f64; 2]];
        let mut z = C64::ZERO;
        // To the reference's escape, as the renderer cuts its orbit.
        for _ in 0..200 {
            z = z.mul(z).add(c);
            orbit.push([z.re, z.im]);
            if z.re * z.re + z.im * z.im > 4.0 {
                break;
            }
        }
        assert!(orbit.len() > 50, "the orbit is too short to test: {}", orbit.len());
        let ours = bla_steps(&config(twin("mandelbrot"), &[]), &orbit, c, 0.0).expect("the Mandelbrot skips");
        let power = super::super::bla::BlaTable::build(&orbit, 2, 0.0);
        assert_eq!(ours.len(), orbit.len());
        for (n, e) in ours.iter().enumerate().take(orbit.len() - 1) {
            let z = C64::new(orbit[n][0], orbit[n][1]);
            let (ar, ai) = e.a.to_f64();
            let (br, bi) = e.b.to_f64();
            assert!((ar - 2.0 * z.re).abs() <= 1e-12 * z.abs().max(1.0) && (ai - 2.0 * z.im).abs() <= 1e-12 * z.abs().max(1.0), "A at {n}: ({ar}, {ai})");
            assert!((br - 1.0).abs() < 1e-12 && bi.abs() < 1e-12, "B at {n}: ({br}, {bi})");
            let (qa, qb) = (e.aq.to_f64(), e.bq.to_f64());
            assert!(qa.0.hypot(qa.1) <= 1e-12 * z.abs().max(1.0) && qb.0.hypot(qb.1) < 1e-12, "Q at {n}: {qa:?} {qb:?}");
            let want = 0.9 * super::super::bla::BLA_EPS * z.abs();
            let got = e.r.m * (e.r.e as f64).exp2();
            assert!(n == 0 || (got / want - 1.0).abs() < 1e-3, "r at {n}: {got} against {want}");
        }
        // The merged table skips as the power table does.
        assert!(!power.levels.is_empty());
        // The Burning Ship (phase 4, step 9b): its Jacobian at Z, where
        // neither part changes sign, is real-linear, P d + Q conj(d) for
        // [[2X, -2Y], [2|Y| sgn X, 2|X| sgn Y]]; and no skip may cross a
        // fold: the radius stays inside min(|X|, |Y|).
        let c = C64::new(-1.75, -0.02);
        let mut orbit = vec![[0.0f64; 2]];
        let mut z = C64::ZERO;
        for _ in 0..200 {
            z = C64::new(z.re.abs(), z.im.abs());
            z = z.mul(z).add(c);
            orbit.push([z.re, z.im]);
            if z.re * z.re + z.im * z.im > 4.0 {
                break;
            }
        }
        assert!(orbit.len() > 50, "the Ship's orbit is too short to test: {}", orbit.len());
        let ship = bla_steps(&config(twin("burning_ship"), &[]), &orbit, c, 0.0).expect("the Ship skips, real-linear");
        for (n, e) in ship.iter().enumerate().skip(1).take(orbit.len() - 2) {
            let (x, y) = (orbit[n][0], orbit[n][1]);
            let (a, b, cc, d) = (2.0 * x, -2.0 * y, 2.0 * y.abs() * x.signum(), 2.0 * x.abs() * y.signum());
            let (p, q) = (C64::new((a + d) / 2.0, (cc - b) / 2.0), C64::new((a - d) / 2.0, (cc + b) / 2.0));
            let (pa, qa) = (e.a.to_f64(), e.aq.to_f64());
            let tol = 1e-12 * x.hypot(y).max(1.0);
            assert!((pa.0 - p.re).abs() <= tol && (pa.1 - p.im).abs() <= tol, "P at {n}: {pa:?} against {p:?}");
            assert!((qa.0 - q.re).abs() <= tol && (qa.1 - q.im).abs() <= tol, "Q at {n}: {qa:?} against {q:?}");
            let r = e.r.m * (e.r.e as f64).exp2();
            assert!(r <= x.abs().min(y.abs()), "r at {n}: {r} past a fold ({x}, {y})");
        }
        // Where skipping is not sound: a carried value, a test that is not
        // z's modulus. A user function set to `abs` folds: real-linear.
        assert!(bla_steps(&config(twin("phoenix"), &[]), &orbit, c, 0.0).is_none(), "a carried value");
        let sine = src("S {\ninit:\n z = #pixel\nloop:\n z = sin(z) * #pixel\nbailout:\n abs(imag(z)) < 40\n}", "S");
        assert!(bla_steps(&config(sine, &[]), &orbit, c, 0.0).is_none(), "a test on Im z");
        let user = src("U {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel + 0.01 * @fn1(z)\nbailout:\n |z| < 4\ndefault:\n func fn1\n  default = sin()\n endfunc\n}", "U");
        assert!(bla_steps(&config(user.clone(), &[]), &orbit, c, 0.0).is_some(), "sin");
        let abs = lang::builtins::USER_FUNCTIONS.iter().position(|f| *f == lang::builtins::Builtin::Abs).unwrap() as f32;
        assert!(bla_steps(&config(user, &[("fn1", abs)]), &orbit, c, 0.0).is_some(), "abs");
    }

    /// Edit as Rhai: a UF formula and its `switch:` Julia become one Rhai
    /// formula on both planes, its comments carried, which compiles and
    /// runs as the pair does; a colouring is not offered.
    #[test]
    fn a_uf_formula_converts_to_rhai_for_editing() {
        let text = include_str!("../../tests/formulas/language.ufm");
        let rhai = convert_to_rhai(text, "SwitchM", EntryKind::Formula, CustomDialect::UltraFractal, "language.ufm").expect("converts");
        assert!(rhai.starts_with("// SwitchM, converted from Ultra Fractal (language.ufm).\n//\n// A Mandelbrot whose switch: names its Julia"), "{rhai}");
        assert!(rhai.contains("if ctx.julia {") || rhai.contains("if !ctx.julia {"), "both planes:\n{rhai}");
        let src = CustomSource { dialect: CustomDialect::UltraFractal, entry: "SwitchM".into(), source: text.to_string() };
        let uf = check_entry(EntryKind::Formula, &src).expect("the pair compiles");
        let back = lang::rhai::program(&rhai).expect("compiles back");
        assert_eq!(lang::to_rhai::parting(&uf, &back, 100), None, "{rhai}");
        // The comment just above an entry comes along too.
        let above = "; By A. Author, 2001.\n; A cubic.\nCubic {\ninit:\n z = 0\nloop:\n z = z^3 + #pixel\nbailout:\n |z| < 4\n}";
        let rhai = convert_to_rhai(above, "Cubic", EntryKind::Formula, CustomDialect::UltraFractal, "cubic.ufm").expect("converts");
        assert!(rhai.contains("//\n// By A. Author, 2001.\n// A cubic.\n"), "{rhai}");
        // Fractint's conventions come out explicit.
        let frm = "F { z = pixel: z = sqr(z) + p1, z }";
        let rhai = convert_to_rhai(frm, "F", EntryKind::Formula, CustomDialect::Fractint, "f.frm").expect("converts");
        assert!(rhai.contains("converted from Fractint") && rhai.contains("!(s.z.re != 0.0)"), "{rhai}");
        let err = convert_to_rhai(text, "NoSuchEntry", EntryKind::Formula, CustomDialect::UltraFractal, "language.ufm").unwrap_err();
        assert!(err.contains("NoSuchEntry"));
        // A colouring converts to a Rhai colouring, which compiles as one.
        let ucl = include_str!("../../tests/formulas/language.ucl");
        let (parsed, _) = lang::parser::parse_file(ucl);
        let name = parsed.entries.iter().find(|e| lang::check::check(e, EntryKind::Coloring, Dialect::UltraFractal).0.is_some()).unwrap().name.clone();
        let rhai = convert_to_rhai(ucl, &name, EntryKind::Coloring, CustomDialect::UltraFractal, "language.ucl").expect("converts");
        assert!(rhai.contains("fn finish(s, z, p"), "{rhai}");
        let src = CustomSource { dialect: CustomDialect::Rhai, entry: name, source: rhai };
        assert!(check_entry(EntryKind::Coloring, &src).is_ok());
    }

    /// Every entry of the example files compiles, as the kind its file
    /// holds, and validates.
    #[test]
    fn every_example_entry_compiles() {
        use crate::storage::formula_store::{is_coloring_file, is_fractint, is_rhai, EXAMPLES};
        for (file, text) in EXAMPLES {
            // A Rhai file is one formula or colouring, as its functions say.
            if is_rhai(file) {
                let src = CustomSource { dialect: CustomDialect::Rhai, entry: lang::rhai::title(text).unwrap_or_default(), source: text.to_string() };
                let kind = lang::rhai::kind_of(text).unwrap_or_else(|| panic!("{file}: neither a formula nor a colouring"));
                let name = name_of(kind, &src);
                let compiled = match kind {
                    EntryKind::Formula => compile_formula(&src, &name).map(|_| ()),
                    EntryKind::Coloring => compile_coloring(&src, &name).map(|_| ()),
                };
                if let Err(d) = compiled {
                    panic!("{file}: {:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>());
                }
                continue;
            }
            let (parsed, d) = lang::parser::parse_file(text);
            assert!(d.is_empty(), "{file}: {:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>());
            assert!(parsed.entries.len() >= 4, "{file}: too few examples");
            for e in &parsed.entries {
                let src = CustomSource {
                    dialect: if is_fractint(file) { CustomDialect::Fractint } else { CustomDialect::UltraFractal },
                    entry: e.name.clone(),
                    source: carried_source(text, &parsed, e),
                };
                let kind = if is_coloring_file(file) { EntryKind::Coloring } else { EntryKind::Formula };
                let name = name_of(kind, &src);
                let ok = match kind {
                    EntryKind::Formula => compile_formula(&src, &name).map(|_| ()),
                    EntryKind::Coloring => compile_coloring(&src, &name).map(|_| ()),
                };
                if let Err(d) = ok {
                    panic!("{file} / {}: {:?}", e.name, d.iter().map(|x| x.render(&src.source)).collect::<Vec<_>>());
                }
            }
        }
        // The Mandelbrot example switches to its Julia: the toggle.
        let (_, text) = EXAMPLES.iter().find(|(f, _)| f.ends_with(".ufm")).expect("a formula file");
        let (parsed, _) = lang::parser::parse_file(text);
        let m = parsed.entries.iter().find(|e| e.name == "Mandelbrot").expect("the Mandelbrot example");
        let src = CustomSource { dialect: CustomDialect::UltraFractal, entry: m.name.clone(), source: carried_source(text, &parsed, m) };
        let def = compile_formula(&src, &name_of(EntryKind::Formula, &src)).expect("compiles");
        assert!(super::super::formula_julia_is_meaningful(def), "the switch is the Julia toggle");
    }

    #[test]
    fn names_are_stable_and_follow_the_source() {
        let a = src("M {\nloop:\n z = z^2 + #pixel\nbailout:\n |z| < 4\n}", "M");
        let n = name_of(EntryKind::Formula, &a);
        assert_eq!(n, name_of(EntryKind::Formula, &a.clone()));
        assert!(n.starts_with(PREFIX) && n.len() == PREFIX.len() + 16);
        let mut b = a.clone();
        b.source.push(' ');
        assert_ne!(n, name_of(EntryKind::Formula, &b), "an edit is a new name");
        assert_ne!(n, name_of(EntryKind::Coloring, &a), "a formula and a colouring never share one");
        // Pinned: a saved file must keep resolving to its entry.
        assert_eq!(name_of(EntryKind::Formula, &src("x", "y")), "custom-22c22db4acbfd02f");
    }

    #[test]
    fn a_registered_formula_resolves_with_its_parameters() {
        let text = "Pow {\ninit:\n z = 0\nloop:\n z = z^@power + #pixel + @seed\nbailout:\n |z| < @bailout\n\
                    default:\n float param power\n  default = 3\n  min = 2\n  max = 8\n endparam\n complex param seed\n endparam\n\
                    float param bailout\n  default = 4\n endparam\n func fn\n  default = sin()\n endfunc\n}\nOther {\n broken (\n}";
        let s = src(text, "pow");
        let name = name_of(EntryKind::Formula, &s);
        let def = compile_formula(&s, &name).expect("compiles; the broken entry beside it is not its business");
        assert!(std::ptr::eq(def, super::super::get_formula(&name)), "get_formula finds it");
        assert!(def.has_feature(FormulaFeature::Program));
        let names: Vec<&str> = def.parameters.iter().map(|p| p.name).collect();
        assert_eq!(names, ["power", "seed.re", "seed.im", "bailout", "fn"]);
        assert_eq!((def.parameters[0].min, def.parameters[0].max, def.parameters[0].default), (2.0, 8.0, 3.0));
        let Err(e) = compile_formula(&src(text, "missing"), "custom-x") else { panic!("a missing entry compiled") };
        assert!(e[0].message.contains("no entry named"));
    }

    #[test]
    fn ensure_registers_a_configs_sources_under_its_names() {
        let f = src("F {\nloop:\n z = z*z + #pixel\nbailout:\n |z| < 4\n}", "F");
        let c = src("C {\nfinal:\n #index = #numiter / 20\n}", "C");
        let mut esc = EscapeConfig::default();
        // The config's name, not a recomputed one, is what resolves.
        esc.formula = "custom-00000000000000f1".to_string();
        esc.coloring = "custom-00000000000000c1".to_string();
        esc.custom_formula = Some(f);
        esc.custom_coloring = Some(c);
        assert!(formula(&esc.formula).is_none());
        ensure(&esc);
        assert_eq!(super::super::get_formula(&esc.formula).name, esc.formula);
        assert_eq!(super::super::get_coloring(&esc.coloring).name, esc.coloring);
        // A broken source is refused once and falls back.
        esc.formula = "custom-00000000000000f2".to_string();
        esc.custom_formula = Some(src("F {\nloop:\n z = q\nbailout:\n true\n}", "F"));
        ensure(&esc);
        assert_eq!(super::super::get_formula(&esc.formula).name, "mandelbrot");
    }

    /// What first keeps a formula from derived perturbation equations:
    /// a rough tally for the measurement, by the loop's shape.
    fn perturb_blocker(p: &ir::Program) -> String {
        use ir::*;
        use lang::builtins::Builtin;
        if lang::delta::last_was_too_large() {
            return "equations past the size budget".into();
        }
        if let Some(why) = lang::delta::last_branch_refusal() {
            return why.into();
        }
        let Some(cases) = lang::delta::init_cases(p) else {
            return "a loop in the init, or a branch it cannot follow".into();
        };
        fn on_pixel(e: &IExpr) -> bool {
            match &e.kind {
                IExprKind::Predef(Predef::Pixel | Predef::Z | Predef::X | Predef::Y | Predef::ScreenPixel | Predef::Random | Predef::WhiteSq) => true,
                IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => on_pixel(a),
                IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => on_pixel(a) || on_pixel(b),
                _ => false,
            }
        }
        if cases.iter().any(|c| c.conds.iter().any(on_pixel)) {
            return "a branch in the init on the pixel".into();
        }
        let inits = cases[0].values.clone();
        fn first(e: &IExpr, p: &Program, inits: &[Option<IExpr>], out: &mut Option<String>) {
            if out.is_some() {
                return;
            }
            match &e.kind {
                IExprKind::Call(f, a) => {
                    // What derives: the exact forms, and the difference
                    // forms (step 6).
                    if !matches!(
                        f,
                        Builtin::Sqr
                            | Builtin::Ident
                            | Builtin::Zero
                            | Builtin::Conj
                            | Builtin::Flip
                            | Builtin::Real
                            | Builtin::Imag
                            | Builtin::Recip
                            | Builtin::Abs
                            | Builtin::Cabs
                            | Builtin::Sqrt
                            | Builtin::Exp
                            | Builtin::Expm1
                            | Builtin::Log
                            | Builtin::Log1p
                            | Builtin::Sin
                            | Builtin::Cos
                            | Builtin::Sinh
                            | Builtin::Cosh
                            | Builtin::Tan
                            | Builtin::Cotan
                            | Builtin::Tanh
                            | Builtin::Cotanh
                    ) {
                        *out = Some(format!("function {}", f.name()));
                        return;
                    }
                    first(a, p, inits, out);
                }
                // A user function derives by its choice (step 6b).
                IExprKind::UserCall(_, a) => first(a, p, inits, out),
                // A power derives by its difference form unless its
                // exponent varies.
                IExprKind::Binary(lang::ast::BinOp::Pow, a, b) => {
                    if varies(b, p, inits).is_some() {
                        *out = Some("a power of a varying exponent".into());
                        return;
                    }
                    first(a, p, inits, out);
                }
                IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::UserDeriv(_, a) => first(a, p, inits, out),
                IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => {
                    first(a, p, inits, out);
                    first(b, p, inits, out);
                }
                _ => {}
            }
        }
        // What makes a branch's condition differ from pixel to pixel.
        fn varies(e: &IExpr, p: &Program, inits: &[Option<IExpr>]) -> Option<&'static str> {
            fn pixel(e: &IExpr) -> bool {
                match &e.kind {
                    IExprKind::Predef(Predef::Pixel | Predef::Z) => true,
                    IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => pixel(a),
                    IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => pixel(a) || pixel(b),
                    _ => false,
                }
            }
            match &e.kind {
                IExprKind::Predef(Predef::Pixel | Predef::Z) => Some("a branch on the iterate"),
                IExprKind::Predef(Predef::NumIter) => Some("a branch on the iteration count"),
                IExprKind::Var(v) if !p.vars[*v].global => {
                    let by_pixel = lang::delta::writes_var(&p.loop_body, *v) || inits[*v].as_ref().map_or(p.init.iter().any(|s| matches!(s, IStmt::Assign(Place::Var(w), _) if w == v)), pixel);
                    by_pixel.then_some("a branch on the iterate")
                }
                IExprKind::Convert(a) | IExprKind::Unary(_, a) | IExprKind::PowInt(a, _) | IExprKind::Modulus(a) | IExprKind::Call(_, a) | IExprKind::UserCall(_, a) | IExprKind::UserDeriv(_, a) => varies(a, p, inits),
                IExprKind::MakeComplex(a, b) | IExprKind::Binary(_, a, b) | IExprKind::UserDiff(_, a, b) => varies(a, p, inits).or_else(|| varies(b, p, inits)),
                _ => None,
            }
        }
        fn walk(stmts: &[IStmt], p: &Program, inits: &[Option<IExpr>], found: &mut Option<String>) {
            for s in stmts {
                if found.is_some() {
                    return;
                }
                match s {
                    IStmt::Assign(_, e) => first(e, p, inits, found),
                    IStmt::If { arms, otherwise } => {
                        for (c, b) in arms {
                            // A branch on the iterate derives (step 6c),
                            // what its test reads as any expression; on
                            // the iteration count it does not.
                            match varies(c, p, inits) {
                                Some(why @ "a branch on the iteration count") => {
                                    *found = Some(why.into());
                                    return;
                                }
                                Some(_) => first(c, p, inits, found),
                                None => {}
                            }
                            walk(b, p, inits, found);
                        }
                        walk(otherwise, p, inits, found);
                    }
                    IStmt::While { .. } | IStmt::Repeat { .. } => *found = Some("a loop inside the loop".into()),
                }
            }
        }
        let mut found = None;
        walk(&p.loop_body, p, &inits, &mut found);
        // The loop has the forms; its test then (a carried value read by
        // it derives, step 5, so this is what is left: an init constant
        // poisoned, a variable never written).
        if found.is_none() && p.bailout.is_some() && lang::perturb::perturb_test(p).is_none() {
            return "its test reads a variable the loop changes, in a shape not followed".into();
        }
        found.unwrap_or_else(|| "an int or bool carried in the loop, or another shape".into())
    }

    /// INSPECTION: one formula of a UF file (`UF_INSPECT=path:entry`), as
    /// the converter and the derivation see it -- its Rhai, its derived
    /// equations and their WGSL -- printed, to look into what the
    /// measurement flags.
    #[test]
    #[ignore = "inspection: set UF_INSPECT to path:entry"]
    fn inspect_a_uf_formula() {
        let Ok(spec) = std::env::var("UF_INSPECT") else {
            eprintln!("UF_INSPECT is not set");
            return;
        };
        let (path, entry) = spec.rsplit_once(':').expect("path:entry");
        let text = String::from_utf8_lossy(&std::fs::read(path).expect("read")).into_owned();
        let (parsed, _) = lang::parser::parse_file(&text);
        let e = parsed.entries.iter().find(|e| e.name == entry).expect("the entry");
        let src = CustomSource { dialect: CustomDialect::UltraFractal, entry: e.name.clone(), source: carried_source(&text, &parsed, e) };
        let p = check_entry(EntryKind::Formula, &src).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(&src.source)).collect::<Vec<_>>()));
        let rhai = lang::to_rhai::formula(&p, &[]).unwrap_or_else(|e| e);
        println!("---- as Rhai
{rhai}");
        match lang::delta::with_derived_equations(&p) {
            None => println!("---- no equations derived{}", if lang::delta::last_was_too_large() { " (past the size budget)" } else { "" }),
            Some(d) => {
                println!("---- perturb_when: {:?}; perturbs: {:?}", d.perturb_when.is_some(), lang::perturb::perturbation(&d));
                let w = lang::wgsl::formula(&d);
                println!("---- WGSL
{w}");
                if let Err(err) = lang::wgsl::validate_formula(&w) {
                    println!("---- invalid: {err}");
                }
            }
        }
        if let Ok(back) = lang::rhai::program(&rhai) {
            match lang::delta::with_derived_equations(&back) {
                None => println!("---- the Rhai derives none"),
                Some(d) => println!("---- the Rhai derives; perturbs: {:?}", lang::perturb::perturbation(&d)),
            }
        }
    }

    /// MEASUREMENT (plan, phase 2 item 6): every entry of a folder of UF
    /// files (`UF_CORPUS`), checked as the editor would, with the first
    /// error of each that fails tallied, each colouring's state counted
    /// against the 4-float cap, and the WGSL of each that compiles
    /// validated (an invalid one is a compiler bug). The files are
    /// third-party, read for measurement and never bundled.
    #[test]
    #[ignore = "measurement: set UF_CORPUS to a folder of .ucl / .ufm files"]
    fn measure_a_uf_corpus() {
        use std::collections::BTreeMap;
        let Ok(dir) = std::env::var("UF_CORPUS") else {
            eprintln!("UF_CORPUS is not set");
            return;
        };
        // A message with its names and numbers blanked, so like counts
        // with like.
        let shape = |m: &str| {
            let mut out = String::new();
            let mut quoted = false;
            for ch in m.chars() {
                if ch == '`' {
                    quoted = !quoted;
                    if quoted {
                        out.push_str("`_`");
                    }
                } else if !quoted {
                    out.push(if ch.is_ascii_digit() { 'N' } else { ch });
                }
            }
            out
        };
        let mut paths: Vec<_> = std::fs::read_dir(&dir).expect("UF_CORPUS").filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        let mut first_errors: BTreeMap<(&str, String), usize> = BTreeMap::new();
        let mut samples: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut param_floats: BTreeMap<usize, usize> = BTreeMap::new();
        let mut state: BTreeMap<usize, usize> = BTreeMap::new();
        let (mut entries, mut compiled, mut parse_failed_files, mut over_cap_only) = ([0usize; 2], [0usize; 2], 0usize, 0usize);
        let mut invalid = Vec::new();
        let mut with_derivative = 0usize;
        let (mut own_equations, mut derived_equations) = (0usize, 0usize);
        let mut blockers: BTreeMap<String, usize> = BTreeMap::new();
        // Phase 4 step 3: each formula and colouring that compiles,
        // converted to Rhai; a colouring compared beside the Mandelbrot.
        let (mut converted, mut compiled_back, mut alike) = (0usize, 0usize, 0usize);
        let (mut c_converted, mut c_compiled_back, mut c_alike, mut c_same_state) = (0usize, 0usize, 0usize, 0usize);
        let mandelbrot = {
            let src = CustomSource {
                dialect: CustomDialect::UltraFractal,
                entry: "M".into(),
                source: "M {\ninit:\n z = 0\nloop:\n z = sqr(z) + #pixel\nbailout:\n |z| <= 4\n}".into(),
            };
            check_entry(EntryKind::Formula, &src).expect("the Mandelbrot")
        };
        let (mut zoomed, mut zoom_kept) = (0usize, 0usize);
        let mut convert_fails: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let rhai_out = std::env::var("UF_RHAI_OUT").ok();
        for path in paths {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let kind = match ext.as_str() {
                "ucl" => EntryKind::Coloring,
                "ufm" => EntryKind::Formula,
                _ => continue,
            };
            let k = (kind == EntryKind::Coloring) as usize;
            let text = String::from_utf8_lossy(&std::fs::read(&path).expect("read")).into_owned();
            let (parsed, d) = lang::parser::parse_file(&text);
            if lang::has_errors(&d) {
                parse_failed_files += 1;
            }
            for e in &parsed.entries {
                entries[k] += 1;
                // UF_TRACE: each entry named as it is measured, to find
                // one that stops the measurement.
                if std::env::var("UF_TRACE").is_ok() {
                    eprintln!("TRACE {} / {}", path.display(), e.name);
                }
                let src = CustomSource { dialect: CustomDialect::UltraFractal, entry: e.name.clone(), source: carried_source(&text, &parsed, e) };
                match check_entry(kind, &src) {
                    Ok(p) => {
                        compiled[k] += 1;
                        if kind == EntryKind::Coloring {
                            let mut fail = |why: String, text: Option<&str>| {
                                let key = format!("colouring: {}", shape(&why));
                                let list = convert_fails.entry(key).or_default();
                                if list.len() < 3 {
                                    list.push(format!("{} / {}: {why}", path.file_name().unwrap().to_string_lossy(), e.name));
                                }
                                if let (Some(dir), Some(t)) = (&rhai_out, text) {
                                    let _ = std::fs::write(format!("{dir}/FAILED-{}-{}.rhai", path.file_stem().unwrap().to_string_lossy(), e.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")), t);
                                }
                            };
                            match lang::to_rhai::coloring(&p, &[]) {
                                Err(why) => fail(why, None),
                                Ok(text) => {
                                    c_converted += 1;
                                    match lang::rhai::coloring(&text) {
                                        Err(d) => fail(d.first().map(|x| x.render(&text)).unwrap_or_default(), Some(&text)),
                                        Ok(back) => {
                                            c_compiled_back += 1;
                                            match lang::to_rhai::parting_coloring(&mandelbrot, &p, &back, 100) {
                                                Some(why) => fail(format!("colours otherwise, {why}"), Some(&text)),
                                                None => c_alike += 1,
                                            }
                                            if back.state_floats() == p.state_floats() {
                                                c_same_state += 1;
                                            } else {
                                                fail(format!("keeps {} floats, the original {}", back.state_floats(), p.state_floats()), Some(&text));
                                            }
                                            if let Some(dir) = &rhai_out {
                                                let _ = std::fs::write(
                                                    format!("{dir}/{}-{}.rhai", path.file_stem().unwrap().to_string_lossy(), e.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")),
                                                    &text,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            *state.entry(p.state_floats()).or_default() += 1;
                            let c = lang::wgsl::coloring(&p);
                            if let Err(err) = lang::wgsl::validate_coloring(&c) {
                                invalid.push(format!("{} / {}: {}", path.display(), e.name, err.lines().next().unwrap_or("")));
                            }
                        } else {
                            // Converted, compiled back, run alike; deep zoom kept.
                            // Equations that hold at the defaults, on either
                            // plane.
                            let holds = |d: &ir::Program| {
                                let params = lang::interp::ParamValues::defaults(d);
                                [false, true].into_iter().any(|julia| {
                                    let mut env = lang::interp::Env::at(lang::builtins::C64::ZERO, 100);
                                    env.julia = julia;
                                    lang::interp::perturb_holds(d, &params, &env)
                                })
                            };
                            let perturbs = |p: &ir::Program| {
                                (lang::perturb::perturbation(p).is_ok() && holds(p))
                                    || lang::delta::with_derived_equations(p).is_some_and(|d| lang::perturb::perturbation(&d).is_ok() && holds(&d))
                            };
                            let mut fail = |why: String, text: Option<&str>| {
                                let key = shape(&why);
                                let list = convert_fails.entry(key).or_default();
                                if list.len() < 3 {
                                    list.push(format!("{} / {}: {why}", path.file_name().unwrap().to_string_lossy(), e.name));
                                }
                                if let (Some(dir), Some(t)) = (&rhai_out, text) {
                                    let _ = std::fs::write(format!("{dir}/FAILED-{}-{}.rhai", path.file_stem().unwrap().to_string_lossy(), e.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")), t);
                                }
                            };
                            match lang::to_rhai::formula(&p, &[]) {
                                Err(why) => fail(why, None),
                                Ok(text) => {
                                    converted += 1;
                                    match lang::rhai::program(&text) {
                                        Err(d) => fail(d.first().map(|x| x.render(&text)).unwrap_or_default(), Some(&text)),
                                        Ok(back) => {
                                            compiled_back += 1;
                                            match lang::to_rhai::parting(&p, &back, 100) {
                                                Some(why) => fail(format!("runs otherwise, {why}"), Some(&text)),
                                                None => alike += 1,
                                            }
                                            if perturbs(&p) {
                                                zoomed += 1;
                                                if perturbs(&back) {
                                                    zoom_kept += 1;
                                                } else {
                                                    fail("deep zoom lost".into(), Some(&text));
                                                }
                                            }
                                            if let Some(dir) = &rhai_out {
                                                let _ = std::fs::write(
                                                    format!("{dir}/{}-{}.rhai", path.file_stem().unwrap().to_string_lossy(), e.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")),
                                                    &text,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            if lang::perturb::perturbation(&p).is_ok() {
                                own_equations += 1;
                            } else if let Some(d) = lang::delta::with_derived_equations(&p) {
                                // The equations' WGSL, scaled rung (the floatexp
                                // one validates assembled, in the GPU tests).
                                if let Err(err) = lang::wgsl::validate_formula(&lang::wgsl::formula(&d)) {
                                    invalid.push(format!("{} / {} (derived equations): {}", path.display(), e.name, err.lines().next().unwrap_or("")));
                                }
                                match lang::perturb::perturbation(&d) {
                                    Ok(_) if holds(&d) => derived_equations += 1,
                                    Ok(_) => *blockers.entry("equations that hold at no default (an arm a parameter picks went without)".into()).or_default() += 1,
                                    Err(why) => *blockers.entry(why).or_default() += 1,
                                }
                            } else {
                                let why = perturb_blocker(&p);
                                *blockers.entry(why).or_default() += 1;
                            }
                            let w = lang::wgsl::formula(&p);
                            if let Err(err) = lang::wgsl::validate_formula(&w) {
                                invalid.push(format!("{} / {}: {}", path.display(), e.name, err.lines().next().unwrap_or("")));
                            }
                            if let Some(dw) = lang::wgsl::formula_with_derivative(&p) {
                                with_derivative += 1;
                                if let Err(err) = lang::wgsl::validate_formula(&dw) {
                                    invalid.push(format!("{} / {} (derivative): {}", path.display(), e.name, err.lines().next().unwrap_or("")));
                                }
                            }
                        }
                    }
                    Err(d) => {
                        let errors: Vec<&Diag> = d.iter().filter(|x| x.severity == lang::Severity::Error).collect();
                        let cap: Vec<usize> = errors
                            .iter()
                            .filter_map(|x| x.message.strip_prefix("this colouring keeps "))
                            .filter_map(|m| m.split_whitespace().next()?.parse().ok())
                            .collect();
                        for x in &errors {
                            if let Some(n) = x.message.strip_prefix("the parameters take ").and_then(|m| m.split_whitespace().next()?.parse::<usize>().ok()) {
                                *param_floats.entry(n.div_ceil(8) * 8).or_default() += 1;
                            }
                        }
                        if errors.len() == 1 && cap.len() == 1 {
                            over_cap_only += 1;
                            *state.entry(cap[0]).or_default() += 1;
                        }
                        let first = errors.first().map_or("(no error)".to_string(), |x| shape(&x.message));
                        if let Some(x) = errors.first() {
                            let list = samples.entry(first.clone()).or_default();
                            if list.len() < 3 {
                                let line = src.source.lines().nth(lang::line_col(&src.source, x.span.start).0.saturating_sub(1)).unwrap_or("");
                                list.push(format!("{} / {}: {} | {}", path.file_name().unwrap().to_string_lossy(), e.name, x.render(&src.source), line.trim()));
                            }
                        }
                        *first_errors.entry((if k == 1 { "colouring" } else { "formula" }, first)).or_default() += 1;
                    }
                }
            }
        }
        println!("files that do not parse cleanly: {parse_failed_files}");
        for (k, name) in [(0, "formulas"), (1, "colourings")] {
            println!("{name}: {} of {} entries compile", compiled[k], entries[k]);
        }
        println!("formulas with a derivative: {with_derivative}");
        println!("formulas that perturb: {own_equations} by their own equations, {derived_equations} by derived ones");
        let mut ranked: Vec<_> = blockers.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1));
        for (why, n) in ranked.iter().take(25) {
            println!("  {n:>5} {why}");
        }
        println!("colouring state floats (compiling, or failing on the cap alone; {over_cap_only} of the latter):");
        for (floats, n) in &state {
            println!("  {floats:>3} floats: {n}");
        }
        println!("entries over the parameter cap, by floats needed (rounded up to 8s):");
        for (floats, n) in &param_floats {
            println!("  <= {floats:>3}: {n}");
        }
        let mut ranked: Vec<_> = first_errors.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        println!("first errors, most common:");
        for ((kind, m), n) in ranked.iter().take(45) {
            println!("  {n:>5}  {kind}: {m}");
            if std::env::var("UF_SAMPLES").is_ok() {
                for s in samples.get(m).into_iter().flatten() {
                    println!("           {s}");
                }
            }
        }
        println!(
            "converted to Rhai: {converted} of {} formulas; {compiled_back} compile back; {alike} run as the original; deep zoom kept {zoom_kept} of {zoomed}",
            compiled[0]
        );
        println!(
            "converted to Rhai: {c_converted} of {} colourings; {c_compiled_back} compile back; {c_alike} colour as the original; {c_same_state} keep the same state",
            compiled[1]
        );
        let mut ranked: Vec<_> = convert_fails.iter().collect();
        ranked.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        for (why, samples) in ranked.iter().take(30) {
            println!("  {why}");
            for s in samples.iter() {
                println!("      {s}");
            }
        }
        println!("invalid WGSL (compiler bugs): {}", invalid.len());
        for line in invalid.iter().take(3) {
            println!("  {line}");
        }
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for line in &invalid {
            *kinds.entry(line.rsplit(": error: ").next().unwrap_or("").to_string()).or_default() += 1;
        }
        for (k, n) in kinds {
            println!("  {n:>5} {k}");
        }
    }
}


#[cfg(test)]
mod bla_cost {
    use super::*;

    /// MEASUREMENT (phase 4, step 9): what a custom formula's BLA steps
    /// cost on the CPU for a million-step orbit (an interior point's).
    #[test]
    #[ignore = "measurement"]
    fn measure_custom_bla_steps() {
        use lang::builtins::C64;
        let src = crate::escape::twins::custom_source("mandelbrot").unwrap();
        let mut esc = EscapeConfig { bailout: 4.0, ..EscapeConfig::default() };
        esc.formula = name_of(EntryKind::Formula, &src);
        esc.custom_formula = Some(src);
        ensure(&esc);
        let c = C64::new(-0.1, 0.1);
        let mut orbit = Vec::with_capacity(1_000_001);
        let mut z = C64::ZERO;
        orbit.push([0.0, 0.0]);
        for _ in 0..1_000_000 {
            z = z.mul(z).add(c);
            orbit.push([z.re, z.im]);
        }
        let t = std::time::Instant::now();
        let steps = bla_steps(&esc, &orbit, c, 0.0).unwrap();
        println!("{} steps in {:.2} s", steps.len(), t.elapsed().as_secs_f64());
        // The Tricorn's twin, by the built-in's path, on a 20,000-step
        // orbit (the deep Tricorn view's).
        let tricorn = EscapeConfig { formula: "tricorn".into(), bailout: 4.0, ..EscapeConfig::default() };
        let t = std::time::Instant::now();
        let steps = twin_bla_steps(&tricorn, "tricorn", &[("power", 2.0)], &orbit[..20_000], c, 1e-9).unwrap();
        println!("the Tricorn's twin: {} steps in {:.3} s", steps.len(), t.elapsed().as_secs_f64());
        let t = std::time::Instant::now();
        let steps = twin_bla_steps(&tricorn, "tricorn", &[("power", 2.0)], &orbit[..20_000], c, 1e-9).unwrap();
        println!("again (compiled): {} steps in {:.3} s", steps.len(), t.elapsed().as_secs_f64());
    }
}
