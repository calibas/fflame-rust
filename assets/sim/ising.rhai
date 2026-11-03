// The Ising model by Metropolis sampling: spins on a lattice at a
// temperature.
//
// The Rhai twin of the built-in `ising` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Channels: x = the spin (-1 or 1), z = the step it last flipped, y and
// w spare.

const PARAMS = #{
    temperature: #{
        value: 2.269, min: 0.2, max: 5.0, caption: "Temperature",
        hint: "Onsager's critical temperature is 2.269. Below it domains coarsen; at it the clusters are fractal at every scale; above it the lattice is uncorrelated noise.",
    },
    field: #{
        value: 0.0, min: -1.0, max: 1.0, caption: "External field (H)",
        hint: "Biases one spin direction. Any nonzero value eventually magnetises the whole lattice.",
    },
    coupling: #{
        value: 1.0, min: -2.0, max: 2.0, caption: "Coupling (J)",
        hint: "Positive couples neighbours (ferromagnetic); negative anti-couples them and gives a checkerboard ground state.",
    },
};
const INFO = #{
    title: "Ising",
    id: "ising",
    description: "Spins on a lattice at temperature T. Domains coarsen below the critical point, are fractal at it, and are noise above it.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 1200,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "critical",
            title: "Critical (T_c)",
            params: #{ temperature: 2.269, field: 0.0, coupling: 1.0 },
            steps: 1200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
        #{
            name: "coarsening",
            title: "Coarsening",
            params: #{ temperature: 1.5, field: 0.0, coupling: 1.0 },
            steps: 1000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    // A checkerboard: one sublattice moves per step, so every cell this
    // pass considers has neighbours standing still. A synchronous
    // Metropolis update breaks detailed balance and gives the wrong
    // equilibrium.
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    if ((x + y) & 1) != ctx.step % 2 {
        return cell;
    }
    let temp = max(p.temperature, 0.001);
    let spin = cell.x;
    let neighbours = read(0, -1).x + read(0, 1).x + read(-1, 0).x + read(1, 0).x;
    let de = 2.0 * spin * (p.coupling * neighbours + p.field);
    // Clamped before exp: at a low temperature de / temp reaches the
    // hundreds.
    let accept = de <= 0.0 || rand(0xc1) < exp(-clamp(de / temp, 0.0, 60.0));
    vec4(if accept { -spin } else { spin }, 0.0, if accept { ctx.step } else { cell.z }, 0.0)
}

// Every spin at random.
fn cell_seed(inside, noise) {
    vec4(if rand(0xc0) < 0.5 { 1.0 } else { -1.0 }, 0.0, 0.0, 0.0)
}
