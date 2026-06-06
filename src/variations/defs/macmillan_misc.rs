//! macmillan
//!
//! Self-iterating McMillan-map walker. Like curliecue2, the variation
//! ignores its input position and emits a deterministic point sequence
//! driven by per-thread persistent state. Each call applies the
//! 2nd-order McMillan map twice:
//!
//!   y' = -xa + 2a · y/(1 + y²) + b·y
//!
//! Output is `(x, y)` (state values after the two-step map) plus a
//! direct-color write via `vc`, with TC = fmod(|0.5·(FPx+FPy+1)|, 1).
//!
//! 5 user params (a, b, N, startx, starty) + 3 state slots
//! (xa, x, y) + custom init (xa=startx, x=startx, y=starty).
//!
//! First port using all of `needs_accum + needs_transform +
//! writes_color + wgsl_state_init`. The transform read provides the
//! variation's own weight so we can replicate cpp's `TC = ...(FPx +
//! FPy)` *after* its own contribution is added, which in our model
//! becomes `TC = ...((accum + weight·own).x + (accum + weight·own).y)`.
//!
//! Source: `output/jwildfire-vars/output/macmillan.cpp`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Self-iterating McMillan-map walker. The McMillan map is the discrete
/// dynamical system `y' = -x + 2a · y/(1+y²) + b·y` (from Edwin McMillan's
/// 1971 nonlinear-dynamics work); each iteration applies the 2nd-order form
/// twice from a per-thread state `(xa, x, y)` initialized to `(startx,
/// startx, starty)`. Ignores the input position and emits the post-
/// iteration `(x, y)` as a deterministic trajectory; also writes a color
/// register `vc` derived from the running accumulator plus this iteration's
/// own contribution. First port to exercise the full feature set
/// (`needs_accum + needs_transform + writes_color + wgsl_state_init`).
pub static MACMILLAN: VariationDef = VariationDef {
    name: "macmillan",
    aliases: &[],
    display_name: "MacMillan",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform, Feature::WritesColor, Feature::NeedsAccum],
    parameters: &[
        param!("a", "A", unlimited_float, 1.6, -10.0, 10.0, "McMillan-map parameter `a` — coefficient of the nonlinear term `2a · y/(1+y²)`. The classic chaotic regime is around `a ≈ 1.6`."),
        param!("b", "B", unlimited_float, 0.4, -10.0, 10.0, "McMillan-map parameter `b` — linear coefficient on `y`. Together with `a` determines whether the trajectory is bounded, periodic, or chaotic."),
        param!("N", "N", unlimited_float, 1.0, -10.0, 10.0, "Declared but unused in the body — the map is hardcoded to two iterations per call. Possibly a port omission (upstream cpp may use `N` as an inner-loop bound). Tracked in TODO."),
        param!("startx", "Start X", unlimited_float, 0.1, -10.0, 10.0, "Initial X coordinate for the per-thread state (read once via `wgsl_state_init` at thread start). Sets both `xa` and `x` slots."),
        param!("starty", "Start Y", unlimited_float, 0.1, -10.0, 10.0, "Initial Y coordinate for the per-thread state."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 3,
    wgsl_state_init: Some(
        "        let startx = get_param(xform_id, variation_id, 3u);\n\
         \x20       let starty = get_param(xform_id, variation_id, 4u);\n\
         \x20       set_state(xform_id, variation_id, 0u, startx);\n\
         \x20       set_state(xform_id, variation_id, 1u, startx);\n\
         \x20       set_state(xform_id, variation_id, 2u, starty);"
    ),
    wgsl_2d: r#"
fn variation_macmillan(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);

    var xa = get_state(xform_id, variation_id, 0u);
    var x = get_state(xform_id, variation_id, 1u);
    var y = get_state(xform_id, variation_id, 2u);

    // First McMillan iteration: x = y; y = map(y, xa, a, b); xa = old x.
    x = y;
    y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;
    xa = x;
    // Second McMillan iteration: x = y; y = map(y, new xa, a, b).
    x = y;
    y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;

    // TC reads "accum + own contribution" (cpp's FPx/FPy after VVAR*x added).
    let weight = transforms[xform_id].variations[variation_id];
    let fpx = accum.x + weight * x;
    let fpy = accum.y + weight * y;
    let s = abs(0.5 * (fpx + fpy + 1.0));
    *vc = s - floor(s);

    // Save state. cpp's FINAL "xa = x" overrides the earlier "xa = old x".
    set_state(xform_id, variation_id, 0u, x);
    set_state(xform_id, variation_id, 1u, x);
    set_state(xform_id, variation_id, 2u, y);

    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_macmillan(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);

    var xa = get_state(xform_id, variation_id, 0u);
    var x = get_state(xform_id, variation_id, 1u);
    var y = get_state(xform_id, variation_id, 2u);

    x = y;
    y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;
    xa = x;
    x = y;
    y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;

    let weight = transforms[xform_id].variations[variation_id];
    let fpx = accum.x + weight * x;
    let fpy = accum.y + weight * y;
    let s = abs(0.5 * (fpx + fpy + 1.0));
    *vc = s - floor(s);

    set_state(xform_id, variation_id, 0u, x);
    set_state(xform_id, variation_id, 1u, x);
    set_state(xform_id, variation_id, 2u, y);

    return vec3<f32>(x, y, p.z);
}
"#,
};
