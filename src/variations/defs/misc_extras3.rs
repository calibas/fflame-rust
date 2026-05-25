//! Miscellaneous misc — third extras file
//!
//! Four more standalone variations:
//!
//!   - `oscilloscope2` (Apophysis pack, DarkBeam tweak) — 2D extension
//!     of `oscilloscope` with frequencies on both axes plus a Y-driven
//!     X-perturbation
//!   - `lineart`       (FractalDesire)                  — 2D version of
//!     `lineart3d` (sign-preserving per-axis power)
//!   - `phoenix_julia` (TyrantWave)                     — Phoenix-Julia
//!     N-th-root sampler with X/Y distortion
//!   - `pow_block`     (cothe / DarkBeam)                — generalized
//!     N-th root of (x²+y²)^p with random branch
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! All factor VVAR cleanly through the outer multiplier.
//!
//! Skipped from the originally-considered set:
//!   - `arctruchet` — uses a malloc'd `_tiltArray` of per-thread persistent
//!     state plus `PluginVarTerminate` to free it. Same architectural
//!     blocker as `farblur`/`exblur`/`nblur`.
//!   - `circleTrans1` — do-while loop + hash-noise rejection sampling.
//!   - `lazyjess` / `lazytravis` — use VVAR as a comparison threshold
//!     with multi-branch body. Tractable via needs_transform, but better
//!     as their own focused batch (the body is long).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// oscilloscope2: 2D oscilloscope with Y-perturbation (Apophysis pack, DarkBeam)
//   tpf  = 2π · frequencyx
//   tpf2 = 2π · frequencyy
//   pt = perturbation · sin(tpf2 · y)
//   t = (no_damping ? amplitude · cos(tpf·x + pt)
//                   : amplitude · exp(−|x|·damping) · cos(tpf·x + pt))
//       + separation
//   if |y| ≤ t: out = (-x, -y)         (note: BOTH X and Y flipped, vs
//                                       oscilloscope which only flips Y)
//   else:        out = (x, y)
// =============================================================================
/// 2D extension of `oscilloscope` — adds a Y-driven sinusoidal perturbation
/// `perturbation · sin(2π·freqy·y)` to the X-axis cosine argument, and
/// flips BOTH X and Y when the input falls inside the band (vs
/// `oscilloscope`, which only flips Y). Produces a 2D oscilloscope-trace
/// masking pattern.
///
/// # Authors
/// - Apophysis Plugin Pack
/// - DarkBeam
pub static OSCILLOSCOPE2: VariationDef = VariationDef {
    name: "oscilloscope2",
    display_name: "Oscilloscope 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("separation", "Separation", unlimited_float, 1.0, -10.0, 10.0, "Vertical offset of the band threshold."),
        param!("frequencyx", "Frequency X", unlimited_float, 3.14159265, -100.0, 100.0, "X-axis cosine frequency (multiplied by 2π internally)."),
        param!("frequencyy", "Frequency Y", unlimited_float, 3.14159265, -100.0, 100.0, "Y-axis sine frequency for the perturbation term (multiplied by 2π)."),
        param!("amplitude", "Amplitude", unlimited_float, 1.0, -10.0, 10.0, "Cosine amplitude."),
        param!("perturbation", "Perturbation", unlimited_float, 1.0, -10.0, 10.0, "Magnitude of the Y-driven X-perturbation."),
        param!("damping", "Damping", unlimited_float, 0.0, -10.0, 10.0, "Exponential damping rate. Values near zero disable the damping term."),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 6..8:
    //   6: tpf   (2π · frequencyx)
    //   7: tpf2  (2π · frequencyy)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_oscilloscope2(user: array<f32, 6>) -> array<f32, 2> {
    let two_pi = 6.28318530717959;
    var out: array<f32, 2>;
    out[0] = two_pi * user[1];
    out[1] = two_pi * user[2];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_oscilloscope2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let separation = get_param(xform_id, variation_id, 0u);
    let amplitude = get_param(xform_id, variation_id, 3u);
    let perturbation = get_param(xform_id, variation_id, 4u);
    let damping = get_param(xform_id, variation_id, 5u);
    let tpf = get_param(xform_id, variation_id, 6u);
    let tpf2 = get_param(xform_id, variation_id, 7u);
    let no_damping = abs(damping) <= 1e-6;
    let pt = perturbation * sin(tpf2 * p.y);
    var t: f32;
    if (no_damping) {
        t = amplitude * cos(tpf * p.x + pt) + separation;
    } else {
        t = amplitude * exp(-abs(p.x) * damping) * cos(tpf * p.x + pt) + separation;
    }
    if (abs(p.y) <= t) {
        return vec2<f32>(-p.x, -p.y);
    }
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_oscilloscope2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let separation = get_param(xform_id, variation_id, 0u);
    let amplitude = get_param(xform_id, variation_id, 3u);
    let perturbation = get_param(xform_id, variation_id, 4u);
    let damping = get_param(xform_id, variation_id, 5u);
    let tpf = get_param(xform_id, variation_id, 6u);
    let tpf2 = get_param(xform_id, variation_id, 7u);
    let no_damping = abs(damping) <= 1e-6;
    let pt = perturbation * sin(tpf2 * p.y);
    var t: f32;
    if (no_damping) {
        t = amplitude * cos(tpf * p.x + pt) + separation;
    } else {
        t = amplitude * exp(-abs(p.x) * damping) * cos(tpf * p.x + pt) + separation;
    }
    if (abs(p.y) <= t) {
        return vec3<f32>(-p.x, -p.y, p.z);
    }
    return p;
}
"#),
};

