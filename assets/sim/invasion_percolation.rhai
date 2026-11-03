// Invasion percolation (Wilkinson and Willemsen): a fluid invading the
// weakest throats next to it, the threshold rising with the step.
//
// The Rhai twin of the built-in `invasion_percolation` model
// (src/sim/models.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters, presets and working values, and the same field.
//
// Channels: x = invaded, y = the site's random strength, z = the step it
// was invaded, w spare.

const PARAMS = #{
    dp: #{
        value: 0.0005, min: 2e-5, max: 0.05, caption: "Threshold rise per step",
        hint: "How fast the invasion threshold climbs. This is a RATE, not a shape — the final cluster is the same set however slowly you reach the ceiling — but it decides what Age draws. Slow, and the front is never the constraint, so Age records the order sites were invaded in, which is the sequential model's own picture. Fast, and everything is eligible at once and Age just measures distance from the seed.",
    },
    p_max: #{
        value: 0.6, min: 0.45, max: 0.8, caption: "Threshold ceiling",
        hint: "Where the threshold stops climbing, letting the remaining steps finish the front. THE INTERESTING WINDOW IS NARROW: the infinite lattice's site threshold is 0.5927, and measured on a 256² grid the cluster spans between 0.600 and 0.610. Below that it is ramified and full of fjords; a few hundredths above, it is half the grid and reads as a solid with holes. Far below, the invasion dies at once.",
    },
};
const INFO = #{
    title: "Invasion Percolation",
    id: "invasion_percolation",
    description: "A cluster that spreads into whichever sites resist it least, as a rising threshold. At the percolation threshold it is a critical cluster, all fjords and dangling ends.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 2000,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "front",
            title: "Invading front",
            params: #{ dp: 0.0005, p_max: 0.6 },
            steps: 2000,
            boundary: "zero",
            init: "line",
            coloring: "age",
            coloring_params: #{ window: 2000.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "spanning",
            title: "Spanning cluster",
            params: #{ dp: 0.0005, p_max: 0.635 },
            steps: 2000,
            boundary: "zero",
            init: "line",
            coloring: "age",
            coloring_params: #{ window: 2000.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    if cell.x > 0.5 {
        return cell;
    }
    // The threshold rises by `dp` a step, to `p_max`.
    let pt = min(to_float(ctx.step + 1) * p.dp, p.p_max);
    if cell.y >= pt {
        return cell;
    }
    let n = read(1, 0).x + read(-1, 0).x + read(0, 1).x + read(0, -1).x;
    if n < 0.5 {
        return cell;
    }
    vec4(1.0, cell.y, ctx.step, 0.0)
}

// Invaded inside the init shape; every site's strength at random.
fn cell_seed(inside, noise) {
    let r = rand(0x1a5);
    vec4(if inside >= 0.5 { 1.0 } else { 0.0 }, r, 0.0, 0.0)
}
