// The hodgepodge machine (Gerhardt and Schuster), a cellular automaton
// of infection that draws Belousov-Zhabotinsky spirals.
//
// The Rhai twin of the built-in `hodgepodge` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// A cell's state is 0 (healthy) to q (ill); in between it is infected.
// Channels: x = the state, z = the step it last changed, y and w spare.

// 1 where `c` holds.

const PARAMS = #{
    states: #{
        value: 200.0, min: 4.0, max: 512.0, caption: "States (q)",
        hint: "How many levels of infection. Larger q gives smoother spiral arms and a longer refractory period.",
    },
    k1: #{
        value: 2.0, min: 1.0, max: 16.0, caption: "k₁",
        hint: "Divides a neighbour count when a healthy cell catches the infection — the ill count under Gerhardt–Schuster's rule, the infected count under Dewdney's, which is one of the three places the two differ. Larger values make infection harder to catch.",
    },
    k2: #{
        value: 3.0, min: 1.0, max: 16.0, caption: "k₂",
        hint: "Divides the other neighbour count — infected under Gerhardt–Schuster, ill under Dewdney. With k₁ it sets how readily waves nucleate.",
    },
    g: #{
        value: 25.0, min: 1.0, max: 200.0, caption: "g (rate)",
        hint: "How fast an infected cell progresses toward ill. Sets the wave speed and therefore the spiral pitch.",
    },
    variant: #{
        value: 0, choices: ["Gerhardt–Schuster", "Dewdney"], caption: "Rule",
        hint: "Which published rule to run. Gerhardt–Schuster is the 1989 paper's own: k₁ divides the ill count, and an infected cell averages the states of the infected cells around it. Dewdney's is the version that circulated afterwards — the counts swapped, and the average taken over every neighbour — and gives coarser, rounder scrolls.",
    },
};
const INFO = #{
    title: "Hodgepodge",
    id: "hodgepodge",
    description: "Belousov–Zhabotinsky as a cellular automaton: dense interlocking spirals and scrolls in an excitable integer medium.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 200,
    state: "discrete",
    time_step: false,
    settles: false,
    presets: [
        #{
            name: "spirals",
            title: "BZ spirals",
            params: #{ states: 200.0, k1: 2.0, k2: 3.0, g: 25.0, variant: 0.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.005, offset: 0.0, wrap: 0.0 },
        },
        #{
            name: "dewdney",
            title: "BZ spirals (Dewdney rule)",
            params: #{ states: 200.0, k1: 2.0, k2: 3.0, g: 70.0, variant: 1.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.005, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn hp_count(c) {
    if c { 1.0 } else { 0.0 }
}

// An infected neighbour's state, else 0.
fn hp_inf(n, q) {
    if n > 0.0 && n < q { n } else { 0.0 }
}

fn cell_step(cell, p, ctx) {
    let q = floor(p.states);
    let k1 = max(floor(p.k1), 1.0);
    let k2 = max(floor(p.k2), 1.0);
    let g = floor(p.g);
    let n0 = read(-1, -1).x;
    let n1 = read(0, -1).x;
    let n2 = read(1, -1).x;
    let n3 = read(-1, 0).x;
    let n4 = read(1, 0).x;
    let n5 = read(-1, 1).x;
    let n6 = read(0, 1).x;
    let n7 = read(1, 1).x;
    let total = cell.x + n0 + n1 + n2 + n3 + n4 + n5 + n6 + n7;
    let ill = hp_count(n0 >= q) + hp_count(n1 >= q) + hp_count(n2 >= q) + hp_count(n3 >= q)
        + hp_count(n4 >= q) + hp_count(n5 >= q) + hp_count(n6 >= q) + hp_count(n7 >= q);
    let nonzero = hp_count(n0 > 0.0) + hp_count(n1 > 0.0) + hp_count(n2 > 0.0) + hp_count(n3 > 0.0)
        + hp_count(n4 > 0.0) + hp_count(n5 > 0.0) + hp_count(n6 > 0.0) + hp_count(n7 > 0.0);
    let infected = nonzero - ill;
    let inf_sum = hp_inf(n0, q) + hp_inf(n1, q) + hp_inf(n2, q) + hp_inf(n3, q)
        + hp_inf(n4, q) + hp_inf(n5, q) + hp_inf(n6, q) + hp_inf(n7, q);
    // Gerhardt and Schuster's rules, or Dewdney's.
    let paper = p.variant == 0;
    let cur = cell.x;
    let next = 0.0;
    if cur >= q {
        next = 0.0;
    } else if cur <= 0.0 {
        next = if paper { floor(ill / k1) + floor(infected / k2) } else { floor(infected / k1) + floor(ill / k2) };
    } else {
        next = if paper { floor((inf_sum + cur) / (infected + 1.0)) + g } else { floor(total / (infected + ill + 1.0)) + g };
    }
    next = clamp(next, 0.0, q);
    let age = if next != cur { ctx.step } else { cell.z };
    vec4(next, 0.0, age, 0.0)
}

// Every state at random.
fn cell_seed(inside, noise, p) {
    let q = floor(p.states);
    vec4(floor(rand(0x91) * (q + 1.0)), 0.0, 0.0, 0.0)
}
