//! `menger` — Menger sponge / tesseract IFS as a single variation
//! (original).
//!
//! The Menger IFS is classically built from dozens of linear
//! transforms (20 for the 3D sponge, 48 for the 4D tesseract at the
//! standard hole rule). This variation runs the whole IFS internally:
//! each call picks a random kept sub-cell and applies
//! `q' = q/3 + (2/3)·size·cell` — one transform replaces the whole
//! bank, the per-iteration transform-selection overhead disappears,
//! and `steps` composes several rounds per call (contraction 1/9 or
//! 1/27 per iteration instead of 1/3), which converges onto the
//! attractor dramatically faster for the same iteration budget.
//!
//! The **hole rule** generalizes the construction: a sub-cell of the
//! 3^d grid is kept iff it has at most `rule` zero coordinates.
//! `rule = 0` keeps only corners (Cantor dust), `1` is the classic
//! Menger sponge/tesseract, higher values remove only the most
//! central cells (fatter sponges). Cells are chosen by rejection
//! sampling — uniform over the kept set for any rule.
//!
//! **Dimension**: the 3D body runs a true 3D sponge or, in 4D mode, a
//! true 4D tesseract with the 4th coordinate carried per-thread via
//! `point_w` (`Feature::NeedsW`) — the fed-forward state stays an
//! honest 4D point and the plot is the orthographic shadow. The
//! `rot_xw/yw/zw` angles rotate the whole 4D attractor exactly (the
//! diagonal contraction commutes with rotations, so rotating the cell
//! offsets rotates the fractal); with them at 0 the shadow degenerates
//! to a layered 3D sponge, so give at least one a few degrees. The 2D
//! body is the Sierpinski carpet (8 of 9 cells at the default rule).
//!
//! Direct color: *Cell* paints by the last sub-cell chosen (the
//! per-transform coloring a transform bank would have given), *W*
//! paints by the 4th coordinate — the 4D depth strata.
//!
//! No JWildfire/Apophysis counterpart — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static MENGER: VariationDef = VariationDef {
    name: "menger",
    aliases: &[],
    display_name: "Menger",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsW, Feature::WritesColor, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("dim", "Dimension", enum, 0, &["3D Sponge", "4D Tesseract"], "3D Sponge iterates the classic Menger IFS in xyz. 4D Tesseract iterates in xyzw (the 4th coordinate rides the per-thread w register) and plots the orthographic shadow — combine with the XW/YW/ZW rotations to see genuine 4D structure. In 2D render mode both give the Sierpinski carpet."),
        param!("rule", "Hole Rule", int, 1.0, 0.0, 3.0, "Maximum zero-coordinates a kept sub-cell may have. 0 = corners only (Cantor dust), 1 = the classic Menger construction, 2+ = fatter sponges where only the most central cells are removed."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Half-extent of the fractal: the attractor spans ±size in every axis."),
        param!("steps", "Steps", int, 2.0, 1.0, 4.0, "IFS rounds per call — each round contracts by 1/3 toward a random kept cell. At 2 every iteration contracts by 1/9, sharpening the attractor much faster for the same iteration budget."),
        param!("rot_xw", "Rotate XW", angle, 0.0, "4D Tesseract mode: rotation in the x–w plane, degrees. Rotates the entire 4D attractor before its shadow is taken."),
        param!("rot_yw", "Rotate YW", angle, 0.0, "4D Tesseract mode: rotation in the y–w plane, degrees."),
        param!("rot_zw", "Rotate ZW", angle, 0.0, "4D Tesseract mode: rotation in the z–w plane, degrees."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Cell", "W"], "Direct-color source (needs the transform's Direct Color > 0). Cell: color by the last sub-cell chosen — the per-transform palette spread a 20/48-transform bank would have had. W: color by the 4th coordinate — the 4D depth strata (4D mode)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes (wrapped with fract)."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
// Random carpet cell in {-1,0,1}^2 with at most `rule` zeros
// (rejection sampled; uniform over the kept set).
fn menger_pick2(rng: ptr<function, RngState>, rule: i32) -> vec2<f32> {
    for (var t = 0; t < 12; t = t + 1) {
        let i = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        let j = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        var zeros = 0;
        if (i == 0.0) { zeros = zeros + 1; }
        if (j == 0.0) { zeros = zeros + 1; }
        if (zeros <= rule) { return vec2<f32>(i, j); }
    }
    return vec2<f32>(1.0, 1.0);
}

fn variation_menger(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let rule = i32(get_param(xform_id, variation_id, 1u));
    let size = get_param(xform_id, variation_id, 2u);
    let steps = i32(get_param(xform_id, variation_id, 3u));
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);

    var q = p;
    var code = 0.0;
    for (var s = 0; s < steps; s = s + 1) {
        let cell = menger_pick2(rng, rule);
        q = q / 3.0 + (2.0 * size / 3.0) * cell;
        code = ((cell.x + 1.0) * 3.0 + (cell.y + 1.0) + 0.5) / 9.0;
    }
    if (dc_mode != 0u) {
        *vc = fract(code * dc_scale);
    }
    return q;
}
"#;

const WGSL_3D: &str = r#"
// Random cell in {-1,0,1}^3 with at most `rule` zeros.
fn menger_pick3(rng: ptr<function, RngState>, rule: i32) -> vec3<f32> {
    for (var t = 0; t < 12; t = t + 1) {
        let i = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        let j = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        let k = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        var zeros = 0;
        if (i == 0.0) { zeros = zeros + 1; }
        if (j == 0.0) { zeros = zeros + 1; }
        if (k == 0.0) { zeros = zeros + 1; }
        if (zeros <= rule) { return vec3<f32>(i, j, k); }
    }
    return vec3<f32>(1.0, 1.0, 1.0);
}

// Random cell in {-1,0,1}^4 with at most `rule` zeros.
fn menger_pick4(rng: ptr<function, RngState>, rule: i32) -> vec4<f32> {
    for (var t = 0; t < 12; t = t + 1) {
        let i = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        let j = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        let k = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        let l = f32(min(u32(rng_nextf(rng) * 3.0), 2u)) - 1.0;
        var zeros = 0;
        if (i == 0.0) { zeros = zeros + 1; }
        if (j == 0.0) { zeros = zeros + 1; }
        if (k == 0.0) { zeros = zeros + 1; }
        if (l == 0.0) { zeros = zeros + 1; }
        if (zeros <= rule) { return vec4<f32>(i, j, k, l); }
    }
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

fn variation_menger(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let dim = u32(get_param(xform_id, variation_id, 0u));
    let rule = i32(get_param(xform_id, variation_id, 1u));
    let size = get_param(xform_id, variation_id, 2u);
    let steps = i32(get_param(xform_id, variation_id, 3u));
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);

    if (dim == 0u) {
        // 3D Menger sponge.
        var q = p;
        var code = 0.0;
        for (var s = 0; s < steps; s = s + 1) {
            let cell = menger_pick3(rng, rule);
            q = q / 3.0 + (2.0 * size / 3.0) * cell;
            code = (((cell.x + 1.0) * 3.0 + (cell.y + 1.0)) * 3.0 + (cell.z + 1.0) + 0.5) / 27.0;
        }
        point_w_out = point_w;
        if (dc_mode == 1u) {
            *vc = fract(code * dc_scale);
        } else if (dc_mode == 2u) {
            *vc = fract(point_w * dc_scale);
        }
        return q;
    }

    // 4D Menger tesseract: honest 4D state via the w register; the
    // rotations turn the whole attractor in 4D (they commute with the
    // diagonal contraction, so rotating the offsets rotates the
    // fractal), then the xyz shadow is plotted.
    let d2r = 0.01745329252;
    let axw = get_param(xform_id, variation_id, 4u) * d2r;
    let ayw = get_param(xform_id, variation_id, 5u) * d2r;
    let azw = get_param(xform_id, variation_id, 6u) * d2r;
    let cxw = cos(axw); let sxw = sin(axw);
    let cyw = cos(ayw); let syw = sin(ayw);
    let czw = cos(azw); let szw = sin(azw);

    var q = vec4<f32>(p, point_w);
    var code = 0.0;
    for (var s = 0; s < steps; s = s + 1) {
        let cell = menger_pick4(rng, rule);
        var off = (2.0 * size / 3.0) * cell;
        off = vec4<f32>(cxw * off.x - sxw * off.w, off.y, off.z, sxw * off.x + cxw * off.w);
        off = vec4<f32>(off.x, cyw * off.y - syw * off.w, off.z, syw * off.y + cyw * off.w);
        off = vec4<f32>(off.x, off.y, czw * off.z - szw * off.w, szw * off.z + czw * off.w);
        q = q / 3.0 + off;
        code = ((((cell.x + 1.0) * 3.0 + (cell.y + 1.0)) * 3.0 + (cell.z + 1.0)) * 3.0 + (cell.w + 1.0) + 0.5) / 81.0;
    }
    point_w_out = q.w;
    if (dc_mode == 1u) {
        *vc = fract(code * dc_scale);
    } else if (dc_mode == 2u) {
        *vc = fract(q.w * dc_scale);
    }
    return q.xyz;
}
"#;
