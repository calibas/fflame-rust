// The Schnakenberg model, the simplest two-species Turing system.
//
// The Rhai twin of the built-in `schnakenberg` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
//   du/dt = D_u lap(u) + a - u + u^2 v
//   dv/dt = D_v lap(v) + b - u^2 v
//
// Channels: x = u, y = v, z = the step u last changed appreciably,
// w spare.

const PARAMS = #{
    a: #{
        value: 0.1, min: 0.0, max: 1.0, caption: "a",
        hint: "Supply of u. With b it fixes the uniform state at u = a + b.",
    },
    b: #{
        value: 0.9, min: 0.0, max: 2.0, caption: "b",
        hint: "Supply of v. The a:b ratio decides whether spots or gaps form.",
    },
    diffusion_u: #{
        value: 1.0, min: 0.0, max: 4.0, caption: "Diffusion u",
        hint: "Spread of the activator.",
    },
    diffusion_v: #{
        value: 40.0, min: 0.0, max: 80.0, caption: "Diffusion v",
        hint: "Spread of the inhibitor; about 40x the activator's for spots. Scaling BOTH rates together grows the features — measured, 16x the rates gives 4.6x the wavelength, and costs proportionally more steps.",
    },
};
const INFO = #{
    title: "Schnakenberg",
    id: "schnakenberg",
    description: "Minimal Turing system: spots at a large diffusion ratio. Feature size scales as the square root of the diffusion rates.",
    dt: 0.01,
    max_dt: 0.02,
    steps: 4900,
    diffusion: ["diffusion_u", "diffusion_v"],
    presets: [
        #{
            name: "turing_spots",
            title: "Turing spots",
            params: #{ a: 0.1, b: 0.9, diffusion_u: 1.0, diffusion_v: 40.0 },
            steps: 4900,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 0.02 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.0, wrap: 0.0 },
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
    let lap_u = -cell.x + 0.2 * (up.x + dn.x + lf.x + rt.x) + 0.05 * (ul.x + ur.x + dl.x + dr.x);
    let lap_v = -cell.y + 0.2 * (up.y + dn.y + lf.y + rt.y) + 0.05 * (ul.y + ur.y + dl.y + dr.y);
    let dt = ctx.dt;
    let u = cell.x;
    let v = cell.y;
    let uuv = u * u * v;
    let nu = max(u + dt * (p.diffusion_u * lap_u + p.a - u + uuv), 0.0);
    let nv = max(v + dt * (p.diffusion_v * lap_v + p.b - uuv), 0.0);
    let age = if abs(nu - u) > 0.0001 { ctx.step } else { cell.z };
    vec4(nu, nv, age, 0.0)
}

// The fixed point (a + b, b / (a + b)^2) and two independent small
// draws, as the Brusselator's.
fn cell_seed(inside, noise, p) {
    let u0 = p.a + p.b;
    let v0 = p.b / max(u0 * u0, 0.001);
    let n0 = (rand(0x81) - 0.5) * 0.04;
    let n1 = (rand(0x82) - 0.5) * 0.04;
    vec4(max(u0 + n0, 0.0), max(v0 + n1, 0.0), 0.0, 0.0)
}
