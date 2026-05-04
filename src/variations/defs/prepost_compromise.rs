//! prepost_circlize, prepost_mobius (Maschke)
//!
//! Both are JWildfire "prepost" priority-2 variations that run BOTH
//! the pre-affine and post-affine inverse-of-each-other transforms in
//! the same flame iteration. Our model only supports pre OR post
//! phase, not both. **Compromise**: port each as a single normal-phase
//! variation that applies just the "post" half — this is the
//! transformation users typically want to see for visual effect.
//!
//!   - prepost_circlize: 3 user (n int 4, rotation 45, reverse int 0)
//!     + 2 init (_pi_n = π/n, _cospi_n = cos(_pi_n)). Body computes
//!     `factor = cos(π/n) / cos((θ - rot) - (π/n) · (2·floor(...)+1))`,
//!     then radial scale via `factor` (or `1/factor` based on reverse).
//!     `lerp(r, r/factor, VVAR)` becomes `r/factor` in our convention
//!     since we drop the VVAR factor (compromise — affects very low
//!     weight blending).
//!
//!   - prepost_mobius: 8 user (re_a/b/c/d, im_a/b/c/d), no init slots.
//!     Body is the post-phase Möbius transform `(a·z + b) / (c·z + d)`
//!     in complex form. Body factors cleanly (cpp's `rad_v = VVAR/d`
//!     becomes `1/d` and the outer multiplier handles VVAR).
//!
//! Sources:
//!   - `output/jwildfire-vars/output/prepost_circlize.cpp`
//!   - `output/jwildfire-vars/output/prepost_mobius.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// prepost_circlize (compromise: single normal-phase)
// ---------------------------------------------------------------------------

pub static PREPOST_CIRCLIZE: VariationDef = VariationDef {
    name: "prepost_circlize",
    display_name: "PrePost Circlize",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("n", "N", int, 4.0, 1.0, 50.0),
        param!("rotation", "Rotation", unlimited_float, 45.0, -360.0, 360.0),
        param!("reverse", "Reverse", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_prepost_circlize(user: array<f32, 3>) -> array<f32, 2> {
    let pi = 3.14159265358979;
    let n_safe = max(user[0], 1.0);
    var out: array<f32, 2>;
    out[0] = pi / n_safe;
    out[1] = cos(out[0]);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_prepost_circlize(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let n = max(get_param(xform_id, variation_id, 0u), 1.0);
    let reverse = i32(get_param(xform_id, variation_id, 2u));
    let pi_n = get_param(xform_id, variation_id, 3u);
    let cospi_n = get_param(xform_id, variation_id, 4u);
    let two_pi = 6.28318530717959;
    let rot_rad = 0.0;  // cpp init sets _rot_rad = 0.0; rotation param is unused in body.

    let theta = atan2(p.x, p.y);
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let theta_shift = theta - rot_rad;
    let inner = theta_shift - pi_n * (2.0 * floor((n * theta_shift) / two_pi) + 1.0);
    let cos_inner = cos(inner);
    let safe_cos = select(cos_inner, 1e-30, abs(cos_inner) < 1e-30);
    let factor = cospi_n / safe_cos;
    let r_out = select(r / factor, r * factor, reverse != 0);
    return vec2<f32>(r_out * cos(theta), r_out * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_prepost_circlize(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let n = max(get_param(xform_id, variation_id, 0u), 1.0);
    let reverse = i32(get_param(xform_id, variation_id, 2u));
    let pi_n = get_param(xform_id, variation_id, 3u);
    let cospi_n = get_param(xform_id, variation_id, 4u);
    let two_pi = 6.28318530717959;
    let rot_rad = 0.0;

    let theta = atan2(p.x, p.y);
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let theta_shift = theta - rot_rad;
    let inner = theta_shift - pi_n * (2.0 * floor((n * theta_shift) / two_pi) + 1.0);
    let cos_inner = cos(inner);
    let safe_cos = select(cos_inner, 1e-30, abs(cos_inner) < 1e-30);
    let factor = cospi_n / safe_cos;
    let r_out = select(r / factor, r * factor, reverse != 0);
    return vec3<f32>(r_out * cos(theta), r_out * sin(theta), p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// prepost_mobius (compromise: single normal-phase, post version)
// ---------------------------------------------------------------------------

pub static PREPOST_MOBIUS: VariationDef = VariationDef {
    name: "prepost_mobius",
    display_name: "PrePost Mobius",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("re_a", "Re A", unlimited_float, 1.0, -10.0, 10.0),
        param!("re_b", "Re B", unlimited_float, 0.0, -10.0, 10.0),
        param!("re_c", "Re C", unlimited_float, 0.0, -10.0, 10.0),
        param!("re_d", "Re D", unlimited_float, 1.0, -10.0, 10.0),
        param!("im_a", "Im A", unlimited_float, 0.0, -10.0, 10.0),
        param!("im_b", "Im B", unlimited_float, 0.0, -10.0, 10.0),
        param!("im_c", "Im C", unlimited_float, 0.0, -10.0, 10.0),
        param!("im_d", "Im D", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_prepost_mobius(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let re_a = get_param(xform_id, variation_id, 0u);
    let re_b = get_param(xform_id, variation_id, 1u);
    let re_c = get_param(xform_id, variation_id, 2u);
    let re_d = get_param(xform_id, variation_id, 3u);
    let im_a = get_param(xform_id, variation_id, 4u);
    let im_b = get_param(xform_id, variation_id, 5u);
    let im_c = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;
    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;
    let d = re_v * re_v + im_v * im_v;
    if (d < 1e-30) {
        return p;
    }
    let inv_d = 1.0 / d;
    return vec2<f32>(inv_d * (re_u * re_v + im_u * im_v),
                     inv_d * (im_u * re_v - re_u * im_v));
}
"#,
    wgsl_3d: Some(r#"
fn variation_prepost_mobius(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let re_a = get_param(xform_id, variation_id, 0u);
    let re_b = get_param(xform_id, variation_id, 1u);
    let re_c = get_param(xform_id, variation_id, 2u);
    let re_d = get_param(xform_id, variation_id, 3u);
    let im_a = get_param(xform_id, variation_id, 4u);
    let im_b = get_param(xform_id, variation_id, 5u);
    let im_c = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;
    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;
    let d = re_v * re_v + im_v * im_v;
    if (d < 1e-30) {
        return p;
    }
    let inv_d = 1.0 / d;
    return vec3<f32>(inv_d * (re_u * re_v + im_u * im_v),
                     inv_d * (im_u * re_v - re_u * im_v),
                     p.z);
}
"#),
};
