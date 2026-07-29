//! The scripting object model: what a script is allowed to touch.
//!
//! Everything a script can do is registered here. Nothing else exists
//! inside the sandbox — no file, network, or process access.
//!
//! Two layers:
//!
//! * **Typed handles** (`flame`, transforms) for structure — the hot,
//!   frequently-scripted operations, with real validation.
//! * **`config.set(key, value)`** for the long tail of scalar settings,
//!   backed by serde. The keys are exactly the `.fflame` JSON keys, so
//!   a user can read a saved file to discover what's settable.

use std::cell::RefCell;
use std::rc::Rc;

use rand::Rng;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Position, Scope};

use crate::config::fractal_config::FractalConfig;
use crate::scene::transforms::Transform;

use super::host::ScriptState;
use super::color::ScriptColor;
use super::{humanize, ParamDecl, ParamValue, ScriptFlags, ScriptKind};
use crate::scene::palette::ColorStop;

/// Accept a whole number wherever a decimal is expected.
///
/// Rhai does not coerce `1` to `1.0` for registered functions, so
/// without this `t.weight = 1` fails with "function not found" — a wall
/// for exactly the audience this feature is for. Every numeric entry
/// point below takes `Dynamic` and comes through here.
fn num(d: &Dynamic, what: &str) -> Result<f64, Box<EvalAltResult>> {
    if let Ok(f) = d.as_float() {
        return Ok(f);
    }
    if let Ok(i) = d.as_int() {
        return Ok(i as f64);
    }
    Err(err(format!("{what} expects a number, got a {}", d.type_name())))
}

/// Build a script-visible runtime error.
fn err(msg: impl Into<String>) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from(msg.into()),
        Position::NONE,
    ))
}

// ============================================================================
// Handles
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pool {
    Normal,
    Linked,
    Final,
}

impl Pool {
    fn name(self) -> &'static str {
        match self {
            Pool::Normal => "normal",
            Pool::Linked => "linked",
            Pool::Final => "final",
        }
    }
}

#[derive(Clone)]
pub struct FlameHandle {
    cfg: Rc<RefCell<FractalConfig>>,
}

#[derive(Clone)]
pub struct ConfigHandle {
    cfg: Rc<RefCell<FractalConfig>>,
}

use crate::animation::EasingFunction;

/// The optional animation a script may define alongside its flame.
///
/// Holds the script state rather than a config: keyframes describe how a
/// parameter moves over time, which is not part of the flame itself.
#[derive(Clone)]
pub struct AnimHandle {
    state: Rc<RefCell<ScriptState>>,
}

/// Points at a transform by pool + index rather than borrowing it, so a
/// handle stays valid across other script operations. Every access is
/// bounds-checked: a handle to a removed transform errors with a
/// message instead of panicking or silently writing elsewhere.
#[derive(Clone)]
pub struct TransformHandle {
    cfg: Rc<RefCell<FractalConfig>>,
    pool: Pool,
    idx: usize,
}

impl TransformHandle {
    /// The pool-aware reference `ConfigPath` construction needs.
    fn xref(&self) -> crate::config::TransformRef {
        match self.pool {
            Pool::Normal => crate::config::TransformRef::Normal(self.idx),
            Pool::Linked => crate::config::TransformRef::Linked(self.idx),
            Pool::Final => crate::config::TransformRef::Final(self.idx),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut Transform) -> R) -> Result<R, Box<EvalAltResult>> {
        let mut cfg = self.cfg.borrow_mut();
        let list = match self.pool {
            Pool::Normal => &mut cfg.flame.transforms,
            Pool::Linked => &mut cfg.flame.linked_transforms,
            Pool::Final => &mut cfg.flame.final_transforms,
        };
        match list.get_mut(self.idx) {
            Some(t) => Ok(f(t)),
            None => Err(err(format!(
                "transform {} no longer exists in the {} pool",
                self.idx,
                self.pool.name()
            ))),
        }
    }
}


/// Backs both arities of `script(...)`.
fn declare_script(
    state: &Rc<RefCell<ScriptState>>,
    name: &str,
    kind: &str,
    flags: Option<Array>,
) -> Result<(), Box<EvalAltResult>> {
    let mut st = state.borrow_mut();
    if st.meta.kind.is_some() {
        return Err(err("script(...) called more than once"));
    }
    if !st.declared.is_empty() {
        return Err(err("script(...) must come before any param(...) declaration"));
    }
    let kind = ScriptKind::parse(kind).ok_or_else(|| {
        err(format!(
            "unknown script kind `{kind}` — expected \"generator\" or \"modifier\""
        ))
    })?;

    let mut parsed = ScriptFlags::default();
    for flag in flags.unwrap_or_default() {
        let text = flag
            .into_string()
            .map_err(|_| err("script flags must be strings, e.g. [\"norng\"]"))?;
        parsed.set(&text).map_err(err)?;
    }

    st.meta.name = name.to_string();
    st.meta.kind = Some(kind);
    st.meta.flags = parsed;
    Ok(())
}

// ============================================================================
// Animation
// ============================================================================

/// `anim` — declare how the flame's parameters move over time.
///
/// Entirely optional: a script that never mentions `anim` produces no
/// animation, and every existing script is unaffected.
///
/// ```ignore
/// anim.name = "Spin";
/// anim.duration = 10.0;                  // optional; defaults to the last key
/// anim.key("rotation", 0.0, 0.0);        // same names config.set takes
/// anim.key("rotation", 10.0, 360.0);
/// anim.smooth("rotation");               // interpolation for the whole track
///
/// t.key("weight", 0.0, 0.2);             // per-transform, on the handle
/// t.key("julian.power", 0.0, 2.0);       // variation parameters too
/// ```
fn register_anim(engine: &mut Engine, state: Rc<RefCell<ScriptState>>) {
    engine.register_get("name", |a: &mut AnimHandle| {
        a.state.borrow().anim.name.clone().unwrap_or_default()
    });
    engine.register_set("name", |a: &mut AnimHandle, value: &str| {
        let mut s = a.state.borrow_mut();
        s.anim.touched = true;
        s.anim.name = Some(value.to_string());
    });

    engine.register_get("duration", |a: &mut AnimHandle| {
        a.state.borrow().anim.duration.unwrap_or(0.0)
    });
    engine.register_set("duration", |a: &mut AnimHandle, value: Dynamic| {
        // Whole numbers are the common case here ("10 seconds"), and Rhai
        // hands those over as INT.
        let seconds = num(&value, "anim.duration").unwrap_or(0.0);
        let mut s = a.state.borrow_mut();
        s.anim.touched = true;
        s.anim.duration = Some(seconds);
    });

    engine.register_fn(
        "key",
        |a: &mut AnimHandle, target: &str, time: Dynamic, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            anim_key(a, target, time, value, EasingFunction::Linear)
        },
    );
    engine.register_fn(
        "key",
        |a: &mut AnimHandle,
         target: &str,
         time: Dynamic,
         value: Dynamic,
         easing: &str|
         -> Result<(), Box<EvalAltResult>> {
            let easing = super::anim::parse_easing(easing).map_err(err)?;
            anim_key(a, target, time, value, easing)
        },
    );

    engine.register_fn(
        "interpolation",
        |a: &mut AnimHandle, target: &str, mode: &str| -> Result<(), Box<EvalAltResult>> {
            let mode = super::anim::parse_interpolation(mode).map_err(err)?;
            let target = super::anim::resolve_flame_target(target).map_err(err)?;
            a.state.borrow_mut().anim.set_interpolation(target, mode);
            Ok(())
        },
    );

    // Per-transform keyframes live on the transform handle, which already
    // knows its pool and index — the same handle the script used to build
    // the transform in the first place.
    let st = Rc::clone(&state);
    engine.register_fn(
        "key",
        move |t: &mut TransformHandle, target: &str, time: Dynamic, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            transform_key(t, &st, target, time, value, EasingFunction::Linear)
        },
    );
    let st = Rc::clone(&state);
    engine.register_fn(
        "key",
        move |t: &mut TransformHandle,
              target: &str,
              time: Dynamic,
              value: Dynamic,
              easing: &str|
              -> Result<(), Box<EvalAltResult>> {
            let easing = super::anim::parse_easing(easing).map_err(err)?;
            transform_key(t, &st, target, time, value, easing)
        },
    );
}

fn anim_key(
    a: &mut AnimHandle,
    target: &str,
    time: Dynamic,
    value: Dynamic,
    easing: EasingFunction,
) -> Result<(), Box<EvalAltResult>> {
    let time = num(&time, "keyframe time")?;
    let target = super::anim::resolve_flame_target(target).map_err(err)?;
    let value = dynamic_to_json(&value)?;
    a.state.borrow_mut().anim.add_key(target, time, value, easing);
    Ok(())
}

fn transform_key(
    t: &mut TransformHandle,
    state: &Rc<RefCell<ScriptState>>,
    target: &str,
    time: Dynamic,
    value: Dynamic,
    easing: EasingFunction,
) -> Result<(), Box<EvalAltResult>> {
    let time = num(&time, "keyframe time")?;
    // Bounds-check the handle first: keyframing a transform that has been
    // removed should report that, not silently animate whatever index the
    // handle used to point at.
    t.with(|_| ())?;
    let target = super::anim::resolve_transform_target(t.xref(), target).map_err(err)?;
    let value = dynamic_to_json(&value)?;
    state.borrow_mut().anim.add_key(target, time, value, easing);
    Ok(())
}

// ============================================================================
// Registration
// ============================================================================

/// Put the top-level objects in scope.
///
/// Pushed as ordinary variables, not constants: Rhai refuses property and
/// indexer assignment on a constant, so `config["gamma"] = 2.2` and
/// `flame.name = "x"` would fail. Rebinding them only breaks the script's
/// own run.
pub(crate) fn push_globals(
    scope: &mut Scope,
    cfg: Rc<RefCell<FractalConfig>>,
    state: Rc<RefCell<ScriptState>>,
) {
    scope.push("flame", FlameHandle { cfg: Rc::clone(&cfg) });
    scope.push("config", ConfigHandle { cfg });
    scope.push("anim", AnimHandle { state });
}

pub(crate) fn register(
    engine: &mut Engine,
    cfg: Rc<RefCell<FractalConfig>>,
    state: Rc<RefCell<ScriptState>>,
) {
    engine.register_type_with_name::<FlameHandle>("Flame");
    engine.register_type_with_name::<TransformHandle>("Transform");
    engine.register_type_with_name::<ConfigHandle>("Config");
    engine.register_type_with_name::<AnimHandle>("Anim");
    register_anim(engine, Rc::clone(&state));

    register_meta(engine, Rc::clone(&state));
    register_rng(engine, Rc::clone(&state));
    register_flame(engine);
    register_transform(engine);
    register_config(engine, Rc::clone(&state));
    register_run_script(engine, Rc::clone(&cfg), Rc::clone(&state));
    register_colors(engine, Rc::clone(&state));
    register_palette_slots(engine);
    register_palettes(engine, Rc::clone(&state));
    register_registry_queries(engine);
    register_builtins(engine);

    // print()/debug() are a beginner's main debugging tool, and stdout is
    // invisible in-app and on the web — capture them for the caller.
    let msg_state = Rc::clone(&state);
    engine.on_print(move |s| msg_state.borrow_mut().messages.push(s.to_string()));
    let dbg_state = Rc::clone(&state);
    engine.on_debug(move |s, _, _| dbg_state.borrow_mut().messages.push(s.to_string()));

    let _ = cfg;
}

