//! Quaternion-style trigonometric / hyperbolic variations
//!
//! Ports of zephyrtronium's "Quaternion Apo" plugin pack:
//! https://zephyrtronium.deviantart.com/art/Quaternion-Apo-Plugin-Pack-165451482
//!
//! These treat (x, y, z) as a "split" quaternion where x is the real part
//! and (y, z) is a single imaginary direction. With v = hypot(y, z) the
//! formulas reduce to the standard 2-vector trig/hyperbolic identities
//! lifted into 3D — `sinq` for instance is `sin(x + iv)` projected back
//! onto the (y, z) direction.
//!
//! In 2D mode (z = 0), v = |y| and these collapse to forms equivalent to
//! the corresponding direct-trig variations from `trig.rs`.
//!
//! Conventions:
//!   - Weight is applied outside the function (we just bake the math).
//!   - Tiny-magnitude denominators are clamped to `1e-20` to avoid NaN /
//!     Inf at exact singularities — the resulting near-singular outputs
//!     are clipped naturally by the histogram.

use crate::variations::{
    definition::VariationDef,
    VariationCategory, VariationPhase,
};

// =============================================================================
// sinq: (sin(x)·cosh(v), C·y, C·z), C = cos(x)·sinh(v)/v
// =============================================================================
pub static SINQ: VariationDef = VariationDef {
    name: "sinq",
    display_name: "Sinq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sinq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = c * sh / v;
    return vec2<f32>(s * ch, coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sinq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = c * sh / v;
    return vec3<f32>(s * ch, coef * p.y, coef * p.z);
}
"#),
};

// =============================================================================
// cosq: (cos(x)·cosh(v), C·y, C·z), C = -sin(x)·sinh(v)/v
// =============================================================================
pub static COSQ: VariationDef = VariationDef {
    name: "cosq",
    display_name: "Cosq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_cosq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = -s * sh / v;
    return vec2<f32>(c * ch, coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cosq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = -s * sh / v;
    return vec3<f32>(c * ch, coef * p.y, coef * p.z);
}
"#),
};

// =============================================================================
// sinhq: (sinh(x)·cos(v), C·y, C·z), C = cosh(x)·sin(v)/v
// =============================================================================
pub static SINHQ: VariationDef = VariationDef {
    name: "sinhq",
    display_name: "Sinhq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sinhq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = ch * s / v;
    return vec2<f32>(sh * c, coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sinhq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = ch * s / v;
    return vec3<f32>(sh * c, coef * p.y, coef * p.z);
}
"#),
};

// =============================================================================
// coshq: (cosh(x)·cos(v), C·y, C·z), C = sinh(x)·sin(v)/v
// =============================================================================
pub static COSHQ: VariationDef = VariationDef {
    name: "coshq",
    display_name: "Coshq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_coshq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = sh * s / v;
    return vec2<f32>(ch * c, coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_coshq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = sh * s / v;
    return vec3<f32>(ch * c, coef * p.y, coef * p.z);
}
"#),
};

// =============================================================================
// secq: 1/cosq, divides by full quaternion norm
//   Upstream uses sin(-x), cos(-x); since sin is odd and cos is even, this
//   ends up as (cos(x)·cosh(v)·ni, sin(x)·sinh(v)·ni·y/v, sin(x)·sinh(v)·ni·z/v)
//   ni = 1/(x² + y² + z²)
// =============================================================================
pub static SECQ: VariationDef = VariationDef {
    name: "secq",
    display_name: "Secq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_secq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(-p.x); let c = cos(-p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = ni * s * sh / v;
    return vec2<f32>(c * ch * ni, -coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_secq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y + p.z * p.z, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(-p.x); let c = cos(-p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = ni * s * sh / v;
    return vec3<f32>(c * ch * ni, -coef * p.y, -coef * p.z);
}
"#),
};

// =============================================================================
// cscq: 1/sinq, divides by full quaternion norm
// =============================================================================
pub static CSCQ: VariationDef = VariationDef {
    name: "cscq",
    display_name: "Cscq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_cscq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = ni * c * sh / v;
    return vec2<f32>(s * ch * ni, -coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cscq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y + p.z * p.z, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let coef = ni * c * sh / v;
    return vec3<f32>(s * ch * ni, -coef * p.y, -coef * p.z);
}
"#),
};

// =============================================================================
// sechq: 1/coshq, divides by full quaternion norm
// =============================================================================
pub static SECHQ: VariationDef = VariationDef {
    name: "sechq",
    display_name: "Sechq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_sechq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = ni * sh * s / v;
    return vec2<f32>(ch * c * ni, -coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sechq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y + p.z * p.z, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = ni * sh * s / v;
    return vec3<f32>(ch * c * ni, -coef * p.y, -coef * p.z);
}
"#),
};

// =============================================================================
// cschq: 1/sinhq, divides by full quaternion norm
// =============================================================================
pub static CSCHQ: VariationDef = VariationDef {
    name: "cschq",
    display_name: "Cschq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_cschq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = ni * ch * s / v;
    return vec2<f32>(sh * c * ni, -coef * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cschq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let norm2 = max(p.x * p.x + p.y * p.y + p.z * p.z, 1e-20);
    let ni = 1.0 / norm2;
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let coef = ni * ch * s / v;
    return vec3<f32>(sh * c * ni, -coef * p.y, -coef * p.z);
}
"#),
};

// =============================================================================
// tanq: full quotient form
//   sysz = y² + z², ni = 1/(x² + sysz)
//   stcv = sin(x)·cosh(v), ctcv = cos(x)·cosh(v)
//   B = -sin(x)·sinh(v)/v, C = cos(x)·sinh(v)/v
//   x out: (stcv·ctcv + C·B·sysz) · ni
//   y out: (-stcv·B·y + C·y·ctcv) · ni
// =============================================================================
pub static TANQ: VariationDef = VariationDef {
    name: "tanq",
    display_name: "Tanq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_tanq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let sysz = p.y * p.y;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    return vec2<f32>(rx, ry);
}
"#,
    wgsl_3d: Some(r#"
fn variation_tanq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let sysz = p.y * p.y + p.z * p.z;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    let rz = (-stcv * bb * p.z + cc * p.z * ctcv) * ni;
    return vec3<f32>(rx, ry, rz);
}
"#),
};

