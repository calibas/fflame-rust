//! `dc_perlin` — Perlin-noise shape with direct-color output (slobo777,
//! JWildfire port by Andreas Maschke).
//!
//! Picks a 2D point per `shape` (square / disc / blur), maps it into 3D
//! per `map` (flat / spherical / hsphere / qsphere / bubble / bubble2),
//! samples Perlin noise (octaved simplex), and uses the noise value
//! both as a notch-band acceptance filter (retries up to `select_bailout`
//! times) and as the palette-position drive for direct color.
//!
//! Depends on [`shaders/core/noise.wgsl`](../../../shaders/core/noise.wgsl) for
//! the `perlin_noise_3d` helper — the shader builder injects that
//! module whenever this variation is active.
//!
//! Source: [`output/variation-jwf-source/DCPerlinFunc.java`](../../../output/variation-jwf-source/DCPerlinFunc.java).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Perlin-noise base shape with direct-color output. 14 params:
///
/// - `shape` (0=square / 1=disc / 2=blur) — picks the 2D point distribution
/// - `map` (0=flat / 1=spherical / 2=hsphere / 3=qsphere / 4=bubble / 5=bubble2) —
///   warps the 2D point into a 3D noise coordinate
/// - `select_centre`, `select_range` — noise-acceptance notch band:
///   only samples whose final `e` lies in `[centre-range, centre+range]`
///   are accepted; others are retried up to `select_bailout` times
/// - `centre`, `range` — palette-position `vc = centre + range·p` (mod 1)
/// - `edge` — softness of the shape boundary (square/disc/blur all use it)
/// - `scale` — noise-input multiplier (high = fine grain)
/// - `octaves` — Perlin octaves (GPU-clamped to ≤ 8 in `perlin_noise_3d`)
/// - `amps`, `freqs` — Perlin amplitude/frequency decay per octave
/// - `z` — Z-slice through the noise field (animate for flow)
/// - `select_bailout` — retry cap (**GPU-clamped to ≤ 4**)
/// - `color_only` — when 1 with `shape=square`, use the affine input as
///   the 2D point instead of random (lets you overlay the noise color
///   on another variation's XY shape)
///
/// # Authors
/// - slobo777 (original Apophysis plugin)
/// - Andreas Maschke (JWildfire port)
pub static DC_PERLIN: VariationDef = VariationDef {
    name: "dc_perlin",
    aliases: &[],
    display_name: "DC Perlin",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("shape", "Shape", unlimited_int, 0.0, 0.0, 2.0, "0 = square (uniform [-0.5, 0.5]²), 1 = disc (uniform unit disc, triangle-distributed radius), 2 = blur (uniform radius in [0, 1+edge]). The shape is what gets plotted; the noise only drives color and acceptance filtering."),
        param!("map", "Map", unlimited_int, 0.0, 0.0, 5.0, "How the 2D shape point gets mapped into the 3D noise coordinate. 0=flat (just scaled XY, Z=scale·z param). 1=spherical (XY/(r²+ε)), 2/3=hsphere/qsphere (XY/(r²+0.5/0.25)). 4=bubble, 5=bubble2 (Z replaced by sqrt(0.25-r²), gives a hemispherical noise sampling that wraps at the shape boundary)."),
        param!("select_centre", "Select Centre", float, 0.0, -1.0, 1.0, "Centre of the noise-value notch band used to accept/retry samples. Combined with `select_range` as the band `[centre - range, centre + range]`."),
        param!("select_range", "Select Range", float, 1.0, 0.1, 2.0, "Half-width of the notch band. Larger = more samples accepted (less variation per call); smaller = stricter filter (more retries up to `select_bailout`)."),
        param!("centre", "Centre", unlimited_float, 0.25, -10.0, 10.0, "Palette-position offset added to `range · noise_value` before the mod-1 wrap. Shifts the color gradient."),
        param!("range", "Range", unlimited_float, 0.25, -10.0, 10.0, "Multiplier on the Perlin noise value `p` before adding `centre`. Controls how much of the palette the noise traverses."),
        param!("edge", "Edge", unlimited_float, 0.0, 0.0, 1.0, "Softness of the shape boundary. 0 = hard edge; > 0 softens by blending a noise-derived edge weight into the acceptance value `e`. Combines with `scale` and the noise output to produce the final notch test."),
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on the 3D noise coordinate. Higher = finer noise detail; lower = broader patterns."),
        param!("octaves", "Octaves", unlimited_int, 2.0, 1.0, 8.0, "Number of Perlin octaves summed. **GPU-clamped to [1, 8]**. Each additional octave doubles the noise calls per sample."),
        param!("amps", "Amps", unlimited_float, 2.0, 0.1, 10.0, "Per-octave amplitude divisor. Each octave's contribution is divided by `amps^octave_index`; values > 1 emphasize lower frequencies."),
        param!("freqs", "Freqs", unlimited_float, 2.0, 0.1, 10.0, "Per-octave frequency multiplier. Each octave's noise input is multiplied by `freqs^octave_index`; the classical fBm uses freqs=2 (doubling)."),
        param!("z", "Z", unlimited_float, 0.0, -100.0, 100.0, "Z-slice through the noise field. Animate for a flowing/morphing pattern."),
        param!("select_bailout", "Select Bailout", unlimited_int, 10.0, 1.0, 100.0, "Maximum retries when the acceptance notch rejects a sample. **GPU-clamped to ≤ 4** to keep the worst-case dispatch under the TDR budget — with octaves=8, bailout=4 is already 32 noise calls per sample."),
        param!("color_only", "Color Only", unlimited_int, 0.0, 0.0, 1.0, "When 1 with `shape=square`, the 2D point is the affine input instead of random — lets you overlay this variation's color on another variation's XY. Has no effect for disc/blur shapes. (JWildfire also has a `pAmount=0` shortcut that bypasses shape selection entirely — not implementable here since variation amount isn't visible at the variation level. Use `color_only=1` + `shape=square` for the same effect.)"),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// 2D body. The full algorithm in one function — no shared helper since
// dc_perlin is a single variation. perlin_noise_3d comes from
// shaders/core/noise.wgsl (injected by the shader builder when this
// variation is active).
//
// JWildfire's init() computes _notch_top/_notch_bottom from
// select_centre/select_range with these clamps:
//   notch_bottom = select_centre - select_range  (clamp >0.75 → 0.75; <-2 → -3)
//   notch_top    = select_centre + select_range  (clamp <-0.75 → -0.75; >3 → 3)
// We inline that math here so live param edits don't need an init-shader
// rebuild. The clamps look weird (`> 0.75 → 0.75`, `< -2 → -3`) but
// they're transcribed verbatim from the cpp.
//
// GPU clamps (documented in tooltips):
//   octaves        ≤ 8         (in perlin_noise_3d)
//   select_bailout ≤ 4         (here)
// Worst case: 4 retries × 8 octaves × ~60 ops/simplex × 32K threads ×
// 256 chaos iters ≈ 16B ops/dispatch — stays within TDR.

const WGSL_2D: &str = r#"
fn variation_dc_perlin(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let shape = u32(get_param(xform_id, variation_id, 0u));
    let map = u32(get_param(xform_id, variation_id, 1u));
    let select_centre = get_param(xform_id, variation_id, 2u);
    let select_range = get_param(xform_id, variation_id, 3u);
    let centre = get_param(xform_id, variation_id, 4u);
    let range = get_param(xform_id, variation_id, 5u);
    let edge_p = get_param(xform_id, variation_id, 6u);
    let scale = get_param(xform_id, variation_id, 7u);
    let octaves = u32(get_param(xform_id, variation_id, 8u));
    let amps = get_param(xform_id, variation_id, 9u);
    let freqs = get_param(xform_id, variation_id, 10u);
    let z = get_param(xform_id, variation_id, 11u);
    let select_bailout_raw = u32(get_param(xform_id, variation_id, 12u));
    let color_only = u32(get_param(xform_id, variation_id, 13u));

    // GPU clamp on the retry loop — see file-level comment.
    let bailout = clamp(select_bailout_raw, 1u, 4u);

    // Recompute the notch band from select_centre/select_range each call
    // — matches JWildfire's init() math but skips the init-shader plumbing.
    var nb = select_centre - select_range;
    if (nb > 0.75) { nb = 0.75; }
    if (nb < -2.0) { nb = -3.0; }
    var nt = select_centre + select_range;
    if (nt < -0.75) { nt = -0.75; }
    if (nt > 3.0) { nt = 3.0; }

    var vx: f32 = 0.0;
    var vy: f32 = 0.0;
    var e: f32 = 0.0;
    var noise_p: f32 = 0.0;
    var t: u32 = 0u;
    let edge_safe = max(edge_p, 1.0e-32);

    loop {
        e = 0.0;
        // Shape — square / disc / blur.
        switch (shape) {
            case 0u: {  // SQUARE
                let bx = select(rng_nextf(rng), p.x, color_only > 0u);
                let by = select(rng_nextf(rng), p.y, color_only > 0u);
                vx = (1.0 + edge_p) * (bx - 0.5);
                vy = (1.0 + edge_p) * (by - 0.5);
                let r = select(vy, vx, vx * vx > vy * vy);
                if (r > 1.0 - edge_p) {
                    e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
                }
            }
            case 1u: {  // DISC
                var r = rng_nextf(rng) + rng_nextf(rng);
                if (r > 1.0) { r = 2.0 - r; }
                r = r * (1.0 + edge_p);
                if (r > 1.0 - edge_p) {
                    e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
                }
                let theta = rng_nextf(rng) * 6.28318530718;
                vx = 0.5 * r * sin(theta);
                vy = 0.5 * r * cos(theta);
            }
            case 2u: {  // BLUR
                let r = (1.0 + edge_p) * rng_nextf(rng);
                if (r > 1.0 - edge_p) {
                    e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
                }
                let theta = rng_nextf(rng) * 6.28318530718;
                vx = 0.5 * r * sin(theta);
                vy = 0.5 * r * cos(theta);
            }
            default: {}
        }

        // Map — 3D noise input from the 2D shape point.
        var nv: vec3<f32>;
        switch (map) {
            case 0u: {  // FLAT
                nv = vec3<f32>(scale * vx, scale * vy, scale * z);
            }
            case 1u: {  // SPHERICAL
                let r = 1.0 / (vx * vx + vy * vy + 1.0e-32);
                nv = vec3<f32>(scale * vx * r, scale * vy * r, scale * z);
            }
            case 2u: {  // HSPHERE
                let r = 1.0 / (vx * vx + vy * vy + 0.5);
                nv = vec3<f32>(scale * vx * r, scale * vy * r, scale * z);
            }
            case 3u: {  // QSPHERE
                let r = 1.0 / (vx * vx + vy * vy + 0.25);
                nv = vec3<f32>(scale * vx * r, scale * vy * r, scale * z);
            }
            case 4u: {  // BUBBLE
                let r2 = 0.25 - (vx * vx + vy * vy);
                let r = sqrt(abs(r2));
                nv = vec3<f32>(scale * vx, scale * vy, scale * (r + z));
            }
            case 5u: {  // BUBBLE2
                let r2 = 0.25 - (vx * vx + vy * vy);
                let r = sqrt(abs(r2));
                nv = vec3<f32>(scale * vx, scale * vy, scale * (2.0 * r + z));
            }
            default: {
                nv = vec3<f32>(0.0, 0.0, 0.0);
            }
        }

        noise_p = perlin_noise_3d(nv, amps, freqs, octaves);
        // Edge effect — combine the noise value with the soft-edge weight
        // computed during shape selection.
        if (noise_p > 0.0) {
            e = noise_p * (1.0 + e * e * 20.0) + 2.0 * e;
        } else {
            e = noise_p * (1.0 + e * e * 20.0) - 2.0 * e;
        }

        let accepted = (e >= nb) && (e <= nt);
        if (accepted || t >= bailout - 1u) { break; }
        t = t + 1u;
    }

    // DC color: vc = (centre + range · p) mod 1
    var col = centre + range * noise_p;
    col = col - floor(col);
    *vc = col;

    return vec2<f32>(vx, vy);
}
"#;

// 3D body: noise math is 2D-driven; the 3D wrapper just passes p.z
// through. JWildfire marks DCPerlinFunc with VARTYPE_3D for the bubble
// maps' Z output, but the per-call Z contribution there is incorporated
// into the noise coordinate, not the output Z — so the variation
// itself contributes nothing to output Z in JWF either.
const WGSL_3D: &str = r#"
fn variation_dc_perlin(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let shape = u32(get_param(xform_id, variation_id, 0u));
    let map = u32(get_param(xform_id, variation_id, 1u));
    let select_centre = get_param(xform_id, variation_id, 2u);
    let select_range = get_param(xform_id, variation_id, 3u);
    let centre = get_param(xform_id, variation_id, 4u);
    let range = get_param(xform_id, variation_id, 5u);
    let edge_p = get_param(xform_id, variation_id, 6u);
    let scale = get_param(xform_id, variation_id, 7u);
    let octaves = u32(get_param(xform_id, variation_id, 8u));
    let amps = get_param(xform_id, variation_id, 9u);
    let freqs = get_param(xform_id, variation_id, 10u);
    let z = get_param(xform_id, variation_id, 11u);
    let select_bailout_raw = u32(get_param(xform_id, variation_id, 12u));
    let color_only = u32(get_param(xform_id, variation_id, 13u));

    let bailout = clamp(select_bailout_raw, 1u, 4u);

    var nb = select_centre - select_range;
    if (nb > 0.75) { nb = 0.75; }
    if (nb < -2.0) { nb = -3.0; }
    var nt = select_centre + select_range;
    if (nt < -0.75) { nt = -0.75; }
    if (nt > 3.0) { nt = 3.0; }

    var vx: f32 = 0.0;
    var vy: f32 = 0.0;
    var e: f32 = 0.0;
    var noise_p: f32 = 0.0;
    var t: u32 = 0u;
    let edge_safe = max(edge_p, 1.0e-32);

    loop {
        e = 0.0;
        switch (shape) {
            case 0u: {
                let bx = select(rng_nextf(rng), p.x, color_only > 0u);
                let by = select(rng_nextf(rng), p.y, color_only > 0u);
                vx = (1.0 + edge_p) * (bx - 0.5);
                vy = (1.0 + edge_p) * (by - 0.5);
                let r = select(vy, vx, vx * vx > vy * vy);
                if (r > 1.0 - edge_p) {
                    e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
                }
            }
            case 1u: {
                var r = rng_nextf(rng) + rng_nextf(rng);
                if (r > 1.0) { r = 2.0 - r; }
                r = r * (1.0 + edge_p);
                if (r > 1.0 - edge_p) {
                    e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
                }
                let theta = rng_nextf(rng) * 6.28318530718;
                vx = 0.5 * r * sin(theta);
                vy = 0.5 * r * cos(theta);
            }
            case 2u: {
                let r = (1.0 + edge_p) * rng_nextf(rng);
                if (r > 1.0 - edge_p) {
                    e = 0.5 * (r - 1.0 + edge_p) / edge_safe;
                }
                let theta = rng_nextf(rng) * 6.28318530718;
                vx = 0.5 * r * sin(theta);
                vy = 0.5 * r * cos(theta);
            }
            default: {}
        }

        var nv: vec3<f32>;
        switch (map) {
            case 0u: {
                nv = vec3<f32>(scale * vx, scale * vy, scale * z);
            }
            case 1u: {
                let r = 1.0 / (vx * vx + vy * vy + 1.0e-32);
                nv = vec3<f32>(scale * vx * r, scale * vy * r, scale * z);
            }
            case 2u: {
                let r = 1.0 / (vx * vx + vy * vy + 0.5);
                nv = vec3<f32>(scale * vx * r, scale * vy * r, scale * z);
            }
            case 3u: {
                let r = 1.0 / (vx * vx + vy * vy + 0.25);
                nv = vec3<f32>(scale * vx * r, scale * vy * r, scale * z);
            }
            case 4u: {
                let r2 = 0.25 - (vx * vx + vy * vy);
                let r = sqrt(abs(r2));
                nv = vec3<f32>(scale * vx, scale * vy, scale * (r + z));
            }
            case 5u: {
                let r2 = 0.25 - (vx * vx + vy * vy);
                let r = sqrt(abs(r2));
                nv = vec3<f32>(scale * vx, scale * vy, scale * (2.0 * r + z));
            }
            default: {
                nv = vec3<f32>(0.0, 0.0, 0.0);
            }
        }

        noise_p = perlin_noise_3d(nv, amps, freqs, octaves);
        if (noise_p > 0.0) {
            e = noise_p * (1.0 + e * e * 20.0) + 2.0 * e;
        } else {
            e = noise_p * (1.0 + e * e * 20.0) - 2.0 * e;
        }

        let accepted = (e >= nb) && (e <= nt);
        if (accepted || t >= bailout - 1u) { break; }
        t = t + 1u;
    }

    var col = centre + range * noise_p;
    col = col - floor(col);
    *vc = col;

    return vec3<f32>(vx, vy, p.z);
}
"#;
