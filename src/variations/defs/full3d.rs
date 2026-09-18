//! Full 3D variations (index 18)
//!
//! Variations that create true 3D structures by modifying all three coordinates.

use crate::scene::ifs_analysis::Kernel;
use crate::variations::inverse::{InverseDef, InverseKernel, Refusal};
use crate::variations::{
    definition::{Feature, VariationDef},
    VariationCategory, VariationPhase,
};

/// Projects each point onto a hemisphere. The XY position picks a spot on
/// the dome and Z becomes the height at that spot — points near the
/// origin sit high, points far away curve down toward the rim.
pub static HEMISPHERE: VariationDef = VariationDef {
    name: "hemisphere",
    aliases: &[],
    display_name: "Hemisphere",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D version - projects onto unit circle
fn variation_hemisphere(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let t = 1.0 / sqrt(r2 + 1.0);
    return vec2<f32>(p.x * t, p.y * t);
}
"#,
    wgsl_3d: r#"
fn variation_hemisphere(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: t = 1 / sqrt(x² + y² + 1)
    // result = (x*t, y*t, t)
    let r2_xy = dot(p.xy, p.xy);
    let t = 1.0 / sqrt(r2_xy + 1.0);
    return vec3<f32>(p.x * t, p.y * t, t);
}
"#,
};

// ------------------------------------------------- the inverse walks

/// `hemisphere`: onto the open unit disc and one-to-one
/// (`ifs-general.md` D1).
///
/// Planar, though it lives in a 3D file: the walk inverts the map it
/// makes of the PLANE, which is `p / sqrt(|p|² + 1)`.
pub static INVERSE_HEMISPHERE: InverseDef = InverseDef {
    name: "hemisphere",
    kernel: InverseKernel::Planar(|_| Ok(Kernel::Hemisphere)),
};
