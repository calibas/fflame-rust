//! `minkowski_camera` — the spacetime observer: full Poincaré-group
//! positioning plus a choice of how the 4th (time) coordinate folds
//! into the 3D plot (original).
//!
//! The final-transform twin of [`minkowski`](super::minkowski): that
//! variation is CONTENT (interval-driven maps + causal coloring on
//! normal transforms, where direct color works); this one is the
//! CAMERA — no color params (finals don't affect color, matching
//! JWF/Apophysis), just the 10-parameter isometry group of Minkowski
//! space and a projection:
//!
//! - **Translations** (tx, ty, tz, tw): spacetime position of the
//!   observer — tw slides the "now".
//! - **Rotations** (rot_x/y/z): the spatial orientation.
//! - **Boosts** (bx, by, bz): the observer's velocity — rapidity
//!   vector, speed = tanh(|b|). A moving spacetime camera literally
//!   sees relativistic aberration: content crowds toward the
//!   direction of motion.
//!
//! W-fold modes (how (xyz, w) becomes a plottable point):
//! - **Drop W**: orthographic spatial slice.
//! - **W to Z**: stack time along z (z += strength·w) — worldlines
//!   become towers; boost-swirled content shows its causal twist.
//! - **Time Perspective**: divide space by (1 − strength·w) — the
//!   time-axis analogue of the z-perspective divide; the future
//!   magnifies, the past recedes.
//! - **Retarded**: light-arrival view — an event at radius r, time w
//!   is seen at radius r + strength·w (arrival time of its light at
//!   the origin observer). strength = 1 is the physical backward
//!   light cone; negative radii fold through the origin.
//! - **Interval Radial**: radius becomes the proper interval √|q| —
//!   the light cone collapses to the origin and causal shells become
//!   spheres.
//!
//! 2D render mode is the R^{1,1} camera (y timelike): tx/tw
//! translate, bx boosts, and the w-fold modes act on (x, t).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Minkowski camera: Poincaré-group observer positioning (spacetime
/// translation, rotation, boost/velocity) + W-fold projection modes
/// (drop / stack / time-perspective / retarded light-arrival /
/// interval-radial). Designed for final transforms; no color params.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static MINKOWSKI_CAMERA: VariationDef = VariationDef {
    name: "minkowski_camera",
    aliases: &[],
    display_name: "Minkowski Camera",
    // Advanced2D, not Full3D: Full3D-category variations are dropped
    // from 2D shaders entirely, and the 2D body is the real R^{1,1}
    // camera (see `minkowski`).
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsW, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("w_mode", "W Fold", enum, 0, &["Drop W", "W to Z", "Time Perspective", "Retarded", "Interval Radial"], "How the 4D spacetime point folds into the 3D plot. Drop W: orthographic spatial slice. W to Z: stack time along z (z += Strength*w) — causal structure becomes height. Time Perspective: divide space by (1 - Strength*w), the time-axis analogue of the perspective divide — future magnifies, past recedes. Retarded: light-arrival view, radius r + Strength*w (Strength 1 = the physical backward light cone; negative radii fold through the origin). Interval Radial: radius = sqrt(|interval|) — the light cone collapses to the origin, causal shells become spheres."),
        param!("tx", "Translate X", unlimited_float, 0.0, -4.0, 4.0, "Observer position x (subtracted before rotation/boost)."),
        param!("ty", "Translate Y", unlimited_float, 0.0, -4.0, 4.0, "Observer position y."),
        param!("tz", "Translate Z", unlimited_float, 0.0, -4.0, 4.0, "Observer position z (3D)."),
        param!("tw", "Translate W", unlimited_float, 0.0, -4.0, 4.0, "Observer position in TIME — slides which 'now' the camera sits at; with Retarded or Time Perspective folding this scrubs through the content's causal history."),
        param!("rot_x", "Rotate X", angle, 0.0, "Spatial rotation about x (3D)."),
        param!("rot_y", "Rotate Y", angle, 0.0, "Spatial rotation about y (3D)."),
        param!("rot_z", "Rotate Z", angle, 0.0, "Spatial rotation about z."),
        param!("bx", "Boost X", float, 0.0, -3.0, 3.0, "Observer velocity: rapidity vector x component. Speed = tanh of the vector's length; direction = the vector. A moving camera sees relativistic aberration — content crowds toward the direction of motion. In 2D this is the boost along x."),
        param!("by", "Boost Y", float, 0.0, -3.0, 3.0, "Rapidity vector y component."),
        param!("bz", "Boost Z", float, 0.0, -3.0, 3.0, "Rapidity vector z component (3D)."),
        param!("strength", "Strength", float, 1.0, -4.0, 4.0, "Intensity of the selected W Fold (W to Z stack height, Time Perspective strength, Retarded light-delay factor)."),
        param!("size", "Size", float, 1.0, 0.05, 4.0, "Output scale."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_minkowski_camera(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w_mode = u32(get_param(xform_id, variation_id, 0u));
    let tx = get_param(xform_id, variation_id, 1u);
    let tw = get_param(xform_id, variation_id, 4u);
    let bx = get_param(xform_id, variation_id, 8u);
    let strength = get_param(xform_id, variation_id, 11u);
    let size = get_param(xform_id, variation_id, 12u);

    // R^{1,1}: x spacelike, y timelike.
    var x = p.x - tx;
    var t = p.y - tw;
    if (bx != 0.0) {
        let ch = cosh(bx); let sh = sinh(bx);
        let nx = x * ch + t * sh;
        t = x * sh + t * ch;
        x = nx;
    }

    if (w_mode == 2u) {
        // Time perspective on the single spatial axis.
        var dn = 1.0 - strength * t;
        if (abs(dn) < 0.05) { dn = select(0.05, -0.05, dn < 0.0); }
        x = x / dn;
    } else if (w_mode == 3u) {
        // Retarded: |x| + s*t at the point's side; folds through 0.
        x = sign(x) * (abs(x) + strength * t);
    } else if (w_mode == 4u) {
        // Interval radial: |x| -> sqrt(|q|).
        let q = x * x - t * t;
        x = sign(x) * sqrt(abs(q));
    }
    // Drop W / W to Z: identity in 2D (t IS the second axis).

    return vec2<f32>(x, t) * size;
}
"#;

const WGSL_3D: &str = r#"
fn variation_minkowski_camera(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w_mode = u32(get_param(xform_id, variation_id, 0u));
    let tx = get_param(xform_id, variation_id, 1u);
    let ty = get_param(xform_id, variation_id, 2u);
    let tz = get_param(xform_id, variation_id, 3u);
    let tw = get_param(xform_id, variation_id, 4u);
    let rx = get_param(xform_id, variation_id, 5u) * 0.01745329252;
    let ry = get_param(xform_id, variation_id, 6u) * 0.01745329252;
    let rz = get_param(xform_id, variation_id, 7u) * 0.01745329252;
    let bv = vec3<f32>(
        get_param(xform_id, variation_id, 8u),
        get_param(xform_id, variation_id, 9u),
        get_param(xform_id, variation_id, 10u));
    let strength = get_param(xform_id, variation_id, 11u);
    let size = get_param(xform_id, variation_id, 12u);

    // Spacetime translation: move the observer to the origin.
    var s = p - vec3<f32>(tx, ty, tz);
    var t = point_w - tw;

    // Spatial rotations Rz * Ry * Rx.
    if (rx != 0.0) {
        let c = cos(rx); let sn = sin(rx);
        s = vec3<f32>(s.x, c * s.y - sn * s.z, sn * s.y + c * s.z);
    }
    if (ry != 0.0) {
        let c = cos(ry); let sn = sin(ry);
        s = vec3<f32>(c * s.x + sn * s.z, s.y, -sn * s.x + c * s.z);
    }
    if (rz != 0.0) {
        let c = cos(rz); let sn = sin(rz);
        s = vec3<f32>(c * s.x - sn * s.y, sn * s.x + c * s.y, s.z);
    }

    // Boost: the observer's velocity (rapidity = |bv|). Relativistic
    // aberration falls out of the frame change.
    let rap = length(bv);
    if (rap > 1e-6) {
        let n = bv / rap;
        let par = dot(s, n);
        let ch = cosh(rap); let sh = sinh(rap);
        let np = par * ch + t * sh;
        t = par * sh + t * ch;
        s = s + (np - par) * n;
    }

    // W fold.
    if (w_mode == 1u) {
        s.z = s.z + strength * t;
    } else if (w_mode == 2u) {
        var dn = 1.0 - strength * t;
        if (abs(dn) < 0.05) { dn = select(0.05, -0.05, dn < 0.0); }
        s = s / dn;
    } else if (w_mode == 3u) {
        let r = length(s);
        if (r > 1e-9) {
            s = (s / r) * (r + strength * t);
        }
    } else if (w_mode == 4u) {
        let r = length(s);
        if (r > 1e-9) {
            let q = r * r - t * t;
            s = (s / r) * sqrt(abs(q));
        }
    }

    point_w_out = t;
    return s * size;
}
"#;
