// Rossler Attractor's orbits as an escape-time picture: each pixel starts
// an orbit of the map
//   x' = x - (y + z) dt,
//   y' = y + (x + a y) dt,
//   z' = z + (b + z (x - c)) dt
// (Iterated Dynamics' `rossler_orbit`; the flame variation
// `variation-rossler-attractor.rhai` draws the orbits themselves). Its
// orbits never leave: they fall onto its attractor, so this is not the
// usual escape-time picture: the colouring reads how each pixel's orbit
// gets there -- Orbit Trap, the averages -- and the Radius only stops one
// that runs off. It is a 3D map: each pixel is a start's (x, y), its z
// the Start z, and the colouring reads the orbit's (x, y). The colouring
// reads the orbit divided by Size, so its settings carry from map to map.

const PARAMS = #{
    dt: #{ value: 0.04, caption: "Time step", hint: "How far each pass flows" },
    a: #{ value: 0.2, caption: "a", hint: "How much y feeds its own growth" },
    b: #{ value: 0.2, caption: "b", hint: "z's constant growth" },
    c: #{ value: 5.7, caption: "c", hint: "How far x must climb before z grows" },
    start_z: #{ value: 0.0, caption: "Start z", hint: "The third coordinate every pixel's orbit starts at" },
    size: #{ value: 15.0, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Rossler Attractor (Orbits)", center: complex(0.0, 10.0), magn: 0.06666666666666667, maxiter: 200 };

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel, h: p.start_z }
}

fn step(s, p) {
    let x = s.m.re;
    let y = s.m.im;
    let h = s.h;
    s.m = complex(x - (y + h) * p.dt, y + (x + p.a * y) * p.dt);
    s.h = h + (p.b + h * (x - p.c)) * p.dt;
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
