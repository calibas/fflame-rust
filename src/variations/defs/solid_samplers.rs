//! Surface-sampler variations for solid rendering.
//!
//! Motivation (docs/projects/solid-rendering.md, field feedback): shell
//! variations like `bubble` map the incoming measure onto an
//! infinitely thin surface — wherever the incoming fractal is sparse,
//! the shell has HOLES you can see straight through, and no amount of
//! per-pixel or volume-based repair can recover data that was never
//! plotted. These variations attack the root: they emit points with
//! guaranteed baseline density and real radial THICKNESS, so the
//! surface exists everywhere — in the histogram, in the depth buffer,
//! and in the Phase 2 density volume (normals/AO/shadows/repair all get
//! a closed shell to work with).
//!
//! Original to this project (no JWildfire/Apophysis counterpart).
//!
//! # Authors
//! - fflame-rust (original)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Uniform spherical-shell sampler. IGNORES the incoming point (like
/// the classic blur family) and emits a volumetrically uniform random
/// point inside the shell `radius·[1−thickness, 1]` around a
/// configurable center. Mix at a low weight into a transform (or give
/// it its own low-weight transform) to guarantee baseline density over
/// an entire sphere — the textured variations still dominate wherever
/// they have density; this fills what they leave empty.
pub static SOLID_SPHERE: VariationDef = VariationDef {
    name: "solid_sphere",
    aliases: &[],
    display_name: "Solid Sphere",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, 0.0, 10.0, "Outer radius of the shell in world units."),
        param!("thickness", "Thickness", float, 0.1, 0.0, 1.0, "Shell depth as a fraction of the radius: samples land volumetrically uniform between radius·(1−thickness) and radius. 0 = infinitely thin surface, 1 = solid ball."),
        param!("cx", "Center X", unlimited_float, 0.0, -5.0, 5.0, "World-space X of the sphere center."),
        param!("cy", "Center Y", unlimited_float, 0.0, -5.0, 5.0, "World-space Y of the sphere center."),
        param!("cz", "Center Z", unlimited_float, 0.0, -5.0, 5.0, "World-space Z of the sphere center."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    // 2D analog: an area-uniform annulus (same parameters, Z dropped).
    wgsl_2d: r#"
fn variation_solid_sphere(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let thickness = clamp(get_param(xform_id, variation_id, 1u), 0.0, 1.0);
    let cx = get_param(xform_id, variation_id, 2u);
    let cy = get_param(xform_id, variation_id, 3u);
    let theta = rng_nextf(rng) * 6.28318530718;
    // Area-uniform radial position within the annulus.
    let ri2 = (1.0 - thickness) * (1.0 - thickness);
    let r = radius * sqrt(mix(ri2, 1.0, rng_nextf(rng)));
    return vec2<f32>(cx + r * cos(theta), cy + r * sin(theta));
}
"#,
    wgsl_3d: r#"
fn variation_solid_sphere(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let thickness = clamp(get_param(xform_id, variation_id, 1u), 0.0, 1.0);
    let cx = get_param(xform_id, variation_id, 2u);
    let cy = get_param(xform_id, variation_id, 3u);
    let cz = get_param(xform_id, variation_id, 4u);
    // Uniform direction on the sphere.
    let z = rng_nextf(rng) * 2.0 - 1.0;
    let phi = rng_nextf(rng) * 6.28318530718;
    let rxy = sqrt(max(1.0 - z * z, 0.0));
    let dir = vec3<f32>(rxy * cos(phi), rxy * sin(phi), z);
    // Volumetrically uniform radial position within the shell:
    // r ~ cbrt(mix(ri³, 1, u)) keeps density independent of depth
    // inside thick shells (thickness 1 = a uniformly filled ball).
    let ri = 1.0 - thickness;
    let ri3 = ri * ri * ri;
    let r = radius * pow(mix(ri3, 1.0, rng_nextf(rng)), 0.33333333);
    return vec3<f32>(cx, cy, cz) + dir * r;
}
"#,
};

/// Pure side-effect volume emitter: adds a spherical shell/ball of
/// density to the solid-rendering DENSITY VOLUME each iteration —
/// occlusion, lighting and shadows see a sealed solid — while the
/// chaos game, the plotted position, and the image colors are
/// completely untouched (the variation contributes zero to the
/// variation sum; its weight merely activates it). Pair it with plain
/// `bubble` on the same transform: bubble paints the surface texture,
/// this seals the interior. Defaults (radius 1, center 0, thickness 1)
/// match bubble's unit sphere exactly. The transform's post-affine is
/// applied to the emitted point; final transforms and post-symmetry
/// are not. Without the Density Volume this variation does nothing.
pub static SPHERE_VOLUME: VariationDef = VariationDef {
    name: "sphere_volume",
    aliases: &[],
    display_name: "Sphere Volume (Solid)",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::VolumeSideEmit],
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, 0.0, 10.0, "Outer radius of the emitted sphere (bubble's surface is the unit sphere: radius 1)."),
        param!("thickness", "Thickness", float, 1.0, 0.0, 1.0, "Shell depth as a fraction of the radius: 1 = solid ball (volume in the middle), small values = just a sealed skin."),
        param!("cx", "Center X", unlimited_float, 0.0, -5.0, 5.0, "Sphere center X (bubble is centered at the origin)."),
        param!("cy", "Center Y", unlimited_float, 0.0, -5.0, 5.0, "Sphere center Y."),
        param!("cz", "Center Z", unlimited_float, 0.0, -5.0, 5.0, "Sphere center Z."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    // 2D: no volume exists — pure no-op pass-through of nothing.
    wgsl_2d: r#"
fn variation_sphere_volume(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    return vec2<f32>(0.0);
}
"#,
    wgsl_3d: r#"
fn variation_sphere_volume(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let thickness = clamp(get_param(xform_id, variation_id, 1u), 0.0, 1.0);
    let cx = get_param(xform_id, variation_id, 2u);
    let cy = get_param(xform_id, variation_id, 3u);
    let cz = get_param(xform_id, variation_id, 4u);
    // Volumetrically uniform point in the shell radius·[1−thickness, 1].
    let z = rng_nextf(rng) * 2.0 - 1.0;
    let phi = rng_nextf(rng) * 6.28318530718;
    let rxy = sqrt(max(1.0 - z * z, 0.0));
    let dir = vec3<f32>(rxy * cos(phi), rxy * sin(phi), z);
    let ri = 1.0 - thickness;
    let ri3 = ri * ri * ri;
    let r = radius * pow(mix(ri3, 1.0, rng_nextf(rng)), 0.33333333);
    volume_side_point = vec3<f32>(cx, cy, cz) + dir * r;
    volume_side_flag = true;
    // Pure side effect: contribute NOTHING to the variation sum.
    return vec3<f32>(0.0);
}
"#,
};
