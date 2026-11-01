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
//
// It colours as a direct-colour variation: each point's palette index
// follows its step (the Color choice), and the transform's Direct Color
// sets how much of that shows (0 gives back the transform's colour).

const PARAMS = #{
    dt: #{ value: 0.04, caption: "Time step", hint: "How far each pass flows" },
    a: #{ value: 0.2, caption: "a", hint: "How much y feeds its own growth" },
    b: #{ value: 0.2, caption: "b", hint: "z's constant growth" },
    c: #{ value: 5.7, caption: "c", hint: "How far x must climb before z grows" },
    color: #{ value: 0, caption: "Color", choices: ["Direction", "Step length", "Distance", "Height"], hint: "What the palette index follows: the step's direction (its turn about the z axis), its length over the Time step (the flow's speed), the new point's distance from the origin, or its height" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction), or per Color Size (Step length, Distance, Height)" },
    color_size: #{ value: 30.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Distance and Height, and the speed it spans for Step length (the attractor spans about 30 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Rossler Attractor", name: "rossler_attractor" };

// The palette index for the step from v to w, as the Color choice says;
// `dt` the time step a step takes.
fn rossler_color(v, w, mode, cycles, size, offset, dt) {
    let d = w - v;
    let q = if mode == 0 {
        atan2(d.y, d.x) / (2.0 * PI()) + 0.5
    } else if mode == 1 {
        if dt > 0.0 { length(d) / (dt * size) } else { 0.0 }
    } else if mode == 2 {
        length(w) / size
    } else {
        w.z / size
    };
    let k = offset + cycles * q;
    k - floor(k)
}

fn variation3(v, p) {
    let w = vec3(
        v.x - (v.y + v.z) * p.dt,
        v.y + (v.x + p.a * v.y) * p.dt,
        v.z + (p.b + v.z * (v.x - p.c)) * p.dt
    );
    #{ z: w, color: rossler_color(v, w, p.color, p.color_cycles, p.color_size, p.color_offset, p.dt.abs()) }
}
