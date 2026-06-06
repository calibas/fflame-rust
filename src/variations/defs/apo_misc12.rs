//! Apophysis miscellany 12 — four more
//!
//!   - `rings`     (Scott Draves)         — 0 user; reads affine `xf.e`
//!                                            (XFORM_COEFF_20) for `dx`;
//!                                            output `(y/r0, x/r0)`
//!                                            matches both flam3 and
//!                                            JWildfire (their `cosA`/
//!                                            `sinA` use swapped-angle
//!                                            precalc, so `cosA = y/r0`
//!                                            and `sinA = x/r0` there);
//!                                            needs_transform divide-out
//!                                            (cpp body lacks VVAR on
//!                                            output)
//!   - `rippled`   (Raykoid666)          — 0 user; clean factor through
//!                                            outer (`w/2 · tanh(d) · 2x
//!                                            = w · tanh(d) · x`)
//!   - `waffle`    (Jed Kelsey)          — 4 user (slices int,
//!                                            xthickness, ythickness,
//!                                            rotation); 2 init slots
//!                                            (cos_rot, sin_rot — w
//!                                            stripped from cpp's
//!                                            `vcosr/vsinr` so it
//!                                            factors cleanly through
//!                                            outer); RNG
//!   - `stripfit`  (dark-beam,
//!                  Java-recovered)       — 1 user (dx); cpp
//!                                            PluginVarCalc empty
//!                                            unported_stub; body has
//!                                            `(y − fity + 1) · dxp`
//!                                            and similar additive
//!                                            unscaled term on x;
//!                                            needs_transform divide-out
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// rings (Scott Draves)
//   dx = xf.e² + ε   (XFORM_COEFF_20 of the transform's affine)
//   r0 = sqrt(x² + y²) + ε
//   r = r0 + dx − floor((r0 + dx)/(2·dx)) · 2·dx − dx + r0 · (1 − dx)
//   out = r · (y/r0, x/r0)
//
// Output matches both flam3 (`var21_rings`: `nx = r·precalc_cosa`,
// `ny = r·precalc_sina`, with `precalc_*a` derived from the swapped
// `atan2(tx, ty)` so `cosa = ty/r0`, `sina = tx/r0`) and JWildfire
// (`RingsFunc.java`: `x += r·getPrecalcCosA()`, `y += r·getPrecalcSinA()`,
// same swapped-angle precalc). The "swap" relative to the input `(x, y)`
// is canonical for this variation, not a porter introduction.
// Body lacks VVAR on output — needs_transform divide-out.
// =============================================================================
/// Modular-radius ring warp — folds the input radius around `2·dx` cycles
/// where `dx = (transform's affine e coefficient)² + ε`, producing
/// concentric rings. Reads the affine `e` coefficient directly (not a
/// normal variation parameter), so the ring spacing changes with the
/// transform's pre-affine.
///
/// # Authors
/// - Scott Draves
pub static RINGS: VariationDef = VariationDef {
    name: "rings",
    aliases: &[],
    display_name: "Rings",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_rings(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xf = transforms[xform_id];
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let dx = xf.e * xf.e + 1e-30;
    let r0 = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let r = r0 + dx - floor((r0 + dx) / (2.0 * dx)) * 2.0 * dx - dx + r0 * (1.0 - dx);
    return vec2<f32>(r * p.y / r0 * inv_w, r * p.x / r0 * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_rings(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xf = transforms[xform_id];
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let dx = xf.e * xf.e + 1e-30;
    let r0 = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let r = r0 + dx - floor((r0 + dx) / (2.0 * dx)) * 2.0 * dx - dx + r0 * (1.0 - dx);
    return vec3<f32>(r * p.y / r0 * inv_w, r * p.x / r0 * inv_w, p.z);
}
"#,
};

// =============================================================================
// rippled (Raykoid666)
//   d = x² + y²
//   out = (1/2 · tanh(d + ε) · 2x, 1/2 · cos(d + ε) · 2y)
//       = (tanh(d) · x, cos(d) · y)
// Clean factor through outer.
// =============================================================================
/// Tanh/cos ripple — emits `(tanh(x²+y²) · x, cos(x²+y²) · y)`. Soft
/// saturation along X (via tanh) combined with cosine modulation along Y
/// produces a ripple/wave pattern.
///
/// # Authors
/// - Raykoid666
pub static RIPPLED: VariationDef = VariationDef {
    name: "rippled",
    aliases: &[],
    display_name: "Rippled",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_rippled(p: vec2<f32>) -> vec2<f32> {
    let d = p.x * p.x + p.y * p.y + 1e-30;
    return vec2<f32>(tanh(d) * p.x, cos(d) * p.y);
}
"#,
    wgsl_3d: r#"
fn variation_rippled(p: vec3<f32>) -> vec3<f32> {
    let d = p.x * p.x + p.y * p.y + 1e-30;
    return vec3<f32>(tanh(d) * p.x, cos(d) * p.y, p.z);
}
"#,
};

// =============================================================================
// waffle (Jed Kelsey)
//   4 user: slices (int), xthickness, ythickness, rotation
//   2 init: cos_rot = cos(rotation), sin_rot = sin(rotation)
//           (cpp's `vcosr = w · cos(rotation)` — w stripped here so
//           outer factors cleanly through)
//   5-way RNG select picks (a, r) cell coords from {floor(rand·slices) +
//     {…}} / slices forms
//   out = ( cos_rot · a + sin_rot · r,
//          -sin_rot · a + cos_rot · r)
// Clean factor through outer.
// =============================================================================
/// Waffle grid scatter — picks one of 5 sampling modes uniformly, then
/// samples from a `slices × slices` grid with configurable per-axis line
/// thickness. The output is rotated by `rotation`. Produces a woven-pattern
/// scatter.
///
/// # Authors
/// - Jed Kelsey
pub static WAFFLE: VariationDef = VariationDef {
    name: "waffle",
    aliases: &[],
    display_name: "Waffle",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("slices", "Slices", int, 6.0, 1.0, 100.0, "Number of grid divisions per axis."),
        param!("xthickness", "X thickness", unlimited_float, 0.5, -10.0, 10.0, "X-axis line thickness (0 = grid lines only, 1 = solid fill)."),
        param!("ythickness", "Y thickness", unlimited_float, 0.5, -10.0, 10.0, "Y-axis line thickness."),
        param!("rotation", "Rotation", unlimited_float, 0.0, -10.0, 10.0, "Output rotation in radians."),
    ],
    // 2 derived values at slots 4..6:
    //   4: cos_rot
    //   5: sin_rot
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_waffle(user: array<f32, 4>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = cos(user[3]);
    out[1] = sin(user[3]);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waffle(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let slices = max(get_param(xform_id, variation_id, 0u), 1.0);
    let xth = get_param(xform_id, variation_id, 1u);
    let yth = get_param(xform_id, variation_id, 2u);
    let cos_rot = get_param(xform_id, variation_id, 4u);
    let sin_rot = get_param(xform_id, variation_id, 5u);

    let inv_s = 1.0 / slices;
    let case_i = i32(floor(rng_nextf(rng) * 5.0));
    var a: f32 = 0.0;
    var r: f32 = 0.0;
    if (case_i == 0) {
        a = (floor(rng_nextf(rng) * slices) + rng_nextf(rng) * xth) * inv_s;
        r = (floor(rng_nextf(rng) * slices) + rng_nextf(rng) * yth) * inv_s;
    } else if (case_i == 1) {
        a = (floor(rng_nextf(rng) * slices) + rng_nextf(rng)) * inv_s;
        r = (floor(rng_nextf(rng) * slices) + yth) * inv_s;
    } else if (case_i == 2) {
        a = (floor(rng_nextf(rng) * slices) + xth) * inv_s;
        r = (floor(rng_nextf(rng) * slices) + rng_nextf(rng)) * inv_s;
    } else if (case_i == 3) {
        a = rng_nextf(rng);
        r = (floor(rng_nextf(rng) * slices) + yth + rng_nextf(rng) * (1.0 - yth)) * inv_s;
    } else {
        a = (floor(rng_nextf(rng) * slices) + xth + rng_nextf(rng) * (1.0 - xth)) * inv_s;
        r = rng_nextf(rng);
    }
    return vec2<f32>(cos_rot * a + sin_rot * r, -sin_rot * a + cos_rot * r);
}
"#,
    wgsl_3d: r#"
fn variation_waffle(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let slices = max(get_param(xform_id, variation_id, 0u), 1.0);
    let xth = get_param(xform_id, variation_id, 1u);
    let yth = get_param(xform_id, variation_id, 2u);
    let cos_rot = get_param(xform_id, variation_id, 4u);
    let sin_rot = get_param(xform_id, variation_id, 5u);

    let inv_s = 1.0 / slices;
    let case_i = i32(floor(rng_nextf(rng) * 5.0));
    var a: f32 = 0.0;
    var r: f32 = 0.0;
    if (case_i == 0) {
        a = (floor(rng_nextf(rng) * slices) + rng_nextf(rng) * xth) * inv_s;
        r = (floor(rng_nextf(rng) * slices) + rng_nextf(rng) * yth) * inv_s;
    } else if (case_i == 1) {
        a = (floor(rng_nextf(rng) * slices) + rng_nextf(rng)) * inv_s;
        r = (floor(rng_nextf(rng) * slices) + yth) * inv_s;
    } else if (case_i == 2) {
        a = (floor(rng_nextf(rng) * slices) + xth) * inv_s;
        r = (floor(rng_nextf(rng) * slices) + rng_nextf(rng)) * inv_s;
    } else if (case_i == 3) {
        a = rng_nextf(rng);
        r = (floor(rng_nextf(rng) * slices) + yth + rng_nextf(rng) * (1.0 - yth)) * inv_s;
    } else {
        a = (floor(rng_nextf(rng) * slices) + xth + rng_nextf(rng) * (1.0 - xth)) * inv_s;
        r = rng_nextf(rng);
    }
    return vec3<f32>(cos_rot * a + sin_rot * r, -sin_rot * a + cos_rot * r, p.z);
}
"#,
};

// =============================================================================
// stripfit (dark-beam, Java-recovered)
//   1 user (dx)
//   dxp = -0.5 · dx
//   if y > 1:    fity = (y + 1) mod 2;  fy = -1 + fity;  fx_extra =
//                  (y - fity + 1) · dxp
//   if y < -1:   fity = (1 - y) mod 2;  fy =  1 - fity;  fx_extra =
//                  (y + fity - 1) · dxp
//   else:        fy = y;                                  fx_extra = 0
//   cpp has FPx += w·x and FPx += fx_extra (two contributions);
//   FPy += w·fy.
//   Body has `+ fx_extra` not w-scaled; needs_transform divide-out
//   (`fx_extra / w` carried so outer × w restores).
// =============================================================================
/// Strip-fit fold — folds the Y axis into `[-1, 1]` strips (via mod-2
/// wrapping), with each fold adding a horizontal shift on X proportional to
/// `dx · stripe_count`. The trailing X shift wasn't VVAR-scaled in upstream
/// — handled via the divide-out pattern.
///
/// # Authors
/// - DarkBeam
pub static STRIPFIT: VariationDef = VariationDef {
    name: "stripfit",
    aliases: &[],
    display_name: "Stripfit",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[
        param!("dx", "DX", unlimited_float, 1.0, -10.0, 10.0, "Per-strip horizontal shift amount (scaled by `-0.5` internally)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_stripfit(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let dx = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let dxp = -0.5 * dx;

    var fy: f32;
    var fx_extra: f32 = 0.0;
    if (p.y > 1.0) {
        let fity = (p.y + 1.0) - floor((p.y + 1.0) * 0.5) * 2.0;
        fy = -1.0 + fity;
        fx_extra = (p.y - fity + 1.0) * dxp;
    } else if (p.y < -1.0) {
        let fity = (1.0 - p.y) - floor((1.0 - p.y) * 0.5) * 2.0;
        fy = 1.0 - fity;
        fx_extra = (p.y + fity - 1.0) * dxp;
    } else {
        fy = p.y;
    }
    return vec2<f32>(p.x + fx_extra * inv_w, fy);
}
"#,
    wgsl_3d: r#"
fn variation_stripfit(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let dx = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let dxp = -0.5 * dx;

    var fy: f32;
    var fx_extra: f32 = 0.0;
    if (p.y > 1.0) {
        let fity = (p.y + 1.0) - floor((p.y + 1.0) * 0.5) * 2.0;
        fy = -1.0 + fity;
        fx_extra = (p.y - fity + 1.0) * dxp;
    } else if (p.y < -1.0) {
        let fity = (1.0 - p.y) - floor((1.0 - p.y) * 0.5) * 2.0;
        fy = 1.0 - fity;
        fx_extra = (p.y + fity - 1.0) * dxp;
    } else {
        fy = p.y;
    }
    return vec3<f32>(p.x + fx_extra * inv_w, fy, p.z);
}
"#,
};
