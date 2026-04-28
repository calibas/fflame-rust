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

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// =============================================================================
// exp: complex exponential
//   exp(x + iy) = e^x · (cos(y), sin(y))
// =============================================================================
pub static EXP: VariationDef = VariationDef {
    name: "exp",
    display_name: "Exp",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_exp(p: vec2<f32>) -> vec2<f32> {
    let e = exp(p.x);
    return vec2<f32>(e * cos(p.y), e * sin(p.y));
}
"#,
    wgsl_3d: Some(r#"
fn variation_exp(p: vec3<f32>) -> vec3<f32> {
    let e = exp(p.x);
    return vec3<f32>(e * cos(p.y), e * sin(p.y), p.z);
}
"#),
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
pub static LOG_DB: VariationDef = VariationDef {
    name: "log_db",
    display_name: "Log DB",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "base", display_name: "Base", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(1e-6), max_value: Some(100.0) },
        VariationParamDef { name: "fix_period", display_name: "Fix Period", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(0.0), max_value: Some(10.0) },
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

    let r2 = max(p.x * p.x + p.y * p.y, 1e-40);
    return vec2<f32>(denom * log(r2), atan2(p.x, p.y) + fix_atan_period);
}
"#,
    wgsl_3d: Some(r#"
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

    let r2 = max(p.x * p.x + p.y * p.y, 1e-40);
    return vec3<f32>(denom * log(r2), atan2(p.x, p.y) + fix_atan_period, p.z);
}
"#),
};

// =============================================================================
// log_tile2: Zy0rg's 3D log-tiled spreader
//   For each axis k in {x, y, z}: spread_k chosen ±spread_k uniformly,
//   output_k = input_k + spread_k · round(log(uniform random in (0, 1)))
//
//   The log-of-uniform draws negative integers with geometric-ish distribution.
//   This produces a tile-stamping effect along each axis.
// =============================================================================
pub static LOG_TILE2: VariationDef = VariationDef {
    name: "log_tile2",
    display_name: "Log Tile2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "spreadx", display_name: "Spread X", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "spready", display_name: "Spread Y", param_type: ParamType::UnlimitedFloat,
                            default_value: 2.0, min_value: Some(-10.0), max_value: Some(10.0) },
        VariationParamDef { name: "spreadz", display_name: "Spread Z", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-10.0), max_value: Some(10.0) },
    ],
    init_param_count: 0,
    wgsl_init: None,
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
    wgsl_3d: Some(r#"
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
"#),
};

// =============================================================================
// tile_log: 1D version of log_tile2 — only spreads along X.
//   output.x = input.x + round(spread · log(uniform random in (0, 1)))
//   spread sign chosen uniformly each iteration.
// =============================================================================
pub static TILE_LOG: VariationDef = VariationDef {
    name: "tile_log",
    display_name: "Tile Log",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "spread", display_name: "Spread", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(-10.0), max_value: Some(10.0) },
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_tile_log(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let sp = get_param(xform_id, variation_id, 0u);
    let x = select(-sp, sp, rng_nextf(rng) < 0.5);
    return vec2<f32>(p.x + round(x * log(max(rng_nextf(rng), 1e-30))), p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_tile_log(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let sp = get_param(xform_id, variation_id, 0u);
    let x = select(-sp, sp, rng_nextf(rng) < 0.5);
    return vec3<f32>(p.x + round(x * log(max(rng_nextf(rng), 1e-30))), p.y, p.z);
}
"#),
};
