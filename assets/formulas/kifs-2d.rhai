// A kaleidoscopic IFS in the plane, as an escape-time formula: the 2D
// cousin of KIFS (Solid), as Kaliset is of the folds it shares. Each step
// folds the point into one wedge of the pattern, turns it, and scales it
// away from the offset; a point of the fractal stays near it forever, any
// other point is thrown out, and how fast it leaves draws the picture.
//
// At the defaults it is the Sierpinski carpet. The colourings that read
// the escape -- Escape Count, a distance estimate -- suit it best.

const PARAMS = #{
    fold: #{
        value: 2,
        choices: ["Square", "Diagonal", "Carpet"],
        caption: "Fold",
        hint: "Square: each axis's mirror. Diagonal: those and the diagonal one. Carpet: those, with the carpet's correction for its middle row.",
    },
    scale: #{ value: 3.0, min: 1.1, max: 4.0, caption: "Scale", hint: "How much each step enlarges the plane about the offset" },
    offset: #{ value: complex(1.0, 1.0), caption: "Offset", hint: "The corner the pattern grows from: a point of the fractal" },
    turn: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Turn", hint: "Degrees each step turns, after the fold" },
    bailout: #{ value: 100.0, min: 4.0, caption: "Bailout", hint: "The squared radius past which a point has left" },
};

const INFO = #{ title: "KIFS 2D", center: complex(0.0, 0.0), magn: 1.5, maxiter: 40 };

fn start(pixel, p) {
    #{ z: pixel }
}

fn step(s, p) {
    // The fold: each axis's mirror, then the diagonal's.
    let x = abs(s.z.re);
    let y = abs(s.z.im);
    if p.fold >= 1 && x < y {
        let t = x;
        x = y;
        y = t;
    }
    // The turn, then the scaling away from the offset.
    let a = p.turn * PI() / 180.0;
    let w = complex(x, y) * complex(cos(a), sin(a));
    let k = p.scale - 1.0;
    s.z = w * p.scale - p.offset * k;
    // The carpet: the middle row's squares move back up.
    if p.fold == 2 && s.z.im < -0.5 * p.offset.im * k {
        s.z = s.z + complex(0.0, p.offset.im * k);
    }
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
