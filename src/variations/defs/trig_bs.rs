//! Brad Stefanov's parameterized direct trig/hyperbolic variations
//!
//! Same math family as `trig.rs` (batch 2), but each axis is independently
//! scaled by a per-variation parameter:
//!   - `x1` scales the argument of the sin/sinh on the X axis
//!   - `x2` scales the argument of the cos/cosh on the X axis
//!   - `y1` scales the argument of the sin/sinh on the Y axis
//!   - `y2` scales the argument of the cos/cosh on the Y axis
//!
//! Defaults are 1.0 for the simple form and 2.0 for the tan/cot/tanh/coth
//! variants (matching upstream's `2.0 * FTx` doubling). At those defaults
//! these reduce exactly to the corresponding `trig.rs` variations.
//!
//! `exp2_bs` is the odd one out: only three parameters (x1, y1, y2).
//!
//! Divide-by-zero guards substitute 1e-20 only on exact zero, matching the
//! upstream `if d == 0 return TRUE` early-return.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// Helper macro-like constants for the four common params. Each variation
// declares its own array because Rust statics can't share slice literals
// across modules.

// =============================================================================
// sin2_bs: parameterized sin
// =============================================================================
/// Parameterized Sin — independent scaling on each sin/cos/sinh/cosh term.
/// At defaults (1.0), reduces to Sin.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static SIN2_BS: VariationDef = VariationDef {
    name: "sin2_bs",
    aliases: &[],
    display_name: "Sin2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sin2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    return vec2<f32>(s * ch, c * sh);
}
"#,
    wgsl_3d: r#"
fn variation_sin2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    return vec3<f32>(s * ch, c * sh, p.z);
}
"#,
};

// =============================================================================
// cos2_bs: parameterized cos
// =============================================================================
/// Parameterized Cos. At defaults (1.0), reduces to Cos.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static COS2_BS: VariationDef = VariationDef {
    name: "cos2_bs",
    aliases: &[],
    display_name: "Cos2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cos2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    return vec2<f32>(c * ch, -s * sh);
}
"#,
    wgsl_3d: r#"
fn variation_cos2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    return vec3<f32>(c * ch, -s * sh, p.z);
}
"#,
};

// =============================================================================
// tan2_bs: parameterized tan
//   FPx += den * sin(x1·x), FPy += den * sinh(y1·y)
//   den = 1 / (cos(x2·x) + cosh(y2·y))
// =============================================================================
/// Parameterized Tan. At defaults (2.0, matching the upstream doubling),
/// reduces to Tan.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static TAN2_BS: VariationDef = VariationDef {
    name: "tan2_bs",
    aliases: &[],
    display_name: "Tan2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_tan2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(x1 * p.x); let c = cos(x2 * p.x);
    let sh = sinh(y1 * p.y); let ch = cosh(y2 * p.y);
    let d = c + ch;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * s, inv * sh);
}
"#,
    wgsl_3d: r#"
fn variation_tan2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(x1 * p.x); let c = cos(x2 * p.x);
    let sh = sinh(y1 * p.y); let ch = cosh(y2 * p.y);
    let d = c + ch;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * s, inv * sh, p.z);
}
"#,
};

// =============================================================================
// sec2_bs: parameterized sec
// =============================================================================
/// Parameterized Sec (1/cos). At defaults (1.0), reduces to Sec.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static SEC2_BS: VariationDef = VariationDef {
    name: "sec2_bs",
    aliases: &[],
    display_name: "Sec2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sec2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    let d = cos(2.0 * p.x) + cosh(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * c * ch, inv * s * sh);
}
"#,
    wgsl_3d: r#"
fn variation_sec2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    let d = cos(2.0 * p.x) + cosh(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * c * ch, inv * s * sh, p.z);
}
"#,
};

// =============================================================================
// csc2_bs: parameterized csc
// =============================================================================
/// Parameterized Csc (1/sin). At defaults (1.0), reduces to Csc.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static CSC2_BS: VariationDef = VariationDef {
    name: "csc2_bs",
    aliases: &[],
    display_name: "Csc2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_csc2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    let d = cosh(2.0 * p.y) - cos(2.0 * p.x);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * s * ch, -inv * c * sh);
}
"#,
    wgsl_3d: r#"
