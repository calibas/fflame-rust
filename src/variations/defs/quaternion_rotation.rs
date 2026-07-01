//! `quaternion_rotation` — 4D rotation of the point by `q' = â·q`.
//!
//! Left-multiplies the running 4D point `q = (x, y, z, w)` (vector part = the
//! 3D point, scalar part = `point_w` via `Feature::NeedsW`) by a constant
//! **unit** quaternion `â = normalize(ax, ay, az, aw)`. Left multiplication by
//! a unit quaternion is a rotation of R⁴ (a *left-isoclinic* rotation), so this
//! genuinely mixes all four components — unlike a 3D conjugation `â·q·â⁻¹`,
//! which would rotate only `(x,y,z)` and leave `w` untouched.
//!
//! Not a fractal on its own — it's a 4D building block. Repeatedly applying it
//! spins the point along a great circle of S³; composed with the Julia /
//! cubic variations (or the affine + multiple transforms) it twists the 4D
//! attractor before projection. For a general 4D rotation `â·q·b̂`, chain this
//! with a right-multiplying sibling (easy to add later).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUATERNION_ROTATION: VariationDef = VariationDef {
    name: "quaternion_rotation",
    aliases: &["qrotate"],
    display_name: "Quaternion Rotation",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // NeedsW: reads/writes the 4th coordinate (the rotation mixes it). No RNG —
    // the rotation is deterministic. WritesColor: optional w-driven palette.
    features: &[Feature::NeedsW, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("ax", "Axis X", float, 0.3, -1.0, 1.0, "Vector-i component of the rotation quaternion a (normalized at runtime)."),
        param!("ay", "Axis Y", float, 0.0, -1.0, 1.0, "Vector-j component of a."),
        param!("az", "Axis Z", float, 0.0, -1.0, 1.0, "Vector-k component of a."),
        param!("aw", "Scalar W", float, 0.95, -1.0, 1.0, "Scalar component of a. With the vector part, sets the rotation angle (aw = cos(θ/2))."),
        param!("projection", "Projection", unlimited_int, 0.0, 0.0, 2.0, "How the rotated 4D point maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z), 2 = Perspective (divide xyz by 1-w)."),
        param!("w_color", "Color by W", float, 0.0, 0.0, 8.0, "0 = off. >0 = write a palette index from the 4th coordinate (3D: fract(w * scale); 2D: fract(|z| * scale)), revealing it as COLOR. Needs the transform's direct_color > 0."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_rotation(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    // 2D: rotate (x, y) by the angle implied by (ax, aw) — a plain 2D rotation
    // (the 4D rotation needs all three spatial axes). `point_w` rides unused.
    let ax = get_param(xform_id, variation_id, 0u);
    let aw = get_param(xform_id, variation_id, 3u);
    let ang = 2.0 * atan2(ax, aw);          // full angle from the half-angle quat
    let cs = cos(ang);
    let sn = sin(ang);
    let out = vec2<f32>(p.x * cs - p.y * sn, p.x * sn + p.y * cs);
    let wcol = get_param(xform_id, variation_id, 5u);
    if (wcol > 1e-6) { *vc = fract(length(out) * wcol); }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn qrot_qmul(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z
    );
}

fn variation_quaternion_rotation(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let a_raw = vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );
    // Unit quaternion → rotation. Guard the all-zero case (fall back to identity).
    let a = normalize(a_raw + vec4<f32>(0.0, 0.0, 0.0, 1e-9));
    let r = qrot_qmul(a, vec4<f32>(p, point_w));   // â · q  (left-isoclinic 4D rotation)

    let wcol = get_param(xform_id, variation_id, 5u);
    if (wcol > 1e-6) { *vc = fract(r.w * wcol); }

    // Project the rotated 4D point (see quaternion_julia for the caveat).
    let mode = u32(get_param(xform_id, variation_id, 4u) + 0.5);
    switch (mode) {
        case 1u: {
            point_w_out = r.z;
            return vec3<f32>(r.x, r.y, r.w);
        }
        case 2u: {
            point_w_out = r.w;
            let denom = 1.0 - r.w;
            let safe = select(denom, 1e-3, abs(denom) < 1e-3);
            return r.xyz / safe;
        }
        default: {
            point_w_out = r.w;
            return r.xyz;
        }
    }
}
"#;
