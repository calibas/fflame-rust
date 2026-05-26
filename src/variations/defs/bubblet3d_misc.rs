//! bubbleT3D (FractalDesire)
//!
//! 3D bubble inversion with optional radial stripes and Z-axis holes
//! controlled by angle, all wrapped in a `modus_blur` smoothing flag.
//!
//! 6 user params:
//!   - number_of_stripes (int 0+; negative inverts which stripe is
//!     drawn vs blanked)
//!   - ratio_of_stripes (clamped [0.01, 1.99] in init)
//!   - angle_of_hole (degrees; negative inverts hole vs stripe)
//!   - exponent_z (default 1.0; controls z-axis bubble shape)
//!   - symmetry_z (int 0/1)
//!   - modus_blur (int 0/1; 0 = clip to zero, 1 = smooth blend)
//!
//! 7 init slots:
//!   - _angHoleComp, _angStrip, _angStrip1, _angStrip2 (derived
//!     trig constants)
//!   - _invStripes (1.0 if number_of_stripes was negative, else 0.0)
//!   - _invHole (1.0 if angle_of_hole was negative, else 0.0)
//!   - _angle_of_hole_eff (the absolute, π-adjusted angle)
//!
//! cpp's `_s, _c` slots are recomputed inline (they were init slots
//! holding `sin/cos(_angStrip)` but the body reassigns them per-
//! iteration in the modus_blur=1 branch — so they were effectively
//! scratch).
//!
//! Body factors cleanly through outer multiplier (cpp uses VVAR
//! consistently on x/y/z output lines). cpp's unbounded
//! `while (angXY > _angStrip2) angXY -= _angStrip2` replaced with
//! `angXY = angXY - floor(angXY/_angStrip2) * _angStrip2`.
//!
//! Source: `output/jwildfire-vars/output/bubblet3d.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 3D bubble inversion with stripes and Z-axis hole — first does a bubble
/// (sphere) inversion of `(x, y)` (`/= (x²+y²)/4 + 1`), optionally erases
/// or rotates wedge-shaped stripes around the XY plane (count via
/// `number_of_stripes`, width-ratio via `ratio_of_stripes`), and finally
/// erases or blends a cap on the Z axis based on `angle_of_hole` (with
/// `exponent_z` controlling the Z bubble shape). `modus_blur = 0` does hard
/// cutouts; `modus_blur = 1` does a smooth angular blend (only effective
/// for the Z hole when `exponent_z == 1`). Negative `number_of_stripes` or
/// `angle_of_hole` inverts which side of the boundary is drawn vs blanked.
///
/// # Authors
/// - FractalDesire
pub static BUBBLE_T3D: VariationDef = VariationDef {
    name: "bubbleT3D",
    display_name: "Bubble T3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("number_of_stripes", "Number of Stripes", int, 0.0, -50.0, 50.0, "Number of angular stripe sectors around the XY plane. 0 disables stripes entirely. Negative value inverts the stripe pattern (swap drawn vs blanked sectors)."),
        param!("ratio_of_stripes", "Ratio of Stripes", unlimited_float, 1.0, 0.01, 1.99, "Width fraction of the drawn stripe within each sector, clamped to [0.01, 1.99]. 1.0 = stripes and gaps are equal width."),
        param!("angle_of_hole", "Angle of Hole", unlimited_float, 0.0, -180.0, 180.0, "Half-angle of the Z-axis hole in degrees. Negative value inverts which side of the hole is drawn vs blanked."),
        param!("exponent_z", "Exponent Z", unlimited_float, 1.0, 0.1, 10.0, "Z-axis bubble shape exponent. 1.0 = standard inversion; smaller values flatten the bubble, larger values elongate it."),
        param!("symmetry_z", "Symmetry Z", bool, false, "When 1, mirror the Z hole symmetrically across the XY plane (clamps `|angle_of_hole|` to < 180°). When 0, apply the hole only to one Z-pole."),
        param!("modus_blur", "Modus Blur", bool, false, "0 = hard cutout (zero out points inside stripes/hole). 1 = smooth blend via angular rotation (only effective for the Z hole when `exponent_z == 1`; otherwise falls back to hard cutout)."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 7,
    wgsl_init: Some(r#"
fn init_bubbleT3D(user: array<f32, 6>) -> array<f32, 7> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    var out: array<f32, 7>;
    let stripes_raw = user[0];
    let abs_stripes = abs(stripes_raw);
    let inv_stripes = select(0.0, 1.0, stripes_raw < 0.0);
    var ratio = clamp(user[1], 0.01, 1.99);
    var ang_strip = 0.0;
    var ang_strip1 = 0.0;
    var ang_strip2 = 0.0;
    if (abs_stripes > 0.5) {
        ang_strip = pi / abs_stripes;
        ang_strip2 = 2.0 * ang_strip;
        ang_strip1 = ratio * ang_strip;
    }
    var angle_h = user[2];
    let symmetry_z = user[4] > 0.5;
    if (symmetry_z) {
        angle_h = abs(angle_h);
        angle_h = min(angle_h, 179.9);
    }
    let inv_hole = select(0.0, 1.0, angle_h < 0.0);
    var angle_eff = abs(angle_h);
    if (inv_hole > 0.5) {
        angle_eff = (angle_eff / 360.0 * two_pi) * 0.5;
    } else {
        angle_eff = pi - (angle_eff / 360.0 * two_pi) * 0.5;
    }
    out[0] = pi - angle_eff;     // _angHoleComp
    out[1] = ang_strip;
    out[2] = ang_strip1;
    out[3] = ang_strip2;
    out[4] = inv_stripes;
    out[5] = inv_hole;
    out[6] = angle_eff;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bubbleT3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let stripes_raw = get_param(xform_id, variation_id, 0u);
    let abs_stripes = abs(stripes_raw);
    let ratio = clamp(get_param(xform_id, variation_id, 1u), 0.01, 1.99);
    let exponent_z = get_param(xform_id, variation_id, 3u);
    let modus_blur = i32(get_param(xform_id, variation_id, 5u));
    let ang_strip = get_param(xform_id, variation_id, 7u);
    let ang_strip1 = get_param(xform_id, variation_id, 8u);
    let ang_strip2 = get_param(xform_id, variation_id, 9u);
    let inv_stripes = get_param(xform_id, variation_id, 10u) > 0.5;
    let two_pi = 6.28318530717959;

    var x = p.x;
    var y = p.y;
    var ang_xy = atan2(x, y);
    if (ang_xy < 0.0) {
        ang_xy = ang_xy + two_pi;
    }
    if (abs_stripes > 0.5 && ang_strip2 > 0.0) {
        ang_xy = ang_xy - floor(ang_xy / ang_strip2) * ang_strip2;
        let stripe_hit = select(ang_xy > ang_strip1, ang_xy < ang_strip1, inv_stripes);
        if (stripe_hit) {
            if (modus_blur == 0) {
                x = 0.0;
                y = 0.0;
            } else {
                var ang_rot: f32;
                if (ratio == 1.0) {
                    ang_rot = ang_strip;
                } else if (inv_stripes) {
                    let t = (ang_xy - ang_strip1) / ang_strip1;
                    ang_rot = ang_xy - t * (ang_strip2 - ang_strip1);
                } else {
                    let t = (ang_xy - ang_strip1) / (ang_strip2 - ang_strip1);
                    ang_rot = ang_xy - t * ang_strip1;
                }
                let s = sin(ang_rot);
                let c = cos(ang_rot);
                let x_tmp = c * x - s * y;
                let y_tmp = s * x + c * y;
                x = x_tmp;
                y = y_tmp;
            }
        }
    }

    let rad = (x * x + y * y) * 0.25 + 1.0;
    let safe_rad = select(rad, 1e-30, rad < 1e-30);
    return vec2<f32>(x / safe_rad, y / safe_rad);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bubbleT3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let stripes_raw = get_param(xform_id, variation_id, 0u);
    let abs_stripes = abs(stripes_raw);
    let ratio = clamp(get_param(xform_id, variation_id, 1u), 0.01, 1.99);
    let exponent_z = get_param(xform_id, variation_id, 3u);
    let symmetry_z = i32(get_param(xform_id, variation_id, 4u));
    let modus_blur = i32(get_param(xform_id, variation_id, 5u));
    let ang_hole_comp = get_param(xform_id, variation_id, 6u);
    let ang_strip = get_param(xform_id, variation_id, 7u);
    let ang_strip1 = get_param(xform_id, variation_id, 8u);
    let ang_strip2 = get_param(xform_id, variation_id, 9u);
    let inv_stripes = get_param(xform_id, variation_id, 10u) > 0.5;
    let inv_hole = get_param(xform_id, variation_id, 11u) > 0.5;
    let angle_eff = get_param(xform_id, variation_id, 12u);
    let pi = 3.14159265358979;
    let pi_2 = 1.5707963267948966;
    let two_pi = 6.28318530717959;

    var x = p.x;
    var y = p.y;
    var z = p.z;
    var ang_xy = atan2(x, y);
    if (ang_xy < 0.0) {
        ang_xy = ang_xy + two_pi;
    }

    // Stripes processing
    if (abs_stripes > 0.5 && ang_strip2 > 0.0) {
        ang_xy = ang_xy - floor(ang_xy / ang_strip2) * ang_strip2;
        let stripe_hit = select(ang_xy > ang_strip1, ang_xy < ang_strip1, inv_stripes);
        if (stripe_hit) {
            if (modus_blur == 0) {
                x = 0.0;
                y = 0.0;
            } else {
                var ang_rot: f32;
                if (ratio == 1.0) {
                    ang_rot = ang_strip;
                } else if (inv_stripes) {
                    let t = (ang_xy - ang_strip1) / ang_strip1;
                    ang_rot = ang_xy - t * (ang_strip2 - ang_strip1);
                } else {
                    let t = (ang_xy - ang_strip1) / (ang_strip2 - ang_strip1);
                    ang_rot = ang_xy - t * ang_strip1;
                }
                let s = sin(ang_rot);
                let c = cos(ang_rot);
                let x_tmp = c * x - s * y;
                let y_tmp = s * x + c * y;
                x = x_tmp;
                y = y_tmp;
            }
        }
    }

    // Hole calculation: bubble inversion + Z modulation
    let rad = (x * x + y * y) * 0.25 + 1.0;
    let safe_rad = select(rad, 1e-30, rad < 1e-30);
    x = x / safe_rad;
    y = y / safe_rad;
    var ang_z = 0.0;
    if (abs(x) > 1e-30 || abs(y) > 1e-30) {
        z = 2.0 / pow(safe_rad, exponent_z) - 1.0;
        if (exponent_z <= 2.0) {
            let denom = x * x + y * y + z * z;
            ang_z = pi - acos(clamp(z / max(denom, 1e-30), -1.0, 1.0));
        } else {
            let r2sq = (x * x + y * y) * (x * x + y * y);
            ang_z = pi - atan2(r2sq, z);
        }
    } else {
        z = 0.0;
        ang_z = 0.0;
    }

    if (symmetry_z == 0) {
        var hit = false;
        if (!inv_hole) {
            hit = ang_z > angle_eff;
        } else {
            hit = ang_z < angle_eff;
        }
        if (hit) {
            if (modus_blur == 0 || exponent_z != 1.0) {
                x = 0.0;
                y = 0.0;
                z = 0.0;
            } else {
                var ang_tmp: f32;
                if (!inv_hole) {
                    ang_tmp = (pi - ang_z) / max(ang_hole_comp, 1e-30) * angle_eff - pi_2;
                } else {
                    ang_tmp = pi - ang_z / max(ang_hole_comp, 1e-30) * angle_eff - pi_2;
                }
                let ang_z_shift = ang_z - pi_2;
                let safe_cos = select(cos(ang_z_shift), 1e-30, abs(cos(ang_z_shift)) < 1e-30);
                let fac = cos(ang_tmp) / safe_cos;
                x = x * fac;
                y = y * fac;
                let safe_sin = select(sin(ang_z_shift), 1e-30, abs(sin(ang_z_shift)) < 1e-30);
                z = z * (sin(ang_tmp) / safe_sin);
            }
        }
    } else {
        if (ang_z > angle_eff || ang_z < (pi - angle_eff)) {
            if (modus_blur == 0 || exponent_z != 1.0) {
                x = 0.0;
                y = 0.0;
                z = 0.0;
            } else {
                var ang_tmp: f32;
                let safe_hc = max(ang_hole_comp, 1e-30);
                if (ang_z > angle_eff) {
                    ang_tmp = (pi - ang_z) / safe_hc * (pi - 2.0 * ang_hole_comp) + ang_hole_comp - pi_2;
                } else {
                    ang_tmp = pi_2 - (ang_z / safe_hc * (pi - 2.0 * ang_hole_comp) + ang_hole_comp);
                }
                let ang_z_shift = ang_z - pi_2;
                let safe_cos = select(cos(ang_z_shift), 1e-30, abs(cos(ang_z_shift)) < 1e-30);
                let fac = cos(ang_tmp) / safe_cos;
                x = x * fac;
                y = y * fac;
                let safe_sin = select(sin(ang_z_shift), 1e-30, abs(sin(ang_z_shift)) < 1e-30);
                z = z * (sin(ang_tmp) / safe_sin);
            }
        }
    }

    return vec3<f32>(x, y, z);
}
"#),
};
