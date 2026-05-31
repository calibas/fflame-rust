//! Direct-color (DC) variations
//!
//! Faithful ports of JWildfire/Chaotica's `dc_*` plugin family. These
//! variations write to the iteration-local color register `vc` based on
//! geometry; the main loop's Step 3 lerp blends `vc` back into
//! `color_index` using the transform's `direct_color` field. When
//! `direct_color = 0` (the default), the variation's color writes have
//! no visible effect — set the per-transform Direct Color slider above 0
//! in the UI to see them.
//!
//! Sources:
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/dc_linear.cpp
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/dc_bubble.cpp
//!
//! Notes on faithfulness:
//!   - Both variations compute color from the WEIGHTED post-variation
//!     position (FPx after the C++ `FPx += VVAR * something` accumulator).
//!     We replicate this with `let weight = transforms[xform_id]
//!     .variations[variation_id]` (needs_transform: true) and use
//!     `weight * unweighted_output` in the color formula. This matches the
//!     C++ exactly when the DC variation is the only normal-phase variation
//!     in its transform, which is the typical use case. Mixed with other
//!     normal variations, the C++ accumulates FPx across all variations
//!     before computing color; our model uses just this variation's
//!     contribution.
//!   - dc_bubble's C++ port has an apparent porter typo:
//!     `FPx += FPx + r4_1 * FTx;` doubles FPx instead of incrementing once.
//!     We follow the JWildfire Java original (single-add bubble) since the
//!     C++ variant produces obvious geometric artifacts. The C++ Z formula
//!     `FPz += FPz + VVAR * (2/r4_1 - 1)` is broken for the same reason;
//!     we pass Z through unchanged in 3D mode (consistent with most 2D-with-z
//!     variations in this codebase).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// dc_linear: Brad Stefanov / JWildfire — color from rotated linear projection
//   Position: linear (input passed through; outer dispatcher applies weight)
//   Color:    vc = fract(abs(0.5 * (ldcs * (cos·FPx + sin·FPy + offset) + 1)))
//   where ldcs = 1/scale (zero-guarded), FPx/FPy are weighted output coords.
// =============================================================================
/// Pass-through positioning (linear) with direct-color writes — colors each
/// iteration based on a rotated linear projection of the post-variation
/// point. Output position is unchanged from the input; effect only visible
/// when the transform's Direct Color slider is > 0.
///
/// # Authors
/// - Xyrus02
pub static DC_LINEAR: VariationDef = VariationDef {
    name: "dc_linear",
    aliases: &[],
    display_name: "DC Linear",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("offset", "Offset", unlimited_float, 0.0, -10.0, 10.0, "Offset added to the projected coordinate before computing color."),
        param!("angle", "Angle", angle, 0.0, "Rotation angle (degrees) for the projection axis."),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Scaling factor on the projection — larger compresses the color gradient, smaller stretches it."),
    ],
    needs_transform: true,
    writes_color: true,
    // 3 derived values at slots 3..6:
    //   3: ldcs       (1/scale, with 1/1e-5 fallback when scale is exactly 0)
    //   4: cos_angle  (cos of angle in radians)
    //   5: sin_angle  (sin of angle in radians)
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_dc_linear(user: array<f32, 3>) -> array<f32, 3> {
    let scale = user[2];
    let angle_deg = user[1];
    let angle_rad = angle_deg * 3.14159265358979 / 180.0;
    var out: array<f32, 3>;
    out[0] = 1.0 / select(scale, 1e-5, scale == 0.0);
    out[1] = cos(angle_rad);
    out[2] = sin(angle_rad);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_dc_linear(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let offset = get_param(xform_id, variation_id, 0u);
    let ldcs = get_param(xform_id, variation_id, 3u);
    let cos_a = get_param(xform_id, variation_id, 4u);
    let sin_a = get_param(xform_id, variation_id, 5u);
    let weight = transforms[xform_id].variations[variation_id];

    // Color from weighted post-variation position (matches C++ FPx after the
    // weighted += accumulator, when dc_linear is the only normal variation).
    let wx = weight * p.x;
    let wy = weight * p.y;
    let proj = cos_a * wx + sin_a * wy + offset;
    *vc = fract(abs(0.5 * (ldcs * proj + 1.0)));

    return p;
}
"#,
    wgsl_3d: r#"
fn variation_dc_linear(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let offset = get_param(xform_id, variation_id, 0u);
    let ldcs = get_param(xform_id, variation_id, 3u);
    let cos_a = get_param(xform_id, variation_id, 4u);
    let sin_a = get_param(xform_id, variation_id, 5u);
    let weight = transforms[xform_id].variations[variation_id];

    let wx = weight * p.x;
    let wy = weight * p.y;
    let proj = cos_a * wx + sin_a * wy + offset;
    *vc = fract(abs(0.5 * (ldcs * proj + 1.0)));

    return p;
}
"#,
};

// =============================================================================
// dc_bubble: JWildfire — bubble warp + radial color from offset center
//   Position: out = (x, y) / (r²/4 + 1)   (classic Apophysis bubble)
//   Color:    vc = fract(abs(bdcs * ((FPx + cx)² + (FPy + cy)²)))
//   where bdcs = 1/scale (zero-guarded), FPx/FPy are weighted output coords.
// =============================================================================
/// Apophysis Bubble warp (spherical projection) with direct-color writes —
/// colors each iteration based on the squared distance from a configurable
/// center point. Same XY warp as Bubble, plus per-iteration color
/// modulation. Color effect only visible when the transform's Direct Color
/// slider is > 0.
///
/// # Authors
/// - Xyrus02
pub static DC_BUBBLE: VariationDef = VariationDef {
    name: "dc_bubble",
    aliases: &[],
    display_name: "DC Bubble",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("centerx", "Center X", unlimited_float, 0.0, -2.0, 2.0, "X coordinate of the radial color center."),
        param!("centery", "Center Y", unlimited_float, 0.0, -2.0, 2.0, "Y coordinate of the radial color center."),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Scaling factor on the squared distance — larger compresses the color gradient, smaller stretches it."),
    ],
    needs_transform: true,
    writes_color: true,
    // 1 derived value at slot 3:
    //   3: bdcs  (1/scale, with 1/1e-5 fallback when scale is exactly 0)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_dc_bubble(user: array<f32, 3>) -> array<f32, 1> {
    let scale = user[2];
    var out: array<f32, 1>;
    out[0] = 1.0 / select(scale, 1e-5, scale == 0.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_dc_bubble(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let bdcs = get_param(xform_id, variation_id, 3u);
    let weight = transforms[xform_id].variations[variation_id];

    let r = p.x * p.x + p.y * p.y;
    let r4_1 = 1.0 / (r / 4.0 + 1.0);
    let new_x = r4_1 * p.x;
    let new_y = r4_1 * p.y;

    // Color from weighted post-variation position with the user-supplied offset.
    let wx = weight * new_x + cx;
    let wy = weight * new_y + cy;
    *vc = fract(abs(bdcs * (wx * wx + wy * wy)));

    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: r#"
fn variation_dc_bubble(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let bdcs = get_param(xform_id, variation_id, 3u);
    let weight = transforms[xform_id].variations[variation_id];

    let r = p.x * p.x + p.y * p.y;
    let r4_1 = 1.0 / (r / 4.0 + 1.0);
    let new_x = r4_1 * p.x;
    let new_y = r4_1 * p.y;

    let wx = weight * new_x + cx;
    let wy = weight * new_y + cy;
    *vc = fract(abs(bdcs * (wx * wx + wy * wy)));

    return vec3<f32>(new_x, new_y, p.z);
}
"#,
};
