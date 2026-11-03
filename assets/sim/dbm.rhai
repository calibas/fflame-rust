// The dielectric breakdown model (Niemeyer, Pietronero and Wiesmann):
// a discharge growing where the electric field is strongest -- Lichtenberg
// figures, lightning.
//
// The Rhai twin of the built-in `dbm` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Three passes, the second repeated `relax` times a step (`INFO.repeat`):
// growth, then Laplace's equation relaxed between the electrode at 1 and
// the pattern at 0 (or its surface tension), then each candidate site's
// growth weight -- or, with one site a step, its exponential race time,
// whose smallest the field's minimum picks (`ctx.field_min`).
// Channels: x = the race time or the weight, y = the potential, z = the
// step a site joined, w = in the pattern.

// The electrode: the grid's rim, or the channel's top row.

const PARAMS = #{
    eta: #{
        value: 1.0, min: 0.0, max: 4.0, caption: "Field exponent η",
        hint: "How sharply growth prefers a strong field. THE DIMENSION DEPENDS ON IT and the paper tabulates it: 2 at η = 0, 1.89 at 0.5, 1.75 at 1, about 1.6 at 2. η = 1 is growth proportional to the field — the realistic case for a discharge, and the one whose pattern and dimension match both a photographed Lichtenberg figure and DLA. Higher makes the structure more nearly linear.",
    },
    relax: #{
        value: 10.0, min: 1.0, max: 200.0, caption: "Relaxation sweeps",
        hint: "How many times Laplace's equation is iterated between one growth and the next. The paper's own note: 'typically good convergence is obtained with a number of iterations between 5 and 50'. It is warm-started from the last solution, and the cost is proportional: measured at 1080p, 1.8 ms a step at 5 sweeps, 5.5 at 20, 12.8 at 50 — while the pattern's dimension at η = 1 is the same from 3 sweeps to 150 (1.69 to 1.71, within sample noise). Ten is inside the paper's range and twice as fast as twenty.",
    },
    surface_tension: #{
        value: 0.0, min: 0.0, max: 0.1, caption: "Tip penalty d₀",
        hint: "Holds the pattern at −d₀ × curvature instead of at 0, so a protruding tip sits at a lower potential and grows more slowly. It thins and straightens the pattern. IT IS NOT SAFFMAN–TAYLOR: it was added as the lattice analogue of their surface-tension boundary condition and measurably does not reproduce their finger — see the model's notes. Above about 0.05 it stops growth altogether. Zero is the dielectric breakdown model as published.",
    },
    selection: #{
        value: 0, choices: ["One site (race)", "All sites (rate)"], caption: "Growth",
        hint: "How many sites join per step. One site is the paper's rule exactly — a single bond added per step, chosen among all candidates in proportion to the field — and it is what a branching DISCHARGE is. All sites advances the whole interface at once, each candidate independently at a rate proportional to the field, which is what a moving FLUID interface is. They are different processes, not an approximation of one another.",
    },
    rate: #{
        value: 0.05, min: 0.001, max: 1.0, caption: "Growth rate",
        hint: "In All-sites mode, the chance per step that a candidate with unit field joins. Small keeps the interface smooth and costs steps; large advances it in coarse jumps. Ignored in One-site mode.",
    },
    electrode: #{
        value: 0, choices: ["Radial", "Channel"], caption: "Far electrode",
        hint: "Where the potential is held at 1. Radial is the paper's figure 2 — a circle at large enough distance, with the discharge starting from a point at the centre. Channel is one plate along an edge, for a discharge from a line — seed it with one.",
    },
};
const INFO = #{
    title: "Dielectric Breakdown",
    id: "dbm",
    description: "A discharge that branches into whichever gap has the strongest field, one site at a time, exactly as the paper draws it. Lightning and Lichtenberg figures.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 5000,
    state: "discrete",
    time_step: false,
    repeat: #{ pass: 2, param: "relax" },
    presets: [
        #{
            name: "lichtenberg",
            title: "Lichtenberg figure",
            params: #{ eta: 1.0, relax: 10.0, surface_tension: 0.0, selection: 0.0, rate: 0.05, electrode: 0.0 },
            steps: 5000,
            boundary: "clamp",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
            matte: #{ channel: "w" },
        },
        #{
            name: "dense",
            title: "Dense (η = 0)",
            params: #{ eta: 0.0, relax: 10.0, surface_tension: 0.0, selection: 0.0, rate: 0.05, electrode: 0.0 },
            steps: 5000,
            boundary: "clamp",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
            matte: #{ channel: "w" },
        },
        #{
            name: "sparse",
            title: "Sparse (η = 2)",
            params: #{ eta: 2.0, relax: 10.0, surface_tension: 0.0, selection: 0.0, rate: 0.05, electrode: 0.0 },
            steps: 5000,
            boundary: "clamp",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 3.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
            matte: #{ channel: "w" },
        },
    ],
};

