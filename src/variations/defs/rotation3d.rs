//! 3D Rotation variations (indices 19-22)
//!
//! Pre and post rotation variations for 3D mode. The variation's own weight
//! is used as the rotation angle in radians, read from
//! `transforms[xform_id].variations[variation_id]` via `needs_transform`.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

/// Rotates the point around the X axis before any other variations run.
/// The rotation angle (in radians) is the variation's own weight slider —
/// e.g. set the weight to 1.57 for a 90° rotation.
pub static PRE_ROTATE_X: VariationDef = VariationDef {
    name: "pre_rotate_x",
    aliases: &[],
    display_name: "Pre Rotate X",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_pre_rotate_x(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_rotate_x(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let weight = transforms[xform_id].variations[variation_id];
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

/// Rotates the point around the Y axis before any other variations run.
/// The rotation angle (in radians) is the variation's own weight slider.
pub static PRE_ROTATE_Y: VariationDef = VariationDef {
    name: "pre_rotate_y",
    aliases: &[],
    display_name: "Pre Rotate Y",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_pre_rotate_y(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_rotate_y(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let weight = transforms[xform_id].variations[variation_id];
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

/// Rotates the output point around the X axis after all other variations
/// have run. The rotation angle (in radians) is the variation's own weight
/// slider.
pub static POST_ROTATE_X: VariationDef = VariationDef {
    name: "post_rotate_x",
    aliases: &[],
    display_name: "Post Rotate X",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_post_rotate_x(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_rotate_x(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let weight = transforms[xform_id].variations[variation_id];
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

/// Rotates the output point around the Y axis after all other variations
/// have run. The rotation angle (in radians) is the variation's own weight
/// slider.
pub static POST_ROTATE_Y: VariationDef = VariationDef {
    name: "post_rotate_y",
    aliases: &[],
    display_name: "Post Rotate Y",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_post_rotate_y(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_rotate_y(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let weight = transforms[xform_id].variations[variation_id];
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
