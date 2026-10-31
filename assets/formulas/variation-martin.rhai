// Martin, of the Martin attractors: Hopalong with a sine for the root,
//   x' = y - sin(x),   y' = a - x.
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/lorenz.cpp `martin2d_orbit`, its defaults and view from
// fractalp.cpp).
//
// Use it alone, at weight 1, on one transform whose triangle is the
// identity: the chaos game iterates it, every thread's orbit from where
// the flame's starts, in [-1, 1]. Fractint draws one orbit from the
// origin, for many more points; here the picture is thousands of
// shorter orbits, so it differs at the same parameters. At a = 3.14 the
// orbits span about 16 units, centred near (1.6, 1.6). To see it whole,
// scale what is plotted, not the view: a final transform whose triangle
// shrinks it by about 4 / 16 (a final plots and does not feed the
// orbit), at a zoom near 1. Zooming the view out that far instead
// darkens it, as the log tone map divides by the view's area
// (Apophysis's too).

const PARAMS = #{
    a: #{ value: 3.14, caption: "a", hint: "The constant y hops to, less x: near pi the orbits weave a lattice" },
};

const INFO = #{ title: "Martin", name: "martin" };

fn variation(z, p) {
    let x = z.re;
    complex(z.im - sin(x), p.a - x)
}
