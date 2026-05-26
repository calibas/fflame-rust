//! Apophysis miscellany 16 — two more
//!
//!   - `seashell3D`   (Jesus Sosa)            — 4 user (a, b, c, n int)
//!                                                Java-recovered (cpp
//!                                                APO_VARIABLES empty);
//!                                                RNG; Full3D
//!                                                parametric sea shell.
//!                                                cpp uses `FPx = …`
//!                                                like anamorphcyl
//!   - `hypershift2`  (tatasz / Stefanov)     — 2 user (p int, q int)
//!                                                + 4 init slots
//!                                                (shift, scale2,
//!                                                scale, inv_p);
//!                                                RNG; Full3D;
//!                                                hyperbolic 2-shift;
//!                                                cpp `FPx = …` like
//!                                                anamorphcyl. cpp's
//!                                                `GOODRAND_0X(INT_MAX)
//!                                                % p` replaced with
//!                                                `floor(rand · p)`
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// seashell3D (Jesus Sosa)
//   t = 2π · rand;  s = 2π · rand
//   x = a · (1 − t/2π) · cos(n·t) · (1 + cos s) + c · cos(n·t)
//   y = a · (1 − t/2π) · sin(n·t) · (1 + cos s) + c · sin(n·t)
//   z = b · t/2π + a · (1 − t/2π) · sin s
//   out = (x, y, z)
// 4 user (a, b, c, n int) Java-recovered; cpp APO_VARIABLES empty.
// =============================================================================
/// Parametric sea shell — samples random `(t, s)` over `[0, 2π]²` and emits
/// a 3D sea-shell surface. The X/Y components trace a spiral of `n` turns
/// with radius tapering as `1 − t/2π`; the Z component is a linear ramp `b
/// · t/2π` plus a radial oscillation `a · (1 − t/2π) · sin s`. `c` adds an
/// inner-spiral offset.
///
/// # Authors
/// - Jesus Sosa
pub static SEASHELL3D: VariationDef = VariationDef {
    name: "seashell3D",
    display_name: "Seashell 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("a", "Final Radius", unlimited_float, 2.0, -10.0, 10.0, "Final-spiral radius (outer-edge radius coefficient)."),
        param!("b", "Height", unlimited_float, 6.0, -20.0, 20.0, "Height ramp coefficient on Z."),
        param!("c", "Inner Radius", unlimited_float, 0.0, -10.0, 10.0, "Inner-spiral radial offset added to X and Y."),
        param!("n", "N Spirals", int, 5.0, 1.0, 50.0, "Number of spiral turns."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_seashell3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let c = get_param(xform_id, variation_id, 2u);
    let n = get_param(xform_id, variation_id, 3u);
    let two_pi = 6.28318530717959;
    let inv_2pi = 0.15915494309189535;

    let t = two_pi * rng_nextf(rng);
    let s = two_pi * rng_nextf(rng);
    let one_minus_t = 1.0 - t * inv_2pi;
    let cos_s = cos(s);
    let cos_nt = cos(n * t);
    let sin_nt = sin(n * t);
    let x = a * one_minus_t * cos_nt * (1.0 + cos_s) + c * cos_nt;
    let y = a * one_minus_t * sin_nt * (1.0 + cos_s) + c * sin_nt;
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_seashell3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let n = get_param(xform_id, variation_id, 3u);
    let two_pi = 6.28318530717959;
    let inv_2pi = 0.15915494309189535;

    let t = two_pi * rng_nextf(rng);
    let s = two_pi * rng_nextf(rng);
    let one_minus_t = 1.0 - t * inv_2pi;
    let cos_s = cos(s);
    let cos_nt = cos(n * t);
    let sin_nt = sin(n * t);
    let x = a * one_minus_t * cos_nt * (1.0 + cos_s) + c * cos_nt;
    let y = a * one_minus_t * sin_nt * (1.0 + cos_s) + c * sin_nt;
    let z = b * t * inv_2pi + a * one_minus_t * sin(s);
    return vec3<f32>(x, y, z);
}
"#),
};

