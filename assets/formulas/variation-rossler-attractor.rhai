// The Rossler attractor, a flow in space,
//   dx/dt = -(y + z),   dy/dt = x + a y,   dz/dt = b + z (x - c),
// stepped by dt each pass. Ported from Iterated Dynamics (the GPL
// Fractint successor; libid/fractals/lorenz.cpp `rossler_orbit`, an
// explicit step from the old point, its defaults and view from
// fractalp.cpp). Not the Simulation panel's Rossler model, which couples
// a grid of these oscillators.
//
// A 3D variation: use it in a 3D flame, alone, at weight 1, on one
// transform whose triangle is the identity. The chaos game steps every
// thread's orbit along the flow; they wind onto the attractor's band,
// some 30 units across (Fractint's view is -30 to 30 by -20 to 40). To
// see it whole, scale what is plotted, not the view: a final transform
// whose triangle shrinks it by about 4 / 30 (a final plots and does not
// feed the orbit), at a zoom near 1. Zooming the view out that far
// instead darkens it, as the log tone map divides by the view's area
// (Apophysis's too).

const PARAMS = #{
    dt: #{ value: 0.04, caption: "Time step", hint: "How far each pass flows" },
    a: #{ value: 0.2, caption: "a", hint: "How much y feeds its own growth" },
    b: #{ value: 0.2, caption: "b", hint: "z's constant growth" },
    c: #{ value: 5.7, caption: "c", hint: "How far x must climb before z grows" },
};

const INFO = #{ title: "Rossler Attractor", name: "rossler_attractor" };

fn variation3(v, p) {
    vec3(
        v.x - (v.y + v.z) * p.dt,
        v.y + (v.x + p.a * v.y) * p.dt,
        v.z + (p.b + v.z * (v.x - p.c)) * p.dt
    )
}
