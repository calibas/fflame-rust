//! Advanced 2D variations (indices 5-15, 24+)
//!
//! More complex 2D variations including polar coordinates, Julia sets, etc.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

pub static POLAR: VariationDef = VariationDef {
    name: "polar",
    display_name: "Polar",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_polar(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p);
    return vec2<f32>(theta / 3.14159265359, r - 1.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_polar(p: vec3<f32>) -> vec3<f32> {
    let theta = atan2(p.x, p.y);
    let r = length(p.xy);
    return vec3<f32>(theta / 3.14159265359, r - 1.0, p.z);
}
"#),
};

pub static HANDKERCHIEF: VariationDef = VariationDef {
    name: "handkerchief",
    display_name: "Handkerchief",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_handkerchief(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2<f32>(r * sin(theta + r), r * cos(theta - r));
}
"#,
    wgsl_3d: Some(r#"
fn variation_handkerchief(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);
    return vec3<f32>(r * sin(theta + r), r * cos(theta - r), p.z);
}
"#),
};

pub static HEART: VariationDef = VariationDef {
    name: "heart",
    display_name: "Heart",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_heart(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_theta = r * theta;
    return vec2<f32>(r * sin(r_theta), -r * cos(r_theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_heart(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.x, p.y);
    let r_theta = r * theta;
    return vec3<f32>(r * sin(r_theta), -r * cos(r_theta), p.z);
}
"#),
};

pub static DISC: VariationDef = VariationDef {
    name: "disc",
    display_name: "Disc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_disc(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p);
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec2<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r));
}
"#,
    wgsl_3d: Some(r#"
fn variation_disc(p: vec3<f32>) -> vec3<f32> {
    let theta = atan2(p.x, p.y);
    let r = length(p.xy);
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec3<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r), p.z);
}
"#),
};

