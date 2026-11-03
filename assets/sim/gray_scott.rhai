// Gray–Scott reaction–diffusion, in Karl Sims' discretisation.
//
// The Rhai twin of the built-in `gray_scott` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, in the same
// order, the same working values, and the same field.
//
//   dA/dt = D_A lap(A) - A B^2 + F (1 - A)
//   dB/dt = D_B lap(B) + A B^2 - (F + k) B
//
// Channels: x = A, y = B, z = the step the cell last changed
// appreciably (for the age colouring), w spare.

const PARAMS = #{
    feed: #{
        value: 0.0545, min: 0.0, max: 0.11, caption: "Feed (F)",
        hint: "Rate at which A is replenished. With kill, selects the pattern class: the interesting band is roughly 0.01–0.07.",
    },
    kill: #{
        value: 0.062, min: 0.0, max: 0.073, caption: "Kill (k)",
        hint: "Rate at which B is removed. Small changes cross between spots, worms and mazes; most of the plane outside 0.04–0.07 decays to empty.",
    },
    diffusion_a: #{
        value: 1.0, min: 0.0, max: 1.0, caption: "Diffusion A",
        hint: "How fast A spreads. 1.0 is the standard scheme; the explicit solver is unstable above it at dt = 1.",
    },
    diffusion_b: #{
        value: 0.5, min: 0.0, max: 1.0, caption: "Diffusion B",
        hint: "How fast B spreads. The A:B ratio sets the feature size; equal rates give no pattern at all.",
    },
};
const INFO = #{
    title: "Gray–Scott",
    id: "gray_scott",
    description: "Two-species reaction–diffusion. Pearson's (F, k) plane holds spots, worms, mazes and mitosis — the classic reaction–diffusion patterns.",
    dt: 1.0,
    // Explicit Euler on the Sims stencil is stable while dt * D * 1.6 < 2:
    // with D_A = 1, dt < 1.25.
    max_dt: 1.25,
    steps: 10000,
    diffusion: ["diffusion_a", "diffusion_b"],
};

fn cell_step(cell, p, ctx) {
    // Karl Sims' 3x3 weights: centre -1, edge 0.2, corner 0.05.
    let up = read(0, -1);
    let dn = read(0, 1);
    let lf = read(-1, 0);
    let rt = read(1, 0);
    let ul = read(-1, -1);
    let ur = read(1, -1);
    let dl = read(-1, 1);
    let dr = read(1, 1);
    let lap_a = -cell.x + 0.2 * (up.x + dn.x + lf.x + rt.x) + 0.05 * (ul.x + ur.x + dl.x + dr.x);
    let lap_b = -cell.y + 0.2 * (up.y + dn.y + lf.y + rt.y) + 0.05 * (ul.y + ur.y + dl.y + dr.y);
    let a = cell.x;
    let b = cell.y;
    let abb = a * b * b;
    // Clamped to the physical range: an overshoot below zero would feed
    // b * b with the wrong sign and reach NaN.
    let na = clamp(a + (p.diffusion_a * lap_a - abb + p.feed * (1.0 - a)) * ctx.dt, 0.0, 1.0);
    let nb = clamp(b + (p.diffusion_b * lap_b + abb - (p.kill + p.feed) * b) * ctx.dt, 0.0, 1.0);
    // The step the cell last changed appreciably: held, not counted.
    let age = if abs(nb - b) > 0.0001 { ctx.step } else { cell.z };
    vec4(na, nb, age, 0.0)
}

// A = 1 everywhere, B = 1 inside the init shape.
fn cell_seed(inside, noise) {
    vec4(1.0, inside, 0.0, 0.0)
}
