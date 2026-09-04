//! Model definitions — the rule a step applies to every cell.
//!
//! One `static ModelDef` per model, WGSL inline, mirroring
//! `src/escape/formulas.rs`. Registered in `super::MODELS`
//! (append-only). Phase 1 ships Gray–Scott alone; the rest of the
//! Tier-1 set follows in phase 2, and every one of them has already
//! been measured on the CPU (`scripts/sim_prototypes/`, results in
//! `docs/projects/simulation-catalog.md`).
//!
//! The template provides, and a model must use rather than
//! reimplement:
//!
//! * `sim_read(p)` — a neighbour's state with the configured boundary
//!   applied. Boundary handling written per model would be invisible
//!   in the middle of the grid and wrong only at its edges.
//! * `mparam(i)` — this model's `i`th parameter, in declaration order.
//! * `sim_dt()` — the config's `dt`.
//! * `sim_rng()` / `sim_step_index()` — for `NeedsRng` models only.

use super::{ModelDef, ModelFeature, SimParamDef, SimPreset};

/// Gray–Scott reaction–diffusion, in Karl Sims' discretisation.
///
/// ```text
/// ∂A/∂t = D_A ∇²A − AB² + f(1 − A)
/// ∂B/∂t = D_B ∇²B + AB² − (f + k)B
/// ```
///
/// with D_A = 1, D_B = 0.5, dt = 1 and the 3×3 Laplacian weights
/// −1 / 0.2 / 0.05. Pearson's (f, k) classes are the presets.
///
/// **The clamp to [0, 1] is not hygiene.** Measured on the CPU
/// prototype: without it an overshoot below zero feeds `B²` with the
/// wrong sign and the field reaches NaN within a few thousand steps at
/// some (f, k) pairs. The GPU kernel does the same thing for the same
/// reason.
///
/// Channels: `.x` = A, `.y` = B, `.z` = age (the step at which the
/// cell last changed appreciably, for the `age` colouring in phase 2),
/// `.w` spare.
pub static GRAY_SCOTT: ModelDef = ModelDef {
    name: "gray_scott",
    display_name: "Gray–Scott",
    description: "Two-species reaction–diffusion. Pearson's (F, k) plane holds spots, \
                  worms, mazes and mitosis — the classic reaction–diffusion patterns.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "feed",
            display_name: "Feed (F)",
            default: 0.0545,
            min: 0.0,
            max: 0.11,
            tooltip: "Rate at which A is replenished. With kill, selects the pattern class: \
                      the interesting band is roughly 0.01–0.07.",
            choices: &[],
        },
        SimParamDef {
            name: "kill",
            display_name: "Kill (k)",
            default: 0.062,
            min: 0.0,
            max: 0.073,
            tooltip: "Rate at which B is removed. Small changes cross between spots, worms \
                      and mazes; most of the plane outside 0.04–0.07 decays to empty.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_a",
            display_name: "Diffusion A",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "How fast A spreads. 1.0 is the standard scheme; the explicit solver \
                      is unstable above it at dt = 1.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_b",
            display_name: "Diffusion B",
            default: 0.5,
            min: 0.0,
            max: 1.0,
            tooltip: "How fast B spreads. The A:B ratio sets the feature size; equal rates \
                      give no pattern at all.",
            choices: &[],
        },
    ],
    // Every preset here was run on the CPU prototype at 256x256 before
    // being written down (docs/projects/simulation-catalog.md section 1);
    // `steps` is the measured settle point, not an estimate.
    presets: &[
        SimPreset {
            name: "mitosis",
            display_name: "Mitosis",
            params: &[("feed", 0.0367), ("kill", 0.0649)],
            steps: 10000,
        },
        SimPreset {
            name: "coral",
            display_name: "Coral",
            params: &[("feed", 0.0545), ("kill", 0.062)],
            steps: 10000,
        },
        SimPreset {
            name: "maze",
            display_name: "Maze",
            params: &[("feed", 0.030), ("kill", 0.057)],
            steps: 10000,
        },
        SimPreset {
            name: "worms",
            display_name: "Worms",
            params: &[("feed", 0.046), ("kill", 0.065)],
            steps: 10000,
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Karl Sims' 3x3 weights: centre -1, edge 0.2, corner 0.05. The
    // same kernel the CPU prototype used, so its measured dt bounds
    // and settle counts transfer to this shader.
    let up = sim_read(p + vec2<i32>(0, -1));
    let dn = sim_read(p + vec2<i32>(0, 1));
    let lf = sim_read(p + vec2<i32>(-1, 0));
    let rt = sim_read(p + vec2<i32>(1, 0));
    let ul = sim_read(p + vec2<i32>(-1, -1));
    let ur = sim_read(p + vec2<i32>(1, -1));
    let dl = sim_read(p + vec2<i32>(-1, 1));
    let dr = sim_read(p + vec2<i32>(1, 1));

    let lap = -s.xy
        + 0.2 * (up.xy + dn.xy + lf.xy + rt.xy)
        + 0.05 * (ul.xy + ur.xy + dl.xy + dr.xy);

    let f = mparam(0u);
    let k = mparam(1u);
    let da = mparam(2u);
    let db = mparam(3u);
    let dt = sim_dt();

    let a = s.x;
    let b = s.y;
    let abb = a * b * b;
    // Clamp to the physical range. Measured on the CPU prototype:
    // without it an overshoot below zero feeds b*b with the wrong sign
    // and the field reaches NaN within a few thousand steps.
    let na = clamp(a + (da * lap.x - abb + f * (1.0 - a)) * dt, 0.0, 1.0);
    let nb = clamp(b + (db * lap.y + abb - (k + f) * b) * dt, 0.0, 1.0);

    // Age: the step at which this cell last changed appreciably. Held
    // rather than incremented, so the `age` colouring reads a time
    // rather than a counter.
    let moved = abs(nb - b) > 1.0e-4;
    let age = select(s.z, f32(sim_step_index()), moved);
    return vec4<f32>(na, nb, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // A = 1 everywhere, B = 1 inside the init shape. The shape's SIZE
    // decides whether the pattern survives: measured, 12-cell blobs die
    // at the mitosis parameters where 24-cell blobs live, which is why
    // SimInit's default radius is 24.
    return vec4<f32>(1.0, inside, 0.0, 0.0);
}
"#,
    default_steps: 10000,
};
