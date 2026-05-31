//! Inverse hyperbolic / arc complex variations
//!
//! Ports from JWildfire/Chaotica. All of these are based on JWildfire's
//! `Complex` class — multiplied through by `pAmount * (2/π)` outside our
//! function (we just compute the underlying complex op and the per-variation
//! `2/π` factor; the shader applies `weight` on the outside).
//!
//! Reference (per-variation):
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/<name>.cpp
//!   - JWildfire Java source linked from each header.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

// =============================================================================
// acoth: (2/π) * Flip(AcotH(z))
//   AcotH(z) = (1/2) * ln((z+1)/(z-1))
//   Flip swaps real and imaginary parts (JWildfire convention)
// =============================================================================
/// Treats the input as a complex number and applies the inverse hyperbolic
/// cotangent, then swaps the real and imaginary parts. Creates two
/// singularity points at (±1, 0) with the pattern flowing between them.
/// 
/// # Authors
/// - Whittaker Courtney
pub static ACOTH: VariationDef = VariationDef {
    name: "acoth",
    aliases: &[],
    display_name: "ACoth",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_acoth(p: vec2<f32>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // (p+1) / (p-1): complex division
    let denom_mag2 = max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-20);
    let ratio_re = ((p.x + 1.0) * (p.x - 1.0) + p.y * p.y) / denom_mag2;
    let ratio_im = -2.0 * p.y / denom_mag2;
    // (1/2) * ln(ratio): complex log scaled by 1/2
    let log_re = 0.25 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = 0.5 * atan2(ratio_im, ratio_re);
    // Flip then scale by 2/π
    return vec2<f32>(two_over_pi * log_im, two_over_pi * log_re);
}
"#,
    wgsl_3d: r#"
fn variation_acoth(p: vec3<f32>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let denom_mag2 = max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-20);
    let ratio_re = ((p.x + 1.0) * (p.x - 1.0) + p.y * p.y) / denom_mag2;
    let ratio_im = -2.0 * p.y / denom_mag2;
    let log_re = 0.25 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = 0.5 * atan2(ratio_im, ratio_re);
    return vec3<f32>(two_over_pi * log_im, two_over_pi * log_re, p.z);
}
"#,
};

// =============================================================================
// acosh: (2/π) * AcosH(z), with random ±sign chosen each iteration
//   AcosH(z) = ln(z + sqrt(z² - 1))
// =============================================================================
/// Inverse hyperbolic cosine on the complex input. Each iteration randomly
/// picks one of the two branches, producing a symmetric upper/lower
/// pattern.
/// 
/// # Authors
/// - Whittaker Courtney
pub static ACOSH: VariationDef = VariationDef {
    name: "acosh",
    aliases: &[],
    display_name: "ACosh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_acosh(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // z² - 1 (complex)
    let z2_re = p.x * p.x - p.y * p.y - 1.0;
    let z2_im = 2.0 * p.x * p.y;
    // Principal sqrt of (z² - 1)
    let r = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r + z2_re));
    let s_im_mag = sqrt(0.5 * max(r - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    // arg = z + sqrt(z²-1)
    let arg_re = p.x + s_re;
    let arg_im = p.y + s_im;
    // log(arg)
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec2<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im);
}
"#,
    wgsl_3d: r#"
fn variation_acosh(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let z2_re = p.x * p.x - p.y * p.y - 1.0;
    let z2_im = 2.0 * p.x * p.y;
    let r = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r + z2_re));
    let s_im_mag = sqrt(0.5 * max(r - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    let arg_re = p.x + s_re;
    let arg_im = p.y + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * two_over_pi * log_re, sign_flip * two_over_pi * log_im, p.z);
}
"#,
};

