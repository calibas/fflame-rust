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
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let q = floor(mparam(0u));
    let k1 = max(floor(mparam(1u)), 1.0);
    let k2 = max(floor(mparam(2u)), 1.0);
    let g = floor(mparam(3u));

    var infected = 0.0;
    var ill = 0.0;
    var total = s.x;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            if (dx == 0 && dy == 0) {
                continue;
            }
            let n = sim_read(p + vec2<i32>(dx, dy)).x;
            total = total + n;
            if (n >= q) {
                ill = ill + 1.0;
            } else if (n > 0.0) {
                infected = infected + 1.0;
            }
        }
    }

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
fn sim_step(s: vec4<f32>, p: vec2<i32>) -> vec4<f32> {
    let n_states = max(floor(mparam(0u)), 2.0);
    let r = i32(clamp(floor(mparam(1u)), 1.0, 5.0));
    let thresh = max(floor(mparam(2u)), 1.0);
    let moore = mparam(3u) >= 0.5;

    let cur = floor(s.x);
    let want = fract_state(cur + 1.0, n_states);

    var count = 0.0;
    for (var dy = -r; dy <= r; dy = dy + 1) {
        for (var dx = -r; dx <= r; dx = dx + 1) {
            if (dx == 0 && dy == 0) {
                continue;
            }
            if (!moore && abs(dx) + abs(dy) > r) {
                continue;
            }
            if (floor(sim_read(p + vec2<i32>(dx, dy)).x) == want) {
                count = count + 1.0;
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
    max_dt: 1.0,
    default_dt: 1.0,
};
