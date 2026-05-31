//! Pre/post-phase bridges
//!
//! Three new pre/post variations that complement variations already in
//! the registry:
//!   - `pre_curl`      (Xyrus02)  — pre-phase of the curl warp
//!   - `post_juliaq`   (Zueuk)    — post-phase of `juliaq` (already ported)
//!   - `post_julia3dq` (Zueuk)    — post-phase of `julia3dq` (already ported)
//!
//! Sources:
//!   - output/jwildfire-vars/output/pre_curl.cpp
//!   - output/jwildfire-vars/output/post_juliaq.cpp
//!   - output/jwildfire-vars/output/post_julia3Dq.cpp
//!
//! Notes on faithfulness:
//!   - All three use `VVAR` (the variation weight) as a multiplicative
//!     factor in their X/Y output. In our pre/post phase model the
//!     variation REPLACES `temp`/`p` rather than adding to an
//!     accumulator (no outer multiplier), so we read the per-variation
//!     weight via `needs_transform: true` and apply it directly inside
//!     the body — matching the cpp output exactly at any weight.
//!   - The cpp posts use `GOODRAND_0X(INT_MAX) * inv_power_2pi` to
//!     pick a random multiple of `2π/power`. We replace that with
//!     `floor(rand · power) * (2π/power)` — semantically equivalent
//!     (uniform branch of the N-th root) and avoids the cpp's distrib-
//!     ution bias when `power` doesn't divide `2^31`.
//!
//! Skipped from the planned set:
//!   - `post_depth` (Zyorg) — body reads BOTH `FTx` (pre-affine input)
//!     and `FPx` (current accumulator) and blends them. Our post-phase
//!     model exposes only the current accumulator, not the pre-affine
//!     point. Architecturally blocked until we expand the post-phase
//!     calling convention.
//!   - `pre_dcztransl` — color-reading DC variation; opposite semantics
//!     from `writes_color`; needs new flag.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// pre_curl: pre-phase form of `curl` (Xyrus02)
//   re = 1 + c1·x + c2·(x² − y²)
//   im = c1·y + 2·c2·x·y
//   r  = w / (re² + im²)
//   FTx = (x·re + y·im) · r
//   FTy = (y·re − x·im) · r
// =============================================================================
/// Pre-phase version of Curl — applies the same complex-polynomial twist as
/// Curl but before the rest of the variations run.
///
/// # Authors
/// - Xyrus02
pub static PRE_CURL: VariationDef = VariationDef {
    name: "pre_curl",
    aliases: &[],
    display_name: "Pre Curl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[
        param!("c1", "C1", unlimited_float, 0.0, -5.0, 5.0, "Linear twist strength. Stronger = tighter curl around the center."),
        param!("c2", "C2", unlimited_float, 0.0, -5.0, 5.0, "Quadratic twist strength. Adds a second-order curl that grows away from the origin."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_curl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];

    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;
    let r = w / max(re * re + im * im, 1e-30);

    return vec2<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r,
    );
}
"#,
    wgsl_3d: r#"
fn variation_pre_curl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];

    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;
    let r = w / max(re * re + im * im, 1e-30);

    return vec3<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r,
        p.z,
    );
}
"#,
};

// =============================================================================
// post_juliaq: post-phase juliaq (Zueuk)
//   inv_power      = divisor / power
//   half_inv_power = 0.5 · inv_power
//   inv_power_2pi  = 2π / power
//   a = atan2(y, x) · inv_power + n · inv_power_2pi   (n = floor(rand·power))
//   r = w · (x² + y²)^half_inv_power
//   out = (r·cos(a), r·sin(a))
// =============================================================================
/// Post-phase version of JuliaQ — applies the rational-power Julia
/// branching after all other variations have run.
///
/// # Authors
/// - Zueuk
pub static POST_JULIAQ: VariationDef = VariationDef {
    name: "post_juliaq",
    aliases: &[],
    display_name: "Post JuliaQ",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", int, 3.0, -50.0, 50.0, "Number of Julia branches in the rational power."),
        param!("divisor", "Divisor", int, 2.0, -50.0, 50.0, "Rational-power divisor. Combined with `power` lets you pick non-integer branch counts (e.g. power=3, divisor=2 → 1.5 branches)."),
    ],
    needs_transform: true,
    writes_color: false,
    // 3 derived values at slots 2..5:
    //   2: half_inv_power  (0.5 · divisor / power)
    //   3: inv_power       (divisor / power)
    //   4: inv_power_2pi   (2π / power)
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_post_juliaq(user: array<f32, 2>) -> array<f32, 3> {
    let power = user[0];
    let divisor = user[1];
    let safe_power = select(power, 1e-30, power == 0.0);
    let two_pi = 6.28318530717959;
    var out: array<f32, 3>;
    out[0] = 0.5 * divisor / safe_power;
    out[1] = divisor / safe_power;
    out[2] = two_pi / safe_power;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_juliaq(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let half_inv_power = get_param(xform_id, variation_id, 2u);
    let inv_power = get_param(xform_id, variation_id, 3u);
    let inv_power_2pi = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];

    let n = floor(rng_nextf(rng) * power);
    let a = atan2(p.y, p.x) * inv_power + n * inv_power_2pi;
    let r = w * pow(max(p.x * p.x + p.y * p.y, 1e-30), half_inv_power);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_post_juliaq(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let half_inv_power = get_param(xform_id, variation_id, 2u);
    let inv_power = get_param(xform_id, variation_id, 3u);
    let inv_power_2pi = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];

    let n = floor(rng_nextf(rng) * power);
    let a = atan2(p.y, p.x) * inv_power + n * inv_power_2pi;
    let r = w * pow(max(p.x * p.x + p.y * p.y, 1e-30), half_inv_power);
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#,
};

