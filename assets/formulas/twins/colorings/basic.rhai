// Basic, the built-in colouring's twin (Ultra Fractal's Basic, its real
// / imaginary / sum modes): `0.05 (4 + v)` of where the orbit escaped.

const PARAMS = #{
    part: #{
        value: 0, choices: ["Real", "Imaginary", "Sum"],
        caption: "Part",
        hint: "Which part of the escaped z: its real part, its imaginary part, or their sum.",
    },
    scale: #{
        value: 1.0, min: 0.01, max: 20.0,
        caption: "Scale",
        hint: "Palette distance per unit of 0.05 (4 + v).",
    },
};

const INFO = #{ title: "Basic", region: "outside" };

fn finish(s, z, p) {
    let v = if p.part == 1 {
        z.im
    } else if p.part == 2 {
        z.re + z.im
    } else {
        z.re
    };
    // An escape past any meaning (an infinite z, the exponential
    // families) reads as zero: a NaN fails the comparison.
    let v = if abs(v) <= 1e30 { v } else { 0.0 };
    0.05 * (4.0 + v) * p.scale
}