// ---------------------------------------------------------------- metadata

fn register_meta(engine: &mut Engine, state: Rc<RefCell<ScriptState>>) {
    let s = Rc::clone(&state);
    // Two arities, because the flag list is optional and most scripts
    // want nothing to do with it: `script(name, kind)` stays exactly as
    // it was, `script(name, kind, ["norng"])` adds switches.
    engine.register_fn("script", {
        let s = Rc::clone(&s);
        move |name: &str, kind: &str| -> Result<(), Box<EvalAltResult>> {
            declare_script(&s, name, kind, None)
        }
    });
    engine.register_fn(
        "script",
        move |name: &str, kind: &str, flags: Array| -> Result<(), Box<EvalAltResult>> {
            declare_script(&s, name, kind, Some(flags))
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn(
        "param",
        move |key: &str, default: Dynamic, min: Dynamic, max: Dynamic| -> Result<f64, Box<EvalAltResult>> {
            let default = num(&default, "param default")?;
            let (min, max) = (num(&min, "param minimum")?, num(&max, "param maximum")?);
            if min > max {
                return Err(err(format!("param `{key}`: min ({min}) is above max ({max})")));
            }
            let decl = ParamDecl::Float {
                key: key.to_string(),
                label: humanize(key),
                default,
                min,
                max,
            };
            match s.borrow_mut().declare(decl).map_err(err)? {
                ParamValue::Float(v) => Ok(v.clamp(min, max)),
                other => Err(err(format!("param `{key}`: expected a number, got {other:?}"))),
            }
        },
    );

    // A colour parameter, rendered with the same picker the rest of the
    // app uses. Declared with a hex default so the script reads as the
    // colour it means.
    let s = Rc::clone(&state);
    engine.register_fn(
        "param_color",
        move |key: &str, default: &str| -> Result<ScriptColor, Box<EvalAltResult>> {
            let fallback = ScriptColor::from_hex(default).map_err(err)?;
            let decl = ParamDecl::Color {
                key: key.to_string(),
                label: humanize(key),
                default: fallback.to_rgb(),
            };
            match s.borrow_mut().declare(decl).map_err(err)? {
                ParamValue::Color(rgb) => Ok(ScriptColor::from_rgb(rgb)),
                other => Err(err(format!("param `{key}`: expected a colour, got {other:?}"))),
            }
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn(
        "param_int",
        move |key: &str, default: i64, min: i64, max: i64| -> Result<i64, Box<EvalAltResult>> {
            if min > max {
                return Err(err(format!("param `{key}`: min ({min}) is above max ({max})")));
            }
            let decl = ParamDecl::Int {
                key: key.to_string(),
                label: humanize(key),
                default,
                min,
                max,
            };
            match s.borrow_mut().declare(decl).map_err(err)? {
                ParamValue::Int(v) => Ok(v.clamp(min, max)),
                ParamValue::Float(v) => Ok((v as i64).clamp(min, max)),
                other => Err(err(format!("param `{key}`: expected a number, got {other:?}"))),
            }
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn(
        "param_bool",
        move |key: &str, default: bool| -> Result<bool, Box<EvalAltResult>> {
            let decl = ParamDecl::Bool {
                key: key.to_string(),
                label: humanize(key),
                default,
            };
            match s.borrow_mut().declare(decl).map_err(err)? {
                ParamValue::Bool(v) => Ok(v),
                other => Err(err(format!("param `{key}`: expected true/false, got {other:?}"))),
            }
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn(
        "param_string",
        move |key: &str, default: &str| -> Result<String, Box<EvalAltResult>> {
            const MAX_LEN: usize = 2048;
            let decl = ParamDecl::Text {
                key: key.to_string(),
                label: humanize(key),
                default: default.to_string(),
                max_len: MAX_LEN,
            };
            match s.borrow_mut().declare(decl).map_err(err)? {
                ParamValue::Text(v) => Ok(v.chars().take(MAX_LEN).collect()),
                other => Err(err(format!("param `{key}`: expected text, got {other:?}"))),
            }
        },
    );

    // Returns the chosen option as a string, so scripts branch readably:
    //     if param_choice("style", ["A", "B"], 0) == "A" { … }
    let s = Rc::clone(&state);
    engine.register_fn(
        "param_choice",
        move |key: &str, options: Array, default: i64| -> Result<String, Box<EvalAltResult>> {
            let opts: Vec<String> = options.iter().map(|d| d.to_string()).collect();
            if opts.is_empty() {
                return Err(err(format!("param `{key}`: needs at least one option")));
            }
            let default = default.clamp(0, opts.len() as i64 - 1) as usize;
            let decl = ParamDecl::Choice {
                key: key.to_string(),
                label: humanize(key),
                options: opts.clone(),
                default,
            };
            let idx = match s.borrow_mut().declare(decl).map_err(err)? {
                ParamValue::Choice(i) => i,
                ParamValue::Int(i) => i.clamp(0, opts.len() as i64 - 1) as usize,
                other => {
                    return Err(err(format!("param `{key}`: expected a choice, got {other:?}")))
                }
            };
            Ok(opts.get(idx).cloned().unwrap_or_else(|| opts[0].clone()))
        },
    );
}

// --------------------------------------------------------------------- rng

fn register_rng(engine: &mut Engine, state: Rc<RefCell<ScriptState>>) {
    let s = Rc::clone(&state);
    engine.register_fn("rand", move || -> f64 { s.borrow_mut().rng.gen::<f64>() });

    let s = Rc::clone(&state);
    engine.register_fn(
        "rand",
        move |min: Dynamic, max: Dynamic| -> Result<f64, Box<EvalAltResult>> {
            let (min, max) = (num(&min, "rand minimum")?, num(&max, "rand maximum")?);
            if min >= max {
                return Ok(min);
            }
            Ok(s.borrow_mut().rng.gen_range(min..max))
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn("rand_int", move |min: i64, max: i64| -> i64 {
        if min >= max {
            return min;
        }
        s.borrow_mut().rng.gen_range(min..=max)
    });

    let s = Rc::clone(&state);
    engine.register_fn("chance", move |p: Dynamic| -> Result<bool, Box<EvalAltResult>> {
        let p = num(&p, "chance")?;
        Ok(s.borrow_mut().rng.gen::<f64>() < p)
    });

    let s = Rc::clone(&state);
    engine.register_fn("pick", move |items: Array| -> Result<Dynamic, Box<EvalAltResult>> {
        if items.is_empty() {
            return Err(err("pick() needs a non-empty array"));
        }
        // u64, never usize: usize is 32-bit on wasm32 and 64-bit on
        // desktop, and rand picks a different integer implementation for
        // each — same seed, different draw. Fixed width keeps the stream
        // identical everywhere, which the whole "script + seed" promise
        // rests on.
        let i = s.borrow_mut().rng.gen_range(0..items.len() as u64) as usize;
        Ok(items[i].clone())
    });

    let s = Rc::clone(&state);
    engine.register_fn("shuffle", move |items: Array| -> Array {
        let mut out = items;
        let mut st = s.borrow_mut();
        // Fisher–Yates against the seeded stream, so shuffles reproduce.
        for i in (1..out.len()).rev() {
            // u64 for the same reason as pick().
            let j = st.rng.gen_range(0..=i as u64) as usize;
            out.swap(i, j);
        }
        out
    });
}

// ------------------------------------------------------------------- flame

fn register_flame(engine: &mut Engine) {
    engine.register_fn("add_transform", |f: &mut FlameHandle| -> TransformHandle {
        let mut cfg = f.cfg.borrow_mut();
        cfg.flame.transforms.push(Transform::new());
        TransformHandle {
            cfg: Rc::clone(&f.cfg),
            pool: Pool::Normal,
            idx: cfg.flame.transforms.len() - 1,
        }
    });

    engine.register_fn("add_final_transform", |f: &mut FlameHandle| -> TransformHandle {
        let mut cfg = f.cfg.borrow_mut();
        cfg.flame.final_transforms.push(Transform::new());
        TransformHandle {
            cfg: Rc::clone(&f.cfg),
            pool: Pool::Final,
            idx: cfg.flame.final_transforms.len() - 1,
        }
    });

    engine.register_fn("add_linked_transform", |f: &mut FlameHandle| -> TransformHandle {
        let mut cfg = f.cfg.borrow_mut();
        cfg.flame.linked_transforms.push(Transform::new());
        TransformHandle {
            cfg: Rc::clone(&f.cfg),
            pool: Pool::Linked,
            idx: cfg.flame.linked_transforms.len() - 1,
        }
    });

    engine.register_fn("transform_count", |f: &mut FlameHandle| -> i64 {
        f.cfg.borrow().flame.transforms.len() as i64
    });

    engine.register_fn("final_count", |f: &mut FlameHandle| -> i64 {
        f.cfg.borrow().flame.final_transforms.len() as i64
    });

    engine.register_fn(
        "transform",
        |f: &mut FlameHandle, i: i64| -> Result<TransformHandle, Box<EvalAltResult>> {
            let len = f.cfg.borrow().flame.transforms.len();
            let idx = usize::try_from(i).map_err(|_| err("transform index must be >= 0"))?;
            if idx >= len {
                return Err(err(format!("no transform {idx} (flame has {len})")));
            }
            Ok(TransformHandle { cfg: Rc::clone(&f.cfg), pool: Pool::Normal, idx })
        },
    );

    engine.register_fn(
        "final_transform",
        |f: &mut FlameHandle, i: i64| -> Result<TransformHandle, Box<EvalAltResult>> {
            let len = f.cfg.borrow().flame.final_transforms.len();
            let idx = usize::try_from(i).map_err(|_| err("transform index must be >= 0"))?;
            if idx >= len {
                return Err(err(format!("no final transform {idx} (flame has {len})")));
            }
            Ok(TransformHandle { cfg: Rc::clone(&f.cfg), pool: Pool::Final, idx })
        },
    );

    // Xaos: how likely each transform is to follow each other one.
    // xaos[from][to]; 1.0 is neutral, 0.0 forbids the transition.
    engine.register_fn(
        "set_xaos",
        |f: &mut FlameHandle, from: i64, to: i64, weight: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let weight = num(&weight, "xaos weight")? as f32;
            let mut cfg = f.cfg.borrow_mut();
            let n = cfg.flame.transforms.len();
            let (from, to) = (
                usize::try_from(from).map_err(|_| err("xaos index must be >= 0"))?,
                usize::try_from(to).map_err(|_| err("xaos index must be >= 0"))?,
            );
            if from >= n || to >= n {
                return Err(err(format!(
                    "xaos index out of range (flame has {n} transforms)"
                )));
            }
            let table = cfg.flame.xaos.get_or_insert_with(|| vec![vec![1.0; n]; n]);
            // Grow if transforms were added after the table was created.
            if table.len() != n || table.iter().any(|r| r.len() != n) {
                let mut grown = vec![vec![1.0f32; n]; n];
                for (i, row) in table.iter().enumerate().take(n) {
                    for (j, v) in row.iter().enumerate().take(n) {
                        grown[i][j] = *v;
                    }
                }
                *table = grown;
            }
            table[from][to] = weight;
            Ok(())
        },
    );

    engine.register_fn("clear_xaos", |f: &mut FlameHandle| {
        f.cfg.borrow_mut().flame.xaos = None;
    });

    engine.register_fn("clear_transforms", |f: &mut FlameHandle| {
        f.cfg.borrow_mut().flame.transforms.clear();
    });

    engine.register_fn(
        "remove_transform",
        |f: &mut FlameHandle, i: i64| -> Result<(), Box<EvalAltResult>> {
            let mut cfg = f.cfg.borrow_mut();
            let len = cfg.flame.transforms.len();
            let idx = usize::try_from(i).map_err(|_| err("transform index must be >= 0"))?;
            if idx >= len {
                return Err(err(format!("no transform {idx} (flame has {len})")));
            }
            cfg.flame.transforms.remove(idx);
            Ok(())
        },
    );

    // Whether the system converges is a property of the WHOLE flame, not
    // of any one transform — see mean_log_scale.
    engine.register_fn("contractiveness", |f: &mut FlameHandle| -> f64 {
        mean_log_scale(&f.cfg.borrow().flame).unwrap_or(f64::NEG_INFINITY)
    });

    engine.register_fn(
        "set_contractiveness",
        |f: &mut FlameHandle, target: Dynamic| -> Result<f64, Box<EvalAltResult>> {
            let target = num(&target, "contractiveness target")?;
            let mut cfg = f.cfg.borrow_mut();
            let current = mean_log_scale(&cfg.flame).ok_or_else(|| {
                err("set_contractiveness needs at least one transform with weight")
            })?;
            if !current.is_finite() {
                return Err(err(
                    "cannot rescale: a transform's affine is degenerate (zero area)",
                ));
            }
            // Scaling every linear part by k shifts each transform's
            // log-scale by ln k, so it shifts the weighted mean by ln k
            // too — one uniform factor lands exactly on the target while
            // leaving the relative character of each transform intact.
            // Scaling the affines by k shifts each transform's log-scale by
            // ln k (the variation-weight term is untouched), so the
            // weighted mean lands exactly on the target.
            let k = (target - current).exp() as f32;
            for t in &mut cfg.flame.transforms {
                t.a *= k;
                t.b *= k;
                t.c *= k;
                t.d *= k;
            }
            Ok(k as f64)
        },
    );

    engine.register_get_set(
        "name",
        |f: &mut FlameHandle| -> String { f.cfg.borrow().flame.name.clone() },
        |f: &mut FlameHandle, v: String| f.cfg.borrow_mut().flame.name = v,
    );

    // Effects route to the density or color chain by their registered
    // category, so scripts don't have to know which pass an effect runs in.
    engine.register_fn(
        "add_effect",
        |f: &mut FlameHandle, name: &str| -> Result<(), Box<EvalAltResult>> {
            use crate::effects::{global_effect_registry, EffectCategory, EffectInstance};
            let info = global_effect_registry()
                .get(name)
                .ok_or_else(|| err(format!("unknown effect `{name}`")))?;
            let category = info.category;
            let instance = EffectInstance::new(name);
            let mut cfg = f.cfg.borrow_mut();
            match category {
                EffectCategory::Density => cfg.density_effects.push(instance),
                EffectCategory::Color => cfg.color_effects.push(instance),
            }
            Ok(())
        },
    );

    engine.register_fn(
        "set_effect_param",
        |f: &mut FlameHandle, name: &str, param: &str, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let value = num(&value, "effect parameter")?;
            let mut cfg = f.cfg.borrow_mut();
            // Most-recently added instance of that effect, color chain
            // first (the borrow checker wants these sequential, not chained).
            let mut applied = false;
            if let Some(e) = cfg.color_effects.iter_mut().rev().find(|e| e.effect_type == name) {
                e.set_param(param, value as f32);
                applied = true;
            }
            if !applied {
                if let Some(e) =
                    cfg.density_effects.iter_mut().rev().find(|e| e.effect_type == name)
                {
                    e.set_param(param, value as f32);
                    applied = true;
                }
            }
            if applied {
                Ok(())
            } else {
                Err(err(format!("no `{name}` effect has been added yet")))
            }
        },
    );
}

// --------------------------------------------------------------- transform

fn register_transform(engine: &mut Engine) {
    macro_rules! prop_f32 {
        ($name:literal, $field:ident) => {
            // Registered separately, not via register_get_set: that ties
            // the setter's type to the getter's, and the setter has to
            // take Dynamic so `t.weight = 1` works alongside `= 1.0`.
            engine.register_get($name, |t: &mut TransformHandle| -> Result<f64, Box<EvalAltResult>> {
                t.with(|x| x.$field as f64)
            });
            engine.register_set(
                $name,
                |t: &mut TransformHandle, v: Dynamic| -> Result<(), Box<EvalAltResult>> {
                    let v = num(&v, $name)?;
                    t.with(|x| x.$field = v as f32)
                },
            );
        };
    }

    prop_f32!("weight", weight);
    prop_f32!("color", color);
    prop_f32!("color_speed", color_speed);
    prop_f32!("opacity", opacity);
    prop_f32!("direct_color", direct_color);
    // Affine coefficients: x' = a·x + b·y + e, y' = c·x + d·y + f
    prop_f32!("a", a);
    prop_f32!("b", b);
    prop_f32!("c", c);
    prop_f32!("d", d);
    prop_f32!("e", e);
    prop_f32!("f", f);
    prop_f32!("g", g);

    engine.register_fn(
        "add_variation",
        |t: &mut TransformHandle, name: &str, weight: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let weight = num(&weight, "variation weight")?;
            if crate::variations::global_registry().get(name).is_none() {
                return Err(err(format!("unknown variation `{name}`")));
            }
            t.with(|x| x.set_variation(name, weight as f32))
        },
    );

    engine.register_fn(
        "remove_variation",
        |t: &mut TransformHandle, name: &str| -> Result<(), Box<EvalAltResult>> {
            t.with(|x| {
                x.remove_variation(name);
            })
        },
    );

    // Params use the app's own "variation.param" key form, so what a
    // script writes matches what the .fflame file shows.
    engine.register_fn(
        "set_variation_param",
        |t: &mut TransformHandle, key: &str, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let value = num(&value, "parameter value")?;
            let (var, param) = key
                .split_once('.')
                .ok_or_else(|| err(format!("param key `{key}` must look like \"variation.param\"")))?;
            validate_variation_param(var, param)?;
            t.with(|x| {
                x.variation_params.insert(key.to_string(), value as f32);
            })
        },
    );

    engine.register_fn(
        "set_variation_param",
        |t: &mut TransformHandle, var: &str, param: &str, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let value = num(&value, "parameter value")?;
            validate_variation_param(var, param)?;
            t.with(|x| {
                x.variation_params
                    .insert(format!("{var}.{param}"), value as f32);
            })
        },
    );

    engine.register_fn(
        "translate",
        |t: &mut TransformHandle, dx: Dynamic, dy: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let (dx, dy) = (num(&dx, "translate x")?, num(&dy, "translate y")?);
            t.with(|x| {
                x.e += dx as f32;
                x.f += dy as f32;
            })
        },
    );

    // Scales the linear part only — the transform's placement (e, f) is
    // left alone, which is what "scale the triangle" means in Apophysis.
    engine.register_fn(
        "scale",
        |t: &mut TransformHandle, s: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let s = num(&s, "scale")? as f32;
            t.with(|x| {
                x.a *= s;
                x.b *= s;
                x.c *= s;
                x.d *= s;
            })
        },
    );

    engine.register_fn(
        "scale_xy",
        |t: &mut TransformHandle, sx: Dynamic, sy: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let (sx, sy) = (num(&sx, "scale x")? as f32, num(&sy, "scale y")? as f32);
            t.with(|x| {
                x.a *= sx;
                x.b *= sx;
                x.c *= sy;
                x.d *= sy;
            })
        },
    );

    engine.register_fn(
        "rotate",
        |t: &mut TransformHandle, degrees: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let (s, c) = (num(&degrees, "rotate")? as f32).to_radians().sin_cos();
            t.with(|x| {
                let (a, b, cc, d) = (x.a, x.b, x.c, x.d);
                x.a = c * a - s * cc;
                x.b = c * b - s * d;
                x.c = s * a + c * cc;
                x.d = s * b + c * d;
            })
        },
    );

    engine.register_fn(
        "set_affine",
        |t: &mut TransformHandle,
         a: Dynamic,
         b: Dynamic,
         c: Dynamic,
         d: Dynamic,
         e: Dynamic,
         f: Dynamic|
         -> Result<(), Box<EvalAltResult>> {
            let (a, b) = (num(&a, "affine a")?, num(&b, "affine b")?);
            let (c, d) = (num(&c, "affine c")?, num(&d, "affine d")?);
            let (e, f) = (num(&e, "affine e")?, num(&f, "affine f")?);
            t.with(|x| {
                x.a = a as f32;
                x.b = b as f32;
                x.c = c as f32;
                x.d = d as f32;
                x.e = e as f32;
                x.f = f as f32;
            })
        },
    );

    engine.register_fn("index", |t: &mut TransformHandle| -> i64 { t.idx as i64 });

    // Reading what's already on a transform is what separates a MODIFIER
    // from an overwriter: a mutation has to nudge what it finds.
    engine.register_fn(
        "variation_names",
        |t: &mut TransformHandle| -> Result<Array, Box<EvalAltResult>> {
            let registry = crate::variations::global_registry();
            t.with(|x| {
                x.ordered_variation_names(&registry)
                    .into_iter()
                    .map(Dynamic::from)
                    .collect()
            })
        },
    );

    engine.register_fn(
        "variation_weight",
        |t: &mut TransformHandle, name: &str| -> Result<f64, Box<EvalAltResult>> {
            t.with(|x| x.variations.get(name).copied().unwrap_or(0.0) as f64)
        },
    );

    engine.register_fn(
        "has_variation",
        |t: &mut TransformHandle, name: &str| -> Result<bool, Box<EvalAltResult>> {
            t.with(|x| x.variations.contains_key(name))
        },
    );

    engine.register_fn(
        "variation_param",
        |t: &mut TransformHandle, key: &str| -> Result<f64, Box<EvalAltResult>> {
            let (var, param) = key
                .split_once('.')
                .ok_or_else(|| err(format!("param key `{key}` must look like \"variation.param\"")))?;
            validate_variation_param(var, param)?;
            // Falls back to the variation's declared default, so a script
            // reading an untouched parameter sees what the renderer uses.
            t.with(|x| x.variation_param_or_default(var, param) as f64)
        },
    );

    // |det| of the linear part: how much this transform scales AREA.
    // Above 1 expands, below 1 contracts — legitimate either way.
    engine.register_fn("area_scale", |t: &mut TransformHandle| -> Result<f64, Box<EvalAltResult>> {
        t.with(|x| ((x.a * x.d - x.b * x.c) as f64).abs())
    });
}

/// Probability-weighted mean log linear-scale of the normal transforms —
/// negative means the flame contracts on average, and the chaos game
/// settles onto a bounded attractor.
///
/// This is the quantity that decides whether a flame converges. It is NOT
/// a per-transform property: a transform scaling by 1.5 is fine, and
/// often desirable, as long as the others pull the weighted mean below
/// zero. Weights are the selection probabilities, so a rarely-chosen
/// expansive transform costs little.
///
/// Each transform contributes `0.5·ln|det A| + ln(Σ w_v)` — the log of
/// its average linear scale (|det| being the area factor, and the
/// post-affine folded in when enabled) plus the total weight of its
/// normal-phase variations.
///
/// That second term matters more than it looks. The dispatcher computes
/// `result = Σ_v w_v · f_v(A·p)`, so the variation weights scale the
/// transform's OUTPUT: a lone `linear` at weight 1.18 is an 18% expansion
/// no matter what the affine says. Ignoring it made
/// `set_contractiveness` silently wrong for any flame whose variation
/// weights weren't exactly 1 — it rebalanced the affines while the real
/// scale drifted, which showed up as a mutation rendering to haze.
///
/// Deliberately an estimate, not a guarantee:
///
/// * Only the affine and the weights are measured. A variation's own
///   scaling is invisible — `spherical` bounds any input however
///   expansive the affine feeding it, so the number is a guide, not a
///   verdict, once curved variations carry real weight.
/// * `det` averages the two axes, so a map that stretches along one axis
///   while squashing the other can read as neutral.
/// * Pre/post-phase variations are function composition rather than terms
///   in the weighted sum, so their weights are excluded here.
/// * Final transforms are excluded: they affect what is plotted, not the
///   trajectory that continues.
///
/// Returns `None` when nothing carries weight.
fn mean_log_scale(flame: &crate::scene::transforms::Flame) -> Option<f64> {
    // Guards against ln(0) on a degenerate (zero-area) affine.
    const FLOOR: f64 = 1e-12;
    let total: f64 = flame.transforms.iter().map(|t| t.weight.max(0.0) as f64).sum();
    if total <= 0.0 {
        return None;
    }
    let mut acc = 0.0;
    for t in &flame.transforms {
        let w = t.weight.max(0.0) as f64;
        if w <= 0.0 {
            continue;
        }
        let mut det = ((t.a * t.d - t.b * t.c) as f64).abs();
        if t.post_affine_enabled {
            det *= ((t.post_a * t.post_d - t.post_b * t.post_c) as f64).abs();
        }
        // Only variations in the weighted sum scale the output. Mirrors
        // the shader builder's own rule: a Normal variation always sums,
        // an `Any` variation sums unless this transform's fx_priority
        // moved it (<0 pre, >0 post), and Pre/Post compose instead.
        let registry = crate::variations::global_registry();
        // Summed in the flame's canonical variation order, NOT HashMap
        // order: float addition isn't associative, so iterating a HashMap
        // makes the last bits depend on the hasher's seed — which differs
        // between builds and platforms. That would leak into
        // set_contractiveness and desynchronise otherwise-identical runs.
        let var_scale: f64 = t
            .ordered_variation_names(&registry)
            .into_iter()
            .filter(|name| match registry.get(name).map(|i| i.phase.clone()) {
                Some(crate::variations::VariationPhase::Normal) => true,
                Some(crate::variations::VariationPhase::Any) => {
                    t.variation_priorities.get(name).copied().unwrap_or(0) == 0
                }
                Some(_) => false,
                None => true,
            })
            .map(|name| t.variations.get(&name).copied().unwrap_or(0.0) as f64)
            .sum::<f64>()
            .abs();
        // Bounded per transform: one degenerate transform (no summing
        // variation at all, so its normal result is the origin) would
        // otherwise drag the mean to ~-27 and make set_contractiveness
        // ask for an astronomical rescale.
        let contribution =
            (0.5 * det.max(FLOOR).ln() + var_scale.max(FLOOR).ln()).clamp(-10.0, 10.0);
        acc += (w / total) * contribution;
    }
    Some(acc)
}

fn validate_variation_param(var: &str, param: &str) -> Result<(), Box<EvalAltResult>> {
    let registry = crate::variations::global_registry();
    let info = registry
        .get(var)
        .ok_or_else(|| err(format!("unknown variation `{var}`")))?;
    if !info.parameters.iter().any(|p| p.name == param) {
        let known: Vec<String> = info.parameters.iter().map(|p| p.name.to_string()).collect();
        return Err(err(format!(
            "variation `{var}` has no parameter `{param}` (has: {})",
            if known.is_empty() { "none".to_string() } else { known.join(", ") }
        )));
    }
    Ok(())
}

// ------------------------------------------------------------------ config

fn register_config(engine: &mut Engine, state: Rc<RefCell<ScriptState>>) {
    let s = Rc::clone(&state);
    engine.register_fn(
        "set",
        move |c: &mut ConfigHandle, key: &str, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let mut cfg = c.cfg.borrow_mut();
            match set_config_field(&mut cfg, key, dynamic_to_json(&value)?) {
                Ok(Applied::Changed) => Ok(()),
                Ok(Applied::NoVisibleChange) => {
                    // Either a no-op (already that value / equals the
                    // default of a skip-if-default field) or a bad key.
                    // Can't tell them apart from the JSON alone, so warn
                    // rather than fail a legitimate no-op.
                    s.borrow_mut().warnings.push(format!(
                        "config.set(\"{key}\", …) changed nothing — check the setting name"
                    ));
                    Ok(())
                }
                Err(e) => Err(err(e)),
            }
        },
    );

    engine.register_fn(
        "get",
        |c: &mut ConfigHandle, key: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let cfg = c.cfg.borrow();
            let root = serde_json::to_value(&*cfg).map_err(|e| err(e.to_string()))?;
            match json_at(&root, key) {
                Some(v) => Ok(json_to_dynamic(v)),
                None => Err(err(format!(
                    "`{key}` is not set (it may be at its default and omitted, or misspelled)"
                ))),
            }
        },
    );

    // config["gamma"] = 2.4 reads better than config.set(...) for simple
    // assignments; same backing path.
    let s = Rc::clone(&state);
    engine.register_indexer_set(
        move |c: &mut ConfigHandle, key: &str, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let mut cfg = c.cfg.borrow_mut();
            match set_config_field(&mut cfg, key, dynamic_to_json(&value)?) {
                Ok(Applied::Changed) => Ok(()),
                Ok(Applied::NoVisibleChange) => {
                    s.borrow_mut().warnings.push(format!(
                        "config[\"{key}\"] = … changed nothing — check the setting name"
                    ));
                    Ok(())
                }
                Err(e) => Err(err(e)),
            }
        },
    );
}

