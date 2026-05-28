//! plusrecip (Dark-Beam, 2019)
//!
//! `k = z + sqrt(z² - a)`, then optionally conjugate-and-rotate when
//! the squared magnitude shrinks below |a|, then ensure positive real
//! part. Produces compelling Joukowski-airfoil-like patterns.
//!
//! 2 user params:
//!   - ar (real part of `a`, default 4.0)
//!   - ai (imag part of `a`, default 0.0)
//!
//! No init slots. Body factors cleanly through outer multiplier (cpp's
//! final `k.Scale(VVAR)` equals our outer w factor).
//!
//! Uses a complex-arithmetic toolkit (`pr_csqrt`, `pr_cabs`,
//! `pr_cmul`) implemented inline. Names prefixed with `pr_` to avoid
//! collisions.
//!
//! Source: `output/jwildfire-vars/output/plusrecip.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Joukowski-airfoil-style complex map — computes `k = z + sqrt(z² - a)`
/// (one root of the quadratic `k² - 2k·z + a = 0`), then mirrors small-
/// magnitude outputs back out via a conjugate-and-rotate step (`k → conj(k)
/// · (-a/|a|)` when `|k|² < |a|`) and forces a positive real part. Named
/// for its resemblance to the Joukowski transform `z + 1/z`.
///
/// # Authors
/// - DarkBeam
pub static PLUSRECIP: VariationDef = VariationDef {
    name: "plusrecip",
    aliases: &[],
    display_name: "Plus Recip",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("ar", "A Real", unlimited_float, 4.0, -10.0, 10.0, "Real part of the complex constant `a` in `k = z + sqrt(z² - a)`. Controls the location of the branch cut."),
        param!("ai", "A Imag", unlimited_float, 0.0, -10.0, 10.0, "Imaginary part of the complex constant `a`."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn pr_cabs(z: vec2<f32>) -> f32 {
    return sqrt(z.x * z.x + z.y * z.y);
}
fn pr_cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}
fn pr_csqrt(z: vec2<f32>) -> vec2<f32> {
    let r = pr_cabs(z);
    let real_p = sqrt(max((r + z.x) * 0.5, 0.0));
    let imag_p = sqrt(max((r - z.x) * 0.5, 0.0));
    let s = select(1.0, -1.0, z.y < 0.0);
    return vec2<f32>(real_p, s * imag_p);
}

fn variation_plusrecip(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let ar = get_param(xform_id, variation_id, 0u);
    let ai = get_param(xform_id, variation_id, 1u);
    let z = vec2<f32>(p.x, p.y);
    let a = vec2<f32>(ar, ai);
    let aa = sqrt(a.x * a.x + a.y * a.y + 1e-30);

    let z2 = pr_cmul(z, z);
    var k = pr_csqrt(z2 - a) + z;

    let k2 = pr_cmul(k, k);
    if (sqrt(k2.x * k2.x + k2.y * k2.y + 1e-30) < aa) {
        let k_conj = vec2<f32>(k.x, -k.y);
        let a_scaled = -a / aa;
        k = pr_cmul(k_conj, a_scaled);
    }

    if (k.x < 0.0) {
        k = -k;
    }
    return k;
}
"#,
    wgsl_3d: Some(r#"
fn pr_cabs(z: vec2<f32>) -> f32 {
    return sqrt(z.x * z.x + z.y * z.y);
}
fn pr_cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}
fn pr_csqrt(z: vec2<f32>) -> vec2<f32> {
    let r = pr_cabs(z);
    let real_p = sqrt(max((r + z.x) * 0.5, 0.0));
    let imag_p = sqrt(max((r - z.x) * 0.5, 0.0));
    let s = select(1.0, -1.0, z.y < 0.0);
    return vec2<f32>(real_p, s * imag_p);
}

fn variation_plusrecip(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let ar = get_param(xform_id, variation_id, 0u);
    let ai = get_param(xform_id, variation_id, 1u);
    let z = vec2<f32>(p.x, p.y);
    let a = vec2<f32>(ar, ai);
    let aa = sqrt(a.x * a.x + a.y * a.y + 1e-30);

    let z2 = pr_cmul(z, z);
    var k = pr_csqrt(z2 - a) + z;

    let k2 = pr_cmul(k, k);
    if (sqrt(k2.x * k2.x + k2.y * k2.y + 1e-30) < aa) {
        let k_conj = vec2<f32>(k.x, -k.y);
        let a_scaled = -a / aa;
        k = pr_cmul(k_conj, a_scaled);
    }

    if (k.x < 0.0) {
        k = -k;
    }
    return vec3<f32>(k.x, k.y, p.z);
}
"#),
};
