//! Every built-in parameter of every kind, as one list, to hold the move
//! to one parameter system to what each kind declared (step 1 of
//! docs/projects/one-parameter-system.md).
//!
//! `write_params_snapshot` writes `output/params-snapshot.json` (run it
//! on the types before a step); `params_match_the_snapshot` compares
//! today's dump with it (after the step). Both are ignored: the
//! snapshot is a local file, and nothing may depend on `output/`.
//!
//! A row is what a panel draws: the kind and owner, the name, label and
//! hint, the control (`float`, `unlimited_float`, `integer`,
//! `unlimited_integer`, `boolean`, `angle`, `choice`, `text`, `color`),
//! the default, the range the control uses (a variation's missing bound
//! as its renderer fills it), the choices, and whether it is slow or
//! logarithmic.

use serde_json::{json, Value};

const PATH: &str = "output/params-snapshot.json";

#[allow(clippy::too_many_arguments)]
fn row(kind: &str, owner: &str, name: &str, label: &str, hint: &str, control: &str, default: f64, range: (f64, f64), choices: Vec<String>, slow: bool, log: bool) -> Value {
    json!({
        "kind": kind, "owner": owner, "name": name, "label": label, "hint": hint, "control": control,
        "default": default, "min": range.0, "max": range.1, "choices": choices, "slow": slow, "log": log,
    })
}

/// The rows, in a stable order.
fn dump() -> Vec<Value> {
    let mut rows = Vec::new();
    variations(&mut rows);
    effects(&mut rows);
    escape(&mut rows);
    sim(&mut rows);
    scripts(&mut rows);
    rows
}

/// A variation's or an effect's control and the range its renderer
/// uses (`ui::variation_params`: a missing bound filled by the control,
/// or by the slow control's 1..100).
fn control_of(t: &crate::variations::ParamType, min: Option<f32>, max: Option<f32>, slow: bool) -> (&'static str, (f64, f64), Vec<String>) {
    use crate::variations::ParamType as T;
    let (control, lo, hi) = match t {
        T::Float => ("float", -10.0, 10.0),
        T::UnlimitedFloat => ("unlimited_float", -10.0, 10.0),
        T::Integer => ("integer", 1.0, 10.0),
        T::UnlimitedInteger => ("unlimited_integer", -100.0, 100.0),
        T::Boolean => ("boolean", 0.0, 1.0),
        T::Angle => ("angle", 0.0, 360.0),
        T::Enum { .. } => ("choice", 0.0, 0.0),
    };
    let (lo, hi) = if slow { (1.0, 100.0) } else { (lo, hi) };
    let choices = match t {
        T::Enum { choices } => choices.iter().map(|c| c.to_string()).collect(),
        _ => Vec::new(),
    };
    let range = (f64::from(min.unwrap_or(lo)), f64::from(max.unwrap_or(hi)));
    (control, range, choices)
}

fn variations(rows: &mut Vec<Value>) {
    let registry = crate::variations::global_registry();
    let mut all = registry.all();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    for v in all.into_iter().filter(|v| v.provenance.is_builtin()) {
        for p in &v.parameters {
            let slow = crate::variations::is_slow_param(&v.name, &p.name);
            let (control, range, choices) = control_of(&p.param_type, p.min_value, p.max_value, slow);
            rows.push(row("variation", &v.name, &p.name, &p.display_name, p.description.as_deref().unwrap_or(""), control, f64::from(p.default_value), range, choices, slow, false));
        }
    }
}

fn effects(rows: &mut Vec<Value>) {
    let registry = crate::effects::global_effect_registry();
    let mut all: Vec<_> = registry.all().filter(|e| e.provenance.is_builtin()).collect();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    for e in all {
        rows.extend(e.parameters.iter().map(|p| def_row("effect", &e.name, p)));
    }
}

fn escape_row(rows: &mut Vec<Value>, kind: &str, owner: &str, params: &[crate::params::ParamDef]) {
    rows.extend(params.iter().map(|p| def_row(kind, owner, p)));
}

fn escape(rows: &mut Vec<Value>) {
    for f in crate::escape::FORMULAS {
        escape_row(rows, "escape-formula", f.name, f.parameters);
    }
    for c in crate::escape::COLORINGS {
        escape_row(rows, "escape-coloring", c.name, c.parameters);
    }
    for f in crate::escape::fields::FIELDS {
        escape_row(rows, "escape-field", f.name, f.parameters);
    }
    for c in crate::escape::fields::FIELD_COLORINGS {
        escape_row(rows, "escape-field-coloring", c.name, c.parameters);
    }
    for s in crate::escape::ifs::IFS_DEFS {
        escape_row(rows, "escape-solid", s.name, s.parameters);
    }
    for c in crate::escape::ifs::IFS_COLORINGS {
        escape_row(rows, "escape-solid-coloring", c.name, c.parameters);
    }
    escape_row(rows, "escape-solid-template", "template", crate::escape::ifs::template_parameters());
}

