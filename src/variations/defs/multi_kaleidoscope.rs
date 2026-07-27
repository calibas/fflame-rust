//! `multi_kaleidoscope` — N-fold kaleidoscope by multi-emit: every
//! iteration plots all N sector copies at once (original).
//!
//! Phase 3 of [docs/projects/multi-emit-stereograms.md]. Differs from
//! post-symmetry in kind, not just degree: it is an ordinary variation,
//! so its parameters ride the animation system's variation-param tracks
//! (a breathing `spiral`, a slowly winding `twist`), it attaches
//! per-transform via `final_attachments`, and it goes beyond rigid
//! symmetry — per-copy log-spiral scaling and a twist increment that
//! shears the fan out of perfect symmetry, neither of which a symmetry
//! group can express.
//!
//! All `order` copies are emitted (the center plot is suppressed via
//! `emit_suppress_main`), each at weight `brightness/order`, so the default is
//! exactly mono-equivalent total density — an order-12 kaleidoscope is
//! no brighter than the unadorned flame, and converges 12× faster than
//! rendering one sector per iteration.
//!
//! Intended for a final transform (emissions skip later Final chains by
//! design — see the PlotEmits contract); legal anywhere.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Multi-emit kaleidoscope: all N sector copies plotted per iteration —
/// dihedral mirrors, log-spiral copy scaling, twist shear, movable
/// center; mono-equivalent brightness at any order.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static MULTI_KALEIDOSCOPE: VariationDef = VariationDef {
    name: "multi_kaleidoscope",
    aliases: &[],
    display_name: "Multi Kaleidoscope",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::PlotEmits(16)],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("order", "Order", int, 6.0, 2.0, 16.0, "Number of sector copies plotted per iteration (all emitted at once; the un-copied center plot is suppressed). Total brightness stays mono-equivalent at any order — each copy carries weight Brightness/Order."),
        param!("angle", "Angle", angle, 0.0, "Phase of the whole fan — rotates every sector together about the center."),
        param!("twist", "Twist", angle, 0.0, "Extra rotation added PER COPY on top of the even 360/Order spacing. Nonzero shears the fan out of exact symmetry into a swirl — the thing a rigid symmetry group cannot do. Animates beautifully."),
        param!("spiral", "Spiral", float, 1.0, 0.5, 1.5, "Per-copy scale factor about the center: copy k is scaled by Spiral^k. 1 = flat kaleidoscope; below 1 the sectors telescope inward into a logarithmic spiral, above 1 outward."),
        param!("mirror", "Mirror", bool, false, "Dihedral mode: odd copies are reflected before rotation, giving the alternating mirror-image sectors of a physical two-mirror kaleidoscope (D_n symmetry instead of C_n)."),
        param!("cx", "Center X", unlimited_float, 0.0, -2.0, 2.0, "Kaleidoscope center x — copies rotate and scale about this point."),
        param!("cy", "Center Y", unlimited_float, 0.0, -2.0, 2.0, "Kaleidoscope center y."),
        param!("brightness", "Brightness", float, 1.0, 0.1, 4.0, "Total density multiplier. 1 = mono-equivalent (each copy at 1/Order weight). Per-copy weights below ~0.01 floor to zero in the histogram, so very high Order plus very low Brightness can vanish."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_multi_kaleidoscope(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let order = clamp(u32(get_param(xform_id, variation_id, 0u)), 2u, 16u);
    let angle = get_param(xform_id, variation_id, 1u) * 0.01745329252;
    let twist = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let spiral = get_param(xform_id, variation_id, 3u);
    let mirror = get_param(xform_id, variation_id, 4u) > 0.5;
    let c = vec2<f32>(
        get_param(xform_id, variation_id, 5u),
        get_param(xform_id, variation_id, 6u));
    let brightness = get_param(xform_id, variation_id, 7u);

    // All copies are emissions; the center plot is suppressed so the
    // brightness accounting is exact (Brightness/Order each).
    emit_suppress_main();
    let v0 = p - c;
    let w = brightness / f32(order);
    let step = 6.28318530718 / f32(order);
    var scale = 1.0;
    for (var k = 0u; k < order; k = k + 1u) {
        var v = v0;
        if (mirror && (k & 1u) == 1u) {
            v = vec2<f32>(v.x, -v.y);
        }
        let a = angle + f32(k) * (step + twist);
        let ca = cos(a);
        let sa = sin(a);
        v = vec2<f32>(ca * v.x - sa * v.y, sa * v.x + ca * v.y) * scale;
        emit_plot_weighted(c + v, w);
        scale = scale * spiral;
    }
    return p;
}
"#;

const WGSL_3D: &str = r#"
fn variation_multi_kaleidoscope(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let order = clamp(u32(get_param(xform_id, variation_id, 0u)), 2u, 16u);
    let angle = get_param(xform_id, variation_id, 1u) * 0.01745329252;
    let twist = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let spiral = get_param(xform_id, variation_id, 3u);
    let mirror = get_param(xform_id, variation_id, 4u) > 0.5;
    let c = vec2<f32>(
        get_param(xform_id, variation_id, 5u),
        get_param(xform_id, variation_id, 6u));
    let brightness = get_param(xform_id, variation_id, 7u);

    // The fan lives in the xy plane; z rides along unchanged per copy
    // (a z-spiral would fight the camera pipeline's depth semantics).
    emit_suppress_main();
    let v0 = p.xy - c;
    let w = brightness / f32(order);
    let step = 6.28318530718 / f32(order);
    var scale = 1.0;
    for (var k = 0u; k < order; k = k + 1u) {
        var v = v0;
        if (mirror && (k & 1u) == 1u) {
            v = vec2<f32>(v.x, -v.y);
        }
        let a = angle + f32(k) * (step + twist);
        let ca = cos(a);
        let sa = sin(a);
        v = vec2<f32>(ca * v.x - sa * v.y, sa * v.x + ca * v.y) * scale;
        emit_plot_weighted(vec3<f32>(c + v, p.z), w);
        scale = scale * spiral;
    }
    return p;
}
"#;