fn electrode(x, y, w, h, channel) {
    if channel {
        y <= 0
    } else {
        let r = min(to_float(w), to_float(h)) * 0.48;
        (complex(to_float(x), to_float(y)) - complex(to_float(w) * 0.5, to_float(h) * 0.5)).abs() >= r
    }
}

// The pattern's sites around the cell, von Neumann.
fn bonds() {
    read(1, 0).w + read(-1, 0).w + read(0, 1).w + read(0, -1).w
}

// The pattern's curvature at a site of it, from its Moore neighbours.
fn curvature() {
    let n = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx != 0 || dy != 0 {
                n = n + read(dx, dy).w;
            }
        }
    }
    if n > 7.5 { 0.0 } else { (5.0 - n) * (1.0 / 3.0) }
}

// A candidate's growth weight: its bonds to the pattern times the
// potential to the power eta.
fn weight(cell, at_electrode, eta) {
    let b = bonds();
    let phi = max(cell.y, 0.0);
    if cell.w > 0.5 || at_electrode || b < 0.5 || phi <= 0.0 { 0.0 } else { b * phi ** eta }
}

fn cell_step(cell, p, ctx) {
    let joins = false;
    if cell.w < 0.5 {
        if p.selection == 1 {
            let x = to_int(ctx.cell.re);
            let y = to_int(ctx.cell.im);
            let w = weight(cell, electrode(x, y, ctx.width, ctx.height, p.electrode == 1), p.eta);
            let peak = max(ctx.field_max, 0.00000000000000000001);
            joins = w > 0.0 && rand(0x7b) < p.rate * w / peak;
        } else {
            let best = ctx.field_min;
            joins = best < 1e30 * 0.5 && cell.x == best;
        }
    }
    if joins {
        return vec4(1e30, 0.0, to_float(ctx.step), 1.0);
    }
    vec4(1e30, cell.y, cell.z, cell.w)
}

fn cell_step2(cell, p, ctx) {
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    if electrode(x, y, ctx.width, ctx.height, p.electrode == 1) {
        return vec4(cell.x, 1.0, cell.z, cell.w);
    }
    if cell.w > 0.5 {
        return vec4(cell.x, -p.surface_tension * curvature(), cell.z, cell.w);
    }
    vec4(cell.x, 0.25 * (read(1, 0).y + read(-1, 0).y + read(0, 1).y + read(0, -1).y), cell.z, cell.w)
}

fn cell_step3(cell, p, ctx) {
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    let w = weight(cell, electrode(x, y, ctx.width, ctx.height, p.electrode == 1), p.eta);
    if p.selection == 1 {
        return vec4(w, cell.y, cell.z, cell.w);
    }
    if w <= 0.0 {
        return vec4(1e30, cell.y, cell.z, cell.w);
    }
    // The race: an exponential time at the weight's rate.
    let u = max(rand(0xdb), 0.0000001);
    vec4(-ln(u) / max(w, 1e-30), cell.y, cell.z, cell.w)
}

// The pattern is the init shape, at potential 0; the rest at 1.
fn cell_seed(inside, noise) {
    let pat = if inside >= 0.5 { 1.0 } else { 0.0 };
    vec4(1e30, if pat > 0.5 { 0.0 } else { 1.0 }, 0.0, pat)
}
