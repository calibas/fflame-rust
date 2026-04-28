//! Square-root-prefixed inverse hyperbolic variations
//!
//! Ports from JWildfire/Chaotica. Each is `f(sqrt(z))` for some inverse
//! hyperbolic `f`, scaled by `2/π`, with a random ±sign chosen each
//! iteration.
//!
//! Notes:
//!   - All six use `GOODRAND_01` upstream, so `needs_rng = true`.
//!   - `sqrt_asech` upstream actually calls `complexAcosH(sqrt(z))` rather
//!     than the `arcsech` formula — it's a copy-paste bug from the
//!     `sqrt_acosh` source. Preserved here so flames match upstream.
//!   - The 3D forms preserve `p.z` (matching upstream's
//!     `if isPreserveZCoordinate() FPz += pAmount * FTz`).

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

// =============================================================================
// sqrt_acoth: AcotH(sqrt(z)) · (2/π) · ±
//   AcotH(w) = (1/2) · ln((w+1)/(w-1))
// =============================================================================
pub static SQRT_ACOTH: VariationDef = VariationDef {
    name: "sqrt_acoth",
    display_name: "Sqrt ACoth",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sqrt_acoth(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // w = sqrt(p) — principal branch
    let r = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r + p.x));
    let w_im_mag = sqrt(0.5 * max(r - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    // (w+1) / (w-1)
    let denom_mag2 = max((w_re - 1.0) * (w_re - 1.0) + w_im * w_im, 1e-20);
    let ratio_re = ((w_re + 1.0) * (w_re - 1.0) + w_im * w_im) / denom_mag2;
    let ratio_im = -2.0 * w_im / denom_mag2;
    // (1/2) · ln(ratio)
    let log_re = 0.25 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = 0.5 * atan2(ratio_im, ratio_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec2<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sqrt_acoth(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let r = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r + p.x));
    let w_im_mag = sqrt(0.5 * max(r - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let denom_mag2 = max((w_re - 1.0) * (w_re - 1.0) + w_im * w_im, 1e-20);
    let ratio_re = ((w_re + 1.0) * (w_re - 1.0) + w_im * w_im) / denom_mag2;
    let ratio_im = -2.0 * w_im / denom_mag2;
    let log_re = 0.25 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = 0.5 * atan2(ratio_im, ratio_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im, p.z);
}
"#),
};

// =============================================================================
// sqrt_acosh: AcosH(sqrt(z)) · (2/π) · ±
//   AcosH(w) = ln(w + sqrt(w² - 1))
// =============================================================================
pub static SQRT_ACOSH: VariationDef = VariationDef {
    name: "sqrt_acosh",
    display_name: "Sqrt ACosh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sqrt_acosh(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // w = sqrt(p)
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    // w² - 1
    let z2_re = w_re * w_re - w_im * w_im - 1.0;
    let z2_im = 2.0 * w_re * w_im;
    // sqrt(w² - 1)
    let r1 = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r1 + z2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    // arg = w + sqrt(w² - 1); log(arg)
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec2<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sqrt_acosh(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let z2_re = w_re * w_re - w_im * w_im - 1.0;
    let z2_im = 2.0 * w_re * w_im;
    let r1 = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r1 + z2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im, p.z);
}
"#),
};

// =============================================================================
// sqrt_acosech: AcosecH(sqrt(z)) · (2/π) · Flip · ±
//   AcosecH(w) = ArcSinh(1/w) = ln(1/w + sqrt(1/w² + 1))
// =============================================================================
pub static SQRT_ACOSECH: VariationDef = VariationDef {
    name: "sqrt_acosech",
    display_name: "Sqrt ACosecH",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sqrt_acosech(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // w = sqrt(p)
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    // u = 1/w
    let w_mag2 = max(w_re * w_re + w_im * w_im, 1e-20);
    let u_re =  w_re / w_mag2;
    let u_im = -w_im / w_mag2;
    // u² + 1
    let u2_re = u_re * u_re - u_im * u_im + 1.0;
    let u2_im = 2.0 * u_re * u_im;
    // sqrt(u² + 1)
    let r1 = sqrt(u2_re * u2_re + u2_im * u2_im);
    let s_re = sqrt(0.5 * (r1 + u2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - u2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, u2_im < 0.0);
    // arg = u + sqrt(u² + 1); log(arg)
    let arg_re = u_re + s_re;
    let arg_im = u_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec2<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sqrt_acosech(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let w_mag2 = max(w_re * w_re + w_im * w_im, 1e-20);
    let u_re =  w_re / w_mag2;
    let u_im = -w_im / w_mag2;
    let u2_re = u_re * u_re - u_im * u_im + 1.0;
    let u2_im = 2.0 * u_re * u_im;
    let r1 = sqrt(u2_re * u2_re + u2_im * u2_im);
    let s_re = sqrt(0.5 * (r1 + u2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - u2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, u2_im < 0.0);
    let arg_re = u_re + s_re;
    let arg_im = u_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im, p.z);
}
"#),
};

// =============================================================================
// sqrt_asech: NOTE — upstream uses AcosH(sqrt(z)), not AsecH. Copy-paste bug
// from sqrt_acosh; preserved for parity. Equivalent to sqrt_acosh.
// =============================================================================
pub static SQRT_ASECH: VariationDef = VariationDef {
    name: "sqrt_asech",
    display_name: "Sqrt ASech",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sqrt_asech(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let z2_re = w_re * w_re - w_im * w_im - 1.0;
    let z2_im = 2.0 * w_re * w_im;
    let r1 = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r1 + z2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec2<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sqrt_asech(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let z2_re = w_re * w_re - w_im * w_im - 1.0;
    let z2_im = 2.0 * w_re * w_im;
    let r1 = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r1 + z2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im, p.z);
}
"#),
};

// =============================================================================
// sqrt_asinh: AsinH(sqrt(z)) · (2/π) · ±
//   AsinH(w) = ln(w + sqrt(w² + 1))
// =============================================================================
pub static SQRT_ASINH: VariationDef = VariationDef {
    name: "sqrt_asinh",
    display_name: "Sqrt ASinh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sqrt_asinh(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let z2_re = w_re * w_re - w_im * w_im + 1.0;
    let z2_im = 2.0 * w_re * w_im;
    let r1 = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r1 + z2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec2<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sqrt_asinh(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let z2_re = w_re * w_re - w_im * w_im + 1.0;
    let z2_im = 2.0 * w_re * w_im;
    let r1 = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r1 + z2_re));
    let s_im_mag = sqrt(0.5 * max(r1 - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im, p.z);
}
"#),
};

// =============================================================================
// sqrt_atanh: AtanH(sqrt(z)) · (2/π) · ±
//   AtanH(w) = (1/2) · ln((1+w)/(1-w))
// =============================================================================
pub static SQRT_ATANH: VariationDef = VariationDef {
    name: "sqrt_atanh",
    display_name: "Sqrt ATanh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sqrt_atanh(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let one_over_pi = 0.3183098861837907;
    // w = sqrt(p)
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    // (1 + w) / (1 - w)
    let denom_mag2 = max((1.0 - w_re) * (1.0 - w_re) + w_im * w_im, 1e-20);
    let ratio_re = ((1.0 + w_re) * (1.0 - w_re) - w_im * w_im) / denom_mag2;
    let ratio_im = (w_im * (1.0 - w_re) + w_im * (1.0 + w_re)) / denom_mag2;
    // (1/2) · ln(ratio)
    let log_re = 0.5 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = atan2(ratio_im, ratio_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    // Combined factor: (2/π) · (1/2) = 1/π
    return vec2<f32>(sign_flip * one_over_pi * 0.5 * log_re, sign_flip * one_over_pi * 0.5 * log_im);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sqrt_atanh(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let one_over_pi = 0.3183098861837907;
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let w_re = sqrt(0.5 * (r0 + p.x));
    let w_im_mag = sqrt(0.5 * max(r0 - p.x, 0.0));
    let w_im = select(w_im_mag, -w_im_mag, p.y < 0.0);
    let denom_mag2 = max((1.0 - w_re) * (1.0 - w_re) + w_im * w_im, 1e-20);
    let ratio_re = ((1.0 + w_re) * (1.0 - w_re) - w_im * w_im) / denom_mag2;
    let ratio_im = (w_im * (1.0 - w_re) + w_im * (1.0 + w_re)) / denom_mag2;
    let log_re = 0.5 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = atan2(ratio_im, ratio_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * one_over_pi * 0.5 * log_re, sign_flip * one_over_pi * 0.5 * log_im, p.z);
}
"#),
};
