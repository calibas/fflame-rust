//! Standalone exotic shapes
//!
//! Three popular standalone variations:
//!   - `kaleidoscope` (Will Evans)               — half-plane mirror warp
//!   - `taurus`       (gossamer light)            — torus-section mapping
//!   - `hole2`        (Faber/Stefanov/Sidwell)    — 10 radial shapes via
//!                                                  user `shape` selector
//!
//! Sources:
//!   - output/jwildfire-vars/output/kaleidoscope.cpp
//!   - output/jwildfire-vars/output/taurus.cpp
//!   - output/jwildfire-vars/output/hole2.cpp  (cpp PluginVarCalc was an
//!     unported_stub; ported from the embedded Java comment block)
//!
//! Notes on faithfulness:
//!   - `kaleidoscope` upstream uses `cos(45.0)` / `sin(45.0)` — that's
//!     45 RADIANS (≈ 0.5253 / 0.8509), not 45 degrees. Both Java and
//!     cpp agree, so this appears to be original-author intent (or a
//!     long-lived quirk). Preserved.
//!   - `kaleidoscope` and `taurus` partly factor through the outer
//!     multiplier and partly don't — body uses `needs_transform` +
//!     divide-out so outer × w restores the cpp output exactly.
//!   - `hole2`'s `atan2` is in standard Java order (`atan2(y, x)`).
//!
//! Skipped:
//!   - `flower_db` — uses FPz assignment (`FPz = -stem_length` clamp)
//!     plus non-linear weight scaling on the FPz output. Fits poorly
//!     in our outer-multiplier convention.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// kaleidoscope: half-plane mirror warp (Will Evans)
//   FPx += (rotate · x) · cos(45) − y · sin(45) + line_up + x_offset
//   if y > 0:
//     FPy += (rotate · y) · cos(45) + x · sin(45) + pull + line_up + y_offset
//   else:
//     FPy += (rotate · y) · cos(45) + x · sin(45) − pull − line_up
// (cos(45) / sin(45) are radians here — preserved from upstream)
// =============================================================================
pub static KALEIDOSCOPE: VariationDef = VariationDef {
    name: "kaleidoscope",
    display_name: "Kaleidoscope",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("pull", "Pull", unlimited_float, 0.0, -5.0, 5.0),
        param!("rotate", "Rotate", unlimited_float, 1.0, -5.0, 5.0),
        param!("line_up", "Line Up", unlimited_float, 1.0, -5.0, 5.0),
        param!("x", "X", unlimited_float, 0.0, -5.0, 5.0),
        param!("y", "Y", unlimited_float, 0.0, -5.0, 5.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_kaleidoscope(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let pull = get_param(xform_id, variation_id, 0u);
    let rotate = get_param(xform_id, variation_id, 1u);
    let line_up = get_param(xform_id, variation_id, 2u);
    let x_off = get_param(xform_id, variation_id, 3u);
    let y_off = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    // 45 RADIANS, matching cpp/Java intent (preserved quirk).
    let c45 = cos(45.0);
    let s45 = sin(45.0);

    let fx = (rotate * p.x) * c45 - p.y * s45 + line_up + x_off;
    var fy: f32;
    if (p.y > 0.0) {
        fy = (rotate * p.y) * c45 + p.x * s45 + pull + line_up + y_off;
    } else {
        fy = (rotate * p.y) * c45 + p.x * s45 - pull - line_up;
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_kaleidoscope(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let pull = get_param(xform_id, variation_id, 0u);
    let rotate = get_param(xform_id, variation_id, 1u);
    let line_up = get_param(xform_id, variation_id, 2u);
    let x_off = get_param(xform_id, variation_id, 3u);
    let y_off = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let c45 = cos(45.0);
    let s45 = sin(45.0);

    let fx = (rotate * p.x) * c45 - p.y * s45 + line_up + x_off;
    var fy: f32;
    if (p.y > 0.0) {
        fy = (rotate * p.y) * c45 + p.x * s45 + pull + line_up + y_off;
    } else {
        fy = (rotate * p.y) * c45 + p.x * s45 - pull - line_up;
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#),
};

// =============================================================================
// taurus: torus-section mapping (gossamer light)
//   ir = inv·r + (1 − inv) · r · cos(n·x)
//   FPx += w · cx · (ir + sy)         (cx = cos(x), sx = sin(x), ...)
//   FPy += w · sx · (ir + sy)
//   FPz += w · sor · cy + (1 − sor) · y     (last term lacks w — divide-out)
// =============================================================================
pub static TAURUS: VariationDef = VariationDef {
    name: "taurus",
    display_name: "Taurus",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("r", "R", unlimited_float, 3.0, -10.0, 10.0),
        param!("n", "N", unlimited_float, 5.0, -20.0, 20.0),
        param!("inv", "Inv", unlimited_float, 1.5, -10.0, 10.0),
        param!("sor", "Sor", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_taurus(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let r_p = get_param(xform_id, variation_id, 0u);
    let n_p = get_param(xform_id, variation_id, 1u);
    let inv_p = get_param(xform_id, variation_id, 2u);

    // FPx and FPy lines factor cleanly through the outer multiplier,
    // so no needs_transform read is required for the 2D form.
    let sx = sin(p.x);
    let cx = cos(p.x);
    let sy = sin(p.y);
    let ir = (inv_p * r_p) + ((1.0 - inv_p) * (r_p * cos(n_p * p.x)));
    return vec2<f32>(cx * (ir + sy), sx * (ir + sy));
}
"#,
    wgsl_3d: Some(r#"
fn variation_taurus(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let r_p = get_param(xform_id, variation_id, 0u);
    let n_p = get_param(xform_id, variation_id, 1u);
    let inv_p = get_param(xform_id, variation_id, 2u);
    let sor_p = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let sx = sin(p.x);
    let cx = cos(p.x);
    let sy = sin(p.y);
    let cy = cos(p.y);
    let ir = (inv_p * r_p) + ((1.0 - inv_p) * (r_p * cos(n_p * p.x)));
    let fx_unweighted = cx * (ir + sy);
    let fy_unweighted = sx * (ir + sy);
    // FPz upstream: VVAR · sor · cy + (1 − sor) · y    (the last term lacks
    // VVAR — divide-out so outer × w restores the cpp result.)
    let fz_full = w * sor_p * cy + (1.0 - sor_p) * p.y;
    return vec3<f32>(fx_unweighted, fy_unweighted, fz_full * inv_w);
}
"#),
};

// =============================================================================
// hole2: 10-shape radial hole (Faber/Stefanov/Sidwell)
//   theta = atan2(y, x) · d
//   delta = (theta/π + 1)^a · c
//   r1 = one of 10 radial formulas, selected by `shape`
//   if inside ≠ 0:  r1 = w / r1
//   else:           r1 = w · r1
//   out = (r1 · cos(theta), r1 · sin(theta))
// (cpp body was an unported_stub — translated from the Java comment block)
// =============================================================================
pub static HOLE2: VariationDef = VariationDef {
    name: "hole2",
    display_name: "Hole 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 2.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0),
        param!("inside", "Inside", int, 0.0, 0.0, 1.0),
        param!("shape", "Shape", int, 0.0, 0.0, 9.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_hole2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let inside = get_param(xform_id, variation_id, 4u);
    let shape = get_param(xform_id, variation_id, 5u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let half_pi = 1.5707963267948966;

    let rhosq = p.x * p.x + p.y * p.y;
    let theta = atan2(p.y, p.x) * d;
    let delta = pow(max(theta * inv_pi + 1.0, 1e-30), a) * c;

    let shape_i = i32(shape);
    var r1 = 1.0;
    if (shape_i == 0) {
        r1 = sqrt(max(rhosq, 0.0)) + delta;
    } else if (shape_i == 1) {
        r1 = sqrt(max(rhosq + delta, 0.0));
    } else if (shape_i == 2) {
        r1 = sqrt(max(rhosq + sin(b * theta) + delta, 0.0));
    } else if (shape_i == 3) {
        r1 = sqrt(max(rhosq + sin(theta) + delta, 0.0));
    } else if (shape_i == 4) {
        r1 = sqrt(max(rhosq + theta * theta - delta + 1.0, 0.0));
    } else if (shape_i == 5) {
        r1 = sqrt(max(rhosq + abs(tan(theta)) + delta, 0.0));
    } else if (shape_i == 6) {
        r1 = sqrt(max(rhosq * (1.0 + sin(b * theta)) + delta, 0.0));
    } else if (shape_i == 7) {
        r1 = sqrt(max(rhosq + abs(sin(0.5 * b * theta)) + delta, 0.0));
    } else if (shape_i == 8) {
        r1 = sqrt(max(rhosq + sin(pi * sin(b * theta)) + delta, 0.0));
    } else {
        r1 = sqrt(max(rhosq + (sin(b * theta) + sin(2.0 * b * theta + half_pi)) * 0.5 + delta, 0.0));
    }

    if (inside != 0.0) {
        r1 = 1.0 / max(r1, 1e-30);
    }
    return vec2<f32>(r1 * cos(theta), r1 * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_hole2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let inside = get_param(xform_id, variation_id, 4u);
    let shape = get_param(xform_id, variation_id, 5u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let half_pi = 1.5707963267948966;

    let rhosq = p.x * p.x + p.y * p.y;
    let theta = atan2(p.y, p.x) * d;
    let delta = pow(max(theta * inv_pi + 1.0, 1e-30), a) * c;

    let shape_i = i32(shape);
    var r1 = 1.0;
    if (shape_i == 0) {
        r1 = sqrt(max(rhosq, 0.0)) + delta;
    } else if (shape_i == 1) {
        r1 = sqrt(max(rhosq + delta, 0.0));
    } else if (shape_i == 2) {
        r1 = sqrt(max(rhosq + sin(b * theta) + delta, 0.0));
    } else if (shape_i == 3) {
        r1 = sqrt(max(rhosq + sin(theta) + delta, 0.0));
    } else if (shape_i == 4) {
        r1 = sqrt(max(rhosq + theta * theta - delta + 1.0, 0.0));
    } else if (shape_i == 5) {
        r1 = sqrt(max(rhosq + abs(tan(theta)) + delta, 0.0));
    } else if (shape_i == 6) {
        r1 = sqrt(max(rhosq * (1.0 + sin(b * theta)) + delta, 0.0));
    } else if (shape_i == 7) {
        r1 = sqrt(max(rhosq + abs(sin(0.5 * b * theta)) + delta, 0.0));
    } else if (shape_i == 8) {
        r1 = sqrt(max(rhosq + sin(pi * sin(b * theta)) + delta, 0.0));
    } else {
        r1 = sqrt(max(rhosq + (sin(b * theta) + sin(2.0 * b * theta + half_pi)) * 0.5 + delta, 0.0));
    }

    if (inside != 0.0) {
        r1 = 1.0 / max(r1, 1e-30);
    }
    return vec3<f32>(r1 * cos(theta), r1 * sin(theta), p.z);
}
"#),
};
