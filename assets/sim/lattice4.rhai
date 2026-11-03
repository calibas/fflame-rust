// Four Turing fields on one lattice, coupled by a 4x4 matrix: the
// activator-inhibitor kernel's response of each species to each other,
// saturated, with noise -- and, with memory, a fourth field that
// remembers the three.
//
// The Rhai twin of the built-in `lattice4` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its kernel table, and the same step.
//
// The kernel is two blocks (`fn kernel(p)`): the activator disc and the
// inhibitor disc, each normalised to sum 1 and anti-aliased over a
// one-cell band; the step gathers their difference (`tap(i) - tap(taps +
// i)`). Channels: the four species.

const PARAMS = #{
    kaa: #{
        value: 1.0, min: -3.0, max: 3.0, caption: "A ← A",
        hint: "How much field A follows its own Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kab: #{
        value: -1.5, min: -3.0, max: 3.0, caption: "A ← B",
        hint: "How much field A follows the next field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kac: #{
        value: 0.0, min: -3.0, max: 3.0, caption: "A ← C",
        hint: "How much field A follows the opposite field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kad: #{
        value: 1.5, min: -3.0, max: 3.0, caption: "A ← D",
        hint: "How much field A follows the previous field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kba: #{
        value: 1.5, min: -3.0, max: 3.0, caption: "B ← A",
        hint: "How much field B follows the previous field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kbb: #{
        value: 1.0, min: -3.0, max: 3.0, caption: "B ← B",
        hint: "How much field B follows its own Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kbc: #{
        value: -1.5, min: -3.0, max: 3.0, caption: "B ← C",
        hint: "How much field B follows the next field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kbd: #{
        value: 0.0, min: -3.0, max: 3.0, caption: "B ← D",
        hint: "How much field B follows the opposite field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kca: #{
        value: 0.0, min: -3.0, max: 3.0, caption: "C ← A",
        hint: "How much field C follows the opposite field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kcb: #{
        value: 1.5, min: -3.0, max: 3.0, caption: "C ← B",
        hint: "How much field C follows the previous field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kcc: #{
        value: 1.0, min: -3.0, max: 3.0, caption: "C ← C",
        hint: "How much field C follows its own Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kcd: #{
        value: -1.5, min: -3.0, max: 3.0, caption: "C ← D",
        hint: "How much field C follows the next field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kda: #{
        value: -1.5, min: -3.0, max: 3.0, caption: "D ← A",
        hint: "How much field D follows the next field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kdb: #{
        value: 0.0, min: -3.0, max: 3.0, caption: "D ← B",
        hint: "How much field D follows the opposite field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kdc: #{
        value: 1.5, min: -3.0, max: 3.0, caption: "D ← C",
        hint: "How much field D follows the previous field's Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    kdd: #{
        value: 1.0, min: -3.0, max: 3.0, caption: "D ← D",
        hint: "How much field D follows its own Turing signal (activator minus inhibitor). Positive follows it, negative pushes against it. The diagonal at 1 is a plain Turing pattern; following the previous field and opposing the next, all the way round, is the ring that cycles.",
    },
    radius: #{
        value: 4.0, min: 1.0, max: 15.0, caption: "Activator radius",
        hint: "The averaging radius of every field's activator, in cells: the size of the pattern. Under inflation it must stay well above the edge's per-step displacement, or the resample blurs the pattern away.",
    },
    ratio: #{
        value: 2.0, min: 1.2, max: 4.0, caption: "Inhibitor ratio",
        hint: "Inhibitor radius over activator radius. McCabe's 2.",
    },
    amount: #{
        value: 0.05, min: 0.001, max: 0.5, caption: "Step",
        hint: "How far a field moves per step, in a range of 2. Larger is faster and coarser-grained in time.",
    },
    noise: #{
        value: 0.01, min: 0.0, max: 0.5, caption: "Fluctuations",
        hint: "Random perturbation of every field every step — the microscopic fluctuations. Under inflation these are what grow into structure.",
    },
    gain: #{
        value: 4.0, min: 0.5, max: 50.0, caption: "Gain",
        hint: "Multiplies the coupled Turing signal before it saturates. High gain is McCabe's all-or-nothing step and a hard-edged picture; low gain is graded, with amplitude falling to nothing where activator and inhibitor cancel — the membranes.",
    },
    decay: #{
        value: 1.0, min: 0.0, max: 2.0, caption: "Decay",
        hint: "Pulls every field back toward zero each step, as a fraction of the step amount. With decay the amplitude settles where the drive balances it; without it every field saturates at its bounds.",
    },
    quadratic: #{
        value: 0.0, min: -2.0, max: 2.0, caption: "Quadratic",
        hint: "An even term in the saturation, x + q·x². Turing theory: an odd nonlinearity selects stripes, an even one selects spots. Which sign gives spots rather than holes depends on the palette's reading of a lead.",
    },
    memory: #{
        value: 0, choices: ["Turing pattern", "Memory"], caption: "Field D is",
        hint: "A fourth Turing pattern, or a MEMORY: a moving average of what the patterns did — row D weights their values, and D ← D weights their amplitude, which is the geometry and does not average away as a cycling field does — so under inflation it holds old, magnified structure. Column D is how it biases each pattern, place by place: different dynamics in different areas.",
    },
    leak: #{
        value: 0.05, min: 0.0, max: 1.0, caption: "Memory leak",
        hint: "How fast the memory fades, as a fraction of the step amount: at 0.05 and step 0.05 it remembers about 400 steps.",
    },
};
const INFO = #{
    title: "Coupled Turing Lattice",
    id: "lattice4",
    description: "Four Turing patterns on one lattice, coupled by a 4×4 matrix. Chained in a ring they chase each other and every cell cycles in place, like a Belousov–Zhabotinsky reaction; under the warp's inflation the structure is carried outward while new structure forms within it.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 2000,
    presets: [
        #{
            name: "ring",
            title: "Chasing ring",
            params: #{ kaa: 1.0, kab: -1.5, kac: 0.0, kad: 1.5, kba: 1.5, kbb: 1.0, kbc: -1.5, kbd: 0.0, kca: 0.0, kcb: 1.5, kcc: 1.0, kcd: -1.5, kda: -1.5, kdb: 0.0, kdc: 1.5, kdd: 1.0, radius: 4.0, ratio: 2.0, amount: 0.05, noise: 0.01, gain: 4.0, decay: 1.0, quadratic: 0.0, memory: 0.0, leak: 0.05 },
            steps: 2000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "species",
            coloring_params: #{ scale: 1.0, rotate: 0.0, fields: 0.0 },
        },
        #{
            name: "independent",
            title: "Four independent patterns",
            params: #{ kaa: 1.0, kab: 0.0, kac: 0.0, kad: 0.0, kba: 0.0, kbb: 1.0, kbc: 0.0, kbd: 0.0, kca: 0.0, kcb: 0.0, kcc: 1.0, kcd: 0.0, kda: 0.0, kdb: 0.0, kdc: 0.0, kdd: 1.0, radius: 4.0, ratio: 2.0, amount: 0.05, noise: 0.01, gain: 4.0, decay: 1.0, quadratic: 0.0, memory: 0.0, leak: 0.05 },
            steps: 2000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "species",
            coloring_params: #{ scale: 1.0, rotate: 0.0, fields: 0.0 },
        },
        #{
            name: "cells",
            title: "Cells",
            params: #{ kaa: 1.0, kab: -1.5, kac: 1.5, kad: 0.2, kba: 1.5, kbb: 1.0, kbc: -1.5, kbd: 0.0, kca: -1.5, kcb: 1.5, kcc: 1.0, kcd: -0.2, kda: 0.0, kdb: 0.0, kdc: 0.0, kdd: 1.0, radius: 6.0, ratio: 2.0, amount: 0.05, noise: 0.01, gain: 4.0, decay: 1.0, quadratic: 1.5, memory: 1.0, leak: 0.05 },
            steps: 3000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "species",
            coloring_params: #{ scale: 1.0, rotate: 0.0, fields: 1.0 },
        },
        #{
            name: "cells_inflating",
            title: "Cells, inflating",
            params: #{ kaa: 1.0, kab: -1.5, kac: 1.5, kad: 0.2, kba: 1.5, kbb: 1.0, kbc: -1.5, kbd: 0.0, kca: -1.5, kcb: 1.5, kcc: 1.0, kcd: -0.2, kda: 0.0, kdb: 0.0, kdc: 0.0, kdd: 1.0, radius: 6.0, ratio: 2.0, amount: 0.05, noise: 0.01, gain: 4.0, decay: 1.0, quadratic: 1.5, memory: 1.0, leak: 0.05 },
            steps: 8000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "species",
            coloring_params: #{ scale: 1.0, rotate: 0.0, fields: 1.0 },
            warp: #{ filter: "bicubic", mode: "octaves", zoom: 1.002 },
        },
        #{
            name: "inflating",
            title: "Inflating space",
            params: #{ kaa: 1.0, kab: -1.5, kac: 0.0, kad: 1.5, kba: 1.5, kbb: 1.0, kbc: -1.5, kbd: 0.0, kca: 0.0, kcb: 1.5, kcc: 1.0, kcd: -1.5, kda: -1.5, kdb: 0.0, kdc: 1.5, kdd: 1.0, radius: 4.0, ratio: 2.0, amount: 0.05, noise: 0.01, gain: 4.0, decay: 1.0, quadratic: 0.0, memory: 0.0, leak: 0.05 },
            steps: 6000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "species",
            coloring_params: #{ scale: 1.0, rotate: 0.0, fields: 0.0 },
            warp: #{ filter: "bicubic", mode: "octaves", zoom: 1.002 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    let r = ctx.kernel_radius;
    let w = 2 * r + 1;
    let taps = ctx.kernel_taps;
    let tx = 0.0;
    let ty = 0.0;
    let tz = 0.0;
    let tw = 0.0;
    for dy in -r..=r {
        for dx in -r..=r {
            let i = (dy + r) * w + (dx + r);
            let k = tap(i) - tap(taps + i);
            let q = read(dx, dy);
            tx = tx + k * q.x;
            ty = ty + k * q.y;
            tz = tz + k * q.z;
            tw = tw + k * q.w;
        }
    }
    let memory = p.memory == 1;
    // With memory, the fourth species drives from its own state.
    let uw = if memory { cell.w } else { tw };
    let drive_x = p.kaa * tx + p.kab * ty + p.kac * tz + p.kad * uw;
    let drive_y = p.kba * tx + p.kbb * ty + p.kbc * tz + p.kbd * uw;
    let drive_z = p.kca * tx + p.kcb * ty + p.kcc * tz + p.kcd * uw;
    let drive_w = p.kda * tx + p.kdb * ty + p.kdc * tz + p.kdd * uw;
    let xi_x = rand(0x41) - 0.5;
    let xi_y = rand(0x42) - 0.5;
    let xi_z = rand(0x43) - 0.5;
    let xi_w = rand(0x44) - 0.5;
    // The response: gain, a quadratic term, saturated.
    let ax = drive_x * p.gain;
    let ay = drive_y * p.gain;
    let az = drive_z * p.gain;
    let aw = drive_w * p.gain;
    ax = ax + p.quadratic * ax * ax;
    ay = ay + p.quadratic * ay * ay;
    az = az + p.quadratic * az * az;
    aw = aw + p.quadratic * aw * aw;
    let keep = 1.0 - p.amount * p.decay;
    let nx = cell.x * keep + p.amount * (ax / (1.0 + abs(ax))) + xi_x * p.noise;
    let ny = cell.y * keep + p.amount * (ay / (1.0 + abs(ay))) + xi_y * p.noise;
    let nz = cell.z * keep + p.amount * (az / (1.0 + abs(az))) + xi_z * p.noise;
    let nw = cell.w * keep + p.amount * (aw / (1.0 + abs(aw))) + xi_w * p.noise;
    if memory {
        // The memory fed by the three species and their balance.
        let fed = p.kda * cell.x + p.kdb * cell.y + p.kdc * cell.z + p.kdd * complex(cell.x - 0.5 * (cell.y + cell.z), 0.8660254 * (cell.y - cell.z)).abs();
        nw = cell.w * (1.0 - p.amount * p.leak) + p.amount * p.leak * fed + xi_w * p.noise;
    }
    vec4(clamp(nx, -1.0, 1.0), clamp(ny, -1.0, 1.0), clamp(nz, -1.0, 1.0), clamp(nw, -1.0, 1.0))
}

