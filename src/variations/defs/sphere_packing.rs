//! `sphere_packing` — Apollonian sphere/circle packing via Soddy-sphere
//! inversions (original).
//!
//! The genuinely 3D Kleinian: where the Möbius-family variations act by
//! SL(2,ℂ) on the plane (their limit sets are inherently planar, and
//! the H3 mode only drapes them), this one acts by **sphere inversions
//! in ℝ³** — honest 3D conformal maps — and its limit set is the
//! **Apollonian sphere packing**, a true 2D-fractal surface in space.
//!
//! Construction (verified numerically to ~1e-15): the symmetric 3D
//! Descartes/Soddy configuration — outer unit sphere + four tetrahedral
//! inner spheres of curvature 1+√(3/2), all mutually tangent — has five
//! **dual spheres**, each orthogonal to four of the five and passing
//! through their six tangency points. Inversions in the five dual
//! spheres generate the packing group; the chaos game over them (an
//! involution is blocked from repeating — I² = id) converges onto the
//! packing. A slab slice reproduces the classic gasket cross-section.
//!
//! The 2D body is the same construction one dimension down: outer unit
//! circle + three 120°-spaced inner circles (curvature 1+2/√3), four
//! dual circles → the classic **Apollonian gasket** as a chaos game of
//! circle inversions (the full reflection group, complementing
//! `apollonian_gasket`'s orientation-preserving Möbius version).
//!
//! `mode` = Tangent Spheres uses the five packing spheres THEMSELVES as
//! the mirrors instead of the duals — a sibling reflection group with
//! its own residual fractal (denser, less circular holes).
//!
//! A sphere inversion is just `x ↦ c + r²(x−c)/|x−c|²` — no quaternions,
//! no shared Möbius lib needed.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Apollonian sphere packing (3D) / gasket (2D) via Soddy dual-sphere
/// inversions — the genuinely 3D Kleinian.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static SPHERE_PACKING: VariationDef = VariationDef {
    name: "sphere_packing",
    aliases: &[],
    display_name: "Sphere Packing",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::AlwaysZ],
    // Slot 0: previous mirror index (inversions are involutions — block
    // immediate repeats). Slot 1: color register.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("mode", "Mode", enum, 0, &["Apollonian (Dual)", "Tangent Spheres"], "Which mirror set generates the group. Apollonian: inversions in the five DUAL spheres of the symmetric Soddy configuration (each orthogonal to four packing spheres, through their tangency points) — the limit set is the exact Apollonian sphere packing (gasket in 2D). Tangent Spheres: the five mutually tangent packing spheres themselves as mirrors — a sibling reflection group with a denser residual fractal."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Radius of the outer sphere/circle in world units."),
        param!("avoid_repeat", "Avoid Repeat", bool, true, "Block choosing the same mirror twice in a row (an inversion is an involution, so a repeat cancels to the identity). Off = all mirrors equally likely every call."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Mirror", "Mirror Blend"], "Direct-color source (needs the transform's Direct Color > 0). Mirror: which inversion was applied this call. Mirror Blend: persistent register pulled toward each mirror's palette slot at Color Speed — colors the packing by reflection itinerary."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "Blend rate of the Mirror Blend register: low = deep itinerary history, high = recent mirrors only."),
        param!("steps", "Steps", int, 3.0, 1.0, 8.0, "Inversions applied per call (only the last point is plotted). Higher values let respawned points converge onto the packing before plotting — suppresses pre-convergence haze and sharpens the residual set — at proportional GPU cost."),
        param!("reseed", "Reseed", float, 0.25, 0.0, 0.5, "Probability per call of planting the point on a random packing circle/sphere surface before the walk. The packing elements are EXACT subsets of the limit set, so seeded points render the gasket/packing crisply from the first plot — the same seeding lesson as von_dyck. 0 = pure feed-through chaos game (hazier: the log tonemap amplifies pre-convergence transients)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Baked configurations (computed + verified in the CPU prototype):
// 3D Soddy: outer unit sphere, inner curvature k = 1+√(3/2), r = 0.4494897,
//   centers at 0.5505103·(±1,±1,±1)/√3 (tetrahedral even-sign pattern).
//   Duals: D0 = (0,0,0 | 0.3178372) orthogonal to the four inner;
//   D1..D4 = ∓√3·tet_dir | 2√2, orthogonal to outer + three inner.
// 2D Descartes: outer unit circle, inner curvature 1+2/√3, r = 0.4641016,
//   centers at 0.5358984·(cos,sin) of 90°/210°/330°.
//   Duals: D0 = (0,0 | 0.2679492); D1..D3 opposite each inner, r = √3.
const WGSL_2D: &str = r#"
fn sp_circle(mode: u32, k: u32) -> vec3<f32> {
    if (mode == 1u) {
        // Tangent circles: outer + three inner (packing mirrors).
        switch k {
            case 0u: { return vec3<f32>(0.0, 0.0, 1.0); }
            case 1u: { return vec3<f32>(0.0, 0.5358984, 0.4641016); }
            case 2u: { return vec3<f32>(-0.4641016, -0.2679492, 0.4641016); }
            default: { return vec3<f32>(0.4641016, -0.2679492, 0.4641016); }
        }
    }
    // Apollonian dual circles.
    switch k {
        case 0u: { return vec3<f32>(0.0, 0.0, 0.2679492); }
        case 1u: { return vec3<f32>(0.0, -2.0, 1.7320508); }
        case 2u: { return vec3<f32>(1.7320508, 1.0, 1.7320508); }
        default: { return vec3<f32>(-1.7320508, 1.0, 1.7320508); }
    }
}

fn variation_sphere_packing(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let size = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let avoid = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 3u));
    let dc_scale = get_param(xform_id, variation_id, 4u);
    let color_speed = get_param(xform_id, variation_id, 5u);

    let steps = i32(get_param(xform_id, variation_id, 6u));
    let reseed = get_param(xform_id, variation_id, 7u);
    var prev = u32(get_state(xform_id, variation_id, 0u));
    var x = p / size;
    if (rng_nextf(rng) < reseed) {
        // Plant on a random packing circle — an exact limit-set subset.
        let cs = sp_circle(1u, min(u32(rng_nextf(rng) * 4.0), 3u));
        let a = rng_nextf(rng) * 6.28318530718;
        x = cs.xy + cs.z * vec2<f32>(cos(a), sin(a));
    }
    var k = 0u;
    for (var i = 0; i < steps; i = i + 1) {
        k = min(u32(rng_nextf(rng) * 4.0), 3u);
        if (avoid && prev < 4u && k == prev) { k = (k + 1u) % 4u; }
        let cr = sp_circle(mode, k);
        let v = x - cr.xy;
        let n = max(dot(v, v), 1e-12);
        x = cr.xy + (cr.z * cr.z / n) * v;
        prev = k;
    }
    set_state(xform_id, variation_id, 0u, f32(prev));

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / 4.0 * dc_scale);
    } else if (dc_mode == 2u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, (f32(k) + 0.5) / 4.0, color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = fract(creg * dc_scale);
    }
    return x * size;
}
"#;

