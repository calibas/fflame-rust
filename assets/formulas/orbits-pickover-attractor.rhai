// Pickover Attractor's orbits as an escape-time picture: each pixel
// starts an orbit of the map
//   x' = sin(a y) - z cos(b x),
//   y' = z sin(c x) - cos(d y),
//   z' = sin(x)
// (Iterated Dynamics' `pickover_orbit`; the flame variation
// `variation-pickover-attractor.rhai` draws the orbits themselves). Its
// orbits never leave: every start falls onto the same chaotic attractor,
// so what the colouring reads is mostly where on it the passes leave each
// orbit, and the picture is grainy, with structure only where orbits take
// long to arrive. The Radius only stops one that runs off. It is a 3D
// map: each pixel is a start's (x, y), its z the Start z, and the
// colouring reads the orbit's (x, y). The colouring reads the orbit
// divided by Size, so its settings carry from map to map.

const PARAMS = #{
    a: #{ value: 2.24, caption: "a", hint: "The frequency of y in x's sine" },
    b: #{ value: 0.43, caption: "b", hint: "The frequency of x in x's cosine" },
    c: #{ value: -0.65, caption: "c", hint: "The frequency of x in y's sine" },
    d: #{ value: -2.43, caption: "d", hint: "The frequency of y in y's cosine" },
    start_z: #{ value: 0.0, caption: "Start z", hint: "The third coordinate every pixel's orbit starts at" },
    size: #{ value: 1.0, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Pickover Attractor (Orbits)", center: complex(0.0, 0.0), magn: 1.0, maxiter: 200 };

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel, h: p.start_z }
}

fn step(s, p) {
    let x = s.m.re;
    let y = s.m.im;
    let h = s.h;
    s.m = complex(sin(p.a * y) - h * cos(p.b * x), h * sin(p.c * x) - cos(p.d * y));
    s.h = sin(x);
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
