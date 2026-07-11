//! `quaternion_camera` — a **4D camera** for the quaternion variations.
//!
//! The engine camera is strictly 3D: it acts on the plotted point *after* the
//! 4D→3D projection, so it can never move or look along `w`. This variation is
//! the missing half: it reads the full 4D point `(p.xyz, point_w)` and applies
//! a camera transform in quaternion space —
//!
//! 1. translate by the 4D eye position (`cam_x/y/z/w` — animate **`cam_w`** to
//!    fly along the 4th axis),
//! 2. rotate the three planes the 3D camera cannot reach (`rot_xw`, `rot_yw`,
//!    `rot_zw`; the ordinary xy/xz/yz rotations stay with the engine camera),
//! 3. project: `p′ = q.xyz / (1 − persp·q.w)` — a pinhole eye on the w axis
//!    (`persp = 0` is the orthographic drop-w). Points **behind the 4D eye**
//!    (`1 − persp·w < 1e-3`) are hidden (`CanHide`), the same clip convention
//!    as the engine's 3D perspective — so flying `cam_w` through an object
//!    makes the parts you pass vanish, exactly like a real fly-through.
//!
//! **Use it on a FINAL transform** (weight 1.0). Finals save/restore `point_w`
//! and don't feed `xyz` forward, so there it is a pure plot-time camera: the 4D
//! attractor's dynamics are untouched, matching engine-camera semantics. On a
//! normal transform it also feeds its output back into the walk (documented
//! projection-mode behavior, occasionally interesting, not a camera).
//!
//! 2D mode is an identity pass-through (there is no `w` in the 2D pipeline).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUATERNION_CAMERA: VariationDef = VariationDef {
    name: "quaternion_camera",
    aliases: &["qcamera"],
    display_name: "Quaternion Camera (4D)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // NeedsW: reads the walk's 4th coordinate. CanHide: behind-the-eye clip.
    // AlwaysZ: writes z unconditionally (camera-space z must survive
    // preserve_z = false).
    features: &[Feature::NeedsW, Feature::CanHide, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cam_x", "Camera X", unlimited_float, 0.0, -4.0, 4.0, "4D eye position, x/i component (subtracted from the point before rotation/projection)."),
        param!("cam_y", "Camera Y", unlimited_float, 0.0, -4.0, 4.0, "4D eye position, y/j component."),
        param!("cam_z", "Camera Z", unlimited_float, 0.0, -4.0, 4.0, "4D eye position, z/k component."),
        param!("cam_w", "Camera W", unlimited_float, 0.0, -4.0, 4.0, "4D eye position along the 4th axis — THE slider this variation exists for. Sweep it to fly through the object in w; with Perspective > 0, parts you pass are clipped away like a real fly-through."),
        param!("rot_xw", "Rotate XW", unlimited_float, 0.0, -3.1416, 3.1416, "Rotation of the (x, w) plane in radians — turns the view into the 4th dimension (a '4D yaw'). The ordinary xy/xz/yz rotations belong to the engine's 3D camera."),
        param!("rot_yw", "Rotate YW", unlimited_float, 0.0, -3.1416, 3.1416, "Rotation of the (y, w) plane in radians (a '4D pitch')."),
        param!("rot_zw", "Rotate ZW", unlimited_float, 0.0, -3.1416, 3.1416, "Rotation of the (z, w) plane in radians."),
        param!("persp", "Perspective", float, 1.0, 0.0, 4.0, "4D perspective strength: p' = xyz / (1 - persp*w) after the eye transform — a pinhole on the w axis. 0 = orthographic (drop w). Higher = stronger w-foreshortening and a closer eye; points behind the eye are hidden."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quaternion_camera(p: vec2<f32>, xform_id: u32, variation_id: u32, hide: ptr<function, bool>) -> vec2<f32> {
    // No w in the 2D pipeline — the 4D camera is an identity there.
    return p;
}
"#;

const WGSL_3D: &str = r#"
fn variation_quaternion_camera(p: vec3<f32>, xform_id: u32, variation_id: u32, hide: ptr<function, bool>) -> vec3<f32> {
    // Full 4D point relative to the 4D eye.
    var q = vec4<f32>(p, point_w) - vec4<f32>(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u)
    );

    // The three w-plane rotations (xw, yw, zw) — the ones a 3D camera lacks.
    let axw = get_param(xform_id, variation_id, 4u);
    let cxw = cos(axw); let sxw = sin(axw);
    q = vec4<f32>(cxw * q.x + sxw * q.w, q.y, q.z, cxw * q.w - sxw * q.x);
    let ayw = get_param(xform_id, variation_id, 5u);
    let cyw = cos(ayw); let syw = sin(ayw);
    q = vec4<f32>(q.x, cyw * q.y + syw * q.w, q.z, cyw * q.w - syw * q.y);
    let azw = get_param(xform_id, variation_id, 6u);
    let czw = cos(azw); let szw = sin(azw);
    q = vec4<f32>(q.x, q.y, czw * q.z + szw * q.w, czw * q.w - szw * q.z);

    point_w_out = q.w;   // camera-space w (finals restore the walk's w anyway)

    // Pinhole on the w axis; persp = 0 → orthographic drop-w.
    let persp = get_param(xform_id, variation_id, 7u);
    if (persp < 1e-6) { return q.xyz; }
    let denom = 1.0 - persp * q.w;
    if (denom < 1e-3) {
        // Behind the 4D eye — same clip convention as the 3D projection.
        *hide = true;
        return q.xyz;
    }
    return q.xyz / denom;
}
"#;
