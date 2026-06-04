//! Apophysis miscellany — eight more ports
//!
//!   - `xerf`            (zephyrtronium / dark-beam) — 3D piecewise
//!                                                      erf/inverse;
//!                                                      needs erf() helper
//!   - `inverted_julia`  (Whittaker Courtney 2018) — 9 user params
//!                                                    (Java-recovered;
//!                                                    cpp APO_VARIABLES
//!                                                    only declared 2);
//!                                                    RNG; clean
//!   - `idisc`           (Faber)                   — 0 user params,
//!                                                    `_v = w/π` init;
//!                                                    needs_transform
//!                                                    divide-out
//!   - `conic`           (cyberxaos 4/2007)        — 2 user params,
//!                                                    RNG; needs_transform
//!                                                    divide-out
//!   - `power`                                     — 0 user params;
//!                                                    cpp swap quirks
//!                                                    preserved (cosA
//!                                                    instead of sinA in
//!                                                    exponent + xy swap)
//!   - `roundspher`      (Raykoid666)              — 0 user params;
//!                                                    body has w · w/d · …,
//!                                                    needs_transform
//!                                                    divide-out
//!   - `checks`          (Keeps / Xyrus02)       — 4 user params + 1
//!                                                    init slot; RNG;
//!                                                    clean
//!   - `cone`            (Brad Stefanov)           — 9 user params
//!                                                    (Java-recovered;
//!                                                    cpp PluginVarCalc
//!                                                    is empty unported
//!                                                    stub); 3D; RNG;
//!                                                    clean
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! Faithfulness:
//!   - `inverted_julia` cpp APO_VARIABLES only lists `power` and
//!     `center`, leaving the other seven params as struct fields
//!     initialized to constants. Java `setParameter` exposes all 9.
//!     Recovered from Java.
//!   - `cone` cpp PluginVarCalc is empty. Recovered from Java.
//!   - `power` cpp deviates from Java: cpp uses `FTx / r` (cosA)
//!     for the pow exponent and outputs `(r·y/r, r·x/r)` (xy swap).
//!     Java uses sinA for exponent and outputs `(r·cosA, r·sinA)`
//!     (no swap). Following cpp.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// xerf: 3D piecewise erf / 1/r² (zephyrtronium / dark-beam)
//   r2 = x² + y² + z²
//   per-component: |c| ≥ 2: c/r2  ;  else: erf(c)
//   out *= w
// Clean factor through outer.
// erf approximation: Abramowitz & Stegun 7.1.26 (max error ~1.5e-7).
// =============================================================================
/// 3D piecewise erf / 1/r² — per-axis, if `|coord| ≥ 2` the output is
/// `coord / r²` (spherical inversion); otherwise it's `erf(coord)`.
/// Combines sigmoid saturation near the origin with spherical inversion far
/// away.
///
/// # Authors
/// - zephyrtronium
/// - DarkBeam
pub static XERF: VariationDef = VariationDef {
    name: "xerf",
    aliases: &[],
    display_name: "X Erf",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn xerf_erf(x: f32) -> f32 {
    // Abramowitz & Stegun 7.1.26 (max error ~1.5e-7)
    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let s = sign(x);
    let ax = abs(x);
    let t = 1.0 / (1.0 + p * ax);
    let poly = ((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t;
    return s * (1.0 - poly * exp(-ax * ax));
}

fn variation_xerf(p: vec2<f32>) -> vec2<f32> {
    let r2 = max(p.x * p.x + p.y * p.y, 1e-30);
    let fx = select(xerf_erf(p.x), p.x / r2, abs(p.x) >= 2.0);
    let fy = select(xerf_erf(p.y), p.y / r2, abs(p.y) >= 2.0);
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn xerf_erf(x: f32) -> f32 {
    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let s = sign(x);
    let ax = abs(x);
    let t = 1.0 / (1.0 + p * ax);
    let poly = ((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t;
    return s * (1.0 - poly * exp(-ax * ax));
}

fn variation_xerf(p: vec3<f32>) -> vec3<f32> {
    let r2 = max(p.x * p.x + p.y * p.y + p.z * p.z, 1e-30);
    let fx = select(xerf_erf(p.x), p.x / r2, abs(p.x) >= 2.0);
    let fy = select(xerf_erf(p.y), p.y / r2, abs(p.y) >= 2.0);
    let fz = select(xerf_erf(p.z), p.z / r2, abs(p.z) >= 2.0);
    return vec3<f32>(fx, fy, fz);
}
"#,
};

// =============================================================================
// inverted_julia: Julia variant with adjustable inward center
//                 (Whittaker Courtney 2018)
//   9 user params: power, y2_mult, a2x_mult, a2y_mult, a2y_add,
//                  cos_mult, y_mult, center, x2y2_add
//   z = (x² + y²·y2_mult)^power + x2y2_add
//   q = atan2(x·a2x_mult, y·a2y_mult + a2y_add) · 0.5 + π·floor(2·rand)
//   out = (cos(z·cos_mult)·sin(q)/z/center,
//          cos(z·cos_mult)·cos(q)/z/center · y_mult)
// Clean factor through outer; needs_rng for the π·floor(2·rand) term.
// (cpp APO_VARIABLES only declares power, center; recovered from
//  Java setParameter.)
// =============================================================================
/// Inverted Julia warp — 9-parameter Julia variant with adjustable inward
/// center. Computes `z = (x² + y²·y2_mult)^power + x2y2_add`, picks a
/// random hemisphere via `q = atan2(...)/2 + π·floor(2·rand)`, then emits
/// `cos(z·cos_mult) · (sin q, cos q · y_mult) / z / center`.
///
/// # Authors
/// - Whittaker Courtney
pub static INVERTED_JULIA: VariationDef = VariationDef {
    name: "inverted_julia",
    aliases: &[],
    display_name: "Inverted Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", unlimited_float, 0.25, -10.0, 10.0, "Exponent on the squared-radius base term `(x² + y²·y2_mult)`."),
        param!("y2_mult", "Y² Mult", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on y² in the base term."),
        param!("a2x_mult", "A2x Mult", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on x in the angle term."),
        param!("a2y_mult", "A2y Mult", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on y in the angle term."),
        param!("a2y_add", "A2y Add", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on y in the angle term."),
        param!("cos_mult", "Cos Mult", unlimited_float, 0.0, -10.0, 10.0, "Frequency multiplier on z in the cosine modulator."),
        param!("y_mult", "Y Mult", unlimited_float, 1.0, -10.0, 10.0, "Y output scaling."),
        param!("center", "Center", unlimited_float, 3.14, -10.0, 10.0, "Divisor on the output radius. Higher = tighter pattern."),
        param!("x2y2_add", "X²Y² Add", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on the base term (added after the pow)."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_inverted_julia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let y2_mult = get_param(xform_id, variation_id, 1u);
    let a2x_mult = get_param(xform_id, variation_id, 2u);
    let a2y_mult = get_param(xform_id, variation_id, 3u);
    let a2y_add = get_param(xform_id, variation_id, 4u);
    let cos_mult = get_param(xform_id, variation_id, 5u);
    let y_mult = get_param(xform_id, variation_id, 6u);
    let center = get_param(xform_id, variation_id, 7u);
    let x2y2_add = get_param(xform_id, variation_id, 8u);
    let pi = 3.14159265358979;

    let xs = p.x * p.x;
    let ys = p.y * p.y;
    let base = xs + ys * y2_mult;
    let z = pow(max(base, 1e-30), power) + x2y2_add;
    let safe_z = select(z, 1e-30, abs(z) < 1e-30);
    let safe_center = select(center, 1e-30, abs(center) < 1e-30);

    let q = atan2(p.x * a2x_mult, p.y * a2y_mult + a2y_add) * 0.5 + pi * floor(2.0 * rng_nextf(rng));

    let cz = cos(z * cos_mult);
    let fx = cz * (sin(q) / safe_z / safe_center);
    let fy = cz * (cos(q) / safe_z / safe_center) * y_mult;
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_inverted_julia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let y2_mult = get_param(xform_id, variation_id, 1u);
    let a2x_mult = get_param(xform_id, variation_id, 2u);
    let a2y_mult = get_param(xform_id, variation_id, 3u);
    let a2y_add = get_param(xform_id, variation_id, 4u);
    let cos_mult = get_param(xform_id, variation_id, 5u);
    let y_mult = get_param(xform_id, variation_id, 6u);
    let center = get_param(xform_id, variation_id, 7u);
    let x2y2_add = get_param(xform_id, variation_id, 8u);
    let pi = 3.14159265358979;

    let xs = p.x * p.x;
    let ys = p.y * p.y;
    let base = xs + ys * y2_mult;
    let z = pow(max(base, 1e-30), power) + x2y2_add;
    let safe_z = select(z, 1e-30, abs(z) < 1e-30);
    let safe_center = select(center, 1e-30, abs(center) < 1e-30);

    let q = atan2(p.x * a2x_mult, p.y * a2y_mult + a2y_add) * 0.5 + pi * floor(2.0 * rng_nextf(rng));

    let cz = cos(z * cos_mult);
    let fx = cz * (sin(q) / safe_z / safe_center);
    let fy = cz * (cos(q) / safe_z / safe_center) * y_mult;
    return vec3<f32>(fx, fy, p.z);
}
"#,
};

// =============================================================================
// idisc: angle-radius disc inversion (Faber)
//   _v = w / π  (init)
//   a = π / (sqrt(x² + y²) + 1)
//   r = atan2(y, x) · _v
//   out = (r · cos a, r · sin a)
// Body has w in `r` → needs_transform divide-out.
// =============================================================================
/// Angle-radius disc inversion — emits `(r·cos a, r·sin a)` where `a = π /
/// (sqrt(x²+y²) + 1)` and `r = atan2(y, x) · w/π`. Swaps the roles of
/// radius and angle in the output relative to a standard polar-to-cartesian
/// mapping.
///
/// # Authors
/// - Michael Faber
pub static IDISC: VariationDef = VariationDef {
    name: "idisc",
    aliases: &[],
    display_name: "I-Disc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_idisc(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let inv_pi = 0.31830988618379;
    let v = w * inv_pi;

    let a = pi / (sqrt(p.x * p.x + p.y * p.y) + 1.0);
    let r = atan2(p.y, p.x) * v;
    return vec2<f32>(r * cos(a) * inv_w, r * sin(a) * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_idisc(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let inv_pi = 0.31830988618379;
    let v = w * inv_pi;

    let a = pi / (sqrt(p.x * p.x + p.y * p.y) + 1.0);
    let r = atan2(p.y, p.x) * v;
    return vec3<f32>(r * cos(a) * inv_w, r * sin(a) * inv_w, p.z);
}
"#,
};

// =============================================================================
// conic: cyberxaos 4/2007
//   ct = x / (sqrt(x² + y²) + ε)
//   r = w · (rand - holes) · ecc / (1 + ecc · ct) / (sqrt + ε)
//   out = (r · x, r · y)
// Body has w in `r` → needs_transform divide-out.
// =============================================================================
/// Conic-section sampler — emits `r · (x, y)` where `r = w · (rand − holes)
/// · eccentricity / (1 + ecc · x/s) / s` and `s = sqrt(x²+y²)`. The `1/(1 +
/// ecc·cos θ)` term is the standard polar equation of a conic section.
///
/// # Authors
/// - cyberxaos
pub static CONIC: VariationDef = VariationDef {
    name: "conic",
    aliases: &[],
    display_name: "Conic",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("eccentricity", "Eccentricity", unlimited_float, 1.0, -10.0, 10.0, "Conic eccentricity. 0 = circle, 1 = parabola, > 1 = hyperbola."),
        param!("holes", "Holes", unlimited_float, 0.0, -10.0, 10.0, "Random shift offset subtracted from the per-iteration random. Larger = sparser pattern."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_conic(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let ecc = get_param(xform_id, variation_id, 0u);
    let holes = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let s = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let ct = p.x / s;
    let denom_a = 1.0 + ecc * ct;
    let safe_denom = select(denom_a, 1e-30, abs(denom_a) < 1e-30);
    let r = w * (rng_nextf(rng) - holes) * ecc / safe_denom / s;
    return vec2<f32>(r * p.x * inv_w, r * p.y * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_conic(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let ecc = get_param(xform_id, variation_id, 0u);
    let holes = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let s = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let ct = p.x / s;
    let denom_a = 1.0 + ecc * ct;
    let safe_denom = select(denom_a, 1e-30, abs(denom_a) < 1e-30);
    let r = w * (rng_nextf(rng) - holes) * ecc / safe_denom / s;
    return vec3<f32>(r * p.x * inv_w, r * p.y * inv_w, p.z);
}
"#,
};

// =============================================================================
// power: power-warp (cpp swap quirks preserved)
//   r = w · pow(sqrt(x²+y²) + ε, x / (sqrt + ε))
//   out = r · (y/sqrt, x/sqrt)
// (cpp uses cosA in the exponent and swaps xy in the output, which
//  rotates the result 90° from Java's sinA / no-swap version.)
// Clean factor through outer.
// pow(base, x) for negative base would NaN — base = sqrt + ε ≥ ε > 0.
// =============================================================================
/// Power warp — emits `r^(x/r) · (y/r, x/r)` where `r = sqrt(x²+y²)`. The
/// exponent depends on the input's angle (`cos θ`), and the output
/// coordinates are swapped relative to the input (a 90° rotation; cpp's xy-
/// swap quirk, preserved).
pub static POWER: VariationDef = VariationDef {
    name: "power",
    aliases: &[],
    display_name: "Power",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_power(p: vec2<f32>) -> vec2<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let exponent = p.x / r;
    let m = pow(r, exponent);
    return vec2<f32>(m * p.y / r, m * p.x / r);
}
"#,
    wgsl_3d: r#"
fn variation_power(p: vec3<f32>) -> vec3<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let exponent = p.x / r;
    let m = pow(r, exponent);
    return vec3<f32>(m * p.y / r, m * p.x / r, p.z);
}
"#,
};

// =============================================================================
// roundspher: rounded-spherical inversion (Raykoid666)
//   d = x² + y²
//   e = 1/d + (2/π)²
//   out = w · (w/d · x / e, w/d · y / e)
// Body has w² → needs_transform divide-out (one w stripped, outer × w
// restores the second).
// =============================================================================
/// Rounded spherical inversion — softens the standard spherical inversion
/// `(x, y)/r²` by adding `(2/π)²` to the reciprocal-of-radius term,
/// yielding `(w·x, w·y) / (1 + (2/π)²·r²)`. Smooths out the singularity at
/// the origin.
///
/// # Authors
/// - Raykoid666
pub static ROUNDSPHER: VariationDef = VariationDef {
    name: "roundspher",
    aliases: &[],
    display_name: "Round Spher",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_roundspher(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_over_pi_sq = 0.40528473456935106;  // (2/π)²

    let d = max(p.x * p.x + p.y * p.y, 1e-30);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = select(e, 1e-30, abs(e) < 1e-30);

    let fx = w * (w / d * p.x / safe_e);
    let fy = w * (w / d * p.y / safe_e);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_roundspher(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_over_pi_sq = 0.40528473456935106;

    let d = max(p.x * p.x + p.y * p.y, 1e-30);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = select(e, 1e-30, abs(e) < 1e-30);

    let fx = w * (w / d * p.x / safe_e);
    let fy = w * (w / d * p.y / safe_e);
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// roundspher3D: 3D companion to roundspher (Larry Berlin, Sep 2009)
//   d = x² + y² + tempTZ²  where tempTZ = z, or cos(sqrt(x²+y²)) if z == 0
//   e = 1/d + (2/π)²
//   out_xy = w² × (x, y) / (d × e)
//   out_z  = tempPZ + w² × tempTZ / (d × e)
//          where tempPZ = result.z, or cos(sqrt(x²+y²)) if result.z == 0
// 3D-aware companion to `roundspher` — distinct variation (not an
// alias). The zero-Z fallback to cos(f) makes the variation behave
// reasonably the first time it runs in an iteration (when accumulator
// Z is still 0), injecting a cylinder-of-z shape rather than
// collapsing to flat. Body has w² → needs_transform divides one out;
// needs_accum reads `result.z` for the `tempPZ` zero-check fallback.
// =============================================================================
/// JWildfire 3D-aware companion to [`ROUNDSPHER`] — Larry Berlin's
/// extension of Raykoid666's rounded spherical inversion. Z participates
/// in the radius denominator (`d = x² + y² + z²`), and a zero input Z
/// is replaced with `cos(sqrt(x² + y²))` so the very first iteration
/// (when the accumulator is still 0) lands on a cylinder rather than
/// collapsing flat. Output Z accumulates additively when other
/// variations already contributed, or absorbs the same cos-of-radius
/// when this is the only Z-writer in the xform.
///
/// # Authors
/// - Raykoid666 (original `roundspher`)
/// - Larry Berlin (3D extension)
pub static ROUNDSPHER3D: VariationDef = VariationDef {
    name: "roundspher3D",
    aliases: &[],
    display_name: "Round Spher 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: true,
    wgsl_2d: r#"
fn variation_roundspher3D(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // 2D mode: no Z register to fall back on, so the cpp's tempPZ
    // branch collapses to the additive case. Body is roundspher's
    // 2D math; the only reason this exists in 2D is so a flame
    // saved with roundspher3D doesn't drop the variation entirely
    // when rendered in 2D mode.
    let w = transforms[xform_id].variations[variation_id];
    let two_over_pi_sq = 0.40528473456935106;  // (2/π)²
    let r2 = p.x * p.x + p.y * p.y;
    let d = max(r2, 1e-30);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = select(e, 1e-30, abs(e) < 1e-30);
    let de = d * safe_e;
    return vec2<f32>(w * p.x / de, w * p.y / de);
}
"#,
    wgsl_3d: r#"
fn variation_roundspher3D(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_over_pi_sq = 0.40528473456935106;  // (2/π)²

    let r2 = p.x * p.x + p.y * p.y;
    let f = sqrt(r2);

    // Zero-Z fallback to cos(f) — mirrors Larry Berlin's cpp.
    // `tempTZ` is the per-iteration input Z (or cos(f) if zero).
    // `inject_z` is the constant "kick" added to result.z when the
    // accumulator is still 0 (first Z-writer in the iteration). After
    // the outer × w multiplier this lands as cos(f), matching the cpp.
    let tempTZ = select(p.z, cos(f), p.z == 0.0);
    let inject_z = select(0.0, cos(f), accum.z == 0.0) * inv_w;

    let d = max(r2 + tempTZ * tempTZ, 1e-30);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = select(e, 1e-30, abs(e) < 1e-30);
    let de = d * safe_e;

    return vec3<f32>(
        w * p.x / de,
        w * p.y / de,
        inject_z + w * tempTZ / de,
    );
}
"#,
};

// =============================================================================
// cubic3D and cubicLattice_3D: 8-corner cubic-lattice scatter (Larry Berlin)
//   Both pick a random node from 8 corners of a unit cube each
//   iteration (3-bit useNode ∈ 0..7; bit 0 → Y sign, bit 1 → Z sign,
//   bit 2 → X sign), then write a node-specific offset on top of a
//   smooth-blended core formula. Distinguished mostly by the core:
//     cubic3D:        smoothing on weight + angular style mix
//                     with `smoothStyle` modulation
//     cubicLattice_3D: straight `(accum + p) × fill × angular` core
//                      with a two-stage style switch (style ≥ 2 turns
//                      on the angular cos/sin; else angular = 1)
//   Both bodies REPLACE the accumulator (`FPx = …` not `+=`) — our
//   additive dispatcher needs needs_accum (read result.x/y/z) and
//   needs_transform (divide one weight out) to invert into a `result
//   += w × return` shape. In 2D mode the Z bookkeeping degenerates
//   (atan2(_, 0) = ±π/2 → cos = 0 / sin = ±1); we still emit a body
//   so a flame saved with the 3D variation renders something in 2D
//   rather than dropping the variation entirely.
// =============================================================================

/// 8-corner cubic lattice with smooth-blended angular mixing — a
/// chaos-game scatter that biases each step toward one of the unit
/// cube's vertices, then blends it back toward the original point
/// based on a per-axis angular weight derived from the input
/// direction. `xpand` controls how aggressively each step pulls
/// toward the cube vertex (default 0.25, with a `sqrt(xpand)` ramp
/// once `|xpand| > 1`). `style` warps the per-axis angular weights:
/// 0 leaves them at 1 (pure cube), positive values introduce a
/// `1 - cos(atan2(x, z))` warp that softens the lattice along Z,
/// and `|style| > 1` further compresses the response.
///
/// # Authors
/// - Larry Berlin
pub static CUBIC3D: VariationDef = VariationDef {
    name: "cubic3D",
    aliases: &[],
    display_name: "Cubic 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("xpand", "Xpand", unlimited_float, 0.25, -10.0, 10.0, "How aggressively each step is pulled toward a cube vertex. Up through ±1 the response is linear (`fill = xpand × 0.5`); past that it follows `sqrt(xpand) × 0.5` so the lattice doesn't blow out at large values."),
        param!("style", "Style", unlimited_float, 0.0, -10.0, 10.0, "Angular warp on the per-axis weights derived from `cos(atan2(x, z))` and `sin(atan2(y, z))`. 0 disables the warp (per-axis weights are exactly 1); positive values soften the lattice along Z. `|style| > 1` introduces a secondary compression on the average XY/Z response."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: true,
    wgsl_2d: r#"
fn variation_cubic3D(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // 2D fallback: angular weights degenerate (atan2(_,0)) but stay
    // bounded; the Z-corner bookkeeping has no visible effect.
    let xpand = get_param(xform_id, variation_id, 0u);
    let style = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let use_node = u32(rng_nextf(rng) * 8.0);
    let sign_x = select(1.0, -1.0, (use_node & 4u) != 0u);
    let sign_y = select(1.0, -1.0, (use_node & 1u) != 0u);

    let smoothed = select(1.0, w * 2.0, abs(w) <= 0.5);

    var smooth_style = style;
    if (abs(style) > 1.0) {
        smooth_style = select(
            (style + 1.0) * 0.25 - 1.0,
            1.0 + (style - 1.0) * 0.25,
            style > 1.0,
        );
    }

    let fill = select(sqrt(abs(xpand)) * 0.5, xpand * 0.5, abs(xpand) <= 1.0);
    let one_minus_fill = 1.0 - fill;

    // atan2(_, 0) → ±π/2: cos = 0, sin = ±1.
    let exnze = 1.0 - smooth_style;
    let wynze = 1.0 - smooth_style * (1.0 - sign(p.y));

    let return_x = (-accum.x * smoothed * one_minus_fill * exnze + p.x * smoothed * fill * exnze) * inv_w + sign_x * 0.5;
    let return_y = (-accum.y * smoothed * one_minus_fill * wynze + p.y * smoothed * fill * wynze) * inv_w + sign_y * 0.5;
    return vec2<f32>(return_x, return_y);
}
"#,
    wgsl_3d: r#"
fn variation_cubic3D(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xpand = get_param(xform_id, variation_id, 0u);
    let style = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    // 8 corners of the unit cube, 3 bits of the random node index
    // pick each sign. bit 0 → Y, bit 1 → Z, bit 2 → X (matches cpp).
    let use_node = u32(rng_nextf(rng) * 8.0);
    let sign_x = select(1.0, -1.0, (use_node & 4u) != 0u);
    let sign_y = select(1.0, -1.0, (use_node & 1u) != 0u);
    let sign_z = select(1.0, -1.0, (use_node & 2u) != 0u);

    // smoothed: ramps the variation strength up over the [0, 0.5]
    // weight range and clamps to 1 past that.
    let smoothed = select(1.0, w * 2.0, abs(w) <= 0.5);

    // smooth_style: identity inside [-1, 1], then per-direction
    // compressed past the boundary so the lattice stays bounded.
    var smooth_style = style;
    if (abs(style) > 1.0) {
        smooth_style = select(
            (style + 1.0) * 0.25 - 1.0,
            1.0 + (style - 1.0) * 0.25,
            style > 1.0,
        );
    }

    // fill: linear up to ±1, sqrt past that. Uses |xpand| under the
    // sqrt so negative values don't NaN.
    let fill = select(sqrt(abs(xpand)) * 0.5, xpand * 0.5, abs(xpand) <= 1.0);
    let one_minus_fill = 1.0 - fill;

    // Per-axis angular weights. cpp uses cos(atan2(x, z)) and
    // sin(atan2(y, z)) — the equivalent closed form is z / sqrt(x²+z²)
    // and y / sqrt(y²+z²), but we keep the cpp expression for direct
    // correspondence with the reference.
    let exnze = 1.0 - smooth_style * (1.0 - cos(atan2(p.x, p.z)));
    let wynze = 1.0 - smooth_style * (1.0 - sin(atan2(p.y, p.z)));
    let znxy_inner = (exnze + wynze) * 0.5;
    let znxy = select(
        1.0 - smooth_style * (1.0 - znxy_inner),
        1.0 - smooth_style * (1.0 - znxy_inner * smooth_style),
        smooth_style > 1.0,
    );

    // cpp's core (FPx = (FPx - smoothed*(1-fill)*FPx*exnze)
    //                  + p.x*smoothed*fill*exnze + sign_x*w*0.5)
    // mapped into our additive dispatcher (`result += w × return`).
    // `sign_x × 0.5` is the per-unit-weight contribution of the
    // `sign_x × lattd = sign_x × w × 0.5` cube-vertex offset.
    let return_x = (-accum.x * smoothed * one_minus_fill * exnze + p.x * smoothed * fill * exnze) * inv_w + sign_x * 0.5;
    let return_y = (-accum.y * smoothed * one_minus_fill * wynze + p.y * smoothed * fill * wynze) * inv_w + sign_y * 0.5;
    let return_z = (-accum.z * smoothed * one_minus_fill * znxy + p.z * smoothed * fill * znxy) * inv_w + sign_z * 0.5;
    return vec3<f32>(return_x, return_y, return_z);
}
"#,
};

/// 8-corner cubic lattice — a simpler relative of [`CUBIC3D`]. Same
/// 8-vertex node scheme each iteration, but the core blend is a
/// straight `(accum + p) × fill × angular + sign × w` rather than
/// `cubic3D`'s smoothed mix. The `style` param is a two-stage
/// switch instead of a continuous warp: `|style| ≥ 2` turns on the
/// angular `cos(atan2(x,z))` / `sin(atan2(y,z))` weights; anything
/// below that pins all three weights to 1 (pure cube). `xpand`
/// controls vertex pull (default 0.2) with the same linear / `sqrt`
/// ramp as `cubic3D`. Lattice spacing scales with the variation
/// weight (`lattd = w` here, vs `w × 0.5` in `cubic3D`).
///
/// # Authors
/// - Larry Berlin
pub static CUBIC_LATTICE_3D: VariationDef = VariationDef {
    name: "cubicLattice_3D",
    aliases: &[],
    display_name: "Cubic Lattice 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("xpand", "Xpand", unlimited_float, 0.2, -10.0, 10.0, "How aggressively each step is pulled toward a cube vertex. Linear (`fill = xpand × 0.5`) up through ±1, `sqrt` past that."),
        param!("style", "Style", unlimited_float, 1.0, -10.0, 10.0, "Two-stage switch on the angular weights. `|style| ≥ 2` enables per-axis `cos(atan2(x, z))` / `sin(atan2(y, z))` warps that bias the lattice toward Z; anything below that pins the angular weights at 1 (pure cube)."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: true,
    wgsl_2d: r#"
fn variation_cubicLattice_3D(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xpand = get_param(xform_id, variation_id, 0u);
    let style = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let use_node = u32(rng_nextf(rng) * 8.0);
    let sign_x = select(1.0, -1.0, (use_node & 4u) != 0u);
    let sign_y = select(1.0, -1.0, (use_node & 1u) != 0u);

    let fill = select(sqrt(abs(xpand)) * 0.5, xpand * 0.5, abs(xpand) <= 1.0);

    // 2D fallback: angular weights degenerate as in cubic3D's 2D
    // body. Style 2+ → cos(±π/2) = 0 for exnze and sin(±π/2) = ±1
    // for wynze. Style < 2 keeps them at 1.
    var exnze = 1.0;
    var wynze = 1.0;
    if (abs(style) >= 2.0) {
        exnze = 0.0;
        wynze = sign(p.y);
    }

    let return_x = ((accum.x + p.x) * fill * exnze - accum.x) * inv_w + sign_x;
    let return_y = ((accum.y + p.y) * fill * wynze - accum.y) * inv_w + sign_y;
    return vec2<f32>(return_x, return_y);
}
"#,
    wgsl_3d: r#"
fn variation_cubicLattice_3D(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xpand = get_param(xform_id, variation_id, 0u);
    let style = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let use_node = u32(rng_nextf(rng) * 8.0);
    let sign_x = select(1.0, -1.0, (use_node & 4u) != 0u);
    let sign_y = select(1.0, -1.0, (use_node & 1u) != 0u);
    let sign_z = select(1.0, -1.0, (use_node & 2u) != 0u);

    let fill = select(sqrt(abs(xpand)) * 0.5, xpand * 0.5, abs(xpand) <= 1.0);

    // Style switch: |style| >= 2 → angular cos/sin/avg weights.
    // Below that, all three pin at 1 for pure cube scatter.
    var exnze = 1.0;
    var wynze = 1.0;
    var znxy = 1.0;
    if (abs(style) >= 2.0) {
        exnze = cos(atan2(p.x, p.z));
        wynze = sin(atan2(p.y, p.z));
        znxy = (exnze + wynze) * 0.5;
    }

    // cpp's core (FPx = (FPx + FTx) × fill × exnze + sign_x × w)
    // mapped into our additive dispatcher (`result += w × return`).
    // sign_x × w corresponds to sign_x × lattd (= sign_x × w here,
    // not × 0.5 like cubic3D), so the per-unit-weight contribution
    // is just `sign_x`.
    let return_x = ((accum.x + p.x) * fill * exnze - accum.x) * inv_w + sign_x;
    let return_y = ((accum.y + p.y) * fill * wynze - accum.y) * inv_w + sign_y;
    let return_z = ((accum.z + p.z) * fill * znxy - accum.z) * inv_w + sign_z;
    return vec3<f32>(return_x, return_y, return_z);
}
"#,
};

// =============================================================================
// checks: checkerboard cell-shift (Keeps / Xyrus02)
//   4 user params: x, y, size, rnd
//   1 init slot: cs = 1 / (size + ε)
//   isXY = round(x · cs) + round(y · cs)
//   if odd: dx = -x_param + rnd·rand; dy = -y_param
//   else:   dx = x_param;             dy = y_param + rnd·rand
//   out = (x + dx, y + dy)
// Clean factor through outer.
// =============================================================================
/// Checkerboard cell-shift — divides space into a grid of cells of size
/// `size`, classifies each cell as odd/even, and applies a different per-
/// axis shift in each parity class. Optionally jitters one component of the
/// shift by `rnd`.
///
/// # Authors
/// - Keeps
/// - Xyrus02
pub static CHECKS: VariationDef = VariationDef {
    name: "checks",
    aliases: &[],
    display_name: "Checks",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("x", "X", unlimited_float, 0.5, -10.0, 10.0, "X-cell-shift magnitude."),
        param!("y", "Y", unlimited_float, 0.5, -10.0, 10.0, "Y-cell-shift magnitude."),
        param!("size", "Size", unlimited_float, 0.5, 0.001, 10.0, "Cell grid size."),
        param!("rnd", "Rnd", unlimited_float, 0.0, -10.0, 10.0, "Random jitter magnitude on the cell shift."),
    ],
    needs_transform: false,
    writes_color: false,
    // 1 derived value at slot 4: cs = 1 / (size + ε)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_checks(user: array<f32, 4>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 1.0 / (user[2] + 1e-30);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_checks(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let rnd = get_param(xform_id, variation_id, 3u);
    let cs = get_param(xform_id, variation_id, 4u);

    let isXY = i32(round(p.x * cs)) + i32(round(p.y * cs));
    let rnx = rnd * rng_nextf(rng);
    let rny = rnd * rng_nextf(rng);
    var dx: f32;
    var dy: f32;
    if ((isXY & 1) != 0) {
        dx = -cx + rnx;
        dy = -cy;
    } else {
        dx = cx;
        dy = cy + rny;
    }
    return vec2<f32>(p.x + dx, p.y + dy);
}
"#,
    wgsl_3d: r#"
fn variation_checks(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let rnd = get_param(xform_id, variation_id, 3u);
    let cs = get_param(xform_id, variation_id, 4u);

    let isXY = i32(round(p.x * cs)) + i32(round(p.y * cs));
    let rnx = rnd * rng_nextf(rng);
    let rny = rnd * rng_nextf(rng);
    var dx: f32;
    var dy: f32;
    if ((isXY & 1) != 0) {
        dx = -cx + rnx;
        dy = -cy;
    } else {
        dx = cx;
        dy = cy + rny;
    }
    return vec3<f32>(p.x + dx, p.y + dy, p.z);
}
"#,
};

// =============================================================================
// cone: julia + hemisphere mix forming a cone (Brad Stefanov)
//   9 user params (Java-recovered; cpp PluginVarCalc empty stub):
//     radius1, radius2, size1, size2, ywave, xwave, height, warp, weight
//   r = w / sqrt(x²·warp + y² + size1) · size2
//   xx = atan2(y, x)·radius1 + π·floor(weight · rand)·radius2
//   out = r · (cos(xx·xwave), sin(xx·ywave), height)
// 3D output (z ≠ pass-through). Clean factor through outer; needs_rng.
// =============================================================================
/// Julia + hemisphere mix forming a cone — combines a Julia-style angular
/// pick `π·floor(weight · rand)·radius2 + atan2(y,x)·radius1` with a
/// hemisphere-style radial term `r = size2 / sqrt(x²·warp + y² + size1)`,
/// plus a configurable `height` Z output. The result traces a cone-shaped
/// surface in 3D.
///
/// # Authors
/// - Brad Stefanov
pub static CONE: VariationDef = VariationDef {
    name: "cone",
    aliases: &[],
    display_name: "Cone",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("radius1", "Radius 1", unlimited_float, 0.5, -10.0, 10.0, "Inner-radius multiplier on the input angle."),
        param!("radius2", "Radius 2", unlimited_float, 1.0, -10.0, 10.0, "Outer-radius multiplier on the random branch offset."),
        param!("size1", "Size 1", unlimited_float, 0.5, -10.0, 10.0, "Squared-radius offset in the denominator."),
        param!("size2", "Size 2", unlimited_float, 2.0, -10.0, 10.0, "Output radius scale (numerator of `r`)."),
        param!("ywave", "Y Wave", unlimited_float, 1.0, -10.0, 10.0, "Y-axis frequency multiplier in `sin(xx·ywave)`."),
        param!("xwave", "X Wave", unlimited_float, 1.0, -10.0, 10.0, "X-axis frequency multiplier in `cos(xx·xwave)`."),
        param!("height", "Height", unlimited_float, 1.0, -10.0, 10.0, "Z output scale (3D only — controls cone height)."),
        param!("warp", "Warp", unlimited_float, 1.0, -10.0, 10.0, "X² weight in the denominator. Controls the aspect ratio of the cone."),
        param!("weight", "Weight", unlimited_float, 2.0, -10.0, 10.0, "Number of random angular branches — `floor(weight · rand)` picks the branch."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_cone(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius1 = get_param(xform_id, variation_id, 0u);
    let radius2 = get_param(xform_id, variation_id, 1u);
    let size1 = get_param(xform_id, variation_id, 2u);
    let size2 = get_param(xform_id, variation_id, 3u);
    let ywave = get_param(xform_id, variation_id, 4u);
    let xwave = get_param(xform_id, variation_id, 5u);
    let warp = get_param(xform_id, variation_id, 7u);
    let weight = get_param(xform_id, variation_id, 8u);
    let pi = 3.14159265358979;

    let denom = sqrt(max(p.x * p.x * warp + p.y * p.y + size1, 1e-30));
    let r = size2 / denom;
    let xx = atan2(p.y, p.x) * radius1 + pi * floor(weight * rng_nextf(rng)) * radius2;
    return vec2<f32>(r * cos(xx * xwave), r * sin(xx * ywave));
}
"#,
    wgsl_3d: r#"
fn variation_cone(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius1 = get_param(xform_id, variation_id, 0u);
    let radius2 = get_param(xform_id, variation_id, 1u);
    let size1 = get_param(xform_id, variation_id, 2u);
    let size2 = get_param(xform_id, variation_id, 3u);
    let ywave = get_param(xform_id, variation_id, 4u);
    let xwave = get_param(xform_id, variation_id, 5u);
    let height = get_param(xform_id, variation_id, 6u);
    let warp = get_param(xform_id, variation_id, 7u);
    let weight = get_param(xform_id, variation_id, 8u);
    let pi = 3.14159265358979;

    let denom = sqrt(max(p.x * p.x * warp + p.y * p.y + size1, 1e-30));
    let r = size2 / denom;
    let xx = atan2(p.y, p.x) * radius1 + pi * floor(weight * rng_nextf(rng)) * radius2;
    return vec3<f32>(r * cos(xx * xwave), r * sin(xx * ywave), r * height);
}
"#,
};
