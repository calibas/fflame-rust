// The Gingerbread Man, a piecewise-linear map whose chaotic orbits fill a
// gingerbread-man shape around islands of stable ones:
//   x' = 1 - y + |x|,   y' = x.
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/lorenz.cpp `ginger_bread_orbit`, whose comment cites
// page 149 of "The Science of Fractal Images"; its view from
// fractalp.cpp, -4.5 to 8.5 each way).
//
// Fractint draws one orbit, from a start its parameters set (-0.1, 0 by
// default). Use it alone, at weight 1, on one transform whose triangle
// is the identity: the chaos game iterates it, every thread's orbit
// from where the flame's starts, in [-1, 1]. The picture is the man
// with his islands, about 11 units across, centred near (2.5, 2.5). To
// see it whole, scale what is plotted, not the view: a final transform
// whose triangle shrinks it by about 4 / 11 (a final plots and does not
// feed the orbit), at a zoom near 1. Zooming the view out that far
// instead darkens it, as the log tone map divides by the view's area
// (Apophysis's too).
//
// It colours as a direct-colour variation: each point's palette index
// follows its step (the Color choice), and the transform's Direct Color
// sets how much of that shows (0 gives back the transform's colour).

const PARAMS = #{
    color: #{ value: 1, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle"], hint: "What the palette index follows: the step's direction, its length, the new point's distance from the origin, or its angle about it" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), or per Color Size (Step length, Distance)" },
    color_size: #{ value: 11.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length and Distance (the picture spans about 11 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Gingerbread Man", name: "gingerbreadman" };

// The palette index for the step from z to w, as the Color choice says.
fn gingerbreadman_color(z, w, mode, cycles, size, offset) {
    let d = w - z;
    let q = if mode == 0 {
        atan2(d.im, d.re) / (2.0 * PI()) + 0.5
    } else if mode == 1 {
        d.abs() / size
    } else if mode == 2 {
        w.abs() / size
    } else {
        atan2(w.im, w.re) / (2.0 * PI()) + 0.5
    };
    let k = offset + cycles * q;
    k - floor(k)
}

fn variation(z, p) {
    let w = complex(1.0 - z.im + z.re.abs(), z.re);
    #{ z: w, color: gingerbreadman_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset) }
}
