//! truchet (TyrantWave 2008)
//!
//! Original Truchet plugin — random tile pattern within a unit-square
//! grid. Each cell is one of two truchet types selected by a
//! deterministic hash of its (intx, inty) cell coordinates. Within
//! each cell, two arc segments connect opposite corners; an emitted
//! point falls on one of those arcs (within `arc_width` of the unit
//! distance).
//!
//! 7 user params:
//!   - extended (int 0/1): 0 = fast LCG hash, 1 = slow iterated LCG
//!   - exponent (clamped [0.001, 2]): Lp-norm exponent for arc shape
//!   - arc_width (clamped [0.001, 1])
//!   - rotation (radians)
//!   - size (clamped [0.001, 10])
//!   - seed
//!   - direct_color (int 0/1) — TC color writes skipped per
//!     writes_color compromise
//!
//! No init slots. Body has `scale = (cos(r) - sin(r)) / VVAR` plus
//! unweighted `+= size · (x + floor(FTx))` output lines — uses
//! needs_transform divide-out.
//!
//! Slow drawmode (extended=1) iterates the LCG `niter` times where
//! niter is bounded ≤ 1000; preserved as a runtime-bounded WGSL loop
//! with explicit `if (i >= 1000) break;` cap.
//!
//! Source: `output/jwildfire-vars/output/truchet.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Truchet tile pattern — divides the input plane into a unit-square grid;
/// each cell is one of two truchet types selected by a deterministic per-
/// cell hash of `(intx, inty)`. Within each cell, two arc segments connect
/// opposite corners; an emitted point falls on one of those arcs (within
/// `arc_width` of the unit distance). Arc shape controlled by the Lp-norm
/// `exponent`.
///
/// # Authors
/// - TyrantWave
pub static TRUCHET: VariationDef = VariationDef {
    name: "truchet",
    display_name: "Truchet",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("extended", "Extended", bool, false, "1 = slow iterated-LCG hash for tile-type selection (more cells produce distinct patterns); 0 = fast single-step LCG."),
        param!("exponent", "Exponent", unlimited_float, 2.0, 0.001, 2.0, "Lp-norm exponent for the arc shape (clamped to [0.001, 2]). 2 = circular arcs; lower values produce more angular shapes."),
        param!("arc_width", "Arc Width", unlimited_float, 0.5, 0.001, 1.0, "Width of the arc segments (clamped to [0.001, 1])."),
        param!("rotation", "Rotation", unlimited_float, 0.0, -10.0, 10.0, "Pre-rotation angle applied to the input, in radians."),
        param!("size", "Size", unlimited_float, 1.0, 0.001, 10.0, "Output scale factor for each cell (clamped to [0.001, 10])."),
        param!("seed", "Seed", unlimited_float, 50.0, -1000.0, 1000.0, "Hash seed for the per-cell tile-type selection."),
        param!("direct_color", "Direct Color", bool, false, "1 = enable direct-color writes (but the color write is dropped in this port — preserved for cpp parity)."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_truchet(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let extended = i32(get_param(xform_id, variation_id, 0u));
    let n = clamp(get_param(xform_id, variation_id, 1u), 0.001, 2.0);
    let width = clamp(get_param(xform_id, variation_id, 2u), 0.001, 1.0);
    let tdeg = get_param(xform_id, variation_id, 3u);
    let size = clamp(get_param(xform_id, variation_id, 4u), 0.001, 10.0);
    let seed_p = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let onen = 1.0 / n;
    let seed_abs = abs(seed_p);
    let seed2 = sqrt(seed_abs + seed_abs * 0.5 + 1e-30) / (seed_abs * 0.5 + 1e-30) * 0.25;
    let r0_init = -tdeg;
    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let scale = (cos(r0_init) - sin(r0_init)) * inv_w;

    let xs = p.x * scale;
    let ys = p.y * scale;
    let intx = round(xs);
    let inty = round(ys);
    let rx = xs - intx;
    let ry = ys - inty;
    let x = select(rx, 1.0 + rx, rx < 0.0);
    let y = select(ry, 1.0 + ry, ry < 0.0);

    var tiletype = 0.0;
    if (seed_abs == 1.0) {
        tiletype = 1.0;
    } else if (seed_abs > 0.0) {
        if (extended == 0) {
            let xrand = round(p.x) * seed2;
            let yrand = round(p.y) * seed2;
            let niter = xrand + yrand + xrand * yrand;
            var randint = (niter + seed_p) * seed2 * 0.5;
            randint = randint * 32747.0 + 12345.0;
            randint = randint - floor(randint / 65535.0) * 65535.0;
            tiletype = randint - floor(randint * 0.5) * 2.0;
        } else {
            let seed_floor = floor(seed_abs);
            let xrand = round(p.x);
            let yrand = round(p.y);
            var niter = abs(xrand + yrand + xrand * yrand);
            niter = min(niter, 1000.0);
            let niter_i = i32(niter);
            var randint = seed_floor + niter;
            for (var i = 0; i < 1000; i = i + 1) {
                if (i >= niter_i) { break; }
                randint = randint * 32747.0 + 12345.0;
                randint = randint - floor(randint / 65535.0) * 65535.0;
            }
            tiletype = randint - floor(randint * 0.5) * 2.0;
        }
    }

    var r0: f32;
    var r1: f32;
    if (extended == 0) {
        if (tiletype < 1.0) {
            r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
        } else {
            r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
        }
    } else {
        if (tiletype == 1.0) {
            r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
        } else {
            r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
        }
    }

    var ox = 0.0;
    var oy = 0.0;
    let cell_x = x + floor(p.x);
    let cell_y = y + floor(p.y);
    if (abs(r0 - 0.5) / safe_rmax < 1.0) {
        ox = ox + size * cell_x;
        oy = oy + size * cell_y;
    }
    if (abs(r1 - 0.5) / safe_rmax < 1.0) {
        ox = ox + size * cell_x;
        oy = oy + size * cell_y;
    }
    return vec2<f32>(ox * inv_w, oy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_truchet(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let extended = i32(get_param(xform_id, variation_id, 0u));
    let n = clamp(get_param(xform_id, variation_id, 1u), 0.001, 2.0);
    let width = clamp(get_param(xform_id, variation_id, 2u), 0.001, 1.0);
    let tdeg = get_param(xform_id, variation_id, 3u);
    let size = clamp(get_param(xform_id, variation_id, 4u), 0.001, 10.0);
    let seed_p = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let onen = 1.0 / n;
    let seed_abs = abs(seed_p);
    let seed2 = sqrt(seed_abs + seed_abs * 0.5 + 1e-30) / (seed_abs * 0.5 + 1e-30) * 0.25;
    let r0_init = -tdeg;
    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let scale = (cos(r0_init) - sin(r0_init)) * inv_w;

    let xs = p.x * scale;
    let ys = p.y * scale;
    let intx = round(xs);
    let inty = round(ys);
    let rx = xs - intx;
    let ry = ys - inty;
    let x = select(rx, 1.0 + rx, rx < 0.0);
    let y = select(ry, 1.0 + ry, ry < 0.0);

    var tiletype = 0.0;
    if (seed_abs == 1.0) {
        tiletype = 1.0;
    } else if (seed_abs > 0.0) {
        if (extended == 0) {
            let xrand = round(p.x) * seed2;
            let yrand = round(p.y) * seed2;
            let niter = xrand + yrand + xrand * yrand;
            var randint = (niter + seed_p) * seed2 * 0.5;
            randint = randint * 32747.0 + 12345.0;
            randint = randint - floor(randint / 65535.0) * 65535.0;
            tiletype = randint - floor(randint * 0.5) * 2.0;
        } else {
            let seed_floor = floor(seed_abs);
            let xrand = round(p.x);
            let yrand = round(p.y);
            var niter = abs(xrand + yrand + xrand * yrand);
            niter = min(niter, 1000.0);
            let niter_i = i32(niter);
            var randint = seed_floor + niter;
            for (var i = 0; i < 1000; i = i + 1) {
                if (i >= niter_i) { break; }
                randint = randint * 32747.0 + 12345.0;
                randint = randint - floor(randint / 65535.0) * 65535.0;
            }
            tiletype = randint - floor(randint * 0.5) * 2.0;
        }
    }

    var r0: f32;
    var r1: f32;
    if (extended == 0) {
        if (tiletype < 1.0) {
            r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
        } else {
            r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
        }
    } else {
        if (tiletype == 1.0) {
            r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
        } else {
            r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
        }
    }

    var ox = 0.0;
    var oy = 0.0;
    let cell_x = x + floor(p.x);
    let cell_y = y + floor(p.y);
    if (abs(r0 - 0.5) / safe_rmax < 1.0) {
        ox = ox + size * cell_x;
        oy = oy + size * cell_y;
    }
    if (abs(r1 - 0.5) / safe_rmax < 1.0) {
        ox = ox + size * cell_x;
        oy = oy + size * cell_y;
    }
    return vec3<f32>(ox * inv_w, oy * inv_w, p.z);
}
"#),
};
