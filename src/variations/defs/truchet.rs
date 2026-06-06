//! Truchet family — Tatyana Zabanova's `truchet_fill` (transcribed by Rick Sidwell)
//!
//! `truchet_fill` is a tile-grid variation that draws Truchet-style arcs
//! across each unit cell, with a deterministic-hash-based per-cell tile
//! orientation. It uses the variation weight non-trivially: `scale =
//! 1/VVAR` divides the input pre-tiling, AND the cpp's FPx/FPy lines
//! lack the usual `VVAR *` multiplier — making the X/Y output
//! weight-independent in the cpp semantics.
//!
//! Source: output/jwildfire-vars/output/truchet_fill.cpp
//!
//! Approach: `needs_transform: true` to read the per-variation weight,
//! then divide the body's output by `w` so the outer multiplier (`× w`)
//! restores the cpp result exactly.
//!
//! `arctruchet` (Jesus Sosa, 2018) added below — a sibling Truchet
//! pattern that uses random per-cell tilt (0°/90°/180°/270°) plus a
//! per-call arc sample. JWildfire's reference seeds a Java `Random`
//! at init to build a per-cell tilt array; we hash `(i, j, seed)` on
//! the fly instead since each GPU thread has isolated state and we
//! can't replicate Java's RNG. Visually equivalent random pattern,
//! not identical per-cell to JWildfire's output.
//!
//! Other Truchet-family entries from upstream (`truchet`, `truchet_ae`,
//! `truchet2`, `triantruchet`) are still deferred — most are in the
//! `unportable_dc` (writes color) or `unported_stub` buckets.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// truchet_fill: Truchet tile fill (Tatyana Zabanova)
//   Init clamps:
//     _exponent  = clamp(exponent, 0.001, 2.0)
//     _onen      = 1 / _exponent
//     _width     = clamp(arc_width, 0.001, 1.0)
//     _seed2     = sqrt(1.5·seed) / (0.5·seed) · 0.25     (NaN at seed=0;
//                  body special-cases seed==0 so the value is never read)
//
//   Body:
//     scale = 1/w
//     (x, y) = wrap(input · scale, [0, 1])
//     tiletype = 0 if seed=0; 1 if seed=1; else hash-derived 0/1
//     r0, r1 = lp-norm distances to two of the four cell corners,
//              choosing pair by tiletype
//     if |r0 − 0.5| / rmax < 1: arc passes through (x1, y1)
//     if |r1 − 0.5| / rmax < 1: arc passes through (1, 1) corner pair
//     FPx += x1 [+ second-arc term] − x   (no VVAR multiplier in upstream)
//     FPy += y1 [+ second-arc term] − y
// =============================================================================
/// Truchet-style tile fill — divides the plane into unit cells and draws
/// arc patterns through each cell with hash-based per-cell orientation.
/// Produces interlocking curved-tile patterns.
///
/// # Authors
/// - Tatyana Zabanova
pub static TRUCHET_FILL: VariationDef = VariationDef {
    name: "truchet_fill",
    aliases: &[],
    display_name: "Truchet Fill",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[
        param!("exponent", "Exponent", unlimited_float, 2.0, 0.001, 2.0,
            "Lp-norm exponent for the cell distance metric — controls how rounded or cornered the arcs are. Clamped to [0.001, 2.0]."),
        param!("arc_width", "Arc Width", unlimited_float, 0.5, 0.001, 1.0,
            "Arc thickness within each cell. Clamped to [0.001, 1.0]."),
        param!("seed", "Seed", unlimited_float, 0.0, 0.0, 100.0,
            "Hash seed for per-cell tile orientation. 0 = all same orientation; 1 = all flipped; other values produce a varied pattern."),
    ],
    // 4 derived values at slots 3..7:
    //   3: _exponent  clamp(exponent, 0.001, 2.0)
    //   4: _onen      1 / _exponent
    //   5: _width     clamp(arc_width, 0.001, 1.0)
    //   6: _seed2     sqrt(1.5·seed) / (0.5·seed) · 0.25
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_truchet_fill(user: array<f32, 3>) -> array<f32, 4> {
    let exp_in = user[0];
    let arc_in = user[1];
    let seed = user[2];
    let exp_clamped = clamp(exp_in, 0.001, 2.0);
    let safe_seed = select(seed, 1e-30, abs(seed) < 1e-30);
    var out: array<f32, 4>;
    out[0] = exp_clamped;
    out[1] = 1.0 / exp_clamped;
    out[2] = clamp(arc_in, 0.001, 1.0);
    out[3] = sqrt(max(seed * 1.5, 0.0)) / (safe_seed * 0.5) * 0.25;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_truchet_fill(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let seed = get_param(xform_id, variation_id, 2u);
    let exponent = get_param(xform_id, variation_id, 3u);
    let onen = get_param(xform_id, variation_id, 4u);
    let width = get_param(xform_id, variation_id, 5u);
    let seed2 = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let scale = inv_w;
    let modbase = 65535.0;
    let multiplier = 32747.0;
    let offset = 12345.0;

    var x = p.x * scale;
    var y = p.y * scale;
    let intx = round(x);
    let inty = round(y);
    var r = x - intx;
    x = select(r, 1.0 + r, r < 0.0);
    r = y - inty;
    y = select(r, 1.0 + r, r < 0.0);

    var tiletype: f32 = 0.0;
    if (seed == 0.0) {
        tiletype = 0.0;
    } else if (seed == 1.0) {
        tiletype = 1.0;
    } else {
        let xrand = round(abs(p.x)) * seed2;
        let yrand = round(abs(p.y)) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        var randint = (seed + niter) * seed2 * 0.5;
        randint = randint * multiplier + offset;
        randint = randint - floor(randint / modbase) * modbase;
        tiletype = randint - floor(randint / 2.0) * 2.0;
    }

    var r0: f32; var r1: f32;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y - 1.0), exponent), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x), exponent) + pow(abs(y - 1.0), exponent), onen);
    }

    var x1: f32 = 0.0;
    var y1: f32 = 0.0;
    let r00 = abs(r0 - 0.5) / safe_rmax;
    if (r00 < 1.0) {
        x1 = 2.0 * (x + floor(p.x));
        y1 = 2.0 * (y + floor(p.y));
    }

    var fpx_inc: f32;
    var fpy_inc: f32;
    let r11 = abs(r1 - 0.5) / safe_rmax;
    if (r11 < 1.0) {
        fpx_inc = x1 + 2.0 * (x + floor(p.x)) - p.x;
        fpy_inc = y1 + 2.0 * (y + floor(p.y)) - p.y;
    } else {
        fpx_inc = x1 - p.x;
        fpy_inc = y1 - p.y;
    }

    return vec2<f32>(fpx_inc * inv_w, fpy_inc * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_truchet_fill(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let seed = get_param(xform_id, variation_id, 2u);
    let exponent = get_param(xform_id, variation_id, 3u);
    let onen = get_param(xform_id, variation_id, 4u);
    let width = get_param(xform_id, variation_id, 5u);
    let seed2 = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = select(rmax, 1e-30, abs(rmax) < 1e-30);
    let scale = inv_w;
    let modbase = 65535.0;
    let multiplier = 32747.0;
    let offset = 12345.0;

    var x = p.x * scale;
    var y = p.y * scale;
    let intx = round(x);
    let inty = round(y);
    var r = x - intx;
    x = select(r, 1.0 + r, r < 0.0);
    r = y - inty;
    y = select(r, 1.0 + r, r < 0.0);

    var tiletype: f32 = 0.0;
    if (seed == 0.0) {
        tiletype = 0.0;
    } else if (seed == 1.0) {
        tiletype = 1.0;
    } else {
        let xrand = round(abs(p.x)) * seed2;
        let yrand = round(abs(p.y)) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        var randint = (seed + niter) * seed2 * 0.5;
        randint = randint * multiplier + offset;
        randint = randint - floor(randint / modbase) * modbase;
        tiletype = randint - floor(randint / 2.0) * 2.0;
    }

    var r0: f32; var r1: f32;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y - 1.0), exponent), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x), exponent) + pow(abs(y - 1.0), exponent), onen);
    }

    var x1: f32 = 0.0;
    var y1: f32 = 0.0;
    let r00 = abs(r0 - 0.5) / safe_rmax;
    if (r00 < 1.0) {
        x1 = 2.0 * (x + floor(p.x));
        y1 = 2.0 * (y + floor(p.y));
    }

    var fpx_inc: f32;
    var fpy_inc: f32;
    let r11 = abs(r1 - 0.5) / safe_rmax;
    if (r11 < 1.0) {
        fpx_inc = x1 + 2.0 * (x + floor(p.x)) - p.x;
        fpy_inc = y1 + 2.0 * (y + floor(p.y)) - p.y;
    } else {
        fpx_inc = x1 - p.x;
        fpy_inc = y1 - p.y;
    }

    // Z preserve scales with weight (FPz += VVAR · FTz upstream); leave
    // p.z so the outer multiplier produces VVAR · p.z.
    return vec3<f32>(fpx_inc * inv_w, fpy_inc * inv_w, p.z);
}
"#,
};

