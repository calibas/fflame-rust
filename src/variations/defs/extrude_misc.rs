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
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static EXTRUDE: VariationDef = VariationDef {
    name: "extrude",
    display_name: "Extrude",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("root_face", "Root Face", unlimited_float, 0.5, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_extrude(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let _ = rng_nextf(rng);
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
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
"#),
};
