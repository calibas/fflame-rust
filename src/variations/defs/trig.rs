//! Direct trigonometric and hyperbolic variations
//!
//! Ports from JWildfire/Chaotica. These compute the named complex function
//! of the input point. Most use the direct double-precision trig identities;
//! `sinh`, `tanh`, and `sech` use a complex-exponential form scaled by π/4.
//!
//! Notes on faithfulness to upstream:
//!   - Upstream `sech` actually computes csch(z·π/4) (its formula divides by
//!     `aux - 1/aux` rather than `aux + 1/aux`). Likely a JWildfire bug, but
//!     we preserve the behavior so JWildfire flames render the same here.
//!   - `sinh`, `tanh`, and `sech` operate on `z·π/4` (not `z`).
//!   - Upstream early-returns on exact divide-by-zero. We instead clamp the
//!     denominator's magnitude to a tiny epsilon — same visual result, no
//!     branch in the shader.
//!   - `sinh`, `tanh`, `sech` upstream lack Z preservation, so the 3D form
//!     of those returns z = 0.0 (i.e. contributes nothing to depth).

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

// =============================================================================
// sin: sin(x + iy) = sin(x)cosh(y) + i·cos(x)sinh(y)
// =============================================================================
/// Treats the input as a complex number and applies the sine function.
/// Output is `sin(x)*cosh(y), cos(x)*sinh(y)` — horizontally periodic,
/// growing vertically away from the real axis.
///
/// # Authors
/// - cothe
pub static SIN: VariationDef = VariationDef {
    name: "sin",
    aliases: &[],
    display_name: "Sin",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sin(p: vec2<f32>) -> vec2<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    return vec2<f32>(s * ch, c * sh);
}
"#,
    wgsl_3d: r#"
fn variation_sin(p: vec3<f32>) -> vec3<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    return vec3<f32>(s * ch, c * sh, p.z);
}
"#,
};

// =============================================================================
// cos: cos(x + iy) = cos(x)cosh(y) − i·sin(x)sinh(y)
// =============================================================================
/// Complex cosine. Same shape as Sin but shifted — output is
/// `cos(x)*cosh(y), -sin(x)*sinh(y)`.
///
/// # Authors
/// - cothe
pub static COS: VariationDef = VariationDef {
    name: "cos",
    aliases: &[],
    display_name: "Cos",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cos(p: vec2<f32>) -> vec2<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    return vec2<f32>(c * ch, -s * sh);
}
"#,
    wgsl_3d: r#"
fn variation_cos(p: vec3<f32>) -> vec3<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    return vec3<f32>(c * ch, -s * sh, p.z);
}
"#,
};

// =============================================================================
// tan: tan(x + iy) = (sin(2x) + i·sinh(2y)) / (cos(2x) + cosh(2y))
// =============================================================================
/// Complex tangent (sin/cos). Singularities at ±π/2 produce dramatic poles
/// in the output.
///
/// # Authors
/// - cothe
pub static TAN: VariationDef = VariationDef {
    name: "tan",
    aliases: &[],
    display_name: "Tan",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_tan(p: vec2<f32>) -> vec2<f32> {
    let s2 = sin(2.0 * p.x);
    let c2 = cos(2.0 * p.x);
    let sh2 = sinh(2.0 * p.y);
    let ch2 = cosh(2.0 * p.y);
    let d = c2 + ch2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * s2, inv * sh2);
}
"#,
    wgsl_3d: r#"
fn variation_tan(p: vec3<f32>) -> vec3<f32> {
    let s2 = sin(2.0 * p.x);
    let c2 = cos(2.0 * p.x);
    let sh2 = sinh(2.0 * p.y);
    let ch2 = cosh(2.0 * p.y);
    let d = c2 + ch2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * s2, inv * sh2, p.z);
}
"#,
};

// =============================================================================
// sec: 2(cos(x)cosh(y) + i·sin(x)sinh(y)) / (cos(2x) + cosh(2y))
// =============================================================================
/// Complex secant (1/cos). Singularities at ±π/2 create high-density rings
/// around the poles.
///
/// # Authors
/// - cothe
pub static SEC: VariationDef = VariationDef {
    name: "sec",
    aliases: &[],
    display_name: "Sec",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sec(p: vec2<f32>) -> vec2<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    let d = cos(2.0 * p.x) + cosh(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * c * ch, inv * s * sh);
}
"#,
    wgsl_3d: r#"
fn variation_sec(p: vec3<f32>) -> vec3<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    let d = cos(2.0 * p.x) + cosh(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * c * ch, inv * s * sh, p.z);
}
"#,
};

