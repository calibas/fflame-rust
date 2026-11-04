//! Rhai scripts: the engine's calls (`script::headless`, shared with the
//! gallery's script module), shaped for Python -- values rather than
//! JSON strings, `run` and a chain of stages one call. A script + seed
//! gives byte-identical config JSON on every platform, the RNG being a
//! pinned-algorithm Pcg64Mcg.

use fractal_flame_wgpu::script::headless;
use serde_json::{json, Value};

/// The embedded library: each script's id, name, kind, summary, flags
/// and parameters.
pub fn list() -> Result<Value, String> {
    Ok(headless::list())
}

/// A library script's source by id.
pub fn source(id: &str) -> Result<Value, String> {
    headless::source(id).map(Value::String)
}

/// A script's name, kind, flags and declared parameters, without
/// running it for real.
pub fn params(source: &str) -> Result<Value, String> {
    headless::params(source)
}

/// Run stages -- a generator and any modifiers -- with one seed, the
/// config threaded between them in memory. One stage is `run`.
/// `{ stages: [{name, kind, warnings, messages}], config, animation }`:
/// `config` is `FractalConfig::to_json()`'s string, `animation` the
/// `.anim` JSON a script defined (null if none). A failed stage of
/// several says which.
pub fn run(stages: &[(String, Value)], seed: u64, base: Option<&str>) -> Result<Value, String> {
    if stages.is_empty() {
        return Err("a run needs at least one stage".into());
    }
    let base = headless::base_config(base)?;
    let pairs: Vec<(&str, &Value)> = stages.iter().map(|(s, p)| (s.as_str(), p)).collect();
    let run = headless::run(&pairs, seed, base)
        .map_err(|e| if stages.len() > 1 { format!("stage {}: {}", e.index, e.message) } else { e.message })?;
    Ok(json!({
        "stages": run.stages,
        "config": run.config.to_json().map_err(|e| e.to_string())?,
        "animation": run.animation_json()?,
    }))
}
