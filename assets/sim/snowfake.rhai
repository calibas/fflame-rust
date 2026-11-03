// Gravner and Griffeath's snowfakes (2008): a hexagonal crystal growing
// by attachment from a diffusing vapour, with a boundary layer and a
// melting rule -- snowflakes, plates and dendrites from eight numbers.
//
// The Rhai twin of the built-in `snowfake` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// The hexagonal lattice on the square grid: odd rows shifted half a
// cell right. Two passes: diffusion and freezing, then attachment and
// melting. Channels: x = attached (a), y = the boundary mass (b),
// z = the crystal mass (c), w = the vapour (d).

const PARAMS = #{
    rho: #{
        value: 0.65, min: 0.1, max: 1.0, caption: "Vapour density ρ",
        hint: "How much vapour the cloud starts with, at every cell that is not the seed. The paper's crystals use 0.35 to 0.8, and it is the easiest parameter to read: more vapour grows faster and pushes the form from plate to sectored plate to dendrite. It is read by the SEED, so a                       change to it shows on the next Reset rather than in the running                       crystal.",
    },
    beta: #{
        value: 1.75, min: 1.0, max: 4.0, caption: "Anisotropy β",
        hint: "How much boundary mass a site with one or two attached neighbours needs before it joins — a tip or a flat face, where attachment is hard. A valley's threshold is 1, so it is the EXCESS OVER 1 that makes the crystal faceted. Raising it delays the first instability, grows the central plate larger, and moves the form from fern to dendrite to sectored plate. The paper uses 1.05 to 3.2.",
    },
    alpha: #{
        value: 0.2, min: 0.0, max: 1.0, caption: "Knife-edge mass α",
        hint: "The reduced boundary-mass threshold a concave site may attach at once the vapour around it has run out — the knife-edge instability, by which a very thin plate spreads on almost no vapour. It is what fills the regions between the six main branches long after they have passed. Below the valley threshold of 1, and the paper's figures use 0 to 0.6.",
    },
    theta: #{
        value: 0.026, min: 0.0, max: 0.5, caption: "Knife-edge vapour θ",
        hint: "How depleted the vapour in a site's neighbourhood must be before that reduced threshold applies. It is compared against the SUM over the site and its six neighbours, which far from the crystal is about 7ρ, so the paper's values of 0.003 to 0.11 mean 'almost nothing left'. Set it too high and the crystal fills into a featureless plate, because every concavity qualifies as soon as it forms.",
    },
    kappa: #{
        value: 0.15, min: 0.0, max: 0.5, caption: "Freezing κ",
        hint: "The proportion of vapour arriving at the boundary that freezes straight to ice; the rest becomes quasi-liquid and is what attachment weighs. Ice deposited this way waits rather than attaching, so raising κ starves the fastest tips of boundary mass and — up to a point — encourages side branching. The paper's figures span 0 to 0.15.",
    },
    mu: #{
        value: 0.015, min: 0.0, max: 0.3, caption: "Melting μ",
        hint: "The proportion of boundary mass that returns to vapour each step. It opposes freezing, and raising it promotes faceting: the paper's sequence from 0.04 to 0.09 turns a stellar dendrite with parabolic tips into a stellar plate with hexagonal ones.",
    },
    gamma: #{
        value: 0.0001, min: 0.0, max: 0.01, caption: "Sublimation γ",
        hint: "The proportion of unattached ICE at the boundary that returns to vapour each step. It acts like melting but is typically far smaller — the paper's figures use 0.000001 to 0.01 — and it is included for completeness rather than for its effect.",
    },
    sigma: #{
        value: 0.0, min: 0.0, max: 0.1, caption: "Vapour noise σ",
        hint: "Multiplies the vapour at every site by 1 ± σ each step, each sign equally likely. The paper's own parameter studies are deterministic (σ = 0) and complex without it; a little noise is how it studies how much of a real snowflake's asymmetry chance accounts for.",
    },
};
const INFO = #{
    title: "Gravner–Griffeath Snowfake",
    id: "snowfake",
    description: "A planar snow crystal grown from vapour on a hexagonal lattice: diffusion, a quasi-liquid boundary layer, and attachment that is harder on a flat face than in a valley. Plates, stars, dendrites and sectored plates.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 6000,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "primitive",
            title: "Primitive (ridges)",
            params: #{ rho: 0.58, beta: 3.2, alpha: 0.0, theta: 0.0, kappa: 0.0, mu: 0.0, gamma: 0.0, sigma: 0.0 },
            steps: 24000,
            boundary: "clamp",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 2.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "simple_star",
            title: "Simple star",
            params: #{ rho: 0.65, beta: 1.75, alpha: 0.2, theta: 0.026, kappa: 0.15, mu: 0.015, gamma: 0.0001, sigma: 0.0 },
            steps: 12000,
            boundary: "clamp",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 2.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "plate_ends",
            title: "Stellar plate ends",
            params: #{ rho: 0.36, beta: 1.09, alpha: 0.01, theta: 0.0745, kappa: 0.0001, mu: 0.14, gamma: 1e-5, sigma: 0.0 },
            steps: 40000,
            boundary: "clamp",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 2.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "dendrite_ends",
            title: "Plate with dendrite ends",
            params: #{ rho: 0.38, beta: 1.06, alpha: 0.35, theta: 0.112, kappa: 0.001, mu: 0.14, gamma: 0.0006, sigma: 0.0 },
            steps: 20000,
            boundary: "clamp",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 2.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    if cell.x > 0.5 {
        return cell;
    }
    let y = to_int(ctx.cell.im);
    let k = if (y & 1) == 1 { 0 } else { -1 };
    // Diffusion over the six neighbours and the cell: an attached
    // neighbour reflects the cell's own vapour.
    let attached = 0.0;
    let sum = cell.w;
    for i in 0..6 {
        let dx = k + 1;
        let dy = 1;
        if i == 0 {
            dx = -1;
            dy = 0;
        } else if i == 1 {
            dx = 1;
            dy = 0;
        } else if i == 2 {
            dx = k;
            dy = -1;
        } else if i == 3 {
            dx = k + 1;
            dy = -1;
        } else if i == 4 {
            dx = k;
            dy = 1;
        }
        let n = read(dx, dy);
        if n.x > 0.5 {
            attached = attached + 1.0;
            sum = sum + cell.w;
        } else {
            sum = sum + n.w;
        }
    }
    let d = sum / 7.0;
    let b = cell.y;
    let c = cell.z;
    // Freezing at the boundary: kappa of the vapour to the crystal, the
    // rest to the boundary layer.
    if attached > 0.5 {
        b = b + (1.0 - p.kappa) * d;
        c = c + p.kappa * d;
        d = 0.0;
    }
    vec4(0.0, b, c, d)
}

