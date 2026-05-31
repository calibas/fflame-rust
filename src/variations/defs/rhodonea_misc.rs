//! rhodonea (CozyG, March 2015)
//!
//! Rose curves: r = cos(k·θ) + radial_offset, where k = knumer/kdenom.
//! Maps incoming points onto the rose with mode-dependent inner/outer
//! spread behaviors.
//!
//! 15 user params + 5 init slots (`_kn, _kd, _k, _cycles_to_close,
//! _cycles`) — over the old 16-slot ceiling, unblocked by packed buffer.
//!
//! Init logic is non-trivial: tries to compute the minimum cycle count
//! to close the curve based on whether `k` is integer, rational
//! (computed via Euclidean gcd), or irrational. The full case ladder
//! mirrors cpp.
//!
//! Body uses 7 inner-mode × 7 outer-mode switch cases. Modes 5/6
//! (mask) are ported but the cpp `pVarTP.doHide = true` path collapses
//! to "no contribution" since we lack a per-iteration hide mechanism;
//! visually equivalent to dropping the point.
//!
//! cpp's `if (FPy == 0) { FPx = 0 }` edge case in modes 1 of both
//! inner/outer is dropped — the condition fires almost never (exact
//! float equality), and replicating it would require the
//! `(desired - accum) / weight` workaround for one branch.
//!
//! Source: `output/jwildfire-vars/output/rhodonea.cpp`.
//! References:
//!   http://en.wikipedia.org/wiki/Rose_%28mathematics%29
//!   http://mathworld.wolfram.com/Rose.html

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Rose curves (rhodonea) — `r = cos(k·θ) + radial_offset` with `k =
/// knumer/kdenom`. Maps incoming points onto the rose with 7 inner modes
/// (when `|rin| ≤ |r|`) and 7 outer modes (when `|rin| > |r|`), giving 49
/// spread/mask combinations. The init logic computes the minimum cycle
/// count needed to close the curve, handling integer `k` (1 or ½ cycle
/// depending on parity), rational `k` (Euclidean gcd reduction), and
/// irrational `k` (16-cycle fallback). `metacycle_expansion` distorts
/// amplitude across cycles for spiral-like radial growth; `fill` adds
/// uniform random radial jitter.
///
/// # Authors
/// - CozyG
pub static RHODONEA: VariationDef = VariationDef {
    name: "rhodonea",
    aliases: &[],
    display_name: "Rhodonea",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("knumer", "K Numer", unlimited_float, 3.0, -50.0, 50.0, "Rose-curve `k = knumer/kdenom` numerator. Integer `k` produces `k` petals (even) or `2k` petals (odd); rational `k` produces nested petal patterns with `cycles_to_close = kdenom` cycles."),
        param!("kdenom", "K Denom", unlimited_float, 4.0, -50.0, 50.0, "Rose-curve `k = knumer/kdenom` denominator."),
        param!("radial_offset", "Radial Offset", unlimited_float, 0.0, -10.0, 10.0, "Additive offset to the radial formula: `r = cos(k·θ) + radial_offset`. Non-zero values create an inner loop or distort the petals away from the origin."),
        param!("inner_mode", "Inner Mode", enum, 1, &["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Hide", "Pass Through"], "Behavior for points inside the rose curve (`|rin| ≤ |r|`)."),
        param!("outer_mode", "Outer Mode", enum, 1, &["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Pass Through", "Hide"], "Behavior for points outside the rose curve (`|rin| > |r|`). Modes 0-4 mirror the spread formulas of `inner_mode` (using `outer_spread` instead). The mask modes (Pass Through / Hide) have the opposite ordering from `inner_mode` — each label describes what happens at *this* side, so the underlying inversion is invisible to the user."),
        param!("inner_spread", "Inner Spread", unlimited_float, 0.0, -10.0, 10.0, "Spread amplitude for inner modes 1-4. Scales how strongly the input position influences the displacement from the rose curve."),
        param!("outer_spread", "Outer Spread", unlimited_float, 0.0, -10.0, 10.0, "Spread amplitude for outer modes 1-4."),
        param!("inner_spread_ratio", "Inner Spread Ratio", unlimited_float, 1.0, -10.0, 10.0, "X/Y asymmetry for the inner spread. 1.0 = symmetric; other values stretch the spread along X relative to Y."),
        param!("outer_spread_ratio", "Outer Spread Ratio", unlimited_float, 1.0, -10.0, 10.0, "X/Y asymmetry for the outer spread."),
        param!("spread_split", "Spread Split", unlimited_float, 1.0, -10.0, 10.0, "Scale factor for the input-radius comparison: `rin = spread_split · |p|`. Larger values make the curve appear smaller (more inputs classify as outside)."),
        param!("cycles", "Cycles", unlimited_float, 0.0, 0.0, 100.0, "Number of θ cycles to traverse per iteration. 0 = auto-compute as `cycles_to_close × metacycles`; non-zero overrides the auto-compute."),
        param!("cycle_offset", "Cycle Offset", unlimited_float, 0.0, -10.0, 10.0, "Phase offset for θ in fractions of 2π. Rotates the rose orientation."),
        param!("metacycle_expansion", "Metacycle Expansion", unlimited_float, 0.0, -10.0, 10.0, "Per-metacycle radial expansion factor. Non-zero values cause the rose amplitude to grow linearly across subsequent metacycles, creating spiral patterns."),
        param!("metacycles", "Metacycles", unlimited_float, 1.0, 0.0, 100.0, "Number of metacycles when `cycles == 0`. Each metacycle is one `cycles_to_close` worth of θ."),
        param!("fill", "Fill", unlimited_float, 0.0, 0.0, 1.0, "Random radial jitter amplitude. Adds `fill · (rng - 0.5)` to `r` per iteration, filling in the rose shape with noise."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 5,
    wgsl_init: Some(r#"
fn init_rhodonea(user: array<f32, 15>) -> array<f32, 5> {
    let knumer = user[0];
    let kdenom = user[1];
    let radial_offset = user[2];
    let inner_spread = user[5];
    let outer_spread = user[6];
    let cycles_param = user[10];
    let metacycles = user[13];
    let fill = user[14];

    var kn = knumer;
    var kd = kdenom;
    let safe_kd = select(kd, 1e-30, abs(kd) < 1e-30);
    var k = kn / safe_kd;
    var cycles_to_close: f32 = 0.0;

    let k_is_integer = abs(k - floor(k + 0.5)) < 1e-6;
    let kn_is_integer = abs(kn - floor(kn + 0.5)) < 1e-6;
    let kd_is_integer = abs(kd - floor(kd + 0.5)) < 1e-6;
    let k_int = floor(k + 0.5);

    if (k_is_integer) {
        let k_even = abs(k_int - floor(k_int * 0.5) * 2.0) < 0.5;
        if (k_even) {
            cycles_to_close = 1.0;
        } else {
            let has_offset = (radial_offset != 0.0) || (inner_spread != 0.0) || (outer_spread != 0.0) || (fill != 0.0);
            if (has_offset) { cycles_to_close = 1.0; } else { cycles_to_close = 0.5; }
        }
    } else if (kn_is_integer && kd_is_integer) {
        // Euclidean gcd via floor-mod, capped at 64 iterations for safety
        var a = abs(floor(kn + 0.5));
        var b = abs(floor(kd + 0.5));
        for (var i = 0u; i < 64u; i = i + 1u) {
            if (b < 0.5) { break; }
            let t = b;
            b = a - floor(a / b) * b;
            a = t;
        }
        let gcd = max(a, 1.0);
        if (gcd > 1.5) {
            kn = kn / gcd;
            kd = kd / gcd;
        }
        let kn_even = abs(kn - floor(kn * 0.5) * 2.0) < 0.5;
        let kd_even = abs(kd - floor(kd * 0.5) * 2.0) < 0.5;
        if (kn_even || kd_even) {
            cycles_to_close = kd;
        } else {
            cycles_to_close = kd * 0.5;
        }
    } else {
        cycles_to_close = 2.0 * kn * kd;
        if (cycles_param < 16.0) { cycles_to_close = 16.0; }
    }

    var cycles_final: f32 = 0.0;
    if (cycles_param == 0.0) {
        cycles_final = cycles_to_close * metacycles;
    } else {
        cycles_final = cycles_param;
    }

    var out: array<f32, 5>;
    out[0] = kn;
    out[1] = kd;
    out[2] = k;
    out[3] = cycles_to_close;
    out[4] = cycles_final;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_rhodonea(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let radial_offset = get_param(xform_id, variation_id, 2u);
    let inner_mode = i32(get_param(xform_id, variation_id, 3u));
    let outer_mode = i32(get_param(xform_id, variation_id, 4u));
    let inner_spread = get_param(xform_id, variation_id, 5u);
    let outer_spread = get_param(xform_id, variation_id, 6u);
    let inner_ratio = get_param(xform_id, variation_id, 7u);
    let outer_ratio = get_param(xform_id, variation_id, 8u);
    let spread_split = get_param(xform_id, variation_id, 9u);
    let cycle_offset = get_param(xform_id, variation_id, 11u);
    let meta_expand = get_param(xform_id, variation_id, 12u);
    let fill = get_param(xform_id, variation_id, 14u);
    let k = get_param(xform_id, variation_id, 17u);
    let cycles_to_close = get_param(xform_id, variation_id, 18u);
    let cycles = get_param(xform_id, variation_id, 19u);

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);

    let rin = spread_split * length(p);
    let tin = atan2(p.y, p.x);
    let t = cycles * (tin + cycle_offset * two_pi);
    var r = cos(k * t) + radial_offset;
    if (fill != 0.0) {
        r = r + fill * (rng_nextf(rng) - 0.5);
    }
    let x_curve = r * cos(t);
    let y_curve = r * sin(t);

    let safe_close = select(cycles_to_close * two_pi, 1e-30, abs(cycles_to_close) < 1e-30);
    let expansion = floor((cycles * (tin + pi)) / safe_close);
    // adjustedAmount = weight + expansion * meta_expand
    // Body must return adjustedAmount/weight times the geometric output so
    // framework's outer multiplier (× weight) reproduces cpp's adjustedAmount * (...).
    let amt_factor = 1.0 + expansion * meta_expand / safe_w;

    var ox = 0.0;
    var oy = 0.0;
    if (abs(rin) > abs(r)) {
        // Outer modes
        if (outer_mode == 0) {
            ox = amt_factor * x_curve;
            oy = amt_factor * y_curve;
        } else if (outer_mode == 1) {
            let rinx = rin * outer_spread * outer_ratio - outer_spread * outer_ratio + 1.0;
            let riny = rin * outer_spread - outer_spread + 1.0;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 2) {
            var xin = abs(p.x);
            var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * (xin - x_curve));
            oy = amt_factor * (y_curve + outer_spread * (yin - y_curve));
        } else if (outer_mode == 3) {
            var xin = abs(p.x);
            var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * xin);
            oy = amt_factor * (y_curve + outer_spread * yin);
        } else if (outer_mode == 4) {
            let rinx = 0.5 * rin + outer_spread * outer_ratio;
            let riny = 0.5 * rin + outer_spread;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 5) {
            // mask "inside" — pass-through (FPx += FTx, FPy += FTy)
            ox = p.x / safe_w;
            oy = p.y / safe_w;
        }
        // mode 6 (mask "outside" / hide): contribution stays zero — point
        // collapses to origin, visually equivalent to hiding.
    } else {
        // Inner modes
        if (inner_mode == 0) {
            ox = amt_factor * x_curve;
            oy = amt_factor * y_curve;
        } else if (inner_mode == 1) {
            let rinx = rin * inner_spread * inner_ratio - inner_spread * inner_ratio + 1.0;
            let riny = rin * inner_spread - inner_spread + 1.0;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (inner_mode == 2) {
            var xin = abs(p.x);
            var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve - inner_spread * inner_ratio * (x_curve - xin));
            oy = amt_factor * (y_curve - inner_spread * (y_curve - yin));
        } else if (inner_mode == 3) {
            var xin = abs(p.x);
            var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve - inner_spread * inner_ratio * xin);
            oy = amt_factor * (y_curve - inner_spread * yin);
        } else if (inner_mode == 4) {
            let rinx = 0.5 * rin + inner_spread * inner_ratio;
            let riny = 0.5 * rin + inner_spread;
            ox = amt_factor * rinx * x_curve;
            oy = amt_factor * riny * y_curve;
        } else if (inner_mode == 6) {
            // mask "outside" — pass-through
            ox = p.x / safe_w;
            oy = p.y / safe_w;
        }
        // mode 5 (mask "inside" / hide): contribution stays zero.
    }

    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: r#"
fn variation_rhodonea(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let r2 = variation_rhodonea_2d(p.xy, xform_id, variation_id, rng);
    return vec3<f32>(r2.x, r2.y, p.z);
}

fn variation_rhodonea_2d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let radial_offset = get_param(xform_id, variation_id, 2u);
    let inner_mode = i32(get_param(xform_id, variation_id, 3u));
    let outer_mode = i32(get_param(xform_id, variation_id, 4u));
    let inner_spread = get_param(xform_id, variation_id, 5u);
    let outer_spread = get_param(xform_id, variation_id, 6u);
    let inner_ratio = get_param(xform_id, variation_id, 7u);
    let outer_ratio = get_param(xform_id, variation_id, 8u);
    let spread_split = get_param(xform_id, variation_id, 9u);
    let cycle_offset = get_param(xform_id, variation_id, 11u);
    let meta_expand = get_param(xform_id, variation_id, 12u);
    let fill = get_param(xform_id, variation_id, 14u);
    let k = get_param(xform_id, variation_id, 17u);
    let cycles_to_close = get_param(xform_id, variation_id, 18u);
    let cycles = get_param(xform_id, variation_id, 19u);

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);

    let rin = spread_split * length(p);
    let tin = atan2(p.y, p.x);
    let t = cycles * (tin + cycle_offset * two_pi);
    var r = cos(k * t) + radial_offset;
    if (fill != 0.0) {
        r = r + fill * (rng_nextf(rng) - 0.5);
    }
    let x_curve = r * cos(t);
    let y_curve = r * sin(t);

    let safe_close = select(cycles_to_close * two_pi, 1e-30, abs(cycles_to_close) < 1e-30);
    let expansion = floor((cycles * (tin + pi)) / safe_close);
    let amt_factor = 1.0 + expansion * meta_expand / safe_w;

    var ox = 0.0;
    var oy = 0.0;
    if (abs(rin) > abs(r)) {
        if (outer_mode == 0) { ox = amt_factor * x_curve; oy = amt_factor * y_curve; }
        else if (outer_mode == 1) {
            let rinx = rin * outer_spread * outer_ratio - outer_spread * outer_ratio + 1.0;
            let riny = rin * outer_spread - outer_spread + 1.0;
            ox = amt_factor * rinx * x_curve; oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 2) {
            var xin = abs(p.x); var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * (xin - x_curve));
            oy = amt_factor * (y_curve + outer_spread * (yin - y_curve));
        } else if (outer_mode == 3) {
            var xin = abs(p.x); var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve + outer_spread * outer_ratio * xin);
            oy = amt_factor * (y_curve + outer_spread * yin);
        } else if (outer_mode == 4) {
            let rinx = 0.5 * rin + outer_spread * outer_ratio;
            let riny = 0.5 * rin + outer_spread;
            ox = amt_factor * rinx * x_curve; oy = amt_factor * riny * y_curve;
        } else if (outer_mode == 5) {
            ox = p.x / safe_w; oy = p.y / safe_w;
        }
    } else {
        if (inner_mode == 0) { ox = amt_factor * x_curve; oy = amt_factor * y_curve; }
        else if (inner_mode == 1) {
            let rinx = rin * inner_spread * inner_ratio - inner_spread * inner_ratio + 1.0;
            let riny = rin * inner_spread - inner_spread + 1.0;
            ox = amt_factor * rinx * x_curve; oy = amt_factor * riny * y_curve;
        } else if (inner_mode == 2) {
            var xin = abs(p.x); var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve - inner_spread * inner_ratio * (x_curve - xin));
            oy = amt_factor * (y_curve - inner_spread * (y_curve - yin));
        } else if (inner_mode == 3) {
            var xin = abs(p.x); var yin = abs(p.y);
            if (x_curve < 0.0) { xin = -xin; }
            if (y_curve < 0.0) { yin = -yin; }
            ox = amt_factor * (x_curve - inner_spread * inner_ratio * xin);
            oy = amt_factor * (y_curve - inner_spread * yin);
        } else if (inner_mode == 4) {
            let rinx = 0.5 * rin + inner_spread * inner_ratio;
            let riny = 0.5 * rin + inner_spread;
            ox = amt_factor * rinx * x_curve; oy = amt_factor * riny * y_curve;
        } else if (inner_mode == 6) {
            ox = p.x / safe_w; oy = p.y / safe_w;
        }
    }

    return vec2<f32>(ox, oy);
}
"#,
};
