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
// of its one orbit spans 16,000). To see it whole, scale what is plotted, not the view:
// a final transform whose triangle shrinks it by about 4 / 1400 (a
// final plots and does not feed the orbit), at a zoom near 1. Zooming
// the view out that far instead darkens it, as the log tone map divides
// by the view's area (Apophysis's too).

const PARAMS = #{
    a: #{ value: -55.0, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: -1.0, caption: "b", hint: "The angle whose cosine weighs sin(x)" },
    c: #{ value: -42.0, caption: "c", hint: "The offset inside the absolute value" },
};

const INFO = #{ title: "Threeply", name: "threeply" };

fn threeply_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

fn variation(z, p) {
    let x = z.re;
    let k = (sin(x) * cos(p.b) + p.c - x * sin(p.a + p.b + p.c)).abs();
    complex(z.im - threeply_sign(x) * k, p.a - x)
}
