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

const PARAMS = #{
    a: #{ value: 0.4, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: 1.0, caption: "b", hint: "The slope inside the square root" },
    c: #{ value: 0.0, caption: "c", hint: "The offset inside the square root" },
};

const INFO = #{ title: "Hopalong", name: "hopalong" };

// Fractint's sign: 0 at 0.
fn hopalong_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

fn variation(z, p) {
    let x = z.re;
    let y = z.im;
    complex(y - hopalong_sign(x) * sqrt((p.b * x - p.c).abs()), p.a - x)
}
