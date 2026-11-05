//! pyfflame's engine (python/): the engine as a WASI module for Python.
//! wasmtime runs it; its GPU calls go to the host through a wgpu custom
//! backend (`backend`), and the host makes them with wgpu-py
//! (python/pyfflame/_gpu.py). docs/projects/pyfflame.md has the design.
//!
//! One call crosses the boundary: `fflame_call`, a JSON request
//! (`{"fn": ..., ...}`) and a block of bytes beside it (pixels to encode),
//! both in this module's memory. It answers with a status (0, or 1 with
//! the error's text as the output), an output of bytes (pixels, a PNG)
//! and a JSON note (`{"result": ...}` for the calls whose answer is data,
//! a render's numbers for the rest). The host allocates with
//! `fflame_alloc` and frees with `fflame_free`.
//!
//! The calls:
//! - `info`, `default_config`, `presets`
//! - `flame_import {xml}` (a `.flame` file's configs), `flame_export {config}`
//! - `render {config, width, height, sequence?, iterations?,
//!   iterations_per_thread?, transparent?, premultiplied?, sticky?}`:
//!   RGBA. `sequence` continues the last sequence frame's engine state
//!   -- a Python program animating a config.
//! - `encode_png {config, width, height, iterations, ms,
//!   iterations_per_thread}` + RGBA: the PNG the CLI writes, metadata
//!   and all.
//! - `animation_open {animation, config?, width, height, fps?, ...}`,
//!   `animation_frame {id, frame}`, `animation_config {id, frame}`,
//!   `animation_close {id}`: the animation engine's frames, as the video
//!   export renders them.
//! - `scripts`, `script_source {id}`, `script_params {source}`,
//!   `script_run {stages: [{source, params?}], seed?, base?}`: Rhai.

mod backend;
mod host;
mod render;
mod scripts;

use std::cell::RefCell;

use fractal_flame_wgpu::config::FractalConfig;
use serde_json::{json, Value};

use render::{Frame, Gpu, Options};

