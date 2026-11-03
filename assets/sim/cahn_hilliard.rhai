// The Cahn-Hilliard equation: a mixture separating into two phases,
// conserving its composition, the domains coarsening.
//
// The Rhai twin of the built-in `cahn_hilliard` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its stable time step, and the same field.
//
//   dc/dt = D lap(mu),   mu = c^3 - c - gamma lap(c)
//
// Two passes: the first writes the chemical potential mu into `.y`, the
// second steps c by its Laplacian. Channels: x = c, y = mu, z = the step
// c last changed appreciably, w spare.

// The 5-point Laplacian of `.x` or `.y`, the cell's value `c`.

const PARAMS = #{
    mobility: #{
        value: 1.0, min: 0.1, max: 4.0, caption: "Mobility (D)",
        hint: "How fast material moves down the chemical-potential gradient. It scales time — but it scales the stable dt down by exactly as much, so raising it buys no speed.",
    },
    gamma: #{
        value: 0.5, min: 0.1, max: 4.0, caption: "γ (interface width)",
        hint: "Sets the width of the boundary between the phases, which goes as √γ. Larger values give thicker, softer interfaces and a coarser pattern.",
    },
    mean: #{
        value: 0.0, min: -0.6, max: 0.6, caption: "Mean composition",
        hint: "The mixture's overall balance, which the equation conserves exactly. At 0 the phases are even and interleave as a labyrinth; past about ±0.3 the minority phase pinches off into droplets.",
    },
};
const INFO = #{
    title: "Cahn–Hilliard",
    id: "cahn_hilliard",
    description: "A mixture separating into two phases and then coarsening: labyrinths at an even mix, droplets of the minority phase away from it.",
    dt: 0.04,
    max_dt: 0.2,
    steps: 20000,
    presets: [
        #{
            name: "labyrinth",
            title: "Labyrinth",
            params: #{ mobility: 1.0, gamma: 0.5, mean: 0.0 },
            steps: 20000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
        #{
            name: "droplets",
            title: "Droplets",
            params: #{ mobility: 1.0, gamma: 0.5, mean: 0.4 },
            steps: 20000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
    ],
};

fn lap_x(c) {
    read(0, -1).x + read(0, 1).x + read(-1, 0).x + read(1, 0).x - 4.0 * c
}

fn lap_y(c) {
    read(0, -1).y + read(0, 1).y + read(-1, 0).y + read(1, 0).y - 4.0 * c
}

fn cell_step(cell, p, ctx) {
    let c = cell.x;
    let mu = c * c * c - c - p.gamma * lap_x(c);
    vec4(c, mu, cell.z, 0.0)
}

fn cell_step2(cell, p, ctx) {
    let c = cell.x;
    let nc = c + ctx.dt * p.mobility * lap_y(cell.y);
    let cl = clamp(nc, -4.0, 4.0);
    vec4(cl, cell.y, if abs(cl - c) > 0.00001 { ctx.step } else { cell.z }, 0.0)
}

// The mean composition, and small noise about it.
fn cell_seed(inside, noise, p) {
    vec4(p.mean + (noise - 0.5) * 0.1, 0.0, 0.0, 0.0)
}

// Explicit Euler on the fourth-order operator: the Nyquist mode's
// eigenvalues, 8 for the Laplacian and 64 for its square.
fn max_dt(p) {
    let d = max(p.mobility, 0.001);
    let gamma = max(p.gamma, 0.0);
    2.0 / (d * (16.0 + 64.0 * gamma))
}