fn variation_csc2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.x * x1); let c = cos(p.x * x2);
    let sh = sinh(p.y * y1); let ch = cosh(p.y * y2);
    let d = cosh(2.0 * p.y) - cos(2.0 * p.x);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * s * ch, -inv * c * sh, p.z);
}
"#,
};

// =============================================================================
// cot2_bs: parameterized cot
//   FPx += den * sin(x1·x), FPy += -den * sinh(y1·y)
//   den = 1 / (cosh(y2·y) - cos(x2·x))
// =============================================================================
/// Parameterized Cot (cos/sin). At defaults (2.0), reduces to Cot.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static COT2_BS: VariationDef = VariationDef {
    name: "cot2_bs",
    aliases: &[],
    display_name: "Cot2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cot2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(x1 * p.x); let c = cos(x2 * p.x);
    let sh = sinh(y1 * p.y); let ch = cosh(y2 * p.y);
    let d = ch - c;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * s, -inv * sh);
}
"#,
    wgsl_3d: r#"
fn variation_cot2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(x1 * p.x); let c = cos(x2 * p.x);
    let sh = sinh(y1 * p.y); let ch = cosh(y2 * p.y);
    let d = ch - c;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * s, -inv * sh, p.z);
}
"#,
};

// =============================================================================
// sinh2_bs: parameterized sinh; UPSTREAM uses real-trig identities (NOT the
// complex-exp π/4-scaled form of trig.rs:sinh).
//   FPx += sinh(x1·x) · cos(y2·y)
//   FPy += cosh(x2·x) · sin(y1·y)
// =============================================================================
/// Parameterized Sinh. At defaults (1.0), reduces to Sinh.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static SINH2_BS: VariationDef = VariationDef {
    name: "sinh2_bs",
    aliases: &[],
    display_name: "Sinh2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sinh2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    return vec2<f32>(sh * c, ch * s);
}
"#,
    wgsl_3d: r#"
fn variation_sinh2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    return vec3<f32>(sh * c, ch * s, p.z);
}
"#,
};

// =============================================================================
// cosh2_bs: parameterized cosh
// =============================================================================
/// Parameterized Cosh. At defaults (1.0), reduces to Cosh.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static COSH2_BS: VariationDef = VariationDef {
    name: "cosh2_bs",
    aliases: &[],
    display_name: "Cosh2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cosh2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    return vec2<f32>(ch * c, sh * s);
}
"#,
    wgsl_3d: r#"
fn variation_cosh2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    return vec3<f32>(ch * c, sh * s, p.z);
}
"#,
};

// =============================================================================
// tanh2_bs: parameterized tanh
//   FPx += den · sinh(x1·x), FPy += den · sin(y1·y)
//   den = 1 / (cos(y2·y) + cosh(x2·x))
// =============================================================================
/// Parameterized Tanh. At defaults (2.0), reduces to Tanh.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static TANH2_BS: VariationDef = VariationDef {
    name: "tanh2_bs",
    aliases: &[],
    display_name: "Tanh2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_tanh2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(y1 * p.y); let c = cos(y2 * p.y);
    let sh = sinh(x1 * p.x); let ch = cosh(x2 * p.x);
    let d = c + ch;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * sh, inv * s);
}
"#,
    wgsl_3d: r#"
fn variation_tanh2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(y1 * p.y); let c = cos(y2 * p.y);
    let sh = sinh(x1 * p.x); let ch = cosh(x2 * p.x);
    let d = c + ch;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * sh, inv * s, p.z);
}
"#,
};

// =============================================================================
// coth2_bs: parameterized coth
//   FPx += den · sinh(x1·x), FPy += den · sin(y1·y)
//   den = 1 / (cosh(x2·x) - cos(y2·y))
// =============================================================================
/// Parameterized Coth. At defaults (2.0), reduces to Coth.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static COTH2_BS: VariationDef = VariationDef {
    name: "coth2_bs",
    aliases: &[],
    display_name: "Coth2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_coth2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(y1 * p.y); let c = cos(y2 * p.y);
    let sh = sinh(x1 * p.x); let ch = cosh(x2 * p.x);
    let d = ch - c;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * sh, inv * s);
}
"#,
    wgsl_3d: r#"
