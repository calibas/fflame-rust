//! Maurer Rose + Hypercrop
//!
//! Two larger param-heavy variations from upstream:
//!
//!   - `maurer_rose` (Gregg Helt / CozyG, 2016) — Maurer-rose curve with
//!     11 user params; samples random distance along nearest-line of the
//!     Maurer rose, with mode selection (line vs endpoint vs underlying
//!     rhodonea curve) by relative weight.
//!   - `hypercrop` (tatasz / Stefanov) — n-gon corner-cropping warp;
//!     3 user params (n, rad, zero), recovered from the Java comment
//!     block (cpp PluginVarCalc was an unported_stub).
//!
//! Sources:
//!   - output/jwildfire-vars/output/maurer_rose.cpp
//!   - output/jwildfire-vars/output/hypercrop.cpp
//!
//! Both factor VVAR through the outer-multiplier convention.
//!
//! `maurer_rose` now uses 10 init slots for values that depend only on
//! user params (k, step_size, safe_step, cycles, the three sampling
//! thresholds, and the three thicknesses). The 16-slot ceiling that
//! originally motivated inlining is gone with the packed-variation-
//! params layout.
//!
//! Skipped from the originally-planned set:
//!   - `synth` (35 user params) and `maurer_lines` (36 user params)
//!     exceed the 16-slot per-variation buffer budget.
//!   - `rhodonea` (15 params) has a BigInteger GCD computation in init
//!     to handle the `kn` / `kd` relative-prime case — translatable but
//!     not in this batch.
//!   - `mandelbrot` (12 params) uses a `vector<double>` dynamic array
//!     and persistent `_x0`/`_y0`/`_z0`/`_pIdx` state across iterations
//!     to implement a "sample-then-walk" pattern. Architecturally
//!     blocked (persistent state).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// maurer_rose: Maurer-rose curve sampler (Gregg Helt / CozyG)
//   k = kn / kd
//   step_size = 2π · (line_offset_degrees / 360)
//   cycles = line_count · step_size / (2π) = line_count · line_offset_degrees / 360
//   Body:
//     tin = atan2(y, x);  t = cycles · tin
//     step = floor(t / step_size)
//     theta1 = step · step_size;  theta2 = theta1 + step_size
//     (x1,y1) = curve(theta1);  (x2,y2) = curve(theta2)        where
//        curve(θ) = (cos(k·θ) + c) · (cos θ, sin θ)
//     pick line / point / endpoint / curve by RNG and weights
//     out = sampled point on chosen geometry
// =============================================================================
/// Maurer-rose curve sampler — a Maurer rose steps around a rhodonea (rose)
/// curve at fixed angular increments and connects consecutive samples with
/// straight lines. This variation picks a random point on the nearest
/// Maurer line, on one of its endpoints, or directly on the underlying rose
/// curve, with the mix between the three modes controlled by relative
/// weights.
///
/// # Authors
/// - CozyG
pub static MAURER_ROSE: VariationDef = VariationDef {
    name: "maurer_rose",
    aliases: &[],
    display_name: "Maurer Rose",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("kn", "K Numerator", unlimited_float, 2.0, -50.0, 50.0, "K numerator — the rose's petal ratio is `kn/kd`."),
        param!("kd", "K Denominator", unlimited_float, 1.0, -50.0, 50.0, "K denominator."),
        param!("c", "C Offset", unlimited_float, 0.0, -10.0, 10.0, "Constant offset added to the rose radius. Shifts the curve outward."),
        param!("line_count", "Line Count", unlimited_float, 360.0, 1.0, 10000.0, "Number of Maurer-rose line segments per cycle."),
        param!("line_offset_degrees", "Line Offset (deg)", unlimited_float, 71.0, -3600.0, 3600.0, "Angular step between successive samples on the rhodonea, in degrees. Together with line_count, controls how many cycles wrap around the rose."),
        param!("show_lines", "Show Lines", float, 1.0, 0.0, 10.0, "Relative weight of sampling along the Maurer line segments."),
        param!("show_points", "Show Points", float, 0.0, 0.0, 10.0, "Relative weight of sampling at the segment endpoints."),
        param!("show_curve", "Show Curve", float, 0.05, 0.0, 10.0, "Relative weight of sampling directly on the underlying rhodonea curve."),
        param!("line_thickness", "Line Thick (×100)", unlimited_float, 0.5, 0.0, 100.0, "Random jitter width around line samples (×100)."),
        param!("point_thickness", "Point Thick (×100)", unlimited_float, 3.0, 0.0, 100.0, "Random scatter radius around endpoint samples (×100)."),
        param!("curve_thickness", "Curve Thick (×100)", unlimited_float, 1.0, 0.0, 100.0, "Random jitter width around rose-curve samples (×100)."),
    ],
    init_param_count: 10,
    wgsl_init: Some(r#"
fn init_maurer_rose(user: array<f32, 11>) -> array<f32, 10> {
    let kn = user[0];
    let kd = user[1];
    let line_count = user[3];
    let line_offset_deg = user[4];
    let show_lines = user[5];
    let show_points = user[6];
    let show_curve = user[7];
    let line_thick_in = user[8];
    let point_thick_in = user[9];
    let curve_thick_in = user[10];
    let two_pi = 6.28318530717959;

    let safe_kd = select(kd, 1e-30, kd == 0.0);
    let step_size = two_pi * (line_offset_deg / 360.0);

    let show_sum = max(show_lines + show_points + show_curve, 1e-30);
    let line_frac = show_lines / show_sum;
    let point_frac = show_points / show_sum;

    var out: array<f32, 10>;
    out[0] = kn / safe_kd;                              // k
    out[1] = step_size;                                  // step_size
    out[2] = select(step_size, 1e-30, step_size == 0.0); // safe_step
    out[3] = (line_count * step_size) / two_pi;          // cycles
    out[4] = line_frac;                                  // line_thr
    out[5] = line_frac + point_frac;                     // point_thr
    out[6] = line_frac + 0.5 * point_frac;               // point_half_thr
    out[7] = line_thick_in / 100.0;                      // line_thickness
    out[8] = point_thick_in / 100.0;                     // point_thickness
    out[9] = curve_thick_in / 100.0;                     // curve_thickness
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_maurer_rose(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let c = get_param(xform_id, variation_id, 2u);
    let k = get_param(xform_id, variation_id, 11u);
    let step_size = get_param(xform_id, variation_id, 12u);
    let safe_step = get_param(xform_id, variation_id, 13u);
    let cycles = get_param(xform_id, variation_id, 14u);
    let line_thr = get_param(xform_id, variation_id, 15u);
    let point_thr = get_param(xform_id, variation_id, 16u);
    let point_half_thr = get_param(xform_id, variation_id, 17u);
    let line_thickness = get_param(xform_id, variation_id, 18u);
    let point_thickness = get_param(xform_id, variation_id, 19u);
    let curve_thickness = get_param(xform_id, variation_id, 20u);

    // ff_atan2: Metal's fast atan2 is pi/4 at same-sign zero pairs and
    // NaN at mixed-sign ones; the origin is reachable (respawn/fuse,
    // and the probe showed NaN there on Metal only). See utilities.wgsl.
    let tin = ff_atan2(p.y, p.x);
    let t = cycles * tin;
    let step = floor(t / safe_step);
    let theta1 = step * step_size;
    let theta2 = theta1 + step_size;

    // curve(θ) = (cos(k·θ) + c) · (cos θ, sin θ)
    let r1 = cos(k * theta1) + c;
    let x1 = r1 * cos(theta1);
    let y1 = r1 * sin(theta1);
    let r2 = cos(k * theta2) + c;
    let x2 = r2 * cos(theta2);
    let y2 = r2 * sin(theta2);

    let xdiff = x2 - x1;
    let ydiff = y2 - y1;
    let m = ydiff / select(xdiff, 1e-30, abs(xdiff) < 1e-30);
    let line_length = sqrt(xdiff * xdiff + ydiff * ydiff);

    let rnd = rng_nextf(rng);
    var xout: f32;
    var yout: f32;
    var xoff: f32 = 0.0;
    var yoff: f32 = 0.0;

    if (rnd < line_thr) {
        let d = rng_nextf(rng) * line_length;
        xoff = d / sqrt(1.0 + m * m);
        if (x2 < x1) { xoff = -xoff; }
        yoff = abs(m * xoff);
        if (y2 < y1) { yoff = -yoff; }
        if (line_thickness != 0.0) {
            xoff = xoff + (rng_nextf(rng) - 0.5) * line_thickness;
            yoff = yoff + (rng_nextf(rng) - 0.5) * line_thickness;
        }
        xout = x1 + xoff;
        yout = y1 + yoff;
    } else if (rnd <= point_thr) {
        if (point_thickness != 0.0) {
            let roff = rng_nextf(rng) * point_thickness;
            let rang = rng_nextf(rng) * 6.28318530717959;
            xoff = roff * cos(rang);
            yoff = roff * sin(rang);
        }
        if (rnd <= point_half_thr) {
            xout = x1 + xoff;
            yout = y1 + yoff;
        } else {
            xout = x2 + xoff;
            yout = y2 + yoff;
        }
    } else {
        // sample directly on the rhodonea curve at parameter t
        let r_curve = cos(k * t) + c;
        let cx = r_curve * cos(t);
        let cy = r_curve * sin(t);
        if (curve_thickness != 0.0) {
            xout = cx + (rng_nextf(rng) - 0.5) * curve_thickness;
            yout = cy + (rng_nextf(rng) - 0.5) * curve_thickness;
        } else {
            xout = cx;
            yout = cy;
        }
    }
    return vec2<f32>(xout, yout);
}
"#,
    wgsl_3d: r#"
fn variation_maurer_rose(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let c = get_param(xform_id, variation_id, 2u);
    let k = get_param(xform_id, variation_id, 11u);
    let step_size = get_param(xform_id, variation_id, 12u);
    let safe_step = get_param(xform_id, variation_id, 13u);
    let cycles = get_param(xform_id, variation_id, 14u);
    let line_thr = get_param(xform_id, variation_id, 15u);
    let point_thr = get_param(xform_id, variation_id, 16u);
    let point_half_thr = get_param(xform_id, variation_id, 17u);
    let line_thickness = get_param(xform_id, variation_id, 18u);
    let point_thickness = get_param(xform_id, variation_id, 19u);
    let curve_thickness = get_param(xform_id, variation_id, 20u);

    // ff_atan2: Metal's fast atan2 is pi/4 at same-sign zero pairs and
    // NaN at mixed-sign ones; the origin is reachable (respawn/fuse,
    // and the probe showed NaN there on Metal only). See utilities.wgsl.
    let tin = ff_atan2(p.y, p.x);
    let t = cycles * tin;
    let step = floor(t / safe_step);
    let theta1 = step * step_size;
    let theta2 = theta1 + step_size;

    let r1 = cos(k * theta1) + c;
    let x1 = r1 * cos(theta1);
    let y1 = r1 * sin(theta1);
    let r2 = cos(k * theta2) + c;
    let x2 = r2 * cos(theta2);
    let y2 = r2 * sin(theta2);

    let xdiff = x2 - x1;
    let ydiff = y2 - y1;
    let m = ydiff / select(xdiff, 1e-30, abs(xdiff) < 1e-30);
    let line_length = sqrt(xdiff * xdiff + ydiff * ydiff);

    let rnd = rng_nextf(rng);
    var xout: f32;
    var yout: f32;
    var xoff: f32 = 0.0;
    var yoff: f32 = 0.0;

    if (rnd < line_thr) {
        let d = rng_nextf(rng) * line_length;
        xoff = d / sqrt(1.0 + m * m);
        if (x2 < x1) { xoff = -xoff; }
        yoff = abs(m * xoff);
        if (y2 < y1) { yoff = -yoff; }
        if (line_thickness != 0.0) {
            xoff = xoff + (rng_nextf(rng) - 0.5) * line_thickness;
            yoff = yoff + (rng_nextf(rng) - 0.5) * line_thickness;
        }
        xout = x1 + xoff;
        yout = y1 + yoff;
    } else if (rnd <= point_thr) {
        if (point_thickness != 0.0) {
            let roff = rng_nextf(rng) * point_thickness;
            let rang = rng_nextf(rng) * 6.28318530717959;
            xoff = roff * cos(rang);
            yoff = roff * sin(rang);
        }
        if (rnd <= point_half_thr) {
            xout = x1 + xoff;
            yout = y1 + yoff;
        } else {
            xout = x2 + xoff;
            yout = y2 + yoff;
        }
    } else {
        let r_curve = cos(k * t) + c;
        let cx = r_curve * cos(t);
        let cy = r_curve * sin(t);
        if (curve_thickness != 0.0) {
            xout = cx + (rng_nextf(rng) - 0.5) * curve_thickness;
            yout = cy + (rng_nextf(rng) - 0.5) * curve_thickness;
        } else {
            xout = cx;
            yout = cy;
        }
    }
    return vec3<f32>(xout, yout, p.z);
}
"#,
};

// =============================================================================
// hypercrop: n-gon corner-cropping warp (tatasz / Stefanov)
//   coef = n / (2π);  a0 = π/n;  len = 1/cos(a0);  d = rad · sin(a0) · len
//   angle = atan2(y, x)
//   angle = floor(angle · coef) / coef + π/n   (snap to nearest n-gon spoke)
//   x0, y0 = (cos(angle), sin(angle)) · len      (n-gon corner)
//   if dist(p, corner) < d:                       (point is inside corner-disc)
//     zero > 1.5: out = corner
//     zero > 0.5: out = (0, 0, 0)
//     else:        out = corner + (cos(rang), sin(rang)) · d
//                  (rang = atan2(p − corner))
//   else:
//     out = (FTx, FTy, FTz)                      (pass-through)
//
// (Recovered from Java comment; cpp PluginVarCalc was unported_stub.)
// =============================================================================
/// N-gon corner-cropping warp — snaps the input angle to the nearest n-gon
/// spoke, finds that spoke's corner, and tests whether the input lies
/// inside a small disc around the corner. Inside, behavior depends on
/// `zero`: snap to corner, collapse to origin, or scatter around the disc
/// edge. Outside, the point passes through.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov
pub static HYPERCROP: VariationDef = VariationDef {
    name: "hypercrop",
    aliases: &[],
    display_name: "Hypercrop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[
        param!("n", "N", int, 4.0, 3.0, 50.0, "Number of n-gon sides (≥ 3)."),
        param!("rad", "Radius", unlimited_float, 1.0, 0.0, 10.0, "Radius of the corner-cropping disc, relative to the n-gon corner radius."),
        param!("zero", "Zero", unlimited_float, 0.0, 0.0, 2.0, "Behavior inside the corner disc. `> 1.5` snaps to the corner; `> 0.5` collapses to origin; else scatters around the disc edge."),
    ],
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_hypercrop(user: array<f32, 3>) -> array<f32, 4> {
    let n = max(user[0], 3.0);
    let rad = user[1];
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let a0 = pi / n;
    let len = 1.0 / cos(a0);
    var out: array<f32, 4>;
    out[0] = n / two_pi;        // coef
    out[1] = a0;                // a0
    out[2] = len;               // len
    out[3] = rad * sin(a0) * len; // d
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hypercrop(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let zero = get_param(xform_id, variation_id, 2u);
    let coef = get_param(xform_id, variation_id, 3u);
    let a0 = get_param(xform_id, variation_id, 4u);
    let len = get_param(xform_id, variation_id, 5u);
    let d = get_param(xform_id, variation_id, 6u);

    // ff_atan2: Metal's fast atan2 is pi/4 at SAME-sign zero pairs and
    // NaN at MIXED-sign ones (measured; IEEE is finite for all four).
    // This variation reaches zero pairs in real renders — the probe
    // showed NaN output at the origin classes on Metal only. See
    // utilities.wgsl.
    var angle = ff_atan2(p.y, p.x);
    angle = floor(angle * coef) / coef + a0;
    let x0 = cos(angle) * len;
    let y0 = sin(angle) * len;

    let dx = p.x - x0;
    let dy = p.y - y0;
    if (sqrt(dx * dx + dy * dy) < d) {
        if (zero > 1.5) {
            return vec2<f32>(x0, y0);
        }
        if (zero > 0.5) {
            return vec2<f32>(0.0, 0.0);
        }
        let rang = ff_atan2(dy, dx);
        return vec2<f32>(x0 + cos(rang) * d, y0 + sin(rang) * d);
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_hypercrop(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let zero = get_param(xform_id, variation_id, 2u);
    let coef = get_param(xform_id, variation_id, 3u);
    let a0 = get_param(xform_id, variation_id, 4u);
    let len = get_param(xform_id, variation_id, 5u);
    let d = get_param(xform_id, variation_id, 6u);

    // ff_atan2: Metal's fast atan2 is pi/4 at SAME-sign zero pairs and
    // NaN at MIXED-sign ones (measured; IEEE is finite for all four).
    // This variation reaches zero pairs in real renders — the probe
    // showed NaN output at the origin classes on Metal only. See
    // utilities.wgsl.
    var angle = ff_atan2(p.y, p.x);
    angle = floor(angle * coef) / coef + a0;
    let x0 = cos(angle) * len;
    let y0 = sin(angle) * len;

    let dx = p.x - x0;
    let dy = p.y - y0;
    if (sqrt(dx * dx + dy * dy) < d) {
        if (zero > 1.5) {
            return vec3<f32>(x0, y0, 0.0);
        }
        if (zero > 0.5) {
            return vec3<f32>(0.0, 0.0, 0.0);
        }
        let rang = ff_atan2(dy, dx);
        return vec3<f32>(x0 + cos(rang) * d, y0 + sin(rang) * d, 0.0);
    }
    return p;
}
"#,
};
