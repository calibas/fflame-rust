// Hopalong, one of the Martin attractors (Iterated Dynamics' name for
// the family): each pass hops the point to
//   x' = y - sign(x) sqrt(|b x - c|),   y' = a - x,
// where sign(0) is 0. Ported from Iterated Dynamics (the GPL Fractint
// successor; libid/fractals/lorenz.cpp `hopalong2d_orbit`, its defaults
// and view from fractalp.cpp).
//
// Use it alone, at weight 1, on one transform whose triangle is the
// identity: the chaos game iterates it, every thread's orbit from where
// the flame's starts, in [-1, 1]. Fractint draws one orbit from the
// origin, for many more points; here the picture is thousands of
// shorter orbits, so it differs at the same parameters. The orbits stay
// within about 4 units of the origin (Fractint's view, -2 to 3 by -1.6
// to 2.6, is of its one orbit). Speed colouring shades it by how far
// each hop goes.
//
// It colours as a direct-colour variation: each point's palette index
// follows its step (the Color choice), and the transform's Direct Color
// sets how much of that shows (0 gives back the transform's colour).

const PARAMS = #{
    a: #{ value: 0.4, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: 1.0, caption: "b", hint: "The slope inside the square root" },
    c: #{ value: 0.0, caption: "c", hint: "The offset inside the square root" },
    color: #{ value: 1, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle"], hint: "What the palette index follows: the step's direction, its length, the new point's distance from the origin, or its angle about it" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), or per Color Size (Step length, Distance)" },
    color_size: #{ value: 4.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length and Distance (the picture spans about 4 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Hopalong", name: "hopalong" };

// Fractint's sign: 0 at 0.
fn hopalong_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// The palette index for the step from z to w, as the Color choice says.
fn hopalong_color(z, w, mode, cycles, size, offset) {
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
    let y = z.im;
    let w = complex(y - hopalong_sign(x) * sqrt((p.b * x - p.c).abs()), p.a - x);
    #{ z: w, color: hopalong_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset) }
}
