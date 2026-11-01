// KAM Torus: an area-preserving quadratic map, a turn by the angle of
// the point bent by x squared:
//   x' = x cos(a) - (y - x^2) sin(a),   y' = x sin(a) + (y - x^2) cos(a).
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/lorenz.cpp `kam_torus_orbit`, its defaults and view
// from fractalp.cpp).
//
// The map keeps area, so an orbit never settles: near the centre each
// stays on its own closed curve (a KAM torus), further out they break
// into island chains and chaos. Fractint starts orbit after orbit along
// the line x = y, a step apart, for so many points each. Use it alone,
// at weight 1, on one transform whose triangle is the identity: the
// chaos game iterates it, every thread's orbit from where the flame's
// starts, in [-1, 1]. The picture is the tori and the chaos between
// them, about 2 units across (Fractint's view is -1 to 1 by -0.75 to
// 0.75); orbits that start outside run off and the flame restarts them.
//
// It colours as a direct-colour variation: each point's palette index
// follows its step (the Color choice), and the transform's Direct Color
// sets how much of that shows (0 gives back the transform's colour).

const PARAMS = #{
    angle: #{ value: 1.3, caption: "Angle", hint: "The turn each pass, in radians (Fractint's first parameter)" },
    color: #{ value: 0, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle"], hint: "What the palette index follows: the step's direction, its length, the new point's distance from the origin, or its angle about it" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), or per Color Size (Step length, Distance)" },
    color_size: #{ value: 2.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length and Distance (the picture spans about 2 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "KAM Torus", name: "kamtorus" };

// The palette index for the step from z to w, as the Color choice says.
fn kamtorus_color(z, w, mode, cycles, size, offset) {
    let d = w - z;
    let q = if mode == 0 {
        atan2(d.im, d.re) / (2.0 * PI()) + 0.5
    } else if mode == 1 {
        d.abs() / size
    } else if mode == 2 {
        w.abs() / size
    } else {
        atan2(w.im, w.re) / (2.0 * PI()) + 0.5
    };
    let k = offset + cycles * q;
    k - floor(k)
}

fn variation(z, p) {
    let x = z.re;
    let s = z.im - x * x;
    let sa = sin(p.angle);
    let ca = cos(p.angle);
    let w = complex(x * ca - s * sa, x * sa + s * ca);
    #{ z: w, color: kamtorus_color(z, w, p.color, p.color_cycles, p.color_size, p.color_offset) }
}
