//! `fract_*_wf` family — escape-time fractal variations (Andreas Maschke).
//!
//! All six variations descend from JWildfire's `AbstractFractWFFunc`
//! and follow the same body shape: pick a random seed in
//! `[xmin, xmax] × [ymin, ymax]`, iterate the escape function up to
//! `max_iter` times, count escape iterations, then map count → Z and
//! optionally drive a direct color. The only per-variation difference
//! is the one-step iterator math, which lives in
//! [`shaders/core/fractwf.wgsl`](../../../shaders/core/fractwf.wgsl) as
//! a switch on `kind` (see `FRACTWF_KIND_*`).
//!
//! Common parameter set (18 slots, identical across the family) is
//! emitted by the `fractwf_common_params!` macro below. Each variation
//! optionally adds custom params (`xseed`, `yseed`, `power`).
//!
//! Buddhabrot mode is NOT implemented in v1. Variations accept the
//! `buddhabrot_mode` param for `.flame` XML round-trip fidelity but
//! always run the iterate path. A future revision can add the 6-slot
//! state machine (chooseNewPoint + trajectory carry) and switch on
//! the param — see the comment in `fractwf.wgsl`.
//!
//! Sources: `output/variation-jwf-source/Fract{Dragon,Julia,Mandelbrot,
//! Meteors,Pearls,Salamander}WFFunc.java` and
//! `AbstractFractWFFunc.java`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// Common parameter set for AbstractFractWFFunc descendants. Order
// matches JWildfire's `getParameterNames()` so .flame XML round-trips
// cleanly — the custom params (`xseed`, `yseed`, `power`) are spliced
// in by the macro at index 6 (after `buddhabrot_mode`), matching the
// `addCustomParameterNames` callback point in the Java.
//
// Slot indices used by the variation bodies below:
//   0  max_iter             10  clip_iter_max
//   1  xmin                 11  max_clip_iter
//   2  xmax                 12  buddhabrot_min_iter
//   3  ymin                 13  scale
//   4  ymax                 14  offsetx
//   5  buddhabrot_mode      15  offsety
//   6+ <custom params>      16  offsetz
//   N  direct_color         17  z_fill
//   N+1 scalez              18  z_logscale
//   N+2 clip_iter_min       19  color_only
// where N depends on the custom count.
//
// `VariationParamDef` doesn't derive Copy (it holds `Option<&str>` and
// a `ParamType` enum with refs), so we can't define the common params
// once as a slice and index into them from a const expression — that
// would be a move from a borrowed slice. Instead the macro expands
// fresh `param!()` invocations inline at each variation declaration
// site, which produces independent literals. The text duplication is
// in the macro body, not the call sites.
macro_rules! fractwf_params {
    ($($custom:expr),* $(,)?) => {
        &[
            param!("max_iter", "Max Iter", unlimited_int, 100.0, 1.0, 100000.0, "Maximum escape iterations per starting point. Higher values resolve finer detail near the fractal boundary but linearly increase per-sample cost. **GPU-clamped to 500** — values above 500 still parse and round-trip with JWildfire `.flame` XML but the inner loop runs no more than 500 iterations to stay within the GPU TDR budget."),
            param!("xmin", "X Min", unlimited_float, -1.6, -100.0, 100.0, "Lower X bound of the random-seed sampling rectangle. The variation picks (x0, y0) uniformly in [xmin, xmax] × [ymin, ymax] each call (or uses the affine input when `color_only` is on)."),
            param!("xmax", "X Max", unlimited_float, 1.6, -100.0, 100.0, "Upper X bound of the random-seed sampling rectangle."),
            param!("ymin", "Y Min", unlimited_float, -1.2, -100.0, 100.0, "Lower Y bound of the random-seed sampling rectangle."),
            param!("ymax", "Y Max", unlimited_float, 1.2, -100.0, 100.0, "Upper Y bound of the random-seed sampling rectangle."),
            param!("buddhabrot_mode", "Buddhabrot Mode", unlimited_int, 0.0, 0.0, 1.0, "0 = standard escape-time render (pick random seed, count iterations). 1 = Buddhabrot trajectory render (NOT yet implemented; accepted for round-trip fidelity but falls back to mode 0)."),
            $($custom,)*
            param!("direct_color", "Direct Color", unlimited_int, 1.0, 0.0, 1.0, "When non-zero, the per-iteration color register `vc` is offset by `iter_count / max_iter`. Drives a palette-position gradient from the escape time."),
            param!("scalez", "Scale Z", unlimited_float, 1.0, -100.0, 100.0, "Multiplier on the escape-time → Z mapping. Combined with the variation amount as the final Z gain. JWildfire divides by 10 internally — we match that."),
            param!("clip_iter_min", "Clip Iter Min", unlimited_int, 3.0, 0.0, 10000.0, "Lower escape-count threshold. If `iter_count <= clip_iter_min`, the seed is rejected and we retry (up to `max_clip_iter` times). Filters out near-boundary fast-escape points."),
            param!("clip_iter_max", "Clip Iter Max", unlimited_int, -5.0, -10000.0, 10000.0, "Upper escape-count threshold *relative to max_iter*. If negative and `iter_count >= max_iter + clip_iter_max`, the seed is rejected. Filters out deep-interior points that never escape."),
            param!("max_clip_iter", "Max Clip Iter", unlimited_int, 3.0, 1.0, 100.0, "Number of seed-rejection retries before giving up and hiding the point. Higher = more uniform coverage at the cost of doubled/tripled per-sample work in sparse areas. **GPU-clamped to 4**. When `color_only` is on, retries are forced to 1 (every retry would produce the same iter_count with a fixed seed)."),
            param!("buddhabrot_min_iter", "Buddhabrot Min Iter", unlimited_int, 7.0, 0.0, 10000.0, "Buddhabrot pre-skip count. Trajectories shorter than this are rejected before plotting. Unused in v1 (Buddhabrot mode not implemented)."),
            param!("scale", "Scale", unlimited_float, 3.0, -100.0, 100.0, "Multiplier on the (x0 + offsetx, y0 + offsety) XY output. The seed lives in the JWildfire sampling rectangle ([xmin..xmax] etc.), so `scale` brings it back into flame-space."),
            param!("offsetx", "Offset X", unlimited_float, 0.0, -100.0, 100.0, "X translation added before the `scale` multiply."),
            param!("offsety", "Offset Y", unlimited_float, 0.0, -100.0, 100.0, "Y translation."),
            param!("offsetz", "Offset Z", unlimited_float, 0.0, -100.0, 100.0, "Z translation added after the `scalez/10 · iter_ratio` term (3D only)."),
            param!("z_fill", "Z Fill", float, 0.0, 0.0, 1.0, "Probability of jittering Z between this step's escape count and the previous step's (lerps with a fresh random). 0 = off. Smooths the discrete Z layers from integer iter counts into a continuous gradient."),
            param!("z_logscale", "Z Log Scale", unlimited_int, 0.0, 0.0, 1.0, "When 1, Z uses `log₁₀(1 + iter_ratio)` instead of the raw `iter_ratio`. Compresses deep-interior points into a narrower band."),
            param!("color_only", "Color Only", unlimited_int, 0.0, 0.0, 1.0, "When 1, the variation reads the affine input as the escape seed (instead of picking randomly) and contributes nothing to XY — only Z and direct color. Lets you overlay an escape-time color/depth map onto another variation's XY shape."),
        ]
    };
}

