//! Variations with substantial init-time precomputation
//!
//! These ports inline the upstream `PluginVarPrepare` / Java `init()` logic
//! into the per-iteration WGSL body since we have no init hook. The cost is
//! redoing constant-fold-able math every iteration; in practice the trig
//! and exp ops here are dominated by the same per-iteration work the
//! variation already does, so the perf overhead is small.
//!
//! This batch:
//!   - `cpow2`, `cpow3` — Zueuk's complex-power julia variants (4 params,
//!     6-7 init values each, RNG)
//!   - `disc2` — Z+ variation Jan 07 with twist+rot (2 params, 3 init
//!     values, conditional adjustments)
//!
//! Notes:
//!   - All three preserve the C++ porter's `atan2(x, y)` (swapped from
//!     Java's `getPrecalcAtanYX() = atan2(y, x)`). Same recurring bug as
//!     `log_db`. Faithful to upstream C++ since flames built against it
//!     have already absorbed the discrepancy.
//!   - `cpow3` body uses a local `d_calc` since the user-param `d` is
//!     reassigned by the init step.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// =============================================================================
// cpow2: Zueuk's complex-power julia, version 2 (range-clamped)
//   Init:
//     ang = 2π / divisor
//     c = r · cos(π/2 · a) / divisor
//     d = r · sin(π/2 · a) / divisor
//     half_c = c/2,  half_d = d/2
//     inv_range = 0.5 / range
//     full_range = 2π · range
//   Body:
//     a = atan2(x, y)                         (C++ porter swap, see notes)
//     n = uniform_int_in [0, range)
//     if a < 0: n++
//     a += 2π · n
//     if cos(a · inv_range) < 2·rand-1: a -= full_range
//     lnr2 = log(x² + y²)
//     r_out = exp(half_c · lnr2 − d · a)
//     th = c · a + half_d · lnr2 + ang · floor(divisor · rand)
//     (r_out · cos(th), r_out · sin(th))
// =============================================================================
pub static CPOW2: VariationDef = VariationDef {
    name: "cpow2",
    display_name: "CPow2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "r", display_name: "R", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "a", display_name: "A", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "divisor", display_name: "Divisor", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "range", display_name: "Range", param_type: ParamType::Integer,
                            default_value: 1.0, min_value: Some(1.0), max_value: Some(64.0) },
    ],
    // 7 derived values stored in slots 4..11 of this variation's slot range:
    //   4: ang        (2π / divisor)
    //   5: c          (r · cos(π/2 · a) / divisor)
    //   6: d          (r · sin(π/2 · a) / divisor)
    //   7: half_c     (c / 2)
    //   8: half_d     (d / 2)
    //   9: inv_range  (0.5 / range)
    //   10: full_range (2π · range)
    needs_affine: false,
    init_param_count: 7,
    wgsl_init: Some(r#"
fn init_cpow2(user: array<f32, 4>) -> array<f32, 7> {
    let r_p = user[0];
    let a_p = user[1];
    let divisor = user[2];
    let range_p = max(user[3], 1.0);
    let safe_div = select(divisor, 1e-30, divisor == 0.0);
    let c = r_p * cos(1.5707963267948966 * a_p) / safe_div;
    let d = r_p * sin(1.5707963267948966 * a_p) / safe_div;
    var out: array<f32, 7>;
    out[0] = 6.28318530717959 / safe_div;        // ang
    out[1] = c;                                   // c
    out[2] = d;                                   // d
    out[3] = c * 0.5;                             // half_c
    out[4] = d * 0.5;                             // half_d
    out[5] = 0.5 / range_p;                       // inv_range
    out[6] = 6.28318530717959 * range_p;          // full_range
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_cpow2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let divisor = get_param(xform_id, variation_id, 2u);
    let range_p = max(get_param(xform_id, variation_id, 3u), 1.0);
    let ang = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let d = get_param(xform_id, variation_id, 6u);
    let half_c = get_param(xform_id, variation_id, 7u);
    let half_d = get_param(xform_id, variation_id, 8u);
    let inv_range = get_param(xform_id, variation_id, 9u);
    let full_range = get_param(xform_id, variation_id, 10u);

    var a = atan2(p.x, p.y);
    var n = i32(rng_next(rng) % u32(range_p));
    if (a < 0.0) { n = n + 1; }
    a = a + two_pi * f32(n);
    if (cos(a * inv_range) < rng_nextf(rng) * 2.0 - 1.0) {
        a = a - full_range;
    }
    let lnr2 = log(max(p.x * p.x + p.y * p.y, 1e-30));
    let r_out = exp(half_c * lnr2 - d * a);
    let th = c * a + half_d * lnr2 + ang * floor(divisor * rng_nextf(rng));
    return vec2<f32>(r_out * cos(th), r_out * sin(th));
}
"#,
    wgsl_3d: Some(r#"
fn variation_cpow2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let divisor = get_param(xform_id, variation_id, 2u);
    let range_p = max(get_param(xform_id, variation_id, 3u), 1.0);
    let ang = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let d = get_param(xform_id, variation_id, 6u);
    let half_c = get_param(xform_id, variation_id, 7u);
    let half_d = get_param(xform_id, variation_id, 8u);
    let inv_range = get_param(xform_id, variation_id, 9u);
    let full_range = get_param(xform_id, variation_id, 10u);

    var a = atan2(p.x, p.y);
    var n = i32(rng_next(rng) % u32(range_p));
    if (a < 0.0) { n = n + 1; }
    a = a + two_pi * f32(n);
    if (cos(a * inv_range) < rng_nextf(rng) * 2.0 - 1.0) {
        a = a - full_range;
    }
    let lnr2 = log(max(p.x * p.x + p.y * p.y, 1e-30));
    let r_out = exp(half_c * lnr2 - d * a);
    let th = c * a + half_d * lnr2 + ang * floor(divisor * rng_nextf(rng));
    return vec3<f32>(r_out * cos(th), r_out * sin(th), p.z);
}
"#),
};

