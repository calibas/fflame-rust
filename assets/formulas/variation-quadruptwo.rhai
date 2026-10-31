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

const PARAMS = #{
    a: #{ value: 34.0, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: 1.0, caption: "b", hint: "The first logarithm's slope, and the second's offset" },
    c: #{ value: 5.0, caption: "c", hint: "The first logarithm's offset, and the second's slope" },
};

const INFO = #{ title: "Quadruptwo", name: "quadruptwo" };

fn quadruptwo_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

fn variation(z, p) {
    let x = z.re;
    let v = ln((p.c * x - p.b).abs());
    complex(z.im - quadruptwo_sign(x) * sin(ln((p.b * x - p.c).abs())) * atan(v * v), p.a - x)
}
