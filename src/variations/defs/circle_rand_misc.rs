//! circleRand + CircleTrans1 (Xyrus02-style, FracFx pack)
//!
//! Both use a deterministic per-cell noise hash (the same n^13 ^ n
//! style hash as circleLinear / pointgrid_wf, here `cr_disc_noise`)
//! plus a rejection-sampling loop bounded at 32 iterations to avoid
//! WGSL non-uniform-control-flow issues.
//!
//!   - circleRand: 5 user (Sc, Dens, X, Y, Seed). Each iteration
//!     samples a random (X, Y) in [-X_param, X_param] × [-Y_param,
//!     Y_param], computes cell M,N and local-cell radius U, accepts
//!     iff `noise(M+seed, N) ≤ Dens` AND `U ≤ (0.3 + 0.7·noise(M+10,
//!     N+3))·Sc`. Body factors cleanly.
//!
//!   - CircleTrans1: 5 user (Sc, Dens, X, Y, Seed). First applies
//!     halfway-translate `Trans(X, Y, FT) = ((FT − X)·0.5 + X)`
//!     toward the (X, Y) point. If the point is in a sparse cell or
//!     outside the cell radius, output the half-translated point
//!     unchanged; otherwise resample via circleRand-style rejection.
//!     Body factors cleanly.
//!
//! Both rejection loops cap at 32 iterations and emit the last
//! generated sample if no acceptance — same compromise as truchet's
//! slow drawmode (batch 91).
//!
//! Sources:
//!   - `output/jwildfire-vars/output/circlerand.cpp`
//!   - `output/jwildfire-vars/output/circletrans1.cpp`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// circleRand
// ---------------------------------------------------------------------------