// =============================================================================
// cpow3: Zueuk's complex-power julia, version 3 (logarithm-shifted)
//   Init (from Java):
//     ang = 2π / divisor
//     p_a = atan2((d<0 ? -log(-d) : log(d)) · r, 2π)
//     c = cos(p_a) · r · cos(p_a) / divisor
//     d_calc = cos(p_a) · r · sin(p_a) / divisor    (NB: shadows the param `d`)
//     half_c = c/2,  half_d = d_calc/2
//     coeff = (d_calc == 0) ? 0 : -0.095 · spread / d_calc
//   Body:
//     a = atan2(x, y)                                (C++ porter swap)
//     if a < 0: a += 2π
//     if cos(a/2) < 2·rand-1: a -= 2π
//     a += (rand<0.5 ? +1 : -1) · 2π · round(log(rand) · coeff)
//     lnr2 = log(x² + y²)
//     r_out = exp(half_c · lnr2 − d_calc · a)
//     th = c · a + half_d · lnr2 + ang · floor(divisor · rand)
// =============================================================================
pub static CPOW3: VariationDef = VariationDef {
    name: "cpow3",
    display_name: "CPow3",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "r", display_name: "R", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "d", display_name: "D", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "divisor", display_name: "Divisor", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "spread", display_name: "Spread", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0) },
    ],
    // 6 derived values stored in slots 4..10 of this variation's slot range:
    //   4: ang        (2π / divisor)
    //   5: c
    //   6: d_calc
    //   7: half_c
    //   8: half_d
    //   9: coeff      (−0.095 · spread / d_calc, or 0 if d_calc == 0)
    needs_affine: false,
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_cpow3(user: array<f32, 4>) -> array<f32, 6> {
    let r_p = user[0];
    let d_p = user[1];
    let divisor = user[2];
    let spread = user[3];
    let safe_div = select(divisor, 1e-30, divisor == 0.0);
    let log_d = select(log(max(d_p, 1e-30)), -log(max(-d_p, 1e-30)), d_p < 0.0);
    let p_a = atan2(log_d * r_p, 6.28318530717959);
    let cos_pa = cos(p_a);
    let c = cos_pa * r_p * cos(p_a) / safe_div;
    let d_calc = cos_pa * r_p * sin(p_a) / safe_div;
    var out: array<f32, 6>;
    out[0] = 6.28318530717959 / safe_div;                            // ang
    out[1] = c;                                                       // c
    out[2] = d_calc;                                                  // d_calc
    out[3] = c * 0.5;                                                 // half_c
    out[4] = d_calc * 0.5;                                            // half_d
    out[5] = select(-0.095 * spread / d_calc, 0.0, d_calc == 0.0);    // coeff
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_cpow3(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let divisor = get_param(xform_id, variation_id, 2u);
    let ang = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let d_calc = get_param(xform_id, variation_id, 6u);
    let half_c = get_param(xform_id, variation_id, 7u);
    let half_d = get_param(xform_id, variation_id, 8u);
    let coeff = get_param(xform_id, variation_id, 9u);

    var a = atan2(p.x, p.y);
    if (a < 0.0) { a = a + two_pi; }
    if (cos(a * 0.5) < rng_nextf(rng) * 2.0 - 1.0) { a = a - two_pi; }
    let sign_step = select(-1.0, 1.0, rng_nextf(rng) < 0.5);
    a = a + sign_step * two_pi * round(log(max(rng_nextf(rng), 1e-30)) * coeff);

    let lnr2 = log(max(p.x * p.x + p.y * p.y, 1e-30));
    let r_out = exp(half_c * lnr2 - d_calc * a);
    let th = c * a + half_d * lnr2 + ang * floor(divisor * rng_nextf(rng));
    return vec2<f32>(r_out * cos(th), r_out * sin(th));
}
"#,
    wgsl_3d: Some(r#"
fn variation_cpow3(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let divisor = get_param(xform_id, variation_id, 2u);
    let ang = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let d_calc = get_param(xform_id, variation_id, 6u);
    let half_c = get_param(xform_id, variation_id, 7u);
    let half_d = get_param(xform_id, variation_id, 8u);
    let coeff = get_param(xform_id, variation_id, 9u);

    var a = atan2(p.x, p.y);
    if (a < 0.0) { a = a + two_pi; }
    if (cos(a * 0.5) < rng_nextf(rng) * 2.0 - 1.0) { a = a - two_pi; }
    let sign_step = select(-1.0, 1.0, rng_nextf(rng) < 0.5);
    a = a + sign_step * two_pi * round(log(max(rng_nextf(rng), 1e-30)) * coeff);

    let lnr2 = log(max(p.x * p.x + p.y * p.y, 1e-30));
    let r_out = exp(half_c * lnr2 - d_calc * a);
    let th = c * a + half_d * lnr2 + ang * floor(divisor * rng_nextf(rng));
    return vec3<f32>(r_out * cos(th), r_out * sin(th), p.z);
}
"#),
};

// =============================================================================
// disc2: Z+ variation (Jan 07) — twist+rot disc
//   Init:
//     timespi = rot · π
//     sinadd = sin(twist)
//     cosadd = cos(twist) − 1
//     if twist > 2π: k = 1 + twist − 2π; sinadd*=k; cosadd*=k
//     if twist < −2π: k = 1 + twist + 2π; sinadd*=k; cosadd*=k
//   Body:
//     t = timespi · (x + y)
//     r = atan2(x, y) / π                            (C++ porter swap)
//     return ((sin(t) + cosadd)·r, (cos(t) + sinadd)·r)
// =============================================================================
pub static DISC2: VariationDef = VariationDef {
    name: "disc2",
    display_name: "Disc2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef { name: "rot", display_name: "Rot", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "twist", display_name: "Twist", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.5, min_value: Some(-10.0), max_value: Some(10.0) },
    ],
    // 3 derived values stored in slots 2..5:
    //   2: timespi  (rot · π)
    //   3: cosadd   (with conditional adjustment)
    //   4: sinadd   (with conditional adjustment)
    needs_affine: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_disc2(user: array<f32, 2>) -> array<f32, 3> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let rot = user[0];
    let twist = user[1];
    var sinadd = sin(twist);
    var cosadd = cos(twist) - 1.0;
    if (twist > two_pi) {
        let k = 1.0 + twist - two_pi;
        cosadd = cosadd * k;
        sinadd = sinadd * k;
    }
    if (twist < -two_pi) {
        let k = 1.0 + twist + two_pi;
        cosadd = cosadd * k;
        sinadd = sinadd * k;
    }
    var out: array<f32, 3>;
    out[0] = rot * pi;   // timespi
    out[1] = cosadd;
    out[2] = sinadd;
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_disc2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let pi = 3.14159265358979;
    let timespi = get_param(xform_id, variation_id, 2u);
    let cosadd = get_param(xform_id, variation_id, 3u);
    let sinadd = get_param(xform_id, variation_id, 4u);
    let t = timespi * (p.x + p.y);
    let r = atan2(p.x, p.y) / pi;
    return vec2<f32>((sin(t) + cosadd) * r, (cos(t) + sinadd) * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_disc2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let pi = 3.14159265358979;
    let timespi = get_param(xform_id, variation_id, 2u);
    let cosadd = get_param(xform_id, variation_id, 3u);
    let sinadd = get_param(xform_id, variation_id, 4u);
    let t = timespi * (p.x + p.y);
    let r = atan2(p.x, p.y) / pi;
    return vec3<f32>((sin(t) + cosadd) * r, (cos(t) + sinadd) * r, p.z);
}
"#),
};
