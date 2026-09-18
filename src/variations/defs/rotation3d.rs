//! 3D Rotation variations (indices 19-22)
//!
//! Pre and post rotation variations for 3D mode. The variation's own weight
//! is used as the rotation angle in radians, read from
//! `transforms[xform_id].variations[variation_id]` via `needs_transform`.

use crate::scene::ifs_analysis::{Affine3, AffineRole, Space};
use crate::variations::inverse::{InverseDef, InverseKernel};
use crate::variations::{
    definition::{Feature, VariationDef},
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
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_pre_rotate_x(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: r#"
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
"#,
};

/// Rotates the point around the Y axis before any other variations run.
/// The rotation angle (in radians) is the variation's own weight slider.
pub static PRE_ROTATE_Y: VariationDef = VariationDef {
    name: "pre_rotate_y",
    aliases: &[],
    display_name: "Pre Rotate Y",
    category: VariationCategory::Rotation3D,
    phase: VariationPhase::Pre,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_pre_rotate_y(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: r#"
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
"#,
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
    features: &[Feature::NeedsTransform, Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_post_rotate_x(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: r#"
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
"#,
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
    features: &[Feature::NeedsTransform, Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_post_rotate_y(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: r#"
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
"#,
};

// ------------------------------------------------- the inverse walks

/// The rotation `pre_rotate_x` and its three siblings are: by the
/// weight in RADIANS about the named axis, replacing the point in
/// its own phase (`ifs-general.md` D1).
///
/// Isometries, so they change no singular value. Their 2D stubs
/// return their input, so nothing in the plane.
///
/// `pre_rotate_x` is `(x, s·z + c·y, c·z − s·y)` and `_y` is
/// `(c·x − s·z, y, s·x + c·z)`, read off the bodies above.
fn rotate_role(w: f64, space: Space, about_x: bool, pre: bool) -> Option<AffineRole> {
    if matches!(space, Space::Planar) {
        return Some(AffineRole::Nothing);
    }
    let (sn, cs) = w.sin_cos();
    let m = if about_x {
        [[1.0, 0.0, 0.0], [0.0, cs, sn], [0.0, -sn, cs]]
    } else {
        [[cs, 0.0, -sn], [0.0, 1.0, 0.0], [sn, 0.0, cs]]
    };
    let r = Affine3 { m, t: [0.0; 3] };
    Some(if pre { AffineRole::Pre(r) } else { AffineRole::Post(r) })
}

pub static INVERSE_PRE_ROTATE_X: InverseDef = InverseDef {
    name: "pre_rotate_x",
    kernel: InverseKernel::Affine(|w, _, space| rotate_role(w, space, true, true)),
};

pub static INVERSE_PRE_ROTATE_Y: InverseDef = InverseDef {
    name: "pre_rotate_y",
    kernel: InverseKernel::Affine(|w, _, space| rotate_role(w, space, false, true)),
};

pub static INVERSE_POST_ROTATE_X: InverseDef = InverseDef {
    name: "post_rotate_x",
    kernel: InverseKernel::Affine(|w, _, space| rotate_role(w, space, true, false)),
};

pub static INVERSE_POST_ROTATE_Y: InverseDef = InverseDef {
    name: "post_rotate_y",
    kernel: InverseKernel::Affine(|w, _, space| rotate_role(w, space, false, false)),
};
