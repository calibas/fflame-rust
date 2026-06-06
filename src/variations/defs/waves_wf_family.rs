//! Waves WF family (waves2_wf, waves3_wf, waves4_wf) + dinis_surface_wf
//!
//! Three sine/cosine perturbation variations from the JWildfire WF
//! (Wildfire) family — all share the same 8 user params and 2 init
//! slots `(_dampingX, _dampingY)` but differ in the trig combination
//! applied per axis. Plus dinis_surface_wf for variety.
//!
//!   - waves2_wf (Joel F): single trig, `s/c(y·freqx)`. 8 user params
//!     (scalex, scaley, freqx, freqy, use_cos_x int, use_cos_y int,
//!     dampx, dampy) + 2 init slots
//!     (_dampingX = exp(dampx) or 1, _dampingY = exp(dampy) or 1).
//!     Body factors cleanly.
//!
//!   - waves3_wf (Joel F): squared trig, `s/c(y·freqx)²`. Same params/
//!     init as waves2_wf.
//!
//!   - waves4_wf (Joel F): triple-trig product, `c·s·c` or `s·c·s`.
//!     Same params/init as waves2_wf.
//!
//!   - dinis_surface_wf: Dini's Surface parametric mapping. From 
//!     Dini's Surface, http://mathworld.wolfram.com/DinisSurface.html
//!     2 user params (a, b). Full3D. Body factors cleanly. Uses
//!     `log(tan(v/2))` which can produce NaN for negative tangents —
//!     guarded with abs+max.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/waves2_wf.cpp`
//!   - `output/jwildfire-vars/output/waves3_wf.cpp`
//!   - `output/jwildfire-vars/output/waves4_wf.cpp`
//!   - `output/jwildfire-vars/output/dinis_surface_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// Shared init for waves2/3/4_wf
// ---------------------------------------------------------------------------
//
// init slot 0: _dampingX = |dampx| < eps ? 1 : exp(dampx)
// init slot 1: _dampingY = |dampy| < eps ? 1 : exp(dampy)

const WAVES_WF_INIT: &str = r#"
fn init_waves2_wf(user: array<f32, 8>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = select(exp(user[6]), 1.0, abs(user[6]) < 1e-6);
    out[1] = select(exp(user[7]), 1.0, abs(user[7]) < 1e-6);
    return out;
}
"#;

const WAVES3_WF_INIT: &str = r#"
fn init_waves3_wf(user: array<f32, 8>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = select(exp(user[6]), 1.0, abs(user[6]) < 1e-6);
    out[1] = select(exp(user[7]), 1.0, abs(user[7]) < 1e-6);
    return out;
}
"#;

const WAVES4_WF_INIT: &str = r#"
fn init_waves4_wf(user: array<f32, 8>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = select(exp(user[6]), 1.0, abs(user[6]) < 1e-6);
    out[1] = select(exp(user[7]), 1.0, abs(user[7]) < 1e-6);
    return out;
}
"#;

// ---------------------------------------------------------------------------
// waves2_wf
// ---------------------------------------------------------------------------

