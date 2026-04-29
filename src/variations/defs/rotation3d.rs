//! 3D Rotation variations (indices 19-22)
//!
//! Pre and post rotation variations for 3D mode.
//! These apply rotation transforms in the XZ or YZ planes.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

pub static PRE_ROTATE_X: VariationDef = VariationDef {
    name: "pre_rotate_x",
    display_name: "Pre Rotate X",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_pre_rotate_x(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_rotate_x(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Pre-phase rotation around X axis
    // The weight is used as the rotation angle (in radians)
    let c = cos(weight);
    let s = sin(weight);
    return vec3<f32>(
        p.x,
        s * p.z + c * p.y,
        c * p.z - s * p.y
    );
}
"#),
};

pub static PRE_ROTATE_Y: VariationDef = VariationDef {
    name: "pre_rotate_y",
    display_name: "Pre Rotate Y",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_pre_rotate_y(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_rotate_y(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Pre-phase rotation around Y axis
    // The weight is used as the rotation angle (in radians)
    let c = cos(weight);
    let s = sin(weight);
    return vec3<f32>(
        c * p.x - s * p.z,
        p.y,
        s * p.x + c * p.z
    );
}
"#),
};

pub static POST_ROTATE_X: VariationDef = VariationDef {
    name: "post_rotate_x",
    display_name: "Post Rotate X",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_post_rotate_x(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_rotate_x(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Post-phase rotation around X axis
    // The weight is used as the rotation angle (in radians)
    let c = cos(weight);
    let s = sin(weight);
    return vec3<f32>(
        p.x,
        s * p.z + c * p.y,
        c * p.z - s * p.y
    );
}
"#),
};

pub static POST_ROTATE_Y: VariationDef = VariationDef {
    name: "post_rotate_y",
    display_name: "Post Rotate Y",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_post_rotate_y(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_rotate_y(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Post-phase rotation around Y axis
    // The weight is used as the rotation angle (in radians)
    let c = cos(weight);
    let s = sin(weight);
    return vec3<f32>(
        c * p.x - s * p.z,
        p.y,
        s * p.x + c * p.z
    );
}
"#),
};
