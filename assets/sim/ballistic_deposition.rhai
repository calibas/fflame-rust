// Ballistic deposition: particles dropped on a line, sticking where they
// first touch -- the interface the KPZ equation describes.
//
// The Rhai twin of the built-in `ballistic_deposition` model
// (src/sim/models.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters, presets and working values, and the same field.
//
// Row 0 holds each column's height in `.y`; every cell of a column
// reads it and fills itself when the column has grown past it.
// Channels: x = filled, y = the column's height (row 0), z = the step a
// cell filled, w spare.

const PARAMS = #{
    p_drop: #{
        value: 0.5, min: 0.01, max: 1.0, caption: "Drop probability",
        hint: "Chance each column receives a particle per step. Lower values are closer to the sequential process, where one particle lands at a time.",
    },
    sideways: #{
        value: 1, choices: ["Vertical only", "Lateral (ballistic)"], caption: "Sticking",
        hint: "Lateral sticking is what makes deposition BALLISTIC: a particle catches on a taller neighbour and leaves a void. Off, the columns never interact and the surface is uncorrelated.",
    },
};
const INFO = #{
    title: "Ballistic Deposition",
    id: "ballistic_deposition",
    description: "Particles fall and stick where they first touch. Lateral sticking builds overhangs and a correlated surface; without it the columns are independent.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 360,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "ballistic",
            title: "Ballistic",
            params: #{ p_drop: 0.5, sideways: 1.0 },
            steps: 360,
            boundary: "periodic",
            init: "center",
            coloring: "age",
            coloring_params: #{ window: 360.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "random_deposition",
            title: "Random deposition",
            params: #{ p_drop: 0.5, sideways: 0.0 },
            steps: 450,
            boundary: "periodic",
            init: "center",
            coloring: "age",
            coloring_params: #{ window: 450.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    // The heights here and either side, read from row 0.
    let h_l = read(-1, -y).y;
    let h_c = read(0, -y).y;
    let h_r = read(1, -y).y;
    // The framework's `sim_rand` at the column's top cell, (x, 0), so
    // every cell of the column agrees on the drop: the cell's index, the
    // seed and the layer, the stream, the step.
    let h = pcg(x ^ ctx.seed_lo ^ (ctx.layer * 0x9E3779B9));
    h = pcg(h ^ ctx.seed_hi ^ 0xe1);
    h = pcg(h ^ ctx.step);
    let hit = to_float(h >> 8) * (1.0 / 16777216.0) < p.p_drop;
    let h_new = h_c;
    if hit {
        // Sticking to a neighbour's side, or falling straight down.
        if p.sideways == 1 {
            h_new = max(max(h_l, h_r), h_c + 1.0);
        } else {
            h_new = h_c + 1.0;
        }
    }
    h_new = min(h_new, to_float(ctx.height));
    let filled = to_float(ctx.height - y) <= h_new;
    let was = cell.x > 0.5;
    let age = if filled && !was { ctx.step } else { cell.z };
    vec4(if filled { 1.0 } else { 0.0 }, if y == 0 { h_new } else { 0.0 }, age, 0.0)
}

// An empty field.
fn cell_seed(inside, noise) {
    vec4(0.0, 0.0, 0.0, 0.0)
}
