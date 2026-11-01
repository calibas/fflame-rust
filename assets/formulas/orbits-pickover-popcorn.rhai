// Pickover Popcorn's orbits as an escape-time picture: each pixel starts
// an orbit of the map
//   x' = x - h sin(y + tan(C y)),
//   y' = y - h sin(x + tan(C x))
// (Iterated Dynamics' `popcorn_fractal (the generalized step, with sin
// and tan)`; the flame variation `variation-pickover-popcorn.rhai` draws
// the orbits themselves). Its orbits never leave, so this is not the
// usual escape-time picture: the colouring reads where each pixel's orbit
// goes -- Orbit Trap (a point trap at Scale 5 rings the islands), the
// averages -- and the Radius only stops one that runs off. The colouring
// reads the orbit divided by Size, so its settings carry from map to map.

const PARAMS = #{
    h: #{ value: 0.05, caption: "Step", hint: "The step h each pass takes (Fractint's real step size)" },
    c: #{ value: 3.0, caption: "Constant", hint: "The frequency inside the tangent (Fractint's real constant C)" },
    size: #{ value: 4.5, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Pickover Popcorn (Orbits)", center: complex(0.0, 0.0), magn: 0.8888888888888888, maxiter: 200 };

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    let x = s.m.re;
    let y = s.m.im;
    s.m = complex(x - p.h * sin(y + tan(p.c * y)), y - p.h * sin(x + tan(p.c * x)));
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