enum Applied {
    Changed,
    NoVisibleChange,
}

/// Set any `FractalConfig` field by its `.fflame` JSON path.
///
/// Goes through serde rather than a hand-written table so the whole
/// config is reachable and the key names are the ones users already see
/// in saved files. The flame itself is excluded: it has richer structure
/// (and session-local IDs that a JSON round trip would reset), so it is
/// edited through the typed `flame` handle instead.
fn set_config_field(
    cfg: &mut FractalConfig,
    key: &str,
    value: serde_json::Value,
) -> Result<Applied, String> {
    if key == "flame" || key.starts_with("flame.") {
        return Err("use the `flame` object to edit the flame, not config.set".to_string());
    }

    let before = serde_json::to_value(&*cfg).map_err(|e| e.to_string())?;
    // A key already present in the JSON is a known setting, even if the
    // assignment turns out to be a no-op (setting brightness to the value
    // it already has). Only a key that is BOTH absent and produced no
    // change is ambiguous enough to warn about.
    let key_existed = json_at(&before, key).is_some();
    let mut patched = before.clone();

    // Walk to the parent, creating nothing: intermediate objects must
    // already exist (a missing one means a bad path, not a default).
    let parts: Vec<&str> = key.split('.').collect();
    let (last, parents) = parts.split_last().ok_or("empty setting name")?;
    let mut cur = &mut patched;
    for p in parents {
        cur = cur
            .get_mut(*p)
            .ok_or_else(|| format!("unknown setting group `{p}` in `{key}`"))?;
    }
    let obj = cur
        .as_object_mut()
        .ok_or_else(|| format!("`{key}` is not a settable field"))?;
    // Insert rather than require presence: fields sitting at their
    // default are omitted from the JSON entirely (skip-if-default).
    obj.insert((*last).to_string(), value);

    let flame = cfg.flame.clone();
    let mut next: FractalConfig = serde_json::from_value(patched)
        .map_err(|e| format!("invalid value for `{key}`: {e}"))?;
    // Session-local IDs are #[serde(skip)]; restore the flame wholesale
    // and re-issue IDs for anything the round trip blanked.
    next.flame = flame;
    next.fixup_ids();

    let after = serde_json::to_value(&next).map_err(|e| e.to_string())?;
    let changed = after != before;
    *cfg = next;
    Ok(if changed || key_existed {
        Applied::Changed
    } else {
        Applied::NoVisibleChange
    })
}

