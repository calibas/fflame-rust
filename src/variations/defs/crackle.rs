//! `crackle` family — Voronoi-cell-based shape variations (slobo777, JWildfire port by Andreas Maschke).
//!
//! Breaks the plane into noise-distorted Voronoi cells, then warps points
//! within each cell by `pow(L, power) * scale` where `L` is the
//! "inside-ness" of the point in its cell (0 at center, ~1 on edge). The
//! random-blur input (when variation weight ≠ 0) means crackle is a
//! **base shape** variation — it generates points rather than warping
//! incoming chaos-game state.
//!
//! `dc_crackle_wf` extends `crackle` with two direct-color params; the
//! XY math is identical so the bodies share the helper call structure.
//!
//! Both variations rely on the noise + voronoi helpers from
//! [`shaders/core/noise.wgsl`](../../../shaders/core/noise.wgsl) and
//! [`shaders/core/voronoi.wgsl`](../../../shaders/core/voronoi.wgsl).
//! The shader builder injects both modules when any of the family is
//! active. Each variation call invokes `simplex_noise_3d` ~18 times (9
//! cells × 2 components × 2 passes — initial 3×3 then re-centered 3×3),
//! which is the main GPU TDR pressure.
//!
//! Sources: [`output/variation-jwf-source/CrackleFunc.java`](../../../output/variation-jwf-source/CrackleFunc.java),
//! [`output/variation-jwf-source/DCCrackleWFFunc.java`](../../../output/variation-jwf-source/DCCrackleWFFunc.java).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Voronoi-cell base shape — picks a blurred unit-disc point, locates it
/// in a noise-distorted square lattice, warps by inside-ness `pow(L,
/// power)`. JWildfire's defaults render as a tile of irregular bubbles.
///
/// 5 params: `cellsize` (square grid scale), `power` (warp curve), `distort`
/// (noise lattice perturbation), `scale` (warp amplitude), `z` (noise
/// z-slice — animate this for a flowing effect).
///
/// # Authors
/// - slobo777
/// - Andreas Maschke
pub static CRACKLE: VariationDef = VariationDef {
    name: "crackle",
    aliases: &[],
    display_name: "Crackle",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, 0.01, 10.0, "Side length of the underlying square Voronoi grid. Smaller cells = finer detail but more visible aliasing. Values near 0 cause an early-return (no contribution) since 'an infinite number of invisible cells? No thanks!' (per the cpp)."),
        param!("power", "Power", unlimited_float, 0.2, -10.0, 10.0, "Exponent on the cell inside-ness `L` before re-applying. Values < 1 push the warp toward the cell boundary (bubble-like); > 1 push toward the centers (mosaic). Negative values invert."),
        param!("distort", "Distort", unlimited_float, 0.0, -10.0, 10.0, "Strength of the simplex-noise perturbation applied to cell centers. 0 = perfect square grid; non-zero displaces each cell center by `distort * simplex_noise_3d(...)` in both axes."),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Multiplier applied after the `pow(L, power)` warp. Combines with `power` to control the overall cell-radius distribution."),
        param!("z", "Z", unlimited_float, 0.0, -100.0, 100.0, "Z-slice through the noise field used to perturb cell centers. Animating this gives a flowing 'living' effect to the cell layout."),
    ],
    wgsl_2d: CRACKLE_WGSL_2D,
    wgsl_3d: CRACKLE_WGSL_3D,
};

/// `crackle` + direct-color output. Same XY math; the cell inside-ness
/// `L` drives a palette-position offset `vc = L * color_scale +
/// color_offset` (mod 1).
///
/// # Authors
/// - slobo777
/// - Andreas Maschke
pub static DC_CRACKLE_WF: VariationDef = VariationDef {
    name: "dc_crackle_wf",
    aliases: &[],
    display_name: "DC Crackle (JWF)",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("cellsize", "Cell Size", unlimited_float, 1.0, 0.01, 10.0, "Side length of the underlying square Voronoi grid. See `crackle`."),
        param!("power", "Power", unlimited_float, 0.2, -10.0, 10.0, "Exponent on cell inside-ness `L`. See `crackle`."),
        param!("distort", "Distort", unlimited_float, 0.0, -10.0, 10.0, "Strength of the simplex-noise perturbation on cell centers."),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Multiplier after the `pow(L, power)` warp."),
        param!("z", "Z", unlimited_float, 0.0, -100.0, 100.0, "Z-slice through the noise field."),
        param!("color_scale", "Color Scale", unlimited_float, 0.5, -10.0, 10.0, "Multiplier on cell inside-ness `L` before assigning to the color register. Combined with `color_offset` as `vc = L * color_scale + color_offset` (then mod 1 to wrap into palette range)."),
        param!("color_offset", "Color Offset", unlimited_float, 0.0, -1.0, 1.0, "Additive offset on the color register after `L * color_scale`. Negative values flip the palette traversal direction."),
    ],
    wgsl_2d: DC_CRACKLE_WGSL_2D,
    wgsl_3d: DC_CRACKLE_WGSL_3D,
};

// Shared body math (`crackle_body`) lives in
// `shaders/core/voronoi.wgsl` — both crackle and dc_crackle_wf (in
// both 2D and 3D modes) call into it. The shader builder reads only
// `wgsl_2d` *or* `wgsl_3d` per call site, and its dedupe rule is
// byte-identical, so keeping the body in one injected module avoids
// drift between the four wrappers. Each variation's WGSL just reads
// its slot-indexed params and delegates.

const CRACKLE_WGSL_2D: &str = r#"
fn variation_crackle(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = crackle_body(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        rng,
    );
    return vec2<f32>(r.x, r.y);
}
"#;

// 3D body: noise + Voronoi math is purely 2D (JWildfire's CrackleFunc
// is VARTYPE_2D); the 3D wrapper passes p.z through.
const CRACKLE_WGSL_3D: &str = r#"
fn variation_crackle(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = crackle_body(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        rng,
    );
    return vec3<f32>(r.x, r.y, p.z);
}
"#;

// DC variant: same body, plus turn `L` (the cell inside-ness in r.z)
// into a palette-position offset `vc = L · color_scale + color_offset`
// (mod 1).
const DC_CRACKLE_WGSL_2D: &str = r#"
fn variation_dc_crackle_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let r = crackle_body(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        rng,
    );
    let color_scale = get_param(xform_id, variation_id, 5u);
    let color_offset = get_param(xform_id, variation_id, 6u);
    var col = r.z * color_scale + color_offset;
    col = col - floor(col);
    *vc = col;
    return vec2<f32>(r.x, r.y);
}
"#;

const DC_CRACKLE_WGSL_3D: &str = r#"
fn variation_dc_crackle_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let r = crackle_body(
        get_param(xform_id, variation_id, 0u),
        get_param(xform_id, variation_id, 1u),
        get_param(xform_id, variation_id, 2u),
        get_param(xform_id, variation_id, 3u),
        get_param(xform_id, variation_id, 4u),
        rng,
    );
    let color_scale = get_param(xform_id, variation_id, 5u);
    let color_offset = get_param(xform_id, variation_id, 6u);
    var col = r.z * color_scale + color_offset;
    col = col - floor(col);
    *vc = col;
    return vec3<f32>(r.x, r.y, p.z);
}
"#;
