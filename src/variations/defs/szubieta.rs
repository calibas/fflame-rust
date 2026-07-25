//! `szubieta` — grid-of-polygons base shape (Jesus Sosa, 2018).
//!
//! Plots a `width × height` grid where each cell is a small regular
//! 20-gon. Cell color comes from one of two bitwise pattern formulas
//! ([CircleSquares](http://paulbourke.net/fractals/circlesquares/) for
//! `type=0`,
//! [Square Tile Fractal](http://paulbourke.net/fractals/squaretile/)
//! for `type=1`).
//!
//! JWildfire's implementation extends `DrawFunc` and builds the grid
//! in `init()` as a list of `Ngon` primitives (~16K objects for
//! defaults), then `transform()` picks a random primitive and samples
//! a point on it via `plotPolygon`. That's the bit that was flagged
//! as architecturally blocked — but the algorithm is just per-call
//! math: pick a random cell, compute its color via the formula, pick
//! a random point on a unit 20-gon's edge, then scale + translate.
//! No primitive list, no per-instance state. This file computes the
//! same thing directly.
//! 
//! "inspired from a program in R language from this site"
//! https://fronkonstin.com/2017/05/22/sunflowers-for-colourlovers/
//!
//! Sources:
//! [`output/variation-jwf-source/SZubietaFunc.java`](../../../output/variation-jwf-source/SZubietaFunc.java),
//! [`output/jwildfire-vars/output/szubieta.cpp`](../../../output/jwildfire-vars/output/szubieta.cpp).


use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// SZubieta - Grid-of-polygons base shape.
///
/// # Authors
/// - Jesus Sosa
pub static SZUBIETA: VariationDef = VariationDef {
    name: "szubieta",
    aliases: &[],
    display_name: "SZubieta",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("width", "Width", unlimited_int, 128.0, 32.0, 256.0, "Grid width in cells. Each cell hosts one 20-gon; the bitwise pattern formula reads i ∈ [0, width) for the color computation."),
        param!("height", "Height", unlimited_int, 128.0, 32.0, 256.0, "Grid height in cells. Same role as `width` but for the j index."),
        param!("type", "Type", unlimited_int, 0.0, 0.0, 1.0, "Pattern formula. 0 = CircleSquares (`|i² − 2·(i|j) + j²| mod 255`) — concentric square-circle interference. 1 = Square Tile Fractal (`|i & (j − 2·(i^j) + j) & i| mod 256`) — bitwise-AND fractal tiling."),
        param!("scale", "Scale", float, 0.5, 0.0, 1.0, "Size of each 20-gon relative to the grid cell. 0 collapses every polygon to its center; 1 makes them touch."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Both bodies share the same per-iteration math:
//   1. Pick random integer (i, j) in [0, width) × [0, height).
//   2. Compute pattern color from (i, j) per `type`.
//   3. Sample a random point on a unit-radius 20-gon's edge — pick a
//      random vertex k ∈ [0, 20), pick edge interpolation t ∈ [0, 1].
//   4. Output = 0.1 · (scale · edge_point + cell_center), where
//      cell_center = (i − width/2, j − height/2).
//
// The Java's Ngon constructor sets the polygon angle to 0, so the
// cosa·x + sina·y rotation in JWildfire's transform body simplifies
// to the identity — no rotation matrix needed here.
//
// The 0.1 factor and the (i − width/2) centering are transcribed
// verbatim from the cpp / Java.

const WGSL_2D: &str = r#"
fn variation_szubieta(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let width = i32(get_param(xform_id, variation_id, 0u));
    let height = i32(get_param(xform_id, variation_id, 1u));
    let pattern_type = u32(get_param(xform_id, variation_id, 2u));
    let scale = get_param(xform_id, variation_id, 3u);

    // Pick random cell. Width/height are clamped to [32, 256] per the
    // Java's `setParameter`, but defend against bad imports with a
    // max(1) — width=0 would divide by zero in the inner conversion.
    let w_safe = max(width, 1);
    let h_safe = max(height, 1);
    let i = i32(floor(rng_nextf(rng) * f32(w_safe)));
    let j = i32(floor(rng_nextf(rng) * f32(h_safe)));

    // Pattern color — bitwise formulas straight from the cpp. Both
    // produce values mod 255/256; we always divide by 255.0 to match
    // JWildfire (yes, even the mod-256 path divides by 255 — the cpp
    // does the same).
    var x_raw: i32 = 0;
    if (pattern_type == 0u) {
        x_raw = (i * i - 2 * (i | j) + j * j) % 255;
    } else {
        x_raw = (i & (j - 2 * (i ^ j) + j) & i) % 256;
    }
    let color = f32(abs(x_raw)) / 255.0;
    *vc = color;

    // Random point on a unit 20-gon's edge: pick a random vertex k
    // and a random fraction t along the edge to the next vertex.
    let n_sides: f32 = 20.0;
    let k = floor(rng_nextf(rng) * n_sides);
    let t = rng_nextf(rng);
    let two_pi: f32 = 6.28318530718;
    let angle_k = two_pi * k / n_sides;
    let angle_k1 = two_pi * (k + 1.0) / n_sides;
    let vx = (1.0 - t) * cos(angle_k) + t * cos(angle_k1);
    let vy = (1.0 - t) * sin(angle_k) + t * sin(angle_k1);

    // Cell-center position (centered grid).
    let cx = f32(i) - f32(width) * 0.5;
    let cy = f32(j) - f32(height) * 0.5;

    return vec2<f32>(0.1 * (vx * scale + cx), 0.1 * (vy * scale + cy));
}
"#;

const WGSL_3D: &str = r#"
fn variation_szubieta(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let width = i32(get_param(xform_id, variation_id, 0u));
    let height = i32(get_param(xform_id, variation_id, 1u));
    let pattern_type = u32(get_param(xform_id, variation_id, 2u));
    let scale = get_param(xform_id, variation_id, 3u);

    let w_safe = max(width, 1);
    let h_safe = max(height, 1);
    let i = i32(floor(rng_nextf(rng) * f32(w_safe)));
    let j = i32(floor(rng_nextf(rng) * f32(h_safe)));

    var x_raw: i32 = 0;
    if (pattern_type == 0u) {
        x_raw = (i * i - 2 * (i | j) + j * j) % 255;
    } else {
        x_raw = (i & (j - 2 * (i ^ j) + j) & i) % 256;
    }
    let color = f32(abs(x_raw)) / 255.0;
    *vc = color;

    let n_sides: f32 = 20.0;
    let k = floor(rng_nextf(rng) * n_sides);
    let t = rng_nextf(rng);
    let two_pi: f32 = 6.28318530718;
    let angle_k = two_pi * k / n_sides;
    let angle_k1 = two_pi * (k + 1.0) / n_sides;
    let vx = (1.0 - t) * cos(angle_k) + t * cos(angle_k1);
    let vy = (1.0 - t) * sin(angle_k) + t * sin(angle_k1);

    let cx = f32(i) - f32(width) * 0.5;
    let cy = f32(j) - f32(height) * 0.5;

    return vec3<f32>(0.1 * (vx * scale + cx), 0.1 * (vy * scale + cy), p.z);
}
"#;
