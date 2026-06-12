//! Direct-color variations: dc_cylinder, dc_cylinder2, dc_triangle
//!
//! Three FracFx / Apophysis "dc_*" variations. Each writes a color value
//! computed from the variation's spatial output (no `*vc` read required —
//! they're write-only DC variations, same shape as dc_linear).
//!
//!   - dc_cylinder (FracFx): 6 user (offset, angle, scale, x, y, blur)
//!     + 4 init slots (_ldcs = 1/scale, _ldca = offset·π, cos_angle,
//!     sin_angle). Body has `FPy += rr + FTy·y` with no VVAR scaling —
//!     needs_transform divide-out for the y component. cpp's persistent
//!     `_r[4]` state replaced with fresh randoms per call (compromise:
//!     would need 4 thread-state slots to fully replicate).
//!
//!   - dc_cylinder2 (FracFx): 6 user + 4 init slots. Same structure
//!     as dc_cylinder but `FPy += rr·FTy·y` (no `+rr` constant).
//!     Same divide-out + fresh-randoms compromise.
//!
//!   - dc_triangle: barycentric-coordinate triangle mapping using the
//!     transform's affine as triangle vertices. 2 user (scatter_area,
//!     zero_edges int) + 1 init (_A = clamp(scatter_area, -1, 1)).
//!     RNG. Body factors cleanly. Reads xf.a/b/c/d/e/f via
//!     needs_transform. Color = `fmod(|u + v|, 1)` of the barycentric
//!     coords — straight write of the spatial output's "position
//!     inside the triangle" value.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/dc_cylinder.cpp`
//!   - `output/jwildfire-vars/output/dc_cylinder2.cpp`
//!   - `output/jwildfire-vars/output/dc_triangle.cpp`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// dc_cylinder
// ---------------------------------------------------------------------------

