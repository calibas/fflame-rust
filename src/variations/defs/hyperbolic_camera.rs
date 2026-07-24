//! `hyperbolic_camera` — hyperbolic model conversions + isometry, for
//! use as a final transform (original).
//!
//! The hyperbolic sibling of [`quaternion_camera`](super::quaternion_camera):
//! interprets the incoming point as a point of hyperbolic space in one
//! MODEL, applies an isometry of hyperbolic space (the "observer
//! position" — hyperbolic translations and rotations, which the flame's
//! Euclidean affines cannot express), and re-projects into another
//! model. On a final transform over `honeycomb`, `von_dyck`,
//! `surface_group`, the hypertiles, or any Kleinian, this is
//! fly-through-hyperbolic-space: translating the camera slides the
//! tessellation through the model in the characteristic
//! everything-crowds-to-the-boundary way.
//!
//! Internally everything routes through the hyperboloid (Minkowski)
//! model — the same `R^{n,1}` machinery as `honeycomb` — where
//! isometries are exact: rotations act on the spatial slice, hyperbolic
//! translations are Lorentz boosts (rapidity = hyperbolic distance).
//!
//! Models (the enum adapts to the render dimension):
//! - **Poincaré** — conformal ball/disk, |p| < 1. The native output of
//!   the Möbius-family variations.
//! - **Beltrami–Klein** — projective ball/disk: geodesics are straight
//!   chords (tilings look "straight-edged"), heavy boundary crush.
//! - **Half-Space/Plane** — upper half-space (height = z in 3D, y in
//!   2D): the Kleinian H3 space-mode convention; ∞ is a boundary point.
//! - **Hyperboloid** — the raw Minkowski spatial part (unbounded
//!   funnel; τ is dropped on output and reconstructed on input as
//!   √(1+|x|²)).
//!
//! Input points outside an input ball model are pulled in by inversion
//! (the honeycomb convention) rather than clamped.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Hyperbolic camera: convert between H2/H3 models (Poincaré, Klein,
/// half-space, hyperboloid) and apply a hyperbolic isometry — the
/// fly-through-hyperbolic-space final transform.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static HYPERBOLIC_CAMERA: VariationDef = VariationDef {
    name: "hyperbolic_camera",
    aliases: &[],
    display_name: "Hyperbolic Camera",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // AlwaysZ: the 3D body writes z unconditionally (model coordinates
    // must survive preserve_z = false).
    features: &[Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("in_model", "In Model", enum, 0, &["Poincaré", "Beltrami-Klein", "Half-Space", "Hyperboloid", "Planar Disk"], "How the incoming point is interpreted as a point of hyperbolic space. Poincaré: conformal ball/disk (the Möbius-family native). Beltrami-Klein: projective ball/disk. Half-Space: upper half-space, height = z (3D) or y (2D) — the Kleinian H3 convention. Hyperboloid: raw Minkowski spatial part. Planar Disk (3D): xy is a 2D Poincaré DISK and z is discarded — feeds a planar-space hyperbolic variation (e.g. von_dyck Planar) in as a flat tiling on the ball's equatorial plane, ready to be tilted and boosted through H3."),
        param!("out_model", "Out Model", enum, 0, &["Poincaré", "Beltrami-Klein", "Half-Space", "Hyperboloid"], "The model the point is re-projected into for output. Converting Poincaré → Klein straightens tiling edges into chords; → Half-Space unrolls the disk against a boundary plane; → Hyperboloid gives the unbounded funnel."),
        param!("tx", "Translate X", unlimited_float, 0.0, -4.0, 4.0, "Hyperbolic translation along x, in units of hyperbolic distance (rapidity of the Lorentz boost). Moving the observer: the scene slides toward the opposite boundary, crowding conformally."),
        param!("ty", "Translate Y", unlimited_float, 0.0, -4.0, 4.0, "Hyperbolic translation along y."),
        param!("tz", "Translate Z", unlimited_float, 0.0, -4.0, 4.0, "Hyperbolic translation along z (3D render mode only)."),
        param!("rot_x", "Rotate X", angle, 0.0, "Rotation about the x axis through the model center (3D only)."),
        param!("rot_y", "Rotate Y", angle, 0.0, "Rotation about the y axis through the model center (3D only)."),
        param!("rot_z", "Rotate Z", angle, 0.0, "Rotation about the z axis (the in-plane rotation in 2D)."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Model scale in world units: input is divided by it, output multiplied — ball models occupy radius `size`."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Order of operations: input model → hyperboloid → rotate (Rz·Ry·Rx) →
// boost (tx, ty, tz as one boost along the combined direction, rapidity
// = |t|) → output model.
const WGSL_2D: &str = r#"
// Hyperboloid vectors carried as (spatial xy | time z), mdot signature ++-.
fn hycam_to_hyp2(u_in: vec2<f32>, model: u32) -> vec3<f32> {
    var u = u_in;
    if (model == 0u || model == 4u) {
        // Poincaré disk (pull outside points in by inversion).
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let den = max(1.0 - dot(u, u), 1e-6);
        return vec3<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
    }
    if (model == 1u) {
        // Beltrami-Klein disk.
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let s = sqrt(max(1.0 - dot(u, u), 1e-9));
        return vec3<f32>(u / s, 1.0 / s);
    }
    if (model == 2u) {
        // Upper half-plane, height = y (see honeycomb's projection 2).
        let w = max(abs(u.y), 1e-6);
        let hx = u.x / w;
        let s2 = u.x * u.x + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec3<f32>(hx, t - 1.0 / w, t);
    }
    // Hyperboloid: spatial part given, time reconstructed.
    return vec3<f32>(u, sqrt(1.0 + dot(u, u)));
}

fn hycam_from_hyp2(h: vec3<f32>, model: u32) -> vec2<f32> {
    let t = max(h.z, 1.0);
    if (model == 0u) { return h.xy / (1.0 + t); }
    if (model == 1u) { return h.xy / t; }
    if (model == 2u) {
        let iw = max(t - h.y, 1e-6);
        return vec2<f32>(h.x / iw, 1.0 / iw);
    }
    return h.xy;
}

fn variation_hyperbolic_camera(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let in_model = u32(get_param(xform_id, variation_id, 0u));
    let out_model = u32(get_param(xform_id, variation_id, 1u));
    let tx = get_param(xform_id, variation_id, 2u);
    let ty = get_param(xform_id, variation_id, 3u);
    let rot_z = get_param(xform_id, variation_id, 7u) * 0.01745329252;
    let size = max(get_param(xform_id, variation_id, 8u), 1e-6);

    var h = hycam_to_hyp2(p / size, in_model);
    // Rotate in the spatial plane.
    if (rot_z != 0.0) {
        let cz = cos(rot_z); let sz = sin(rot_z);
        h = vec3<f32>(cz * h.x - sz * h.y, sz * h.x + cz * h.y, h.z);
    }
    // Boost along (tx, ty), rapidity |t|: spatial-parallel and time mix.
    let d = sqrt(tx * tx + ty * ty);
    if (d > 1e-9) {
        let n = vec2<f32>(tx, ty) / d;
        let par = dot(h.xy, n);
        let perp = h.xy - par * n;
        let ch = cosh(d); let sh = sinh(d);
        let par2 = ch * par + sh * h.z;
        let t2 = sh * par + ch * h.z;
        h = vec3<f32>(perp + par2 * n, t2);
    }
    return hycam_from_hyp2(h, out_model) * size;
}
"#;

const WGSL_3D: &str = r#"
// Hyperboloid vectors carried as (spatial xyz | time w), signature +++-.
fn hycam_to_hyp3(u_in: vec3<f32>, model: u32) -> vec4<f32> {
    var u = u_in;
    if (model == 4u) { u.z = 0.0; }
    if (model == 0u || model == 4u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let den = max(1.0 - dot(u, u), 1e-6);
        return vec4<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
    }
    if (model == 1u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let s = sqrt(max(1.0 - dot(u, u), 1e-9));
        return vec4<f32>(u / s, 1.0 / s);
    }
    if (model == 2u) {
        // Upper half-space, height = z; the vertical axis maps to the
        // LAST spatial slot of the hyperboloid (same pattern as the
        // half-plane, one dimension up).
        let w = max(abs(u.z), 1e-6);
        let hx = u.xy / w;
        let s2 = dot(u.xy, u.xy) + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec4<f32>(hx, t - 1.0 / w, t);
    }
    return vec4<f32>(u, sqrt(1.0 + dot(u, u)));
}

fn hycam_from_hyp3(h: vec4<f32>, model: u32) -> vec3<f32> {
    let t = max(h.w, 1.0);
    if (model == 0u) { return h.xyz / (1.0 + t); }
    if (model == 1u) { return h.xyz / t; }
    if (model == 2u) {
        let iw = max(t - h.z, 1e-6);
        return vec3<f32>(h.xy / iw, 1.0 / iw);
    }
    return h.xyz;
}

fn variation_hyperbolic_camera(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let in_model = u32(get_param(xform_id, variation_id, 0u));
    let out_model = u32(get_param(xform_id, variation_id, 1u));
    let tx = get_param(xform_id, variation_id, 2u);
    let ty = get_param(xform_id, variation_id, 3u);
    let tz = get_param(xform_id, variation_id, 4u);
    let rot_x = get_param(xform_id, variation_id, 5u) * 0.01745329252;
    let rot_y = get_param(xform_id, variation_id, 6u) * 0.01745329252;
    let rot_z = get_param(xform_id, variation_id, 7u) * 0.01745329252;
    let size = max(get_param(xform_id, variation_id, 8u), 1e-6);

    var h = hycam_to_hyp3(p / size, in_model);
    // Spatial rotations Rz · Ry · Rx about the model center.
    if (rot_x != 0.0) {
        let c = cos(rot_x); let s = sin(rot_x);
        h = vec4<f32>(h.x, c * h.y - s * h.z, s * h.y + c * h.z, h.w);
    }
    if (rot_y != 0.0) {
        let c = cos(rot_y); let s = sin(rot_y);
        h = vec4<f32>(c * h.x + s * h.z, h.y, -s * h.x + c * h.z, h.w);
    }
    if (rot_z != 0.0) {
        let c = cos(rot_z); let s = sin(rot_z);
        h = vec4<f32>(c * h.x - s * h.y, s * h.x + c * h.y, h.z, h.w);
    }
    // Boost along (tx, ty, tz), rapidity = length.
    let d = sqrt(tx * tx + ty * ty + tz * tz);
    if (d > 1e-9) {
        let n = vec3<f32>(tx, ty, tz) / d;
        let par = dot(h.xyz, n);
        let perp = h.xyz - par * n;
        let ch = cosh(d); let sh = sinh(d);
        let par2 = ch * par + sh * h.w;
        let t2 = sh * par + ch * h.w;
        h = vec4<f32>(perp + par2 * n, t2);
    }
    return hycam_from_hyp3(h, out_model) * size;
}
"#;
