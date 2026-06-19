//! dc_mandelbox2D (Jesus Sosa) — 2D Mandelbox escape coloring (DC_BaseFunc).
//!
//! Samples a point (random in [-0.5, 0.5], or the input point when
//! `ColorOnly`), runs it through a 20-iteration Mandelbox escape map at
//! `zoom`, and turns the escape value into an RGB colour. Shares the exact
//! iteration with our `glsl_mandelbox2D`, but is a distinct JWildfire
//! variation with its own sampling (zoom + ColorOnly), output (the sample
//! position), and the `DC_BaseFunc` colour/Z model — so JWF flames that
//! name `dc_mandelbox2D` import it correctly.
//!
//! Colour follows the Java/CPU path (JWildfire's default renderer):
//!   I = zoom · sample; 20× boxfold/ballfold/scale; d = pow(|I|/d, 0.1)·5;
//!   rgb = (cos d, sin(10d+1), cos(3d+1)) · 0.5 + 0.5
//!
//! **`gradient` modes** — our plot path applies the direct-RGB register
//! (`vrc`) per *transform*, not per *point*, and there's no per-point
//! RGB-vs-palette flag, so only mode 0 is honored:
//!   - 0 (direct RGB) — written to `vrc`. The default and primary look.
//!   - 1 (nearest palette colour) — needs a palette-texture read in the
//!     variation; falls back to direct RGB.
//!   - 2 (greyscale → palette index) — would need per-flame WGSL
//!     specialization to switch the whole flame to the palette path;
//!     falls back to direct RGB.
//! `gradient` round-trips regardless. Faithfully supporting 2 is tracked
//! as a framework follow-up (per-flame colour-mode specialization).
//!
//! Z model (`DC_BaseFunc`): `dz = greyscale(rgb)·scale_z + offset_z`, then
//! `reset_z` replaces z with `dz` (default) or adds it. The default
//! (`scale_z=0, offset_z=0`) gives z = 0. Applied via an idisc-style
//! divide so the dispatcher's outer `w·` cancels the unweighted dz (xy
//! stay weight-scaled, matching JWF). reset-vs-add collapse to the same
//! contribution in our summed model (they differ only with multiple
//! z-writers on one xform).
//!
//! Sources:
//!   - `output/variation-jwf-source/DC_MandelBox2DFunc.java`
//!   - `output/variation-jwf-source/DC_BaseFunc.java`
//!
//! # Authors
//! - Jesus Sosa

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 2D Mandelbox escape colouring — runs a sampled point through a 20-iter
/// Mandelbox at `zoom` and writes the escape-derived RGB to the colour
/// register. `ColorOnly` samples the input point instead of a random one;
/// the `DC_BaseFunc` `scale_z`/`offset_z`/`reset_z` drive an optional Z
/// output.
///
/// # Authors
/// - Jesus Sosa
pub static DC_MANDELBOX2D: VariationDef = VariationDef {
    name: "dc_mandelbox2D",
    aliases: &[],
    display_name: "DC MandelBox2D",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::WritesRgb, Feature::AlwaysZ],
    parameters: &[
        param!("zoom", "Zoom", unlimited_float, 7.0, 0.1, 50.0, "Scale applied to the sample before the Mandelbox iteration. Higher zoom shows finer escape structure."),
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 10000.0, "JWildfire CPU-only: seeds a Java Random and assigns `time` as a side effect. Not honored here — set `time` directly. Accepted for round-trip."),
        param!("time", "Time", unlimited_float, 0.0, -10000.0, 10000.0, "Animation parameter, embedded as `O.z = -5·cos(time·0.1)` in the iteration. Animate for a morphing pattern."),
        param!("ColorOnly", "Color Only", bool, false, "When on, samples the input point (colours it by the Mandelbox at that position) instead of scattering a random sample."),
        param!("Gradient", "Gradient", enum, 0, &["Direct RGB", "Nearest Palette", "Greyscale Index"], "Colour output mode. Only mode 0 (direct RGB) is honored; modes 1/2 fall back to RGB (framework limitation — see module doc). Round-trips regardless."),
        param!("scale_z", "Scale Z", unlimited_float, 0.0, -10.0, 10.0, "Z = greyscale(rgb)·scale_z + offset_z. 0 (default) gives a flat z = offset_z."),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0, "Constant added to the Z output."),
        param!("reset_z", "Reset Z", bool, true, "When on (default), the variation's Z replaces the running z with `dz`; when off, it adds `dz`."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn dc_mbox2d_rgb(i_in: vec2<f32>, time: f32) -> vec3<f32> {
    var I = i_in;
    var O = vec4<f32>(I.x, I.y, -5.0 * cos(time * 0.1), 1.0);
    var d: f32 = 1.0;
    for (var k: u32 = 0u; k < 20u; k = k + 1u) {
        I = clamp(I, vec2<f32>(-1.0), vec2<f32>(1.0)) * 2.0 - I;   // boxfold
        O.w = length(I);
        var b: f32;
        if (O.w < 0.5) { b = 4.0; }                                // ballfold
        else if (O.w < 1.0) { b = 1.0 / O.w; }
        else { b = 1.0; }
        I = I * (O.z * b) + vec2<f32>(O.x, O.y);                    // scaling
        d = b * d * abs(O.z) + 1.0;                                 // bound DE
    }
    let df = pow(length(I) / d, 0.1) * 5.0;
    return vec3<f32>(cos(df), sin(10.0 * df + 1.0), cos(3.0 * df + 1.0)) * 0.5 + 0.5;
}
fn variation_dc_mandelbox2D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    let zoom = get_param(xform_id, variation_id, 0u);
    let time = get_param(xform_id, variation_id, 2u);
    let color_only = i32(get_param(xform_id, variation_id, 3u));
    var uv: vec2<f32>;
    if (color_only == 1) {
        uv = p;
    } else {
        uv = vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5);
    }
    // All gradient modes write the direct RGB (modes 1/2 fall back — see
    // module doc).
    *vrc = dc_mbox2d_rgb(uv * zoom, time);
    return uv;
}
"#,
    wgsl_3d: r#"
