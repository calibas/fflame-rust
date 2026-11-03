// Physarum (Jeff Jones, "Characteristics of pattern formation and
// evolution in approximations of Physarum transport networks", 2010):
// agents sense the trail ahead, turn toward it, move and deposit; the
// trail spreads and decays, and the feedback builds a transport network.
//
// The Rhai twin of the built-in `physarum` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, the same trail step and the same two agent passes.
//
// The cells are the trail map: x = the trail, w = what the agents
// deposited this step. The agents are records of four, `vec4(x, y,
// heading, state)`: seeded at random over the grid, then each step a pass
// that senses, turns and claims the cell it wants (the lowest index wins
// a contested cell), and a pass that moves the winners, which deposit, and
// turns the rest at random.

const PARAMS = #{
    population: #{
        value: 5.0, min: 0.5, max: 25.0, caption: "Population (% of grid)",
        hint: "Agents as a percentage of the grid's cells — the paper's %p, whose useful range is 3 to 15. Too few and the trails never meet; too many and the network fills in.",
    },
    sensor_angle: #{
        value: 22.5, min: 5.0, max: 90.0, caption: "Sensor angle (SA)",
        hint: "How far to each side an agent looks, in degrees. One of the three parameters the paper says actually change the pattern; it uses 22.5 or 45.",
    },
    rotation_angle: #{
        value: 45.0, min: 5.0, max: 90.0, caption: "Rotation angle (RA)",
        hint: "How far an agent turns when it turns, in degrees. The paper uses 45.",
    },
    sensor_offset: #{
        value: 9.0, min: 1.0, max: 32.0, caption: "Sensor distance (SO)",
        hint: "How far ahead the sensors sit, in cells. The paper uses 9, and notes that a distance of at least 3 is what makes the population couple strongly enough for networks to form at all.",
    },
    step_size: #{
        value: 1.0, min: 0.25, max: 4.0, caption: "Step size (SS)",
        hint: "How far an agent moves per step, in cells.",
    },
    deposit: #{
        value: 5.0, min: 0.5, max: 40.0, caption: "Deposit (depT)",
        hint: "How much trail an agent lays each step. Scales against the decay: what matters is the ratio.",
    },
    decay: #{
        value: 0.1, min: 0.005, max: 0.5, caption: "Decay (decayT)",
        hint: "How fast the trail fades. The other parameter the paper says matters: low values let old trails persist into thick networks, high values keep only what is currently travelled.",
    },
};
const INFO = #{
    title: "Physarum",
    id: "physarum",
    description: "Slime mould: thousands of agents lay a chemical trail and follow it, and the feedback builds a transport network of filaments and junctions.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 600,
    time_step: false,
    settles: false,
    presets: [
        #{
            name: "network",
            title: "Transport network",
            params: #{ population: 5.0, sensor_angle: 22.5, rotation_angle: 45.0, sensor_offset: 9.0, step_size: 1.0, deposit: 5.0, decay: 0.1 },
            steps: 600,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 0.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.15, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "coarse",
            title: "Coarse mesh",
            params: #{ population: 5.0, sensor_angle: 45.0, rotation_angle: 45.0, sensor_offset: 9.0, step_size: 1.0, deposit: 5.0, decay: 0.1 },
            steps: 600,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 0.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.15, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    // Jones' trail map update: a 3x3 mean, then the decay factor.
    let acc = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            acc = acc + read(dx, dy).x;
        }
    }
    // What the agents left here since the last step, and clear it: one
    // step later than Jones, measured to make no difference (the built-in
    // says how).
    let dep = take_deposit();
    let trail = (acc * (1.0 / 9.0) + dep) * (1.0 - p.decay);
    // The deposit rides in .w: where the agents are.
    rgba(trail, 0.0, cell.z, dep)
}

// An empty trail map: the agents are the initial condition.
fn cell_seed(inside, noise) {
    rgba(0.0, 0.0, 0.0, 0.0)
}

// Would a move to (x, y) leave the grid? Only a periodic field lets a
// position wrap; under any other boundary a wall is an unsuccessful move.
// Both passes ask, and must agree: a claim is released only by the same
// agent's check in the next pass.
fn phys_off_grid(x, y, ctx) {
    let gx = to_float(ctx.width);
    let gy = to_float(ctx.height);
    ctx.periodic == 0 && (x < -0.5 || y < -0.5 || x >= gx - 0.5 || y >= gy - 0.5)
}

// One sensor: the trail `sensor_offset` ahead, `off` from the heading.
fn phys_sense(a, off, p) {
    let qx = a.x + cos(a.z + off) * p.sensor_offset;
    let qy = a.y + sin(a.z + off) * p.sensor_offset;
    read_at(to_int(floor(qx + 0.5)), to_int(floor(qy + 0.5))).x
}

// Jones: a random location and a random heading over the full circle.
fn agent_seed(p, ctx) {
    let gx = to_float(ctx.width);
    let gy = to_float(ctx.height);
    rgba(rand(0x11) * gx, rand(0x12) * gy, rand(0x13) * 6.28318530718, 0.0)
}

// Pass 1: sense, turn, and claim the cell this agent wants.
fn agent_step(a, p, ctx) {
    let sa = p.sensor_angle * 0.017453292519943295;
    let ra = p.rotation_angle * 0.017453292519943295;
    let f = phys_sense(a, 0.0, p);
    let l = phys_sense(a, sa, p);
    let r = phys_sense(a, -sa, p);
    // Jones' figure 3, in its order: when both sides beat the front the
    // turn is random, whichever side is stronger.
    let turn = 0.0;
    if f >= l && f >= r {
        turn = 0.0;
    } else if f < l && f < r {
        turn = if rand(0x14) < 0.5 { ra } else { -ra };
    } else if l > r {
        turn = ra;
    } else if r > l {
        turn = -ra;
    }
    let heading = a.z + turn;
    let dx = a.x + cos(heading) * p.step_size;
    let dy = a.y + sin(heading) * p.step_size;
    if !phys_off_grid(dx, dy, ctx) {
        claim(to_int(floor(dx + 0.5)), to_int(floor(dy + 0.5)));
    }
    rgba(a.x, a.y, heading, a.w)
}

// Pass 2: move if this agent won the cell, and deposit; otherwise stay,
// deposit nothing, and take a new random heading.
fn agent_step2(a, p, ctx) {
    let gx = to_float(ctx.width);
    let gy = to_float(ctx.height);
    let dx = a.x + cos(a.z) * p.step_size;
    let dy = a.y + sin(a.z) * p.step_size;
    let x = a.x;
    let y = a.y;
    let heading = a.z;
    if !phys_off_grid(dx, dy, ctx) && claim_check(to_int(floor(dx + 0.5)), to_int(floor(dy + 0.5))) {
        // The position is continuous and only the deposit is on the
        // lattice; a periodic field wraps the position, any other keeps
        // it (the off-grid test has confined it).
        x = dx;
        y = dy;
        if ctx.periodic == 1 {
            x = dx - gx * floor(dx / gx);
            y = dy - gy * floor(dy / gy);
        }
        deposit(to_int(floor(x + 0.5)), to_int(floor(y + 0.5)), p.deposit);
    } else {
        heading = rand(0x15) * 6.28318530718;
    }
    rgba(x, y, heading, a.w)
}

// The paper's %p: a percentage of the grid's area, so a setting is the
// same density at any size.
fn agent_count(p, w, h) {
    let pct = min(max(p.population, 0.1), 50.0);
    to_float(w) * to_float(h) * pct / 100.0
}
