// Pickover's Popcorn, the original orbit plot (not flam3's popcorn
// variation, which borrows its name):
//   x' = x - h sin(y + tan(c y)),   y' = y - h sin(x + tan(c x)).
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/popcorn.cpp, its generalized step with the functions
// its own default test names -- sin, tan, sin, tan -- and its default
// step and constant from fractalp.cpp). The program also takes a
// complex step and constant, and other functions; this keeps the real,
// classic case.
//
// Fractint plots every point of the orbit from every pixel; here the
// chaos game's orbits do the same work. Use it alone, at weight 1, on
// one transform whose triangle is the identity: the chaos game iterates
// it, every thread's orbit from where the flame's starts, in [-1, 1].
// They wander over about 22 units, centred near the origin (Fractint's
// view, -3 to 3 by -2.25 to 2.25, is of its shorter orbits from the
// screen's pixels). To see it whole, scale what is plotted, not the
// view: a final transform whose triangle shrinks it by about 4 / 22 (a
// final plots and does not feed the orbit), at a zoom near 1. Zooming
// the view out that far instead darkens it, as the log tone map divides
// by the view's area (Apophysis's too). Speed colouring brings out the
// kernels.

const PARAMS = #{
    h: #{ value: 0.05, caption: "Step", hint: "The step h each pass takes (Fractint's real step size)" },
    c: #{ value: 3.0, caption: "Constant", hint: "The frequency inside the tangent (Fractint's real constant C)" },
};

const INFO = #{ title: "Pickover Popcorn", name: "pickover_popcorn" };

fn variation(z, p) {
    let x = z.re;
    let y = z.im;
    complex(x - p.h * sin(y + tan(p.c * y)), y - p.h * sin(x + tan(p.c * x)))
}