// =============================================================================
// acosech: (2/π) * Flip(AcosecH(z)), with random ±sign each iteration
//   AcosecH(z) = ArcSinh(1/z) = ln(1/z + sqrt(1/z² + 1))
// =============================================================================
/// Inverse hyperbolic cosecant on the complex input (arcsinh of 1/z), then
/// swaps the real and imaginary parts. Random branch selection per
/// iteration produces symmetric two-branch patterns.
pub static ACOSECH: VariationDef = VariationDef {
    name: "acosech",
    aliases: &[],
    display_name: "ACosecH",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_acosech(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // w = 1/z (complex inverse)
    let p_mag2 = max(dot(p, p), 1e-20);
    let w_re =  p.x / p_mag2;
    let w_im = -p.y / p_mag2;
    // w² + 1
    let w2_re = w_re * w_re - w_im * w_im + 1.0;
    let w2_im = 2.0 * w_re * w_im;
    // sqrt(w² + 1) principal branch
    let r = sqrt(w2_re * w2_re + w2_im * w2_im);
    let s_re = sqrt(0.5 * (r + w2_re));
    let s_im_mag = sqrt(0.5 * max(r - w2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, w2_im < 0.0);
    // arg = w + sqrt(w² + 1)
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    // log(arg)
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    // Flip then scale 2/π, then random ±
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec2<f32>(sign_flip * two_over_pi * log_im, sign_flip * two_over_pi * log_re);
}
"#,
    wgsl_3d: r#"
fn variation_acosech(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let p_mag2 = max(p.x * p.x + p.y * p.y, 1e-20);
    let w_re =  p.x / p_mag2;
    let w_im = -p.y / p_mag2;
    let w2_re = w_re * w_re - w_im * w_im + 1.0;
    let w2_im = 2.0 * w_re * w_im;
    let r = sqrt(w2_re * w2_re + w2_im * w2_im);
    let s_re = sqrt(0.5 * (r + w2_re));
    let s_im_mag = sqrt(0.5 * max(r - w2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, w2_im < 0.0);
    let arg_re = w_re + s_re;
    let arg_im = w_im + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let sign_flip = select(1.0, -1.0, rng_nextf(rng) < 0.5);
    return vec3<f32>(sign_flip * two_over_pi * log_im, sign_flip * two_over_pi * log_re, p.z);
}
"#,
};

// =============================================================================
// arcsech: (2/π) * ArcCosh(1/z) = (2/π) * ln(1/z + sqrt(1/z + 1) * sqrt(1/z - 1))
//
// Note: upstream omits Z preservation, so the 3D version writes p.z through
// via the standard outer affine — i.e. variation contributes nothing to Z.
// =============================================================================
/// Inverse hyperbolic secant on the complex input (arccosh of 1/z).
/// Singular at the origin and outflows along the real axis.
pub static ARCSECH: VariationDef = VariationDef {
    name: "arcsech",
    aliases: &[],
    display_name: "ArcSech",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_arcsech(p: vec2<f32>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // w = 1/p
    let p_mag2 = max(dot(p, p), 1e-20);
    let w_re =  p.x / p_mag2;
    let w_im = -p.y / p_mag2;
    // a = w + 1, b = w - 1
    let a_re = w_re + 1.0; let a_im = w_im;
    let b_re = w_re - 1.0; let b_im = w_im;
    // sqrt(a)
    let a_r = sqrt(a_re * a_re + a_im * a_im);
    let sa_re = sqrt(0.5 * (a_r + a_re));
    let sa_im_mag = sqrt(0.5 * max(a_r - a_re, 0.0));
    let sa_im = select(sa_im_mag, -sa_im_mag, a_im < 0.0);
    // sqrt(b)
    let b_r = sqrt(b_re * b_re + b_im * b_im);
    let sb_re = sqrt(0.5 * (b_r + b_re));
    let sb_im_mag = sqrt(0.5 * max(b_r - b_re, 0.0));
    let sb_im = select(sb_im_mag, -sb_im_mag, b_im < 0.0);
    // sqrt(a) * sqrt(b)
    let prod_re = sa_re * sb_re - sa_im * sb_im;
    let prod_im = sa_re * sb_im + sa_im * sb_re;
    // arg = w + prod
    let arg_re = w_re + prod_re;
    let arg_im = w_im + prod_im;
    // log(arg)
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    return vec2<f32>(two_over_pi * log_re, two_over_pi * log_im);
}
"#,
    wgsl_3d: r#"
fn variation_arcsech(p: vec3<f32>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let p_mag2 = max(p.x * p.x + p.y * p.y, 1e-20);
    let w_re =  p.x / p_mag2;
    let w_im = -p.y / p_mag2;
    let a_re = w_re + 1.0; let a_im = w_im;
    let b_re = w_re - 1.0; let b_im = w_im;
    let a_r = sqrt(a_re * a_re + a_im * a_im);
    let sa_re = sqrt(0.5 * (a_r + a_re));
    let sa_im_mag = sqrt(0.5 * max(a_r - a_re, 0.0));
    let sa_im = select(sa_im_mag, -sa_im_mag, a_im < 0.0);
    let b_r = sqrt(b_re * b_re + b_im * b_im);
    let sb_re = sqrt(0.5 * (b_r + b_re));
    let sb_im_mag = sqrt(0.5 * max(b_r - b_re, 0.0));
    let sb_im = select(sb_im_mag, -sb_im_mag, b_im < 0.0);
    let prod_re = sa_re * sb_re - sa_im * sb_im;
    let prod_im = sa_re * sb_im + sa_im * sb_re;
    let arg_re = w_re + prod_re;
    let arg_im = w_im + prod_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    return vec3<f32>(two_over_pi * log_re, two_over_pi * log_im, 0.0);
}
"#,
};

// =============================================================================
// arcsech2: 2 * (2/π) * ArcCosh(1/z), with translation by ±i depending on sign(im)
// =============================================================================
/// Variant of ArcSecH with translation by ±i depending on the imaginary
/// sign — produces two parallel arcs instead of one.
pub static ARCSECH2: VariationDef = VariationDef {
    name: "arcsech2",
    aliases: &[],
    display_name: "ArcSech2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_arcsech2(p: vec2<f32>) -> vec2<f32> {
    let scale = 2.0 * 0.6366197723675814;
    let p_mag2 = max(dot(p, p), 1e-20);
    let w_re =  p.x / p_mag2;
    let w_im = -p.y / p_mag2;
    let a_re = w_re + 1.0; let a_im = w_im;
    let b_re = w_re - 1.0; let b_im = w_im;
    let a_r = sqrt(a_re * a_re + a_im * a_im);
    let sa_re = sqrt(0.5 * (a_r + a_re));
    let sa_im_mag = sqrt(0.5 * max(a_r - a_re, 0.0));
    let sa_im = select(sa_im_mag, -sa_im_mag, a_im < 0.0);
    let b_r = sqrt(b_re * b_re + b_im * b_im);
    let sb_re = sqrt(0.5 * (b_r + b_re));
    let sb_im_mag = sqrt(0.5 * max(b_r - b_re, 0.0));
    let sb_im = select(sb_im_mag, -sb_im_mag, b_im < 0.0);
    let prod_re = sa_re * sb_re - sa_im * sb_im;
    let prod_im = sa_re * sb_im + sa_im * sb_re;
    let arg_re = w_re + prod_re;
    let arg_im = w_im + prod_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let res_re = scale * log_re;
    let res_im = scale * log_im;
    // Translate by ±i and flip sign of real depending on imaginary sign
    return select(
        vec2<f32>(-res_re, res_im - 1.0),
        vec2<f32>( res_re, res_im + 1.0),
        res_im < 0.0,
    );
}
"#,
    wgsl_3d: r#"
fn variation_arcsech2(p: vec3<f32>) -> vec3<f32> {
    let scale = 2.0 * 0.6366197723675814;
    let p_mag2 = max(p.x * p.x + p.y * p.y, 1e-20);
    let w_re =  p.x / p_mag2;
    let w_im = -p.y / p_mag2;
    let a_re = w_re + 1.0; let a_im = w_im;
    let b_re = w_re - 1.0; let b_im = w_im;
    let a_r = sqrt(a_re * a_re + a_im * a_im);
    let sa_re = sqrt(0.5 * (a_r + a_re));
    let sa_im_mag = sqrt(0.5 * max(a_r - a_re, 0.0));
    let sa_im = select(sa_im_mag, -sa_im_mag, a_im < 0.0);
    let b_r = sqrt(b_re * b_re + b_im * b_im);
    let sb_re = sqrt(0.5 * (b_r + b_re));
    let sb_im_mag = sqrt(0.5 * max(b_r - b_re, 0.0));
    let sb_im = select(sb_im_mag, -sb_im_mag, b_im < 0.0);
    let prod_re = sa_re * sb_re - sa_im * sb_im;
    let prod_im = sa_re * sb_im + sa_im * sb_re;
    let arg_re = w_re + prod_re;
    let arg_im = w_im + prod_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    let res_re = scale * log_re;
    let res_im = scale * log_im;
    let xy = select(
        vec2<f32>(-res_re, res_im - 1.0),
        vec2<f32>( res_re, res_im + 1.0),
        res_im < 0.0,
    );
    return vec3<f32>(xy.x, xy.y, 0.0);
}
"#,
};

// =============================================================================
// arcsinh: (2/π) * ArcSinh(z) = (2/π) * ln(z + sqrt(z² + 1))
// =============================================================================
/// Inverse hyperbolic sine on the complex input. Maps the entire plane onto
/// a horizontal strip — acts as a `spreading` transform.
pub static ARCSINH: VariationDef = VariationDef {
    name: "arcsinh",
    aliases: &[],
    display_name: "ArcSinh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_arcsinh(p: vec2<f32>) -> vec2<f32> {
    let two_over_pi = 0.6366197723675814;
    // z² + 1
    let z2_re = p.x * p.x - p.y * p.y + 1.0;
    let z2_im = 2.0 * p.x * p.y;
    // sqrt(z² + 1)
    let r = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r + z2_re));
    let s_im_mag = sqrt(0.5 * max(r - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    // arg = z + sqrt(z² + 1)
    let arg_re = p.x + s_re;
    let arg_im = p.y + s_im;
    // log(arg)
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    return vec2<f32>(two_over_pi * log_re, two_over_pi * log_im);
}
"#,
    wgsl_3d: r#"
fn variation_arcsinh(p: vec3<f32>) -> vec3<f32> {
    let two_over_pi = 0.6366197723675814;
    let z2_re = p.x * p.x - p.y * p.y + 1.0;
    let z2_im = 2.0 * p.x * p.y;
    let r = sqrt(z2_re * z2_re + z2_im * z2_im);
    let s_re = sqrt(0.5 * (r + z2_re));
    let s_im_mag = sqrt(0.5 * max(r - z2_re, 0.0));
    let s_im = select(s_im_mag, -s_im_mag, z2_im < 0.0);
    let arg_re = p.x + s_re;
    let arg_im = p.y + s_im;
    let log_re = 0.5 * log(arg_re * arg_re + arg_im * arg_im + 1e-40);
    let log_im = atan2(arg_im, arg_re);
    return vec3<f32>(two_over_pi * log_re, two_over_pi * log_im, 0.0);
}
"#,
};

