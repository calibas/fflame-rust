//! Blur variations (zblur, blur3d, pre_blur)
//!
//! These variations add randomized blur effects.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

pub static ZBLUR: VariationDef = VariationDef {
    name: "zblur",
    display_name: "Z-Blur",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_affine: false,
    init_param_count: 0,
    wgsl_init: None,
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

pub static BLUR3D: VariationDef = VariationDef {
    name: "blur3d",
    display_name: "Blur 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_affine: false,
    init_param_count: 0,
    wgsl_init: None,
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

pub static PRE_BLUR: VariationDef = VariationDef {
    name: "pre_blur",
    display_name: "Pre-Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: true,
    parameters: &[],
    needs_affine: false,
    init_param_count: 0,
    wgsl_init: None,
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