fn json_at<'a>(root: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut cur = root;
    for p in key.split('.') {
        cur = cur.get(p)?;
    }
    Some(cur)
}

fn dynamic_to_json(d: &Dynamic) -> Result<serde_json::Value, Box<EvalAltResult>> {
    use serde_json::Value;
    if d.is::<bool>() {
        return Ok(Value::Bool(d.as_bool().unwrap()));
    }
    if d.is::<i64>() {
        return Ok(Value::from(d.as_int().unwrap()));
    }
    if d.is::<f64>() {
        return Ok(Value::from(d.as_float().unwrap()));
    }
    if d.is::<String>() || d.is::<rhai::ImmutableString>() {
        return Ok(Value::String(d.clone().into_string().unwrap_or_default()));
    }
    if d.is::<Array>() {
        let arr = d.clone().into_array().unwrap_or_default();
        let mut out = Vec::with_capacity(arr.len());
        for item in &arr {
            out.push(dynamic_to_json(item)?);
        }
        return Ok(Value::Array(out));
    }
    Err(err(format!(
        "cannot use a {} as a config value",
        d.type_name()
    )))
}

fn json_to_dynamic(v: &serde_json::Value) -> Dynamic {
    use serde_json::Value;
    match v {
        Value::Null => Dynamic::UNIT,
        Value::Bool(b) => Dynamic::from(*b),
        Value::Number(n) => n
            .as_i64()
            .map(Dynamic::from)
            .or_else(|| n.as_f64().map(Dynamic::from))
            .unwrap_or(Dynamic::UNIT),
        Value::String(s) => Dynamic::from(s.clone()),
        Value::Array(a) => Dynamic::from(a.iter().map(json_to_dynamic).collect::<Array>()),
        Value::Object(_) => Dynamic::from(v.to_string()),
    }
}

