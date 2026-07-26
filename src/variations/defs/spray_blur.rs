//! `spray_blur` — K-sample stochastic blur by multi-emit (original).
//!
//! Phase 3 of [docs/projects/multi-emit-stereograms.md]. A conventional
//! stochastic blur deposits ONE jittered sample per iteration, so the
//! blur's own noise converges no faster than the flame. This one emits
//! `count` jittered copies per iteration at weight `brightness/count` —
//! the blur region fills in count× faster at identical total density,
//! and because the copies share their source trajectory point, the
//! kernel is sampled coherently around real structure instead of
//! scattering it.
//!
//! Kernels: Gaussian (Irwin–Hall, the same distribution the classic
//! `gaussian_blur` draws), uniform Disc, Ring (exact circle — bokeh
//! rings), and Streak (uniform line segment — motion blur). `aspect`
//! squashes the kernel into an ellipse and `angle` orients it, so
//! directional bokeh and diagonal motion smears are one slider away.
//!
//! The un-blurred center plot is suppressed (`CanHide`), so Brightness
//! 1 is exactly mono-equivalent total density. Intended for a final
//! transform (emissions skip later Final chains by design); legal
//! anywhere. Deliberately NOT flagged `AnalyticBlur`: the analytic
//! mean-splat models a single-sample contract, and the flame-wide
//! analytic gate already excludes multi-emitters.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Multi-emit stochastic blur: count jittered copies per iteration
/// (Gaussian / Disc / Ring / Streak kernels, elliptical aspect,
/// orientable), mono-equivalent brightness, count× faster convergence
/// than a one-sample blur.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static SPRAY_BLUR: VariationDef = VariationDef {
    name: "spray_blur",
    aliases: &[],
    display_name: "Spray Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::CanHide, Feature::PlotEmits(16), Feature::NeedsRng],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("count", "Count", int, 8.0, 1.0, 16.0, "Jittered copies plotted per iteration. The blur fills in Count× faster than a one-sample stochastic blur at the same total density (each copy carries weight Brightness/Count)."),
        param!("radius", "Radius", float, 0.15, 0.0, 2.0, "Kernel size in world units: Gaussian scale, Disc/Ring radius, Streak half-length."),
        param!("shape", "Shape", enum, 0, &["Gaussian", "Disc", "Ring", "Streak"], "Jitter kernel. Gaussian: soft falloff (the classic gaussian_blur distribution). Disc: uniform fill — flat bokeh. Ring: exact circle — bokeh rings / neon doubling. Streak: uniform line segment along Angle — motion blur."),
        param!("angle", "Angle", angle, 0.0, "Kernel orientation: the Streak direction, and the major-axis direction of an elliptical (Aspect ≠ 1) Gaussian/Disc/Ring."),
        param!("aspect", "Aspect", float, 1.0, 0.05, 4.0, "Kernel y-scale before orientation: below 1 squashes the kernel into an ellipse along Angle — directional bokeh without a full streak."),
        param!("brightness", "Brightness", float, 1.0, 0.1, 4.0, "Total density multiplier. 1 = mono-equivalent (each copy at 1/Count weight). Per-copy weights below ~0.01 floor to zero in the histogram."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn spray_blur_offset(shape: u32, radius: f32, rng: ptr<function, RngState>) -> vec2<f32> {
    if (shape == 0u) {
        // Irwin–Hall gaussian approximation — same as gaussian_blur.
        let theta = rng_nextf(rng) * 6.28318530718;
        let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
        return radius * r * vec2<f32>(cos(theta), sin(theta));
    }
    if (shape == 1u) {
        let theta = rng_nextf(rng) * 6.28318530718;
        let r = radius * sqrt(rng_nextf(rng));
        return r * vec2<f32>(cos(theta), sin(theta));
    }
    if (shape == 2u) {
        let theta = rng_nextf(rng) * 6.28318530718;
        return radius * vec2<f32>(cos(theta), sin(theta));
    }
    // Streak: uniform along the local x axis; Angle orients it below.
    return vec2<f32>(radius * (rng_nextf(rng) * 2.0 - 1.0), 0.0);
}

fn variation_spray_blur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    let count = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 16u);
    let radius = get_param(xform_id, variation_id, 1u);
    let shape = u32(get_param(xform_id, variation_id, 2u));
    let angle = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let aspect = get_param(xform_id, variation_id, 4u);
    let brightness = get_param(xform_id, variation_id, 5u);

    *hide = true;
    let w = brightness / f32(count);
    let ca = cos(angle);
    let sa = sin(angle);
    for (var k = 0u; k < count; k = k + 1u) {
        var o = spray_blur_offset(shape, radius, rng);
        o = vec2<f32>(o.x, o.y * aspect);
        o = vec2<f32>(ca * o.x - sa * o.y, sa * o.x + ca * o.y);
        emit_plot_weighted(p + o, w);
    }
    return p;
}
"#;

const WGSL_3D: &str = r#"
fn spray_blur_offset(shape: u32, radius: f32, rng: ptr<function, RngState>) -> vec2<f32> {
    if (shape == 0u) {
        let theta = rng_nextf(rng) * 6.28318530718;
        let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
        return radius * r * vec2<f32>(cos(theta), sin(theta));
    }
    if (shape == 1u) {
        let theta = rng_nextf(rng) * 6.28318530718;
        let r = radius * sqrt(rng_nextf(rng));
        return r * vec2<f32>(cos(theta), sin(theta));
    }
    if (shape == 2u) {
        let theta = rng_nextf(rng) * 6.28318530718;
        return radius * vec2<f32>(cos(theta), sin(theta));
    }
    return vec2<f32>(radius * (rng_nextf(rng) * 2.0 - 1.0), 0.0);
}

fn variation_spray_blur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec3<f32> {
    let count = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 16u);
    let radius = get_param(xform_id, variation_id, 1u);
    let shape = u32(get_param(xform_id, variation_id, 2u));
    let angle = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let aspect = get_param(xform_id, variation_id, 4u);
    let brightness = get_param(xform_id, variation_id, 5u);

    // Jitter in the xy plane; z rides along (a screen-plane blur —
    // depth effects then act on each copy's true z as usual).
    *hide = true;
    let w = brightness / f32(count);
    let ca = cos(angle);
    let sa = sin(angle);
    for (var k = 0u; k < count; k = k + 1u) {
        var o = spray_blur_offset(shape, radius, rng);
        o = vec2<f32>(o.x, o.y * aspect);
        o = vec2<f32>(ca * o.x - sa * o.y, sa * o.x + ca * o.y);
        emit_plot_weighted(vec3<f32>(p.xy + o, p.z), w);
    }
    return p;
}
"#;
