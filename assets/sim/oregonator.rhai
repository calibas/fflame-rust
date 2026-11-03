// The Oregonator, Field and Noyes' model of the Belousov-Zhabotinsky
// reaction, in Tyson and Fife's two-variable form.
//
// The Rhai twin of the built-in `oregonator` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its stable time step, and the same field.
//
//   eps du/dt = D_u lap(u) + u (1 - u) - f v (u - q) / (u + q)
//       dv/dt = D_v lap(v) + u - v
//
// Channels: x = u (bromous acid), y = v (the catalyst), z = the step the
// cell last fired, w spare.

const PARAMS = #{
    epsilon: #{
        value: 0.04, min: 0.005, max: 0.3, caption: "ε (timescale)",
        hint: "How much faster the activator moves than the catalyst. Small values give sharp wave fronts and a stiffer solve — the stable time step is proportional to this.",
    },
    q: #{
        value: 0.002, min: 0.0002, max: 0.05, caption: "q",
        hint: "The reaction's small parameter. It sets the activator's threshold, and the stable time step is proportional to it.",
    },
    f: #{
        value: 1.4, min: 0.4, max: 3.5, caption: "f (stoichiometry)",
        hint: "Selects the regime. Around 1 the medium oscillates on its own and fills with travelling wave trains; above about 2.5 it is merely excitable, so a seed fires one wave and the field goes quiet.",
    },
    diffusion_u: #{
        value: 1.0, min: 0.1, max: 2.0, caption: "Diffusion u",
        hint: "Activator diffusion, which sets the wave speed.",
    },
    diffusion_v: #{
        value: 0.6, min: 0.0, max: 2.0, caption: "Diffusion v",
        hint: "Catalyst diffusion. The real catalyst is a large ion and barely diffuses; zero is a defensible setting.",
    },
};
const INFO = #{
    title: "Oregonator",
    id: "oregonator",
    description: "The Belousov–Zhabotinskii reaction as chemistry rather than as an automaton: an oscillating medium that carries travelling wave trains.",
    dt: 0.0001,
    max_dt: 0.01,
    steps: 15000,
    settles: false,
    presets: [
        #{
            name: "waves",
            title: "Excitation waves",
            params: #{ epsilon: 0.04, q: 0.002, f: 1.4, diffusion_u: 1.0, diffusion_v: 0.6 },
            steps: 15000,
            boundary: "periodic",
            init: #{ blobs: #{ count: 5, radius: 5 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.2, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    let eps = max(p.epsilon, 0.0001);
    let q = max(p.q, 0.000001);
    let dt = ctx.dt;
    let up = read(0, -1);
    let dn = read(0, 1);
    let lf = read(-1, 0);
    let rt = read(1, 0);
    let ul = read(-1, -1);
    let ur = read(1, -1);
    let dl = read(-1, 1);
    let dr = read(1, 1);
    let lap_u = -cell.x + 0.2 * (up.x + dn.x + lf.x + rt.x) + 0.05 * (ul.x + ur.x + dl.x + dr.x);
    let lap_v = -cell.y + 0.2 * (up.y + dn.y + lf.y + rt.y) + 0.05 * (ul.y + ur.y + dl.y + dr.y);
    let u = cell.x;
    let v = cell.y;
    let react = u * (1.0 - u) - p.f * v * (u - q) / (u + q);
    let nu = max(u + dt * (p.diffusion_u * lap_u + react) / eps, 0.0);
    let nv = max(v + dt * (p.diffusion_v * lap_v + u - v), 0.0);
    // The step a cell last fired: u crossing 0.3 upward.
    let age = if u <= 0.3 && nu > 0.3 { ctx.step } else { cell.z };
    vec4(nu, nv, age, 0.0)
}

// Excited inside the init shape.
fn cell_seed(inside, noise) {
    vec4(if inside >= 0.5 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0)
}

// Explicit Euler on the stiff u equation: its reaction's slope at u = q
// is about f / (2 q), its diffusion's 1.6 D_u, both divided by eps.
fn max_dt(p) {
    let eps = max(p.epsilon, 0.0001);
    let q = max(p.q, 0.000001);
    2.0 * eps / (p.f / (2.0 * q) + 1.6 * p.diffusion_u)
}
