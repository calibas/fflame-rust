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

pub static CHLADNI: VariationDef = VariationDef {
    name: "chladni",
    aliases: &[],
    display_name: "Chladni",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
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
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Shared math (see module docs): `steps` damped-Newton iterations onto
// the zero set of the plate field, then a strength blend and optional
// jitter. The gradient is analytic (plain sin/cos). Two guards keep
// the projection stable:
//   * epsilon in the |∇f|² denominator — at antinodes the gradient
//     vanishes and a raw Newton step would fire the point to infinity;
//   * a step-length clamp (half a pattern cell) — points near critical
//     lines take several short steps instead of one wild one.

const WGSL_2D: &str = r#"
fn variation_chladni(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let m = get_param(xform_id, variation_id, 0u);
    let n = get_param(xform_id, variation_id, 1u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let strength = get_param(xform_id, variation_id, 6u);
    let jitter = get_param(xform_id, variation_id, 7u);

    let pi = 3.14159265359;
    // Longest nodal-cell dimension — used to clamp Newton steps.
    let max_step = 0.5 * size / max(min(m, n), 1e-3);

    var q = p;
    for (var i = 0; i < steps; i = i + 1) {
        let sx = q / size * pi;
        let smx = sin(m * sx.x); let cmx = cos(m * sx.x);
        let snx = sin(n * sx.x); let cnx = cos(n * sx.x);
        let smy = sin(m * sx.y); let cmy = cos(m * sx.y);
        let sny = sin(n * sx.y); let cny = cos(n * sx.y);

        let f = a * smx * sny + b * snx * smy;
        let k = pi / size;
        let g = vec2<f32>(
            k * (a * m * cmx * sny + b * n * cnx * smy),
            k * (a * n * smx * cny + b * m * snx * cmy),
        );

        var step = f * g / (dot(g, g) + 1e-6);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }

    var out = mix(p, q, strength);
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn variation_chladni(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let m = get_param(xform_id, variation_id, 0u);
    let n = get_param(xform_id, variation_id, 1u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let size = max(get_param(xform_id, variation_id, 4u), 1e-6);
    let steps = i32(get_param(xform_id, variation_id, 5u));
    let strength = get_param(xform_id, variation_id, 6u);
    let jitter = get_param(xform_id, variation_id, 7u);

    let pi = 3.14159265359;
    let max_step = 0.5 * size / max(min(m, n), 1e-3);

    var q = p.xy;
    for (var i = 0; i < steps; i = i + 1) {
        let sx = q / size * pi;
        let smx = sin(m * sx.x); let cmx = cos(m * sx.x);
        let snx = sin(n * sx.x); let cnx = cos(n * sx.x);
        let smy = sin(m * sx.y); let cmy = cos(m * sx.y);
        let sny = sin(n * sx.y); let cny = cos(n * sx.y);

        let f = a * smx * sny + b * snx * smy;
        let k = pi / size;
        let g = vec2<f32>(
            k * (a * m * cmx * sny + b * n * cnx * smy),
            k * (a * n * smx * cny + b * m * snx * cmy),
        );

        var step = f * g / (dot(g, g) + 1e-6);
        let sl = length(step);
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }

    var out = mix(p.xy, q, strength);
    if (jitter > 0.0) {
        out = out + vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5) * (2.0 * jitter);
    }
    return vec3<f32>(out, p.z);
}
"#;
