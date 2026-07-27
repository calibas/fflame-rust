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
//! The un-blurred center plot is suppressed (`emit_suppress_main`), so
//! Brightness 1 is exactly mono-equivalent total density.
//!
//! Works on ANY transform, not just finals: emissions are OFFSETS
//! (`emit_plot_offset`), which the plot stage adds to the iteration's
//! final plotted position — after the transform's post-affine and the
//! whole Final chain — so the copies always center on what actually
//! plots. The variation runs in the POST phase and returns its input —
//! an exact identity for the trajectory, so it mixes with any other
//! variations on the transform with zero distortion and the walk
//! continues un-blurred (a non-destructive display blur). Weight is an
//! on/off switch (post dispatch carries no weight). One consequence: a
//! transform with ONLY spray on it has an empty normal-phase result —
//! pair it with `linear` 1 to pass the point through. Deliberately NOT
//! flagged
//! `AnalyticBlur`: the analytic mean-splat models a single-sample
//! contract, and the flame-wide analytic gate already excludes
//! multi-emitters.

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
    // POST phase, deliberately: post dispatch is `result = f(result)`, and
    // this body returns its input — an exact identity for the trajectory.
    // In the normal phase the identity return entered the weighted sum as
    // `weight × p`, a phantom `linear` that visibly distorted whatever
    // shared the transform (reported with von_dyck: the tiling bent toward
    // the previous trajectory point before blurring). Post placement also
    // means spray sees the transform's finished output, which is the right
    // thing to center a blur on. Weight is now purely an on/off switch.
    phase: VariationPhase::Post,
    features: &[Feature::PlotEmits(16), Feature::NeedsRng],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("count", "Count", int, 8.0, 1.0, 16.0, "Jittered copies plotted per iteration. The blur fills in Count× faster than a one-sample stochastic blur at the same total density (each copy carries weight Brightness/Count). Mixes distortion-free with other variations (post phase, identity pass-through); copies center on the final plotted position (post-affine and Final chains included) and the trajectory continues un-blurred. A transform with ONLY spray needs a companion linear 1 (the post phase acts on the normal-phase result)."),
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

fn variation_spray_blur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let count = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 16u);
    let radius = get_param(xform_id, variation_id, 1u);
    let shape = u32(get_param(xform_id, variation_id, 2u));
    let angle = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let aspect = get_param(xform_id, variation_id, 4u);
    let brightness = get_param(xform_id, variation_id, 5u);

    emit_suppress_main();
    let w = brightness / f32(count);
    let ca = cos(angle);
    let sa = sin(angle);
    for (var k = 0u; k < count; k = k + 1u) {
        var o = spray_blur_offset(shape, radius, rng);
        o = vec2<f32>(o.x, o.y * aspect);
        o = vec2<f32>(ca * o.x - sa * o.y, sa * o.x + ca * o.y);
        // OFFSET emission: the plot stage adds this to the iteration's
        // final plotted position, so the blur centers on the true
        // end-of-chain point — post-affine and Final chain included —
        // from ANY transform, normal or final.
        emit_plot_offset(o, w);
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

fn variation_spray_blur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let count = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 16u);
    let radius = get_param(xform_id, variation_id, 1u);
    let shape = u32(get_param(xform_id, variation_id, 2u));
    let angle = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let aspect = get_param(xform_id, variation_id, 4u);
    let brightness = get_param(xform_id, variation_id, 5u);

    // Jitter in the xy plane; z rides along (a screen-plane blur —
    // depth effects then act on each copy's true z as usual).
    emit_suppress_main();
    let w = brightness / f32(count);
    let ca = cos(angle);
    let sa = sin(angle);
    for (var k = 0u; k < count; k = k + 1u) {
        var o = spray_blur_offset(shape, radius, rng);
        o = vec2<f32>(o.x, o.y * aspect);
        o = vec2<f32>(ca * o.x - sa * o.y, sa * o.x + ca * o.y);
        // OFFSET emission (see the 2D body); z-offset 0 keeps the blur
        // in the screen plane at the point's own depth.
        emit_plot_offset(vec3<f32>(o, 0.0), w);
    }
    return p;
}
"#;