thread_local! {
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static INFO: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn fflame_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` and `len` from one `fflame_alloc`.
#[no_mangle]
pub unsafe extern "C" fn fflame_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

#[no_mangle]
pub extern "C" fn fflame_output_ptr() -> *const u8 {
    OUTPUT.with(|o| o.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn fflame_output_len() -> usize {
    OUTPUT.with(|o| o.borrow().len())
}

#[no_mangle]
pub extern "C" fn fflame_info_ptr() -> *const u8 {
    INFO.with(|o| o.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn fflame_info_len() -> usize {
    INFO.with(|o| o.borrow().len())
}

/// One call: the request is `len` bytes of JSON at `ptr`. Returns 0, or
/// 1 with the error's text as the output.
///
/// # Safety
/// `ptr`/`len` a UTF-8 string, `data`/`data_len` bytes, in this module's
/// memory.
#[no_mangle]
pub unsafe extern "C" fn fflame_call(ptr: *const u8, len: usize, data: *const u8, data_len: usize) -> i32 {
    let text = std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("");
    let data = if data_len == 0 { Vec::new() } else { std::slice::from_raw_parts(data, data_len).to_vec() };
    let outcome = serde_json::from_str::<Value>(text).map_err(|e| format!("request is not JSON: {e}")).and_then(|r| call(&r, data));
    let (status, output, info) = match outcome {
        Ok((output, info)) => (0, output, info.to_string().into_bytes()),
        Err(e) => (1, e.into_bytes(), Vec::new()),
    };
    OUTPUT.with(|o| *o.borrow_mut() = output);
    INFO.with(|o| *o.borrow_mut() = info);
    status
}

fn str_arg<'a>(r: &'a Value, key: &str) -> Result<&'a str, String> {
    r[key].as_str().ok_or_else(|| format!("`{key}` (a string) is missing"))
}

fn u32_arg(r: &Value, key: &str) -> Result<u32, String> {
    r[key].as_u64().map(|n| n as u32).ok_or_else(|| format!("`{key}` (a number) is missing"))
}

fn config_arg(r: &Value, key: &str) -> Result<FractalConfig, String> {
    FractalConfig::from_json(str_arg(r, key)?).map_err(|e| format!("`{key}` did not parse: {e}"))
}

/// Data, as the note's `result`.
fn data(v: Value) -> (Vec<u8>, Value) {
    (Vec::new(), json!({ "result": v }))
}

/// A frame: its pixels, and its numbers as the note.
fn frame_output(frame: Frame) -> Result<(Vec<u8>, Value), String> {
    let info = frame.info();
    Ok((frame.out.rgba_data, info))
}

fn with_gpu<R>(f: impl FnOnce(&mut Gpu) -> Result<R, String>) -> Result<R, String> {
    GPU.with(|g| {
        let mut g = g.borrow_mut();
        if g.is_none() {
            *g = Some(Gpu::new()?);
        }
        f(g.as_mut().expect("made above"))
    })
}

fn call(r: &Value, bytes: Vec<u8>) -> Result<(Vec<u8>, Value), String> {
    let f = r["fn"].as_str().ok_or("`fn` is missing")?;
    match f {
        "info" => {
            let v = fractal_flame_wgpu::version::get_version_info();
            Ok(data(json!({
                "version": v.version, "git_hash": v.git_hash,
                "engines": ["flame", "escape", "simulation", "terrain"],
            })))
        }
        "default_config" => Ok(data(json!(FractalConfig::default().to_json().map_err(|e| e.to_string())?))),
        // For the Python `Config`'s properties: what a field a `.fflame`
        // leaves out READS as -- not the struct's default, which can
        // differ (an absent `preserve_z` is true, for flames older than
        // the field; `FractalConfig::default()` has it false). So: the
        // default config without those fields, read back. And a new
        // transform as the app makes it, and a transform's absent fields.
        "defaults" => {
            use fractal_flame_wgpu::scene::transforms::Transform;
            const FIELDS: [&str; 10] = [
                "render_mode", "zoom", "pan_x", "pan_y", "rotation", "camera_rotation_x",
                "camera_rotation_y", "perspective_strength", "preserve_z", "max_iterations",
            ];
            let mut v: Value = serde_json::from_str(&FractalConfig::default().to_json().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            if let Some(o) = v.as_object_mut() {
                for k in FIELDS {
                    o.remove(k);
                }
            }
            let d = FractalConfig::from_json(&v.to_string()).map_err(|e| e.to_string())?;
            let absent = serde_json::from_value::<Transform>(json!({})).unwrap_or_default();
            Ok(data(json!({
                "config": {
                    "render_mode": d.render_mode, "zoom": d.zoom, "pan_x": d.pan_x, "pan_y": d.pan_y,
                    "rotation": d.rotation, "camera_rotation_x": d.camera_rotation_x,
                    "camera_rotation_y": d.camera_rotation_y, "perspective_strength": d.perspective_strength,
                    "preserve_z": d.preserve_z, "max_iterations": d.max_iterations,
                },
                "new_transform": serde_json::to_value(Transform::default()).map_err(|e| e.to_string())?,
                "transform": serde_json::to_value(absent).map_err(|e| e.to_string())?,
            })))
        }
        // The config as the app writes it: parsed (so a broken one is
        // refused) and serialized again.
        "config_normalize" => Ok(data(json!(config_arg(r, "config")?.to_json().map_err(|e| e.to_string())?))),
        // A setting by name (`Config.set`): the names and rules a script's
        // `config.set` has (`config::by_name`, through the app's one
        // setter). The config back as the app writes it, and whether the
        // write did anything a name the config knows would.
        "config_set" => {
            use fractal_flame_wgpu::config::by_name;
            let mut config = config_arg(r, "config")?;
            let applied = by_name::set(&mut config, str_arg(r, "key")?, r["value"].clone())?;
            Ok(data(json!({
                "config": config.to_json().map_err(|e| e.to_string())?,
                "changed": applied == by_name::Applied::Changed,
            })))
        }
        // Every variation in registration order, with its parameters' names.
        "variations" => {
            let registry = fractal_flame_wgpu::variations::global_registry();
            let list: Vec<Value> = registry
                .all()
                .iter()
                .map(|v| json!({ "name": v.name, "params": v.parameters.iter().map(|p| p.name.to_string()).collect::<Vec<_>>() }))
                .collect();
            Ok(data(Value::Array(list)))
        }
        "presets" => {
            let presets = fractal_flame_wgpu::resources::presets::load_embedded_presets().map_err(|e| e.to_string())?;
            let list: Result<Vec<Value>, String> = presets
                .iter()
                .map(|c| Ok(json!({ "name": c.flame.name, "config": c.to_json().map_err(|e| e.to_string())? })))
                .collect();
            Ok(data(Value::Array(list?)))
        }
        "flame_import" => {
            let configs = fractal_flame_wgpu::flame_xml::parse_flame_xml(str_arg(r, "xml")?).map_err(|e| e.to_string())?;
            let list: Result<Vec<Value>, String> = configs.iter().map(|c| c.to_json().map(Value::String).map_err(|e| e.to_string())).collect();
            Ok(data(Value::Array(list?)))
        }
        "flame_export" => Ok(data(json!(fractal_flame_wgpu::flame_xml::write_flame_xml(&config_arg(r, "config")?)))),
        "render" => {
            let config = config_arg(r, "config")?;
            let (w, h) = (u32_arg(r, "width")?, u32_arg(r, "height")?);
            let sequence = r["sequence"].as_bool().unwrap_or(false);
            let frame = with_gpu(|g| g.render(config, w, h, &Options::from_json(r), sequence))?;
            frame_output(frame)
        }
        "encode_png" => {
            let config = config_arg(r, "config")?;
            let n = |k: &str| r[k].as_f64().unwrap_or(0.0);
            let png = render::png(bytes, u32_arg(r, "width")?, u32_arg(r, "height")?, n("iterations") as u64, n("ms"), n("iterations_per_thread") as u32, &config)?;
            Ok((png, Value::Null))
        }
        "animation_open" => {
            let (w, h) = (u32_arg(r, "width")?, u32_arg(r, "height")?);
            let fps = r["fps"].as_u64().unwrap_or(30) as u32;
            let (_, info) = with_gpu(|g| g.open(str_arg(r, "animation")?, r["config"].as_str(), w, h, fps, Options::from_json(r)))?;
            Ok(data(info))
        }
        "animation_frame" => {
            let frame = with_gpu(|g| g.frame(u32_arg(r, "id")?, u32_arg(r, "frame")?))?;
            frame_output(frame)
        }
        "animation_config" => {
            let config = with_gpu(|g| g.frame_config(u32_arg(r, "id")?, u32_arg(r, "frame")?))?;
            Ok(data(json!(config.to_json().map_err(|e| e.to_string())?)))
        }
        "animation_close" => {
            with_gpu(|g| {
                g.close(u32_arg(r, "id")?);
                Ok(())
            })?;
            Ok(data(Value::Null))
        }
        "scripts" => Ok(data(scripts::list()?)),
        "script_source" => Ok(data(scripts::source(str_arg(r, "id")?)?)),
        "script_params" => Ok(data(scripts::params(str_arg(r, "source")?)?)),
        "script_run" => {
            let stages: Vec<(String, Value)> = r["stages"]
                .as_array()
                .ok_or("`stages` ([{source, params?}]) is missing")?
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let source = s["source"].as_str().ok_or_else(|| format!("stage {i}: `source` is missing"))?;
                    Ok((source.to_string(), s["params"].clone()))
                })
                .collect::<Result<_, String>>()?;
            Ok(data(scripts::run(&stages, r["seed"].as_u64().unwrap_or(0), r["base"].as_str())?))
        }
        other => Err(format!("unknown call `{other}`")),
    }
}