// =============================================================================
// post_julia3dq: post-phase julia3dq (Zueuk)
//   inv_power      = divisor / power
//   abs_inv_power  = |inv_power|
//   half_inv_power = 0.5 · inv_power − 0.5
//   inv_power_2pi  = 2π / power
//   a = atan2(y, x) · inv_power + n · inv_power_2pi   (n = floor(rand·power))
//   z = old_z · abs_inv_power
//   r = w · (x² + y² + z²)^half_inv_power
//   out_z = r · z
//   r *= sqrt(x² + y²)
//   out = (r·cos(a), r·sin(a), out_z)
// =============================================================================
/// Post-phase version of Julia3DQ — applies the 3D rational-power Julia
/// branching after all other variations have run.
///
/// # Authors
/// - Zueuk
pub static POST_JULIA3DQ: VariationDef = VariationDef {
    name: "post_julia3Dq",
    aliases: &[],
    display_name: "Post Julia3DQ",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Post,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", int, 3.0, -50.0, 50.0, "Number of Julia branches."),
        param!("divisor", "Divisor", int, 2.0, -50.0, 50.0, "Rational-power divisor."),
    ],
    needs_transform: true,
    writes_color: false,
    // 4 derived values at slots 2..6:
    //   2: inv_power        (divisor / power)
    //   3: abs_inv_power    (|divisor / power|)
    //   4: half_inv_power   (0.5 · inv_power − 0.5)
    //   5: inv_power_2pi    (2π / power)
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_post_julia3Dq(user: array<f32, 2>) -> array<f32, 4> {
    let power = user[0];
    let divisor = user[1];
    let safe_power = select(power, 1e-30, power == 0.0);
    let inv_power = divisor / safe_power;
    let two_pi = 6.28318530717959;
    var out: array<f32, 4>;
    out[0] = inv_power;
    out[1] = abs(inv_power);
    out[2] = 0.5 * inv_power - 0.5;
    out[3] = two_pi / safe_power;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_julia3Dq(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let inv_power = get_param(xform_id, variation_id, 2u);
    let half_inv_power = get_param(xform_id, variation_id, 4u);
    let inv_power_2pi = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let n = floor(rng_nextf(rng) * power);
    let a = atan2(p.y, p.x) * inv_power + n * inv_power_2pi;
    let r2d = p.x * p.x + p.y * p.y;
    // 2D form: z = 0, so z² drops out of r and out_z = 0.
    var r = w * pow(max(r2d, 1e-30), half_inv_power);
    r = r * sqrt(r2d);
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_post_julia3Dq(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let inv_power = get_param(xform_id, variation_id, 2u);
    let abs_inv_power = get_param(xform_id, variation_id, 3u);
    let half_inv_power = get_param(xform_id, variation_id, 4u);
    let inv_power_2pi = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let n = floor(rng_nextf(rng) * power);
    let a = atan2(p.y, p.x) * inv_power + n * inv_power_2pi;
    let z = p.z * abs_inv_power;
    let r2d = p.x * p.x + p.y * p.y;
    var r = w * pow(max(r2d + z * z, 1e-30), half_inv_power);
    let out_z = r * z;
    r = r * sqrt(r2d);
    return vec3<f32>(r * cos(a), r * sin(a), out_z);
}
"#,
};
