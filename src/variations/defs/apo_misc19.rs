//! Apophysis miscellany batch 19: mobius_strip, circleLinear
//!
//!   - mobius_strip (slobo777 / chronologicaldot / CozyG): parametric
//!     Möbius-strip surface with twists, width and radial wrap/edge/
//!     hide/leave modes, plus optional rotation about X and Y. 10 user
//!     params (radius, width, twists, range_x, range_y, rotate_x,
//!     rotate_y, modify_z, width_mode, radial_mode) + 4 init slots
//!     (rotxSin, rotxCos, rotySin, rotyCos). Java-recovered (cpp
//!     PluginVarCalc empty unported_stub). Compromises: HIDE branch
//!     returns (0,0,0); LEAVE uses needs_transform divide-out for the
//!     unscaled `+= pAffineTP.x` cpp path; modify_z==0 passes through
//!     z=0 (cpp's accumulator self-amp `+= pAmount * pVarTP.z` cannot
//!     be replicated mid-iteration).
//!
//!   - circleLinear (Xyrus02-style, FracFx pack): grid of "linear
//!     blobs" at each (M, N) lattice cell, with deterministic per-cell
//!     noise selecting between full-linear, scaled, and ring-radius
//!     mappings. 8 user params (Sc, K, Reverse, Dens1, Dens2, X, Y,
//!     Seed; X/Y are unused in body — kept to match cpp). No init.
//!     Pure 2D; Z passes through outer multiplier in 3D shader.
//!
//! (bubble2 already exists in numbered.rs — skipped.)
//!
//! Sources:
//!   - `output/jwildfire-vars/output/mobius_strip.cpp` (Java-recovered)
//!   - `output/jwildfire-vars/output/circlelinear.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// mobius_strip
// ---------------------------------------------------------------------------

