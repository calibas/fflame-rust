// Gingerbread Man's orbits as an escape-time picture: each pixel starts
// an orbit of the map
//   x' = 1 - y + |x|,   y' = x
// (Iterated Dynamics' `ginger_bread_orbit`; the flame variation
// `variation-gingerbreadman.rhai` draws the orbits themselves). Its
// orbits never leave, so this is not the usual escape-time picture: the
// colouring reads where each pixel's orbit goes -- Orbit Trap (a point
// trap at Scale 5 rings the islands), the averages -- and the Radius only
// stops one that runs off. The colouring reads the orbit divided by Size,
// so its settings carry from map to map.

const PARAMS = #{
    size: #{ value: 2.75, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Gingerbread Man (Orbits)", center: complex(2.5, 2.5), magn: 0.36363636363636365, maxiter: 200 };

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    s.m = complex(1.0 - s.m.im + s.m.re.abs(), s.m.re);
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