fn cell_step2(cell, p, ctx) {
    if cell.x > 0.5 {
        return cell;
    }
    let y = to_int(ctx.cell.im);
    let k = if (y & 1) == 1 { 0 } else { -1 };
    let n = 0.0;
    let dsum = cell.w;
    for i in 0..6 {
        let dx = k + 1;
        let dy = 1;
        if i == 0 {
            dx = -1;
            dy = 0;
        } else if i == 1 {
            dx = 1;
            dy = 0;
        } else if i == 2 {
            dx = k;
            dy = -1;
        } else if i == 3 {
            dx = k + 1;
            dy = -1;
        } else if i == 4 {
            dx = k;
            dy = 1;
        }
        let q = read(dx, dy);
        n = n + if q.x > 0.5 { 1.0 } else { 0.0 };
        dsum = dsum + q.w;
    }
    let ox = cell.x;
    let oy = cell.y;
    let oz = cell.z;
    let ow = cell.w;
    if n > 0.5 {
        // Attachment: by the attached neighbours' count and the boundary
        // mass; else melting, mu of the boundary and gamma of the crystal
        // back to vapour.
        let attach = false;
        if n > 3.5 {
            attach = true;
        } else if n > 2.5 {
            attach = cell.y >= 1.0 || (dsum < p.theta && cell.y >= p.alpha);
        } else {
            attach = cell.y >= p.beta;
        }
        if attach {
            ox = 1.0;
            oy = 0.0;
            oz = cell.y + cell.z;
            ow = 0.0;
        } else {
            ox = 0.0;
            oy = (1.0 - p.mu) * cell.y;
            oz = (1.0 - p.gamma) * cell.z;
            ow = cell.w + p.mu * cell.y + p.gamma * cell.z;
        }
    }
    // Noise in the vapour, sigma either way.
    if p.sigma > 0.0 && ox < 0.5 {
        ow = ow * (1.0 + if rand(0x5f) < 0.5 { p.sigma } else { -p.sigma });
    }
    vec4(ox, oy, oz, ow)
}

// The crystal's seed inside the init shape, the vapour at rho elsewhere.
fn cell_seed(inside, noise, p) {
    if inside >= 0.5 {
        return vec4(1.0, 0.0, 1.0, 0.0);
    }
    vec4(0.0, 0.0, 0.0, p.rho)
}
