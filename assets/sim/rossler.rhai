// A Rössler oscillator in every cell, coupled by diffusion: chemical
// chaos as a medium (spiral waves whose period follows the attractor).
//
// The Rhai twin of the built-in `rossler` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its stable time step, and the same field.
//
//   du/dt = D_uv lap(u) - v - w
//   dv/dt = D_uv lap(v) + u + a v
//   dw/dt = D_w  lap(w) + b + w (u - c)
//
// Channels: x = u, y = v, z = w, w = the envelope of u (its running
// maximum, forgotten at `forget` a unit of time).

const PARAMS = #{
    a: #{
        value: 0.2, min: 0.05, max: 0.5, caption: "a",
        hint: "Rössler's a. The paper's 0.2.",
    },
    b: #{
        value: 0.2, min: 0.05, max: 2.0, caption: "b",
        hint: "Rössler's b. The paper's 0.2.",
    },
    c: #{
        value: 4.5, min: 2.0, max: 8.0, caption: "c",
        hint: "Rössler's c: the route to chaos. With a = b = 0.2, period one near 2.3, period doubling past 3, chaos by the paper's 4.5.",
    },
    duv: #{
        value: 0.017, min: 0.0, max: 0.1, caption: "D_u = D_v",
        hint: "Diffusion of u and v, in the paper's units (domain length 10). Its Fig. 3 runs 0.003 to 0.048; above the Fig. 1(a) threshold the lattice synchronises and there is no pattern.",
    },
    dw: #{
        value: 1.25, min: 0.0, max: 3.0, caption: "D_w",
        hint: "Diffusion of w, in the paper's units. Its Fig. 3 runs 0.25 to 2.5. The largest diffusion sets the time step through the spacing.",
    },
    spacing: #{
        value: 0.25, min: 0.05, max: 1.0, caption: "Cell spacing",
        hint: "Physical size of a cell: the paper's 40 cells for a domain of 10 is 0.25. Diffusion in cells is D / spacing², so a coarser spacing allows a larger time step and a shorter run, at the cost of the paper's resolution.",
    },
    forget: #{
        value: 0.0, min: 0.0, max: 1.0, caption: "Envelope forgetting",
        hint: "How fast the time-maximum in the fourth channel decays, per unit time. 0 is the paper's lifetime maximum; a small value makes it an envelope of the recent past that keeps following the pattern.",
    },
};
const INFO = #{
    title: "Rössler Lattice",
    id: "rossler",
    description: "Chaotic Rössler oscillators coupled by diffusion. The snapshots are chaos; the time-maximum of u, kept in the fourth channel, is a still, ordered pattern — Xiao et al.'s chaotic Turing patterns.",
    dt: 0.0018,
    max_dt: 0.002,
    steps: 2000000,
    presets: [
        #{
            name: "asymmetric",
            title: "Asymmetric",
            params: #{ a: 0.2, b: 0.2, c: 4.5, duv: 0.017, dw: 1.25, spacing: 0.25, forget: 0.0 },
            steps: 2000000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 0.2961, offset: -3.1622, wrap: 0.0 },
        },
        #{
            name: "diagonal_mirror",
            title: "Diagonal mirror",
            params: #{ a: 0.2, b: 0.2, c: 4.5, duv: 0.015, dw: 0.65, spacing: 0.25, forget: 0.0 },
            steps: 2000000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 0.2616, offset: -2.302, wrap: 0.0 },
        },
        #{
            name: "translational",
            title: "Translational",
            params: #{ a: 0.2, b: 0.2, c: 4.5, duv: 0.023, dw: 2.5, spacing: 0.25, forget: 0.0 },
            steps: 2000000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 0.1271, offset: -1.0928, wrap: 0.0 },
        },
        #{
            name: "carpet",
            title: "Carpet",
            params: #{ a: 0.2, b: 0.2, c: 4.5, duv: 0.003, dw: 2.5, spacing: 0.25, forget: 0.0 },
            steps: 2000000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 0.0315, offset: -0.3621, wrap: 0.0 },
        },
        #{
            name: "architecture",
            title: "Architecture",
            params: #{ a: 0.2, b: 0.2, c: 4.5, duv: 0.01, dw: 2.5, spacing: 0.25, forget: 0.0 },
            steps: 2000000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 0.0655, offset: -0.5557, wrap: 0.0 },
        },
        #{
            name: "square",
            title: "Square",
            params: #{ a: 0.2, b: 0.2, c: 4.5, duv: 0.032, dw: 2.5, spacing: 0.25, forget: 0.0 },
            steps: 2000000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 0.1471, offset: -1.2853, wrap: 0.0 },
        },
        #{
            name: "conventional",
            title: "Conventional",
            params: #{ a: 0.2, b: 0.2, c: 4.5, duv: 0.03, dw: 2.5, spacing: 0.25, forget: 0.0 },
            steps: 2000000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 0.1385, offset: -1.2031, wrap: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    let up = read(0, -1);
    let dn = read(0, 1);
    let lf = read(-1, 0);
    let rt = read(1, 0);
    let lap_u = up.x + dn.x + lf.x + rt.x - 4.0 * cell.x;
    let lap_v = up.y + dn.y + lf.y + rt.y - 4.0 * cell.y;
    let lap_w = up.z + dn.z + lf.z + rt.z - 4.0 * cell.z;
    // The grid's spacing scales the diffusion.
    let h = max(p.spacing, 0.001);
    let d_uv = p.duv / (h * h);
    let d_w = p.dw / (h * h);
    let dt = ctx.dt;
    let u = cell.x;
    let v = cell.y;
    let w = cell.z;
    let nu = u + dt * (d_uv * lap_u + (-v - w));
    let nv = v + dt * (d_uv * lap_v + (u + p.a * v));
    let nw = w + dt * (d_w * lap_w + (p.b + w * (u - p.c)));
    vec4(nu, nv, nw, max(nu, cell.w - p.forget * dt))
}

// A small displacement inside the init shape, the same in every channel.
fn cell_seed(inside, noise) {
    let delta = rand(0xa1) * 0.2 * inside;
    vec4(delta, delta, delta, delta)
}

// Explicit Euler on the 5-point Laplacian at spacing h: dt D / h^2 < 1/4,
// and never past 0.002, where the oscillator itself goes unstable.
fn max_dt(p) {
    let h = max(p.spacing, 0.001);
    let dmax = max(max(p.duv, p.dw), 0.000001) / (h * h);
    min(0.25 / dmax, 0.002)
}
