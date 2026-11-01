// Dynamic System, Fractint's: a flow in the plane,
//   dx/dt = -sin(y + a sin(b y)),   dy/dt = sin(x + a sin(b x)),
// stepped by dt each pass. Ported from Iterated Dynamics (the GPL
// Fractint successor; libid/fractals/lorenz.cpp `dynamic_orbit` and
// `dynamic2d_per_image`, its defaults and view from fractalp.cpp). The
// program lets the inner sine be another function; this keeps the sine.
//
// Fractint steps both from the old point, or, with a negative time
// step, y first and then x from the new y (its "Euler" choice): Update
// below. It starts one orbit in each cell of a grid over its view (-20
// to 20). Use it alone, at weight 1, on one transform whose triangle is
// the identity: the chaos game iterates it, every thread's orbit from
// where the flame's starts, in [-1, 1]. Stepped so, an orbit spirals
// out of the centre over a few hundred passes and settles onto loops
// about the origin, about 14 units across: the picture is mostly that
// way out, which an orbit kept from dispatch to dispatch would draw
// once. So a flame using it starts its orbits afresh every dispatch,
// each Iterations per Thread long (`resets_orbits` in INFO), much as
// Fractint draws many short orbits. To see it whole, scale what is
// plotted, not the view: a final transform whose triangle shrinks it by
// about 4 / 14 (a final plots and does not feed the orbit), at a zoom
// near 1. Zooming the view out that far instead darkens it, as the log
// tone map divides by the view's area (Apophysis's too).
//
// It colours as a direct-colour variation: each point's palette index
// follows its step or its orbit's iteration (the Color choice), and the
// transform's Direct Color sets how much of that shows (0 gives back
// the transform's colour).

const PARAMS = #{
    dt: #{ value: 0.1, caption: "Time step", hint: "How far each pass flows" },
    a: #{ value: 1.0, caption: "a", hint: "How much the inner sine bends the flow" },
    b: #{ value: 3.0, caption: "b", hint: "The inner sine's frequency" },
    update: #{ value: 0, caption: "Update", choices: ["Both from the old point", "y first (Euler)"], hint: "Fractint's two orders: both parts from the old point, or y first and x from the new y (its negative time step)" },
    color: #{ value: 0, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle", "Iteration"], hint: "What the palette index follows: the step's direction, its length against the longest a step can be (the Time step times the square root of 2), the new point's distance from the origin, its angle about it, or the orbit's iteration since it started" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), from no step to the longest (Step length), per Color Size (Distance), or per Color Period (Iteration)" },
    color_size: #{ value: 14.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Distance (the picture spans about 14 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
    color_period: #{ value: 1024.0, min: 1.0, caption: "Color Period", hint: "The iterations one cycle of the palette spans, for Iteration (an orbit is Iterations per Thread long, 1,024 by default)" },
};

const INFO = #{ title: "Dynamic System", name: "dynamic_system", resets_orbits: true };

// Each thread's count of its orbit's iterations, for the Iteration colour.
fn init(p) {
    #{ n: 0.0 }
}

// The palette index for the step from z to w, as the Color choice says;
// `longest` the longest step.
fn dynamic_color(z, w, mode, cycles, size, offset, longest, t) {
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
    let ny = y + p.dt * sin(x + p.a * sin(p.b * x));
    let yy = if p.update == 1 { ny } else { y };
    let w = complex(x - p.dt * sin(yy + p.a * sin(p.b * yy)), ny);
    #{ z: w, color: dynamic_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset, p.dt.abs() * sqrt(2.0), this.n / p.color_period) }
}
