//! Bipolar (B-) and Elliptic (E-) series — Michael Faber's coordinate-system
//! warp families
//!
//! Two parallel families of variations sharing a common coordinate
//! transformation:
//!
//!   B-series uses bipolar coords (τ, σ):
//!     τ = ½ · (log((x+1)² + y²) − log((x−1)² + y²))
//!     σ = π − atan2(y, x+1) − atan2(y, 1−x)
//!     out = (sinh τ, sin σ) / (cosh τ − cos σ)
//!
//!   E-series uses elliptic coords (μ, ν):
//!     xmax = (sqrt(x²+y²+1+2x) + sqrt(x²+y²+1−2x)) / 2     (clamped ≥ 1)
//!     μ = acosh(xmax);   ν = acos(clamp(x/xmax, −1, 1)) · sign(y)
//!     out = (cosh μ · cos ν, sinh μ · sin ν)
//!
//! Each variation tweaks σ (B-series) or (μ, ν) (E-series) before the
//! final mapping. All factor VVAR cleanly through the outer multiplier.
//!
//! Plus `barycentroid` (Xyrus02) which uses barycentric coordinates of
//! the input point relative to a user-defined triangle.
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`. All 10 ports share the same
//! "VVAR strictly outer multiplier" pattern.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// bcollide: bipolar-σ branch-and-mod (Faber)
//   alt = floor(σ · num/π)
//   if alt even: σ = alt · π/num + ((σ + a·π/num) mod π/num)
//   else:        σ = alt · π/num + ((σ − a·π/num) mod π/num)
// =============================================================================
/// Bipolar warp with σ-axis branch-and-mod — converts the input to bipolar
/// coordinates `(τ, σ)`, splits σ into `num` equal arcs, and mirrors
/// alternating arcs around their boundaries with a per-arc offset `a`.
/// Produces a bipolar tiling with controllable wedge count.
///
/// # Authors
/// - Michael Faber
pub static BCOLLIDE: VariationDef = VariationDef {
    name: "bCollide",
    aliases: &[],
    display_name: "BCollide",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("num", "N", int, 1.0, 1.0, 50.0, "Number of σ wedges. Higher = finer tiling."),
        param!("a", "A", unlimited_float, 0.0, -10.0, 10.0, "Per-wedge angular offset."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bCollide(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let num = max(get_param(xform_id, variation_id, 0u), 1.0);
    let a = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_n = pi / num;
    let a_n = pi * a / num;
    let n_invpi = num * inv_pi;

    let tau = 0.5 * (log(max((p.x + 1.0) * (p.x + 1.0) + p.y * p.y, 1e-30))
                  - log(max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-30)));
    var sigma = pi - atan2(p.y, p.x + 1.0) - atan2(p.y, 1.0 - p.x);
    let alt = floor(sigma * n_invpi);
    if (i32(alt) % 2 == 0) {
        sigma = alt * pi_n + ((sigma + a_n) - floor((sigma + a_n) / pi_n) * pi_n);
    } else {
        sigma = alt * pi_n + ((sigma - a_n) - floor((sigma - a_n) / pi_n) * pi_n);
    }
    let temp = cosh(tau) - cos(sigma);
    let inv_t = 1.0 / select(temp, 1e-30, abs(temp) < 1e-30);
    return vec2<f32>(sinh(tau) * inv_t, sin(sigma) * inv_t);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bCollide(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let num = max(get_param(xform_id, variation_id, 0u), 1.0);
    let a = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_n = pi / num;
    let a_n = pi * a / num;
    let n_invpi = num * inv_pi;

    let tau = 0.5 * (log(max((p.x + 1.0) * (p.x + 1.0) + p.y * p.y, 1e-30))
                  - log(max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-30)));
    var sigma = pi - atan2(p.y, p.x + 1.0) - atan2(p.y, 1.0 - p.x);
    let alt = floor(sigma * n_invpi);
    if (i32(alt) % 2 == 0) {
        sigma = alt * pi_n + ((sigma + a_n) - floor((sigma + a_n) / pi_n) * pi_n);
    } else {
        sigma = alt * pi_n + ((sigma - a_n) - floor((sigma - a_n) / pi_n) * pi_n);
    }
    let temp = cosh(tau) - cos(sigma);
    let inv_t = 1.0 / select(temp, 1e-30, abs(temp) < 1e-30);
    return vec3<f32>(sinh(tau) * inv_t, sin(sigma) * inv_t, p.z);
}
"#),
};

// =============================================================================
// bmod: bipolar-τ clamp-and-mod (Faber)
//   if |τ| < radius: τ = ((τ + radius + distance·radius) mod 2·radius) − radius
// =============================================================================
/// Bipolar warp with τ-axis clamp-and-mod — converts to bipolar
/// coordinates, then wraps τ around within `±radius` (offset by
/// `distance·radius`). Points with τ outside the band pass through
/// unchanged.
///
/// # Authors
/// - Michael Faber
pub static BMOD: VariationDef = VariationDef {
    name: "bMod",
    aliases: &[],
    display_name: "BMod",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0, "Half-width of the τ band to mod-wrap."),
        param!("distance", "Distance", unlimited_float, 0.0, -10.0, 10.0, "Additional τ offset applied before mod, in units of `radius`."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bMod(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let distance = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    var tau = 0.5 * (log(max((p.x + 1.0) * (p.x + 1.0) + p.y * p.y, 1e-30))
                  - log(max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-30)));
    let sigma = pi - atan2(p.y, p.x + 1.0) - atan2(p.y, 1.0 - p.x);
    if (tau < radius && -tau < radius) {
        let two_r = 2.0 * radius;
        let safe_two_r = select(two_r, 1e-30, abs(two_r) < 1e-30);
        let v = tau + radius + distance * radius;
        tau = (v - floor(v / safe_two_r) * safe_two_r) - radius;
    }
    let temp = cosh(tau) - cos(sigma);
    if (temp == 0.0) { return vec2<f32>(0.0, 0.0); }
    return vec2<f32>(sinh(tau) / temp, sin(sigma) / temp);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bMod(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let distance = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    var tau = 0.5 * (log(max((p.x + 1.0) * (p.x + 1.0) + p.y * p.y, 1e-30))
                  - log(max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-30)));
    let sigma = pi - atan2(p.y, p.x + 1.0) - atan2(p.y, 1.0 - p.x);
    if (tau < radius && -tau < radius) {
        let two_r = 2.0 * radius;
        let safe_two_r = select(two_r, 1e-30, abs(two_r) < 1e-30);
        let v = tau + radius + distance * radius;
        tau = (v - floor(v / safe_two_r) * safe_two_r) - radius;
    }
    let temp = cosh(tau) - cos(sigma);
    if (temp == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
    return vec3<f32>(sinh(tau) / temp, sin(sigma) / temp, p.z);
}
"#),
};

// =============================================================================
// bswirl: bipolar swirl (Faber)
//   σ' = σ + τ · out + in / τ
// =============================================================================
/// Bipolar swirl — adds `τ·out + in/τ` to the bipolar σ coordinate. The
/// combination of an additive (`out`) and reciprocal (`in`) term produces a
/// hyperbolic-style swirl pattern aligned with the bipolar grid.
///
/// # Authors
/// - Michael Faber
pub static BSWIRL: VariationDef = VariationDef {
    name: "bSwirl",
    aliases: &[],
    display_name: "BSwirl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("in_p", "In", unlimited_float, 0.0, -10.0, 10.0, "Reciprocal-τ swirl coefficient — adds `in/τ` to σ."),
        param!("out_p", "Out", unlimited_float, 0.0, -10.0, 10.0, "Linear-τ swirl coefficient — adds `τ·out` to σ."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_bSwirl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let in_p = get_param(xform_id, variation_id, 0u);
    let out_p = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let tau = 0.5 * (log(max((p.x + 1.0) * (p.x + 1.0) + p.y * p.y, 1e-30))
                  - log(max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-30)));
    let safe_tau = select(tau, 1e-30, abs(tau) < 1e-30);
    var sigma = pi - atan2(p.y, p.x + 1.0) - atan2(p.y, 1.0 - p.x);
    sigma = sigma + tau * out_p + in_p / safe_tau;
    let temp = cosh(tau) - cos(sigma);
    if (temp == 0.0) { return vec2<f32>(0.0, 0.0); }
    return vec2<f32>(sinh(tau) / temp, sin(sigma) / temp);
}
"#,
    wgsl_3d: Some(r#"
fn variation_bSwirl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let in_p = get_param(xform_id, variation_id, 0u);
    let out_p = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let tau = 0.5 * (log(max((p.x + 1.0) * (p.x + 1.0) + p.y * p.y, 1e-30))
                  - log(max((p.x - 1.0) * (p.x - 1.0) + p.y * p.y, 1e-30)));
    let safe_tau = select(tau, 1e-30, abs(tau) < 1e-30);
    var sigma = pi - atan2(p.y, p.x + 1.0) - atan2(p.y, 1.0 - p.x);
    sigma = sigma + tau * out_p + in_p / safe_tau;
    let temp = cosh(tau) - cos(sigma);
    if (temp == 0.0) { return vec3<f32>(0.0, 0.0, p.z); }
    return vec3<f32>(sinh(tau) / temp, sin(sigma) / temp, p.z);
}
"#),
};

// =============================================================================
// barycentroid: barycentric-coordinate warp (Xyrus02)
//   Triangle defined by V0=(a,b), V1=(c,d), V2=(x,y).
//   Compute (u, v) as barycentric coords of (x, y) in triangle.
//   Output: (sign(u) · sqrt(u² + x²), sign(v) · sqrt(v² + y²))
// =============================================================================
/// Barycentric-coordinate warp — treats the input as the third vertex of a
/// triangle whose other two vertices are user-defined: `V0 = (a, b)`, `V1 =
/// (c, d)`, `V2 = (x, y)`. Computes barycentric coordinates `(u, v)` of
/// `(x, y)` in the triangle, then emits `(sign(u)·sqrt(u²+x²),
/// sign(v)·sqrt(v²+y²))`.
///
/// # Authors
/// - Xyrus02
pub static BARYCENTROID: VariationDef = VariationDef {
    name: "barycentroid",
    aliases: &[],
    display_name: "Barycentroid",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "V0.x", unlimited_float, 1.0, -10.0, 10.0, "V0 X coordinate (first triangle vertex)."),
        param!("b", "V0.y", unlimited_float, 0.0, -10.0, 10.0, "V0 Y coordinate."),
        param!("c", "V1.x", unlimited_float, 0.0, -10.0, 10.0, "V1 X coordinate (second triangle vertex)."),
        param!("d", "V1.y", unlimited_float, 1.0, -10.0, 10.0, "V1 Y coordinate."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_barycentroid(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);

    let dot00 = a * a + b * b;
    let dot01 = a * c + b * d;
    let dot02 = a * p.x + b * p.y;
    let dot11 = c * c + d * d;
    let dot12 = c * p.x + d * p.y;
    let denom = dot00 * dot11 - dot01 * dot01;
    let inv_denom = 1.0 / select(denom, 1e-30, abs(denom) < 1e-30);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    let sgn_u = select(sign(u), 0.0, u == 0.0);
    let sgn_v = select(sign(v), 0.0, v == 0.0);
    return vec2<f32>(
        sqrt(u * u + p.x * p.x) * sgn_u,
        sqrt(v * v + p.y * p.y) * sgn_v,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_barycentroid(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);

    let dot00 = a * a + b * b;
    let dot01 = a * c + b * d;
    let dot02 = a * p.x + b * p.y;
    let dot11 = c * c + d * d;
    let dot12 = c * p.x + d * p.y;
    let denom = dot00 * dot11 - dot01 * dot01;
    let inv_denom = 1.0 / select(denom, 1e-30, abs(denom) < 1e-30);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    let sgn_u = select(sign(u), 0.0, u == 0.0);
    let sgn_v = select(sign(v), 0.0, v == 0.0);
    return vec3<f32>(
        sqrt(u * u + p.x * p.x) * sgn_u,
        sqrt(v * v + p.y * p.y) * sgn_v,
        p.z,
    );
}
"#),
};

