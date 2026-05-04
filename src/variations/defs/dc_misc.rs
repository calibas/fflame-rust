//! Direct-color variations: dc_cylinder, dc_cylinder2, dc_triangle
//!
//! Three FracFx / Apophysis "dc_*" variations. Each writes a color
//! value (TC) — those writes are skipped per writes_color compromise
//! established in batch 60 (spirograph3D). The spatial transforms are
//! ported faithfully.
//!
//!   - dc_cylinder (FracFx): 6 user (offset, angle, scale, x, y, blur)
//!     + 2 init slots (_ldcs = 1/scale, _ldca = offset·π). Body has
//!     `FPy += rr + FTy·y` with no VVAR scaling — needs_transform
//!     divide-out for the y component. cpp's persistent `_r[4]` state
//!     replaced with fresh randoms per call (same compromise as
//!     exblur/farblur).
//!
//!   - dc_cylinder2 (FracFx): 6 user + 2 init slots. Same structure
//!     as dc_cylinder but `FPy += rr·FTy·y` (no `+rr` constant).
//!     Same divide-out + fresh-randoms compromise.
//!
//!   - dc_triangle (Apophysis): barycentric-coordinate triangle
//!     mapping using the transform's affine as triangle vertices.
//!     2 user (scatter_area, zero_edges int) + 1 init (_A =
//!     clamp(scatter_area, -1, 1)). RNG. Body factors cleanly.
//!     Reads xf.a/b/c/d/e/f via needs_transform.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/dc_cylinder.cpp`
//!   - `output/jwildfire-vars/output/dc_cylinder2.cpp`
//!   - `output/jwildfire-vars/output/dc_triangle.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// dc_cylinder
// ---------------------------------------------------------------------------

pub static DC_CYLINDER: VariationDef = VariationDef {
    name: "dc_cylinder",
    display_name: "DC Cylinder",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("offset", "Offset", unlimited_float, 0.0, -10.0, 10.0),
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0),
        param!("scale", "Scale", unlimited_float, 0.5, -10.0, 10.0),
        param!("x", "X", unlimited_float, 0.125, -10.0, 10.0),
        param!("y", "Y", unlimited_float, 0.125, -10.0, 10.0),
        param!("blur", "Blur", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_dc_cylinder(user: array<f32, 6>) -> array<f32, 2> {
    var out: array<f32, 2>;
    let pi = 3.14159265358979;
    let scale = user[2];
    let safe_scale = select(scale, 1e-5, abs(scale) < 1e-30);
    out[0] = 1.0 / safe_scale;
    out[1] = user[0] * pi;
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_dc_cylinder(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr + p.y * yp) * inv_w;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn variation_dc_cylinder(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let cr = cos(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr + p.y * yp) * inv_w;
    let nz = cos(p.x + rr * cr);
    return vec3<f32>(nx, ny, nz);
}
"#),
};

// ---------------------------------------------------------------------------
// dc_cylinder2
// ---------------------------------------------------------------------------

pub static DC_CYLINDER2: VariationDef = VariationDef {
    name: "dc_cylinder2",
    display_name: "DC Cylinder 2",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("offset", "Offset", unlimited_float, 0.0, -10.0, 10.0),
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0),
        param!("scale", "Scale", unlimited_float, 0.5, -10.0, 10.0),
        param!("x", "X", unlimited_float, 0.125, -10.0, 10.0),
        param!("y", "Y", unlimited_float, 0.125, -10.0, 10.0),
        param!("blur", "Blur", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_dc_cylinder2(user: array<f32, 6>) -> array<f32, 2> {
    var out: array<f32, 2>;
    let pi = 3.14159265358979;
    let scale = user[2];
    let safe_scale = select(scale, 1e-5, abs(scale) < 1e-30);
    out[0] = 1.0 / safe_scale;
    out[1] = user[0] * pi;
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_dc_cylinder2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr * p.y * yp) * inv_w;
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn variation_dc_cylinder2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xp = get_param(xform_id, variation_id, 3u);
    let yp = get_param(xform_id, variation_id, 4u);
    let blur = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let a = rng_nextf(rng) * two_pi;
    let sr = sin(a);
    let cr = cos(a);
    let r0 = rng_nextf(rng);
    let r1 = rng_nextf(rng);
    let r2 = rng_nextf(rng);
    let r3 = rng_nextf(rng);
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);

    let nx = sin(p.x + rr * sr) * xp;
    let ny = (rr * p.y * yp) * inv_w;
    let nz = cos(p.x + rr * cr);
    return vec3<f32>(nx, ny, nz);
}
"#),
};

