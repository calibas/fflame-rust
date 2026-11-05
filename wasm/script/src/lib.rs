//! fflame-script — the Endless Gallery's script evaluator.
//!
//! Thin wrapper over the main crate's sandboxed Rhai host. The API is
//! four calls: list the embedded script library, collect a script's
//! declared parameters, run a generator, run a modifier on a base
//! config. Everything crosses the boundary as JSON strings.
//!
//! The config a run returns (`config_json` in the envelope) is the
//! EXACT string `FractalConfig::to_json()` produces — the same bytes
//! the desktop `generate` CLI writes to an `.fflame` file. That is the
//! module's core guarantee: script + seed ⇒ byte-identical JSON on
//! every platform (the RNG is a pinned-algorithm Pcg64Mcg), verified
//! by the CLI-parity test in `tests/`.
//!
//! Determinism note for callers: every stage of a gallery pipeline —
//! the generator and each applied modifier — runs with the same
//! hallway seed `n`. Preferred form, one call per tile (the config
//! never round-trips through JSON between stages):
//!
//! ```js
//! let env = JSON.parse(run_chain(JSON.stringify([
//!     { source: gen_src },
//!     ...rooms.map(r => ({ source: r.src, params: r.params })),
//! ]), n, null));
//! ```
//!
//! The per-stage loop is equivalent (byte-identical `config_json`),
//! just slower:
//!
//! ```js
//! let env = JSON.parse(run(gen_src, n, "{}"));
//! for (const room of rooms) {
//!     env = JSON.parse(run_on(room.src, n, room.params, env.config_json));
//! }
//! ```

use fractal_flame_wgpu::script::headless;
use serde_json::{json, Value};

// The calls are the engine's (`script::headless`), shared with Python;
// this module shapes them for wasm-bindgen: JSON strings both ways.

/// JSON: the embedded script library, each script's kind, doc summary,
/// flags and declared parameters.
pub fn list_scripts_impl() -> Result<String, String> {
    serde_json::to_string(&headless::list()).map_err(|e| e.to_string())
}

/// A script's source by library id.
pub fn script_source_impl(id: &str) -> Result<String, String> {
    headless::source(id)
}

/// JSON: a script's metadata and declared parameters, without running it.
pub fn collect_params_impl(source: &str) -> Result<String, String> {
    serde_json::to_string(&headless::params(source)?).map_err(|e| e.to_string())
}

/// A stage's `params_json`: a JSON object, or empty for none.
fn params_value(params_json: &str) -> Result<Value, String> {
    if params_json.trim().is_empty() {
        return Ok(Value::Null);
    }
    let obj: serde_json::Map<String, Value> =
        serde_json::from_str(params_json).map_err(|e| format!("params must be a JSON object: {e}"))?;
    Ok(Value::Object(obj))
}

/// Run a script. `base_config_json` is `None` for a generator (starts
/// from the default config) and the current config for a modifier.
///
/// Returns an envelope:
/// `{ name, kind, warnings: [..], messages: [..], config_json: "...", animation_json }`
/// where `config_json` is the exact `FractalConfig::to_json()` string --
/// pass it untouched to the renderer or to the next `run_on` -- and
/// `animation_json` the `.anim` a script defined (null if none): the
/// Animation wing's door, as a turntable room is useless if its animation
/// is silently dropped.
pub fn run_impl(source: &str, seed: u64, params_json: &str, base_config_json: Option<&str>) -> Result<String, String> {
    let base = headless::base_config(base_config_json)?;
    let params = params_value(params_json)?;
    let run = headless::run(&[(source, &params)], seed, base).map_err(|e| e.message)?;
    let stage = &run.stages[0];
    serde_json::to_string(&json!({
        "name": stage["name"],
        "kind": stage["kind"],
        "warnings": stage["warnings"],
        "messages": stage["messages"],
        "config_json": run.config.to_json().map_err(|e| e.to_string())?,
        "animation_json": run.animation_json()?,
    }))
    .map_err(|e| e.to_string())
}

