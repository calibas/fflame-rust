// Gaussian Integer, the built-in colouring's twin (techmatt, Ultra
// Fractal): the distance from each iterate to the nearest Gaussian
// integer -- the lattice point round(z) -- reduced over the orbit,
// normalised by sqrt(2)/2 to 0..1. It colours the interior as well.

const PARAMS = #{
    reduction: #{
        value: 0, choices: ["Smallest", "Mean", "Largest", "Angle at smallest"],
        caption: "Reduction",
        hint: "How the orbit's distances become one value: the smallest, their mean, the largest, or the direction from the lattice point at the smallest (techmatt's angle at minimum).",
    },
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of the normalised value.",
    },
};

const INFO = #{ title: "Gaussian Integer" };

// The smallest distance, their sum, the largest, the angle at the
// smallest.
fn start(z, p) {
    #{ smallest: 1.0, total: 0.0, largest: 0.0, angle: 0.0 }
}

fn step(s, z, p) {
    // WGSL's round, halves to even, as the built-in's.
    let g = z - round_even(z);
    let d = g.abs() * 1.4142135;
    if d < s.smallest {
        s.smallest = d;
        // At a lattice point the direction is undefined: 0.
        s.angle = if d > 1e-6 { g.arg() * 0.15915494 + 0.5 } else { 0.0 };
    }
    s.total = s.total + d;
    s.largest = max(s.largest, d);
    s
}

fn finish(s, z, p, ctx) {
    let v = if p.reduction == 1 {
        s.total / max(to_float(ctx.numiter), 1.0)
    } else if p.reduction == 2 {
        s.largest
    } else if p.reduction == 3 {
        s.angle
    } else {
        s.smallest
    };
    v * p.scale
}
