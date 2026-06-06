//! Apophysis miscellany 15 — three more
//!
//!   - `pre_sinusoidal3d` (gossamer light) — 0 user; pre-phase
//!                                            sinusoidal with 3D output
//!                                            (z = atan2(x_new², y_new²)
//!                                            · cos(z_in)). Reads the
//!                                            JUST-WRITTEN x, y in the
//!                                            z computation. Distinct
//!                                            from existing
//!                                            `pre_sinusoidal` (no Z)
//!   - `pre_blur3D`       (?)              — 0 user; pre-phase
//!                                            Gaussian-blur via
//!                                            sum-of-6-uniforms · sphere.
//!                                            cpp uses a persistent
//!                                            `_gauss_rnd[6]` buffer
//!                                            cycled across iterations
//!                                            (perf optimization);
//!                                            we use 6 fresh randoms
//!                                            per call (statistically
//!                                            equivalent without
//!                                            persistent state). RNG
//!   - `julian3Dx`        (Xyrus02)         — 8 user (power int,
//!                                            dist, a..f) + 2 init
//!                                            slots (absN, cN); RNG;
//!                                            Full3D. Body has w in
//!                                            `radius_out = w · pow(...)`
//!                                            then in `gamma = radius_out
//!                                            · sqrt(...)`; output has
//!                                            one w factor.
//!                                            needs_transform divide-out.
//!                                            cpp's
//!                                            `(int)(rand · absN)`
//!                                            replaced with
//!                                            `floor(rand · absN)`
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// pre_sinusoidal3d (gossamer light)
//   FTx = w · sin(x_in)
//   FTy = w · sin(y_in)
//   FTz = w · atan2(FTx², FTy²) · cos(z_in)   (uses JUST-WRITTEN FTx, FTy)
// 0 user; needs_transform.
// =============================================================================
/// Pre-phase sinusoidal with 3D output — applies `(sin x, sin y)` to XY
/// (same as the standard sinusoidal), plus a Z output `atan2(sin²x, sin²y)
/// · cos(z)` that uses the just-written X/Y values. Distinct from the
/// existing `pre_sinusoidal`, which leaves Z alone.
///
/// # Authors
/// - gossamer light
pub static PRE_SINUSOIDAL3D: VariationDef = VariationDef {
    name: "pre_sinusoidal3d",
    aliases: &[],
    display_name: "Pre Sinusoidal 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Pre,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_pre_sinusoidal3d(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    return vec2<f32>(w * sin(p.x), w * sin(p.y));
}
"#,
    wgsl_3d: r#"
fn variation_pre_sinusoidal3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let nx = w * sin(p.x);
    let ny = w * sin(p.y);
    let nz = w * atan2(nx * nx, ny * ny) * cos(p.z);
    return vec3<f32>(nx, ny, nz);
}
"#,
};

// =============================================================================
// pre_blur3D
//   r = w · (g₀ + g₁ + ... + g₅ − 3)   (Gaussian via central limit)
//   angle1 = rand · 2π
//   angle2 = rand · π
//   FTx += r · sin(angle2) · cos(angle1)
//   FTy += r · sin(angle2) · sin(angle1)
//   FTz += r · cos(angle2)
// 0 user; needs_rng; needs_transform.
// =============================================================================
/// Pre-phase 3D Gaussian-sphere blur — adds a random offset along a unit-
/// sphere direction with Gaussian-distributed radius. The radius is `(sum
/// of 6 uniforms) − 3` (central-limit Gaussian approximation); direction is
/// picked uniformly on the sphere via `(sin b · cos a, sin b · sin a, cos
/// b)` with `a ∈ [0, 2π], b ∈ [0, π]`. Effectively a 3D Gaussian blur of
/// the input.
pub static PRE_BLUR3D: VariationDef = VariationDef {
    name: "pre_blur3D",
    aliases: &[],
    display_name: "Pre Blur 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Pre,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_pre_blur3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let pi = 3.14159265358979;
    let w = transforms[xform_id].variations[variation_id];
    let gauss = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) +
                rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 3.0;
    let r = w * gauss;
    let a = rng_nextf(rng) * two_pi;
    let b = rng_nextf(rng) * pi;
    let sb = sin(b);
    return vec2<f32>(p.x + r * sb * cos(a), p.y + r * sb * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_pre_blur3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let pi = 3.14159265358979;
    let w = transforms[xform_id].variations[variation_id];
    let gauss = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) +
                rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 3.0;
    let r = w * gauss;
    let a = rng_nextf(rng) * two_pi;
    let b = rng_nextf(rng) * pi;
    let sb = sin(b);
    return vec3<f32>(p.x + r * sb * cos(a),
                     p.y + r * sb * sin(a),
                     p.z + r * cos(b));
}
"#,
};

