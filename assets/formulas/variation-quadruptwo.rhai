// Quadruptwo, one of the Hopalong family:
//   x' = y - sign(x) sin(ln|b x - c|) atan(sqr(ln|c x - b|)),
//   y' = a - x,
// where sign(0) is 0. Ported from Iterated Dynamics (the GPL Fractint
// successor; libid/fractals/lorenz.cpp `quadrup_two2d_orbit`, its
// defaults and view from fractalp.cpp).
//
// Use it alone, at weight 1, on one transform whose triangle is the
// identity: the chaos game iterates it, every thread's orbit from where
// the flame's starts, in [-1, 1]. Fractint draws one orbit from the
// origin, for many more points; here the picture is thousands of
// shorter orbits, so it differs at the same parameters. At the defaults
// they span about 90 units, centred near (17, 17). To see it whole,
// scale what is plotted, not the view: a final transform whose triangle
// shrinks it by about 4 / 90 (a final plots and does not feed the
// orbit), at a zoom near 1. Zooming the view out that far instead
// darkens it, as the log tone map divides by the view's area
// (Apophysis's too).
//
// It colours as a direct-colour variation: each point's palette index
// follows its step (the Color choice), and the transform's Direct Color
// sets how much of that shows (0 gives back the transform's colour).

const PARAMS = #{
    a: #{ value: 34.0, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: 1.0, caption: "b", hint: "The first logarithm's slope, and the second's offset" },
    c: #{ value: 5.0, caption: "c", hint: "The first logarithm's offset, and the second's slope" },
    color: #{ value: 1, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle"], hint: "What the palette index follows: the step's direction, its length, the new point's distance from the origin, or its angle about it" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), or per Color Size (Step length, Distance)" },
    color_size: #{ value: 90.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length and Distance (the picture spans about 90 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Quadruptwo", name: "quadruptwo" };

fn quadruptwo_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// The palette index for the step from z to w, as the Color choice says.
fn quadruptwo_color(z, w, mode, cycles, size, offset) {
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
    let x = z.re;
    let v = ln((p.c * x - p.b).abs());
    let w = complex(z.im - quadruptwo_sign(x) * sin(ln((p.b * x - p.c).abs())) * atan(v * v), p.a - x);
    #{ z: w, color: quadruptwo_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset) }
}
