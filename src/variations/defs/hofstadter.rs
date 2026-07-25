//! `hofstadter` — Hofstadter-butterfly spectrum attractor (original).
//!
//! Renders the fractal energy spectrum of an electron on a 2D lattice
//! in a magnetic field (the Harper / almost-Mathieu operator). The
//! point's position maps to the butterfly plane: x is energy
//! (`E = 4x/size`) and y is magnetic flux per plaquette
//! (`α = y/size + 1/2`, wrapped — the spectrum is 1-periodic in α, so
//! the butterfly tiles vertically).
//!
//! Each call:
//! 1. snaps α to its **dominant rational** p/q (q ≤ `depth`) by the
//!    Diophantine score `q^hierarchy · |α − p/q|` — simple fractions
//!    win wide basins (1/2 widest, then 1/3, 2/3, …), which is the
//!    Farey hierarchy that gives the butterfly its structure;
//! 2. computes the spectrum discriminant at that flux via the product
//!    of q transfer matrices `T_j = [[E − 2λ·cos(2π p j/q + θ), −1],
//!    [1, 0]]` with Chambers' phase `θ = π/(2q)`: E is in a band iff
//!    `|tr Π T_j| ≤ 2 + 2λ^q` (= 4 for the classic λ = 1 butterfly);
//! 3. if E is outside the spectrum, Newton-walks it to the nearest
//!    band edge (1D, along energy only); points already inside a band
//!    stay put — so the bands fill in as solid segments.
//!
//! The transfer product grows like e^(q·Lyapunov) outside the
//! spectrum and overflows f32 within a few dozen factors, so the
//! discriminant is accumulated in **log space** with per-factor
//! renormalization; the band threshold `ln(2 + 2λ^q)` is evaluated
//! stably for the same reason. `coupling` (λ) generalizes to the
//! almost-Mathieu operator — λ < 1 fattens the bands (metallic), λ > 1
//! thins them toward the critical point's measure-zero butterfly.
//!
//! Direct color: *Distance* (log-distance outside the spectrum — the
//! gap structure) and *Order* — coloring each flux row by its
//! denominator q, which paints the Farey hierarchy: coarse rationals
//! at one palette end, fine structure at the other.
//!
//! 2D-native (the butterfly lives in the (E, α) plane); the 3D body
//! applies in x/y and passes z through.
//!
//! No JWildfire/Apophysis equivalent — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Hofstadter-butterfly spectrum attractor (electron on a 2D lattice in
/// a magnetic field).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static HOFSTADTER: VariationDef = VariationDef {
    name: "hofstadter",
    aliases: &[],
    display_name: "Hofstadter Butterfly",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Spatial scale: the full butterfly (E from −4 to 4, flux from 0 to 1) spans size×size units, centered on the origin. The flux axis wraps, so the butterfly tiles vertically."),
        param!("depth", "Max Denominator", int, 20.0, 2.0, 96.0, "Largest flux denominator q considered. Higher values add finer flux rows (each with q bands) — more hierarchy levels, thinner structure, and proportionally more computation."),
        param!("coupling", "Coupling", float, 1.0, 0.2, 3.0, "Almost-Mathieu coupling λ. 1 is the classic self-dual butterfly; < 1 fattens the bands (metallic regime), > 1 thins them (insulating regime)."),
        param!("hierarchy", "Hierarchy", float, 2.0, 0.5, 4.0, "Farey-basin weight exponent: flux snaps to the rational minimizing q^hierarchy·|α − p/q|. 2 is the natural Diophantine metric; lower values let fine rationals grab more territory (denser rows), higher values favor the simple fractions."),
        param!("snap", "Flux Snap", float, 1.0, 0.0, 1.0, "How hard the y coordinate snaps to its rational flux row. 1 collapses points onto discrete rows (the classic look); lower values leave vertical haze between rows."),
        param!("steps", "Steps", int, 3.0, 1.0, 6.0, "Newton iterations walking the energy to the nearest band edge. Points already inside a band never move."),
        param!("strength", "Strength", float, 0.9, 0.0, 1.0, "Blend between the untouched input point (0) and the fully projected point (1)."),
        param!("jitter", "Jitter", float, 0.0, 0.0, 0.2, "Isotropic random offset added after projection. 0 keeps the band rows razor thin."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Distance", "Order"], "Direct-color source, applied through the transform's Direct Color slider. Distance: palette 1 inside the spectrum, fading with log-distance into the gaps. Order: colors each flux row by its denominator q — the Farey hierarchy painted across the palette (coarse rationals at one end, fine structure at the other)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 4.0, "Contrast for the direct-color modes: Distance falloff sharpness, Order spread."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Helpers are duplicated into the 2D and 3D strings (only one body is
// compiled per flame). Numerics:
//   * The transfer product is renormalized whenever its largest entry
//     exceeds 1e10, accumulating the log scale — |tr| spans thousands
//     of decades across the plane, but its log is smooth and cheap.
//   * The 1D Newton on g(E) = ln|tr| − ln(threshold) uses a damped
//     step with a per-q clamp (band spacing shrinks like 1/q).

const WGSL_2D: &str = r#"
// Dominant rational p/q for alpha in [0,1): minimizes q^w * |alpha - p/q|
// over q <= qmax. Returns vec2(p, q) as floats.
fn hofstadter_best_rational(alpha: f32, qmax: i32, w: f32) -> vec2<f32> {
    var best_p = 0.0;
    var best_q = 1.0;
    var best_score = 1e30;
    for (var q = 1; q <= qmax; q = q + 1) {
        let fq = f32(q);
        let p = floor(alpha * fq + 0.5);
        let err = abs(alpha - p / fq);
        let score = pow(fq, w) * err;
        if (score < best_score) {
            best_score = score;
            best_p = p;
            best_q = fq;
        }
    }
    return vec2<f32>(best_p, best_q);
}

// Discriminant of the q-step transfer product at Chambers' phase:
// vec3(ln|tr|, d(ln|tr|)/dE, sign(tr)). The energy derivative is
// accumulated analytically alongside the product (product rule:
// D_k = T' M_{k-1} + T_k D_{k-1}, with T' = [[1,0],[0,0]]) — one pass
// instead of finite differences, exact even where high-q bands are
// thinner than any FD step. Renormalization (shared scale for M and D
// so their ratio stays intact) guards f32 overflow; the scale cancels
// in tr'/tr and is added back onto ln|tr|.
fn hofstadter_disc(e: f32, p: f32, q: f32, lambda: f32) -> vec3<f32> {
    let two_pi = 6.28318530718;
    let theta = 3.14159265359 / (2.0 * q);
    var m00 = 1.0; var m01 = 0.0;
    var m10 = 0.0; var m11 = 1.0;
    var d00 = 0.0; var d01 = 0.0;
    var d10 = 0.0; var d11 = 0.0;
    var scale = 0.0;
    let iq = i32(q);
    for (var j = 0; j < iq; j = j + 1) {
        let v = e - 2.0 * lambda * cos(two_pi * p * f32(j) / q + theta);
        let n00 = v * m00 - m10;
        let n01 = v * m01 - m11;
        let e00 = m00 + v * d00 - d10;
        let e01 = m01 + v * d01 - d11;
        d10 = d00; d11 = d01;
        d00 = e00; d01 = e01;
        m10 = m00; m11 = m01;
        m00 = n00; m01 = n01;
        let mag = max(
            max(max(abs(m00), abs(m01)), max(abs(m10), abs(m11))),
            max(max(abs(d00), abs(d01)), max(abs(d10), abs(d11))),
        );
        if (mag > 1e10) {
            let inv = 1.0 / mag;
            m00 = m00 * inv; m01 = m01 * inv;
            m10 = m10 * inv; m11 = m11 * inv;
            d00 = d00 * inv; d01 = d01 * inv;
            d10 = d10 * inv; d11 = d11 * inv;
            scale = scale + log(mag);
        }
    }
    let tr = m00 + m11;
    let trp = d00 + d11;
    let sgn = select(-1.0, 1.0, tr >= 0.0);
    let dln = trp / (sgn * max(abs(tr), 1e-12));
    return vec3<f32>(log(abs(tr) + 1e-20) + scale, dln, sgn);
}

// Band-edge threshold ln(2 + 2*lambda^q), overflow-stable.
fn hofstadter_ln_threshold(q: f32, lambda: f32) -> f32 {
    let t = q * log(lambda);
    if (t > 20.0) {
        return 0.69314718 + t;
    }
    return log(2.0 + 2.0 * exp(t));
}

fn variation_hofstadter(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let size = max(get_param(xform_id, variation_id, 0u), 1e-6);
    let depth = i32(get_param(xform_id, variation_id, 1u));
    let lambda = max(get_param(xform_id, variation_id, 2u), 1e-3);
    let hierarchy = get_param(xform_id, variation_id, 3u);
    let snap = get_param(xform_id, variation_id, 4u);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let strength = get_param(xform_id, variation_id, 6u);
    let jitter = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);

    // Map to the butterfly plane. Flux is 1-periodic: keep the tile
    // index so the output lands in the same vertical tile.
    var e = 4.0 * p.x / size;
    let u = p.y / size + 0.5;
    let tile = floor(u);
    let alpha = u - tile;

    let pq = hofstadter_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.x;
    let fq = pq.y;
    let ln_thr = hofstadter_ln_threshold(fq, lambda);

    // 1D Newton along energy onto the band edge; inside a band
    // (g <= 0) the point stays where it is. Value + exact derivative
    // come from one transfer-product pass.
    let max_step = 4.0 / fq;
    var g0 = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        let d = hofstadter_disc(e, fp, fq, lambda);
        let g = d.x - ln_thr;
        if (i == 0) { g0 = g; }
        if (g <= 0.0) { break; }
        var step = g * d.y / (d.y * d.y + 1e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }

    if (dc_mode == 1u) {
        // Distance: 1 inside the spectrum, log-decay into the gaps.
        *vc = exp(-dc_scale * max(g0, 0.0));
    } else if (dc_mode == 2u) {
        // Order: the flux row's denominator across the palette.
        *vc = pow((fq - 1.0) / max(f32(depth) - 1.0, 1.0), 1.0 / max(dc_scale, 0.1));
    }

    let x_row = e * size / 4.0;
    let y_row = (tile + fp / fq - 0.5) * size;
    var out = vec2<f32>(
        mix(p.x, x_row, strength),
        mix(p.y, y_row, snap * strength),
    );
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
// Dominant rational p/q for alpha in [0,1): minimizes q^w * |alpha - p/q|
// over q <= qmax. Returns vec2(p, q) as floats.
fn hofstadter_best_rational(alpha: f32, qmax: i32, w: f32) -> vec2<f32> {
    var best_p = 0.0;
    var best_q = 1.0;
    var best_score = 1e30;
    for (var q = 1; q <= qmax; q = q + 1) {
        let fq = f32(q);
        let p = floor(alpha * fq + 0.5);
        let err = abs(alpha - p / fq);
        let score = pow(fq, w) * err;
        if (score < best_score) {
            best_score = score;
            best_p = p;
            best_q = fq;
        }
    }
    return vec2<f32>(best_p, best_q);
}

// Discriminant of the q-step transfer product at Chambers' phase:
// vec3(ln|tr|, d(ln|tr|)/dE, sign(tr)). The energy derivative is
// accumulated analytically alongside the product (product rule:
// D_k = T' M_{k-1} + T_k D_{k-1}, with T' = [[1,0],[0,0]]) — one pass
// instead of finite differences, exact even where high-q bands are
// thinner than any FD step. Renormalization (shared scale for M and D
// so their ratio stays intact) guards f32 overflow; the scale cancels
// in tr'/tr and is added back onto ln|tr|.
fn hofstadter_disc(e: f32, p: f32, q: f32, lambda: f32) -> vec3<f32> {
    let two_pi = 6.28318530718;
    let theta = 3.14159265359 / (2.0 * q);
    var m00 = 1.0; var m01 = 0.0;
    var m10 = 0.0; var m11 = 1.0;
    var d00 = 0.0; var d01 = 0.0;
    var d10 = 0.0; var d11 = 0.0;
    var scale = 0.0;
    let iq = i32(q);
    for (var j = 0; j < iq; j = j + 1) {
        let v = e - 2.0 * lambda * cos(two_pi * p * f32(j) / q + theta);
        let n00 = v * m00 - m10;
        let n01 = v * m01 - m11;
        let e00 = m00 + v * d00 - d10;
        let e01 = m01 + v * d01 - d11;
        d10 = d00; d11 = d01;
        d00 = e00; d01 = e01;
        m10 = m00; m11 = m01;
        m00 = n00; m01 = n01;
        let mag = max(
            max(max(abs(m00), abs(m01)), max(abs(m10), abs(m11))),
            max(max(abs(d00), abs(d01)), max(abs(d10), abs(d11))),
        );
        if (mag > 1e10) {
            let inv = 1.0 / mag;
            m00 = m00 * inv; m01 = m01 * inv;
            m10 = m10 * inv; m11 = m11 * inv;
            d00 = d00 * inv; d01 = d01 * inv;
            d10 = d10 * inv; d11 = d11 * inv;
            scale = scale + log(mag);
        }
    }
    let tr = m00 + m11;
    let trp = d00 + d11;
    let sgn = select(-1.0, 1.0, tr >= 0.0);
    let dln = trp / (sgn * max(abs(tr), 1e-12));
    return vec3<f32>(log(abs(tr) + 1e-20) + scale, dln, sgn);
}

// Band-edge threshold ln(2 + 2*lambda^q), overflow-stable.
fn hofstadter_ln_threshold(q: f32, lambda: f32) -> f32 {
    let t = q * log(lambda);
    if (t > 20.0) {
        return 0.69314718 + t;
    }
    return log(2.0 + 2.0 * exp(t));
}

fn variation_hofstadter(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let size = max(get_param(xform_id, variation_id, 0u), 1e-6);
    let depth = i32(get_param(xform_id, variation_id, 1u));
    let lambda = max(get_param(xform_id, variation_id, 2u), 1e-3);
    let hierarchy = get_param(xform_id, variation_id, 3u);
    let snap = get_param(xform_id, variation_id, 4u);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let strength = get_param(xform_id, variation_id, 6u);
    let jitter = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);

    var e = 4.0 * p.x / size;
    let u = p.y / size + 0.5;
    let tile = floor(u);
    let alpha = u - tile;

    let pq = hofstadter_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.x;
    let fq = pq.y;
    let ln_thr = hofstadter_ln_threshold(fq, lambda);

    let max_step = 4.0 / fq;
    var g0 = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        let d = hofstadter_disc(e, fp, fq, lambda);
        let g = d.x - ln_thr;
        if (i == 0) { g0 = g; }
        if (g <= 0.0) { break; }
        var step = g * d.y / (d.y * d.y + 1e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }

    if (dc_mode == 1u) {
        *vc = exp(-dc_scale * max(g0, 0.0));
    } else if (dc_mode == 2u) {
        *vc = pow((fq - 1.0) / max(f32(depth) - 1.0, 1.0), 1.0 / max(dc_scale, 0.1));
    }

    let x_row = e * size / 4.0;
    let y_row = (tile + fp / fq - 0.5) * size;
    var out = vec2<f32>(
        mix(p.x, x_row, strength),
        mix(p.y, y_row, snap * strength),
    );
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return vec3<f32>(out, p.z);
}
"#;