/// WF waves2 (single-trig perturbation) — adds a per-axis sin or cos wave
/// to the input: `output = (x + dampX·scalex·trig(y·freqx), y +
/// dampY·scaley·trig(x·freqy))`, with the whole output scaled by the per-
/// axis damping factors. `use_cos_x`/`use_cos_y` pick sin or cos per axis.
///
/// # Authors
/// - Joel Faber
pub static WAVES2_WF: VariationDef = VariationDef {
    name: "waves2_wf",
    aliases: &[],
    display_name: "Waves 2 WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("scalex", "Scale X", unlimited_float, 0.25, -10.0, 10.0, "X-axis wave amplitude."),
        param!("scaley", "Scale Y", unlimited_float, 0.5, -10.0, 10.0, "Y-axis wave amplitude."),
        param!("freqx", "Freq X", unlimited_float, 1.5707963267948966, -100.0, 100.0, "X-axis wave frequency (applied to the Y input)."),
        param!("freqy", "Freq Y", unlimited_float, 0.7853981633974483, -100.0, 100.0, "Y-axis wave frequency (applied to the X input)."),
        param!("use_cos_x", "Use Cos X", bool, true, "When on, use cosine on the X axis. When off, sine."),
        param!("use_cos_y", "Use Cos Y", bool, false, "When on, use cosine on the Y axis. When off, sine."),
        param!("dampx", "Damp X", unlimited_float, 0.0, -10.0, 10.0, "X-axis exponential damping factor — the output is multiplied by `exp(dampx)` (or 1 when `|dampx|` is tiny)."),
        param!("dampy", "Damp Y", unlimited_float, 0.0, -10.0, 10.0, "Y-axis exponential damping factor — same as dampx but for Y."),
    ],
    init_param_count: 2,
    wgsl_init: Some(WAVES_WF_INIT),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waves2_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let use_cos_x = i32(get_param(xform_id, variation_id, 4u));
    let use_cos_y = i32(get_param(xform_id, variation_id, 5u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);
    let tx = select(sin(p.y * freqx), cos(p.y * freqx), use_cos_x == 1);
    let ty = select(sin(p.x * freqy), cos(p.x * freqy), use_cos_y == 1);
    let nx = (p.x + dx * scalex * tx) * dx;
    let ny = (p.y + dy * scaley * ty) * dy;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_waves2_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let use_cos_x = i32(get_param(xform_id, variation_id, 4u));
    let use_cos_y = i32(get_param(xform_id, variation_id, 5u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);
    let tx = select(sin(p.y * freqx), cos(p.y * freqx), use_cos_x == 1);
    let ty = select(sin(p.x * freqy), cos(p.x * freqy), use_cos_y == 1);
    let nx = (p.x + dx * scalex * tx) * dx;
    let ny = (p.y + dy * scaley * ty) * dy;
    return vec3<f32>(nx, ny, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// waves3_wf
// ---------------------------------------------------------------------------

/// WF waves3 (squared-trig perturbation) — same structure as `waves2_wf`
/// but the trig terms are squared (`trig²`), producing always-positive
/// waves with double the apparent frequency.
///
/// # Authors
/// - Joel Faber
pub static WAVES3_WF: VariationDef = VariationDef {
    name: "waves3_wf",
    aliases: &[],
    display_name: "Waves 3 WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("scalex", "Scale X", unlimited_float, 0.25, -10.0, 10.0, "X-axis wave amplitude."),
        param!("scaley", "Scale Y", unlimited_float, 0.5, -10.0, 10.0, "Y-axis wave amplitude."),
        param!("freqx", "Freq X", unlimited_float, 1.5707963267948966, -100.0, 100.0, "X-axis wave frequency (applied to the Y input)."),
        param!("freqy", "Freq Y", unlimited_float, 0.7853981633974483, -100.0, 100.0, "Y-axis wave frequency (applied to the X input)."),
        param!("use_cos_x", "Use Cos X", bool, true, "When on, use cosine on the X axis. When off, sine."),
        param!("use_cos_y", "Use Cos Y", bool, false, "When on, use cosine on the Y axis. When off, sine."),
        param!("dampx", "Damp X", unlimited_float, 0.0, -10.0, 10.0, "X-axis exponential damping factor — the output is multiplied by `exp(dampx)` (or 1 when `|dampx|` is tiny)."),
        param!("dampy", "Damp Y", unlimited_float, 0.0, -10.0, 10.0, "Y-axis exponential damping factor — same as dampx but for Y."),
    ],
    init_param_count: 2,
    wgsl_init: Some(WAVES3_WF_INIT),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waves3_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let use_cos_x = i32(get_param(xform_id, variation_id, 4u));
    let use_cos_y = i32(get_param(xform_id, variation_id, 5u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);
    let tx_base = select(sin(p.y * freqx), cos(p.y * freqx), use_cos_x == 1);
    let ty_base = select(sin(p.x * freqy), cos(p.x * freqy), use_cos_y == 1);
    let tx = tx_base * tx_base;
    let ty = ty_base * ty_base;
    let nx = (p.x + dx * scalex * tx) * dx;
    let ny = (p.y + dy * scaley * ty) * dy;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_waves3_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let use_cos_x = i32(get_param(xform_id, variation_id, 4u));
    let use_cos_y = i32(get_param(xform_id, variation_id, 5u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);
    let tx_base = select(sin(p.y * freqx), cos(p.y * freqx), use_cos_x == 1);
    let ty_base = select(sin(p.x * freqy), cos(p.x * freqy), use_cos_y == 1);
    let tx = tx_base * tx_base;
    let ty = ty_base * ty_base;
    let nx = (p.x + dx * scalex * tx) * dx;
    let ny = (p.y + dy * scaley * ty) * dy;
    return vec3<f32>(nx, ny, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// waves4_wf
// ---------------------------------------------------------------------------

/// WF waves4 (triple-trig product perturbation) — same structure as
/// `waves2_wf` but the trig terms are triple products: `cos·sin·cos` when
/// `use_cos = 1`, `sin·cos·sin` when `use_cos = 0`. Produces a sharper,
/// more lobed wave shape.
///
/// # Authors
/// - Joel Faber
pub static WAVES4_WF: VariationDef = VariationDef {
    name: "waves4_wf",
    aliases: &[],
    display_name: "Waves 4 WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("scalex", "Scale X", unlimited_float, 0.25, -10.0, 10.0, "X-axis wave amplitude."),
        param!("scaley", "Scale Y", unlimited_float, 0.5, -10.0, 10.0, "Y-axis wave amplitude."),
        param!("freqx", "Freq X", unlimited_float, 1.5707963267948966, -100.0, 100.0, "X-axis wave frequency (applied to the Y input)."),
        param!("freqy", "Freq Y", unlimited_float, 0.7853981633974483, -100.0, 100.0, "Y-axis wave frequency (applied to the X input)."),
        param!("use_cos_x", "Use Cos X", bool, true, "When on, use cosine on the X axis. When off, sine."),
        param!("use_cos_y", "Use Cos Y", bool, false, "When on, use cosine on the Y axis. When off, sine."),
        param!("dampx", "Damp X", unlimited_float, 0.0, -10.0, 10.0, "X-axis exponential damping factor — the output is multiplied by `exp(dampx)` (or 1 when `|dampx|` is tiny)."),
        param!("dampy", "Damp Y", unlimited_float, 0.0, -10.0, 10.0, "Y-axis exponential damping factor — same as dampx but for Y."),
    ],
    init_param_count: 2,
    wgsl_init: Some(WAVES4_WF_INIT),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waves4_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let use_cos_x = i32(get_param(xform_id, variation_id, 4u));
    let use_cos_y = i32(get_param(xform_id, variation_id, 5u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);
    let cx = cos(p.y * freqx);
    let sx = sin(p.y * freqx);
    let cy = cos(p.x * freqy);
    let sy = sin(p.x * freqy);
    let tx = select(sx * cx * sx, cx * sx * cx, use_cos_x == 1);
    let ty = select(sy * cy * sy, cy * sy * cy, use_cos_y == 1);
    let nx = (p.x + dx * scalex * tx) * dx;
    let ny = (p.y + dy * scaley * ty) * dy;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_waves4_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let use_cos_x = i32(get_param(xform_id, variation_id, 4u));
    let use_cos_y = i32(get_param(xform_id, variation_id, 5u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);
    let cx = cos(p.y * freqx);
    let sx = sin(p.y * freqx);
    let cy = cos(p.x * freqy);
    let sy = sin(p.x * freqy);
    let tx = select(sx * cx * sx, cx * sx * cx, use_cos_x == 1);
    let ty = select(sy * cy * sy, cy * sy * cy, use_cos_y == 1);
    let nx = (p.x + dx * scalex * tx) * dx;
    let ny = (p.y + dy * scaley * ty) * dy;
    return vec3<f32>(nx, ny, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// dinis_surface_wf
// ---------------------------------------------------------------------------

/// Dini's Surface parametric mapping — emits `(a·cos(u)·sin(v),
/// a·sin(u)·sin(v), −(a·(cos(v) + log tan(v/2)) + b·u))` where `(u, v) =
/// (x, y)`. Dini's surface is a helicoid of constant negative Gaussian
/// curvature, a generalization of the pseudosphere. See [mathworld.wolfram.
/// com/DinisSurface](https://mathworld.wolfram.com/DinisSurface.html).
pub static DINIS_SURFACE_WF: VariationDef = VariationDef {
    name: "dinis_surface_wf",
    aliases: &[],
    display_name: "Dini's Surface WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("a", "A", unlimited_float, 0.8, -10.0, 10.0, "Radial scale of the surface (controls XY radius and Z magnitude)."),
        param!("b", "B", unlimited_float, 0.2, -10.0, 10.0, "Helical twist coefficient — multiplies the input U to add a linear helical Z offset."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_dinis_surface_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let u = p.x;
    let v = p.y;
    let sinv = sin(v);
    return vec2<f32>(a * cos(u) * sinv, a * sin(u) * sinv);
}
"#,
    wgsl_3d: r#"
fn variation_dinis_surface_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let u = p.x;
    let v = p.y;
    let sinv = sin(v);
    let tan_half = tan(v * 0.5);
    let log_tan = log(max(abs(tan_half), 1e-30));
    return vec3<f32>(
        a * cos(u) * sinv,
        a * sin(u) * sinv,
        -(a * (cos(v) + log_tan) + b * u),
    );
}
"#,
};
