//! Standalone shape variations — third batch (continuation of shapes.rs / shapes2.rs)
//!
//! Three popular non-trig standalone shapes:
//!   - `super_shape` (apo plugin pack) — Gielis super-formula warp
//!   - `henon`       (TyrantWave)      — classic Hénon strange attractor
//!   - `apollony`    (Sosa, after Bourke) — Apollonian gasket IFS
//!
//! Sources:
//!   - output/jwildfire-vars/output/super_shape.cpp
//!   - output/jwildfire-vars/output/henon.cpp  (cpp body empty — Java
//!     comment block was the source of truth)
//!   - output/jwildfire-vars/output/apollony.cpp
//!
//! Notes on faithfulness:
//!   - All factor VVAR through the outer-multiplier convention.
//!   - `super_shape` preserves the cpp's `atan2(FTx, FTy)` ordering
//!     (Java's `getPrecalcAtanYX()` is `atan2(y, x)`; cpp swapped). Same
//!     systematic porter mistake as elsewhere; faithful to upstream cpp.
//!   - `henon` is one of the upstream stub ports — `PluginVarCalc` was
//!     left empty and the formula lives in the Java comment block. We
//!     translated directly from the Java.
//!   - `apollony` rounds `(int)(4·rand)` and tests `w % 3`. WGSL uses
//!     `i32(...)` (truncation) and `%` (signed modulo) — same semantics
//!     for non-negative inputs.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// super_shape: Gielis super-formula warp
//   theta = m/4 · atan2(x, y) + π/4    (upstream cpp swap)
//   t1    = |cos(theta)|^n2
//   t2    = |sin(theta)|^n3
//   r     = (rnd · rand + (1-rnd) · |p|) - holes      (radial offset)
//   r    *= |p|^(-1) · (t1 + t2)^(-1/n1)
//   out   = r · (x, y)
// =============================================================================
pub static SUPER_SHAPE: VariationDef = VariationDef {
    name: "super_shape",
    display_name: "Super Shape",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("rnd", "RND", unlimited_float, 3.0, -10.0, 10.0),
        param!("m", "M", unlimited_float, 1.0, -20.0, 20.0),
        param!("n1", "N1", unlimited_float, 1.0, -20.0, 20.0),
        param!("n2", "N2", unlimited_float, 1.0, -20.0, 20.0),
        param!("n3", "N3", unlimited_float, 1.0, -20.0, 20.0),
        param!("holes", "Holes", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_super_shape(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let rnd = get_param(xform_id, variation_id, 0u);
    let m = get_param(xform_id, variation_id, 1u);
    let n1 = get_param(xform_id, variation_id, 2u);
    let n2 = get_param(xform_id, variation_id, 3u);
    let n3 = get_param(xform_id, variation_id, 4u);
    let holes = get_param(xform_id, variation_id, 5u);
    let pi_4 = 0.7853981633974483;

    let theta = m * 0.25 * atan2(p.x, p.y) + pi_4;
    let t1 = pow(max(abs(cos(theta)), 1e-30), n2);
    let t2 = pow(max(abs(sin(theta)), 1e-30), n3);
    let len = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let safe_n1 = select(n1, 1e-30, n1 == 0.0);
    let r = ((rnd * rng_nextf(rng) + (1.0 - rnd) * len) - holes)
            * pow(max(t1 + t2, 1e-30), -1.0 / safe_n1) / len;
    return vec2<f32>(r * p.x, r * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_super_shape(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let rnd = get_param(xform_id, variation_id, 0u);
    let m = get_param(xform_id, variation_id, 1u);
    let n1 = get_param(xform_id, variation_id, 2u);
    let n2 = get_param(xform_id, variation_id, 3u);
    let n3 = get_param(xform_id, variation_id, 4u);
    let holes = get_param(xform_id, variation_id, 5u);
    let pi_4 = 0.7853981633974483;

    let theta = m * 0.25 * atan2(p.x, p.y) + pi_4;
    let t1 = pow(max(abs(cos(theta)), 1e-30), n2);
    let t2 = pow(max(abs(sin(theta)), 1e-30), n3);
    let len = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let safe_n1 = select(n1, 1e-30, n1 == 0.0);
    let r = ((rnd * rng_nextf(rng) + (1.0 - rnd) * len) - holes)
            * pow(max(t1 + t2, 1e-30), -1.0 / safe_n1) / len;
    return vec3<f32>(r * p.x, r * p.y, p.z);
}
"#),
};

// =============================================================================
// henon: Hénon strange attractor (TyrantWave)
//   out_x = c - a · x² + y
//   out_y = b · x
// (translated from the Java comment block — cpp PluginVarCalc was empty.)
// =============================================================================
pub static HENON: VariationDef = VariationDef {
    name: "henon",
    display_name: "Henon",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 0.5, -10.0, 10.0),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_henon(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    return vec2<f32>(c - a * p.x * p.x + p.y, b * p.x);
}
"#,
    wgsl_3d: Some(r#"
fn variation_henon(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    return vec3<f32>(c - a * p.x * p.x + p.y, b * p.x, p.z);
}
"#),
};

// =============================================================================
// apollony: Apollonian-gasket IFS (Sosa, after Paul Bourke)
//   r       = sqrt(3)
//   denom   = (1+r-x)² + y²
//   a0      = 3·(1+r-x)/denom − (1+r)/(2+r)
//   b0      = 3·y/denom
//   f1      = (a0, -b0) / (a0² + b0²)         (complex inversion)
//   pick branch w = floor(4·rand) mod 3:
//     0:  out = (a0, b0)
//     1:  out = (-f1.x/2 - f1.y·r/2,  f1.x·r/2 - f1.y/2)
//     2:  out = (-f1.x/2 + f1.y·r/2, -f1.x·r/2 - f1.y/2)
// =============================================================================
pub static APOLLONY: VariationDef = VariationDef {
    name: "apollony",
    display_name: "Apollony",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_apollony(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = 1.7320508075688772;  // sqrt(3)
    let denom = (1.0 + r - p.x) * (1.0 + r - p.x) + p.y * p.y + 1e-30;
    let a0 = 3.0 * (1.0 + r - p.x) / denom - (1.0 + r) / (2.0 + r);
    let b0 = 3.0 * p.y / denom;
    let mag2 = a0 * a0 + b0 * b0 + 1e-30;
    let f1x = a0 / mag2;
    let f1y = -b0 / mag2;

    let w = i32(4.0 * rng_nextf(rng)) % 3;
    if (w == 0) {
        return vec2<f32>(a0, b0);
    } else if (w == 1) {
        return vec2<f32>(-f1x * 0.5 - f1y * r * 0.5,  f1x * r * 0.5 - f1y * 0.5);
    }
    return vec2<f32>(-f1x * 0.5 + f1y * r * 0.5, -f1x * r * 0.5 - f1y * 0.5);
}
"#,
    wgsl_3d: Some(r#"
fn variation_apollony(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = 1.7320508075688772;
    let denom = (1.0 + r - p.x) * (1.0 + r - p.x) + p.y * p.y + 1e-30;
    let a0 = 3.0 * (1.0 + r - p.x) / denom - (1.0 + r) / (2.0 + r);
    let b0 = 3.0 * p.y / denom;
    let mag2 = a0 * a0 + b0 * b0 + 1e-30;
    let f1x = a0 / mag2;
    let f1y = -b0 / mag2;

    let w = i32(4.0 * rng_nextf(rng)) % 3;
    if (w == 0) {
        return vec3<f32>(a0, b0, p.z);
    } else if (w == 1) {
        return vec3<f32>(-f1x * 0.5 - f1y * r * 0.5,  f1x * r * 0.5 - f1y * 0.5, p.z);
    }
    return vec3<f32>(-f1x * 0.5 + f1y * r * 0.5, -f1x * r * 0.5 - f1y * 0.5, p.z);
}
"#),
};
