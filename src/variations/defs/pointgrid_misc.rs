//! pointgrid family + apocarpet_js
//!
//!   - pointgrid_wf (Andreas Maschke): grid of cells in a rectangle, with each
//!     cell jittered by a deterministic per-cell pseudo-random offset.
//!     8 user params (xmin, xmax, xcount, ymin, ymax, ycount,
//!     distortion, seed) + 2 init slots (_dx, _dy = (max−min)/count).
//!     RNG (2 calls/iter to pick xIdx, yIdx). cpp's `GOODRAND_SEED`
//!     reseeding for deterministic per-cell jitter is replaced with
//!     a deterministic integer hash (n^13 ^ n style) keyed on
//!     (seed, xIdx, yIdx).
//!
//!   - pointgrid3d_wf (Andreas Maschke): 3D grid version. 11 user params + 3
//!     init slots (_dx, _dy, _dz). Same hash-based jitter approach.
//!     Full3D.
//!
//!   - apocarpet_js (Jesus Sosa, based on Paul Bourke's roger17.c):
//!     Apollonian Carpet — 6-branch random IFS using inversion (cases
//!     0, 5) and four scaled translates (cases 1-4) by `r = 1/(1+√2)`.
//!     0 user params. RNG (1 call/iter for 6-way branch). Body
//!     factors cleanly.
//!
//! Sources:
//!   - `output/jwildfire-vars/output/pointgrid_wf.cpp`
//!   - `output/jwildfire-vars/output/pointgrid3d_wf.cpp`
//!   - `output/jwildfire-vars/output/apocarpet_js.cpp`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// pointgrid_wf
// ---------------------------------------------------------------------------

