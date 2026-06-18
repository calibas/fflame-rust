//! dc_hexes_wf (thargor6, on slobo777's Hexes) — hexagonal Voronoi cell
//! warp with Voronoi-distance direct color.
//!
//! Breaks the plane into hexagonal cells and applies a per-cell
//! power/scale/rotation warp (slobo777's "Hexes",
//! http://slobo777.deviantart.com/art/Apo-Plugins-Hexes-And-Crackle-99243824).
//! `dc_hexes_wf` is the direct-color specialization: it writes the
//! Voronoi distance `L` (scaled/offset, clamped) to the colour register,
//! so cell interiors fade through the palette and the hex lattice reads
//! as a coloured texture.
//!
//! Deterministic — no RNG. The math is a faithful transcription of
//! JWildfire's `getGPUCode`/`getGPUFunctions` (the CUDA port carries the
//! whole algorithm inline): map the point to hex axial coordinates, find
//! the nearest of the 3×3 surrounding cell centres, then Voronoi-warp
//! within the chosen cell and its six neighbours. `cellsize = 0` makes
//! the spatial warp a no-op (JWildfire skips the block), but Z still
//! passes through under preserve_z.
//!
//! Hex constants: a_hex=1/3, b_hex=√3/3, c_hex=-1/3, d_hex=√3/3
//! (cartesian→hex axial); a_cart=1.5, b_cart=-1.5, c_cart=d_cart=√3/2
//! (hex→cartesian). The helper block is inlined into both body strings;
//! only one of wgsl_2d/wgsl_3d compiles per flame, so the names don't
//! collide.
//!
//! Sources:
//!   - `output/variation-jwf-source/DCHexesWFFunc.java`
//!   - `output/variation-jwf-source/HexesFunc.java` (base params)
//!
//! # Authors
//! - slobo777 (Hexes algorithm)
//! - thargor6 (direct-color specialization)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Hexagonal Voronoi cell warp with Voronoi-distance direct color — tiles
/// the plane into hex cells, warps each by `power`/`scale`/`rotate`, and
/// writes the Voronoi distance to the colour register (`L·color_scale +
/// color_offset`, clamped). Produces coloured hexagonal-lattice textures.
///
/// # Authors
/// - slobo777 (Hexes)
/// - thargor6 (dc_ specialization)
pub static DC_HEXES_WF: VariationDef = VariationDef {
    name: "dc_hexes_wf",
    aliases: &[],
    display_name: "DC Hexes WF",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::WritesColor],
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, 0.0, 10.0, "Hex cell size. 0 disables the spatial warp entirely (Z still passes through)."),
        param!("power", "Power", unlimited_float, 1.0, -10.0, 10.0, "Exponent on the Voronoi distance for the target radius: `trgL = (L1 + 1e-6)^power · scale`."),
        param!("rotate", "Rotate", unlimited_float, 0.166, -10.0, 10.0, "Per-cell rotation, in turns (multiplied by 2π). 0.166 ≈ 1/6 turn aligns with the hex symmetry."),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Per-cell scale factor applied to the warped radius."),
        param!("color_scale", "Color Scale", unlimited_float, 0.5, -10.0, 10.0, "Multiplier on the Voronoi distance `L` for the colour index. Visible colour requires the transform's Direct Color slider > 0."),
        param!("color_offset", "Color Offset", unlimited_float, 0.0, -10.0, 10.0, "Offset added to `L·color_scale` before clamping the colour index to [0, 1]."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn hexes_cell_centre(x: i32, y: i32, s: f32) -> vec2<f32> {
    let c = 0.86602540378443864;   // sqrt(3)/2
    return vec2<f32>((1.5 * f32(x) - 1.5 * f32(y)) * s, (c * f32(x) + c * f32(y)) * s);
}
fn hexes_closest9(P: array<vec2<f32>, 9>, U: vec2<f32>) -> i32 {
    var d2min = 1.0e32;
    var j = 0;
    for (var i = 0; i < 9; i = i + 1) {
        let d = P[i] - U;
        let d2 = dot(d, d);
        if (d2 < d2min) { d2min = d2; j = i; }
    }
    return j;
}
fn hexes_cell_choice(q: i32) -> vec2<i32> {
    // {-1,-1},{-1,0},{-1,1},{0,-1},{0,0},{0,1},{1,-1},{1,0},{1,1}
    return vec2<i32>(q / 3 - 1, q % 3 - 1);
}
fn hexes_vratio(P: vec2<f32>, Q: vec2<f32>, U: vec2<f32>) -> f32 {
    let pmq = P - Q;
    if (pmq.x == 0.0 && pmq.y == 0.0) { return 1.0; }
    return 2.0 * dot(U - Q, pmq) / dot(pmq, pmq);
}
fn hexes_voronoi7(P: array<vec2<f32>, 9>, U: vec2<f32>) -> f32 {
    var ratiomax = -1.0e20;
    for (var i = 1; i < 7; i = i + 1) {
        let ratio = hexes_vratio(P[i], P[0], U);
        if (ratio > ratiomax) { ratiomax = ratio; }
    }
    return ratiomax;
}
fn hexes_warp(px: f32, py: f32, cellsize: f32, power: f32, rotate: f32, scale: f32,
              color_scale: f32, color_offset: f32, vc: ptr<function, f32>) -> vec2<f32> {
    let s = cellsize;
    let two_pi = 6.28318530717959;
    let rot_sin = sin(rotate * two_pi);
    let rot_cos = cos(rotate * two_pi);
    let a_hex = 1.0 / 3.0;
    let b_hex = 0.5773502691896258;   // sqrt(3)/3
    let c_hex = -1.0 / 3.0;
    let d_hex = 0.5773502691896258;

    var U = vec2<f32>(px, py);
    var hx = i32(floor((a_hex * U.x + b_hex * U.y) / s));
    var hy = i32(floor((c_hex * U.x + d_hex * U.y) / s));

    var P: array<vec2<f32>, 9>;
    var idx = 0;
    for (var di = -1; di < 2; di = di + 1) {
        for (var dj = -1; dj < 2; dj = dj + 1) {
            P[idx] = hexes_cell_centre(hx + di, hy + dj, s);
            idx = idx + 1;
        }
    }
    let q = hexes_closest9(P, U);
    let ch = hexes_cell_choice(q);
    hx = hx + ch.x;
    hy = hy + ch.y;

    P[0] = hexes_cell_centre(hx, hy, s);
    P[1] = hexes_cell_centre(hx, hy + 1, s);
    P[2] = hexes_cell_centre(hx + 1, hy + 1, s);
    P[3] = hexes_cell_centre(hx + 1, hy, s);
    P[4] = hexes_cell_centre(hx, hy - 1, s);
    P[5] = hexes_cell_centre(hx - 1, hy - 1, s);
    P[6] = hexes_cell_centre(hx - 1, hy, s);

    let l1 = hexes_voronoi7(P, U);
    let dxo = U.x - P[0].x;
    let dyo = U.y - P[0].y;
    // max() keeps the pow base positive: l1 (a Voronoi distance) is
    // ~0 at cell centres and f32 rounding can nudge it slightly
    // negative, and WGSL pow(neg, y) is NaN (JWF's powf is finite).
    // JWF adds 1e-6 here for the same reason — to keep the base > 0.
    let trg_l = pow(max(l1 + 1e-06, 1e-06), power) * scale;
    var vx = dxo * rot_cos + dyo * rot_sin;
    var vy = -dxo * rot_sin + dyo * rot_cos;
    let u2 = vec2<f32>(vx + P[0].x, vy + P[0].y);
    let l2 = hexes_voronoi7(P, u2);
    let l = max(l1, l2);
    var r: f32;
    if (l < 0.5) {
        r = trg_l / l1;
    } else if (l > 0.8) {
        r = trg_l / l2;
    } else {
        r = ((trg_l / l1) * (0.8 - l) + (trg_l / l2) * (l - 0.5)) / 0.3;
    }
    vx = vx * r + P[0].x;
    vy = vy * r + P[0].y;

    *vc = clamp(l * color_scale + color_offset, 0.0, 1.0);
    return vec2<f32>(vx, vy);
}
fn variation_dc_hexes_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    if (cellsize == 0.0) {
        // JWF skips the cell block — zero spatial contribution, colour
        // register unchanged.
        return vec2<f32>(0.0, 0.0);
    }
    let power = get_param(xform_id, variation_id, 1u);
    let rotate = get_param(xform_id, variation_id, 2u);
    let scale = get_param(xform_id, variation_id, 3u);
    let color_scale = get_param(xform_id, variation_id, 4u);
    let color_offset = get_param(xform_id, variation_id, 5u);
    return hexes_warp(p.x, p.y, cellsize, power, rotate, scale, color_scale, color_offset, vc);
}
"#,
    wgsl_3d: r#"
