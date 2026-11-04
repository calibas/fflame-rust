//! Headless script running: `FractalArtEditor generate`.
//!
//! Phase 1's proving ground — the whole engine is exercised without any
//! UI, so script semantics can be tested and iterated on before a panel
//! exists.

use std::collections::HashMap;
use std::path::Path;

use crate::config::fractal_config::FractalConfig;

use super::{ParamValue, ScriptHost, ScriptKind};
use crate::params::{Given, ParamDef, ParamKind};

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
    // As `export_mode` does. Without it every `log::warn!` on this path
    // is discarded — including "your script is being ignored because it
    // takes a shipped name", which is precisely the kind of thing a
    // headless run needs to say out loud.
    let _ = env_logger::try_init();

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

    // Same palette library the app loads, and the same script library,
    // so `generate` and the Scripts panel resolve names identically —
    // including one script calling another by id.
    let (entries, conflicts) = super::library::discover_with_conflicts(&base);
    // Report through stderr, not the log: env_logger defaults to `error`,
    // so a `warn!` here would be invisible unless someone thought to set
    // RUST_LOG — and a user script being ignored is exactly what a
    // headless run must not swallow.
    for stem in &conflicts {
        eprintln!(
            "Warning: your `{stem}.rhai` was not loaded — `{stem}` is a shipped script's name. \
             Rename it to use it."
        );
    }
    let host = ScriptHost::with_palettes(
        crate::scene::palette::PaletteLibrary::new().iter().cloned().collect(),
    )
    .with_scripts(entries.into_iter().map(|e| (e.id, e.source)).collect());

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
            let key = &p.name;
            match &p.kind {
                ParamKind::Color { default } => println!(
                    "  {key}: colour = {}",
                    crate::script::color::ScriptColor::from_rgb(*default).to_hex()
                ),
                ParamKind::Float { default, min, max, .. } => {
                    println!("  {key}: number = {default}  [{min} … {max}]")
                }
                ParamKind::Int { default, min, max, .. } => {
                    println!("  {key}: integer = {default}  [{min} … {max}]")
                }
                ParamKind::Bool { default } => {
                    println!("  {key}: true/false = {default}")
                }
                ParamKind::Text { default, .. } => {
                    println!("  {key}: text = \"{default}\"")
                }
                ParamKind::Choice { options, default } => println!(
                    "  {key}: one of [{}] = {}",
                    options.iter().collect::<Vec<_>>().join(", "),
                    options.get(*default).unwrap_or("")
                ),
            }
        }
        return;
    }

    if meta.kind == Some(ScriptKind::Library) {
        eprintln!(
            "`{}` is a library: it has nothing to generate. Import it from another script \
             with `import \"<its id>\" as lib;`.",
            meta.name
        );
        std::process::exit(1);
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

    // A script that defined an animation gets a .anim beside its .fflame.
    // Written only when there is one, so scripts that don't animate leave
    // no stray file.
    if let Some(animation) = &outcome.animation {
        let anim_path = Path::new(&out_path).with_extension("anim");
        match animation.to_json() {
            Ok(json) => match std::fs::write(&anim_path, json) {
                Ok(()) => println!(
                    "Wrote {} — {:.3}s, {} track(s)",
                    anim_path.display(),
                    animation.duration,
                    animation.tracks.len()
                ),
                Err(e) => eprintln!("Error: cannot write `{}`: {e}", anim_path.display()),
            },
            Err(e) => eprintln!("Error: cannot serialize the animation: {e}"),
        }
    }
}

/// Turn `key=value` strings into typed values using the declarations
/// (`params::parse_all`, as the script module reads its JSON).
fn resolve_sets(sets: &[String], decls: &[ParamDef]) -> Result<HashMap<String, ParamValue>, String> {
    let pairs = sets
        .iter()
        .map(|raw| raw.split_once('=').map(|(k, v)| (k, Given::Text(v))).ok_or_else(|| format!("--set expects key=value, got `{raw}`")))
        .collect::<Result<Vec<_>, _>>()?;
    crate::params::parse_all(decls, pairs)
}