// ---------------------------------------------------------------- palettes

/// `run_script(id)` / `run_script(id, #{ ... })` — run another script on
/// the same flame.
///
/// The callee works on the SAME config, so a palette script called from
/// a generator simply sets that flame's palette. There is no return
/// value: the shared config covers every case we have, and it keeps
/// both the model and the sandbox simple.
///
/// The callee continues the caller's RNG stream rather than re-using its
/// seed, so the whole run still reproduces from (script, seed) while two
/// calls give two different results.
fn register_run_script(
    engine: &mut Engine,
    cfg: Rc<RefCell<FractalConfig>>,
    state: Rc<RefCell<ScriptState>>,
) {
    let state2 = Rc::clone(&state);
    let c = Rc::clone(&cfg);
    let s = Rc::clone(&state);
    engine.register_fn("run_script", move |id: &str| -> Result<(), Box<EvalAltResult>> {
        call_script(&c, &s, id, rhai::Map::new(), None)
    });

    let c = Rc::clone(&cfg);
    let s = Rc::clone(&state);
    engine.register_fn(
        "run_script",
        move |id: &str, params: rhai::Map| -> Result<(), Box<EvalAltResult>> {
            call_script(&c, &s, id, params, None)
        },
    );

    // With a seed, the callee reproduces on its own rather than
    // continuing the caller's stream.
    engine.register_fn(
        "run_script",
        move |id: &str, params: rhai::Map, seed: i64| -> Result<(), Box<EvalAltResult>> {
            call_script(&cfg, &state, id, params, Some(seed as u64))
        },
    );

    let s = Rc::clone(&state2);
    engine.register_fn("seed", move || -> i64 { s.borrow().seed as i64 });
}

fn call_script(
    cfg: &Rc<RefCell<FractalConfig>>,
    state: &Rc<RefCell<ScriptState>>,
    id: &str,
    params: rhai::Map,
    seed: Option<u64>,
) -> Result<(), Box<EvalAltResult>> {
    // Collect mode only gathers the CALLER's parameters. Running the
    // callee would cost time and side effects for metadata that is not
    // wanted: its parameters belong to it, not to the caller's panel.
    if state.borrow().mode == super::host::Mode::Collect {
        return Ok(());
    }

    let mut supplied: std::collections::HashMap<String, ParamValue> = std::collections::HashMap::new();
    for (key, value) in params {
        supplied.insert(key.to_string(), dynamic_to_param(&value)?);
    }
    super::host::run_sub_script(cfg, state, id, supplied, seed).map_err(err)
}

/// Turn a value from a `run_script` parameter map into a ParamValue.
///
/// Bools are tested before integers because Rhai, like the rest of the
/// object model here, will happily read one as the other.
fn dynamic_to_param(value: &Dynamic) -> Result<ParamValue, Box<EvalAltResult>> {
    if let Ok(b) = value.as_bool() {
        return Ok(ParamValue::Bool(b));
    }
    if let Ok(i) = value.as_int() {
        return Ok(ParamValue::Int(i));
    }
    if let Ok(f) = value.as_float() {
        return Ok(ParamValue::Float(f));
    }
    if let Some(c) = value.clone().try_cast::<ScriptColor>() {
        return Ok(ParamValue::Color(c.to_rgb()));
    }
    if let Ok(text) = value.clone().into_string() {
        return Ok(ParamValue::Text(text));
    }
    Err(err("script parameters must be a number, true/false, a string or a color"))
}