const WGSL_3D: &str = r#"
fn sp_sphere(mode: u32, k: u32) -> vec4<f32> {
    if (mode == 1u) {
        // Tangent spheres: outer + four tetrahedral inner mirrors.
        switch k {
            case 0u: { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
            case 1u: { return vec4<f32>(0.3178372, 0.3178372, 0.3178372, 0.4494897); }
            case 2u: { return vec4<f32>(0.3178372, -0.3178372, -0.3178372, 0.4494897); }
            case 3u: { return vec4<f32>(-0.3178372, 0.3178372, -0.3178372, 0.4494897); }
            default: { return vec4<f32>(-0.3178372, -0.3178372, 0.3178372, 0.4494897); }
        }
    }
    // Apollonian dual spheres (orthogonal to 4-subsets, through the
    // tangency points; verified numerically).
    switch k {
        case 0u: { return vec4<f32>(0.0, 0.0, 0.0, 0.3178372); }
        case 1u: { return vec4<f32>(-1.7320508, -1.7320508, -1.7320508, 2.8284271); }
        case 2u: { return vec4<f32>(-1.7320508, 1.7320508, 1.7320508, 2.8284271); }
        case 3u: { return vec4<f32>(1.7320508, -1.7320508, 1.7320508, 2.8284271); }
        default: { return vec4<f32>(1.7320508, 1.7320508, -1.7320508, 2.8284271); }
    }
}

fn variation_sphere_packing(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let size = max(get_param(xform_id, variation_id, 1u), 1e-6);
    let avoid = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 3u));
    let dc_scale = get_param(xform_id, variation_id, 4u);
    let color_speed = get_param(xform_id, variation_id, 5u);

    let steps = i32(get_param(xform_id, variation_id, 6u));
    let reseed = get_param(xform_id, variation_id, 7u);
    var prev = u32(get_state(xform_id, variation_id, 0u));
    var x = p / size;
    if (rng_nextf(rng) < reseed) {
        // Plant on a random packing sphere — an exact limit-set subset.
        let cs = sp_sphere(1u, min(u32(rng_nextf(rng) * 5.0), 4u));
        let za = rng_nextf(rng) * 2.0 - 1.0;
        let ph = rng_nextf(rng) * 6.28318530718;
        let sa = sqrt(max(1.0 - za * za, 0.0));
        x = cs.xyz + cs.w * vec3<f32>(sa * cos(ph), sa * sin(ph), za);
    }
    var k = 0u;
    for (var i = 0; i < steps; i = i + 1) {
        k = min(u32(rng_nextf(rng) * 5.0), 4u);
        if (avoid && prev < 5u && k == prev) { k = (k + 1u) % 5u; }
        let sp = sp_sphere(mode, k);
        let v = x - sp.xyz;
        let n = max(dot(v, v), 1e-12);
        x = sp.xyz + (sp.w * sp.w / n) * v;
        prev = k;
    }
    set_state(xform_id, variation_id, 0u, f32(prev));

    if (dc_mode == 1u) {
        *vc = fract((f32(k) + 0.5) / 5.0 * dc_scale);
    } else if (dc_mode == 2u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, (f32(k) + 0.5) / 5.0, color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = fract(creg * dc_scale);
    }
    return x * size;
}
"#;
