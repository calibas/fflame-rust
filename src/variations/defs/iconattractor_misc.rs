//! iconattractor_js (Jesus Sosa, 2018)
//!
//! Symmetric Icon Attractors from "Symmetry in Chaos" by Field and
//! Golubitsky. cpp picks one of 17 preset (degree, a, b, g, o, l)
//! tuples randomly at init; we expose `preset_id` as a user param so
//! the user picks a specific attractor. The 17 presets are baked into
//! a runtime switch in the WGSL body.
//!
//! 4 user params:
//!   - preset_id (int 0-16)
//!   - centerx, centery (color register inputs — unused since we
//!     skip the TC color write per writes_color compromise; kept to
//!     match cpp interface)
//!   - scale (used only for the color register; 1 init slot
//!     `_bdcs = 1/scale` is preserved to match cpp interface)
//!
//! Body computes the symmetric icon attractor:
//!   p   = a · |z|² + l + b · Re(z^degree)
//!   x'  = p · z.x + g · Re(z^(degree-1)) − o · z.y
//!   y'  = p · z.y − g · Im(z^(degree-1)) + o · z.x
//! where z^(degree-1) is built by iterating complex multiplication
//! (degree-2 iterations after the initial z^1). cpp uses a runtime
//! `for (int i = 1; i <= degree-2; i++)` loop; preserved as
//! a runtime-bounded WGSL loop with explicit cap.
//!
//! Body factors cleanly through outer multiplier (cpp uses VVAR
//! consistently). cpp's TC color write skipped.
//!
//! Source: `output/jwildfire-vars/output/iconattractor_js.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Symmetric Icon Attractor — implements one of 17 baked-in presets from
/// Field & Golubitsky's *Symmetry in Chaos*. Each preset is a `(degree, a,
/// b, g, o, l)` tuple selected by `preset_id`. The body computes the
/// standard symmetric-icon iteration: `p = a·|z|² + l + b·Re(z^degree)`,
/// then `out = p·z + g·Re(z^(degree-1)) − o·rot90(z)`. The complex-power
/// iteration is bounded at 24 to keep WGSL happy.
///
/// # Authors
/// - Jesus Sosa
pub static ICONATTRACTOR_JS: VariationDef = VariationDef {
    name: "iconattractor_js",
    display_name: "Icon Attractor (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("preset_id", "Preset ID", int, 0.0, 0.0, 16.0, "Which Field & Golubitsky preset to use (0-16). Each corresponds to a different symmetric icon attractor shape from the original book."),
        param!("centerx", "Center X", unlimited_float, 0.0, -10.0, 10.0, "Color register X center — unused since the color write is dropped. Preserved as a parameter for cpp parity."),
        param!("centery", "Center Y", unlimited_float, 0.0, -10.0, 10.0, "Color register Y center — unused since the color write is dropped. Preserved as a parameter for cpp parity."),
        param!("scale", "Scale", unlimited_float, 5.0, -100.0, 100.0, "Color register scale — unused since the color write is dropped. Init still precomputes `1/scale` to match cpp's `_bdcs`. Preserved as a parameter for cpp parity."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_iconattractor_js(user: array<f32, 4>) -> array<f32, 1> {
    var out: array<f32, 1>;
    let scale = user[3];
    let safe_scale = select(scale, 1e-5, abs(scale) < 1e-30);
    out[0] = 1.0 / safe_scale;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn ic_preset(id: i32) -> array<f32, 6> {
    // Returns [degree, a, b, g, o, l] for preset id in [0, 16].
    var out: array<f32, 6>;
    let i = clamp(id, 0, 16);
    var deg: f32 = 5.0;
    var a: f32 = 5.0;
    var b: f32 = -1.9;
    var g: f32 = 1.0;
    var o: f32 = 0.188;
    var l: f32 = -2.5;
    if (i == 1) { deg = 3.0; a = -1.0; b = 0.1; g = -0.82; o = 0.12; l = 1.56; }
    else if (i == 2) { deg = 5.0; a = 1.806; b = 0.0; g = 1.0; o = 0.0; l = -1.806; }
    else if (i == 3) { deg = 3.0; a = 10.0; b = -12.0; g = 1.0; o = 0.0; l = -2.195; }
    else if (i == 4) { deg = 3.0; a = -2.5; b = 0.0; g = 0.9; o = 0.0; l = 2.5; }
    else if (i == 5) { deg = 9.0; a = 3.0; b = -16.79; g = 1.0; o = 0.0; l = -2.05; }
    else if (i == 6) { deg = 6.0; a = 5.0; b = 1.5; g = 1.0; o = 0.0; l = -2.7; }
    else if (i == 7) { deg = 23.0; a = -2.5; b = 0.0; g = 0.9; o = 0.0; l = 2.409; }
    else if (i == 8) { deg = 7.0; a = 1.0; b = -0.1; g = 0.167; o = 0.0; l = -2.08; }
    else if (i == 9) { deg = 5.0; a = 2.32; b = 0.0; g = 0.75; o = 0.0; l = -2.32; }
    else if (i == 10) { deg = 5.0; a = -2.0; b = 0.0; g = -0.5; o = 0.0; l = 2.6; }
    else if (i == 11) { deg = 5.0; a = 2.0; b = 0.2; g = 0.1; o = 0.0; l = -2.34; }
    else if (i == 12) { deg = 4.0; a = 2.0; b = 0.0; g = 1.0; o = 0.1; l = -1.86; }
    else if (i == 13) { deg = 3.0; a = -1.0; b = 0.1; g = -0.82; o = 0.0; l = 1.56; }
    else if (i == 14) { deg = 3.0; a = -1.0; b = 0.1; g = -0.805; o = 0.0; l = 1.5; }
    else if (i == 15) { deg = 3.0; a = -1.0; b = 0.03; g = -0.80; o = 0.0; l = 1.455; }
    else if (i == 16) { deg = 16.0; a = -2.5; b = -0.1; g = 0.90; o = -0.15; l = 2.39; }
    out[0] = deg;
    out[1] = a;
    out[2] = b;
    out[3] = g;
    out[4] = o;
    out[5] = l;
    return out;
}

fn variation_iconattractor_js(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let preset = ic_preset(preset_id);
    let degree = i32(preset[0]);
    let a = preset[1];
    let b = preset[2];
    let g = preset[3];
    let o = preset[4];
    let l = preset[5];

    let zzbar = p.x * p.x + p.y * p.y;
    var pp = a * zzbar + l;

    var zreal = p.x;
    var zimag = p.y;
    let max_loop = max(degree - 2, 0);
    for (var i = 0; i < 24; i = i + 1) {
        if (i >= max_loop) { break; }
        let za = zreal * p.x - zimag * p.y;
        let zb = zimag * p.x + zreal * p.y;
        zreal = za;
        zimag = zb;
    }
    let zn = p.x * zreal - p.y * zimag;
    pp = pp + b * zn;

    let nx = pp * p.x + g * zreal - o * p.y;
    let ny = pp * p.y - g * zimag + o * p.x;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn ic_preset(id: i32) -> array<f32, 6> {
    // Returns [degree, a, b, g, o, l] for preset id in [0, 16].
    var out: array<f32, 6>;
    let i = clamp(id, 0, 16);
    var deg: f32 = 5.0;
    var a: f32 = 5.0;
    var b: f32 = -1.9;
    var g: f32 = 1.0;
    var o: f32 = 0.188;
    var l: f32 = -2.5;
    if (i == 1) { deg = 3.0; a = -1.0; b = 0.1; g = -0.82; o = 0.12; l = 1.56; }
    else if (i == 2) { deg = 5.0; a = 1.806; b = 0.0; g = 1.0; o = 0.0; l = -1.806; }
    else if (i == 3) { deg = 3.0; a = 10.0; b = -12.0; g = 1.0; o = 0.0; l = -2.195; }
    else if (i == 4) { deg = 3.0; a = -2.5; b = 0.0; g = 0.9; o = 0.0; l = 2.5; }
    else if (i == 5) { deg = 9.0; a = 3.0; b = -16.79; g = 1.0; o = 0.0; l = -2.05; }
    else if (i == 6) { deg = 6.0; a = 5.0; b = 1.5; g = 1.0; o = 0.0; l = -2.7; }
    else if (i == 7) { deg = 23.0; a = -2.5; b = 0.0; g = 0.9; o = 0.0; l = 2.409; }
    else if (i == 8) { deg = 7.0; a = 1.0; b = -0.1; g = 0.167; o = 0.0; l = -2.08; }
    else if (i == 9) { deg = 5.0; a = 2.32; b = 0.0; g = 0.75; o = 0.0; l = -2.32; }
    else if (i == 10) { deg = 5.0; a = -2.0; b = 0.0; g = -0.5; o = 0.0; l = 2.6; }
    else if (i == 11) { deg = 5.0; a = 2.0; b = 0.2; g = 0.1; o = 0.0; l = -2.34; }
    else if (i == 12) { deg = 4.0; a = 2.0; b = 0.0; g = 1.0; o = 0.1; l = -1.86; }
    else if (i == 13) { deg = 3.0; a = -1.0; b = 0.1; g = -0.82; o = 0.0; l = 1.56; }
    else if (i == 14) { deg = 3.0; a = -1.0; b = 0.1; g = -0.805; o = 0.0; l = 1.5; }
    else if (i == 15) { deg = 3.0; a = -1.0; b = 0.03; g = -0.80; o = 0.0; l = 1.455; }
    else if (i == 16) { deg = 16.0; a = -2.5; b = -0.1; g = 0.90; o = -0.15; l = 2.39; }
    out[0] = deg;
    out[1] = a;
    out[2] = b;
    out[3] = g;
    out[4] = o;
    out[5] = l;
    return out;
}

fn variation_iconattractor_js(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let preset = ic_preset(preset_id);
    let degree = i32(preset[0]);
    let a = preset[1];
    let b = preset[2];
    let g = preset[3];
    let o = preset[4];
    let l = preset[5];

    let zzbar = p.x * p.x + p.y * p.y;
    var pp = a * zzbar + l;

    var zreal = p.x;
    var zimag = p.y;
    let max_loop = max(degree - 2, 0);
    for (var i = 0; i < 24; i = i + 1) {
        if (i >= max_loop) { break; }
        let za = zreal * p.x - zimag * p.y;
        let zb = zimag * p.x + zreal * p.y;
        zreal = za;
        zimag = zb;
    }
    let zn = p.x * zreal - p.y * zimag;
    pp = pp + b * zn;

    let nx = pp * p.x + g * zreal - o * p.y;
    let ny = pp * p.y - g * zimag + o * p.x;
    return vec3<f32>(nx, ny, p.z);
}
"#),
};