// =============================================================================
// cotq: same as tanq but y/z signs negated (subtract the y/z components)
// =============================================================================
pub static COTQ: VariationDef = VariationDef {
    name: "cotq",
    display_name: "Cotq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_cotq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let sysz = p.y * p.y;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = -(-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    return vec2<f32>(rx, ry);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cotq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let sysz = p.y * p.y + p.z * p.z;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(p.x); let c = cos(p.x);
    let sh = sinh(v); let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = -(-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    let rz = -(-stcv * bb * p.z + cc * p.z * ctcv) * ni;
    return vec3<f32>(rx, ry, rz);
}
"#),
};

// =============================================================================
// tanhq: hyperbolic on x, trig on v; same formula structure as tanq with
// roles swapped (sh/ch become x-based; s/c become v-based)
//   B in upstream is +sh*s/v (no negation, unlike tanq)
// =============================================================================
pub static TANHQ: VariationDef = VariationDef {
    name: "tanhq",
    display_name: "Tanhq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_tanhq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let sysz = p.y * p.y;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let cc = ch * s / v;
    let bb = sh * s / v;
    let stcv = sh * c;
    let ctcv = ch * c;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    return vec2<f32>(rx, ry);
}
"#,
    wgsl_3d: Some(r#"
fn variation_tanhq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let sysz = p.y * p.y + p.z * p.z;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let cc = ch * s / v;
    let bb = sh * s / v;
    let stcv = sh * c;
    let ctcv = ch * c;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    let rz = (-stcv * bb * p.z + cc * p.z * ctcv) * ni;
    return vec3<f32>(rx, ry, rz);
}
"#),
};

// =============================================================================
// cothq: same as tanhq but y/z components added with opposite sign... wait,
// upstream cothq uses `+=` for y/z (matching tanhq), unlike cotq vs tanq.
// =============================================================================
pub static COTHQ: VariationDef = VariationDef {
    name: "cothq",
    display_name: "Cothq",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_cothq(p: vec2<f32>) -> vec2<f32> {
    let v = max(abs(p.y), 1e-20);
    let sysz = p.y * p.y;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let cc = ch * s / v;
    let bb = sh * s / v;
    let stcv = sh * c;
    let ctcv = ch * c;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    return vec2<f32>(rx, ry);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cothq(p: vec3<f32>) -> vec3<f32> {
    let v = max(sqrt(p.y * p.y + p.z * p.z), 1e-20);
    let sysz = p.y * p.y + p.z * p.z;
    let ni = 1.0 / max(p.x * p.x + sysz, 1e-20);
    let s = sin(v); let c = cos(v);
    let sh = sinh(p.x); let ch = cosh(p.x);
    let cc = ch * s / v;
    let bb = sh * s / v;
    let stcv = sh * c;
    let ctcv = ch * c;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * p.y + cc * p.y * ctcv) * ni;
    let rz = (-stcv * bb * p.z + cc * p.z * ctcv) * ni;
    return vec3<f32>(rx, ry, rz);
}
"#),
};
