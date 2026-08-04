//! Exponential / logarithm variations
//!
//! Includes:
//!   - `exp` — complex exponential
//!   - `log_db` — DarkBeam's randomly-shifted complex log
//!   - `log_tile2` — Zy0rg's 3D log-tiled spreader
//!   - `tile_log` — Zy0rg's 1D log-tiled spreader (X axis only)
//!
//! `log` itself is already in the registry from the original 84 with the
//! Apophysis-style formula (`log(x²+y²) · 0.5/log(base)`). Upstream's
//! `log_apo` is functionally identical — the `log_apo` port was dropped
//! before merge to avoid a redundant entry.
//!
//! # The `1.1754944e-38` guards
//!
//! That is `f32::MIN_POSITIVE`, the smallest NORMAL float, and it is
//! chosen for exactly that property.
//!
//! These sites were ported as `1e-40`, which is what the JWildfire /
//! Apophysis source uses. There it is fine: those run in f64, where the
//! smallest normal is ~2.2e-308, so `log(0 + 1e-40)` is a perfectly
//! ordinary -92.103.
//!
//! In f32 the smallest normal is 1.1754944e-38, so `1e-40` is SUBNORMAL —
//! and GPUs flush subnormals to zero. NVIDIA/Vulkan and Apple/Metal both
//! do. So the guard evaluated to `max(r2, 0)` / `log(x + 0)` and did
//! nothing, and a zero argument produced -Inf where the reference
//! implementation produces a finite number. -Inf is not a near miss: it
//! trips the bad-value recovery in `main_template.wgsl` and respawns the
//! point, where Apophysis carries on.
//!
//! This is a straight f64 -> f32 porting hazard, NOT a platform
//! difference — it was equally broken on both. It is fixed here rather
//! than kept "consistent" because the reference behaviour is the one we
//! are trying to match.
//!
//! The smallest normal is used rather than the `1e-30` common elsewhere
//! in this codebase because it is the closest we can get to the source's
//! intent while surviving the flush: it lands at -87.337 against
//! JWildfire's -92.103, in the same class, where `1e-30` would give
//! -69.078.
//!
//! Found by the variation math probe; see
//! `docs/projects/variation-math-probe.md`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// =============================================================================
// exp: complex exponential
//   exp(x + iy) = e^x · (cos(y), sin(y))
// =============================================================================
/// Complex exponential — `e^x · (cos(y), sin(y))`. Stretches the plane
/// exponentially along X while wrapping Y onto a unit-circle phase.
///
/// # Authors
/// - cothe
pub static EXP: VariationDef = VariationDef {
    name: "exp",
    aliases: &[],
    display_name: "Exp",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_exp(p: vec2<f32>) -> vec2<f32> {
    let e = exp(p.x);
    return vec2<f32>(e * cos(p.y), e * sin(p.y));
}
"#,
    wgsl_3d: r#"
fn variation_exp(p: vec3<f32>) -> vec3<f32> {
    let e = exp(p.x);
    return vec3<f32>(e * cos(p.y), e * sin(p.y), p.z);
}
"#,
};

// =============================================================================
// log_db (DarkBeam's "logdb"): complex log with random period jitter on the
// imaginary part.
//
//   _denom = 0.5 / log(e · base) (or 0.5 if base <= 0)
//   _fixpe = π · fix_period (or π if fix_period <= 0)
//   fix = sum over 7 iterations of clamped binomial samples × _fixpe
//
//   x' = _denom · log(x² + y²)        (NOTE: upstream C++ omits weight here,
//                                        so we put _denom (without weight)
//                                        and let the outer multiply add it)
//   y' = atan2(x, y) + fix              (NOTE: argument order is (x, y) in
//                                        the C++ port — this is a porter
//                                        bug vs. the Java source, preserved
//                                        for behavior parity)
// =============================================================================
/// Complex log with random period jitter on the imaginary part. Like the
/// basic Log variation but the angle output gets shifted by a random
/// multiple of π (configured via `fix_period`), producing repeating-strip
/// patterns.
///
/// # Authors
/// - DarkBeam
pub static LOG_DB: VariationDef = VariationDef {
    name: "log_db",
    aliases: &[],
    display_name: "Log DB",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        VariationParamDef { name: "base", display_name: "Base", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(1e-6), max_value: Some(100.0), description: Some("Logarithm base. Larger values compress the output, smaller values stretch it. Mirrors the basic Log variation's `base`.") },
        VariationParamDef { name: "fix_period", display_name: "Fix Period", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(0.0), max_value: Some(10.0), description: Some("How much random vertical shift gets added each iteration. 0 = no shift; higher = more striping.") },
    ],
    // 2 derived values stored in slots 2..4:
    //   2: denom    (0.5 / log(e · base), or 0.5 if base <= 0)
    //   3: fixpe    (π · fix_period, or π if fix_period <= 0)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_log_db(user: array<f32, 2>) -> array<f32, 2> {
    let base = user[0];
    let fix_period = user[1];
    var out: array<f32, 2>;
    out[0] = select(0.5 / log(max(2.71828182845905 * base, 1e-20)), 0.5, base <= 1e-20);
    out[1] = select(3.14159265358979 * fix_period, 3.14159265358979, fix_period <= 1e-20);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_log_db(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let denom = get_param(xform_id, variation_id, 2u);
    let fixpe = get_param(xform_id, variation_id, 3u);

    var fix_atan_period: f32 = 0.0;
    for (var i: u32 = 0u; i < 7u; i = i + 1u) {
        var adp: i32 = i32(rng_next(rng) % 10u) - 5;
        if (adp >= 3 || adp <= -3) {
            adp = 0;
        }
        fix_atan_period = fix_atan_period + f32(adp);
    }
    fix_atan_period = fix_atan_period * fixpe;

    let r2 = max(p.x * p.x + p.y * p.y, 1.1754944e-38);
    // ff_atan2: the origin is reachable (every fuse starts there); see utilities.wgsl.
    return vec2<f32>(denom * log(r2), ff_atan2(p.x, p.y) + fix_atan_period);
}
"#,
    wgsl_3d: r#"
