// Dynamic System's orbits as an escape-time picture: each pixel starts an
// orbit of the map
//   dx/dt = -sin(y + a sin(b y)),   dy/dt = sin(x + a sin(b x)),
//   stepped by dt each pass
// (Iterated Dynamics' `dynamic_orbit`; the flame variation
// `variation-dynamic-system.rhai` draws the orbits themselves). Its
// orbits never leave: they fall onto its loops, so this is not the usual
// escape-time picture: the colouring reads how each pixel's orbit gets
// there -- Orbit Trap, the averages -- and the Radius only stops one that
// runs off. The colouring reads the orbit divided by Size, so its
// settings carry from map to map.

const PARAMS = #{
    dt: #{ value: 0.1, caption: "Time step", hint: "How far each pass flows" },
    a: #{ value: 1.0, caption: "a", hint: "How much the inner sine bends the flow" },
    b: #{ value: 3.0, caption: "b", hint: "The inner sine's frequency" },
    update: #{ value: 0, caption: "Update", choices: ["Both from the old point", "y first (Euler)"], hint: "Fractint's two orders: both parts from the old point, or y first and x from the new y (its negative time step)" },
    size: #{ value: 3.4, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: the orbit, in the map's units, is divided by it before the colouring sees it" },
    radius: #{ value: 1000.0, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from the origin stops" },
};

const INFO = #{ title: "Dynamic System (Orbits)", center: complex(0.0, 0.0), magn: 0.29411764705882354, maxiter: 200 };

// `m` is the orbit in the map's units; `z`, what the colouring reads,
// is it over Size.
fn start(pixel, p, ctx) {
    #{ z: pixel / p.size, m: pixel }
}

fn step(s, p) {
    let x = s.m.re;
    let y = s.m.im;
    let ny = y + p.dt * sin(x + p.a * sin(p.b * x));
    let yy = if p.update == 1 { ny } else { y };
    s.m = complex(x - p.dt * sin(yy + p.a * sin(p.b * yy)), ny);
    s.z = s.m / p.size;
    s
}

fn escaped(s, p) {
    s.m.norm_sqr() >= p.radius * p.radius
}
