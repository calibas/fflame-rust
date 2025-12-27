//! 3D Depth variations (indices 16-17, 23)
//!
//! These variations modify only the Z component for depth effects.
//! They are only used in 3D rendering mode.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

pub static ZCONE: VariationDef = VariationDef {
    name: "zcone",
    display_name: "ZCone",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_zcone(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_zcone(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Z-only variation: adds distance from origin to Z
    // result.z += weight * length(p.xy)
    return vec3<f32>(0.0, 0.0, length(p.xy));
}
"#),
};

pub static FLATTEN: VariationDef = VariationDef {
    name: "flatten",
    display_name: "Flatten",
    category: VariationCategory::Depth3D,
    // NOTE: Flatten is treated as POST despite being index 1 (Apophysis XForm.pas)
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_flatten(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_flatten(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Z-only variation: flattens Z toward zero
    // result.z -= weight * p.z (subtracts to cancel out Z)
    return vec3<f32>(0.0, 0.0, -p.z);
}
"#),
};

pub static ZSCALE: VariationDef = VariationDef {
    name: "zscale",
    display_name: "ZScale",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_zscale(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_zscale(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Z-only variation: scales Z by weight
    // result.z += weight * p.z (adds scaled Z)
    return vec3<f32>(0.0, 0.0, p.z);
}
"#),
};