// =============================================================================
// csc: 2(sin(x)cosh(y) − i·cos(x)sinh(y)) / (cosh(2y) − cos(2x))
// =============================================================================
/// Complex cosecant (1/sin). Singularities at 0 and ±π.
///
/// # Authors
/// - cothe
pub static CSC: VariationDef = VariationDef {
    name: "csc",
    aliases: &[],
    display_name: "Csc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_csc(p: vec2<f32>) -> vec2<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    let d = cosh(2.0 * p.y) - cos(2.0 * p.x);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * s * ch, -inv * c * sh);
}
"#,
    wgsl_3d: r#"
fn variation_csc(p: vec3<f32>) -> vec3<f32> {
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(p.y); let ch = cosh(p.y);
    let d = cosh(2.0 * p.y) - cos(2.0 * p.x);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * s * ch, -inv * c * sh, p.z);
}
"#,
};

// =============================================================================
// cot: cot(x + iy) = (sin(2x) − i·sinh(2y)) / (cosh(2y) − cos(2x))
// =============================================================================
/// Complex cotangent (cos/sin). Singularities at 0 and ±π.
///
/// # Authors
/// - cothe
pub static COT: VariationDef = VariationDef {
    name: "cot",
    aliases: &[],
    display_name: "Cot",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cot(p: vec2<f32>) -> vec2<f32> {
    let s2 = sin(2.0 * p.x);
    let c2 = cos(2.0 * p.x);
    let sh2 = sinh(2.0 * p.y);
    let ch2 = cosh(2.0 * p.y);
    let d = ch2 - c2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * s2, -inv * sh2);
}
"#,
    wgsl_3d: r#"
fn variation_cot(p: vec3<f32>) -> vec3<f32> {
    let s2 = sin(2.0 * p.x);
    let c2 = cos(2.0 * p.x);
    let sh2 = sinh(2.0 * p.y);
    let ch2 = cosh(2.0 * p.y);
    let d = ch2 - c2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * s2, -inv * sh2, p.z);
}
"#,
};

// =============================================================================
// sinh: sinh(z · π/4)
//   sinh(u + iv) = sinh(u)cos(v) + i·cosh(u)sin(v)
//   with u = x·π/4, v = y·π/4
// =============================================================================
/// Complex hyperbolic sine, applied to `z·π/4`. Stretches the plane
/// vertically.
///
/// # Authors
/// - cothe
pub static SINH: VariationDef = VariationDef {
    name: "sinh",
    aliases: &[],
    display_name: "Sinh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sinh(p: vec2<f32>) -> vec2<f32> {
    let pi_4 = 0.7853981633974483;
    let u = p.x * pi_4;
    let v = p.y * pi_4;
    return vec2<f32>(sinh(u) * cos(v), cosh(u) * sin(v));
}
"#,
    wgsl_3d: r#"
fn variation_sinh(p: vec3<f32>) -> vec3<f32> {
    let pi_4 = 0.7853981633974483;
    let u = p.x * pi_4;
    let v = p.y * pi_4;
    return vec3<f32>(sinh(u) * cos(v), cosh(u) * sin(v), 0.0);
}
"#,
};

// =============================================================================
// cosh: cosh(x + iy) = cosh(x)cos(y) + i·sinh(x)sin(y)
// (Upstream uses x for the hyperbolic argument; trig argument is y.)
// =============================================================================
/// Complex hyperbolic cosine. Sister function to Sinh — even symmetry
/// instead of odd.
///
/// # Authors
/// - cothe
pub static COSH: VariationDef = VariationDef {
    name: "cosh",
    aliases: &[],
    display_name: "Cosh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cosh(p: vec2<f32>) -> vec2<f32> {
    let s = sin(p.y); let c = cos(p.y);
    let sh = sinh(p.x); let ch = cosh(p.x);
    return vec2<f32>(ch * c, sh * s);
}
"#,
    wgsl_3d: r#"
fn variation_cosh(p: vec3<f32>) -> vec3<f32> {
    let s = sin(p.y); let c = cos(p.y);
    let sh = sinh(p.x); let ch = cosh(p.x);
    return vec3<f32>(ch * c, sh * s, p.z);
}
"#,
};

