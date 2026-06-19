//! `extrude` (Xyrus02)
//!
//! Z-only random "extrusion": with probability `root_face` set
//! `FPz = max(w, 0)`; otherwise set `FPz = w · rand(0..1)`. The
//! cpp uses `FPz = …` (assignment) but for Depth3D-style Z-only
//! variations our outer additive model produces equivalent output
//! as long as extrude is the only z-modifier in its transform.
//!
//!   - 1 user param: root_face (default 0.5)
//!
//! cpp's `(rand ^ (rand << 15)) & 0xfffffff / (double)0xfffffff`
//! bit-mixed double-rand pattern produces a uniform [0, 1) sample;
//! we use a single `rng_nextf()` (statistically equivalent uniform
//! draw without the bit-mixing).
//!
//! Source: `output/jwildfire-vars/output/extrude.cpp`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Z-axis random extrusion — with probability `root_face`, sets Z to the
/// variation weight (clamped at 0 for negative weights); otherwise sets Z
/// to a random fraction of the weight. The XY output is zero. Useful as a
/// Z-only depth contribution to give 2D flames a flat Z extrusion.
///
/// # Authors
/// - Xyrus02
pub static EXTRUDE: VariationDef = VariationDef {
    name: "extrude",
    aliases: &[],
    display_name: "Extrude",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::AlwaysZ],
    parameters: &[
        param!("root_face", "Root Face", unlimited_float, 0.5, -10.0, 10.0, "Probability threshold for the `root face` branch — values below it set Z to the (clamped) weight; values above set Z to `weight · rand`."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_extrude(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let _ = rng_nextf(rng);
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_extrude(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let root_face = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    var fz_cpp: f32;
    if (rng_nextf(rng) < root_face) {
        fz_cpp = select(w, 0.0, w < 0.0);
    } else {
        fz_cpp = w * rng_nextf(rng);
    }
    return vec3<f32>(0.0, 0.0, fz_cpp * inv_w);
}
"#,
};
