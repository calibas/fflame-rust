//! Circle / blur distortion family
//!
//! Four small circle-related distortion and blur primitives:
//!   - `circleblur`  (Zyorg)        — pure RNG blur on the unit disc
//!   - `circlesplit` (Zabanova)     — radius-thresholded outward push
//!   - `flipcircle`  (Faber)        — Y-flip outside a weight-sized radius
//!   - `blur_linear` (Faber/DarkBeam) — directional linear-segment blur
//!
//! Sources:
//!   - output/jwildfire-vars/output/circleblur.cpp
//!   - output/jwildfire-vars/output/circlesplit.cpp
//!   - output/jwildfire-vars/output/flipcircle.cpp
//!   - output/jwildfire-vars/output/blur_linear.cpp
//!
//! Notes:
//!   - All factor VVAR through the outer-multiplier convention.
//!   - `flipcircle` uses VVAR (the variation weight) as the radius of its
//!     flip threshold (`r² > VVAR²`). We expose the weight to the body via
//!     `needs_transform: true` rather than hard-coding weight=1.
//!   - `blur_linear` uses an init step to precompute sin/cos of the angle
//!     parameter once per flame setup.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// circleblur: uniform random sample inside the unit disc (Zyorg)
//   rad = sqrt(rand)            (sqrt for area-uniform distribution)
//   ang = rand · 2π
//   out = (cos ang · rad, sin ang · rad)
// =============================================================================
pub static CIRCLEBLUR: VariationDef = VariationDef {
    name: "circleblur",
    display_name: "Circle Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_circleblur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let rad = sqrt(rng_nextf(rng));
    let ang = rng_nextf(rng) * 6.28318530717959;
    return vec2<f32>(cos(ang) * rad, sin(ang) * rad);
}
"#,
    wgsl_3d: Some(r#"
fn variation_circleblur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let rad = sqrt(rng_nextf(rng));
    let ang = rng_nextf(rng) * 6.28318530717959;
    return vec3<f32>(cos(ang) * rad, sin(ang) * rad, p.z);
}
"#),
};

// =============================================================================
// circlesplit: pass-through inside r < (radius - split), pushed outward by
// `split` outside it (Zabanova / Stefanov)
//   r = sqrt(x² + y²)
//   if r < radius - split:
//       (x', y') = (x, y)
//   else:
//       (x', y') = (cos α · (r + split), sin α · (r + split))
//
// Effect: punches a circular "ring" wider by `split` around the radius
// boundary, splitting the inner and outer regions visually.
// =============================================================================
pub static CIRCLESPLIT: VariationDef = VariationDef {
    name: "circlesplit",
    display_name: "Circle Split",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, 0.0, 5.0),
        param!("split", "Split", unlimited_float, 0.5, 0.0, 5.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_circlesplit(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let split = get_param(xform_id, variation_id, 1u);
    let r = sqrt(p.x * p.x + p.y * p.y);
    if (r < radius - split) {
        return p;
    }
    let a = atan2(p.y, p.x);
    let len = r + split;
    return vec2<f32>(cos(a) * len, sin(a) * len);
}
"#,
    wgsl_3d: Some(r#"
fn variation_circlesplit(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let split = get_param(xform_id, variation_id, 1u);
    let r = sqrt(p.x * p.x + p.y * p.y);
    if (r < radius - split) {
        return p;
    }
    let a = atan2(p.y, p.x);
    let len = r + split;
    return vec3<f32>(cos(a) * len, sin(a) * len, p.z);
}
"#),
};

// =============================================================================
// flipcircle: Y-flip outside a radius-VVAR disc (Faber)
//   if x² + y² > VVAR²:  out = (x,  y)        (no flip)
//   else:                 out = (x, -y)        (flip Y inside the disc)
//
// To match upstream's r-vs-VVAR comparison faithfully, we read the
// variation's per-transform weight via `needs_transform: true`.
// Comparison threshold scales with weight; output magnitude factors
// through the outer multiplier as usual.
// =============================================================================
pub static FLIPCIRCLE: VariationDef = VariationDef {
    name: "flipcircle",
    display_name: "Flip Circle",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_flipcircle(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inside = (p.x * p.x + p.y * p.y) <= w * w;
    var y_out = p.y;
    if (inside) {
        y_out = -p.y;
    }
    return vec2<f32>(p.x, y_out);
}
"#,
    wgsl_3d: Some(r#"
fn variation_flipcircle(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inside = (p.x * p.x + p.y * p.y) <= w * w;
    var y_out = p.y;
    if (inside) {
        y_out = -p.y;
    }
    return vec3<f32>(p.x, y_out, p.z);
}
"#),
};

// =============================================================================
// blur_linear: directional linear-segment blur (Joel Faber / DarkBeam)
//   r = length · rand
//   out = (x + r·cos(angle), y + r·sin(angle))
// (sin/cos of `angle` precomputed once via init.)
// =============================================================================
pub static BLUR_LINEAR: VariationDef = VariationDef {
    name: "blur_linear",
    display_name: "Blur Linear",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("length", "Length", unlimited_float, 1.0, 0.0, 10.0),
        param!("angle", "Angle (rad)", unlimited_float, 0.5, -6.28318, 6.28318),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 2..4:
    //   2: cos_angle
    //   3: sin_angle
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_blur_linear(user: array<f32, 2>) -> array<f32, 2> {
    let angle = user[1];
    var out: array<f32, 2>;
    out[0] = cos(angle);
    out[1] = sin(angle);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_blur_linear(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let length_p = get_param(xform_id, variation_id, 0u);
    let cos_a = get_param(xform_id, variation_id, 2u);
    let sin_a = get_param(xform_id, variation_id, 3u);
    let r = length_p * rng_nextf(rng);
    return vec2<f32>(p.x + r * cos_a, p.y + r * sin_a);
}
"#,
    wgsl_3d: Some(r#"
fn variation_blur_linear(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let length_p = get_param(xform_id, variation_id, 0u);
    let cos_a = get_param(xform_id, variation_id, 2u);
    let sin_a = get_param(xform_id, variation_id, 3u);
    let r = length_p * rng_nextf(rng);
    return vec3<f32>(p.x + r * cos_a, p.y + r * sin_a, p.z);
}
"#),
};
