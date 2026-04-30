//! Post-phase variations
//!
//! Post-phase variations directly modify the output coordinates after normal variations.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static POST_BWRAPS: VariationDef = VariationDef {
    name: "post_bwraps",
    display_name: "Post Bwraps",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, -10.0, 10.0),
        param!("space", "Space", unlimited_float, 0.0, -1.0, 1.0),
        param!("gain", "Gain", unlimited_float, 1.0, -5.0, 5.0),
        param!("inner_twist", "Inner Twist", unlimited_float, 0.0, -10.0, 10.0),
        param!("outer_twist", "Outer Twist", unlimited_float, 0.0, -10.0, 10.0),
    ],
    // 5 derived values at slots 5..10 (identical layout to pre_bwraps):
    //   5: g2,  6: r2,  7: rfactor,  8: inner_twist_rad,  9: outer_twist_rad
    needs_transform: false,
    writes_color: false,
    init_param_count: 5,
    wgsl_init: Some(r#"
fn init_post_bwraps(user: array<f32, 5>) -> array<f32, 5> {
    let cellsize = user[0];
    let space = user[1];
    let gain = user[2];
    let inner_twist = user[3];
    let outer_twist = user[4];
    var out: array<f32, 5>;
    if (cellsize == 0.0) {
        out[0] = 0.0; out[1] = 0.0; out[2] = 0.0;
        out[3] = inner_twist * 3.14159265358979 / 180.0;
        out[4] = outer_twist * 3.14159265358979 / 180.0;
        return out;
    }
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = (gain * gain) / (radius + 1e-6) + 1e-6;
    var max_bubble = g2 * radius;
    if (max_bubble > 2.0) {
        max_bubble = 1.0;
    } else {
        max_bubble = max_bubble * (1.0 / ((max_bubble * max_bubble) / 4.0 + 1.0));
    }
    out[0] = g2;
    out[1] = radius * radius;
    out[2] = radius / max_bubble;
    out[3] = inner_twist * 3.14159265358979 / 180.0;
    out[4] = outer_twist * 3.14159265358979 / 180.0;
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_post_bwraps(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let inner_twist = get_param(xform_id, variation_id, 8u);
    let outer_twist = get_param(xform_id, variation_id, 9u);

    if cellsize == 0.0 {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if (lx * lx + ly * ly) <= r2 {
        lx = lx * g2;
        ly = ly * g2;

        var r = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);

        lx = lx * r;
        ly = ly * r;

        r = (lx * lx + ly * ly) / r2;
        let theta = inner_twist * (1.0 - r) + outer_twist * r;

        let s = sin(theta);
        let c = cos(theta);

        return vec2<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly
        );
    }

    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_bwraps(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);
    let inner_twist = get_param(xform_id, variation_id, 8u);
    let outer_twist = get_param(xform_id, variation_id, 9u);

    if cellsize == 0.0 {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if (lx * lx + ly * ly) <= r2 {
        lx = lx * g2;
        ly = ly * g2;

        var r = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);

        lx = lx * r;
        ly = ly * r;

        r = (lx * lx + ly * ly) / r2;
        let theta = inner_twist * (1.0 - r) + outer_twist * r;

        let s = sin(theta);
        let c = cos(theta);

        return vec3<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly,
            p.z
        );
    }

    return p;
}
"#),
};

pub static POST_CROP: VariationDef = VariationDef {
    name: "post_crop",
    display_name: "Post Crop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("left", "Left", unlimited_float, -1.0, -5.0, 5.0),
        param!("top", "Top", unlimited_float, -1.0, -5.0, 5.0),
        param!("right", "Right", unlimited_float, 1.0, -5.0, 5.0),
        param!("bottom", "Bottom", unlimited_float, 1.0, -5.0, 5.0),
        param!("scatter_area", "Scatter Area", float, 0.0, -1.0, 1.0),
        param!("zero", "Zero", bool, false),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_post_crop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Post_Crop - same as crop but applied after variations
    let x0 = get_param(xform_id, variation_id, 0u);
    let y0 = get_param(xform_id, variation_id, 1u);
    let x1 = get_param(xform_id, variation_id, 2u);
    let y1 = get_param(xform_id, variation_id, 3u);
    let scatter = get_param(xform_id, variation_id, 4u);
    let zero = get_param(xform_id, variation_id, 5u);

    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec2<f32>(0.0, 0.0);
    }

    if x < _x0 {
        x = _x0 + rng_nextf(rng) * w;
    } else if x > _x1 {
        x = _x1 - rng_nextf(rng) * w;
    }

    if y < _y0 {
        y = _y0 + rng_nextf(rng) * h;
    } else if y > _y1 {
        y = _y1 - rng_nextf(rng) * h;
    }

    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_crop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Post_Crop - 3D (Z passes through)
    let x0 = get_param(xform_id, variation_id, 0u);
    let y0 = get_param(xform_id, variation_id, 1u);
    let x1 = get_param(xform_id, variation_id, 2u);
    let y1 = get_param(xform_id, variation_id, 3u);
    let scatter = get_param(xform_id, variation_id, 4u);
    let zero = get_param(xform_id, variation_id, 5u);

    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec3<f32>(0.0, 0.0, p.z);
    }

    if x < _x0 {
        x = _x0 + rng_nextf(rng) * w;
    } else if x > _x1 {
        x = _x1 - rng_nextf(rng) * w;
    }

    if y < _y0 {
        y = _y0 + rng_nextf(rng) * h;
    } else if y > _y1 {
        y = _y1 - rng_nextf(rng) * h;
    }

    return vec3<f32>(x, y, p.z);
}
"#),
};