// Small noise inside the init shape, each species its own.
fn cell_seed(inside, noise) {
    vec4((rand(0x51) - 0.5) * inside, (rand(0x52) - 0.5) * inside, (rand(0x53) - 0.5) * inside, (rand(0x54) - 0.5) * inside)
}

// The activator disc and the inhibitor disc, anti-aliased, each summing
// to 1, the second block after the first.
fn kernel(p) {
    let ra = min(max(p.radius, 1.0), 15.0);
    let rb = ra * min(max(p.ratio, 1.2), 4.0);
    let r = min(max(to_int(ceiling(rb)) + 1, 1), 32);
    let w = 2 * r + 1;
    let a = [];
    let b = [];
    let sa = 0.0;
    let sb = 0.0;
    for iy in 0..w {
        for ix in 0..w {
            let dx = to_float(ix - r);
            let dy = to_float(iy - r);
            let d = sqrt(dx * dx + dy * dy);
            let wa = min(max(ra + 0.5 - d, 0.0), 1.0);
            let wb = min(max(rb + 0.5 - d, 0.0), 1.0);
            a.push(wa);
            b.push(wb);
            sa += wa;
            sb += wb;
        }
    }
    for i in 0..a.len() {
        a[i] = a[i] / max(sa, 1e-12);
        b[i] = b[i] / max(sb, 1e-12);
    }
    a.append(b);
    #{ radius: r, weights: a }
}
