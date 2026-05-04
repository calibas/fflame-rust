//! truchet2 (tatasz, ported by Brad Stefanov / Jesus Sosa)
//!
//! Truchet2 — variant of truchet (TyrantWave) with two interpolated
//! exponents/widths controlled by `xp` (the position within the cell
//! along the x-axis, mapped to [0, 1] via fmod-and-fold).
//!
//! 7 user params:
//!   - exponent1, exponent2  (clamped to [-1, 3] — capped at 2 in body)
//!   - width1, width2        (clamped to [-1, 2] — capped at 1 in body)
//!   - scale, seed
//!   - inverse (int 0/1)
//!
//! No init slots. Body factors cleanly through outer multiplier (cpp
//! uses VVAR/pAmount on every output line). Fast tile-type selection
//! uses the same multiplicative-LCG hash as the original truchet
//! (`(niter + seed) · seed2 / 2`, then `· 32747 + 12345 mod 65535`).
//!
//! cpp's "fill" sentinels (100.0 / 10000.0 to push points off-screen)
//! are preserved to match upstream's flicker-suppression behavior.
//!
//! Source: `output/jwildfire-vars/output/truchet2.cpp` (Java-recovered;
//! cpp PluginVarCalc was empty unported_stub).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static TRUCHET2: VariationDef = VariationDef {
    name: "truchet2",
    display_name: "Truchet 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("exponent1", "Exponent 1", unlimited_float, 1.0, -1.0, 3.0),
        param!("exponent2", "Exponent 2", unlimited_float, 2.0, -1.0, 3.0),
        param!("width1", "Width 1", unlimited_float, 0.5, -1.0, 2.0),
        param!("width2", "Width 2", unlimited_float, 0.5, -1.0, 2.0),
        param!("scale", "Scale", unlimited_float, 10.0, -100.0, 100.0),
        param!("seed", "Seed", unlimited_float, 50.0, -1000.0, 1000.0),
        param!("inverse", "Inverse", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_truchet2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let exp1 = get_param(xform_id, variation_id, 0u);
    let exp2 = get_param(xform_id, variation_id, 1u);
    let w1 = get_param(xform_id, variation_id, 2u);
    let w2 = get_param(xform_id, variation_id, 3u);
    let scale = get_param(xform_id, variation_id, 4u);
    let seed_p = get_param(xform_id, variation_id, 5u);
    let inverse = i32(get_param(xform_id, variation_id, 6u));

    let safe_scale = select(scale, 1e-30, abs(scale) < 1e-30);
    let xs = p.x / safe_scale;
    let xp_in = abs((xs - floor(xs)) - 0.5) * 2.0;
    var width = w1 * (1.0 - xp_in) + xp_in * w2;
    width = min(width, 1.0);
    if (width <= 0.0) {
        return vec2<f32>(p.x, p.y);
    }

    let xp2 = exp1 * (1.0 - xp_in) + xp_in * exp2;
    var n = xp2;
    n = min(n, 2.0);
    if (n <= 0.0) {
        return vec2<f32>(p.x, p.y);
    }

    let onen = 1.0 / xp2;
    let seed_abs = abs(seed_p);
    let seed2 = sqrt(seed_abs + seed_abs * 0.5 + 1e-30) / (seed_abs * 0.5 + 1e-30) * 0.25;

    let intx = round(p.x);
    let inty = round(p.y);
    var x = p.x - intx;
    if (x < 0.0) { x = 1.0 + x; }
    var y = p.y - inty;
    if (y < 0.0) { y = 1.0 + y; }

    var tiletype = 0.0;
    if (seed_abs == 1.0) {
        tiletype = 1.0;
    } else if (seed_abs > 0.0) {
        let xrand = round(p.x) * seed2;
        let yrand = round(p.y) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        var randint = (niter + seed_p) * seed2 * 0.5;
        randint = randint - floor(randint / 65535.0) * 65535.0;
        randint = randint * 32747.0 + 12345.0;
        randint = randint - floor(randint / 65535.0) * 65535.0;
        tiletype = randint - floor(randint * 0.5) * 2.0;
    }

    var r0: f32;
    var r1: f32;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
    }

    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let r00 = abs(r0 - 0.5) / safe_rmax;
    let r11 = abs(r1 - 0.5) / safe_rmax;

    if (inverse == 0) {
        if (r00 < 1.0 || r11 < 1.0) {
            return vec2<f32>(x + floor(p.x), y + floor(p.y));
        }
        return vec2<f32>(100.0, 100.0);
    }
    if (r00 > 1.0 && r11 > 1.0) {
        return vec2<f32>(x + floor(p.x), y + floor(p.y));
    }
    return vec2<f32>(10000.0, 10000.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_truchet2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let exp1 = get_param(xform_id, variation_id, 0u);
    let exp2 = get_param(xform_id, variation_id, 1u);
    let w1 = get_param(xform_id, variation_id, 2u);
    let w2 = get_param(xform_id, variation_id, 3u);
    let scale = get_param(xform_id, variation_id, 4u);
    let seed_p = get_param(xform_id, variation_id, 5u);
    let inverse = i32(get_param(xform_id, variation_id, 6u));

    let safe_scale = select(scale, 1e-30, abs(scale) < 1e-30);
    let xs = p.x / safe_scale;
    let xp_in = abs((xs - floor(xs)) - 0.5) * 2.0;
    var width = w1 * (1.0 - xp_in) + xp_in * w2;
    width = min(width, 1.0);
    if (width <= 0.0) {
        return vec3<f32>(p.x, p.y, p.z);
    }

    let xp2 = exp1 * (1.0 - xp_in) + xp_in * exp2;
    var n = xp2;
    n = min(n, 2.0);
    if (n <= 0.0) {
        return vec3<f32>(p.x, p.y, p.z);
    }

    let onen = 1.0 / xp2;
    let seed_abs = abs(seed_p);
    let seed2 = sqrt(seed_abs + seed_abs * 0.5 + 1e-30) / (seed_abs * 0.5 + 1e-30) * 0.25;

    let intx = round(p.x);
    let inty = round(p.y);
    var x = p.x - intx;
    if (x < 0.0) { x = 1.0 + x; }
    var y = p.y - inty;
    if (y < 0.0) { y = 1.0 + y; }

    var tiletype = 0.0;
    if (seed_abs == 1.0) {
        tiletype = 1.0;
    } else if (seed_abs > 0.0) {
        let xrand = round(p.x) * seed2;
        let yrand = round(p.y) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        var randint = (niter + seed_p) * seed2 * 0.5;
        randint = randint - floor(randint / 65535.0) * 65535.0;
        randint = randint * 32747.0 + 12345.0;
        randint = randint - floor(randint / 65535.0) * 65535.0;
        tiletype = randint - floor(randint * 0.5) * 2.0;
    }

    var r0: f32;
    var r1: f32;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
    }

    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let r00 = abs(r0 - 0.5) / safe_rmax;
    let r11 = abs(r1 - 0.5) / safe_rmax;

    if (inverse == 0) {
        if (r00 < 1.0 || r11 < 1.0) {
            return vec3<f32>(x + floor(p.x), y + floor(p.y), p.z);
        }
        return vec3<f32>(100.0, 100.0, p.z);
    }
    if (r00 > 1.0 && r11 > 1.0) {
        return vec3<f32>(x + floor(p.x), y + floor(p.y), p.z);
    }
    return vec3<f32>(10000.0, 10000.0, p.z);
}
"#),
};
