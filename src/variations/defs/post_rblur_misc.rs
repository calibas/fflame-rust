//! `post_rblur` (Xyrus02)
//!
//! Post-phase radial blur: jitters the accumulator point by an
//! amount proportional to its distance from `(center_x, center_y)`
//! minus `offset`, scaled by `2·strength`. Uses two RNG calls.
//!
//!   - 4 user params: strength, offset, center_x, center_y
//!   - 1 init slot: s2 = 2·strength
//!
//! Body uses cpp's `FPx = w · (FPx + jitter·r)` (assignment with w
//! applied directly). Post-phase has no outer multiplier;
//! `needs_transform` reads w.
//!
//! Source: `output/jwildfire-vars/output/post_rblur.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Post-phase radial blur — jitters the accumulator point by an amount
/// proportional to its distance from `(center_x, center_y)` minus `offset`,
/// scaled by `2·strength`. Two uniform-random jitter offsets per axis.
/// Inside the `offset` radius the jitter is suppressed (clamped to 0).
///
/// # Authors
/// - Xyrus02
pub static POST_RBLUR: VariationDef = VariationDef {
    name: "post_rblur",
    aliases: &[],
    display_name: "Post R-Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("strength", "Strength", unlimited_float, 1.0, -10.0, 10.0, "Jitter intensity (multiplied by 2 internally and by the radial distance)."),
        param!("offset", "Offset", unlimited_float, 1.0, -10.0, 10.0, "Inner radius — jitter is suppressed to 0 inside this distance from the center."),
        param!("center_x", "Center X", unlimited_float, 0.0, -10.0, 10.0, "X center of the radial blur."),
        param!("center_y", "Center Y", unlimited_float, 1.0, -10.0, 10.0, "Y center of the radial blur."),
    ],
    needs_transform: true,
    writes_color: false,
    // 1 derived value at slot 4: s2 = 2 · strength
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_post_rblur(user: array<f32, 4>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 2.0 * user[0];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_rblur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let offset = get_param(xform_id, variation_id, 1u);
    let cx = get_param(xform_id, variation_id, 2u);
    let cy = get_param(xform_id, variation_id, 3u);
    let s2 = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];

    let dx = p.x - cx;
    let dy = p.y - cy;
    var r = sqrt(dx * dx + dy * dy) - offset;
    if (r < 0.0) {
        r = 0.0;
    }
    r = r * s2;
    return vec2<f32>(w * (p.x + (rng_nextf(rng) - 0.5) * r),
                     w * (p.y + (rng_nextf(rng) - 0.5) * r));
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_rblur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let offset = get_param(xform_id, variation_id, 1u);
    let cx = get_param(xform_id, variation_id, 2u);
    let cy = get_param(xform_id, variation_id, 3u);
    let s2 = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];

    let dx = p.x - cx;
    let dy = p.y - cy;
    var r = sqrt(dx * dx + dy * dy) - offset;
    if (r < 0.0) {
        r = 0.0;
    }
    r = r * s2;
    return vec3<f32>(w * (p.x + (rng_nextf(rng) - 0.5) * r),
                     w * (p.y + (rng_nextf(rng) - 0.5) * r),
                     p.z);
}
"#),
};