fn variation_log_db(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let denom = get_param(xform_id, variation_id, 2u);
    let fixpe = get_param(xform_id, variation_id, 3u);

    var fix_atan_period: f32 = 0.0;
    for (var i: u32 = 0u; i < 7u; i = i + 1u) {
        var adp: i32 = i32(rng_next(rng) % 10u) - 5;
        if (adp >= 3 || adp <= -3) {
            adp = 0;
        }
        fix_atan_period = fix_atan_period + f32(adp);
    }
    fix_atan_period = fix_atan_period * fixpe;

    let r2 = max(p.x * p.x + p.y * p.y, 1.1754944e-38);
    // ff_atan2: the origin is reachable (every fuse starts there); see utilities.wgsl.
    return vec3<f32>(denom * log(r2), ff_atan2(p.x, p.y) + fix_atan_period, p.z);
}
"#,
};

// =============================================================================
// log_tile2: Zy0rg's 3D log-tiled spreader
//   For each axis k in {x, y, z}: spread_k chosen ±spread_k uniformly,
//   output_k = input_k + spread_k · round(log(uniform random in (0, 1)))
//
//   The log-of-uniform draws negative integers with geometric-ish distribution.
//   This produces a tile-stamping effect along each axis.
// =============================================================================
/// 3D log-tiled spreader — each axis is independently shifted by a random
/// integer drawn from `log(uniform)` rounded to the nearest integer.
/// Produces a stamped tile effect with geometric falloff.
///
/// # Authors
/// - Zy0rg
pub static LOG_TILE2: VariationDef = VariationDef {
    name: "log_tile2",
    aliases: &[],
    display_name: "Log Tile2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        VariationParamDef { name: "spreadx", display_name: "Spread X", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("X-axis tile spacing. The random integer from the log-of-uniform draw is multiplied by this value.") },
        VariationParamDef { name: "spready", display_name: "Spread Y", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Y-axis tile spacing.") },
        VariationParamDef { name: "spreadz", display_name: "Spread Z", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Z-axis tile spacing (3D mode only).") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_log_tile2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let sx = get_param(xform_id, variation_id, 0u);
    let sy = get_param(xform_id, variation_id, 1u);
    let spreadx = select(-sx, sx, rng_nextf(rng) < 0.5);
    let spready = select(-sy, sy, rng_nextf(rng) < 0.5);
    let lx = round(log(max(rng_nextf(rng), 1e-30)));
    let ly = round(log(max(rng_nextf(rng), 1e-30)));
    return vec2<f32>(p.x + spreadx * lx, p.y + spready * ly);
}
"#,
    wgsl_3d: r#"
fn variation_log_tile2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let sx = get_param(xform_id, variation_id, 0u);
    let sy = get_param(xform_id, variation_id, 1u);
    let sz = get_param(xform_id, variation_id, 2u);
    let spreadx = select(-sx, sx, rng_nextf(rng) < 0.5);
    let spready = select(-sy, sy, rng_nextf(rng) < 0.5);
    let spreadz = select(-sz, sz, rng_nextf(rng) < 0.5);
    let lx = round(log(max(rng_nextf(rng), 1e-30)));
    let ly = round(log(max(rng_nextf(rng), 1e-30)));
    let lz = round(log(max(rng_nextf(rng), 1e-30)));
    return vec3<f32>(p.x + spreadx * lx, p.y + spready * ly, p.z + spreadz * lz);
}
"#,
};

// =============================================================================
// tile_log: 1D version of log_tile2 — only spreads along X.
//   output.x = input.x + round(spread · log(uniform random in (0, 1)))
//   spread sign chosen uniformly each iteration.
// =============================================================================
/// 1D version of Log Tile2 — only shifts along X with the same random log-
/// of-uniform trick. Y (and Z) pass through unchanged.
///
/// # Authors
/// - Zy0rg
pub static TILE_LOG: VariationDef = VariationDef {
    name: "tile_log",
    aliases: &[],
    display_name: "Tile Log",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        VariationParamDef { name: "spread", display_name: "Spread", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0), description: Some("Tile spacing along X. The random integer from the log-of-uniform draw is multiplied by this value.") },
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_tile_log(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let sp = get_param(xform_id, variation_id, 0u);
    let x = select(-sp, sp, rng_nextf(rng) < 0.5);
    return vec2<f32>(p.x + round(x * log(max(rng_nextf(rng), 1e-30))), p.y);
}
"#,
    wgsl_3d: r#"
fn variation_tile_log(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let sp = get_param(xform_id, variation_id, 0u);
    let x = select(-sp, sp, rng_nextf(rng) < 0.5);
    return vec3<f32>(p.x + round(x * log(max(rng_nextf(rng), 1e-30))), p.y, p.z);
}
"#,
};
