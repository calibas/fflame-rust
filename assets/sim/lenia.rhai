// Lenia (Bert Chan): a continuous cellular automaton -- a smooth ring
// kernel and a growth function, and lifelike "creatures" glide.
//
// The Rhai twin of the built-in `lenia` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its kernel table, and the same step.
//
// Its kernel is built on the CPU by `fn kernel(p)` (the exponential core
// scaled to the radius, normalised to sum 1), and the step gathers
// against it through `tap(i)`. Channels: x = the state, y = the
// neighbourhood potential, z = the step the cell last changed
// appreciably, w spare.

// The framework's `sim_rand` at the cell (x, y), stream `k`.

const PARAMS = #{
    radius: #{
        value: 13.0, min: 4.0, max: 32.0, caption: "Kernel radius (R)",
        hint: "Half-width of the ring, in cells — it sets the size of everything the rule builds. Cost grows as R²: 729 taps a cell at 13, 4,225 at 32.",
    },
    mu: #{
        value: 0.15, min: 0.0, max: 0.5, caption: "μ (growth centre)",
        hint: "The neighbourhood average a cell is rewarded for having. Higher values need a denser neighbourhood to survive.",
    },
    sigma: #{
        value: 0.015, min: 0.001, max: 0.1, caption: "σ (growth width)",
        hint: "How forgiving that reward is, and the parameter that decides whether there is anything to look at — measured, 0.015 gives a living field and 0.03 upward freezes into saturated blobs.",
    },
};
const INFO = #{
    title: "Lenia",
    id: "lenia",
    description: "Life made continuous: a ring-shaped kernel and a smooth growth rule, giving soft filaments and cells that crawl.",
    dt: 0.1,
    max_dt: 0.5,
    steps: 600,
    settles: false,
    presets: [
        #{
            name: "soup",
            title: "Soup",
            params: #{ radius: 13.0, mu: 0.15, sigma: 0.015 },
            steps: 600,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn rand_at(x, y, k, w, layer, lo, hi, step) {
    let h = pcg((y * w + x) ^ lo ^ (layer * 0x9E3779B9));
    h = pcg(h ^ hi ^ k);
    h = pcg(h ^ step);
    to_float(h >> 8) * (1.0 / 16777216.0)
}

fn cell_step(cell, p, ctx) {
    let r = ctx.kernel_radius;
    let w = 2 * r + 1;
    let u = 0.0;
    for dy in -r..=r {
        for dx in -r..=r {
            let k = tap((dy + r) * w + (dx + r));
            if k > 0.0 {
                u = u + k * read(dx, dy).x;
            }
        }
    }
    // The growth: a Gaussian bump about mu, from -1 to 1.
    let sg = max(p.sigma, 0.00001);
    let d = u - p.mu;
    let g = 2.0 * exp(-(d * d) / (2.0 * sg * sg)) - 1.0;
    let na = clamp(cell.x + ctx.dt * g, 0.0, 1.0);
    vec4(na, u, if abs(na - cell.x) > 0.0001 { ctx.step } else { cell.z }, 0.0)
}

// Noise in blocks of the kernel's radius.
fn cell_seed(inside, noise, p, ctx) {
    let r = max(ctx.kernel_radius, 1);
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    vec4(rand_at(x / r, y / r, 0x1e, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step), 0.0, 0.0, 0.0)
}

// The kernel: exp(4 - 4 / (4 t (1 - t))) at t the distance over the
// radius, inside the disc, normalised to sum 1.
fn kernel(p) {
    let r = to_int(min(max(round(p.radius), 2.0), 32.0));
    let w = 2 * r + 1;
    let weights = [];
    let sum = 0.0;
    for iy in 0..w {
        for ix in 0..w {
            let dx = to_float(ix - r);
            let dy = to_float(iy - r);
            let d = sqrt(dx * dx + dy * dy) / to_float(r);
            let v = 0.0;
            if d < 1.0 {
                // Away from 0 and 1, where the exponent is -inf.
                let t = min(max(d, 0.000001), 1.0 - 0.000001);
                v = exp(4.0 - 4.0 / (4.0 * t * (1.0 - t)));
                sum += v;
            }
            weights.push(v);
        }
    }
    if sum > 0.0 {
        for i in 0..weights.len() {
            weights[i] = weights[i] / sum;
        }
    }
    #{ radius: r, weights: weights }
}
