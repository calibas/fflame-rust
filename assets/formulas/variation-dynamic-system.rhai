// Dynamic System, Fractint's: a flow in the plane,
//   dx/dt = -sin(y + a sin(b y)),   dy/dt = sin(x + a sin(b x)),
// stepped by dt each pass. Ported from Iterated Dynamics (the GPL
// Fractint successor; libid/fractals/lorenz.cpp `dynamic_orbit` and
// `dynamic2d_per_image`, its defaults and view from fractalp.cpp). The
// program lets the inner sine be another function; this keeps the sine.
//
// Fractint steps both from the old point, or, with a negative time
// step, y first and then x from the new y (its "Euler" choice): Update
// below. It starts one orbit in each cell of a grid over its view (-20
// to 20). Use it alone, at weight 1, on one transform whose triangle is
// the identity: the chaos game iterates it, every thread's orbit from
// where the flame's starts, in [-1, 1]. An orbit follows its own flow
// line near where it starts, so the picture is the flow about the
// origin, about 14 units across. To see it whole, scale what is
// plotted, not the view: a final transform whose triangle shrinks it by
// about 4 / 14 (a final plots and does not feed the orbit), at a zoom
// near 1. Zooming the view out that far instead darkens it, as the log
// tone map divides by the view's area (Apophysis's too).

const PARAMS = #{
    dt: #{ value: 0.1, caption: "Time step", hint: "How far each pass flows" },
    a: #{ value: 1.0, caption: "a", hint: "How much the inner sine bends the flow" },
    b: #{ value: 3.0, caption: "b", hint: "The inner sine's frequency" },
    update: #{ value: 0, caption: "Update", choices: ["Both from the old point", "y first (Euler)"], hint: "Fractint's two orders: both parts from the old point, or y first and x from the new y (its negative time step)" },
};

const INFO = #{ title: "Dynamic System", name: "dynamic_system" };

fn variation(z, p) {
    let x = z.re;
    let y = z.im;
    let ny = y + p.dt * sin(x + p.a * sin(p.b * x));
    let yy = if p.update == 1 { ny } else { y };
    complex(x - p.dt * sin(yy + p.a * sin(p.b * yy)), ny)
}
