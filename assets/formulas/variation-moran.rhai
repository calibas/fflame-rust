// Level-dependent IFS ("Moran" flames): a flame whose large-scale shape
// and fine texture are different fractals -- a Sierpinski triangle made
// of ferns, a carpet made of dragons. An ordinary flame repeats one set
// of maps at every scale; here each point is a composition of one map per
// level, the coarsest level outermost, and each level takes its map from
// the set its schedule names. A schedule that repeats (Alternate) is only
// an ordinary IFS of composed maps; the split and the Sturmian schedules
// are not, and the chaos game cannot draw them.
//
// It ignores the point it is given: use it as the only variation of a
// flame's only transform, with Direct Color at 1 for its colours (the
// coarsest level's map). Every set is drawn in the unit square: view it
// at zoom 3 around (0.5, 0.5).
//
// It shows a variation building each point itself: a loop of `depth`
// levels from a fixed start, a draw per level (`rand()`), and functions
// choosing and applying each set's maps.

const PARAMS = #{
    coarse: #{ value: 0, caption: "Coarse", choices: ["Sierpinski triangle", "Sierpinski carpet", "Fern", "Dragon", "Levy C"], hint: "The set of the levels the schedule calls coarse" },
    fine: #{ value: 2, caption: "Fine", choices: ["Sierpinski triangle", "Sierpinski carpet", "Fern", "Dragon", "Levy C"], hint: "The set of the other levels" },
    schedule: #{ value: 0, caption: "Schedule", choices: ["Coarse above the split", "Alternate", "Sturmian (golden)"], hint: "Which levels are coarse: the first Split levels, every other level, or the golden ratio's never-repeating sequence" },
    split: #{ value: 3, min: 1, max: 16, caption: "Split", hint: "With the first schedule, how many of the coarsest levels are coarse" },
    depth: #{ value: 24, min: 2, max: 40, caption: "Depth", hint: "How many levels make each point: the fern shrinks slowly, and needs the most" },
};

const INFO = #{ title: "Moran Flame (Rhai)", name: "rhai_moran_flame" };

// How many maps a set has.
fn count(set) {
    if set == 0 { 3 } else if set == 1 { 8 } else if set == 2 { 4 } else { 2 }
}

// Which of a set's maps a draw `r` in [0, 1) picks: the fern's by
// Barnsley's weights, the rest alike.
fn choose(set, r) {
    if set == 2 {
        if r < 0.01 { 0 } else if r < 0.86 { 1 } else if r < 0.93 { 2 } else { 3 }
    } else {
        let n = count(set);
        let i = (r * n.to_float()).floor().to_int();
        if i >= n { n - 1 } else { i }
    }
}

// The set's map `i`, applied to `z` in the unit square.
fn apply(set, i, z) {
    if set == 0 {
        // Sierpinski triangle.
        let o = if i == 0 { complex(0.0, 0.0) } else if i == 1 { complex(0.5, 0.0) } else { complex(0.25, 0.4330127) };
        z * 0.5 + o
    } else if set == 1 {
        // Sierpinski carpet: the 3x3 grid less its centre.
        let n = if i >= 4 { i + 1 } else { i };
        z / 3.0 + complex((n % 3).to_float() / 3.0, (n.to_float() / 3.0).floor() / 3.0)
    } else if set == 2 {
        // Barnsley's fern, its own coordinates fitted to the square.
        let w = complex((z.re - 0.258) * 10.0 - 2.182, z.im * 10.0);
        let v = if i == 0 {
            complex(0.0, 0.16 * w.im)
        } else if i == 1 {
            complex(0.85 * w.re + 0.04 * w.im, -0.04 * w.re + 0.85 * w.im + 1.6)
        } else if i == 2 {
            complex(0.2 * w.re - 0.26 * w.im, 0.23 * w.re + 0.22 * w.im + 1.6)
        } else {
            complex(-0.15 * w.re + 0.28 * w.im, 0.26 * w.re + 0.24 * w.im + 0.44)
        };
        complex((v.re + 2.182) * 0.1 + 0.258, v.im * 0.1)
    } else if set == 3 {
        // Heighway's dragon, fitted to the square.
        let w = complex(z.re * 1.5 - 0.333, (z.im - 0.167) * 1.5 - 0.333);
        let v = if i == 0 { w * complex(0.5, 0.5) } else { complex(1.0, 0.0) - w * complex(0.5, -0.5) };
        complex((v.re + 0.333) / 1.5, (v.im + 0.333) / 1.5 + 0.167)
    } else {
        // Levy's C curve, fitted to the square.
        let w = complex(z.re * 2.0 - 0.5, (z.im - 0.19) * 2.0 - 1.0);
        let v = if i == 0 { w * complex(0.5, -0.5) } else { complex(1.0, 0.0) + (w - complex(1.0, 0.0)) * complex(0.5, 0.5) };
        complex((v.re + 0.5) / 2.0, (v.im + 1.0) / 2.0 + 0.19)
    }
}

// Whether level `k` (1 the coarsest) takes the coarse set.
fn coarse_at(k, schedule, split) {
    if schedule == 0 {
        k <= split
    } else if schedule == 1 {
        k % 2 == 1
    } else {
        let a = 0.6180339887;
        let kf = k.to_float();
        ((kf + 1.0) * a).floor() - (kf * a).floor() > 0.5
    }
}

fn variation(z, p) {
    // The deepest level first, the coarsest last: the outermost map. A
    // fixed start: where the levels have not shrunk the square below a
    // pixel (the slow fern), a random start would show its images.
    let w = complex(0.5, 0.5);
    let colour = 0.0;
    let k = p.depth;
    while k >= 1 {
        let set = if coarse_at(k, p.schedule, p.split) { p.coarse } else { p.fine };
        let i = choose(set, rand());
        w = apply(set, i, w);
        if k == 1 {
            colour = (i.to_float() + 0.5) / count(set).to_float();
        }
        k -= 1;
    }
    // Up the screen: a flame draws its +y down it.
    #{ z: complex(w.re, 1.0 - w.im), color: colour }
}
