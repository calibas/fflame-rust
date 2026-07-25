//! `minkowski` — flat spacetime as a variation: Lorentz boosts, boost
//! swirl, light-cone inversion, and Rindler unwrapping (original).
//!
//! Minkowski space is NOT "hyperbolic space plus a dimension" — it is
//! the flat ambient spacetime that hyperbolic space lives inside: H³
//! is the unit hyperboloid {⟨x,x⟩ = −1} of R^{3,1}, and our hyperbolic
//! machinery has been doing Minkowski algebra confined to that shell
//! all along. This variation uses the WHOLE spacetime: the interval
//! q = |space|² − time², the light cone q = 0 as a real geometric
//! object, causal character (timelike q < 0 / spacelike q > 0), and
//! the Lorentz group acting on everything.
//!
//! Coordinates: the 3D body treats `(p.xyz, point_w)` as R^{3,1} with
//! w timelike (`Feature::NeedsW`). The 2D body is R^{1,1} — the
//! split-complex plane — with y timelike.
//!
//! Modes, each with no Euclidean analogue:
//! - **Boost**: the Lorentz transform itself — a "rotation" mixing
//!   space with time that preserves the light cone; attractors shear
//!   along hyperbola shells instead of circles.
//! - **Boost Swirl**: `swirl` with rapidity ∝ interval — where swirl
//!   winds circles, this shears every point along its own hyperbola,
//!   antisymmetrically across the cone, piling structure onto it.
//! - **Inversion**: x ↦ x/q, the Minkowski conformal inversion. The
//!   singular set is the entire light cone (not a point) — nearby
//!   points flare outward, carving cone-shaped voids.
//! - **Rindler Wrap**: Minkowski "polar coordinates" (ρ = √|q|,
//!   η = rapidity) — the log/polar unwrap that flattens hyperbola
//!   shells into strips, with the four causal wedges as quadrant
//!   charts (spacelike and timelike wedges unwrap perpendicular to
//!   each other).
//!
//! The Causal color mode drives the palette by the interval, so the
//! light cone becomes a visible color boundary through the fractal.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Minkowski spacetime: Lorentz boost / boost swirl / light-cone
/// inversion / Rindler unwrap on (xyz, w) with w timelike (2D: y
/// timelike), with causal coloring by the spacetime interval.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static MINKOWSKI: VariationDef = VariationDef {
    name: "minkowski",
    aliases: &[],
    display_name: "Minkowski",
    // Advanced2D, not Full3D: the shader builder drops Full3D-category
    // variations from 2D shaders entirely, and the 2D body here is a
    // real geometry (R^{1,1}, the split-complex plane), not a
    // degenerate projection — same placement as lorentz_mobius.
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // NeedsW: (p.xyz, point_w) is the R^{3,1} point in 3D mode.
    // AlwaysZ: genuinely-4D map, z written unconditionally.
    features: &[Feature::NeedsW, Feature::AlwaysZ, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("mode", "Mode", enum, 0, &["Boost", "Boost Swirl", "Inversion", "Rindler Wrap"], "Boost: a fixed Lorentz boost (rapidity along the boost axis) — the spacetime 'rotation' that mixes space with time and preserves the light cone. Boost Swirl: rapidity proportional to each point's own interval q — the Minkowski swirl, shearing every point along its own hyperbola shell (antisymmetric across the cone). Inversion: x/q, singular on the whole light cone — near-cone points flare outward. Rindler Wrap: Minkowski log-polar (rho = sqrt(|q|), eta = rapidity angle) — hyperbola shells flatten into strips; the spacelike and timelike wedges unwrap perpendicular to each other."),
        param!("rapidity", "Rapidity", float, 0.5, -4.0, 4.0, "Boost mode: the boost rapidity (hyperbolic angle — velocity = tanh(rapidity)). Also added as a constant pre-boost in the other modes, so any mode can be viewed from a moving frame."),
        param!("swirl", "Swirl", float, 1.0, -10.0, 10.0, "Boost Swirl mode: rapidity = Swirl x interval q, clamped to ±8. Positive shears future-ward on spacelike points and past-ward on timelike ones; negative reverses."),
        param!("bx", "Boost X", unlimited_float, 1.0, -1.0, 1.0, "Boost axis x component (3D Boost mode; normalized internally, 2D boosts along x)."),
        param!("by", "Boost Y", unlimited_float, 0.0, -1.0, 1.0, "Boost axis y component."),
        param!("bz", "Boost Z", unlimited_float, 0.0, -1.0, 1.0, "Boost axis z component."),
        param!("size", "Size", float, 1.0, 0.05, 4.0, "Output scale."),
        param!("guard", "Cone Guard", float, 0.05, 0.001, 1.0, "Inversion mode: floor on |q| — caps how hard near-cone points flare (the light cone is the singular set of x/q). Smaller = sharper, hotter cone flares."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Causal", "Interval"], "Causal: palette position 0.5 + 0.5*tanh(q * DC Scale) — timelike interiors sit below palette center, spacelike outside above, and the light cone is exactly the center color: a visible causal boundary. Interval: cyclic bands by |q| (hyperbola shells become palette rings)."),
        param!("dc_scale", "DC Scale", float, 1.0, 0.0, 20.0, "Causal: sharpness of the cone boundary. Interval: band frequency."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_minkowski(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let rap = get_param(xform_id, variation_id, 1u);
    let swirl = get_param(xform_id, variation_id, 2u);
    let size = get_param(xform_id, variation_id, 6u);
    let guard = max(get_param(xform_id, variation_id, 7u), 1e-4);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);

    // R^{1,1}: x spacelike, y timelike. Interval and causal color from
    // the INPUT point (the pre-image's causal character).
    var x = p.x;
    var t = p.y;
    let q = x * x - t * t;
    if (dc_mode == 1u) {
        *vc = 0.5 + 0.5 * tanh(q * dc_scale);
    } else if (dc_mode == 2u) {
        *vc = fract(abs(q) * dc_scale);
    }

    // Constant frame boost (the only transform in Boost mode).
    if (rap != 0.0) {
        let ch = cosh(rap); let sh = sinh(rap);
        let nx = x * ch + t * sh;
        t = x * sh + t * ch;
        x = nx;
    }

    if (mode == 1u) {
        // Boost swirl: each point boosts by its own interval.
        let eta = clamp(swirl * q, -8.0, 8.0);
        let ch = cosh(eta); let sh = sinh(eta);
        let nx = x * ch + t * sh;
        t = x * sh + t * ch;
        x = nx;
    } else if (mode == 2u) {
        // Minkowski inversion x/q — singular on the light cone.
        let qc = x * x - t * t;
        let dn = sign(qc) * max(abs(qc), guard * guard);
        x = x / dn;
        t = t / dn;
    } else if (mode == 3u) {
        // Rindler wrap: (rho, eta) log-polar per causal wedge.
        let qc = x * x - t * t;
        if (qc > 0.0) {
            // Spacelike wedges: eta along x', log-shell along y'.
            let rho = sqrt(qc);
            let eta = atanh(clamp(t / x, -0.999999, 0.999999)) * sign(x);
            x = eta;
            t = clamp(log(max(rho, 1e-9)), -20.0, 20.0);
        } else if (qc < 0.0) {
            // Timelike wedges: unwrapped perpendicular.
            let rho = sqrt(-qc);
            let eta = atanh(clamp(x / t, -0.999999, 0.999999)) * sign(t);
            x = clamp(log(max(rho, 1e-9)), -20.0, 20.0);
            t = eta;
        }
    }

    return vec2<f32>(x, t) * size;
}
"#;

const WGSL_3D: &str = r#"
fn variation_minkowski(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let mode = u32(get_param(xform_id, variation_id, 0u));
    let rap = get_param(xform_id, variation_id, 1u);
    let swirl = get_param(xform_id, variation_id, 2u);
    let bx = get_param(xform_id, variation_id, 3u);
    let by = get_param(xform_id, variation_id, 4u);
    let bz = get_param(xform_id, variation_id, 5u);
    let size = get_param(xform_id, variation_id, 6u);
    let guard = max(get_param(xform_id, variation_id, 7u), 1e-4);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);

    // R^{3,1}: p.xyz spacelike, point_w timelike.
    var s = p;
    var t = point_w;
    let q = dot(s, s) - t * t;
    if (dc_mode == 1u) {
        *vc = 0.5 + 0.5 * tanh(q * dc_scale);
    } else if (dc_mode == 2u) {
        *vc = fract(abs(q) * dc_scale);
    }

    // Constant frame boost along the boost axis.
    if (rap != 0.0) {
        var n = vec3<f32>(bx, by, bz);
        let nl = length(n);
        n = select(vec3<f32>(1.0, 0.0, 0.0), n / nl, nl > 1e-6);
        let par = dot(s, n);
        let ch = cosh(rap); let sh = sinh(rap);
        let np = par * ch + t * sh;
        t = par * sh + t * ch;
        s = s + (np - par) * n;
    }

    if (mode == 1u) {
        // Boost swirl: boost along the point's own spatial direction,
        // rapidity = swirl * interval.
        let r = length(s);
        if (r > 1e-9) {
            let n = s / r;
            let eta = clamp(swirl * q, -8.0, 8.0);
            let ch = cosh(eta); let sh = sinh(eta);
            let nr = r * ch + t * sh;
            t = r * sh + t * ch;
            s = n * nr;
        }
    } else if (mode == 2u) {
        // Minkowski inversion x/q — the light cone flares.
        let qc = dot(s, s) - t * t;
        let dn = sign(qc) * max(abs(qc), guard * guard);
        s = s / dn;
        t = t / dn;
    } else if (mode == 3u) {
        // Rindler wrap per causal region: shells (const rho) become
        // spheres of radius ln(rho) in the spacelike region; the
        // timelike region unwraps into rapidity-radius form.
        let r = length(s);
        let qc = r * r - t * t;
        if (r > 1e-9 && qc > 0.0) {
            let n = s / r;
            let rho = sqrt(qc);
            let eta = atanh(clamp(t / r, -0.999999, 0.999999));
            s = n * clamp(log(max(rho, 1e-9)), -20.0, 20.0);
            t = eta;
        } else if (qc < 0.0 && abs(t) > 1e-9) {
            let rho = sqrt(-qc);
            let eta = atanh(clamp(r / abs(t), 0.0, 0.999999));
            var n = vec3<f32>(1.0, 0.0, 0.0);
            if (r > 1e-9) { n = s / r; }
            s = n * eta;
            t = sign(t) * clamp(log(max(rho, 1e-9)), -20.0, 20.0);
        }
    }

    point_w_out = t;
    return s * size;
}
"#;
