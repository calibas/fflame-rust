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

use std::collections::HashMap;

use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::scene::palette::PaletteLibrary;
use fractal_flame_wgpu::script::{
    library, parse_doc, ParamDecl, ParamValue, ScriptHost, ScriptKind,
};

/// The host every call uses: the same palette library and the same
/// script library the app and the CLI resolve names against, so
/// `flame.set_palette("...")` and `run_script("random_palette", ...)`
/// behave identically from every entry point.
///
/// Built ONCE per thread and reused — this is load-bearing for
/// throughput, not tidiness. The old per-call construction went
/// through `library::discover`, which parses and declaration-runs
/// every script in the library to learn metadata a runner never reads:
/// ~46 ms of fixed overhead per `run`/`run_on` call, which the gallery
/// pays per tile stage — measured at 20–50% of tile throughput. Reuse
/// is safe because `ScriptHost::run`/`collect` are `&self` with
/// per-execute state (the main crate's tests share hosts across runs),
/// so determinism per (script, seed) is unaffected.
///
/// The snapshot is taken at first use: a user script saved to the
/// store afterwards is not visible until the module reloads. For the
/// gallery worker that is the correct trade.
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

fn decl_json(d: &ParamDecl) -> serde_json::Value {
    match d {
        ParamDecl::Float { key, label, default, min, max } => serde_json::json!({
            "type": "float", "key": key, "label": label,
            "default": default, "min": min, "max": max,
        }),
        ParamDecl::Int { key, label, default, min, max } => serde_json::json!({
            "type": "int", "key": key, "label": label,
            "default": default, "min": min, "max": max,
        }),
        ParamDecl::Bool { key, label, default } => serde_json::json!({
            "type": "bool", "key": key, "label": label, "default": default,
        }),
        ParamDecl::Choice { key, label, options, default } => serde_json::json!({
            "type": "choice", "key": key, "label": label,
            "options": options, "default": default,
        }),
        ParamDecl::Text { key, label, default, max_len } => serde_json::json!({
            "type": "text", "key": key, "label": label,
            "default": default, "max_len": max_len,
        }),
        ParamDecl::Color { key, label, default } => serde_json::json!({
            "type": "color", "key": key, "label": label, "default": default,
        }),
    }
}

/// Resolve a `{key: value}` JSON object against a script's declared
/// parameters — the same semantics as the CLI's `--set`, with JSON
/// types where the CLI has strings. Unknown keys are an error, not a
/// warning: a gallery URL carrying a mistyped param should fail loudly.
fn resolve_params(
    params_json: &str,
    decls: &[ParamDecl],
) -> Result<HashMap<String, ParamValue>, String> {
    let obj: serde_json::Map<String, serde_json::Value> = if params_json.trim().is_empty() {
        serde_json::Map::new()
    } else {
        serde_json::from_str(params_json)
            .map_err(|e| format!("params must be a JSON object: {e}"))?
    };

    let mut out = HashMap::new();
    for (key, value) in &obj {
        let decl = decls.iter().find(|d| d.key() == key).ok_or_else(|| {
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
                    .as_f64()
                    .ok_or_else(|| format!("`{key}` expects a number, got {value}"))?,
            ),
            ParamDecl::Int { .. } => ParamValue::Int(
                value
                    .as_i64()
                    .ok_or_else(|| format!("`{key}` expects a whole number, got {value}"))?,
            ),
            ParamDecl::Bool { .. } => ParamValue::Bool(
                value
                    .as_bool()
                    .ok_or_else(|| format!("`{key}` expects true/false, got {value}"))?,
            ),
            ParamDecl::Text { .. } => ParamValue::Text(
                value
                    .as_str()
                    .ok_or_else(|| format!("`{key}` expects a string, got {value}"))?
                    .to_string(),
            ),
            // The option's name or its index, as with --set.
            ParamDecl::Choice { options, .. } => {
                let idx = if let Some(s) = value.as_str() {
                    options.iter().position(|o| o.eq_ignore_ascii_case(s))
                } else {
                    value
                        .as_u64()
                        .map(|i| i as usize)
                        .filter(|i| *i < options.len())
                };
                ParamValue::Choice(idx.ok_or_else(|| {
                    format!(
                        "`{key}` expects one of [{}], got {value}",
                        options.join(", ")
                    )
                })?)
            }
            // A "#rrggbb" hex string (the CLI's form) or an [r, g, b]
            // array of 0–1 floats.
            ParamDecl::Color { .. } => {
                if let Some(s) = value.as_str() {
                    ParamValue::Color(
                        fractal_flame_wgpu::script::color::ScriptColor::from_hex(s)
                            .map_err(|e| format!("`{key}`: {e}"))?
                            .to_rgb(),
                    )
                } else if let Some(a) = value.as_array() {
                    let mut rgb = [0f32; 3];
                    if a.len() != 3 {
                        return Err(format!("`{key}` expects \"#rrggbb\" or [r, g, b]"));
                    }
                    for (i, v) in a.iter().enumerate() {
                        rgb[i] = v
                            .as_f64()
                            .ok_or_else(|| format!("`{key}` expects numbers in [r, g, b]"))?
                            as f32;
                    }
                    ParamValue::Color(rgb)
                } else {
                    return Err(format!("`{key}` expects \"#rrggbb\" or [r, g, b]"));
                }
            }
        };
        out.insert(key.clone(), parsed);
    }
    Ok(out)
}

