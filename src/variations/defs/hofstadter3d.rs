//! `hofstadter3d` — the Hofstadter butterfly's full 3D spectral body
//! (original).
//!
//! The classic butterfly ([`hofstadter`](super::hofstadter)) is the
//! union over a hidden third parameter: the Bloch phase θ of the
//! almost-Mathieu operator. Chambers' relation makes the θ-dependence
//! exact —
//!
//! ```text
//! tr M(E, θ) = Δ₀(E) + 2λ^q·cos(qθ)
//! ```
//!
//! — so at fixed phase the spectrum condition is
//! `|Δ₀(E) + 2λ^q·cos(φ)| ≤ 2` (φ = qθ), giving narrower bands inside
//! the union envelope that sweep sinusoidally as φ varies. This
//! variation maps **z ↦ φ** (one full period per `size`, so the body
//! tiles in z) and Newton-walks energy onto the phase-resolved band
//! edge: each flux row becomes a rippling ribbon, and the stack of
//! ribbons is the genuine 3D spectral body whose z-union is the
//! classic butterfly.
//!
//! `axis` selects an alternative third dimension: **Coupling** sweeps
//! λ across z instead (`λ_eff = λ·10^(span·z/size)`), rendering the
//! metal → critical → insulator transition as depth — bands fatten on
//! one side of the slab and evaporate on the other.
//!
//! Everything is evaluated in log space (the transfer product and the
//! Chambers combination both overflow f32 otherwise), and the energy
//! derivative is accumulated analytically in the same transfer-matrix
//! pass — see `hofstadter` for both techniques.
//!
//! In 2D render mode the variation shows the fixed-phase butterfly at
//! `φ = 2π·slice_z/size` (thickness > 0 superimposes nearby phases —
//! and since the full θ-union is the classic butterfly, cranking the
//! thickness morphs the slice toward `hofstadter`'s picture). In 3D,
//! `slice_thickness > 0` confines the attractor to a z-slab.
//!
//! Direct color: *Distance*, *Order* (flux denominator — the Farey
//! hierarchy), and *Depth* (output z), as in the family.
//!
//! No JWildfire/Apophysis equivalent — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// The Hofstadter butterfly's full 3D spectral body.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static HOFSTADTER3D: VariationDef = VariationDef {
    name: "hofstadter3d",
    aliases: &[],
    display_name: "Hofstadter 3D",
    // Advanced2D so the 2D fixed-phase-slice body isn't filtered out
    // of 2D shaders; AlwaysZ keeps z under preserve_z = false (the 3D
    // structure lives along z).
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Spatial scale: the butterfly (E −4..4, flux 0..1) spans size×size in x/y, and one full phase (or coupling) period spans size in z. Flux and phase both wrap, so the body tiles."),
        param!("depth", "Max Denominator", int, 20.0, 2.0, 96.0, "Largest flux denominator q considered — hierarchy depth, band count per row, and compute cost."),
        param!("coupling", "Coupling", float, 1.0, 0.2, 3.0, "Almost-Mathieu coupling λ (at z = 0 in Coupling axis mode). 1 is the classic self-dual butterfly."),
        param!("hierarchy", "Hierarchy", float, 2.0, 0.5, 4.0, "Farey-basin weight exponent for the flux snap (see hofstadter)."),
        param!("snap", "Flux Snap", float, 1.0, 0.0, 1.0, "How hard y snaps to its rational flux row."),
        param!("axis", "Z Axis", enum, 0, &["Phase", "Coupling"], "Meaning of the z dimension. Phase: the Bloch phase — bands sweep sinusoidally with z and the z-union is the classic butterfly (the honest 3D spectral body). Coupling: λ sweeps across z — the metal-to-insulator transition rendered as depth."),
        param!("coupling_span", "Coupling Span", float, 1.0, 0.1, 3.0, "Coupling axis mode only: how many decades λ sweeps per size of z (λ_eff = λ·10^(span·z/size))."),
        param!("slice_z", "Slice Z", float, 0.25, -2.0, 2.0, "Slice position. 2D render mode: the phase (or coupling) slice, φ = 2π·slice_z/size. 3D with Slice Thickness > 0: center of the z-slab."),
        param!("slice_thickness", "Slice Thickness", float, 0.0, 0.0, 2.0, "Slab thickness around Slice Z. 2D: > 0 superimposes nearby phase slices (approaches the classic θ-union butterfly). 3D: > 0 confines the attractor to the slab; 0 = full volume."),
        param!("steps", "Steps", int, 3.0, 1.0, 6.0, "Newton iterations walking the energy to the nearest band edge. Points already inside a band never move."),
        param!("strength", "Strength", float, 0.9, 0.0, 1.0, "Blend between the untouched input point (0) and the fully projected point (1)."),
        param!("jitter", "Jitter", float, 0.0, 0.0, 0.2, "Isotropic random offset added after projection."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Distance", "Order", "Depth"], "Direct-color source. Distance: palette 1 inside the spectrum, fading into the gaps. Order: flux-row denominator q — the Farey hierarchy across the palette. Depth: colors by the output z (phase / coupling position)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 4.0, "Contrast for the direct-color modes."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Helpers are duplicated per string and prefixed hofstadter3d_ so this
// variation can coexist with `hofstadter` in one flame. The Chambers
// combination Δ₀ + 2λ^q·cos(φ) is evaluated in log space: both terms
// can individually exceed f32 range, so they're combined at a shared
// max-log scale (the standard log-sum trick, sign-aware).

const WGSL_3D: &str = r#"
// Dominant rational p/q for alpha in [0,1): minimizes q^w * |alpha - p/q|.
fn hofstadter3d_best_rational(alpha: f32, qmax: i32, w: f32) -> vec2<f32> {
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

// Transfer-product discriminant at Chambers' phase with analytic
// energy derivative: vec3(ln|tr|, d(ln|tr|)/dE, sign(tr)).
fn hofstadter3d_disc(e: f32, p: f32, q: f32, lambda: f32) -> vec3<f32> {
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
fn hofstadter3d_ln_threshold(q: f32, lambda: f32) -> f32 {
    let t = q * log(lambda);
    if (t > 20.0) {
        return 0.69314718 + t;
    }
    return log(2.0 + 2.0 * exp(t));
}

// Phase-resolved band function at fixed phi:
// g = ln|Delta0 + 2 lambda^q cos(phi)| - ln 2, and dg/dE, combined at
// a shared max-log scale so either term may exceed f32 range alone.
// Returns vec2(g, dg/dE).
fn hofstadter3d_phase_band(e: f32, p: f32, q: f32, lambda: f32, phi: f32) -> vec2<f32> {
    let d = hofstadter3d_disc(e, p, q, lambda);
    let cphi = cos(phi);
    let lc = 0.69314718 + q * log(lambda) + log(abs(cphi) + 1e-20);
    let sc = select(-1.0, 1.0, cphi >= 0.0);
    let m = max(d.x, lc);
    let f_s = d.z * exp(d.x - m) + sc * exp(lc - m);
    let g = log(abs(f_s) + 1e-20) + m - 0.69314718;
    // dF/dE = Delta0' ; dg/dE = Delta0'/F, in scaled space.
    let dg = d.y * d.z * exp(d.x - m) / (f_s + select(1e-12, -1e-12, f_s < 0.0));
    return vec2<f32>(g, dg);
}

fn variation_hofstadter3d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let size = max(get_param(xform_id, variation_id, 0u), 1e-6);
    let depth = i32(get_param(xform_id, variation_id, 1u));
    let lambda = max(get_param(xform_id, variation_id, 2u), 1e-3);
    let hierarchy = get_param(xform_id, variation_id, 3u);
    let snap = get_param(xform_id, variation_id, 4u);
    let axis = u32(get_param(xform_id, variation_id, 5u));
    let span = get_param(xform_id, variation_id, 6u);
    let slice_z = get_param(xform_id, variation_id, 7u);
    let slice_thickness = get_param(xform_id, variation_id, 8u);
    let steps = i32(get_param(xform_id, variation_id, 9u));
    let strength = get_param(xform_id, variation_id, 10u);
    let jitter = get_param(xform_id, variation_id, 11u);
    let dc_mode = u32(get_param(xform_id, variation_id, 12u));
    let dc_scale = get_param(xform_id, variation_id, 13u);

    var e = 4.0 * p.x / size;
    let u = p.y / size + 0.5;
    let tile = floor(u);
    let alpha = u - tile;

    var zz = p.z;
    let half_t = 0.5 * slice_thickness;
    if (half_t > 0.0) {
        zz = clamp(zz, slice_z - half_t, slice_z + half_t);
    }

    let pq = hofstadter3d_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.x;
    let fq = pq.y;

    let phi = 6.28318530718 * zz / size;
    let lambda_eff = lambda * pow(10.0, span * zz / size);
    let max_step = 4.0 / fq;
    var g0 = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        var g = 0.0;
        var dg = 0.0;
        if (axis == 0u) {
            let b = hofstadter3d_phase_band(e, fp, fq, lambda, phi);
            g = b.x;
            dg = b.y;
        } else {
            let d = hofstadter3d_disc(e, fp, fq, lambda_eff);
            g = d.x - hofstadter3d_ln_threshold(fq, lambda_eff);
            dg = d.y;
        }
        if (i == 0) { g0 = g; }
        if (g <= 0.0) { break; }
        var step = g * dg / (dg * dg + 1e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }

    let x_row = e * size / 4.0;
    let y_row = (tile + fp / fq - 0.5) * size;
    var out = vec3<f32>(
        mix(p.x, x_row, strength),
        mix(p.y, y_row, snap * strength),
        zz,
    );

    if (dc_mode == 1u) {
        *vc = exp(-dc_scale * max(g0, 0.0));
    } else if (dc_mode == 2u) {
        *vc = pow((fq - 1.0) / max(f32(depth) - 1.0, 1.0), 1.0 / max(dc_scale, 0.1));
    } else if (dc_mode == 3u) {
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * out.z / size);
    }

    if (jitter > 0.0) {
        out = out + vec3<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;

const WGSL_2D: &str = r#"
// Dominant rational p/q for alpha in [0,1): minimizes q^w * |alpha - p/q|.
fn hofstadter3d_best_rational(alpha: f32, qmax: i32, w: f32) -> vec2<f32> {
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

// Transfer-product discriminant at Chambers' phase with analytic
// energy derivative: vec3(ln|tr|, d(ln|tr|)/dE, sign(tr)).
fn hofstadter3d_disc(e: f32, p: f32, q: f32, lambda: f32) -> vec3<f32> {
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
fn hofstadter3d_ln_threshold(q: f32, lambda: f32) -> f32 {
    let t = q * log(lambda);
    if (t > 20.0) {
        return 0.69314718 + t;
    }
    return log(2.0 + 2.0 * exp(t));
}

// Phase-resolved band function at fixed phi:
// g = ln|Delta0 + 2 lambda^q cos(phi)| - ln 2, and dg/dE, combined at
// a shared max-log scale so either term may exceed f32 range alone.
// Returns vec2(g, dg/dE).
fn hofstadter3d_phase_band(e: f32, p: f32, q: f32, lambda: f32, phi: f32) -> vec2<f32> {
    let d = hofstadter3d_disc(e, p, q, lambda);
    let cphi = cos(phi);
    let lc = 0.69314718 + q * log(lambda) + log(abs(cphi) + 1e-20);
    let sc = select(-1.0, 1.0, cphi >= 0.0);
    let m = max(d.x, lc);
    let f_s = d.z * exp(d.x - m) + sc * exp(lc - m);
    let g = log(abs(f_s) + 1e-20) + m - 0.69314718;
    // dF/dE = Delta0' ; dg/dE = Delta0'/F, in scaled space.
    let dg = d.y * d.z * exp(d.x - m) / (f_s + select(1e-12, -1e-12, f_s < 0.0));
    return vec2<f32>(g, dg);
}

fn variation_hofstadter3d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let size = max(get_param(xform_id, variation_id, 0u), 1e-6);
    let depth = i32(get_param(xform_id, variation_id, 1u));
    let lambda = max(get_param(xform_id, variation_id, 2u), 1e-3);
    let hierarchy = get_param(xform_id, variation_id, 3u);
    let snap = get_param(xform_id, variation_id, 4u);
    let axis = u32(get_param(xform_id, variation_id, 5u));
    let span = get_param(xform_id, variation_id, 6u);
    let slice_z = get_param(xform_id, variation_id, 7u);
    let slice_thickness = get_param(xform_id, variation_id, 8u);
    let steps = i32(get_param(xform_id, variation_id, 9u));
    let strength = get_param(xform_id, variation_id, 10u);
    let jitter = get_param(xform_id, variation_id, 11u);
    let dc_mode = u32(get_param(xform_id, variation_id, 12u));
    let dc_scale = get_param(xform_id, variation_id, 13u);

    var e = 4.0 * p.x / size;
    let u = p.y / size + 0.5;
    let tile = floor(u);
    let alpha = u - tile;

    // Phase (or coupling) slice for this point, with the family's
    // thick-slice sampling.
    let z_eval = slice_z + (rng_nextf(rng) - 0.5) * slice_thickness;

    let pq = hofstadter3d_best_rational(alpha, max(depth, 1), hierarchy);
    let fp = pq.x;
    let fq = pq.y;

    let phi = 6.28318530718 * z_eval / size;
    let lambda_eff = lambda * pow(10.0, span * z_eval / size);
    let max_step = 4.0 / fq;
    var g0 = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        var g = 0.0;
        var dg = 0.0;
        if (axis == 0u) {
            let b = hofstadter3d_phase_band(e, fp, fq, lambda, phi);
            g = b.x;
            dg = b.y;
        } else {
            let d = hofstadter3d_disc(e, fp, fq, lambda_eff);
            g = d.x - hofstadter3d_ln_threshold(fq, lambda_eff);
            dg = d.y;
        }
        if (i == 0) { g0 = g; }
        if (g <= 0.0) { break; }
        var step = g * dg / (dg * dg + 1e-6);
        step = clamp(step, -max_step, max_step);
        e = e - step;
    }

    if (dc_mode == 1u) {
        *vc = exp(-dc_scale * max(g0, 0.0));
    } else if (dc_mode == 2u) {
        *vc = pow((fq - 1.0) / max(f32(depth) - 1.0, 1.0), 1.0 / max(dc_scale, 0.1));
    } else if (dc_mode == 3u) {
        *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * z_eval / size);
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
