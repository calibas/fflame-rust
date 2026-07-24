//! `quaternion_linear` — a full **4D affine** on the point `q = (x, y, z, w)`.
//!
//! `q' = M·q + t`, where `M` is a user-specified 4×4 matrix and `t` a 4-vector
//! (20 degrees of freedom). Unlike the engine's built-in affine — which is a
//! *3D* map (`a,b,c,d,e,f` + the `yz`/`zx` plane coefs) and never touches the
//! 4th coordinate — this exposes the whole 4×4, including the **`w`-coupling**
//! the built-in affine structurally can't express: the 4th row (`w'` depends on
//! `x,y,z`) and 4th column (`x,y,z` depend on `w`, via `point_w` /
//! `Feature::NeedsW`).
//!
//! Defaults to the identity (a no-op) so it does nothing until you edit the
//! entries. Params are row-major: `m00..m03` (row 0) … `m30..m33` (row 3), then
//! `tx,ty,tz,tw`. 2D mode uses the top-left 2×2 plus `(tx, ty)`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// A full 4D affine q' = M·q + t on the point q = (x,y,z,w).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static QUATERNION_LINEAR: VariationDef = VariationDef {
    name: "quaternion_linear",
    aliases: &["qlinear"],
    display_name: "Quaternion Linear",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // AlwaysZ: row 2 (z') is computed unconditionally; without it
    // preserve_z = false zeroes z each iteration, silently disabling the m2x
    // row and the z couplings this variation exists to expose. WritesRgb:
    // optional w-driven brightness/saturation shading.
    features: &[Feature::NeedsW, Feature::WritesColor, Feature::WritesRgb, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        // Row 0 → x'
        param!("m00", "M00 (x'<-x)", unlimited_float, 1.0, -2.0, 2.0, "Row 0 of the 4x4 matrix M — the coefficients producing x'. m03 is the w->x coupling the 3D affine can't express."),
        param!("m01", "M01 (x'<-y)", unlimited_float, 0.0, -2.0, 2.0, "x' coefficient on y."),
        param!("m02", "M02 (x'<-z)", unlimited_float, 0.0, -2.0, 2.0, "x' coefficient on z."),
        param!("m03", "M03 (x'<-w)", unlimited_float, 0.0, -2.0, 2.0, "x' coefficient on w (4th-column coupling; needs 3D)."),
        // Row 1 → y'
        param!("m10", "M10 (y'<-x)", unlimited_float, 0.0, -2.0, 2.0, "y' coefficient on x."),
        param!("m11", "M11 (y'<-y)", unlimited_float, 1.0, -2.0, 2.0, "y' coefficient on y."),
        param!("m12", "M12 (y'<-z)", unlimited_float, 0.0, -2.0, 2.0, "y' coefficient on z."),
        param!("m13", "M13 (y'<-w)", unlimited_float, 0.0, -2.0, 2.0, "y' coefficient on w."),
        // Row 2 → z'
        param!("m20", "M20 (z'<-x)", unlimited_float, 0.0, -2.0, 2.0, "z' coefficient on x. 3D only."),
        param!("m21", "M21 (z'<-y)", unlimited_float, 0.0, -2.0, 2.0, "z' coefficient on y. 3D only."),
        param!("m22", "M22 (z'<-z)", unlimited_float, 1.0, -2.0, 2.0, "z' coefficient on z. 3D only."),
        param!("m23", "M23 (z'<-w)", unlimited_float, 0.0, -2.0, 2.0, "z' coefficient on w. 3D only."),
        // Row 3 → w'
        param!("m30", "M30 (w'<-x)", unlimited_float, 0.0, -2.0, 2.0, "Row 3 — the w-coupling the built-in affine lacks. w' coefficient on x. 3D only."),
        param!("m31", "M31 (w'<-y)", unlimited_float, 0.0, -2.0, 2.0, "w' coefficient on y. 3D only."),
        param!("m32", "M32 (w'<-z)", unlimited_float, 0.0, -2.0, 2.0, "w' coefficient on z. 3D only."),
        param!("m33", "M33 (w'<-w)", unlimited_float, 1.0, -2.0, 2.0, "w' coefficient on w. 3D only."),
        // Translation
        param!("tx", "Translate X", unlimited_float, 0.0, -2.0, 2.0, "4D offset added to x'."),
        param!("ty", "Translate Y", unlimited_float, 0.0, -2.0, 2.0, "4D offset added to y'."),
        param!("tz", "Translate Z", unlimited_float, 0.0, -2.0, 2.0, "4D offset added to z'. 3D only."),
        param!("tw", "Translate W", unlimited_float, 0.0, -2.0, 2.0, "4D offset added to w'. 3D only."),
        // Display helpers (not part of the affine's 20 DOF).
        param!("projection", "Projection", unlimited_int, 0.0, 0.0, 2.0, "How the 4D result maps to the plotted 3D point (3D only). 0 = Vector (drop w), 1 = Depth (surface w as z), 2 = Perspective (divide xyz by 1-w)."),
        param!("w_color", "Color by W", float, 0.0, 0.0, 8.0, "0 = off. >0 = write a palette index from the 4th coordinate. Needs the transform's direct_color > 0."),
        param!("w_bright", "Brightness by W", unlimited_float, 0.0, -2.0, 2.0, "0 = off. Scales the sample's palette color by (1 + w_bright*w): positive = high-w structure glows brighter (feeds the Glow post-effect nicely), negative = it dims. Hue-preserving; 3D only. Needs the transform's direct_color > 0."),
        param!("w_sat", "Saturation by W", unlimited_float, 0.0, -2.0, 2.0, "0 = off. Shifts the sample's color saturation by (1 + w_sat*w) around its luminance: negative w_sat washes high-w structure toward gray, >1 total over-saturates. Hue-preserving; 3D only. Needs the transform's direct_color > 0."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_linear(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>, vrc: ptr<function, vec3<f32>>) -> vec2<f32> {
    // vrc unused in 2D: w-shading (w_bright/w_sat) is a 4D-only effect.
    // 2D: the top-left 2x2 of M plus (tx, ty).
    let m00 = get_param(xform_id, variation_id, 0u);
    let m01 = get_param(xform_id, variation_id, 1u);
    let m10 = get_param(xform_id, variation_id, 4u);
    let m11 = get_param(xform_id, variation_id, 5u);
    let tx = get_param(xform_id, variation_id, 16u);
    let ty = get_param(xform_id, variation_id, 17u);
    let out = vec2<f32>(m00 * p.x + m01 * p.y + tx, m10 * p.x + m11 * p.y + ty);
    let wcol = get_param(xform_id, variation_id, 21u);
    if (wcol > 1e-6) { *vc = fract(length(out) * wcol); }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn variation_quaternion_linear(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>, vrc: ptr<function, vec3<f32>>) -> vec3<f32> {
    let q = vec4<f32>(p, point_w);
    // Row-major M: each row dotted with q, then + t.
    let row0 = vec4<f32>(get_param(xform_id, variation_id, 0u),  get_param(xform_id, variation_id, 1u),  get_param(xform_id, variation_id, 2u),  get_param(xform_id, variation_id, 3u));
    let row1 = vec4<f32>(get_param(xform_id, variation_id, 4u),  get_param(xform_id, variation_id, 5u),  get_param(xform_id, variation_id, 6u),  get_param(xform_id, variation_id, 7u));
    let row2 = vec4<f32>(get_param(xform_id, variation_id, 8u),  get_param(xform_id, variation_id, 9u),  get_param(xform_id, variation_id, 10u), get_param(xform_id, variation_id, 11u));
    let row3 = vec4<f32>(get_param(xform_id, variation_id, 12u), get_param(xform_id, variation_id, 13u), get_param(xform_id, variation_id, 14u), get_param(xform_id, variation_id, 15u));
    let t = vec4<f32>(get_param(xform_id, variation_id, 16u), get_param(xform_id, variation_id, 17u), get_param(xform_id, variation_id, 18u), get_param(xform_id, variation_id, 19u));
    let r = vec4<f32>(dot(row0, q), dot(row1, q), dot(row2, q), dot(row3, q)) + t;

    let wcol = get_param(xform_id, variation_id, 21u);
    if (wcol > 1e-6) { *vc = fract(r.w * wcol); }

    // w-shading: brightness / saturation of the palette color scaled by w
    // (see quaternion_julia for the contract).
    let wb = get_param(xform_id, variation_id, 22u);
    let ws = get_param(xform_id, variation_id, 23u);
    if (abs(wb) > 1e-6 || abs(ws) > 1e-6) {
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(clamp(*vc, 0.0, 1.0), 0.5), 0.0).rgb;
        var col = srgb_to_linear(srgb) * clamp(1.0 + wb * r.w, 0.0, 4.0);
        let luma = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
        col = mix(vec3<f32>(luma), col, clamp(1.0 + ws * r.w, 0.0, 2.0));
        *vrc = col;
    }

    // Project the 4D result to the plotted/fed-forward 3D point.
    let mode = u32(get_param(xform_id, variation_id, 20u) + 0.5);
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