pub static MOBIUS_STRIP: VariationDef = VariationDef {
    name: "mobius_strip",
    display_name: "Mobius Strip",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0),
        param!("width", "Width", unlimited_float, 1.0, -10.0, 10.0),
        param!("twists", "Twists", int, 1.0, -10.0, 10.0),
        param!("range_x", "Range X", unlimited_float, 1.0, -10.0, 10.0),
        param!("range_y", "Range Y", unlimited_float, 1.0, -10.0, 10.0),
        param!("rotate_x", "Rotate X", unlimited_float, 0.0, -1.0, 1.0),
        param!("rotate_y", "Rotate Y", unlimited_float, 0.0, -1.0, 1.0),
        param!("modify_z", "Modify Z", unlimited_float, 1.0, -10.0, 10.0),
        param!("width_mode", "Width Mode", int, 0.0, 0.0, 3.0),
        param!("radial_mode", "Radial Mode", int, 0.0, 0.0, 3.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_mobius_strip(user: array<f32, 10>) -> array<f32, 4> {
    let two_pi = 6.28318530717959;
    var out: array<f32, 4>;
    out[0] = sin(user[5] * two_pi);  // rotxSin
    out[1] = cos(user[5] * two_pi);  // rotxCos
    out[2] = sin(user[6] * two_pi);  // rotySin
    out[3] = cos(user[6] * two_pi);  // rotyCos
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_mobius_strip(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let width = get_param(xform_id, variation_id, 1u);
    let twists = get_param(xform_id, variation_id, 2u);
    let range_x = get_param(xform_id, variation_id, 3u);
    let range_y = get_param(xform_id, variation_id, 4u);
    let modify_z = get_param(xform_id, variation_id, 7u);
    let width_mode = i32(get_param(xform_id, variation_id, 8u));
    let radial_mode = i32(get_param(xform_id, variation_id, 9u));
    let rotx_sin = get_param(xform_id, variation_id, 10u);
    let rotx_cos = get_param(xform_id, variation_id, 11u);
    let roty_sin = get_param(xform_id, variation_id, 12u);
    let roty_cos = get_param(xform_id, variation_id, 13u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;

    let xmax = range_x * 0.5;
    let ymax = range_y * 0.5;
    let xmin = -xmax;
    let ymin = -ymax;

    var x = p.x;
    var y = p.y;

    if (radial_mode == 2 && (x > xmax || x < xmin)) {
        return vec2<f32>(0.0, 0.0);
    }
    if (width_mode == 2 && (y > ymax || y < ymin)) {
        return vec2<f32>(0.0, 0.0);
    }
    if (radial_mode == 3 && (x > xmax || x < xmin)) {
        return vec2<f32>(p.x * inv_w, p.y * inv_w);
    }
    if (width_mode == 3 && (y > ymax || y < ymin)) {
        return vec2<f32>(p.x * inv_w, p.y * inv_w);
    }

    var t: f32;
    if (abs(range_x) < 1e-30) {
        t = 0.0;
    } else {
        if (radial_mode == 0) {
            x = x - trunc(x / xmax) * xmax;
        } else if (radial_mode == 1) {
            x = clamp(x, xmin, xmax);
        }
        t = x * pi / xmax + pi;
    }

    var s: f32;
    if (abs(range_y) < 1e-30) {
        s = 0.0;
    } else {
        if (width_mode == 0) {
            y = y - trunc(y / ymax) * ymax;
        } else if (width_mode == 1) {
            y = clamp(y, ymin, ymax);
        }
        s = y * (width * 0.5) / ymax;
    }

    let half_t = twists * t * 0.5;
    let chs = cos(half_t);
    let r_off = radius + s * chs;
    let mx = r_off * cos(t);
    let my = r_off * sin(t);

    let rx = mx;
    let ry = my * roty_cos;
    let rz = -my * roty_sin;

    let mx_out = rx * rotx_cos - rz * rotx_sin;
    let my_out = ry;

    return vec2<f32>(mx_out, my_out);
}
"#,
    wgsl_3d: Some(r#"
fn variation_mobius_strip(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let width = get_param(xform_id, variation_id, 1u);
    let twists = get_param(xform_id, variation_id, 2u);
    let range_x = get_param(xform_id, variation_id, 3u);
    let range_y = get_param(xform_id, variation_id, 4u);
    let modify_z = get_param(xform_id, variation_id, 7u);
    let width_mode = i32(get_param(xform_id, variation_id, 8u));
    let radial_mode = i32(get_param(xform_id, variation_id, 9u));
    let rotx_sin = get_param(xform_id, variation_id, 10u);
    let rotx_cos = get_param(xform_id, variation_id, 11u);
    let roty_sin = get_param(xform_id, variation_id, 12u);
    let roty_cos = get_param(xform_id, variation_id, 13u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;

    let xmax = range_x * 0.5;
    let ymax = range_y * 0.5;
    let xmin = -xmax;
    let ymin = -ymax;

    var x = p.x;
    var y = p.y;

    if (radial_mode == 2 && (x > xmax || x < xmin)) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    if (width_mode == 2 && (y > ymax || y < ymin)) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    if (radial_mode == 3 && (x > xmax || x < xmin)) {
        return vec3<f32>(p.x * inv_w, p.y * inv_w, 0.0);
    }
    if (width_mode == 3 && (y > ymax || y < ymin)) {
        return vec3<f32>(p.x * inv_w, p.y * inv_w, 0.0);
    }

    var t: f32;
    if (abs(range_x) < 1e-30) {
        t = 0.0;
    } else {
        if (radial_mode == 0) {
            x = x - trunc(x / xmax) * xmax;
        } else if (radial_mode == 1) {
            x = clamp(x, xmin, xmax);
        }
        t = x * pi / xmax + pi;
    }

    var s: f32;
    if (abs(range_y) < 1e-30) {
        s = 0.0;
    } else {
        if (width_mode == 0) {
            y = y - trunc(y / ymax) * ymax;
        } else if (width_mode == 1) {
            y = clamp(y, ymin, ymax);
        }
        s = y * (width * 0.5) / ymax;
    }

    let half_t = twists * t * 0.5;
    let chs = cos(half_t);
    let shs = sin(half_t);
    let r_off = radius + s * chs;
    let mx = r_off * cos(t);
    let my = r_off * sin(t);
    let mz = s * shs;

    let rx = mx;
    let ry = my * roty_cos + mz * roty_sin;
    let rz = mz * roty_cos - my * roty_sin;

    let mx_out = rx * rotx_cos - rz * rotx_sin;
    let my_out = ry;
    let mz_out = rz * rotx_cos + rx * rotx_sin;

    let z_factor = select(modify_z, 0.0, abs(modify_z) < 1e-30);
    return vec3<f32>(mx_out, my_out, mz_out * z_factor);
}
"#),
};

// ---------------------------------------------------------------------------
// circleLinear
// ---------------------------------------------------------------------------

pub static CIRCLE_LINEAR: VariationDef = VariationDef {
    name: "circleLinear",
    display_name: "Circle Linear",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("Sc", "Scale", unlimited_float, 1.0, -10.0, 10.0),
        param!("K", "K", unlimited_float, 0.5, -10.0, 10.0),
        param!("Dens1", "Density 1", unlimited_float, 0.5, 0.0, 1.0),
        param!("Dens2", "Density 2", unlimited_float, 0.5, 0.0, 1.0),
        param!("Reverse", "Reverse", unlimited_float, 1.0, -10.0, 10.0),
        param!("X", "X", unlimited_float, 10.0, -100.0, 100.0),
        param!("Y", "Y", unlimited_float, 10.0, -100.0, 100.0),
        param!("Seed", "Seed", int, 0.0, -1000.0, 1000.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn cl_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_circleLinear(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let k = get_param(xform_id, variation_id, 1u);
    let dens1 = get_param(xform_id, variation_id, 2u);
    let dens2 = get_param(xform_id, variation_id, 3u);
    let reverse = get_param(xform_id, variation_id, 4u);
    let seed = i32(get_param(xform_id, variation_id, 7u));
    let safe_sc = select(sc, 1e-30, abs(sc) < 1e-30);

    let m = floor(0.5 * p.x / safe_sc);
    let n = floor(0.5 * p.y / safe_sc);
    let mi = i32(m);
    let ni = i32(n);

    var x = p.x - (m * 2.0 + 1.0) * sc;
    var y = p.y - (n * 2.0 + 1.0) * sc;
    let u = sqrt(x * x + y * y);
    let v = (0.3 + 0.7 * cl_disc_noise(mi + 10, ni + 3)) * sc;
    let z1 = cl_disc_noise(mi + seed, ni);

    if (z1 < dens1 && u < v) {
        if (reverse > 0.0) {
            if (z1 < dens1 * dens2) {
                x = k * x;
                y = k * y;
            } else {
                let safe_u = select(u, 1e-30, abs(u) < 1e-30);
                let z = v / safe_u * (1.0 - k) + k;
                x = z * x;
                y = z * y;
            }
        } else {
            if (z1 > dens1 * dens2) {
                x = k * x;
                y = k * y;
            } else {
                let safe_u = select(u, 1e-30, abs(u) < 1e-30);
                let z = v / safe_u * (1.0 - k) + k;
                x = z * x;
                y = z * y;
            }
        }
    }

    return vec2<f32>(x + (m * 2.0 + 1.0) * sc, y + (n * 2.0 + 1.0) * sc);
}
"#,
    wgsl_3d: Some(r#"
fn cl_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_circleLinear(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let k = get_param(xform_id, variation_id, 1u);
    let dens1 = get_param(xform_id, variation_id, 2u);
    let dens2 = get_param(xform_id, variation_id, 3u);
    let reverse = get_param(xform_id, variation_id, 4u);
    let seed = i32(get_param(xform_id, variation_id, 7u));
    let safe_sc = select(sc, 1e-30, abs(sc) < 1e-30);

    let m = floor(0.5 * p.x / safe_sc);
    let n = floor(0.5 * p.y / safe_sc);
    let mi = i32(m);
    let ni = i32(n);

    var x = p.x - (m * 2.0 + 1.0) * sc;
    var y = p.y - (n * 2.0 + 1.0) * sc;
    let u = sqrt(x * x + y * y);
    let v = (0.3 + 0.7 * cl_disc_noise(mi + 10, ni + 3)) * sc;
    let z1 = cl_disc_noise(mi + seed, ni);

    if (z1 < dens1 && u < v) {
        if (reverse > 0.0) {
            if (z1 < dens1 * dens2) {
                x = k * x;
                y = k * y;
            } else {
                let safe_u = select(u, 1e-30, abs(u) < 1e-30);
                let z = v / safe_u * (1.0 - k) + k;
                x = z * x;
                y = z * y;
            }
        } else {
            if (z1 > dens1 * dens2) {
                x = k * x;
                y = k * y;
            } else {
                let safe_u = select(u, 1e-30, abs(u) < 1e-30);
                let z = v / safe_u * (1.0 - k) + k;
                x = z * x;
                y = z * y;
            }
        }
    }

    return vec3<f32>(x + (m * 2.0 + 1.0) * sc, y + (n * 2.0 + 1.0) * sc, p.z);
}
"#),
};

