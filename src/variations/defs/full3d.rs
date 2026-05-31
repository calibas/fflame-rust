//! Full 3D variations (index 18)
//!
//! Variations that create true 3D structures by modifying all three coordinates.

use crate::variations::{
    definition::VariationDef,
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
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
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
