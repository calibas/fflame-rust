//! `onion2` (Nicolaus Anderson, 2013, Java-recovered)
//!
//! Onion-shaped 3D transform that smoothly transitions a circle into
//! a (negative) exponential curve at a "meeting point", projecting
//! the result onto a sphere.
//!
//! 8 user params (all Java-recovered, cpp APO_VARIABLES empty):
//!   - meeting_pt: where the circle and exp meet (radians)
//!   - circle_a, circle_b: radius scales for circle/exp
//!   - shift_x, shift_y, shift_z: offset (z unused in body)
//!   - top_crop: crops z above this value when using exp branch
//!   - stretch: divides the radius
//!
//! Body has w in `r` and `z` (multiplied by circle_a/b · w), with
//! unscaled `+ shift_x`/`+ shift_y` offsets at the end and a
//! `1/(r²+z²)` term in the second-block fz that has `1/w²` after
//! divide-out. needs_transform divide-out works mathematically; the
//! 1/w factor is bounded by the safe_w guards.
//!
//! Source: `output/jwildfire-vars/output/onion2.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static ONION2: VariationDef = VariationDef {
    name: "onion2",
    display_name: "Onion 2",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("meeting_pt", "Meeting Pt", unlimited_float, 0.5, -10.0, 10.0),
        param!("circle_a", "Circle A", unlimited_float, 1.0, -10.0, 10.0),
        param!("circle_b", "Circle B", unlimited_float, 1.0, -10.0, 10.0),
        param!("shift_x", "Shift X", unlimited_float, 0.0, -10.0, 10.0),
        param!("shift_y", "Shift Y", unlimited_float, 0.0, -10.0, 10.0),
        param!("shift_z", "Shift Z", unlimited_float, 0.0, -10.0, 10.0),
        param!("top_crop", "Top Crop", unlimited_float, 0.0, -10.0, 10.0),
        param!("stretch", "Stretch", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_onion2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let meeting_pt = get_param(xform_id, variation_id, 0u);
    let circle_a = get_param(xform_id, variation_id, 1u);
    let shift_x = get_param(xform_id, variation_id, 3u);
    let shift_y = get_param(xform_id, variation_id, 4u);
    let top_crop = get_param(xform_id, variation_id, 6u);
    let stretch = get_param(xform_id, variation_id, 7u);
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let safe_stretch = select(stretch, 1e-30, abs(stretch) < 1e-30);

    let inix = p.x - shift_x;
    let iniy = p.y - shift_y;
    let sqtr = max(sqrt(inix * inix + iniy * iniy), 1e-30);
    let t = sqtr / safe_stretch - pi_2;

    var r: f32;
    if (t > meeting_pt) {
        r = cos(t);
        let tmpt = tan(meeting_pt);
        if (abs(tmpt) > 1e-30) {
            // Compute the exponential branch z to use for cropping
            let z_local = exp(cos(meeting_pt) - r) / tmpt + sin(meeting_pt) - 1.0 / tmpt;
            if (z_local > top_crop && top_crop > 0.0) {
                r = 0.0;
            }
        }
    } else {
        r = cos(t);
    }
    r = r * circle_a * w;

    let fx = r * (inix / sqtr) + shift_x;
    let fy = r * (iniy / sqtr) + shift_y;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_onion2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let meeting_pt = get_param(xform_id, variation_id, 0u);
    let circle_a = get_param(xform_id, variation_id, 1u);
    let circle_b = get_param(xform_id, variation_id, 2u);
    let shift_x = get_param(xform_id, variation_id, 3u);
    let shift_y = get_param(xform_id, variation_id, 4u);
    let top_crop = get_param(xform_id, variation_id, 6u);
    let stretch = get_param(xform_id, variation_id, 7u);
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let safe_stretch = select(stretch, 1e-30, abs(stretch) < 1e-30);

    let inix = p.x - shift_x;
    let iniy = p.y - shift_y;
    let iniz = p.z;
    let sqtr = max(sqrt(inix * inix + iniy * iniy + iniz * iniz), 1e-30);
    let t = sqtr / safe_stretch - pi_2;

    var r: f32;
    var z: f32;
    if (t > meeting_pt) {
        r = cos(t);
        let tmpt = tan(meeting_pt);
        if (abs(tmpt) > 1e-30) {
            z = exp(cos(meeting_pt) - r) / tmpt + sin(meeting_pt) - 1.0 / tmpt;
            if (z > top_crop && top_crop > 0.0) {
                z = top_crop;
                r = 0.0;
            }
        } else {
            z = top_crop;
        }
    } else {
        r = cos(t);
        z = sin(t);
    }
    r = r * circle_a * w;
    z = z * circle_b * w;

    var fx = r * (inix / sqtr);
    var fy = r * (iniy / sqtr);
    var fz = z;
    fx = fx + r * iniz * (inix / sqtr);
    fy = fy + r * iniz * (iniy / sqtr);
    let denom = max(r * r + z * z, 1e-30);
    fz = fz + iniz * z / denom;
    fx = fx + shift_x;
    fy = fy + shift_y;
    return vec3<f32>(fx * inv_w, fy * inv_w, fz * inv_w);
}
"#),
};
