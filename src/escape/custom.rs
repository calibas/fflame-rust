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
        CustomDialect::UltraFractal => Dialect::UltraFractal,
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
    });
    eat(src.entry.as_bytes());
    eat(src.source.as_bytes());
    format!("{PREFIX}{h:016x}")
}

/// Parse and check the named entry of a source. The source may hold
/// other entries; only this one is compiled.
pub fn check_entry(kind: EntryKind, src: &CustomSource) -> Result<ir::Program, Vec<Diag>> {
    let (file, mut diags) = lang::parser::parse_file(&src.source);
    let entry = file.entries.iter().find(|e| e.name.eq_ignore_ascii_case(&src.entry));
    let Some(entry) = entry else {
        return Err(vec![Diag::error(lang::Span::default(), format!("no entry named `{}`", src.entry))]);
    };
    // Errors elsewhere in the file are not this entry's.
    diags.retain(|d| d.span.start >= entry.span.start && d.span.end <= entry.span.end);
    let (program, d) = lang::check::check(entry, kind, dialect(src.dialect));
    diags.extend(d);
    match program {
        Some(p) if !lang::has_errors(&diags) => Ok(p),
        _ => Err(diags),
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// The panel's view of a program's parameters: one def per float slot,
/// in slot order, as `pack_params` packs them. A complex parameter is
/// two (`name.re`, `name.im`); a user function an enum over the
/// functions it may be.
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
        parameters: Box::leak(param_defs(&p).into_boxed_slice()),
        wgsl: leak(wgsl),
        wgsl_param_seed: "",
        wgsl_prev_init: "",
        escape_metric: EscapeMetric::NormSq,
        escape_degree: None,
        derived_data: None,
        wgsl_derivative: leak(derivative),
        presets: &[],
    }));
    if let Ok(mut r) = registry().write() {
        r.formulas.insert(name.to_string(), def);
        if !p.globals.is_empty() {
            r.programs.insert(name.to_string(), Arc::new(p));
        }
    }
    Ok(def)
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
        parameters: Box::leak(param_defs(&p).into_boxed_slice()),
        wgsl: leak(c.wgsl),
        accum_init: leak(c.accum_init),
        wgsl_accum: leak(c.wgsl_accum),
        recommended_bailout: None,
        pick_params: &[],
    }));
    if let Ok(mut r) = registry().write() {
        r.colorings.insert(name.to_string(), def);
        if !p.globals.is_empty() {
            r.programs.insert(name.to_string(), Arc::new(p));
        }
    }
    Ok(def)
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

    /// Every entry of the example files compiles, as the kind its file
    /// holds, and validates.
    #[test]
    fn every_example_entry_compiles() {
        use crate::storage::formula_store::{is_coloring_file, is_fractint, EXAMPLES};
        for (file, text) in EXAMPLES {
            let (parsed, d) = lang::parser::parse_file(text);
            assert!(d.is_empty(), "{file}: {:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>());
            assert!(parsed.entries.len() >= 4, "{file}: too few examples");
            for e in &parsed.entries {
                let src = CustomSource {
                    dialect: if is_fractint(file) { CustomDialect::Fractint } else { CustomDialect::UltraFractal },
                    entry: e.name.clone(),
                    source: text[e.span.start..e.span.end].to_string(),
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
}
