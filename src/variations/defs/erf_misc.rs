//! `erf` family + small misc batch
//!
//!   - `erf`         (zephyrtronium / dark-beam) — 2D component-wise erf
//!   - `erf3d`       (zephyrtronium / dark-beam) — 3D component-wise erf
//!   - `d_spherical` (Tatyana Zabanova)          — RNG-blended
//!                                                  spherical/linear
//!   - `dustpoint`   (Jesus Sosa)                — 3-point pivot/overlap
//!                                                  IFS triangle (RNG)
//!   - `deltaa`      (Faber)                     — radial ratio + half
//!                                                  angle difference;
//!                                                  needs_transform
//!                                                  divide-out
//!   - `edisc`       (Apophysis Plugin Pack)     — elliptic disc;
//!                                                  needs_transform
//!                                                  divide-out (w/11.57…
//!                                                  internal factor)
//!   - `curve`       (Apophysis Plugin Pack)     — gaussian X/Y bumps,
//!                                                  4 user + 2 init
//!   - `elliptic2`   (Brad Stefanov)             — 11 user + 1 init
//!                                                  slot; RNG;
//!                                                  needs_transform
//!                                                  divide-out (`_v =
//!                                                  w·h/π`, plus an
//!                                                  unweighted `+ ps`
//!                                                  offset that costs
//!                                                  one extra `1/w`)
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! `erf`/`erf3d` inline the Abramowitz & Stegun 7.1.26 polynomial
//! (max error ~1.5e-7) rather than sharing a helper, since the
//! shader builder concatenates active variation bodies and a
//! shared helper name would clash if both variations are active.
//! `xerf` (apo_misc.rs) names its helper `xerf_erf`, so it does
//! not collide with these inlined copies.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// erf: 2D component-wise erf
//   out = (erf(x), erf(y))
// Clean factor through outer.
// =============================================================================
/// 2D component-wise erf — applies the error function `erf(coord)` to each
/// axis independently. Saturates the input smoothly toward ±1, like a soft
/// clip.
///
/// # Authors
/// - zephyrtronium
/// - DarkBeam
pub static ERF: VariationDef = VariationDef {
    name: "erf",
    aliases: &[],
    display_name: "Erf",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_erf(p: vec2<f32>) -> vec2<f32> {
    // A&S 7.1.26 inlined twice
    let pp = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let sx = sign(p.x);
    let ax = abs(p.x);
    let tx = 1.0 / (1.0 + pp * ax);
    let polyx = ((((a5 * tx + a4) * tx + a3) * tx + a2) * tx + a1) * tx;
    let fx = sx * (1.0 - polyx * exp(-ax * ax));
    let sy = sign(p.y);
    let ay = abs(p.y);
    let ty = 1.0 / (1.0 + pp * ay);
    let polyy = ((((a5 * ty + a4) * ty + a3) * ty + a2) * ty + a1) * ty;
    let fy = sy * (1.0 - polyy * exp(-ay * ay));
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_erf(p: vec3<f32>) -> vec3<f32> {
    let pp = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let sx = sign(p.x);
    let ax = abs(p.x);
    let tx = 1.0 / (1.0 + pp * ax);
    let polyx = ((((a5 * tx + a4) * tx + a3) * tx + a2) * tx + a1) * tx;
    let fx = sx * (1.0 - polyx * exp(-ax * ax));
    let sy = sign(p.y);
    let ay = abs(p.y);
    let ty = 1.0 / (1.0 + pp * ay);
    let polyy = ((((a5 * ty + a4) * ty + a3) * ty + a2) * ty + a1) * ty;
    let fy = sy * (1.0 - polyy * exp(-ay * ay));
    return vec3<f32>(fx, fy, p.z);
}
"#,
};

// =============================================================================
// erf3d: 3D component-wise erf
//   out = (erf(x), erf(y), erf(z))
// Clean factor through outer; Full3D (writes z explicitly).
// =============================================================================
/// 3D component-wise erf — same as `erf` but with `erf(z)` applied to the Z
/// axis too.
///
/// # Authors
/// - zephyrtronium
/// - DarkBeam
pub static ERF3D: VariationDef = VariationDef {
    name: "erf3D",
    aliases: &[],
    display_name: "Erf 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_erf3D(p: vec2<f32>) -> vec2<f32> {
    let pp = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let sx = sign(p.x);
    let ax = abs(p.x);
    let tx = 1.0 / (1.0 + pp * ax);
    let polyx = ((((a5 * tx + a4) * tx + a3) * tx + a2) * tx + a1) * tx;
    let fx = sx * (1.0 - polyx * exp(-ax * ax));
    let sy = sign(p.y);
    let ay = abs(p.y);
    let ty = 1.0 / (1.0 + pp * ay);
    let polyy = ((((a5 * ty + a4) * ty + a3) * ty + a2) * ty + a1) * ty;
    let fy = sy * (1.0 - polyy * exp(-ay * ay));
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_erf3D(p: vec3<f32>) -> vec3<f32> {
    let pp = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let sx = sign(p.x);
    let ax = abs(p.x);
    let tx = 1.0 / (1.0 + pp * ax);
    let polyx = ((((a5 * tx + a4) * tx + a3) * tx + a2) * tx + a1) * tx;
    let fx = sx * (1.0 - polyx * exp(-ax * ax));
    let sy = sign(p.y);
    let ay = abs(p.y);
    let ty = 1.0 / (1.0 + pp * ay);
    let polyy = ((((a5 * ty + a4) * ty + a3) * ty + a2) * ty + a1) * ty;
    let fy = sy * (1.0 - polyy * exp(-ay * ay));
    let sz = sign(p.z);
    let az = abs(p.z);
    let tz = 1.0 / (1.0 + pp * az);
    let polyz = ((((a5 * tz + a4) * tz + a3) * tz + a2) * tz + a1) * tz;
    let fz = sz * (1.0 - polyz * exp(-az * az));
    return vec3<f32>(fx, fy, fz);
}
"#,
};

// =============================================================================
// d_spherical: random-blend spherical/linear (Tatyana Zabanova)
//   if rand < d_spher_weight: out = (x·w/r², y·w/r²)  (spherical)
//   else:                     out = (x·w, y·w)        (linear)
// Both branches factor cleanly through outer.
// =============================================================================
/// Random-blend spherical/linear — each iteration flips a weighted coin:
/// with probability `d_spher_weight` applies a spherical inversion `(x, y)
/// / (x²+y²)`, otherwise passes through unchanged. Smoothly blends the two
/// effects across many samples.
///
/// # Authors
/// - Tatyana Zabanova
pub static D_SPHERICAL: VariationDef = VariationDef {
    name: "d_spherical",
    aliases: &[],
    display_name: "D-Spherical",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("d_spher_weight", "D Spher Weight", unlimited_float, 0.5, 0.0, 1.0, "Probability of applying the spherical inversion (vs linear pass-through)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_d_spherical(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let d_spher_weight = get_param(xform_id, variation_id, 0u);
    if (rng_nextf(rng) < d_spher_weight) {
        let r2 = max(p.x * p.x + p.y * p.y, 1e-30);
        return vec2<f32>(p.x / r2, p.y / r2);
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_d_spherical(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let d_spher_weight = get_param(xform_id, variation_id, 0u);
    if (rng_nextf(rng) < d_spher_weight) {
        let r2 = max(p.x * p.x + p.y * p.y, 1e-30);
        return vec3<f32>(p.x / r2, p.y / r2, p.z);
    }
    return p;
}
"#,
};

// =============================================================================
// dustpoint: 3-point pivot/overlap IFS triangle (Jesus Sosa)
//   p_sign = ±1 from rand < 0.5
//   r = sqrt(x² + y²)
//   w  ∈ [0, 1):
//     < 0.50: (x/r - 1, p_sign · y/r)
//     < 0.75: (x/3,     y/3)
//     else:   (x/3 + 2/3, y/3)
// Clean factor through outer.
// =============================================================================
/// 3-point pivot/overlap IFS triangle — picks one of three sub-
/// transformations uniformly: a Y-sign-flipped polar pivot, a scale-to-
/// origin by 1/3, or a scale-by-1/3 with X offset of 2/3. Together these
/// implement an IFS that draws a Sierpinski-like dust pattern with a
/// circular pivot.
///
/// # Authors
/// - Jesus Sosa
pub static DUSTPOINT: VariationDef = VariationDef {
    name: "dustpoint",
    aliases: &[],
    display_name: "Dust Point",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_dustpoint(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let p_sign = select(-1.0, 1.0, rng_nextf(rng) < 0.5);
    let r = sqrt(max(p.x * p.x + p.y * p.y, 1e-30));
    let w = rng_nextf(rng);
    if (w < 0.5) {
        return vec2<f32>(p.x / r - 1.0, p_sign * p.y / r);
    } else if (w < 0.75) {
        return vec2<f32>(p.x / 3.0, p.y / 3.0);
    }
    return vec2<f32>(p.x / 3.0 + 2.0 / 3.0, p.y / 3.0);
}
"#,
    wgsl_3d: r#"
fn variation_dustpoint(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let p_sign = select(-1.0, 1.0, rng_nextf(rng) < 0.5);
    let r = sqrt(max(p.x * p.x + p.y * p.y, 1e-30));
    let w = rng_nextf(rng);
    if (w < 0.5) {
        return vec3<f32>(p.x / r - 1.0, p_sign * p.y / r, p.z);
    } else if (w < 0.75) {
        return vec3<f32>(p.x / 3.0, p.y / 3.0, p.z);
    }
    return vec3<f32>(p.x / 3.0 + 2.0 / 3.0, p.y / 3.0, p.z);
}
"#,
};

// =============================================================================
// deltaa: radial ratio + half angle difference (Faber)
//   avgr = w · sqrt(y² + (x+1)²) / sqrt(y² + (x-1)²)
//   avga = (atan2(y, x-1) - atan2(y, x+1)) / 2
//   out = avgr · (cos avga, sin avga)
// Body has w in `avgr` → needs_transform divide-out.
// =============================================================================
/// Radial ratio + half angle difference — emits `r · (cos a, sin a)` where
/// `r = sqrt(y² + (x+1)²) / sqrt(y² + (x−1)²)` (ratio of distances to two
/// foci at `(∓1, 0)`) and `a` is half the angular difference between the
/// focus-angles. Produces conformal-mapping-style patterns.
///
/// # Authors
/// - Michael Faber
pub static DELTAA: VariationDef = VariationDef {
    name: "deltaA",
    aliases: &[],
    display_name: "Delta A",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_deltaA(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let denom = sqrt(max(p.y * p.y + (p.x - 1.0) * (p.x - 1.0), 1e-30));
    let num = sqrt(max(p.y * p.y + (p.x + 1.0) * (p.x + 1.0), 1e-30));
    let avgr = w * (num / denom);
    let avga = (atan2(p.y, p.x - 1.0) - atan2(p.y, p.x + 1.0)) * 0.5;
    return vec2<f32>(avgr * cos(avga) * inv_w, avgr * sin(avga) * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_deltaA(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let denom = sqrt(max(p.y * p.y + (p.x - 1.0) * (p.x - 1.0), 1e-30));
    let num = sqrt(max(p.y * p.y + (p.x + 1.0) * (p.x + 1.0), 1e-30));
    let avgr = w * (num / denom);
    let avga = (atan2(p.y, p.x - 1.0) - atan2(p.y, p.x + 1.0)) * 0.5;
    return vec3<f32>(avgr * cos(avga) * inv_w, avgr * sin(avga) * inv_w, p.z);
}
"#,
};

// =============================================================================
// edisc: elliptic disc (Apophysis Plugin Pack)
//   tmp = x² + y² + 1; tmp2 = 2x
//   r1 = sqrt(tmp + tmp2);  r2 = sqrt(tmp - tmp2)
//   xmax = (r1 + r2) / 2
//   a1 = log(xmax + sqrt(xmax - 1));   a2 = -acos(x / xmax)
//   wlocal = w / 11.57034632
//   sn = sin a1; cs = cos a1; snh = sinh a2; csh = cosh a2
//   if y > 0: sn = -sn
//   out = wlocal · (csh · cs, snh · sn)
// Body has w in `wlocal` → needs_transform divide-out.
// (11.57034632 ≈ 4·acosh(2) — the Apophysis classic empirical scale.)
// =============================================================================
/// Elliptic disc — maps the input through an elliptic-coordinate
/// transformation: `xmax = (sqrt(r² + 2x + 1) + sqrt(r² − 2x + 1)) / 2`,
/// then emits `(wlocal · cosh(−acos(x/xmax)) · cos(log(xmax + sqrt(xmax² − 1))),
/// wlocal · sinh(−acos(x/xmax)) · sin(...))`. The `1/11.57 ≈ 1/(4·acosh(2))`
/// scaling is the empirical Apophysis normalization.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static EDISC: VariationDef = VariationDef {
    name: "edisc",
    aliases: &[],
    display_name: "E-Disc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_edisc(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let tmp = p.x * p.x + p.y * p.y + 1.0;
    let tmp2 = 2.0 * p.x;
    let r1 = sqrt(max(tmp + tmp2, 0.0));
    let r2 = sqrt(max(tmp - tmp2, 0.0));
    let xmax = (r1 + r2) * 0.5;
    let safe_xmax = max(xmax, 1.0);
    let a1 = log(safe_xmax + sqrt(max(safe_xmax - 1.0, 0.0)));
    let a2 = -acos(clamp(p.x / max(safe_xmax, 1e-30), -1.0, 1.0));
    let wlocal = w * 0.08643743168737844;  // 1/11.57034632

    var sn = sin(a1);
    let cs = cos(a1);
    let snh = sinh(a2);
    let csh = cosh(a2);
    if (p.y > 0.0) {
        sn = -sn;
    }
    return vec2<f32>(wlocal * csh * cs * inv_w, wlocal * snh * sn * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_edisc(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let tmp = p.x * p.x + p.y * p.y + 1.0;
    let tmp2 = 2.0 * p.x;
    let r1 = sqrt(max(tmp + tmp2, 0.0));
    let r2 = sqrt(max(tmp - tmp2, 0.0));
    let xmax = (r1 + r2) * 0.5;
    let safe_xmax = max(xmax, 1.0);
    let a1 = log(safe_xmax + sqrt(max(safe_xmax - 1.0, 0.0)));
    let a2 = -acos(clamp(p.x / max(safe_xmax, 1e-30), -1.0, 1.0));
    let wlocal = w * 0.08643743168737844;

    var sn = sin(a1);
    let cs = cos(a1);
    let snh = sinh(a2);
    let csh = cosh(a2);
    if (p.y > 0.0) {
        sn = -sn;
    }
    return vec3<f32>(wlocal * csh * cs * inv_w, wlocal * snh * sn * inv_w, p.z);
}
"#,
};

// =============================================================================
// curve: gaussian X/Y bumps (Apophysis Plugin Pack)
//   pc_xlen = max(xlength², 1e-20);  pc_ylen = max(ylength², 1e-20)
//   out_x = x + xamp · exp(-y² / pc_xlen)
//   out_y = y + yamp · exp(-x² / pc_ylen)
// Clean factor through outer.
// =============================================================================
/// Gaussian X/Y bumps — adds a gaussian bump `xamp · exp(−y²/xlength²)`
/// to the X output and `yamp · exp(−x²/ylength²)` to the Y output. The
/// bumps decay smoothly away from the X and Y axes respectively, producing
/// a soft cross-shaped distortion.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static CURVE: VariationDef = VariationDef {
    name: "curve",
    aliases: &[],
    display_name: "Curve",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("xamp", "X Amp", unlimited_float, 0.25, -10.0, 10.0, "X-axis gaussian bump amplitude."),
        param!("yamp", "Y Amp", unlimited_float, 0.5, -10.0, 10.0, "Y-axis gaussian bump amplitude."),
        param!("xlength", "X Length", unlimited_float, 1.0, -10.0, 10.0, "X-axis gaussian width — the Y-direction decay scale of the X bump."),
        param!("ylength", "Y Length", unlimited_float, 1.0, -10.0, 10.0, "Y-axis gaussian width — the X-direction decay scale of the Y bump."),
    ],
    // 2 derived values at slots 4..6:
    //   4: pc_xlen = max(xlength², 1e-20)
    //   5: pc_ylen = max(ylength², 1e-20)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_curve(user: array<f32, 4>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = max(user[2] * user[2], 1e-20);
    out[1] = max(user[3] * user[3], 1e-20);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_curve(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xamp = get_param(xform_id, variation_id, 0u);
    let yamp = get_param(xform_id, variation_id, 1u);
    let pc_xlen = get_param(xform_id, variation_id, 4u);
    let pc_ylen = get_param(xform_id, variation_id, 5u);

    let fx = p.x + xamp * exp(-p.y * p.y / pc_xlen);
    let fy = p.y + yamp * exp(-p.x * p.x / pc_ylen);
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_curve(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xamp = get_param(xform_id, variation_id, 0u);
    let yamp = get_param(xform_id, variation_id, 1u);
    let pc_xlen = get_param(xform_id, variation_id, 4u);
    let pc_ylen = get_param(xform_id, variation_id, 5u);

    let fx = p.x + xamp * exp(-p.y * p.y / pc_xlen);
    let fy = p.y + yamp * exp(-p.x * p.x / pc_ylen);
    return vec3<f32>(fx, fy, p.z);
}
"#,
};

// =============================================================================
// elliptic2: elliptic with knobs (Brad Stefanov)
//   11 user params: a1, a2, a3, b1, b2, c, d, e, f, g, h
//   1 init slot: _v = w · h / π
//   tmp = y² + x² + a1;   x2 = b1 · x
//   xmax = c · (sqrt(tmp + x2) + sqrt(tmp - x2))
//   a = x / xmax · a2;    b = sqrt_safe(d - a²) · b2
//   ps = -π/2 · a3
//   FPx += _v · atan2(a, b) + ps
//   FPy += ±_v · log(xmax + sqrt_safe(xmax - {f|g}))   (sign by rand < e)
// Body has internal w in `_v`; the `+ ps` term is an unweighted offset.
// needs_transform divide-out — `+ ps / w` carried so outer × w restores
// `+ ps`.
// =============================================================================
/// Elliptic warp with 11 knobs — heavily parameterized elliptic variation.
/// Computes `xmax = c·(sqrt(x²+y²+a1+b1·x) + sqrt(x²+y²+a1−b1·x))`, then
/// emits `(v·atan2(a, b) + ps, ±v·log(xmax + sqrt(xmax − f or g)))` with
/// the Y branch chosen by random vs `e`. The 11 parameters give fine-
/// grained control over the elliptic shape.
///
/// # Authors
/// - Brad Stefanov
pub static ELLIPTIC2: VariationDef = VariationDef {
    name: "elliptic2",
    aliases: &[],
    display_name: "Elliptic 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[
        param!("a1", "A1", unlimited_float, 1.0, -10.0, 10.0, "Constant offset added to `x² + y²` in the elliptic-radius formula."),
        param!("a2", "A2", unlimited_float, 1.0, -10.0, 10.0, "Scale on the `atan2` argument `a = (x/xmax) · a2`."),
        param!("a3", "A3", unlimited_float, 0.0, -10.0, 10.0, "Phase shift on the X output (scaled by −π/2)."),
        param!("b1", "B1", unlimited_float, 2.0, -10.0, 10.0, "Multiplier on x in the xmax sqrt-difference."),
        param!("b2", "B2", unlimited_float, 1.0, -10.0, 10.0, "Scale on `sqrt(d − a²)` in the b term."),
        param!("c", "C", unlimited_float, 0.5, -10.0, 10.0, "Overall scale on xmax."),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0, "Constant in the `sqrt(d − a²)` term."),
        param!("e", "E", unlimited_float, 0.5, 0.0, 1.0, "Probability threshold for the Y output sign."),
        param!("f", "F", unlimited_float, 1.0, -10.0, 10.0, "Sqrt offset for the positive-Y branch (`sqrt(xmax − f)`)."),
        param!("g", "G", unlimited_float, 1.0, -10.0, 10.0, "Sqrt offset for the negative-Y branch (`sqrt(xmax − g)`)."),
        param!("h", "H", unlimited_float, 2.0, -10.0, 10.0, "V-multiplier — output is scaled by `v = w · h / π`."),
    ],
    // 1 derived value at slot 11: _v_div_w = h / π  (so body computes
    // `_v = w · _v_div_w`)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_elliptic2(user: array<f32, 11>) -> array<f32, 1> {
    let inv_pi = 0.31830988618379;
    var out: array<f32, 1>;
    out[0] = user[10] * inv_pi;  // h / π
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_elliptic2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let a1 = get_param(xform_id, variation_id, 0u);
    let a2 = get_param(xform_id, variation_id, 1u);
    let a3 = get_param(xform_id, variation_id, 2u);
    let b1 = get_param(xform_id, variation_id, 3u);
    let b2 = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let d = get_param(xform_id, variation_id, 6u);
    let e = get_param(xform_id, variation_id, 7u);
    let f = get_param(xform_id, variation_id, 8u);
    let g = get_param(xform_id, variation_id, 9u);
    let v_div_w = get_param(xform_id, variation_id, 11u);
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let v = w * v_div_w;

    let tmp = p.y * p.y + p.x * p.x + a1;
    let x2 = b1 * p.x;
    let safe_xmax_arg_a = max(tmp + x2, 0.0);
    let safe_xmax_arg_b = max(tmp - x2, 0.0);
    let xmax = c * (sqrt(safe_xmax_arg_a) + sqrt(safe_xmax_arg_b));
    let safe_xmax = select(xmax, 1e-30, abs(xmax) < 1e-30);

    let a_val = p.x / safe_xmax * a2;
    let b_val = sqrt(max(d - a_val * a_val, 0.0)) * b2;
    let ps = -pi_2 * a3;
    let fx = v * atan2(a_val, b_val) + ps;

    var fy: f32;
    if (rng_nextf(rng) < e) {
        fy = v * log(max(xmax + sqrt(max(xmax - f, 0.0)), 1e-30));
    } else {
        fy = -v * log(max(xmax + sqrt(max(xmax - g, 0.0)), 1e-30));
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_elliptic2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let a1 = get_param(xform_id, variation_id, 0u);
    let a2 = get_param(xform_id, variation_id, 1u);
    let a3 = get_param(xform_id, variation_id, 2u);
    let b1 = get_param(xform_id, variation_id, 3u);
    let b2 = get_param(xform_id, variation_id, 4u);
    let c = get_param(xform_id, variation_id, 5u);
    let d = get_param(xform_id, variation_id, 6u);
    let e = get_param(xform_id, variation_id, 7u);
    let f = get_param(xform_id, variation_id, 8u);
    let g = get_param(xform_id, variation_id, 9u);
    let v_div_w = get_param(xform_id, variation_id, 11u);
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let v = w * v_div_w;

    let tmp = p.y * p.y + p.x * p.x + a1;
    let x2 = b1 * p.x;
    let safe_xmax_arg_a = max(tmp + x2, 0.0);
    let safe_xmax_arg_b = max(tmp - x2, 0.0);
    let xmax = c * (sqrt(safe_xmax_arg_a) + sqrt(safe_xmax_arg_b));
    let safe_xmax = select(xmax, 1e-30, abs(xmax) < 1e-30);

    let a_val = p.x / safe_xmax * a2;
    let b_val = sqrt(max(d - a_val * a_val, 0.0)) * b2;
    let ps = -pi_2 * a3;
    let fx = v * atan2(a_val, b_val) + ps;

    var fy: f32;
    if (rng_nextf(rng) < e) {
        fy = v * log(max(xmax + sqrt(max(xmax - f, 0.0)), 1e-30));
    } else {
        fy = -v * log(max(xmax + sqrt(max(xmax - g, 0.0)), 1e-30));
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};