pub static SPIRAL: VariationDef = VariationDef {
    name: "spiral",
    display_name: "Spiral",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_spiral(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_inv = 1.0 / r;
    return vec2<f32>(
        r_inv * (cos(theta) + sin(r)),
        r_inv * (sin(theta) - cos(r))
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_spiral(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy) + 1e-6;
    let theta = atan2(p.x, p.y);
    let r_inv = 1.0 / r;
    return vec3<f32>(
        r_inv * (cos(theta) + sin(r)),
        r_inv * (sin(theta) - cos(r)),
        p.z
    );
}
"#),
};

pub static HYPERBOLIC: VariationDef = VariationDef {
    name: "hyperbolic",
    display_name: "Hyperbolic",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_hyperbolic(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;
    return vec2<f32>(p.x / r2, p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_hyperbolic(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy) + 1e-6;
    return vec3<f32>(p.x / r2, p.y, p.z);
}
"#),
};

pub static DIAMOND: VariationDef = VariationDef {
    name: "diamond",
    display_name: "Diamond",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_diamond(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    return vec2<f32>(sin(theta) * cos(r), cos(theta) * sin(r));
}
"#,
    wgsl_3d: Some(r#"
fn variation_diamond(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.x, p.y);
    return vec3<f32>(sin(theta) * cos(r), cos(theta) * sin(r), p.z);
}
"#),
};

pub static EX: VariationDef = VariationDef {
    name: "ex",
    display_name: "Ex",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ex(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let n0 = sin(theta + r);
    let n1 = cos(theta - r);
    let m0 = n0 * n0 * n0;  // n0³
    let m1 = n1 * n1 * n1;  // n1³
    return vec2<f32>(r * (m0 + m1), r * (m0 - m1));
}
"#,
    wgsl_3d: Some(r#"
fn variation_ex(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);
    let n0 = sin(theta + r);
    let n1 = cos(theta - r);
    let m0 = n0 * n0 * n0;
    let m1 = n1 * n1 * n1;
    return vec3<f32>(r * (m0 + m1), r * (m0 - m1), p.z);
}
"#),
};

pub static JULIA: VariationDef = VariationDef {
    name: "julia",
    display_name: "Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn julia(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let sqrt_r = sqrt(r);
    let omega = select(0.0, 3.14159265359, rng_nextf(rng) < 0.5);
    let half_theta = theta / 2.0 + omega;
    return vec2<f32>(sqrt_r * cos(half_theta), sqrt_r * sin(half_theta));
}
"#,
    wgsl_3d: Some(r#"
fn julia(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);
    let sqrt_r = sqrt(r);
    let omega = select(0.0, 3.14159265359, rng_nextf(rng) < 0.5);
    let half_theta = theta / 2.0 + omega;
    return vec3<f32>(sqrt_r * cos(half_theta), sqrt_r * sin(half_theta), p.z);
}
"#),
};

pub static BENT: VariationDef = VariationDef {
    name: "bent",
    display_name: "Bent",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bent(p: vec2<f32>) -> vec2<f32> {
    let nx = select(2.0 * p.x, p.x, p.x >= 0.0);
    let ny = select(p.y / 2.0, p.y, p.y >= 0.0);
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bent(p: vec3<f32>) -> vec3<f32> {
    let nx = select(2.0 * p.x, p.x, p.x >= 0.0);
    let ny = select(p.y / 2.0, p.y, p.y >= 0.0);
    return vec3<f32>(nx, ny, p.z);
}
"#),
};

// waves: Scott Draves's classic waves variation, using affine matrix fields.
//   x' = x + b · sin(y / (c² + ε))
//   y' = y + e · sin(x / (f² + ε))
// In the standard Apophysis affine notation, b/c/e/f are read directly from
// the transform's affine matrix:
//   our `xform.b`  (= XFORM_COEFF_10) → X amplitude
//   our `xform.e`  (= XFORM_COEFF_20, X translation) → X wavelength (squared)
//   our `xform.d`  (= XFORM_COEFF_11) → Y amplitude
//   our `xform.f`  (= XFORM_COEFF_21, Y translation) → Y wavelength (squared)
//
// First variation to use `needs_transform: true` — the body reads from the
// `transforms` storage buffer via xform_id.
pub static WAVES: VariationDef = VariationDef {
    name: "waves",
    display_name: "Waves",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_waves(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xf = transforms[xform_id];
    return vec2<f32>(
        p.x + xf.b * sin(p.y / (xf.e * xf.e + 1e-6)),
        p.y + xf.d * sin(p.x / (xf.f * xf.f + 1e-6))
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_waves(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xf = transforms[xform_id];
    return vec3<f32>(
        p.x + xf.b * sin(p.y / (xf.e * xf.e + 1e-6)),
        p.y + xf.d * sin(p.x / (xf.f * xf.f + 1e-6)),
        p.z
    );
}
"#),
};

// Parameterized variations

pub static JULIAN: VariationDef = VariationDef {
    name: "julian",
    display_name: "JuliaN",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef {
            name: "power",
            display_name: "Power",
            param_type: ParamType::UnlimitedInteger,
            default_value: 2.0,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: None,
        },
        VariationParamDef {
            name: "dist",
            display_name: "Distance",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-100.0),
            max_value: Some(100.0),
            description: None,
        },
    ],
    needs_transform: false,
    // 1 derived value at slot 2:
    //   2: cpower  (dist / |power| / 2)
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_julian(user: array<f32, 2>) -> array<f32, 1> {
    let power = user[0];
    let dist = user[1];
    var out: array<f32, 1>;
    out[0] = dist / max(abs(power), 1e-30) / 2.0;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_julian(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let cpower = get_param(xform_id, variation_id, 2u);

    let r2 = dot(p, p);
    let r = pow(r2, cpower);
    let theta = atan2(p.y, p.x);

    let trunc_val = floor(abs(power) * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec2<f32>(r * cos(t), r * sin(t));
}
"#,
    wgsl_3d: Some(r#"
fn variation_julian(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let cpower = get_param(xform_id, variation_id, 2u);

    let r2 = dot(p.xy, p.xy);
    let r = pow(r2, cpower);
    let theta = atan2(p.y, p.x);

    let trunc_val = floor(abs(power) * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec3<f32>(r * cos(t), r * sin(t), p.z);
}
"#),
};

pub static BLOB: VariationDef = VariationDef {
    name: "blob",
    display_name: "Blob",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "high",
            display_name: "High",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-3.0),
            max_value: Some(3.0),
            description: None,
        },
        VariationParamDef {
            name: "low",
            display_name: "Low",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-3.0),
            max_value: Some(3.0),
            description: None,
        },
        VariationParamDef {
            name: "waves",
            display_name: "Waves",
            param_type: ParamType::UnlimitedInteger,
            default_value: 6.0,
            min_value: Some(1.0),
            max_value: Some(20.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_blob(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let p1 = get_param(xform_id, variation_id, 0u);  // high
    let p2 = get_param(xform_id, variation_id, 1u);  // low
    let p3 = get_param(xform_id, variation_id, 2u);  // waves

    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)

    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec2<f32>(scale * cos(theta), scale * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_blob(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let p1 = get_param(xform_id, variation_id, 0u);
    let p2 = get_param(xform_id, variation_id, 1u);
    let p3 = get_param(xform_id, variation_id, 2u);

    let r = length(p.xy);
    let theta = atan2(p.x, p.y);

    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec3<f32>(scale * cos(theta), scale * sin(theta), p.z);
}
"#),
};

pub static EYEFISH: VariationDef = VariationDef {
    name: "eyefish",
    display_name: "Eyefish",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eyefish(p: vec2<f32>) -> vec2<f32> {
    let r_xy = length(p) + 1.0;
    let scale = 2.0 / r_xy;
    return vec2<f32>(scale * p.x, scale * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_eyefish(p: vec3<f32>) -> vec3<f32> {
    let r_xy = length(p.xy) + 1.0;
    let scale = 2.0 / r_xy;
    return vec3<f32>(scale * p.x, scale * p.y, p.z);
}
"#),
};

pub static BUBBLE: VariationDef = VariationDef {
    name: "bubble",
    display_name: "Bubble",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bubble(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let r = r2 / 4.0 + 1.0;
    let scale = 1.0 / r;
    return vec2<f32>(scale * p.x, scale * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bubble(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: r = (x² + y²)/4 + 1
    // z' = 2/r - 1, scale = 1/r
    let r2_xy = dot(p.xy, p.xy);
    let r = r2_xy / 4.0 + 1.0;
    let scale = 1.0 / r;
    let new_z = 2.0 / r - 1.0;
    return vec3<f32>(scale * p.x, scale * p.y, new_z);
}
"#),
};

pub static CYLINDER: VariationDef = VariationDef {
    name: "cylinder",
    display_name: "Cylinder",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cylinder(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(p.x), p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cylinder(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: result = (sin(x), y, cos(x))
    return vec3<f32>(sin(p.x), p.y, cos(p.x));
}
"#),
};

pub static NOISE: VariationDef = VariationDef {
    name: "noise",
    display_name: "Noise",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_noise(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec2<f32>(p.x * r * cos(theta), p.y * r * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_noise(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng);
    return vec3<f32>(p.x * r * cos(theta), p.y * r * sin(theta), p.z);
}
"#),
};

pub static BLUR: VariationDef = VariationDef {
    name: "blur",
    display_name: "Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec2<f32>(r * cos(theta), r * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng);
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}
"#),
};

pub static GAUSSIAN_BLUR: VariationDef = VariationDef {
    name: "gaussian_blur",
    display_name: "Gaussian Blur",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_gaussian_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}
"#,
    wgsl_3d: Some(r#"
fn variation_gaussian_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let theta = rng_nextf(rng) * 6.28318530718;
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}
"#),
};

// Extended variations

pub static POLAR2: VariationDef = VariationDef {
    name: "polar2",
    display_name: "Polar2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_polar2(p: vec2<f32>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let r2 = dot(p, p);
    let new_x = atan2(p.x, p.y) / PI;
    let new_y = 0.5 * log(r2) / PI;
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_polar2(p: vec3<f32>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let r2 = dot(p.xy, p.xy);
    let new_x = atan2(p.x, p.y) / PI;
    let new_y = 0.5 * log(r2) / PI;
    return vec3<f32>(new_x, new_y, p.z);
}
"#),
};

pub static CROSS: VariationDef = VariationDef {
    name: "cross",
    display_name: "Cross",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cross(p: vec2<f32>) -> vec2<f32> {
    var r = abs((p.x - p.y) * (p.x + p.y) + 1e-6);
    if (r < 0.0) { r = r * -1.0; }
    r = 1.0 / r;
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cross(p: vec3<f32>) -> vec3<f32> {
    var r = abs((p.x - p.y) * (p.x + p.y) + 1e-6);
    if (r < 0.0) { r = r * -1.0; }
    r = 1.0 / r;
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#),
};

pub static LOONIE: VariationDef = VariationDef {
    name: "loonie",
    display_name: "Loonie",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_loonie(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    if (r2 < 1.0 && r2 != 0.0) {
        let r = sqrt(1.0 / r2 - 1.0);
        return vec2<f32>(p.x * r, p.y * r);
    } else {
        return p;
    }
}
"#,
    wgsl_3d: Some(r#"
fn variation_loonie(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy);
    if (r2 < 1.0 && r2 != 0.0) {
        let r = sqrt(1.0 / r2 - 1.0);
        return vec3<f32>(p.x * r, p.y * r, p.z);
    } else {
        return p;
    }
}
"#),
};

pub static SCRY: VariationDef = VariationDef {
    name: "scry",
    display_name: "Scry",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_scry(p: vec2<f32>) -> vec2<f32> {
    let t = dot(p, p);
    var r = 1.0 / (sqrt(t) * (t + 1.0));
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_scry(p: vec3<f32>) -> vec3<f32> {
    let t = dot(p.xy, p.xy);
    var r = 1.0 / (sqrt(t) * (t + 1.0));
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#),
};

pub static FOCI: VariationDef = VariationDef {
    name: "foci",
    display_name: "Foci",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_foci(p: vec2<f32>) -> vec2<f32> {
    let expx = exp(p.x) * 0.5;
    let expnx = 0.5 * exp(-p.x);
    var tmp = expx + expnx - cos(p.y);
    if (tmp == 0.0) { tmp = 1e-6; }
    tmp = 1.0 / tmp;
    let new_x = (expx - expnx) * tmp;
    let new_y = sin(p.y) * tmp;
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_foci(p: vec3<f32>) -> vec3<f32> {
    let expx = exp(p.x) * 0.5;
    let expnx = 0.5 * exp(-p.x);
    var tmp = expx + expnx - cos(p.y);
    if (tmp == 0.0) { tmp = 1e-6; }
    tmp = 1.0 / tmp;
    let new_x = (expx - expnx) * tmp;
    let new_y = sin(p.y) * tmp;
    return vec3<f32>(new_x, new_y, p.z);
}
"#),
};

pub static ELLIPTIC: VariationDef = VariationDef {
    name: "elliptic",
    display_name: "Elliptic",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_elliptic(p: vec2<f32>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let v = 2.0 / PI;
    let tmp = dot(p, p) + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(tmp + x2) + sqrt(tmp - x2));
    let a = p.x / xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));
    let new_x = v * atan2(a, b);
    var new_y = v * log(xmax + sqrt(max(0.0, xmax - 1.0)));
    if (p.y < 0.0) { new_y = -new_y; }
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_elliptic(p: vec3<f32>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let v = 2.0 / PI;
    let tmp = dot(p.xy, p.xy) + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(tmp + x2) + sqrt(tmp - x2));
    let a = p.x / xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));
    let new_x = v * atan2(a, b);
    var new_y = v * log(xmax + sqrt(max(0.0, xmax - 1.0)));
    if (p.y < 0.0) { new_y = -new_y; }
    return vec3<f32>(new_x, new_y, p.z);
}
"#),
};

// Parameterized extended variations

pub static WAVES2: VariationDef = VariationDef {
    name: "waves2",
    display_name: "Waves2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "freqx",
            display_name: "Freq X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.0,
            min_value: Some(0.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "scalex",
            display_name: "Scale X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "freqy",
            display_name: "Freq Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.0,
            min_value: Some(0.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "scaley",
            display_name: "Scale Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "freqz",
            display_name: "Freq Z",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(0.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "scalez",
            display_name: "Scale Z",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_waves2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let freqx = get_param(xform_id, variation_id, 0u);
    let scalex = get_param(xform_id, variation_id, 1u);
    let freqy = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);
    let new_x = p.x + scalex * sin(p.y * freqx);
    let new_y = p.y + scaley * sin(p.x * freqy);
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_waves2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Waves2: Sine wave distortion with 6 parameters (3D version)
    let freqx = get_param(xform_id, variation_id, 0u);
    let scalex = get_param(xform_id, variation_id, 1u);
    let freqy = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);
    let freqz = get_param(xform_id, variation_id, 4u);
    let scalez = get_param(xform_id, variation_id, 5u);
    let r_xy = length(p.xy);
    let new_x = p.x + scalex * sin(p.y * freqx);
    let new_y = p.y + scaley * sin(p.x * freqy);
    let new_z = p.z + scalez * sin(r_xy * freqz);
    return vec3<f32>(new_x, new_y, new_z);
}
"#),
};

pub static LOG: VariationDef = VariationDef {
    name: "log",
    display_name: "Log",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "base",
            display_name: "Base",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.718281828,
            min_value: Some(1.01),
            max_value: Some(100.0),
            description: None,
        },
    ],
    needs_transform: false,
    // 1 derived value at slot 1:
    //   1: denom  (0.5 / log(base))
    writes_color: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_log(user: array<f32, 1>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 0.5 / log(max(user[0], 1.000001));
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_log(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let denom = get_param(xform_id, variation_id, 1u);
    let r2 = dot(p, p);
    return vec2<f32>(log(r2) * denom, atan2(p.y, p.x));
}
"#,
    wgsl_3d: Some(r#"
fn variation_log(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let denom = get_param(xform_id, variation_id, 1u);
    let r2 = dot(p.xy, p.xy);
    return vec3<f32>(log(r2) * denom, atan2(p.y, p.x), p.z);
}
"#),
};

pub static ESCHER: VariationDef = VariationDef {
    name: "escher",
    display_name: "Escher",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "beta",
            display_name: "Beta",
            param_type: ParamType::Angle,
            default_value: 0.0,
            min_value: Some(-180.0),
            max_value: Some(180.0),
            description: None,
        },
    ],
    needs_transform: false,
    // 2 derived values at slots 1..3:
    //   1: c  (0.5 · (1 + cos(beta·π/180)))
    //   2: d  (0.5 · sin(beta·π/180))
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_escher(user: array<f32, 1>) -> array<f32, 2> {
    let beta = user[0] * 3.14159265358979 / 180.0;
    var out: array<f32, 2>;
    out[0] = 0.5 * (1.0 + cos(beta));  // c
    out[1] = 0.5 * sin(beta);          // d
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_escher(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let c = get_param(xform_id, variation_id, 1u);
    let d = get_param(xform_id, variation_id, 2u);
    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p, p));
    let m = exp(c * lnr - d * a);
    let angle = c * a + d * lnr;
    return vec2<f32>(m * cos(angle), m * sin(angle));
}
"#,
    wgsl_3d: Some(r#"
fn variation_escher(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let c = get_param(xform_id, variation_id, 1u);
    let d = get_param(xform_id, variation_id, 2u);
    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p.xy, p.xy));
    let m = exp(c * lnr - d * a);
    let angle = c * a + d * lnr;
    return vec3<f32>(m * cos(angle), m * sin(angle), p.z);
}
"#),
};

pub static BIPOLAR: VariationDef = VariationDef {
    name: "bipolar",
    display_name: "Bipolar",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "shift",
            display_name: "Shift",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bipolar(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    const HALF_PI: f32 = 1.57079632679;
    let shift = get_param(xform_id, variation_id, 0u);
    let x2y2 = dot(p, p);
    var y = 0.5 * atan2(2.0 * p.y, x2y2 - 1.0) + (-HALF_PI * shift);
    if (y > HALF_PI) {
        y = -HALF_PI + (y + HALF_PI) % PI;
    } else if (y < -HALF_PI) {
        y = HALF_PI - (HALF_PI - y) % PI;
    }
    let t = x2y2 + 1.0;
    let x2 = 2.0 * p.x;
    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return vec2<f32>(0.0, 0.0);
    }
    let new_x = (1.0 / (2.0 * PI)) * log(f / g);
    let new_y = (2.0 / PI) * y;
    return vec2<f32>(new_x, new_y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bipolar(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    const HALF_PI: f32 = 1.57079632679;
    let shift = get_param(xform_id, variation_id, 0u);
    let x2y2 = dot(p.xy, p.xy);
    var y = 0.5 * atan2(2.0 * p.y, x2y2 - 1.0) + (-HALF_PI * shift);
    if (y > HALF_PI) {
        y = -HALF_PI + (y + HALF_PI) % PI;
    } else if (y < -HALF_PI) {
        y = HALF_PI - (HALF_PI - y) % PI;
    }
    let t = x2y2 + 1.0;
    let x2 = 2.0 * p.x;
    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let new_x = (1.0 / (2.0 * PI)) * log(f / g);
    let new_y = (2.0 / PI) * y;
    return vec3<f32>(new_x, new_y, p.z);
}
"#),
};

pub static LAZYSUSAN: VariationDef = VariationDef {
    name: "lazysusan",
    display_name: "LazySusan",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "spin",
            display_name: "Spin",
            param_type: ParamType::Angle,
            default_value: 0.0,
            min_value: Some(-360.0),
            max_value: Some(360.0),
            description: None,
        },
        VariationParamDef {
            name: "space",
            display_name: "Space",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "twist",
            display_name: "Twist",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-5.0),
            max_value: Some(5.0),
            description: None,
        },
        VariationParamDef {
            name: "x",
            display_name: "X Offset",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "y",
            display_name: "Y Offset",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_lazysusan(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let spin = get_param(xform_id, variation_id, 0u) * PI / 180.0;
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_offset = get_param(xform_id, variation_id, 3u);
    let y_offset = get_param(xform_id, variation_id, 4u);
    let x = p.x - x_offset;
    let y = p.y + y_offset;
    let r = sqrt(x * x + y * y);
    if (r < 1.0) {
        let a = atan2(y, x) + spin + twist * (1.0 - r);
        return vec2<f32>(r * cos(a) + x_offset, r * sin(a) - y_offset);
    } else {
        let r_scale = 1.0 + space / (r + 1e-6);
        return vec2<f32>(r_scale * x + x_offset, r_scale * y - y_offset);
    }
}
"#,
    wgsl_3d: Some(r#"
fn variation_lazysusan(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let spin = get_param(xform_id, variation_id, 0u) * PI / 180.0;
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_offset = get_param(xform_id, variation_id, 3u);
    let y_offset = get_param(xform_id, variation_id, 4u);
    let x = p.x - x_offset;
    let y = p.y + y_offset;
    let r = sqrt(x * x + y * y);
    if (r < 1.0) {
        let a = atan2(y, x) + spin + twist * (1.0 - r);
        return vec3<f32>(r * cos(a) + x_offset, r * sin(a) - y_offset, p.z);
    } else {
        let r_scale = 1.0 + space / (r + 1e-6);
        return vec3<f32>(r_scale * x + x_offset, r_scale * y - y_offset, p.z);
    }
}
"#),
};

pub static RINGS2: VariationDef = VariationDef {
    name: "rings2",
    display_name: "Rings2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "val",
            display_name: "Value",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_rings2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let val = get_param(xform_id, variation_id, 0u);
    let dx = val * val + 1e-10;
    let length = sqrt(dot(p, p));
    let r = 2.0 - dx * (floor((length / dx + 1.0) / 2.0) * 2.0 / length + 1.0);
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_rings2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let val = get_param(xform_id, variation_id, 0u);
    let dx = val * val + 1e-10;
    let length = sqrt(dot(p.xy, p.xy));
    let r = 2.0 - dx * (floor((length / dx + 1.0) / 2.0) * 2.0 / length + 1.0);
    return vec3<f32>(p.x * r, p.y * r, p.z);
}
"#),
};

pub static FAN2: VariationDef = VariationDef {
    name: "fan2",
    display_name: "Fan2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "x",
            display_name: "X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: None,
        },
        VariationParamDef {
            name: "y",
            display_name: "Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-5.0),
            max_value: Some(5.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_fan2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let x_param = get_param(xform_id, variation_id, 0u);
    let y_param = get_param(xform_id, variation_id, 1u);
    let dx = PI * (x_param * x_param + 1e-10);
    let dx2 = dx / 2.0;
    let angle = atan2(p.x, p.y);
    var a: f32;
    if (fract((angle + y_param) / dx) > 0.5) {
        a = angle - dx2;
    } else {
        a = angle + dx2;
    }
    let r = sqrt(dot(p, p));
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_fan2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let x_param = get_param(xform_id, variation_id, 0u);
    let y_param = get_param(xform_id, variation_id, 1u);
    let dx = PI * (x_param * x_param + 1e-10);
    let dx2 = dx / 2.0;
    let angle = atan2(p.x, p.y);
    var a: f32;
    if (fract((angle + y_param) / dx) > 0.5) {
        a = angle - dx2;
    } else {
        a = angle + dx2;
    }
    let r = sqrt(dot(p.xy, p.xy));
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

pub static PDJ: VariationDef = VariationDef {
    name: "pdj",
    display_name: "PDJ",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "a",
            display_name: "A",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "b",
            display_name: "B",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "c",
            display_name: "C",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "d",
            display_name: "D",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pdj(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    return vec2<f32>(
        sin(a * p.y) - cos(b * p.x),
        sin(c * p.x) - cos(d * p.y)
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_pdj(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    return vec3<f32>(
        sin(a * p.y) - cos(b * p.x),
        sin(c * p.x) - cos(d * p.y),
        p.z
    );
}
"#),
};

pub static CURL: VariationDef = VariationDef {
    name: "curl",
    display_name: "Curl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "c1",
            display_name: "C1",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "c2",
            display_name: "C2",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_curl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
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
fn variation_curl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
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

pub static RECTANGLES: VariationDef = VariationDef {
    name: "rectangles",
    display_name: "Rectangles",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "x",
            display_name: "X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: None,
        },
        VariationParamDef {
            name: "y",
            display_name: "Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.01),
            max_value: Some(5.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_rectangles(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let rect_x = get_param(xform_id, variation_id, 0u);
    let rect_y = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(
        (2.0 * floor(p.x / rect_x) + 1.0) * rect_x - p.x,
        (2.0 * floor(p.y / rect_y) + 1.0) * rect_y - p.y
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_rectangles(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let rect_x = get_param(xform_id, variation_id, 0u);
    let rect_y = get_param(xform_id, variation_id, 1u);
    return vec3<f32>(
        (2.0 * floor(p.x / rect_x) + 1.0) * rect_x - p.x,
        (2.0 * floor(p.y / rect_y) + 1.0) * rect_y - p.y,
        p.z
    );
}
"#),
};

pub static SPLITS: VariationDef = VariationDef {
    name: "splits",
    display_name: "Splits",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "x",
            display_name: "X",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.4,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "y",
            display_name: "Y",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.4,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_splits(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let splits_x = get_param(xform_id, variation_id, 0u);
    let splits_y = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(
        select(p.x - splits_x, p.x + splits_x, p.x >= 0.0),
        select(p.y - splits_y, p.y + splits_y, p.y >= 0.0)
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_splits(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let splits_x = get_param(xform_id, variation_id, 0u);
    let splits_y = get_param(xform_id, variation_id, 1u);
    return vec3<f32>(
        select(p.x - splits_x, p.x + splits_x, p.x >= 0.0),
        select(p.y - splits_y, p.y + splits_y, p.y >= 0.0),
        p.z
    );
}
"#),
};

pub static NGON: VariationDef = VariationDef {
    name: "ngon",
    display_name: "Ngon",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "sides",
            display_name: "Sides",
            param_type: ParamType::UnlimitedInteger,
            default_value: 5.0,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: None,
        },
        VariationParamDef {
            name: "power",
            display_name: "Power",
            param_type: ParamType::UnlimitedFloat,
            default_value: 3.0,
            min_value: Some(-2.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "circle",
            display_name: "Circle",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: None,
        },
        VariationParamDef {
            name: "corners",
            display_name: "Corners",
            param_type: ParamType::UnlimitedFloat,
            default_value: 2.0,
            min_value: Some(-20.0),
            max_value: Some(20.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ngon(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    let sides = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let circle = get_param(xform_id, variation_id, 2u);
    let corners = get_param(xform_id, variation_id, 3u);
    let theta = atan2(p.y, p.x);
    let phi = theta - (PI * 2.0 / sides) * floor(sides * theta / (PI * 2.0));
    let phi_adj = select(phi, phi - 2.0 * PI / sides, phi > PI / sides);
    let amp = cos(phi_adj) * pow(1.0 / (cos(phi_adj * sides / 2.0) + 1e-10), circle);
    let r = pow(dot(p, p), power * 0.5);
    return vec2<f32>(
        amp * r * cos(theta) + corners,
        amp * r * sin(theta)
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_ngon(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let sides = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let circle = get_param(xform_id, variation_id, 2u);
    let corners = get_param(xform_id, variation_id, 3u);
    let theta = atan2(p.y, p.x);
    let phi = theta - (PI * 2.0 / sides) * floor(sides * theta / (PI * 2.0));
    let phi_adj = select(phi, phi - 2.0 * PI / sides, phi > PI / sides);
    let amp = cos(phi_adj) * pow(1.0 / (cos(phi_adj * sides / 2.0) + 1e-10), circle);
    let r = pow(dot(p.xy, p.xy), power * 0.5);
    return vec3<f32>(
        amp * r * cos(theta) + corners,
        amp * r * sin(theta),
        p.z
    );
}
"#),
};

pub static AUGER: VariationDef = VariationDef {
    name: "auger",
    display_name: "Auger",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef {
            name: "freq",
            display_name: "Frequency",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(0.0),
            max_value: Some(10.0),
            description: None,
        },
        VariationParamDef {
            name: "weight",
            display_name: "Weight",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.5,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "scale",
            display_name: "Scale",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.5,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
        VariationParamDef {
            name: "sym",
            display_name: "Symmetry",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.0,
            min_value: Some(-2.0),
            max_value: Some(2.0),
            description: None,
        },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_auger(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let freq = get_param(xform_id, variation_id, 0u);
    let weight = get_param(xform_id, variation_id, 1u);
    let scale = get_param(xform_id, variation_id, 2u);
    let sym = get_param(xform_id, variation_id, 3u);
    let s = sin(freq * p.x);
    let t = sin(freq * p.y);
    let dx = p.x + weight * (0.5 * scale * t + abs(p.x) * t);
    let dy = p.y + weight * (0.5 * scale * s + abs(p.y) * s);
    return vec2<f32>(
        p.x + sym * (dx - p.x),
        dy
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_auger(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let freq = get_param(xform_id, variation_id, 0u);
    let weight = get_param(xform_id, variation_id, 1u);
    let scale = get_param(xform_id, variation_id, 2u);
    let sym = get_param(xform_id, variation_id, 3u);
    let s = sin(freq * p.x);
    let t = sin(freq * p.y);
    let dx = p.x + weight * (0.5 * scale * t + abs(p.x) * t);
    let dy = p.y + weight * (0.5 * scale * s + abs(p.y) * s);
    return vec3<f32>(
        p.x + sym * (dx - p.x),
        dy,
        p.z
    );
}
"#),
};

/// CPow — complex power variation (JWildfire port)
///
/// Reference: github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/cpow.cpp
///
/// Math:
///   a = atan2(y, x)
///   lnr = 0.5 * ln(x² + y²)
///   va = 2π / power, vc = r / power, vd = i / power
///   ang = vc*a + vd*lnr + va * floor(power * rand())
///   m = exp(vc*lnr - vd*a)
///   out = m * (cos(ang), sin(ang))
pub static CPOW: VariationDef = VariationDef {
    name: "cpow",
    display_name: "CPow",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef {
            name: "r",
            display_name: "R",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.0,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: None,
        },
        VariationParamDef {
            name: "i",
            display_name: "I",
            param_type: ParamType::UnlimitedFloat,
            default_value: 0.1,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: None,
        },
        VariationParamDef {
            name: "power",
            display_name: "Power",
            param_type: ParamType::UnlimitedFloat,
            default_value: 1.5,
            min_value: Some(-10.0),
            max_value: Some(10.0),
            description: None,
        },
    ],
    needs_transform: false,
    // 3 derived values at slots 3..6:
    //   3: va  (2π / power)
    //   4: vc  (r / power)
    //   5: vd  (i / power)
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_cpow(user: array<f32, 3>) -> array<f32, 3> {
    let r_param = user[0];
    let i_param = user[1];
    let power = user[2];
    let safe_power = select(power, 1e-30, power == 0.0);
    var out: array<f32, 3>;
    out[0] = 6.28318530717959 / safe_power;  // va
    out[1] = r_param / safe_power;           // vc
    out[2] = i_param / safe_power;           // vd
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cpow(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 2u);
    let va = get_param(xform_id, variation_id, 3u);
    let vc = get_param(xform_id, variation_id, 4u);
    let vd = get_param(xform_id, variation_id, 5u);

    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p, p) + 1e-20);

    let ang = vc * a + vd * lnr + va * floor(power * rng_nextf(rng));
    let m = exp(vc * lnr - vd * a);

    return vec2<f32>(m * cos(ang), m * sin(ang));
}
"#,
    wgsl_3d: Some(r#"
fn variation_cpow(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 2u);
    let va = get_param(xform_id, variation_id, 3u);
    let vc = get_param(xform_id, variation_id, 4u);
    let vd = get_param(xform_id, variation_id, 5u);

    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p.xy, p.xy) + 1e-20);

    let ang = vc * a + vd * lnr + va * floor(power * rng_nextf(rng));
    let m = exp(vc * lnr - vd * a);

    return vec3<f32>(m * cos(ang), m * sin(ang), p.z);
}
"#),
};