fn hexes_cell_centre(x: i32, y: i32, s: f32) -> vec2<f32> {
    let c = 0.86602540378443864;   // sqrt(3)/2
    return vec2<f32>((1.5 * f32(x) - 1.5 * f32(y)) * s, (c * f32(x) + c * f32(y)) * s);
}
fn hexes_closest9(P: array<vec2<f32>, 9>, U: vec2<f32>) -> i32 {
    var d2min = 1.0e32;
    var j = 0;
    for (var i = 0; i < 9; i = i + 1) {
        let d = P[i] - U;
        let d2 = dot(d, d);
        if (d2 < d2min) { d2min = d2; j = i; }
    }
    return j;
}
fn hexes_cell_choice(q: i32) -> vec2<i32> {
    return vec2<i32>(q / 3 - 1, q % 3 - 1);
}
fn hexes_vratio(P: vec2<f32>, Q: vec2<f32>, U: vec2<f32>) -> f32 {
    let pmq = P - Q;
    if (pmq.x == 0.0 && pmq.y == 0.0) { return 1.0; }
    return 2.0 * dot(U - Q, pmq) / dot(pmq, pmq);
}
fn hexes_voronoi7(P: array<vec2<f32>, 9>, U: vec2<f32>) -> f32 {
    var ratiomax = -1.0e20;
    for (var i = 1; i < 7; i = i + 1) {
        let ratio = hexes_vratio(P[i], P[0], U);
        if (ratio > ratiomax) { ratiomax = ratio; }
    }
    return ratiomax;
}
fn hexes_warp(px: f32, py: f32, cellsize: f32, power: f32, rotate: f32, scale: f32,
              color_scale: f32, color_offset: f32, vc: ptr<function, f32>) -> vec2<f32> {
    let s = cellsize;
    let two_pi = 6.28318530717959;
    let rot_sin = sin(rotate * two_pi);
    let rot_cos = cos(rotate * two_pi);
    let a_hex = 1.0 / 3.0;
    let b_hex = 0.5773502691896258;   // sqrt(3)/3
    let c_hex = -1.0 / 3.0;
    let d_hex = 0.5773502691896258;

    var U = vec2<f32>(px, py);
    var hx = i32(floor((a_hex * U.x + b_hex * U.y) / s));
    var hy = i32(floor((c_hex * U.x + d_hex * U.y) / s));

    var P: array<vec2<f32>, 9>;
    var idx = 0;
    for (var di = -1; di < 2; di = di + 1) {
        for (var dj = -1; dj < 2; dj = dj + 1) {
            P[idx] = hexes_cell_centre(hx + di, hy + dj, s);
            idx = idx + 1;
        }
    }
    let q = hexes_closest9(P, U);
    let ch = hexes_cell_choice(q);
    hx = hx + ch.x;
    hy = hy + ch.y;

    P[0] = hexes_cell_centre(hx, hy, s);
    P[1] = hexes_cell_centre(hx, hy + 1, s);
    P[2] = hexes_cell_centre(hx + 1, hy + 1, s);
    P[3] = hexes_cell_centre(hx + 1, hy, s);
    P[4] = hexes_cell_centre(hx, hy - 1, s);
    P[5] = hexes_cell_centre(hx - 1, hy - 1, s);
    P[6] = hexes_cell_centre(hx - 1, hy, s);

    let l1 = hexes_voronoi7(P, U);
    let dxo = U.x - P[0].x;
    let dyo = U.y - P[0].y;
    // max() keeps the pow base positive: l1 (a Voronoi distance) is
    // ~0 at cell centres and f32 rounding can nudge it slightly
    // negative, and WGSL pow(neg, y) is NaN (JWF's powf is finite).
    // JWF adds 1e-6 here for the same reason — to keep the base > 0.
    let trg_l = pow(max(l1 + 1e-06, 1e-06), power) * scale;
    var vx = dxo * rot_cos + dyo * rot_sin;
    var vy = -dxo * rot_sin + dyo * rot_cos;
    let u2 = vec2<f32>(vx + P[0].x, vy + P[0].y);
    let l2 = hexes_voronoi7(P, u2);
    let l = max(l1, l2);
    var r: f32;
    if (l < 0.5) {
        r = trg_l / l1;
    } else if (l > 0.8) {
        r = trg_l / l2;
    } else {
        r = ((trg_l / l1) * (0.8 - l) + (trg_l / l2) * (l - 0.5)) / 0.3;
    }
    vx = vx * r + P[0].x;
    vy = vy * r + P[0].y;

    *vc = clamp(l * color_scale + color_offset, 0.0, 1.0);
    return vec2<f32>(vx, vy);
}
fn variation_dc_hexes_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    // Z passes through (JWF gates `pVarTP.z += pAmount·z` on preserve_z,
    // OUTSIDE the cellsize block — applies even when cellsize == 0).
    if (cellsize == 0.0) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let power = get_param(xform_id, variation_id, 1u);
    let rotate = get_param(xform_id, variation_id, 2u);
    let scale = get_param(xform_id, variation_id, 3u);
    let color_scale = get_param(xform_id, variation_id, 4u);
    let color_offset = get_param(xform_id, variation_id, 5u);
    let v = hexes_warp(p.x, p.y, cellsize, power, rotate, scale, color_scale, color_offset, vc);
    return vec3<f32>(v.x, v.y, p.z);
}
"#,
};
