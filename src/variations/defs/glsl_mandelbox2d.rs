//! `glsl_mandelbox2D` — 2D Mandelbox shadertoy port (Jesus Sosa, Oct 2018).
//!
//! Picks a uniform random point in a square grid (`density_pixels` cells
//! per side), maps it into `[-7, 7]²` complex-plane space, runs 20
//! iterations of the boxfold-then-ballfold Mandelbox map with `time`
//! controlling the scaling depth, then derives RGB from the final
//! distance estimate via `(cos(d), sin(10d+1), cos(3d+1)) · 0.5 + 0.5`.
//!
//! Spatial output is the uniform `[-0.5, 0.5]²` sample — same shape as
//! every other `glsl_*` variation. The interesting visual content is
//! entirely in the per-pixel color computation.
//!
//! Source: [`output/variation-jwf-source/GLSLMandelBox2DFunc.java`](../../../output/variation-jwf-source/GLSLMandelBox2DFunc.java).
//!
//! # Authors
//! - Jesus Sosa (Apophysis plugin, 2018)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 2D Mandelbox with shadertoy-style direct-RGB color output. The
/// spatial pattern is a uniform unit square — every `glsl_*` variation
/// shares this — and the variation's contribution is the per-pixel
/// color computed via the inlined boxfold/ballfold loop.
///
/// 4 params: `density_pixels` (sampling grid resolution), `seed`
/// (JWildfire-CPU only, accepted for round-trip), `time` (animation
/// parameter embedded in the formula), `gradient` (color output mode —
/// only mode 0, direct RGB, is honored; see param tooltip).
///
/// # Authors
/// - Jesus Sosa
pub static GLSL_MANDELBOX2D: VariationDef = VariationDef {
    name: "glsl_mandelbox2D",
    aliases: &[],
    display_name: "GLSL MandelBox2D",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesRgb],
    parameters: &[
        param!("density_pixels", "Density Pixels", unlimited_int, 1000000.0, 100.0, 10000000.0, "Discrete grid resolution for sampling. Higher = smoother continuous output; lower = visible pixelation as the same `(i, j)` cell gets repeatedly hit. JWildfire's UI label is 'Density Pixels'; clamped at runtime to `[100, 10000000]`."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only: seeds a Java `Random` and assigns `time = rand(0, 10000)` as a side effect. **Not honored on GPU** — `time` is a directly-user-set parameter here. Value accepted for round-trip with `.flame` XML."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter; embedded in the Mandelbox iteration as `O.z = -5 · cos(time · 0.1)`. Animate this for a flowing/morphing pattern (the same `time` slider every `glsl_*` variation honors)."),
        param!("gradient", "Gradient", unlimited_int, 0.0, 0.0, 1.0, "JWildfire color output mode. **Only mode 0 (direct RGB) is honored** — value accepted for round-trip but the variation always writes RGB to `vrc`. JWildfire mode 1 (palette index via `color.r · color.g`) would require both `WritesColor` and `WritesRgb` features, and the unwritten register's sentinel-init darkens the plot-time blend; deferred until the framework supports per-call feature selection."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Algorithm (transcribed from JWildfire's getRGBColor + transform):
//   1. Pick uniform random (i, j) in [0, density_pixels)².
//   2. Map to I ∈ [-7, 7]² via I = 7 · (2·(i+0.5)/W - 1).
//   3. Init O = vec4(I.x, I.y, -5·cos(time·0.1), 1).
//      O.x, O.y stay fixed (the "+c" offset added each iteration).
//      O.z is the scaling factor, fixed throughout the loop.
//      O.w gets reused as a scratch register for length(I).
//   4. 20 iterations of:
//      - boxfold: I = clamp(I, -1, 1) · 2 - I
//      - ballfold: O.w = length(I); b = 4 if O.w<0.5 else 1/O.w if O.w<1 else 1
//      - scaling: I = I · (O.z · b) + (O.x, O.y)
//      - dist: d = b · d · |O.z| + 1
//   5. d = pow(length(I) / d, 0.1) · 5
//   6. color = (cos(d), sin(10d+1), cos(3d+1)) · 0.5 + 0.5
//   7. *vrc = color
//   8. Output spatial = (i/W - 0.5, j/W - 0.5).
//
// Per call: 20 inner iterations × ~15 ops each = ~300 ops. With 32K
// threads × 256 chaos iters, that's ~2.5B ops per dispatch — well within
// the TDR budget. No runtime clamps needed.

const WGSL_2D: &str = r#"
fn variation_glsl_mandelbox2D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let resolution_raw = get_param(xform_id, variation_id, 0u);
    let resolution = max(resolution_raw, 100.0);
    // seed (slot 1) is CPU-only — ignored.
    let time = get_param(xform_id, variation_id, 2u);
    // gradient (slot 3) is accepted for round-trip — only mode 0 is honored.

    // Pick a random cell in [0, resolution)². Match JWildfire's offset
    // by +0.5 inside the cell so the center is sampled.
    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;

    // Map cell center to mandelbox input space [-7, 7]².
    var i_vec = vec2<f32>(
        7.0 * (x + x - resolution) / resolution,
        7.0 * (y + y - resolution) / resolution,
    );

    // O.x, O.y = original I (added each iteration as the +c offset).
    // O.z = scaling factor, time-derived, fixed.
    // O.w = scratch (set to length(I) inside the loop for the ballfold test).
    var o = vec4<f32>(i_vec.x, i_vec.y, -5.0 * cos(time * 0.1), 1.0);
    var d: f32 = 1.0;

    for (var k: u32 = 0u; k < 20u; k = k + 1u) {
        // boxfold: reflect I out of the [-1, 1] box.
        i_vec = clamp(i_vec, vec2<f32>(-1.0), vec2<f32>(1.0)) * 2.0 - i_vec;
        // ballfold: branch on |I|.
        o.w = length(i_vec);
        var b: f32;
        if (o.w < 0.5) {
            b = 4.0;
        } else if (o.w < 1.0) {
            b = 1.0 / o.w;
        } else {
            b = 1.0;
        }
        // scaling: I' = I · (O.z · b) + (O.x, O.y).
        i_vec = i_vec * (o.z * b) + vec2<f32>(o.x, o.y);
        // distance estimate accumulator.
        d = b * d * abs(o.z) + 1.0;
    }

    // Final distance value → palette-traversal angle d_final.
    let d_final = pow(length(i_vec) / d, 0.1) * 5.0;
    // Color from interleaved sin/cos of d_final, mapped to [0, 1]³.
    let color = vec3<f32>(
        cos(d_final),
        sin(10.0 * d_final + 1.0),
        cos(3.0 * d_final + 1.0),
    ) * 0.5 + 0.5;
    *vrc = color;

    // Spatial output: uniform [-0.5, 0.5]² (every glsl_* variation).
    return vec2<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5);
}
"#;

// 3D body: noise math is purely 2D (the original is VARTYPE_2D); the
// wrapper passes p.z through unchanged.
const WGSL_3D: &str = r#"
fn variation_glsl_mandelbox2D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let resolution_raw = get_param(xform_id, variation_id, 0u);
    let resolution = max(resolution_raw, 100.0);
    let time = get_param(xform_id, variation_id, 2u);

    let i_f = floor(rng_nextf(rng) * resolution);
    let j_f = floor(rng_nextf(rng) * resolution);
    let x = i_f + 0.5;
    let y = j_f + 0.5;

    var i_vec = vec2<f32>(
        7.0 * (x + x - resolution) / resolution,
        7.0 * (y + y - resolution) / resolution,
    );

    var o = vec4<f32>(i_vec.x, i_vec.y, -5.0 * cos(time * 0.1), 1.0);
    var d: f32 = 1.0;

    for (var k: u32 = 0u; k < 20u; k = k + 1u) {
        i_vec = clamp(i_vec, vec2<f32>(-1.0), vec2<f32>(1.0)) * 2.0 - i_vec;
        o.w = length(i_vec);
        var b: f32;
        if (o.w < 0.5) {
            b = 4.0;
        } else if (o.w < 1.0) {
            b = 1.0 / o.w;
        } else {
            b = 1.0;
        }
        i_vec = i_vec * (o.z * b) + vec2<f32>(o.x, o.y);
        d = b * d * abs(o.z) + 1.0;
    }

    let d_final = pow(length(i_vec) / d, 0.1) * 5.0;
    let color = vec3<f32>(
        cos(d_final),
        sin(10.0 * d_final + 1.0),
        cos(3.0 * d_final + 1.0),
    ) * 0.5 + 0.5;
    *vrc = color;

    return vec3<f32>(i_f / resolution - 0.5, j_f / resolution - 0.5, p.z);
}
"#;