// =============================================================================
// arctruchet (Jesus Sosa, August 2018)
//
// Quarter-circle Truchet tile pattern. The plane is divided into a
// `tiles_per_row × tiles_per_column` grid; each cell hosts a randomly-
// tilted quarter-arc (0°/90°/180°/270° rotation), and per call the
// variation samples a uniformly-random point within the cell's arc
// band (radius `[radius, radius + thickness]`, angle from `phi1` to
// `phi2`).
//
// Hardcoded `radius = 0.25` in the source (private field, not a
// param); we follow suit. `tile_size = 2 · radius = 0.5`.
//
// Two quadrants per cell at 50/50 odds:
//   `(phi1, phi2) = (0°, 90°)`   — upper-right quadrant
//   `(phi1, phi2) = (180°, 270°)` — lower-left quadrant
// The anchor point `d` shifts the arc origin to a cell corner so the
// quarter-arc visually connects to neighbouring cells.
//
// JWildfire's per-cell tilt array is seeded with a Java `Random` at
// init; we hash `(i, j, seed)` per call instead, since GPU threads
// have isolated RNG state and we can't replicate Java's PRNG. The
// resulting pattern is statistically equivalent (each cell gets a
// uniformly-random tilt in {0, 1, 2, 3}) but not bit-identical
// per-cell to JWildfire's output.
// =============================================================================
/// Quarter-arc Truchet tile pattern. Each cell in a `tiles_per_row ×
/// tiles_per_column` grid hosts a randomly-tilted quarter-arc, and the
/// variation samples points uniformly inside each cell's arc band
/// (width controlled by `thickness`). Produces interlocking
/// curve-and-corner tile patterns reminiscent of an Escher-style maze.
///
/// # Authors
/// - Jesus Sosa
pub static ARCTRUCHET: VariationDef = VariationDef {
    name: "arctruchet",
    aliases: &[],
    display_name: "Arctruchet",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        // Cell-tilt PRNG seed. We feed it into a per-call hash of
        // `(i, j, seed)` rather than into a Java `Random` constructor.
        // Values not matching JWildfire's Java RNG output bit-for-bit
        // produce visually equivalent random Truchet patterns; users
        // who want a different pattern just dial the seed.
        param!("seed", "Seed", unlimited_int, 10000.0, 0.0, 100000.0,
            "PRNG seed feeding the per-cell tilt hash. JWildfire seeds a Java `Random` at init to build a static per-cell tilt array; on GPU we hash `(i, j, seed)` per call instead. Visually equivalent random Truchet pattern, not bit-identical per-cell to JWildfire's output. Same range as JWF."),
        param!("thickness", "Thickness", unlimited_float, 0.025, 0.0, 1.0,
            "Arc band thickness in cell units (`radius = 0.25` is hardcoded). Wider = chunkier arcs. JWildfire clamps to [0, 1]; we follow suit at runtime."),
        param!("tiles_per_row", "Tiles Per Row", unlimited_int, 10.0, 1.0, 100.0,
            "Grid columns (matches JWF's `TilesPerRow` label). Higher = denser pattern. **GPU-clamped to [1, 100]** matching JWF."),
        param!("tiles_per_column", "Tiles Per Column", unlimited_int, 10.0, 1.0, 100.0,
            "Grid rows. **GPU-clamped to [1, 100]** matching JWF."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: ARCTRUCHET_2D,
    wgsl_3d: ARCTRUCHET_3D,
};

// Per call: ~30 ops + 4 rng draws + 1 hash. Cheap.
const ARCTRUCHET_2D: &str = r#"
// PCG-style integer hash. Mixes `(i, j, seed)` into a u32 we mod by 4
// for the cell's tilt count. Output distribution is uniform enough
// for visual purposes — not a cryptographic hash, just an avalanche
// step or two so neighbouring (i, j) cells get unrelated tilts.
fn arctruchet_cell_tilt(i: u32, j: u32, seed: u32) -> u32 {
    var h: u32 = i * 374761393u + j * 668265263u + seed * 1274126177u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return h % 4u;
}

fn variation_arctruchet(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let seed_f = get_param(xform_id, variation_id, 0u);
    let thickness = clamp(get_param(xform_id, variation_id, 1u), 0.0, 1.0);
    let tpr_f = clamp(get_param(xform_id, variation_id, 2u), 1.0, 100.0);
    let tpc_f = clamp(get_param(xform_id, variation_id, 3u), 1.0, 100.0);
    let tpr = u32(tpr_f);
    let tpc = u32(tpc_f);

    let radius: f32 = 0.25;        // hardcoded in JWF source
    let tile_size: f32 = 0.5;      // 2 * radius

    // Pick a random cell (i, j).
    let i = u32(rng_nextf(rng) * tpr_f);
    let j = u32(rng_nextf(rng) * tpc_f);

    // Cell-center world position.
    let cx = f32(i) * tile_size + tile_size * 0.5;
    let cy = f32(j) * tile_size + tile_size * 0.5;

    // Per-cell tilt angle from the hash → 0°, 90°, 180°, 270°.
    let tilt_idx = arctruchet_cell_tilt(i, j, u32(seed_f));
    let ang = f32(tilt_idx) * 1.57079632679;  // π/2 radians

    // 50/50 choice between the two quadrants. The `radio` sign flips
    // for the lower-left quadrant per JWF — that's what makes the arcs
    // line up at the corner instead of overlapping at the centre.
    let phi1: f32 = select(180.0, 0.0, rng_nextf(rng) < 0.5);
    let phi2: f32 = phi1 + 90.0;
    let radio: f32 = select(-radius, radius, phi1 == 0.0);

    let phi10 = phi1 * 0.01745329251;  // π/180
    let phi20 = phi2 * 0.01745329251;
    let delta = phi20 - phi10;

    // Annulus radial range — sample uniformly within
    // `[radius + thickness - gamma, radius + thickness]`. The `gamma`
    // formula keeps the visible arc-band width close to `thickness`
    // even as `radius` varies; we keep it for parity with JWF.
    let denom = radius + thickness;
    let gamma = thickness * (2.0 * radius + thickness) / max(denom, 1.0e-32);
    let r = radius + thickness - gamma * rng_nextf(rng);
    let phi = phi10 + delta * rng_nextf(rng);

    // Arc sample then per-cell rotation.
    let xp = r * cos(phi);
    let yp = r * sin(phi);
    let ca = cos(ang);
    let sa = sin(ang);
    let prx = xp * ca + yp * sa;
    let pry = -xp * sa + yp * ca;

    // Anchor offset shifts the arc to a cell corner.
    let dx = radio * ca + radio * sa;
    let dy = -radio * sa + radio * ca;

    let local_x = prx - dx;
    let local_y = pry - dy;

    // Centre the grid around the origin.
    let half_w = tile_size * f32(tpr) * 0.5;
    let half_h = tile_size * f32(tpc) * 0.5;

    return vec2<f32>(local_x + cx - half_w, local_y + cy - half_h);
}
"#;

// 3D body: arctruchet is a 2D base shape (VARTYPE_BASE_SHAPE in JWF);
// the JWF source passes p.z through when `preserve_z` is set. We
// always pass p.z through.
const ARCTRUCHET_3D: &str = r#"
fn arctruchet_cell_tilt(i: u32, j: u32, seed: u32) -> u32 {
    var h: u32 = i * 374761393u + j * 668265263u + seed * 1274126177u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return h % 4u;
}

fn variation_arctruchet(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let seed_f = get_param(xform_id, variation_id, 0u);
    let thickness = clamp(get_param(xform_id, variation_id, 1u), 0.0, 1.0);
    let tpr_f = clamp(get_param(xform_id, variation_id, 2u), 1.0, 100.0);
    let tpc_f = clamp(get_param(xform_id, variation_id, 3u), 1.0, 100.0);
    let tpr = u32(tpr_f);
    let tpc = u32(tpc_f);

    let radius: f32 = 0.25;
    let tile_size: f32 = 0.5;

    let i = u32(rng_nextf(rng) * tpr_f);
    let j = u32(rng_nextf(rng) * tpc_f);
    let cx = f32(i) * tile_size + tile_size * 0.5;
    let cy = f32(j) * tile_size + tile_size * 0.5;
    let tilt_idx = arctruchet_cell_tilt(i, j, u32(seed_f));
    let ang = f32(tilt_idx) * 1.57079632679;

    let phi1: f32 = select(180.0, 0.0, rng_nextf(rng) < 0.5);
    let phi2: f32 = phi1 + 90.0;
    let radio: f32 = select(-radius, radius, phi1 == 0.0);

    let phi10 = phi1 * 0.01745329251;
    let phi20 = phi2 * 0.01745329251;
    let delta = phi20 - phi10;
    let denom = radius + thickness;
    let gamma = thickness * (2.0 * radius + thickness) / max(denom, 1.0e-32);
    let r = radius + thickness - gamma * rng_nextf(rng);
    let phi = phi10 + delta * rng_nextf(rng);

    let xp = r * cos(phi);
    let yp = r * sin(phi);
    let ca = cos(ang);
    let sa = sin(ang);
    let prx = xp * ca + yp * sa;
    let pry = -xp * sa + yp * ca;
    let dx = radio * ca + radio * sa;
    let dy = -radio * sa + radio * ca;
    let local_x = prx - dx;
    let local_y = pry - dy;
    let half_w = tile_size * f32(tpr) * 0.5;
    let half_h = tile_size * f32(tpc) * 0.5;

    return vec3<f32>(local_x + cx - half_w, local_y + cy - half_h, p.z);
}
"#;
