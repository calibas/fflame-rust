// Site percolation: open sites at random, clusters labelled by the
// smallest index they reach.
//
// The Rhai twin of the built-in `percolation` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Each open cell carries a label, its index to start with, and takes
// the smallest of its open neighbours' and of the cell its label names
// (pointer jumping: a cluster's labels converge in a logarithmic number
// of steps rather than its diameter). Channels: x = the label, y = open,
// z = the step the label last changed, w spare.

const PARAMS = #{
    p_open: #{
        value: 0.592746, min: 0.3, max: 0.9, caption: "Open probability",
        hint: "Fraction of sites that are open. The square-lattice site threshold is 0.592746: below it every cluster is finite, above it one spans the grid, and at it the spanning cluster is fractal at every scale.",
    },
};
const INFO = #{
    title: "Percolation",
    id: "percolation",
    description: "Random open sites, coloured by which connected cluster they belong to. At the critical threshold the spanning cluster is fractal.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 400,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "critical",
            title: "Critical (p_c)",
            params: #{ p_open: 0.592746 },
            steps: 400,
            boundary: "zero",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "label",
            coloring_params: #{ channel: 0.0, mask_channel: 0.0 },
        },
        #{
            name: "subcritical",
            title: "Below threshold",
            params: #{ p_open: 0.45 },
            steps: 400,
            boundary: "zero",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "label",
            coloring_params: #{ channel: 0.0, mask_channel: 0.0 },
        },
        #{
            name: "supercritical",
            title: "Above threshold",
            params: #{ p_open: 0.75 },
            steps: 400,
            boundary: "zero",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "label",
            coloring_params: #{ channel: 0.0, mask_channel: 0.0 },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    if cell.y < 0.5 {
        return cell;
    }
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    let best = cell.x;
    let n0 = read(-1, 0);
    let n1 = read(1, 0);
    let n2 = read(0, -1);
    let n3 = read(0, 1);
    if n0.y > 0.5 {
        best = min(best, n0.x);
    }
    if n1.y > 0.5 {
        best = min(best, n1.x);
    }
    if n2.y > 0.5 {
        best = min(best, n2.x);
    }
    if n3.y > 0.5 {
        best = min(best, n3.x);
    }
    // The cell the label names: a label is an index, exact below 2^24
    // cells, as the label itself is.
    let li = to_int(clamp(best, 0.0, to_float(ctx.width * ctx.height - 1)));
    let lx = li % ctx.width;
    let ly = to_int(to_float(li - lx) / to_float(ctx.width));
    let root = read(lx - x, ly - y);
    if root.y > 0.5 {
        best = min(best, root.x);
    }
    vec4(best, cell.y, if best != cell.x { ctx.step } else { cell.z }, 0.0)
}

// Open at random, each cell labelled by its index.
fn cell_seed(inside, noise, p, ctx) {
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    let open = if rand(0xf1) < p.p_open { 1.0 } else { 0.0 };
    vec4(to_float(y * ctx.width + x), open, 0.0, 0.0)
}
