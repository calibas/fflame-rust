//! curliecue2 (Jesus Sosa, August 2018)
//!
//! Self-iterating Curlicue walker. The variation ignores its input
//! position (except for Z pass-through in 3D) and emits a deterministic
//! point sequence driven by per-thread persistent state:
//!
//!   x' = x + 0.001 · cos(phi)
//!   y' = y + 0.001 · sin(phi)
//!   phi  = (theta + phi) mod 2π
//!   theta = (theta + 2π·speed) mod 2π
//!   output = (x', y')
//!
//! The cpp version randomizes `_s` (speed) once at flame init via
//! GOODRAND_01. We expose it as a user parameter instead so the visual
//! is reproducible and adjustable.
//!
//! State (4 slots, zero-initialized): `_x0, _y0, _theta, _phi`.
//!
//! First port to validate per-thread state infrastructure. See
//! `docs/projects/intra-iteration-state-and-accum.md`.
//!
//! Source: `output/jwildfire-vars/output/curliecue2.cpp`.
//! Reference: https://oolong.co.uk/curlicue.htm

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Self-iterating Curlicue walker — ignores the input position and emits a
/// deterministic point sequence from a 4-slot per-thread state machine `(x,
/// y, θ, φ)`. Each iteration advances `(x, y)` by `0.001 · (cos φ, sin φ)`,
/// then updates `φ ← (θ + φ) mod 2π` and `θ ← (θ + 2π·speed) mod 2π`. With
/// `speed` set to an irrational fraction (e.g. golden ratio), the
/// trajectory traces the classical curlicue fractal of Berry and Goldberg.
/// Upstream cpp randomizes `speed` once at flame init via `GOODRAND_01`; we
/// expose it as a user parameter for reproducibility.
///
/// # Authors
/// - Jesus Sosa
pub static CURLIECUE2: VariationDef = VariationDef {
    name: "curliecue2",
    aliases: &[],
    display_name: "Curliecue 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("speed", "Speed", float, 0.5, 0.0, 1.0, "Angular velocity of the curlicue walker as a fraction of 2π per iteration. Small irrational values (e.g. golden-ratio fractions) produce the classical curlicue fractal pattern; rational values produce closed loops."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 4,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_curliecue2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let speed = get_param(xform_id, variation_id, 0u);

    var x0 = get_state(xform_id, variation_id, 0u);
    var y0 = get_state(xform_id, variation_id, 1u);
    var theta = get_state(xform_id, variation_id, 2u);
    var phi = get_state(xform_id, variation_id, 3u);

    let x1 = x0 + 0.001 * cos(phi);
    let y1 = y0 + 0.001 * sin(phi);
    x0 = x1;
    y0 = y1;
    let new_phi = (theta + phi) - floor((theta + phi) / two_pi) * two_pi;
    let new_theta = (theta + two_pi * speed) - floor((theta + two_pi * speed) / two_pi) * two_pi;

    set_state(xform_id, variation_id, 0u, x0);
    set_state(xform_id, variation_id, 1u, y0);
    set_state(xform_id, variation_id, 2u, new_theta);
    set_state(xform_id, variation_id, 3u, new_phi);

    return vec2<f32>(x0, y0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_curliecue2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let speed = get_param(xform_id, variation_id, 0u);

    var x0 = get_state(xform_id, variation_id, 0u);
    var y0 = get_state(xform_id, variation_id, 1u);
    var theta = get_state(xform_id, variation_id, 2u);
    var phi = get_state(xform_id, variation_id, 3u);

    let x1 = x0 + 0.001 * cos(phi);
    let y1 = y0 + 0.001 * sin(phi);
    x0 = x1;
    y0 = y1;
    let new_phi = (theta + phi) - floor((theta + phi) / two_pi) * two_pi;
    let new_theta = (theta + two_pi * speed) - floor((theta + two_pi * speed) / two_pi) * two_pi;

    set_state(xform_id, variation_id, 0u, x0);
    set_state(xform_id, variation_id, 1u, y0);
    set_state(xform_id, variation_id, 2u, new_theta);
    set_state(xform_id, variation_id, 3u, new_phi);

    return vec3<f32>(x0, y0, p.z);
}
"#),
};
