// Quadruptwo's orbits as an escape-time picture: each pixel starts an
// orbit of the map
//   x' = y - sign(x) sin(ln|b x - c|) atan(sqr(ln|c x - b|)),
//   y' = a - x,
// where sign(0) is 0 (Iterated Dynamics' `quadrup_two2d_orbit`; the flame
// variation `variation-quadruptwo.rhai` draws the orbits themselves). Its
// orbits never leave, so this is not the usual escape-time picture: the
// colouring reads where each pixel's orbit goes -- Orbit Trap (a point
// trap at Scale 5 rings the islands), the averages -- and the Radius only
// stops one that runs off. The colouring reads the orbit divided by Size,
// so its settings carry from map to map.

const PARAMS = #{
    a: #{ value: 34.0, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: 1.0, caption: "b", hint: "The first logarithm's slope, and the second's offset" },
    c: #{ value: 5.0, caption: "c", hint: "The first logarithm's offset, and the second's slope" },
    size: #{ value: 22.0, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 10000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Quadruptwo (Orbits)", center: complex(17.0, 17.0), magn: 0.045454545454545456, maxiter: 200 };

// Fractint's sign: 0 at 0.
fn quadruptwo_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    let x = s.m.re;
    let v = ln((p.c * x - p.b).abs());
    s.m = complex(s.m.im - quadruptwo_sign(x) * sin(ln((p.b * x - p.c).abs())) * atan(v * v), p.a - x);
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