// =============================================================================
// julian3Dx (Xyrus02)
//   8 user: power int, dist, a, b, c, d, e, f (affine pre-transform)
//   2 init: absN = |power|;  cN = (dist / power − 1) / 2
//   z_local = x / absN
//   radius_square = x² + y²
//   radius_out = w · pow(r² + z_local², cN)
//   FPz_contrib = radius_out · z_local
//   X = a·x + b·y + e;  Y = c·x + d·y + f
//   alpha = (atan2(Y, X) + 2π · floor(rand · absN)) / power
//   gamma = radius_out · sqrt(r²)
//   out = (gamma · cos α, gamma · sin α, FPz_contrib)
// Body has one w factor — needs_transform divide-out.
// =============================================================================
/// 3D Julia-N with affine pre-transform — extends `julian2` to 3D: same
/// affine pre-transform `(X, Y) = (a·x + b·y + e, c·x + d·y + f)` and
/// random angular branch, plus a `z_local = x / |power|` term that feeds
/// into the radius computation and produces a Z output `radius_out ·
/// z_local`.
///
/// # Authors
/// - Xyrus02
pub static JULIAN3DX: VariationDef = VariationDef {
    name: "julian3Dx",
    aliases: &[],
    display_name: "JuliaN 3D X",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[
        param!("power", "Power", int, 0.0, -100.0, 100.0, "Number of angular branches. 0 produces zero output."),
        param!("dist", "Distance", unlimited_float, 1.0, -100.0, 100.0, "Radial-power exponent — `cN = (dist/power − 1)/2`."),
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Pre-affine xx coefficient."),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine xy coefficient."),
        param!("c", "C", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine yx coefficient."),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0, "Pre-affine yy coefficient."),
        param!("e", "E", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine x offset."),
        param!("f", "F", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine y offset."),
    ],
    // 2 derived values at slots 8..10:
    //   8: absN = |power|
    //   9: cN   = (dist / power − 1) / 2
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_julian3Dx(user: array<f32, 8>) -> array<f32, 2> {
    let safe_power = select(user[0], 1.0, abs(user[0]) < 1e-30);
    var out: array<f32, 2>;
    out[0] = abs(safe_power);
    out[1] = (user[1] / safe_power - 1.0) * 0.5;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_julian3Dx(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d = get_param(xform_id, variation_id, 5u);
    let e = get_param(xform_id, variation_id, 6u);
    let f = get_param(xform_id, variation_id, 7u);
    let absN = max(get_param(xform_id, variation_id, 8u), 1.0);
    let cN = get_param(xform_id, variation_id, 9u);
    let two_pi = 6.28318530717959;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    if (abs(power) < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let z_local = p.x / absN;
    let radius_square = p.x * p.x + p.y * p.y;
    let radius_out = w * pow(max(radius_square + z_local * z_local, 1e-30), cN);

    let xx = a * p.x + b * p.y + e;
    let yy = c * p.x + d * p.y + f;
    let alpha = (atan2(yy, xx) + two_pi * floor(rng_nextf(rng) * absN)) / power;
    let gamma = radius_out * sqrt(max(radius_square, 0.0));
    return vec2<f32>(gamma * cos(alpha) * inv_w, gamma * sin(alpha) * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_julian3Dx(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d = get_param(xform_id, variation_id, 5u);
    let e = get_param(xform_id, variation_id, 6u);
    let f = get_param(xform_id, variation_id, 7u);
    let absN = max(get_param(xform_id, variation_id, 8u), 1.0);
    let cN = get_param(xform_id, variation_id, 9u);
    let two_pi = 6.28318530717959;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    if (abs(power) < 1e-30) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let z_local = p.x / absN;
    let radius_square = p.x * p.x + p.y * p.y;
    let radius_out = w * pow(max(radius_square + z_local * z_local, 1e-30), cN);
    let fz = radius_out * z_local;

    let xx = a * p.x + b * p.y + e;
    let yy = c * p.x + d * p.y + f;
    let alpha = (atan2(yy, xx) + two_pi * floor(rng_nextf(rng) * absN)) / power;
    let gamma = radius_out * sqrt(max(radius_square, 0.0));
    return vec3<f32>(gamma * cos(alpha) * inv_w, gamma * sin(alpha) * inv_w, fz * inv_w);
}
"#,
};