/// Point grid with deterministic jitter — each iteration picks a random
/// cell from an `xcount × ycount` grid spanning `[xmin, xmax] × [ymin,
/// ymax]`, then optionally jitters that cell's position by a deterministic
/// per-cell offset (hashed from `seed` + cell index, scaled by
/// `distortion`). Useful for placing IFS attractors at grid points with a
/// controllable random perturbation.
///
/// # Authors
/// - Andreas Maschke
pub static POINTGRID_WF: VariationDef = VariationDef {
    name: "pointgrid_wf",
    aliases: &[],
    display_name: "Point Grid WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("xmin", "X Min", unlimited_float, -3.0, -100.0, 100.0, "Left edge of the grid region on the X axis."),
        param!("xmax", "X Max", unlimited_float, 3.0, -100.0, 100.0, "Right edge of the grid region on the X axis."),
        param!("xcount", "X Count", int, 32.0, 1.0, 1000.0, "Number of grid cells along X."),
        param!("ymin", "Y Min", unlimited_float, -3.0, -100.0, 100.0, "Bottom edge of the grid region on the Y axis."),
        param!("ymax", "Y Max", unlimited_float, 3.0, -100.0, 100.0, "Top edge of the grid region on the Y axis."),
        param!("ycount", "Y Count", int, 32.0, 1.0, 1000.0, "Number of grid cells along Y."),
        param!("distortion", "Distortion", unlimited_float, 2.3, -10.0, 10.0, "Per-cell jitter amplitude. 0 = no jitter (pure grid); larger = more scatter around each cell center."),
        param!("seed", "Seed", int, 1234.0, -100000.0, 100000.0, "Hash seed for the per-cell jitter — changing it reshuffles the jitter pattern."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_pointgrid_wf(user: array<f32, 8>) -> array<f32, 2> {
    var out: array<f32, 2>;
    let xc = max(user[2], 1.0);
    let yc = max(user[5], 1.0);
    out[0] = (user[1] - user[0]) / xc;
    out[1] = (user[4] - user[3]) / yc;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn pg_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_pointgrid_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xmin = get_param(xform_id, variation_id, 0u);
    let xcount = max(i32(get_param(xform_id, variation_id, 2u)), 1);
    let ymin = get_param(xform_id, variation_id, 3u);
    let ycount = max(i32(get_param(xform_id, variation_id, 5u)), 1);
    let distortion = get_param(xform_id, variation_id, 6u);
    let seed = i32(get_param(xform_id, variation_id, 7u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);

    let x_idx = clamp(i32(rng_nextf(rng) * f32(xcount)), 0, xcount - 1);
    let y_idx = clamp(i32(rng_nextf(rng) * f32(ycount)), 0, ycount - 1);
    var x = xmin + dx * f32(x_idx);
    var y = ymin + dy * f32(y_idx);
    if (distortion > 0.0) {
        let distx = (0.5 - pg_disc_noise(seed + 1563 + x_idx, y_idx)) * distortion;
        let disty = (0.5 - pg_disc_noise(seed + 6715 + y_idx, x_idx)) * distortion;
        x = x + distx;
        y = y + disty;
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn pg_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_pointgrid_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xmin = get_param(xform_id, variation_id, 0u);
    let xcount = max(i32(get_param(xform_id, variation_id, 2u)), 1);
    let ymin = get_param(xform_id, variation_id, 3u);
    let ycount = max(i32(get_param(xform_id, variation_id, 5u)), 1);
    let distortion = get_param(xform_id, variation_id, 6u);
    let seed = i32(get_param(xform_id, variation_id, 7u));
    let dx = get_param(xform_id, variation_id, 8u);
    let dy = get_param(xform_id, variation_id, 9u);

    let x_idx = clamp(i32(rng_nextf(rng) * f32(xcount)), 0, xcount - 1);
    let y_idx = clamp(i32(rng_nextf(rng) * f32(ycount)), 0, ycount - 1);
    var x = xmin + dx * f32(x_idx);
    var y = ymin + dy * f32(y_idx);
    if (distortion > 0.0) {
        let distx = (0.5 - pg_disc_noise(seed + 1563 + x_idx, y_idx)) * distortion;
        let disty = (0.5 - pg_disc_noise(seed + 6715 + y_idx, x_idx)) * distortion;
        x = x + distx;
        y = y + disty;
    }
    return vec3<f32>(x, y, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// pointgrid3d_wf
// ---------------------------------------------------------------------------

/// 3D version of `pointgrid_wf` — same algorithm extended to a 3D `xcount ×
/// ycount × zcount` grid spanning `[xmin, xmax] × [ymin, ymax] × [zmin,
/// zmax]`. Each axis gets its own hashed jitter offset.
///
/// # Authors
/// - Andreas Maschke
pub static POINTGRID3D_WF: VariationDef = VariationDef {
    name: "pointgrid3d_wf",
    aliases: &[],
    display_name: "Point Grid 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("xmin", "X Min", unlimited_float, -3.0, -100.0, 100.0, "Left edge of the grid region on the X axis."),
        param!("xmax", "X Max", unlimited_float, 3.0, -100.0, 100.0, "Right edge of the grid region on the X axis."),
        param!("xcount", "X Count", int, 10.0, 1.0, 1000.0, "Number of grid cells along X."),
        param!("ymin", "Y Min", unlimited_float, -3.0, -100.0, 100.0, "Bottom edge of the grid region on the Y axis."),
        param!("ymax", "Y Max", unlimited_float, 3.0, -100.0, 100.0, "Top edge of the grid region on the Y axis."),
        param!("ycount", "Y Count", int, 10.0, 1.0, 1000.0, "Number of grid cells along Y."),
        param!("zmin", "Z Min", unlimited_float, -1.0, -100.0, 100.0, "Floor edge of the grid region on the Z axis."),
        param!("zmax", "Z Max", unlimited_float, 1.0, -100.0, 100.0, "Ceiling edge of the grid region on the Z axis."),
        param!("zcount", "Z Count", int, 10.0, 1.0, 1000.0, "Number of grid cells along Z."),
        param!("distortion", "Distortion", unlimited_float, 2.3, -10.0, 10.0, "Per-cell jitter amplitude. 0 = no jitter (pure grid); larger = more scatter around each cell center."),
        param!("seed", "Seed", int, 1234.0, -100000.0, 100000.0, "Hash seed for the per-cell jitter — changing it reshuffles the jitter pattern."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_pointgrid3d_wf(user: array<f32, 11>) -> array<f32, 3> {
    var out: array<f32, 3>;
    let xc = max(user[2], 1.0);
    let yc = max(user[5], 1.0);
    let zc = max(user[8], 1.0);
    out[0] = (user[1] - user[0]) / xc;
    out[1] = (user[4] - user[3]) / yc;
    out[2] = (user[7] - user[6]) / zc;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn pg_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_pointgrid3d_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xmin = get_param(xform_id, variation_id, 0u);
    let xcount = max(i32(get_param(xform_id, variation_id, 2u)), 1);
    let ymin = get_param(xform_id, variation_id, 3u);
    let ycount = max(i32(get_param(xform_id, variation_id, 5u)), 1);
    let distortion = get_param(xform_id, variation_id, 9u);
    let seed = i32(get_param(xform_id, variation_id, 10u));
    let dx = get_param(xform_id, variation_id, 11u);
    let dy = get_param(xform_id, variation_id, 12u);

    let x_idx = clamp(i32(rng_nextf(rng) * f32(xcount)), 0, xcount - 1);
    let y_idx = clamp(i32(rng_nextf(rng) * f32(ycount)), 0, ycount - 1);
    var x = xmin + dx * f32(x_idx);
    var y = ymin + dy * f32(y_idx);
    if (distortion > 0.0) {
        let distx = (0.5 - pg_disc_noise(seed + 1563 + x_idx, y_idx)) * distortion;
        let disty = (0.5 - pg_disc_noise(seed + 6715 + y_idx, x_idx)) * distortion;
        x = x + distx;
        y = y + disty;
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn pg_disc_noise(x: i32, y: i32) -> f32 {
    var n = x + y * 57;
    n = (n << 13u) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    return f32(h) * (1.0 / 2147483647.0);
}

fn variation_pointgrid3d_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xmin = get_param(xform_id, variation_id, 0u);
    let xcount = max(i32(get_param(xform_id, variation_id, 2u)), 1);
    let ymin = get_param(xform_id, variation_id, 3u);
    let ycount = max(i32(get_param(xform_id, variation_id, 5u)), 1);
    let zmin = get_param(xform_id, variation_id, 6u);
    let zcount = max(i32(get_param(xform_id, variation_id, 8u)), 1);
    let distortion = get_param(xform_id, variation_id, 9u);
    let seed = i32(get_param(xform_id, variation_id, 10u));
    let dx = get_param(xform_id, variation_id, 11u);
    let dy = get_param(xform_id, variation_id, 12u);
    let dz = get_param(xform_id, variation_id, 13u);

    let x_idx = clamp(i32(rng_nextf(rng) * f32(xcount)), 0, xcount - 1);
    let y_idx = clamp(i32(rng_nextf(rng) * f32(ycount)), 0, ycount - 1);
    let z_idx = clamp(i32(rng_nextf(rng) * f32(zcount)), 0, zcount - 1);
    var x = xmin + dx * f32(x_idx);
    var y = ymin + dy * f32(y_idx);
    var z = zmin + dz * f32(z_idx);
    if (distortion > 0.0) {
        let distx = (0.5 - pg_disc_noise(seed + 1563 + x_idx, z_idx)) * distortion;
        let disty = (0.5 - pg_disc_noise(seed + 6715 + y_idx, x_idx)) * distortion;
        let distz = (0.5 - pg_disc_noise(seed + 4761 + z_idx, y_idx)) * distortion;
        x = x + distx;
        y = y + disty;
        z = z + distz;
    }
    return vec3<f32>(x, y, z);
}
"#,
};

// ---------------------------------------------------------------------------
// apocarpet_js
// ---------------------------------------------------------------------------

/// Apollonian Carpet — 6-branch random IFS using complex inversion
/// (branches 0 and 5) and four scaled translates by `r = 1/(1 + √2)`
/// (branches 1-4). Produces the classic Apollonian Carpet fractal. Based on
/// Paul Bourke's `roger17.c`.
///
/// # Authors
/// - Jesus Sosa
pub static APOCARPET_JS: VariationDef = VariationDef {
    name: "apocarpet_js",
    aliases: &[],
    display_name: "Apocarpet (JS)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_apocarpet_js(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = 1.0 / (1.0 + sqrt(2.0));
    let denom = p.x * p.x + p.y * p.y;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    let weight = i32(6.0 * rng_nextf(rng));
    var x: f32;
    var y: f32;
    if (weight <= 0) {
        x = 2.0 * p.x * p.y / safe_denom;
        y = (p.x * p.x - p.y * p.y) / safe_denom;
    } else if (weight == 1) {
        x = p.x * r - r;
        y = p.y * r - r;
    } else if (weight == 2) {
        x = p.x * r + r;
        y = p.y * r + r;
    } else if (weight == 3) {
        x = p.x * r + r;
        y = p.y * r - r;
    } else if (weight == 4) {
        x = p.x * r - r;
        y = p.y * r + r;
    } else {
        x = (p.x * p.x - p.y * p.y) / safe_denom;
        y = 2.0 * p.x * p.y / safe_denom;
    }
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn variation_apocarpet_js(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = 1.0 / (1.0 + sqrt(2.0));
    let denom = p.x * p.x + p.y * p.y;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    let weight = i32(6.0 * rng_nextf(rng));
    var x: f32;
    var y: f32;
    if (weight <= 0) {
        x = 2.0 * p.x * p.y / safe_denom;
        y = (p.x * p.x - p.y * p.y) / safe_denom;
    } else if (weight == 1) {
        x = p.x * r - r;
        y = p.y * r - r;
    } else if (weight == 2) {
        x = p.x * r + r;
        y = p.y * r + r;
    } else if (weight == 3) {
        x = p.x * r + r;
        y = p.y * r - r;
    } else if (weight == 4) {
        x = p.x * r - r;
        y = p.y * r + r;
    } else {
        x = (p.x * p.x - p.y * p.y) / safe_denom;
        y = 2.0 * p.x * p.y / safe_denom;
    }
    return vec3<f32>(x, y, p.z);
}
"#,
};