// =============================================================================
// E-series helper macro: each E-series variation begins with the same
// elliptic-coordinate setup. Inlined here as a WGSL function only used
// within this file's bodies.
//
// Note: we don't have a way to share WGSL helper functions across
// variations in our shader builder, so each E-series body inlines the
// setup. Hopefully the GPU compiler CSE's them when multiple E-series
// variations run in the same flame.
// =============================================================================

// =============================================================================
// ecollide: elliptic-ν branch-and-mod (Faber)
// =============================================================================
/// Elliptic warp with ν-axis branch-and-mod — converts to elliptic
/// coordinates `(μ, ν)`, splits ν into `num` equal wedges, and mirrors
/// alternating wedges with a per-wedge offset. Elliptic analogue of
/// `bcollide`.
///
/// # Authors
/// - Michael Faber
pub static ECOLLIDE: VariationDef = VariationDef {
    name: "eCollide",
    aliases: &[],
    display_name: "ECollide",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("num", "N", int, 1.0, 1.0, 50.0, "Number of ν wedges."),
        param!("a", "A", unlimited_float, 0.0, -10.0, 10.0, "Per-wedge angular offset."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eCollide(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let num = max(get_param(xform_id, variation_id, 0u), 1.0);
    let a = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_n = pi / num;
    let a_n = pi * a / num;

    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    let alt = floor(nu * num * inv_pi);
    if (i32(alt) % 2 == 0) {
        nu = alt * pi_n + ((nu + a_n) - floor((nu + a_n) / pi_n) * pi_n);
    } else {
        nu = alt * pi_n + ((nu - a_n) - floor((nu - a_n) / pi_n) * pi_n);
    }
    if (p.y <= 0.0) { nu = -nu; }
    return vec2<f32>(
        xmax * cos(nu),
        sqrt(max(xmax - 1.0, 0.0)) * sqrt(max(xmax + 1.0, 0.0)) * sin(nu),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_eCollide(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let num = max(get_param(xform_id, variation_id, 0u), 1.0);
    let a = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_n = pi / num;
    let a_n = pi * a / num;

    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    let alt = floor(nu * num * inv_pi);
    if (i32(alt) % 2 == 0) {
        nu = alt * pi_n + ((nu + a_n) - floor((nu + a_n) / pi_n) * pi_n);
    } else {
        nu = alt * pi_n + ((nu - a_n) - floor((nu - a_n) / pi_n) * pi_n);
    }
    if (p.y <= 0.0) { nu = -nu; }
    return vec3<f32>(
        xmax * cos(nu),
        sqrt(max(xmax - 1.0, 0.0)) * sqrt(max(xmax + 1.0, 0.0)) * sin(nu),
        p.z,
    );
}
"#),
};

// =============================================================================
// emod: elliptic-μ clamp-and-mod (Faber)
// =============================================================================
/// Elliptic warp with μ-axis clamp-and-mod — wraps μ around within
/// `±radius` (offset by `distance·radius`). The sign of ν determines the
/// mod direction. Elliptic analogue of `bmod`.
///
/// # Authors
/// - Michael Faber
pub static EMOD: VariationDef = VariationDef {
    name: "eMod",
    aliases: &[],
    display_name: "EMod",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("radius", "Radius", unlimited_float, 1.0, -10.0, 10.0, "Half-width of the μ band to mod-wrap."),
        param!("distance", "Distance", unlimited_float, 0.0, -10.0, 10.0, "Additional μ offset, in units of `radius`."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eMod(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let distance = get_param(xform_id, variation_id, 1u);
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    if (mu < radius && -mu < radius) {
        let two_r = 2.0 * radius;
        let safe_two_r = select(two_r, 1e-30, abs(two_r) < 1e-30);
        if (nu > 0.0) {
            let v = mu + radius + distance * radius;
            mu = (v - floor(v / safe_two_r) * safe_two_r) - radius;
        } else {
            let v = mu - radius - distance * radius;
            mu = (v - floor(v / safe_two_r) * safe_two_r) + radius;
        }
    }
    return vec2<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu));
}
"#,
    wgsl_3d: Some(r#"
fn variation_eMod(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let distance = get_param(xform_id, variation_id, 1u);
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    if (mu < radius && -mu < radius) {
        let two_r = 2.0 * radius;
        let safe_two_r = select(two_r, 1e-30, abs(two_r) < 1e-30);
        if (nu > 0.0) {
            let v = mu + radius + distance * radius;
            mu = (v - floor(v / safe_two_r) * safe_two_r) - radius;
        } else {
            let v = mu - radius - distance * radius;
            mu = (v - floor(v / safe_two_r) * safe_two_r) + radius;
        }
    }
    return vec3<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu), p.z);
}
"#),
};

// =============================================================================
// eswirl: elliptic-ν swirl (Faber)
//   ν' = ν + μ · out + in / μ
// =============================================================================
/// Elliptic swirl — adds `μ·out + in/μ` to the elliptic ν coordinate.
/// Elliptic analogue of `bswirl`.
///
/// # Authors
/// - Michael Faber
pub static ESWIRL: VariationDef = VariationDef {
    name: "eSwirl",
    aliases: &[],
    display_name: "ESwirl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("in_p", "In", unlimited_float, 1.2, -10.0, 10.0, "Reciprocal-μ swirl coefficient — adds `in/μ` to ν."),
        param!("out_p", "Out", unlimited_float, 0.2, -10.0, 10.0, "Linear-μ swirl coefficient — adds `μ·out` to ν."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eSwirl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let in_p = get_param(xform_id, variation_id, 0u);
    let out_p = get_param(xform_id, variation_id, 1u);
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let mu = acosh(xmax);
    let safe_mu = select(mu, 1e-30, abs(mu) < 1e-30);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    nu = nu + mu * out_p + in_p / safe_mu;
    return vec2<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu));
}
"#,
    wgsl_3d: Some(r#"
fn variation_eSwirl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let in_p = get_param(xform_id, variation_id, 0u);
    let out_p = get_param(xform_id, variation_id, 1u);
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let mu = acosh(xmax);
    let safe_mu = select(mu, 1e-30, abs(mu) < 1e-30);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    nu = nu + mu * out_p + in_p / safe_mu;
    return vec3<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu), p.z);
}
"#),
};

