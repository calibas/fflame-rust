//! waveblur_wf
//!
//! Polar wave-blur — emits points on a circle whose radius is a wave
//! function of a uniform-random angle. The wave function uses
//! `acos(rnd)` (or `acos(-rnd)`, picked 50/50) shifted by a random
//! integer multiple of π and the user phase, scaled to [-VVAR/π, VVAR/π].
//!
//! 7 user params:
//!   - count (int, default 5; max integer multiplier of π)
//!   - amplitude_z (default 0.5)
//!   - phase
//!   - damping_z (default 0; clamped to ≥0 in cpp setter — preserved)
//!   - color_scale, color_offset (drive the color write when
//!     `direct_color` is on)
//!   - direct_color (int): when on, writes `TC = clamp(acos(rnd) ·
//!     color_scale + color_offset, 0, 1)` — uses the original signed
//!     `rnd` (not the `acos(-rnd)` branch used for the radius).
//!
//! No init slots. Body factors cleanly through outer multiplier.
//! RNG (4 calls/iter for ang, rnd, branch select, count). Full3D —
//! z component depends on `cos(rnd)` and optional damping factor
//! `exp(r · damping_z)`.
//!
//! Source: `output/jwildfire-vars/output/waveblur_wf.cpp`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Polar wave-blur — emits points on a circle whose radius is a wave
/// function `(acos(±rnd) − (n+1)·π + phase) / (count·π)` of a uniform-
/// random `rnd ∈ [-1, 1]`, with a random integer `n ∈ [0, count)`. The
/// angular position is uniform. 3D mode adds a Z component `(cos(rnd) −
/// 0.5) · amplitude_z`, optionally exponentially damped by `damping_z`.
pub static WAVEBLUR_WF: VariationDef = VariationDef {
    name: "waveblur_wf",
    aliases: &[],
    display_name: "Wave Blur WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::WritesColor, Feature::AlwaysZ],
    parameters: &[
        param!("count", "Count", int, 5.0, 1.0, 100.0, "Maximum integer multiplier of π in the wave-radius formula (≥ 1)."),
        param!("amplitude_z", "Amplitude Z", unlimited_float, 0.5, -10.0, 10.0, "Z-axis wave amplitude (3D only). 0 zeroes the Z output."),
        param!("phase", "Phase", unlimited_float, 0.0, -10.0, 10.0, "Phase offset added to the wave-radius numerator."),
        param!("damping_z", "Damping Z", unlimited_float, 0.0, 0.0, 10.0, "Exponential damping rate on Z (clamped ≥ 0 in upstream). 0 disables damping."),
        param!("color_scale", "Color Scale", unlimited_float, 0.5, -10.0, 10.0, "Color register multiplier in `clamp(acos(rnd) · color_scale + color_offset, 0, 1)`. Active when `direct_color` is on."),
        param!("color_offset", "Color Offset", unlimited_float, 0.0, -10.0, 10.0, "Color register offset added after the `acos(rnd) · color_scale` term."),
        param!("direct_color", "Direct Color", bool, false, "When on, writes the color register from `acos(rnd)` scaled by `color_scale` and shifted by `color_offset`, clamped to `[0, 1]`. Visible color requires the transform's Direct Color slider > 0."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_waveblur_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let count_p = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let phase = get_param(xform_id, variation_id, 2u);
    let color_scale = get_param(xform_id, variation_id, 4u);
    let color_offset = get_param(xform_id, variation_id, 5u);
    let direct_color = i32(get_param(xform_id, variation_id, 6u));
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let ang = rng_nextf(rng) * two_pi;
    let rnd = 1.0 - 2.0 * rng_nextf(rng);
    let n = i32(rng_nextf(rng) * f32(count_p));
    var r: f32;
    if (rng_nextf(rng) < 0.5) {
        r = acos(rnd) - f32(n + 1) * pi + phase;
    } else {
        r = acos(-rnd) - f32(n + 1) * pi + phase;
    }
    let scale = 1.0 / (f32(count_p) * pi);
    r = r * scale;

    if (direct_color != 0) {
        *vc = clamp(acos(rnd) * color_scale + color_offset, 0.0, 1.0);
    }

    return vec2<f32>(r * cos(ang), r * sin(ang));
}
"#,
    wgsl_3d: r#"
fn variation_waveblur_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let count_p = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let amplitude_z = get_param(xform_id, variation_id, 1u);
    let phase = get_param(xform_id, variation_id, 2u);
    let damping_z = get_param(xform_id, variation_id, 3u);
    let color_scale = get_param(xform_id, variation_id, 4u);
    let color_offset = get_param(xform_id, variation_id, 5u);
    let direct_color = i32(get_param(xform_id, variation_id, 6u));
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let ang = rng_nextf(rng) * two_pi;
    let rnd = 1.0 - 2.0 * rng_nextf(rng);
    let n = i32(rng_nextf(rng) * f32(count_p));
    var r: f32;
    if (rng_nextf(rng) < 0.5) {
        r = acos(rnd) - f32(n + 1) * pi + phase;
    } else {
        r = acos(-rnd) - f32(n + 1) * pi + phase;
    }
    let scale = 1.0 / (f32(count_p) * pi);
    r = r * scale;

    if (direct_color != 0) {
        *vc = clamp(acos(rnd) * color_scale + color_offset, 0.0, 1.0);
    }

    var nz = 0.0;
    if (abs(amplitude_z) > 1e-30) {
        var zamp = (cos(rnd) - 0.5) * amplitude_z;
        if (abs(damping_z) > 1e-6) {
            let dmp = r * w * damping_z;
            zamp = zamp * exp(dmp);
        }
        nz = zamp;
    }
    return vec3<f32>(r * cos(ang), r * sin(ang), nz);
}
"#,
};
