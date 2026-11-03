// FitzHugh–Nagumo, the canonical excitable medium.
//
// The Rhai twin of the built-in `fitzhugh_nagumo` model
// (src/sim/models.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters, presets and working values, and the same field.
//
//   dv/dt = D_v lap(v) + v - v^3/3 - w + I
//   tau dw/dt = D_w lap(w) + v + a - b w
//
// Channels: x = v (the fast activator), y = w (the slow recovery),
// z = the step the cell last fired, w spare.

const PARAMS = #{
    a: #{
        value: 0.7, min: 0.0, max: 1.5, caption: "a",
        hint: "Offset in the recovery equation. With b, sets where the rest state sits.",
    },
    b: #{
        value: 0.8, min: 0.0, max: 1.5, caption: "b",
        hint: "Recovery decay. Larger values damp the slow variable faster.",
    },
    tau: #{
        value: 12.5, min: 1.0, max: 100.0, caption: "τ (recovery time)",
        hint: "How much slower recovery is than excitation. The separation of timescales is what makes the medium excitable rather than oscillatory.",
    },
    drive: #{
        value: 0.5, min: -1.0, max: 2.0, caption: "Drive (I)",
        hint: "External current. Around 0.5 the medium is excitable with a stable rest state; higher values make it oscillate on its own.",
    },
    diffusion_v: #{
        value: 1.0, min: 0.0, max: 4.0, caption: "Diffusion v",
        hint: "Spread of the excitation variable. Sets the wave speed and width.",
    },
    diffusion_w: #{
        value: 0.0, min: 0.0, max: 4.0, caption: "Diffusion w",
        hint: "Spread of the recovery variable. 0 is the classic excitable choice; raising it is the Turing direction, which is unverified here.",
    },
};
const INFO = #{
    title: "FitzHugh–Nagumo",
    id: "fitzhugh_nagumo",
    description: "Excitable medium: travelling pulses and rotating spiral waves. Needs a cut wavefront to nucleate — from noise it relaxes flat.",
    dt: 0.1,
    max_dt: 0.75,
    steps: 4000,
    diffusion: ["diffusion_v", "diffusion_w"],
    settles: false,
    presets: [
        #{
            name: "spiral",
            title: "Spiral waves",
            params: #{ a: 0.7, b: 0.8, tau: 12.5, drive: 0.5, diffusion_v: 1.0, diffusion_w: 0.0 },
            steps: 4000,
            boundary: "periodic",
            init: "broken_wave",
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.4, offset: 0.5, wrap: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    // The Sims stencil, as Gray-Scott's.
    let up = read(0, -1);
    let dn = read(0, 1);
    let lf = read(-1, 0);
    let rt = read(1, 0);
    let ul = read(-1, -1);
    let ur = read(1, -1);
    let dl = read(-1, 1);
    let dr = read(1, 1);
    let lap_v = -cell.x + 0.2 * (up.x + dn.x + lf.x + rt.x) + 0.05 * (ul.x + ur.x + dl.x + dr.x);
    let lap_w = -cell.y + 0.2 * (up.y + dn.y + lf.y + rt.y) + 0.05 * (ul.y + ur.y + dl.y + dr.y);
    let dt = ctx.dt;
    let v = cell.x;
    let w = cell.y;
    // v clamped: the cubic runs away past the nullcline's knees at a
    // large dt.
    let nv = clamp(v + dt * (p.diffusion_v * lap_v + v - v * v * v / 3.0 - w + p.drive), -3.0, 3.0);
    let nw = w + dt / p.tau * (p.diffusion_w * lap_w + v + p.a - p.b * w);
    // The step a cell last fired: v crossing zero upward.
    let age = if v <= 0.0 && nv > 0.0 { ctx.step } else { cell.z };
    vec4(nv, nw, age, 0.0)
}

// The rest state, excited inside the init shape's full cover and
// refractory in its half cover (the broken wave's tail).
fn cell_seed(inside, noise) {
    let v = -1.2;
    let w = -0.6;
    if inside >= 0.75 {
        v = 2.0;
    } else if inside >= 0.25 {
        w = 1.0;
    }
    vec4(v, w, 0.0, 0.0)
}
