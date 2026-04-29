//! Extended normal-phase variations
//!
//! Additional variations beyond the basic and advanced sets.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static ZTRANSLATE: VariationDef = VariationDef {
    name: "ztranslate",
    display_name: "ZTranslate",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_ztranslate(p: vec2<f32>) -> vec2<f32> {
    // ZTranslate only affects Z (3D mode), pass through in 2D
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_ztranslate(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: Normal-phase Z translation
    // FPz += vars[variation_id] (added during weighted sum)
    // Return (0, 0, 1) so weighted sum adds weight to Z: result.z += weight * 1
    return vec3<f32>(0.0, 0.0, 1.0);
}
"#),
};

pub static JULIA3D: VariationDef = VariationDef {
    name: "julia3d",
    display_name: "Julia3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", unlimited_int, 2.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_julia3d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Julia3D in 2D mode (Z = 0)
    let power_f = get_param(xform_id, variation_id, 0u);
    let N = i32(power_f);

    if (N == 0) {
        return p;
    }

    let absN = abs(N);
    let absN_f = f32(absN);

    if (N == 1) {
        return p;
    } else if (N == -1) {
        let r2 = dot(p, p);
        return p / r2;
    } else if (N == 2) {
        let r2d = dot(p, p);
        let r = 1.0 / sqrt(sqrt(r2d));
        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        return vec2<f32>(r * sqrt(r2d) * cos(angle), r * sqrt(r2d) * sin(angle));
    } else if (N == -2) {
        let r2d = dot(p, p);
        let r3d = sqrt(r2d);
        let r = 1.0 / (sqrt(r3d) * r3d);
        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        return vec2<f32>(r * sqrt(r2d) * cos(angle), -r * sqrt(r2d) * sin(angle));
    } else {
        let r2d = dot(p, p);
        let cN = (1.0 / power_f - 1.0) / 2.0;
        let r = pow(r2d, cN);

        let random_idx = i32(rng_nextf(rng) * absN_f);
        let angle = (atan2(p.y, p.x) + 6.28318530718 * f32(random_idx)) / power_f;
        let tmp = r * sqrt(r2d);

        if (N > 0) {
            return vec2<f32>(tmp * cos(angle), tmp * sin(angle));
        } else {
            return vec2<f32>(tmp * cos(angle), -tmp * sin(angle));
        }
    }
}
"#,
    wgsl_3d: Some(r#"
fn variation_julia3d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Julia3D: Full 3D Julia set implementation by Joel Faber
    let power_f = get_param(xform_id, variation_id, 0u);
    let N = i32(power_f);

    if (N == 0) {
        return p;
    }

    let absN = abs(N);
    let absN_f = f32(absN);

    if (N == 1) {
        return p;
    } else if (N == -1) {
        let r2 = dot(p, p);
        return p / r2;
    } else if (N == 2) {
        let z = p.z / 2.0;
        let r2d = dot(p.xy, p.xy);
        let r3d = sqrt(r2d + z * z);
        let r = 1.0 / sqrt(sqrt(r3d));
        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        let tmp = r * sqrt(r2d);
        return vec3<f32>(tmp * cos(angle), tmp * sin(angle), r * z);
    } else if (N == -2) {
        let z = p.z / 2.0;
        let r2d = dot(p.xy, p.xy);
        let r3d = sqrt(r2d + z * z);
        let r = 1.0 / (sqrt(r3d) * r3d);
        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        let tmp = r * sqrt(r2d);
        return vec3<f32>(tmp * cos(angle), -tmp * sin(angle), r * z);
    } else {
        let z = p.z / absN_f;
        let r2d = dot(p.xy, p.xy);
        let cN = (1.0 / power_f - 1.0) / 2.0;
        let r = pow(r2d + z * z, cN);
        let random_idx = i32(rng_nextf(rng) * absN_f);
        let angle = (atan2(p.y, p.x) + 6.28318530718 * f32(random_idx)) / power_f;
        let tmp = r * sqrt(r2d);

        if (N > 0) {
            return vec3<f32>(tmp * cos(angle), tmp * sin(angle), r * z);
        } else {
            return vec3<f32>(tmp * cos(angle), -tmp * sin(angle), r * z);
        }
    }
}
"#),
};

