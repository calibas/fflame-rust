// The cyclic cellular automaton (Griffeath): N states in a ring, each
// cell advanced by enough neighbours one state ahead of it.
//
// The Rhai twin of the built-in `cyclic_ca` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// Channels: x = the state, z = the step it last advanced, y and w spare.

// `v` mod `n`, a whole number.

const PARAMS = #{
    states: #{
        value: 14.0, min: 3.0, max: 24.0, caption: "States (N)",
        hint: "Length of the cycle. More states means a longer path back around, so spirals have more arms and take longer to form.",
    },
    range: #{
        value: 1.0, min: 1.0, max: 5.0, caption: "Range (R)",
        hint: "Neighbourhood radius. Cost grows as R²: range 5 is 121 taps a cell.",
    },
    threshold: #{
        value: 1.0, min: 1.0, max: 25.0, caption: "Threshold (T)",
        hint: "How many neighbours must already hold the next state. Written R/T/N in the literature.",
    },
    neighbourhood: #{
        value: 0, choices: ["Von Neumann", "Moore"], caption: "Neighbourhood",
        hint: "Von Neumann counts the diamond (|dx| + |dy| ≤ R) and gives 45° fronts; Moore counts the square.",
    },
};
const INFO = #{
    title: "Cyclic CA",
    id: "cyclic_ca",
    description: "States chase each other around a cycle: debris coarsens into droplets and then into spirals whose cores never settle.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 300,
    state: "discrete",
    time_step: false,
    settles: false,
    slow: ["range"],
    presets: [
        #{
            name: "spirals_1_1_14",
            title: "1/1/14 spirals",
            params: #{ states: 14.0, range: 1.0, threshold: 1.0, neighbourhood: 0.0 },
            steps: 300,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.0714286, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "moore_1_3_3",
            title: "1/3/3 Moore",
            params: #{ states: 3.0, range: 1.0, threshold: 3.0, neighbourhood: 1.0 },
            steps: 60,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.3333333, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn fract_state(v, n) {
    floor(v - floor(v / n) * n)
}

// 1 where the neighbour `dx`, `dy` away holds `want`.
fn cyc_hit(dx, dy, want) {
    if floor(read(dx, dy).x) == want { 1.0 } else { 0.0 }
}

fn cell_step(cell, p, ctx) {
    let n_states = max(floor(p.states), 2.0);
    let r = to_int(clamp(floor(p.range), 1.0, 5.0));
    let thresh = max(floor(p.threshold), 1.0);
    let moore = p.neighbourhood == 1;
    let cur = floor(cell.x);
    let want = fract_state(cur + 1.0, n_states);
    let count = 0.0;
    if r == 1 {
        count = cyc_hit(0, -1, want) + cyc_hit(0, 1, want) + cyc_hit(-1, 0, want) + cyc_hit(1, 0, want);
        if moore {
            count = count + cyc_hit(-1, -1, want) + cyc_hit(1, -1, want) + cyc_hit(-1, 1, want) + cyc_hit(1, 1, want);
        }
    } else {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if !moore && abs(dx) + abs(dy) > r {
                    continue;
                }
                count = count + cyc_hit(dx, dy, want);
            }
        }
    }
    let advance = count >= thresh;
    vec4(if advance { want } else { cur }, 0.0, if advance { ctx.step } else { cell.z }, 0.0)
}

// Every state at random.
fn cell_seed(inside, noise, p) {
    let n_states = max(floor(p.states), 2.0);
    vec4(floor(rand(0xa1) * n_states), 0.0, 0.0, 0.0)
}
