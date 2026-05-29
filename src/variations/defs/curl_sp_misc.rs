//! `curl_sp` (Xyrus02)
//!
//! Curl-power 3D variation with sign-preserving power and configurable
//! "spread" function. Operates on `(x, y, z) → (x^pow, y^pow, z^pow)`
//! preserving sign, then applies a curl-style Möbius warp scaled by
//! `r = w/c`.
//!
//!   - 6 user params: pow, c1, c2, sx, sy, dc
//!   - 4 init slots: c2_x2 (= 2·c2), dc_adjust (= 0.1·dc),
//!                    power_inv (= 1/pow), power (clamped to ε)
//!
//! Body has w in `r = w/c` (output) and `(z·w)/c` (Z output);
//! needs_transform divide-out.
//!
//! cpp's `TC = range(0, TC + dc_adjust·c, 1)` (read-modify-write color
//! update) is not portable to our writes_color model (which assigns
//! rather than adds). The `dc` parameter is preserved for preset
//! parity but the color write is dropped — flames built against the
//! cpp will lose the color drift.
//!
//! Source: `output/jwildfire-vars/output/curl_sp.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Curl-power 3D variation with sign-preserving power — raises each axis to
/// `pow` (preserving sign), applies a `c1`/`c2`-tuned Möbius-style curl
/// warp with a configurable spread function (`sx`, `sy` exponents), then
/// scales the output by `w/c` where `c = (re² + im²)^(1/pow)` is the
/// squared output magnitude raised to the inverse power.
///
/// # Authors
/// - Xyrus02
pub static CURL_SP: VariationDef = VariationDef {
    name: "curl_sp",
    aliases: &[],
    display_name: "Curl SP",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("pow", "Power", unlimited_float, 1.0, -10.0, 10.0, "Sign-preserving power exponent applied to each input axis."),
        param!("c1", "C1", unlimited_float, -0.01, -10.0, 10.0, "Linear-term coefficient in the curl warp."),
        param!("c2", "C2", unlimited_float, 0.03, -10.0, 10.0, "Quadratic-term coefficient in the curl warp."),
        param!("sx", "SX", unlimited_float, 0.0, -10.0, 10.0, "Spread function exponent for the X component."),
        param!("sy", "SY", unlimited_float, 0.0, -10.0, 10.0, "Spread function exponent for the Y component."),
        param!("dc", "DC", unlimited_float, 0.0, -10.0, 10.0, "Color drift amount. Preserved as a parameter for preset compatibility, but the actual color write was dropped during the port (see module header)."),
    ],
    needs_transform: true,
    writes_color: false,
    // 4 derived values at slots 6..10:
    //   6: power      (clamped to ≥ ε)
    //   7: c2_x2      (= 2·c2)
    //   8: power_inv  (= 1/power)
    //   9: dc_adjust  (= 0.1·dc, kept for preset parity even though
    //                  the color write is dropped)
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_curl_sp(user: array<f32, 6>) -> array<f32, 4> {
    let p_raw = user[0];
    let p_safe = select(p_raw, 1e-6, abs(p_raw) < 1e-30);
    var out: array<f32, 4>;
    out[0] = p_safe;
    out[1] = 2.0 * user[2];
    out[2] = 1.0 / p_safe;
    out[3] = 0.1 * user[5];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn curl_sp_powq4c(x: f32, e: f32) -> f32 {
    if (abs(e - 1.0) < 1e-30) {
        return x;
    }
    return pow(max(abs(x), 1e-30), e) * sign(x);
}

fn curl_sp_spread(a: f32, b: f32) -> f32 {
    return sqrt(a * a + b * b) * select(-1.0, 1.0, a > 0.0);
}

fn variation_curl_sp(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let c1 = get_param(xform_id, variation_id, 1u);
    let c2 = get_param(xform_id, variation_id, 2u);
    let sx = get_param(xform_id, variation_id, 3u);
    let sy = get_param(xform_id, variation_id, 4u);
    let power = get_param(xform_id, variation_id, 6u);
    let c2_x2 = get_param(xform_id, variation_id, 7u);
    let power_inv = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x = curl_sp_powq4c(p.x, power);
    let y = curl_sp_powq4c(p.y, power);
    let d = x * x - y * y;
    let re = curl_sp_spread(c1 * x + c2 * d, sx) + 1.0;
    let im = curl_sp_spread(c1 * y + c2_x2 * x * y, sy);
    let c = curl_sp_powq4c(re * re + im * im, power_inv);
    let safe_c = select(c, 1e-30, abs(c) < 1e-30);
    let r = w / safe_c;

    let fx = (x * re + y * im) * r;
    let fy = (y * re - x * im) * r;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn curl_sp_powq4c(x: f32, e: f32) -> f32 {
    if (abs(e - 1.0) < 1e-30) {
        return x;
    }
    return pow(max(abs(x), 1e-30), e) * sign(x);
}

fn curl_sp_spread(a: f32, b: f32) -> f32 {
    return sqrt(a * a + b * b) * select(-1.0, 1.0, a > 0.0);
}

fn variation_curl_sp(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let c1 = get_param(xform_id, variation_id, 1u);
    let c2 = get_param(xform_id, variation_id, 2u);
    let sx = get_param(xform_id, variation_id, 3u);
    let sy = get_param(xform_id, variation_id, 4u);
    let power = get_param(xform_id, variation_id, 6u);
    let c2_x2 = get_param(xform_id, variation_id, 7u);
    let power_inv = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x = curl_sp_powq4c(p.x, power);
    let y = curl_sp_powq4c(p.y, power);
    let z = curl_sp_powq4c(p.z, power);
    let d = x * x - y * y;
    let re = curl_sp_spread(c1 * x + c2 * d, sx) + 1.0;
    let im = curl_sp_spread(c1 * y + c2_x2 * x * y, sy);
    let c = curl_sp_powq4c(re * re + im * im, power_inv);
    let safe_c = select(c, 1e-30, abs(c) < 1e-30);
    let r = w / safe_c;

    let fx = (x * re + y * im) * r;
    let fy = (y * re - x * im) * r;
    let fz = z * w / safe_c;
    return vec3<f32>(fx * inv_w, fy * inv_w, fz * inv_w);
}
"#),
};
