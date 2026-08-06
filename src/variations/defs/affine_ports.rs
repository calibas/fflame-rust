//! Variations that depend on affine matrix access
//!
//! These need `needs_transform: true` so the body can read from
//! `transforms[xform_id]`. Before the affine-access plumbing landed, these
//! couldn't be ported faithfully.
//!
//! Currently:
//!   - `popcorn` (Scott Draves) — uses XFORM_COEFF_20/_21 (translation X/Y)
//!     for the per-axis sine displacement amplitude.

use crate::variations::{
    definition::{Feature, VariationDef},
    VariationCategory, VariationPhase,
};

// =============================================================================
// popcorn: Scott Draves's popcorn variation
//   dx = tan(3·y)    (zeroed if non-finite — see the guard in the body)
//   dy = tan(3·x)
//   x' = x + COEFF_20 · sin(dx)
//   y' = y + COEFF_21 · sin(dy)
//
// COEFF_20 = our xform.e (X translation), COEFF_21 = our xform.f (Y).
// =============================================================================
/// Adds sinusoidal displacement to each axis using the transform's affine
/// translation fields (`e` and `f`) as amplitudes. Like Popcorn2 but takes
/// its strength directly from the affine matrix instead of dedicated
/// sliders.
///
/// # Authors
/// - Scott Draves
pub static POPCORN: VariationDef = VariationDef {
    name: "popcorn",
    aliases: &[],
    display_name: "Popcorn",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_popcorn(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xf = transforms[xform_id];
    // Non-finite guard. NOT `dx != dx`: Metal compiles shaders with
    // fast-math on (wgpu never clears `fastMathEnabled`, which defaults
    // to true), so the compiler may assume no NaNs exist and fold a
    // self-compare to a constant false — measured on an M2, where
    // `x != x` returns 0 for a verified NaN. `!(abs(x) <= C)` survives,
    // and is the same idiom main_template.wgsl uses for bad-value
    // recovery. Catches NaN and +/-Inf; tan() cannot reach 1e32 in f32,
    // so finite results are untouched and the behaviour on backends
    // without fast-math is unchanged.
    var dx = tan(3.0 * p.y);
    if (!(abs(dx) <= 1e32)) { dx = 0.0; }
    var dy = tan(3.0 * p.x);
    if (!(abs(dy) <= 1e32)) { dy = 0.0; }
    return vec2<f32>(
        p.x + xf.e * sin(dx),
        p.y + xf.f * sin(dy),
    );
}
"#,
    wgsl_3d: r#"
fn variation_popcorn(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xf = transforms[xform_id];
    // See the 2D body: `dx != dx` is folded away by Metal's fast-math.
    var dx = tan(3.0 * p.y);
    if (!(abs(dx) <= 1e32)) { dx = 0.0; }
    var dy = tan(3.0 * p.x);
    if (!(abs(dy) <= 1e32)) { dy = 0.0; }
    return vec3<f32>(
        p.x + xf.e * sin(dx),
        p.y + xf.f * sin(dy),
        p.z,
    );
}
"#,
};
