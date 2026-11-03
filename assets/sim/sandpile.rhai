// The abelian sandpile (Bak, Tang and Wiesenfeld): a pile toppling
// grains to its neighbours until every site is stable.
//
// The Rhai twin of the built-in `sandpile` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Every site topples as often as it can at once (its height over the
// threshold, floored): the abelian property makes the order irrelevant.
// Channels: x = the grains, y = the times it toppled, z = the step it
// last toppled, w spare.

// How many times a site of height `h` topples.

const PARAMS = #{
    grains_log2: #{
        value: 16.0, min: 10.0, max: 24.0, caption: "Grains (2ⁿ)",
        hint: "How many grains land on the seed, as a power of two. THE COST IS QUADRATIC IN THE PICTURE'S RADIUS: measured, 2¹⁶ stabilises in 12,837 rounds and 2²⁰ in 190,006, so raising this by one costs roughly twice the steps and needs a grid √2 wider. Raise Steps with it — a pile that runs out of steps is simply unfinished. 24 is the ceiling because the field is f32, which counts exactly to 2²⁴ and no further.",
    },
    neighbourhood: #{
        value: 0, choices: ["Von Neumann (4)", "Moore (8)"], caption: "Neighbourhood",
        hint: "The graph the pile topples on. Von Neumann is the classic square lattice: topple at 4, one grain to each edge neighbour, and stable heights are 0–3. Moore is the same rule on the 8-neighbour graph — topple at 8, stable heights 0–7 — so the same mass settles denser and over a smaller, rounder shape (measured at 2¹⁶: 133 cells across in 4,652 rounds, against 189 in 12,837). Colouring by height wants a scale of 1/7 rather than 1/3.",
    },
};
const INFO = #{
    title: "Abelian Sandpile",
    id: "sandpile",
    description: "Grains dropped on one site and toppled until stable. The four-colour height field is a fractal limit shape; the edges are sinks.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 12837,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "pile",
            title: "Pile",
            params: #{ grains_log2: 16.0, neighbourhood: 0.0 },
            steps: 12837,
            boundary: "zero",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.3333333, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "moore",
            title: "Moore pile",
            params: #{ grains_log2: 16.0, neighbourhood: 1.0 },
            steps: 4652,
            boundary: "zero",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.1428571, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn sand_fires(h, thresh) {
    floor(h / thresh)
}

fn cell_step(cell, p, ctx) {
    let moore = p.neighbourhood == 1;
    let thresh = if moore { 8.0 } else { 4.0 };
    let fired = sand_fires(cell.x, thresh);
    let got = sand_fires(read(1, 0).x, thresh) + sand_fires(read(-1, 0).x, thresh) + sand_fires(read(0, 1).x, thresh)
        + sand_fires(read(0, -1).x, thresh);
    if moore {
        got = got + sand_fires(read(1, 1).x, thresh) + sand_fires(read(1, -1).x, thresh) + sand_fires(read(-1, 1).x, thresh)
            + sand_fires(read(-1, -1).x, thresh);
    }
    let h = cell.x - fired * thresh + got;
    vec4(h, cell.y + fired, if fired > 0.0 { ctx.step } else { cell.z }, 0.0)
}

// 2^grains_log2 grains inside the init shape: a whole power of two,
// doubled up exactly.
fn cell_seed(inside, noise, p) {
    let k = to_int(floor(clamp(p.grains_log2, 1.0, 24.0)));
    let n = 1.0;
    for i in 0..k {
        n = n * 2.0;
    }
    n = floor(n + 0.5);
    vec4(if inside >= 0.5 { n } else { 0.0 }, 0.0, 0.0, 0.0)
}
