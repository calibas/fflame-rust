// Threeply, one of the Hopalong family:
//   x' = y - sign(x) |sin(x) cos(b) + c - x sin(a + b + c)|,
//   y' = a - x,
// where sign(0) is 0. Ported from Iterated Dynamics (the GPL Fractint
// successor; libid/fractals/lorenz.cpp `three_ply2d_orbit`, its defaults
// and view from fractalp.cpp).
//
// Use it alone, at weight 1, on one transform whose triangle is the
// identity: the chaos game iterates it, every thread's orbit from where
// the flame's starts, in [-1, 1]. Fractint draws one orbit from the
// origin, for many more points; here the picture is thousands of
// shorter orbits, so it differs at the same parameters. At the defaults
// they span about 1,400 units, centred near the origin (Fractint's view
// of its one orbit spans 16,000). To see it whole, scale what is
// plotted, not the view: a final transform whose triangle shrinks it by
// about 4 / 1400 (a final plots and does not feed the orbit), at a zoom
// near 1. Zooming the view out that far instead darkens it, as the log
// tone map divides by the view's area (Apophysis's too).
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
    a: #{ value: -55.0, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: -1.0, caption: "b", hint: "The angle whose cosine weighs sin(x)" },
    c: #{ value: -42.0, caption: "c", hint: "The offset inside the absolute value" },
    orbit_length: #{ value: 1024, min: 16, max: 10000, caption: "Orbit Length", hint: "How many iterations each orbit runs from a fresh start: how far it goes. Saved with the flame, in place of Iterations per Thread" },
    color: #{ value: 1, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle", "Iteration"], hint: "What the palette index follows: the step's direction, its length, the new point's distance from the origin, its angle about it, or the orbit's iteration since it started" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), per Color Size (Step length, Distance), or per orbit (Iteration)" },
    color_size: #{ value: 1400.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length and Distance (the picture spans about 1,400 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Threeply", name: "threeply", resets_orbits: true };

// Each thread's count of its orbit's iterations, for the Iteration colour.
fn init(p) {
    #{ n: 0.0 }
}

fn threeply_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// The palette index for the step from z to w, as the Color choice says.
fn threeply_color(z, w, mode, cycles, size, offset, t) {
    let d = w - z;
    let q = if mode == 0 {
        atan2(d.im, d.re) / (2.0 * PI()) + 0.5
    } else if mode == 1 {
        d.abs() / size
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
    let k = (sin(x) * cos(p.b) + p.c - x * sin(p.a + p.b + p.c)).abs();
    let w = complex(z.im - threeply_sign(x) * k, p.a - x);
    #{ z: w, color: threeply_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset, this.n / p.orbit_length.to_float()) }
}
