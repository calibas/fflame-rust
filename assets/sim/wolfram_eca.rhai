// Wolfram's elementary cellular automata: one row a step, each cell from
// the three above it by the rule's bits.
//
// The Rhai twin of the built-in `wolfram_eca` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Row t + 1 is written at step t; the rows above it are the history, so
// the picture is the space-time diagram. Channels: x = the cell (0 or
// 1), z = its row, y and w spare.

// WGSL's `round`: half to even, as the built-in reads its rule (the
// language's is Rhai's, half away from zero).

const PARAMS = #{
    rule: #{
        value: 90.0, min: 0.0, max: 255.0, caption: "Rule",
        hint: "The 8-bit rule number: bit (4·left + 2·self + right) gives the next state. 90 draws Sierpinski's triangle, 30 is chaotic, 110 is Turing-complete, 184 models traffic.",
    },
};
const INFO = #{
    title: "Wolfram ECA",
    id: "wolfram_eca",
    description: "One-dimensional binary automata drawn as space-time: rule 90 is Sierpinski's triangle, rule 30 is chaotic, rule 110 is Turing-complete.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 256,
    state: "discrete",
    time_step: false,
    presets: [
        #{
            name: "rule_90",
            title: "Rule 90 (Sierpinski)",
            params: #{ rule: 90.0 },
            steps: 256,
            boundary: "periodic",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "rule_30",
            title: "Rule 30 (chaotic)",
            params: #{ rule: 30.0 },
            steps: 256,
            boundary: "periodic",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "rule_110",
            title: "Rule 110 (universal)",
            params: #{ rule: 110.0 },
            steps: 256,
            boundary: "periodic",
            init: "center",
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 1.0, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn round_even(x) {
    let r = floor(x + 0.5);
    if r - x == 0.5 && r % 2.0 != 0.0 { r - 1.0 } else { r }
}

fn cell_step(cell, p, ctx) {
    let y = to_int(ctx.cell.im);
    if y != ctx.step + 1 {
        return cell;
    }
    let l = read(-1, -1).x;
    let c = read(0, -1).x;
    let r = read(1, -1).x;
    let idx = to_int(4.0 * l + 2.0 * c + r);
    let rule = to_int(clamp(round_even(p.rule), 0.0, 255.0));
    let bit = (rule >> idx) & 1;
    vec4(to_float(bit), 0.0, to_float(y), 0.0)
}

// Row 0 is the init shape along the grid's middle row.
fn cell_seed(inside, noise, p, ctx) {
    let x = to_int(ctx.cell.re);
    let y = to_int(ctx.cell.im);
    if y != 0 {
        return vec4(0.0, 0.0, 0.0, 0.0);
    }
    let m = init_at(x, ctx.height >> 1);
    vec4(if m >= 0.5 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0)
}
