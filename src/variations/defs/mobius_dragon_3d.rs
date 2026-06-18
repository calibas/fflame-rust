//! mobius_dragon_3D (Whittaker Courtney) — 3D Möbius/reciprocal feedback
//! with a log-tile spread, by way of Zy0rg's Log_tile (via Brad Stefanov).
//!
//! Iterates a Möbius-style map up to `iterations` times: divide the input
//! by the complex constant `(re, im)`, take a 3D reciprocal (scaled by the
//! variation weight), and feed the result back as the next iteration's
//! input. After the loop a Log_tile step scatters the point by
//! `spread{x,y,z} · round(log(rand)/log(log_spread))` with a random
//! sign/offset branch, and an optional `tanh`-of-magnitude term tints the
//! colour.
//!
//! Replace-style: JWF assigns `pVarTP = (z.re, z.im, …)` with the weight
//! baked into the formula, so the body reads its own weight and divides
//! the result by it (idisc pattern) for the dispatcher's outer `w·` to
//! cancel. The captured `random` gates both the per-iteration line branch
//! and the final sign branch (one draw, reused — a JWF quirk).
//!
//! Faithful-port notes:
//!   - `line_color_shift` is **dead in JWildfire**: the loop does
//!     `pVarTP.color += line_color_shift`, but `pVarTP.color` is then
//!     overwritten by the pre-loop snapshot `zc` (+ magnitude term). The
//!     param is kept for `.flame` round-trip but has no colour effect, here
//!     or in JWF. (Confirmed in both the Java and the CUDA `getGPUCode`.)
//!   - Z: the log-tile writes z unconditionally (`Feature::AlwaysZ`). JWF
//!     additionally appends the standard `if preserve_z: z += w·affine.z`
//!     passthrough; we can't read preserve_z at runtime (it's a build-time
//!     constant driving the gated-z codegen), so we emit only the log-tile
//!     z. Exact for `preserve_z = false` (the default); under
//!     `preserve_z = true` the `+w·z` passthrough term is omitted.
//!
//! Source:
//!   - `output/variation-jwf-source/MobiusDragon3DFunc.java`
//!
//! # Authors
//! - Whittaker Courtney
//! - Zy0rg (Log_tile), Brad Stefanov (JWildfire port)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 3D Möbius/reciprocal dragon with a Log_tile spread — divides by a complex
/// constant, takes a weighted 3D reciprocal, feeds back for `iterations`,
/// then scatters via `spread·round(log(rand)/log(log_spread))` with a random
/// sign branch. Optional magnitude-based colour tint.
///
/// # Authors
/// - Whittaker Courtney (Zy0rg's Log_tile)
pub static MOBIUS_DRAGON_3D: VariationDef = VariationDef {
    name: "mobius_dragon_3D",
    aliases: &[],
    display_name: "Mobius Dragon 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::WritesColor, Feature::AlwaysZ],
    parameters: &[
        param!("re", "Re", unlimited_float, 1.0, -10.0, 10.0, "Real part of the complex divisor `(re, im)` applied each iteration."),
        param!("im", "Im", unlimited_float, 0.0, -10.0, 10.0, "Imaginary part of the complex divisor."),
        param!("x_spread", "X Spread", unlimited_float, 1.0, -10.0, 10.0, "Log-tile scatter magnitude on X (sign randomized per iteration)."),
        param!("y_spread", "Y Spread", unlimited_float, 0.0, -10.0, 10.0, "Log-tile scatter magnitude on Y."),
        param!("z_spread", "Z Spread", unlimited_float, 0.0, -10.0, 10.0, "Log-tile scatter magnitude on Z."),
        param!("x_add", "X Add", unlimited_float, 0.0, -10.0, 10.0, "Constant X offset added in the positive log-tile branch."),
        param!("y_add", "Y Add", unlimited_float, 0.0, -10.0, 10.0, "Constant Y offset added in the positive log-tile branch."),
        param!("log_spread", "Log Spread", unlimited_float, 2.71828, 1.01, 100.0, "Log base for the tile index `round(log(rand)/log(log_spread))`. Default e."),
        param!("line_enable", "Line Enable", bool, true, "When on, with probability `line_weight` the iteration collapses to a horizontal line segment (`z.re = rand·spreadx`, z.im = 0)."),
        param!("line_weight", "Line Weight", unlimited_float, 0.125, 0.0, 1.0, "Probability that the line branch fires (compared against the captured per-call random, so it's all-or-nothing across the loop)."),
        param!("line_color_shift", "Line Color Shift", unlimited_float, 0.1, -10.0, 10.0, "Dead in JWildfire — the loop's `color += line_color_shift` is overwritten by the pre-loop colour snapshot. Kept for round-trip; no effect."),
        param!("mag_color", "Mag Color", bool, true, "When on, adds `tanh(|output|·mag_color_scale/6)` to the colour register, tinting by point magnitude."),
        param!("mag_color_scale", "Mag Color Scale", unlimited_float, 0.5, -10.0, 10.0, "Scales the magnitude before the `tanh` colour term."),
        param!("iterations", "Iterations", int, 1.0, 0.0, 24.0, "Number of Möbius feedback iterations (0-24). Each divides by `(re, im)`, takes the weighted 3D reciprocal, and feeds back."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_mobius_dragon_3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let re = get_param(xform_id, variation_id, 0u);
    let im = get_param(xform_id, variation_id, 1u);
    let x_spread = get_param(xform_id, variation_id, 2u);
    let y_spread = get_param(xform_id, variation_id, 3u);
    let x_add = get_param(xform_id, variation_id, 5u);
    let y_add = get_param(xform_id, variation_id, 6u);
    let log_spread = get_param(xform_id, variation_id, 7u);
    let line_enable = i32(get_param(xform_id, variation_id, 8u));
    let line_weight = get_param(xform_id, variation_id, 9u);
    let mag_color = i32(get_param(xform_id, variation_id, 11u));
    let mag_color_scale = get_param(xform_id, variation_id, 12u);
    let iterations = clamp(i32(get_param(xform_id, variation_id, 13u)), 0, 24);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let random = rng_nextf(rng);
    let z1 = 0.0;                       // 2D: no input Z
    var aff_x = p.x;
    var aff_y = p.y;
    var spreadx = -x_spread;
    var spready = -y_spread;
    var zre = p.x;
    var zim = p.y + 1.0;
    var pz = 0.0;

    let d2 = re * re + im * im;
    let safe_d2 = select(d2, 1e-30, abs(d2) < 1e-30);
    let safe_log_ls = select(log(log_spread), 1e-30, abs(log(log_spread)) < 1e-30);

    for (var i = 0; i < 24; i = i + 1) {
        if (i >= iterations) { break; }
        zre = aff_x;
        zim = aff_y + 1.0;
        let nre = (zre * re + zim * im) / safe_d2;   // complex div by (re, im)
        let nim = (zim * re - zre * im) / safe_d2;
        let denom = nre * nre + nim * nim + z1 * z1;
        let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
        let r1 = w / safe_denom;
        zre = nre * r1;
        zim = -nim * r1;
        pz = -z1 * r1;
        zre = zre * w;
        zim = zim * w;
        aff_x = zre;
        aff_y = zim + 1.0;
        if (line_enable == 1 && random < line_weight) {
            zre = rng_nextf(rng) * spreadx;
            zim = 0.0;
            pz = 0.0;
        }
    }

    if (rng_nextf(rng) < 0.5) { spreadx = x_spread; }
    if (rng_nextf(rng) < 0.5) { spready = y_spread; }
    let _zskip = rng_nextf(rng);        // z spread coin (no Z in 2D), stream parity

    if (random < 0.5) {
        zre = w * x_add + (zre + spreadx * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls));
        zim = w * (y_add + (zim + spready * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls))) + 1.0;
        let _z = rng_nextf(rng);
    } else {
        zre = w * (-zre + spreadx * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls));
        zim = w * (-2.0 - (zim + spready * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls))) + 1.0;
        let _z = rng_nextf(rng);
    }

    if (mag_color == 1) {
        let mag = sqrt(zre * zre + zim * zim + z1 * z1) * (mag_color_scale / 6.0);
        *vc = *vc + tanh(mag);
    }

    return vec2<f32>(zre * inv_w, zim * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_mobius_dragon_3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let re = get_param(xform_id, variation_id, 0u);
    let im = get_param(xform_id, variation_id, 1u);
    let x_spread = get_param(xform_id, variation_id, 2u);
    let y_spread = get_param(xform_id, variation_id, 3u);
    let z_spread = get_param(xform_id, variation_id, 4u);
    let x_add = get_param(xform_id, variation_id, 5u);
    let y_add = get_param(xform_id, variation_id, 6u);
    let log_spread = get_param(xform_id, variation_id, 7u);
    let line_enable = i32(get_param(xform_id, variation_id, 8u));
    let line_weight = get_param(xform_id, variation_id, 9u);
    let mag_color = i32(get_param(xform_id, variation_id, 11u));
    let mag_color_scale = get_param(xform_id, variation_id, 12u);
    let iterations = clamp(i32(get_param(xform_id, variation_id, 13u)), 0, 24);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let random = rng_nextf(rng);
    let z1 = p.z;                       // original affine Z, never mutated in the loop
    var aff_x = p.x;
    var aff_y = p.y;
    var spreadx = -x_spread;
    var spready = -y_spread;
    var spreadz = -z_spread;
    var zre = p.x;
    var zim = p.y + 1.0;
    var pz = 0.0;

    let d2 = re * re + im * im;
    let safe_d2 = select(d2, 1e-30, abs(d2) < 1e-30);
    let safe_log_ls = select(log(log_spread), 1e-30, abs(log(log_spread)) < 1e-30);

    for (var i = 0; i < 24; i = i + 1) {
        if (i >= iterations) { break; }
        zre = aff_x;
        zim = aff_y + 1.0;
        let nre = (zre * re + zim * im) / safe_d2;   // complex div by (re, im)
        let nim = (zim * re - zre * im) / safe_d2;
        let denom = nre * nre + nim * nim + z1 * z1;
        let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
        let r1 = w / safe_denom;
        zre = nre * r1;
        zim = -nim * r1;
        pz = -z1 * r1;
        zre = zre * w;
        zim = zim * w;
        aff_x = zre;
        aff_y = zim + 1.0;
        if (line_enable == 1 && random < line_weight) {
            zre = rng_nextf(rng) * spreadx;
            zim = 0.0;
            pz = 0.0;
        }
    }

    if (rng_nextf(rng) < 0.5) { spreadx = x_spread; }
    if (rng_nextf(rng) < 0.5) { spready = y_spread; }
    if (rng_nextf(rng) < 0.5) { spreadz = z_spread; }

    if (random < 0.5) {
        zre = w * x_add + (zre + spreadx * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls));
        zim = w * (y_add + (zim + spready * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls))) + 1.0;
        pz = w * (pz + spreadz * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls));
    } else {
        zre = w * (-zre + spreadx * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls));
        zim = w * (-2.0 - (zim + spready * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls))) + 1.0;
        pz = w * (-pz + spreadz * round(log(max(rng_nextf(rng), 1e-30)) / safe_log_ls));
    }

    if (mag_color == 1) {
        let mag = sqrt(zre * zre + zim * zim + z1 * z1) * (mag_color_scale / 6.0);
        *vc = *vc + tanh(mag);
    }

    // Replace-style assign with idisc divide (outer w· cancels). z is the
    // log-tile pz (AlwaysZ); the preserve_z passthrough term is omitted.
    return vec3<f32>(zre * inv_w, zim * inv_w, pz * inv_w);
}
"#,
};