/// `Color` — the value colour-generating scripts work in, plus the two
/// palette builders that make generating one possible at all.
///
/// Scripts could previously only SELECT a palette from the loaded
/// library; there was no way to construct one, which is what made
/// palette generation a Rust feature rather than a script.
fn register_colors(engine: &mut Engine, state: Rc<RefCell<ScriptState>>) {
    engine.register_type_with_name::<ScriptColor>("Color");

    engine.register_fn("color", |r: Dynamic, g: Dynamic, b: Dynamic| -> Result<ScriptColor, Box<EvalAltResult>> {
        Ok(ScriptColor::new(
            num(&r, "red")? as f32,
            num(&g, "green")? as f32,
            num(&b, "blue")? as f32,
        ))
    });
    // Hue in DEGREES, so the numbers match what colour theory talks
    // about (complementary = +180) and what the app's pickers show.
    engine.register_fn("color_hsv", |h: Dynamic, s: Dynamic, v: Dynamic| -> Result<ScriptColor, Box<EvalAltResult>> {
        Ok(ScriptColor::from_hsv(
            num(&h, "hue")? as f32,
            num(&s, "saturation")? as f32,
            num(&v, "value")? as f32,
        ))
    });
    engine.register_fn("color_hex", |text: &str| -> Result<ScriptColor, Box<EvalAltResult>> {
        ScriptColor::from_hex(text).map_err(err)
    });

    engine.register_get("r", |c: &mut ScriptColor| c.r as f64);
    engine.register_get("g", |c: &mut ScriptColor| c.g as f64);
    engine.register_get("b", |c: &mut ScriptColor| c.b as f64);
    engine.register_get("h", |c: &mut ScriptColor| c.hue() as f64);
    engine.register_get("s", |c: &mut ScriptColor| c.saturation() as f64);
    engine.register_get("v", |c: &mut ScriptColor| c.value() as f64);

    engine.register_fn("rotate_hue", |c: &mut ScriptColor, d: Dynamic| -> Result<ScriptColor, Box<EvalAltResult>> {
        Ok(c.rotate_hue(num(&d, "degrees")? as f32))
    });
    engine.register_fn("with_hue", |c: &mut ScriptColor, h: Dynamic| -> Result<ScriptColor, Box<EvalAltResult>> {
        Ok(c.with_hue(num(&h, "hue")? as f32))
    });
    engine.register_fn("with_saturation", |c: &mut ScriptColor, s: Dynamic| -> Result<ScriptColor, Box<EvalAltResult>> {
        Ok(c.with_saturation(num(&s, "saturation")? as f32))
    });
    engine.register_fn("with_value", |c: &mut ScriptColor, v: Dynamic| -> Result<ScriptColor, Box<EvalAltResult>> {
        Ok(c.with_value(num(&v, "value")? as f32))
    });
    engine.register_fn("mix", |c: &mut ScriptColor, other: ScriptColor, t: Dynamic| -> Result<ScriptColor, Box<EvalAltResult>> {
        Ok(c.mix(other, num(&t, "mix amount")? as f32))
    });
    engine.register_fn("hex", |c: &mut ScriptColor| c.to_hex());
    engine.register_fn("to_string", |c: &mut ScriptColor| c.to_hex());

    // ---- building a palette ----

    let s = Rc::clone(&state);
    engine.register_fn(
        "set_palette_colors",
        move |f: &mut FlameHandle, name: &str, colors: Array| -> Result<(), Box<EvalAltResult>> {
            let cols = colors_from_array(&colors)?;
            let last = cols.len().saturating_sub(1).max(1) as f32;
            let stops = cols
                .iter()
                .enumerate()
                .map(|(i, c)| ColorStop { position: i as f32 / last, color: c.to_rgb() })
                .collect();
            apply_palette(f, &s, name, stops)
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn(
        "set_palette_stops",
        move |f: &mut FlameHandle, name: &str, stops: Array| -> Result<(), Box<EvalAltResult>> {
            let mut built: Vec<ColorStop> = Vec::with_capacity(stops.len());
            for entry in &stops {
                let pair = entry
                    .clone()
                    .try_cast::<Array>()
                    .ok_or_else(|| err("each stop must be [position, color]"))?;
                if pair.len() != 2 {
                    return Err(err("each stop must be [position, color]"));
                }
                let pos = num(&pair[0], "stop position")? as f32;
                let color = pair[1]
                    .clone()
                    .try_cast::<ScriptColor>()
                    .ok_or_else(|| err("the second item of a stop must be a color"))?;
                built.push(ColorStop { position: pos.clamp(0.0, 1.0), color: color.to_rgb() });
            }
            // The gradient is walked in order; a script is free to list
            // its stops in any.
            built.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap_or(std::cmp::Ordering::Equal));
            apply_palette(f, &s, name, built)
        },
    );
}

/// Working on a palette slot by slot.
///
/// A gradient's stops sit wherever the script put them, which makes
/// "slice it up and reorder it" fiddly and "add noise per colour"
/// uneven. Fixed mode lays the palette out as 256 evenly spaced slots —
/// the same conversion the Palette Editor's Fixed switch performs — and
/// then both are just array work.
fn register_palette_slots(engine: &mut Engine) {
    engine.register_fn("palette_to_fixed", |f: &mut FlameHandle| {
        f.cfg.borrow_mut().palette.convert_to_fixed();
    });

    engine.register_fn("palette_colors", |f: &mut FlameHandle| -> Array {
        f.cfg
            .borrow()
            .palette
            .stops
            .iter()
            .map(|s| Dynamic::from(ScriptColor::from_rgb(s.color)))
            .collect()
    });

    engine.register_fn(
        "set_palette_fixed",
        |f: &mut FlameHandle, name: &str, colors: Array| -> Result<(), Box<EvalAltResult>> {
            let cols = colors_from_array(&colors)?;
            // Locked palettes hold exactly 256 slots. Resample rather
            // than reject: a script that built 64 colours still means a
            // palette, and the Palette Editor's own Fixed mode resamples
            // the same way.
            let last = cols.len() - 1;
            let stops = (0..256)
                .map(|i| {
                    let t = i as f32 / 255.0;
                    let x = t * last as f32;
                    let lo = (x.floor() as usize).min(last);
                    let hi = (lo + 1).min(last);
                    let c = cols[lo].mix(cols[hi], x - lo as f32);
                    ColorStop { position: t, color: c.to_rgb() }
                })
                .collect();
            f.cfg.borrow_mut().palette =
                crate::scene::palette::Palette::new_locked(name.to_string(), stops);
            Ok(())
        },
    );
}

/// Two colours are the fewest that make a gradient; one would be a flat
/// fill and none would silently blank the flame.
fn colors_from_array(colors: &Array) -> Result<Vec<ScriptColor>, Box<EvalAltResult>> {
    if colors.len() < 2 {
        return Err(err("a palette needs at least two colours"));
    }
    colors
        .iter()
        .map(|c| {
            c.clone()
                .try_cast::<ScriptColor>()
                .ok_or_else(|| err("expected a color — build one with color(), color_hsv() or color_hex()"))
        })
        .collect()
}

fn apply_palette(
    f: &mut FlameHandle,
    state: &Rc<RefCell<ScriptState>>,
    name: &str,
    stops: Vec<ColorStop>,
) -> Result<(), Box<EvalAltResult>> {
    if stops.len() < 2 {
        return Err(err("a palette needs at least two stops"));
    }
    if stops.len() > 256 {
        return Err(err("a palette holds at most 256 stops"));
    }
    let _ = state;
    f.cfg.borrow_mut().palette = crate::scene::palette::Palette::new(name.to_string(), stops);
    Ok(())
}


/// Palette choice, in three modes:
///
/// * **Leave it alone** — a script that calls nothing here keeps whatever
///   palette the flame already had. This is the default and needs no API.
/// * **Pick an existing one** — `flame.set_palette(name)`, or
///   `flame.random_palette()` for a seeded random choice from the loaded
///   library.
/// * **Generate one** — not yet: a colour-theory palette generator
///   (main/secondary/tertiary colours combined by complementary,
///   analogous or monochromatic rules) is planned as a system shared with
///   the Palette UI. See the project doc.
fn register_palettes(engine: &mut Engine, state: Rc<RefCell<ScriptState>>) {
    let s = Rc::clone(&state);
    engine.register_fn(
        "set_palette",
        move |f: &mut FlameHandle, name: &str| -> Result<(), Box<EvalAltResult>> {
            let st = s.borrow();
            let found = st
                .palettes
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(name))
                .cloned();
            match found {
                Some(p) => {
                    f.cfg.borrow_mut().palette = p;
                    Ok(())
                }
                None if st.palettes.is_empty() => Err(err(
                    "no palette library is available here (try running from the app)",
                )),
                None => Err(err(format!(
                    "no palette named `{name}` — see palette_names() for what's loaded"
                ))),
            }
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn(
        "random_palette",
        move |f: &mut FlameHandle| -> Result<String, Box<EvalAltResult>> {
            let mut st = s.borrow_mut();
            if st.palettes.is_empty() {
                return Err(err(
                    "no palette library is available here (try running from the app)",
                ));
            }
            let count = st.palettes.len() as u64;
            let i = st.rng.gen_range(0..count) as usize;
            let chosen = st.palettes[i].clone();
            let name = chosen.name.clone();
            f.cfg.borrow_mut().palette = chosen;
            Ok(name)
        },
    );

    let s = Rc::clone(&state);
    engine.register_fn("palette_names", move || -> Array {
        s.borrow()
            .palettes
            .iter()
            .map(|p| Dynamic::from(p.name.clone()))
            .collect()
    });
}

/// `[x1, y1, x2, y2, depth, symbol]`.
fn segment_to_array(s: &crate::script::builtins::Segment) -> Array {
    vec![
        Dynamic::from(s.x1),
        Dynamic::from(s.y1),
        Dynamic::from(s.x2),
        Dynamic::from(s.y2),
        Dynamic::from(s.depth as f64),
        Dynamic::from(s.symbol.to_string()),
    ]
}

/// The similarity carrying the unit edge (0,0)-(1,0) onto a segment.
///
/// Without `mirror` it is a rotation and scale by the segment's vector.
/// With it, the perpendicular is flipped too — a reflected similarity.
/// Curves built from mirror-paired rules need that: the endpoints are
/// identical either way, so the chirality has to come from the symbol.
fn apply_segment(
    t: &mut TransformHandle,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    mirror: bool,
) -> Result<(), Box<EvalAltResult>> {
    let (dx, dy) = (x2 - x1, y2 - y1);
    t.with(|x| {
        x.a = dx as f32;
        x.c = dy as f32;
        if mirror {
            x.b = dy as f32;
            x.d = -dx as f32;
        } else {
            x.b = -dy as f32;
            x.d = dx as f32;
        }
        x.e = x1 as f32;
        x.f = y1 as f32;
        x.set_variation("linear", 1.0);
    })
}

// ------------------------------------------------------------- built-ins

/// Group recipes and the helpers that map them onto transforms.
fn register_builtins(engine: &mut Engine) {
    use crate::script::builtins::{avoid_xaos_row, schottky_generators, Circle};

    // schottky_generators([[x,y,r] x 4], twist_a, twist_b) -> 4 x 8 numbers
    engine.register_fn(
        "schottky_generators",
        |circles: Array, twist_a: Dynamic, twist_b: Dynamic| -> Result<Array, Box<EvalAltResult>> {
            let (twist_a, twist_b) = (num(&twist_a, "twist A")?, num(&twist_b, "twist B")?);
            if circles.len() != 4 {
                return Err(err(format!(
                    "schottky_generators needs 4 circles as [x, y, radius], got {}",
                    circles.len()
                )));
            }
            let mut parsed = [Circle { x: 0.0, y: 0.0, r: 1.0 }; 4];
            for (i, c) in circles.iter().enumerate() {
                let arr = c
                    .clone()
                    .into_array()
                    .map_err(|_| err(format!("circle {i} must be [x, y, radius]")))?;
                if arr.len() != 3 {
                    return Err(err(format!("circle {i} must be [x, y, radius]")));
                }
                parsed[i] = Circle {
                    x: num(&arr[0], "circle x")?,
                    y: num(&arr[1], "circle y")?,
                    r: num(&arr[2], "circle radius")?,
                };
            }
            Ok(schottky_generators(parsed, twist_a, twist_b)
                .iter()
                .map(|m| {
                    Dynamic::from(
                        m.to_params()
                            .iter()
                            .map(|v| Dynamic::from(*v))
                            .collect::<Array>(),
                    )
                })
                .collect())
        },
    );

    // apollonian_generators(deform, angle, hyper_angle, strength)
    engine.register_fn(
        "apollonian_generators",
        |deform: bool, theta: Dynamic, eta: Dynamic, delta: Dynamic| -> Result<Array, Box<EvalAltResult>> {
            let theta = num(&theta, "conjugation angle")?;
            let eta = num(&eta, "hyperbolic angle")?;
            let delta = num(&delta, "deform strength")?;
            Ok(
                crate::script::builtins::apollonian_generators(deform, theta, eta, delta)
                    .iter()
                    .map(|m| {
                        Dynamic::from(
                            m.to_params()
                                .iter()
                                .map(|v| Dynamic::from(*v))
                                .collect::<Array>(),
                        )
                    })
                    .collect(),
            )
        },
    );

    // The xaos row reproducing a Schottky group's "never undo the last
    // generator" rule, for transform `from` of four.
    engine.register_fn("avoid_xaos_row", |from: i64| -> Result<Array, Box<EvalAltResult>> {
        let from = usize::try_from(from).map_err(|_| err("transform index must be >= 0"))?;
        if from >= 4 {
            return Err(err("avoid_xaos_row expects a 4-generator group (index 0-3)"));
        }
        Ok(avoid_xaos_row(from).iter().map(|v| Dynamic::from(*v as f64)).collect())
    });

    // lsystem(axiom, #{"F": "F+F--F+F"}, depth) -> expanded string
    engine.register_fn(
        "lsystem",
        |axiom: &str, rules: rhai::Map, depth: i64| -> Result<String, Box<EvalAltResult>> {
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                let mut chars = key.chars();
                let symbol = chars
                    .next()
                    .ok_or_else(|| err("an L-system rule key must be a single symbol"))?;
                if chars.next().is_some() {
                    return Err(err(format!(
                        "rule key `{key}` must be ONE symbol — rules rewrite single characters"
                    )));
                }
                parsed.push((symbol, value.clone().into_string().unwrap_or_default()));
            }
            // Deterministic order, so a script's output never depends on
            // map iteration order.
            parsed.sort_by_key(|(k, _)| *k);
            let depth = depth.clamp(0, 32) as u32;
            crate::script::builtins::lsystem_expand(axiom, &parsed, depth).map_err(err)
        },
    );

    // turtle(expanded, angle) -> [[x1, y1, x2, y2, depth], ...]
    engine.register_fn(
        "turtle",
        |expanded: &str, angle: Dynamic| -> Result<Array, Box<EvalAltResult>> {
            let angle = num(&angle, "turtle angle")?;
            Ok(crate::script::builtins::turtle(expanded, angle)
                .iter()
                .map(|s| Dynamic::from(segment_to_array(s)))
                .collect())
        },
    );

    // The symbol whose rule mirrors the given one, or "" if there is no
    // such pair. Pieces it draws need a reflected transform.
    engine.register_fn(
        "lsystem_mirror_symbol",
        |rules: rhai::Map, primary: &str| -> String {
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                if let Some(sym) = key.chars().next() {
                    parsed.push((sym, value.clone().into_string().unwrap_or_default()));
                }
            }
            parsed.sort_by_key(|(k, _)| *k);
            primary
                .chars()
                .next()
                .and_then(|p| crate::script::builtins::mirror_partner(&parsed, p))
                .map(|c| c.to_string())
                .unwrap_or_default()
        },
    );

    // hilbert3d_maps() -> 8 x [12 affine floats]: a self-similar 3D
    // Hilbert curve (octants in face-adjacent order, exits chaining into
    // entries, ending at (1,0,0)). Feed to lsystem_path_3D or matrix3D.
    engine.register_fn("hilbert3d_maps", || -> Array {
        crate::script::builtins::hilbert3d_maps()
            .iter()
            .map(|m| Dynamic::from(m.iter().map(|v| Dynamic::from(*v)).collect::<Array>()))
            .collect()
    });

    // lsystem_graph_pieces(axiom, rules, angle)
    //   -> #{ pieces: [[12 floats, depth, occ, owner], ...], note: "" }
    // Multi-variable (graph-directed) extraction; the word structure is
    // meant for xaos: a piece may follow another only when it CONSUMES
    // the type the other PRODUCED (occ(next) == owner(prev)).
    engine.register_fn(
        "lsystem_graph_pieces",
        |axiom: &str, rules: rhai::Map, angle: Dynamic| -> Result<rhai::Map, Box<EvalAltResult>> {
            let angle = num(&angle, "turtle angle")?;
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                if let Some(sym) = key.chars().next() {
                    parsed.push((sym, value.clone().into_string().unwrap_or_default()));
                }
            }
            parsed.sort_by_key(|(k, _)| *k);
            let mut out = rhai::Map::new();
            match crate::script::builtins::lsystem_graph_pieces(axiom, &parsed, angle) {
                Ok(pieces) => {
                    let arr: Array = pieces
                        .iter()
                        .map(|p| {
                            let mut a: Array = p.m.iter().map(|v| Dynamic::from(*v)).collect();
                            a.push(Dynamic::from(p.depth as f64));
                            a.push(Dynamic::from(p.occ.to_string()));
                            a.push(Dynamic::from(p.owner.to_string()));
                            Dynamic::from(a)
                        })
                        .collect();
                    out.insert("pieces".into(), Dynamic::from(arr));
                    out.insert("note".into(), Dynamic::from(String::new()));
                }
                Err(e) => {
                    out.insert("pieces".into(), Dynamic::from(Array::new()));
                    out.insert("note".into(), Dynamic::from(e));
                }
            }
            Ok(out)
        },
    );

    // lsystem_pieces3(axiom, rules, angle)
    //   -> #{ branches: [...], stems: [...], note: "" }
    // Pieces are [12 affine floats row-major, depth, symbol]. The 3D
    // construction (plants, edge and node curves unify once pieces carry
    // a full frame).
    engine.register_fn(
        "lsystem_pieces3",
        |axiom: &str, rules: rhai::Map, angle: Dynamic| -> Result<rhai::Map, Box<EvalAltResult>> {
            let angle = num(&angle, "turtle angle")?;
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                if let Some(sym) = key.chars().next() {
                    parsed.push((sym, value.clone().into_string().unwrap_or_default()));
                }
            }
            parsed.sort_by_key(|(k, _)| *k);
            let piece_arr = |p: &crate::script::builtins::Piece3| -> Dynamic {
                let mut a: Array = p.m.iter().map(|v| Dynamic::from(*v)).collect();
                a.push(Dynamic::from(p.depth as f64));
                a.push(Dynamic::from(p.symbol.to_string()));
                Dynamic::from(a)
            };
            let mut out = rhai::Map::new();
            match crate::script::builtins::lsystem_pieces3(axiom, &parsed, angle) {
                Ok(p) => {
                    out.insert("branches".into(), Dynamic::from(p.branches.iter().map(piece_arr).collect::<Array>()));
                    out.insert("stems".into(), Dynamic::from(p.stems.iter().map(piece_arr).collect::<Array>()));
                    out.insert("note".into(), Dynamic::from(String::new()));
                }
                Err(e) => {
                    out.insert("branches".into(), Dynamic::from(Array::new()));
                    out.insert("stems".into(), Dynamic::from(Array::new()));
                    out.insert("note".into(), Dynamic::from(e));
                }
            }
            Ok(out)
        },
    );

    // Does this rule set use the 3D turtle commands (& ^ \\ /)?
    engine.register_fn("lsystem_uses_3d", |axiom: &str, rules: rhai::Map| -> bool {
        let mut parsed: Vec<(char, String)> = Vec::new();
        for (key, value) in rules.iter() {
            if let Some(sym) = key.chars().next() {
                parsed.push((sym, value.clone().into_string().unwrap_or_default()));
            }
        }
        crate::script::builtins::lsystem_uses_3d(axiom, &parsed)
    });

    // lsystem_bounds3(axiom, rules, depth, angle)
    //   -> [min_x, min_y, min_z, max_x, max_y, max_z]
    engine.register_fn(
        "lsystem_bounds3",
        |axiom: &str, rules: rhai::Map, depth: i64, angle: Dynamic| -> Result<Array, Box<EvalAltResult>> {
            let angle = num(&angle, "turtle angle")?;
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                if let Some(sym) = key.chars().next() {
                    parsed.push((sym, value.clone().into_string().unwrap_or_default()));
                }
            }
            parsed.sort_by_key(|(k, _)| *k);
            let b = crate::script::builtins::lsystem_bounds3(
                axiom,
                &parsed,
                depth.clamp(0, 32) as u32,
                angle,
                1_000_000,
            )
            .ok_or_else(|| err("could not measure this system"))?;
            Ok([b.0, b.1, b.2, b.3, b.4, b.5]
                .iter()
                .map(|v| Dynamic::from(*v))
                .collect())
        },
    );

    // Turn a transform into an exact 3D affine piece: the matrix3D
    // variation with the twelve coefficients from a pieces3 entry,
    // optionally squashing the two off-axis columns (the 3D Barnsley
    // stem: 0 lays a flattened copy of the whole along the piece).
    engine.register_fn(
        "set_matrix3d",
        |t: &mut TransformHandle, piece: Array, thickness: Dynamic| -> Result<(), Box<EvalAltResult>> {
            if piece.len() < 12 {
                return Err(err("set_matrix3d expects at least 12 affine numbers"));
            }
            let th = num(&thickness, "thickness")?;
            let mut m = [0.0f64; 12];
            for (i, v) in piece.iter().take(12).enumerate() {
                m[i] = num(v, "matrix coefficient")?;
            }
            // Columns 1 (left) and 2 (up) carry the off-axis spread.
            for row in 0..3 {
                m[row * 3 + 1] *= th;
                m[row * 3 + 2] *= th;
            }
            let names = ["xx", "xy", "xz", "yx", "yy", "yz", "zx", "zy", "zz", "tx", "ty", "tz"];
            t.with(|x| {
                x.set_variation("matrix3D", 1.0);
                for (name, v) in names.iter().zip(m) {
                    x.variation_params.insert(format!("matrix3D.{name}"), v as f32);
                }
            })
        },
    );

    // lsystem_plant_pieces(axiom, rules, angle)
    //   -> #{ branches: [...], stems: [...], note: "" }
    // The Barnsley-fern construction for bracketed (plant) rules.
    engine.register_fn(
        "lsystem_plant_pieces",
        |axiom: &str, rules: rhai::Map, angle: Dynamic| -> Result<rhai::Map, Box<EvalAltResult>> {
            let angle = num(&angle, "turtle angle")?;
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                if let Some(sym) = key.chars().next() {
                    parsed.push((sym, value.clone().into_string().unwrap_or_default()));
                }
            }
            parsed.sort_by_key(|(k, _)| *k);
            let mut out = rhai::Map::new();
            match crate::script::builtins::lsystem_plant_segments(axiom, &parsed, angle) {
                Ok(p) => {
                    let to_arr = |v: &Vec<crate::script::builtins::Segment>| -> Array {
                        v.iter().map(|s| Dynamic::from(segment_to_array(s))).collect()
                    };
                    out.insert("branches".into(), Dynamic::from(to_arr(&p.branches)));
                    out.insert("stems".into(), Dynamic::from(to_arr(&p.stems)));
                    out.insert("note".into(), Dynamic::from(String::new()));
                }
                Err(e) => {
                    out.insert("branches".into(), Dynamic::from(Array::new()));
                    out.insert("stems".into(), Dynamic::from(Array::new()));
                    out.insert("note".into(), Dynamic::from(e));
                }
            }
            Ok(out)
        },
    );

    // set_segment(seg, mirror, thickness): the squashed variant — the
    // perpendicular axis is scaled by `thickness`, so at 0 the transform
    // lays a flattened copy of the whole attractor along the segment.
    // That is the Barnsley fern's stem map: the rachis is drawn by
    // squashed images of the entire plant.
    engine.register_fn(
        "set_segment",
        |t: &mut TransformHandle, seg: Array, mirror: bool, thickness: Dynamic| -> Result<(), Box<EvalAltResult>> {
            if seg.len() < 4 {
                return Err(err("set_segment expects [x1, y1, x2, y2]"));
            }
            let x1 = num(&seg[0], "x1")?;
            let y1 = num(&seg[1], "y1")?;
            let x2 = num(&seg[2], "x2")?;
            let y2 = num(&seg[3], "y2")?;
            let th = num(&thickness, "thickness")?;
            let (dx, dy) = (x2 - x1, y2 - y1);
            t.with(|x| {
                x.a = dx as f32;
                x.c = dy as f32;
                if mirror {
                    x.b = (dy * th) as f32;
                    x.d = (-dx * th) as f32;
                } else {
                    x.b = (-dy * th) as f32;
                    x.d = (dx * th) as f32;
                }
                x.e = x1 as f32;
                x.f = y1 as f32;
                x.set_variation("linear", 1.0);
            })
        },
    );

    // lsystem_node_pieces(axiom, rules, angle)
    //   -> #{ pieces: [[x1,y1,x2,y2,0,symbol], ...], note: "" }
    // The node-rewriting (space-filling) construction; `note` explains an
    // empty result instead of guessing.
    engine.register_fn(
        "lsystem_node_pieces",
        |axiom: &str, rules: rhai::Map, angle: Dynamic| -> Result<rhai::Map, Box<EvalAltResult>> {
            let angle = num(&angle, "turtle angle")?;
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                if let Some(sym) = key.chars().next() {
                    parsed.push((sym, value.clone().into_string().unwrap_or_default()));
                }
            }
            parsed.sort_by_key(|(k, _)| *k);
            let mut out = rhai::Map::new();
            match crate::script::builtins::lsystem_node_segments(axiom, &parsed, angle) {
                Ok(segs) => {
                    let arr: Array = segs.iter().map(|s| Dynamic::from(segment_to_array(s))).collect();
                    out.insert("pieces".into(), Dynamic::from(arr));
                    out.insert("note".into(), Dynamic::from(String::new()));
                }
                Err(e) => {
                    out.insert("pieces".into(), Dynamic::from(Array::new()));
                    out.insert("note".into(), Dynamic::from(e));
                }
            }
            Ok(out)
        },
    );

    // lsystem_bounds(axiom, rules, depth, angle)
    //   -> [min_x, min_y, max_x, max_y, depth_actually_used]
    engine.register_fn(
        "lsystem_bounds",
        |axiom: &str, rules: rhai::Map, depth: i64, angle: Dynamic| -> Result<Array, Box<EvalAltResult>> {
            let angle = num(&angle, "turtle angle")?;
            let mut parsed: Vec<(char, String)> = Vec::new();
            for (key, value) in rules.iter() {
                if let Some(sym) = key.chars().next() {
                    parsed.push((sym, value.clone().into_string().unwrap_or_default()));
                }
            }
            parsed.sort_by_key(|(k, _)| *k);
            // Well inside the engine's array limit, since this walk stays
            // in Rust and never becomes a script value.
            const MAX_SEGMENTS: usize = 400_000;
            let (x0, y0, x1, y1, used) = crate::script::builtins::lsystem_bounds(
                axiom,
                &parsed,
                depth.clamp(0, 32) as u32,
                angle,
                MAX_SEGMENTS,
            )
            .ok_or_else(|| {
                err("could not measure this curve — it may return to its start, so it has no unit-segment form")
            })?;
            Ok([x0, y0, x1, y1, used as f64]
                .iter()
                .map(|v| Dynamic::from(*v))
                .collect())
        },
    );

    // Rescale a turtle path so its overall run is the unit segment, which
    // turns the pieces into the contractions of an IFS.
    engine.register_fn(
        "normalize_segments",
        |segs: Array| -> Result<Array, Box<EvalAltResult>> {
            let mut parsed = Vec::with_capacity(segs.len());
            for (i, d) in segs.iter().enumerate() {
                let a = d
                    .clone()
                    .into_array()
                    .map_err(|_| err(format!("segment {i} must be [x1, y1, x2, y2, depth]")))?;
                if a.len() < 4 {
                    return Err(err(format!("segment {i} must have at least 4 numbers")));
                }
                parsed.push(crate::script::builtins::Segment {
                    x1: num(&a[0], "x1")?,
                    y1: num(&a[1], "y1")?,
                    x2: num(&a[2], "x2")?,
                    y2: num(&a[3], "y2")?,
                    depth: if a.len() > 4 { num(&a[4], "depth")? as u32 } else { 0 },
                    symbol: a
                        .get(5)
                        .and_then(|d| d.clone().into_string().ok())
                        .and_then(|s| s.chars().next())
                        .unwrap_or('F'),
                });
            }
            let normalized = crate::script::builtins::normalize_segments(&parsed).ok_or_else(|| {
                err("this path returns to where it started, so it has no unit-segment form — \
                     use an open curve, or place the segments yourself")
            })?;
            Ok(normalized
                .iter()
                .map(|s| Dynamic::from(segment_to_array(s)))
                .collect())
        },
    );

    // Turn a transform into the similarity carrying the unit segment
    // (0,0)-(1,0) onto the given one: rotate+scale by the segment's
    // vector, then translate to its start.
    engine.register_fn(
        "set_segment",
        |t: &mut TransformHandle, seg: Array| -> Result<(), Box<EvalAltResult>> {
            if seg.len() < 4 {
                return Err(err("set_segment expects [x1, y1, x2, y2]"));
            }
            let x1 = num(&seg[0], "x1")?;
            let y1 = num(&seg[1], "y1")?;
            let x2 = num(&seg[2], "x2")?;
            let y2 = num(&seg[3], "y2")?;
            apply_segment(t, x1, y1, x2, y2, false)
        },
    );

    // Same, but mirrored across the segment: the piece is laid down
    // back-to-front. Needed by curves whose rules come in mirror pairs.
    engine.register_fn(
        "set_segment",
        |t: &mut TransformHandle, seg: Array, mirror: bool| -> Result<(), Box<EvalAltResult>> {
            if seg.len() < 4 {
                return Err(err("set_segment expects [x1, y1, x2, y2]"));
            }
            let x1 = num(&seg[0], "x1")?;
            let y1 = num(&seg[1], "y1")?;
            let x2 = num(&seg[2], "x2")?;
            let y2 = num(&seg[3], "y2")?;
            apply_segment(t, x1, y1, x2, y2, mirror)
        },
    );

    // The symbol that drew a segment, as a one-character string.
    engine.register_fn("segment_symbol", |seg: Array| -> String {
        seg.get(5)
            .and_then(|d| d.clone().into_string().ok())
            .unwrap_or_else(|| "F".to_string())
    });

    // klein_generators(recipe, a_re, a_im, b_re, b_im, weight)
    //   -> [a, a-inverse, b, b-inverse], each 8 numbers
    engine.register_fn(
        "klein_generators",
        |recipe: i64,
         a_re: Dynamic,
         a_im: Dynamic,
         b_re: Dynamic,
         b_im: Dynamic,
         weight: Dynamic|
         -> Result<Array, Box<EvalAltResult>> {
            let gens = crate::script::builtins::klein_generators(
                recipe.clamp(0, 6) as u32,
                num(&a_re, "A real")?,
                num(&a_im, "A imaginary")?,
                num(&b_re, "B real")?,
                num(&b_im, "B imaginary")?,
                num(&weight, "variation weight")?,
            );
            Ok(gens
                .iter()
                .map(|m| {
                    Dynamic::from(
                        m.to_params()
                            .iter()
                            .map(|v| Dynamic::from(*v))
                            .collect::<Array>(),
                    )
                })
                .collect())
        },
    );

    // Xaos row that forbids one index outright, leaving the rest equal.
    engine.register_fn(
        "exclude_xaos_row",
        |forbidden: i64, count: i64| -> Result<Array, Box<EvalAltResult>> {
            let count = usize::try_from(count).map_err(|_| err("count must be >= 0"))?;
            let forbidden = usize::try_from(forbidden).map_err(|_| err("index must be >= 0"))?;
            if count == 0 {
                return Err(err("exclude_xaos_row needs at least one transform"));
            }
            Ok(crate::script::builtins::exclude_xaos_row(forbidden, count)
                .iter()
                .map(|v| Dynamic::from(*v as f64))
                .collect())
        },
    );

    // sphere_packing_mirrors(mode, size, ring_n, ring_scale, cap_scale,
    //                        jitter, tilt, three_d) -> [[x, y, z, r], ...]
    engine.register_fn(
        "sphere_packing_mirrors",
        |mode: i64,
         size: Dynamic,
         ring_n: i64,
         ring_scale: Dynamic,
         cap_scale: Dynamic,
         jitter: Dynamic,
         tilt: Dynamic,
         three_d: bool|
         -> Result<Array, Box<EvalAltResult>> {
            let mirrors = crate::script::builtins::sphere_packing_mirrors(
                mode.clamp(0, 3) as u32,
                num(&size, "packing size")?,
                ring_n.clamp(2, 16) as u32,
                num(&ring_scale, "ring scale")?,
                num(&cap_scale, "cap scale")?,
                num(&jitter, "size jitter")?,
                num(&tilt, "ring tilt")?,
                three_d,
            );
            Ok(mirrors
                .iter()
                .map(|s| {
                    Dynamic::from(
                        [s.x, s.y, s.z, s.r]
                            .iter()
                            .map(|v| Dynamic::from(*v))
                            .collect::<Array>(),
                    )
                })
                .collect())
        },
    );

    // Xaos row blocking a repeat of the same mirror (inversions are their
    // own inverse), for `count` mirrors.
    engine.register_fn(
        "repeat_xaos_row",
        |from: i64, count: i64| -> Result<Array, Box<EvalAltResult>> {
            let count = usize::try_from(count).map_err(|_| err("count must be >= 0"))?;
            let from = usize::try_from(from).map_err(|_| err("index must be >= 0"))?;
            if count == 0 || from >= count {
                return Err(err("repeat_xaos_row: index outside the mirror count"));
            }
            Ok(crate::script::builtins::repeat_xaos_row(from, count)
                .iter()
                .map(|v| Dynamic::from(*v as f64))
                .collect())
        },
    );

    // Turn a transform into an inversion in the given sphere.
    //
    // An inversion x -> c + r^2 (x - c)/|x - c|^2 is exactly "translate so
    // the centre is at the origin, apply p/|p|^2 scaled by r^2, translate
    // back" — so it needs no special variation, just the affine machinery
    // the flame already has. Using spherical3D_wf (whose default exponent
    // gives p/|p|^2, in the plane or in space) means the result is an
    // ordinary transform the triangle editor can grab.
    engine.register_fn(
        "set_inversion",
        |t: &mut TransformHandle, centre: Array, radius: Dynamic| -> Result<(), Box<EvalAltResult>> {
            if centre.len() != 2 && centre.len() != 3 {
                return Err(err("set_inversion centre must be [x, y] or [x, y, z]"));
            }
            let cx = num(&centre[0], "centre x")?;
            let cy = num(&centre[1], "centre y")?;
            let cz = if centre.len() == 3 { num(&centre[2], "centre z")? } else { 0.0 };
            let r = num(&radius, "radius")?;
            if r <= 0.0 {
                return Err(err("set_inversion radius must be positive"));
            }
            t.with(|x| {
                // Pre-affine: move the sphere's centre to the origin.
                x.a = 1.0; x.b = 0.0; x.c = 0.0; x.d = 1.0;
                x.e = -cx as f32;
                x.f = -cy as f32;
                x.g = -cz as f32;
                // The inversion kernel, weighted by r^2.
                x.set_variation("spherical3D_wf", (r * r) as f32);
                // Post-affine: move it back.
                x.post_affine_enabled = true;
                x.post_a = 1.0; x.post_b = 0.0; x.post_c = 0.0; x.post_d = 1.0;
                x.post_e = cx as f32;
                x.post_f = cy as f32;
                x.post_g = cz as f32;
            })
        },
    );

    // Write a generator onto a transform: adds `mobius` and its 8 params.
    engine.register_fn(
        "set_mobius",
        |t: &mut TransformHandle, coeffs: Array| -> Result<(), Box<EvalAltResult>> {
            if coeffs.len() != 8 {
                return Err(err(format!(
                    "set_mobius expects 8 numbers [re_a, im_a, re_b, im_b, re_c, im_c, re_d, im_d], got {}",
                    coeffs.len()
                )));
            }
            let names = [
                "re_a", "im_a", "re_b", "im_b", "re_c", "im_c", "re_d", "im_d",
            ];
            let mut values = [0.0f32; 8];
            for (i, d) in coeffs.iter().enumerate() {
                values[i] = num(d, "mobius coefficient")? as f32;
            }
            t.with(|x| {
                x.set_variation("mobius", 1.0);
                for (name, v) in names.iter().zip(values) {
                    x.variation_params.insert(format!("mobius.{name}"), v);
                }
            })
        },
    );
}

// -------------------------------------------------------- registry queries

fn register_registry_queries(engine: &mut Engine) {
    engine.register_fn("variation_names", || -> Array {
        crate::variations::global_registry()
            .names()
            .iter()
            .map(|n| Dynamic::from(n.clone()))
            .collect()
    });

    engine.register_fn("variation_exists", |name: &str| -> bool {
        crate::variations::global_registry().get(name).is_some()
    });

    // Does this variation write z UNCONDITIONALLY (Feature::AlwaysZ)?
    //
    // Under preserve_z = false the renderer flattens z every iteration,
    // and only AlwaysZ variations survive that. A decomposition therefore
    // has to know whether it is trading an AlwaysZ variation for one that
    // is not — see the preserve_z rule in decompose_group.rhai.
    engine.register_fn("variation_always_z", |name: &str| -> bool {
        crate::variations::global_registry()
            .get(name)
            .is_some_and(|info| {
                info.has_feature(crate::variations::definition::Feature::AlwaysZ)
            })
    });

    engine.register_fn("effect_exists", |name: &str| -> bool {
        crate::effects::global_effect_registry().get(name).is_some()
    });
}