/// Dragon-curve escape-time fractal (slobo777-style iterator,
/// JWildfire's [`FractDragonWFFunc`](../../../output/variation-jwf-source/FractDragonWFFunc.java)).
/// `xseed` / `yseed` parameterize the complex multiplier
/// `z → z·(z-1)·(xseed + i·yseed)` at each step.
///
/// 19 common params + 2 custom (xseed, yseed) = 21 total.
///
/// # Authors
/// - Andreas Maschke
pub static FRACT_DRAGON_WF: VariationDef = VariationDef {
    name: "fract_dragon_wf",
    aliases: &[],
    display_name: "Fract Dragon (JWF)",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: fractwf_params!(
        param!("xseed", "X Seed", unlimited_float, 1.4, -10.0, 10.0, "Real component of the Dragon-iterator complex multiplier. JWildfire's default 1.4 produces the canonical Heighway dragon shape; small deviations morph the attractor."),
        param!("yseed", "Y Seed", unlimited_float, 0.85, -10.0, 10.0, "Imaginary component of the Dragon-iterator complex multiplier."),
    ),
    wgsl_2d: FRACT_DRAGON_WGSL_2D,
    wgsl_3d: FRACT_DRAGON_WGSL_3D,
};

// =============================================================================
// Variation bodies
//
// All six bodies follow the same shape: read the per-variation slots,
// then call `fractwf_variation_body_2d` / `_3d` (defined in
// `fractwf.wgsl`). The differences are:
//
//   - which slots hold which custom param (xseed/yseed/power vs none),
//     which shifts the post-section indices accordingly,
//   - the iterator `kind` constant (Dragon/Meteors/Pearls/Salamander),
//     or a small switch for the power-dispatched Julia/Mandelbrot.
//
// Slot layout reference — the post-common params always occupy the
// last 13 slots, regardless of how many custom params the variation
// has. For N custom params: post-common starts at slot 6 + N.
//
//   pre-common   slots 0..5   (max_iter, xmin, xmax, ymin, ymax, buddhabrot_mode)
//   custom       slots 6..6+N (variation-specific)
//   post-common  slots 6+N..6+N+12
//     +0 direct_color   +1 scalez       +2 clip_iter_min   +3 clip_iter_max
//     +4 max_clip_iter  +5 buddhabrot_min_iter
//     +6 scale          +7 offsetx      +8 offsety         +9 offsetz
//     +10 z_fill        +11 z_logscale  +12 color_only
// =============================================================================