fn dc_mbox2d_rgb(i_in: vec2<f32>, time: f32) -> vec3<f32> {
    var I = i_in;
    var O = vec4<f32>(I.x, I.y, -5.0 * cos(time * 0.1), 1.0);
    var d: f32 = 1.0;
    for (var k: u32 = 0u; k < 20u; k = k + 1u) {
        I = clamp(I, vec2<f32>(-1.0), vec2<f32>(1.0)) * 2.0 - I;
        O.w = length(I);
        var b: f32;
        if (O.w < 0.5) { b = 4.0; }
        else if (O.w < 1.0) { b = 1.0 / O.w; }
        else { b = 1.0; }
        I = I * (O.z * b) + vec2<f32>(O.x, O.y);
        d = b * d * abs(O.z) + 1.0;
    }
    let df = pow(length(I) / d, 0.1) * 5.0;
    return vec3<f32>(cos(df), sin(10.0 * df + 1.0), cos(3.0 * df + 1.0)) * 0.5 + 0.5;
}
// Greyscale (JWF): floored 0-255 channels, luminance, /255.
fn dc_mbox2d_grey(color: vec3<f32>) -> f32 {
    let ri = clamp(floor(color.r * 256.0), 0.0, 255.0);
    let gi = clamp(floor(color.g * 256.0), 0.0, 255.0);
    let bi = clamp(floor(color.b * 256.0), 0.0, 255.0);
    return (floor(ri * 0.299) + floor(gi * 0.587) + floor(bi * 0.114)) / 255.0;
}
fn variation_dc_mandelbox2D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let zoom = get_param(xform_id, variation_id, 0u);
    let time = get_param(xform_id, variation_id, 2u);
    let color_only = i32(get_param(xform_id, variation_id, 3u));
    let scale_z = get_param(xform_id, variation_id, 5u);
    let offset_z = get_param(xform_id, variation_id, 6u);
    var uv: vec2<f32>;
    if (color_only == 1) {
        uv = vec2<f32>(p.x, p.y);
    } else {
        uv = vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5);
    }
    let color = dc_mbox2d_rgb(uv * zoom, time);
    *vrc = color;

    // DC_BaseFunc Z: dz = greyscale·scale_z + offset_z, applied unweighted
    // (idisc divide so the dispatcher's outer w· cancels). reset_z replace
    // vs add collapse to the same contribution in our summed model.
    let z = dc_mbox2d_grey(color);
    let dz = z * scale_z + offset_z;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    return vec3<f32>(uv.x, uv.y, dz * inv_w);
}
"#,
};
