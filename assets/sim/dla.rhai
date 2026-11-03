// Diffusion-limited aggregation (Witten and Sander, 1981): random walkers
// that stick where they first touch a growing cluster.
//
// The Rhai twin of the built-in `dla` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, the same step and the same walkers.
//
// A frozen cell stores its distance from the centre plus one, so the
// field's maximum (`ctx.field_max`) is the cluster's radius plus one --
// which is where the walkers are launched from, just outside it. A walker
// is dormant, parked on the launch circle, until the cluster is big
// enough for it: so many walkers per cell of the circle's circumference,
// so they arrive one at a time, which is what makes it DLA.

const PARAMS = #{
    walkers: #{
        value: 4.0, min: 0.1, max: 20.0, caption: "Walkers (% of grid)",
        hint: "How many walkers are in flight at once, as a percentage of the grid's cells. More is faster but crowds the cluster, which thickens the branches — the aggregate's dimension depends on walkers arriving one at a time.",
    },
    p_stick: #{
        value: 1.0, min: 0.02, max: 1.0, caption: "Sticking probability",
        hint: "Chance a walker that touches the cluster actually sticks. Below 1 a walker explores further before attaching, which fills the fjords and makes a denser, less branched cluster.",
    },
    crowding: #{
        value: 5.0, min: 0.5, max: 16.0, caption: "Crowding",
        hint: "How many walkers may work the cluster at once, per cell of its circumference. This is the speed-against-fidelity knob: DLA is what it is because particles arrive ONE AT A TIME, so low values grow a truer aggregate slowly and high values grow a denser, blunter one fast.",
    },
    launch_gap: #{
        value: 2.0, min: 2.0, max: 40.0, caption: "Launch gap",
        hint: "How far outside the cluster's current radius walkers are launched.",
    },
};
const INFO = #{
    title: "Diffusion-Limited Aggregation",
    id: "dla",
    description: "Random walkers that stick where they first touch a growing cluster, building the branching aggregate of soot, frost and mineral dendrites.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 1200,
    time_step: false,
    settles: false,
    presets: [
        #{
            name: "cluster",
            title: "Cluster",
            params: #{ walkers: 4.0, p_stick: 1.0, crowding: 5.0, launch_gap: 2.0 },
            steps: 1200,
            boundary: "clamp",
            init: "center",
            coloring: "age",
            coloring_params: #{ window: 1200.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    // Taken first: taking it clears it.
    let dep = take_deposit();
    // Frozen is permanent.
    if cell.x > 0.0 {
        return cell;
    }
    if dep <= 0.0 {
        return cell;
    }
    // Freeze, at the distance from the centre.
    let dx = ctx.cell.re - to_float(ctx.width) * 0.5;
    let dy = ctx.cell.im - to_float(ctx.height) * 0.5;
    rgba(sqrt(dx * dx + dy * dy) + 1.0, 0.0, to_float(ctx.step), 0.0)
}

// The nucleus: the init shape, frozen.
fn cell_seed(inside, noise, p, ctx) {
    if inside < 0.5 {
        return rgba(0.0, 0.0, 0.0, 0.0);
    }
    let dx = ctx.cell.re - to_float(ctx.width) * 0.5;
    let dy = ctx.cell.im - to_float(ctx.height) * 0.5;
    rgba(sqrt(dx * dx + dy * dy) + 1.0, 0.0, 0.0, 0.0)
}

// The cluster's radius, from the field's maximum.
fn dla_radius(ctx) {
    max(ctx.field_max - 1.0, 0.0)
}

// A fresh walker on the launch circle, just outside the cluster.
fn dla_launch(salt, p, ctx) {
    let gx = to_float(ctx.width);
    let gy = to_float(ctx.height);
    let limit = min(gx, gy) * 0.5 - 2.0;
    let r = min(dla_radius(ctx) + p.launch_gap, limit);
    let a = rand(salt) * 6.28318530718;
    // The offset first: the shader compiler fuses this sum's multiplies
    // as it does the built-in's `c + dir * r`, to the last bit.
    rgba(cos(a) * r + gx * 0.5, sin(a) * r + gy * 0.5, 0.0, 0.0)
}

fn agent_seed(p, ctx) {
    dla_launch(0x21, p, ctx)
}

fn agent_step(a, p, ctx) {
    let gx = to_float(ctx.width);
    let gy = to_float(ctx.height);
    // Only as many walkers as the launch circle has room for: the rest
    // stay parked on it.
    let launch_r = min(dla_radius(ctx) + p.launch_gap, min(gx, gy) * 0.5 - 2.0);
    if to_float(ctx.agent) > p.crowding * 6.28318530718 * launch_r {
        return dla_launch(0x26, p, ctx);
    }
    let x = to_int(floor(a.x + 0.5));
    let y = to_int(floor(a.y + 0.5));
    // Touching the cluster? Four-neighbour contact, the lattice
    // convention. Stick here, and start again: the deposit is what the
    // step turns into a frozen cell.
    let touch = read_at(x + 1, y).x > 0.0 || read_at(x - 1, y).x > 0.0 || read_at(x, y + 1).x > 0.0 || read_at(x, y - 1).x > 0.0;
    if touch && read_at(x, y).x <= 0.0 {
        if rand(0x22) < p.p_stick {
            deposit(x, y, 1.0);
            return dla_launch(0x23, p, ctx);
        }
    }
    // One lattice step.
    let r = rand(0x24);
    let dx = 1.0;
    let dy = 0.0;
    if r < 0.25 {
        dx = -1.0;
    } else if r < 0.5 {
        dx = 0.0;
        dy = 1.0;
    } else if r < 0.75 {
        dx = 0.0;
        dy = -1.0;
    }
    let nx = a.x + dx;
    let ny = a.y + dy;
    // Gone too far to be worth following: relaunch. Three times the
    // cluster's radius, and always past the launch circle.
    let kill = min(max(3.0 * dla_radius(ctx), launch_r + 16.0), min(gx, gy) * 0.5 - 1.0);
    let ex = nx - gx * 0.5;
    let ey = ny - gy * 0.5;
    if sqrt(ex * ex + ey * ey) > kill {
        return dla_launch(0x25, p, ctx);
    }
    rgba(nx, ny, a.z, a.w)
}

// A percentage of the grid's area.
fn agent_count(p, w, h) {
    let pct = min(max(p.walkers, 0.05), 40.0);
    to_float(w) * to_float(h) * pct / 100.0
}
