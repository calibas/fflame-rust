// The Brusselator, Prigogine and Lefever's autocatalytic oscillator.
//
// The Rhai twin of the built-in `brusselator` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
//   dX/dt = D_X lap(X) + A - (B + 1) X + X^2 Y
//   dY/dt = D_Y lap(Y) + B X - X^2 Y
//
// Channels: x = X, y = Y, z = the step X last changed appreciably,
// w spare.

const PARAMS = #{
    feed_a: #{
        value: 1.0, min: 0.1, max: 3.0, caption: "A",
        hint: "Constant supply of X. The fixed point sits at X = A.",
    },
    feed_b: #{
        value: 3.0, min: 0.1, max: 20.0, caption: "B",
        hint: "Drives the instability. Above 1 + A² the uniform state oscillates; with unequal diffusion it forms stationary spots instead.",
    },
    diffusion_x: #{
        value: 1.0, min: 0.0, max: 100.0, caption: "Diffusion X",
        hint: "Spread of the activator.",
    },
    diffusion_y: #{
        value: 8.0, min: 0.0, max: 400.0, caption: "Diffusion Y",
        hint: "Spread of the inhibitor. It must exceed X's for a Turing pattern; equal rates give bulk oscillation and no structure.",
    },
    stencil: #{
        value: 0, choices: ["9-point (Sims)", "5-point"], caption: "Laplacian",
        hint: "The 9-point Sims stencil (this model's own) or the plain 5-point one the coupled-layer papers use. Two Brusselator layers on the 5-point stencil under a cubic coupling are the two-layer Brusselator model.",
    },
};
const INFO = #{
    title: "Brusselator",
    id: "brusselator",
    description: "The textbook Turing system: stationary spots when Y diffuses faster than X, bulk oscillation when it does not.",
    dt: 0.01,
    max_dt: 0.04,
    steps: 1180,
    diffusion: ["diffusion_x", "diffusion_y"],
    presets: [
        #{
            name: "turing_spots",
            title: "Turing spots",
            params: #{ feed_a: 1.0, feed_b: 3.0, diffusion_x: 1.0, diffusion_y: 8.0 },
            steps: 1180,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 0.05 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.3, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "oscillating",
            title: "Oscillating",
            params: #{ feed_a: 1.0, feed_b: 2.5, diffusion_x: 1.0, diffusion_y: 1.0 },
            steps: 2000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 0.05 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.3, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    let up = read(0, -1);
    let dn = read(0, 1);
    let lf = read(-1, 0);
    let rt = read(1, 0);
    let ul = read(-1, -1);
    let ur = read(1, -1);
    let dl = read(-1, 1);
    let dr = read(1, 1);
    // The 9-point Sims stencil, or the 5-point one.
    let lap_x = -cell.x + 0.2 * (up.x + dn.x + lf.x + rt.x) + 0.05 * (ul.x + ur.x + dl.x + dr.x);
    let lap_y = -cell.y + 0.2 * (up.y + dn.y + lf.y + rt.y) + 0.05 * (ul.y + ur.y + dl.y + dr.y);
    if p.stencil == 1 {
        lap_x = up.x + dn.x + lf.x + rt.x - 4.0 * cell.x;
        lap_y = up.y + dn.y + lf.y + rt.y - 4.0 * cell.y;
    }
    let dt = ctx.dt;
    let x = cell.x;
    let y = cell.y;
    let xxy = x * x * y;
    let nx = max(x + dt * (p.diffusion_x * lap_x + p.feed_a - (p.feed_b + 1.0) * x + xxy), 0.0);
    let ny = max(y + dt * (p.diffusion_y * lap_y + p.feed_b * x - xxy), 0.0);
    let age = if abs(nx - x) > 0.0001 { ctx.step } else { cell.z };
    vec4(nx, ny, age, 0.0)
}

// The fixed point (A, B/A) and two independent small draws: the
// instability grows from any asymmetry, and starting exactly on the
// fixed point is a stationary solution that never leaves it.
fn cell_seed(inside, noise, p) {
    let n0 = (rand(0x71) - 0.5) * 0.1;
    let n1 = (rand(0x72) - 0.5) * 0.1;
    vec4(max(p.feed_a + n0, 0.0), max(p.feed_b / max(p.feed_a, 0.001) + n1, 0.0), 0.0, 0.0)
}
