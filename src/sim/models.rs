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

use super::{ModelDef, ModelFeature, SimParamDef, SimPreset, MAX_KERNEL_RADIUS};

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
            init: None,
            coloring: Some("channel"),
            coloring_params: &[("channel", 1.0), ("scale", 3.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "coral",
            display_name: "Coral",
            params: &[("feed", 0.0545), ("kill", 0.062)],
            steps: 10000,
            init: None,
            coloring: Some("channel"),
            coloring_params: &[("channel", 1.0), ("scale", 3.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "maze",
            display_name: "Maze",
            params: &[("feed", 0.030), ("kill", 0.057)],
            steps: 10000,
            init: None,
            coloring: Some("channel"),
            coloring_params: &[("channel", 1.0), ("scale", 3.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "worms",
            display_name: "Worms",
            params: &[("feed", 0.046), ("kill", 0.065)],
            steps: 10000,
            init: None,
            coloring: Some("channel"),
            coloring_params: &[("channel", 1.0), ("scale", 3.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
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
    // Explicit Euler on the Sims stencil is stable while
    // dt * D * |lambda_max| < 2, and the stencil's most negative
    // eigenvalue is the Nyquist mode: -1 - 4*0.2 + 4*0.05 = -1.6. With
    // D_A = 1 that is dt < 1.25. The [0, 1] clamp would hide anything
    // past it as garbage rather than NaN, which is worse -- so the cap
    // is enforced everywhere dt can be set, not left to the clamp.
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &["diffusion_a", "diffusion_b"],
    max_dt: 1.25,
    default_dt: 1.0,
};


/// FitzHugh-Nagumo, the canonical excitable medium.
///
/// ```text
/// dv/dt = D_v lap(v) + v - v^3/3 - w + I
/// tau dw/dt = D_w lap(w) + v + a - b w
/// ```
///
/// **The seed decides whether there is a picture at all.** Measured on
/// the CPU prototype: these constants from a NOISE seed relax to the
/// rest state -- spatial sd 0.0014, a flat field -- and from a cut
/// wavefront produce textbook counter-rotating spirals. That is why
/// the preset carries `SimInit::BrokenWave` rather than only numbers.
///
/// Also measured, and the reason no Turing preset ships: D_w = 4 with
/// I = 0 from noise gives a spatial sd of **0.0000** after 4,000 steps.
/// The labyrinth regime of FHN is real in the literature; the constant
/// set for it is not known here, and the catalogue's rule is that
/// nothing ships as a preset that has not been run.
///
/// Channels: `.x` = v (fast/excitation), `.y` = w (slow/recovery),
/// `.z` = the step this cell last fired, `.w` spare.
pub static FITZHUGH_NAGUMO: ModelDef = ModelDef {
    name: "fitzhugh_nagumo",
    display_name: "FitzHugh–Nagumo",
    description: "Excitable medium: travelling pulses and rotating spiral waves. \
                  Needs a cut wavefront to nucleate — from noise it relaxes flat.",
    features: &[ModelFeature::NeverStills],
    parameters: &[
        SimParamDef {
            name: "a",
            display_name: "a",
            default: 0.7,
            min: 0.0,
            max: 1.5,
            tooltip: "Offset in the recovery equation. With b, sets where the rest state sits.",
            choices: &[],
        },
        SimParamDef {
            name: "b",
            display_name: "b",
            default: 0.8,
            min: 0.0,
            max: 1.5,
            tooltip: "Recovery decay. Larger values damp the slow variable faster.",
            choices: &[],
        },
        SimParamDef {
            name: "tau",
            display_name: "τ (recovery time)",
            default: 12.5,
            min: 1.0,
            max: 100.0,
            tooltip: "How much slower recovery is than excitation. The separation of \
                      timescales is what makes the medium excitable rather than oscillatory.",
            choices: &[],
        },
        SimParamDef {
            name: "drive",
            display_name: "Drive (I)",
            default: 0.5,
            min: -1.0,
            max: 2.0,
            tooltip: "External current. Around 0.5 the medium is excitable with a stable \
                      rest state; higher values make it oscillate on its own.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_v",
            display_name: "Diffusion v",
            default: 1.0,
            min: 0.0,
            max: 4.0,
            tooltip: "Spread of the excitation variable. Sets the wave speed and width.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_w",
            display_name: "Diffusion w",
            default: 0.0,
            min: 0.0,
            max: 4.0,
            tooltip: "Spread of the recovery variable. 0 is the classic excitable choice; \
                      raising it is the Turing direction, which is unverified here.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "spiral",
        display_name: "Spiral waves",
        params: &[
            ("a", 0.7),
            ("b", 0.8),
            ("tau", 12.5),
            ("drive", 0.5),
            ("diffusion_v", 1.0),
            ("diffusion_w", 0.0),
        ],
        // Tips curling by 1,000 steps, fully formed by 4,000 (measured;
        // an earlier note said 1,000 without looking at that frame).
        steps: 4000,
        init: Some(crate::config::sim::SimInit::BrokenWave),
        coloring: Some("channel"),
        coloring_params: &[("channel", 0.0), ("scale", 0.4), ("offset", 0.5), ("wrap", 0.0)],
        matte: None,
    }],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
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

    let a = mparam(0u);
    let b = mparam(1u);
    let tau = mparam(2u);
    let drive = mparam(3u);
    let dv = mparam(4u);
    let dw = mparam(5u);
    let dt = sim_dt();

    let v = s.x;
    let w = s.y;
    // Both updates read the SAME old v, which is what the CPU
    // prototype did; feeding the new v into the recovery equation is a
    // different (and less stable) scheme.
    let nv = clamp(v + dt * (dv * lap.x + v - v * v * v / 3.0 - w + drive), -3.0, 3.0);
    let nw = w + dt / tau * (dw * lap.y + v + a - b * w);

    // Age is the step the cell last FIRED -- crossed into excitation --
    // rather than the last time it changed at all, which for a
    // continuously rotating spiral would be every step.
    let fired = v <= 0.0 && nv > 0.0;
    let age = select(s.z, f32(sim_step_index()), fired);
    return vec4<f32>(nv, nw, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Rest state everywhere, then the init mask carves the wavefront:
    // 1.0 is the excited band, 0.5 the refractory tail behind it. A
    // mask that is neither (noise, blobs) leaves the medium at rest --
    // which is exactly what it does physically, and why this model
    // ships with BrokenWave.
    var v = -1.2;
    var w = -0.6;
    if (inside >= 0.75) {
        v = 2.0;
    } else if (inside >= 0.25) {
        w = 1.0;
    }
    return vec4<f32>(v, w, 0.0, 0.0);
}
"#,
    default_steps: 4000,
    // Measured on the SPIRAL, not on a resting field: the [-3, 3] clamp
    // means instability shows as cells pinned to the rails rather than
    // as NaN, and 0.0% rail at every dt through 0.75 while 14.6% do at
    // 1.0. A first probe ran from noise and reported 0.5 -- it was
    // measuring the stability of a field doing nothing.
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &["diffusion_v", "diffusion_w"],
    max_dt: 0.75,
    default_dt: 0.1,
};

/// The Brusselator (Prigogine-Lefever), the textbook Turing system.
///
/// ```text
/// dX/dt = D_X lap(X) + A - (B + 1) X + X^2 Y
/// dY/dt = D_Y lap(Y) + B X - X^2 Y
/// ```
///
/// Fixed point (A, B/A). Hopf (oscillation) above B = 1 + A^2; Turing
/// (stationary pattern) needs D_Y > D_X.
///
/// Channels: `.x` = X, `.y` = Y, `.z` = age, `.w` spare.
pub static BRUSSELATOR: ModelDef = ModelDef {
    name: "brusselator",
    display_name: "Brusselator",
    description: "The textbook Turing system: stationary spots when Y diffuses faster \
                  than X, bulk oscillation when it does not.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "feed_a",
            display_name: "A",
            default: 1.0,
            min: 0.1,
            max: 3.0,
            tooltip: "Constant supply of X. The fixed point sits at X = A.",
            choices: &[],
        },
        SimParamDef {
            name: "feed_b",
            display_name: "B",
            default: 3.0,
            min: 0.1,
            max: 6.0,
            tooltip: "Drives the instability. Above 1 + A² the uniform state oscillates; \
                      with unequal diffusion it forms stationary spots instead.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_x",
            display_name: "Diffusion X",
            default: 1.0,
            min: 0.0,
            max: 4.0,
            tooltip: "Spread of the activator.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_y",
            display_name: "Diffusion Y",
            default: 8.0,
            min: 0.0,
            max: 40.0,
            tooltip: "Spread of the inhibitor. It must exceed X's for a Turing pattern; \
                      equal rates give bulk oscillation and no structure.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "turing_spots",
            display_name: "Turing spots",
            params: &[
                ("feed_a", 1.0),
                ("feed_b", 3.0),
                ("diffusion_x", 1.0),
                ("diffusion_y", 8.0),
            ],
            // Measured settle point, 256^2 (a first run reported 4,960
            // through a settle-window bug).
            steps: 1180,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 0.05 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.3), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "oscillating",
            display_name: "Oscillating",
            params: &[
                ("feed_a", 1.0),
                ("feed_b", 2.5),
                ("diffusion_x", 1.0),
                ("diffusion_y", 1.0),
            ],
            // Never stills, so this is a state rather than a settle
            // point: the field is developed and keeps moving.
            steps: 2000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 0.05 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.3), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
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

    let ca = mparam(0u);
    let cb = mparam(1u);
    let dx = mparam(2u);
    let dy = mparam(3u);
    let dt = sim_dt();

    let x = s.x;
    let y = s.y;
    let xxy = x * x * y;
    // Both species are concentrations: negative is unphysical and, fed
    // back through x*x*y, is how this scheme reaches NaN. Clamped at
    // zero for the same reason Gray-Scott clamps.
    let nx = max(x + dt * (dx * lap.x + ca - (cb + 1.0) * x + xxy), 0.0);
    let ny = max(y + dt * (dy * lap.y + cb * x - xxy), 0.0);

    let moved = abs(nx - x) > 1.0e-4;
    let age = select(s.z, f32(sim_step_index()), moved);
    return vec4<f32>(nx, ny, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // The fixed point plus a small perturbation: the instability grows
    // from any asymmetry, and starting exactly ON the fixed point is a
    // stationary solution that never leaves it.
    //
    // Two INDEPENDENT draws. sim_rand is in scope, so a model needing
    // more randomness than the one `noise` argument carries takes it
    // rather than reusing one value for both species.
    let ca = mparam(0u);
    let cb = mparam(1u);
    let n0 = (sim_rand(p, 0x71u) - 0.5) * 0.1;
    let n1 = (sim_rand(p, 0x72u) - 0.5) * 0.1;
    return vec4<f32>(max(ca + n0, 0.0), max(cb / max(ca, 1.0e-3) + n1, 0.0), 0.0, 0.0);
}
"#,
    default_steps: 1180,
    // Measured ladder: 0.01, 0.02, 0.03 and 0.04 all stable with
    // identical spatial sd; 0.05 diverges at step 90. An earlier note
    // said 0.02 after testing only 0.01 and 0.05 -- a cap written down
    // without running the rung it names.
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &["diffusion_x", "diffusion_y"],
    max_dt: 0.04,
    default_dt: 0.01,
};

/// Schnakenberg, the minimal two-species Turing system.
///
/// ```text
/// du/dt = D_u lap(u) + a - u + u^2 v
/// dv/dt = D_v lap(v) + b - u^2 v
/// ```
///
/// Fixed point u = a + b, v = b/(a+b)^2. Needs a large diffusion ratio.
///
/// Channels: `.x` = u, `.y` = v, `.z` = age, `.w` spare.
pub static SCHNAKENBERG: ModelDef = ModelDef {
    name: "schnakenberg",
    display_name: "Schnakenberg",
    description: "Minimal Turing system: spots at a large diffusion ratio. Feature size \
                  scales as the square root of the diffusion rates.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "a",
            display_name: "a",
            default: 0.1,
            min: 0.0,
            max: 1.0,
            tooltip: "Supply of u. With b it fixes the uniform state at u = a + b.",
            choices: &[],
        },
        SimParamDef {
            name: "b",
            display_name: "b",
            default: 0.9,
            min: 0.0,
            max: 2.0,
            tooltip: "Supply of v. The a:b ratio decides whether spots or gaps form.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_u",
            display_name: "Diffusion u",
            default: 1.0,
            min: 0.0,
            max: 4.0,
            tooltip: "Spread of the activator.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_v",
            display_name: "Diffusion v",
            default: 40.0,
            min: 0.0,
            max: 80.0,
            tooltip: "Spread of the inhibitor; about 40x the activator's for spots. \
                      Scaling BOTH rates together grows the features — measured, 16x the \
                      rates gives 4.6x the wavelength, and costs proportionally more steps.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "turing_spots",
        display_name: "Turing spots",
        params: &[("a", 0.1), ("b", 0.9), ("diffusion_u", 1.0), ("diffusion_v", 40.0)],
        // Measured settle point; an independent run in the wavelength
        // sweep gave 5,100 from a different seed.
        steps: 4900,
        init: Some(crate::config::sim::SimInit::Noise { amplitude: 0.02 }),
        coloring: Some("channel"),
        coloring_params: &[("channel", 0.0), ("scale", 0.5), ("offset", 0.0), ("wrap", 0.0)],
        matte: None,
    }],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
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

    let ca = mparam(0u);
    let cb = mparam(1u);
    let du = mparam(2u);
    let dv = mparam(3u);
    let dt = sim_dt();

    let u = s.x;
    let v = s.y;
    let uuv = u * u * v;
    let nu = max(u + dt * (du * lap.x + ca - u + uuv), 0.0);
    let nv = max(v + dt * (dv * lap.y + cb - uuv), 0.0);

    let moved = abs(nu - u) > 1.0e-4;
    let age = select(s.z, f32(sim_step_index()), moved);
    return vec4<f32>(nu, nv, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    let ca = mparam(0u);
    let cb = mparam(1u);
    let u0 = ca + cb;
    let v0 = cb / max(u0 * u0, 1.0e-3);
    let n0 = (sim_rand(p, 0x81u) - 0.5) * 0.04;
    let n1 = (sim_rand(p, 0x82u) - 0.5) * 0.04;
    return vec4<f32>(max(u0 + n0, 0.0), max(v0 + n1, 0.0), 0.0, 0.0);
}
"#,
    default_steps: 4900,
    // Every rung run: 0.01 and 0.02 stable, 0.03 diverges at step 486,
    // 0.04 at 26, 0.05 at 17.
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &["diffusion_u", "diffusion_v"],
    max_dt: 0.02,
    default_dt: 0.01,
};


// ---------------------------------------------------------------------
// Cellular automata.
//
// Integer state rides in an f32 channel: every integer up to 2^24 is
// exact, which covers every state count in the catalogue, and it keeps
// one binding path instead of adding an r32uint texture for no gain.
// None of these has a time step -- a generation is a generation -- so
// they declare NoTimeStep and the panel hides the dt slider.
// ---------------------------------------------------------------------

/// The hodgepodge machine, a Belousov-Zhabotinsky cellular automaton.
///
/// States 0..q: 0 healthy, q ill, everything between infected. **Two
/// rules ship, because the one everybody quotes is not the one the
/// paper states**, and both make BZ scrolls.
///
/// Gerhardt and Schuster, *Physica D* 36 (1989) 209-221, eqs. (3)-(9),
/// with K the count of ILL neighbours, I the count of INFECTED ones
/// and S the sum of the states of the infected cells only:
///
/// ```text
/// healthy:   s' = floor(K/k1) + floor(I/k2)
/// infected:  s' = min(floor(S/I) + g, q)
/// ill:       s' = 0
/// ```
///
/// Figure 2's caption -- "the center cell is always considered as a
/// neighbour of itself" -- is what keeps I >= 1 for an infected cell,
/// so that division is always defined.
///
/// The widely circulated version (Dewdney's *Scientific American*
/// column and the implementations descended from it) differs in three
/// places: k1 and k2 are swapped, S runs over ALL cells rather than
/// the infected ones, and the divisor is A + B + 1. That is what this
/// engine shipped in phase 2, from a secondary source and flagged
/// `[verify]` in the catalogue; reading the paper is what turned the
/// flag into this parameter. It is kept because it is a real variant
/// with its own look -- coarser, rounder scrolls -- and because a
/// preset shipped under it.
///
/// Measured: with q=200, k1=2, k2=3 both rules give a developed scroll
/// field by step 200 and NOT by step 50 -- at 50 it is one state with
/// scattered specks. They want different g, because the paper's rule
/// averages over the infected cells alone and its waves therefore run
/// faster: g = 70 for the circulated rule, g = 25 for the paper's,
/// where 70 gives a fine busy texture and 10 gives mush.
///
/// Channels: `.x` = state, `.z` = age, `.w` spare.
pub static HODGEPODGE: ModelDef = ModelDef {
    name: "hodgepodge",
    display_name: "Hodgepodge",
    description: "Belousov–Zhabotinsky as a cellular automaton: dense interlocking \
                  spirals and scrolls in an excitable integer medium.",
    features: &[ModelFeature::NeverStills, ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "states",
            display_name: "States (q)",
            default: 200.0,
            min: 4.0,
            max: 512.0,
            tooltip: "How many levels of infection. Larger q gives smoother spiral arms \
                      and a longer refractory period.",
            choices: &[],
        },
        SimParamDef {
            name: "k1",
            display_name: "k₁",
            default: 2.0,
            min: 1.0,
            max: 16.0,
            tooltip: "Divides a neighbour count when a healthy cell catches the infection —                       the ill count under Gerhardt–Schuster's rule, the infected count under                       Dewdney's, which is one of the three places the two differ. Larger                       values make infection harder to catch.",
            choices: &[],
        },
        SimParamDef {
            name: "k2",
            display_name: "k₂",
            default: 3.0,
            min: 1.0,
            max: 16.0,
            tooltip: "Divides the other neighbour count — infected under Gerhardt–Schuster,                       ill under Dewdney. With k₁ it sets how readily waves nucleate.",
            choices: &[],
        },
        SimParamDef {
            name: "g",
            display_name: "g (rate)",
            default: 25.0,
            min: 1.0,
            max: 200.0,
            tooltip: "How fast an infected cell progresses toward ill. Sets the wave speed                       and therefore the spiral pitch.",
            choices: &[],
        },
        SimParamDef {
            name: "variant",
            display_name: "Rule",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Which published rule to run. Gerhardt–Schuster is the 1989 paper's                       own: k₁ divides the ill count, and an infected cell averages the                       states of the infected cells around it. Dewdney's is the version that                       circulated afterwards — the counts swapped, and the average taken over                       every neighbour — and gives coarser, rounder scrolls.",
            choices: &["Gerhardt–Schuster", "Dewdney"],
        },
    ],
    presets: &[
        SimPreset {
            name: "spirals",
            display_name: "BZ spirals",
            params: &[
                ("states", 200.0),
                ("k1", 2.0),
                ("k2", 3.0),
                ("g", 25.0),
                ("variant", 0.0),
            ],
            // Developed by 200; at 50 it is still one state with
            // specks, which an earlier note claimed was "developed".
            //
            // g = 25 rather than the 70 the circulated rule uses: the
            // paper's rule averages over the infected cells alone, so
            // its waves run faster, and at 70 the scrolls are a fine
            // busy texture. At 25 they open out into scrolls with
            // visible spiral cores; at 10 the field is mush.
            steps: 200,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.005), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "dewdney",
            display_name: "BZ spirals (Dewdney rule)",
            params: &[
                ("states", 200.0),
                ("k1", 2.0),
                ("k2", 3.0),
                ("g", 70.0),
                ("variant", 1.0),
            ],
            steps: 200,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.005), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn hp_count(c: bool) -> f32 {
    return select(0.0, 1.0, c);
}

// A neighbour's state if it is INFECTED, and zero otherwise: the sum
// in Gerhardt-Schuster eq. (6) runs over the infected cells only.
fn hp_inf(n: f32, q: f32) -> f32 {
    return select(0.0, n, n > 0.0 && n < q);
}

fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let q = floor(mparam(0u));
    let k1 = max(floor(mparam(1u)), 1.0);
    let k2 = max(floor(mparam(2u)), 1.0);
    let g = floor(mparam(3u));

    // Unrolled and branchless. Measured at 1080p: the 3x3 loop with an
    // if/else chain per neighbour cost 0.77 ms/step, the same loop
    // with selects 0.69, and this 0.29 -- so the loop was the cost,
    // not the branches, and this 8-read kernel now costs what the
    // 8-read reaction-diffusion kernels do.
    let n0 = sim_read(p + vec2<i32>(-1, -1)).x;
    let n1 = sim_read(p + vec2<i32>(0, -1)).x;
    let n2 = sim_read(p + vec2<i32>(1, -1)).x;
    let n3 = sim_read(p + vec2<i32>(-1, 0)).x;
    let n4 = sim_read(p + vec2<i32>(1, 0)).x;
    let n5 = sim_read(p + vec2<i32>(-1, 1)).x;
    let n6 = sim_read(p + vec2<i32>(0, 1)).x;
    let n7 = sim_read(p + vec2<i32>(1, 1)).x;
    let total = s.x + n0 + n1 + n2 + n3 + n4 + n5 + n6 + n7;
    let ill = hp_count(n0 >= q) + hp_count(n1 >= q) + hp_count(n2 >= q) + hp_count(n3 >= q)
        + hp_count(n4 >= q) + hp_count(n5 >= q) + hp_count(n6 >= q) + hp_count(n7 >= q);
    // Infected = nonzero and not ill.
    let nonzero = hp_count(n0 > 0.0) + hp_count(n1 > 0.0) + hp_count(n2 > 0.0)
        + hp_count(n3 > 0.0) + hp_count(n4 > 0.0) + hp_count(n5 > 0.0)
        + hp_count(n6 > 0.0) + hp_count(n7 > 0.0);
    let infected = nonzero - ill;
    // Gerhardt-Schuster eq. (6): the sum over the INFECTED cells only.
    let inf_sum = hp_inf(n0, q) + hp_inf(n1, q) + hp_inf(n2, q) + hp_inf(n3, q)
        + hp_inf(n4, q) + hp_inf(n5, q) + hp_inf(n6, q) + hp_inf(n7, q);

    let paper = mparam(4u) < 0.5;
    let cur = s.x;
    var next = 0.0;
    if (cur >= q) {
        // Ill cells recover completely (eq. 8).
        next = 0.0;
    } else if (cur <= 0.0) {
        // Healthy. The paper divides the ILL count by k1; the
        // circulated rule has the two the other way round.
        next = select(
            floor(infected / k1) + floor(ill / k2),
            floor(ill / k1) + floor(infected / k2),
            paper,
        );
    } else {
        // Infected. The paper averages the states of the infected
        // cells INCLUDING this one -- figure 2's caption makes the
        // cell its own neighbour, which is what keeps the divisor at
        // or above 1 and the division defined. The circulated rule
        // averages every cell and divides by A + B + 1.
        next = select(
            floor(total / (infected + ill + 1.0)) + g,
            floor((inf_sum + cur) / (infected + 1.0)) + g,
            paper,
        );
    }
    next = clamp(next, 0.0, q);

    let moved = next != cur;
    let age = select(s.z, f32(sim_step_index()), moved);
    return vec4<f32>(next, 0.0, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Uniform random states. The mask is ignored: this medium needs a
    // disordered start everywhere, and seeding a shape into it just
    // leaves the rest healthy and inert.
    let q = floor(mparam(0u));
    return vec4<f32>(floor(sim_rand(p, 0x91u) * (q + 1.0)), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 200,
    // No time step: a generation is a generation. The value is unused,
    // and the panel hides the slider.
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};

/// Cyclic cellular automaton (Fisch-Gravner-Griffeath).
///
/// A cell in state `s` advances to `(s + 1) mod N` when at least `T` of
/// its neighbours within range `R` are already there. From random
/// states the system passes through debris, then droplets, then
/// spirals.
///
/// Measured: 1/1/14 von Neumann is fully spiralled by ~300 steps and
/// gives the characteristic 45-degree diamond fronts of a range-1 von
/// Neumann neighbourhood; 1/3/3 Moore develops in ~7 and is far
/// quieter. The two want very different `steps`, which is why the
/// defaults are per-preset.
///
/// Channels: `.x` = state, `.z` = age, `.w` spare.
pub static CYCLIC_CA: ModelDef = ModelDef {
    name: "cyclic_ca",
    display_name: "Cyclic CA",
    description: "States chase each other around a cycle: debris coarsens into droplets \
                  and then into spirals whose cores never settle.",
    features: &[ModelFeature::NeverStills, ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "states",
            display_name: "States (N)",
            default: 14.0,
            min: 3.0,
            max: 24.0,
            tooltip: "Length of the cycle. More states means a longer path back around, \
                      so spirals have more arms and take longer to form.",
            choices: &[],
        },
        SimParamDef {
            name: "range",
            display_name: "Range (R)",
            default: 1.0,
            min: 1.0,
            max: 5.0,
            tooltip: "Neighbourhood radius. Cost grows as R²: range 5 is 121 taps a cell.",
            choices: &[],
        },
        SimParamDef {
            name: "threshold",
            display_name: "Threshold (T)",
            default: 1.0,
            min: 1.0,
            max: 25.0,
            tooltip: "How many neighbours must already hold the next state. Written R/T/N \
                      in the literature.",
            choices: &[],
        },
        SimParamDef {
            name: "neighbourhood",
            display_name: "Neighbourhood",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Von Neumann counts the diamond (|dx| + |dy| ≤ R) and gives 45° \
                      fronts; Moore counts the square.",
            choices: &["Von Neumann", "Moore"],
        },
    ],
    presets: &[
        SimPreset {
            name: "spirals_1_1_14",
            display_name: "1/1/14 spirals",
            params: &[
                ("states", 14.0),
                ("range", 1.0),
                ("threshold", 1.0),
                ("neighbourhood", 0.0),
            ],
            steps: 300,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.0714286), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "moore_1_3_3",
            display_name: "1/3/3 Moore",
            params: &[
                ("states", 3.0),
                ("range", 1.0),
                ("threshold", 3.0),
                ("neighbourhood", 1.0),
            ],
            steps: 60,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.3333333), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn cyc_hit(q: vec2<i32>, want: f32) -> f32 {
    return select(0.0, 1.0, floor(sim_read(q).x) == want);
}

fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let n_states = max(floor(mparam(0u)), 2.0);
    let r = i32(clamp(floor(mparam(1u)), 1.0, 5.0));
    let thresh = max(floor(mparam(2u)), 1.0);
    let moore = mparam(3u) >= 0.5;

    let cur = floor(s.x);
    let want = fract_state(cur + 1.0, n_states);

    var count = 0.0;
    if (r == 1) {
        // The default, unrolled: a loop with a uniform bound cost
        // 0.44 ms/step at 1080p against 0.25 for the same four reads
        // written out (measured on Eden). Von Neumann first, the four
        // corners only for Moore.
        count = cyc_hit(p + vec2<i32>(0, -1), want) + cyc_hit(p + vec2<i32>(0, 1), want)
            + cyc_hit(p + vec2<i32>(-1, 0), want) + cyc_hit(p + vec2<i32>(1, 0), want);
        if (moore) {
            count = count + cyc_hit(p + vec2<i32>(-1, -1), want)
                + cyc_hit(p + vec2<i32>(1, -1), want)
                + cyc_hit(p + vec2<i32>(-1, 1), want)
                + cyc_hit(p + vec2<i32>(1, 1), want);
        }
    } else {
        for (var dy = -r; dy <= r; dy = dy + 1) {
            for (var dx = -r; dx <= r; dx = dx + 1) {
                if (dx == 0 && dy == 0) {
                    continue;
                }
                if (!moore && abs(dx) + abs(dy) > r) {
                    continue;
                }
                count = count + cyc_hit(p + vec2<i32>(dx, dy), want);
            }
        }
    }

    let advance = count >= thresh;
    let next = select(cur, want, advance);
    let age = select(s.z, f32(sim_step_index()), advance);
    return vec4<f32>(next, 0.0, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    let n_states = max(floor(mparam(0u)), 2.0);
    return vec4<f32>(floor(sim_rand(p, 0xa1u) * n_states), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 300,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};

/// Spatial rock-paper-scissors: cyclic competition on a lattice.
///
/// Each cell picks one random neighbour; if that neighbour's species
/// beats it, it is taken over with probability `p_sel`. An empty cell
/// is colonised by a random non-empty neighbour with probability
/// `p_rep`.
///
/// Measured: with p_sel = p_rep = 1 and three species, all three
/// coexist and the field develops by ~27 steps. The synchronous
/// parallel update differs from the paper's sequential random-site
/// Monte Carlo; the coexistence survives it, which is the property
/// worth checking.
///
/// Channels: `.x` = species (0 empty, 1..k), `.z` = age, `.w` spare.
pub static SPATIAL_RPS: ModelDef = ModelDef {
    name: "spatial_rps",
    display_name: "Spatial RPS",
    description: "Cyclic competition: each species beats the next and loses to the one \
                  before, and no species can win. Rotating domain spirals.",
    features: &[
        ModelFeature::NeedsRng,
        ModelFeature::NeverStills,
        ModelFeature::NoTimeStep,
    ],
    parameters: &[
        SimParamDef {
            name: "species",
            display_name: "Species",
            default: 3.0,
            min: 3.0,
            max: 5.0,
            tooltip: "Length of the dominance cycle. Five species (rock-paper-scissors-\
                      lizard-Spock) forms two nested levels of spiral.",
            choices: &[],
        },
        SimParamDef {
            name: "p_select",
            display_name: "Predation rate",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Chance a cell is taken over by a neighbour that beats it. Lower \
                      values slow the fronts and widen the domains.",
            choices: &[],
        },
        SimParamDef {
            name: "p_reproduce",
            display_name: "Colonisation rate",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Chance an empty cell is colonised by a neighbour.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "three_species",
        display_name: "Three species",
        params: &[("species", 3.0), ("p_select", 1.0), ("p_reproduce", 1.0)],
        steps: 400,
        init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
        coloring: Some("channel"),
        coloring_params: &[("channel", 0.0), ("scale", 0.25), ("offset", 0.0), ("wrap", 0.0)],
        matte: None,
    }],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let k = max(floor(mparam(0u)), 3.0);
    let p_sel = mparam(1u);
    let p_rep = mparam(2u);

    // One random neighbour of the eight. Unrolled rather than indexed:
    // WGSL has no dynamic vector indexing, and eight branches cost less
    // than the array round-trip would.
    let pick = i32(floor(sim_rand(p, 0xb1u) * 8.0));
    var off = vec2<i32>(-1, -1);
    if (pick == 1) { off = vec2<i32>(0, -1); }
    else if (pick == 2) { off = vec2<i32>(1, -1); }
    else if (pick == 3) { off = vec2<i32>(-1, 0); }
    else if (pick == 4) { off = vec2<i32>(1, 0); }
    else if (pick == 5) { off = vec2<i32>(-1, 1); }
    else if (pick == 6) { off = vec2<i32>(0, 1); }
    else if (pick >= 7) { off = vec2<i32>(1, 1); }

    let cur = floor(s.x);
    let other = floor(sim_read(p + off).x);
    var next = cur;

    if (cur <= 0.0) {
        // Empty: colonised by whatever it happened to look at.
        if (other > 0.0 && sim_rand(p, 0xb2u) < p_rep) {
            next = other;
        }
    } else if (other > 0.0) {
        // `other` beats `cur` when other == cur + 1 in the cycle, with
        // species numbered 1..k.
        let beats = fract_state(cur, k) + 1.0;
        if (other == beats && sim_rand(p, 0xb3u) < p_sel) {
            next = other;
        }
    }

    let moved = next != cur;
    let age = select(s.z, f32(sim_step_index()), moved);
    return vec4<f32>(next, 0.0, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Species 0..k inclusive: 0 is empty, so a fraction of the lattice
    // starts open and the fronts have somewhere to grow into.
    let k = max(floor(mparam(0u)), 3.0);
    return vec4<f32>(floor(sim_rand(p, 0xb0u) * (k + 1.0)), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 400,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};

/// The Ising model, Metropolis dynamics on a checkerboard.
///
/// Spins ±1; a flip costs `dE = 2 s (J * sum_neighbours + H)` and is
/// accepted with probability `min(1, exp(-dE / T))`. Onsager's critical
/// temperature is `2 / ln(1 + sqrt(2)) ~= 2.269`.
///
/// **One step is one HALF-sweep.** The two sublattices update
/// alternately, chosen by the step index's parity, because a cell's
/// energy then depends only on cells that are not moving — a fully
/// synchronous update breaks detailed balance and gives the wrong
/// statistics. So a sweep is two steps, and the presets' step counts
/// are twice their sweep counts.
///
/// Measured across the transition (600 sweeps from random, |m| over the
/// last 50): 0.90 at T = 1.5, 0.33 at T_c, 0.007 at T = 3.5.
///
/// Channels: `.x` = spin, `.z` = age, `.w` spare.
pub static ISING: ModelDef = ModelDef {
    name: "ising",
    display_name: "Ising",
    description: "Spins on a lattice at temperature T. Domains coarsen below the critical \
                  point, are fractal at it, and are noise above it.",
    features: &[ModelFeature::NeedsRng, ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "temperature",
            display_name: "Temperature",
            default: 2.269,
            min: 0.2,
            max: 5.0,
            tooltip: "Onsager's critical temperature is 2.269. Below it domains coarsen; \
                      at it the clusters are fractal at every scale; above it the lattice \
                      is uncorrelated noise.",
            choices: &[],
        },
        SimParamDef {
            name: "field",
            display_name: "External field (H)",
            default: 0.0,
            min: -1.0,
            max: 1.0,
            tooltip: "Biases one spin direction. Any nonzero value eventually magnetises \
                      the whole lattice.",
            choices: &[],
        },
        SimParamDef {
            name: "coupling",
            display_name: "Coupling (J)",
            default: 1.0,
            min: -2.0,
            max: 2.0,
            tooltip: "Positive couples neighbours (ferromagnetic); negative anti-couples \
                      them and gives a checkerboard ground state.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "critical",
            display_name: "Critical (T_c)",
            params: &[("temperature", 2.269), ("field", 0.0), ("coupling", 1.0)],
            // 600 sweeps; a step is a half-sweep.
            steps: 1200,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.5), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "coarsening",
            display_name: "Coarsening",
            params: &[("temperature", 1.5), ("field", 0.0), ("coupling", 1.0)],
            // Measured to need ~436 sweeps before it looks like anything.
            steps: 1000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.5), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // CHECKERBOARD. Only one sublattice moves per step, so every cell
    // this pass considers has neighbours that are all standing still.
    // A fully synchronous Metropolis update breaks detailed balance and
    // gives the wrong equilibrium, which is a wrong picture rather than
    // an obviously broken one.
    let parity = i32(sim_step_index() % 2u);
    if (((p.x + p.y) & 1) != parity) {
        return s;
    }

    let temp = max(mparam(0u), 1.0e-3);
    let h = mparam(1u);
    let j = mparam(2u);

    let spin = s.x;
    let neighbours = sim_read(p + vec2<i32>(0, -1)).x
        + sim_read(p + vec2<i32>(0, 1)).x
        + sim_read(p + vec2<i32>(-1, 0)).x
        + sim_read(p + vec2<i32>(1, 0)).x;
    let de = 2.0 * spin * (j * neighbours + h);

    // clamp before exp: at low temperature de/temp reaches the hundreds,
    // and exp of that is an infinity the comparison would still answer
    // correctly but which shows up in a NaN sweep as a false positive.
    let accept = de <= 0.0 || sim_rand(p, 0xc1u) < exp(-clamp(de / temp, 0.0, 60.0));
    let next = select(spin, -spin, accept);
    let age = select(s.z, f32(sim_step_index()), accept);
    return vec4<f32>(next, 0.0, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Infinite temperature: half up, half down.
    return vec4<f32>(select(-1.0, 1.0, sim_rand(p, 0xc0u) < 0.5), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 1200,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


// ---------------------------------------------------------------------
// Growth and deposition.
//
// These grow into empty space rather than filling it, so `age` is the
// colouring they are for: the growth rings ARE the picture, and
// colouring the occupancy alone shows only the final silhouette.
// ---------------------------------------------------------------------

/// The Eden growth model: a compact cluster with a rough, KPZ-class
/// interface.
///
/// Parallel formulation: every empty site with an occupied neighbour is
/// occupied with probability `p` per step. For small `p` this
/// approaches the sequential single-site process, and the interface
/// universality class is unchanged.
///
/// Measured steps to reach the edge of a 256 grid from a point seed:
/// 127 at p = 1 (exactly the radius), 256 at p = 0.3, 1,158 at
/// p = 0.05. So `radius / p` is exact at p = 1 and overestimates by
/// about 2x at small p, because the front is long and many sites get
/// their chance each step.
///
/// Channels: `.x` = occupied, `.z` = arrival step, `.w` spare.
pub static EDEN: ModelDef = ModelDef {
    name: "eden",
    display_name: "Eden Growth",
    description: "A cluster grown one site at a time into its neighbourhood. Compact, with \
                  a rough interface — the growth rings are the subject.",
    features: &[ModelFeature::NeedsRng, ModelFeature::NoTimeStep],
    parameters: &[SimParamDef {
        name: "p_grow",
        display_name: "Growth probability",
        default: 0.3,
        min: 0.01,
        max: 1.0,
        tooltip: "Chance an eligible empty site is occupied each step. Lower values are \
                  closer to the sequential Eden process and take proportionally longer; \
                  at 1.0 the cluster is a diamond, because every front site fires at once.",
        choices: &[],
    }],
    presets: &[
        SimPreset {
            name: "cluster",
            display_name: "Cluster",
            params: &[("p_grow", 0.3)],
            // Measured: 256 steps to reach the edge of a 256 grid.
            steps: 250,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("age"),
            coloring_params: &[("window", 250.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "rough_front",
            display_name: "Rough front",
            params: &[("p_grow", 0.3)],
            // A line seed grows upward; measured 505 steps to fill.
            // NOTE the boundary: with Periodic the bottom row wraps to
            // the top and the run ends on step one. The front cases
            // want Zero.
            steps: 500,
            init: Some(crate::config::sim::SimInit::Line),
            coloring: Some("age"),
            coloring_params: &[("window", 500.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Already grown: nothing ever un-grows.
    if (s.x > 0.5) {
        return s;
    }
    // Von Neumann adjacency: the diagonal-inclusive version rounds the
    // cluster off and loses the interface roughness that is the point.
    let n = sim_read(p + vec2<i32>(0, -1)).x
        + sim_read(p + vec2<i32>(0, 1)).x
        + sim_read(p + vec2<i32>(-1, 0)).x
        + sim_read(p + vec2<i32>(1, 0)).x;
    if (n < 0.5) {
        return s;
    }
    if (sim_rand(p, 0xd1u) >= mparam(0u)) {
        return s;
    }
    // Arrival step, which is what the `age` colouring draws as rings.
    return vec4<f32>(1.0, 0.0, f32(sim_step_index()), 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // The init shape IS the initial cluster: Center for a compact
    // cluster, Line for a growing front.
    let occupied = select(0.0, 1.0, inside >= 0.5);
    return vec4<f32>(occupied, 0.0, 0.0, 0.0);
}
"#,
    default_steps: 250,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};

/// Ballistic deposition: particles fall down columns and stick on
/// contact, building a correlated rough surface.
///
/// `h(i) <- max(h(i-1), h(i) + 1, h(i+1))` for a column that receives a
/// particle. The lateral term is what makes it ballistic: it builds
/// overhangs and correlates neighbouring columns. Turn it off and the
/// columns are independent (random deposition).
///
/// **The column heights live in channel `.y` of ROW 0**, and every cell
/// reads the three it needs from there. That keeps the rule cell-local
/// — no separate height buffer and no second dispatch shape — at the
/// cost of three extra reads per cell.
///
/// Measured at 256 columns, p = 0.5: 361 steps to fill with lateral
/// sticking and 452 without, so "about the grid height" is right to
/// within a factor of 1.4-1.8. The interface widths separate the two
/// variants cleanly (2.84 against 10.59), though a single realisation
/// does not pin the KPZ exponent — see the catalogue.
///
/// Channels: `.x` = occupied, `.y` = column height (row 0 only),
/// `.z` = arrival step.
pub static BALLISTIC_DEPOSITION: ModelDef = ModelDef {
    name: "ballistic_deposition",
    display_name: "Ballistic Deposition",
    description: "Particles fall and stick where they first touch. Lateral sticking builds \
                  overhangs and a correlated surface; without it the columns are independent.",
    features: &[ModelFeature::NeedsRng, ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "p_drop",
            display_name: "Drop probability",
            default: 0.5,
            min: 0.01,
            max: 1.0,
            tooltip: "Chance each column receives a particle per step. Lower values are \
                      closer to the sequential process, where one particle lands at a time.",
            choices: &[],
        },
        SimParamDef {
            name: "sideways",
            display_name: "Sticking",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Lateral sticking is what makes deposition BALLISTIC: a particle \
                      catches on a taller neighbour and leaves a void. Off, the columns \
                      never interact and the surface is uncorrelated.",
            choices: &["Vertical only", "Lateral (ballistic)"],
        },
    ],
    presets: &[
        SimPreset {
            name: "ballistic",
            display_name: "Ballistic",
            params: &[("p_drop", 0.5), ("sideways", 1.0)],
            steps: 360,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("age"),
            coloring_params: &[("window", 360.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "random_deposition",
            display_name: "Random deposition",
            params: &[("p_drop", 0.5), ("sideways", 0.0)],
            steps: 450,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("age"),
            coloring_params: &[("window", 450.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let g = sim_grid();
    // Heights live in .y of row 0. Reading three of them makes the rule
    // cell-local: every cell in a column derives the same new height.
    let h_l = sim_read(vec2<i32>(p.x - 1, 0)).y;
    let h_c = sim_read(vec2<i32>(p.x, 0)).y;
    let h_r = sim_read(vec2<i32>(p.x + 1, 0)).y;

    // One draw per COLUMN per step: keyed on (x, 0) so every cell in
    // the column agrees about whether a particle arrived.
    let hit = sim_rand(vec2<i32>(p.x, 0), 0xe1u) < mparam(0u);
    var h_new = h_c;
    if (hit) {
        if (mparam(1u) >= 0.5) {
            h_new = max(max(h_l, h_r), h_c + 1.0);
        } else {
            h_new = h_c + 1.0;
        }
    }
    h_new = min(h_new, f32(g.y));

    // Height is measured from the bottom, so a cell at row y sits at
    // height (g.y - y).
    let height_here = f32(g.y - p.y);
    let filled = height_here <= h_new;
    let was = s.x > 0.5;
    let age = select(s.z, f32(sim_step_index()), filled && !was);
    // Only row 0 carries the height register.
    let store_h = select(0.0, h_new, p.y == 0);
    return vec4<f32>(select(0.0, 1.0, filled), store_h, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // A flat substrate: nothing deposited, every column at height zero.
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
"#,
    default_steps: 360,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};

/// Wolfram's elementary cellular automata, drawn as a space-time
/// diagram.
///
/// The next state of a cell is bit `(4*left + 2*self + right)` of the
/// rule number. Rule 90 is left XOR right and draws Sierpinski's
/// triangle from a single seed; rule 30 is chaotic; rule 110 is
/// Turing-complete.
///
/// **The field is the diagram, not a state.** Row `t` is generation
/// `t`, so `steps` is the grid height exactly, by construction, and a
/// step writes one row rather than updating everything. Cells outside
/// the active row return unchanged, which means most threads in a
/// dispatch do nothing -- a 256-row image evaluates 256x more cells
/// than it writes. Each is trivial and the whole diagram is under a
/// millisecond, so the row-shaped dispatch the plan mentions stays a
/// phase-3 option rather than a need.
///
/// The bit convention was verified against independently computed
/// binomials: rule 90 from a single seed matches Pascal's triangle
/// mod 2 on 2,079 of 2,079 cells.
///
/// Channels: `.x` = cell state, `.z` = row index (its generation).
pub static WOLFRAM_ECA: ModelDef = ModelDef {
    name: "wolfram_eca",
    display_name: "Wolfram ECA",
    description: "One-dimensional binary automata drawn as space-time: rule 90 is \
                  Sierpinski's triangle, rule 30 is chaotic, rule 110 is Turing-complete.",
    features: &[ModelFeature::NoTimeStep],
    parameters: &[SimParamDef {
        name: "rule",
        display_name: "Rule",
        default: 90.0,
        min: 0.0,
        max: 255.0,
        tooltip: "The 8-bit rule number: bit (4·left + 2·self + right) gives the next \
                  state. 90 draws Sierpinski's triangle, 30 is chaotic, 110 is \
                  Turing-complete, 184 models traffic.",
        choices: &[],
    }],
    presets: &[
        SimPreset {
            name: "rule_90",
            display_name: "Rule 90 (Sierpinski)",
            params: &[("rule", 90.0)],
            steps: 256,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "rule_30",
            display_name: "Rule 30 (chaotic)",
            params: &[("rule", 30.0)],
            steps: 256,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "rule_110",
            display_name: "Rule 110 (universal)",
            params: &[("rule", 110.0)],
            steps: 256,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Row t is generation t. Only the active row is written; everything
    // already drawn stays drawn.
    let t = i32(sim_step_index()) + 1;
    if (p.y != t) {
        return s;
    }
    let above = p.y - 1;
    let l = sim_read(vec2<i32>(p.x - 1, above)).x;
    let c = sim_read(vec2<i32>(p.x, above)).x;
    let r = sim_read(vec2<i32>(p.x + 1, above)).x;
    let idx = u32(4.0 * l + 2.0 * c + r);
    let rule = u32(clamp(round(mparam(0u)), 0.0, 255.0));
    let bit = (rule >> idx) & 1u;
    return vec4<f32>(f32(bit), 0.0, f32(p.y), 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Only the top row is seeded; the diagram grows downward into the
    // rest. Center gives the single cell that draws Sierpinski's
    // triangle, Noise gives a random first generation.
    if (p.y != 0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // The init shapes are 2-D, but generation 0 is one row. Evaluating
    // the mask AT THIS CELL would put a Center seed at the grid's
    // middle -- a row this model never writes -- so the whole diagram
    // came out blank. Sample the shape on the centre row instead and
    // read it along x: Center becomes the centre COLUMN, which is the
    // single seed rule 90 needs, and Noise still varies per column.
    let g = sim_grid();
    let m = sim_init_mask(vec2<i32>(p.x, g.y / 2));
    return vec4<f32>(select(0.0, 1.0, m >= 0.5), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 256,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


// ---------------------------------------------------------------------
// The two that need addressing the square stencil does not provide: a
// hexagonal lattice, and a gather at an arbitrary cell.
// ---------------------------------------------------------------------

/// Packard's digital snowflake on a hexagonal lattice.
///
/// A vacant cell freezes when its number of frozen neighbours is in a
/// chosen set S; Packard's rules are named by that set (1, 13, 134,
/// 1345, 1356). Rule 1 -- freeze on exactly one frozen neighbour --
/// grows the classic plate with branches.
///
/// **The lattice is hexagonal, stored as offset rows**: odd rows sit
/// half a cell to the right, so the six neighbour offsets depend on row
/// parity. That is the awkward part and it is written once here. A
/// wrong parity is not subtle in the output -- the six-fold symmetry
/// collapses to four-fold, which is exactly what the visual baseline
/// pins.
///
/// The resolve still samples the offset grid as a square one, which
/// shears each cell by half a width. At the scale a snowflake is viewed
/// this reads as a clean hexagon (the CPU prototype's images agree); a
/// true axial-to-pixel resolve is a later refinement, not a
/// correctness gap.
///
/// Measured: rules {1}, {1,3} and {1,3,4} all reach the edge of a 256
/// grid in exactly 125 steps, which is the radius. `steps ~ radius` is
/// exact rather than approximate, because the fastest growth direction
/// advances one cell per step whatever the rule. What the rule changes
/// is density -- 45%, 57% and 66% of the disc filled.
///
/// Channels: `.x` = frozen, `.z` = freeze step, `.w` spare.
pub static PACKARD_SNOWFLAKE: ModelDef = ModelDef {
    name: "packard_snowflake",
    display_name: "Packard Snowflake",
    description: "Hexagonal solidification: a vacant cell freezes on a chosen count of \
                  frozen neighbours. Plates, branches and dendrites.",
    features: &[ModelFeature::NoTimeStep],
    parameters: &[SimParamDef {
        name: "rule_mask",
        display_name: "Freeze on",
        default: 2.0,
        min: 1.0,
        max: 126.0,
        tooltip: "A 6-bit mask: bit n set means a cell freezes when exactly n of its six \
                  neighbours are frozen. 2 is Packard's rule 1 (one neighbour), 10 is \
                  rule 13, 26 is rule 134.",
        choices: &[],
    }],
    presets: &[
        SimPreset {
            name: "rule_1",
            display_name: "Rule 1 (plate)",
            params: &[("rule_mask", 2.0)],
            steps: 125,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("age"),
            coloring_params: &[("window", 125.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "rule_13",
            display_name: "Rule 13",
            params: &[("rule_mask", 10.0)],
            steps: 125,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("age"),
            coloring_params: &[("window", 125.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "rule_134",
            display_name: "Rule 134",
            params: &[("rule_mask", 26.0)],
            steps: 125,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("age"),
            coloring_params: &[("window", 125.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
    ],
    wgsl: r#"
// The six neighbours of an offset-row hex lattice. Odd rows are shifted
// half a cell right, so the two diagonals on each side move with the
// parity -- the two horizontal neighbours do not.
fn hex_frozen_count(p: vec2<i32>) -> f32 {
    var n = sim_read(p + vec2<i32>(-1, 0)).x + sim_read(p + vec2<i32>(1, 0)).x;
    if ((p.y & 1) == 1) {
        n = n + sim_read(p + vec2<i32>(0, -1)).x
              + sim_read(p + vec2<i32>(1, -1)).x
              + sim_read(p + vec2<i32>(0, 1)).x
              + sim_read(p + vec2<i32>(1, 1)).x;
    } else {
        n = n + sim_read(p + vec2<i32>(0, -1)).x
              + sim_read(p + vec2<i32>(-1, -1)).x
              + sim_read(p + vec2<i32>(0, 1)).x
              + sim_read(p + vec2<i32>(-1, 1)).x;
    }
    return n;
}

fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Frozen is permanent: this is solidification, not an automaton
    // that breathes.
    if (s.x > 0.5) {
        return s;
    }
    let count = i32(round(hex_frozen_count(p)));
    if (count < 1 || count > 6) {
        return s;
    }
    let mask = u32(clamp(round(mparam(0u)), 0.0, 126.0));
    if (((mask >> u32(count)) & 1u) == 0u) {
        return s;
    }
    return vec4<f32>(1.0, 0.0, f32(sim_step_index()), 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // The crystal nucleus. Center is the single seed the named rules
    // are defined from.
    return vec4<f32>(select(0.0, 1.0, inside >= 0.5), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 125,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};

/// Site percolation, coloured by connected component.
///
/// A static random field -- each site open with probability `p` -- and
/// then LABEL PROPAGATION: every open site takes the smallest label
/// among itself and its open neighbours, until nothing changes. At
/// `p_c = 0.592746` the spanning cluster is a fractal of dimension
/// 91/48.
///
/// **The label also chases its own pointer.** Plain propagation moves
/// a label one cell per step, so it costs the longest CHEMICAL path in
/// the cluster -- measured at p_c, a median 645 rounds at 256² with a
/// range of 485 to 760, and a 4x spread between samples at one size
/// because a critical cluster's longest path is not self-averaging.
/// Reading the cell a label points AT compresses the path, and the
/// count drops by more than an order of magnitude (measured below).
///
/// Measured with compression, against phase 0's plain-propagation
/// medians: 53 rounds at 64², 93 at 128², **167 at 256² against 645**,
/// and 491 at 512² against 1,409 -- so it is worth 3.9x and 2.9x at the
/// two sizes there is a comparison for.
///
/// Labels only ever decrease, so extra steps are no-ops: over-running
/// is safe and only costs time. That is why this model needs no settle
/// reduction to be CORRECT -- a settle would be an optimisation, and
/// the plan's reduction stage is deferred on that basis. The presets
/// carry roughly twice the measured count because the requirement is
/// NOT self-averaging: at p_c the longest chemical path varied
/// four-fold between samples at one size.
///
/// Channels: `.x` = label (a cell index, exact in f32 to 2²⁴),
/// `.y` = open, `.z` = the step it last changed.
pub static PERCOLATION: ModelDef = ModelDef {
    name: "percolation",
    display_name: "Percolation",
    description: "Random open sites, coloured by which connected cluster they belong to. \
                  At the critical threshold the spanning cluster is fractal.",
    features: &[ModelFeature::NeedsRng, ModelFeature::NoTimeStep],
    parameters: &[SimParamDef {
        name: "p_open",
        display_name: "Open probability",
        default: 0.592746,
        min: 0.3,
        max: 0.9,
        tooltip: "Fraction of sites that are open. The square-lattice site threshold is \
                  0.592746: below it every cluster is finite, above it one spans the \
                  grid, and at it the spanning cluster is fractal at every scale.",
        choices: &[],
    }],
    presets: &[
        SimPreset {
            name: "critical",
            display_name: "Critical (p_c)",
            params: &[("p_open", 0.592746)],
            steps: 400,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("label"),
            coloring_params: &[("channel", 0.0), ("mask_channel", 1.0)],
            matte: None,
        },
        SimPreset {
            name: "subcritical",
            display_name: "Below threshold",
            params: &[("p_open", 0.45)],
            steps: 400,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("label"),
            coloring_params: &[("channel", 0.0), ("mask_channel", 1.0)],
            matte: None,
        },
        SimPreset {
            name: "supercritical",
            display_name: "Above threshold",
            params: &[("p_open", 0.75)],
            steps: 400,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("label"),
            coloring_params: &[("channel", 0.0), ("mask_channel", 1.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Closed sites never carry a label.
    if (s.y < 0.5) {
        return s;
    }
    let g = sim_grid();
    var best = s.x;

    // One cell of propagation, through OPEN neighbours only -- a label
    // must not cross a closed site or the clusters merge into one.
    let n0 = sim_read(p + vec2<i32>(-1, 0));
    let n1 = sim_read(p + vec2<i32>(1, 0));
    let n2 = sim_read(p + vec2<i32>(0, -1));
    let n3 = sim_read(p + vec2<i32>(0, 1));
    if (n0.y > 0.5) { best = min(best, n0.x); }
    if (n1.y > 0.5) { best = min(best, n1.x); }
    if (n2.y > 0.5) { best = min(best, n2.x); }
    if (n3.y > 0.5) { best = min(best, n3.x); }

    // Path compression: a label is a cell index, so read the cell it
    // points at and take ITS label. This is what turns a walk down the
    // cluster's longest chemical path into something logarithmic.
    let li = i32(clamp(best, 0.0, f32(g.x * g.y - 1)));
    let lp = vec2<i32>(li % g.x, li / g.x);
    // `target` is a WGSL reserved keyword; the assembler's naga matrix
    // caught it before it could reach a device.
    let root = sim_read(lp);
    if (root.y > 0.5) {
        best = min(best, root.x);
    }

    let moved = best != s.x;
    let age = select(s.z, f32(sim_step_index()), moved);
    return vec4<f32>(best, s.y, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Each site open with probability p, and every open site starts as
    // its own cluster: the label is the cell's own index, so the
    // smallest index in a component wins and names it.
    let g = sim_grid();
    let open = select(0.0, 1.0, sim_rand(p, 0xf1u) < mparam(0u));
    let idx = f32(p.y * g.x + p.x);
    return vec4<f32>(idx, open, 0.0, 0.0);
}
"#,
    default_steps: 400,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


// ---------------------------------------------------------------------
// Fourth-order PDEs.
//
// These declare `passes: 2`. A fourth-order operator cannot be applied
// in one dispatch: the second derivative of a derivative needs the
// NEIGHBOURS' first-pass values, which do not exist until every cell
// has been written. So pass 1 stores its derivative into `.y` and pass
// 2 takes the derivative of that.
//
// THEY DO NOT USE THE SIMS LAPLACIAN. Phase 1 and 2's reaction-
// diffusion models take Karl Sims' 3x3 kernel (centre -1, edge 0.2,
// corner 0.05), whose Fourier symbol is -0.3*k^2 at small k -- a
// Laplacian scaled by 0.3. For a second-order model that scale is
// absorbed into the free diffusion constant and nothing observable
// changes. Here it would: Swift-Hohenberg's operator SELECTS the
// wavelength at which lap = -q0^2, so a 0.3 scale would move the
// selected wavelength by 1/sqrt(0.3) and the documented
// lambda = 2*pi/q0 would be wrong by 83%. Both models below use the
// standard 5-point Laplacian (centre -4, edge 1), which is what their
// stability bounds are stated against and what the prototype measured.
// ---------------------------------------------------------------------

/// Swift-Hohenberg: the canonical pattern-forming equation.
///
/// ```text
/// du/dt = r u - (q0^2 + lap)^2 u + g u^2 - u^3
/// ```
///
/// The quartic operator is a band-pass filter peaked at |k| = q0: it
/// amplifies one wavelength and damps the rest, which is why this is
/// the equation to reach for when the spacing itself is the subject.
///
/// **The drive is exposed RELATIVE to the band's selectivity, not as
/// the literal r**, and that is a measured decision rather than a
/// convenience. Growth at the band is r; growth at the uniform mode is
/// r - q0^4. So the equation only selects a wavelength when r is small
/// compared with q0^4 -- and q0^4 is tiny for any wavelength worth
/// looking at (2.4e-2 at 16 cells, 9.3e-5 at 64). The textbook q0 = 1
/// hides this, because q0^4 = 1 swamps any sensible r. Measured at
/// lambda = 16: r = 0.2 (the value the catalogue carried) gives
/// r/q0^4 = 8.4 and the field phase-separates into ~100-cell blobs
/// with no pattern at all, while every ratio from 0.1 to 4 gives a
/// clean 16.5-cell pattern. One slider position therefore has to mean
/// the same thing at every wavelength, so `drive` IS that ratio and
/// `r = drive * q0^4`.
///
/// Measured (256^2, dt = 0.0292, 12,000 steps): at drive 2 the
/// wavelength is 16.5 cells against the 16.0 the theory predicts, with
/// a spectral peak 21x the mean -- so `lambda = 2*pi/q0` is confirmed,
/// which is the claim most at risk from the discretisation.
///
/// Channels: `.x` = u, `.y` = the pass-1 scratch `w = q0^2 u + lap u`,
/// `.z` = age, `.w` spare.
pub static SWIFT_HOHENBERG: ModelDef = ModelDef {
    name: "swift_hohenberg",
    display_name: "Swift–Hohenberg",
    description: "The canonical pattern-forming equation: one wavelength is amplified and \
                  every other damped, giving a labyrinth with a definite spacing.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "wavelength",
            display_name: "Wavelength (cells)",
            default: 16.0,
            min: 6.0,
            max: 64.0,
            tooltip: "The spacing the equation selects, in cells — it is 2π/q₀, and the \
                      measurement agrees with it to 3%. Larger wavelengths need \
                      proportionally more steps, because the pattern grows on a 1/r \
                      timescale and r falls as the fourth power of 1/wavelength.",
            choices: &[],
        },
        SimParamDef {
            name: "drive",
            display_name: "Drive (r / q₀⁴)",
            default: 2.0,
            min: 0.05,
            max: 4.0,
            tooltip: "How hard the pattern is driven, in units of the band's own \
                      selectivity — the literal r is this times q₀⁴. Low values grow \
                      slowly and give a faint, very regular pattern; above about 4 the \
                      uniform mode grows nearly as fast as the pattern and the field \
                      phase-separates into blobs instead.",
            choices: &[],
        },
        SimParamDef {
            name: "asymmetry",
            display_name: "Asymmetry (g)",
            default: 0.0,
            min: 0.0,
            max: 0.35,
            tooltip: "Breaks the symmetry between peaks and troughs. At 0 there is no \
                      reason to prefer either and the field makes a labyrinth of equal \
                      stripes; raising it pinches the stripes into spots. Past about \
                      0.35 the quadratic term overwhelms the cubic and the field goes \
                      uniform — measured, so the slider stops below it.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "labyrinth",
            display_name: "Labyrinth",
            params: &[("wavelength", 16.0), ("drive", 2.0), ("asymmetry", 0.0)],
            // Measured: settles at 4,600 steps at 256^2.
            steps: 5000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 1.2), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "spots",
            display_name: "Spots",
            params: &[("wavelength", 16.0), ("drive", 2.0), ("asymmetry", 0.25)],
            // Measured: settles at 5,900; skew -1.02 against the
            // labyrinth's +0.00, which is the asymmetry showing up.
            steps: 6000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 1.2), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
// The standard 5-point Laplacian, not the Sims kernel the
// reaction-diffusion models use. See the note above the model.
fn sh_lap_x(p: vec2<i32>, c: f32) -> f32 {
    return sim_read(p + vec2<i32>(0, -1)).x + sim_read(p + vec2<i32>(0, 1)).x
        + sim_read(p + vec2<i32>(-1, 0)).x + sim_read(p + vec2<i32>(1, 0)).x
        - 4.0 * c;
}

fn sh_lap_y(p: vec2<i32>, c: f32) -> f32 {
    return sim_read(p + vec2<i32>(0, -1)).y + sim_read(p + vec2<i32>(0, 1)).y
        + sim_read(p + vec2<i32>(-1, 0)).y + sim_read(p + vec2<i32>(1, 0)).y
        - 4.0 * c;
}

fn sh_q0() -> f32 {
    // Wavelength in cells to wavenumber. Guarded: a wavelength of zero
    // is not reachable through the slider, but a hand-edited config
    // carries whatever it likes.
    return 6.28318530718 / max(mparam(0u), 1.0);
}

// Pass 1: w = q0^2 u + lap u, into the scratch channel.
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let q0 = sh_q0();
    let w = q0 * q0 * s.x + sh_lap_x(p, s.x);
    return vec4<f32>(s.x, w, s.z, 0.0);
}

// Pass 2: (q0^2 + lap)^2 u is (q0^2 + lap) applied to w.
fn sim_step2(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let q0 = sh_q0();
    let q4 = q0 * q0 * q0 * q0;
    // The drive is a RATIO to the band selectivity; see the model docs.
    let r = mparam(1u) * q4;
    let g = mparam(2u);
    let dt = sim_dt();
    let u = s.x;
    let bi = q0 * q0 * s.y + sh_lap_y(p, s.y);
    let nu = u + dt * (r * u - bi + g * u * u - u * u * u);
    // A guard, not a mechanism: the cubic saturates the growth near
    // |u| = 1 and the dt cap keeps the linear operator stable.
    // Measured, the field settles inside [-1, 1]; if this ever binds,
    // the bound is wrong.
    let cl = clamp(nu, -10.0, 10.0);
    let age = select(s.z, f32(sim_step_index()), abs(cl - u) > 1.0e-5);
    return vec4<f32>(cl, s.y, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Small noise everywhere. The init shape is deliberately ignored:
    // u = 0 is a fixed point of the whole equation, so outside a shape
    // every cell would sit at exactly zero forever and the pattern
    // would grow only within it. The medium has to be perturbed
    // everywhere, which is why hodgepodge ignores its mask too.
    return vec4<f32>((noise - 0.5) * 0.2, 0.0, 0.0, 0.0);
}
"#,
    default_steps: 5000,
    passes: 2,
    repeat: None,
    // Explicit Euler on the 5-point Laplacian, whose symbol runs over
    // [-8, 0]: the quartic operator is largest at the checkerboard,
    // (8 - q0^2)^2, and the drive r offsets it. Measured at
    // lambda = 16: stable at 0.03249, diverges at 0.03574, and this
    // formula gives 0.0325.
    agents: None,
    kernel: None,
    dt_bound: Some(|p| {
        let q0 = 6.283_185_3 / p.get("wavelength").max(1.0);
        let q2 = q0 * q0;
        let stiff = (8.0 - q2) * (8.0 - q2) - p.get("drive") * q2 * q2;
        2.0 / stiff.max(1.0e-3)
    }),
    diffusion: &[],
    // The ceiling; `dt_bound` is what binds everywhere in the slider's
    // range (0.031 to 0.042 across it).
    max_dt: 0.05,
    default_dt: 0.03,
};

/// Cahn-Hilliard: phase separation, and the coarsening that follows.
///
/// ```text
/// dc/dt = D lap( c^3 - c - gamma lap c )
/// ```
///
/// A mixture that is unstable at its mean composition separates into
/// domains of c = +1 and c = -1 with interfaces of width ~sqrt(gamma),
/// and those domains then coarsen forever. At mean 0 the phases are
/// even and interleave as a labyrinth; away from 0 the minority phase
/// pinches off into droplets.
///
/// **The mean composition is conserved exactly.** The update is a
/// discrete divergence -- a Laplacian sums to zero over a periodic
/// lattice -- so the mean cannot drift except by rounding. Measured on
/// the CPU mirror over 40,000 steps: 1.2e-16. That is a property no
/// picture can fake, and it is what the GPU test pins.
///
/// Measured coarsening (256^2, 40,000 steps): the domain size grows
/// 6.2 -> 22.4 cells at mean 0, an exponent of 0.25 against the
/// Lifshitz-Slyozov 1/3. The shortfall is expected at this size -- 22
/// cells in a 256 box is already into finite-size effects -- and the
/// growth itself is the point.
///
/// Channels: `.x` = c, `.y` = the pass-1 chemical potential mu,
/// `.z` = age, `.w` spare.
pub static CAHN_HILLIARD: ModelDef = ModelDef {
    name: "cahn_hilliard",
    display_name: "Cahn–Hilliard",
    description: "A mixture separating into two phases and then coarsening: labyrinths at \
                  an even mix, droplets of the minority phase away from it.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "mobility",
            display_name: "Mobility (D)",
            default: 1.0,
            min: 0.1,
            max: 4.0,
            tooltip: "How fast material moves down the chemical-potential gradient. It \
                      scales time — but it scales the stable dt down by exactly as much, \
                      so raising it buys no speed.",
            choices: &[],
        },
        SimParamDef {
            name: "gamma",
            display_name: "γ (interface width)",
            default: 0.5,
            min: 0.1,
            max: 4.0,
            tooltip: "Sets the width of the boundary between the phases, which goes as √γ. \
                      Larger values give thicker, softer interfaces and a coarser pattern.",
            choices: &[],
        },
        SimParamDef {
            name: "mean",
            display_name: "Mean composition",
            default: 0.0,
            min: -0.6,
            max: 0.6,
            tooltip: "The mixture's overall balance, which the equation conserves exactly. \
                      At 0 the phases are even and interleave as a labyrinth; past about \
                      ±0.3 the minority phase pinches off into droplets.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "labyrinth",
            display_name: "Labyrinth",
            params: &[("mobility", 1.0), ("gamma", 0.5), ("mean", 0.0)],
            // Never stills -- it coarsens forever -- so `steps` is a
            // choice of how coarse. Measured at 256^2: domain size 8.5
            // cells at 1,000 steps, 13.2 at 5,000, 18.6 at 20,000.
            steps: 20000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.5), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "droplets",
            display_name: "Droplets",
            params: &[("mobility", 1.0), ("gamma", 0.5), ("mean", 0.4)],
            steps: 20000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.5), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn ch_lap_x(p: vec2<i32>, c: f32) -> f32 {
    return sim_read(p + vec2<i32>(0, -1)).x + sim_read(p + vec2<i32>(0, 1)).x
        + sim_read(p + vec2<i32>(-1, 0)).x + sim_read(p + vec2<i32>(1, 0)).x
        - 4.0 * c;
}

fn ch_lap_y(p: vec2<i32>, c: f32) -> f32 {
    return sim_read(p + vec2<i32>(0, -1)).y + sim_read(p + vec2<i32>(0, 1)).y
        + sim_read(p + vec2<i32>(-1, 0)).y + sim_read(p + vec2<i32>(1, 0)).y
        - 4.0 * c;
}

// Pass 1: the chemical potential mu = c^3 - c - gamma lap c.
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let gamma = mparam(1u);
    let c = s.x;
    let mu = c * c * c - c - gamma * ch_lap_x(p, c);
    return vec4<f32>(c, mu, s.z, 0.0);
}

// Pass 2: dc/dt = D lap mu. A Laplacian sums to zero over the lattice,
// so this form conserves the mean of c exactly -- which is the
// physical content of the equation, and what the test pins.
fn sim_step2(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let d = mparam(0u);
    let dt = sim_dt();
    let c = s.x;
    let nc = c + dt * d * ch_lap_y(p, s.y);
    // A NaN guard only, and deliberately far outside the dynamics:
    // measured, c settles inside [-1.03, 1.02]. Clamping near the
    // physical range would clip the overshoot at a sharp interface and
    // break conservation, which is worth more here than tidy bounds.
    let cl = clamp(nc, -4.0, 4.0);
    let age = select(s.z, f32(sim_step_index()), abs(cl - c) > 1.0e-5);
    return vec4<f32>(cl, s.y, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // A nearly uniform mixture at the configured mean, with just
    // enough noise to be unstable. The init shape is ignored for the
    // same reason Swift-Hohenberg ignores it, and for one more: the
    // mean is a quantity this model conserves, so a shape seed would
    // silently set a different one than the slider says.
    return vec4<f32>(mparam(2u) + (noise - 0.5) * 0.1, 0.0, 0.0, 0.0);
}
"#,
    default_steps: 20000,
    passes: 2,
    repeat: None,
    // Linearised about |c| = 1, the symbol is
    //   D L (3c^2 - 1 - gamma L),  L in [-8, 0]
    // whose most negative value is at the checkerboard L = -8:
    //   -8 D (3c^2 - 1) - 64 D gamma  =  -D (16 + 64 gamma).
    //
    // The catalogue derived 1/(32 D gamma) from the gamma term alone,
    // which is 0.0625 at the defaults -- and that is NOT a bound: the
    // prototype was finite for 400 steps there and infinite by 1,000,
    // which is why the first ladder called it stable. With the cubic
    // kept, this formula gives 0.04167, and the measurement is stable
    // at 0.041667 and diverges at 0.045833.
    agents: None,
    kernel: None,
    dt_bound: Some(|p| {
        let d = p.get("mobility").max(1.0e-3);
        let gamma = p.get("gamma").max(0.0);
        2.0 / (d * (16.0 + 64.0 * gamma))
    }),
    diffusion: &[],
    max_dt: 0.2,
    default_dt: 0.04,
};


/// The Oregonator, in Tyson and Fife's two-variable reduction.
///
/// ```text
/// eps du/dt = D_u lap(u) + u(1-u) - f v (u-q)/(u+q)
///     dv/dt = D_v lap(v) + u - v
/// ```
///
/// u is HBrO2 (the activator), v the oxidised catalyst. Verified
/// against J. J. Tyson and P. C. Fife, *J. Chem. Phys.* 73 (1980)
/// 2224, eq. (17) -- the paper writes the parameters (a, b) where the
/// later literature writes (q, f), and its Table I gives the
/// correspondence. The paper states eps << 1, q << 1 and f ~ 1
/// (eq. 16) but is analytic throughout and gives no numeric set for a
/// two-dimensional simulation, so the values here are measured rather
/// than quoted.
///
/// **What it does, measured.** Each seed fires ONE excitation wave
/// which propagates outward at constant speed and annihilates against
/// its neighbours, leaving the medium reduced behind it -- the
/// travelling-wave behaviour the BZ reaction is known for. It does not
/// re-fire on its own: at f = 1.4 the medium is excitable rather than
/// oscillatory, and 6 time units of running produced no second wave.
/// Lower f broadens the fronts until they merge; higher f narrows them
/// until a seed barely fires at all.
///
/// **Spirals were NOT obtained**, and the catalogue's remembered
/// "spiral waves for f ~ 1.4" is not reproduced. A broken front (the
/// engine's `BrokenWave` init, which nucleates FitzHugh-Nagumo's
/// spirals) was run at eps in {0.01, 0.02, 0.04} and f in
/// {1.4 ... 3.5}: in every case the free end RETRACTED and the front
/// healed into an expanding closed loop rather than curling. A
/// pacemaker -- an oscillatory disc inside an excitable bulk, which
/// is the mechanism the paper's own title and abstract are about --
/// fires and emits one ring, but a sustained target pattern did not
/// appear within the 1.5 time units tested. Both are recorded in the
/// catalogue as open rather than papered over.
///
/// Channels: `.x` = u, `.y` = v, `.z` = age, `.w` spare.
pub static OREGONATOR: ModelDef = ModelDef {
    name: "oregonator",
    display_name: "Oregonator",
    description: "The Belousov–Zhabotinskii reaction as chemistry rather than as an \
                  automaton: an oscillating medium that carries travelling wave trains.",
    features: &[ModelFeature::NeverStills],
    parameters: &[
        SimParamDef {
            name: "epsilon",
            display_name: "ε (timescale)",
            default: 0.04,
            min: 0.005,
            max: 0.3,
            tooltip: "How much faster the activator moves than the catalyst. Small values \
                      give sharp wave fronts and a stiffer solve — the stable time step is \
                      proportional to this.",
            choices: &[],
        },
        SimParamDef {
            name: "q",
            display_name: "q",
            default: 0.002,
            min: 0.0002,
            max: 0.05,
            tooltip: "The reaction's small parameter. It sets the activator's threshold, \
                      and the stable time step is proportional to it.",
            choices: &[],
        },
        SimParamDef {
            name: "f",
            display_name: "f (stoichiometry)",
            default: 1.4,
            min: 0.4,
            max: 3.5,
            tooltip: "Selects the regime. Around 1 the medium oscillates on its own and \
                      fills with travelling wave trains; above about 2.5 it is merely \
                      excitable, so a seed fires one wave and the field goes quiet.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_u",
            display_name: "Diffusion u",
            default: 1.0,
            min: 0.1,
            max: 2.0,
            tooltip: "Activator diffusion, which sets the wave speed.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion_v",
            display_name: "Diffusion v",
            default: 0.6,
            min: 0.0,
            max: 2.0,
            tooltip: "Catalyst diffusion. The real catalyst is a large ion and barely \
                      diffuses; zero is a defensible setting.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "waves",
        display_name: "Excitation waves",
        params: &[
            ("epsilon", 0.04),
            ("q", 0.002),
            ("f", 1.4),
            ("diffusion_u", 1.0),
            ("diffusion_v", 0.6),
        ],
        // Measured: each seed's front is well formed and the seeds
        // have begun to collide by 15,000 steps at dt = 1e-4.
        steps: 15000,
        init: Some(crate::config::sim::SimInit::Blobs { count: 5, radius: 5 }),
        coloring: Some("channel"),
        coloring_params: &[("channel", 0.0), ("scale", 1.2), ("offset", 0.0), ("wrap", 0.0)],
        matte: None,
    }],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let eps = max(mparam(0u), 1.0e-4);
    let q = max(mparam(1u), 1.0e-6);
    let f = mparam(2u);
    let du = mparam(3u);
    let dv = mparam(4u);
    let dt = sim_dt();

    // The same Sims kernel every other reaction-diffusion model here
    // uses: this is a second-order system, so the kernel's 0.3 scale
    // is absorbed into the free diffusion constants.
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

    let u = s.x;
    let v = s.y;
    // u + q > 0 for u >= 0, and u is clamped non-negative below, so
    // the denominator is bounded away from zero by q.
    let react = u * (1.0 - u) - f * v * (u - q) / (u + q);
    let nu = max(u + dt * (du * lap.x + react) / eps, 0.0);
    let nv = max(v + dt * (dv * lap.y + u - v), 0.0);

    // The step a cell last crossed into excitation, which is what the
    // `age` colouring draws as the wave's history.
    let fired = u <= 0.3 && nu > 0.3;
    let age = select(s.z, f32(sim_step_index()), fired);
    return vec4<f32>(nu, nv, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // The init shape is excited; the rest sits at the reduced state.
    // NOT at exactly zero for both: u = v = 0 is an exact fixed point
    // of the reaction, so a field seeded there never moves at all --
    // measured, a pacemaker in a zero field produced nothing.
    return vec4<f32>(select(0.0, 1.0, inside >= 0.5), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 15000,
    passes: 1,
    repeat: None,
    // The stiff term is the activator's threshold. Differentiating the
    // reaction, d/du[-f v (u-q)/(u+q)] = -2 f v q/(u+q)^2, which is
    // largest near u = q at -f v/(2q); with v of order 1 and the
    // diffusion term 1.6 D_u on the Sims stencil, both divided by eps:
    //   dt <= 2 eps / (f/(2q) + 1.6 D_u)
    // At the defaults that is 2.3e-4. Measured: stable at 5e-4, and at
    // 1e-3 the field collapses to zero rather than diverging -- the
    // max(.., 0) clamp turns the instability into death, which is why
    // the ladder judges by the field's amplitude and not by isfinite.
    agents: None,
    kernel: None,
    dt_bound: Some(|p| {
        let eps = p.get("epsilon").max(1.0e-4);
        let q = p.get("q").max(1.0e-6);
        2.0 * eps / (p.get("f") / (2.0 * q) + 1.6 * p.get("diffusion_u"))
    }),
    diffusion: &[],
    max_dt: 0.01,
    default_dt: 0.0001,
};

/// Kobayashi's phase-field dendrite.
///
/// ```text
/// tau dp/dt = div(J) + p(1-p)(p - 1/2 + m) + a p(1-p) chi
///     dT/dt = lap(T) + K dp/dt
/// J        = (eps^2 p_x - eps eps' p_y,  eps^2 p_y + eps eps' p_x)
/// m(T)     = (alpha/pi) atan(gamma (T_e - T))
/// eps(th)  = eps_bar (1 + delta cos(j(th - th0))),  th = angle of grad p
/// ```
///
/// Verified against R. Kobayashi, "Modeling and numerical simulations
/// of dendritic crystal growth", *Physica D* 63 (1993) 410-423,
/// section 2. The anisotropic operator is written above as one
/// divergence, which is algebraically the paper's
/// `-d/dx(eps eps' p_y) + d/dy(eps eps' p_x) + div(eps^2 grad p)`;
/// collecting it that way is what lets the two scratch channels be
/// exactly two.
///
/// The paper's constants, used here and NOT exposed: eps_bar = 0.01,
/// tau = 0.0003, alpha = 0.9, gamma = 10, T_e = 1, noise amplitude
/// 0.01, and dx = 0.03 (its 9.0-wide domain on a 300 mesh). The grid
/// therefore sets the VESSEL's size, not the crystal's: a 1080p
/// viewport grid is a wider melt around a dendrite of the same size
/// in cells. The visual configs use the paper's 300 x 300; a preset
/// carries no grid, so the panel's grid setting is what to change to
/// match. What the paper varies, and what is exposed, is K, delta, j
/// and theta0.
///
/// **The discretisation is staggered, and that is not a detail.** The
/// obvious reading of "one pass takes a gradient, the next takes its
/// divergence" uses central differences twice, which composes to
/// `(f[i+2] - 2f[i] + f[i-2])/4dx^2` -- a stencil that skips the
/// immediate neighbour, so the odd and even sublattices decouple and
/// nothing damps the Nyquist mode. Measured on the CPU mirror, that
/// version filled the field with a diagonal checkerboard while
/// staying inside [0, 1] and finite, so an `isfinite` ladder called it
/// stable. Here the flux lives on cell FACES -- cell (i,j) holds the
/// flux through its +x and +y faces, from a forward difference across
/// that face -- and pass 2's backward difference composes to the
/// compact Laplacian.
///
/// Channels: `.x` = phase p (0 liquid, 1 solid), `.y` = temperature T,
/// `.z` = the +x face flux, `.w` = the +y face flux. There is no age
/// channel; the crystal's history is in T.
pub static KOBAYASHI: ModelDef = ModelDef {
    name: "kobayashi",
    display_name: "Kobayashi Dendrite",
    description: "Phase-field solidification: a crystal grown into an undercooled melt, \
                  with the anisotropy that turns a blob into a snowflake.",
    features: &[ModelFeature::NeedsRng],
    parameters: &[
        SimParamDef {
            name: "latent_heat",
            display_name: "Latent heat (K)",
            default: 1.6,
            min: 0.5,
            max: 2.5,
            tooltip: "How much heat solidification releases, which is what stops the \
                      crystal. Below 1 the whole vessel freezes; above it roughly 1/K of \
                      the region solidifies and the arms stay slender.",
            choices: &[],
        },
        SimParamDef {
            name: "delta",
            display_name: "Anisotropy (δ)",
            default: 0.04,
            min: 0.0,
            max: 0.08,
            tooltip: "Strength of the directional preference. At 0 the growth is isotropic \
                      and splits like viscous fingering; the paper's ice dendrites use \
                      0.040 and its four-fold ones 0.020.",
            choices: &[],
        },
        SimParamDef {
            name: "mode",
            display_name: "Symmetry (j)",
            default: 2.0,
            min: 0.0,
            max: 3.0,
            tooltip: "How many directions the crystal prefers. Six-fold is the snowflake; \
                      four-fold is the metallic dendrite of the paper's figure 7.",
            choices: &["2-fold", "4-fold", "6-fold", "8-fold"],
        },
        SimParamDef {
            name: "theta0",
            display_name: "Orientation (θ₀)",
            default: 1.5708,
            min: 0.0,
            max: 6.2832,
            tooltip: "Rotates the preferred directions. The paper's ice dendrites use π/2 \
                      and its four-fold ones 0.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "ice",
            display_name: "Ice dendrite (six-fold)",
            // The paper's figure 8: delta = 0.040, j = 6, theta0 = pi/2.
            params: &[
                ("latent_heat", 1.6),
                ("delta", 0.04),
                ("mode", 2.0),
                ("theta0", 1.5708),
            ],
            // Measured: reaches the edge of a 300 grid by ~6,000 steps
            // at dt = 1e-4, so 4,000 leaves the arms clear of the wall.
            steps: 4000,
            init: Some(crate::config::sim::SimInit::Blob { radius: 4 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "metallic",
            display_name: "Metallic dendrite (four-fold)",
            // The paper's figure 7: j = 4, theta0 = 0, K = 2.0.
            params: &[
                ("latent_heat", 2.0),
                ("delta", 0.02),
                ("mode", 1.0),
                ("theta0", 0.0),
            ],
            steps: 4000,
            init: Some(crate::config::sim::SimInit::Blob { radius: 4 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
const KOB_DX: f32 = 0.03;      // the paper's mesh: 9.0 across 300 cells
const KOB_EPS_BAR: f32 = 0.01;
const KOB_TAU: f32 = 0.0003;
const KOB_ALPHA: f32 = 0.9;
const KOB_GAMMA: f32 = 10.0;
const KOB_TE: f32 = 1.0;
const KOB_NOISE: f32 = 0.01;
const KOB_PI: f32 = 3.14159265359;

fn kob_mode() -> f32 {
    // 2, 4, 6 or 8 from the enum index.
    return 2.0 + 2.0 * round(clamp(mparam(2u), 0.0, 3.0));
}

// eps and eps' for a gradient direction. THE GUARD: in the bulk grad p
// is exactly zero and the angle is undefined. Metal's fast math makes
// atan2(0,0) a plausible pi/4 and other targets make it NaN, and
// either would poison the flux -- NaN * 0 is NaN, so even multiplying
// by a zero gradient does not save it. Where there is no gradient
// there is no flux, so say that directly rather than relying on the
// arithmetic.
fn kob_eps(gx: f32, gy: f32) -> vec2<f32> {
    if (gx * gx + gy * gy < 1.0e-16) {
        return vec2<f32>(KOB_EPS_BAR, 0.0);
    }
    let j = kob_mode();
    let a = j * (ff_atan2(gy, gx) - mparam(3u));
    let d = mparam(1u);
    return vec2<f32>(
        KOB_EPS_BAR * (1.0 + d * cos(a)),
        -KOB_EPS_BAR * d * j * sin(a),
    );
}

// Pass 1: the anisotropic flux, on the cell's +x and +y FACES.
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let inv = 1.0 / KOB_DX;
    let c = s.x;
    let px1 = sim_read(p + vec2<i32>(1, 0)).x;
    let py1 = sim_read(p + vec2<i32>(0, 1)).x;
    let pxm = sim_read(p + vec2<i32>(-1, 0)).x;
    let pym = sim_read(p + vec2<i32>(0, -1)).x;

    // Forward differences ACROSS each face -- this is what makes the
    // two passes compose to the compact Laplacian.
    let dpdx_xf = (px1 - c) * inv;
    let dpdy_yf = (py1 - c) * inv;

    // Transverse derivatives at the faces: the average of the central
    // differences at the two cells the face separates.
    let dy_c = (py1 - pym) * 0.5 * inv;
    let dy_c1 = (sim_read(p + vec2<i32>(1, 1)).x - sim_read(p + vec2<i32>(1, -1)).x)
        * 0.5 * inv;
    let dpdy_xf = 0.5 * (dy_c + dy_c1);
    let dx_c = (px1 - pxm) * 0.5 * inv;
    let dx_c1 = (sim_read(p + vec2<i32>(1, 1)).x - sim_read(p + vec2<i32>(-1, 1)).x)
        * 0.5 * inv;
    let dpdx_yf = 0.5 * (dx_c + dx_c1);

    let ex = kob_eps(dpdx_xf, dpdy_xf);
    let ey = kob_eps(dpdx_yf, dpdy_yf);
    let jx = ex.x * ex.x * dpdx_xf - ex.x * ex.y * dpdy_xf;
    let jy = ey.x * ey.x * dpdy_yf + ey.x * ey.y * dpdx_yf;
    return vec4<f32>(s.x, s.y, jx, jy);
}

// Pass 2: the divergence of those fluxes, then p and T.
fn sim_step2(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let inv = 1.0 / KOB_DX;
    let dt = sim_dt();
    // Backward difference of the face fluxes.
    let jxm = sim_read(p + vec2<i32>(-1, 0)).z;
    let jym = sim_read(p + vec2<i32>(0, -1)).w;
    let div = ((s.z - jxm) + (s.w - jym)) * inv;

    let m = (KOB_ALPHA / KOB_PI) * atan(KOB_GAMMA * (KOB_TE - s.y));
    // The paper adds noise to the dynamical term as a p(1-p) chi with
    // chi uniform on [-1/2, 1/2]; it vanishes in both bulks, so it
    // perturbs only the interface. Section 1 calls its influence on
    // side branching crucial.
    let noise = KOB_NOISE * s.x * (1.0 - s.x) * (sim_rand(p, 0x4bu) - 0.5);
    let dpdt = (div + s.x * (1.0 - s.x) * (s.x - 0.5 + m) + noise) / KOB_TAU;
    let np = s.x + dt * dpdt;

    let lap_t = (sim_read(p + vec2<i32>(1, 0)).y + sim_read(p + vec2<i32>(-1, 0)).y
        + sim_read(p + vec2<i32>(0, 1)).y + sim_read(p + vec2<i32>(0, -1)).y
        - 4.0 * s.y) * inv * inv;
    let nt = s.y + dt * (lap_t + mparam(0u) * dpdt);
    return vec4<f32>(np, nt, s.z, s.w);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // A nucleus in a uniformly undercooled melt: the paper takes the
    // initial temperature as zero everywhere and the equilibrium
    // temperature as 1, so the whole vessel is supercooled by one unit
    // and the seed is the only solid.
    //
    // The nucleus has to clear the CRITICAL RADIUS or surface tension
    // dissolves it: measured, `Center` (a single cell) melts and the
    // render is empty, which is why both presets seed a `Blob` of
    // radius 4.
    return vec4<f32>(select(0.0, 1.0, inside >= 0.5), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 4000,
    passes: 2,
    repeat: None,
    // The temperature equation is plain diffusion with D = 1 on a mesh
    // of 0.03, so explicit Euler needs dt <= dx^2/4 = 2.25e-4. That is
    // the binding constraint: the phase equation's eps^2/tau = 0.333
    // is three times weaker. The paper used dt = 2e-4 with an IMPLICIT
    // scheme for T for exactly this reason; measured here fully
    // explicit, 1e-4 is clean (Nyquist amplitude 8e-5), 2e-4 carries a
    // trace (2.8e-3) and 3e-4 diverges outright at step 1,389.
    agents: None,
    kernel: None,
    dt_bound: Some(|_| KOB_DX_SQ_OVER_4),
    diffusion: &[],
    max_dt: 0.001,
    default_dt: 0.0001,
};

/// `dx^2 / 4` for Kobayashi's fixed mesh -- the explicit diffusion
/// limit for its temperature field. A `const` because `dt_bound` is a
/// plain `fn` pointer and cannot capture.
const KOB_DX_SQ_OVER_4: f32 = 0.03 * 0.03 / 4.0;


// ---------------------------------------------------------------------
// Large-kernel models.
//
// These are not stencils. Their rule is a convolution against a table
// of weights that varies continuously with radius -- Lenia's ring,
// SmoothLife's pair of anti-aliased discs -- so they declare a
// `kernel` builder, and the step shader gathers against the table the
// renderer uploads. Cost is (2R+1)^2 taps a cell: 729 at R = 13 and
// 4,225 at R = 32.
//
// The catalogue's formulas for both were read from secondary
// statements rather than from the papers -- unlike Kobayashi and
// Tyson-Fife, neither paper was available -- so what the prototype
// checked is that the formulas AS RECORDED produce the behaviour
// claimed for them. Both do.
// ---------------------------------------------------------------------

/// Lenia: Conway's Life taken continuous in space, state and time.
///
/// ```text
/// A' = clip( A + dt * G(K * A), 0, 1 )
/// K_C(r) = exp(alpha - alpha / (4 r (1 - r))),  alpha = 4
/// K      = K_C(|x| / R) / sum K_C
/// G(u)   = 2 exp(-(u - mu)^2 / (2 sigma^2)) - 1
/// ```
///
/// The kernel is a normalised RING, so the gather is a weighted
/// neighbourhood average; the growth mapping then rewards a cell whose
/// average sits near `mu` and punishes everything else. That is the
/// whole rule.
///
/// Measured (256^2, 600 steps): R = 13, mu = 0.15, sigma = 0.015 --
/// the constants the catalogue records for Chan's Orbium -- give a
/// living filamentary field, still moving at 600 steps, 3.6% of cells
/// on a soft edge. Widening sigma saturates it: 0.03 gives 1.5% edge
/// and 0.05 or 0.07 less still, all of them frozen. So sigma is the
/// parameter that decides whether there is anything to look at.
///
/// **Orbium itself is not shipped.** The creature needs its specific
/// 20x20 initial array, which is a `Pattern` init this engine does not
/// have; what ships is the soup those constants make from noise.
/// Multi-ring kernels and Chan's polynomial and rectangular cores are
/// likewise not implemented -- the catalogue marks their formulas
/// `[verify]`, and nothing here has verified them.
///
/// Channels: `.x` = A, `.y` = the last potential K*A, `.z` = age.
pub static LENIA: ModelDef = ModelDef {
    name: "lenia",
    display_name: "Lenia",
    description: "Life made continuous: a ring-shaped kernel and a smooth growth rule, \
                  giving soft filaments and cells that crawl.",
    features: &[ModelFeature::NeverStills],
    parameters: &[
        SimParamDef {
            name: "radius",
            display_name: "Kernel radius (R)",
            default: 13.0,
            min: 4.0,
            max: 32.0,
            tooltip: "Half-width of the ring, in cells — it sets the size of everything the \
                      rule builds. Cost grows as R²: 729 taps a cell at 13, 4,225 at 32.",
            choices: &[],
        },
        SimParamDef {
            name: "mu",
            display_name: "μ (growth centre)",
            default: 0.15,
            min: 0.0,
            max: 0.5,
            tooltip: "The neighbourhood average a cell is rewarded for having. Higher values \
                      need a denser neighbourhood to survive.",
            choices: &[],
        },
        SimParamDef {
            name: "sigma",
            display_name: "σ (growth width)",
            default: 0.015,
            min: 0.001,
            max: 0.1,
            tooltip: "How forgiving that reward is, and the parameter that decides whether \
                      there is anything to look at — measured, 0.015 gives a living field \
                      and 0.03 upward freezes into saturated blobs.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "soup",
        display_name: "Soup",
        params: &[("radius", 13.0), ("mu", 0.15), ("sigma", 0.015)],
        // Measured: filaments by 200, still moving at 600.
        steps: 600,
        init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
        coloring: Some("channel"),
        coloring_params: &[("channel", 0.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
        matte: None,
    }],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let r = sim_kernel_radius();
    let w = 2 * r + 1;
    var u = 0.0;
    for (var dy = -r; dy <= r; dy = dy + 1) {
        for (var dx = -r; dx <= r; dx = dx + 1) {
            let k = klut(u32((dy + r) * w + (dx + r)));
            // The square's corners lie outside the ring and weigh
            // nothing, so skipping them skips the TEXTURE READ, which
            // is the cost. The test is uniform across the workgroup --
            // every thread is at the same (dx, dy) on the same
            // iteration -- so it costs no divergence.
            if (k > 0.0) {
                u = u + k * sim_read(p + vec2<i32>(dx, dy)).x;
            }
        }
    }

    let mu = mparam(1u);
    let sg = max(mparam(2u), 1.0e-5);
    let d = u - mu;
    let g = 2.0 * exp(-(d * d) / (2.0 * sg * sg)) - 1.0;
    let na = clamp(s.x + sim_dt() * g, 0.0, 1.0);
    let age = select(s.z, f32(sim_step_index()), abs(na - s.x) > 1.0e-4);
    // The potential rides in .y so `two_channel` can show what the
    // rule was actually looking at.
    return vec4<f32>(na, u, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Patches the size of the kernel, not per-cell noise: a radius-13
    // ring averages a per-cell random field flat before anything can
    // grow, so the seed has to carry structure at the scale the rule
    // works on. Measured on the prototype, which seeds the same way.
    let g = sim_grid();
    let r = max(i32(params.kernel_radius), 1);
    let cell = vec2<i32>(p.x / r, p.y / r);
    return vec4<f32>(sim_rand(cell, 0x1eu), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 600,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: Some(|p| {
        // The exponential core, scaled to R and normalised to sum 1.
        let r = p.get("radius").round().clamp(2.0, MAX_KERNEL_RADIUS as f32) as u32;
        let w = 2 * r as usize + 1;
        let mut weights = vec![0.0f32; w * w];
        let mut sum = 0.0f64;
        for iy in 0..w {
            for ix in 0..w {
                let dx = ix as f32 - r as f32;
                let dy = iy as f32 - r as f32;
                let d = (dx * dx + dy * dy).sqrt() / r as f32;
                if d < 1.0 {
                    // Guarded away from 0 and 1, where the core's
                    // exponent is -inf; the weight there is zero
                    // anyway, and this reaches it by a route that
                    // cannot produce a NaN.
                    let t = d.clamp(1.0e-6, 1.0 - 1.0e-6);
                    let v = (4.0 - 4.0 / (4.0 * t * (1.0 - t))).exp();
                    weights[iy * w + ix] = v;
                    sum += v as f64;
                }
            }
        }
        if sum > 0.0 {
            for x in weights.iter_mut() {
                *x = (*x as f64 / sum) as f32;
            }
        }
        crate::sim::SimKernel { radius: r, weights }
    }),
    dt_bound: None,
    diffusion: &[],
    // Not a stability bound: the growth term is bounded in [-1, 1] and
    // the state is clipped, so no dt diverges. Past about 0.5 a step
    // simply jumps over the dynamics.
    max_dt: 0.5,
    default_dt: 0.1,
};

/// SmoothLife: Conway's Life on a continuous domain.
///
/// ```text
/// m = disc average over |x| < r_i          ("cell filling")
/// n = annulus average over r_i < |x| < r_a  ("neighbourhood")
/// sig(x, a, al) = 1 / (1 + exp(-(x - a) 4/al))
/// s(n, m) = sig(n, lo, al_n) * (1 - sig(n, hi, al_n))
///   with lo = b1 + (d1 - b1) sig(m, 1/2, al_m)
///        hi = b2 + (d2 - b2) sig(m, 1/2, al_m)
/// f' = f + dt (s(n, m) - f)
/// ```
///
/// Life's rule read as thresholds: a cell's own filling `m` chooses
/// which birth/survival window applies, and the neighbourhood `n` is
/// tested against it. Rafler gives two time forms; this is the one
/// that stays in [0, 1].
///
/// Both averages come from ONE gather with two accumulators, which is
/// why the kernel table carries two blocks -- the disc first, then the
/// annulus -- rather than two tables.
///
/// Measured (256^2, 400 steps): Rafler's glider set gives the
/// characteristic smooth labyrinth, ~10% of cells on a soft edge, at
/// dt 0.1 and 0.3 alike and at both r_i = 7 and r_i = 4. The discrete
/// time form is not shipped: it is the one that does not stay in
/// [0, 1].
///
/// Channels: `.x` = f, `.y` = the last neighbourhood n, `.z` = age.
pub static SMOOTHLIFE: ModelDef = ModelDef {
    name: "smoothlife",
    display_name: "SmoothLife",
    description: "Conway's Life on a continuous domain: the same birth and survival rules \
                  read as smooth thresholds, giving gliders and rolling labyrinths.",
    features: &[ModelFeature::NeverStills],
    parameters: &[
        SimParamDef {
            name: "inner_radius",
            display_name: "Inner radius (rᵢ)",
            default: 7.0,
            min: 2.0,
            max: 10.0,
            tooltip: "Radius of the disc a cell measures itself over. The outer radius is \
                      three times it, as in the paper, so this sets the whole scale — and \
                      the cost, which grows as rᵢ².",
            choices: &[],
        },
        SimParamDef {
            name: "b1",
            display_name: "Birth low (b₁)",
            default: 0.278,
            min: 0.0,
            max: 1.0,
            tooltip: "Lower edge of the window in which an empty cell is born.",
            choices: &[],
        },
        SimParamDef {
            name: "b2",
            display_name: "Birth high (b₂)",
            default: 0.365,
            min: 0.0,
            max: 1.0,
            tooltip: "Upper edge of that window.",
            choices: &[],
        },
        SimParamDef {
            name: "d1",
            display_name: "Survive low (d₁)",
            default: 0.267,
            min: 0.0,
            max: 1.0,
            tooltip: "Lower edge of the window in which a filled cell survives.",
            choices: &[],
        },
        SimParamDef {
            name: "d2",
            display_name: "Survive high (d₂)",
            default: 0.445,
            min: 0.0,
            max: 1.0,
            tooltip: "Upper edge of that window. The gap between it and b₂ is what lets \
                      structures persist without filling the plane.",
            choices: &[],
        },
        SimParamDef {
            name: "alpha_n",
            display_name: "Neighbourhood softness (αₙ)",
            default: 0.028,
            min: 0.001,
            max: 0.3,
            tooltip: "How sharply the neighbourhood threshold switches. Small values are \
                      nearly a hard rule.",
            choices: &[],
        },
        SimParamDef {
            name: "alpha_m",
            display_name: "Filling softness (αₘ)",
            default: 0.147,
            min: 0.001,
            max: 0.3,
            tooltip: "How sharply a cell's own filling picks between the birth and survival \
                      windows.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "glider",
        display_name: "Rafler's glider set",
        params: &[
            ("inner_radius", 7.0),
            ("b1", 0.278),
            ("b2", 0.365),
            ("d1", 0.267),
            ("d2", 0.445),
            ("alpha_n", 0.028),
            ("alpha_m", 0.147),
        ],
        // Measured: the soup organises by ~100 steps and is a settled
        // labyrinth by 400.
        steps: 400,
        init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
        coloring: Some("channel"),
        coloring_params: &[("channel", 0.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
        matte: None,
    }],
    wgsl: r#"
fn sl_sigma(x: f32, a: f32, al: f32) -> f32 {
    return 1.0 / (1.0 + exp(-(x - a) * 4.0 / max(al, 1.0e-4)));
}

fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let r = sim_kernel_radius();
    let w = 2 * r + 1;
    let taps = sim_kernel_taps();

    // ONE gather, two accumulators: the disc and the annulus differ
    // only in their weights, so reading the field twice would double
    // the only expensive part.
    var m = 0.0;
    var n = 0.0;
    for (var dy = -r; dy <= r; dy = dy + 1) {
        for (var dx = -r; dx <= r; dx = dx + 1) {
            let i = u32((dy + r) * w + (dx + r));
            let ki = klut(i);
            let ko = klut(taps + i);
            if (ki > 0.0 || ko > 0.0) {
                let v = sim_read(p + vec2<i32>(dx, dy)).x;
                m = m + ki * v;
                n = n + ko * v;
            }
        }
    }

    let al_m = mparam(6u);
    let al_n = mparam(5u);
    // A cell's own filling chooses which window applies.
    let pick = sl_sigma(m, 0.5, al_m);
    let lo = mparam(1u) + (mparam(3u) - mparam(1u)) * pick;
    let hi = mparam(2u) + (mparam(4u) - mparam(2u)) * pick;
    let alive = sl_sigma(n, lo, al_n) * (1.0 - sl_sigma(n, hi, al_n));

    let nf = clamp(s.x + sim_dt() * (alive - s.x), 0.0, 1.0);
    let age = select(s.z, f32(sim_step_index()), abs(nf - s.x) > 1.0e-4);
    return vec4<f32>(nf, n, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Patches the size of the inner disc, for the same reason Lenia
    // seeds in patches: a radius-21 gather averages per-cell noise
    // flat before the rule can act on it.
    let blk = max(i32(round(mparam(0u))), 1);
    let cell = vec2<i32>(p.x / blk, p.y / blk);
    return vec4<f32>(select(0.0, 1.0, sim_rand(cell, 0x5fu) < 0.5), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 400,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: Some(|p| {
        // Two blocks: the inner disc, then the annulus out to 3 r_i.
        // Both anti-aliased over a one-cell band, which is what keeps
        // a smooth rule from inheriting the lattice's square symmetry.
        let ri = p.get("inner_radius").clamp(1.0, 10.0);
        let ra = ri * 3.0;
        let r = ((ra.ceil() as u32) + 1).clamp(1, MAX_KERNEL_RADIUS);
        let w = 2 * r as usize + 1;
        let taps = w * w;
        let mut inner = vec![0.0f32; taps];
        let mut outer = vec![0.0f32; taps];
        let (mut si, mut so) = (0.0f64, 0.0f64);
        for iy in 0..w {
            for ix in 0..w {
                let dx = ix as f32 - r as f32;
                let dy = iy as f32 - r as f32;
                let d = (dx * dx + dy * dy).sqrt();
                let i = (ri + 0.5 - d).clamp(0.0, 1.0);
                let o = (ra + 0.5 - d).clamp(0.0, 1.0) * (1.0 - i);
                inner[iy * w + ix] = i;
                outer[iy * w + ix] = o;
                si += i as f64;
                so += o as f64;
            }
        }
        for x in inner.iter_mut() {
            *x = (*x as f64 / si.max(1.0e-12)) as f32;
        }
        for x in outer.iter_mut() {
            *x = (*x as f64 / so.max(1.0e-12)) as f32;
        }
        inner.extend_from_slice(&outer);
        crate::sim::SimKernel { radius: r, weights: inner }
    }),
    dt_bound: None,
    diffusion: &[],
    // As Lenia: `s` is bounded in [0, 1] and the update is a
    // relaxation toward it, so no dt diverges. At 1.0 the rule becomes
    // Rafler's discrete form.
    max_dt: 1.0,
    default_dt: 0.15,
};


// ---------------------------------------------------------------------
// Multi-scale.
// ---------------------------------------------------------------------

/// McCabe's cyclic symmetric multi-scale Turing patterns.
///
/// One field f in [-1, 1]. For each scale i, with activator radius
/// r_a,i < inhibitor radius r_b,i and step amount s_i:
///
/// ```text
/// a_i = average of f over radius r_a,i
/// b_i = average of f over radius r_b,i
/// v_i = |a_i - b_i|                    ("variation")
/// ```
///
/// The scale with the SMALLEST variation fires: f moves by +s_i if
/// a_i > b_i and by -s_i otherwise. Then f is renormalised to [-1, 1]
/// over the whole field. Cyclic symmetry, the paper's contribution,
/// replaces each a_i and b_i by its average with copies rotated about
/// the image centre by 2πk/n.
///
/// J. McCabe, "Cyclic Symmetric Multi-Scale Turing Patterns", Bridges
/// 2010. The paper gives NO radius table and does not describe
/// colouring; the ladder here (radii doubling from `base_radius`, the
/// inhibitor `ratio` times the activator, amounts falling linearly)
/// is the phase-0 prototype's, and `scale_mix` is the look later
/// implementations gave it.
///
/// **How the averages are taken, and why it is not a box.** Each
/// average is a trilinear read of a GAUSSIAN pyramid the renderer
/// builds every step -- eight loads per radius, whatever the radius.
/// The plan's pyramid was a box downsample, and that is refuted by
/// measurement: a box converges to a SQUARE kernel however many
/// levels it has, and the texture came out visibly axis-aligned with
/// a spectrum half as peaked as the exact-disc reference's. The
/// Gaussian pyramid is isotropic, and with the level mapping
/// calibrated (`log2(0.55 r)`) it reproduces the reference's feature
/// size to 0.1% and amplitude to 1%. See
/// `scripts/sim_prototypes/proto_mccabe_pyramid.py`.
///
/// **Renormalisation is one step behind, and that is the reference's
/// own dependency.** A step can only normalise by a range that has
/// been measured; the reduce pass measures each step's output and the
/// next step normalises its input by it, which is exactly the order
/// the prototype does it in. The field therefore holds values in
/// roughly [-1 - s, 1 + s] rather than exactly [-1, 1].
///
/// Channels: `.x` = f, `.y` = the scale that fired (an integer),
/// `.z` = the step at which the firing scale last changed, `.w` spare.
pub static MCCABE: ModelDef = ModelDef {
    name: "mccabe",
    display_name: "McCabe Multi-Scale",
    description: "Turing patterns at several scales at once, the finest structure nested \
                  inside the coarsest — the electron-microscope look — with optional \
                  rotational symmetry.",
    features: &[
        ModelFeature::NeverStills,
        ModelFeature::NoTimeStep,
        ModelFeature::NeedsPyramid,
        ModelFeature::NeedsMinMax,
    ],
    parameters: &[
        SimParamDef {
            name: "scales",
            display_name: "Scales",
            default: 5.0,
            min: 1.0,
            max: 6.0,
            tooltip: "How many scales compete. Each is twice the radius of the last, so five \
                      scales span a factor of sixteen.",
            choices: &[],
        },
        SimParamDef {
            name: "base_radius",
            display_name: "Finest radius",
            default: 1.0,
            min: 0.5,
            max: 8.0,
            tooltip: "Activator radius of the finest scale, in cells. Everything else is \
                      built from it.",
            choices: &[],
        },
        SimParamDef {
            name: "ratio",
            display_name: "Inhibitor ratio",
            default: 2.0,
            min: 1.25,
            max: 4.0,
            tooltip: "Inhibitor radius over activator radius, at every scale. The paper's \
                      figures use 2.",
            choices: &[],
        },
        SimParamDef {
            name: "amount",
            display_name: "Step (finest)",
            default: 0.05,
            min: 0.001,
            max: 0.2,
            tooltip: "How far the finest scale moves a cell per step. Larger is faster and \
                      grainier.",
            choices: &[],
        },
        SimParamDef {
            name: "amount_min",
            display_name: "Step (coarsest)",
            default: 0.01,
            min: 0.001,
            max: 0.2,
            tooltip: "The coarsest scale's step; scales in between interpolate. Coarse \
                      scales moving slowly is what lets fine detail live inside them.",
            choices: &[],
        },
        SimParamDef {
            name: "symmetry",
            display_name: "Symmetry",
            default: 0.0,
            min: 0.0,
            max: 8.0,
            tooltip: "n-fold rotational symmetry about the centre — the paper's own \
                      contribution. 0 or 1 is none. Costs n times the reads, and a \
                      periodic field that is rotated about its centre no longer tiles, \
                      so the corners stay asymmetric.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "multiscale",
            display_name: "Multi-scale",
            params: &[
                ("scales", 5.0),
                ("base_radius", 1.0),
                ("ratio", 2.0),
                ("amount", 0.05),
                ("amount_min", 0.01),
                ("symmetry", 0.0),
            ],
            // Measured on the prototype: the nested texture is present
            // by step 20 and fully developed by 100; the field never
            // stills.
            steps: 200,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("scale_mix"),
            coloring_params: &[("scales", 5.0), ("value_scale", 0.5)],
            matte: None,
        },
        SimPreset {
            name: "coarse",
            display_name: "Coarse (nested contours)",
            // Measured on the GPU: at base radius 3 the nested contour
            // texture is unmistakable, where the paper-faithful base
            // radius 1 is fine and subtle at 256^2. Same ladder, three
            // times the scale.
            params: &[
                ("scales", 5.0),
                ("base_radius", 3.0),
                ("ratio", 2.0),
                ("amount", 0.05),
                ("amount_min", 0.01),
                ("symmetry", 0.0),
            ],
            steps: 200,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.5), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "rosette",
            display_name: "Five-fold rosette",
            params: &[
                ("scales", 5.0),
                ("base_radius", 3.0),
                ("ratio", 2.0),
                ("amount", 0.05),
                ("amount_min", 0.01),
                ("symmetry", 5.0),
            ],
            steps: 200,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.5), ("offset", 0.5), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
const MC_TAU: f32 = 6.28318530718;

// An average of f over radius r at a base-cell position, with the
// cyclic symmetry folded in: the mean over the n rotations of the
// position about the grid centre.
fn mc_avg(r: f32, pos: vec2<f32>, sym: i32) -> f32 {
    let level = pyr_level_for_radius(r);
    if (sym < 2) {
        return pyr_sample(level, pos);
    }
    let g = vec2<f32>(sim_grid());
    let c = g * 0.5;
    var acc = 0.0;
    for (var k = 0; k < sym; k = k + 1) {
        let a = MC_TAU * f32(k) / f32(sym);
        let d = pos - c;
        var q = vec2<f32>(cos(a) * d.x - sin(a) * d.y, sin(a) * d.x + cos(a) * d.y) + c;
        // Float periodic wrap, so a rotated corner reads the tile.
        q = q - g * floor(q / g);
        acc = acc + pyr_sample(level, q);
    }
    return acc / f32(sym);
}

fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let n = i32(clamp(round(mparam(0u)), 1.0, 6.0));
    let base = mparam(1u);
    let ratio = mparam(2u);
    let amount = mparam(3u);
    let amount_min = mparam(4u);
    let sym = i32(round(mparam(5u)));
    let pos = vec2<f32>(p) + vec2<f32>(0.5, 0.5);
    pyr_prepare();

    var best_var = 1.0e30;
    var best_dir = 0.0;
    var best_scale = 0.0;
    for (var i = 0; i < n; i = i + 1) {
        let ra = base * f32(1 << u32(i));
        let rb = ra * ratio;
        let act = mc_avg(ra, pos, sym);
        let inh = mc_avg(rb, pos, sym);
        let v = abs(act - inh);
        // The amounts fall linearly from the finest scale to the
        // coarsest; one scale gets the finest amount.
        let t = select(0.0, f32(i) / f32(n - 1), n > 1);
        let amt = mix(amount, amount_min, t);
        if (v < best_var) {
            best_var = v;
            best_dir = select(-amt, amt, act > inh);
            best_scale = f32(i);
        }
    }

    // Normalise by the previous step's measured range, then step.
    let mm = sim_minmax();
    let span = max(mm.y - mm.x, 1.0e-6);
    let f = (s.x - mm.x) / span * 2.0 - 1.0;
    let age = select(s.z, f32(sim_step_index()), best_scale != s.y);
    return vec4<f32>(f + best_dir, best_scale, age, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Uniform noise in [-1, 1]. The init shape is ignored: the rule
    // needs a disordered start everywhere, and a shape inside a flat
    // field is a single scale's worth of structure that the others
    // average away.
    return vec4<f32>(noise * 2.0 - 1.0, 0.0, 0.0, 0.0);
}
"#,
    default_steps: 200,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


// ---------------------------------------------------------------------
// Agent models.
//
// The state is a POPULATION, not a field: agents persist across steps,
// move themselves, and what they leave behind is an integer deposit
// the step pass folds into the field and clears. Integer, because
// thousands of agents land in one cell in an order the hardware
// chooses and `atomicAdd` on a u32 does not care about that order --
// an f32 accumulation would give a different picture every run.
// ---------------------------------------------------------------------

/// Jones' Physarum transport networks.
///
/// Each agent senses the trail map at three forward sensors -- ahead
/// and at +-SA -- turns by RA toward the strongest, steps SS forward,
/// and deposits depT. The trail is then averaged over a 3x3 kernel and
/// multiplied by (1 - decay). What emerges is the polygonal transport
/// network the slime mould builds.
///
/// J. Jones, "Characteristics of pattern formation and evolution in
/// approximations of Physarum transport networks", *Artificial Life*
/// 16(2) (2010) 127-153. **Every parameter here is the paper's Table
/// 1**, which was read: SA 22.5 or 45 deg, RA 45 deg, SO 9 pixels,
/// SS 1 pixel/step, depT 5, decayT 0.1, 3x3 diffusion, periodic, and
/// a population of 3-15% of the image area. The catalogue had recorded
/// those from a secondary source and every one of them is confirmed.
///
/// **The exclusion is not optional, and that is measured.** Jones'
/// section 2.1: a cell holds one agent, and an agent whose target is
/// occupied stays put, deposits nothing, and takes a random new
/// heading. The catalogue's GPU sketch left it out. Run without it on
/// the CPU prototype, the population collapses into a handful of thick
/// arcs; with it, the same parameters give the network. So the model
/// declares two agent passes: the first turns and CLAIMS a target
/// cell, the second moves only if it won. The claim is an atomic
/// minimum over agent indices, so the winner is the lowest index
/// rather than whoever ran first, and the run reproduces exactly.
///
/// Channels: `.x` = trail, `.w` = this step's deposit (where the
/// agents actually are, for the `occupancy` colouring).
pub static PHYSARUM: ModelDef = ModelDef {
    name: "physarum",
    display_name: "Physarum",
    description: "Slime mould: thousands of agents lay a chemical trail and follow it, and \
                  the feedback builds a transport network of filaments and junctions.",
    features: &[
        ModelFeature::NeedsRng,
        ModelFeature::NeedsAgents,
        ModelFeature::NeverStills,
        ModelFeature::NoTimeStep,
    ],
    parameters: &[
        SimParamDef {
            name: "population",
            display_name: "Population (% of grid)",
            default: 5.0,
            min: 0.5,
            max: 25.0,
            tooltip: "Agents as a percentage of the grid's cells — the paper's %p, whose \
                      useful range is 3 to 15. Too few and the trails never meet; too many \
                      and the network fills in.",
            choices: &[],
        },
        SimParamDef {
            name: "sensor_angle",
            display_name: "Sensor angle (SA)",
            default: 22.5,
            min: 5.0,
            max: 90.0,
            tooltip: "How far to each side an agent looks, in degrees. One of the three \
                      parameters the paper says actually change the pattern; it uses 22.5 \
                      or 45.",
            choices: &[],
        },
        SimParamDef {
            name: "rotation_angle",
            display_name: "Rotation angle (RA)",
            default: 45.0,
            min: 5.0,
            max: 90.0,
            tooltip: "How far an agent turns when it turns, in degrees. The paper uses 45.",
            choices: &[],
        },
        SimParamDef {
            name: "sensor_offset",
            display_name: "Sensor distance (SO)",
            default: 9.0,
            min: 1.0,
            max: 32.0,
            tooltip: "How far ahead the sensors sit, in cells. The paper uses 9, and notes \
                      that a distance of at least 3 is what makes the population couple \
                      strongly enough for networks to form at all.",
            choices: &[],
        },
        SimParamDef {
            name: "step_size",
            display_name: "Step size (SS)",
            default: 1.0,
            min: 0.25,
            max: 4.0,
            tooltip: "How far an agent moves per step, in cells.",
            choices: &[],
        },
        SimParamDef {
            name: "deposit",
            display_name: "Deposit (depT)",
            default: 5.0,
            min: 0.5,
            max: 40.0,
            tooltip: "How much trail an agent lays each step. Scales against the decay: \
                      what matters is the ratio.",
            choices: &[],
        },
        SimParamDef {
            name: "decay",
            display_name: "Decay (decayT)",
            default: 0.1,
            min: 0.005,
            max: 0.5,
            tooltip: "How fast the trail fades. The other parameter the paper says matters: \
                      low values let old trails persist into thick networks, high values \
                      keep only what is currently travelled.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "network",
            display_name: "Transport network",
            params: &[
                ("population", 5.0),
                ("sensor_angle", 22.5),
                ("rotation_angle", 45.0),
                ("sensor_offset", 9.0),
                ("step_size", 1.0),
                ("deposit", 5.0),
                ("decay", 0.1),
            ],
            // Measured on the prototype: filaments by 100 steps, the
            // polygonal network by 600, and it keeps rearranging.
            steps: 600,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 0.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.15), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "coarse",
            display_name: "Coarse mesh",
            // SA 45 gives a wider, blockier mesh -- the paper's other
            // sensor angle.
            params: &[
                ("population", 5.0),
                ("sensor_angle", 45.0),
                ("rotation_angle", 45.0),
                ("sensor_offset", 9.0),
                ("step_size", 1.0),
                ("deposit", 5.0),
                ("decay", 0.1),
            ],
            steps: 600,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 0.0 }),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.15), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Jones' trail map update: a 3x3 MEAN, then the decay factor.
    var acc = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            acc = acc + sim_read(p + vec2<i32>(dx, dy)).x;
        }
    }
    // What the agents left here since the last step, and clear it.
    //
    // ONE STEP LATER THAN JONES, and measured to not matter. The paper
    // deposits, then takes the 3x3 mean, then decays, so a fresh
    // deposit is spread the step it is laid; this takes the mean of
    // the OLD trail and adds the raw deposit, which spreads it on the
    // next step instead. Doing it the paper's way needs a cell to read
    // its neighbours' deposits while they are being cleared, which is
    // a race without a second buffer. Run both ways on the prototype
    // from the same seed: sd 3.58 against 3.67, the same polygonal
    // network, filaments slightly grainier. Not worth the buffer.
    let dep = sim_take_deposit(p);
    let trail = (acc * (1.0 / 9.0) + dep) * (1.0 - mparam(6u));
    // The deposit rides in .w: it is where the agents ARE, which is a
    // different and grainier picture than where they have been.
    return vec4<f32>(trail, 0.0, s.z, dep);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // An empty trail map. The agents are the initial condition, and
    // they are seeded by their own pass.
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
"#,
    default_steps: 600,
    passes: 1,
    repeat: None,
    agents: Some(crate::sim::AgentDef {
        count: |p, w, h| {
            // The paper's %p: a percentage of the image AREA, so the
            // same setting is the same density at any grid size.
            let pct = p.get("population").clamp(0.1, 50.0);
            // Clamped here rather than only in the renderer, so the
            // declared count is the count: 25% of a 4096 grid is
            // 4.19M, past the engine's ceiling.
            (((w as f64) * (h as f64) * pct as f64 / 100.0) as u32)
                .clamp(1, crate::sim::MAX_AGENTS)
        },
        passes: 2,
        wgsl: r#"
const PHYS_TAU: f32 = 6.28318530718;

fn phys_dir(h: f32) -> vec2<f32> {
    return vec2<f32>(cos(h), sin(h));
}

// One sensor: the trail at SO ahead, offset by `off` from the heading.
// Would a move to `dest` leave the grid? Only a periodic field lets a
// position wrap; under any other boundary a wall is an unsuccessful
// move. BOTH passes ask this, and must agree: a claim made in pass 1
// is released only by the same agent's check in pass 2, so a move that
// pass 2 will refuse must not be claimed in pass 1 -- the first
// version did, and the claim stayed on the edge cell for ever, until
// the whole edge column was unenterable. The wall test caught it.
fn phys_off_grid(dest: vec2<f32>) -> bool {
    let g = vec2<f32>(sim_grid());
    return !SIM_PERIODIC
        && (dest.x < -0.5 || dest.y < -0.5 || dest.x >= g.x - 0.5 || dest.y >= g.y - 0.5);
}

fn phys_sense(a: SimAgent, off: f32) -> f32 {
    let q = a.pos + phys_dir(a.heading + off) * mparam(3u);
    return sim_read(vec2<i32>(floor(q + vec2<f32>(0.5, 0.5)))).x;
}

fn sim_agent_seed(i: u32) -> SimAgent {
    // Jones: a random unoccupied location and a random orientation
    // over the full circle, which frees the agent from the lattice.
    let g = vec2<f32>(sim_grid());
    var a: SimAgent;
    a.pos = vec2<f32>(agent_rand(i, 0x11u), agent_rand(i, 0x12u)) * g;
    a.heading = agent_rand(i, 0x13u) * PHYS_TAU;
    a.state = 0.0;
    return a;
}

// Pass 1: sense, turn, and claim the cell this agent wants.
fn sim_agent(a: SimAgent, i: u32) -> SimAgent {
    let sa = radians(mparam(1u));
    let ra = radians(mparam(2u));
    let f = phys_sense(a, 0.0);
    let l = phys_sense(a, sa);
    let r = phys_sense(a, -sa);

    // Jones' figure 3, in its order. The one that is easy to get
    // wrong is the second branch: when BOTH sides beat the front the
    // turn is random, whichever side is stronger -- not a turn toward
    // the stronger one. The first version of this shader did the
    // latter; the prototype that validated the parameters did not, and
    // the phase-4 review caught the difference.
    var turn = 0.0;
    if (f >= l && f >= r) {
        // Forward is strongest: keep going. The "forward bias" the
        // paper says keeps the dynamic continuous.
        turn = 0.0;
    } else if (f < l && f < r) {
        turn = select(-ra, ra, agent_rand(i, 0x14u) < 0.5);
    } else if (l > r) {
        turn = ra;
    } else if (r > l) {
        turn = -ra;
    }

    var out = a;
    out.heading = a.heading + turn;
    // `dest`, not `target`: that is a WGSL reserved keyword, and naga
    // rejects it -- the same trap the escape assembler hit with `root`.
    let dest = out.pos + phys_dir(out.heading) * mparam(4u);
    if (!phys_off_grid(dest)) {
        agent_claim(vec2<i32>(floor(dest + vec2<f32>(0.5, 0.5))), i);
    }
    return out;
}

// Pass 2: move if this agent won the cell; otherwise stay put, deposit
// nothing, and take a new random heading.
fn sim_agent2(a: SimAgent, i: u32) -> SimAgent {
    let g = vec2<f32>(sim_grid());
    let dest = a.pos + phys_dir(a.heading) * mparam(4u);
    let cell = vec2<i32>(floor(dest + vec2<f32>(0.5, 0.5)));
    var out = a;
    // A wall is an unsuccessful move: the agent stays and takes a new
    // random heading, exactly as for an occupied cell. (The first
    // version wrapped the position under every boundary while the
    // deposit clamped, so an agent could walk off one edge and
    // reappear on the other.) Pass 1 made no claim for this move, so
    // there is none to check.
    if (!phys_off_grid(dest) && agent_claim_check(cell, i)) {
        // The agent is semi-continuous, and only its DEPOSIT is on the
        // lattice. A periodic field wraps the float position; any other
        // keeps it, and the off-grid test above has already confined
        // it to [-0.5, g - 0.5). Wrapping unconditionally was the third
        // wall bug: a destination of x = -0.4 is inside cell 0 and
        // passes the wall test, and the wrap then put it at g - 0.4.
        out.pos = select(dest, dest - g * floor(dest / g), SIM_PERIODIC);
        agent_deposit(vec2<i32>(floor(out.pos + vec2<f32>(0.5, 0.5))), mparam(5u));
    } else {
        out.heading = agent_rand(i, 0x15u) * PHYS_TAU;
    }
    return out;
}
"#,
    }),
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


/// Diffusion-limited aggregation.
///
/// A seed particle is fixed at the centre. Walkers random-walk on the
/// lattice; a walker that finds itself next to the cluster sticks, and
/// one that wanders past the kill radius is relaunched. The result is
/// the classic branching aggregate, whose fractal dimension in two
/// dimensions is about 1.71.
///
/// T. A. Witten and L. M. Sander, *Phys. Rev. Lett.* 47 (1981) 1400.
///
/// **A frozen cell stores its DISTANCE from the centre, not a flag.**
/// That is what lets the launch radius be measured: the model declares
/// `NeedsMinMax`, and the maximum of channel `.x` is then the cluster's
/// radius plus one, for free, from a reduction the engine already had.
/// Walkers launch just outside it and die well beyond it, which is what
/// keeps the dimension right -- a walker relaunched at a uniformly
/// random cell would spawn inside the cluster's fjords and fill them,
/// driving the dimension toward 2.
///
/// **Many walkers advance at once**, which is not Witten and Sander's
/// sequential process. It is the standard parallel variant, and it
/// preserves the dimension as long as the walker density near the
/// cluster stays low; the test measures the dimension rather than
/// assuming it.
///
/// Channels: `.x` = 0 in the melt, or the frozen cell's distance from
/// the centre plus one, `.z` = the step it froze (the `age` colouring
/// draws the growth order -- the classic DLA rainbow).
pub static DLA: ModelDef = ModelDef {
    name: "dla",
    display_name: "Diffusion-Limited Aggregation",
    description: "Random walkers that stick where they first touch a growing cluster, \
                  building the branching aggregate of soot, frost and mineral dendrites.",
    features: &[
        ModelFeature::NeedsRng,
        ModelFeature::NeedsAgents,
        ModelFeature::NeedsMinMax,
        ModelFeature::NeverStills,
        ModelFeature::NoTimeStep,
    ],
    parameters: &[
        SimParamDef {
            name: "walkers",
            display_name: "Walkers (% of grid)",
            default: 4.0,
            min: 0.1,
            max: 20.0,
            tooltip: "How many walkers are in flight at once, as a percentage of the grid's \
                      cells. More is faster but crowds the cluster, which thickens the \
                      branches — the aggregate's dimension depends on walkers arriving one \
                      at a time.",
            choices: &[],
        },
        SimParamDef {
            name: "p_stick",
            display_name: "Sticking probability",
            default: 1.0,
            min: 0.02,
            max: 1.0,
            tooltip: "Chance a walker that touches the cluster actually sticks. Below 1 a \
                      walker explores further before attaching, which fills the fjords and \
                      makes a denser, less branched cluster.",
            choices: &[],
        },
        SimParamDef {
            name: "crowding",
            display_name: "Crowding",
            default: 2.0,
            min: 0.5,
            max: 16.0,
            tooltip: "How many walkers may work the cluster at once, per cell of its                       circumference. This is the speed-against-fidelity knob: DLA is what                       it is because particles arrive ONE AT A TIME, so low values grow a                       truer aggregate slowly and high values grow a denser, blunter one                       fast.",
            choices: &[],
        },
        SimParamDef {
            name: "launch_gap",
            display_name: "Launch gap",
            default: 5.0,
            min: 2.0,
            max: 40.0,
            tooltip: "How far outside the cluster's current radius walkers are launched.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "cluster",
        display_name: "Cluster",
        params: &[("walkers", 4.0), ("p_stick", 1.0), ("crowding", 2.0), ("launch_gap", 5.0)],
        // Measured at 512^2: 1,200 steps grows ~43,000 particles to a
        // radius of 230, which is a full aggregate still clear of the
        // walls.
        steps: 1200,
        init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("age"),
            coloring_params: &[("window", 1200.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
    }],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let dep = sim_take_deposit(p);
    // Frozen is permanent.
    if (s.x > 0.0) {
        return s;
    }
    if (dep <= 0.0) {
        return s;
    }
    // Freeze, storing the distance from the centre so the min/max
    // reduction can report the cluster's radius.
    let c = vec2<f32>(sim_grid()) * 0.5;
    let d = length(vec2<f32>(p) - c);
    return vec4<f32>(d + 1.0, 0.0, f32(sim_step_index()), 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // The nucleus: the init shape, frozen. Distance plus one, as the
    // step pass stores it.
    if (inside < 0.5) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let c = vec2<f32>(sim_grid()) * 0.5;
    return vec4<f32>(length(vec2<f32>(p) - c) + 1.0, 0.0, 0.0, 0.0);
}
"#,
    default_steps: 1200,
    passes: 1,
    repeat: None,
    agents: Some(crate::sim::AgentDef {
        count: |p, w, h| {
            let pct = p.get("walkers").clamp(0.05, 40.0);
            (((w as f64) * (h as f64) * pct as f64 / 100.0) as u32)
                .clamp(1, crate::sim::MAX_AGENTS)
        },
        passes: 1,
        wgsl: r#"
const DLA_TAU: f32 = 6.28318530718;

// The cluster's radius, from the maximum of channel .x -- which is a
// frozen cell's distance from the centre plus one.
fn dla_radius() -> f32 {
    return max(sim_minmax().y - 1.0, 0.0);
}

// A fresh walker on the launch circle, just outside the cluster.
fn dla_launch(i: u32, salt: u32) -> SimAgent {
    let g = vec2<f32>(sim_grid());
    let c = g * 0.5;
    let limit = min(g.x, g.y) * 0.5 - 2.0;
    let r = min(dla_radius() + mparam(2u), limit);
    let a = agent_rand(i, salt) * DLA_TAU;
    var out: SimAgent;
    out.pos = c + vec2<f32>(cos(a), sin(a)) * r;
    out.heading = 0.0;
    out.state = 0.0;
    return out;
}

fn sim_agent_seed(i: u32) -> SimAgent {
    return dla_launch(i, 0x21u);
}

fn sim_agent(a: SimAgent, i: u32) -> SimAgent {
    let g = vec2<f32>(sim_grid());
    let c = g * 0.5;

    // ONLY AS MANY WALKERS AS THE LAUNCH CIRCLE HAS ROOM FOR. The
    // parallel variant is DLA only while walkers arrive at the cluster
    // one at a time; launch a thousand of them onto a circle of radius
    // 5 and every cell around the seed freezes at once, which is a
    // solid disc, not an aggregate. Measured: a 4% population grew a
    // 40-cell solid core, and even 0.5% grew a 15-cell one -- the
    // circle simply has 31 cells at radius 5.
    //
    // So a walker is dormant until the cluster is big enough for it,
    // one walker per cell of circumference, and parks on the launch
    // circle meanwhile. The active population then grows with the
    // cluster on its own, whatever the parameter says.
    let launch_r = min(dla_radius() + mparam(2u), min(g.x, g.y) * 0.5 - 2.0);
    if (f32(i) > mparam(3u) * 6.28318530718 * launch_r) {
        return dla_launch(i, 0x26u);
    }

    let cell = vec2<i32>(floor(a.pos + vec2<f32>(0.5, 0.5)));

    // Touching the cluster? Four-neighbour contact, which is the
    // lattice DLA convention.
    let touch = sim_read(cell + vec2<i32>(1, 0)).x > 0.0
        || sim_read(cell + vec2<i32>(-1, 0)).x > 0.0
        || sim_read(cell + vec2<i32>(0, 1)).x > 0.0
        || sim_read(cell + vec2<i32>(0, -1)).x > 0.0;
    if (touch && sim_read(cell).x <= 0.0) {
        if (agent_rand(i, 0x22u) < mparam(1u)) {
            // Stick here, and start again. The deposit is what the
            // step pass turns into a frozen cell.
            agent_deposit(cell, 1.0);
            return dla_launch(i, 0x23u);
        }
    }

    // One lattice step.
    let r = agent_rand(i, 0x24u);
    var d = vec2<f32>(1.0, 0.0);
    if (r < 0.25) { d = vec2<f32>(-1.0, 0.0); }
    else if (r < 0.5) { d = vec2<f32>(0.0, 1.0); }
    else if (r < 0.75) { d = vec2<f32>(0.0, -1.0); }
    var out = a;
    out.pos = a.pos + d;

    // Gone too far to be worth following: relaunch. The classic kill
    // radius is three times the cluster's -- but it must ALWAYS exceed
    // the launch radius, or every walker dies on the step it is born
    // and the cluster never grows past its seed. Measured: with a
    // launch gap of 20 and a bare `max(3r, 16)` the run ended with one
    // particle, the seed.
    let kill = min(max(3.0 * dla_radius(), launch_r + 16.0), min(g.x, g.y) * 0.5 - 1.0);
    if (length(out.pos - c) > kill) {
        return dla_launch(i, 0x25u);
    }
    return out;
}
"#,
    }),
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


/// The Abelian sandpile: integer grains that topple when a site holds
/// four or more, one to each von Neumann neighbour, until nothing is
/// over full.
///
/// Bak, Tang and Wiesenfeld introduced it as the canonical model of
/// self-organised criticality (*Phys. Rev. Lett.* 59 (1987) 381); the
/// picture here is the one Pegden and Smart's scaling-limit theorem is
/// about -- N grains dropped on ONE site and stabilised, whose
/// coloured height field converges to a fractal limit shape as
/// N grows.
///
/// **Why a GPU may topple every site at once.** The model is Abelian:
/// the stable configuration does not depend on the order the topplings
/// are applied in. So a round may fire every over-full site
/// simultaneously, and a site holding `h` may fire `floor(h / 4)` times
/// in that one round, which is just that many consecutive single
/// topplings. One round is one dispatch, a five-tap integer gather with
/// no atomics.
///
/// **The cost is quadratic in the picture's radius**, which is the one
/// thing the plan could not estimate and `proto_sandpile.py` measured:
/// rounds ~ N^0.978 and radius ~ N^0.495, so rounds / radius itself
/// grows as sqrt(N). Measured round counts, edge sinks, mass
/// conservation asserted: 787 at 2^12, 3,695 at 2^14, 12,837 at 2^16,
/// 49,232 at 2^18, **190,006 at 2^20**. A guess of "rounds >= radius"
/// undersold the target size by a factor of 500.
///
/// Channels: `.x` = height, `.y` = the ODOMETER (how many times the
/// site has ever toppled -- the discrete superharmonic function the
/// theory is written in terms of), `.z` = the last round it toppled,
/// which is what `age` draws as the avalanche front.
pub static SANDPILE: ModelDef = ModelDef {
    name: "sandpile",
    display_name: "Abelian Sandpile",
    description: "Grains dropped on one site and toppled until stable. The four-colour \
                  height field is a fractal limit shape; the edges are sinks.",
    features: &[ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "grains_log2",
            display_name: "Grains (2ⁿ)",
            default: 16.0,
            min: 10.0,
            max: 24.0,
            tooltip: "How many grains land on the seed, as a power of two. THE COST IS \
                      QUADRATIC IN THE PICTURE'S RADIUS: measured, 2¹⁶ stabilises in 12,837 \
                      rounds and 2²⁰ in 190,006, so raising this by one costs roughly twice \
                      the steps and needs a grid √2 wider. Raise Steps with it — a pile that \
                      runs out of steps is simply unfinished. 24 is the ceiling because the \
                      field is f32, which counts exactly to 2²⁴ and no further.",
            choices: &[],
        },
        SimParamDef {
            name: "neighbourhood",
            display_name: "Neighbourhood",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "The graph the pile topples on. Von Neumann is the classic square \
                      lattice: topple at 4, one grain to each edge neighbour, and stable \
                      heights are 0–3. Moore is the same rule on the 8-neighbour graph — \
                      topple at 8, stable heights 0–7 — so the same mass settles denser \
                      and over a smaller, rounder shape (measured at 2¹⁶: 133 cells across \
                      in 4,652 rounds, against 189 in 12,837). Colouring by height wants a \
                      scale of 1/7 rather than 1/3.",
            choices: &["Von Neumann (4)", "Moore (8)"],
        },
    ],
    presets: &[
        SimPreset {
            name: "pile",
            display_name: "Pile",
            params: &[("grains_log2", 16.0), ("neighbourhood", 0.0)],
            // Measured, both on the CPU prototype and against this
            // shader: 2^16 grains stabilise in exactly 12,837 rounds
            // and span 188 cells, which fits a 256 grid with margin.
            steps: 12_837,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.3333333), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "moore",
            display_name: "Moore pile",
            params: &[("grains_log2", 16.0), ("neighbourhood", 1.0)],
            // Measured against the CPU mirror at this size: 4,652
            // rounds, spanning 133 cells against von Neumann's 189.
            // The 8-neighbour graph holds up to 7 grains per site, so
            // the same mass settles denser, smaller and sooner.
            steps: 4_652,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 0.0), ("scale", 0.1428571), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
// How many times a site holding `h` fires in one round: floor(h / n),
// which is that many consecutive single topplings.
fn sand_fires(h: f32, thresh: f32) -> f32 {
    return floor(h / thresh);
}

fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let moore = mparam(1u) >= 0.5;
    let thresh = select(4.0, 8.0, moore);

    // What leaves, and what arrives. Every neighbour's contribution is
    // read from ITS height, so this is a pure gather -- no atomics, and
    // the round is order-independent for the same reason the model is
    // Abelian.
    let fired = sand_fires(s.x, thresh);
    var got = sand_fires(sim_read(p + vec2<i32>( 1,  0)).x, thresh)
            + sand_fires(sim_read(p + vec2<i32>(-1,  0)).x, thresh)
            + sand_fires(sim_read(p + vec2<i32>( 0,  1)).x, thresh)
            + sand_fires(sim_read(p + vec2<i32>( 0, -1)).x, thresh);
    if (moore) {
        got = got + sand_fires(sim_read(p + vec2<i32>( 1,  1)).x, thresh)
                  + sand_fires(sim_read(p + vec2<i32>( 1, -1)).x, thresh)
                  + sand_fires(sim_read(p + vec2<i32>(-1,  1)).x, thresh)
                  + sand_fires(sim_read(p + vec2<i32>(-1, -1)).x, thresh);
    }

    // THE BOUNDARY IS THE SINK, and it has to be Zero. Grains sent off
    // the edge are read by nobody and are gone, which is the model's
    // open boundary. Under Clamp the outside mirrors the edge cell, so
    // an edge site receives copies of its own topplings and the pile
    // GAINS mass; under Periodic it wraps and the picture is a torus.
    // Both are boundary conditions, neither is Bak-Tang-Wiesenfeld.
    let h = s.x - fired * thresh + got;
    let last = select(s.z, f32(sim_step_index()), fired > 0.0);
    return vec4<f32>(h, s.y + fired, last, 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // The whole pile on the init shape. Center is one cell, which is
    // the classic picture; a Blob spreads the same mass over a square
    // and settles into the same limit shape from a blunter start.
    //
    // Rounded rather than taken raw: exp2 of an integer should be
    // exact, but the seed mass is not a thing to leave to a library's
    // last ulp, and every value here is far more than half a unit from
    // its neighbours.
    let n = floor(exp2(floor(clamp(mparam(0u), 1.0, 24.0))) + 0.5);
    return vec4<f32>(select(0.0, n, inside >= 0.5), 0.0, 0.0, 0.0);
}
"#,
    default_steps: 12_837,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};

/// Invasion percolation: a cluster that grows by always taking the
/// easiest site available to it.
///
/// Wilkinson and Willemsen (*J. Phys. A* 16 (1983) 3365) gave every
/// site a random threshold and grew the invaded region by repeatedly
/// adding the boundary site with the LOWEST threshold — one site per
/// step, a global argmin.
///
/// **The GPU rule is a rising threshold, and it is the same set.**
/// Because the sequential rule only ever adds sites in increasing
/// order of threshold, the region it has invaded at the moment its
/// running maximum reaches p is exactly the seed's connected component
/// among the sites with r < p. So raise a threshold p(t) instead and
/// let every eligible front site join at once: the invaded set is that
/// component, and a whole front advances per dispatch instead of one
/// site per dispatch. What it does NOT reproduce is the sequential
/// ORDER within a step, so the `age` colouring draws the front's
/// arrival time rather than the invasion sequence.
///
/// **The one thing that can go wrong is the front falling behind.**
/// The set is right only once the one-cell-per-step wavefront has
/// caught up with the threshold; a p that rises too fast leaves a
/// ball, not a cluster. That is what `p_max` is for -- the threshold
/// stops there and the remaining steps let the front finish. It is
/// also why `dp` is small by default: with the threshold rising slowly
/// the front is never the constraint, so `age` records the ORDER
/// SITES WERE INVADED IN rather than how far they are from the seed,
/// which is the picture the sequential model draws. Measured at 256²
/// with the shipped preset, the invaded set equals a CPU flood fill of
/// {r < p_max} from the seed, exactly, and the front finishes with
/// steps to spare.
///
/// **A point seed is a lottery, and that is why the presets inject
/// from an edge** -- which is also Wilkinson and Willemsen's own
/// geometry, a fluid pushed in at one face of a porous medium.
/// Measured at 256², seeding one cell and flooding to p = 0.60: five
/// seeds gave 8,526, 5,007, 87, 97 and 78 sites. Three of five landed
/// in a small finite cluster, because at p near the threshold almost
/// every site belongs to one. An injected edge always meets the large
/// cluster and the picture is the same from any seed.
///
/// **The interesting window is narrow and slightly above p_c.**
/// Measured at 256² from an edge: the cluster spans between p = 0.600
/// and p = 0.610, above the infinite-lattice threshold 0.5927 as a
/// finite grid should. Below spanning it is ramified (15,907 sites at
/// p = 0.600, a quarter of the grid); a few hundredths higher it is
/// half the grid and reads as a solid with holes. Both presets sit in
/// that window. Box counting at this size does NOT resolve the
/// critical exponent -- it reads 1.66 to 1.71 near the threshold
/// against the exact 91/48 = 1.896, a crossover the grid is too small
/// to escape, and it climbs past 1.89 once the cluster is dense, so
/// the number is a ramification check and not evidence of criticality.
///
/// Channels: `.x` = invaded, `.y` = the site's threshold r, `.z` = the
/// step it was invaded.
pub static INVASION_PERCOLATION: ModelDef = ModelDef {
    name: "invasion_percolation",
    display_name: "Invasion Percolation",
    description: "A cluster that spreads into whichever sites resist it least, as a rising \
                  threshold. At the percolation threshold it is a critical cluster, all \
                  fjords and dangling ends.",
    features: &[ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "dp",
            display_name: "Threshold rise per step",
            default: 0.0005,
            min: 0.00002,
            max: 0.05,
            tooltip: "How fast the invasion threshold climbs. This is a RATE, not a shape — \
                      the final cluster is the same set however slowly you reach the ceiling \
                      — but it decides what Age draws. Slow, and the front is never the \
                      constraint, so Age records the order sites were invaded in, which is \
                      the sequential model's own picture. Fast, and everything is eligible \
                      at once and Age just measures distance from the seed.",
            choices: &[],
        },
        SimParamDef {
            name: "p_max",
            display_name: "Threshold ceiling",
            default: 0.60,
            min: 0.45,
            max: 0.80,
            tooltip: "Where the threshold stops climbing, letting the remaining steps finish \
                      the front. THE INTERESTING WINDOW IS NARROW: the infinite lattice's \
                      site threshold is 0.5927, and measured on a 256² grid the cluster \
                      spans between 0.600 and 0.610. Below that it is ramified and full of \
                      fjords; a few hundredths above, it is half the grid and reads as a \
                      solid with holes. Far below, the invasion dies at once.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "front",
            display_name: "Invading front",
            // Injection from an edge, stopped just short of spanning:
            // measured at 256^2 this is 15,907 sites, a quarter of the
            // grid, all fjords and dangling ends.
            params: &[("dp", 0.0005), ("p_max", 0.60)],
            // p reaches the ceiling at step 1,200 and the front needs
            // a few hundred more; measured, it finishes well inside
            // this and extra steps then change nothing.
            steps: 2_000,
            init: Some(crate::config::sim::SimInit::Line),
            coloring: Some("age"),
            coloring_params: &[("window", 2000.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "spanning",
            display_name: "Spanning cluster",
            // The same injection carried past the transition, where
            // the cluster crosses the grid and the fjords behind it
            // fill in.
            params: &[("dp", 0.0005), ("p_max", 0.635)],
            steps: 2_000,
            init: Some(crate::config::sim::SimInit::Line),
            coloring: Some("age"),
            coloring_params: &[("window", 2000.0), ("invert", 1.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
    ],
    wgsl: r#"
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Invasion is permanent.
    if (s.x > 0.5) {
        return s;
    }
    // The threshold at this step, held at the ceiling once it arrives.
    // `step_index + 1` so the first step already has somewhere to go.
    let pt = min(f32(sim_step_index() + 1u) * mparam(0u), mparam(1u));
    if (s.y >= pt) {
        return s;
    }
    // Von Neumann adjacency, as in Wilkinson-Willemsen: the invaded
    // region is a lattice animal, and diagonal contact would let it
    // cross its own fjords.
    let n = sim_read(p + vec2<i32>( 1,  0)).x
          + sim_read(p + vec2<i32>(-1,  0)).x
          + sim_read(p + vec2<i32>( 0,  1)).x
          + sim_read(p + vec2<i32>( 0, -1)).x;
    if (n < 0.5) {
        return s;
    }
    return vec4<f32>(1.0, s.y, f32(sim_step_index()), 0.0);
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // Every site draws its threshold once, here, and keeps it for the
    // whole run: the disorder is quenched, which is what makes the
    // cluster a property of the field rather than of the schedule.
    // Its own salt, so an init shape that is itself noise does not
    // correlate the thresholds with the seed region.
    let r = sim_rand(p, 0x1a5u);
    return vec4<f32>(select(0.0, 1.0, inside >= 0.5), r, 0.0, 0.0);
}
"#,
    default_steps: 2_000,
    passes: 1,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


/// The Gravner-Griffeath snowfake: a mesoscopic lattice map that grows
/// a planar snow crystal from diffusion-limited aggregation,
/// anisotropic attachment kinetics and an idealised quasi-liquid layer.
///
/// J. Gravner, D. Griffeath, "Modeling snow crystal growth II: A
/// mesoscopic lattice map with plausible dynamics", *Physica D* 237
/// (2008) 385-404. Read in full (revised version, September 2007) --
/// **not Part III**, which is the three-dimensional sequel and whose
/// rule this catalogue previously stood in for. They differ where it
/// matters, see below.
///
/// **State**, four fields per site of the triangular lattice, which is
/// exactly the four channels: `.x` = a, attached (0 or 1); `.y` = b,
/// boundary mass (the quasi-liquid layer); `.z` = c, crystal mass
/// (ice); `.w` = d, diffusive mass (vapour). Initially one attached
/// cell with a = c = 1 at the origin and d = rho everywhere else.
///
/// **The rule**, its four deterministic substeps in the paper's order.
/// N_x is the site and its six neighbours; the boundary is an
/// unattached site with at least one attached neighbour.
///
/// (i) DIFFUSION, on the unattached sites: d' = (1/7) sum over N_x,
/// with reflecting conditions at the crystal -- a term for an attached
/// neighbour is replaced by the site's own d.
/// (ii) FREEZING, at boundary sites: a proportion kappa of the vapour
/// becomes ice directly and the rest quasi-liquid, and the vapour is
/// spent: b += (1-kappa) d, c += kappa d, d = 0.
/// (iii) ATTACHMENT, at boundary sites, by the number n of attached
/// neighbours: n of 1 or 2 needs b >= beta; n = 3 needs b >= 1, or
/// the knife-edge instability -- vapour in the neighbourhood below
/// theta and b >= alpha; n >= 4 attaches unconditionally. On
/// attaching, c += b and b = 0.
/// (iv) MELTING, at boundary sites: b and c give proportions mu and
/// gamma back to the vapour.
/// (v) NOISE, optional: d is perturbed by +/- sigma with equal
/// probability. The paper's own parameter studies are deterministic.
///
/// **TWO PASSES, not the four the plan expected.** Substeps (ii) and
/// (iv) read no neighbour: freezing needs only the vapour the
/// diffusion just left at the site itself, and melting only the site's
/// own masses. So (i)+(ii) are one dispatch and (iii)+(iv) another,
/// and the pass boundary sits exactly where it must -- attachment
/// reads its neighbours' vapour AFTER freezing has zeroed it at every
/// boundary site, which is a neighbour read of a field the previous
/// substep wrote.
///
/// **Where Part III's rule, which this model was planned from,
/// differs.** Part III's freezing keeps kappa of the vapour AS vapour
/// (d' = kappa d); Part II spends it all, kappa going to ICE. Part
/// III's kappa, beta and mu are functions of the neighbour count;
/// Part II's are single constants, and the neighbour count enters only
/// through the three attachment cases. Part II also has alpha and
/// theta -- the knife-edge instability, which has no Part III
/// analogue here -- and gamma, crystal mass melting back to vapour.
/// The plan's guessed `kappa1..3 / beta1..3 / mu1..3` parameter set was
/// wrong in shape as well as in value.
///
/// **The paper contradicts itself about alpha and theta, and the
/// appendix decides it.** Equation (3b) has theta bounding the vapour
/// sum and alpha the boundary mass; section 5's prose says the
/// reverse. The appendix tabulates every figure's parameters in the
/// order rho beta alpha theta kappa mu gamma sigma, and for two of
/// the three case studies of section 6 the table and the running text
/// agree -- but for figure 13 left the text says alpha = .026,
/// theta = .2 where the table says alpha = .2, theta = .026.
///
/// **Measured, the table is right.** Under the equation with the
/// TABLE's values every one of the three reproduces the morphology
/// the text describes. Under the equation with the TEXT's values,
/// figure 13 left grows a featureless hexagonal plate at any size or
/// duration tried -- 40,000 steps on a 1024 grid included -- because
/// a vapour cutoff of 0.2 fires the knife-edge everywhere around a
/// large crystal and an attachment threshold of 0.026 then fills
/// every concavity as fast as it appears. The equation is
/// implemented, and the presets take their numbers from the appendix.
/// (The appendix also gives figure 13 right gamma = .0006 where the
/// text says .00006.)
///
/// **One ambiguity, decided on mechanism.** Steps (ii) and (iv) are
/// written "for x in dA_t", the boundary at the START of the cycle,
/// which would include a site that attached in step (iii); but the
/// paper also says attachment is permanent and there are "no further
/// dynamics at attached sites". Melting a freshly attached site would
/// move gamma*c into vapour AT AN ATTACHED SITE, where nothing
/// diffuses it and it is stranded. Melting therefore skips sites that
/// just attached.
pub static SNOWFAKE: ModelDef = ModelDef {
    name: "snowfake",
    display_name: "Gravner–Griffeath Snowfake",
    description: "A planar snow crystal grown from vapour on a hexagonal lattice: diffusion, \
                  a quasi-liquid boundary layer, and attachment that is harder on a flat face \
                  than in a valley. Plates, stars, dendrites and sectored plates.",
    features: &[ModelFeature::NeedsRng, ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "rho",
            display_name: "Vapour density ρ",
            default: 0.65,
            min: 0.10,
            max: 1.00,
            tooltip: "How much vapour the cloud starts with, at every cell that is not the \
                      seed. The paper's crystals use 0.35 to 0.8, and it is the easiest \
                      parameter to read: more vapour grows faster and pushes the form from \
                      plate to sectored plate to dendrite. It is read by the SEED, so a                       change to it shows on the next Reset rather than in the running                       crystal.",
            choices: &[],
        },
        SimParamDef {
            name: "beta",
            display_name: "Anisotropy β",
            default: 1.75,
            min: 1.0,
            max: 4.0,
            tooltip: "How much boundary mass a site with one or two attached neighbours needs \
                      before it joins — a tip or a flat face, where attachment is hard. A \
                      valley's threshold is 1, so it is the EXCESS OVER 1 that makes the \
                      crystal faceted. Raising it delays the first instability, grows the \
                      central plate larger, and moves the form from fern to dendrite to \
                      sectored plate. The paper uses 1.05 to 3.2.",
            choices: &[],
        },
        SimParamDef {
            name: "alpha",
            display_name: "Knife-edge mass α",
            default: 0.2,
            min: 0.0,
            max: 1.0,
            tooltip: "The reduced boundary-mass threshold a concave site may attach at once \
                      the vapour around it has run out — the knife-edge instability, by which \
                      a very thin plate spreads on almost no vapour. It is what fills the \
                      regions between the six main branches long after they have passed. \
                      Below the valley threshold of 1, and the paper's figures use 0 to 0.6.",
            choices: &[],
        },
        SimParamDef {
            name: "theta",
            display_name: "Knife-edge vapour θ",
            default: 0.026,
            min: 0.0,
            max: 0.5,
            tooltip: "How depleted the vapour in a site's neighbourhood must be before that \
                      reduced threshold applies. It is compared against the SUM over the site \
                      and its six neighbours, which far from the crystal is about 7ρ, so the \
                      paper's values of 0.003 to 0.11 mean 'almost nothing left'. Set it too \
                      high and the crystal fills into a featureless plate, because every \
                      concavity qualifies as soon as it forms.",
            choices: &[],
        },
        SimParamDef {
            name: "kappa",
            display_name: "Freezing κ",
            default: 0.15,
            min: 0.0,
            max: 0.5,
            tooltip: "The proportion of vapour arriving at the boundary that freezes straight \
                      to ice; the rest becomes quasi-liquid and is what attachment weighs. \
                      Ice deposited this way waits rather than attaching, so raising κ starves \
                      the fastest tips of boundary mass and — up to a point — encourages side \
                      branching. The paper's figures span 0 to 0.15.",
            choices: &[],
        },
        SimParamDef {
            name: "mu",
            display_name: "Melting μ",
            default: 0.015,
            min: 0.0,
            max: 0.3,
            tooltip: "The proportion of boundary mass that returns to vapour each step. It \
                      opposes freezing, and raising it promotes faceting: the paper's sequence \
                      from 0.04 to 0.09 turns a stellar dendrite with parabolic tips into a \
                      stellar plate with hexagonal ones.",
            choices: &[],
        },
        SimParamDef {
            name: "gamma",
            display_name: "Sublimation γ",
            default: 0.0001,
            min: 0.0,
            max: 0.01,
            tooltip: "The proportion of unattached ICE at the boundary that returns to vapour \
                      each step. It acts like melting but is typically far smaller — the \
                      paper's figures use 0.000001 to 0.01 — and it is included for \
                      completeness rather than for its effect.",
            choices: &[],
        },
        SimParamDef {
            name: "sigma",
            display_name: "Vapour noise σ",
            default: 0.0,
            min: 0.0,
            max: 0.1,
            tooltip: "Multiplies the vapour at every site by 1 ± σ each step, each sign \
                      equally likely. The paper's own parameter studies are deterministic \
                      (σ = 0) and complex without it; a little noise is how it studies how \
                      much of a real snowflake's asymmetry chance accounts for.",
            choices: &[],
        },
    ],
    presets: &[
        // Every row here is the appendix's, in its own column order:
        // rho, beta, alpha, theta, kappa, mu, gamma, sigma.
        SimPreset {
            name: "primitive",
            display_name: "Primitive (ridges)",
            // Figure 4, "the first instability: in the lab and
            // simulated" -- the primitive case, in which every
            // parameter but rho and beta is zero. No quasi-liquid
            // layer, no knife-edge, no melting: the anisotropy of
            // attachment alone, and it is enough for ridges.
            params: &[
                ("rho", 0.58), ("beta", 3.2), ("alpha", 0.0), ("theta", 0.0),
                ("kappa", 0.0), ("mu", 0.0), ("gamma", 0.0), ("sigma", 0.0),
            ],
            // Measured at 512^2: 24,000 steps is 14% of the grid and has
            // grown the side branches the paper says the primitive
            // case tends to -- "should resemble a fern once it grows
            // large enough".
            steps: 24_000,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 2.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "simple_star",
            display_name: "Simple star",
            // Figure 13 left: "rather high vapour density compared to
            // the anisotropy index promotes early onset of the first
            // instability and rapid advance of the six main tips. Very
            // strong direct freezing inhibits further branching."
            params: &[
                ("rho", 0.65), ("beta", 1.75), ("alpha", 0.2), ("theta", 0.026),
                ("kappa", 0.15), ("mu", 0.015), ("gamma", 0.0001), ("sigma", 0.0),
            ],
            // Measured at 512^2: 22% of the grid, six faceted arms with
            // the internal markings the figure is described by.
            steps: 12_000,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 2.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "plate_ends",
            display_name: "Stellar plate ends",
            // Figure 13 middle: low vapour and low anisotropy, with a
            // very high melting rate "repeatedly repairing tip
            // instabilities" into large hexagonal plate ends.
            params: &[
                ("rho", 0.36), ("beta", 1.09), ("alpha", 0.01), ("theta", 0.0745),
                ("kappa", 0.0001), ("mu", 0.14), ("gamma", 0.00001), ("sigma", 0.0),
            ],
            // Measured at 512^2: this one is the paper's slow grower --
            // "more than 100,000 updates" at its own scale -- and
            // 40,000 is where the plate ends are unmistakable here.
            steps: 40_000,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 2.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "dendrite_ends",
            display_name: "Plate with dendrite ends",
            // Figure 13 right: the same melting with quite different
            // aftergrowth, so the central plate keeps spreading as the
            // knife-edge fills between the six main tips.
            params: &[
                ("rho", 0.38), ("beta", 1.06), ("alpha", 0.35), ("theta", 0.112),
                ("kappa", 0.001), ("mu", 0.14), ("gamma", 0.0006), ("sigma", 0.0),
            ],
            // Measured at 512^2: 31% of the grid, with side branching.
            steps: 20_000,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 2.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::X,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
    ],
    wgsl: r#"
// One of the six neighbours on an offset-row hex lattice: odd rows sit
// half a cell to the right, so the four diagonal neighbours move with
// the row parity and the two horizontal ones do not. Same addressing
// as PACKARD_SNOWFLAKE, and the same consequence — the array's rows
// are one apart where a triangular lattice's are √3/2, so the picture
// is stretched vertically by 2/√3.
fn gg_nb(p: vec2<i32>, i: i32) -> vec2<i32> {
    // 0 on odd rows, -1 on even: which side the diagonals lean.
    let k = select(-1, 0, (p.y & 1) == 1);
    if (i == 0) { return p + vec2<i32>(-1, 0); }
    if (i == 1) { return p + vec2<i32>(1, 0); }
    if (i == 2) { return p + vec2<i32>(k, -1); }
    if (i == 3) { return p + vec2<i32>(k + 1, -1); }
    if (i == 4) { return p + vec2<i32>(k, 1); }
    return p + vec2<i32>(k + 1, 1);
}

// Pass 1 — (i) diffusion and (ii) freezing.
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    // Attachment is permanent and attached sites hold no vapour.
    if (s.x > 0.5) {
        return s;
    }

    // Uniform weight 1/7 on the site and its six neighbours, with
    // REFLECTING conditions at the crystal: a term for an attached
    // neighbour is replaced by this site's own vapour, so nothing
    // diffuses into the ice. Away from the crystal no neighbour is
    // attached and this is the plain average.
    var attached = 0.0;
    var sum = s.w;
    for (var i = 0; i < 6; i = i + 1) {
        let n = sim_read(gg_nb(p, i));
        if (n.x > 0.5) {
            attached = attached + 1.0;
            sum = sum + s.w;
        } else {
            sum = sum + n.w;
        }
    }
    // DIVIDED, not multiplied by a reciprocal. The nearest f32 to 1/7
    // is larger than 1/7, so `sum * (1.0/7.0)` creates mass at a
    // steady 4.4e-8 a step -- measured, 1.2e-4 of the total after
    // 4,000 steps, in a model whose own paper offers conservation as
    // the way to check an implementation. A division rounds to
    // nearest and the drift becomes a random walk instead.
    var d = sum / 7.0;

    var b = s.y;
    var c = s.z;
    if (attached > 0.5) {
        // A boundary site. The vapour that reached it is spent: a
        // proportion kappa freezes straight to ice, the rest joins the
        // quasi-liquid layer.
        let kappa = mparam(4u);
        b = b + (1.0 - kappa) * d;
        c = c + kappa * d;
        d = 0.0;
    }
    return vec4<f32>(0.0, b, c, d);
}

// Pass 2 — (iii) attachment, (iv) melting, (v) noise.
fn sim_step2(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    if (s.x > 0.5) {
        return s;
    }
    var n = 0.0;
    // The vapour sum over N_x, which freezing has just zeroed at every
    // boundary site -- so this reads "how much vapour is left anywhere
    // near here", which is what the knife-edge condition wants.
    var dsum = s.w;
    for (var i = 0; i < 6; i = i + 1) {
        let q = sim_read(gg_nb(p, i));
        n = n + select(0.0, 1.0, q.x > 0.5);
        dsum = dsum + q.w;
    }

    var out = s;
    if (n > 0.5) {
        var attach = false;
        if (n > 3.5) {
            // (3c) Four or more attached neighbours: enough ice nearby
            // that the threshold is 0. Stops single-cell holes.
            attach = true;
        } else if (n > 2.5) {
            // (3b) A concavity: the threshold is normalised to 1, or
            // the knife-edge lets it through on alpha once the vapour
            // around it is below theta.
            // Equation (3b): theta bounds the VAPOUR, alpha the
            // boundary mass. See the swap note in the module docs.
            attach = s.y >= 1.0 || (dsum < mparam(3u) && s.y >= mparam(2u));
        } else {
            // (3a) One or two: a tip or a flat face, the hard case,
            // and beta over 1 is the whole anisotropy of the model.
            attach = s.y >= mparam(1u);
        }
        if (attach) {
            // (3d) The quasi-liquid becomes ice with the rest.
            out = vec4<f32>(1.0, 0.0, s.y + s.z, 0.0);
        } else {
            // (iv) Melting, at boundary sites that did NOT just
            // attach: an attached site has no further dynamics, and
            // vapour released there would be stranded where nothing
            // diffuses it.
            let mu = mparam(5u);
            let gamma = mparam(6u);
            out = vec4<f32>(0.0, (1.0 - mu) * s.y, (1.0 - gamma) * s.z,
                            s.w + mu * s.y + gamma * s.z);
        }
    }

    // (v) Noise: the vapour at every unattached site, up or down by
    // sigma with equal probability.
    let sigma = mparam(7u);
    if (sigma > 0.0 && out.x < 0.5) {
        out.w = out.w * (1.0 + select(-sigma, sigma, sim_rand(p, 0x5fu) < 0.5));
    }
    return out;
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // A mesoscopic prism at the origin -- attached, carrying one unit
    // of crystal mass -- in homogeneous vapour of density rho. Center
    // is the paper's single cell; a Blob starts from a wider nucleus.
    if (inside >= 0.5) {
        return vec4<f32>(1.0, 0.0, 1.0, 0.0);
    }
    return vec4<f32>(0.0, 0.0, 0.0, mparam(0u));
}
"#,
    default_steps: 6_000,
    passes: 2,
    repeat: None,
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


/// The dielectric breakdown model: a discharge pattern that grows one
/// site at a time, each chosen with probability proportional to the
/// local field raised to a power.
///
/// L. Niemeyer, L. Pietronero, H. J. Wiesmann, "Fractal dimension of
/// dielectric breakdown", *Phys. Rev. Lett.* 52 (1984) 1033. Read in
/// full (the scan is `output/pdf/dielectric_break.pdf`).
///
/// **The rule**, its section (a)-(c) and equations (3)-(5). The
/// pattern is an equipotential at φ = 0 and the far electrode is held
/// at φ = 1; between them ∇²φ = 0, discretised as
/// φ = ¼(φ₊ₓ + φ₋ₓ + φ₊ᵧ + φ₋ᵧ) and solved by iterating it. At each
/// step ONE bond is added, linking a site of the pattern to a new
/// site, chosen among all such bonds with
/// p ∝ (φ of the new site)^η. The paper's own convergence note is
/// where the relaxation slider's range comes from: "typically good
/// convergence is obtained with a number of iterations between 5 and
/// 50".
///
/// **One site per step, exactly, without a prefix scan.** The plan
/// expected the paper's global weighted choice to need a scan and
/// shipped a parallel approximation instead. It does not: drawing
/// E ~ Exp(1) per candidate and taking the ARGMIN of E/w is a draw
/// proportional to w -- the exponential race -- and a global minimum
/// is the min/max reduction phase 3 already built. So this is the
/// paper's rule and not an approximation of it, at the cost of one
/// extra pass.
///
/// **A white point reachable from several black points is likelier**,
/// because each bond is a candidate: the weight is the number of
/// pattern neighbours times φ^η. The paper says so in passing, for
/// η = 0, and it is what makes that case not quite the Eden model.
///
/// Table I is the gate: D = 2 at η = 0, 1.89 ± 0.01 at η = 0.5,
/// 1.75 ± 0.02 at η = 1, about 1.6 at η = 2. The η = 1 value is also
/// the experimental Lichtenberg figure's (≈1.7 from photographs) and
/// DLA's, which this catalogue measured at 1.753 in phase 4.
///
/// **The Saffman-Taylor half of this wave did not work, and the knob
/// that remains is named for what it does.** Saffman and Taylor
/// (*Proc. R. Soc. A* 245 (1958) 312, read) measured a finger
/// occupying "a little more than half the width of the channel" --
/// λ = 0.485, 0.502, 0.508, 0.514 at four stations at 1 mm/s, and
/// 0.87 at a twentieth of that speed, where surface tension matters
/// more. The catalogue proposed reaching that by holding the pattern
/// at φ = −d₀κ instead of 0, with κ from the 3×3 occupancy.
///
/// Measured in a 256-wide channel, it does not. At d₀ of 0, 0.01 and
/// 0.03 the pattern crosses the cell as a branched dendrite whose
/// width at three stations reads 0.02 to 0.23 of the channel, against
/// the experiment's 0.5 -- and raising d₀ makes it NARROWER, where
/// the experiment makes it wider. Above about 0.05 growth stops
/// entirely (78 sites in 8,000 steps), because a tip held at a
/// negative potential drags its whole neighbourhood below zero and
/// nothing is a candidate any more. Growing every candidate at once
/// rather than one per step -- which is the right rule for a moving
/// interface, and is why `selection` exists -- fixed the stalling but
/// not the width.
///
/// So there is no "viscous finger" preset: a lattice interface that
/// advances by independent per-site coin flips stays rough, and
/// smoothing its potential does not make it compact. `d₀` ships as a
/// tip penalty, which is what it measurably is.
///
/// Channels: `.x` = this step's race key (the reduce's channel),
/// `.y` = φ, `.z` = the step the site joined, `.w` = in the pattern.
pub static DBM: ModelDef = ModelDef {
    name: "dbm",
    display_name: "Dielectric Breakdown",
    description: "A discharge that branches into whichever gap has the strongest field, one \
                  site at a time, exactly as the paper draws it. Lightning and Lichtenberg \
                  figures.",
    features: &[
        ModelFeature::NeedsRng,
        ModelFeature::NeedsMinMax,
        ModelFeature::NoTimeStep,
    ],
    parameters: &[
        SimParamDef {
            name: "eta",
            display_name: "Field exponent η",
            default: 1.0,
            min: 0.0,
            max: 4.0,
            tooltip: "How sharply growth prefers a strong field. THE DIMENSION DEPENDS ON IT \
                      and the paper tabulates it: 2 at η = 0, 1.89 at 0.5, 1.75 at 1, about \
                      1.6 at 2. η = 1 is growth proportional to the field — the realistic case \
                      for a discharge, and the one whose pattern and dimension match both a \
                      photographed Lichtenberg figure and DLA. Higher makes the structure more \
                      nearly linear.",
            choices: &[],
        },
        SimParamDef {
            name: "relax",
            display_name: "Relaxation sweeps",
            default: 10.0,
            min: 1.0,
            max: 200.0,
            tooltip: "How many times Laplace's equation is iterated between one growth and the \
                      next. The paper's own note: 'typically good convergence is obtained with \
                      a number of iterations between 5 and 50'. It is warm-started from the \
                      last solution, and the cost is proportional: measured at 1080p, 1.8 ms a \
                      step at 5 sweeps, 5.5 at 20, 12.8 at 50 — while the pattern's dimension \
                      at η = 1 is the same from 3 sweeps to 150 (1.69 to 1.71, within sample \
                      noise). Ten is inside the paper's range and twice as fast as twenty.",
            choices: &[],
        },
        SimParamDef {
            name: "surface_tension",
            display_name: "Tip penalty d₀",
            default: 0.0,
            min: 0.0,
            max: 0.1,
            tooltip: "Holds the pattern at −d₀ × curvature instead of at 0, so a protruding tip \
                      sits at a lower potential and grows more slowly. It thins and straightens \
                      the pattern. IT IS NOT SAFFMAN–TAYLOR: it was added as the lattice \
                      analogue of their surface-tension boundary condition and measurably does \
                      not reproduce their finger — see the model's notes. Above about 0.05 it \
                      stops growth altogether. Zero is the dielectric breakdown model as \
                      published.",
            choices: &[],
        },
        SimParamDef {
            name: "selection",
            display_name: "Growth",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "How many sites join per step. One site is the paper's rule exactly — a \
                      single bond added per step, chosen among all candidates in proportion to \
                      the field — and it is what a branching DISCHARGE is. All sites advances \
                      the whole interface at once, each candidate independently at a rate \
                      proportional to the field, which is what a moving FLUID interface is. \
                      They are different processes, not an approximation of one another.",
            choices: &["One site (race)", "All sites (rate)"],
        },
        SimParamDef {
            name: "rate",
            display_name: "Growth rate",
            default: 0.05,
            min: 0.001,
            max: 1.0,
            tooltip: "In All-sites mode, the chance per step that a candidate with unit field \
                      joins. Small keeps the interface smooth and costs steps; large advances \
                      it in coarse jumps. Ignored in One-site mode.",
            choices: &[],
        },
        SimParamDef {
            name: "electrode",
            display_name: "Far electrode",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Where the potential is held at 1. Radial is the paper's figure 2 — a \
                      circle at large enough distance, with the discharge starting from a \
                      point at the centre. Channel is one plate along an edge, for a \
                      discharge from a line — seed it with one.",
            choices: &["Radial", "Channel"],
        },
    ],
    presets: &[
        SimPreset {
            name: "lichtenberg",
            display_name: "Lichtenberg figure",
            // eta = 1, the paper's own case: "the most realistic case
            // for the present experiment", D = 1.75.
            params: &[("eta", 1.0), ("relax", 10.0), ("surface_tension", 0.0),
                     ("selection", 0.0), ("rate", 0.05), ("electrode", 0.0)],
            // One site per step, so this is the particle count. The
            // paper's figure 3 is "about 5000 steps".
            steps: 5_000,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 3.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::W,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "dense",
            display_name: "Dense (η = 0)",
            // Growth independent of the field: D = 2, the compact end
            // of the table.
            params: &[("eta", 0.0), ("relax", 10.0), ("surface_tension", 0.0),
                     ("selection", 0.0), ("rate", 0.05), ("electrode", 0.0)],
            steps: 5_000,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 3.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::W,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
        SimPreset {
            name: "sparse",
            display_name: "Sparse (η = 2)",
            // D about 1.6: fewer, longer branches.
            params: &[("eta", 2.0), ("relax", 10.0), ("surface_tension", 0.0),
                     ("selection", 0.0), ("rate", 0.05), ("electrode", 0.0)],
            steps: 5_000,
            init: Some(crate::config::sim::SimInit::Center),
            coloring: Some("channel"),
            coloring_params: &[("channel", 3.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: Some(crate::config::sim::SimMatte {
                channel: crate::config::sim::SimMatteChannel::W,
                cutoff: 0.5,
                softness: 0.0,
                invert: false,
            }),
        },
    ],
    wgsl: r#"
// The key a site that cannot grow writes: larger than any real race
// key, so the global minimum ignores it.
const DBM_FAR: f32 = 1.0e30;

// Where the potential is held at 1.
fn dbm_electrode(p: vec2<i32>) -> bool {
    let g = sim_grid();
    if (mparam(5u) >= 0.5) {
        // Channel: one plate along the near edge of the cell.
        return p.y <= 0;
    }
    // Radial: "the other electrode is modeled as a circle at large
    // enough distance", the paper's figure 2.
    let c = vec2<f32>(g) * 0.5;
    let r = min(f32(g.x), f32(g.y)) * 0.48;
    return length(vec2<f32>(p) - c) >= r;
}

// How many of the four nearest neighbours are in the pattern. Each is
// one of the paper's dashed bonds, so it is also the number of ways
// this site can be reached.
fn dbm_bonds(p: vec2<i32>) -> f32 {
    return sim_read(p + vec2<i32>( 1,  0)).w
         + sim_read(p + vec2<i32>(-1,  0)).w
         + sim_read(p + vec2<i32>( 0,  1)).w
         + sim_read(p + vec2<i32>( 0, -1)).w;
}

// A 3x3 occupancy estimate of interface curvature, positive at a
// protruding tip. A flat interface has five of eight neighbours in the
// pattern, a tip fewer, a notch more. OURS: Saffman and Taylor give
// the continuum boundary condition, not a lattice estimate of it.
// Interior sites are not interface and stay at 0.
fn dbm_curvature(p: vec2<i32>) -> f32 {
    var n = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            if (dx != 0 || dy != 0) {
                n = n + sim_read(p + vec2<i32>(dx, dy)).w;
            }
        }
    }
    if (n > 7.5) {
        return 0.0;
    }
    return (5.0 - n) * (1.0 / 3.0);
}

// What a site's growth is proportional to: the field to the power eta,
// times the number of bonds that reach it. Zero for anything that is
// not a candidate.
fn dbm_weight(s: vec4<f32>, p: vec2<i32>) -> f32 {
    if (s.w > 0.5 || dbm_electrode(p)) {
        return 0.0;
    }
    let bonds = dbm_bonds(p);
    let phi = max(s.y, 0.0);
    if (bonds < 0.5 || phi <= 0.0) {
        return 0.0;
    }
    // Per BOND, so a site reachable from several pattern sites is
    // likelier in proportion -- the paper says so in passing, and it
    // is what makes eta = 0 not quite the Eden model.
    return bonds * pow(phi, mparam(0u));
}

// Pass 1 of 3 -- growth, by whichever rule `selection` names.
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    var out = s;
    var joins = false;
    if (s.w < 0.5) {
        if (mparam(3u) >= 0.5) {
            // All sites: every candidate independently, at a rate
            // proportional to its weight. A moving interface, not a
            // discharge -- and `steps` is then time rather than a
            // particle count.
            //
            // NORMALISED BY THE STRONGEST CANDIDATE, which the same
            // reduce provides as the maximum of channel .x. Without
            // it the rate would depend on how far away the electrode
            // is: in a channel 254 cells deep the converged field at
            // the interface is about 1/254, so a rate of 0.05 grew 373
            // sites in 3,000 steps -- measured -- and the slider would
            // mean something different at every grid size.
            let w = dbm_weight(s, p);
            let peak = max(sim_minmax().y, 1.0e-20);
            joins = w > 0.0 && sim_rand(p, 0x7bu) < mparam(4u) * w / peak;
        } else {
            // One site: the winner of the exponential race started at
            // the end of the last step. Every site that could not grow
            // wrote DBM_FAR, so a minimum anywhere near it means there
            // was no candidate -- which is also what the reduce's
            // fallback gives before the first race has run.
            let best = sim_minmax().x;
            joins = best < DBM_FAR * 0.5 && s.x == best;
        }
    }
    if (joins) {
        out.w = 1.0;
        out.y = 0.0;
        out.z = f32(sim_step_index());
    }
    // A key belongs to one step. Clearing here means a site that lost
    // cannot win later on a stale draw.
    out.x = DBM_FAR;
    return out;
}

// Pass 2 of 3 -- one Jacobi sweep of Laplace's equation, equation (5).
// This is the pass that repeats: `relax` of them between one growth
// and the next, warm-started from the last solution.
fn sim_step2(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    var out = s;
    if (dbm_electrode(p)) {
        out.y = 1.0;
        return out;
    }
    if (s.w > 0.5) {
        // The discharge pattern is an equipotential at 0 -- or, with
        // surface tension, at minus the interface's curvature, so a
        // tip is at a lower potential than a flat face and grows more
        // slowly.
        out.y = -mparam(2u) * dbm_curvature(p);
        return out;
    }
    out.y = 0.25 * (sim_read(p + vec2<i32>( 1,  0)).y
                  + sim_read(p + vec2<i32>(-1,  0)).y
                  + sim_read(p + vec2<i32>( 0,  1)).y
                  + sim_read(p + vec2<i32>( 0, -1)).y);
    return out;
}

// Pass 3 of 3 -- weigh every candidate and start its race.
fn sim_step3(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    var out = s;
    // A candidate is a site outside the pattern with a pattern
    // neighbour: the far end of one of the paper's dashed bonds.
    let w = dbm_weight(s, p);

    // All-sites mode wants the largest weight, so channel .x carries
    // the weights themselves and the reduce's MAXIMUM is what the next
    // step normalises by. One-site mode wants the smallest race key,
    // and the same reduce's MINIMUM is the winner.
    if (mparam(3u) >= 0.5) {
        out.x = w;
        return out;
    }
    out.x = DBM_FAR;
    if (w <= 0.0) {
        return out;
    }

    // The exponential race. With E ~ Exp(1) drawn independently per
    // candidate, argmin(E / w) is distributed exactly as the paper's
    // equation (3) -- and argmin over the grid is the reduction.
    let u = max(sim_rand(p, 0xdbu), 1.0e-7);
    out.x = -log(u) / max(w, 1.0e-30);
    return out;
}
"#,
    wgsl_seed: r#"
// Declared again because the seed pass carries `wgsl_seed` alone, not
// the model's `wgsl`; the two never share a module, so this is not a
// redefinition.
const DBM_FAR: f32 = 1.0e30;

fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    // The init shape IS the starting pattern: Center for the paper's
    // radial discharge from a point, Line for a plane interface.
    let pat = select(0.0, 1.0, inside >= 0.5);
    // The potential starts at the electrode's value everywhere else
    // and the relaxation pulls it down toward the pattern.
    return vec4<f32>(DBM_FAR, select(1.0, 0.0, pat > 0.5), 0.0, pat);
}
"#,
    default_steps: 5_000,
    passes: 3,
    // The relaxation: pass 1 runs `relax` times per growth.
    repeat: Some((1, "relax")),
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};


/// Viscous fingering: a less viscous fluid pushed into a more viscous
/// one through a porous medium or Hele-Shaw cell, as a MISCIBLE
/// displacement -- concentration-dependent viscosity, Darcy flow, and
/// advection of the concentration by the flow it sets up.
///
/// **The instability is Saffman and Taylor's** (*Proc. R. Soc. A* 245
/// (1958) 312, read): "the interface between them is liable to be
/// unstable if the driving fluid is the less viscous of the two". That
/// sentence is the gate -- see `fingering_is_unstable_only_when_the_driving_fluid_is_less_viscous`.
///
/// **The formulation is Holzbecher's** ("Modeling of Viscous
/// Fingering", COMSOL Conference 2009, Milan; `output/pdf/Holzbecher.pdf`,
/// read), after Zimmerman & Homsy and Coutinho & Alves: incompressible
/// Darcy flow ∇·u = 0, u = −(k/μ)∇p, with the mobility depending on
/// concentration as k/μ = exp(R c) (his equation 8, R = ln M for
/// mobility ratio M), and the transport equation ∂c/∂t = ∇·(D∇c) − u·∇c
/// (his 2). The less viscous fluid enters at one edge with c = 1 into
/// a cell initially at c = 0, between two impermeable walls, and the
/// initial concentration near the inlet carries a random disturbance
/// of size ζ (his 12) -- without it a perfectly flat front stays flat
/// for ever. His own runs are at "M log(3)" in Table 1, whose meaning
/// the extracted text does not settle (M = 3, or ln M = 3); this
/// model does not claim his value and exposes R.
///
/// **What is simplified, and why.** Holzbecher's dispersion is a
/// tensor aligned with the flow (his 3); this uses isotropic diffusion,
/// which changes how fingers smear but not whether they grow. And the
/// cell is driven at constant PEAK SPEED rather than his constant
/// flux: the velocity field is Darcy's exactly in shape and is
/// rescaled each step so the fastest cell moves `speed` cells a step
/// -- the maximum the min/max reduce already computes. Darcy flow is
/// quasi-static and linear in the pressure drop, so this is the same
/// flow under a pressure drop that varies in time, and it is what
/// makes the explicit upwind advection unconditionally stable: the
/// Courant number is `speed`, by construction, whatever R and the
/// pattern do to the field.
///
/// **This is what the dielectric breakdown model's tip penalty was
/// for and could not do.** A lattice interface advancing by per-site
/// coin flips stays rough; a concentration advected by a divergence-
/// free flow does not. The catalogue's section 23 records both.
///
/// Channels: `.x` = |u| (the reduce's channel, for the speed
/// normalisation), `.y` = p, `.z` = c -- the picture, `.w` = spare.
pub static FINGERING: ModelDef = ModelDef {
    name: "fingering",
    display_name: "Viscous Fingering",
    description: "A thin fluid pushed into a thick one in a porous slab: the front is \
                  unstable and breaks into fingers that race ahead and shield their \
                  neighbours. Saffman–Taylor's instability, as a miscible displacement.",
    features: &[ModelFeature::NeedsMinMax, ModelFeature::NoTimeStep],
    parameters: &[
        SimParamDef {
            name: "log_mobility",
            display_name: "Log mobility ratio R",
            default: 2.0,
            min: -3.0,
            max: 5.0,
            tooltip: "ln of the viscosity ratio between the displaced fluid and the displacing \
                      one: mobility is exp(R × concentration), so R > 0 means the entering \
                      fluid is thinner. THIS IS THE INSTABILITY: positive and the front \
                      fingers, negative and it flattens, zero and it just advects. Larger \
                      makes fingers narrower and faster.",
            choices: &[],
        },
        SimParamDef {
            name: "speed",
            display_name: "Peak speed (cells/step)",
            default: 0.3,
            min: 0.02,
            max: 0.5,
            tooltip: "How far the fastest cell moves in one step. The flow is rescaled to \
                      this every step, which is a choice of time unit and also the Courant \
                      number of the advection — so it is capped at one half, where an upwind \
                      step stays stable.",
            choices: &[],
        },
        SimParamDef {
            name: "diffusion",
            display_name: "Diffusion",
            default: 0.02,
            min: 0.0,
            max: 0.2,
            tooltip: "Isotropic mixing of the two fluids, per step. It sets the smallest \
                      finger that can survive: less and the front breaks into many fine \
                      fingers, more and only a few broad ones grow. Capped at 0.2, where the \
                      explicit five-point step is still stable.",
            choices: &[],
        },
        SimParamDef {
            name: "disturbance",
            display_name: "Inlet disturbance ζ",
            default: 0.05,
            min: 0.0,
            max: 0.5,
            tooltip: "Random noise on the initial concentration next to the inlet, which the \
                      instability amplifies. Zero is a perfectly flat front, which stays flat: \
                      the fingers need something to grow from. Holzbecher uses 0.01 with a \
                      much finer mesh than a grid this size has.",
            choices: &[],
        },
        SimParamDef {
            name: "relax",
            display_name: "Pressure sweeps",
            default: 20.0,
            min: 1.0,
            max: 200.0,
            tooltip: "Jacobi iterations of the pressure equation per step, warm-started from \
                      the last step. Darcy flow is quasi-static, so the pressure has to be \
                      re-solved every time the concentration moves; too few sweeps and the \
                      flow lags the front.",
            choices: &[],
        },
    ],
    presets: &[
        SimPreset {
            name: "fingers",
            display_name: "Fingers",
            params: &[
                ("log_mobility", 2.0), ("speed", 0.3), ("diffusion", 0.02),
                ("disturbance", 0.05), ("relax", 20.0),
            ],
            // Measured at 256^2: 800 steps carries the front about 40% of
            // the way with the fingers' shielding well developed.
            steps: 800,
            init: Some(crate::config::sim::SimInit::Line),
            coloring: Some("channel"),
            coloring_params: &[("channel", 2.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
        SimPreset {
            name: "stable",
            display_name: "Stable (thick pushes thin)",
            // The same disturbance with the viscosities the other way
            // round: the front flattens instead.
            params: &[
                ("log_mobility", -2.0), ("speed", 0.3), ("diffusion", 0.02),
                ("disturbance", 0.05), ("relax", 20.0),
            ],
            steps: 600,
            init: Some(crate::config::sim::SimInit::Line),
            coloring: Some("channel"),
            coloring_params: &[("channel", 2.0), ("scale", 1.0), ("offset", 0.0), ("wrap", 0.0)],
            matte: None,
        },
    ],
    wgsl: r#"
// Mobility k/mu as a function of concentration, Holzbecher's (8).
fn fing_mobility(c: f32) -> f32 {
    return exp(mparam(0u) * clamp(c, 0.0, 1.0));
}

// The inlet is the LAST rows (where the Line init puts the entering
// fluid) and the outlet the first: the flow runs toward y = 0.
fn fing_inlet(p: vec2<i32>) -> bool {
    return p.y >= sim_grid().y - 1;
}
fn fing_outlet(p: vec2<i32>) -> bool {
    return p.y <= 0;
}

// Pass 1 of 2 -- one Jacobi sweep of the pressure equation
// div(m grad p) = 0, with the face mobility the harmonic mean of the
// two cells' (the finite-volume transmissibility). Repeated `relax`
// times per step. Dirichlet at inlet and outlet; the side walls are
// closed, which Clamp gives for free -- a ghost cell equal to the edge
// cell is a zero normal gradient.
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    var out = s;
    if (fing_inlet(p)) {
        out.y = 1.0;
        return out;
    }
    if (fing_outlet(p)) {
        out.y = 0.0;
        return out;
    }
    let m0 = fing_mobility(s.z);
    var num = 0.0;
    var den = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        var d = vec2<i32>(1, 0);
        if (i == 1) { d = vec2<i32>(-1, 0); }
        else if (i == 2) { d = vec2<i32>(0, 1); }
        else if (i == 3) { d = vec2<i32>(0, -1); }
        let q = sim_read(p + d);
        let m1 = fing_mobility(q.z);
        let t = 2.0 * m0 * m1 / max(m0 + m1, 1.0e-20);
        num = num + t * q.y;
        den = den + t;
    }
    out.y = num / max(den, 1.0e-20);
    return out;
}

// Pass 2 of 2 -- Darcy velocity from the pressure, then one upwind
// advection-diffusion step of the concentration.
fn sim_step2(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    var out = s;
    let l = sim_read(p + vec2<i32>(-1, 0));
    let r = sim_read(p + vec2<i32>( 1, 0));
    let d = sim_read(p + vec2<i32>( 0, -1));
    let u = sim_read(p + vec2<i32>( 0, 1));

    // u = -m grad p, central differences. Stored unscaled in .x so the
    // reduce finds this step's peak; USED scaled by last step's peak so
    // the fastest cell moves `speed` cells. Darcy flow is linear in
    // the pressure drop, so this is a time unit, not a change of flow.
    let m = fing_mobility(s.z);
    let vel = -m * 0.5 * vec2<f32>(r.y - l.y, u.y - d.y);
    out.x = length(vel);
    let peak = max(sim_minmax().y, 1.0e-12);
    let v = vel * (mparam(1u) / peak);

    if (fing_inlet(p)) {
        // The entering fluid.
        out.z = 1.0;
        return out;
    }

    // Upwind: each axis takes the difference from the cell the flow
    // comes from. Stable for a Courant number up to 1 per axis; the
    // rescaling above holds it at `speed` <= 0.5.
    let dcx = select(s.z - l.z, r.z - s.z, v.x < 0.0);
    let dcy = select(s.z - d.z, u.z - s.z, v.y < 0.0);
    let adv = v.x * dcx + v.y * dcy;
    let lap = l.z + r.z + d.z + u.z - 4.0 * s.z;
    out.z = clamp(s.z - adv + mparam(2u) * lap, 0.0, 1.0);
    return out;
}
"#,
    wgsl_seed: r#"
fn sim_seed(inside: f32, noise: f32, p: vec2<i32>) -> vec4<f32> {
    let g = vec2<f32>(sim_grid());
    // Pressure: the linear profile that already satisfies the boundary
    // conditions, Holzbecher's initial state.
    let pr = f32(p.y) / max(g.y - 1.0, 1.0);
    // Concentration: the init shape is the entering fluid (Line puts
    // it along the inlet), plus a random disturbance that decays away
    // from the inlet over a few cells -- his (12), with the penetration
    // length in cells rather than metres.
    let dist = g.y - 1.0 - f32(p.y);
    let bump = mparam(3u) * (noise - 0.5) * 2.0 * exp(-dist * dist / 16.0);
    let c = clamp(select(0.0, 1.0, inside >= 0.5) + bump, 0.0, 1.0);
    return vec4<f32>(0.0, pr, c, 0.0);
}
"#,
    default_steps: 800,
    passes: 2,
    repeat: Some((0, "relax")),
    agents: None,
    kernel: None,
    dt_bound: None,
    diffusion: &[],
    max_dt: 1.0,
    default_dt: 1.0,
};