// =============================================================================
// tanh: tanh(z · π/4)
//   tanh(u + iv) = (sinh(2u) + i·sin(2v)) / (cosh(2u) + cos(2v))
//   with u = x·π/4, v = y·π/4 (so 2u = x·π/2, 2v = y·π/2)
// =============================================================================
/// Complex hyperbolic tangent, applied to `z·π/4`. Compresses extreme
/// values toward ±1.
///
/// # Authors
/// - cothe
pub static TANH: VariationDef = VariationDef {
    name: "tanh",
    aliases: &[],
    display_name: "Tanh",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_tanh(p: vec2<f32>) -> vec2<f32> {
    let pi_2 = 1.5707963267948966;
    let s2 = sinh(p.x * pi_2);
    let c2 = cosh(p.x * pi_2);
    let sin2 = sin(p.y * pi_2);
    let cos2 = cos(p.y * pi_2);
    let d = c2 + cos2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * s2, inv * sin2);
}
"#,
    wgsl_3d: r#"
fn variation_tanh(p: vec3<f32>) -> vec3<f32> {
    let pi_2 = 1.5707963267948966;
    let s2 = sinh(p.x * pi_2);
    let c2 = cosh(p.x * pi_2);
    let sin2 = sin(p.y * pi_2);
    let cos2 = cos(p.y * pi_2);
    let d = c2 + cos2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * s2, inv * sin2, 0.0);
}
"#,
};

// =============================================================================
// coth: coth(x + iy) = (sinh(2x) + i·sin(2y)) / (cosh(2x) − cos(2y))
// =============================================================================
/// Complex hyperbolic cotangent. Singularities at 0.
///
/// # Authors
/// - cothe
pub static COTH: VariationDef = VariationDef {
    name: "coth",
    aliases: &[],
    display_name: "Coth",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_coth(p: vec2<f32>) -> vec2<f32> {
    let s2 = sin(2.0 * p.y);
    let c2 = cos(2.0 * p.y);
    let sh2 = sinh(2.0 * p.x);
    let ch2 = cosh(2.0 * p.x);
    let d = ch2 - c2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * sh2, inv * s2);
}
"#,
    wgsl_3d: r#"
fn variation_coth(p: vec3<f32>) -> vec3<f32> {
    let s2 = sin(2.0 * p.y);
    let c2 = cos(2.0 * p.y);
    let sh2 = sinh(2.0 * p.x);
    let ch2 = cosh(2.0 * p.x);
    let d = ch2 - c2;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * sh2, inv * s2, p.z);
}
"#,
};

// =============================================================================
// sech (UPSTREAM BUG: actually computes csch(z·π/4))
//   2 / (e^(zπ/4) − e^(-zπ/4)) = 1 / sinh(z·π/4) = csch(z·π/4)
//   csch(u+iv) = (sinh(u)cos(v) − i·cosh(u)sin(v)) / (sinh²(u)cos²(v) + cosh²(u)sin²(v))
//   with u = x·π/4, v = y·π/4
// =============================================================================
/// Complex hyperbolic secant (note: ported with a JWildfire formula quirk
/// that makes it equivalent to a sign-flipped csch — preserved so existing
/// flames render the same).
///
/// # Authors
/// - cothe
pub static SECH: VariationDef = VariationDef {
    name: "sech",
    aliases: &[],
    display_name: "Sech",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sech(p: vec2<f32>) -> vec2<f32> {
    let pi_4 = 0.7853981633974483;
    let u = p.x * pi_4;
    let v = p.y * pi_4;
    let a = sinh(u) * cos(v);  // real(sinh(z·π/4))
    let b = cosh(u) * sin(v);  // imag(sinh(z·π/4))
    let d = a * a + b * b;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(a * inv, -b * inv);
}
"#,
    wgsl_3d: r#"
fn variation_sech(p: vec3<f32>) -> vec3<f32> {
    let pi_4 = 0.7853981633974483;
    let u = p.x * pi_4;
    let v = p.y * pi_4;
    let a = sinh(u) * cos(v);
    let b = cosh(u) * sin(v);
    let d = a * a + b * b;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(a * inv, -b * inv, 0.0);
}
"#,
};

// =============================================================================
// csch: 2(sinh(x)cos(y) − i·cosh(x)sin(y)) / (cosh(2x) − cos(2y))
// =============================================================================
/// Complex hyperbolic cosecant. Singularities at 0.
///
/// # Authors
/// - cothe
pub static CSCH: VariationDef = VariationDef {
    name: "csch",
    aliases: &[],
    display_name: "Csch",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_csch(p: vec2<f32>) -> vec2<f32> {
    let s = sin(p.y); let c = cos(p.y);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let d = cosh(2.0 * p.x) - cos(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * sh * c, -inv * ch * s);
}
"#,
    wgsl_3d: r#"
fn variation_csch(p: vec3<f32>) -> vec3<f32> {
    let s = sin(p.y); let c = cos(p.y);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let d = cosh(2.0 * p.x) - cos(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * sh * c, -inv * ch * s, p.z);
}
"#,
};
