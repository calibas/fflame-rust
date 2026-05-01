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
//! Note: We inline `maurer_rose`'s init values per-iteration rather than
//! stash them in init slots — the per-flame budget is 16 slots, and the
//! cpp's 11 init derivations are all cheap arithmetic. With 11 user
//! params that leaves 5 free slots; we use 0 of them and inline.
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
    definition::{VariationDef, VariationParamDef},
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
pub static MAURER_ROSE: VariationDef = VariationDef {
    name: "maurer_rose",
    display_name: "Maurer Rose",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("kn", "K Numerator", unlimited_float, 2.0, -50.0, 50.0),
        param!("kd", "K Denominator", unlimited_float, 1.0, -50.0, 50.0),
        param!("c", "C Offset", unlimited_float, 0.0, -10.0, 10.0),
        param!("line_count", "Line Count", unlimited_float, 360.0, 1.0, 10000.0),
        param!("line_offset_degrees", "Line Offset (deg)", unlimited_float, 71.0, -3600.0, 3600.0),
        param!("show_lines", "Show Lines", float, 1.0, 0.0, 10.0),
        param!("show_points", "Show Points", float, 0.0, 0.0, 10.0),
        param!("show_curve", "Show Curve", float, 0.05, 0.0, 10.0),
        param!("line_thickness", "Line Thick (×100)", unlimited_float, 0.5, 0.0, 100.0),
        param!("point_thickness", "Point Thick (×100)", unlimited_float, 3.0, 0.0, 100.0),
        param!("curve_thickness", "Curve Thick (×100)", unlimited_float, 1.0, 0.0, 100.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_maurer_rose(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let kn = get_param(xform_id, variation_id, 0u);
    let kd = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let line_count = get_param(xform_id, variation_id, 3u);
    let line_offset_deg = get_param(xform_id, variation_id, 4u);
    let show_lines = get_param(xform_id, variation_id, 5u);
    let show_points = get_param(xform_id, variation_id, 6u);
    let show_curve = get_param(xform_id, variation_id, 7u);
    let line_thick_in = get_param(xform_id, variation_id, 8u);
    let point_thick_in = get_param(xform_id, variation_id, 9u);
    let curve_thick_in = get_param(xform_id, variation_id, 10u);
    let two_pi = 6.28318530717959;

    let safe_kd = select(kd, 1e-30, kd == 0.0);
    let k = kn / safe_kd;
    let step_size = two_pi * (line_offset_deg / 360.0);
    let safe_step = select(step_size, 1e-30, step_size == 0.0);
    let cycles = (line_count * step_size) / two_pi;

    let show_sum = max(show_lines + show_points + show_curve, 1e-30);
    let line_frac = show_lines / show_sum;
    let point_frac = show_points / show_sum;
    let line_thr = line_frac;
    let point_thr = line_frac + point_frac;
    let point_half_thr = line_frac + 0.5 * point_frac;
    let line_thickness = line_thick_in / 100.0;
    let point_thickness = point_thick_in / 100.0;
    let curve_thickness = curve_thick_in / 100.0;

    let tin = atan2(p.y, p.x);
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
    wgsl_3d: Some(r#"
fn variation_maurer_rose(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let kn = get_param(xform_id, variation_id, 0u);
    let kd = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let line_count = get_param(xform_id, variation_id, 3u);
    let line_offset_deg = get_param(xform_id, variation_id, 4u);
    let show_lines = get_param(xform_id, variation_id, 5u);
    let show_points = get_param(xform_id, variation_id, 6u);
    let show_curve = get_param(xform_id, variation_id, 7u);
    let line_thick_in = get_param(xform_id, variation_id, 8u);
    let point_thick_in = get_param(xform_id, variation_id, 9u);
    let curve_thick_in = get_param(xform_id, variation_id, 10u);
    let two_pi = 6.28318530717959;

    let safe_kd = select(kd, 1e-30, kd == 0.0);
    let k = kn / safe_kd;
    let step_size = two_pi * (line_offset_deg / 360.0);
    let safe_step = select(step_size, 1e-30, step_size == 0.0);
    let cycles = (line_count * step_size) / two_pi;

    let show_sum = max(show_lines + show_points + show_curve, 1e-30);
    let line_frac = show_lines / show_sum;
    let point_frac = show_points / show_sum;
    let line_thr = line_frac;
    let point_thr = line_frac + point_frac;
    let point_half_thr = line_frac + 0.5 * point_frac;
    let line_thickness = line_thick_in / 100.0;
    let point_thickness = point_thick_in / 100.0;
    let curve_thickness = curve_thick_in / 100.0;

    let tin = atan2(p.y, p.x);
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
"#),
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
pub static HYPERCROP: VariationDef = VariationDef {
    name: "hypercrop",
    display_name: "Hypercrop",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("n", "N", int, 4.0, 3.0, 50.0),
        param!("rad", "Radius", unlimited_float, 1.0, 0.0, 10.0),
        param!("zero", "Zero", unlimited_float, 0.0, 0.0, 2.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_hypercrop(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let n = max(get_param(xform_id, variation_id, 0u), 3.0);
    let rad = get_param(xform_id, variation_id, 1u);
    let zero = get_param(xform_id, variation_id, 2u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let coef = n / two_pi;
    let a0 = pi / n;
    let len = 1.0 / cos(a0);
    let d = rad * sin(a0) * len;

    var angle = atan2(p.y, p.x);
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
        let rang = atan2(dy, dx);
        return vec2<f32>(x0 + cos(rang) * d, y0 + sin(rang) * d);
    }
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_hypercrop(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let n = max(get_param(xform_id, variation_id, 0u), 3.0);
    let rad = get_param(xform_id, variation_id, 1u);
    let zero = get_param(xform_id, variation_id, 2u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let coef = n / two_pi;
    let a0 = pi / n;
    let len = 1.0 / cos(a0);
    let d = rad * sin(a0) * len;

    var angle = atan2(p.y, p.x);
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
        let rang = atan2(dy, dx);
        return vec3<f32>(x0 + cos(rang) * d, y0 + sin(rang) * d, 0.0);
    }
    return p;
}
"#),
};