// =============================================================================
// lineart: 2D per-axis sign-preserving power (FractalDesire)
//   out = (sign(x) · |x|^powX, sign(y) · |y|^powY)
//
// (cpp filename is `lineart` but APO_PLUGIN string is `linearT` — porter
// naming inconsistency; same lowercase form as `lineart3d` in our
// registry.)
// =============================================================================
/// Per-axis sign-preserving power (2D) — applies `sign(coord) ·
/// |coord|^pow` to each of X and Y independently, with separate exponents
/// per axis. The 2D analogue of `lineart3d`.
///
/// # Authors
/// - FractalDesire
pub static LINEART: VariationDef = VariationDef {
    name: "lineart",
    display_name: "Line Art",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("powX", "Power X", unlimited_float, 1.2, -10.0, 10.0, "X-axis power exponent."),
        param!("powY", "Power Y", unlimited_float, 1.2, -10.0, 10.0, "Y-axis power exponent."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_lineart(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let powx = get_param(xform_id, variation_id, 0u);
    let powy = get_param(xform_id, variation_id, 1u);
    let sx = select(sign(p.x), 1.0, p.x == 0.0);
    let sy = select(sign(p.y), 1.0, p.y == 0.0);
    return vec2<f32>(
        sx * pow(max(abs(p.x), 1e-30), powx),
        sy * pow(max(abs(p.y), 1e-30), powy),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_lineart(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let powx = get_param(xform_id, variation_id, 0u);
    let powy = get_param(xform_id, variation_id, 1u);
    let sx = select(sign(p.x), 1.0, p.x == 0.0);
    let sy = select(sign(p.y), 1.0, p.y == 0.0);
    return vec3<f32>(
        sx * pow(max(abs(p.x), 1e-30), powx),
        sy * pow(max(abs(p.y), 1e-30), powy),
        p.z,
    );
}
"#),
};

// =============================================================================
// phoenix_julia: Phoenix-Julia N-th-root sampler with X/Y distortion (TyrantWave)
//   pre = (x · (x_distort + 1), y · (y_distort + 1))
//   a   = atan2(pre.y, pre.x) · invN + n · inv_2π_N      (n = floor(rand·power))
//   r   = (x² + y²)^cN                                    (cN = dist/(2·power))
//   out = r · (cos a, sin a)
// =============================================================================
/// Phoenix-Julia N-th-root sampler with X/Y distortion — distorts the input
/// by `(1 + x_distort, 1 + y_distort)`, scales the resulting angle by
/// `1/power`, adds a random branch `n · 2π/power` (with `n` random from `0`
/// to `power − 1`), and emits at radius `r^(dist/power)` in the new angular
/// direction.
///
/// # Authors
/// - TyrantWave
pub static PHOENIX_JULIA: VariationDef = VariationDef {
    name: "phoenix_julia",
    display_name: "Phoenix Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", unlimited_float, 3.0, -50.0, 50.0, "Number of angular branches — `floor(rand · power)` picks the branch each iteration."),
        param!("dist", "Distance", unlimited_float, 1.0, -10.0, 10.0, "Radial-power exponent — output magnitude is `r^(dist/power)`."),
        param!("x_distort", "X distort", unlimited_float, -0.5, -10.0, 10.0, "X-axis pre-rotation distortion (multiplier on X before angle computation, offset by +1)."),
        param!("y_distort", "Y distort", unlimited_float, 0.0, -10.0, 10.0, "Y-axis pre-rotation distortion (offset by +1)."),
    ],
    needs_transform: false,
    writes_color: false,
    // 3 derived values at slots 4..7:
    //   4: invN       (dist / power)
    //   5: inv_2pi_N  (2π / power)
    //   6: cN         (dist / power / 2)
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_phoenix_julia(user: array<f32, 4>) -> array<f32, 3> {
    let power = user[0];
    let dist = user[1];
    let safe_power = select(power, 1e-30, power == 0.0);
    let two_pi = 6.28318530717959;
    var out: array<f32, 3>;
    out[0] = dist / safe_power;
    out[1] = two_pi / safe_power;
    out[2] = dist / safe_power * 0.5;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_phoenix_julia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let x_distort = get_param(xform_id, variation_id, 2u);
    let y_distort = get_param(xform_id, variation_id, 3u);
    let invn = get_param(xform_id, variation_id, 4u);
    let inv_2pi_n = get_param(xform_id, variation_id, 5u);
    let cn = get_param(xform_id, variation_id, 6u);

    let pre_x = p.x * (x_distort + 1.0);
    let pre_y = p.y * (y_distort + 1.0);
    let n = floor(rng_nextf(rng) * power);
    let a = atan2(pre_y, pre_x) * invn + n * inv_2pi_n;
    let r = pow(max(p.x * p.x + p.y * p.y, 1e-30), cn);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_phoenix_julia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let x_distort = get_param(xform_id, variation_id, 2u);
    let y_distort = get_param(xform_id, variation_id, 3u);
    let invn = get_param(xform_id, variation_id, 4u);
    let inv_2pi_n = get_param(xform_id, variation_id, 5u);
    let cn = get_param(xform_id, variation_id, 6u);

    let pre_x = p.x * (x_distort + 1.0);
    let pre_y = p.y * (y_distort + 1.0);
    let n = floor(rng_nextf(rng) * power);
    let a = atan2(pre_y, pre_x) * invn + n * inv_2pi_n;
    let r = pow(max(p.x * p.x + p.y * p.y, 1e-30), cn);
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// pow_block: generalized N-th root of (x²+y²)^p with random branch (cothe / DarkBeam)
//   _power = numerator·0.5 / (denominator · correctn / |correctd|)
//   _deneps = 1 / denominator
//   theta = atan2(x, y)        (upstream cpp swap)
//   r2 = (x² + y²)^_power · w
//   ran = (theta · _deneps + root · 2π · floor(rand · denominator) · _deneps) · numerator
//   out = r2 · (cos ran, sin ran)
// =============================================================================
/// Generalized N-th root of `(x²+y²)^p` with random branch — computes the
/// input angle, scales it by `1/denominator`, adds a random `root · 2π ·
/// k/denominator` branch (with `k` random from `0` to `denominator − 1`),
/// multiplies the whole angle by `numerator`, and emits at radius governed
/// by `numerator / (2·denominator) · (correctd/correctn)`.
///
/// # Authors
/// - cothe
/// - DarkBeam
pub static POW_BLOCK: VariationDef = VariationDef {
    name: "pow_block",
    display_name: "Pow Block",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("numerator", "Numerator", unlimited_float, 3.0, -50.0, 50.0, "Output-angle multiplier and effective power numerator."),
        param!("denominator", "Denominator", unlimited_float, 2.0, -50.0, 50.0, "Branch count — `floor(rand · denominator)` picks the branch."),
        param!("correctn", "Correct N", unlimited_float, 1.0, -10.0, 10.0, "Power-correction numerator (divides the effective exponent)."),
        param!("correctd", "Correct D", unlimited_float, 1.0, -10.0, 10.0, "Power-correction denominator (multiplies the effective exponent)."),
        param!("root", "Root", unlimited_float, 1.0, -10.0, 10.0, "Per-branch angular offset multiplier (× 2π)."),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 5..7:
    //   5: power   ( numerator · 0.5 / (denominator · correctn / max(|correctd|, ε)) )
    //   6: deneps  ( 1 / max(|denominator|, ε) )
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_pow_block(user: array<f32, 5>) -> array<f32, 2> {
    let numerator = user[0];
    let denominator = user[1];
    let correctn = user[2];
    let correctd = user[3];
    let abs_cd = max(abs(correctd), 1e-30);
    var pwr = denominator * correctn / abs_cd;
    if (abs(pwr) <= 1e-30) { pwr = 1e-30; }
    pwr = numerator * 0.5 / pwr;
    let abs_d = max(abs(denominator), 1e-30);
    var out: array<f32, 2>;
    out[0] = pwr;
    out[1] = 1.0 / abs_d;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pow_block(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let numerator = get_param(xform_id, variation_id, 0u);
    let denominator = get_param(xform_id, variation_id, 1u);
    let root = get_param(xform_id, variation_id, 4u);
    let pwr = get_param(xform_id, variation_id, 5u);
    let deneps = get_param(xform_id, variation_id, 6u);
    let two_pi = 6.28318530717959;

    let theta = atan2(p.x, p.y);  // upstream cpp swap (atan2(FTx, FTy))
    // r2 here factors VVAR through the outer multiplier — drop the
    // upstream's `... * VVAR` and let the dispatcher reapply.
    let r2 = pow(max(p.x * p.x + p.y * p.y, 1e-30), pwr);
    let ran = (theta * deneps + root * two_pi * floor(rng_nextf(rng) * denominator) * deneps) * numerator;
    return vec2<f32>(r2 * cos(ran), r2 * sin(ran));
}
"#,
    wgsl_3d: Some(r#"
fn variation_pow_block(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let numerator = get_param(xform_id, variation_id, 0u);
    let denominator = get_param(xform_id, variation_id, 1u);
    let root = get_param(xform_id, variation_id, 4u);
    let pwr = get_param(xform_id, variation_id, 5u);
    let deneps = get_param(xform_id, variation_id, 6u);
    let two_pi = 6.28318530717959;

    let theta = atan2(p.x, p.y);
    let r2 = pow(max(p.x * p.x + p.y * p.y, 1e-30), pwr);
    let ran = (theta * deneps + root * two_pi * floor(rng_nextf(rng) * denominator) * deneps) * numerator;
    return vec3<f32>(r2 * cos(ran), r2 * sin(ran), p.z);
}
"#),
};
