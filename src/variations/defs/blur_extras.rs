//! Additional blur variations
//!
//! Three small RNG-based blur variations from upstream's blur cluster:
//!   - `sineblur`     (Zyorg)             — radial sine-density blur
//!   - `starblur`     (Zyorg)             — N-pointed star-shape blur
//!   - `r_circleblur` (Zabanova/Stefanov) — radial-truncated circle blur
//!
//! Sources:
//!   - output/jwildfire-vars/output/sineblur.cpp
//!   - output/jwildfire-vars/output/starblur.cpp
//!   - output/jwildfire-vars/output/r_circleblur.cpp
//!
//! Notes on faithfulness:
//!   - All three have VVAR strictly as an outer multiplier on the X/Y
//!     output (`FPx += VVAR * stuff`), so they factor cleanly through
//!     our outer-multiplier convention with no `needs_transform` needed.
//!
//! Skipped from this batch (architectural):
//!   - `farblur`, `exblur`, `nblur` — all use a persistent `_r[4]` array
//!     of recent random draws as a circular buffer, updating one slot
//!     per iteration. Our shader has no per-variation per-thread
//!     persistent state; would need infrastructure work.
//!   - `farblur` additionally reads the running `FPx`/`FPy` accumulator
//!     mid-iteration (post-variations-up-to-this-point), which our
//!     normal-phase calling convention doesn't expose.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// sineblur: Zyorg's sine-density radial blur
//   power_clamped = max(power, 0)
//   ang = rand · 2π
//   r   = (power == 1.0) ? acos(2·rand − 1) / π
//                        : acos(2 · exp(power · log(rand)) − 1) / π
//   out = (r · cos ang, r · sin ang)
// (weight applied outside)
// =============================================================================
pub static SINEBLUR: VariationDef = VariationDef {
    name: "sineblur",
    display_name: "Sine Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", unlimited_float, 1.0, 0.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sineblur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power_in = get_param(xform_id, variation_id, 0u);
    let power = max(power_in, 0.0);
    let inv_pi = 0.3183098861837907;

    let ang = rng_nextf(rng) * 6.28318530717959;
    var r: f32;
    if (power == 1.0) {
        r = acos(rng_nextf(rng) * 2.0 - 1.0) * inv_pi;
    } else {
        let u = max(rng_nextf(rng), 1e-30);
        r = acos(exp(log(u) * power) * 2.0 - 1.0) * inv_pi;
    }
    return vec2<f32>(r * cos(ang), r * sin(ang));
}
"#,
    wgsl_3d: Some(r#"
fn variation_sineblur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power_in = get_param(xform_id, variation_id, 0u);
    let power = max(power_in, 0.0);
    let inv_pi = 0.3183098861837907;

    let ang = rng_nextf(rng) * 6.28318530717959;
    var r: f32;
    if (power == 1.0) {
        r = acos(rng_nextf(rng) * 2.0 - 1.0) * inv_pi;
    } else {
        let u = max(rng_nextf(rng), 1e-30);
        r = acos(exp(log(u) * power) * 2.0 - 1.0) * inv_pi;
    }
    return vec3<f32>(r * cos(ang), r * sin(ang), p.z);
}
"#),
};

// =============================================================================
// starblur: Zyorg's N-pointed star blur
//   alpha₀ = π / power
//   length = sqrt(1 + range² − 2·range·cos(alpha₀))
//   alpha  = asin(sin(alpha₀) · range / length)
//   Body:  picks a random arm, parameterizes a point along the arm's
//          inner edge, then jitters radially with sqrt(rand) for area-
//          uniform distribution.
//
// Init computes alpha and length from (power, range); we keep both
// as float user params (the C++ `power` is int but we store as f32).
// =============================================================================
pub static STARBLUR: VariationDef = VariationDef {
    name: "starblur",
    display_name: "Star Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", int, 5.0, 2.0, 50.0),
        param!("range", "Range", unlimited_float, 0.40162283, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 2..4:
    //   2: alpha   asin(sin(π/power) · range / length)
    //   3: length  sqrt(1 + range² − 2·range·cos(π/power))
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_starblur(user: array<f32, 2>) -> array<f32, 2> {
    let power = max(user[0], 1.0);
    let range = user[1];
    let alpha0 = 3.14159265358979 / power;
    let length = sqrt(max(1.0 + range * range - 2.0 * range * cos(alpha0), 1e-30));
    var out: array<f32, 2>;
    out[0] = asin(sin(alpha0) * range / length);
    out[1] = length;
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_starblur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let alpha = get_param(xform_id, variation_id, 2u);
    let length = get_param(xform_id, variation_id, 3u);
    let two_pi = 6.28318530717959;
    let half_pi = 1.5707963267948966;

    let safe_power = max(power, 1.0);
    var f = rng_nextf(rng) * safe_power * 2.0;
    let arm = trunc(f);
    f = f - arm;
    let x = f * length;
    let z0 = sqrt(max(1.0 + x * x - 2.0 * x * cos(alpha), 1e-30));
    let arm_i = i32(arm);
    let arm_pair = f32(arm_i / 2);
    var ang_pt: f32;
    if (arm_i % 2 == 0) {
        ang_pt = two_pi / safe_power * arm_pair + asin(sin(alpha) * x / z0);
    } else {
        ang_pt = two_pi / safe_power * arm_pair - asin(sin(alpha) * x / z0);
    }
    let z = z0 * sqrt(rng_nextf(rng));
    let ang = ang_pt - half_pi;
    return vec2<f32>(z * cos(ang), z * sin(ang));
}
"#,
    wgsl_3d: Some(r#"
fn variation_starblur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let alpha = get_param(xform_id, variation_id, 2u);
    let length = get_param(xform_id, variation_id, 3u);
    let two_pi = 6.28318530717959;
    let half_pi = 1.5707963267948966;

    let safe_power = max(power, 1.0);
    var f = rng_nextf(rng) * safe_power * 2.0;
    let arm = trunc(f);
    f = f - arm;
    let x = f * length;
    let z0 = sqrt(max(1.0 + x * x - 2.0 * x * cos(alpha), 1e-30));
    let arm_i = i32(arm);
    let arm_pair = f32(arm_i / 2);
    var ang_pt: f32;
    if (arm_i % 2 == 0) {
        ang_pt = two_pi / safe_power * arm_pair + asin(sin(alpha) * x / z0);
    } else {
        ang_pt = two_pi / safe_power * arm_pair - asin(sin(alpha) * x / z0);
    }
    let z = z0 * sqrt(rng_nextf(rng));
    let ang = ang_pt - half_pi;
    return vec3<f32>(z * cos(ang), z * sin(ang), p.z);
}
"#),
};

