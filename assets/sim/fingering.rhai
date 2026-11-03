// Viscous fingering (Saffman-Taylor): a thin fluid driven into a thick
// one through a Hele-Shaw cell, the interface splitting into fingers.
//
// The Rhai twin of the built-in `fingering` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Two passes, the first repeated `relax` times a step (`INFO.repeat`):
// the pressure relaxed under the concentration's mobility, between the
// inlet (the bottom row, at 1) and the outlet (the top, at 0); then the
// concentration advected by Darcy's velocity, scaled so its peak a step
// ago (`ctx.field_max`) moves `speed` cells. Channels: x = the speed,
// y = the pressure, z = the concentration of the driving fluid, w spare.

// Mobility: exp(log_mobility * c).

const PARAMS = #{
    log_mobility: #{
        value: 2.0, min: -3.0, max: 5.0, caption: "Log mobility ratio R",
        hint: "ln of the viscosity ratio between the displaced fluid and the displacing one: mobility is exp(R × concentration), so R > 0 means the entering fluid is thinner. THIS IS THE INSTABILITY: positive and the front fingers, negative and it flattens, zero and it just advects. Larger makes fingers narrower and faster.",
    },
    speed: #{
        value: 0.3, min: 0.02, max: 0.5, caption: "Peak speed (cells/step)",
        hint: "How far the fastest cell moves in one step. The flow is rescaled to this every step, which is a choice of time unit and also the Courant number of the advection — so it is capped at one half, where an upwind step stays stable.",
    },
    diffusion: #{
        value: 0.02, min: 0.0, max: 0.2, caption: "Diffusion",
        hint: "Isotropic mixing of the two fluids, per step. It sets the smallest finger that can survive: less and the front breaks into many fine fingers, more and only a few broad ones grow. Capped at 0.2, where the explicit five-point step is still stable.",
    },
    disturbance: #{
        value: 0.05, min: 0.0, max: 0.5, caption: "Inlet disturbance ζ",
        hint: "Random noise on the initial concentration next to the inlet, which the instability amplifies. Zero is a perfectly flat front, which stays flat: the fingers need something to grow from. Holzbecher uses 0.01 with a much finer mesh than a grid this size has.",
    },
    relax: #{
        value: 20.0, min: 1.0, max: 200.0, caption: "Pressure sweeps",
        hint: "Jacobi iterations of the pressure equation per step, warm-started from the last step. Darcy flow is quasi-static, so the pressure has to be re-solved every time the concentration moves; too few sweeps and the flow lags the front.",
    },
};
const INFO = #{
    title: "Viscous Fingering",
    id: "fingering",
    description: "A thin fluid pushed into a thick one in a porous slab: the front is unstable and breaks into fingers that race ahead and shield their neighbours. Saffman–Taylor's instability, as a miscible displacement.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 800,
    state: "discrete",
    time_step: false,
    repeat: #{ pass: 1, param: "relax" },
    presets: [
        #{
            name: "fingers",
            title: "Fingers",
            params: #{ log_mobility: 2.0, speed: 0.3, diffusion: 0.02, disturbance: 0.05, relax: 20.0 },
            steps: 800,
            boundary: "clamp",
            init: "line",
            coloring: "channel",
            coloring_params: #{ channel: 2.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "stable",
            title: "Stable (thick pushes thin)",
            params: #{ log_mobility: -2.0, speed: 0.3, diffusion: 0.02, disturbance: 0.05, relax: 20.0 },
            steps: 600,
            boundary: "clamp",
            init: "line",
            coloring: "channel",
            coloring_params: #{ channel: 2.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn mobility(c, log_mobility) {
    exp(log_mobility * clamp(c, 0.0, 1.0))
}

fn cell_step(cell, p, ctx) {
    let y = to_int(ctx.cell.im);
    if y >= ctx.height - 1 {
        return vec4(cell.x, 1.0, cell.z, cell.w);
    }
    if y <= 0 {
        return vec4(cell.x, 0.0, cell.z, cell.w);
    }
    // The pressure: the mobility-weighted mean of the neighbours', the
    // faces' mobilities their harmonic means.
    let m0 = mobility(cell.z, p.log_mobility);
    let num = 0.0;
    let den = 0.0;
    for i in 0..4 {
        let dx = 1;
        let dy = 0;
        if i == 1 {
            dx = -1;
        } else if i == 2 {
            dx = 0;
            dy = 1;
        } else if i == 3 {
            dx = 0;
            dy = -1;
        }
        let q = read(dx, dy);
        let m1 = mobility(q.z, p.log_mobility);
        let t = 2.0 * m0 * m1 / max(m0 + m1, 0.00000000000000000001);
        num = num + t * q.y;
        den = den + t;
    }
    vec4(cell.x, num / max(den, 0.00000000000000000001), cell.z, cell.w)
}

fn cell_step2(cell, p, ctx) {
    let l = read(-1, 0);
    let r = read(1, 0);
    let d = read(0, -1);
    let u = read(0, 1);
    let m = mobility(cell.z, p.log_mobility);
    // Darcy's velocity, down the pressure's gradient.
    let vx = -m * 0.5 * (r.y - l.y);
    let vy = -m * 0.5 * (u.y - d.y);
    let speed = complex(vx, vy).abs();
    let y = to_int(ctx.cell.im);
    if y >= ctx.height - 1 {
        return vec4(speed, cell.y, 1.0, cell.w);
    }
    let peak = max(ctx.field_max, 0.000000000001);
    let wx = vx * (p.speed / peak);
    let wy = vy * (p.speed / peak);
    // Upwind differences, and a little diffusion.
    let dcx = if wx < 0.0 { r.z - cell.z } else { cell.z - l.z };
    let dcy = if wy < 0.0 { u.z - cell.z } else { cell.z - d.z };
    let adv = wx * dcx + wy * dcy;
    let lap = l.z + r.z + d.z + u.z - 4.0 * cell.z;
    vec4(speed, cell.y, clamp(cell.z - adv + p.diffusion * lap, 0.0, 1.0), cell.w)
}

// The pressure falling from the inlet; the driving fluid inside the init
// shape, its front disturbed near the inlet.
fn cell_seed(inside, noise, p, ctx) {
    let y = to_int(ctx.cell.im);
    let gy = to_float(ctx.height);
    let pr = to_float(y) / max(gy - 1.0, 1.0);
    let dist = gy - 1.0 - to_float(y);
    let bump = p.disturbance * (noise - 0.5) * 2.0 * exp(-dist * dist / 16.0);
    let pat = if inside >= 0.5 { 1.0 } else { 0.0 };
    vec4(0.0, pr, clamp(pat + bump, 0.0, 1.0), 0.0)
}
