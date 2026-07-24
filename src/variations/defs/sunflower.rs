//! sunflower (Jesus Sosa, 2018) — phyllotaxis spiral of regular polygons.
//!
//! Places `nPoints` regular n-gons on a Vogel sunflower spiral (golden-angle
//! phyllotaxis), each scaled by its distance from the centre, and plots a
//! random point inside a randomly-chosen polygon. Inspired by an R-language
//! program (fronkonstin.com/2017/05/22/sunflowers-for-colourlovers).
//!
//! JWildfire extends `DrawFunc`: `init()` builds the `nPoints` polygons as
//! `Ngon` primitives, then `transform()` picks one at random and samples a
//! point on it via the **nBlur** polygon sampler (`DrawFunc.randXY` /
//! `plotPolygon`, by FractalDesire). Like `szubieta`, that reduces to
//! per-call math — the build loop is deterministic in the index `i`, so we
//! pick a random `i`, recompute that floret's centre / scale / colour
//! directly, then run the nBlur sampler with our RNG. No primitive list,
//! no per-instance state.
//!
//! The floret colour (`sc`, the distance-based scale factor) is written to
//! the colour register, so the spiral fades through the palette from centre
//! to rim. The declared `color` param is dead (JWF overwrites it with the
//! polygon colour) — kept for `.flame` round-trip. Follows the Apophysis
//! `sunflower.cpp`: the JWildfire-only "F. filling" (hole-ratio) param is
//! not exposed; polygons are solid (`fill = 0`).
//!
//! Sources:
//!   - `output/jwildfire-vars/output/sunflower.cpp`
//!   - `output/variation-jwf-source/SunFlowersFunc.java`
//!   - `output/variation-jwf-source/plot/DrawFunc.java` (randXY / plotPolygon)
//!
//! # Authors
//! - Jesus Sosa
//! - FractalDesire (nBlur polygon sampler)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Phyllotaxis spiral of regular polygons — scatters `nPoints` n-gons along a
/// golden-angle sunflower spiral, scaled by distance from the centre, and
/// plots a random point inside a random floret. The floret's distance-scale
/// drives the colour, fading the spiral through the palette.
///
/// # Authors
/// - Jesus Sosa
/// - FractalDesire
pub static SUNFLOWER: VariationDef = VariationDef {
    name: "sunflower",
    aliases: &[],
    display_name: "Sunflower",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    parameters: &[
        param!("nPoints", "Points", int, 500.0, 10.0, 1000.0, "Number of polygons (florets) on the spiral. More points fill the disc more densely."),
        param!("shape", "Shape", int, 10.0, 3.0, 20.0, "Number of sides on each floret polygon (3 = triangle, 4 = square, …)."),
        param!("scale", "Scale", unlimited_float, 0.02, 0.0, 100.0, "Base size of each floret. Multiplied by the per-floret distance factor `sc`, so florets shrink toward the rim (or centre, when inverted)."),
        param!("angle", "Angle", unlimited_float, 180.0, 0.0, 360.0, "Phyllotaxis angle seed. Multiplied by the golden-angle factor `(3 − √5)`; near 180 gives the classic sunflower packing."),
        param!("color", "Color", unlimited_float, 0.0, 0.0, 1.0, "Unused — JWildfire overwrites the colour with the per-floret distance factor. Kept for `.flame` round-trip parity."),
        param!("invert", "Invert", bool, false, "When on, florets grow toward the rim instead of the centre (`sc = r/rmax` instead of `1 − r/rmax`); also flips the colour gradient."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// Sample a point inside a unit regular `n_edges`-gon (solid, fill=0) using
// FractalDesire's nBlur distribution, then write the floret colour. Returns
// the sampled point in the floret's local frame (centre-relative, unit
// polygon). Shared by the 2D and 3D bodies.
fn sunflower_sample(shape: i32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let n_edges = f32(max(shape, 3));
    let mid_angle = two_pi / n_edges;
    let tan90 = tan(pi * 0.5 + mid_angle * 0.5);
    let half = mid_angle * 0.5;
    let sin_h = sin(half);
    let cos_h = cos(half);
    let arc_tan1 = 13.0 / pow(n_edges, 1.3);
    let arc_tan2 = 2.0 * atan(arc_tan1 / -2.0);

    // angXY: a random sector (rand() % nEdges) plus a smooth in-sector
    // position from the arctan angular bias.
    let sector = floor(rng_nextf(rng) * n_edges);
    let frac = atan(arc_tan1 * (rng_nextf(rng) - 0.5)) / arc_tan2 + 0.5;
    let ang_xy = (frac + sector) * mid_angle;
    let sx = sin(ang_xy);
    let sy = cos(ang_xy);

    // Edge-limit radius for the sector (JWF's while-reduce of angXY into one
    // sector, then the tan-based edge intersection). sqrt(random) gives a
    // uniform-area (solid) fill.
    let ang_r = ang_xy - mid_angle * floor(ang_xy / mid_angle);
    let x_tmp = tan90 / (tan90 - tan(ang_r));
    let y_tmp = x_tmp * tan(ang_r);
    let len_outer = sqrt(x_tmp * x_tmp + y_tmp * y_tmp);
    let ran = sqrt(rng_nextf(rng)) * len_outer;
    let px = sx * ran;
    let py = sy * ran;

    // plotPolygon's mid-angle/2 rotation.
    return vec2<f32>(cos_h * px - sin_h * py, sin_h * px + cos_h * py);
}

fn variation_sunflower(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let n_points = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let shape = i32(get_param(xform_id, variation_id, 1u));
    let scale = get_param(xform_id, variation_id, 2u);
    let angle = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 5u));
    let pi = 3.14159265358979;

    // Pick a random floret and recompute its spiral position / scale / colour
    // from the index (the build loop is deterministic in `i`).
    let i = floor(rng_nextf(rng) * f32(n_points));
    let ang = angle * (3.0 - sqrt(5.0));
    let rmax = sqrt(f32(n_points) + 1.0) / 30.0;
    let r = sqrt(i + 1.0) / 30.0;
    let t = (i + 1.0) * ang * pi / 180.0;
    let cx = r * cos(t) / rmax;
    let cy = r * sin(t) / rmax;
    var sc = 1.0 - r / rmax;
    if (invert == 1) { sc = r / rmax; }
    let poly_scale = scale * sc;

    *vc = sc;

    // Floret Ngon angle is 0 (cosa=1, sina=0), so the body rotation is
    // identity: out = sample · poly_scale + centre.
    let s = sunflower_sample(shape, rng);
    return vec2<f32>(s.x * poly_scale + cx, s.y * poly_scale + cy);
}
"#,
    wgsl_3d: r#"
fn sunflower_sample(shape: i32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let n_edges = f32(max(shape, 3));
    let mid_angle = two_pi / n_edges;
    let tan90 = tan(pi * 0.5 + mid_angle * 0.5);
    let half = mid_angle * 0.5;
    let sin_h = sin(half);
    let cos_h = cos(half);
    let arc_tan1 = 13.0 / pow(n_edges, 1.3);
    let arc_tan2 = 2.0 * atan(arc_tan1 / -2.0);

    let sector = floor(rng_nextf(rng) * n_edges);
    let frac = atan(arc_tan1 * (rng_nextf(rng) - 0.5)) / arc_tan2 + 0.5;
    let ang_xy = (frac + sector) * mid_angle;
    let sx = sin(ang_xy);
    let sy = cos(ang_xy);

    let ang_r = ang_xy - mid_angle * floor(ang_xy / mid_angle);
    let x_tmp = tan90 / (tan90 - tan(ang_r));
    let y_tmp = x_tmp * tan(ang_r);
    let len_outer = sqrt(x_tmp * x_tmp + y_tmp * y_tmp);
    let ran = sqrt(rng_nextf(rng)) * len_outer;
    let px = sx * ran;
    let py = sy * ran;

    return vec2<f32>(cos_h * px - sin_h * py, sin_h * px + cos_h * py);
}

fn variation_sunflower(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let n_points = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let shape = i32(get_param(xform_id, variation_id, 1u));
    let scale = get_param(xform_id, variation_id, 2u);
    let angle = get_param(xform_id, variation_id, 3u);
    let invert = i32(get_param(xform_id, variation_id, 5u));
    let pi = 3.14159265358979;

    let i = floor(rng_nextf(rng) * f32(n_points));
    let ang = angle * (3.0 - sqrt(5.0));
    let rmax = sqrt(f32(n_points) + 1.0) / 30.0;
    let r = sqrt(i + 1.0) / 30.0;
    let t = (i + 1.0) * ang * pi / 180.0;
    let cx = r * cos(t) / rmax;
    let cy = r * sin(t) / rmax;
    var sc = 1.0 - r / rmax;
    if (invert == 1) { sc = r / rmax; }
    let poly_scale = scale * sc;

    *vc = sc;

    let s = sunflower_sample(shape, rng);
    // Z passes through (JWF gates `pVarTP.z += pAmount·z` on preserve_z; the
    // shader builder zeroes this contribution when preserve_z is off).
    return vec3<f32>(s.x * poly_scale + cx, s.y * poly_scale + cy, p.z);
}
"#,
};