// =============================================================================
// r_circleblur: Zabanova's radial-truncated circle blur
//   Truncates the input radius modulo |n|, picks a per-cell hash to
//   choose circle position, then samples a small disc within that cell
//   with size driven by the user `min`/`max` range and `dist` jitter.
// =============================================================================
pub static R_CIRCLEBLUR: VariationDef = VariationDef {
    name: "r_circleblur",
    display_name: "R Circle Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("n", "N", unlimited_float, 1.0, 0.1, 50.0),
        param!("seed", "Seed", unlimited_float, 0.0, -100.0, 100.0),
        param!("dist", "Dist", unlimited_float, 0.5, 0.0, 5.0),
        param!("min", "Min", float, 0.1, 0.0, 1.0),
        param!("max", "Max", float, 1.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_r_circleblur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let seed = get_param(xform_id, variation_id, 1u);
    let dist = get_param(xform_id, variation_id, 2u);
    let min_v = get_param(xform_id, variation_id, 3u);
    let max_v = get_param(xform_id, variation_id, 4u);

    let rcn = max(abs(n), 1e-6);
    let angle = atan2(p.y, p.x);
    var rad = sqrt(p.x * p.x + p.y * p.y);
    rad = rad - floor(rad / rcn) * rcn;
    var by = sin(angle + rad);
    var bx = cos(angle + rad);
    by = round(by * rad);
    bx = round(bx * rad);

    let rad2 = sqrt(rng_nextf(rng)) * 0.5;
    let angle2 = rng_nextf(rng) * 6.28318530717959;
    var a1 = sin(bx * 127.1 + by * 311.7 + seed) * 43758.5453;
    a1 = a1 - trunc(a1);
    var a2 = sin(bx * 269.5 + by * 183.3 + seed) * 43758.5453;
    a2 = a2 - trunc(a2);
    var a3 = sin(by * 12.9898 + bx * 78.233 + seed) * 43758.5453;
    a3 = a3 - trunc(a3);
    a3 = a3 * (max_v - min_v) + min_v;
    let rad3 = rad2 * a3;
    by = by + rad3 * sin(angle2) + a1 * dist;
    bx = bx + rad3 * cos(angle2) + a2 * dist;

    return vec2<f32>(bx, by);
}
"#,
    wgsl_3d: Some(r#"
fn variation_r_circleblur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let seed = get_param(xform_id, variation_id, 1u);
    let dist = get_param(xform_id, variation_id, 2u);
    let min_v = get_param(xform_id, variation_id, 3u);
    let max_v = get_param(xform_id, variation_id, 4u);

    let rcn = max(abs(n), 1e-6);
    let angle = atan2(p.y, p.x);
    var rad = sqrt(p.x * p.x + p.y * p.y);
    rad = rad - floor(rad / rcn) * rcn;
    var by = sin(angle + rad);
    var bx = cos(angle + rad);
    by = round(by * rad);
    bx = round(bx * rad);

    let rad2 = sqrt(rng_nextf(rng)) * 0.5;
    let angle2 = rng_nextf(rng) * 6.28318530717959;
    var a1 = sin(bx * 127.1 + by * 311.7 + seed) * 43758.5453;
    a1 = a1 - trunc(a1);
    var a2 = sin(bx * 269.5 + by * 183.3 + seed) * 43758.5453;
    a2 = a2 - trunc(a2);
    var a3 = sin(by * 12.9898 + bx * 78.233 + seed) * 43758.5453;
    a3 = a3 - trunc(a3);
    a3 = a3 * (max_v - min_v) + min_v;
    let rad3 = rad2 * a3;
    by = by + rad3 * sin(angle2) + a1 * dist;
    bx = bx + rad3 * cos(angle2) + a2 * dist;

    return vec3<f32>(bx, by, p.z);
}
"#),
};