// =============================================================================
// arctanh: (2/π) * ArcTanh(z) = (2/π) * (1/2) * ln((1 + z)/(1 - z))
// =============================================================================
/// Inverse hyperbolic tangent on the complex input. Maps the unit disc onto
/// the entire plane; everything inside |z|=1 expands outward, everything
/// outside compresses inward.
pub static ARCTANH: VariationDef = VariationDef {
    name: "arctanh",
    aliases: &[],
    display_name: "ArcTanh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_arctanh(p: vec2<f32>) -> vec2<f32> {
    let one_over_pi = 0.3183098861837907;
    // (1 + p) / (1 - p) — complex
    let denom_mag2 = max((1.0 - p.x) * (1.0 - p.x) + p.y * p.y, 1e-20);
    let ratio_re = ((1.0 + p.x) * (1.0 - p.x) - p.y * p.y) / denom_mag2;
    let ratio_im = (p.y * (1.0 - p.x) + p.y * (1.0 + p.x)) / denom_mag2;
    // (1/2) * ln(ratio): scaled below by extra (2/π)/2 = 1/π
    let log_re = 0.5 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = atan2(ratio_im, ratio_re);
    return vec2<f32>(one_over_pi * 0.5 * log_re, one_over_pi * 0.5 * log_im);
}
"#,
    wgsl_3d: r#"
fn variation_arctanh(p: vec3<f32>) -> vec3<f32> {
    let one_over_pi = 0.3183098861837907;
    let denom_mag2 = max((1.0 - p.x) * (1.0 - p.x) + p.y * p.y, 1e-20);
    let ratio_re = ((1.0 + p.x) * (1.0 - p.x) - p.y * p.y) / denom_mag2;
    let ratio_im = (p.y * (1.0 - p.x) + p.y * (1.0 + p.x)) / denom_mag2;
    let log_re = 0.5 * log(ratio_re * ratio_re + ratio_im * ratio_im + 1e-40);
    let log_im = atan2(ratio_im, ratio_re);
    return vec3<f32>(one_over_pi * 0.5 * log_re, one_over_pi * 0.5 * log_im, 0.0);
}
"#,
};
