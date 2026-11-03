// The Eden growth model: a cluster that grows by occupying, at random,
// empty sites on its boundary.
//
// The Rhai twin of the built-in `eden` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Channels: x = occupied (1) or empty (0), z = the step a site was
// occupied, y and w spare.

const PARAMS = #{
    p_grow: #{
        value: 0.3, min: 0.01, max: 1.0, caption: "Growth probability",
        hint: "Chance an eligible empty site is occupied each step. Lower values are closer to the sequential Eden process and take proportionally longer; at 1.0 the cluster is a diamond, because every front site fires at once.",
    },
};
const INFO = #{
    title: "Eden Growth",
    id: "eden",
    description: "A cluster grown one site at a time into its neighbourhood. Compact, with a rough interface — the growth rings are the subject.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 250,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "cluster",
            title: "Cluster",
            params: #{ p_grow: 0.3 },
            steps: 250,
            boundary: "periodic",
            init: "center",
            coloring: "age",
            coloring_params: #{ window: 250.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "rough_front",
            title: "Rough front",
            params: #{ p_grow: 0.3 },
            steps: 500,
            boundary: "zero",
            init: "line",
            coloring: "age",
            coloring_params: #{ window: 500.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
    ],
};

fn cell_step(cell, p, ctx) {
    if cell.x > 0.5 {
        return cell;
    }
    // A boundary site: an occupied von Neumann neighbour.
    let n = read(0, -1).x + read(0, 1).x + read(-1, 0).x + read(1, 0).x;
    if n < 0.5 {
        return cell;
    }
    if rand(0xd1) >= p.p_grow {
        return cell;
    }
    vec4(1.0, 0.0, ctx.step, 0.0)
}

// The seed is the init shape.
fn cell_seed(inside, noise) {
    vec4(if inside >= 0.5 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0)
}