// 2 custom params (xseed, yseed) at slots 6, 7 → post-common starts at slot 8.
const FRACT_DRAGON_WGSL_2D: &str = r#"
fn variation_fract_dragon_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    return fractwf_variation_body_2d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 8u)),
        i32(get_param(xform_id, variation_id, 10u)),
        i32(get_param(xform_id, variation_id, 11u)),
        u32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        u32(get_param(xform_id, variation_id, 20u)),
        2u, FRACTWF_KIND_DRAGON,
        rng, vc,
    );
}
"#;

const FRACT_DRAGON_WGSL_3D: &str = r#"
fn variation_fract_dragon_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    return fractwf_variation_body_3d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 8u)),
        get_param(xform_id, variation_id, 9u),
        i32(get_param(xform_id, variation_id, 10u)),
        i32(get_param(xform_id, variation_id, 11u)),
        u32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        get_param(xform_id, variation_id, 17u),
        get_param(xform_id, variation_id, 18u),
        u32(get_param(xform_id, variation_id, 19u)),
        u32(get_param(xform_id, variation_id, 20u)),
        2u, FRACTWF_KIND_DRAGON,
        rng, vc,
    );
}
"#;

// 0 custom params → post-common starts at slot 6.
const FRACT_METEORS_WGSL_2D: &str = r#"
fn variation_fract_meteors_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    return fractwf_variation_body_2d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        0.0, 0.0,
        u32(get_param(xform_id, variation_id, 6u)),
        i32(get_param(xform_id, variation_id, 8u)),
        i32(get_param(xform_id, variation_id, 9u)),
        u32(get_param(xform_id, variation_id, 10u)),
        get_param(xform_id, variation_id, 12u),
        get_param(xform_id, variation_id, 13u),
        get_param(xform_id, variation_id, 14u),
        u32(get_param(xform_id, variation_id, 18u)),
        2u, FRACTWF_KIND_METEORS,
        rng, vc,
    );
}
"#;

const FRACT_METEORS_WGSL_3D: &str = r#"
fn variation_fract_meteors_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    return fractwf_variation_body_3d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        0.0, 0.0,
        u32(get_param(xform_id, variation_id, 6u)),
        get_param(xform_id, variation_id, 7u),
        i32(get_param(xform_id, variation_id, 8u)),
        i32(get_param(xform_id, variation_id, 9u)),
        u32(get_param(xform_id, variation_id, 10u)),
        get_param(xform_id, variation_id, 12u),
        get_param(xform_id, variation_id, 13u),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        u32(get_param(xform_id, variation_id, 17u)),
        u32(get_param(xform_id, variation_id, 18u)),
        2u, FRACTWF_KIND_METEORS,
        rng, vc,
    );
}
"#;

// 2 custom params (xseed, yseed) at slots 6, 7 → same layout as Dragon.
const FRACT_PEARLS_WGSL_2D: &str = r#"
fn variation_fract_pearls_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    return fractwf_variation_body_2d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 8u)),
        i32(get_param(xform_id, variation_id, 10u)),
        i32(get_param(xform_id, variation_id, 11u)),
        u32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        u32(get_param(xform_id, variation_id, 20u)),
        2u, FRACTWF_KIND_PEARLS,
        rng, vc,
    );
}
"#;

