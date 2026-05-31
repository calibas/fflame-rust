//! Internal-weight watchlist ports
//!
//! Variations from the long-standing internal-weight watchlist that
//! become tractable now that `needs_transform: true` lets the body
//! read its own weight via `transforms[xform_id].variations[variation_id]`.
//!
//! Two patterns in this batch:
//!
//! 1. **Threshold-only weight** (`loonie3`, `loonie_3d`):
//!    weight appears only as a comparison threshold (`r² < weight²`).
//!    The output factors cleanly through the outer multiplier, so the
//!    body reads the weight, computes `sqrvvar = w²`, and returns the
//!    unweighted scaling factor times the input.
//!
//! 2. **Non-linear weight** (`sigmoid`, `blocky`):
//!    weight enters the math non-linearly (`|w|` or `w²`). The body
//!    computes the full cpp output using the read weight, then divides
//!    by `w` to "pre-cancel" the outer multiplier — outer × w then
//!    restores the cpp output exactly.
//!
//! Sources:
//!   - output/jwildfire-vars/output/loonie3.cpp
//!   - output/jwildfire-vars/output/loonie_3d.cpp
//!   - output/jwildfire-vars/output/sigmoid.cpp
//!   - output/jwildfire-vars/output/blocky.cpp
//!
//! Faithfulness notes:
//!   - `loonie_3d` upstream computes `rmod = random01() * 0.5 + 0.125`
//!     but never reads it — porter dead code. Skipped (we don't
//!     consume an RNG draw, since our PCG isn't state-equivalent to
//!     Java MT anyway).
//!   - `blocky` upstream's `sqrt_safe(vp, x)` ignores its function
//!     argument and reads the user `x` parameter via `VAR(x)` — a
//!     macro-expansion porter bug. We follow the obvious Java intent
//!     `b = sqrt(max(1 - a², 0))` rather than reproducing the bug,
//!     which would make the variation's behavior depend on a setting
//!     it shouldn't.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// loonie3: dark-beam's loonie variant 3 (single-arm form)
//   sqrvvar = w²
//   r2 = (x²+y²)² / x²       (defaults to 2·sqrvvar when x ≤ ε to skip the
//                              if-branch; matches upstream)
//   if r2 < sqrvvar:  scale = sqrt(sqrvvar / r2 - 1);  out = scale · (x, y)
//   else:             out = (x, y)
//   (weight applied outside in both branches)
// =============================================================================
/// Variant 3 of Loonie — same coin-shape inversion as Loonie but uses
/// `(r²)²/x²` for the radial threshold check, producing a stretched single-
/// arm form.
///
/// # Authors
/// - DarkBeam
pub static LOONIE3: VariationDef = VariationDef {
    name: "loonie3",
    aliases: &[],
    display_name: "Loonie 3",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_loonie3(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let sqrvvar = w * w;
    var r2 = 2.0 * sqrvvar;
    if (p.x > 1e-6) {
        let s = (p.x * p.x + p.y * p.y) / p.x;
        r2 = s * s;
    }
    if (r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
        return vec2<f32>(scale * p.x, scale * p.y);
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_loonie3(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let sqrvvar = w * w;
    var r2 = 2.0 * sqrvvar;
    if (p.x > 1e-6) {
        let s = (p.x * p.x + p.y * p.y) / p.x;
        r2 = s * s;
    }
    if (r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
        return vec3<f32>(scale * p.x, scale * p.y, p.z);
    }
    return p;
}
"#,
};

// =============================================================================
// loonie_3d: 3D loonie variant (Larry Berlin, 2009)
//   sqrvvar = w²
//   ef_z    = z if z ≠ 0, else atan2(y, x)
//   r2      = x² + y² + ef_z²
//   if r2 < sqrvvar: scale = sqrt(sqrvvar/r2 − 1);
//                    out  = (scale·x, scale·y, scale·ef_z·0.5)
//   else:            out  = (x, y, ef_z·0.5)
//   (weight applied outside in both branches)
//
// Upstream computes `rmod` from RNG but never uses it — skipped here.
// =============================================================================
/// 3D version of Loonie — inverts points inside a sphere sized by the
/// variation's weight; Z gets folded through an atan2 substitution for non-
/// zero depth handling.
///
/// # Authors
/// - Larry Berlin
pub static LOONIE_3D: VariationDef = VariationDef {
    name: "loonie_3D",
    aliases: &[],
    display_name: "Loonie 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    // 2D form: ef_z = atan2(y, x) (since p.z = 0 in 2D mode); same math
    // otherwise. The output's z-component is dropped via the wrapper.
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_loonie_3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let sqrvvar = w * w;
    let ef_z = atan2(p.y, p.x);
    let r2 = p.x * p.x + p.y * p.y + ef_z * ef_z;
    if (r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
        return vec2<f32>(scale * p.x, scale * p.y);
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_loonie_3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let sqrvvar = w * w;
    var ef_z = p.z;
    if (ef_z == 0.0) {
        ef_z = atan2(p.y, p.x);
    }
    let r2 = p.x * p.x + p.y * p.y + ef_z * ef_z;
    if (r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1e-30) - 1.0, 0.0));
        return vec3<f32>(scale * p.x, scale * p.y, scale * ef_z * 0.5);
    }
    return vec3<f32>(p.x, p.y, ef_z * 0.5);
}
"#,
};