// =============================================================================
// escale: elliptic scale + angle (Faber)
//   μ' = μ · scale
//   ν' = ((ν+π+angle)·scale mod 2π·scale) − angle − scale·π   (mod 2π)
// =============================================================================
/// Elliptic scale + angular wrap — scales μ by `scale` and applies a scale-
/// plus-angle modular operation to ν, wrapping the result into `[-π, π]`.
/// Useful for periodic ellipse-tiled patterns.
///
/// # Authors
/// - Michael Faber
pub static ESCALE: VariationDef = VariationDef {
    name: "eScale",
    aliases: &[],
    display_name: "EScale",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Multiplicative scale on μ; also scales the ν mod-wrap window."),
        param!("angle", "Angle", angle, 0.0, "Angular offset applied to ν before scaling, in degrees."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eScale(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let scale = get_param(xform_id, variation_id, 0u);
    let angle_deg = get_param(xform_id, variation_id, 1u);
    let angle = angle_deg * 3.14159265358979 / 180.0;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    mu = mu * scale;
    let two_pi_s = two_pi * scale;
    let safe_tps = select(two_pi_s, 1e-30, abs(two_pi_s) < 1e-30);
    let v1 = scale * (nu + pi + angle);
    let v1m = v1 - floor(v1 / safe_tps) * safe_tps - angle - scale * pi;
    nu = v1m - floor(v1m / two_pi) * two_pi;
    if (nu > pi) { nu = nu - two_pi; }
    if (nu < -pi) { nu = nu + two_pi; }
    return vec2<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu));
}
"#,
    wgsl_3d: Some(r#"
fn variation_eScale(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let scale = get_param(xform_id, variation_id, 0u);
    let angle_deg = get_param(xform_id, variation_id, 1u);
    let angle = angle_deg * 3.14159265358979 / 180.0;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    mu = mu * scale;
    let two_pi_s = two_pi * scale;
    let safe_tps = select(two_pi_s, 1e-30, abs(two_pi_s) < 1e-30);
    let v1 = scale * (nu + pi + angle);
    let v1m = v1 - floor(v1 / safe_tps) * safe_tps - angle - scale * pi;
    nu = v1m - floor(v1m / two_pi) * two_pi;
    if (nu > pi) { nu = nu - two_pi; }
    if (nu < -pi) { nu = nu + two_pi; }
    return vec3<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu), p.z);
}
"#),
};