const FRACT_PEARLS_WGSL_3D: &str = r#"
fn variation_fract_pearls_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    return fractwf_variation_body_3d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 8u)),
        get_param(xform_id, variation_id, 9u),
        i32(get_param(xform_id, variation_id, 10u)),
        i32(get_param(xform_id, variation_id, 11u)),
        u32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        get_param(xform_id, variation_id, 17u),
        get_param(xform_id, variation_id, 18u),
        u32(get_param(xform_id, variation_id, 19u)),
        u32(get_param(xform_id, variation_id, 20u)),
        2u, FRACTWF_KIND_PEARLS,
        rng, vc,
    );
}
"#;

// Salamander — same layout as Dragon / Pearls.
const FRACT_SALAMANDER_WGSL_2D: &str = r#"
fn variation_fract_salamander_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    return fractwf_variation_body_2d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 8u)),
        i32(get_param(xform_id, variation_id, 10u)),
        i32(get_param(xform_id, variation_id, 11u)),
        u32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        u32(get_param(xform_id, variation_id, 20u)),
        2u, FRACTWF_KIND_SALAMANDER,
        rng, vc,
    );
}
"#;

const FRACT_SALAMANDER_WGSL_3D: &str = r#"
fn variation_fract_salamander_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    return fractwf_variation_body_3d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 8u)),
        get_param(xform_id, variation_id, 9u),
        i32(get_param(xform_id, variation_id, 10u)),
        i32(get_param(xform_id, variation_id, 11u)),
        u32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        get_param(xform_id, variation_id, 17u),
        get_param(xform_id, variation_id, 18u),
        u32(get_param(xform_id, variation_id, 19u)),
        u32(get_param(xform_id, variation_id, 20u)),
        2u, FRACTWF_KIND_SALAMANDER,
        rng, vc,
    );
}
"#;

// Julia — 3 custom params (xseed, yseed, power) at slots 6, 7, 8 →
// post-common starts at slot 9.
const FRACT_JULIA_WGSL_2D: &str = r#"
fn variation_fract_julia_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    // power is GPU-clamped to [2, 8]. Beyond 8 the closed-form fast paths
    // are unreachable and each escape step adds (power - 5) extra
    // complex multiplications, which compounds with max_iter ·
    // max_clip_iter · iterations_per_thread · 32K threads and crosses
    // the TDR budget.
    let power_i = i32(get_param(xform_id, variation_id, 8u));
    let power = u32(clamp(power_i, 2, 8));
    var kind: u32 = FRACTWF_KIND_JULIA_N;
    if (power == 2u) { kind = FRACTWF_KIND_JULIA2; }
    else if (power == 3u) { kind = FRACTWF_KIND_JULIA3; }
    else if (power == 4u) { kind = FRACTWF_KIND_JULIA4; }
    return fractwf_variation_body_2d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 9u)),
        i32(get_param(xform_id, variation_id, 11u)),
        i32(get_param(xform_id, variation_id, 12u)),
        u32(get_param(xform_id, variation_id, 13u)),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        get_param(xform_id, variation_id, 17u),
        u32(get_param(xform_id, variation_id, 21u)),
        power, kind,
        rng, vc,
    );
}
"#;

const FRACT_JULIA_WGSL_3D: &str = r#"
fn variation_fract_julia_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    // power is GPU-clamped to [2, 8]. Beyond 8 the closed-form fast paths
    // are unreachable and each escape step adds (power - 5) extra
    // complex multiplications, which compounds with max_iter ·
    // max_clip_iter · iterations_per_thread · 32K threads and crosses
    // the TDR budget.
    let power_i = i32(get_param(xform_id, variation_id, 8u));
    let power = u32(clamp(power_i, 2, 8));
    var kind: u32 = FRACTWF_KIND_JULIA_N;
    if (power == 2u) { kind = FRACTWF_KIND_JULIA2; }
    else if (power == 3u) { kind = FRACTWF_KIND_JULIA3; }
    else if (power == 4u) { kind = FRACTWF_KIND_JULIA4; }
    return fractwf_variation_body_3d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        get_param(xform_id, variation_id, 6u),
        get_param(xform_id, variation_id, 7u),
        u32(get_param(xform_id, variation_id, 9u)),
        get_param(xform_id, variation_id, 10u),
        i32(get_param(xform_id, variation_id, 11u)),
        i32(get_param(xform_id, variation_id, 12u)),
        u32(get_param(xform_id, variation_id, 13u)),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        get_param(xform_id, variation_id, 17u),
        get_param(xform_id, variation_id, 18u),
        get_param(xform_id, variation_id, 19u),
        u32(get_param(xform_id, variation_id, 20u)),
        u32(get_param(xform_id, variation_id, 21u)),
        power, kind,
        rng, vc,
    );
}
"#;