/// The embedded script library, with each script's kind, doc summary,
/// flags and declared parameters — everything a picker needs in one
/// call. `flags.norng` matters to a gallery: such a script ignores the
/// seed, so a hallway of it would show one image over and over.
pub fn list_scripts_impl() -> Result<String, String> {
    let base = FractalConfig::default();
    let mut out = Vec::new();
    // The one API call that genuinely needs discover's metadata pass —
    // it runs once per picker, not per tile.
    for entry in library::discover(&base) {
        let doc = parse_doc(&entry.source);
        let (params, flags) = match with_host(|h| h.collect(&entry.source, &base)) {
            Ok(meta) => (
                meta.params.iter().map(decl_json).collect::<Vec<_>>(),
                serde_json::json!({ "norng": meta.flags.no_rng, "palette": meta.flags.palette }),
            ),
            // A broken script still lists (its doc says what it meant
            // to be); it just offers no parameters.
            Err(_) => (Vec::new(), serde_json::json!({ "norng": false, "palette": false })),
        };
        out.push(serde_json::json!({
            "id": entry.id,
            "name": entry.display_name,
            "kind": entry.kind.as_str(),
            "summary": doc.summary,
            "flags": flags,
            "params": params,
        }));
    }
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

/// A script's source by library id, so a caller can list once and run
/// by id without shipping sources itself.
pub fn script_source_impl(id: &str) -> Result<String, String> {
    library::sources()
        .into_iter()
        .find(|(sid, _)| sid == id)
        .map(|(_, source)| source)
        .ok_or_else(|| format!("no script with id `{id}`"))
}

/// Collect a script's metadata and declared parameters without
/// running it for real.
pub fn collect_params_impl(source: &str) -> Result<String, String> {
    let base = FractalConfig::default();
    let meta = with_host(|h| h.collect(source, &base)).map_err(|e| e.to_string())?;
    let params: Vec<serde_json::Value> = meta.params.iter().map(decl_json).collect();
    serde_json::to_string(&serde_json::json!({
        "name": meta.name,
        "kind": meta.kind.map(ScriptKind::as_str),
        "flags": { "norng": meta.flags.no_rng, "palette": meta.flags.palette },
        "params": params,
    }))
    .map_err(|e| e.to_string())
}

/// Run a script. `base_config_json` is `None` for a generator (starts
/// from the default config) and the current config for a modifier.
///
/// Returns an envelope:
/// `{ name, kind, warnings: [..], messages: [..], config_json: "..." }`
/// where `config_json` is the exact `FractalConfig::to_json()` string —
/// pass it untouched to the renderer or to the next `run_on`.
pub fn run_impl(
    source: &str,
    seed: u64,
    params_json: &str,
    base_config_json: Option<&str>,
) -> Result<String, String> {
    let base = match base_config_json {
        Some(json) => FractalConfig::from_json(json)
            .map_err(|e| format!("base config did not parse: {e}"))?,
        None => FractalConfig::default(),
    };

    let outcome = with_host(|h| -> Result<_, String> {
        let meta = h.collect(source, &base).map_err(|e| e.to_string())?;
        let params = resolve_params(params_json, &meta.params)?;
        h.run(source, &base, seed, params).map_err(|e| e.to_string())
    })?;

    let config_json = outcome.config.to_json().map_err(|e| e.to_string())?;
    // A script that defined an animation gets it in the envelope — the
    // `.anim` JSON the CLI would write beside the `.fflame`, standalone
    // (it carries the flame as its base_config). Null otherwise. This
    // is the Animation wing's door: a turntable room is useless if its
    // animation is silently dropped.
    let animation_json = match &outcome.animation {
        Some(a) => serde_json::Value::String(a.to_json().map_err(|e| e.to_string())?),
        None => serde_json::Value::Null,
    };
    serde_json::to_string(&serde_json::json!({
        "name": outcome.meta.name,
        "kind": outcome.meta.kind.map(ScriptKind::as_str),
        "warnings": outcome.warnings,
        "messages": outcome.messages,
        "config_json": config_json,
        "animation_json": animation_json,
    }))
    .map_err(|e| e.to_string())
}

/// Run a whole pipeline — a generator followed by any number of
/// modifiers — in ONE call, threading the config between stages
/// in memory.
///
/// `stages_json` is `[{ "source": "...", "params": {...} }, ...]`;
/// `params` may be omitted per stage. Every stage runs with the same
/// `seed`, exactly as the per-call loop did. The first stage starts
/// from `base_config_json` (or the default config when `None`).
///
/// This exists because the per-stage loop pays the config's JSON
/// round-trip at every boundary: `run_on` parses the base string and
/// re-serializes the result, and the caller carries the string across
/// the JS boundary twice per stage. Here the `FractalConfig` never
/// leaves Rust between stages. The result envelope is
/// `{ stages: [{name, kind, warnings, messages}, ...],
///    config_json, animation_json }` — `config_json` byte-identical
/// to what the equivalent `run` + `run_on` chain produces.
pub fn run_chain_impl(
    stages_json: &str,
    seed: u64,
    base_config_json: Option<&str>,
) -> Result<String, String> {
    struct Stage {
        source: String,
        params: serde_json::Value,
    }
    let raw: Vec<serde_json::Value> = serde_json::from_str(stages_json)
        .map_err(|e| format!("stages must be [{{source, params?}}, ...]: {e}"))?;
    let stages: Vec<Stage> = raw
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let source = v
                .get("source")
                .and_then(|s| s.as_str())
                .ok_or_else(|| format!("stage {i}: missing `source`"))?
                .to_string();
            let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
            Ok(Stage { source, params })
        })
        .collect::<Result<_, String>>()?;
    if stages.is_empty() {
        return Err("run_chain needs at least one stage".to_string());
    }

    let mut cfg = match base_config_json {
        Some(json) => FractalConfig::from_json(json)
            .map_err(|e| format!("base config did not parse: {e}"))?,
        None => FractalConfig::default(),
    };

    let mut stage_reports = Vec::new();
    let mut animation = None;
    with_host(|h| -> Result<(), String> {
        for (i, stage) in stages.iter().enumerate() {
            let label = |e: String| format!("stage {i}: {e}");
            let meta = h.collect(&stage.source, &cfg).map_err(|e| label(e.to_string()))?;
            let params_str = if stage.params.is_null() {
                String::new()
            } else {
                stage.params.to_string()
            };
            let params = resolve_params(&params_str, &meta.params).map_err(label)?;
            let outcome =
                h.run(&stage.source, &cfg, seed, params).map_err(|e| label(e.to_string()))?;
            stage_reports.push(serde_json::json!({
                "name": outcome.meta.name,
                "kind": outcome.meta.kind.map(ScriptKind::as_str),
                "warnings": outcome.warnings,
                "messages": outcome.messages,
            }));
            // Later stages win, matching the sequential loop where each
            // envelope replaced the last non-null the caller kept.
            if outcome.animation.is_some() {
                animation = outcome.animation;
            }
            cfg = outcome.config;
        }
        Ok(())
    })?;

    let config_json = cfg.to_json().map_err(|e| e.to_string())?;
    let animation_json = match &animation {
        Some(a) => serde_json::Value::String(a.to_json().map_err(|e| e.to_string())?),
        None => serde_json::Value::Null,
    };
    serde_json::to_string(&serde_json::json!({
        "stages": stage_reports,
        "config_json": config_json,
        "animation_json": animation_json,
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