// =============================================================================
// hypershift2 (tatasz)
//   2 user: p (int), q (int)
//   4 init slots:
//     shift  = sin(π/2 − π/q − π/p) / sqrt(1 − sin²(π/q) − sin²(π/p))
//     scale2 = (sin(π/2 + π/p)/sin(π/q) − 1) / sqrt(sin²(π/2 + π/p)/sin²(π/q) − 1)
//     scale  = 1 − shift²
//     inv_p  = 1/p
//   FX = x · scale2;  FY = y · scale2
//   rad = 1/(FX² + FY²)
//   x' = rad · FX + shift;  y' = rad · FY
//   rad = w · scale / (x'² + y'²)
//   angle = (floor(rand · p)·2 + 1) · π/p
//   X = rad · x' + shift;  Y = rad · y'
//   FPx = cos(angle)·X − sin(angle)·Y
//   FPy = sin(angle)·X + cos(angle)·Y
//   FPz = z · rad
// Body has w in `rad` cleanly; clean factor through outer.
// =============================================================================
/// Hyperbolic {p, q} tiling — generates a hyperbolic tiling whose faces are
/// regular `p`-gons meeting `q`-fold at each vertex (e.g., {3,7} = the
/// order-7 triangular tiling). Reflects the input through a hyperbolic
/// shift transform tuned by the `(p, q)` Schläfli pair, then rotates to one
/// of `p` random angular sectors per iteration.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov
pub static HYPERSHIFT2: VariationDef = VariationDef {
    name: "hypershift2",
    display_name: "Hyper Shift 2",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("p", "P", int, 3.0, 2.0, 50.0, "Polygon side count — the {p, q} tiling has regular `p`-gon faces."),
        param!("q", "Q", int, 7.0, 2.0, 50.0, "Vertices per polygon — `q` of them meet at each vertex of the tiling."),
    ],
    needs_transform: false,
    writes_color: false,
    // 4 derived values at slots 2..6: shift, scale2, scale, inv_p
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_hypershift2(user: array<f32, 2>) -> array<f32, 4> {
    let pi = 3.14159265358979;
    let pi_2 = 1.5707963267948966;
    let p = max(user[0], 2.0);
    let q = max(user[1], 2.0);
    let pq = pi / q;
    let pp = pi / p;
    let spq = sin(pq);
    let spp = sin(pp);

    let shift_num = sin(pi_2 - pq - pp);
    let shift_den = sqrt(max(1.0 - spq * spq - spp * spp, 1e-30));
    let shift = shift_num / shift_den;

    let s_p2pp = sin(pi_2 + pp);
    let r_inner = s_p2pp * s_p2pp / (spq * spq) - 1.0;
    let scale2_a = 1.0 / sqrt(max(r_inner, 1e-30));
    let scale2 = scale2_a * (s_p2pp / spq - 1.0);

    let scale = 1.0 - shift * shift;
    var out: array<f32, 4>;
    out[0] = shift;
    out[1] = scale2;
    out[2] = scale;
    out[3] = 1.0 / p;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_hypershift2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let p_u = max(get_param(xform_id, variation_id, 0u), 2.0);
    let shift = get_param(xform_id, variation_id, 2u);
    let scale2 = get_param(xform_id, variation_id, 3u);
    let scale = get_param(xform_id, variation_id, 4u);
    let pi = 3.14159265358979;

    let FX = p.x * scale2;
    let FY = p.y * scale2;
    let denom1 = max(FX * FX + FY * FY, 1e-30);
    var rad = 1.0 / denom1;
    let xp = rad * FX + shift;
    let yp = rad * FY;
    let denom2 = max(xp * xp + yp * yp, 1e-30);
    rad = scale / denom2;
    let angle = (floor(rng_nextf(rng) * p_u) * 2.0 + 1.0) * pi / p_u;
    let X = rad * xp + shift;
    let Y = rad * yp;
    let cosa = cos(angle);
    let sina = sin(angle);
    return vec2<f32>(cosa * X - sina * Y, sina * X + cosa * Y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_hypershift2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let p_u = max(get_param(xform_id, variation_id, 0u), 2.0);
    let shift = get_param(xform_id, variation_id, 2u);
    let scale2 = get_param(xform_id, variation_id, 3u);
    let scale = get_param(xform_id, variation_id, 4u);
    let pi = 3.14159265358979;

    let FX = p.x * scale2;
    let FY = p.y * scale2;
    let denom1 = max(FX * FX + FY * FY, 1e-30);
    var rad = 1.0 / denom1;
    let xp = rad * FX + shift;
    let yp = rad * FY;
    let denom2 = max(xp * xp + yp * yp, 1e-30);
    rad = scale / denom2;
    let angle = (floor(rng_nextf(rng) * p_u) * 2.0 + 1.0) * pi / p_u;
    let X = rad * xp + shift;
    let Y = rad * yp;
    let cosa = cos(angle);
    let sina = sin(angle);
    return vec3<f32>(cosa * X - sina * Y, sina * X + cosa * Y, p.z * rad);
}
"#),
};
