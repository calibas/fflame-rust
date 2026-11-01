// Pickover's Popcorn, the original orbit plot (not flam3's popcorn
// variation, which borrows its name):
//   x' = x - h sin(y + tan(c y)),   y' = y - h sin(x + tan(c x)).
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/popcorn.cpp, its generalized step with the functions
// its own default test names -- sin, tan, sin, tan -- and its default
// step and constant from fractalp.cpp). The program also takes a
// complex step and constant, and other functions; this keeps the real,
// classic case.
//
// Fractint plots every point of the orbit from every pixel; here the
// chaos game's orbits do the same work. Use it alone, at weight 1, on
// one transform whose triangle is the identity: the chaos game iterates
// it, every thread's orbit from where the flame's starts, in [-1, 1].
// They wander over about 22 units, centred near the origin (Fractint's
// view, -3 to 3 by -2.25 to 2.25, is of its shorter orbits from the
// screen's pixels). To see it whole, scale what is plotted, not the
// view: a final transform whose triangle shrinks it by about 4 / 22 (a
// final plots and does not feed the orbit), at a zoom near 1. Zooming
// the view out that far instead darkens it, as the log tone map divides
// by the view's area (Apophysis's too). Speed colouring brings out the
// kernels.
//
// It colours as a direct-colour variation: each point's palette index
// follows its step or its orbit's iteration (the Color choice), and the
// transform's Direct Color sets how much of that shows (0 gives back
// the transform's colour).
//
// A flame using it starts its orbits afresh every dispatch
// (`resets_orbits` in INFO), each its Orbit Length long: how far an
// orbit goes, saved with the flame in place of Iterations per Thread.
// Max Iterations is how many are drawn.

const PARAMS = #{
    h: #{ value: 0.05, caption: "Step", hint: "The step h each pass takes (Fractint's real step size)" },
    c: #{ value: 3.0, caption: "Constant", hint: "The frequency inside the tangent (Fractint's real constant C)" },
    orbit_length: #{ value: 1024, min: 16, max: 10000, caption: "Orbit Length", hint: "How many iterations each orbit runs from a fresh start: how far it goes. Saved with the flame, in place of Iterations per Thread" },
    color: #{ value: 0, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle", "Iteration"], hint: "What the palette index follows: the step's direction, its length against the longest a step can be (the Step times the square root of 2), the new point's distance from the origin, its angle about it, or the orbit's iteration since it started" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), from no step to the longest (Step length), per Color Size (Distance), or per orbit (Iteration)" },
    color_size: #{ value: 22.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Distance (the picture spans about 22 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Pickover Popcorn", name: "pickover_popcorn", resets_orbits: true };

// Each thread's count of its orbit's iterations, for the Iteration colour.
fn init(p) {
    #{ n: 0.0 }
}

// The palette index for the step from z to w, as the Color choice says;
// `longest` the longest step.
fn popcorn_color(z, w, mode, cycles, size, offset, longest, t) {
    let d = w - z;
    let q = if mode == 0 {
        atan2(d.im, d.re) / (2.0 * PI()) + 0.5
    } else if mode == 1 {
        if longest > 0.0 { d.abs() / longest } else { 0.0 }
    } else if mode == 2 {
        w.abs() / size
    } else if mode == 3 {
        atan2(w.im, w.re) / (2.0 * PI()) + 0.5
    } else {
        t
    };
    let k = offset + cycles * q;
    k - floor(k)
}

fn variation(z, p) {
    this.n = this.n + 1.0;
    let x = z.re;
    let y = z.im;
    let w = complex(x - p.h * sin(y + tan(p.c * y)), y - p.h * sin(x + tan(p.c * x)));
    #{ z: w, color: popcorn_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset, p.h.abs() * sqrt(2.0), this.n / p.orbit_length.to_float()) }
}