pub static FALLOFF2: VariationDef = VariationDef {
    name: "falloff2",
    display_name: "Falloff2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
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
fn variation_falloff2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Falloff2: Distance-based blur effect with 3 modes
    const PI: f32 = 3.14159265359;
    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blur_type = get_param(xform_id, variation_id, 10u);

    let rmax = 0.04 * scatter;
    var d = sqrt((p.x - x0) * (p.x - x0) + (p.y - y0) * (p.y - y0));

    if (invert > 0.5) {
        d = 1.0 - d;
    }
    if (d < 0.0) {
        d = 0.0;
    }

    d = (d - mindist) * rmax;
    if (d < 0.0) {
        d = 0.0;
    }

    if (blur_type < 0.5) {
        let rand_x = rng_nextf(rng);
        let rand_y = rng_nextf(rng);
        return vec2<f32>(
            p.x + mul_x * rand_x * d,
            p.y + mul_y * rand_y * d
        );
    } else if (blur_type < 1.5) {
        let r_in = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
        let phi = atan2(p.y, p.x) + mul_y * rng_nextf(rng) * d;
        let r = r_in + mul_x * rng_nextf(rng) * d;
        return vec2<f32>(r * cos(phi), r * sin(phi));
    } else {
        let phi = d * rng_nextf(rng) * PI;
        let r = d * rng_nextf(rng);
        return vec2<f32>(
            p.x + mul_x * r * cos(phi),
            p.y + mul_y * r * sin(phi)
        );
    }
}
"#,
    wgsl_3d: Some(r#"
fn variation_falloff2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Falloff2: Distance-based blur effect with 3 modes (3D)
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
    let blur_type = get_param(xform_id, variation_id, 10u);

    let rmax = 0.04 * scatter;
    var d = sqrt((p.x - x0) * (p.x - x0) + (p.y - y0) * (p.y - y0) + (p.z - z0) * (p.z - z0));

    if (invert > 0.5) {
        d = 1.0 - d;
    }
    if (d < 0.0) {
        d = 0.0;
    }

    d = (d - mindist) * rmax;
    if (d < 0.0) {
        d = 0.0;
    }

    if (blur_type < 0.5) {
        let rand_x = rng_nextf(rng);
        let rand_y = rng_nextf(rng);
        let rand_z = rng_nextf(rng);
        return vec3<f32>(
            p.x + mul_x * rand_x * d,
            p.y + mul_y * rand_y * d,
            p.z + mul_z * rand_z * d
        );
    } else if (blur_type < 1.5) {
        let r_in = sqrt(p.x * p.x + p.y * p.y + p.z * p.z) + 1e-6;
        let sigma = asin(p.z / r_in) + mul_z * rng_nextf(rng) * d;
        let phi = atan2(p.y, p.x) + mul_y * rng_nextf(rng) * d;
        let r = r_in + mul_x * rng_nextf(rng) * d;
        return vec3<f32>(
            r * cos(sigma) * cos(phi),
            r * cos(sigma) * sin(phi),
            r * sin(sigma)
        );
    } else {
        let sigma = d * rng_nextf(rng) * 2.0 * PI;
        let phi = d * rng_nextf(rng) * PI;
        let r = d * rng_nextf(rng);
        return vec3<f32>(
            p.x + mul_x * r * cos(sigma) * cos(phi),
            p.y + mul_y * r * cos(sigma) * sin(phi),
            p.z + mul_z * r * sin(sigma)
        );
    }
}
"#),
};

