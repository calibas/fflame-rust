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
//! fly-through-hyperbolic-space.
//!
//! Models are EXPLICIT about dimension — six 2D charts and five 3D
//! charts in one list, freely mixable in 3D render mode:
//! - 2D: **Poincaré Disk**, **Klein Disk**, **Half-Plane** (height =
//!   y), **Hyperboloid/Gans**, **Equidistant** (azimuthal — Euclidean
//!   radius = hyperbolic distance, no boundary crush), **Band** (the
//!   conformal strip, log of the half-plane — infinite frieze ribbons).
//! - 3D: **Poincaré Ball**, **Klein Ball**, **Half-Space** (height =
//!   z, the Kleinian convention), **Hyperboloid/Gans**, **Equidistant**.
//!
//! Cross-dimension rules (3D render mode):
//! - 3D in → 2D out: the H3 point projects geodesically onto the
//!   equatorial H2 plane for the chart, and the dropped spatial
//!   component is emitted as output z — a "2D projection of 3D
//!   hyperbolic content" that keeps the off-plane structure as depth.
//! - 2D in → any out: xy reads through the 2D chart and embeds
//!   equatorially with input z as the third hyperboloid coordinate —
//!   planar tilings become positionable H3 objects (tilt with rot_x,
//!   lift with tz).
//! In 2D render mode, 3D chart selections demote to their 2D
//! counterparts (Ball → Disk, Half-Space → Half-Plane, …).
//!
//! Internally everything routes through the hyperboloid (Minkowski)
//! model — the same `R^{n,1}` machinery as `honeycomb` — where
//! isometries are exact: rotations act on the spatial slice, hyperbolic
//! translations are Lorentz boosts (rapidity = hyperbolic distance).
//! Input points outside a ball model are pulled in by inversion (the
//! honeycomb convention) rather than clamped.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Hyperbolic camera: convert between explicit 2D/3D hyperbolic models
/// (Poincaré, Klein, half-plane/space, hyperboloid, equidistant, band)
/// and apply a hyperbolic isometry — fly-through-hyperbolic-space as a
/// final transform; 2D charts of 3D content keep depth as z.
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
        param!("in_model", "In Model", enum, 0, &["Poincaré Disk (2D)", "Klein Disk (2D)", "Half-Plane (2D)", "Hyperboloid (2D)", "Equidistant (2D)", "Band (2D)", "Poincaré Ball (3D)", "Klein Ball (3D)", "Half-Space (3D)", "Hyperboloid (3D)", "Equidistant (3D)"], "How the incoming point is interpreted. 2D charts read xy (input z passes in as the third hyperboloid coordinate — planar content becomes a positionable H3 object). 3D charts read the full point. In 2D render mode, 3D selections demote to their 2D counterparts. Half-Plane height = y; Half-Space height = z (the Kleinian H3 space-mode convention); Hyperboloid is the raw Minkowski spatial part (Gans model); Equidistant has Euclidean radius = true hyperbolic distance; Band is the conformal strip (log half-plane)."),
        param!("out_model", "Out Model", enum, 0, &["Poincaré Disk (2D)", "Klein Disk (2D)", "Half-Plane (2D)", "Hyperboloid (2D)", "Equidistant (2D)", "Band (2D)", "Poincaré Ball (3D)", "Klein Ball (3D)", "Half-Space (3D)", "Hyperboloid (3D)", "Equidistant (3D)"], "The model the point is re-projected into. Selecting a 2D chart in 3D render mode projects the H3 point geodesically onto the equatorial plane for the chart and emits the dropped spatial component as z — a 2D projection of 3D hyperbolic content that keeps depth. Poincaré → Klein straightens tiling edges into chords; → Half-Plane/Space unrolls against a boundary; → Hyperboloid gives the unbounded Gans funnel; → Equidistant keeps distances radially true; → Band unrolls into an infinite conformal frieze ribbon."),
        param!("tx", "Translate X", unlimited_float, 0.0, -4.0, 4.0, "Hyperbolic translation along x, in units of hyperbolic distance (rapidity of the Lorentz boost). Moving the observer: the scene slides toward the opposite boundary, crowding conformally."),
        param!("ty", "Translate Y", unlimited_float, 0.0, -4.0, 4.0, "Hyperbolic translation along y."),
        param!("tz", "Translate Z", unlimited_float, 0.0, -4.0, 4.0, "Hyperbolic translation along z (3D render mode only). With a 2D in-chart this lifts the planar content off the equatorial plane."),
        param!("rot_x", "Rotate X", angle, 0.0, "Rotation about the x axis through the model center (3D only). With a 2D in-chart this tilts the planar content through H3."),
        param!("rot_y", "Rotate Y", angle, 0.0, "Rotation about the y axis through the model center (3D only)."),
        param!("rot_z", "Rotate Z", angle, 0.0, "Rotation about the z axis (the in-plane rotation in 2D)."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Model scale in world units: input is divided by it, output multiplied — ball models occupy radius `size`."),
        param!("curvature", "Curvature K", unlimited_float, -1.0, -100.0, -0.01, "Sectional curvature the camera experiences (K < 0; content is interpreted at K = -1). The content's intrinsic scale is locked to the curvature radius 1/sqrt(-K) while the camera's ruler stays fixed — the exact radial rescale d' = d/sqrt(-K) of hyperbolic distance about the OBSERVER (applied after the isometry, so the camera position is the fixed point). K = -100: the horizon crowds inward, objects shrink faster, area growth explodes. K = -0.01: the world flattens toward Euclidean — content huddles at the model center, parallels barely diverge, the horizon recedes. Only possible because hyperbolic geometry has an intrinsic scale — flat space has no such dial."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Unified chart indices: 0 Poincaré, 1 Klein, 2 half-plane/space,
// 3 hyperboloid (Gans), 4 equidistant, 5 band (2D only); entries 6..10
// are the 3D charts (index − 6 in the 3D chart functions).
// Order of operations: in-chart → hyperboloid → rotate (Rz·Ry·Rx) →
// boost (tx, ty, tz — one boost along the combined direction, rapidity
// = |t|) → out-chart.
const WGSL_2D: &str = r#"
// 2D charts on the H2 hyperboloid (spatial xy | time z), mdot ++-.
fn hycam_to_hyp2(u_in: vec2<f32>, model: u32) -> vec3<f32> {
    var u = u_in;
    if (model == 0u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let den = max(1.0 - dot(u, u), 1e-6);
        return vec3<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
    }
    if (model == 1u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let s = sqrt(max(1.0 - dot(u, u), 1e-9));
        return vec3<f32>(u / s, 1.0 / s);
    }
    if (model == 2u) {
        let w = max(abs(u.y), 1e-6);
        let hx = u.x / w;
        let s2 = u.x * u.x + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec3<f32>(hx, t - 1.0 / w, t);
    }
    if (model == 4u) {
        let d = length(u);
        if (d < 1e-9) { return vec3<f32>(0.0, 0.0, 1.0); }
        return vec3<f32>((sinh(d) / d) * u, cosh(d));
    }
    if (model == 5u) {
        let by = clamp(u.y, -1.5507, 1.5507);
        let ex = exp(clamp(u.x, -20.0, 20.0));
        let hp = ex * vec2<f32>(-sin(by), cos(by));
        let w = max(hp.y, 1e-6);
        let s2 = hp.x * hp.x + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec3<f32>(hp.x / w, t - 1.0 / w, t);
    }
    // Hyperboloid / Gans: spatial part given, time reconstructed.
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
    if (model == 4u) {
        let sn = length(h.xy);
        if (sn < 1e-9) { return vec2<f32>(0.0, 0.0); }
        return (acosh(t) / sn) * h.xy;
    }
    if (model == 5u) {
        let iw = max(t - h.y, 1e-6);
        let hp = vec2<f32>(h.x / iw, 1.0 / iw);
        return vec2<f32>(0.5 * log(max(dot(hp, hp), 1e-20)), atan2(hp.y, hp.x) - 1.5707963);
    }
    return h.xy;
}

fn variation_hyperbolic_camera(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    var in_model = u32(get_param(xform_id, variation_id, 0u));
    var out_model = u32(get_param(xform_id, variation_id, 1u));
    // 2D render mode: 3D chart selections demote to their 2D versions.
    if (in_model >= 6u) { in_model = in_model - 6u; }
    if (out_model >= 6u) { out_model = out_model - 6u; }
    let tx = get_param(xform_id, variation_id, 2u);
    let ty = get_param(xform_id, variation_id, 3u);
    let rot_z = get_param(xform_id, variation_id, 7u) * 0.01745329252;
    let size = max(get_param(xform_id, variation_id, 8u), 1e-6);
    let curv = sqrt(clamp(-get_param(xform_id, variation_id, 9u), 1e-4, 1e4));

    var h = hycam_to_hyp2(p / size, in_model);
    if (rot_z != 0.0) {
        let cz = cos(rot_z); let sz = sin(rot_z);
        h = vec3<f32>(cz * h.x - sz * h.y, sz * h.x + cz * h.y, h.z);
    }
    let d = sqrt(tx * tx + ty * ty);
    if (d > 1e-9) {
        let n = vec2<f32>(tx, ty) / d;
        let par = dot(h.xy, n);
        let perp = h.xy - par * n;
        let ch = cosh(d); let sh = sinh(d);
        h = vec3<f32>(perp + (ch * par + sh * h.z) * n, sh * par + ch * h.z);
    }
    // Curvature morph: radial rescale of hyperbolic distance about the
    // observer (origin), d' = d*sqrt(-K). Not an isometry — the point.
    if (abs(curv - 1.0) > 1e-6) {
        let sn = length(h.xy);
        if (sn > 1e-9) {
            let dd = min(acosh(max(h.z, 1.0)) / curv, 40.0);
            h = vec3<f32>((sinh(dd) / sn) * h.xy, cosh(dd));
        }
    }
    return hycam_from_hyp2(h, out_model) * size;
}
"#;

const WGSL_3D: &str = r#"
// 2D charts on the H2 hyperboloid (spatial xy | time z) — used for the
// cross-dimension paths.
fn hycam_to_hyp2(u_in: vec2<f32>, model: u32) -> vec3<f32> {
    var u = u_in;
    if (model == 0u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let den = max(1.0 - dot(u, u), 1e-6);
        return vec3<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
    }
    if (model == 1u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let s = sqrt(max(1.0 - dot(u, u), 1e-9));
        return vec3<f32>(u / s, 1.0 / s);
    }
    if (model == 2u) {
        let w = max(abs(u.y), 1e-6);
        let hx = u.x / w;
        let s2 = u.x * u.x + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec3<f32>(hx, t - 1.0 / w, t);
    }
    if (model == 4u) {
        let d = length(u);
        if (d < 1e-9) { return vec3<f32>(0.0, 0.0, 1.0); }
        return vec3<f32>((sinh(d) / d) * u, cosh(d));
    }
    if (model == 5u) {
        let by = clamp(u.y, -1.5507, 1.5507);
        let ex = exp(clamp(u.x, -20.0, 20.0));
        let hp = ex * vec2<f32>(-sin(by), cos(by));
        let w = max(hp.y, 1e-6);
        let s2 = hp.x * hp.x + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec3<f32>(hp.x / w, t - 1.0 / w, t);
    }
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
    if (model == 4u) {
        let sn = length(h.xy);
        if (sn < 1e-9) { return vec2<f32>(0.0, 0.0); }
        return (acosh(t) / sn) * h.xy;
    }
    if (model == 5u) {
        let iw = max(t - h.y, 1e-6);
        let hp = vec2<f32>(h.x / iw, 1.0 / iw);
        return vec2<f32>(0.5 * log(max(dot(hp, hp), 1e-20)), atan2(hp.y, hp.x) - 1.5707963);
    }
    return h.xy;
}

// 3D charts on the H3 hyperboloid (spatial xyz | time w), c = index − 6.
fn hycam_to_hyp3(u_in: vec3<f32>, c: u32) -> vec4<f32> {
    var u = u_in;
    if (c == 0u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let den = max(1.0 - dot(u, u), 1e-6);
        return vec4<f32>(2.0 * u / den, (1.0 + dot(u, u)) / den);
    }
    if (c == 1u) {
        let r2 = dot(u, u);
        if (r2 >= 1.0) { u = u / (r2 + 1e-9); }
        let s = sqrt(max(1.0 - dot(u, u), 1e-9));
        return vec4<f32>(u / s, 1.0 / s);
    }
    if (c == 2u) {
        let w = max(abs(u.z), 1e-6);
        let hx = u.xy / w;
        let s2 = dot(u.xy, u.xy) + w * w;
        let t = (1.0 + s2) / (2.0 * w);
        return vec4<f32>(hx, t - 1.0 / w, t);
    }
    if (c == 4u) {
        let d = length(u);
        if (d < 1e-9) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
        return vec4<f32>((sinh(d) / d) * u, cosh(d));
    }
    return vec4<f32>(u, sqrt(1.0 + dot(u, u)));
}

fn hycam_from_hyp3(h: vec4<f32>, c: u32) -> vec3<f32> {
    let t = max(h.w, 1.0);
    if (c == 0u) { return h.xyz / (1.0 + t); }
    if (c == 1u) { return h.xyz / t; }
    if (c == 2u) {
        let iw = max(t - h.z, 1e-6);
        return vec3<f32>(h.xy / iw, 1.0 / iw);
    }
    if (c == 4u) {
        let sn = length(h.xyz);
        if (sn < 1e-9) { return vec3<f32>(0.0, 0.0, 0.0); }
        return (acosh(t) / sn) * h.xyz;
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
    let curv = sqrt(clamp(-get_param(xform_id, variation_id, 9u), 1e-4, 1e4));

    let u = p / size;
    var h: vec4<f32>;
    if (in_model < 6u) {
        // 2D in-chart: read xy through the 2D chart, embed equatorially
        // with input z as the third hyperboloid coordinate — planar
        // content becomes a positionable H3 object.
        let h2 = hycam_to_hyp2(u.xy, in_model);
        h = vec4<f32>(h2.x, h2.y, u.z, sqrt(1.0 + h2.x * h2.x + h2.y * h2.y + u.z * u.z));
    } else {
        h = hycam_to_hyp3(u, in_model - 6u);
    }

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
        h = vec4<f32>(perp + (ch * par + sh * h.w) * n, sh * par + ch * h.w);
    }
    // Curvature morph: radial rescale about the observer, d' = d*sqrt(-K).
    if (abs(curv - 1.0) > 1e-6) {
        let sn = length(h.xyz);
        if (sn > 1e-9) {
            let dd = min(acosh(max(h.w, 1.0)) / curv, 40.0);
            h = vec4<f32>((sinh(dd) / sn) * h.xyz, cosh(dd));
        }
    }

    if (out_model < 6u) {
        // 2D out-chart: geodesic projection onto the equatorial H2
        // plane for the chart; the dropped spatial component is emitted
        // as z — a 2D projection of 3D hyperbolic content with depth.
        let t2 = sqrt(1.0 + h.x * h.x + h.y * h.y);
        let out2 = hycam_from_hyp2(vec3<f32>(h.x, h.y, t2), out_model);
        return vec3<f32>(out2, h.z) * size;
    }
    return hycam_from_hyp3(h, out_model - 6u) * size;
}
"#;
