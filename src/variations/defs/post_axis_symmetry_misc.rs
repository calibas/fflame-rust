//! post_axis_symmetry_wf (Maschke)
//!
//! Post-phase axis-symmetry mirror across X, Y, or Z axis at a chosen
//! center, with optional rotation. RNG (1 call/iter) picks one of two
//! sides (left/right of axis).
//!
//! 5 spatial user params:
//!   - axis (int, 0=X, 1=Y, 2=Z)
//!   - centre_x, centre_y, centre_z
//!   - rotation (degrees, full circle = 360)
//!
//! cpp also includes 6 colorshift params (x1/x2/y1/y2/z1/z2) for
//! direct-color writes — omitted per writes_color compromise.
//!
//! 2 init slots: _sina, _cosa for `sin/cos(rotation·2π/180/2)`. cpp's
//! `_halve_dist = VVAR/2` is computed inline at runtime since it
//! depends on weight (which we read via needs_transform anyway).
//!
//! Body reads `p` (post-phase input) directly; needs_transform=true
//! to access weight for the half-distance offset. cpp's `_doRotate`
//! flag is replaced by an inline `|sina|>ε` test.
//!
//! Source: `output/jwildfire-vars/output/post_axis_symmetry_wf.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static POST_AXIS_SYMMETRY_WF: VariationDef = VariationDef {
    name: "post_axis_symmetry_wf",
    display_name: "Post Axis Symmetry WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("axis", "Axis", int, 0.0, 0.0, 2.0),
        param!("centre_x", "Centre X", unlimited_float, 0.25, -10.0, 10.0),
        param!("centre_y", "Centre Y", unlimited_float, 0.5, -10.0, 10.0),
        param!("centre_z", "Centre Z", unlimited_float, 0.5, -10.0, 10.0),
        param!("rotation", "Rotation", unlimited_float, 30.0, -360.0, 360.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_post_axis_symmetry_wf(user: array<f32, 5>) -> array<f32, 2> {
    let two_pi = 6.28318530717959;
    let a = user[4] * two_pi / 180.0 * 0.5;
    var out: array<f32, 2>;
    out[0] = sin(a);
    out[1] = cos(a);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_post_axis_symmetry_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let axis = i32(get_param(xform_id, variation_id, 0u));
    let cx = get_param(xform_id, variation_id, 1u);
    let cy = get_param(xform_id, variation_id, 2u);
    let sina = get_param(xform_id, variation_id, 5u);
    let cosa = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let halve = w * 0.5;
    let do_rotate = abs(sina) > 1e-6;

    var x = p.x;
    var y = p.y;
    if (axis == 0) {
        let dx0 = p.x - cx;
        if (rng_nextf(rng) < 0.5) {
            x = cx + dx0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            x = cx - dx0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    } else if (axis == 1) {
        let dy0 = p.y - cy;
        if (rng_nextf(rng) < 0.5) {
            y = cy + dy0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            y = cy - dy0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_axis_symmetry_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let axis = i32(get_param(xform_id, variation_id, 0u));
    let cx = get_param(xform_id, variation_id, 1u);
    let cy = get_param(xform_id, variation_id, 2u);
    let cz = get_param(xform_id, variation_id, 3u);
    let sina = get_param(xform_id, variation_id, 5u);
    let cosa = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let halve = w * 0.5;
    let do_rotate = abs(sina) > 1e-6;

    var x = p.x;
    var y = p.y;
    var z = p.z;
    if (axis == 0) {
        let dx0 = p.x - cx;
        if (rng_nextf(rng) < 0.5) {
            x = cx + dx0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            x = cx - dx0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    } else if (axis == 1) {
        let dy0 = p.y - cy;
        if (rng_nextf(rng) < 0.5) {
            y = cy + dy0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa + dya * sina;
                y = cy + dya * cosa - dxa * sina;
            }
        } else {
            y = cy - dy0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dya = y - cy;
                x = cx + dxa * cosa - dya * sina;
                y = cy + dya * cosa + dxa * sina;
            }
        }
    } else {
        let dz0 = p.z - cz;
        if (rng_nextf(rng) < 0.5) {
            z = cz + dz0 + halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dza = z - cz;
                x = cx + dxa * cosa + dza * sina;
                z = cy + dza * cosa - dxa * sina;
            }
        } else {
            z = cz - dz0 - halve;
            if (do_rotate) {
                let dxa = x - cx;
                let dza = z - cz;
                x = cx + dxa * cosa - dza * sina;
                z = cy + dza * cosa + dxa * sina;
            }
        }
    }
    return vec3<f32>(x, y, z);
}
"#),
};
