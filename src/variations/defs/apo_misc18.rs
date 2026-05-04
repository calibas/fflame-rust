//! Apophysis miscellany batch 18: lazysensen, spherecrop, xheart_blur_wf
//!
//!   - lazysensen (bezo97): per-axis floor-and-flip — for each axis with
//!     non-zero scale, negate the coordinate when floor(c·scale) parity
//!     matches a sign-dependent rule. 3 user params (scale_x, scale_y,
//!     scale_z). Java-recovered (cpp PluginVarCalc empty unported_stub).
//!   - spherecrop (Xyrus02): 3D version of circlecrop. 6 user params
//!     (radius, x, y, z, scatter_area, zero) + 1 init slot
//!     (_cA = clamp(scatter_area, -1, 1)). Uses needs_transform divide-out
//!     to thread the constant `+x0/+y0/+z0` offsets through the outer
//!     multiplier. doHide branch returns (0,0,0) compromise (no doHide
//!     flag in our model).
//!   - xheart_blur_wf (Xyrus02-derived): random heart-shape blur.
//!     2 user params (angle, ratio) + 3 init slots (_sina, _cosa, _rat).
//!     RNG (2 calls per iter); body factors cleanly through outer.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/lazysensen.cpp` (Java-recovered)
//!   - `output/jwildfire-vars/output/spherecrop.cpp`
//!   - `output/jwildfire-vars/output/xheart_blur_wf.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// lazysensen
// ---------------------------------------------------------------------------

