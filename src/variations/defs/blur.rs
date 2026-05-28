//! Blur variations (zblur, blur3d, pre_blur)
//!
//! These variations add randomized blur effects.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

/// Adds gaussian random noise on the Z axis only — XY position is kept,
/// Z is replaced with a bell-curve random value. Useful for softening
/// the depth profile of an otherwise crisp 3D shape.
pub static ZBLUR: VariationDef = VariationDef {
    name: "zblur",
    aliases: &[],
    display_name: "Z-Blur",
    category: VariationCategory::Depth3D,
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
fn variation_zblur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // ZBlur only affects Z (3D mode), pass through in 2D
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_zblur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: Gaussian blur on Z axis only
    // result = (x, y, (rand₁ + rand₂ + rand₃ + rand₄ - 2))
    let z_offset = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(p.x, p.y, z_offset);
}
"#),
};

/// Random gaussian scatter across all three axes — replaces the point
/// with a random sample from a spherical cloud around the origin. The
/// 3D counterpart of Gaussian Blur.
pub static BLUR3D: VariationDef = VariationDef {
    name: "blur3d",
    aliases: &[],
    display_name: "Blur 3D",
    category: VariationCategory::Full3D,
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
fn variation_blur3d(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis: 3D Gaussian spherical blur
    // In 2D mode, apply XY components only
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let phi = rng_nextf(rng) * 3.14159265359;    // π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(r * sin(phi) * cos(theta), r * sin(phi) * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_blur3d(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: 3D Gaussian spherical blur
    // θ = random × 2π (azimuth)
    // φ = random × π (polar angle)
    // r = Gaussian (sum of 4 randoms - 2)
    // result = (r × sin(φ) × cos(θ), r × sin(φ) × sin(θ), r × cos(φ))
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let phi = rng_nextf(rng) * 3.14159265359;    // π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(r * sin(phi) * cos(theta), r * sin(phi) * sin(theta), r * cos(phi));
}
"#),
};

/// Adds gaussian random offset to the input point before the rest of
/// the variations run. Softens sharp features by jittering each
/// iteration's starting position.
pub static PRE_BLUR: VariationDef = VariationDef {
    name: "pre_blur",
    aliases: &[],
    display_name: "Pre-Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
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
fn variation_pre_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis: Pre-phase Gaussian blur applied before variations
    // FTx += r * cos(θ), FTy += r * sin(θ)
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(p.x + r * cos(theta), p.y + r * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: Pre-phase Gaussian blur applied before variations
    // FTx += r * cos(θ), FTy += r * sin(θ), FTz unchanged
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(p.x + r * cos(theta), p.y + r * sin(theta), p.z);
}
"#),
};
