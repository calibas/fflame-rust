//! Analytic blur variations (experimental) — opt-in copies of `blur` and
//! `gaussian_blur` tagged with `Feature::AnalyticBlur`, so a flame can
//! choose the analytic (mean-splat + convolution) path without changing the
//! behavior of the stochastic originals.
//!
//! The WGSL offset formulas are byte-identical to their parents; the host
//! kernel sampler in `src/variations/analytic_blur.rs` mirrors them. See
//! `docs/projects/analytic-blur-buffer.md`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// `strength` and `residual` are ROUTING knobs, not part of the offset
// formula, so the WGSL bodies ignore the `xform_id`/`variation_id` args that
// carrying parameters introduces. The host reads them from the transform's
// variation params and packs them into GpuTransform (strength scales the
// mean-splat density; residual routes the next N plots through the blur
// buffer). See docs/projects/analytic-blur-buffer.md.

/// Analytic counterpart of `blur` (uniform-radius disc). Same fuzz, but
/// eligible for the analytic blur buffer.
pub static ANALYTIC_BLUR: VariationDef = VariationDef {
    name: "analytic_blur",
    aliases: &[],
    display_name: "Analytic Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AnalyticBlur],
    parameters: &[
        param!("strength", "Strength", float, 1.0, 0.0, 8.0, "Density multiplier for the analytic mean-splat. >1 makes the smooth blur dominate (and mask) other transforms in overlapping regions; <1 dims it. Artistic — deviates from a faithful render."),
        param!("residual", "Residual", int, 0.0, 0.0, 16.0, "Keep routing the next N plots after this blur through the blur buffer, smoothing the propagated fuzz. Higher = softer trailing blur (and more smoothing of following transforms), but over-blurs detail."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_analytic_blur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec2<f32>(r * cos(theta), r * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_analytic_blur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng);
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}
"#,
};

/// Analytic counterpart of `gaussian_blur` (Irwin-Hall(4) bell radius).
pub static ANALYTIC_GAUSSIAN_BLUR: VariationDef = VariationDef {
    name: "analytic_gaussian_blur",
    aliases: &[],
    display_name: "Analytic Gaussian Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AnalyticBlur],
    parameters: &[
        param!("strength", "Strength", float, 1.0, 0.0, 8.0, "Density multiplier for the analytic mean-splat. >1 makes the smooth blur dominate (and mask) other transforms in overlapping regions; <1 dims it. Artistic — deviates from a faithful render."),
        param!("residual", "Residual", int, 0.0, 0.0, 16.0, "Keep routing the next N plots after this blur through the blur buffer, smoothing the propagated fuzz. Higher = softer trailing blur (and more smoothing of following transforms), but over-blurs detail."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_analytic_gaussian_blur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_analytic_gaussian_blur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}
"#,
};