// Mandelbrot — 1 custom param (power) at slot 6 → post-common starts at slot 7.
// Iterator math uses startX/startY (= random seed each call) as `c`, so no
// xseed/yseed needed.
const FRACT_MANDELBROT_WGSL_2D: &str = r#"
fn variation_fract_mandelbrot_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    // power is GPU-clamped to [2, 8]. Beyond 8 the closed-form fast paths
    // are unreachable and each escape step adds (power - 5) extra
    // complex multiplications — see the matching clamp + comment in
    // FRACT_JULIA_WGSL_*.
    let power_i = i32(get_param(xform_id, variation_id, 6u));
    let power = u32(clamp(power_i, 2, 8));
    var kind: u32 = FRACTWF_KIND_MAND_N;
    if (power == 2u) { kind = FRACTWF_KIND_MAND2; }
    else if (power == 3u) { kind = FRACTWF_KIND_MAND3; }
    else if (power == 4u) { kind = FRACTWF_KIND_MAND4; }
    return fractwf_variation_body_2d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        0.0, 0.0,
        u32(get_param(xform_id, variation_id, 7u)),
        i32(get_param(xform_id, variation_id, 9u)),
        i32(get_param(xform_id, variation_id, 10u)),
        u32(get_param(xform_id, variation_id, 11u)),
        get_param(xform_id, variation_id, 13u),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        u32(get_param(xform_id, variation_id, 19u)),
        power, kind,
        rng, vc,
    );
}
"#;

const FRACT_MANDELBROT_WGSL_3D: &str = r#"
fn variation_fract_mandelbrot_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    // power is GPU-clamped to [2, 8]. Beyond 8 the closed-form fast paths
    // are unreachable and each escape step adds (power - 5) extra
    // complex multiplications — see the matching clamp + comment in
    // FRACT_JULIA_WGSL_*.
    let power_i = i32(get_param(xform_id, variation_id, 6u));
    let power = u32(clamp(power_i, 2, 8));
    var kind: u32 = FRACTWF_KIND_MAND_N;
    if (power == 2u) { kind = FRACTWF_KIND_MAND2; }
    else if (power == 3u) { kind = FRACTWF_KIND_MAND3; }
    else if (power == 4u) { kind = FRACTWF_KIND_MAND4; }
    return fractwf_variation_body_3d(
        p,
        u32(get_param(xform_id, variation_id, 0u)),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        0.0, 0.0,
        u32(get_param(xform_id, variation_id, 7u)),
        get_param(xform_id, variation_id, 8u),
        i32(get_param(xform_id, variation_id, 9u)),
        i32(get_param(xform_id, variation_id, 10u)),
        u32(get_param(xform_id, variation_id, 11u)),
        get_param(xform_id, variation_id, 13u),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        get_param(xform_id, variation_id, 16u),
        get_param(xform_id, variation_id, 17u),
        u32(get_param(xform_id, variation_id, 18u)),
        u32(get_param(xform_id, variation_id, 19u)),
        power, kind,
        rng, vc,
    );
}
"#;

// Variation registrations for the five new family members. Each one
// re-declares the common param set via `fractwf_params!` (the macro
// inlines fresh `param!()` literals) and slots in its own custom
// param defaults — JWildfire's per-variation `initParams()` overrides.

/// Meteors fractal (JWildfire's
/// [`FractMeteorsWFFunc`](../../../output/variation-jwf-source/FractMeteorsWFFunc.java)).
/// No custom params; the iterator math uses the random-seed point as the
/// dynamic constant.
///
/// # Authors
/// - Andreas Maschke
pub static FRACT_METEORS_WF: VariationDef = VariationDef {
    name: "fract_meteors_wf",
    aliases: &[],
    display_name: "Fract Meteors (JWF)",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: fractwf_params!(),
    wgsl_2d: FRACT_METEORS_WGSL_2D,
    wgsl_3d: FRACT_METEORS_WGSL_3D,
};

