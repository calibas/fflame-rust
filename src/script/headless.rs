//! The script calls every surface without the editor makes -- the
//! gallery's script module (`wasm/script`) and Python (`wasm/python`):
//! list the library, a script's source and its parameters, run stages.
//! One home, so a script and a seed give the same config from each, and
//! the CLI's `generate` (`cli.rs`, through the same host) gives it too.
//! The surfaces only shape the results for their boundary (JSON strings
//! for wasm-bindgen, values for Python) and label a failed stage.

use std::collections::HashMap;

use serde_json::{json, Value};

use super::{library, parse_doc, ParamValue, ScriptHost, ScriptKind};
use crate::config::FractalConfig;
use crate::params::{Given, ParamDef};
use crate::scene::palette::PaletteLibrary;

/// The host every call uses: the same palette library and the same
/// script library the app and the CLI resolve names against, so
/// `flame.set_palette("...")` and `run_script("random_palette", ...)`
/// behave identically from every entry point.
///
/// Built ONCE per thread and reused -- this is load-bearing for
/// throughput, not tidiness. Building it per call went through
/// `library::discover`, which parses and declaration-runs every script in
/// the library to learn metadata a runner never reads: ~46 ms of fixed
/// overhead per run, which the gallery paid per tile stage -- measured at
/// 20-50% of tile throughput. Reuse is safe because `ScriptHost::run` /
/// `collect` are `&self` with per-execute state, so determinism per
/// (script, seed) is unaffected.
///
/// The snapshot is taken at first use: a user script saved to the store
/// afterwards is not visible until the module reloads. For the gallery
/// worker and a Python process that is the right trade.
pub fn with_host<R>(f: impl FnOnce(&ScriptHost) -> R) -> R {
    thread_local! {
        static HOST: std::cell::OnceCell<ScriptHost> = const { std::cell::OnceCell::new() };
    }
    HOST.with(|cell| {
        f(cell.get_or_init(|| {
            ScriptHost::with_palettes(PaletteLibrary::new().iter().cloned().collect()).with_scripts(library::sources())
        }))
    })
}

/// `{key: value}` against a script's declared parameters -- the CLI's
/// `--set` with JSON types where the CLI has strings (`params::parse_all`);
/// null is none. An unknown key is an error, not a warning: a gallery URL
/// carrying a mistyped parameter should fail loudly.
pub fn resolve_params(params: &Value, decls: &[ParamDef]) -> Result<HashMap<String, ParamValue>, String> {
    let empty = serde_json::Map::new();
    let obj = match params {
        Value::Null => &empty,
        Value::Object(o) => o,
        _ => return Err("params must be a JSON object".into()),
    };
    crate::params::parse_all(decls, obj.iter().map(|(k, v)| (k.as_str(), Given::Json(v))))
}

/// A run's starting config: a modifier's base, or the default config for
/// a generator.
pub fn base_config(json: Option<&str>) -> Result<FractalConfig, String> {
    match json {
        Some(json) => FractalConfig::from_json(json).map_err(|e| format!("base config did not parse: {e}")),
        None => Ok(FractalConfig::default()),
    }
}

/// The embedded library, with each script's id, name, kind, doc summary,
/// flags and declared parameters -- everything a picker needs in one
/// call. `flags.norng` matters to a gallery: such a script ignores the
/// seed, so a hallway of it would show one image over and over.
pub fn list() -> Value {
    let base = FractalConfig::default();
    let mut out = Vec::new();
    // The one call that genuinely needs discover's metadata pass -- it
    // runs once per picker, not per tile.
    for entry in library::discover(&base) {
        let doc = parse_doc(&entry.source);
        let (params, flags) = match with_host(|h| h.collect(&entry.source, &base)) {
            Ok(meta) => (
                meta.params.iter().map(crate::params::script_json).collect::<Vec<_>>(),
                json!({ "norng": meta.flags.no_rng, "palette": meta.flags.palette }),
            ),
            // A broken script still lists (its doc says what it meant to
            // be); it just offers no parameters.
            Err(_) => (Vec::new(), json!({ "norng": false, "palette": false })),
        };
        out.push(json!({
            "id": entry.id,
            "name": entry.display_name,
            "kind": entry.kind.as_str(),
            "summary": doc.summary,
            "flags": flags,
            "params": params,
        }));
    }
    Value::Array(out)
}

/// A script's source by library id, so a caller can list once and run by
/// id without shipping sources itself.
pub fn source(id: &str) -> Result<String, String> {
    library::sources()
        .into_iter()
        .find(|(sid, _)| sid == id)
        .map(|(_, source)| source)
        .ok_or_else(|| format!("no script with id `{id}`"))
}

/// A script's name, kind, flags and declared parameters, without running
/// it for real.
pub fn params(source: &str) -> Result<Value, String> {
    let base = FractalConfig::default();
    let meta = with_host(|h| h.collect(source, &base)).map_err(|e| e.to_string())?;
    Ok(json!({
        "name": meta.name,
        "kind": meta.kind.map(ScriptKind::as_str),
        "flags": { "norng": meta.flags.no_rng, "palette": meta.flags.palette },
        "params": meta.params.iter().map(crate::params::script_json).collect::<Vec<_>>(),
    }))
}

/// What a run made: a report per stage (`{name, kind, warnings,
/// messages}`), the config, and the animation the last stage to define
/// one defined.
pub struct Run {
    pub stages: Vec<Value>,
    pub config: FractalConfig,
    pub animation: Option<crate::animation::Animation>,
}

/// The stage that failed, and why; each surface labels it its own way.
#[derive(Debug)]
pub struct StageError {
    pub index: usize,
    pub message: String,
}

/// Run stages -- a generator and any modifiers, each a source and its
/// parameters -- with one seed, the config threaded between them in
/// memory: byte-identical to running them one call at a time, without the
/// config's JSON round trip at every boundary.
pub fn run(stages: &[(&str, &Value)], seed: u64, base: FractalConfig) -> Result<Run, StageError> {
    let mut config = base;
    let mut reports = Vec::new();
    let mut animation = None;
    with_host(|h| -> Result<(), StageError> {
        for (index, (source, params)) in stages.iter().enumerate() {
            let fail = |message: String| StageError { index, message };
            let meta = h.collect(source, &config).map_err(|e| fail(e.to_string()))?;
            let params = resolve_params(params, &meta.params).map_err(fail)?;
            let outcome = h.run(source, &config, seed, params).map_err(|e| fail(e.to_string()))?;
            reports.push(json!({
                "name": outcome.meta.name,
                "kind": outcome.meta.kind.map(ScriptKind::as_str),
                "warnings": outcome.warnings,
                "messages": outcome.messages,
            }));
            // Later stages win, as the one-call-at-a-time loop's caller
            // kept the last animation it was handed.
            if outcome.animation.is_some() {
                animation = outcome.animation;
            }
            config = outcome.config;
        }
        Ok(())
    })?;
    Ok(Run { stages: reports, config, animation })
}

impl Run {
    /// The animation as the `.anim` JSON the CLI would write beside the
    /// `.fflame` (it carries the flame as its base config), or null.
    pub fn animation_json(&self) -> Result<Value, String> {
        match &self.animation {
            Some(a) => Ok(Value::String(a.to_json().map_err(|e| e.to_string())?)),
            None => Ok(Value::Null),
        }
    }
}
