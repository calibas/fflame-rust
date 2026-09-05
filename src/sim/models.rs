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
            init: None,
        },
        SimPreset {
            name: "coral",
            display_name: "Coral",
            params: &[("feed", 0.0545), ("kill", 0.062)],
            steps: 10000,
            init: None,
        },
        SimPreset {
            name: "maze",
            display_name: "Maze",
            params: &[("feed", 0.030), ("kill", 0.057)],
            steps: 10000,
            init: None,
        },
        SimPreset {
            name: "worms",
            display_name: "Worms",
            params: &[("feed", 0.046), ("kill", 0.065)],
            steps: 10000,
            init: None,
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

/// The hodgepodge machine (Gerhardt-Schuster), a Belousov-Zhabotinsky
/// cellular automaton.
///
/// States 0..q: 0 healthy, q ill, everything between infected. With A
/// the count of infected neighbours, B the count of ill ones and S the
/// sum of the cell and its neighbours,
///
/// ```text
/// healthy:   s' = floor(A/k1) + floor(B/k2)
/// infected:  s' = floor(S/(A+B+1)) + g
/// ill:       s' = 0
/// ```
///
/// capped at q. Measured on the CPU prototype: the secondary-source
/// parameters q=200, k1=2, k2=3, g=70 give the BZ spiral field, but
/// NOT by step 50 -- at 50 it is one state with scattered specks, and
/// it is fully spiralled by 200.
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
            display_name: "k₁ (infection)",
            default: 2.0,
            min: 1.0,
            max: 16.0,
            tooltip: "Divides the infected-neighbour count when a healthy cell is infected. \
                      Larger values make infection harder to catch.",
            choices: &[],
        },
        SimParamDef {
            name: "k2",
            display_name: "k₂ (illness)",
            default: 3.0,
            min: 1.0,
            max: 16.0,
            tooltip: "Divides the ill-neighbour count. With k₁ it sets how readily waves \
                      nucleate.",
            choices: &[],
        },
        SimParamDef {
            name: "g",
            display_name: "g (rate)",
            default: 70.0,
            min: 1.0,
            max: 200.0,
            tooltip: "How fast an infected cell progresses toward ill. Sets the wave speed \
                      and therefore the spiral pitch.",
            choices: &[],
        },
    ],
    presets: &[SimPreset {
        name: "spirals",
        display_name: "BZ spirals",
        params: &[("states", 200.0), ("k1", 2.0), ("k2", 3.0), ("g", 70.0)],
        // Fully spiralled by 200; at 50 it is still one state with
        // specks, which an earlier note claimed was "developed".
        steps: 200,
        init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
    }],
    wgsl: r#"
fn hp_count(c: bool) -> f32 {
    return select(0.0, 1.0, c);
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

    let cur = s.x;
    var next = 0.0;
    if (cur >= q) {
        // Ill cells recover completely.
        next = 0.0;
    } else if (cur <= 0.0) {
        next = floor(infected / k1) + floor(ill / k2);
    } else {
        next = floor(total / (infected + ill + 1.0)) + g;
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
        },
        SimPreset {
            name: "coarsening",
            display_name: "Coarsening",
            params: &[("temperature", 1.5), ("field", 0.0), ("coupling", 1.0)],
            // Measured to need ~436 sweeps before it looks like anything.
            steps: 1000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
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
        },
        SimPreset {
            name: "random_deposition",
            display_name: "Random deposition",
            params: &[("p_drop", 0.5), ("sideways", 0.0)],
            steps: 450,
            init: Some(crate::config::sim::SimInit::Center),
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
        },
        SimPreset {
            name: "rule_30",
            display_name: "Rule 30 (chaotic)",
            params: &[("rule", 30.0)],
            steps: 256,
            init: Some(crate::config::sim::SimInit::Center),
        },
        SimPreset {
            name: "rule_110",
            display_name: "Rule 110 (universal)",
            params: &[("rule", 110.0)],
            steps: 256,
            init: Some(crate::config::sim::SimInit::Center),
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
        },
        SimPreset {
            name: "rule_13",
            display_name: "Rule 13",
            params: &[("rule_mask", 10.0)],
            steps: 125,
            init: Some(crate::config::sim::SimInit::Center),
        },
        SimPreset {
            name: "rule_134",
            display_name: "Rule 134",
            params: &[("rule_mask", 26.0)],
            steps: 125,
            init: Some(crate::config::sim::SimInit::Center),
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
        },
        SimPreset {
            name: "subcritical",
            display_name: "Below threshold",
            params: &[("p_open", 0.45)],
            steps: 400,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
        },
        SimPreset {
            name: "supercritical",
            display_name: "Above threshold",
            params: &[("p_open", 0.75)],
            steps: 400,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
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
        },
        SimPreset {
            name: "spots",
            display_name: "Spots",
            params: &[("wavelength", 16.0), ("drive", 2.0), ("asymmetry", 0.25)],
            // Measured: settles at 5,900; skew -1.02 against the
            // labyrinth's +0.00, which is the asymmetry showing up.
            steps: 6000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
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
    // Explicit Euler on the 5-point Laplacian, whose symbol runs over
    // [-8, 0]: the quartic operator is largest at the checkerboard,
    // (8 - q0^2)^2, and the drive r offsets it. Measured at
    // lambda = 16: stable at 0.03249, diverges at 0.03574, and this
    // formula gives 0.0325.
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
        },
        SimPreset {
            name: "droplets",
            display_name: "Droplets",
            params: &[("mobility", 1.0), ("gamma", 0.5), ("mean", 0.4)],
            steps: 20000,
            init: Some(crate::config::sim::SimInit::Noise { amplitude: 1.0 }),
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
    dt_bound: Some(|p| {
        let d = p.get("mobility").max(1.0e-3);
        let gamma = p.get("gamma").max(0.0);
        2.0 / (d * (16.0 + 64.0 * gamma))
    }),
    diffusion: &[],
    max_dt: 0.2,
    default_dt: 0.04,
};
