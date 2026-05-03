//! gamma (zephyrtronium / dark-beam)
//!
//! `output.x = lgamma(hypot(x, y)) · w`
//! `output.y = atan2(y, x) · w`
//!
//! 0 user params, no init slots. Body factors cleanly through outer
//! multiplier.
//!
//! Implements `lgamma(x)` (log of gamma function) via Lanczos
//! approximation (g = 7, 8 coefficients). Standard formula:
//!   if x < 0.5: lgamma(x) = ln(π) - ln(|sin(π·x)|) - lgamma(1-x)
//!   else: x -= 1; t = x + g + 0.5
//!         A = c[0] + Σᵢ c[i]/(x+i) for i = 1..8
//!         lgamma(x) = 0.5·ln(2π) + (x+0.5)·ln(t) - t + ln(A)
//!
//! Since the input is `hypot(x, y) ≥ 0`, the reflection branch
//! (negative argument) is unreachable in practice, but kept for
//! safety. cpp's `FastMath.hypot` becomes `length(vec2(x, y))`.
//!
//! Source: `output/jwildfire-vars/output/gamma.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

const LGAMMA_BODY: &str = r#"
fn gm_lgamma(x: f32) -> f32 {
    let log_2pi_half = 0.9189385332046727;  // 0.5*ln(2π)
    let pi = 3.14159265358979;
    if (x < 0.5) {
        // Reflection formula
        let s = sin(pi * x);
        let safe_s = select(abs(s), 1e-30, abs(s) < 1e-30);
        return log(pi) - log(safe_s) - gm_lgamma(1.0 - x);
    }
    let xx = x - 1.0;
    let t = xx + 7.5;
    var a: f32 = 0.99999999999980993;
    a = a + 676.5203681218851 / (xx + 1.0);
    a = a - 1259.1392167224028 / (xx + 2.0);
    a = a + 771.3234287776531 / (xx + 3.0);
    a = a - 176.6150291621406 / (xx + 4.0);
    a = a + 12.507343278686905 / (xx + 5.0);
    a = a - 0.13857109526572012 / (xx + 6.0);
    a = a + 9.9843695780195716e-6 / (xx + 7.0);
    a = a + 1.5056327351493116e-7 / (xx + 8.0);
    return log_2pi_half + (xx + 0.5) * log(t) - t + log(max(abs(a), 1e-30));
}
"#;

pub static GAMMA: VariationDef = VariationDef {
    name: "gamma",
    display_name: "Gamma",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn gm_lgamma_iter(x: f32) -> f32 {
    let log_2pi_half = 0.9189385332046727;
    let xx = x - 1.0;
    let t = xx + 7.5;
    var a: f32 = 0.99999999999980993;
    a = a + 676.5203681218851 / (xx + 1.0);
    a = a - 1259.1392167224028 / (xx + 2.0);
    a = a + 771.3234287776531 / (xx + 3.0);
    a = a - 176.6150291621406 / (xx + 4.0);
    a = a + 12.507343278686905 / (xx + 5.0);
    a = a - 0.13857109526572012 / (xx + 6.0);
    a = a + 9.9843695780195716e-6 / (xx + 7.0);
    a = a + 1.5056327351493116e-7 / (xx + 8.0);
    return log_2pi_half + (xx + 0.5) * log(t) - t + log(max(abs(a), 1e-30));
}

fn gm_lgamma(x: f32) -> f32 {
    let pi = 3.14159265358979;
    if (x < 0.5) {
        let s = sin(pi * x);
        let safe_s = select(abs(s), 1e-30, abs(s) < 1e-30);
        return log(pi) - log(safe_s) - gm_lgamma_iter(1.0 - x);
    }
    return gm_lgamma_iter(x);
}

fn variation_gamma(p: vec2<f32>) -> vec2<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y);
    return vec2<f32>(gm_lgamma(r), atan2(p.y, p.x));
}
"#,
    wgsl_3d: Some(r#"
fn gm_lgamma_iter(x: f32) -> f32 {
    let log_2pi_half = 0.9189385332046727;
    let xx = x - 1.0;
    let t = xx + 7.5;
    var a: f32 = 0.99999999999980993;
    a = a + 676.5203681218851 / (xx + 1.0);
    a = a - 1259.1392167224028 / (xx + 2.0);
    a = a + 771.3234287776531 / (xx + 3.0);
    a = a - 176.6150291621406 / (xx + 4.0);
    a = a + 12.507343278686905 / (xx + 5.0);
    a = a - 0.13857109526572012 / (xx + 6.0);
    a = a + 9.9843695780195716e-6 / (xx + 7.0);
    a = a + 1.5056327351493116e-7 / (xx + 8.0);
    return log_2pi_half + (xx + 0.5) * log(t) - t + log(max(abs(a), 1e-30));
}

fn gm_lgamma(x: f32) -> f32 {
    let pi = 3.14159265358979;
    if (x < 0.5) {
        let s = sin(pi * x);
        let safe_s = select(abs(s), 1e-30, abs(s) < 1e-30);
        return log(pi) - log(safe_s) - gm_lgamma_iter(1.0 - x);
    }
    return gm_lgamma_iter(x);
}

fn variation_gamma(p: vec3<f32>) -> vec3<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y);
    return vec3<f32>(gm_lgamma(r), atan2(p.y, p.x), p.z);
}
"#),
};

#[allow(dead_code)]
const _UNUSED: &str = LGAMMA_BODY;
