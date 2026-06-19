//! `minkowskope` (oscilloscope from Apophysis plugin pack, dark-beam tweak 2017)
//!
//! Uses Minkowski's question-mark function as a "wave" for an
//! oscilloscope-style band test: if `|y| <= t` (where `t` is a
//! Minkowski-cosine wave of x), flip x/y signs; else pass through.
//!
//!   - 6 user params: separation, frequencyx, frequencyy,
//!                    amplitude, perturbation, damping
//!   - 2 init slots: tpf = 0.5·frequencyx, tpf2 = 0.5·frequencyy
//!     (`_noDamping` and `_altWave` flags computed at runtime)
//!
//! `minkowski(x)` is the standard SB-tree implementation (20
//! iterations, ~1e-6 precision); same shape as `minkQM`'s helper
//! but with the hardcoded a=b=c=d=1, e=0.5 constants.
//!
//! Output has VVAR factor cleanly — clean factor through outer.
//!
//! Source: `output/jwildfire-vars/output/minkowskope.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Minkowski-question-mark oscilloscope — like
/// `oscilloscope`/`oscilloscope2` but uses Minkowski's question-mark
/// function as the wave shape instead of a cosine. Builds a wave `t(x) =
/// amplitude · minkowski_cosine(0.5·freqx·x + perturbation ·
/// minkowski_sine(0.5·freqy·y)) [· exp(−|x|·damping)] + separation`, then
/// flips both X and Y signs when `|y| ≤ t(x)`. Negative `frequencyx`
/// enables an alternative wave variant where the Minkowski result is offset
/// by its input.
///
/// # Authors
/// - Apophysis Plugin Pack
/// - DarkBeam
pub static MINKOWSKOPE: VariationDef = VariationDef {
    name: "minkowskope",
    aliases: &[],
    display_name: "Minkowskope",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("separation", "Separation", unlimited_float, 0.5, -10.0, 10.0, "Vertical offset of the band threshold."),
        param!("frequencyx", "Frequency X", unlimited_float, 1.5, -10.0, 10.0, "X-axis Minkowski-cosine frequency (multiplied by 0.5 internally). Negative values enable the `alt_wave` variant."),
        param!("frequencyy", "Frequency Y", unlimited_float, 1.0, -10.0, 10.0, "Y-axis Minkowski-sine frequency for the perturbation term (multiplied by 0.5 internally)."),
        param!("amplitude", "Amplitude", unlimited_float, 0.5, -10.0, 10.0, "Wave amplitude."),
        param!("perturbation", "Perturbation", unlimited_float, 0.3, -10.0, 10.0, "Magnitude of the Y-driven X-perturbation."),
        param!("damping", "Damping", unlimited_float, 0.0, -10.0, 10.0, "Exponential damping rate. Near-zero disables the damping term."),
    ],
    // 2 derived values at slots 6..8: tpf, tpf2
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_minkowskope(user: array<f32, 6>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = 0.5 * user[1];
    out[1] = 0.5 * user[2];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn minkowskope_mink(xv: f32) -> f32 {
    var p: f32 = 0.0;
    var q: f32 = 1.0;
    var r: f32 = p + 1.0;
    var s: f32 = 1.0;
    var d: f32 = 1.0;
    var y: f32 = p;
    for (var it: i32 = 0; it < 20; it = it + 1) {
        d = d * 0.5;
        let m = p + r;
        let n = q + s;
        let safe_n = select(n, 1e-30, abs(n) < 1e-30);
        if (xv < (m / safe_n)) {
            r = m;
            s = n;
        } else {
            y = y + d;
            p = m;
            q = n;
        }
    }
    return y + d;
}

fn minkowskope_sine(xv: f32, alt_wave: bool) -> f32 {
    let lp = abs(xv) - floor(abs(xv) * 0.25) * 4.0;
    var p_val = abs(xv) - floor(abs(xv) * 0.5) * 2.0;
    if (p_val > 1.0) {
        p_val = 2.0 - p_val;
    }
    var mink: f32;
    if (alt_wave) {
        mink = minkowskope_mink(p_val) - p_val;
    } else {
        mink = minkowskope_mink(p_val);
    }
    let cond_xor = (lp < 2.0) != (xv > 0.0);
    return select(-mink, mink, cond_xor);
}

fn minkowskope_cosine(xv: f32, alt_wave: bool) -> f32 {
    return minkowskope_sine(xv - 1.0, alt_wave);
}

fn variation_minkowskope(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let separation = get_param(xform_id, variation_id, 0u);
    let frequencyx = get_param(xform_id, variation_id, 1u);
    let amplitude = get_param(xform_id, variation_id, 3u);
    let perturbation = get_param(xform_id, variation_id, 4u);
    let damping = get_param(xform_id, variation_id, 5u);
    let tpf = get_param(xform_id, variation_id, 6u);
    let tpf2 = get_param(xform_id, variation_id, 7u);
    let no_damping = abs(damping) <= 1e-9;
    let alt_wave = frequencyx <= 0.0;

    let pt = perturbation * minkowskope_sine(tpf2 * p.y, alt_wave);
    var t: f32;
    if (no_damping) {
        t = amplitude * minkowskope_cosine(tpf * p.x + pt, alt_wave) + separation;
    } else {
        t = amplitude * exp(-abs(p.x) * damping) * minkowskope_cosine(tpf * p.x + pt, alt_wave) + separation;
    }
    let inside = abs(p.y) <= t;
    let s = select(1.0, -1.0, inside);
    return vec2<f32>(s * p.x, s * p.y);
}
"#,
    wgsl_3d: r#"
fn minkowskope_mink(xv: f32) -> f32 {
    var p: f32 = 0.0;
    var q: f32 = 1.0;
    var r: f32 = p + 1.0;
    var s: f32 = 1.0;
    var d: f32 = 1.0;
    var y: f32 = p;
    for (var it: i32 = 0; it < 20; it = it + 1) {
        d = d * 0.5;
        let m = p + r;
        let n = q + s;
        let safe_n = select(n, 1e-30, abs(n) < 1e-30);
        if (xv < (m / safe_n)) {
            r = m;
            s = n;
        } else {
            y = y + d;
            p = m;
            q = n;
        }
    }
    return y + d;
}

fn minkowskope_sine(xv: f32, alt_wave: bool) -> f32 {
    let lp = abs(xv) - floor(abs(xv) * 0.25) * 4.0;
    var p_val = abs(xv) - floor(abs(xv) * 0.5) * 2.0;
    if (p_val > 1.0) {
        p_val = 2.0 - p_val;
    }
    var mink: f32;
    if (alt_wave) {
        mink = minkowskope_mink(p_val) - p_val;
    } else {
        mink = minkowskope_mink(p_val);
    }
    let cond_xor = (lp < 2.0) != (xv > 0.0);
    return select(-mink, mink, cond_xor);
}

fn minkowskope_cosine(xv: f32, alt_wave: bool) -> f32 {
    return minkowskope_sine(xv - 1.0, alt_wave);
}

fn variation_minkowskope(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let separation = get_param(xform_id, variation_id, 0u);
    let frequencyx = get_param(xform_id, variation_id, 1u);
    let amplitude = get_param(xform_id, variation_id, 3u);
    let perturbation = get_param(xform_id, variation_id, 4u);
    let damping = get_param(xform_id, variation_id, 5u);
    let tpf = get_param(xform_id, variation_id, 6u);
    let tpf2 = get_param(xform_id, variation_id, 7u);
    let no_damping = abs(damping) <= 1e-9;
    let alt_wave = frequencyx <= 0.0;

    let pt = perturbation * minkowskope_sine(tpf2 * p.y, alt_wave);
    var t: f32;
    if (no_damping) {
        t = amplitude * minkowskope_cosine(tpf * p.x + pt, alt_wave) + separation;
    } else {
        t = amplitude * exp(-abs(p.x) * damping) * minkowskope_cosine(tpf * p.x + pt, alt_wave) + separation;
    }
    let inside = abs(p.y) <= t;
    let s = select(1.0, -1.0, inside);
    return vec3<f32>(s * p.x, s * p.y, p.z);
}
"#,
};