// ---------------------------------------------------------------------------
// dc_triangle
// ---------------------------------------------------------------------------

pub static DC_TRIANGLE: VariationDef = VariationDef {
    name: "dc_triangle",
    display_name: "DC Triangle",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("scatter_area", "Scatter Area", unlimited_float, 0.0, -1.0, 1.0),
        param!("zero_edges", "Zero Edges", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_dc_triangle(user: array<f32, 2>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = clamp(user[0], -1.0, 1.0);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_dc_triangle(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let zero_edges = i32(get_param(xform_id, variation_id, 1u));
    let a_param = get_param(xform_id, variation_id, 2u);
    let xf = transforms[xform_id];
    let xx = xf.a;
    let xy = xf.b;
    let yx = -xf.c;
    let yy = -xf.d;
    let ox = xf.e;
    let oy = xf.f;
    let px = p.x - ox;
    let py = p.y - oy;

    let dot00 = xx * xx + xy * xy;
    let dot01 = xx * yx + xy * yy;
    let dot02 = xx * px + xy * py;
    let dot11 = yx * yx + yy * yy;
    let dot12 = yx * px + yy * py;

    let denom = dot00 * dot11 - dot01 * dot01;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var u = (dot11 * dot02 - dot01 * dot12) / safe_denom;
    var v = (dot00 * dot12 - dot01 * dot02) / safe_denom;
    var inside = false;
    var f = 1.0;

    if (u + v > 1.0) {
        f = -1.0;
        if (u > v) {
            u = min(u, 1.0);
            v = 1.0 - u;
        } else {
            v = min(v, 1.0);
            u = 1.0 - v;
        }
    } else if (u < 0.0 || v < 0.0) {
        u = clamp(u, 0.0, 1.0);
        v = clamp(v, 0.0, 1.0);
    } else {
        inside = true;
    }

    if (zero_edges != 0 && !inside) {
        u = 0.0;
        v = 0.0;
    } else if (!inside) {
        u = u + rng_nextf(rng) * a_param * f;
        v = v + rng_nextf(rng) * a_param * f;
        u = clamp(u, -1.0, 1.0);
        v = clamp(v, -1.0, 1.0);
        if (u + v > 1.0 && a_param > 0.0) {
            if (u > v) {
                u = min(u, 1.0);
                v = 1.0 - u;
            } else {
                v = min(v, 1.0);
                u = 1.0 - v;
            }
        }
    }

    return vec2<f32>(ox + u * xx + v * yx, oy + u * xy + v * yy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_dc_triangle(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let zero_edges = i32(get_param(xform_id, variation_id, 1u));
    let a_param = get_param(xform_id, variation_id, 2u);
    let xf = transforms[xform_id];
    let xx = xf.a;
    let xy = xf.b;
    let yx = -xf.c;
    let yy = -xf.d;
    let ox = xf.e;
    let oy = xf.f;
    let px = p.x - ox;
    let py = p.y - oy;

    let dot00 = xx * xx + xy * xy;
    let dot01 = xx * yx + xy * yy;
    let dot02 = xx * px + xy * py;
    let dot11 = yx * yx + yy * yy;
    let dot12 = yx * px + yy * py;

    let denom = dot00 * dot11 - dot01 * dot01;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var u = (dot11 * dot02 - dot01 * dot12) / safe_denom;
    var v = (dot00 * dot12 - dot01 * dot02) / safe_denom;
    var inside = false;
    var f = 1.0;

    if (u + v > 1.0) {
        f = -1.0;
        if (u > v) {
            u = min(u, 1.0);
            v = 1.0 - u;
        } else {
            v = min(v, 1.0);
            u = 1.0 - v;
        }
    } else if (u < 0.0 || v < 0.0) {
        u = clamp(u, 0.0, 1.0);
        v = clamp(v, 0.0, 1.0);
    } else {
        inside = true;
    }

    if (zero_edges != 0 && !inside) {
        u = 0.0;
        v = 0.0;
    } else if (!inside) {
        u = u + rng_nextf(rng) * a_param * f;
        v = v + rng_nextf(rng) * a_param * f;
        u = clamp(u, -1.0, 1.0);
        v = clamp(v, -1.0, 1.0);
        if (u + v > 1.0 && a_param > 0.0) {
            if (u > v) {
                u = min(u, 1.0);
                v = 1.0 - u;
            } else {
                v = min(v, 1.0);
                u = 1.0 - v;
            }
        }
    }

    return vec3<f32>(ox + u * xx + v * yx, oy + u * xy + v * yy, p.z);
}
"#),
};