// =============================================================================
// epush: elliptic push (Faber)
//   ν' = ν + rotate
//   μ' = μ · dist + push
// =============================================================================
/// Elliptic push — additively offsets μ by `push`, multiplicatively scales
/// μ by `dist`, and rotates ν by `rotate`.
///
/// # Authors
/// - Michael Faber
pub static EPUSH: VariationDef = VariationDef {
    name: "ePush",
    aliases: &[],
    display_name: "EPush",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("push", "Push", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on μ."),
        param!("dist", "Dist", unlimited_float, 1.0, -10.0, 10.0, "Multiplicative scale on μ."),
        param!("rotate", "Rotate", unlimited_float, 0.0, -10.0, 10.0, "Angular shift on ν, in radians."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_ePush(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let push = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);
    let rotate = get_param(xform_id, variation_id, 2u);
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    nu = nu + rotate;
    mu = mu * dist + push;
    return vec2<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu));
}
"#,
    wgsl_3d: Some(r#"
fn variation_ePush(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let push = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);
    let rotate = get_param(xform_id, variation_id, 2u);
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    nu = nu + rotate;
    mu = mu * dist + push;
    return vec3<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu), p.z);
}
"#),
};

// =============================================================================
// erotate: elliptic rotate (Faber)
//   ν' = ((ν + rotate + π) mod 2π) − π   (kept in [−π, π])
//   Output uses xmax · cos ν / sqrt-twin · sin ν (NOT cosh/sinh — same
//   form as ecollide).
// =============================================================================
/// Elliptic rotation — adds `rotate` to the elliptic ν coordinate and wraps
/// the result into `[-π, π]`. Uses the `xmax·cos(ν)` and
/// `sqrt(xmax²−1)·sin(ν)` output form (matching `ecollide`) rather than the
/// cosh/sinh form used by the other E-series variations.
///
/// # Authors
/// - Michael Faber
pub static EROTATE: VariationDef = VariationDef {
    name: "eRotate",
    aliases: &[],
    display_name: "ERotate",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("rotate", "Rotate", unlimited_float, 0.0, -10.0, 10.0, "Angular shift on ν, in radians."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eRotate(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let rotate = get_param(xform_id, variation_id, 0u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    let v = nu + rotate + pi;
    nu = (v - floor(v / two_pi) * two_pi) - pi;
    return vec2<f32>(
        xmax * cos(nu),
        sqrt(max(xmax - 1.0, 0.0)) * sqrt(max(xmax + 1.0, 0.0)) * sin(nu),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_eRotate(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let rotate = get_param(xform_id, variation_id, 0u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax = max((sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5, 1.0);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) { nu = -nu; }
    let v = nu + rotate + pi;
    nu = (v - floor(v / two_pi) * two_pi) - pi;
    return vec3<f32>(
        xmax * cos(nu),
        sqrt(max(xmax - 1.0, 0.0)) * sqrt(max(xmax + 1.0, 0.0)) * sin(nu),
        p.z,
    );
}
"#),
};
