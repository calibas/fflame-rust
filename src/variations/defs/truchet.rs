//! Truchet family — Tatyana Zabanova's `truchet_fill` (transcribed by Rick Sidwell)
//!
//! `truchet_fill` is a tile-grid variation that draws Truchet-style arcs
//! across each unit cell, with a deterministic-hash-based per-cell tile
//! orientation. It uses the variation weight non-trivially: `scale =
//! 1/VVAR` divides the input pre-tiling, AND the cpp's FPx/FPy lines
//! lack the usual `VVAR *` multiplier — making the X/Y output
//! weight-independent in the cpp semantics.
//!
//! Source: output/jwildfire-vars/output/truchet_fill.cpp
//!
//! Approach: `needs_transform: true` to read the per-variation weight,
//! then divide the body's output by `w` so the outer multiplier (`× w`)
//! restores the cpp result exactly.
//!
//! Other Truchet-family entries from upstream (`truchet`, `truchet_ae`,
//! `truchet2`, `triantruchet`, `arctruchet`) are deferred — most are in
//! the `unportable_dc` (writes color) or `unported_stub` buckets.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// truchet_fill: Truchet tile fill (Tatyana Zabanova)
//   Init clamps:
//     _exponent  = clamp(exponent, 0.001, 2.0)
//     _onen      = 1 / _exponent
//     _width     = clamp(arc_width, 0.001, 1.0)
//     _seed2     = sqrt(1.5·seed) / (0.5·seed) · 0.25     (NaN at seed=0;
//                  body special-cases seed==0 so the value is never read)
//
//   Body:
//     scale = 1/w
//     (x, y) = wrap(input · scale, [0, 1])
//     tiletype = 0 if seed=0; 1 if seed=1; else hash-derived 0/1
//     r0, r1 = lp-norm distances to two of the four cell corners,
//              choosing pair by tiletype
//     if |r0 − 0.5| / rmax < 1: arc passes through (x1, y1)
//     if |r1 − 0.5| / rmax < 1: arc passes through (1, 1) corner pair
//     FPx += x1 [+ second-arc term] − x   (no VVAR multiplier in upstream)
//     FPy += y1 [+ second-arc term] − y
// =============================================================================
/// Truchet-style tile fill — divides the plane into unit cells and draws
/// arc patterns through each cell with hash-based per-cell orientation.
/// Produces interlocking curved-tile patterns.
///
/// # Authors
/// - Tatyana Zabanova
pub static TRUCHET_FILL: VariationDef = VariationDef {
    name: "truchet_fill",
    aliases: &[],
    display_name: "Truchet Fill",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("exponent", "Exponent", unlimited_float, 2.0, 0.001, 2.0,
            "Lp-norm exponent for the cell distance metric — controls how rounded or cornered the arcs are. Clamped to [0.001, 2.0]."),
        param!("arc_width", "Arc Width", unlimited_float, 0.5, 0.001, 1.0,
            "Arc thickness within each cell. Clamped to [0.001, 1.0]."),
        param!("seed", "Seed", unlimited_float, 0.0, 0.0, 100.0,
            "Hash seed for per-cell tile orientation. 0 = all same orientation; 1 = all flipped; other values produce a varied pattern."),
    ],
    needs_transform: true,
    writes_color: false,
    // 4 derived values at slots 3..7:
    //   3: _exponent  clamp(exponent, 0.001, 2.0)
    //   4: _onen      1 / _exponent
    //   5: _width     clamp(arc_width, 0.001, 1.0)
    //   6: _seed2     sqrt(1.5·seed) / (0.5·seed) · 0.25
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_truchet_fill(user: array<f32, 3>) -> array<f32, 4> {
    let exp_in = user[0];
    let arc_in = user[1];
    let seed = user[2];
    let exp_clamped = clamp(exp_in, 0.001, 2.0);
    let safe_seed = select(seed, 1e-30, abs(seed) < 1e-30);
    var out: array<f32, 4>;
    out[0] = exp_clamped;
    out[1] = 1.0 / exp_clamped;
    out[2] = clamp(arc_in, 0.001, 1.0);
    out[3] = sqrt(max(seed * 1.5, 0.0)) / (safe_seed * 0.5) * 0.25;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_truchet_fill(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let seed = get_param(xform_id, variation_id, 2u);
    let exponent = get_param(xform_id, variation_id, 3u);
    let onen = get_param(xform_id, variation_id, 4u);
    let width = get_param(xform_id, variation_id, 5u);
    let seed2 = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let scale = inv_w;
    let modbase = 65535.0;
    let multiplier = 32747.0;
    let offset = 12345.0;

    var x = p.x * scale;
    var y = p.y * scale;
    let intx = round(x);
    let inty = round(y);
    var r = x - intx;
    x = select(r, 1.0 + r, r < 0.0);
    r = y - inty;
    y = select(r, 1.0 + r, r < 0.0);

    var tiletype: f32 = 0.0;
    if (seed == 0.0) {
        tiletype = 0.0;
    } else if (seed == 1.0) {
        tiletype = 1.0;
    } else {
        let xrand = round(abs(p.x)) * seed2;
        let yrand = round(abs(p.y)) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        var randint = (seed + niter) * seed2 * 0.5;
        randint = randint * multiplier + offset;
        randint = randint - floor(randint / modbase) * modbase;
        tiletype = randint - floor(randint / 2.0) * 2.0;
    }

    var r0: f32; var r1: f32;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y - 1.0), exponent), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x), exponent) + pow(abs(y - 1.0), exponent), onen);
    }

    var x1: f32 = 0.0;
    var y1: f32 = 0.0;
    let r00 = abs(r0 - 0.5) / safe_rmax;
    if (r00 < 1.0) {
        x1 = 2.0 * (x + floor(p.x));
        y1 = 2.0 * (y + floor(p.y));
    }

    var fpx_inc: f32;
    var fpy_inc: f32;
    let r11 = abs(r1 - 0.5) / safe_rmax;
    if (r11 < 1.0) {
        fpx_inc = x1 + 2.0 * (x + floor(p.x)) - p.x;
        fpy_inc = y1 + 2.0 * (y + floor(p.y)) - p.y;
    } else {
        fpx_inc = x1 - p.x;
        fpy_inc = y1 - p.y;
    }

    return vec2<f32>(fpx_inc * inv_w, fpy_inc * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_truchet_fill(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let seed = get_param(xform_id, variation_id, 2u);
    let exponent = get_param(xform_id, variation_id, 3u);
    let onen = get_param(xform_id, variation_id, 4u);
    let width = get_param(xform_id, variation_id, 5u);
    let seed2 = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let scale = inv_w;
    let modbase = 65535.0;
    let multiplier = 32747.0;
    let offset = 12345.0;

    var x = p.x * scale;
    var y = p.y * scale;
    let intx = round(x);
    let inty = round(y);
    var r = x - intx;
    x = select(r, 1.0 + r, r < 0.0);
    r = y - inty;
    y = select(r, 1.0 + r, r < 0.0);

    var tiletype: f32 = 0.0;
    if (seed == 0.0) {
        tiletype = 0.0;
    } else if (seed == 1.0) {
        tiletype = 1.0;
    } else {
        let xrand = round(abs(p.x)) * seed2;
        let yrand = round(abs(p.y)) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        var randint = (seed + niter) * seed2 * 0.5;
        randint = randint * multiplier + offset;
        randint = randint - floor(randint / modbase) * modbase;
        tiletype = randint - floor(randint / 2.0) * 2.0;
    }

    var r0: f32; var r1: f32;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y - 1.0), exponent), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x), exponent) + pow(abs(y - 1.0), exponent), onen);
    }

    var x1: f32 = 0.0;
    var y1: f32 = 0.0;
    let r00 = abs(r0 - 0.5) / safe_rmax;
    if (r00 < 1.0) {
        x1 = 2.0 * (x + floor(p.x));
        y1 = 2.0 * (y + floor(p.y));
    }

    var fpx_inc: f32;
    var fpy_inc: f32;
    let r11 = abs(r1 - 0.5) / safe_rmax;
    if (r11 < 1.0) {
        fpx_inc = x1 + 2.0 * (x + floor(p.x)) - p.x;
        fpy_inc = y1 + 2.0 * (y + floor(p.y)) - p.y;
    } else {
        fpx_inc = x1 - p.x;
        fpy_inc = y1 - p.y;
    }

    // Z preserve scales with weight (FPz += VVAR · FTz upstream); leave
    // p.z so the outer multiplier produces VVAR · p.z.
    return vec3<f32>(fpx_inc * inv_w, fpy_inc * inv_w, p.z);
}
"#),
};