/// Random-circle sampler — repeatedly samples a random `(rx, ry)` point in
/// the rectangle `[-X, X] × [-Y, Y]`, checks whether the point falls inside
/// an 'active' cell (cell density ≤ `Dens` AND distance from cell center ≤
/// a hashed per-cell radius). Returns the first accepted sample. Rejection-
/// sampling loop is capped at 32 iterations to avoid WGSL non-uniform-
/// control-flow issues; the last sample is returned if none accept.
pub static CIRCLE_RAND: VariationDef = VariationDef {
    name: "circleRand",
    aliases: &[],
    display_name: "Circle Rand",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("Sc", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Cell size — each cell occupies a `2·Sc × 2·Sc` region."),
        param!("Dens", "Density", unlimited_float, 0.5, 0.0, 1.0, "Per-cell density threshold (probability that a cell is 'active' and accepts a sample)."),
        param!("X", "X", unlimited_float, 10.0, -100.0, 100.0, "Bounding-rectangle X half-extent (samples drawn from `[-X, X]`)."),
        param!("Y", "Y", unlimited_float, 10.0, -100.0, 100.0, "Bounding-rectangle Y half-extent."),
        param!("Seed", "Seed", int, 0.0, -1000.0, 1000.0, "Hash seed for the per-cell deterministic noise."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cr_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_circleRand(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let dens = get_param(xform_id, variation_id, 1u);
    let xp = get_param(xform_id, variation_id, 2u);
    let yp = get_param(xform_id, variation_id, 3u);
    let seed = i32(get_param(xform_id, variation_id, 4u));
    let safe_sc = select(sc, 1e-30, abs(sc) < 1e-30);

    var ox = 0.0;
    var oy = 0.0;
    for (var i = 0; i < 32; i = i + 1) {
        let rx = xp * (1.0 - 2.0 * rng_nextf(rng));
        let ry = yp * (1.0 - 2.0 * rng_nextf(rng));
        let m = floor(0.5 * rx / safe_sc);
        let n = floor(0.5 * ry / safe_sc);
        let mi = i32(m);
        let ni = i32(n);
        let dx = rx - (m * 2.0 + 1.0) * sc;
        let dy = ry - (n * 2.0 + 1.0) * sc;
        let u = sqrt(dx * dx + dy * dy);
        ox = dx + (m * 2.0 + 1.0) * sc;
        oy = dy + (n * 2.0 + 1.0) * sc;
        let n_seed = cr_disc_noise(mi + seed, ni);
        let n_radius = (0.3 + 0.7 * cr_disc_noise(mi + 10, ni + 3)) * sc;
        if (n_seed <= dens && u <= n_radius) {
            break;
        }
    }
    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: r#"
fn cr_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_circleRand(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let dens = get_param(xform_id, variation_id, 1u);
    let xp = get_param(xform_id, variation_id, 2u);
    let yp = get_param(xform_id, variation_id, 3u);
    let seed = i32(get_param(xform_id, variation_id, 4u));
    let safe_sc = select(sc, 1e-30, abs(sc) < 1e-30);

    var ox = 0.0;
    var oy = 0.0;
    for (var i = 0; i < 32; i = i + 1) {
        let rx = xp * (1.0 - 2.0 * rng_nextf(rng));
        let ry = yp * (1.0 - 2.0 * rng_nextf(rng));
        let m = floor(0.5 * rx / safe_sc);
        let n = floor(0.5 * ry / safe_sc);
        let mi = i32(m);
        let ni = i32(n);
        let dx = rx - (m * 2.0 + 1.0) * sc;
        let dy = ry - (n * 2.0 + 1.0) * sc;
        let u = sqrt(dx * dx + dy * dy);
        ox = dx + (m * 2.0 + 1.0) * sc;
        oy = dy + (n * 2.0 + 1.0) * sc;
        let n_seed = cr_disc_noise(mi + seed, ni);
        let n_radius = (0.3 + 0.7 * cr_disc_noise(mi + 10, ni + 3)) * sc;
        if (n_seed <= dens && u <= n_radius) {
            break;
        }
    }
    return vec3<f32>(ox, oy, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// CircleTrans1
// ---------------------------------------------------------------------------

/// Halfway-translate + conditional resample — first applies a halfway
/// translation of the input toward the target point `(X, Y)`. If the
/// translated point lands in a sparse cell (`noise(M+seed, N) > Dens`) or
/// outside the cell's circular radius, returns it directly. Otherwise
/// resamples a new point on a random circle within an active cell (up to 32
/// rejection attempts).
pub static CIRCLE_TRANS1: VariationDef = VariationDef {
    name: "CircleTrans1",
    aliases: &[],
    display_name: "Circle Trans 1",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("Sc", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Cell size — each cell occupies a `2·Sc × 2·Sc` region."),
        param!("Dens", "Density", unlimited_float, 0.5, 0.0, 1.0, "Per-cell density threshold (probability that a cell is 'active' and triggers resampling)."),
        param!("X", "X", unlimited_float, 10.0, -100.0, 100.0, "X coordinate of the halfway-translate target point. Also bounds the resample rectangle (`|X|`)."),
        param!("Y", "Y", unlimited_float, 10.0, -100.0, 100.0, "Y coordinate of the halfway-translate target point. Also bounds the resample rectangle."),
        param!("Seed", "Seed", int, 0.0, -1000.0, 1000.0, "Hash seed for the per-cell deterministic noise."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn cr_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_CircleTrans1(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let dens = get_param(xform_id, variation_id, 1u);
    let xp = get_param(xform_id, variation_id, 2u);
    let yp = get_param(xform_id, variation_id, 3u);
    let seed = i32(get_param(xform_id, variation_id, 4u));
    let safe_sc = select(sc, 1e-30, abs(sc) < 1e-30);
    let two_pi = 6.28318530717959;

    let ux = (p.x - xp) * 0.5 + xp;
    let uy = (p.y - yp) * 0.5 + yp;
    let m0 = floor(0.5 * ux / safe_sc);
    let n0 = floor(0.5 * uy / safe_sc);
    let mi0 = i32(m0);
    let ni0 = i32(n0);
    let dx0 = ux - (m0 * 2.0 + 1.0) * sc;
    let dy0 = uy - (n0 * 2.0 + 1.0) * sc;
    let u0 = sqrt(dx0 * dx0 + dy0 * dy0);
    let n0_seed = cr_disc_noise(mi0 + seed, ni0);
    let n0_radius = (0.3 + 0.7 * cr_disc_noise(mi0 + 10, ni0 + 3)) * sc;
    if (n0_seed > dens || u0 > n0_radius) {
        return vec2<f32>(ux, uy);
    }

    var ox = 0.0;
    var oy = 0.0;
    let abs_xp = abs(xp);
    let abs_yp = abs(yp);
    for (var i = 0; i < 32; i = i + 1) {
        let rx = abs_xp * (1.0 - 2.0 * rng_nextf(rng));
        let ry = abs_yp * (1.0 - 2.0 * rng_nextf(rng));
        let m = floor(0.5 * rx / safe_sc);
        let n = floor(0.5 * ry / safe_sc);
        let mi = i32(m);
        let ni = i32(n);
        let alpha = two_pi * rng_nextf(rng);
        let u_radius = 0.3 + 0.7 * cr_disc_noise(mi + 10, ni + 3);
        let dx = u_radius * cos(alpha);
        let dy = u_radius * sin(alpha);
        ox = dx + (m * 2.0 + 1.0) * sc;
        oy = dy + (n * 2.0 + 1.0) * sc;
        if (cr_disc_noise(mi + seed, ni) <= dens) {
            break;
        }
    }
    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: r#"
fn cr_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_CircleTrans1(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let dens = get_param(xform_id, variation_id, 1u);
    let xp = get_param(xform_id, variation_id, 2u);
    let yp = get_param(xform_id, variation_id, 3u);
    let seed = i32(get_param(xform_id, variation_id, 4u));
    let safe_sc = select(sc, 1e-30, abs(sc) < 1e-30);
    let two_pi = 6.28318530717959;

    let ux = (p.x - xp) * 0.5 + xp;
    let uy = (p.y - yp) * 0.5 + yp;
    let m0 = floor(0.5 * ux / safe_sc);
    let n0 = floor(0.5 * uy / safe_sc);
    let mi0 = i32(m0);
    let ni0 = i32(n0);
    let dx0 = ux - (m0 * 2.0 + 1.0) * sc;
    let dy0 = uy - (n0 * 2.0 + 1.0) * sc;
    let u0 = sqrt(dx0 * dx0 + dy0 * dy0);
    let n0_seed = cr_disc_noise(mi0 + seed, ni0);
    let n0_radius = (0.3 + 0.7 * cr_disc_noise(mi0 + 10, ni0 + 3)) * sc;
    if (n0_seed > dens || u0 > n0_radius) {
        return vec3<f32>(ux, uy, p.z);
    }

    var ox = 0.0;
    var oy = 0.0;
    let abs_xp = abs(xp);
    let abs_yp = abs(yp);
    for (var i = 0; i < 32; i = i + 1) {
        let rx = abs_xp * (1.0 - 2.0 * rng_nextf(rng));
        let ry = abs_yp * (1.0 - 2.0 * rng_nextf(rng));
        let m = floor(0.5 * rx / safe_sc);
        let n = floor(0.5 * ry / safe_sc);
        let mi = i32(m);
        let ni = i32(n);
        let alpha = two_pi * rng_nextf(rng);
        let u_radius = 0.3 + 0.7 * cr_disc_noise(mi + 10, ni + 3);
        let dx = u_radius * cos(alpha);
        let dy = u_radius * sin(alpha);
        ox = dx + (m * 2.0 + 1.0) * sc;
        oy = dy + (n * 2.0 + 1.0) * sc;
        if (cr_disc_noise(mi + seed, ni) <= dens) {
            break;
        }
    }
    return vec3<f32>(ox, oy, p.z);
}
"#,
};
