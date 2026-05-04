//! `rosoni` (DarkBeam, July 2014)
//!
//! Rosoni — a clipping/rotational pattern that takes the input
//! point's polar position relative to a "cutoff" radius and either
//! passes through (outside cutoff) or runs an N-step rotation loop
//! that XOR-toggles a "cerc" flag based on whether the rotated point
//! falls inside an inner shape (circle/square/lemniscate/angle).
//! The output point is taken at iteration `sweetiter` if the toggle
//! ends `true`, with sign tweaks for sweetiter == 0.
//!
//!   - 7 user params: maxiter (int), sweetiter (int), altshapes (int),
//!                    cutoff, radius, dx, dy
//!   - 2 init slots: sina = sin(2π / maxiter);  cosa = cos(2π / maxiter)
//!
//! Body has VVAR factor cleanly on every output line — clean factor
//! through outer.
//!
//! Source: `output/jwildfire-vars/output/rosoni.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static ROSONI: VariationDef = VariationDef {
    name: "rosoni",
    display_name: "Rosoni",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("maxiter", "Max Iter", int, 25.0, 1.0, 100.0),
        param!("sweetiter", "Sweet Iter", int, 3.0, 0.0, 100.0),
        param!("altshapes", "Alt Shapes", int, 0.0, 0.0, 1.0),
        param!("cutoff", "Cutoff", unlimited_float, 1.0, -10.0, 10.0),
        param!("radius", "Radius", unlimited_float, 0.4, -10.0, 10.0),
        param!("dx", "DX", unlimited_float, 0.6, -10.0, 10.0),
        param!("dy", "DY", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 7..9: sina, cosa
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_rosoni(user: array<f32, 7>) -> array<f32, 2> {
    let two_pi = 6.28318530717959;
    let phi = two_pi / max(user[0], 1.0);
    var out: array<f32, 2>;
    out[0] = sin(phi);
    out[1] = cos(phi);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_rosoni(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let maxiter_f = max(get_param(xform_id, variation_id, 0u), 1.0);
    let sweetiter = i32(get_param(xform_id, variation_id, 1u));
    let altshapes = i32(get_param(xform_id, variation_id, 2u));
    let cutoff = get_param(xform_id, variation_id, 3u);
    let radius = get_param(xform_id, variation_id, 4u);
    let dx = get_param(xform_id, variation_id, 5u);
    let dy = get_param(xform_id, variation_id, 6u);
    let sina = get_param(xform_id, variation_id, 7u);
    let cosa = get_param(xform_id, variation_id, 8u);
    let inv_pi = 0.31830988618379;

    var r = sqrt(p.x * p.x + p.y * p.y) - cutoff;
    if (cutoff < 0.0) {
        r = max(abs(p.x), abs(p.y)) + cutoff;
    }
    if (r > 0.0) {
        return p;
    }

    var cerc = dx > 0.0;
    var xrt = p.x;
    var yrt = p.y;
    var sweetx = xrt;
    var sweety = yrt;
    let maxiter_i = i32(maxiter_f);
    for (var i: i32 = 0; i < maxiter_i; i = i + 1) {
        var r2: f32;
        if (altshapes == 0) {
            let dxx = xrt - dx;
            let dyy = yrt - dy;
            r2 = dxx * dxx + dyy * dyy - radius * radius;
            if (radius < 0.0) {
                r2 = max(abs(xrt - dx), abs(p.y - dy)) + radius;
            }
        } else {
            let dxx = xrt - dx;
            let dyy = yrt - dy;
            r2 = select((dyy * dyy) - dxx * dxx * (radius * radius - dxx * dxx),
                        -(xrt - dx),
                        (xrt - dx) < 0.0);
            if (radius < 0.0) {
                r2 = abs(atan2(p.y - dy, dxx)) * inv_pi + radius;
            }
        }
        if (r2 <= 0.0) {
            cerc = !cerc;
        }
        if (i == sweetiter) {
            sweetx = xrt;
            sweety = yrt;
        }
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
    }

    if (cerc) {
        if (sweetiter == 0) {
            let fx = select(sweetx, -sweetx, dy != 0.0);
            return vec2<f32>(fx, -sweety);
        }
        return vec2<f32>(sweetx, sweety);
    }
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_rosoni(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let maxiter_f = max(get_param(xform_id, variation_id, 0u), 1.0);
    let sweetiter = i32(get_param(xform_id, variation_id, 1u));
    let altshapes = i32(get_param(xform_id, variation_id, 2u));
    let cutoff = get_param(xform_id, variation_id, 3u);
    let radius = get_param(xform_id, variation_id, 4u);
    let dx = get_param(xform_id, variation_id, 5u);
    let dy = get_param(xform_id, variation_id, 6u);
    let sina = get_param(xform_id, variation_id, 7u);
    let cosa = get_param(xform_id, variation_id, 8u);
    let inv_pi = 0.31830988618379;

    var r = sqrt(p.x * p.x + p.y * p.y) - cutoff;
    if (cutoff < 0.0) {
        r = max(abs(p.x), abs(p.y)) + cutoff;
    }
    if (r > 0.0) {
        return p;
    }

    var cerc = dx > 0.0;
    var xrt = p.x;
    var yrt = p.y;
    var sweetx = xrt;
    var sweety = yrt;
    let maxiter_i = i32(maxiter_f);
    for (var i: i32 = 0; i < maxiter_i; i = i + 1) {
        var r2: f32;
        if (altshapes == 0) {
            let dxx = xrt - dx;
            let dyy = yrt - dy;
            r2 = dxx * dxx + dyy * dyy - radius * radius;
            if (radius < 0.0) {
                r2 = max(abs(xrt - dx), abs(p.y - dy)) + radius;
            }
        } else {
            let dxx = xrt - dx;
            let dyy = yrt - dy;
            r2 = select((dyy * dyy) - dxx * dxx * (radius * radius - dxx * dxx),
                        -(xrt - dx),
                        (xrt - dx) < 0.0);
            if (radius < 0.0) {
                r2 = abs(atan2(p.y - dy, dxx)) * inv_pi + radius;
            }
        }
        if (r2 <= 0.0) {
            cerc = !cerc;
        }
        if (i == sweetiter) {
            sweetx = xrt;
            sweety = yrt;
        }
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
    }

    if (cerc) {
        if (sweetiter == 0) {
            let fx = select(sweetx, -sweetx, dy != 0.0);
            return vec3<f32>(fx, -sweety, p.z);
        }
        return vec3<f32>(sweetx, sweety, p.z);
    }
    return p;
}
"#),
};
