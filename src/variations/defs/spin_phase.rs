//! Spin / pre / post phase variations
//!
//! Four pre/post variations that apply a transform directly to the
//! input or accumulator without going through the outer multiplier:
//!
//!   - `pre_spin_z`     — pre-phase rotation by `w · π/2`
//!   - `post_spin_z`    — post-phase rotation by `w · π/2`
//!   - `post_spherical` (Scott Draves)   — post-phase spherical inversion `r = w / (p²)`
//!   - `pre_disc3d`     (gossamer light) — pre-phase disc with explicit
//!                                          z output `vv · r · cos(z)`,
//!                                          1 user param `pi`
//!                                          (configurable, default π).
//!                                          Uses `atan2(x, y)` to match
//!                                          JWildfire's `PreDisc3DFunc`
//!                                          which passes args in that
//!                                          order (i.e. `atan2(y_arg=x,
//!                                          x_arg=y)`).
//!
//! All four use `needs_transform: true` to read the per-variation weight
//! and apply it directly inside the body (pre/post phases have no outer
//! multiplier).
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// pre_spin_z: pre-phase rotation by `w · π/2` around Z
//   sina = sin(w · π/2);  cosa = cos(w · π/2)
//   out = (sina · y + cosa · x, cosa · y − sina · x, z)
// =============================================================================
/// Pre-phase Z-axis rotation — applies a rotation by `w · π/2` around the Z
/// axis to the input before any normal-phase variations run. Useful as a
/// per-transform pre-rotation step driven by the variation weight.
pub static PRE_SPIN_Z: VariationDef = VariationDef {
    name: "pre_spin_z",
    aliases: &[],
    display_name: "Pre Spin Z",
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
fn variation_pre_spin_z(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let angle = w * pi_2;
    let sina = sin(angle);
    let cosa = cos(angle);
    return vec2<f32>(sina * p.y + cosa * p.x, cosa * p.y - sina * p.x);
}
"#,
    wgsl_3d: r#"
fn variation_pre_spin_z(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let angle = w * pi_2;
    let sina = sin(angle);
    let cosa = cos(angle);
    return vec3<f32>(sina * p.y + cosa * p.x, cosa * p.y - sina * p.x, p.z);
}
"#,
};

// =============================================================================
// post_spin_z: post-phase rotation by `w · π/2` around Z
//   Same body as pre_spin_z, applied at post phase.
// =============================================================================
/// Post-phase Z-axis rotation — same rotation as `pre_spin_z` (`w · π/2`
/// around Z), applied after all normal-phase variations.
pub static POST_SPIN_Z: VariationDef = VariationDef {
    name: "post_spin_z",
    aliases: &[],
    display_name: "Post Spin Z",
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
fn variation_post_spin_z(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let angle = w * pi_2;
    let sina = sin(angle);
    let cosa = cos(angle);
    return vec2<f32>(sina * p.y + cosa * p.x, cosa * p.y - sina * p.x);
}
"#,
    wgsl_3d: r#"
fn variation_post_spin_z(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let angle = w * pi_2;
    let sina = sin(angle);
    let cosa = cos(angle);
    return vec3<f32>(sina * p.y + cosa * p.x, cosa * p.y - sina * p.x, p.z);
}
"#,
};

// =============================================================================
// post_spherical: post-phase spherical inversion (Scott Draves)
//   r = w / (x² + y² + ε)
//   out = (x · r, y · r)
// =============================================================================
/// Post-phase spherical inversion — applies `r = w / (x² + y² + ε)` then
/// `(x·r, y·r)` to the accumulated output. Scott Draves's classic
/// `spherical` variation, applied in post-phase rather than as a normal
/// variation.
///
/// # Authors
/// - Scott Draves
pub static POST_SPHERICAL: VariationDef = VariationDef {
    name: "post_spherical",
    aliases: &[],
    display_name: "Post Spherical",
    category: VariationCategory::Advanced2D,
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
fn variation_post_spherical(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let r = w / (p.x * p.x + p.y * p.y + 1e-30);
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: r#"
fn variation_post_spherical(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let r = w / (p.x * p.x + p.y * p.y + 1e-30);
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#,
};

// =============================================================================
// pre_disc3d (gossamer light)
//   r = sqrt(x² + y² + ε)
//   a = pi · r
//   sr = sin(a);  cr = cos(a)
//   vv = w · atan2(x, y) / (pi + ε)
//   out = (vv · sr, vv · cr, vv · r · cos(z))
// 1 user param `pi` (default π — yes, weirdly user-configurable).
//
// Matches JWildfire `PreDisc3DFunc.java` exactly: Java calls
// `atan2(pAffineTP.x, pAffineTP.y)` (i.e. y-arg = x, x-arg = y, giving
// the swapped-angle convention), and we do the same. The doc-comment
// previously claimed Java used `atan2(y, x)`; that was wrong.
// =============================================================================
/// Pre-phase 3D disc warp — applies a disc-style warp with an explicit Z
/// output `vv · r · cos(z)`. The `pi` parameter is user-configurable
/// (default π) and supplies the divisor in the `vv = w · atan2(x, y) / pi`
/// term, which makes it a non-standard variation where the meaning of π is
/// itself tunable.
///
/// # Authors
/// - gossamer light
pub static PRE_DISC3D: VariationDef = VariationDef {
    name: "pre_disc3d",
    aliases: &[],
    display_name: "Pre Disc 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[
        param!("pi", "Pi", unlimited_float, 3.141592653589793, -10.0, 10.0, "Divisor in the `vv = w · atan2(x, y) / pi` term. Default is the literal value of π."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_disc3d(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let pi_p = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let r = sqrt(p.y * p.y + p.x * p.x + 1e-30);
    let a = pi_p * r;
    let sr = sin(a);
    let cr = cos(a);
    let vv = w * atan2(p.x, p.y) / (pi_p + 1e-30);
    return vec2<f32>(vv * sr, vv * cr);
}
"#,
    wgsl_3d: r#"
fn variation_pre_disc3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let pi_p = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let r = sqrt(p.y * p.y + p.x * p.x + 1e-30);
    let a = pi_p * r;
    let sr = sin(a);
    let cr = cos(a);
    let vv = w * atan2(p.x, p.y) / (pi_p + 1e-30);
    return vec3<f32>(vv * sr, vv * cr, vv * r * cos(p.z));
}
"#,
};
