// Latoocarfian, from Pickover's "Chaos in Wonderland":
//   x' = sin(b y) + c sin(b x),   y' = sin(a x) + d sin(a y).
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/lorenz.cpp `latoo_orbit`, its defaults and view from
// fractalp.cpp). The program lets each of the four sines be another
// function; this one keeps the sines its source comment writes (a Rhai
// variation's function menus are a later step).
//
// Use it alone, at weight 1, on one transform whose triangle is the
// identity: the chaos game iterates it, every thread's orbit from where
// the flame's starts, in [-1, 1]. The orbits settle on the attractor,
// about 4 units across (Fractint's view is -2 to 2 by -1.5 to 1.5).
// Fractint draws it as a density histogram, as a flame does.

const PARAMS = #{
    a: #{ value: -0.966918, caption: "a", hint: "The frequency of y's sines" },
    b: #{ value: 2.879879, caption: "b", hint: "The frequency of x's sines" },
    c: #{ value: 0.765145, caption: "c", hint: "How much sin(b x) adds to x" },
    d: #{ value: 0.744728, caption: "d", hint: "How much sin(a y) adds to y" },
};

const INFO = #{ title: "Latoocarfian", name: "latoocarfian" };

fn variation(z, p) {
    let x = z.re;
    let y = z.im;
    complex(sin(y * p.b) + p.c * sin(x * p.b), sin(x * p.a) + p.d * sin(y * p.a))
}
