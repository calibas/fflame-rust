//! farblur (zephyrtronium)
//!
//! "Far blur" — scales the radial blur magnitude by the squared
//! distance of the running accumulator from a configurable origin,
//! so points further from origin get more blur. Uses a rolling buffer
//! of 4 random samples to drive a slowly-varying signed scale.
//!
//! Reads the variation accumulator (cpp's `FPx/FPy/FPz`), so this is
//! the first port using `needs_accum`. The framework passes the
//! current `result` value (sum of contributions from prior variations
//! in this iteration) as the `accum` parameter.
//!
//! State (5 slots, zero-initialized): `_r[0..3]` rolling random
//! buffer + `_n` index (0..4 cycling). The cpp version seeds `_r`
//! with 4 GOODRAND_01 calls in Prepare; we accept the zero-init bias
//! for the first ~4 iterations (negligible vs typical 4096+ iter
//! dispatches) rather than implementing wgsl_state_init for one port.
//!
//! In 2D the cpp formula's `FPz` term reduces to `(0 - z_origin)²`
//! since our 2D accum is vec2 — the z_origin contribution becomes a
//! constant offset on `r`. The output drops the z component entirely
//! (no preserve-z semantics in 2D).
//!
//! Source: `output/jwildfire-vars/output/farblur.cpp`.
//! Reference: zephyrtronium's deviantArt page (now defunct).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Far-distance random blur — emits a spherical-coordinate random offset
/// whose magnitude scales with the squared distance of the running
/// accumulator from a configurable origin, so points further from the
/// origin receive proportionally more blur. The scale factor is modulated
/// by a rolling buffer of 4 random samples (`r[0..3]`), refreshed one slot
/// per iteration; their sum minus 2 gives a slowly-varying signed
/// multiplier. In 2D the missing Z accumulator collapses the `dz` term to a
/// constant `(0 - z_origin)²`, so `z_origin` becomes a constant additive
/// offset on the radial magnitude.
///
/// # Authors
/// - zephyrtronium
pub static FARBLUR: VariationDef = VariationDef {
    name: "farblur",
    aliases: &[],
    display_name: "Far Blur",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsAccum, Feature::AlwaysZ],
    parameters: &[
        param!("x", "X Scale", unlimited_float, 1.0, -10.0, 10.0, "X-axis blur scale. Multiplies the X component of the random spherical offset."),
        param!("y", "Y Scale", unlimited_float, 1.0, -10.0, 10.0, "Y-axis blur scale."),
        param!("z", "Z Scale", unlimited_float, 1.0, -10.0, 10.0, "Z-axis blur scale (3D only)."),
        param!("x_origin", "X Origin", unlimited_float, 0.0, -10.0, 10.0, "Origin X coordinate. The accumulator's distance from this point (squared) modulates the blur magnitude — larger distance → more blur."),
        param!("y_origin", "Y Origin", unlimited_float, 0.0, -10.0, 10.0, "Origin Y coordinate."),
        param!("z_origin", "Z Origin", unlimited_float, 0.0, -10.0, 10.0, "Origin Z coordinate. In 2D mode this becomes a constant offset (the 2D accumulator has no Z, so `dz` reduces to `-z_origin`)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 5,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_farblur(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let x_param = get_param(xform_id, variation_id, 0u);
    let y_param = get_param(xform_id, variation_id, 1u);
    let x_origin = get_param(xform_id, variation_id, 3u);
    let y_origin = get_param(xform_id, variation_id, 4u);
    let z_origin = get_param(xform_id, variation_id, 5u);

    let r0 = get_state(xform_id, variation_id, 0u);
    let r1 = get_state(xform_id, variation_id, 1u);
    let r2 = get_state(xform_id, variation_id, 2u);
    let r3 = get_state(xform_id, variation_id, 3u);
    let n_idx = get_state(xform_id, variation_id, 4u);

    let dx = accum.x - x_origin;
    let dy = accum.y - y_origin;
    let dz = -z_origin;  // 2D: FPz = 0
    let r = (dx * dx + dy * dy + dz * dz) * (r0 + r1 + r2 + r3 - 2.0);

    // Roll: replace _r[n_idx] with a fresh sample, advance n_idx.
    let new_sample = rng_nextf(rng);
    let n = u32(n_idx) & 3u;
    set_state(xform_id, variation_id, n, new_sample);
    set_state(xform_id, variation_id, 4u, f32((n + 1u) & 3u));

    let u = rng_nextf(rng) * two_pi;
    let v = rng_nextf(rng) * two_pi;
    let sv = sin(v);
    return vec2<f32>(x_param * r * sv * cos(u), y_param * r * sv * sin(u));
}
"#,
    wgsl_3d: r#"
fn variation_farblur(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let x_param = get_param(xform_id, variation_id, 0u);
    let y_param = get_param(xform_id, variation_id, 1u);
    let z_param = get_param(xform_id, variation_id, 2u);
    let x_origin = get_param(xform_id, variation_id, 3u);
    let y_origin = get_param(xform_id, variation_id, 4u);
    let z_origin = get_param(xform_id, variation_id, 5u);

    let r0 = get_state(xform_id, variation_id, 0u);
    let r1 = get_state(xform_id, variation_id, 1u);
    let r2 = get_state(xform_id, variation_id, 2u);
    let r3 = get_state(xform_id, variation_id, 3u);
    let n_idx = get_state(xform_id, variation_id, 4u);

    let dx = accum.x - x_origin;
    let dy = accum.y - y_origin;
    let dz = accum.z - z_origin;
    let r = (dx * dx + dy * dy + dz * dz) * (r0 + r1 + r2 + r3 - 2.0);

    let new_sample = rng_nextf(rng);
    let n = u32(n_idx) & 3u;
    set_state(xform_id, variation_id, n, new_sample);
    set_state(xform_id, variation_id, 4u, f32((n + 1u) & 3u));

    let u = rng_nextf(rng) * two_pi;
    let v = rng_nextf(rng) * two_pi;
    let sv = sin(v);
    let cv = cos(v);
    return vec3<f32>(x_param * r * sv * cos(u), y_param * r * sv * sin(u), z_param * r * cv);
}
"#,
};
