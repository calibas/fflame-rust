//! Misc trig-family + standalone shape variations
//!
//! Two clusters in one file:
//!
//! Trig-family extras:
//!   - tancos, tangent, tangent3d, secant2, cosine, petal
//!
//! Standalone parameterized shapes:
//!   - cardioid, helix, helicoid, parabola, pie, pie3d
//!
//! Notes:
//!   - `secant2` upstream has an internal-weight artifact: the radius
//!     computed for the cosine denominator includes `pAmount`, so the
//!     non-linear cos(r) part scales with weight. We compute with the
//!     unweighted radius and let the outer multiply apply weight. At the
//!     conventional `weight = 1` case results match upstream exactly;
//!     they drift at other weights. The classification heuristic missed
//!     this — adding to the doc's internal-weight watchlist as a follow-up.
//!   - `helix` / `helicoid` upstream Z preserves; 2D form falls out
//!     trivially since `FTz = 0` collapses the trig terms.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// =============================================================================
// tancos: Raykoid666's tan/cos blend
//   d1 = ε + x² + y²
//   x' = (1/d1) · tanh(d1) · 2x
//   y' = (1/d1) · cos(d1) · 2y
// =============================================================================
/// Tan/cos blend — X gets scaled by tanh of the squared radius, Y by cos of
/// the squared radius. Produces wavy concentric ring patterns.
///
/// # Authors
/// - Raykoid666
pub static TANCOS: VariationDef = VariationDef {
    name: "tancos",
    display_name: "TanCos",
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
fn variation_tancos(p: vec2<f32>) -> vec2<f32> {
    let d1 = 1e-30 + p.x * p.x + p.y * p.y;
    let inv = 1.0 / d1;
    return vec2<f32>(inv * tanh(d1) * 2.0 * p.x, inv * cos(d1) * 2.0 * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_tancos(p: vec3<f32>) -> vec3<f32> {
    let d1 = 1e-30 + p.x * p.x + p.y * p.y;
    let inv = 1.0 / d1;
    return vec3<f32>(inv * tanh(d1) * 2.0 * p.x, inv * cos(d1) * 2.0 * p.y, p.z);
}
"#),
};

// =============================================================================
// tangent: real tan-by-cos (NOT the same as `tan` from trig.rs)
//   x' = sin(x) / cos(y)        — guarded
//   y' = tan(y)
// =============================================================================
/// Real tan-by-cos — X is `sin(x)/cos(y)`, Y is `tan(y)`. Note: not the
/// same as Tan from trig.rs (which is the complex tangent).
pub static TANGENT: VariationDef = VariationDef {
    name: "tangent",
    display_name: "Tangent",
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
fn variation_tangent(p: vec2<f32>) -> vec2<f32> {
    let d = cos(p.y);
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(sin(p.x) * inv, tan(p.y));
}
"#,
    wgsl_3d: Some(r#"
fn variation_tangent(p: vec3<f32>) -> vec3<f32> {
    let d = cos(p.y);
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(sin(p.x) * inv, tan(p.y), p.z);
}
"#),
};

// =============================================================================
// tangent3d: 3D extension — adds z' = tan(x)
// =============================================================================
/// 3D extension of Tangent. Adds `z' = tan(x)` so the variation contributes
/// depth modulation along the X coordinate.
pub static TANGENT3D: VariationDef = VariationDef {
    name: "tangent3d",
    display_name: "Tangent 3D",
    category: VariationCategory::Depth3D,
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
fn variation_tangent3d(p: vec2<f32>) -> vec2<f32> {
    let d = cos(p.y);
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec2<f32>(sin(p.x) * inv, tan(p.y));
}
"#,
    wgsl_3d: Some(r#"
fn variation_tangent3d(p: vec3<f32>) -> vec3<f32> {
    let d = cos(p.y);
    let inv = 1.0 / select(d, 1e-20, d == 0.0);
    return vec3<f32>(sin(p.x) * inv, tan(p.y), tan(p.x));
}
"#),
};

// =============================================================================
// secant2: "fixed" secant with sign-dependent constant offset
//   r = sqrt(x²+y²) + ε                  (NOTE: upstream multiplies by VVAR
//                                          here — we don't, see file comment)
//   icr = 1 / cos(r)                       guarded
//   x' = x
//   y' = icr - 1   if cos(r) ≥ 0
//   y' = icr + 1   if cos(r) < 0
// =============================================================================
/// Variant of secant with a sign-dependent constant offset on Y. Passes X
/// through; Y becomes `1/cos(r) ± 1` depending on the sign of `cos(r)`.
/// Produces banded patterns with a sharp jump at the cos-sign boundary.
pub static SECANT2: VariationDef = VariationDef {
    name: "secant2",
    display_name: "Secant2",
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
fn variation_secant2(p: vec2<f32>) -> vec2<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let cr = cos(r);
    let icr = 1.0 / select(cr, 1e-20, cr == 0.0);
    let y_offset = select(-1.0, 1.0, cr < 0.0);
    return vec2<f32>(p.x, icr + y_offset);
}
"#,
    wgsl_3d: Some(r#"
fn variation_secant2(p: vec3<f32>) -> vec3<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let cr = cos(r);
    let icr = 1.0 / select(cr, 1e-20, cr == 0.0);
    let y_offset = select(-1.0, 1.0, cr < 0.0);
    return vec3<f32>(p.x, icr + y_offset, p.z);
}
"#),
};

// =============================================================================
// cosine: complex cosine of (π·x + iy)
//   x' =  cos(π·x) · cosh(y)
//   y' = -sin(π·x) · sinh(y)
// =============================================================================
/// Complex cosine of `π·x + iy` — output is `(cos(πx)·cosh(y),
/// -sin(πx)·sinh(y))`. Horizontally periodic with vertical exponential
/// growth.
pub static COSINE: VariationDef = VariationDef {
    name: "cosine",
    display_name: "Cosine",
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
fn variation_cosine(p: vec2<f32>) -> vec2<f32> {
    let r = p.x * 3.14159265358979;
    return vec2<f32>(cos(r) * cosh(p.y), -sin(r) * sinh(p.y));
}
"#,
    wgsl_3d: Some(r#"
fn variation_cosine(p: vec3<f32>) -> vec3<f32> {
    let r = p.x * 3.14159265358979;
    return vec3<f32>(cos(r) * cosh(p.y), -sin(r) * sinh(p.y), p.z);
}
"#),
};

// =============================================================================
// petal: Raykoid666's petal shape
//   bx = (cos(x) · cos(y))³
//   by = (sin(x) · cos(y))³
//   x' = cos(x) · bx
//   y' = cos(x) · by
// =============================================================================
/// Petal shape — `(cos(x)·bx, cos(x)·by)` where `bx, by` are cubed
/// sine/cosine products of `(x, y)`. Produces flower-like radial
/// structures.
///
/// # Authors
/// - Raykoid666
pub static PETAL: VariationDef = VariationDef {
    name: "petal",
    display_name: "Petal",
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
fn variation_petal(p: vec2<f32>) -> vec2<f32> {
    let cx = cos(p.x);
    let cy = cos(p.y);
    let sx = sin(p.x);
    let bx_base = cx * cy;
    let by_base = sx * cy;
    let bx = bx_base * bx_base * bx_base;
    let by = by_base * by_base * by_base;
    return vec2<f32>(cx * bx, cx * by);
}
"#,
    wgsl_3d: Some(r#"
fn variation_petal(p: vec3<f32>) -> vec3<f32> {
    let cx = cos(p.x);
    let cy = cos(p.y);
    let sx = sin(p.x);
    let bx_base = cx * cy;
    let by_base = sx * cy;
    let bx = bx_base * bx_base * bx_base;
    let by = by_base * by_base * by_base;
    return vec3<f32>(cx * bx, cx * by, p.z);
}
"#),
};

// =============================================================================
// cardioid: Michael Faber's parameterized cardioid
//   a = atan2(y, x)
//   r = sqrt(x² + y² + sin(a · param) + 1)
//   x' = r · cos(a)
//   y' = r · sin(a)
// =============================================================================
/// Parameterized cardioid (heart-shaped curve). The `a` parameter controls
/// how many lobes/cusps the shape has.
///
/// # Authors
/// - Michael Faber
pub static CARDIOID: VariationDef = VariationDef {
    name: "cardioid",
    display_name: "Cardioid",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef { name: "a", display_name: "A", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Number of cusps/lobes in the cardioid shape. 1 = standard heart, 2 = figure-eight, higher values add more lobes.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cardioid(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a_param = get_param(xform_id, variation_id, 0u);
    let a = atan2(p.y, p.x);
    let r = sqrt(max(p.x * p.x + p.y * p.y + sin(a * a_param) + 1.0, 0.0));
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_cardioid(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a_param = get_param(xform_id, variation_id, 0u);
    let a = atan2(p.y, p.x);
    let r = sqrt(max(p.x * p.x + p.y * p.y + sin(a * a_param) + 1.0, 0.0));
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// helix: zy0rg's 3D helix — winds (x, y) around z by `width`, modulated at
// `frequency` cycles per unit z. In 2D (z=0) collapses to (x + width, y).
// =============================================================================
/// 3D helix — winds the (x, y) coordinates around the Z axis with the given
/// frequency and width. In 2D mode (z = 0) collapses to a simple horizontal
/// shift by `width`.
///
/// # Authors
/// - Zy0rg
pub static HELIX: VariationDef = VariationDef {
    name: "helix",
    display_name: "Helix",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef { name: "frequency", display_name: "Frequency", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("How many full turns the helix makes per unit of Z.") },
        VariationParamDef { name: "width", display_name: "Width", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.5, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Radius of the helical winding around the Z axis.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_helix(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // 2D collapse: z=0, so sin(0)=0 cos(0)=1 → x' = x + width, y' = y
    let width = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(p.x + width, p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_helix(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let frequency = get_param(xform_id, variation_id, 0u);
    let width = get_param(xform_id, variation_id, 1u);
    let phase = p.z * two_pi * frequency;
    return vec3<f32>(p.x + cos(phase) * width, p.y + sin(phase) * width, p.z);
}
"#),
};

// =============================================================================
// helicoid: zy0rg's 3D helicoid — rotates (x, y) by an angle proportional to
// z, while preserving its radius from the origin.
// =============================================================================
/// 3D helicoid — rotates (x, y) by an angle proportional to Z, preserving
/// the radius from the origin. In 2D mode collapses to identity.
///
/// # Authors
/// - Zy0rg
pub static HELICOID: VariationDef = VariationDef {
    name: "helicoid",
    display_name: "Helicoid",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef { name: "frequency", display_name: "Frequency", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("How fast the (x, y) plane rotates as Z increases. Larger = tighter spiral.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_helicoid(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // 2D collapse: phase = atan2(y,x) — rotation by 0 → identity
    let _f = get_param(xform_id, variation_id, 0u);
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_helicoid(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let frequency = get_param(xform_id, variation_id, 0u);
    let range = sqrt(p.x * p.x + p.y * p.y);
    let phase = p.z * two_pi * frequency + atan2(p.y, p.x);
    return vec3<f32>(cos(phase) * range, sin(phase) * range, p.z);
}
"#),
};

// =============================================================================
// parabola: cyberxaos's randomly-amplitude parabola
//   r = sqrt(x²+y²) + ε
//   x' = height · sin²(r) · u₁
//   y' = width · cos(r) · u₂
//   u₁, u₂ are independent uniform [0, 1)
// =============================================================================
/// Randomly-amplitude parabola. Output X is height-scaled `sin²(r)` times a
/// uniform random; Y is width-scaled `cos(r)` times another uniform.
/// Produces blurry parabolic arcs.
///
/// # Authors
/// - cyberxaos
pub static PARABOLA: VariationDef = VariationDef {
    name: "parabola",
    display_name: "Parabola",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "width", display_name: "Width", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Horizontal scaling of the parabolic envelope.") },
        VariationParamDef { name: "height", display_name: "Height", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.5, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Vertical scaling of the parabolic envelope.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_parabola(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let width = get_param(xform_id, variation_id, 0u);
    let height = get_param(xform_id, variation_id, 1u);
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let sr = sin(r);
    let cr = cos(r);
    return vec2<f32>(height * sr * sr * rng_nextf(rng), width * cr * rng_nextf(rng));
}
"#,
    wgsl_3d: Some(r#"
fn variation_parabola(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let width = get_param(xform_id, variation_id, 0u);
    let height = get_param(xform_id, variation_id, 1u);
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let sr = sin(r);
    let cr = cos(r);
    return vec3<f32>(height * sr * sr * rng_nextf(rng), width * cr * rng_nextf(rng), p.z);
}
"#),
};

// =============================================================================
// pie: discrete pie-slice splatter
//   sl = floor(uniform · slices + 0.5)             (slice index)
//   a  = rotation + 2π · (sl + uniform · thickness) / slices
//   r  = uniform
//   x' = r · cos(a), y' = r · sin(a)
// =============================================================================
/// Discrete pie-slice splatter — divides the unit disc into `slices`
/// wedges and scatters output points uniformly inside one randomly
/// chosen wedge per iteration. The slice's angular extent is controlled
/// by `thickness`.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static PIE: VariationDef = VariationDef {
    name: "pie",
    display_name: "Pie",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "slices", display_name: "Slices", param_type: ParamType::Float,
                            default_value: 6.0, min_value: Some(1.0), max_value: Some(64.0), description: Some("Number of pie wedges (1-64).") },
        VariationParamDef { name: "rotation", display_name: "Rotation", param_type: ParamType::Angle,
                            default_value: 0.0, min_value: Some(0.0), max_value: Some(360.0), description: Some("Rotation angle of the whole pie in degrees.") },
        VariationParamDef { name: "thickness", display_name: "Thickness", param_type: ParamType::Float,
                            default_value: 0.5, min_value: Some(0.0), max_value: Some(1.0), description: Some("Wedge thickness within its slice. 0 = razor-thin spokes, 1 = wedges fill their entire slice.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pie(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let slices = max(get_param(xform_id, variation_id, 0u), 1.0);
    let rotation = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let sl = floor(rng_nextf(rng) * slices + 0.5);
    let a = rotation + two_pi * (sl + rng_nextf(rng) * thickness) / slices;
    let r = rng_nextf(rng);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_pie(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let slices = max(get_param(xform_id, variation_id, 0u), 1.0);
    let rotation = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let sl = floor(rng_nextf(rng) * slices + 0.5);
    let a = rotation + two_pi * (sl + rng_nextf(rng) * thickness) / slices;
    let r = rng_nextf(rng);
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// pie3d: 3D version of pie — adds z' = r · sin(r)
// =============================================================================
/// 3D version of Pie — same pie-slice splatter as Pie but adds `z' =
/// r·sin(r)` for depth modulation.
pub static PIE3D: VariationDef = VariationDef {
    name: "pie3d",
    display_name: "Pie 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "slices", display_name: "Slices", param_type: ParamType::Float,
                            default_value: 7.0, min_value: Some(1.0), max_value: Some(64.0), description: Some("Number of pie wedges (1-64).") },
        VariationParamDef { name: "rotation", display_name: "Rotation", param_type: ParamType::Angle,
                            default_value: 0.0, min_value: Some(0.0), max_value: Some(360.0), description: Some("Rotation angle of the whole pie in degrees.") },
        VariationParamDef { name: "thickness", display_name: "Thickness", param_type: ParamType::Float,
                            default_value: 0.5, min_value: Some(0.0), max_value: Some(1.0), description: Some("Wedge thickness within its slice. 0 = razor-thin spokes, 1 = wedges fill their entire slice.") },
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pie3d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let slices = max(get_param(xform_id, variation_id, 0u), 1.0);
    let rotation = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let sl = floor(rng_nextf(rng) * slices + 0.5);
    let a = rotation + two_pi * (sl + rng_nextf(rng) * thickness) / slices;
    let r = rng_nextf(rng);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_pie3d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let slices = max(get_param(xform_id, variation_id, 0u), 1.0);
    let rotation = get_param(xform_id, variation_id, 1u);
    let thickness = get_param(xform_id, variation_id, 2u);
    let sl = floor(rng_nextf(rng) * slices + 0.5);
    let a = rotation + two_pi * (sl + rng_nextf(rng) * thickness) / slices;
    let r = rng_nextf(rng);
    return vec3<f32>(r * cos(a), r * sin(a), r * sin(r));
}
"#),
};