/// Direct-color cylinder warp — wraps the input around a cylinder via
/// `sin(x + rr·sin(a))` on X with a Gaussian-approximated Y blur (`rr =
/// blur · (sum of 4 uniforms − 2)`). 3D mode adds a `cos(x + rr·cos(a))` Z
/// component. Color is written from a rotated projection of the weighted
/// output position (`TC = fmod(|0.5 · (ldcs · proj + 1)|, 1)`), matching
/// the cpp/Java original. Visible color requires the transform's Direct
/// Color slider > 0.
///
/// # Authors
/// - FracFx
pub static DC_CYLINDER: VariationDef = VariationDef {
    name: "dc_cylinder",
    aliases: &[],
    display_name: "DC Cylinder",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::WritesColor, Feature::AlwaysZ],
    parameters: &[
        param!("offset", "Offset", unlimited_float, 0.0, -10.0, 10.0, "Color-projection offset. Init precomputes `offset · π` for parity (unused) and the raw `offset` is added to the projection."),
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0, "Color-projection rotation angle (radians). Init precomputes `cos(angle)` and `sin(angle)`."),
        param!("scale", "Scale", unlimited_float, 0.5, -10.0, 10.0, "Color-projection scale. Init precomputes `1/scale` (with zero-guard)."),
        param!("x", "X", unlimited_float, 0.125, -10.0, 10.0, "X-axis output scale (multiplies the `sin(x + rr·sin(a))` term)."),
        param!("y", "Y", unlimited_float, 0.125, -10.0, 10.0, "Y-axis output scale (multiplies the input Y in the output)."),
        param!("blur", "Blur", unlimited_float, 1.0, -10.0, 10.0, "Gaussian blur amplitude — multiplies the sum-of-4-uniforms approximation."),
    ],
    // 4 init slots (6..10):
    //   6: ldcs = 1/scale
    //   7: ldca = offset·π  (kept for cpp parity, unused in body)
    //   8: cos(angle)
    //   9: sin(angle)
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_dc_cylinder(user: array<f32, 6>) -> array<f32, 4> {
    var out: array<f32, 4>;
    let pi = 3.14159265358979;
    let scale = user[2];
    let safe_scale = select(scale, 1e-5, abs(scale) < 1e-30);
    out[0] = 1.0 / safe_scale;
    out[1] = user[0] * pi;
    out[2] = cos(user[1]);
    out[3] = sin(user[1]);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_dc_cylinder(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let offset = get_param(xform_id, variation_id, 0u);
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let ldcs = get_param(xform_id, variation_id, 6u);
    let cos_a = get_param(xform_id, variation_id, 8u);
    let sin_a = get_param(xform_id, variation_id, 9u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr + p.y * yp) * inv_w;

    // Color from rotated projection of weighted output (matches cpp's
    // FPx/FPy after the weighted accumulator). `w * nx` and `w * ny` give
    // the eventual accumulator contributions of this variation.
    let fpx = w * nx;
    let fpy = w * ny;
    let proj = cos_a * fpx + sin_a * fpy + offset;
    let raw = abs(0.5 * (ldcs * proj + 1.0));
    *vc = raw - floor(raw);

    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_dc_cylinder(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let offset = get_param(xform_id, variation_id, 0u);
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let ldcs = get_param(xform_id, variation_id, 6u);
    let cos_a = get_param(xform_id, variation_id, 8u);
    let sin_a = get_param(xform_id, variation_id, 9u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let cr = cos(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr + p.y * yp) * inv_w;
    let nz = cos(p.x + rr * cr);

    let fpx = w * nx;
    let fpy = w * ny;
    let proj = cos_a * fpx + sin_a * fpy + offset;
    let raw = abs(0.5 * (ldcs * proj + 1.0));
    *vc = raw - floor(raw);

    return vec3<f32>(nx, ny, nz);
}
"#,
};

// ---------------------------------------------------------------------------
// dc_cylinder2
// ---------------------------------------------------------------------------

/// Variant of `dc_cylinder` — same XY/Z structure but the Y output is `rr ·
/// y · y_scale` (multiplicative) instead of `rr + y · y_scale` (additive).
/// Same color-projection write.
///
/// # Authors
/// - FracFx
pub static DC_CYLINDER2: VariationDef = VariationDef {
    name: "dc_cylinder2",
    aliases: &[],
    display_name: "DC Cylinder 2",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::WritesColor, Feature::AlwaysZ],
    parameters: &[
        param!("offset", "Offset", unlimited_float, 0.0, -10.0, 10.0, "Color-projection offset. Init precomputes `offset · π` for parity (unused) and the raw `offset` is added to the projection."),
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0, "Color-projection rotation angle (radians). Init precomputes `cos(angle)` and `sin(angle)`."),
        param!("scale", "Scale", unlimited_float, 0.5, -10.0, 10.0, "Color-projection scale. Init precomputes `1/scale` (with zero-guard)."),
        param!("x", "X", unlimited_float, 0.125, -10.0, 10.0, "X-axis output scale (multiplies the `sin(x + rr·sin(a))` term)."),
        param!("y", "Y", unlimited_float, 0.125, -10.0, 10.0, "Y-axis output scale (multiplies the input Y in the output)."),
        param!("blur", "Blur", unlimited_float, 1.0, -10.0, 10.0, "Gaussian blur amplitude — multiplies the sum-of-4-uniforms approximation."),
    ],
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_dc_cylinder2(user: array<f32, 6>) -> array<f32, 4> {
    var out: array<f32, 4>;
    let pi = 3.14159265358979;
    let scale = user[2];
    let safe_scale = select(scale, 1e-5, abs(scale) < 1e-30);
    out[0] = 1.0 / safe_scale;
    out[1] = user[0] * pi;
    out[2] = cos(user[1]);
    out[3] = sin(user[1]);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_dc_cylinder2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let offset = get_param(xform_id, variation_id, 0u);
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let ldcs = get_param(xform_id, variation_id, 6u);
    let cos_a = get_param(xform_id, variation_id, 8u);
    let sin_a = get_param(xform_id, variation_id, 9u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr * p.y * yp) * inv_w;

    let fpx = w * nx;
    let fpy = w * ny;
    let proj = cos_a * fpx + sin_a * fpy + offset;
    let raw = abs(0.5 * (ldcs * proj + 1.0));
    *vc = raw - floor(raw);

    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_dc_cylinder2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let offset = get_param(xform_id, variation_id, 0u);
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let ldcs = get_param(xform_id, variation_id, 6u);
    let cos_a = get_param(xform_id, variation_id, 8u);
    let sin_a = get_param(xform_id, variation_id, 9u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let cr = cos(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr * p.y * yp) * inv_w;
    let nz = cos(p.x + rr * cr);

    let fpx = w * nx;
    let fpy = w * ny;
    let proj = cos_a * fpx + sin_a * fpy + offset;
    let raw = abs(0.5 * (ldcs * proj + 1.0));
    *vc = raw - floor(raw);

    return vec3<f32>(nx, ny, nz);
}
"#,
};

// ---------------------------------------------------------------------------
// dc_triangle
// ---------------------------------------------------------------------------

/// Barycentric-coordinate triangle mapping with direct-color writes — uses
/// the transform's affine `(a, b)` and `(-c, -d)` as the triangle edge
/// basis, with `(e, f)` as the origin vertex. Tests whether the input lies
/// inside the triangle via barycentric coordinates `(u, v)`. Inside, passes
/// through. Outside, either collapses to origin (`zero_edges = 1`) or
/// scatters by `scatter_area` from the nearest edge. Color is `fmod(|u +
/// v|, 1)` of the post-clamp barycentric coords (matches cpp `TC =
/// fmod(fabs(u+v), 1.0)`).
pub static DC_TRIANGLE: VariationDef = VariationDef {
    name: "dc_triangle",
    aliases: &[],
    display_name: "DC Triangle",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::WritesColor],
    parameters: &[
        param!("scatter_area", "Scatter Area", unlimited_float, 0.0, -1.0, 1.0, "Random scatter amount applied to points outside the triangle. Clamped to `[-1, 1]` internally."),
        param!("zero_edges", "Zero Edges", bool, false, "When on, out-of-triangle points collapse to the origin. When off, they scatter by `scatter_area`."),
    ],
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_dc_triangle(user: array<f32, 2>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = clamp(user[0], -1.0, 1.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_dc_triangle(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let zero_edges = i32(get_param(xform_id, variation_id, 1u));
    let a_param = get_param(xform_id, variation_id, 2u);
    let xf = transforms[xform_id];
    let xx = xf.a;
    let xy = xf.b;
    let yx = -xf.c;
    let yy = -xf.d;
    let ox = xf.e;
    let oy = xf.f;
    let px = p.x - ox;
    let py = p.y - oy;

    let dot00 = xx * xx + xy * xy;
    let dot01 = xx * yx + xy * yy;
    let dot02 = xx * px + xy * py;
    let dot11 = yx * yx + yy * yy;
    let dot12 = yx * px + yy * py;

    let denom = dot00 * dot11 - dot01 * dot01;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var u = (dot11 * dot02 - dot01 * dot12) / safe_denom;
    var v = (dot00 * dot12 - dot01 * dot02) / safe_denom;
    var inside = false;
    var f = 1.0;

    if (u + v > 1.0) {
        f = -1.0;
        if (u > v) {
            u = min(u, 1.0);
            v = 1.0 - u;
        } else {
            v = min(v, 1.0);
            u = 1.0 - v;
        }
    } else if (u < 0.0 || v < 0.0) {
        u = clamp(u, 0.0, 1.0);
        v = clamp(v, 0.0, 1.0);
    } else {
        inside = true;
    }

    if (zero_edges != 0 && !inside) {
        u = 0.0;
        v = 0.0;
    } else if (!inside) {
        u = u + rng_nextf(rng) * a_param * f;
        v = v + rng_nextf(rng) * a_param * f;
        u = clamp(u, -1.0, 1.0);
        v = clamp(v, -1.0, 1.0);
        if (u + v > 1.0 && a_param > 0.0) {
            if (u > v) {
                u = min(u, 1.0);
                v = 1.0 - u;
            } else {
                v = min(v, 1.0);
                u = 1.0 - v;
            }
        }
    }

    // Color from final barycentric coords: TC = fmod(|u+v|, 1.0)
    let raw = abs(u + v);
    *vc = raw - floor(raw);

    return vec2<f32>(ox + u * xx + v * yx, oy + u * xy + v * yy);
}
"#,
    wgsl_3d: r#"
fn variation_dc_triangle(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let zero_edges = i32(get_param(xform_id, variation_id, 1u));
    let a_param = get_param(xform_id, variation_id, 2u);
    let xf = transforms[xform_id];
    let xx = xf.a;
    let xy = xf.b;
    let yx = -xf.c;
    let yy = -xf.d;
    let ox = xf.e;
    let oy = xf.f;
    let px = p.x - ox;
    let py = p.y - oy;

    let dot00 = xx * xx + xy * xy;
    let dot01 = xx * yx + xy * yy;
    let dot02 = xx * px + xy * py;
    let dot11 = yx * yx + yy * yy;
    let dot12 = yx * px + yy * py;

    let denom = dot00 * dot11 - dot01 * dot01;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var u = (dot11 * dot02 - dot01 * dot12) / safe_denom;
    var v = (dot00 * dot12 - dot01 * dot02) / safe_denom;
    var inside = false;
    var f = 1.0;

    if (u + v > 1.0) {
        f = -1.0;
        if (u > v) {
            u = min(u, 1.0);
            v = 1.0 - u;
        } else {
            v = min(v, 1.0);
            u = 1.0 - v;
        }
    } else if (u < 0.0 || v < 0.0) {
        u = clamp(u, 0.0, 1.0);
        v = clamp(v, 0.0, 1.0);
    } else {
        inside = true;
    }

    if (zero_edges != 0 && !inside) {
        u = 0.0;
        v = 0.0;
    } else if (!inside) {
        u = u + rng_nextf(rng) * a_param * f;
        v = v + rng_nextf(rng) * a_param * f;
        u = clamp(u, -1.0, 1.0);
        v = clamp(v, -1.0, 1.0);
        if (u + v > 1.0 && a_param > 0.0) {
            if (u > v) {
                u = min(u, 1.0);
                v = 1.0 - u;
            } else {
                v = min(v, 1.0);
                u = 1.0 - v;
            }
        }
    }

    let raw = abs(u + v);
    *vc = raw - floor(raw);

    return vec3<f32>(ox + u * xx + v * yx, oy + u * xy + v * yy, p.z);
}
"#,
};
