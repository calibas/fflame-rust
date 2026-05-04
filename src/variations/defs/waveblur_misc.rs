//! waveblur_wf (Maschke)
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
//!   - color_scale, color_offset (skipped per writes_color compromise)
//!   - direct_color (int, color writes skipped)
//!
//! No init slots. Body factors cleanly through outer multiplier.
//! RNG (4 calls/iter for ang, rnd, branch select, count). Full3D —
//! z component depends on `cos(rnd)` and optional damping factor
//! `exp(r · damping_z)`.
//!
//! Source: `output/jwildfire-vars/output/waveblur_wf.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static WAVEBLUR_WF: VariationDef = VariationDef {
    name: "waveblur_wf",
    display_name: "Wave Blur WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("count", "Count", int, 5.0, 1.0, 100.0),
        param!("amplitude_z", "Amplitude Z", unlimited_float, 0.5, -10.0, 10.0),
        param!("phase", "Phase", unlimited_float, 0.0, -10.0, 10.0),
        param!("damping_z", "Damping Z", unlimited_float, 0.0, 0.0, 10.0),
        param!("color_scale", "Color Scale", unlimited_float, 0.5, -10.0, 10.0),
        param!("color_offset", "Color Offset", unlimited_float, 0.0, -10.0, 10.0),
        param!("direct_color", "Direct Color", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_waveblur_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let count_p = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let phase = get_param(xform_id, variation_id, 2u);
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
    return vec2<f32>(r * cos(ang), r * sin(ang));
}
"#,
    wgsl_3d: Some(r#"
fn variation_waveblur_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let count_p = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let amplitude_z = get_param(xform_id, variation_id, 1u);
    let phase = get_param(xform_id, variation_id, 2u);
    let damping_z = get_param(xform_id, variation_id, 3u);
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
"#),
};
