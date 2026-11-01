// KAM Torus's orbits as an escape-time picture: each pixel starts an
// orbit of the map
//   (x, y) -> (x, y - x^2) turned by Angle
// (Iterated Dynamics' `kam_torus_orbit`; the flame variation
// `variation-kamtorus.rhai` draws the orbits themselves). Orbits outside
// its tori run off, so there this is an escape-time picture, the count;
// inside, the colouring reads where each pixel's orbit goes -- Orbit
// Trap, the averages. The colouring reads the orbit divided by Size, so
// its settings carry from map to map.

const PARAMS = #{
    angle: #{ value: 1.3, caption: "Angle", hint: "The turn each pass, in radians (Fractint's first parameter)" },
    size: #{ value: 1.5, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "KAM Torus (Orbits)", center: complex(0.0, 0.0), magn: 2.6666666666666665, maxiter: 200 };

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    let x = s.m.re;
    let t = s.m.im - x * x;
    let sa = sin(p.angle);
    let ca = cos(p.angle);
    s.m = complex(x * ca - t * sa, x * sa + t * ca);
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
