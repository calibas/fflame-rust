//! Variations that depend on affine matrix access
//!
//! These need `needs_affine: true` so the body can read from
//! `transforms[xform_id]`. Before the affine-access plumbing landed, these
//! couldn't be ported faithfully.
//!
//! Currently:
//!   - `popcorn` (Scott Draves) — uses XFORM_COEFF_20/_21 (translation X/Y)
//!     for the per-axis sine displacement amplitude.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

// =============================================================================
// popcorn: Scott Draves's popcorn variation
//   dx = tan(3·y)    (zeroed if NaN — happens near tan asymptotes)
//   dy = tan(3·x)
//   x' = x + COEFF_20 · sin(dx)
//   y' = y + COEFF_21 · sin(dy)
//
// COEFF_20 = our xform.e (X translation), COEFF_21 = our xform.f (Y).
// =============================================================================
pub static POPCORN: VariationDef = VariationDef {
    name: "popcorn",
    display_name: "Popcorn",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    needs_affine: true,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_popcorn(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    let xf = transforms[xform_id];
    var dx = tan(3.0 * p.y);
    if (dx != dx) { dx = 0.0; }   // NaN guard
    var dy = tan(3.0 * p.x);
    if (dy != dy) { dy = 0.0; }
    return vec2<f32>(
        p.x + xf.e * sin(dx),
        p.y + xf.f * sin(dy),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_popcorn(p: vec3<f32>, xform_id: u32) -> vec3<f32> {
    let xf = transforms[xform_id];
    var dx = tan(3.0 * p.y);
    if (dx != dx) { dx = 0.0; }
    var dy = tan(3.0 * p.x);
    if (dy != dy) { dy = 0.0; }
    return vec3<f32>(
        p.x + xf.e * sin(dx),
        p.y + xf.f * sin(dy),
        p.z,
    );
}
"#),
};
