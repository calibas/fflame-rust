//! Headless script running: `fractal_flame_wgpu generate`.
//!
//! Phase 1's proving ground — the whole engine is exercised without any
//! UI, so script semantics can be tested and iterated on before a panel
//! exists.

use std::collections::HashMap;
use std::path::Path;

use crate::config::fractal_config::FractalConfig;

use super::{ParamDecl, ParamValue, ScriptHost, ScriptKind};

/// Run a script and write the resulting `.fflame`.
///
/// `sets` are `key=value` strings; they're resolved against the script's
/// declared parameters so `--set style=Parabolic` can name a choice
/// option rather than its index.
pub fn generate_mode(
    script_path: &str,
    output: Option<&str>,
    seed: u64,
    base_path: Option<&str>,
    sets: &[String],
    list_params: bool,
) {
    let text = match std::fs::read_to_string(script_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: cannot read script `{script_path}`: {e}");
            std::process::exit(1);
        }
    };

    let base = match base_path {
        Some(p) => match FractalConfig::load_from_file(Path::new(p)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: cannot read base config `{p}`: {e}");
                std::process::exit(1);
            }
        },
        None => FractalConfig::default(),
    };

    // Same palette library the app loads, so `generate` and the Scripts
    // panel resolve palette names identically.
    let host = ScriptHost::with_palettes(
        crate::scene::palette::PaletteLibrary::new().iter().cloned().collect(),
    );

    // Collect first: it tells us the declared parameters (so --set can be
    // type-checked) and the script's kind.
    let meta = match host.collect(&text, &base) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Script error ({script_path}) {e}");
            std::process::exit(1);
        }
    };

    if list_params {
        println!("{} [{}]",
            if meta.name.is_empty() { "(unnamed)" } else { &meta.name },
            meta.kind.map(ScriptKind::as_str).unwrap_or("generator"));
        if meta.params.is_empty() {
            println!("  (no parameters)");
        }
        for p in &meta.params {
            match p {
                ParamDecl::Float { key, default, min, max, .. } => {
                    println!("  {key}: number = {default}  [{min} … {max}]")
                }
                ParamDecl::Int { key, default, min, max, .. } => {
                    println!("  {key}: integer = {default}  [{min} … {max}]")
                }
                ParamDecl::Bool { key, default, .. } => {
                    println!("  {key}: true/false = {default}")
                }
                ParamDecl::Choice { key, options, default, .. } => println!(
                    "  {key}: one of [{}] = {}",
                    options.join(", "),
                    options.get(*default).map(String::as_str).unwrap_or("")
                ),
            }
        }
        return;
    }

    if meta.kind == Some(ScriptKind::Modifier) && base_path.is_none() {
        eprintln!(
            "Warning: `{}` is a modifier script but no --base was given; \
             it will modify a default flame.",
            meta.name
        );
    }

    let params = match resolve_sets(sets, &meta.params) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let outcome = match host.run(&text, &base, seed, params) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Script error ({script_path}) {e}");
            std::process::exit(1);
        }
    };

    for m in &outcome.messages {
        println!("{m}");
    }
    for w in &outcome.warnings {
        eprintln!("Warning: {w}");
    }

    let out_path = output.map(String::from).unwrap_or_else(|| {
        Path::new(script_path)
            .with_extension("fflame")
            .to_string_lossy()
            .into_owned()
    });

    if let Err(e) = outcome.config.save_to_file(Path::new(&out_path)) {
        eprintln!("Error: cannot write `{out_path}`: {e}");
        std::process::exit(1);
    }

    let flame = &outcome.config.flame;
    println!(
        "Wrote {out_path} — seed {seed}, {} transform(s){}",
        flame.transforms.len(),
        if flame.final_transforms.is_empty() {
            String::new()
        } else {
            format!(", {} final", flame.final_transforms.len())
        }
    );
}

/// Turn `key=value` strings into typed values using the declarations.
fn resolve_sets(
    sets: &[String],
    decls: &[ParamDecl],
) -> Result<HashMap<String, ParamValue>, String> {
    let mut out = HashMap::new();
    for raw in sets {
        let (key, value) = raw
            .split_once('=')
            .ok_or_else(|| format!("--set expects key=value, got `{raw}`"))?;
        let decl = decls
            .iter()
            .find(|d| d.key() == key)
            .ok_or_else(|| {
                let known: Vec<&str> = decls.iter().map(ParamDecl::key).collect();
                format!(
                    "script has no parameter `{key}`{}",
                    if known.is_empty() {
                        " (it declares none)".to_string()
                    } else {
                        format!(" (has: {})", known.join(", "))
                    }
                )
            })?;

        let parsed = match decl {
            ParamDecl::Float { .. } => ParamValue::Float(
                value
                    .parse::<f64>()
                    .map_err(|_| format!("`{key}` expects a number, got `{value}`"))?,
            ),
            ParamDecl::Int { .. } => ParamValue::Int(
                value
                    .parse::<i64>()
                    .map_err(|_| format!("`{key}` expects a whole number, got `{value}`"))?,
            ),
            ParamDecl::Bool { .. } => ParamValue::Bool(
                match value.to_ascii_lowercase().as_str() {
                    "true" | "yes" | "on" | "1" => true,
                    "false" | "no" | "off" | "0" => false,
                    _ => return Err(format!("`{key}` expects true/false, got `{value}`")),
                },
            ),
            ParamDecl::Choice { options, .. } => {
                // Accept the option's name or its index.
                let idx = options
                    .iter()
                    .position(|o| o.eq_ignore_ascii_case(value))
                    .or_else(|| value.parse::<usize>().ok().filter(|i| *i < options.len()))
                    .ok_or_else(|| {
                        format!(
                            "`{key}` expects one of [{}], got `{value}`",
                            options.join(", ")
                        )
                    })?;
                ParamValue::Choice(idx)
            }
        };
        out.insert(key.to_string(), parsed);
    }
    Ok(out)
}
