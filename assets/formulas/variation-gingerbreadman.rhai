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

const INFO = #{ title: "Gingerbread Man", name: "gingerbreadman" };

fn variation(z, p) {
    complex(1.0 - z.im + z.re.abs(), z.re)
}