/// Run a whole pipeline -- a generator followed by any number of
/// modifiers -- in ONE call, threading the config between stages in
/// memory (`headless::run`).
///
/// `stages_json` is `[{ "source": "...", "params": {...} }, ...]`;
/// `params` may be omitted per stage. Every stage runs with the same
/// `seed`, exactly as the per-call loop did. The first stage starts from
/// `base_config_json` (or the default config when `None`). The result
/// envelope is `{ stages: [{name, kind, warnings, messages}, ...],
/// config_json, animation_json }` -- `config_json` byte-identical to what
/// the equivalent `run` + `run_on` chain produces, without that loop's
/// JSON round trip of the config at every boundary.
pub fn run_chain_impl(stages_json: &str, seed: u64, base_config_json: Option<&str>) -> Result<String, String> {
    let raw: Vec<Value> =
        serde_json::from_str(stages_json).map_err(|e| format!("stages must be [{{source, params?}}, ...]: {e}"))?;
    let stages: Vec<(String, Value)> = raw
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let source = v
                .get("source")
                .and_then(|s| s.as_str())
                .ok_or_else(|| format!("stage {i}: missing `source`"))?
                .to_string();
            Ok((source, v.get("params").cloned().unwrap_or(Value::Null)))
        })
        .collect::<Result<_, String>>()?;
    if stages.is_empty() {
        return Err("run_chain needs at least one stage".to_string());
    }
    let base = headless::base_config(base_config_json)?;
    let stages: Vec<(&str, &Value)> = stages.iter().map(|(s, p)| (s.as_str(), p)).collect();
    let run = headless::run(&stages, seed, base).map_err(|e| format!("stage {}: {}", e.index, e.message))?;
    serde_json::to_string(&json!({
        "stages": run.stages,
        "config_json": run.config.to_json().map_err(|e| e.to_string())?,
        "animation_json": run.animation_json()?,
    }))
    .map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    fn js(e: String) -> JsValue {
        JsValue::from_str(&e)
    }

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
    }

    /// JSON: `[{id, name, kind, summary, params: [...]}, ...]`
    #[wasm_bindgen]
    pub fn list_scripts() -> Result<String, JsValue> {
        crate::list_scripts_impl().map_err(js)
    }

    /// The source of an embedded script, by id from `list_scripts`.
    #[wasm_bindgen]
    pub fn script_source(id: &str) -> Result<String, JsValue> {
        crate::script_source_impl(id).map_err(js)
    }

    /// JSON: `{name, kind, params: [...]}` for arbitrary source.
    #[wasm_bindgen]
    pub fn collect_params(source: &str) -> Result<String, JsValue> {
        crate::collect_params_impl(source).map_err(js)
    }

    /// Run a generator. `seed` is a JS BigInt. Returns the envelope
    /// JSON; its `config_json` field is the config, byte-identical to
    /// the desktop CLI's output for the same script + seed + params.
    #[wasm_bindgen]
    pub fn run(source: &str, seed: u64, params_json: &str) -> Result<String, JsValue> {
        crate::run_impl(source, seed, params_json, None).map_err(js)
    }

    /// Run a modifier on a base config (a room in the gallery).
    #[wasm_bindgen]
    pub fn run_on(
        source: &str,
        seed: u64,
        params_json: &str,
        base_config_json: &str,
    ) -> Result<String, JsValue> {
        crate::run_impl(source, seed, params_json, Some(base_config_json)).map_err(js)
    }

    /// Run a generator + modifiers pipeline in one call — the config
    /// never round-trips through JSON between stages. Preferred over a
    /// `run` + `run_on` loop for per-tile work. `stages_json`:
    /// `[{source, params?}, ...]`; `base_config_json` optional (null =
    /// start from the default config, i.e. first stage is a generator).
    #[wasm_bindgen]
    pub fn run_chain(
        stages_json: &str,
        seed: u64,
        base_config_json: Option<String>,
    ) -> Result<String, JsValue> {
        crate::run_chain_impl(stages_json, seed, base_config_json.as_deref()).map_err(js)
    }
}
