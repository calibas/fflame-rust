//! `quaternion_julia` — 4D quaternion Julia map `q' = q² + c`.
//!
//! A starter/experimental 4D variation. The running 3D point supplies the
//! quaternion's vector part `(x, y, z)`; the scalar part `w` lives in the
//! per-thread `point_w` (`Feature::NeedsW`), so the full 4D quaternion
//! survives transform switches across the walk. Each iteration squares the
//! quaternion (Hamilton product) and adds the constant `c = (cx, cy, cz, cw)`.
//!
//! Intended to be used **alone** on a transform at weight 1.0 to start: the
//! flame dispatcher does `result += weight · body(p)`, so at weight 1 the
//! output *is* `q² + c`. Mixing with other variations sums the xyz outputs
//! but not `w` (the deferred combining question — see `Feature::NeedsW`).
//!
//! 2D mode degrades to the complex Julia `z² + (cx + i·cy)` since the
//! quaternion math needs all three spatial axes.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUATERNION_JULIA: VariationDef = VariationDef {
    name: "quaternion_julia",
    aliases: &["qjulia"],
    display_name: "Quaternion Julia",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    // NeedsW: emits the per-thread `point_w` 4th coordinate + resets it on
    // bad-value respawn. No signature change — the body reads/writes the
    // `point_w` global directly.
    features: &[Feature::NeedsW],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cx", "Constant X", float, -0.2, -2.0, 2.0, "Vector-i component of the Julia constant quaternion c."),
        param!("cy", "Constant Y", float, 0.4, -2.0, 2.0, "Vector-j component of c."),
        param!("cz", "Constant Z", float, 0.0, -2.0, 2.0, "Vector-k component of c. 3D only."),
        param!("cw", "Constant W", float, 0.0, -2.0, 2.0, "Scalar component of c (drives the 4th dimension). 3D only."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_julia(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // 2D fallback: complex Julia z^2 + (cx + i*cy). The full quaternion map
    // needs 3D mode; `point_w` rides unused here.
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(p.x * p.x - p.y * p.y + cx, 2.0 * p.x * p.y + cy);
}
"#;

const WGSL_3D: &str = r#"
// Hamilton product. Quaternion convention here: q = (x, y, z, w) where w is
// the scalar part (stored in `point_w`) and (x,y,z) is the vector part (the
// running point).
fn qjulia_qmul(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z
    );
}

fn variation_quaternion_julia(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let q = vec4<f32>(p, point_w);          // 4D point: vector = p, scalar = w
    let q2 = qjulia_qmul(q, q);             // q^2
    let c = vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );
    let r = q2 + c;
    point_w = r.w;                          // carry the 4th coordinate forward
    return r.xyz;
}
"#;
