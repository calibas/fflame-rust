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
use super::{humanize, ParamDecl, ParamValue, ScriptKind};

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

// ============================================================================
// Registration
// ============================================================================

/// Put the top-level objects in scope.
///
/// Pushed as ordinary variables, not constants: Rhai refuses property and
/// indexer assignment on a constant, so `config["gamma"] = 2.2` and
/// `flame.name = "x"` would fail. Rebinding them only breaks the script's
/// own run.
pub(crate) fn push_globals(scope: &mut Scope, cfg: Rc<RefCell<FractalConfig>>) {
    scope.push("flame", FlameHandle { cfg: Rc::clone(&cfg) });
    scope.push("config", ConfigHandle { cfg });
}

pub(crate) fn register(
    engine: &mut Engine,
    cfg: Rc<RefCell<FractalConfig>>,
    state: Rc<RefCell<ScriptState>>,
) {
    engine.register_type_with_name::<FlameHandle>("Flame");
    engine.register_type_with_name::<TransformHandle>("Transform");
    engine.register_type_with_name::<ConfigHandle>("Config");

    register_meta(engine, Rc::clone(&state));
    register_rng(engine, Rc::clone(&state));
    register_flame(engine);
    register_transform(engine);
    register_config(engine, Rc::clone(&state));
    register_palettes(engine, Rc::clone(&state));
    register_registry_queries(engine);

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
    engine.register_fn("script", move |name: &str, kind: &str| -> Result<(), Box<EvalAltResult>> {
        let mut st = s.borrow_mut();
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
        st.meta.name = name.to_string();
        st.meta.kind = Some(kind);
        Ok(())
    });

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
        let i = s.borrow_mut().rng.gen_range(0..items.len());
        Ok(items[i].clone())
    });

    let s = Rc::clone(&state);
    engine.register_fn("shuffle", move |items: Array| -> Array {
        let mut out = items;
        let mut st = s.borrow_mut();
        // Fisher–Yates against the seeded stream, so shuffles reproduce.
        for i in (1..out.len()).rev() {
            let j = st.rng.gen_range(0..=i);
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
        let var_scale: f64 = t
            .variations
            .iter()
            .filter(|(name, _)| match registry.get(name).map(|i| i.phase.clone()) {
                Some(crate::variations::VariationPhase::Normal) => true,
                Some(crate::variations::VariationPhase::Any) => {
                    t.variation_priorities.get(*name).copied().unwrap_or(0) == 0
                }
                Some(_) => false,
                None => true,
            })
            .map(|(_, w)| *w as f64)
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
            let count = st.palettes.len();
            let i = st.rng.gen_range(0..count);
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

    engine.register_fn("effect_exists", |name: &str| -> bool {
        crate::effects::global_effect_registry().get(name).is_some()
    });
}
