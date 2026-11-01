// Latoocarfian, from Pickover's "Chaos in Wonderland":
//   x' = sin(b y) + c sin(b x),   y' = sin(a x) + d sin(a y).
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/lorenz.cpp `latoo_orbit`, its defaults and view from
// fractalp.cpp). The program lets each of the four sines be another
// function; this one keeps the sines its source comment writes (a Rhai
// variation's function menus are a later step).
//
// Use it alone, at weight 1, on one transform whose triangle is the
// identity: the chaos game iterates it, every thread's orbit from where
// the flame's starts, in [-1, 1]. The orbits settle on the attractor,
// about 4 units across (Fractint's view is -2 to 2 by -1.5 to 1.5).
// Fractint draws it as a density histogram, as a flame does.
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
    a: #{ value: -0.966918, caption: "a", hint: "The frequency of y's sines" },
    b: #{ value: 2.879879, caption: "b", hint: "The frequency of x's sines" },
    c: #{ value: 0.765145, caption: "c", hint: "How much sin(b x) adds to x" },
    d: #{ value: 0.744728, caption: "d", hint: "How much sin(a y) adds to y" },
    orbit_length: #{ value: 1024, min: 16, max: 10000, caption: "Orbit Length", hint: "How many iterations each orbit runs from a fresh start: how far it goes. Saved with the flame, in place of Iterations per Thread" },
    color: #{ value: 0, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle", "Iteration"], hint: "What the palette index follows: the step's direction, its length, the new point's distance from the origin, its angle about it, or the orbit's iteration since it started" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), per Color Size (Step length, Distance), or per orbit (Iteration)" },
    color_size: #{ value: 4.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length and Distance (the picture spans about 4 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Latoocarfian", name: "latoocarfian", resets_orbits: true };

// Each thread's count of its orbit's iterations, for the Iteration colour.
fn init(p) {
    #{ n: 0.0 }
}

// The palette index for the step from z to w, as the Color choice says.
fn latoocarfian_color(z, w, mode, cycles, size, offset, t) {
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
    let y = z.im;
    let w = complex(sin(y * p.b) + p.c * sin(x * p.b), sin(x * p.a) + p.d * sin(y * p.a));
    #{ z: w, color: latoocarfian_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset, this.n / p.orbit_length.to_float()) }
}
