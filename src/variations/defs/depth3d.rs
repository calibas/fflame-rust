//! 3D Depth variations (indices 16-17, 23)
//!
//! These variations modify only the Z component for depth effects.
//! They are only used in 3D rendering mode.

use crate::scene::ifs_analysis::{Affine3, AffineRole, Space};
use crate::variations::inverse::{InverseDef, InverseKernel};
use crate::variations::{
    definition::{Feature, VariationDef},
    VariationCategory, VariationPhase,
};

/// Lifts every point along the Z axis by its horizontal distance from
/// the origin. Turns a flat XY input into a cone whose tip sits at the
/// origin and whose base spreads outward.
pub static ZCONE: VariationDef = VariationDef {
    name: "zcone",
    aliases: &[],
    display_name: "ZCone",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D stub: zcone only writes Z. Return (0, 0) so the additive
// dispatch `result += weight * var(p)` contributes nothing in 2D
// mode. Returning `p` would inject `weight × p.xy` as a phantom
// linear (same bug class as the zblur fix).
fn variation_zcone(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_zcone(p: vec3<f32>) -> vec3<f32> {
    // Normal-phase Z-only contribution. Standard dispatch is
    // `result += weight * variation_zcone(p)`, so returning
    // `(0, 0, length(p.xy))` accumulates `weight * length(p.xy)` into Z.
    return vec3<f32>(0.0, 0.0, length(p.xy));
}
"#,
};

/// Cancels the Z coordinate, projecting everything back onto the XY
/// plane. Runs in the post phase to undo accumulated depth from earlier
/// 3D variations.
pub static FLATTEN: VariationDef = VariationDef {
    name: "flatten",
    aliases: &[],
    display_name: "Flatten",
    category: VariationCategory::Depth3D,
    // NOTE: Flatten is treated as POST despite being index 1 (Apophysis XForm.pas)
    phase: VariationPhase::Post,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D stub - not used in 2D mode
fn variation_flatten(p: vec2<f32>) -> vec2<f32> {
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_flatten(p: vec3<f32>) -> vec3<f32> {
    // Post-phase Z-zeroing. Standard post dispatch is
    // `result = variation_flatten(result)` (gated on weight != 0), so
    // returning `(p.x, p.y, 0)` matches Apophysis's `FPz := 0`.
    return vec3<f32>(p.x, p.y, 0.0);
}
"#,
};

/// Multiplies the Z coordinate by the variation's weight slider. Combine
/// with other 3D variations to amplify or dampen the depth they
/// contribute.
pub static ZSCALE: VariationDef = VariationDef {
    name: "zscale",
    aliases: &[],
    display_name: "ZScale",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// 2D stub: zscale only writes Z. See zcone above for the rationale.
fn variation_zscale(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_zscale(p: vec3<f32>) -> vec3<f32> {
    // Normal-phase Z-only contribution. Standard dispatch is
    // `result += weight * variation_zscale(p)`, so returning
    // `(0, 0, p.z)` accumulates `weight * p.z` into Z.
    return vec3<f32>(0.0, 0.0, p.z);
}
"#,
};

// ------------------------------------------------- the inverse walks

/// `diag(x, y, z)` as an affine with no translation.
fn diag3(x: f64, y: f64, z: f64) -> Affine3 {
    Affine3 { m: [[x, 0.0, 0.0], [0.0, y, 0.0], [0.0, 0.0, z]], t: [0.0; 3] }
}

/// `zscale`: `w·z` on the z axis, summed (`ifs-general.md` D1).
///
/// Nothing in the plane: the 2D stub returns zero, and the planar
/// chaos game has no z for it to scale.
pub static INVERSE_ZSCALE: InverseDef = InverseDef {
    name: "zscale",
    kernel: InverseKernel::Affine(|w, _, space| {
        Some(match space {
            Space::Planar => AffineRole::Nothing,
            Space::Solid => AffineRole::Sum(diag3(0.0, 0.0, w)),
        })
    }),
};

/// `flatten`: post-phase `z ← 0`.
///
/// Nothing in the plane, where its stub returns its input. As a
/// solid it is affine and SINGULAR, and the criterion says so: a map
/// with no inverse collapses the attractor. Five shipped flames were
/// lost to this one variation before the rule distinguished the two
/// spaces.
pub static INVERSE_FLATTEN: InverseDef = InverseDef {
    name: "flatten",
    kernel: InverseKernel::Affine(|_, _, space| {
        Some(match space {
            Space::Planar => AffineRole::Nothing,
            Space::Solid => AffineRole::Post(diag3(1.0, 1.0, 0.0)),
        })
    }),
};

/// `zcone`: nothing in the plane, where its 2D stub returns zero;
/// nonlinear as a solid, so no role there.
pub static INVERSE_ZCONE: InverseDef = InverseDef {
    name: "zcone",
    kernel: InverseKernel::Affine(|_, _, space| {
        matches!(space, Space::Planar).then_some(AffineRole::Nothing)
    }),
};
