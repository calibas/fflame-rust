// Spatial rock-paper-scissors (Reichenbach, Mobilia and Frey): K species
// in a cycle of dominance on a lattice with empty sites.
//
// The Rhai twin of the built-in `spatial_rps` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, and the same field.
//
// A cell is empty (0) or holds species 1..K. Each step it meets one
// neighbour at random: an empty cell is colonised at `p_reproduce`, a
// species beaten by its neighbour's is replaced at `p_select`.
// Channels: x = the species, z = the step it last changed, y and w
// spare.

// `v` mod `n`, a whole number.

const PARAMS = #{
    species: #{
        value: 3.0, min: 3.0, max: 5.0, caption: "Species",
        hint: "Length of the dominance cycle. Five species (rock-paper-scissors-lizard-Spock) forms two nested levels of spiral.",
    },
    p_select: #{
        value: 1.0, min: 0.0, max: 1.0, caption: "Predation rate",
        hint: "Chance a cell is taken over by a neighbour that beats it. Lower values slow the fronts and widen the domains.",
    },
    p_reproduce: #{
        value: 1.0, min: 0.0, max: 1.0, caption: "Colonisation rate",
        hint: "Chance an empty cell is colonised by a neighbour.",
    },
};
const INFO = #{
    title: "Spatial RPS",
    id: "spatial_rps",
    description: "Cyclic competition: each species beats the next and loses to the one before, and no species can win. Rotating domain spirals.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 400,
    state: "discrete",
    time_step: false,
    settles: false,
    presets: [
        #{
            name: "three_species",
            title: "Three species",
            params: #{ species: 3.0, p_select: 1.0, p_reproduce: 1.0 },
            steps: 400,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.25, offset: 0.0, wrap: 0.0 },
        },
    ],
};

fn fract_state(v, n) {
    floor(v - floor(v / n) * n)
}

fn cell_step(cell, p, ctx) {
    let k = max(floor(p.species), 3.0);
    // One of the eight neighbours, at random.
    let pick = to_int(floor(rand(0xb1) * 8.0));
    let dx = -1;
    let dy = -1;
    if pick == 1 {
        dx = 0;
    } else if pick == 2 {
        dx = 1;
    } else if pick == 3 {
        dy = 0;
    } else if pick == 4 {
        dx = 1;
        dy = 0;
    } else if pick == 5 {
        dy = 1;
    } else if pick == 6 {
        dx = 0;
        dy = 1;
    } else if pick >= 7 {
        dx = 1;
        dy = 1;
    }
    let cur = floor(cell.x);
    let other = floor(read(dx, dy).x);
    let next = cur;
    if cur <= 0.0 {
        if other > 0.0 && rand(0xb2) < p.p_reproduce {
            next = other;
        }
    } else if other > 0.0 {
        let beats = fract_state(cur, k) + 1.0;
        if other == beats && rand(0xb3) < p.p_select {
            next = other;
        }
    }
    vec4(next, 0.0, if next != cur { ctx.step } else { cell.z }, 0.0)
}

// Every species, and empty sites, at random.
fn cell_seed(inside, noise, p) {
    let k = max(floor(p.species), 3.0);
    vec4(floor(rand(0xb0) * (k + 1.0)), 0.0, 0.0, 0.0)
}
