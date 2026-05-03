//! siercarpet_js (Sosa, 2017)
//!
//! Cross Carpet by Roger Bagula
//! (http://paulbourke.net/fractals/crosscarpet/roger21.basic) ported
//! to JWildfire by Sosa. Java-recovered (cpp APO_VARIABLES is empty
//! `unported_stub`; Java has 1 user param `m` clamped [3, 12]).
//!
//! 1 user param: `m` (int 3-12, default 3 — number of arms in the
//! cross). No init slots.
//!
//! cpp computes 25-element `_a[]` and `_b[]` lookup tables of cos/sin
//! values inside the body each iteration. We compute the single needed
//! `(a[l], b[l])` pair inline based on the random index `l` parity:
//!   - odd l:  a = cos(2π·i/m),  b = sin(2π·i/m), with i = (l+1)/2
//!   - even l: a = (cos(2π·i/m) + cos(2π·(i-1)/m))/2, similarly for b,
//!             with i = l/2
//!
//! cpp's persistent `_d` state is irrelevant — its update guard
//! `d % (2m) != 5 % 2*m` evaluates to true on the very first iteration
//! and stays true forever (after `d = 1` is set), so we simply always
//! enter the body's branch.
//!
//! Body factors cleanly through outer multiplier.
//!
//! Source: `output/jwildfire-vars/output/siercarpet_js.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static SIERCARPET_JS: VariationDef = VariationDef {
    name: "siercarpet_js",
    display_name: "Siercarpet (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("m", "M", int, 3.0, 3.0, 12.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_siercarpet_js(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let m = clamp(i32(get_param(xform_id, variation_id, 0u)), 3, 12);
    let m_f = f32(m);
    let two_pi_over_m = 6.28318530717959 / m_f;
    let inv_sqrt2 = 0.7071067811865476;

    let l_raw = 1 + i32(2.0 * m_f * rng_nextf(rng));
    let l = clamp(l_raw, 1, 2 * m);
    var a_l: f32;
    var b_l: f32;
    if ((l & 1) == 1) {
        let i_idx = f32((l + 1) / 2);
        a_l = cos(two_pi_over_m * i_idx);
        b_l = sin(two_pi_over_m * i_idx);
    } else {
        let i_idx = f32(l / 2);
        let i_prev = i_idx - 1.0;
        a_l = (cos(two_pi_over_m * i_idx) + cos(two_pi_over_m * i_prev)) * 0.5;
        b_l = (sin(two_pi_over_m * i_idx) + sin(two_pi_over_m * i_prev)) * 0.5;
    }

    let x = p.x / 3.0 + (a_l - b_l) * inv_sqrt2;
    let y = p.y / 3.0 + (a_l + b_l) * inv_sqrt2;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_siercarpet_js(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let m = clamp(i32(get_param(xform_id, variation_id, 0u)), 3, 12);
    let m_f = f32(m);
    let two_pi_over_m = 6.28318530717959 / m_f;
    let inv_sqrt2 = 0.7071067811865476;

    let l_raw = 1 + i32(2.0 * m_f * rng_nextf(rng));
    let l = clamp(l_raw, 1, 2 * m);
    var a_l: f32;
    var b_l: f32;
    if ((l & 1) == 1) {
        let i_idx = f32((l + 1) / 2);
        a_l = cos(two_pi_over_m * i_idx);
        b_l = sin(two_pi_over_m * i_idx);
    } else {
        let i_idx = f32(l / 2);
        let i_prev = i_idx - 1.0;
        a_l = (cos(two_pi_over_m * i_idx) + cos(two_pi_over_m * i_prev)) * 0.5;
        b_l = (sin(two_pi_over_m * i_idx) + sin(two_pi_over_m * i_prev)) * 0.5;
    }

    let x = p.x / 3.0 + (a_l - b_l) * inv_sqrt2;
    let y = p.y / 3.0 + (a_l + b_l) * inv_sqrt2;
    return vec3<f32>(x, y, p.z);
}
"#),
};
