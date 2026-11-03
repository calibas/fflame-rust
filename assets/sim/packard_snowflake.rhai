// Packard's snowflakes: a hexagonal automaton where an empty site freezes
// when the number of frozen neighbours is one the rule allows.
//
// The Rhai twin of the built-in `packard_snowflake` model
// (src/sim/models.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters, presets and working values, and the same field.
//
// The hexagonal lattice on the square grid: odd rows shifted half a
// cell right. Channels: x = frozen, z = the step it froze, y and w
// spare.

// WGSL's `round`: half to even, as the built-in reads its mask.

const PARAMS = #{
    rule_mask: #{
        value: 2.0, min: 1.0, max: 126.0, caption: "Freeze on",
        hint: "A 6-bit mask: bit n set means a cell freezes when exactly n of its six neighbours are frozen. 2 is Packard's rule 1 (one neighbour), 10 is rule 13, 26 is rule 134.",
    },
};
const INFO = #{
    title: "Packard Snowflake",
    id: "packard_snowflake",
    description: "Hexagonal solidification: a vacant cell freezes on a chosen count of frozen neighbours. Plates, branches and dendrites.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 125,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "rule_1",
            title: "Rule 1 (plate)",
            params: #{ rule_mask: 2.0 },
            steps: 125,
            boundary: "zero",
            init: "center",
            coloring: "age",
            coloring_params: #{ window: 125.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "rule_13",
            title: "Rule 13",
            params: #{ rule_mask: 10.0 },
            steps: 125,
            boundary: "zero",
            init: "center",
            coloring: "age",
            coloring_params: #{ window: 125.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
        #{
            name: "rule_134",
            title: "Rule 134",
            params: #{ rule_mask: 26.0 },
            steps: 125,
            boundary: "zero",
            init: "center",
            coloring: "age",
            coloring_params: #{ window: 125.0, invert: 1.0, wrap: 0.0 },
            matte: #{ channel: "x" },
        },
    ],
};

fn round_even(x) {
    let r = floor(x + 0.5);
    if r - x == 0.5 && r % 2.0 != 0.0 { r - 1.0 } else { r }
}

// The frozen count of the six hexagonal neighbours.
fn hex_frozen_count(odd) {
    let n = read(-1, 0).x + read(1, 0).x;
    if odd {
        n = n + read(0, -1).x + read(1, -1).x + read(0, 1).x + read(1, 1).x;
    } else {
        n = n + read(0, -1).x + read(-1, -1).x + read(0, 1).x + read(-1, 1).x;
    }
    n
}

fn cell_step(cell, p, ctx) {
    if cell.x > 0.5 {
        return cell;
    }
    let y = to_int(ctx.cell.im);
    let count = to_int(round_even(hex_frozen_count((y & 1) == 1)));
    if count < 1 || count > 6 {
        return cell;
    }
    // Bit n of the mask allows freezing next to n frozen sites.
    let mask = to_int(clamp(round_even(p.rule_mask), 0.0, 126.0));
    if ((mask >> count) & 1) == 0 {
        return cell;
    }
    vec4(1.0, 0.0, ctx.step, 0.0)
}

// Frozen inside the init shape.
fn cell_seed(inside, noise) {
    vec4(if inside >= 0.5 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0)
}
