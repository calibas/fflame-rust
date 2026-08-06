//! Apophysis miscellany 7 — five more
//!
//!   - `asteria`    (dark-beam)         — RNG-blended linear/asteria;
//!                                          1 user param (alpha) + 2 init
//!                                          slots (sina, cosa); body
//!                                          uses `x0 = w·x, y0 = w·y`
//!                                          throughout — needs_transform
//!                                          divide-out
//!   - `estiq`      (zephyrtronium)     — quaternion exponential
//!                                          extension; Full3D; clean
//!   - `fdisc`      (CozyG)             — fractal disc; 8 user params
//!                                          recovered from Java
//!                                          (cpp APO_VARIABLES empty —
//!                                          porter omitted them all
//!                                          while leaving `_ashift,
//!                                          _rshift, _xshift, _yshift,
//!                                          _term1..4` as struct
//!                                          fields). Clean factor
//!                                          through outer
//!   - `bTransform` (Faber)             — bipolar transform; 4 user
//!                                          params (rotate, power int
//!                                          ≥ 1, move, split); RNG;
//!                                          clean. Rand integer
//!                                          modulo replaced with
//!                                          `floor(rand · power)`
//!                                          (semantically equivalent
//!                                          uniform N-th-root branch)
//!   - `nPolar`                         — N-power polar; 2 user
//!                                          (parity int, n int) + 4
//!                                          init slots (nnz, isodd,
//!                                          absn, cn). Body has
//!                                          internal `vvar = w/π` and
//!                                          `vvar_2 = vvar/2` factors
//!                                          plus a final `r = w · …`;
//!                                          needs_transform divide-out
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// asteria (dark-beam)
//   x0 = w·x; y0 = w·y; r = x0² + y0²
//   xx = (|x0|-1)²; yy = (|y0|-1)²; r2 = sqrt(yy + xx)
//   in1 = (r < 1); out2 = (r2 < 1)
//   if in1 && out2: in1 = (rand > 0.35) else in1 = !in1
//   if in1: out = (x0, y0)                       (linear branch)
//   else:  rotate by α, do squareroot bend, rotate back  (asteria branch)
// Body uses w internally throughout — needs_transform divide-out.
// =============================================================================
/// RNG-blended linear/asteria warp — splits behavior based on whether the
/// input falls in a star-shaped region (inside the unit circle, with
/// corners cut out by unit-discs around the four corners `(±1, ±1)`).
/// Inside, linear pass-through dominates (with 35% chance of falling
/// through to the asteria branch). Outside, applies a square-root-bend warp
/// rotated by `alpha`.
///
/// # Authors
/// - DarkBeam
pub static ASTERIA: VariationDef = VariationDef {
    name: "asteria",
    aliases: &[],
    display_name: "Asteria",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[
        param!("alpha", "Alpha", unlimited_float, 0.0, -10.0, 10.0, "Rotation angle (× π) applied before and inverted after the asteria branch's square-root bend."),
    ],
    // 2 derived values at slots 1..3:
    //   1: sina = sin(π · alpha)
    //   2: cosa = cos(π · alpha)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_asteria(user: array<f32, 1>) -> array<f32, 2> {
    let pi = 3.14159265358979;
    var out: array<f32, 2>;
    out[0] = sin(pi * user[0]);
    out[1] = cos(pi * user[0]);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_asteria(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let sina = get_param(xform_id, variation_id, 1u);
    let cosa = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x0 = w * p.x;
    let y0 = w * p.y;
    let r = x0 * x0 + y0 * y0;
    var xx = abs(x0) - 1.0;
    xx = xx * xx;
    var yy = abs(y0) - 1.0;
    yy = yy * yy;
    let r2 = sqrt(xx + yy);
    var in1 = r < 1.0;
    let out2 = r2 < 1.0;
    if (in1 && out2) {
        in1 = rng_nextf(rng) > 0.35;
    } else {
        in1 = !in1;
    }

    var fx: f32;
    var fy: f32;
    if (in1) {
        fx = x0;
        fy = y0;
    } else {
        let xr = x0 * cosa - y0 * sina;
        let yr = x0 * sina + y0 * cosa;
        let inner = max(1.0 - yr * yr, 0.0);
        let safe_inner = select(sqrt(inner), 1e-30, inner < 1e-30);
        let bend = 1.0 - sqrt(max(1.0 - (1.0 - abs(yr)) * (1.0 - abs(yr)), 0.0));
        let nx = xr / safe_inner * bend;
        fx = nx * cosa + yr * sina;
        fy = -nx * sina + yr * cosa;
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_asteria(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let sina = get_param(xform_id, variation_id, 1u);
    let cosa = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x0 = w * p.x;
    let y0 = w * p.y;
    let r = x0 * x0 + y0 * y0;
    var xx = abs(x0) - 1.0;
    xx = xx * xx;
    var yy = abs(y0) - 1.0;
    yy = yy * yy;
    let r2 = sqrt(xx + yy);
    var in1 = r < 1.0;
    let out2 = r2 < 1.0;
    if (in1 && out2) {
        in1 = rng_nextf(rng) > 0.35;
    } else {
        in1 = !in1;
    }

    var fx: f32;
    var fy: f32;
    if (in1) {
        fx = x0;
        fy = y0;
    } else {
        let xr = x0 * cosa - y0 * sina;
        let yr = x0 * sina + y0 * cosa;
        let inner = max(1.0 - yr * yr, 0.0);
        let safe_inner = select(sqrt(inner), 1e-30, inner < 1e-30);
        let bend = 1.0 - sqrt(max(1.0 - (1.0 - abs(yr)) * (1.0 - abs(yr)), 0.0));
        let nx = xr / safe_inner * bend;
        fx = nx * cosa + yr * sina;
        fy = -nx * sina + yr * cosa;
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// estiq (zephyrtronium)
//   e = exp(x);  abs_v = sqrt(y² + z²)
//   s = sin(abs_v);  c = cos(abs_v)
//   a = e · s / abs_v
//   out = (e · c, a · y, a · z)
// Clean factor through outer; Full3D.
// (In 2D mode `abs_v = |y|`; `a · y = e · sin(|y|) · sign(y)`;
//  `e · c = e · cos(|y|) = e · cos(y)`. Reasonable degenerate.)
// =============================================================================
/// Quaternion exponential extension — emits `(e^x · cos|v|, e^x ·
/// sin|v|/|v| · y, e^x · sin|v|/|v| · z)` where `|v| = sqrt(y² + z²)`.
/// Generalizes the complex exponential `(x + iy → e^x · (cos y, sin y))` to
/// a 3D quaternion-like form.
///
/// # Authors
/// - zephyrtronium
pub static ESTIQ: VariationDef = VariationDef {
    name: "estiq",
    aliases: &[],
    display_name: "Estiq",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_estiq(p: vec2<f32>) -> vec2<f32> {
    let e = exp(p.x);
    let abs_v = max(abs(p.y), 1e-30);
    let s = sin(abs_v);
    let c = cos(abs_v);
    let a = e * s / abs_v;
    return vec2<f32>(e * c, a * p.y);
}
"#,
    wgsl_3d: r#"
fn variation_estiq(p: vec3<f32>) -> vec3<f32> {
    let e = exp(p.x);
    let abs_v = max(sqrt(p.y * p.y + p.z * p.z), 1e-30);
    let s = sin(abs_v);
    let c = cos(abs_v);
    let a = e * s / abs_v;
    return vec3<f32>(e * c, a * p.y, a * p.z);
}
"#,
};

// =============================================================================
// fdisc (CozyG; original from Apophysis7X plugin)
//   afactor = 2π / (sqrt(x² + y²) + ε + ashift)
//   r = (atan2(y, x)/π + rshift) / 2
//   xfactor = cos(afactor + xshift);  yfactor = sin(afactor + yshift)
//   pr = w · r;  prx = pr · xfactor;  pry = pr · yfactor
//   out_x = term1·prx + term2·x·prx + term3·x·pr + term4·x
//   out_y = term1·pry + term2·y·pry + term3·y·pr + term4·y
// Clean factor through outer.
// (cpp APO_VARIABLES empty — porter omitted all 8 params. Recovered
// from Java setParameter.)
// =============================================================================
/// Fractal disc — combines an inverse-radius cosine wave `cos(2π / (r +
/// ashift) + xshift)`, an angular term `(atan2(y,x)/π + rshift) / 2`, and
/// four blending weights. Each `term_i` controls a different contribution
/// to the output (pure-prx, x-scaled-prx, x-scaled-pr, pass-through),
/// producing rich layered disc patterns.
///
/// # Authors
/// - CozyG
pub static FDISC: VariationDef = VariationDef {
    name: "fdisc",
    aliases: &[],
    display_name: "F-Disc",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("ashift", "A shift", unlimited_float, 1.0, -10.0, 10.0, "Radial denominator offset added to `sqrt(x² + y²)`."),
        param!("rshift", "R shift", unlimited_float, 1.0, -10.0, 10.0, "Angular offset added to `atan2(y, x) / π`."),
        param!("xshift", "X shift", unlimited_float, 0.0, -10.0, 10.0, "Phase offset on the X cosine wave."),
        param!("yshift", "Y shift", unlimited_float, 0.0, -10.0, 10.0, "Phase offset on the Y sine wave."),
        param!("term1", "Term 1", unlimited_float, 1.0, -10.0, 10.0, "Weight on the pure `prx`/`pry` (radius × axis-wave) contribution."),
        param!("term2", "Term 2", unlimited_float, 0.0, -10.0, 10.0, "Weight on the input-scaled `x·prx`/`y·pry` contribution."),
        param!("term3", "Term 3", unlimited_float, 0.0, -10.0, 10.0, "Weight on the `x·pr`/`y·pr` (input-scaled radius) contribution."),
        param!("term4", "Term 4", unlimited_float, 0.0, -10.0, 10.0, "Weight on the pass-through `x`/`y` contribution."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_fdisc(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let ashift = get_param(xform_id, variation_id, 0u);
    let rshift = get_param(xform_id, variation_id, 1u);
    let xshift = get_param(xform_id, variation_id, 2u);
    let yshift = get_param(xform_id, variation_id, 3u);
    let term1 = get_param(xform_id, variation_id, 4u);
    let term2 = get_param(xform_id, variation_id, 5u);
    let term3 = get_param(xform_id, variation_id, 6u);
    let term4 = get_param(xform_id, variation_id, 7u);
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;

    let s = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let afactor = two_pi / (s + ashift);
    let r = (atan2(p.y, p.x) * inv_pi + rshift) * 0.5;
    let xfactor = cos(afactor + xshift);
    let yfactor = sin(afactor + yshift);
    let pr = r;
    let prx = pr * xfactor;
    let pry = pr * yfactor;
    let fx = term1 * prx + term2 * p.x * prx + term3 * p.x * pr + term4 * p.x;
    let fy = term1 * pry + term2 * p.y * pry + term3 * p.y * pr + term4 * p.y;
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_fdisc(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let ashift = get_param(xform_id, variation_id, 0u);
    let rshift = get_param(xform_id, variation_id, 1u);
    let xshift = get_param(xform_id, variation_id, 2u);
    let yshift = get_param(xform_id, variation_id, 3u);
    let term1 = get_param(xform_id, variation_id, 4u);
    let term2 = get_param(xform_id, variation_id, 5u);
    let term3 = get_param(xform_id, variation_id, 6u);
    let term4 = get_param(xform_id, variation_id, 7u);
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;

    let s = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let afactor = two_pi / (s + ashift);
    let r = (atan2(p.y, p.x) * inv_pi + rshift) * 0.5;
    let xfactor = cos(afactor + xshift);
    let yfactor = sin(afactor + yshift);
    let pr = r;
    let prx = pr * xfactor;
    let pry = pr * yfactor;
    let fx = term1 * prx + term2 * p.x * prx + term3 * p.x * pr + term4 * p.x;
    let fy = term1 * pry + term2 * p.y * pry + term3 * p.y * pr + term4 * p.y;
    return vec3<f32>(fx, fy, p.z);
}
"#,
};

// =============================================================================
// bTransform (Faber)
//   tau = log(...) / 2 / power + move    (or ± split)
//   sigma = π - atan2(y, x+1) - atan2(y, 1-x) + rotate
//   sigma = sigma / power + 2π/power · floor(rand · power)
//   sinht = sinh(tau);  cosht = cosh(tau)
//   sins = sin(sigma);  coss = cos(sigma)
//   temp = cosht - coss
//   out = (sinht / temp, sins / temp)
// Clean factor through outer; needs_rng for the discrete-N branch.
// =============================================================================
/// Bipolar transform — extends the bipolar-coords mapping `(τ, σ)` with
/// adjustable `power`, `move`, `rotate`, and `split` (asymmetric τ offset
/// based on the sign of x). Picks a random angular branch from `floor(rand
/// · power)`, producing a generalized bipolar warp with N-fold rotational
/// structure.
///
/// # Authors
/// - Michael Faber
pub static BTRANSFORM: VariationDef = VariationDef {
    name: "bTransform",
    aliases: &[],
    display_name: "B-Transform",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("rotate", "Rotate", unlimited_float, 0.0, -10.0, 10.0, "Additive rotation on the bipolar σ coordinate."),
        param!("power", "Power", int, 1.0, 1.0, 100.0, "Number of angular branches (≥ 1)."),
        param!("move", "Move", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on τ."),
        param!("split", "Split", unlimited_float, 0.0, -10.0, 10.0, "Asymmetric τ offset: added when x ≥ 0, subtracted when x < 0."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bTransform(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let rotate = get_param(xform_id, variation_id, 0u);
    let power = max(get_param(xform_id, variation_id, 1u), 1.0);
    let move_p = get_param(xform_id, variation_id, 2u);
    let split = get_param(xform_id, variation_id, 3u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let xp1 = p.x + 1.0;
    let xm1 = p.x - 1.0;
    let arg_p = max(xp1 * xp1 + p.y * p.y, 1e-30);
    let arg_m = max(xm1 * xm1 + p.y * p.y, 1e-30);
    var tau = 0.5 * (log(arg_p) - log(arg_m)) / power + move_p;
    var sigma = pi - atan2(p.y, xp1) - atan2(p.y, 1.0 - p.x) + rotate;
    sigma = sigma / power + two_pi / power * floor(rng_nextf(rng) * power);

    if (p.x >= 0.0) {
        tau = tau + split;
    } else {
        tau = tau - split;
    }

    let sinht = sinh(tau);
    let cosht = cosh(tau);
    let sins = sin(sigma);
    let coss = cos(sigma);
    let temp = cosht - coss;
    let safe_temp = select(temp, 1e-30, abs(temp) < 1e-30);
    return vec2<f32>(sinht / safe_temp, sins / safe_temp);
}
"#,
    wgsl_3d: r#"
fn variation_bTransform(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let rotate = get_param(xform_id, variation_id, 0u);
    let power = max(get_param(xform_id, variation_id, 1u), 1.0);
    let move_p = get_param(xform_id, variation_id, 2u);
    let split = get_param(xform_id, variation_id, 3u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let xp1 = p.x + 1.0;
    let xm1 = p.x - 1.0;
    let arg_p = max(xp1 * xp1 + p.y * p.y, 1e-30);
    let arg_m = max(xm1 * xm1 + p.y * p.y, 1e-30);
    var tau = 0.5 * (log(arg_p) - log(arg_m)) / power + move_p;
    var sigma = pi - atan2(p.y, xp1) - atan2(p.y, 1.0 - p.x) + rotate;
    sigma = sigma / power + two_pi / power * floor(rng_nextf(rng) * power);

    if (p.x >= 0.0) {
        tau = tau + split;
    } else {
        tau = tau - split;
    }

    let sinht = sinh(tau);
    let cosht = cosh(tau);
    let sins = sin(sigma);
    let coss = cos(sigma);
    let temp = cosht - coss;
    let safe_temp = select(temp, 1e-30, abs(temp) < 1e-30);
    return vec3<f32>(sinht / safe_temp, sins / safe_temp, p.z);
}
"#,
};

// =============================================================================
// nPolar 
//   2 user params: parity (int), n (int)
//   4 init slots: nnz = (n==0)?1:n;  isodd = |parity| mod 2;
//                 absn = |nnz|;       cn = 1/(2·nnz)
//   At runtime: vvar = w/π;  vvar_2 = vvar/2
//   if isodd:    x = X;        y = Y
//   else:        x = vvar·atan2(X, Y);  y = vvar_2·log(X² + Y²)
//   angle = (atan2(y, x) + 2π·floor(rand·absn)) / nnz
//   r = w · pow(x² + y², cn) · (isodd ? parity : 1)
//   cosa = cos(angle)·r;  sina = sin(angle)·r
//   if isodd:    out = (cosa, sina)
//   else:        out = (vvar_2·log(cos² + sin²),  vvar·atan2(cos, sin))
// Body has internal `vvar = w/π` and `r = w · …` factors —
// needs_transform divide-out.
//
// Note: cpp uses `atan2(FTx, FTy)` (swapped from Java's
// `atan2(y, x)`) in the `else` branch (line 67: `vvar · atan2(FTx,
// FTy)`). The closing-form `atan2(cosa, sina)` (line 76) is also the
// cpp swap. Preserved for parity with cpp output.
// =============================================================================
/// N-power polar warp with parity-based branch selection — even-parity
/// (`|parity|` even) routes through a log-polar mid-step before the angular
/// power; odd-parity stays in cartesian. A random `floor(rand · |n|)`
/// selects one of `|n|` angular branches.
pub static NPOLAR: VariationDef = VariationDef {
    name: "npolar",
    aliases: &[],
    display_name: "N-Polar",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[
        param!("parity", "Parity", int, 0.0, -100.0, 100.0, "Parity selector. `|parity| mod 2` chooses the branch (even = log-polar mid-step, odd = cartesian); the value itself also flips the sign of the even-branch's radial magnitude."),
        param!("n", "N", int, 1.0, -100.0, 100.0, "Angular branch count. 0 is forced to 1 internally; negative values use `|n|` branches."),
    ],
    // 4 derived values at slots 2..6:
    //   2: nnz   (n==0 ? 1 : n)
    //   3: isodd (|parity| mod 2)
    //   4: absn  (|nnz|)
    //   5: cn    (1 / (2·nnz))
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_npolar(user: array<f32, 2>) -> array<f32, 4> {
    let parity_i = i32(user[0]);
    let n_i = i32(user[1]);
    let nnz_i = select(n_i, 1, n_i == 0);
    let isodd_i = abs(parity_i) - (abs(parity_i) / 2) * 2;
    let absn_i = abs(nnz_i);
    var out: array<f32, 4>;
    out[0] = f32(nnz_i);
    out[1] = f32(isodd_i);
    out[2] = f32(absn_i);
    out[3] = 1.0 / (2.0 * f32(nnz_i));
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// atan2(0,0) is pi/4 under Metal fast-math, and npolar reaches the
// origin on EVERY call at its default parity (parity_factor = 0 makes
// cosa = sina = 0, a constant map). ff_atan2 (utilities.wgsl) is the
// IEEE-exact guard.
fn variation_npolar(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let parity = get_param(xform_id, variation_id, 0u);
    let nnz = get_param(xform_id, variation_id, 2u);
    let isodd = get_param(xform_id, variation_id, 3u);
    let absn = get_param(xform_id, variation_id, 4u);
    let cn = get_param(xform_id, variation_id, 5u);
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let vvar = w * inv_pi;
    let vvar_2 = vvar * 0.5;

    var x: f32;
    var y: f32;
    if (isodd != 0.0) {
        x = p.x;
        y = p.y;
    } else {
        x = vvar * ff_atan2(p.x, p.y);  // cpp swap (Java uses atan2(y,x))
        y = vvar_2 * log(max(p.x * p.x + p.y * p.y, 1e-30));
    }
    let angle = (ff_atan2(y, x) + two_pi * floor(rng_nextf(rng) * absn)) / nnz;
    let radial = pow(max(x * x + y * y, 1e-30), cn);
    let parity_factor = select(1.0, parity, isodd == 0.0);
    let r = w * radial * parity_factor;
    var cosa = cos(angle) * r;
    var sina = sin(angle) * r;
    var fx: f32;
    var fy: f32;
    if (isodd != 0.0) {
        fx = cosa;
        fy = sina;
    } else {
        fx = vvar_2 * log(max(cosa * cosa + sina * sina, 1e-30));
        fy = vvar * ff_atan2(cosa, sina);  // cpp swap preserved
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
// atan2(0,0) is pi/4 under Metal fast-math, and npolar reaches the
// origin on EVERY call at its default parity (parity_factor = 0 makes
// cosa = sina = 0, a constant map). ff_atan2 (utilities.wgsl) is the
// IEEE-exact guard.
fn variation_npolar(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let parity = get_param(xform_id, variation_id, 0u);
    let nnz = get_param(xform_id, variation_id, 2u);
    let isodd = get_param(xform_id, variation_id, 3u);
    let absn = get_param(xform_id, variation_id, 4u);
    let cn = get_param(xform_id, variation_id, 5u);
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let vvar = w * inv_pi;
    let vvar_2 = vvar * 0.5;

    var x: f32;
    var y: f32;
    if (isodd != 0.0) {
        x = p.x;
        y = p.y;
    } else {
        x = vvar * ff_atan2(p.x, p.y);
        y = vvar_2 * log(max(p.x * p.x + p.y * p.y, 1e-30));
    }
    let angle = (ff_atan2(y, x) + two_pi * floor(rng_nextf(rng) * absn)) / nnz;
    let radial = pow(max(x * x + y * y, 1e-30), cn);
    let parity_factor = select(1.0, parity, isodd == 0.0);
    let r = w * radial * parity_factor;
    var cosa = cos(angle) * r;
    var sina = sin(angle) * r;
    var fx: f32;
    var fy: f32;
    if (isodd != 0.0) {
        fx = cosa;
        fy = sina;
    } else {
        fx = vvar_2 * log(max(cosa * cosa + sina * sina, 1e-30));
        fy = vvar * ff_atan2(cosa, sina);
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};
