// Kobayashi's phase-field model of dendritic solidification (1993): an
// anisotropic interface growing into an undercooled melt.
//
// The Rhai twin of the built-in `kobayashi` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its stable time step, and the same field.
//
// Two passes: the first writes the anisotropic flux's x and y faces
// into `.z` and `.w`, the second steps the phase by their divergence and
// the temperature by its Laplacian and the latent heat. The paper's
// constants: a mesh of 0.03, eps-bar 0.01, tau 0.0003, alpha 0.9,
// gamma 10, the melting point 1, noise 0.01. Channels: x = the phase
// (1 solid), y = the temperature, z and w = the flux.

// The anisotropy's fold: 2, 4, 6 or 8.

const PARAMS = #{
    latent_heat: #{
        value: 1.6, min: 0.5, max: 2.5, caption: "Latent heat (K)",
        hint: "How much heat solidification releases, which is what stops the crystal. Below 1 the whole vessel freezes; above it roughly 1/K of the region solidifies and the arms stay slender.",
    },
    delta: #{
        value: 0.04, min: 0.0, max: 0.08, caption: "Anisotropy (δ)",
        hint: "Strength of the directional preference. At 0 the growth is isotropic and splits like viscous fingering; the paper's ice dendrites use 0.040 and its four-fold ones 0.020.",
    },
    mode: #{
        value: 2, choices: ["2-fold", "4-fold", "6-fold", "8-fold"], caption: "Symmetry (j)",
        hint: "How many directions the crystal prefers. Six-fold is the snowflake; four-fold is the metallic dendrite of the paper's figure 7.",
    },
    theta0: #{
        value: 1.5708, min: 0.0, max: 6.2832, caption: "Orientation (θ₀)",
        hint: "Rotates the preferred directions. The paper's ice dendrites use π/2 and its four-fold ones 0.",
    },
};
const INFO = #{
    title: "Kobayashi Dendrite",
    id: "kobayashi",
    description: "Phase-field solidification: a crystal grown into an undercooled melt, with the anisotropy that turns a blob into a snowflake.",
    dt: 0.0001,
    max_dt: 0.001,
    steps: 4000,
    presets: [
        #{
            name: "ice",
            title: "Ice dendrite (six-fold)",
            params: #{ latent_heat: 1.6, delta: 0.04, mode: 2.0, theta0: 1.5708 },
            steps: 4000,
            boundary: "clamp",
            init: #{ blob: #{ radius: 4 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "metallic",
            title: "Metallic dendrite (four-fold)",
            params: #{ latent_heat: 2.0, delta: 0.02, mode: 1.0, theta0: 0.0 },
            steps: 4000,
            boundary: "clamp",
            init: #{ blob: #{ radius: 4 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn kob_mode(mode) {
    2.0 + 2.0 * to_float(mode)
}

// eps(theta) and its derivative at the gradient (gx, gy), as a complex.
fn kob_eps(gx, gy, mode, delta, theta0) {
    if gx * gx + gy * gy < 0.0000000000000001 {
        complex(0.01, 0.0)
    } else {
        let j = kob_mode(mode);
        let a = j * (atan2(gy, gx) - theta0);
        complex(0.01 * (1.0 + delta * cos(a)), -0.01 * delta * j * sin(a))
    }
}

fn cell_step(cell, p, ctx) {
    let inv = 1.0 / 0.03;
    let c = cell.x;
    let px1 = read(1, 0).x;
    let py1 = read(0, 1).x;
    let pxm = read(-1, 0).x;
    let pym = read(0, -1).x;
    // The gradient on the x face and on the y face.
    let dpdx_xf = (px1 - c) * inv;
    let dpdy_yf = (py1 - c) * inv;
    let dy_c = (py1 - pym) * 0.5 * inv;
    let dy_c1 = (read(1, 1).x - read(1, -1).x) * 0.5 * inv;
    let dpdy_xf = 0.5 * (dy_c + dy_c1);
    let dx_c = (px1 - pxm) * 0.5 * inv;
    let dx_c1 = (read(1, 1).x - read(-1, 1).x) * 0.5 * inv;
    let dpdx_yf = 0.5 * (dx_c + dx_c1);
    let ex = kob_eps(dpdx_xf, dpdy_xf, p.mode, p.delta, p.theta0);
    let ey = kob_eps(dpdx_yf, dpdy_yf, p.mode, p.delta, p.theta0);
    let jx = ex.re * ex.re * dpdx_xf - ex.re * ex.im * dpdy_xf;
    let jy = ey.re * ey.re * dpdy_yf + ey.re * ey.im * dpdx_yf;
    vec4(cell.x, cell.y, jx, jy)
}

fn cell_step2(cell, p, ctx) {
    let inv = 1.0 / 0.03;
    let dt = ctx.dt;
    let jxm = read(-1, 0).z;
    let jym = read(0, -1).w;
    let div = ((cell.z - jxm) + (cell.w - jym)) * inv;
    let m = (0.9 / 3.14159265359) * atan(10.0 * (1.0 - cell.y));
    let noise = 0.01 * cell.x * (1.0 - cell.x) * (rand(0x4b) - 0.5);
    let dpdt = (div + cell.x * (1.0 - cell.x) * (cell.x - 0.5 + m) + noise) / 0.0003;
    let np = cell.x + dt * dpdt;
    let lap_t = (read(1, 0).y + read(-1, 0).y + read(0, 1).y + read(0, -1).y - 4.0 * cell.y) * inv * inv;
    let nt = cell.y + dt * (lap_t + p.latent_heat * dpdt);
    vec4(np, nt, cell.z, cell.w)
}

// Solid inside the init shape, the melt at 0 elsewhere.
fn cell_seed(inside, noise) {
    vec4(if inside >= 0.5 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0)
}

// Explicit Euler on the temperature's Laplacian: dt < dx^2 / 4.
fn max_dt(p) {
    0.03 * 0.03 / 4.0
}
