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
//! spins the point along a great circle of S³; composed with `quaternion_julia`
//! / `quaternion_linear` (or the affine + multiple transforms) it twists the 4D
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
    // WritesRgb: optional w-driven brightness/saturation shading. AlwaysZ: z
    // is written unconditionally (the rotation mixes all four components);
    // without it preserve_z = false re-flattens z each step and the 4D
    // rotation stops tracing great circles of S³.
    features: &[Feature::NeedsW, Feature::WritesColor, Feature::WritesRgb, Feature::AlwaysZ],
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
        param!("w_bright", "Brightness by W", unlimited_float, 0.0, -2.0, 2.0, "0 = off. Scales the sample's palette color by (1 + w_bright*w): positive = high-w structure glows brighter (feeds the Glow post-effect nicely), negative = it dims. Hue-preserving; 3D only. Needs the transform's direct_color > 0."),
        param!("w_sat", "Saturation by W", unlimited_float, 0.0, -2.0, 2.0, "0 = off. Shifts the sample's color saturation by (1 + w_sat*w) around its luminance: negative w_sat washes high-w structure toward gray, >1 total over-saturates. Hue-preserving; 3D only. Needs the transform's direct_color > 0."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_rotation(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    // vrc unused in 2D: w-shading (w_bright/w_sat) is a 4D-only effect.
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

fn variation_quaternion_rotation(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
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

    // w-shading: brightness / saturation of the palette color scaled by w
    // (see quaternion_julia for the contract).
    let wb = get_param(xform_id, variation_id, 6u);
    let ws = get_param(xform_id, variation_id, 7u);
    if (abs(wb) > 1e-6 || abs(ws) > 1e-6) {
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(clamp(*vc, 0.0, 1.0), 0.5), 0.0).rgb;
        var col = srgb_to_linear(srgb) * clamp(1.0 + wb * r.w, 0.0, 4.0);
        let luma = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
        col = mix(vec3<f32>(luma), col, clamp(1.0 + ws * r.w, 0.0, 2.0));
        *vrc = col;
    }

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
