//! Apophysis miscellany 14 — three more
//!
//!   - `waves2_radial`  (Tatyana Zabanova / Stefanov) — 6 user
//!                                                      params; radial
//!                                                      falloff applied
//!                                                      to waves2-style
//!                                                      sine offsets;
//!                                                      clean factor
//!                                                      through outer
//!   - `spliptic_bs`    (Brad Stefanov)               — 2 user (x, y) +
//!                                                      1 init slot
//!                                                      `_v_div_w = 2/π`
//!                                                      (so body has
//!                                                      `_v = w · _v_div_w`);
//!                                                      RNG;
//!                                                      needs_transform
//!                                                      divide-out for
//!                                                      the unscaled
//!                                                      `+ x` / `± y`
//!                                                      offsets.
//!                                                      cpp's `sqrt_safe`
//!                                                      macro-bug
//!                                                      bypassed (same
//!                                                      as blocky)
//!   - `poincare3D`     (Zueuk)                       — 3 user (r, a, b)
//!                                                      + 10 init slots
//!                                                      (cx, cy, cz, c2,
//!                                                      c2x, c2y, c2z,
//!                                                      s2x, s2y, s2z);
//!                                                      Full3D
//!                                                      hyperbolic tiling;
//!                                                      clean factor
//!                                                      through outer
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// waves2_radial (Tatyana Zabanova / Stefanov)
//   dist = sqrt(x² + y²)
//   factor = (dist < distance) ? (dist − null) / (distance − null) : 1
//   factor = (dist < null) ? 0 : factor
//   out = (x + factor · sin(y · freqx) · scalex,
//          y + factor · sin(x · freqy) · scaley)
// 6 user params; clean factor through outer.
// =============================================================================
/// Radially-falloff variant of `waves2` — applies the `waves2` per-axis
/// sine offsets (`sin(y·freqx)·scalex` on X, `sin(x·freqy)·scaley` on Y)
/// with a smooth radial falloff: full effect below the `null` radius, zero
/// above the `distance` radius, linearly interpolated in between.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov
pub static WAVES2_RADIAL: VariationDef = VariationDef {
    name: "waves2_radial",
    aliases: &[],
    display_name: "Waves2 Radial",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("w2r_scalex", "Scale X", unlimited_float, 0.1, -10.0, 10.0, "X-axis sine amplitude."),
        param!("w2r_scaley", "Scale Y", unlimited_float, 0.1, -10.0, 10.0, "Y-axis sine amplitude."),
        param!("w2r_freqx", "Freq X", unlimited_float, 7.0, -50.0, 50.0, "X-axis sine frequency."),
        param!("w2r_freqy", "Freq Y", unlimited_float, 13.0, -50.0, 50.0, "Y-axis sine frequency."),
        param!("w2r_null", "Null", unlimited_float, 2.0, -10.0, 10.0, "Inner radius — full effect below this distance from the origin."),
        param!("w2r_distance", "Distance", unlimited_float, 10.0, -100.0, 100.0, "Outer radius — zero effect above this distance."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waves2_radial(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let null_p = get_param(xform_id, variation_id, 4u);
    let distance = get_param(xform_id, variation_id, 5u);

    let dist = sqrt(p.x * p.x + p.y * p.y);
    let denom = distance - null_p;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var factor = select(1.0, (dist - null_p) / safe_denom, dist < distance);
    factor = select(factor, 0.0, dist < null_p);

    return vec2<f32>(p.x + factor * sin(p.y * freqx) * scalex,
                     p.y + factor * sin(p.x * freqy) * scaley);
}
"#,
    wgsl_3d: r#"
fn variation_waves2_radial(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let scalex = get_param(xform_id, variation_id, 0u);
    let scaley = get_param(xform_id, variation_id, 1u);
    let freqx = get_param(xform_id, variation_id, 2u);
    let freqy = get_param(xform_id, variation_id, 3u);
    let null_p = get_param(xform_id, variation_id, 4u);
    let distance = get_param(xform_id, variation_id, 5u);

    let dist = sqrt(p.x * p.x + p.y * p.y);
    let denom = distance - null_p;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var factor = select(1.0, (dist - null_p) / safe_denom, dist < distance);
    factor = select(factor, 0.0, dist < null_p);

    return vec3<f32>(p.x + factor * sin(p.y * freqx) * scalex,
                     p.y + factor * sin(p.x * freqy) * scaley,
                     p.z);
}
"#,
};

// =============================================================================
// spliptic_bs (Brad Stefanov)
//   _v_div_w = 2 / π   (so body computes _v = w · _v_div_w)
//   tmp = y² + x² + 1; x2 = 2x
//   xmax = (sqrt(max(tmp+x2,0)) + sqrt(max(tmp-x2,0))) / 2
//   a = x / xmax;  b = sqrt(max(1 − a², 0))
//   if x ≥ 0: fx = _v · atan2(a, b) + x_p
//   else:    fx = _v · atan2(a, b) − x_p
//   if rand < 0.5: fy =  _v · log(xmax + sqrt(max(xmax−1, 0))) + y_p
//   else:          fy = -_v · log(xmax + sqrt(max(xmax−1, 0))) − y_p
//   (cpp adds + y_p in else branch via `-(... + y_p)` in source, which
//    distributes to `... − y_p`. Preserved.)
// needs_transform divide-out: the `± x_p` / `± y_p` constant offsets
// don't factor through outer. Body returns cpp_output / w.
// =============================================================================
/// Elliptic-coordinate warp with random Y-sign + constant offsets —
/// converts the input to elliptic coordinates, emits `v · atan2(a, b)` on X
/// (sign-shifted by `±x_p`), and `±v · log(xmax + sqrt(xmax−1))` on Y (sign
/// chosen randomly each iteration, with `±y_p` offset). A Stefanov-tuned
/// elliptic warp.
///
/// # Authors
/// - Brad Stefanov
pub static SPLIPTIC_BS: VariationDef = VariationDef {
    name: "spliptic_bs",
    aliases: &[],
    display_name: "Spliptic BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[
        param!("x", "X", unlimited_float, 0.05, -10.0, 10.0, "X-axis constant offset. Sign added when input x ≥ 0, subtracted when input x < 0."),
        param!("y", "Y", unlimited_float, 0.05, -10.0, 10.0, "Y-axis constant offset. Sign chosen by the same random branch that picks the Y-output sign."),
    ],
    // 1 derived value at slot 2: _v_div_w = 2/π
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_spliptic_bs(user: array<f32, 2>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 0.6366197723675814;  // 2 / π
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_spliptic_bs(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let v_div_w = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let v = w * v_div_w;

    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(max(tmp + x2, 0.0)) + sqrt(max(tmp - x2, 0.0)));
    let safe_xmax = max(xmax, 1e-30);
    let a = p.x / safe_xmax;
    let b = sqrt(max(1.0 - a * a, 0.0));

    var fx: f32;
    if (p.x >= 0.0) {
        fx = v * atan2(a, b) + x_p;
    } else {
        fx = v * atan2(a, b) - x_p;
    }
    let log_arg = xmax + sqrt(max(xmax - 1.0, 0.0));
    let log_term = log(max(log_arg, 1e-30));
    var fy: f32;
    if (rng_nextf(rng) < 0.5) {
        fy = v * log_term + y_p;
    } else {
        fy = -v * log_term - y_p;
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_spliptic_bs(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let x_p = get_param(xform_id, variation_id, 0u);
    let y_p = get_param(xform_id, variation_id, 1u);
    let v_div_w = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let v = w * v_div_w;

    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(max(tmp + x2, 0.0)) + sqrt(max(tmp - x2, 0.0)));
    let safe_xmax = max(xmax, 1e-30);
    let a = p.x / safe_xmax;
    let b = sqrt(max(1.0 - a * a, 0.0));

    var fx: f32;
    if (p.x >= 0.0) {
        fx = v * atan2(a, b) + x_p;
    } else {
        fx = v * atan2(a, b) - x_p;
    }
    let log_arg = xmax + sqrt(max(xmax - 1.0, 0.0));
    let log_term = log(max(log_arg, 1e-30));
    var fy: f32;
    if (rng_nextf(rng) < 0.5) {
        fy = v * log_term + y_p;
    } else {
        fy = -v * log_term - y_p;
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// poincare3D (Zueuk)
//   3 user params: r, a, b
//   10 init slots (all derived from r, a, b):
//     cx = -r·cos(a·π/2)·cos(b·π/2)
//     cy =  r·sin(a·π/2)·cos(b·π/2)
//     cz = -r·sin(b·π/2)
//     c2 = cx² + cy² + cz²
//     c2x, c2y, c2z = 2·cx, 2·cy, 2·cz
//     s2x = cx² − cy² − cz² + 1
//     s2y = cy² − cx² − cz² + 1
//     s2z = cz² − cy² − cx² + 1
//   r2 = x² + y² + z²
//   x2cx = c2x · x;  y2cy = c2y · y;  z2cz = c2z · z
//   d = w / (c2·r2 − x2cx − y2cy − z2cz + 1)
//   out = d · ( x · s2x + cx · (y2cy + z2cz − r2 − 1),
//               y · s2y + cy · (x2cx + z2cz − r2 − 1),
//               z · s2z + cz · (y2cy + x2cx − r2 − 1))
// Full3D; clean factor through outer.
// =============================================================================
/// Poincaré-disc-style 3D hyperbolic-tiling generator — projects the input
/// through a Möbius inversion centered at `c = (-r·cos(a·π/2)·cos(b·π/2),
/// r·sin(a·π/2)·cos(b·π/2), -r·sin(b·π/2))`. The `r, a, b` parameters
/// control the center's distance from origin and its angular position on a
/// sphere of radius `r`.
///
/// # Authors
/// - Zueuk
pub static POINCARE3D: VariationDef = VariationDef {
    name: "poincare3D",
    aliases: &[],
    display_name: "Poincare 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::AlwaysZ],
    parameters: &[
        param!("r", "R", unlimited_float, 0.0, -10.0, 10.0, "Center distance from origin (radius of the inversion center)."),
        param!("a", "A", unlimited_float, 0.0, -10.0, 10.0, "Azimuthal angle of the inversion center (in units of π/2)."),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0, "Polar angle of the inversion center (in units of π/2)."),
    ],
    // 10 derived values at slots 3..13
    init_param_count: 10,
    wgsl_init: Some(r#"
fn init_poincare3D(user: array<f32, 3>) -> array<f32, 10> {
    let pi_2 = 1.5707963267948966;
    let r = user[0];
    let a = user[1];
    let b = user[2];
    let cb = cos(b * pi_2);
    let cx = -r * cos(a * pi_2) * cb;
    let cy = r * sin(a * pi_2) * cb;
    let cz = -r * sin(b * pi_2);
    let cx2 = cx * cx;
    let cy2 = cy * cy;
    let cz2 = cz * cz;
    var out: array<f32, 10>;
    out[0] = cx;
    out[1] = cy;
    out[2] = cz;
    out[3] = cx2 + cy2 + cz2;
    out[4] = 2.0 * cx;
    out[5] = 2.0 * cy;
    out[6] = 2.0 * cz;
    out[7] = cx2 - cy2 - cz2 + 1.0;
    out[8] = cy2 - cx2 - cz2 + 1.0;
    out[9] = cz2 - cy2 - cx2 + 1.0;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_poincare3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 3u);
    let cy = get_param(xform_id, variation_id, 4u);
    let c2 = get_param(xform_id, variation_id, 6u);
    let c2x = get_param(xform_id, variation_id, 7u);
    let c2y = get_param(xform_id, variation_id, 8u);
    let s2x = get_param(xform_id, variation_id, 10u);
    let s2y = get_param(xform_id, variation_id, 11u);

    let r2 = p.x * p.x + p.y * p.y;
    let x2cx = c2x * p.x;
    let y2cy = c2y * p.y;
    let denom = c2 * r2 - x2cx - y2cy + 1.0;
    let d = 1.0 / select(denom, 1e-30, abs(denom) < 1e-30);
    let fx = d * (p.x * s2x + cx * (y2cy - r2 - 1.0));
    let fy = d * (p.y * s2y + cy * (x2cx - r2 - 1.0));
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_poincare3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cx = get_param(xform_id, variation_id, 3u);
    let cy = get_param(xform_id, variation_id, 4u);
    let cz = get_param(xform_id, variation_id, 5u);
    let c2 = get_param(xform_id, variation_id, 6u);
    let c2x = get_param(xform_id, variation_id, 7u);
    let c2y = get_param(xform_id, variation_id, 8u);
    let c2z = get_param(xform_id, variation_id, 9u);
    let s2x = get_param(xform_id, variation_id, 10u);
    let s2y = get_param(xform_id, variation_id, 11u);
    let s2z = get_param(xform_id, variation_id, 12u);

    let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
    let x2cx = c2x * p.x;
    let y2cy = c2y * p.y;
    let z2cz = c2z * p.z;
    let denom = c2 * r2 - x2cx - y2cy - z2cz + 1.0;
    let d = 1.0 / select(denom, 1e-30, abs(denom) < 1e-30);
    let fx = d * (p.x * s2x + cx * (y2cy + z2cz - r2 - 1.0));
    let fy = d * (p.y * s2y + cy * (x2cx + z2cz - r2 - 1.0));
    let fz = d * (p.z * s2z + cz * (y2cy + x2cx - r2 - 1.0));
    return vec3<f32>(fx, fy, fz);
}
"#,
};
