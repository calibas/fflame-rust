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
