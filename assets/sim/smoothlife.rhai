// SmoothLife (Stephan Rafler): Conway's Life in the continuum -- a disc
// and the annulus around it, smooth birth and death intervals.
//
// The Rhai twin of the built-in `smoothlife` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its kernel table and spectral discs, and the same step.
//
// The neighbourhoods are the inner disc's mean m and the annulus's mean
// n: by FFT when the spectral stage has them (`ctx.exact_discs`,
// `disc(i)`, the discs `fn discs(p)` names), else by a gather against the
// two-block table `fn kernel(p)` builds. Channels: x = the state, y = n,
// z = the step the cell last changed appreciably, w spare.

// The smooth step: 0 below a, 1 above, over a width al.

const PARAMS = #{
    inner_radius: #{
        value: 7.0, min: 2.0, max: 10.0, caption: "Inner radius (rᵢ)",
        hint: "Radius of the disc a cell measures itself over. The outer radius is three times it, as in the paper, so this sets the whole scale — and the cost, which grows as rᵢ².",
    },
    b1: #{
        value: 0.278, min: 0.0, max: 1.0, caption: "Birth low (b₁)",
        hint: "Lower edge of the window in which an empty cell is born.",
    },
    b2: #{
        value: 0.365, min: 0.0, max: 1.0, caption: "Birth high (b₂)",
        hint: "Upper edge of that window.",
    },
    d1: #{
        value: 0.267, min: 0.0, max: 1.0, caption: "Survive low (d₁)",
        hint: "Lower edge of the window in which a filled cell survives.",
    },
    d2: #{
        value: 0.445, min: 0.0, max: 1.0, caption: "Survive high (d₂)",
        hint: "Upper edge of that window. The gap between it and b₂ is what lets structures persist without filling the plane.",
    },
    alpha_n: #{
        value: 0.028, min: 0.001, max: 0.3, caption: "Neighbourhood softness (αₙ)",
        hint: "How sharply the neighbourhood threshold switches. Small values are nearly a hard rule.",
    },
    alpha_m: #{
        value: 0.147, min: 0.001, max: 0.3, caption: "Filling softness (αₘ)",
        hint: "How sharply a cell's own filling picks between the birth and survival windows.",
    },
    ratio: #{
        value: 3.0, min: 1.5, max: 5.0, caption: "Outer / inner radius",
        hint: "How much wider the neighbourhood ring reaches than the inner disc. The paper's is 3; other rule sets use 2.3 to 4. The outer radius stops at 30 cells.",
    },
    stepping: #{
        value: 0, choices: ["Relax", "Increment"], caption: "Time stepping",
        hint: "How a step moves a cell toward the rule. Relax: f + dt·(s − f), which at dt 1 is Rafler's discrete SmoothLife, f = s. Increment: f + dt·(2s − 1), the paper's continuous scheme and SmoothLifeL's. A rule set's gliders exist in one scheme and not the other.",
    },
    blend: #{
        value: 0, choices: ["Thresholds (2011)", "Windows (SmoothLifeL)"], caption: "Window blend",
        hint: "How a cell's own filling chooses between birth and survival. Thresholds: the window's edges slide from birth's to survival's (the 2011 paper). Windows: the birth and survival windows are each computed and mixed (SmoothLifeL).",
    },
    fill: #{
        value: 0.5, min: 0.05, max: 0.95, caption: "Initial density",
        hint: "How much of the field the seed fills.",
    },
    seed_shape: #{
        value: 0, choices: ["Patches", "Blobs"], caption: "Seed",
        hint: "What the run starts from. Patches: squares the size of the inner disc, each filled at the density. Blobs: smooth noise a little wider than the outer ring, thresholded at the density with a soft edge, as Reusser's notebook seeds -- the structures the rules' gliders grow out of.",
    },
};
const INFO = #{
    title: "SmoothLife",
    id: "smoothlife",
    description: "Conway's Life on a continuous domain: the same birth and survival rules read as smooth thresholds, giving gliders and rolling labyrinths.",
    dt: 0.15,
    max_dt: 1.0,
    steps: 400,
    settles: false,
    slow: ["inner_radius"],
    presets: [
        #{
            name: "glider",
            title: "Rafler's glider set",
            params: #{ inner_radius: 7.0, b1: 0.278, b2: 0.365, d1: 0.267, d2: 0.445, alpha_n: 0.028, alpha_m: 0.147, ratio: 3.0, stepping: 0.0, blend: 0.0, fill: 0.3, seed_shape: 1.0 },
            steps: 200,
            dt: 1.0,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "smoothlife_l",
            title: "SmoothLifeL gliders",
            params: #{ inner_radius: 6.6667, b1: 0.257, b2: 0.336, d1: 0.365, d2: 0.549, alpha_n: 0.028, alpha_m: 0.147, ratio: 3.0, stepping: 1.0, blend: 1.0, fill: 0.4, seed_shape: 1.0 },
            steps: 640,
            dt: 0.05,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "soft_gliders",
            title: "Soft gliders",
            params: #{ inner_radius: 4.0, b1: 0.269, b2: 0.34, d1: 0.523, d2: 0.746, alpha_n: 0.028, alpha_m: 0.147, ratio: 3.0, stepping: 0.0, blend: 0.0, fill: 0.5, seed_shape: 1.0 },
            steps: 600,
            dt: 0.115,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn sl_sigma(x, a, al) {
    1.0 / (1.0 + exp(-(x - a) * 4.0 / max(al, 0.0001)))
}

// WGSL's `round`: half to even, as the built-in reads its radius.
fn round_even(x) {
    let r = floor(x + 0.5);
    if r - x == 0.5 && r % 2.0 != 0.0 { r - 1.0 } else { r }
}

// The framework's `sim_rand` at the cell (x, y), stream `k`.
fn rand_at(x, y, k, w, layer, lo, hi, step) {
    let h = pcg((y * w + x) ^ lo ^ (layer * 0x9E3779B9));
    h = pcg(h ^ hi ^ k);
    h = pcg(h ^ step);
    to_float(h >> 8) * (1.0 / 16777216.0)
}

fn cell_step(cell, p, ctx) {
    let m = 0.0;
    let n = 0.0;
    if ctx.exact_discs == 1 {
        m = disc(0) + cell.x;
        n = disc(1);
    } else {
        let r = ctx.kernel_radius;
        let w = 2 * r + 1;
        let taps = ctx.kernel_taps;
        for dy in -r..=r {
            for dx in -r..=r {
                let i = (dy + r) * w + (dx + r);
                let ki = tap(i);
                let ko = tap(taps + i);
                if ki > 0.0 || ko > 0.0 {
                    let v = read(dx, dy).x;
                    m = m + ki * v;
                    n = n + ko * v;
                }
            }
        }
    }
    let pick = sl_sigma(m, 0.5, p.alpha_m);
    let alive = 0.0;
    if p.blend == 1 {
        // SmoothLifeL's windows: birth and survival each an interval,
        // blended by the disc.
        let born = sl_sigma(n, p.b1, p.alpha_n) * (1.0 - sl_sigma(n, p.b2, p.alpha_n));
        let lives = sl_sigma(n, p.d1, p.alpha_n) * (1.0 - sl_sigma(n, p.d2, p.alpha_n));
        alive = born + (lives - born) * pick;
    } else {
        // The 2011 paper's: the thresholds blended, then one interval.
        let lo = p.b1 + (p.d1 - p.b1) * pick;
        let hi = p.b2 + (p.d2 - p.b2) * pick;
        alive = sl_sigma(n, lo, p.alpha_n) * (1.0 - sl_sigma(n, hi, p.alpha_n));
    }
    let nf = clamp(cell.x + ctx.dt * (alive - cell.x), 0.0, 1.0);
    if p.stepping == 1 {
        nf = clamp(cell.x + ctx.dt * (2.0 * alive - 1.0), 0.0, 1.0);
    }
    vec4(nf, n, if abs(nf - cell.x) > 0.0001 { ctx.step } else { cell.z }, 0.0)
}

// Patches of the outer radius's size, or blobs a fill's worth of blocks.
fn cell_seed(inside, noise, p, ctx) {
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    if p.seed_shape == 1 {
        // Value noise at a lattice of 1.2 outer radii, thresholded.
        let ra = p.inner_radius * p.ratio;
        let s = max(3.0, ra * 1.2);
        let qx = to_float(x) / s;
        let qy = to_float(y) / s;
        let ix = to_int(floor(qx));
        let iy = to_int(floor(qy));
        let fx = fract(qx);
        let fy = fract(qy);
        let ux = fx * fx * (3.0 - 2.0 * fx);
        let uy = fy * fy * (3.0 - 2.0 * fy);
        let a = rand_at(ix, iy, 0x61, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step);
        let b = rand_at(ix + 1, iy, 0x61, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step);
        let c = rand_at(ix, iy + 1, 0x61, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step);
        let d = rand_at(ix + 1, iy + 1, 0x61, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step);
        let v = mix(mix(a, b, ux), mix(c, d, ux), uy);
        let t = 1.0 - p.fill;
        let k = clamp((v - (t - 0.08)) / ((t + 0.08) - (t - 0.08)), 0.0, 1.0);
        return vec4(k * k * (3.0 - 2.0 * k), 0.0, 0.0, 0.0);
    }
    let blk = max(to_int(round_even(p.inner_radius)), 1);
    let on = rand_at(x / blk, y / blk, 0x5f, ctx.width, ctx.layer, ctx.seed_lo, ctx.seed_hi, ctx.step) < p.fill;
    vec4(if on { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0)
}

// The inner disc and the annulus out to `ratio` times it, anti-aliased,
// each summing to 1, the annulus's block after the disc's.
fn kernel(p) {
    let ri = min(max(p.inner_radius, 1.0), 10.0);
    let ra = min(ri * min(max(p.ratio, 1.5), 5.0), 32.0 - 1.5);
    let r = min(max(to_int(ceiling(ra)) + 1, 1), 32);
    let w = 2 * r + 1;
    let inner = [];
    let outer = [];
    let si = 0.0;
    let so = 0.0;
    for iy in 0..w {
        for ix in 0..w {
            let dx = to_float(ix - r);
            let dy = to_float(iy - r);
            let d = sqrt(dx * dx + dy * dy);
            let i = min(max(ri + 0.5 - d, 0.0), 1.0);
            let o = min(max(ra + 0.5 - d, 0.0), 1.0) * (1.0 - i);
            inner.push(i);
            outer.push(o);
            si += i;
            so += o;
        }
    }
    for k in 0..inner.len() {
        inner[k] = inner[k] / max(si, 1e-12);
        outer[k] = outer[k] / max(so, 1e-12);
    }
    inner.append(outer);
    #{ radius: r, weights: inner }
}

// The spectral stage's discs: the inner disc, and the annulus between it
// and the outer radius, averaged.
fn discs(p) {
    let ri = min(max(p.inner_radius, 1.0), 10.0);
    let ra = min(ri * min(max(p.ratio, 1.5), 5.0), 32.0 - 1.5);
    [#{ ra: ri, rb: 0.0 }, #{ ra: ra, rb: ri, annulus: true }]
}
