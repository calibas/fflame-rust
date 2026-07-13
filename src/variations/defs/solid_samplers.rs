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

/// `bubble` with a solid shell: the same inverse-stereographic mapping
/// of the plane onto the unit sphere, plus (a) real radial THICKNESS
/// so the surface has volume instead of being one sample thin, and
/// (b) a `fill` probability of ignoring the incoming direction and
/// sampling the sphere uniformly — guaranteed baseline density even
/// where the incoming fractal has holes. `thickness 0, fill 0` is
/// classic bubble.
pub static BUBBLE_SOLID: VariationDef = VariationDef {
    name: "bubble_solid",
    aliases: &[],
    display_name: "Bubble (Solid)",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::VolumeFill],
    parameters: &[
        param!("thickness", "Thickness", float, 0.02, 0.0, 1.0, "Radial shell depth as a fraction of the sphere radius. Gives the surface real volume - the depth buffer and density volume see a closed shell instead of a one-sample-thin film."),
        param!("fill", "Fill", float, 0.1, 0.0, 1.0, "Probability that a sample seals the sphere in the density volume instead of plotting: uniform shell geometry for occlusion/lighting/shadows, with ZERO effect on image colors. Only meaningful with the Density Volume enabled - such samples are dropped otherwise."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bubble_solid(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let thickness = clamp(get_param(xform_id, variation_id, 0u), 0.0, 1.0);
    let fill = clamp(get_param(xform_id, variation_id, 1u), 0.0, 1.0);
    var pt: vec2<f32>;
    if (rng_nextf(rng) < fill) {
        // Volume-only: seals geometry, never plots (dropped in 2D).
        volume_fill_flag = true;
        // Uniform point on the unit circle (the 2D bubble image's rim).
        let theta = rng_nextf(rng) * 6.28318530718;
        pt = vec2<f32>(cos(theta), sin(theta));
    } else {
        // Classic 2D bubble.
        let r = dot(p, p) / 4.0 + 1.0;
        pt = p / r;
    }
    let s = 1.0 + (rng_nextf(rng) - 0.5) * thickness;
    return pt * s;
}
"#,
    wgsl_3d: r#"
fn variation_bubble_solid(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let thickness = clamp(get_param(xform_id, variation_id, 0u), 0.0, 1.0);
    let fill = clamp(get_param(xform_id, variation_id, 1u), 0.0, 1.0);
    var pt: vec3<f32>;
    if (rng_nextf(rng) < fill) {
        // Volume-only: seals the shell's geometry in the density volume
        // (occlusion / lighting / shadows) without diluting image colors.
        volume_fill_flag = true;
        // Uniform point on the unit sphere.
        let z = rng_nextf(rng) * 2.0 - 1.0;
        let phi = rng_nextf(rng) * 6.28318530718;
        let rxy = sqrt(max(1.0 - z * z, 0.0));
        pt = vec3<f32>(rxy * cos(phi), rxy * sin(phi), z);
    } else {
        // Classic bubble: inverse stereographic projection of the XY
        // plane onto the unit sphere (matches `bubble`'s 3D body).
        let r = dot(p.xy, p.xy) / 4.0 + 1.0;
        pt = vec3<f32>(p.x / r, p.y / r, 2.0 / r - 1.0);
    }
    // Radial thickness jitter: the shell gets real volume.
    let s = 1.0 + (rng_nextf(rng) - 0.5) * thickness;
    return pt * s;
}
"#,
};
