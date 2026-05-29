//! Sosa attractors batch 4: hadamard_js, invtree_js, crown_js
//!
//!   - hadamard_js (Sosa, based on Paul Bourke's roger18.c):
//!     3-branch Hadamard IFS. 0 user params. RNG (2 calls/iter for
//!     3-way branch). Body factors cleanly through outer multiplier.
//!
//!   - invtree_js (Sosa, based on Paul Bourke's trifraction2):
//!     3-branch inverse-tree IFS. 0 user params. RNG (2 calls/iter
//!     for 3-way branch). Body factors cleanly through outer
//!     multiplier.
//!
//!   - crown_js (Sosa, based on Paul Bourke's Crown / Roger Bagula):
//!     14-iteration complex sum; 2 user params (a default 5, b
//!     default log(2)/log(3)) recovered from Java. RNG (1 call/iter
//!     for the t parameter). Full3D — z output is mag²² of the
//!     accumulated complex value. Note cpp has a porter typo
//!     `y = wt.real()`; Java's `y = wt.im` is restored. cpp's TC
//!     color write skipped per writes_color compromise.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/hadamard_js.cpp`
//!   - `output/jwildfire-vars/output/invtree_js.cpp`
//!   - `output/jwildfire-vars/output/crown_js.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// hadamard_js
// ---------------------------------------------------------------------------

/// 3-branch Hadamard IFS — picks one of three affine maps uniformly per
/// iteration: half-scale identity, or one of two half-scale rotation-plus-
/// offset maps. Produces a Hadamard fractal pattern. Based on Paul Bourke's
/// `roger18.c`.
///
/// # Authors
/// - Jesus Sosa
pub static HADAMARD_JS: VariationDef = VariationDef {
    name: "hadamard_js",
    aliases: &[],
    display_name: "Hadamard (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_hadamard_js(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r1 = rng_nextf(rng);
    var x: f32;
    var y: f32;
    if (r1 < 0.333) {
        x = p.x * 0.5;
        y = p.y * 0.5;
    } else {
        let r2 = rng_nextf(rng);
        if (r2 < 0.666) {
            x = p.y * 0.5;
            y = -p.x * 0.5 - 0.5;
        } else {
            x = -p.y * 0.5 - 0.5;
            y = p.x * 0.5;
        }
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_hadamard_js(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r1 = rng_nextf(rng);
    var x: f32;
    var y: f32;
    if (r1 < 0.333) {
        x = p.x * 0.5;
        y = p.y * 0.5;
    } else {
        let r2 = rng_nextf(rng);
        if (r2 < 0.666) {
            x = p.y * 0.5;
            y = -p.x * 0.5 - 0.5;
        } else {
            x = -p.y * 0.5 - 0.5;
            y = p.x * 0.5;
        }
    }
    return vec3<f32>(x, y, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// invtree_js
// ---------------------------------------------------------------------------

/// 3-branch inverse-tree IFS — picks one of three branches uniformly per
/// iteration: half-scale identity, or per-axis `1/(coord + 1)` inverse (one
/// axis at a time). Produces an inverse-tree fractal. Based on Paul
/// Bourke's `trifraction2`.
///
/// # Authors
/// - Jesus Sosa
pub static INVTREE_JS: VariationDef = VariationDef {
    name: "invtree_js",
    aliases: &[],
    display_name: "Inv-Tree (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_invtree_js(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r1 = rng_nextf(rng);
    var x: f32;
    var y: f32;
    if (r1 < 0.333) {
        x = p.x * 0.5;
        y = p.y * 0.5;
    } else {
        let r2 = rng_nextf(rng);
        let safe_x_p1 = select(p.x + 1.0, 1e-30, abs(p.x + 1.0) < 1e-30);
        let safe_y_p1 = select(p.y + 1.0, 1e-30, abs(p.y + 1.0) < 1e-30);
        if (r2 < 0.666) {
            x = 1.0 / safe_x_p1;
            y = p.y / safe_y_p1;
        } else {
            x = p.x / safe_x_p1;
            y = 1.0 / safe_y_p1;
        }
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_invtree_js(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r1 = rng_nextf(rng);
    var x: f32;
    var y: f32;
    if (r1 < 0.333) {
        x = p.x * 0.5;
        y = p.y * 0.5;
    } else {
        let r2 = rng_nextf(rng);
        let safe_x_p1 = select(p.x + 1.0, 1e-30, abs(p.x + 1.0) < 1e-30);
        let safe_y_p1 = select(p.y + 1.0, 1e-30, abs(p.y + 1.0) < 1e-30);
        if (r2 < 0.666) {
            x = 1.0 / safe_x_p1;
            y = p.y / safe_y_p1;
        } else {
            x = p.x / safe_x_p1;
            y = 1.0 / safe_y_p1;
        }
    }
    return vec3<f32>(x, y, p.z);
}
"#),
};

// ---------------------------------------------------------------------------
// crown_js
// ---------------------------------------------------------------------------

/// Crown / Roger Bagula attractor — sums 14 iterations of `(sin(a^k · ±t),
/// cos(a^k · ±t)) / |a|^(b·k)` to form a complex-valued accumulator. Per-
/// iteration `t` is sampled uniformly from `[-π, π]`. The Z output is
/// `|wt|^4` (the squared magnitude squared). Based on Paul Bourke's Crown /
/// Roger Bagula attractor.
///
/// # Authors
/// - Jesus Sosa
pub static CROWN_JS: VariationDef = VariationDef {
    name: "crown_js",
    aliases: &[],
    display_name: "Crown (JS)",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("a", "A", unlimited_float, 5.0, -10.0, 10.0, "Base of the exponential growth (`a^k` in the iteration). Default 5."),
        param!("b", "B", unlimited_float, 0.6309297535714574, -10.0, 10.0, "Power decay rate (`|a|^(b·k)` in the denominator). Default log(2)/log(3) ≈ 0.631."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_crown_js(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let t = -pi + 2.0 * pi * rng_nextf(rng);
    var wt_re = 0.0;
    var wt_im = 0.0;
    var pow_a_k = a;
    var sign_k = -1.0;
    let safe_a = select(a, 1e-30, abs(a) < 1e-30);
    for (var k = 1; k < 15; k = k + 1) {
        let kf = f32(k);
        let denom = pow(max(abs(safe_a), 1e-30), b * kf);
        let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
        let th = pow_a_k * sign_k * t;
        wt_re = wt_re + sin(th) / safe_denom;
        wt_im = wt_im + cos(th) / safe_denom;
        pow_a_k = pow_a_k * a;
        sign_k = -sign_k;
    }
    return vec2<f32>(wt_re, wt_im);
}
"#,
    wgsl_3d: Some(r#"
fn variation_crown_js(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let t = -pi + 2.0 * pi * rng_nextf(rng);
    var wt_re = 0.0;
    var wt_im = 0.0;
    var pow_a_k = a;
    var sign_k = -1.0;
    let safe_a = select(a, 1e-30, abs(a) < 1e-30);
    for (var k = 1; k < 15; k = k + 1) {
        let kf = f32(k);
        let denom = pow(max(abs(safe_a), 1e-30), b * kf);
        let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
        let th = pow_a_k * sign_k * t;
        wt_re = wt_re + sin(th) / safe_denom;
        wt_im = wt_im + cos(th) / safe_denom;
        pow_a_k = pow_a_k * a;
        sign_k = -sign_k;
    }
    let mag2 = wt_re * wt_re + wt_im * wt_im;
    let z = mag2 * mag2;
    return vec3<f32>(wt_re, wt_im, z);
}
"#),
};
