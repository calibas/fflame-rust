// Latoocarfian's orbits as an escape-time picture: each pixel starts an
// orbit of the map
//   x' = sin(b y) + c sin(b x),
//   y' = sin(a x) + d sin(a y)
// (Iterated Dynamics' `latoo_orbit`; the flame variation
// `variation-latoocarfian.rhai` draws the orbits themselves). Its orbits
// never leave: every start falls onto the same chaotic attractor, so what
// the colouring reads is mostly where on it the passes leave each orbit,
// and the picture is grainy, with structure only where orbits take long
// to arrive. The Radius only stops one that runs off. The colouring reads
// the orbit divided by Size, so its settings carry from map to map.

const PARAMS = #{
    a: #{ value: -0.966918, caption: "a", hint: "The frequency of y's sines" },
    b: #{ value: 2.879879, caption: "b", hint: "The frequency of x's sines" },
    c: #{ value: 0.765145, caption: "c", hint: "How much sin(b x) adds to x" },
    d: #{ value: 0.744728, caption: "d", hint: "How much sin(a y) adds to y" },
    size: #{ value: 0.75, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Latoocarfian (Orbits)", center: complex(0.0, 0.0), magn: 1.3333333333333333, maxiter: 200 };

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    let x = s.m.re;
    let y = s.m.im;
    s.m = complex(sin(y * p.b) + p.c * sin(x * p.b), sin(x * p.a) + p.d * sin(y * p.a));
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