/// Pearls fractal (JWildfire's
/// [`FractPearlsWFFunc`](../../../output/variation-jwf-source/FractPearlsWFFunc.java)).
/// Two-parameter inverse-radial iterator (`xseed`, `yseed`).
///
/// # Authors
/// - Andreas Maschke
pub static FRACT_PEARLS_WF: VariationDef = VariationDef {
    name: "fract_pearls_wf",
    aliases: &[],
    display_name: "Fract Pearls (JWF)",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: fractwf_params!(
        param!("xseed", "X Seed", unlimited_float, 0.31, -10.0, 10.0, "Real component of the Pearls iterator complex multiplier."),
        param!("yseed", "Y Seed", unlimited_float, 0.21, -10.0, 10.0, "Imaginary component of the Pearls iterator complex multiplier."),
    ),
    wgsl_2d: FRACT_PEARLS_WGSL_2D,
    wgsl_3d: FRACT_PEARLS_WGSL_3D,
};

/// Salamander fractal (JWildfire's
/// [`FractSalamanderWFFunc`](../../../output/variation-jwf-source/FractSalamanderWFFunc.java)).
/// Quadratic iterator with a `−1` shift in the real component, paired
/// with the (`xseed`, `yseed`) complex multiplier.
///
/// # Authors
/// - Andreas Maschke
pub static FRACT_SALAMANDER_WF: VariationDef = VariationDef {
    name: "fract_salamander_wf",
    aliases: &[],
    display_name: "Fract Salamander (JWF)",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: fractwf_params!(
        param!("xseed", "X Seed", unlimited_float, 0.951, -10.0, 10.0, "Real component of the Salamander iterator complex multiplier."),
        param!("yseed", "Y Seed", unlimited_float, 0.0, -10.0, 10.0, "Imaginary component of the Salamander iterator complex multiplier."),
    ),
    wgsl_2d: FRACT_SALAMANDER_WGSL_2D,
    wgsl_3d: FRACT_SALAMANDER_WGSL_3D,
};

/// Julia escape-time fractal (JWildfire's
/// [`FractJuliaWFFunc`](../../../output/variation-jwf-source/FractJuliaWFFunc.java)).
/// Iterator is `z = z^power + c` where `c = (xseed, yseed)` is fixed
/// per call. `power` selects between four sub-iterators (2, 3, 4, ≥5);
/// most flames use `power = 2` (the classic Julia set).
///
/// # Authors
/// - Andreas Maschke
pub static FRACT_JULIA_WF: VariationDef = VariationDef {
    name: "fract_julia_wf",
    aliases: &[],
    display_name: "Fract Julia (JWF)",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: fractwf_params!(
        param!("xseed", "X Seed", unlimited_float, -0.4, -10.0, 10.0, "Real component of the fixed Julia constant `c`. JWildfire's default (-0.4, 0.6) produces the canonical Julia set; small deviations morph the attractor."),
        param!("yseed", "Y Seed", unlimited_float, 0.6, -10.0, 10.0, "Imaginary component of the fixed Julia constant `c`."),
        param!("power", "Power", unlimited_int, 2.0, 2.0, 32.0, "Exponent on `z` in the escape map `z ← z^power + c`. **GPU-clamped to [2, 8]**. Powers 2/3/4 use closed-form expansion (fastest); 5..8 fall through to a small loop that adds (power - 5) extra complex multiplies per step. Values above 8 in the .flame XML still round-trip but render as power 8."),
    ),
    wgsl_2d: FRACT_JULIA_WGSL_2D,
    wgsl_3d: FRACT_JULIA_WGSL_3D,
};

/// Mandelbrot escape-time fractal (JWildfire's
/// [`FractMandelbrotWFFunc`](../../../output/variation-jwf-source/FractMandelbrotWFFunc.java)).
/// Iterator is `z = z^power + c` where `c` is the random seed point
/// itself — the standard Mandelbrot construction. `power` selects
/// between four sub-iterators (2, 3, 4, ≥5).
///
/// # Authors
/// - Andreas Maschke
pub static FRACT_MANDELBROT_WF: VariationDef = VariationDef {
    name: "fract_mandelbrot_wf",
    aliases: &[],
    display_name: "Fract Mandelbrot (JWF)",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: fractwf_params!(
        param!("power", "Power", unlimited_int, 2.0, 2.0, 32.0, "Exponent on `z` in `z ← z^power + c` (`c` = random seed). **GPU-clamped to [2, 8]**. Powers 2/3/4 use closed-form expansion (fastest); 5..8 add (power - 5) extra complex multiplies per step. Values above 8 in the .flame XML round-trip but render as power 8."),
    ),
    wgsl_2d: FRACT_MANDELBROT_WGSL_2D,
    wgsl_3d: FRACT_MANDELBROT_WGSL_3D,
};