pub static LAZYSENSEN: VariationDef = VariationDef {
    name: "lazysensen",
    display_name: "Lazy Sensen",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("scale_x", "Scale X", unlimited_float, 1.0, -10.0, 10.0),
        param!("scale_y", "Scale Y", unlimited_float, 1.0, -10.0, 10.0),
        param!("scale_z", "Scale Z", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn lazysensen_flip(c: f32, scale: f32) -> f32 {
    if (abs(scale) < 1e-30) {
        return c;
    }
    let nr = floor(c * scale);
    let nri = i32(nr);
    let parity = nri - (nri / 2) * 2;
    if (nri >= 0) {
        if (parity == 1 || parity == -1) {
            return -c;
        }
    } else {
        if (parity == 0) {
            return -c;
        }
    }
    return c;
}

fn variation_lazysensen(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sx = get_param(xform_id, variation_id, 0u);
    let sy = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(lazysensen_flip(p.x, sx), lazysensen_flip(p.y, sy));
}
"#,
    wgsl_3d: Some(r#"
fn lazysensen_flip(c: f32, scale: f32) -> f32 {
    if (abs(scale) < 1e-30) {
        return c;
    }
    let nr = floor(c * scale);
    let nri = i32(nr);
    let parity = nri - (nri / 2) * 2;
    if (nri >= 0) {
        if (parity == 1 || parity == -1) {
            return -c;
        }
    } else {
        if (parity == 0) {
            return -c;
        }
    }
    return c;
}

fn variation_lazysensen(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sx = get_param(xform_id, variation_id, 0u);
    let sy = get_param(xform_id, variation_id, 1u);
    let sz = get_param(xform_id, variation_id, 2u);
    return vec3<f32>(
        lazysensen_flip(p.x, sx),
        lazysensen_flip(p.y, sy),
        lazysensen_flip(p.z, sz),
    );
}
"#),
};

// ---------------------------------------------------------------------------
// spherecrop
// ---------------------------------------------------------------------------

pub static SPHERECROP: VariationDef = VariationDef {
    name: "spherecrop",
    display_name: "Sphere Crop",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0),
        param!("x", "X", unlimited_float, 0.0, -10.0, 10.0),
        param!("y", "Y", unlimited_float, 0.0, -10.0, 10.0),
        param!("z", "Z", unlimited_float, 0.0, -10.0, 10.0),
        param!("scatter_area", "Scatter Area", unlimited_float, 0.0, -1.0, 1.0),
        param!("zero", "Zero", int, 1.0, 0.0, 1.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_spherecrop(user: array<f32, 6>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = clamp(user[4], -1.0, 1.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_spherecrop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let zero = i32(get_param(xform_id, variation_id, 5u));
    let ca = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    if (cr0 && esc) {
        return vec2<f32>(0.0, 0.0);
    }
    if (!cr0 && esc) {
        return vec2<f32>((w * rdc * cos(ang) + x0) * inv_w, (w * rdc * sin(ang) + y0) * inv_w);
    }
    return vec2<f32>((w * dx + x0) * inv_w, (w * dy + y0) * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_spherecrop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let cr = get_param(xform_id, variation_id, 0u);
    let x0 = get_param(xform_id, variation_id, 1u);
    let y0 = get_param(xform_id, variation_id, 2u);
    let z0 = get_param(xform_id, variation_id, 3u);
    let zero = i32(get_param(xform_id, variation_id, 5u));
    let ca = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let dz = p.z - z0;
    let rad = sqrt(dx * dx + dy * dy + dz * dz);
    let safe_rad = select(rad, 1e-30, rad < 1e-30);
    let ang = atan2(dy, dx);
    let az = acos(clamp(dz / safe_rad, -1.0, 1.0));
    let rdc = cr + rng_nextf(rng) * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;

    let s = sin(ang);
    let c = cos(ang);
    let saz = sin(az);
    let caz = cos(az);

    if (cr0 && esc) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    if (!cr0 && esc) {
        return vec3<f32>(
            (w * rdc * c * saz + x0) * inv_w,
            (w * rdc * s * saz + y0) * inv_w,
            (w * rdc * caz + z0) * inv_w,
        );
    }
    return vec3<f32>(
        (w * dx + x0) * inv_w,
        (w * dy + y0) * inv_w,
        (w * dz + z0) * inv_w,
    );
}
"#),
};

// ---------------------------------------------------------------------------
// xheart_blur_wf
// ---------------------------------------------------------------------------

pub static XHEART_BLUR_WF: VariationDef = VariationDef {
    name: "xheart_blur_wf",
    display_name: "X-Heart Blur WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0),
        param!("ratio", "Ratio", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_xheart_blur_wf(user: array<f32, 2>) -> array<f32, 3> {
    let pi_4 = 0.7853981633974483;
    let ang = pi_4 + 0.5 * pi_4 * user[0];
    var out: array<f32, 3>;
    out[0] = sin(ang);
    out[1] = cos(ang);
    out[2] = 6.0 + 2.0 * user[1];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_xheart_blur_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let sina = get_param(xform_id, variation_id, 2u);
    let cosa = get_param(xform_id, variation_id, 3u);
    let rat = get_param(xform_id, variation_id, 4u);

    let dx = 2.0 - rng_nextf(rng) * 4.0;
    let dy = 2.0 - rng_nextf(rng) * 4.0;
    var r2_4 = dx * dx + dy * dy + 4.0;
    if (abs(r2_4) < 1e-30) {
        r2_4 = 1.0;
    }
    let bx = 4.0 / r2_4;
    let by = rat / r2_4;
    let x = cosa * (bx * dx) - sina * (by * dy);
    let y = sina * (bx * dx) + cosa * (by * dy);

    if (x > 0.0) {
        return vec2<f32>(x, y);
    }
    return vec2<f32>(x, -y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_xheart_blur_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let sina = get_param(xform_id, variation_id, 2u);
    let cosa = get_param(xform_id, variation_id, 3u);
    let rat = get_param(xform_id, variation_id, 4u);

    let dx = 2.0 - rng_nextf(rng) * 4.0;
    let dy = 2.0 - rng_nextf(rng) * 4.0;
    var r2_4 = dx * dx + dy * dy + 4.0;
    if (abs(r2_4) < 1e-30) {
        r2_4 = 1.0;
    }
    let bx = 4.0 / r2_4;
    let by = rat / r2_4;
    let x = cosa * (bx * dx) - sina * (by * dy);
    let y = sina * (bx * dx) + cosa * (by * dy);

    if (x > 0.0) {
        return vec3<f32>(x, y, p.z);
    }
    return vec3<f32>(x, -y, p.z);
}
"#),
};