pub static POST_FALLOFF2: VariationDef = VariationDef {
    name: "post_falloff2",
    display_name: "Post Falloff2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("scatter", "Scatter", unlimited_float, 1.0, 0.000001, 10.0),
        param!("mindist", "Min Distance", unlimited_float, 0.5, 0.0, 10.0),
        param!("mul_x", "Multiply X", float, 1.0, 0.0, 1.0),
        param!("mul_y", "Multiply Y", float, 1.0, 0.0, 1.0),
        param!("mul_z", "Multiply Z", float, 0.0, 0.0, 1.0),
        param!("mul_c", "Multiply Color", float, 0.0, 0.0, 1.0),
        param!("x0", "X Center", unlimited_float, 0.0, -10.0, 10.0),
        param!("y0", "Y Center", unlimited_float, 0.0, -10.0, 10.0),
        param!("z0", "Z Center", unlimited_float, 0.0, -10.0, 10.0),
        param!("invert", "Invert", bool, false),
        VariationParamDef {
            name: "type",
            display_name: "Blur Type",
            param_type: ParamType::Integer,
            default_value: 0.0,
            min_value: Some(0.0),
            max_value: Some(2.0),
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_post_falloff2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Post_Falloff2 - Same as pre_falloff2 but applied after variations
    const PI: f32 = 3.14159265359;

    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blurtype = get_param(xform_id, variation_id, 10u);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let dist = sqrt(dx * dx + dy * dy);

    var factor: f32;
    if invert > 0.5 {
        factor = select(1.0, dist / mindist, dist < mindist);
    } else {
        factor = select(1.0, mindist / dist, dist > mindist);
    }

    var sx: f32;
    var sy: f32;

    let blurtype_int = i32(blurtype + 0.5);

    if blurtype_int == 0 {
        let r = scatter * factor;
        let angle = rng_nextf(rng) * 2.0 * PI;
        let d = rng_nextf(rng) * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    } else if blurtype_int == 1 {
        let r = scatter * factor;
        let angle = rng_nextf(rng) * 2.0 * PI;
        let d = (rng_nextf(rng) + rng_nextf(rng)) * 0.5 * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    } else {
        let r = scatter * factor;
        let angle = rng_nextf(rng) * 2.0 * PI;
        let d = sqrt(-log(rng_nextf(rng) + 1e-10)) * r;
        sx = d * cos(angle);
        sy = d * sin(angle);
    }

    return vec2<f32>(p.x + sx * mul_x, p.y + sy * mul_y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_falloff2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Post_Falloff2 - Same as pre_falloff2 but applied after variations (3D)
    const PI: f32 = 3.14159265359;

    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let mul_z = get_param(xform_id, variation_id, 4u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let z0 = get_param(xform_id, variation_id, 8u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blurtype = get_param(xform_id, variation_id, 10u);

    let dx = p.x - x0;
    let dy = p.y - y0;
    let dz = p.z - z0;
    let dist = sqrt(dx * dx + dy * dy + dz * dz);

    var factor: f32;
    if invert > 0.5 {
        factor = select(1.0, dist / mindist, dist < mindist);
    } else {
        factor = select(1.0, mindist / dist, dist > mindist);
    }

    var sx: f32;
    var sy: f32;
    var sz: f32;

    let blurtype_int = i32(blurtype + 0.5);

    if blurtype_int == 0 {
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = rng_nextf(rng) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else if blurtype_int == 1 {
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = (rng_nextf(rng) + rng_nextf(rng)) * 0.5 * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else {
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = sqrt(-log(rng_nextf(rng) + 1e-10)) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    }

    return vec3<f32>(p.x + sx * mul_x, p.y + sy * mul_y, p.z + sz * mul_z);
}
"#),
};

pub static POST_CURL: VariationDef = VariationDef {
    name: "post_curl",
    display_name: "Post Curl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[
        param!("c1", "C1", unlimited_float, 0.0, -5.0, 5.0),
        param!("c2", "C2", unlimited_float, 0.0, -5.0, 5.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_post_curl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Post_Curl - Same as curl but applied after variations
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);

    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;

    let r = 1.0 / (re * re + im * im);

    return vec2<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_curl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Post_Curl - 3D version (Z passes through)
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);

    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;

    let r = 1.0 / (re * re + im * im);

    return vec3<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r,
        p.z
    );
}
"#),
};

pub static POST_CURL3D: VariationDef = VariationDef {
    name: "post_curl3d",
    display_name: "Post Curl 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[
        param!("cx", "CX", unlimited_float, 0.0, -5.0, 5.0),
        param!("cy", "CY", unlimited_float, 0.0, -5.0, 5.0),
        param!("cz", "CZ", unlimited_float, 0.0, -5.0, 5.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_post_curl3d(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Post_Curl3D - 2D version (placeholder)
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_curl3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Post_Curl3D - Full 3D curl transformation applied after variations
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let cz = get_param(xform_id, variation_id, 2u);

    let x = clamp(p.x, -1e30, 1e30);
    let y = clamp(p.y, -1e30, 1e30);
    let z = clamp(p.z, -1e30, 1e30);

    let r2 = x * x + y * y + z * z;
    let c2 = cx * cx + cy * cy + cz * cz;
    let denom = r2 * c2 + 2.0 * cx * x - 2.0 * cy * y + 2.0 * cz * z + 1.0;
    let r = 1.0 / denom;

    return vec3<f32>(
        r * (x + cx * r2),
        r * (y - cy * r2),
        r * (z + cz * r2)
    );
}
"#),
};
