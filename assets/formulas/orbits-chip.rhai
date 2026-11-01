// Chip's orbits as an escape-time picture: each pixel starts an orbit of
// the map
//   x' = y - sign(x) cos(sqr(ln|b x - c|)) atan(sqr(ln|c x - b|)),
//   y' = a - x,
// where sign(0) is 0 (Iterated Dynamics' `chip2d_orbit`; the flame
// variation `variation-chip.rhai` draws the orbits themselves). Its
// orbits never leave, so this is not the usual escape-time picture: the
// colouring reads where each pixel's orbit goes -- Orbit Trap (a point
// trap at Scale 5 rings the islands), the averages -- and the Radius only
// stops one that runs off. The colouring reads the orbit divided by Size,
// so its settings carry from map to map.

const PARAMS = #{
    a: #{ value: -15.0, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: -19.0, caption: "b", hint: "The first logarithm's slope, and the second's offset" },
    c: #{ value: 1.0, caption: "c", hint: "The first logarithm's offset, and the second's slope" },
    size: #{ value: 220.0, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 10000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Chip (Orbits)", center: complex(-7.5, -7.5), magn: 0.01809954751131222, maxiter: 200 };

// Fractint's sign: 0 at 0.
fn chip_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    let x = s.m.re;
    let u = ln((p.b * x - p.c).abs());
    let v = ln((p.c * x - p.b).abs());
    s.m = complex(s.m.im - chip_sign(x) * cos(u * u) * atan(v * v), p.a - x);
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