/// A [`ParamDef`]'s row: its control by its kind, its range (a choice's
/// its indices, a switch's 0..1), its flags as they are.
fn def_row(kind: &str, owner: &str, d: &crate::params::ParamDef) -> Value {
    use crate::params::ParamKind as K;
    let (control, default, range, choices) = match &d.kind {
        K::Float { default, min, max, free, angle, .. } => (if *free { "unlimited_float" } else if *angle { "angle" } else { "float" }, *default, (*min, *max), Vec::new()),
        K::Int { default, min, max, free } => (if *free { "unlimited_integer" } else { "integer" }, *default as f64, (*min as f64, *max as f64), Vec::new()),
        K::Bool { default } => ("boolean", f64::from(u8::from(*default)), (0.0, 1.0), Vec::new()),
        K::Choice { default, options } => ("choice", *default as f64, (0.0, options.len().saturating_sub(1) as f64), options.iter().map(str::to_string).collect()),
        K::Text { default, max_len } => ("text", 0.0, (0.0, *max_len as f64), vec![default.to_string()]),
        K::Color { default } => ("color", 0.0, (0.0, 0.0), default.iter().map(|c| c.to_string()).collect()),
    };
    let log = matches!(d.kind, K::Float { log: true, .. });
    row(kind, owner, &d.name, &d.label, &d.hint, control, default, range, choices, d.slow, log)
}

fn sim(rows: &mut Vec<Value>) {
    for m in crate::sim::MODELS {
        rows.extend(m.parameters.iter().map(|p| def_row("sim-model", m.name, p)));
    }
    for c in crate::sim::COLORINGS {
        rows.extend(c.parameters.iter().map(|p| def_row("sim-coloring", c.name, p)));
    }
}

fn scripts(rows: &mut Vec<Value>) {
    use crate::script::ParamDecl as D;
    let host = crate::script::ScriptHost::new();
    let base = crate::config::FractalConfig::default();
    for (file, source) in crate::script::library::EMBEDDED {
        let meta = host.collect(source, &base).unwrap_or_else(|e| panic!("{file}: {e}"));
        for p in &meta.params {
            let (name, label, control, default, range, choices) = match p {
                D::Float { key, label, default, min, max, angle } => (key, label, if *angle { "angle" } else { "float" }, *default, (*min, *max), Vec::new()),
                D::Int { key, label, default, min, max } => (key, label, "integer", *default as f64, (*min as f64, *max as f64), Vec::new()),
                D::Bool { key, label, default } => (key, label, "boolean", f64::from(u8::from(*default)), (0.0, 1.0), Vec::new()),
                D::Choice { key, label, options, default } => (key, label, "choice", *default as f64, (0.0, 0.0), options.clone()),
                D::Text { key, label, default, max_len } => (key, label, "text", 0.0, (0.0, *max_len as f64), vec![default.clone()]),
                D::Color { key, label, default } => (key, label, "color", 0.0, (0.0, 0.0), default.iter().map(|c| c.to_string()).collect()),
            };
            rows.push(row("script", file, name, label, "", control, default, range, choices, false, false));
        }
    }
}

#[test]
#[ignore = "writes output/params-snapshot.json: run on the types before a migration step"]
fn write_params_snapshot() {
    std::fs::create_dir_all("output").unwrap();
    let rows = dump();
    std::fs::write(PATH, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
    println!("{} parameters", rows.len());
}

#[test]
#[ignore = "reads output/params-snapshot.json, which write_params_snapshot makes"]
fn params_match_the_snapshot() {
    let want: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(PATH).expect("the snapshot: run write_params_snapshot first")).unwrap();
    // Through the same round trip as the file: serde_json's parse of a
    // long f64 can be an ulp off (no `float_roundtrip`), far below an f32.
    let got: Vec<Value> = serde_json::from_str(&serde_json::to_string(&dump()).unwrap()).unwrap();
    let mut off = Vec::new();
    for (a, b) in want.iter().zip(&got) {
        if a != b {
            off.push(format!("was {a}\n now {b}"));
        }
    }
    assert_eq!(want.len(), got.len(), "parameter count: {} rows were {}", got.len(), want.len());
    assert!(off.is_empty(), "{} parameters moved:\n{}", off.len(), off.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}