// =============================================================================
// sigmoid: Xyrus / Brad Stefanov — saturating sigmoid in both axes
//   ax, sx = sign and 1/shiftx if |shiftx|<1; else 1, shiftx
//   ay, sy = same for shifty
//   sx, sy *= -5
//   c0 = ax / (1 + exp(sx·x));  c1 = ay / (1 + exp(sy·y))
//   out = (2·(c0 − 0.5), 2·(c1 − 0.5))
//   FPx += |w| · out.x  (no Z line in upstream)
//
// |w| in cpp; we read w via needs_transform and emit `sign(w) · out` so
// outer multiplier (w) yields |w| · out.
// =============================================================================
/// Saturating sigmoid in both axes — pushes coordinates through `1/(1 +
/// exp(...))` to compress them toward the [-1, 1] range. `shiftx` /
/// `shifty` control how steep the saturation curve is on each axis.
///
/// # Authors
/// - Xyrus02
/// - Brad Stefanov
pub static SIGMOID: VariationDef = VariationDef {
    name: "sigmoid",
    aliases: &[],
    display_name: "Sigmoid",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("shiftx", "Shift X", unlimited_float, 1.0, -10.0, 10.0, "X-axis saturation curve. Higher absolute value = steeper transition."),
        param!("shifty", "Shift Y", unlimited_float, 1.0, -10.0, 10.0, "Y-axis saturation curve. Higher absolute value = steeper transition."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_sigmoid(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let shiftx_in = get_param(xform_id, variation_id, 0u);
    let shifty_in = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let sgn = select(sign(w), 1.0, w == 0.0);

    var ax = 1.0;
    var sx = shiftx_in;
    if (sx < 1.0 && sx > -1.0) {
        if (sx == 0.0) {
            sx = 1e-6;
        } else {
            ax = select(1.0, -1.0, sx < 0.0);
            sx = 1.0 / sx;
        }
    }
    var ay = 1.0;
    var sy = shifty_in;
    if (sy < 1.0 && sy > -1.0) {
        if (sy == 0.0) {
            sy = 1e-6;
        } else {
            ay = select(1.0, -1.0, sy < 0.0);
            sy = 1.0 / sy;
        }
    }
    sx = sx * -5.0;
    sy = sy * -5.0;

    let c0 = ax / (1.0 + exp(sx * p.x));
    let c1 = ay / (1.0 + exp(sy * p.y));
    return vec2<f32>(sgn * 2.0 * (c0 - 0.5), sgn * 2.0 * (c1 - 0.5));
}
"#,
    // Upstream has no FPz line — return 0 for z so outer × weight = 0
    // contribution to the z accumulator.
    wgsl_3d: r#"
fn variation_sigmoid(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let shiftx_in = get_param(xform_id, variation_id, 0u);
    let shifty_in = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let sgn = select(sign(w), 1.0, w == 0.0);

    var ax = 1.0;
    var sx = shiftx_in;
    if (sx < 1.0 && sx > -1.0) {
        if (sx == 0.0) {
            sx = 1e-6;
        } else {
            ax = select(1.0, -1.0, sx < 0.0);
            sx = 1.0 / sx;
        }
    }
    var ay = 1.0;
    var sy = shifty_in;
    if (sy < 1.0 && sy > -1.0) {
        if (sy == 0.0) {
            sy = 1e-6;
        } else {
            ay = select(1.0, -1.0, sy < 0.0);
            sy = 1.0 / sy;
        }
    }
    sx = sx * -5.0;
    sy = sy * -5.0;

    let c0 = ax / (1.0 + exp(sx * p.x));
    let c1 = ay / (1.0 + exp(sy * p.y));
    return vec3<f32>(sgn * 2.0 * (c0 - 0.5), sgn * 2.0 * (c1 - 0.5), 0.0);
}
"#,
};

// =============================================================================
// blocky: FracFx 2D-block warp (Brad Stefanov)
//   v   = w / (π/2)         (depends on weight)
//   T   = (cos(x) + cos(y))/mp + 1
//   r   = w / T             (depends on weight)
//   xmax, ymax = ellipse axes from x²+y²+1 ± 2·{x,y}
//   FPx += v · atan2(x/xmax, sqrt(1 − (x/xmax)²)) · r · param_x
//   FPy += v · atan2(y/ymax, sqrt(1 − (y/ymax)²)) · r · param_y
//
// Output is quadratic in w (v · r contains w²). We compute the full cpp
// output using the read w and divide by w to pre-cancel the outer
// multiplier — outer × w then restores the cpp result exactly.
//
// Upstream's `sqrt_safe` is buggy (reads the user `x` param via a macro
// instead of its function argument). We follow the obvious Java intent
// `sqrt(max(1 − a², 0))` rather than reproducing the bug.
// =============================================================================
/// 2D-block warp — maps points through an ellipse-bounded arctan to produce
/// angular blocky patterns. `mp` controls block size; `x` and `y` set per-
/// axis aspect.
///
/// # Authors
/// - Brad Stefanov
pub static BLOCKY: VariationDef = VariationDef {
    name: "blocky",
    aliases: &[],
    display_name: "Blocky",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("x", "X", unlimited_float, 1.0, -10.0, 10.0, "X-axis scaling on the arctan output."),
        param!("y", "Y", unlimited_float, 1.0, -10.0, 10.0, "Y-axis scaling on the arctan output."),
        param!("mp", "MP", unlimited_float, 4.0, 0.1, 20.0, "Block size — smaller values produce more, finer blocks."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_blocky(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let px = get_param(xform_id, variation_id, 0u);
    let py = get_param(xform_id, variation_id, 1u);
    let mp = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_over_pi = 0.6366197723675813;  // 1 / (π/2)

    let v_in = w * two_over_pi;
    let safe_mp = select(mp, 1e-6, mp == 0.0);
    let T = (cos(p.x) + cos(p.y)) / safe_mp + 1.0;
    let safe_T = select(T, 1e-6, T == 0.0);
    let r = w / safe_T;

    let tmp = p.x * p.x + p.y * p.y + 1.0;
    let x2 = 2.0 * p.x;
    let y2 = 2.0 * p.y;
    let xmax = 0.5 * (sqrt(max(tmp + x2, 0.0)) + sqrt(max(tmp - x2, 0.0)));
    let ymax = 0.5 * (sqrt(max(tmp + y2, 0.0)) + sqrt(max(tmp - y2, 0.0)));

    let ax_n = p.x / max(xmax, 1e-30);
    let bx = sqrt(max(1.0 - ax_n * ax_n, 0.0));
    let out_x = v_in * atan2(ax_n, bx) * r * px;

    let ay_n = p.y / max(ymax, 1e-30);
    let by = sqrt(max(1.0 - ay_n * ay_n, 0.0));
    let out_y = v_in * atan2(ay_n, by) * r * py;

    return vec2<f32>(out_x * inv_w, out_y * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_blocky(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let px = get_param(xform_id, variation_id, 0u);
    let py = get_param(xform_id, variation_id, 1u);
    let mp = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_over_pi = 0.6366197723675813;

    let v_in = w * two_over_pi;
    let safe_mp = select(mp, 1e-6, mp == 0.0);
    let T = (cos(p.x) + cos(p.y)) / safe_mp + 1.0;
    let safe_T = select(T, 1e-6, T == 0.0);
    let r = w / safe_T;

    let tmp = p.x * p.x + p.y * p.y + 1.0;
    let x2 = 2.0 * p.x;
    let y2 = 2.0 * p.y;
    let xmax = 0.5 * (sqrt(max(tmp + x2, 0.0)) + sqrt(max(tmp - x2, 0.0)));
    let ymax = 0.5 * (sqrt(max(tmp + y2, 0.0)) + sqrt(max(tmp - y2, 0.0)));

    let ax_n = p.x / max(xmax, 1e-30);
    let bx = sqrt(max(1.0 - ax_n * ax_n, 0.0));
    let out_x = v_in * atan2(ax_n, bx) * r * px;

    let ay_n = p.y / max(ymax, 1e-30);
    let by = sqrt(max(1.0 - ay_n * ay_n, 0.0));
    let out_y = v_in * atan2(ay_n, by) * r * py;

    // Z preserve scales with w (FPz += VVAR · FTz upstream); leave p.z so
    // the outer multiplier produces VVAR · p.z.
    return vec3<f32>(out_x * inv_w, out_y * inv_w, p.z);
}
"#,
};
