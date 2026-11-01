// Hopalong's orbits as an escape-time picture: each pixel starts an orbit
// of the map
//   x' = y - sign(x) sqrt|b x - c|,
//   y' = a - x,
// where sign(0) is 0 (Iterated Dynamics' `hopalong2d_orbit`; the flame
// variation `variation-hopalong.rhai` draws the orbits themselves). Its
// orbits never leave, so this is not the usual escape-time picture: the
// colouring reads where each pixel's orbit goes -- Orbit Trap (a point
// trap at Scale 5 rings the islands), the averages -- and the Radius only
// stops one that runs off. The colouring reads the orbit divided by Size,
// so its settings carry from map to map.

const PARAMS = #{
    a: #{ value: 0.4, caption: "a", hint: "The constant y hops to, less x" },
    b: #{ value: 1.0, caption: "b", hint: "The slope inside the square root" },
    c: #{ value: 0.0, caption: "c", hint: "The offset inside the square root" },
    size: #{ value: 1.0, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Hopalong (Orbits)", center: complex(0.5, 0.5), magn: 1.0, maxiter: 200 };

// Fractint's sign: 0 at 0.
fn hopalong_sign(x) {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    let x = s.m.re;
    s.m = complex(s.m.im - hopalong_sign(x) * sqrt((p.b * x - p.c).abs()), p.a - x);
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
