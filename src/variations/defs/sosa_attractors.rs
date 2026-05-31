//! Sosa attractors (Jesus Sosa, 2017): clifford_js, svensson_js, sattractor_js
//!
//! Three classical strange-attractor variations from Paul Bourke's
//! collection (http://paulbourke.net/fractals/), packaged by Jesus Sosa
//! into the JS-suffix family. Each cpp file's `PluginVarCalc` was an
//! `unported_stub` — recovered here from the embedded Java comment.
//!
//!   - clifford_js: Clifford attractor.
//!     `xn+1 = sin(a·yn) + c·cos(a·xn)`
//!     `yn+1 = sin(b·xn) + d·cos(b·yn)`
//!     4 user params (a, b, c, d) with Java defaults
//!     (-1.4, 1.6, 1.0, 0.7); cpp's `_a = 0.0` and `_d = 0.0` are
//!     porter typos — restored to Java values. cpp uses `FPx = …`
//!     (assignment) like anamorphcyl. Body factors cleanly.
//!
//!   - svensson_js: Johnny Svensson attractor.
//!     `xn+1 = d·sin(a·xn) - sin(b·yn)`
//!     `yn+1 = c·cos(a·xn) + cos(b·yn)`
//!     4 user params (a, b, c, d) with Java defaults
//!     (1.4, 1.56, 1.4, -6.56); cpp's `_d = 0.0` is a porter typo.
//!
//!   - sattractor_js: Sosa's "S Attractor" — Henon IFS variant, picks
//!     a random vertex `l ∈ [1, m]` per iteration and chooses one of
//!     two affine maps (50/50). 1 user param `m` (default 10) recovered
//!     from Java setParameter (cpp APO_VARIABLES empty). Body factors
//!     cleanly. cpp's persistent `_a[13]/_b[13]` lookup tables replaced
//!     with on-the-fly `cos(2π·l/m)/sin(2π·l/m)` to fit our 16-slot
//!     init budget. RNG (2 calls/iter).
//!
//! Sources:
//!   - `output/jwildfire-vars/output/clifford_js.cpp`
//!   - `output/jwildfire-vars/output/svensson_js.cpp`
//!   - `output/jwildfire-vars/output/sattractor_js.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// clifford_js
// ---------------------------------------------------------------------------

/// Clifford strange attractor — iterates `(sin(a·y) + c·cos(a·x), sin(b·x)
/// + d·cos(b·y))`. Classic 4-parameter chaotic attractor with
/// characteristic Lissajous-like curves and folds. From Paul Bourke's
/// collection.
///
/// # Authors
/// - Jesus Sosa
pub static CLIFFORD_JS: VariationDef = VariationDef {
    name: "clifford_js",
    aliases: &[],
    display_name: "Clifford (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, -1.4, -10.0, 10.0, "Clifford attractor parameter `a` (in `sin(a·y) + c·cos(a·x)`)."),
        param!("b", "B", unlimited_float, 1.6, -10.0, 10.0, "Clifford attractor parameter `b` (in `sin(b·x) + d·cos(b·y)`)."),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0, "Clifford attractor parameter `c` — cosine coefficient on the X output."),
        param!("d", "D", unlimited_float, 0.7, -10.0, 10.0, "Clifford attractor parameter `d` — cosine coefficient on the Y output."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_clifford_js(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let x = sin(a * p.y) + c * cos(a * p.x);
    let y = sin(b * p.x) + d * cos(b * p.y);
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_clifford_js(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let x = sin(a * p.y) + c * cos(a * p.x);
    let y = sin(b * p.x) + d * cos(b * p.y);
    return vec3<f32>(x, y, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// svensson_js
// ---------------------------------------------------------------------------

/// Johnny Svensson attractor — iterates `(d·sin(a·x) − sin(b·y), c·cos(a·x)
/// + cos(b·y))`. 4-parameter strange attractor from Paul Bourke's
/// collection.
///
/// # Authors
/// - Jesus Sosa
pub static SVENSSON_JS: VariationDef = VariationDef {
    name: "svensson_js",
    aliases: &[],
    display_name: "Svensson (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.4, -10.0, 10.0, "Svensson attractor parameter `a` (X-axis sin/cos frequency)."),
        param!("b", "B", unlimited_float, 1.56, -10.0, 10.0, "Svensson attractor parameter `b` (Y-axis sin/cos frequency)."),
        param!("c", "C", unlimited_float, 1.4, -10.0, 10.0, "Svensson attractor parameter `c` — cosine amplitude on Y output."),
        param!("d", "D", unlimited_float, -6.56, -10.0, 10.0, "Svensson attractor parameter `d` — sine amplitude on X output."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_svensson_js(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let x = d * sin(a * p.x) - sin(b * p.y);
    let y = c * cos(a * p.x) + cos(b * p.y);
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_svensson_js(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let x = d * sin(a * p.x) - sin(b * p.y);
    let y = c * cos(a * p.x) + cos(b * p.y);
    return vec3<f32>(x, y, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// sattractor_js
// ---------------------------------------------------------------------------

/// Sosa's 'S Attractor' — Henon-style IFS variant. Each iteration picks a
/// random vertex angle `θ = 2π·l/m` (with `l ∈ [1, m]`) and chooses one of
/// two affine maps with 50/50 probability: either a half-scale plus offset
/// `(cos θ, sin θ)`, or a rotated quadratic warp.
///
/// # Authors
/// - Jesus Sosa
pub static SATTRACTOR_JS: VariationDef = VariationDef {
    name: "sattractor_js",
    aliases: &[],
    display_name: "S-Attractor (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("m", "M", int, 10.0, 1.0, 12.0, "Number of vertex angles (1-12). Each iteration samples one of `m` evenly-spaced angles around the unit circle."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_sattractor_js(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let m = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let mf = f32(m);
    let two_pi = 6.28318530717959;
    let l = i32(rng_nextf(rng) * mf) + 1;
    let l_clamped = clamp(l, 1, m);
    let theta = two_pi * f32(l_clamped) / mf;
    let a = cos(theta);
    let b = sin(theta);

    var x: f32;
    var y: f32;
    if (rng_nextf(rng) < 0.5) {
        x = p.x * 0.5 + a;
        y = p.y * 0.5 + b;
    } else {
        x = p.x * a + p.y * b + p.x * p.x * b;
        y = p.y * a - p.x * b + p.x * p.x * a;
    }
    return vec2<f32>(x * 0.5, y * 0.5);
}
"#,
    wgsl_3d: r#"
fn variation_sattractor_js(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let m = max(i32(get_param(xform_id, variation_id, 0u)), 1);
    let mf = f32(m);
    let two_pi = 6.28318530717959;
    let l = i32(rng_nextf(rng) * mf) + 1;
    let l_clamped = clamp(l, 1, m);
    let theta = two_pi * f32(l_clamped) / mf;
    let a = cos(theta);
    let b = sin(theta);

    var x: f32;
    var y: f32;
    if (rng_nextf(rng) < 0.5) {
        x = p.x * 0.5 + a;
        y = p.y * 0.5 + b;
    } else {
        x = p.x * a + p.y * b + p.x * p.x * b;
        y = p.y * a - p.x * b + p.x * p.x * a;
    }
    return vec3<f32>(x * 0.5, y * 0.5, p.z);
}
"#,
};