pub static WEDGE: VariationDef = VariationDef {
    name: "wedge",
    display_name: "Wedge",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("angle", "Angle", angle, 90.0),
        param!("hole", "Hole", unlimited_float, 0.0, -5.0, 5.0),
        param!("count", "Count", int, 2.0, 1.0, 20.0),
        param!("swirl", "Swirl", unlimited_float, 0.0, -30.0, 30.0),
    ],
    // 2 derived values at slots 4..6:
    //   4: angle_rad  (angle_deg · π/180)
    //   5: comp_fac   (1 − angle_rad · count / (2π))
    needs_transform: false,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_wedge(user: array<f32, 4>) -> array<f32, 2> {
    let angle_deg = user[0];
    let count = user[2];
    let angle_rad = angle_deg * 3.14159265358979 / 180.0;
    var out: array<f32, 2>;
    out[0] = angle_rad;
    out[1] = 1.0 - angle_rad * count * 0.15915494309189534;  // 1/(2π)
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_wedge(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    const C1_2PI: f32 = 0.15915494309189534;

    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    let angle_rad = get_param(xform_id, variation_id, 4u);
    let comp_fac = get_param(xform_id, variation_id, 5u);

    let r = sqrt(dot(p, p));
    var a = atan2(p.y, p.x) + swirl * r;
    let c = floor((count * a + PI) * C1_2PI);
    a = a * comp_fac + c * angle_rad;

    let r_out = r + hole;
    return vec2<f32>(r_out * cos(a), r_out * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_wedge(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    const C1_2PI: f32 = 0.15915494309189534;

    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    let angle_rad = get_param(xform_id, variation_id, 4u);
    let comp_fac = get_param(xform_id, variation_id, 5u);

    let r = sqrt(dot(p.xy, p.xy));
    var a = atan2(p.y, p.x) + swirl * r;
    let c = floor((count * a + PI) * C1_2PI);
    a = a * comp_fac + c * angle_rad;

    let r_out = r + hole;
    return vec3<f32>(r_out * cos(a), r_out * sin(a), p.z);
}
"#),
};

pub static EPISPIRAL: VariationDef = VariationDef {
    name: "epispiral",
    display_name: "Epispiral",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("n", "N", unlimited_float, 6.0, -20.0, 20.0),
        param!("thickness", "Thickness", unlimited_float, 0.0, -2.0, 2.0),
        param!("holes", "Holes", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_epispiral(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Epispiral: Epicycloid spiral pattern with random thickness
    let n = get_param(xform_id, variation_id, 0u);
    let thickness = get_param(xform_id, variation_id, 1u);
    let holes = get_param(xform_id, variation_id, 2u);

    let theta = atan2(p.y, p.x);
    let t = rng_nextf(rng) * thickness / cos(n * theta) - holes;

    if (abs(t) < 1e-6) {
        return vec2<f32>(0.0, 0.0);
    }

    return vec2<f32>(t * cos(theta), t * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_epispiral(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Epispiral: 3D (Z passes through)
    let n = get_param(xform_id, variation_id, 0u);
    let thickness = get_param(xform_id, variation_id, 1u);
    let holes = get_param(xform_id, variation_id, 2u);

    let theta = atan2(p.y, p.x);
    let t = rng_nextf(rng) * thickness / cos(n * theta) - holes;

    if (abs(t) < 1e-6) {
        return vec3<f32>(0.0, 0.0, p.z);
    }

    return vec3<f32>(t * cos(theta), t * sin(theta), p.z);
}
"#),
};

pub static BWRAPS: VariationDef = VariationDef {
    name: "bwraps",
    display_name: "BWraps",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, -10.0, 10.0),
        param!("space", "Space", unlimited_float, 0.0, -1.0, 1.0),
        param!("gain", "Gain", unlimited_float, 1.0, -5.0, 5.0),
        param!("inner_twist", "Inner Twist", unlimited_float, 0.0, -10.0, 10.0),
        param!("outer_twist", "Outer Twist", unlimited_float, 0.0, -10.0, 10.0),
    ],
    // 3 derived values at slots 5..8:
    //   5: g2        (gain² / (radius + ε) + ε)
    //   6: r2        (radius²)
    //   7: rfactor   (radius / max_bubble, where max_bubble = clamp(g2·radius))
    needs_transform: false,
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_bwraps(user: array<f32, 5>) -> array<f32, 3> {
    let cellsize = user[0];
    let space = user[1];
    let gain = user[2];
    var out: array<f32, 3>;
    if (cellsize == 0.0) {
        // Body short-circuits when cellsize == 0; init values irrelevant.
        out[0] = 0.0; out[1] = 0.0; out[2] = 0.0;
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
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_bwraps(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);

    if (cellsize == 0.0) {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if ((lx * lx + ly * ly) > r2) {
        return p;
    }

    lx = lx * g2;
    ly = ly * g2;

    let r_dist = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);
    lx = lx * r_dist;
    ly = ly * r_dist;

    let r_ratio = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - r_ratio) + outer_twist * r_ratio;

    let vx = cx + cos(theta) * lx + sin(theta) * ly;
    let vy = cy - sin(theta) * lx + cos(theta) * ly;

    return vec2<f32>(vx, vy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bwraps(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cellsize = get_param(xform_id, variation_id, 0u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    let g2 = get_param(xform_id, variation_id, 5u);
    let r2 = get_param(xform_id, variation_id, 6u);
    let rfactor = get_param(xform_id, variation_id, 7u);

    if (cellsize == 0.0) {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    if ((lx * lx + ly * ly) > r2) {
        return p;
    }

    lx = lx * g2;
    ly = ly * g2;

    let r_dist = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);
    lx = lx * r_dist;
    ly = ly * r_dist;

    let r_ratio = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - r_ratio) + outer_twist * r_ratio;

    let vx = cx + cos(theta) * lx + sin(theta) * ly;
    let vy = cy - sin(theta) * lx + cos(theta) * ly;

    return vec3<f32>(vx, vy, p.z);
}
"#),
};

pub static JULIASCOPE: VariationDef = VariationDef {
    name: "juliascope",
    display_name: "JuliaScope",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", unlimited_int, 2.0, -20.0, 20.0),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_juliascope(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis JuliaScope variation
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let dist = get_param(xform_id, variation_id, 1u);

    let r2 = dot(p, p);

    let rnd = rng_nextf(rng);
    let t = (atan2(p.y, p.x) + 2.0 * PI * f32(i32(abs(f32(power)) * rnd))) / f32(power);

    let sign = select(-1.0, 1.0, (i32(abs(f32(power)) * rng_nextf(rng)) & 1) == 0);

    if (power == 1) {
        let r_out = pow(r2, dist * 0.5);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else if (power == -1) {
        let r_out = pow(r2, dist * 0.5);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else if (power == 2) {
        let r_out = pow(r2, dist * 0.25);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else if (power == -2) {
        let r_out = pow(r2, dist * 0.25);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else {
        let cn = dist / f32(power) * 0.5;
        let r_out = pow(r2, cn);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    }
}
"#,
    wgsl_3d: Some(r#"
fn variation_juliascope(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis JuliaScope: 3D (Z passes through)
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let dist = get_param(xform_id, variation_id, 1u);

    let r2 = p.x * p.x + p.y * p.y;

    let rnd = rng_nextf(rng);
    let t = (atan2(p.y, p.x) + 2.0 * PI * f32(i32(abs(f32(power)) * rnd))) / f32(power);

    let sign = select(-1.0, 1.0, (i32(abs(f32(power)) * rng_nextf(rng)) & 1) == 0);

    if (power == 1) {
        let r_out = pow(r2, dist * 0.5);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else if (power == -1) {
        let r_out = pow(r2, dist * 0.5);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else if (power == 2) {
        let r_out = pow(r2, dist * 0.25);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else if (power == -2) {
        let r_out = pow(r2, dist * 0.25);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else {
        let cn = dist / f32(power) * 0.5;
        let r_out = pow(r2, cn);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    }
}
"#),
};

pub static JULIA3DZ: VariationDef = VariationDef {
    name: "julia3dz",
    display_name: "Julia3Dz",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", unlimited_int, 2.0, -20.0, 20.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_julia3dz(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Julia3Dz is a 3D variation, pass through in 2D
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_julia3dz(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Julia3Dz - Full 3D Julia set with Z modification
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let abs_power = abs(power);
    let power_f = f32(power);

    let r2d = p.x * p.x + p.y * p.y;

    if (power == 1) {
        return p;
    }

    if (power == -1) {
        let r = 1.0 / r2d;
        return vec3<f32>(r * p.x, -r * p.y, r * p.z);
    }

    let rnd_int = i32(rng_nextf(rng) * f32(abs_power));
    let angle = (atan2(p.y, p.x) + 2.0 * PI * f32(rnd_int)) / power_f;

    if (power == 2) {
        let r2d_sqrt = sqrt(r2d);
        let r = sqrt(r2d_sqrt);
        let z_out = r * p.z / r2d_sqrt / 2.0;
        return vec3<f32>(r * cos(angle), r * sin(angle), z_out);
    }

    if (power == -2) {
        let r2d_sqrt = sqrt(r2d);
        let r = 1.0 / sqrt(r2d_sqrt);
        let z_out = r * p.z / r2d_sqrt / 2.0;
        return vec3<f32>(r * cos(angle), -r * sin(angle), z_out);
    }

    let cN = 1.0 / power_f / 2.0;
    let r = pow(r2d, cN);
    let r2d_sqrt = sqrt(r2d);
    let z_out = r * p.z / (r2d_sqrt * f32(abs_power));

    return vec3<f32>(r * cos(angle), r * sin(angle), z_out);
}
"#),
};

pub static CURL3D: VariationDef = VariationDef {
    name: "curl3d",
    display_name: "Curl3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
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
fn variation_curl3d(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Curl3D is a 3D variation, apply XY curl in 2D
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);

    let r2 = p.x * p.x + p.y * p.y;
    let c2 = cx * cx + cy * cy;
    let denom = r2 * c2 + 2.0 * cx * p.x - 2.0 * cy * p.y + 1.0;
    let r = 1.0 / denom;

    return vec2<f32>(
        r * (p.x + cx * r2),
        r * (p.y - cy * r2)
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_curl3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Curl3D - Full 3D curl transformation
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let cz = get_param(xform_id, variation_id, 2u);

    let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
    let c2 = cx * cx + cy * cy + cz * cz;
    let denom = r2 * c2 + 2.0 * cx * p.x - 2.0 * cy * p.y + 2.0 * cz * p.z + 1.0;
    let r = 1.0 / denom;

    return vec3<f32>(
        r * (p.x + cx * r2),
        r * (p.y - cy * r2),
        r * (p.z + cz * r2)
    );
}
"#),
};

pub static RADIAL_BLUR: VariationDef = VariationDef {
    name: "radial_blur",
    display_name: "Radial Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("angle", "Angle", angle, 0.0),
    ],
    // 2 derived values at slots 1..3:
    //   1: spin_var  (sin(angle_deg · π/360))
    //   2: zoom_var  (cos(angle_deg · π/360))
    needs_transform: false,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_radial_blur(user: array<f32, 1>) -> array<f32, 2> {
    let half_angle_rad = user[0] * 3.14159265358979 / 360.0;  // (deg·π/180)·0.5
    var out: array<f32, 2>;
    out[0] = sin(half_angle_rad);  // spin_var
    out[1] = cos(half_angle_rad);  // zoom_var
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_radial_blur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let spin_var = get_param(xform_id, variation_id, 1u);
    let zoom_var = get_param(xform_id, variation_id, 2u);

    let rnd_g = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;

    let ra = sqrt(p.x * p.x + p.y * p.y);
    let angle_out = atan2(p.y, p.x) + spin_var * rnd_g;
    let rz = zoom_var * rnd_g - 1.0;

    return vec2<f32>(
        ra * cos(angle_out) + rz * p.x,
        ra * sin(angle_out) + rz * p.y
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_radial_blur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let spin_var = get_param(xform_id, variation_id, 1u);
    let zoom_var = get_param(xform_id, variation_id, 2u);

    let rnd_g = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;

    let ra = sqrt(p.x * p.x + p.y * p.y);
    let angle_out = atan2(p.y, p.x) + spin_var * rnd_g;
    let rz = zoom_var * rnd_g - 1.0;

    return vec3<f32>(
        ra * cos(angle_out) + rz * p.x,
        ra * sin(angle_out) + rz * p.y,
        p.z
    );
}
"#),
};

pub static BLUR_CIRCLE: VariationDef = VariationDef {
    name: "blur_circle",
    display_name: "Blur Circle",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_blur_circle(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Circle - uniform blur in a circle
    const PI: f32 = 3.14159265359;
    const PI_4: f32 = 0.78539816339;

    let x = 2.0 * rng_nextf(rng) - 1.0;
    let y = 2.0 * rng_nextf(rng) - 1.0;

    let absx = abs(x);
    let absy = abs(y);

    var perimeter: f32;
    var side: f32;

    if (absx >= absy) {
        if (x >= absy) {
            perimeter = absx + y;
        } else {
            perimeter = 5.0 * absx - y;
        }
        side = absx;
    } else {
        if (y >= absx) {
            perimeter = 3.0 * absy - x;
        } else {
            perimeter = 7.0 * absy + x;
        }
        side = absy;
    }

    let r = side;
    let angle = PI_4 * perimeter / side - PI_4;

    return vec2<f32>(r * cos(angle), r * sin(angle));
}
"#,
    wgsl_3d: Some(r#"
fn variation_blur_circle(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Circle - 3D (Z passes through)
    const PI: f32 = 3.14159265359;
    const PI_4: f32 = 0.78539816339;

    let x = 2.0 * rng_nextf(rng) - 1.0;
    let y = 2.0 * rng_nextf(rng) - 1.0;

    let absx = abs(x);
    let absy = abs(y);

    var perimeter: f32;
    var side: f32;

    if (absx >= absy) {
        if (x >= absy) {
            perimeter = absx + y;
        } else {
            perimeter = 5.0 * absx - y;
        }
        side = absx;
    } else {
        if (y >= absx) {
            perimeter = 3.0 * absy - x;
        } else {
            perimeter = 7.0 * absy + x;
        }
        side = absy;
    }

    let r = side;
    let angle = PI_4 * perimeter / side - PI_4;

    return vec3<f32>(r * cos(angle), r * sin(angle), p.z);
}
"#),
};

pub static BLUR_ZOOM: VariationDef = VariationDef {
    name: "blur_zoom",
    display_name: "Blur Zoom",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("length", "Length", unlimited_float, 0.0, -5.0, 5.0),
        param!("x", "X", unlimited_float, 0.0, -20.0, 20.0),
        param!("y", "Y", unlimited_float, 0.0, -20.0, 20.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_blur_zoom(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Zoom - zoom blur from a center point
    let length = get_param(xform_id, variation_id, 0u);
    let zoom_x = get_param(xform_id, variation_id, 1u);
    let zoom_y = get_param(xform_id, variation_id, 2u);

    let z = 1.0 + length * rng_nextf(rng);

    return vec2<f32>(
        (p.x - zoom_x) * z + zoom_x,
        (p.y - zoom_y) * z + zoom_y
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_blur_zoom(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Zoom - 3D (Z passes through)
    let length = get_param(xform_id, variation_id, 0u);
    let zoom_x = get_param(xform_id, variation_id, 1u);
    let zoom_y = get_param(xform_id, variation_id, 2u);

    let z = 1.0 + length * rng_nextf(rng);

    return vec3<f32>(
        (p.x - zoom_x) * z + zoom_x,
        (p.y - zoom_y) * z + zoom_y,
        p.z
    );
}
"#),
};

pub static BLUR_PIXELIZE: VariationDef = VariationDef {
    name: "blur_pixelize",
    display_name: "Blur Pixelize",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("size", "Size", unlimited_float, 0.1, 0.0000001, 10.0),
        param!("scale", "Scale", unlimited_float, 1.0, -20.0, 20.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_blur_pixelize(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Pixelize - pixelated/mosaic blur effect
    let size = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);

    let inv_size = 1.0 / size;

    let x = floor(p.x * inv_size);
    let y = floor(p.y * inv_size);

    return vec2<f32>(
        size * (x + scale * (rng_nextf(rng) - 0.5) + 0.5),
        size * (y + scale * (rng_nextf(rng) - 0.5) + 0.5)
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_blur_pixelize(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Pixelize - 3D (Z passes through)
    let size = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);

    let inv_size = 1.0 / size;

    let x = floor(p.x * inv_size);
    let y = floor(p.y * inv_size);

    return vec3<f32>(
        size * (x + scale * (rng_nextf(rng) - 0.5) + 0.5),
        size * (y + scale * (rng_nextf(rng) - 0.5) + 0.5),
        p.z
    );
}
"#),
};

pub static SEPARATION: VariationDef = VariationDef {
    name: "separation",
    display_name: "Separation",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("x", "X", unlimited_float, 1.0, -20.0, 20.0),
        param!("y", "Y", unlimited_float, 1.0, -20.0, 20.0),
        param!("xinside", "X Inside", unlimited_float, 0.0, -20.0, 20.0),
        param!("yinside", "Y Inside", unlimited_float, 0.0, -20.0, 20.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_separation(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Separation - separates positive/negative quadrants
    let sep_x = get_param(xform_id, variation_id, 0u);
    let sep_y = get_param(xform_id, variation_id, 1u);
    let xinside = get_param(xform_id, variation_id, 2u);
    let yinside = get_param(xform_id, variation_id, 3u);

    let x_out = select(
        -(sqrt(p.x * p.x + sep_x * sep_x) + p.x * xinside),
        sqrt(p.x * p.x + sep_x * sep_x) - p.x * xinside,
        p.x > 0.0
    );

    let y_out = select(
        -(sqrt(p.y * p.y + sep_y * sep_y) + p.y * yinside),
        sqrt(p.y * p.y + sep_y * sep_y) - p.y * yinside,
        p.y > 0.0
    );

    return vec2<f32>(x_out, y_out);
}
"#,
    wgsl_3d: Some(r#"
fn variation_separation(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Separation - 3D (Z passes through)
    let sep_x = get_param(xform_id, variation_id, 0u);
    let sep_y = get_param(xform_id, variation_id, 1u);
    let xinside = get_param(xform_id, variation_id, 2u);
    let yinside = get_param(xform_id, variation_id, 3u);

    let x_out = select(
        -(sqrt(p.x * p.x + sep_x * sep_x) + p.x * xinside),
        sqrt(p.x * p.x + sep_x * sep_x) - p.x * xinside,
        p.x > 0.0
    );

    let y_out = select(
        -(sqrt(p.y * p.y + sep_y * sep_y) + p.y * yinside),
        sqrt(p.y * p.y + sep_y * sep_y) - p.y * yinside,
        p.y > 0.0
    );

    return vec3<f32>(x_out, y_out, p.z);
}
"#),
};

pub static MOBIUS: VariationDef = VariationDef {
    name: "mobius",
    display_name: "Mobius",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("re_a", "Re A", unlimited_float, 1.0, -20.0, 20.0),
        param!("im_a", "Im A", unlimited_float, 0.0, -20.0, 20.0),
        param!("re_b", "Re B", unlimited_float, 0.0, -20.0, 20.0),
        param!("im_b", "Im B", unlimited_float, 0.0, -20.0, 20.0),
        param!("re_c", "Re C", unlimited_float, 0.0, -20.0, 20.0),
        param!("im_c", "Im C", unlimited_float, 0.0, -20.0, 20.0),
        param!("re_d", "Re D", unlimited_float, 1.0, -20.0, 20.0),
        param!("im_d", "Im D", unlimited_float, 0.0, -20.0, 20.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_mobius(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Mobius - Möbius transformation f(z) = (Az + B)/(Cz + D)
    let re_a = get_param(xform_id, variation_id, 0u);
    let im_a = get_param(xform_id, variation_id, 1u);
    let re_b = get_param(xform_id, variation_id, 2u);
    let im_b = get_param(xform_id, variation_id, 3u);
    let re_c = get_param(xform_id, variation_id, 4u);
    let im_c = get_param(xform_id, variation_id, 5u);
    let re_d = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;

    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;

    let v_denom = re_v * re_v + im_v * im_v + 1e-10;

    return vec2<f32>(
        (re_u * re_v + im_u * im_v) / v_denom,
        (im_u * re_v - re_u * im_v) / v_denom
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_mobius(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Mobius - 3D (Z passes through)
    let re_a = get_param(xform_id, variation_id, 0u);
    let im_a = get_param(xform_id, variation_id, 1u);
    let re_b = get_param(xform_id, variation_id, 2u);
    let im_b = get_param(xform_id, variation_id, 3u);
    let re_c = get_param(xform_id, variation_id, 4u);
    let im_c = get_param(xform_id, variation_id, 5u);
    let re_d = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;

    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;

    let v_denom = re_v * re_v + im_v * im_v + 1e-10;

    return vec3<f32>(
        (re_u * re_v + im_u * im_v) / v_denom,
        (im_u * re_v - re_u * im_v) / v_denom,
        p.z
    );
}
"#),
};

pub static CROP: VariationDef = VariationDef {
    name: "crop",
    display_name: "Crop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
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
fn variation_crop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Crop - crops to a rectangular region with optional scatter
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
fn variation_crop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Crop - 3D (Z passes through)
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
