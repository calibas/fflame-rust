//! Basic 2D variations (indices 0-4)
//!
//! These are the fundamental variations from the original fractal flame algorithm.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

pub static LINEAR: VariationDef = VariationDef {
    name: "linear",
    display_name: "Linear",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
fn variation_linear(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_linear(p: vec3<f32>) -> vec3<f32> {
    return p;
}
"#),
};

pub static SINUSOIDAL: VariationDef = VariationDef {
    name: "sinusoidal",
    display_name: "Sinusoidal",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
fn variation_sinusoidal(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(p.x), sin(p.y));
}
"#,
    wgsl_3d: Some(r#"
fn variation_sinusoidal(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(sin(p.x), sin(p.y), p.z);
}
"#),
};

pub static SPHERICAL: VariationDef = VariationDef {
    name: "spherical",
    display_name: "Spherical",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
fn variation_spherical(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;
    return p / r2;
}
"#,
    wgsl_3d: Some(r#"
fn variation_spherical(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy) + 1e-6;
    return vec3(p.xy / r2, p.z);
}
"#),
};

pub static SWIRL: VariationDef = VariationDef {
    name: "swirl",
    display_name: "Swirl",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
fn variation_swirl(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let s = sin(r2);
    let c = cos(r2);
    return vec2<f32>(p.x * s - p.y * c, p.x * c + p.y * s);
}
"#,
    wgsl_3d: Some(r#"
fn variation_swirl(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy);
    let s = sin(r2);
    let c = cos(r2);
    return vec3<f32>(p.x * s - p.y * c, p.x * c + p.y * s, p.z);
}
"#),
};

pub static HORSESHOE: VariationDef = VariationDef {
    name: "horseshoe",
    display_name: "Horseshoe",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
fn variation_horseshoe(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let r_inv = 1.0 / r;
    return vec2<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_horseshoe(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy) + 1e-6;
    let r_inv = 1.0 / r;
    return vec3<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv,
        p.z
    );
}
"#),
};
