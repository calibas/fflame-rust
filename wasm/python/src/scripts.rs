//! Rhai scripts: the gallery script module's calls (wasm/script), with
//! the same semantics -- a script + seed gives byte-identical config
//! JSON on every platform, the RNG being a pinned-algorithm Pcg64Mcg.
//!
//! COPIED from wasm/script/src/lib.rs (its exports are wasm-bindgen's),
//! and listed for review in docs/projects/pyfflame.md, "Copied code":
//! the two should become one module of the engine's.

use std::collections::HashMap;

use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::params::{Given, ParamDef};
use fractal_flame_wgpu::scene::palette::PaletteLibrary;
use fractal_flame_wgpu::script::{library, parse_doc, ParamValue, ScriptHost, ScriptKind};
use serde_json::{json, Value};

/// One host per thread, built on first use: the same palette and script
/// libraries the app and the CLI resolve names against. Discovering the
/// library per call cost ~46 ms in the gallery (wasm/script's note).
fn with_host<R>(f: impl FnOnce(&ScriptHost) -> R) -> R {
    thread_local! {
        static HOST: std::cell::OnceCell<ScriptHost> = const { std::cell::OnceCell::new() };
    }
    HOST.with(|cell| {
        f(cell.get_or_init(|| {
            ScriptHost::with_palettes(PaletteLibrary::new().iter().cloned().collect())
                .with_scripts(library::sources())
        }))
    })
}

/// `{key: value}` against a script's declared parameters, as the CLI's
/// `--set` with JSON types. An unknown key is an error.
fn resolve_params(params: &Value, decls: &[ParamDef]) -> Result<HashMap<String, ParamValue>, String> {
    let empty = serde_json::Map::new();
    let obj = match params {
        Value::Null => &empty,
        Value::Object(o) => o,
        _ => return Err("params must be an object".into()),
    };
    fractal_flame_wgpu::params::parse_all(decls, obj.iter().map(|(k, v)| (k.as_str(), Given::Json(v))))
}

fn base_config(base: Option<&str>) -> Result<FractalConfig, String> {
    match base {
        Some(json) => FractalConfig::from_json(json).map_err(|e| format!("base config did not parse: {e}")),
        None => Ok(FractalConfig::default()),
    }
}

/// The embedded library: each script's id, name, kind, summary, flags
/// and parameters.
pub fn list() -> Result<Value, String> {
    let base = FractalConfig::default();
    let mut out = Vec::new();
    for entry in library::discover(&base) {
        let doc = parse_doc(&entry.source);
        let (params, flags) = match with_host(|h| h.collect(&entry.source, &base)) {
            Ok(meta) => (
                meta.params.iter().map(fractal_flame_wgpu::params::script_json).collect::<Vec<_>>(),
                json!({ "norng": meta.flags.no_rng, "palette": meta.flags.palette }),
            ),
            Err(_) => (Vec::new(), json!({ "norng": false, "palette": false })),
        };
        out.push(json!({
            "id": entry.id, "name": entry.display_name, "kind": entry.kind.as_str(),
            "summary": doc.summary, "flags": flags, "params": params,
        }));
    }
    Ok(Value::Array(out))
}

/// A library script's source by id.
pub fn source(id: &str) -> Result<Value, String> {
    library::sources()
        .into_iter()
        .find(|(sid, _)| sid == id)
        .map(|(_, source)| Value::String(source))
        .ok_or_else(|| format!("no script with id `{id}`"))
}

/// A script's name, kind, flags and declared parameters, without
/// running it for real.
pub fn params(source: &str) -> Result<Value, String> {
    let base = FractalConfig::default();
    let meta = with_host(|h| h.collect(source, &base)).map_err(|e| e.to_string())?;
    Ok(json!({
        "name": meta.name,
        "kind": meta.kind.map(ScriptKind::as_str),
        "flags": { "norng": meta.flags.no_rng, "palette": meta.flags.palette },
        "params": meta.params.iter().map(fractal_flame_wgpu::params::script_json).collect::<Vec<_>>(),
    }))
}

/// Run stages -- a generator and any modifiers -- with one seed, the
/// config threaded between them in memory. One stage is `run`.
/// `{ stages: [{name, kind, warnings, messages}], config, animation }`:
/// `config` is `FractalConfig::to_json()`'s string, `animation` the
/// `.anim` JSON a script defined (null if none).
pub fn run(stages: &[(String, Value)], seed: u64, base: Option<&str>) -> Result<Value, String> {
    if stages.is_empty() {
        return Err("a run needs at least one stage".into());
    }
    let mut cfg = base_config(base)?;
    let mut reports = Vec::new();
    let mut animation = None;
    with_host(|h| -> Result<(), String> {
        for (i, (source, params)) in stages.iter().enumerate() {
            let label = |e: String| if stages.len() > 1 { format!("stage {i}: {e}") } else { e };
            let meta = h.collect(source, &cfg).map_err(|e| label(e.to_string()))?;
            let params = resolve_params(params, &meta.params).map_err(label)?;
            let outcome = h.run(source, &cfg, seed, params).map_err(|e| label(e.to_string()))?;
            reports.push(json!({
                "name": outcome.meta.name,
                "kind": outcome.meta.kind.map(ScriptKind::as_str),
                "warnings": outcome.warnings,
                "messages": outcome.messages,
            }));
            if outcome.animation.is_some() {
                animation = outcome.animation;
            }
            cfg = outcome.config;
        }
        Ok(())
    })?;
    let animation = match &animation {
        Some(a) => Value::String(a.to_json().map_err(|e| e.to_string())?),
        None => Value::Null,
    };
    Ok(json!({
        "stages": reports,
        "config": cfg.to_json().map_err(|e| e.to_string())?,
        "animation": animation,
    }))
}
