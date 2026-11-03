// A Turing field as a layer: the activator-inhibitor kernel's response,
// saturated, with noise -- and the drive of the couplings aimed at it,
// folded in before its saturation, so several layers make one coupled
// pattern.
//
// The Rhai twin of the built-in `turing` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its kernel table, and the same step.
//
// Two passes: the first writes the kernel's response -- its signal --
// into `.y`, which a Signal coupling from this layer reads
// (`INFO.publishes_signal`); the second adds the drive (`drive()`) and
// steps. Channels: x = the field, y = its signal.

// The model's own hash of the cell, the seed, the stream and the layer.

const PARAMS = #{
    self: #{
        value: 1.0, min: -3.0, max: 3.0, caption: "Self",
        hint: "How much the field follows its own Turing signal (activator minus inhibitor). 1 is a plain Turing pattern; the other fields' signals arrive through Signal couplings at their strengths.",
    },
    radius: #{
        value: 4.0, min: 1.0, max: 15.0, caption: "Activator radius",
        hint: "The averaging radius of the activator, in cells: the size of the pattern. Differs per layer, so coupled fields can be multi-scale.",
    },
    ratio: #{
        value: 2.0, min: 1.2, max: 4.0, caption: "Inhibitor ratio",
        hint: "Inhibitor radius over activator radius. McCabe's 2.",
    },
    amount: #{
        value: 0.05, min: 0.001, max: 0.5, caption: "Step",
        hint: "How far the field moves per step, in a range of 2. Larger is faster and coarser-grained in time.",
    },
    noise: #{
        value: 0.01, min: 0.0, max: 0.5, caption: "Fluctuations",
        hint: "Random perturbation every step — the microscopic fluctuations. Under inflation these are what grow into structure.",
    },
    gain: #{
        value: 4.0, min: 0.5, max: 50.0, caption: "Gain",
        hint: "Multiplies the drive before it saturates. High gain is an all-or-nothing step and a hard-edged picture; low gain is graded, with amplitude falling to nothing where activator and inhibitor cancel.",
    },
    decay: #{
        value: 1.0, min: 0.0, max: 2.0, caption: "Decay",
        hint: "Pulls the field back toward zero each step, as a fraction of the step amount. With decay the amplitude settles where the drive balances it; without it the field saturates at its bounds.",
    },
    quadratic: #{
        value: 0.0, min: -2.0, max: 2.0, caption: "Quadratic",
        hint: "An even term in the saturation, x + q·x². An odd nonlinearity selects stripes, an even one selects spots.",
    },
};
const INFO = #{
    title: "Turing Field",
    id: "turing",
    description: "One difference-of-discs Turing field, made to be a layer: coupled to other Turing fields through Signal couplings it is one field of the coupled lattice, with its own radius, and its own transform.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 2000,
    publishes_signal: true,
    presets: [
        #{
            name: "pattern",
            title: "One pattern",
            params: #{ self: 1.0, radius: 4.0, ratio: 2.0, amount: 0.05, noise: 0.01, gain: 4.0, decay: 1.0, quadratic: 0.0 },
            steps: 2000,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
    ],
};

fn tu_rand(x, y, salt, w, layer, lo, hi, step) {
    let h = pcg((y * w + x) ^ lo);
    h = pcg(h ^ hi ^ (salt + layer));
    h = pcg(h ^ step);
    to_float(h >> 8) * (1.0 / 16777216.0)
}

fn cell_step(cell, p, ctx) {
    let r = ctx.kernel_radius;
    let w = 2 * r + 1;
    let taps = ctx.kernel_taps;
    let t = 0.0;
    for dy in -r..=r {
        for dx in -r..=r {
            let i = (dy + r) * w + (dx + r);
            t = t + (tap(i) - tap(taps + i)) * read(dx, dy).x;
        }
    }
    vec4(cell.x, t, 0.0, 0.0)
}

fn cell_step2(cell, p, ctx) {
    let drive = p.self * cell.y + drive().x;
    let xi = tu_rand(to_int(ctx.cell.re), to_int(ctx.cell.im), 0x41, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step) - 0.5;
    let x = drive * p.gain;
    x = x + p.quadratic * x * x;
    let sat = x / (1.0 + abs(x));
    let n = cell.x * (1.0 - p.amount * p.decay) + p.amount * sat + xi * p.noise;
    vec4(clamp(n, -1.0, 1.0), cell.y, 0.0, 0.0)
}

// Small noise inside the init shape.
fn cell_seed(inside, noise, p, ctx) {
    let r = tu_rand(to_int(ctx.cell.re), to_int(ctx.cell.im), 0x51, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step);
    vec4((r - 0.5) * inside, 0.0, 0.0, 0.0)
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
