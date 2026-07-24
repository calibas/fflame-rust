//! `chladni` — Chladni-plate nodal-line attractor (original).
//!
//! Simulates the sand-on-a-vibrating-plate cymatics experiment. The
//! standing-wave field of a square plate with mode numbers (m, n) is
//!
//! ```text
//! f(x, y) = a·sin(π·m·x)·sin(π·n·y) + b·sin(π·n·x)·sin(π·m·y)
//! ```
//!
//! and real Chladni figures form where sand migrates to the nodal
//! lines (`f = 0`). This variation makes the chaos game do the same
//! thing: each call takes `steps` Newton steps toward the zero set,
//!
//! ```text
//! p' = p − f(p)·∇f(p) / |∇f(p)|²
//! ```
//!
//! so the nodal lines become an attractor in the IFS sense — points
//! condense onto the pattern and the flame's transforms scatter them
//! along it. `strength` blends between the raw input (0) and the fully
//! projected point (1) so it composes with other variations; `jitter`
//! adds isotropic noise for a sand-grain look.
//!
//! Direct color (`dc_mode` + the transform's Direct Color slider):
//! *Distance* colors by first-order distance to the nodal set
//! (`|f|/|∇f|`, cell-normalized — pairs well with low `strength`),
//! *Amplitude* colors the cells between nodal lines by the signed
//! field value (adjacent cells of a real plate vibrate in opposite
//! phase — this is that checkerboard), *Mode Mix* colors by which of
//! the two interfering terms dominates locally.
//!
//! The field is periodic (period `2·size` in each axis), so the
//! pattern tiles the whole plane rather than stopping at a plate edge
//! — zooming transforms extend the figure indefinitely.
//!
//! No JWildfire/Apophysis equivalent — original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Chladni-plate nodal-line attractor (sand-on-a-vibrating-plate
/// cymatics).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static CHLADNI: VariationDef = VariationDef {
    name: "chladni",
    aliases: &[],
    display_name: "Chladni",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("m", "Mode M", float, 3.0, 1.0, 20.0, "First mode number of the standing wave. Integer values give classic Chladni figures; fractional values morph smoothly between them (good for animation)."),
        param!("n", "Mode N", float, 5.0, 1.0, 20.0, "Second mode number. Patterns are most interesting when M ≠ N; swapping M and N mirrors the figure across the diagonal."),
        param!("a", "Mix A", float, 1.0, -2.0, 2.0, "Amplitude of the sin(πMx)·sin(πNy) term. With b = ±a you get the classic symmetric plate figures; unequal values skew the pattern."),
        param!("b", "Mix B", float, 1.0, -2.0, 2.0, "Amplitude of the mirrored sin(πNx)·sin(πMy) term. Negative values flip the interference and produce the diagonal-symmetric family."),
        param!("size", "Size", float, 1.0, 0.1, 4.0, "Spatial scale of the plate: one period of the pattern spans 2·size units. Purely a coordinate scale — same figure, bigger or smaller."),
        param!("steps", "Steps", int, 3.0, 1.0, 6.0, "Newton iterations toward the nodal line per call. 1 is soft and halo-like; 3+ lands points crisply on the line."),
        param!("strength", "Strength", float, 0.9, 0.0, 1.0, "Blend between the untouched input point (0) and the fully projected point (1). Below 1 the pattern reads as attraction rather than hard snapping, which composes better with other variations."),
        param!("jitter", "Jitter", float, 0.0, 0.0, 0.2, "Isotropic random offset added after projection — a sand-grain look. 0 keeps the nodal lines razor thin."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Distance", "Amplitude", "Mode Mix"], "Direct-color source, applied through the transform's Direct Color slider. Distance: palette position 1 on the nodal lines fading to 0 away from them (great at low Strength). Amplitude: colors the cells between lines by signed vibration phase — the physical checkerboard of a real plate. Mode Mix: colors along the figure by the two modes' signed push-pull balance (varies along the nodal curves, unlike a ratio which is constant on them)."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 4.0, "Contrast for the direct-color modes: Distance falloff sharpness, Amplitude saturation. No effect when Color Mode is Off."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Both bodies share the per-iteration math (the 3D body applies in XY
// and passes z through): `steps` damped-Newton iterations onto the
// zero set of the plate field, a strength blend, optional jitter, then
// the direct-color write. Two guards keep the projection stable:
//   * epsilon in the |∇f|² denominator — at antinodes the gradient
//     vanishes and a raw Newton step would fire the point to infinity;
//   * a step-length clamp (half a pattern cell) — points near critical
//     lines take several short steps instead of one wild one.

const WGSL_2D: &str = r#"
// Plate field at q: vec3(f, ∂f/∂x, ∂f/∂y).
fn chladni_field(q: vec2<f32>, m: f32, n: f32, a: f32, b: f32, size: f32) -> vec3<f32> {
    let pi = 3.14159265359;
    let sx = q / size * pi;
    let smx = sin(m * sx.x); let cmx = cos(m * sx.x);
    let snx = sin(n * sx.x); let cnx = cos(n * sx.x);
    let smy = sin(m * sx.y); let cmy = cos(m * sx.y);
    let sny = sin(n * sx.y); let cny = cos(n * sx.y);
    let k = pi / size;
    return vec3<f32>(
        a * smx * sny + b * snx * smy,
        k * (a * m * cmx * sny + b * n * cnx * smy),
        k * (a * n * smx * cny + b * m * snx * cmy),
    );
}

fn variation_chladni(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let m = get_param(xform_id, variation_id, 0u);
    let n = get_param(xform_id, variation_id, 1u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let strength = get_param(xform_id, variation_id, 6u);
    let jitter = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);

    // Longest nodal-cell dimension — used to clamp Newton steps and to
    // normalize the Distance color mode.
    let cell = size / max(min(m, n), 1e-3);
    let max_step = 0.5 * cell;

    var q = p;
    for (var i = 0; i < steps; i = i + 1) {
        let fd = chladni_field(q, m, n, a, b, size);
        let g = fd.yz;
        var step = fd.x * g / (dot(g, g) + 1e-6);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }

    // Direct color — evaluated at the incoming point, so it reflects
    // where the point was relative to the figure, not the (near-zero)
    // residual after projection.
    if (dc_mode != 0u) {
        let fd = chladni_field(p, m, n, a, b, size);
        if (dc_mode == 1u) {
            let dist = abs(fd.x) / (length(fd.yz) + 1e-6);
            *vc = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == 2u) {
            let f_norm = abs(a) + abs(b) + 1e-6;
            *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        } else {
            // Signed difference of the two terms: on the nodal set the
            // terms cancel (t1 = -t2), so their DIFFERENCE equals 2*t1
            // and varies along the figure — a ratio would be a
            // constant 0.5 everywhere on the attractor.
            let pi = 3.14159265359;
            let sx = p / size * pi;
            let t1 = a * sin(m * sx.x) * sin(n * sx.y);
            let t2 = b * sin(n * sx.x) * sin(m * sx.y);
            let f_norm = abs(a) + abs(b) + 1e-6;
            *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * (t1 - t2) / f_norm);
        }
    }

    var out = mix(p, q, strength);
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
// Plate field at q: vec3(f, ∂f/∂x, ∂f/∂y).
fn chladni_field(q: vec2<f32>, m: f32, n: f32, a: f32, b: f32, size: f32) -> vec3<f32> {
    let pi = 3.14159265359;
    let sx = q / size * pi;
    let smx = sin(m * sx.x); let cmx = cos(m * sx.x);
    let snx = sin(n * sx.x); let cnx = cos(n * sx.x);
    let smy = sin(m * sx.y); let cmy = cos(m * sx.y);
    let sny = sin(n * sx.y); let cny = cos(n * sx.y);
    let k = pi / size;
    return vec3<f32>(
        a * smx * sny + b * snx * smy,
        k * (a * m * cmx * sny + b * n * cnx * smy),
        k * (a * n * smx * cny + b * m * snx * cmy),
    );
}

fn variation_chladni(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let m = get_param(xform_id, variation_id, 0u);
    let n = get_param(xform_id, variation_id, 1u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let strength = get_param(xform_id, variation_id, 6u);
    let jitter = get_param(xform_id, variation_id, 7u);
    let dc_mode = u32(get_param(xform_id, variation_id, 8u));
    let dc_scale = get_param(xform_id, variation_id, 9u);

    let cell = size / max(min(m, n), 1e-3);
    let max_step = 0.5 * cell;

    var q = p.xy;
    for (var i = 0; i < steps; i = i + 1) {
        let fd = chladni_field(q, m, n, a, b, size);
        let g = fd.yz;
        var step = fd.x * g / (dot(g, g) + 1e-6);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }

    if (dc_mode != 0u) {
        let fd = chladni_field(p.xy, m, n, a, b, size);
        if (dc_mode == 1u) {
            let dist = abs(fd.x) / (length(fd.yz) + 1e-6);
            *vc = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == 2u) {
            let f_norm = abs(a) + abs(b) + 1e-6;
            *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        } else {
            // Signed difference of the two terms: on the nodal set the
            // terms cancel (t1 = -t2), so their DIFFERENCE equals 2*t1
            // and varies along the figure — a ratio would be a
            // constant 0.5 everywhere on the attractor.
            let pi = 3.14159265359;
            let sx = p.xy / size * pi;
            let t1 = a * sin(m * sx.x) * sin(n * sx.y);
            let t2 = b * sin(n * sx.x) * sin(m * sx.y);
            let f_norm = abs(a) + abs(b) + 1e-6;
            *vc = 0.5 + 0.5 * tanh(2.0 * dc_scale * (t1 - t2) / f_norm);
        }
    }

    var out = mix(p.xy, q, strength);
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return vec3<f32>(out, p.z);
}
"#;