fn variation_coth2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(y1 * p.y); let c = cos(y2 * p.y);
    let sh = sinh(x1 * p.x); let ch = cosh(x2 * p.x);
    let d = ch - c;
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * sh, inv * s, p.z);
}
"#,
};

// =============================================================================
// sech2_bs: parameterized sech (correct formula — sums in denominator unlike
// trig.rs:sech which mirrors upstream's csch-mislabeled-as-sech bug)
//   FPx += den · cos(y2·y) · cosh(x2·x)
//   FPy += -den · sin(y1·y) · sinh(x1·x)
//   den = 2 / (cos(2y) + cosh(2x))
// =============================================================================
/// Parameterized Sech. At defaults (1.0), reduces to Sech (with the same
/// JWildfire formula quirk noted in trig.rs).
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static SECH2_BS: VariationDef = VariationDef {
    name: "sech2_bs",
    aliases: &[],
    display_name: "Sech2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sech2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    let d = cos(2.0 * p.y) + cosh(2.0 * p.x);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * c * ch, -inv * s * sh);
}
"#,
    wgsl_3d: r#"
fn variation_sech2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    let d = cos(2.0 * p.y) + cosh(2.0 * p.x);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * c * ch, -inv * s * sh, p.z);
}
"#,
};

// =============================================================================
// csch2_bs: parameterized csch
// =============================================================================
/// Parameterized Csch. At defaults (1.0), reduces to Csch.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static CSCH2_BS: VariationDef = VariationDef {
    name: "csch2_bs",
    aliases: &[],
    display_name: "Csch2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(x)` in the internal computation.") },
        VariationParamDef { name: "x2", display_name: "X2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(x)` in the internal computation.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sinh(y)` in the internal computation.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cosh(y)` in the internal computation.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_csch2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    let d = cosh(2.0 * p.x) - cos(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(inv * sh * c, -inv * ch * s);
}
"#,
    wgsl_3d: r#"
fn variation_csch2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let x2 = get_param(xform_id, variation_id, 1u);
    let y1 = get_param(xform_id, variation_id, 2u);
    let y2 = get_param(xform_id, variation_id, 3u);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    let sh = sinh(p.x * x1); let ch = cosh(p.x * x2);
    let d = cosh(2.0 * p.x) - cos(2.0 * p.y);
    let inv = 2.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(inv * sh * c, -inv * ch * s, p.z);
}
"#,
};

// =============================================================================
// exp2_bs: complex exponential, parameterized
//   exp(x1·x) · (cos(y2·y), sin(y1·y))
//   Note only THREE params upstream (no x2).
// =============================================================================
/// Parameterized complex exponential — `e^(x·x1)` modulated by `sin(y·y1)`
/// and `cos(y·y2)`. At defaults (1.0), reduces to the unparameterized
/// complex exp.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static EXP2_BS: VariationDef = VariationDef {
    name: "exp2_bs",
    aliases: &[],
    display_name: "Exp2 BS",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        VariationParamDef { name: "x1", display_name: "X1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the X exponent — output uses `exp(x · x1)`.") },
        VariationParamDef { name: "y1", display_name: "Y1", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `sin(y)`.") },
        VariationParamDef { name: "y2", display_name: "Y2", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Scales the argument of `cos(y)`.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_exp2_bs(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let y1 = get_param(xform_id, variation_id, 1u);
    let y2 = get_param(xform_id, variation_id, 2u);
    let e = exp(p.x * x1);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    return vec2<f32>(e * c, e * s);
}
"#,
    wgsl_3d: r#"
fn variation_exp2_bs(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x1 = get_param(xform_id, variation_id, 0u);
    let y1 = get_param(xform_id, variation_id, 1u);
    let y2 = get_param(xform_id, variation_id, 2u);
    let e = exp(p.x * x1);
    let s = sin(p.y * y1); let c = cos(p.y * y2);
    return vec3<f32>(e * c, e * s, p.z);
}
"#,
};
