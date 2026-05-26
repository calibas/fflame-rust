//! Apophysis miscellany 17 — two more
//!
//!   - `loq`           (zephyrtronium)       — 1 user `base` (default
//!                                                e); 3D quaternion-Apo
//!                                                inverse-log; body has
//!                                                w in `C` and `denom`;
//!                                                needs_transform
//!                                                divide-out
//!   - `spirograph3D`  (?, Java-recovered)    — 7 user (a, b, c, tmin,
//!                                                tmax, width, mode int);
//!                                                RNG; Full3D; cpp
//!                                                PluginVarCalc empty
//!                                                unported_stub. Five
//!                                                width modes (uniform,
//!                                                phased-sin, 3-uniform,
//!                                                4-sum-Gaussian, ±width
//!                                                binary). Clean factor
//!                                                through outer.
//!                                                cpp's `direct_color`
//!                                                flag dropped — adding
//!                                                conditional `vc`
//!                                                writes complicates
//!                                                the writes_color
//!                                                model; can be added
//!                                                later if needed
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// loq (zephyrtronium)
//   abs_v = sqrt(y² + z²)
//   C = atan2(abs_v, x) / abs_v
//   denom = 0.5 / log(base)
//   fx_cpp = log(x² + abs_v²) · w · denom
//   fy_cpp = w · C · y
//   fz_cpp = w · C · z
// needs_transform divide-out: body returns cpp_output / w; outer ×
// w restores.
// =============================================================================
/// 3D quaternion-Apo inverse-log — emits `(log(r²) / (2·log(base)), C·y,
/// C·z)` where `C = atan2(|v|, x) / |v|` and `|v| = sqrt(y² + z²)`. Inverse
/// of the `exp`/`q_log` family with a configurable log base. The X output
/// is logarithmic; the Y and Z components scale by an angular factor.
///
/// # Authors
/// - zephyrtronium
pub static LOQ: VariationDef = VariationDef {
    name: "loq",
    display_name: "Loq",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("base", "Base", unlimited_float, 2.7182818284590452, 1.01, 100.0, "Log base for the X-output normalization. Defaults to e."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_loq(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let base = max(get_param(xform_id, variation_id, 0u), 1.000001);
    // 2D form: abs_v = |y|; matches cpp with FTz = 0.
    let abs_v = max(abs(p.y), 1e-30);
    let denom = 0.5 / log(base);
    let c_val = atan2(abs_v, p.x) / abs_v;
    let log_arg = max(p.x * p.x + abs_v * abs_v, 1e-30);
    let fx = log(log_arg) * denom;
    let fy = c_val * p.y;
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_loq(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let base = max(get_param(xform_id, variation_id, 0u), 1.000001);
    let abs_v = max(sqrt(p.y * p.y + p.z * p.z), 1e-30);
    let denom = 0.5 / log(base);
    let c_val = atan2(abs_v, p.x) / abs_v;
    let log_arg = max(p.x * p.x + abs_v * abs_v, 1e-30);
    let fx = log(log_arg) * denom;
    let fy = c_val * p.y;
    let fz = c_val * p.z;
    return vec3<f32>(fx, fy, fz);
}
"#),
};

// =============================================================================
// spirograph3D (Java-recovered, cpp PluginVarCalc empty)
//   t = (tmax − tmin) · rand + tmin
//   per `mode`:
//     0: w1 = w2 = w3 = width · rand − width/2
//     1: w1 = width · rand − width/2
//        w2 = w1 · sin(36t + 2π/3)
//        w3 = w1 · sin(36t + 4π/3)
//        w1 = w1 · sin(36t)
//     2: w1, w2, w3 each = width · rand − width/2 (independent)
//     3: w1, w2, w3 each = width · (sum-of-4-uniforms − 2) / 2
//        (Gaussian via central limit)
//     4: w1 = ±width; w2 = w3 = 0
//   x1 = (a+b)·cos(t) − c·cos((a+b)/b·t)
//   y1 = (a+b)·sin(t) − c·sin((a+b)/b·t)
//   z1 = c · sin((a+b)/b · t)
//   out = (x1 + w1, y1 + w2, z1 + w3)
// 7 user; RNG; Full3D; clean factor through outer.
// =============================================================================
/// 3D spirograph curve sampler — picks a random `t ∈ [tmin, tmax]` and
/// emits a 3D spirograph trajectory `((a+b)·cos(t) − c·cos((a+b)/b·t),
/// (a+b)·sin(t) − c·sin((a+b)/b·t), c·sin((a+b)/b·t))`. The `mode`
/// parameter selects one of 5 width-jitter patterns added to each axis.
pub static SPIROGRAPH3D: VariationDef = VariationDef {
    name: "spirograph3D",
    display_name: "Spirograph 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Outer wheel radius."),
        param!("b", "B", unlimited_float, -0.3, -10.0, 10.0, "Inner wheel radius (signed; negative produces an internal spirograph)."),
        param!("c", "C", unlimited_float, 0.4, -10.0, 10.0, "Pen-arm length."),
        param!("tmin", "T Min", unlimited_float, 0.0, -1000.0, 1000.0, "Minimum value of the random parameter `t`."),
        param!("tmax", "T Max", unlimited_float, 1000.0, -10000.0, 10000.0, "Maximum value of the random parameter `t`."),
        param!("width", "Width", unlimited_float, 0.0, -10.0, 10.0, "Per-axis jitter magnitude (interpretation depends on `mode`)."),
        param!("mode", "Mode", enum, 0, &["Uniform", "Phased Sin", "Independent", "Gaussian", "X Only"], "Jitter pattern: 0 = single uniform on all axes, 1 = phased-sin per axis, 2 = independent uniform per axis, 3 = central-limit Gaussian per axis, 4 = ±width on X only."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_spirograph3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c_p = get_param(xform_id, variation_id, 2u);
    let tmin = get_param(xform_id, variation_id, 3u);
    let tmax = get_param(xform_id, variation_id, 4u);
    let width = get_param(xform_id, variation_id, 5u);
    let mode = i32(get_param(xform_id, variation_id, 6u));
    let two_pi = 6.28318530717959;

    let t = (tmax - tmin) * rng_nextf(rng) + tmin;
    var w1: f32 = 0.0;
    var w2: f32 = 0.0;
    if (mode == 0) {
        let r = width * rng_nextf(rng) - width * 0.5;
        w1 = r; w2 = r;
    } else if (mode == 1) {
        let r = width * rng_nextf(rng) - width * 0.5;
        w2 = r * sin(36.0 * t + two_pi / 3.0);
        w1 = r * sin(36.0 * t);
    } else if (mode == 2) {
        w1 = width * rng_nextf(rng) - width * 0.5;
        w2 = width * rng_nextf(rng) - width * 0.5;
    } else if (mode == 3) {
        w1 = width * (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
        w2 = width * (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
    } else if (mode == 4) {
        w1 = select(-width, width, rng_nextf(rng) < 0.5);
        w2 = 0.0;
    }

    let safe_b = select(b, 1e-30, abs(b) < 1e-30);
    let ab = a + b;
    let abdb = ab / safe_b;
    let x1 = ab * cos(t) - c_p * cos(abdb * t);
    let y1 = ab * sin(t) - c_p * sin(abdb * t);
    return vec2<f32>(x1 + w1, y1 + w2);
}
"#,
    wgsl_3d: Some(r#"
fn variation_spirograph3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c_p = get_param(xform_id, variation_id, 2u);
    let tmin = get_param(xform_id, variation_id, 3u);
    let tmax = get_param(xform_id, variation_id, 4u);
    let width = get_param(xform_id, variation_id, 5u);
    let mode = i32(get_param(xform_id, variation_id, 6u));
    let two_pi = 6.28318530717959;

    let t = (tmax - tmin) * rng_nextf(rng) + tmin;
    var w1: f32 = 0.0;
    var w2: f32 = 0.0;
    var w3: f32 = 0.0;
    if (mode == 0) {
        let r = width * rng_nextf(rng) - width * 0.5;
        w1 = r; w2 = r; w3 = r;
    } else if (mode == 1) {
        let r = width * rng_nextf(rng) - width * 0.5;
        w2 = r * sin(36.0 * t + two_pi / 3.0);
        w3 = r * sin(36.0 * t + 2.0 * two_pi / 3.0);
        w1 = r * sin(36.0 * t);
    } else if (mode == 2) {
        w1 = width * rng_nextf(rng) - width * 0.5;
        w2 = width * rng_nextf(rng) - width * 0.5;
        w3 = width * rng_nextf(rng) - width * 0.5;
    } else if (mode == 3) {
        w1 = width * (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
        w2 = width * (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
        w3 = width * (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
    } else if (mode == 4) {
        w1 = select(-width, width, rng_nextf(rng) < 0.5);
        w2 = 0.0; w3 = 0.0;
    }

    let safe_b = select(b, 1e-30, abs(b) < 1e-30);
    let ab = a + b;
    let abdb = ab / safe_b;
    let x1 = ab * cos(t) - c_p * cos(abdb * t);
    let y1 = ab * sin(t) - c_p * sin(abdb * t);
    let z1 = c_p * sin(abdb * t);
    return vec3<f32>(x1 + w1, y1 + w2, z1 + w3);
}
"#),
};
