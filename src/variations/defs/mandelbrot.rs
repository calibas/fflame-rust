//! Standalone `mandelbrot` variation (Jed Kelsey, 2007 — ported to
//! JWildfire by Andreas Maschke).
//!
//! Unlike the `fract_mandelbrot_wf` cousin (which is part of the
//! `AbstractFractWFFunc` family with the standard iterate-mode body),
//! this variation runs a **random-walk Buddhabrot** through the
//! Mandelbrot escape set:
//!
//!   1. State `(x0, y0, z0)` persists per (thread, xform, variation_id)
//!      across iterations of the chaos game.
//!   2. When `(x0, y0) == (0, 0)`, pick a fresh random seed in
//!      `[xmin, xmax] × [ymin, ymax]`; otherwise step from the
//!      previous seed by a Gaussian-ish wiggle scaled by `(skin +
//!      0.001)`. This creates a low-discrepancy walk along the
//!      Mandelbrot boundary.
//!   3. Iterate `z ← z² + c` up to `iter` times.
//!   4. Accept the result if the escape count falls in the band
//!      determined by `invert` / `skin`; otherwise retry up to 10
//!      times. On the 10th failure, plot off-screen (`x1 = y1 = ~50000`).
//!   5. Output is `(x1 + cx·x, y1 + cy·y, z0)` — a blend of the seed
//!      point and the final iterate.
//!
//! Two parameters from JWildfire don't translate: `max_points` (caches
//! N visited points for replay, which our flat state buffer can't
//! hold) and `seed` (initializes a per-instance Marsaglia RNG). Both
//! are accepted for `.flame` XML round-trip but ignored at render
//! time. JWildfire's own `getGPUCode()` drops them the same way.
//!
//! State slots (3): `x0`, `y0`, `z0` — see `wgsl_state_init`.
//!
//! Source: [`output/variation-jwf-source/MandelbrotFunc.java`](../../../output/variation-jwf-source/MandelbrotFunc.java).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Random-walk Buddhabrot Mandelbrot. Per-thread state carries the
/// last seed point across iterations, so successive plots walk along
/// the fractal boundary instead of independently sampling. The
/// `skin` parameter controls the walk step; `invert` flips between
/// "plot seeds that escape" and "plot seeds that don't" semantics.
///
/// # Authors
/// - Jed Kelsey
/// - Andreas Maschke
pub static MANDELBROT: VariationDef = VariationDef {
    name: "mandelbrot",
    aliases: &[],
    display_name: "Mandelbrot",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 3,
    wgsl_state_init: None,
    parameters: &[
        param!("iter", "Iter", unlimited_int, 100.0, 1.0, 1000.0, "Maximum escape iterations per seed. **GPU-clamped to 250** to stay within the TDR budget when combined with the 10-retry outer loop and per-dispatch thread count. Values above 250 in the .flame XML round-trip but render at 250."),
        param!("xmin", "X Min", unlimited_float, -1.6, -10.0, 10.0, "Lower X bound of the random-seed sampling rectangle."),
        param!("xmax", "X Max", unlimited_float, 1.6, -10.0, 10.0, "Upper X bound of the random-seed sampling rectangle."),
        param!("ymin", "Y Min", unlimited_float, -1.2, -10.0, 10.0, "Lower Y bound of the random-seed sampling rectangle."),
        param!("ymax", "Y Max", unlimited_float, 1.2, -10.0, 10.0, "Upper Y bound of the random-seed sampling rectangle."),
        param!("invert", "Invert", unlimited_int, 0.0, 0.0, 1.0, "When 1, the acceptance criterion flips: plot seeds that escape *quickly* instead of seeds that don't escape. Treated as a probability — the random check runs each retry."),
        param!("skin", "Skin", unlimited_float, 0.012, 0.0, 1.0, "Controls two things: (1) the random-walk step size (next seed is within `skin + 0.001` of the previous), and (2) the acceptance band for normal mode (`iter_count` must be < `0.1 · iter · (1 - skin)`, so smaller skin = stricter near-boundary filter). 1.0 disables the second filter entirely."),
        param!("cx", "CX", unlimited_float, 0.0, -10.0, 10.0, "Weight on the final iterate's X added to the seed X for output. 0 = pure-seed output (classic Buddhabrot point cloud); higher = blend with the iterate trajectory."),
        param!("cy", "CY", unlimited_float, 0.0, -10.0, 10.0, "Weight on the final iterate's Y."),
        param!("max_points", "Max Points", unlimited_int, -1.0, -1.0, 1000000.0, "JWildfire CPU-only: caches the first N visited seeds and replays them. **Not honored on GPU** — value accepted for .flame XML round-trip but the shader always uses fresh randoms. Behavior matches JWildfire's own getGPUCode()."),
        param!("seed", "Seed", unlimited_int, 1234.0, 0.0, 1000000.0, "JWildfire CPU-only: seeds a per-instance Marsaglia RNG. **Not honored on GPU** — the shader uses the thread-shared RNG. Value accepted for round-trip."),
        param!("rnd_z_range", "Rnd Z Range", unlimited_float, 0.0, -10.0, 10.0, "Range of the random Z value picked when starting a fresh seed. Final output Z is `random() · rnd_z_range`. 0 = always Z = 0 (flat 2D plot)."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Both bodies share the same XY math. The 3D body picks up `_z0` from
// state, which `wgsl_state_init` would have zeroed; the 2D body
// computes z but discards it (return type is vec2). Keeping them
// parallel rather than skipping the z work in 2D — the cost is a
// handful of muls per call and the branching saves nothing.
//
// State slot layout:
//   0: x0   (random-walk seed X carried across iterations)
//   1: y0   (random-walk seed Y)
//   2: z0   (random per-fresh-pick Z, used only by the 3D body's output)
//
// GPU clamps (matching the rest of this branch's fract_* family):
//   iter ≤ 250            (worst-case inner loop bound)
//   outer retry ≤ 10      (the JWF loop bound itself)
//   ⇒ ~64M inner ops per thread per dispatch worst case at iter=250.
//
const WGSL_2D: &str = r#"
fn variation_mandelbrot(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let iter_param = u32(get_param(xform_id, variation_id, 0u));
    let iter_safe = min(max(iter_param, 1u), 250u);
    let xmin = get_param(xform_id, variation_id, 1u);
    let xmax = get_param(xform_id, variation_id, 2u);
    let ymin = get_param(xform_id, variation_id, 3u);
    let ymax = get_param(xform_id, variation_id, 4u);
    let invert_p = get_param(xform_id, variation_id, 5u);
    let skin = get_param(xform_id, variation_id, 6u);
    let cx = get_param(xform_id, variation_id, 7u);
    let cy = get_param(xform_id, variation_id, 8u);
    let rnd_z_range = get_param(xform_id, variation_id, 11u);

    var x0 = get_state(xform_id, variation_id, 0u);
    var y0 = get_state(xform_id, variation_id, 1u);
    var z0 = get_state(xform_id, variation_id, 2u);

    let iter_f = f32(iter_safe);
    let inverted = rng_nextf(rng) < invert_p;
    var curr_iter: u32 = select(iter_safe, 0u, inverted);

    var x1: f32 = x0;
    var y1: f32 = y0;
    var x: f32 = x0;
    var y: f32 = y0;

    // Outer retry loop (JWF caps at 10). Body picks a seed (fresh or
    // walked from previous), iterates, then decides whether to keep
    // or retry.
    var k: u32 = 0u;
    loop {
        // Loop condition transcribed from the cpp:
        //   inverted && curr_iter < iter           — invert mode wants escape
        //   !inverted && curr_iter >= iter         — too deep (didn't escape)
        //   !inverted && skin < 1 && curr_iter < 0.1·iter·(1-skin)  — escaped too fast
        let still_searching =
            (inverted && curr_iter < iter_safe) ||
            (!inverted && (curr_iter >= iter_safe ||
                (skin < 1.0 && f32(curr_iter) < 0.1 * iter_f * (1.0 - skin))));
        if (k >= 10u || !still_searching) { break; }
        k = k + 1u;

        // Pick a seed.
        if (x0 == 0.0 && y0 == 0.0) {
            x0 = (xmax - xmin) * rng_nextf(rng) + xmin;
            y0 = (ymax - ymin) * rng_nextf(rng) + ymin;
            z0 = rng_nextf(rng) * rnd_z_range;
        } else {
            x0 = (skin + 0.001) * (rng_nextf(rng) - 0.5) + x0;
            y0 = (skin + 0.001) * (rng_nextf(rng) - 0.5) + y0;
        }
        x1 = x0;
        y1 = y0;
        x = x0;
        y = y0;
        curr_iter = 0u;

        // Inner Mandelbrot escape — `iter_safe` already clamped above.
        loop {
            if (curr_iter >= iter_safe || (x * x + y * y >= 4.0)) { break; }
            let xtemp = x * x - y * y + x0;
            y = 2.0 * x * y + y0;
            x = xtemp;
            curr_iter = curr_iter + 1u;
        }

        // Decide whether this seed's result is acceptable. If not,
        // reset (x0, y0) to 0 so the next outer iteration picks a
        // fresh seed (JWF semantic — failed seeds clear the walk).
        let escaped_too_deep = curr_iter >= iter_safe;
        let escaped_too_fast = (skin < 1.0) && (f32(curr_iter) < 0.1 * iter_f * (1.0 - skin));
        if (escaped_too_deep || skin == 1.0 || escaped_too_fast) {
            x0 = 0.0;
            y0 = 0.0;
        }
    }

    // 10-retry fallback: plot far off-screen (JWF: 50000 ± 50000) so
    // the splat lands outside the histogram and contributes nothing.
    if (k >= 10u) {
        let far = 50000.0 - rng_nextf(rng) * 100000.0;
        x1 = far;
        y1 = far;
    }

    // Persist state for next call. (The chaos-game-state hook isn't
    // visible here; state lives in the per-thread state buffer.)
    set_state(xform_id, variation_id, 0u, x0);
    set_state(xform_id, variation_id, 1u, y0);
    set_state(xform_id, variation_id, 2u, z0);

    return vec2<f32>(x1 + cx * x, y1 + cy * y);
}
"#;

const WGSL_3D: &str = r#"
fn variation_mandelbrot(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let iter_param = u32(get_param(xform_id, variation_id, 0u));
    let iter_safe = min(max(iter_param, 1u), 250u);
    let xmin = get_param(xform_id, variation_id, 1u);
    let xmax = get_param(xform_id, variation_id, 2u);
    let ymin = get_param(xform_id, variation_id, 3u);
    let ymax = get_param(xform_id, variation_id, 4u);
    let invert_p = get_param(xform_id, variation_id, 5u);
    let skin = get_param(xform_id, variation_id, 6u);
    let cx = get_param(xform_id, variation_id, 7u);
    let cy = get_param(xform_id, variation_id, 8u);
    let rnd_z_range = get_param(xform_id, variation_id, 11u);

    var x0 = get_state(xform_id, variation_id, 0u);
    var y0 = get_state(xform_id, variation_id, 1u);
    var z0 = get_state(xform_id, variation_id, 2u);

    let iter_f = f32(iter_safe);
    let inverted = rng_nextf(rng) < invert_p;
    var curr_iter: u32 = select(iter_safe, 0u, inverted);

    var x1: f32 = x0;
    var y1: f32 = y0;
    var x: f32 = x0;
    var y: f32 = y0;

    var k: u32 = 0u;
    loop {
        let still_searching =
            (inverted && curr_iter < iter_safe) ||
            (!inverted && (curr_iter >= iter_safe ||
                (skin < 1.0 && f32(curr_iter) < 0.1 * iter_f * (1.0 - skin))));
        if (k >= 10u || !still_searching) { break; }
        k = k + 1u;

        if (x0 == 0.0 && y0 == 0.0) {
            x0 = (xmax - xmin) * rng_nextf(rng) + xmin;
            y0 = (ymax - ymin) * rng_nextf(rng) + ymin;
            z0 = rng_nextf(rng) * rnd_z_range;
        } else {
            x0 = (skin + 0.001) * (rng_nextf(rng) - 0.5) + x0;
            y0 = (skin + 0.001) * (rng_nextf(rng) - 0.5) + y0;
        }
        x1 = x0;
        y1 = y0;
        x = x0;
        y = y0;
        curr_iter = 0u;

        loop {
            if (curr_iter >= iter_safe || (x * x + y * y >= 4.0)) { break; }
            let xtemp = x * x - y * y + x0;
            y = 2.0 * x * y + y0;
            x = xtemp;
            curr_iter = curr_iter + 1u;
        }

        let escaped_too_deep = curr_iter >= iter_safe;
        let escaped_too_fast = (skin < 1.0) && (f32(curr_iter) < 0.1 * iter_f * (1.0 - skin));
        if (escaped_too_deep || skin == 1.0 || escaped_too_fast) {
            x0 = 0.0;
            y0 = 0.0;
        }
    }

    if (k >= 10u) {
        let far = 50000.0 - rng_nextf(rng) * 100000.0;
        x1 = far;
        y1 = far;
    }

    set_state(xform_id, variation_id, 0u, x0);
    set_state(xform_id, variation_id, 1u, y0);
    set_state(xform_id, variation_id, 2u, z0);

    return vec3<f32>(x1 + cx * x, y1 + cy * y, z0);
}
"#;
