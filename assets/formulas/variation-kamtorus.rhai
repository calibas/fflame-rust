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

const PARAMS = #{
    angle: #{ value: 1.3, caption: "Angle", hint: "The turn each pass, in radians (Fractint's first parameter)" },
};

const INFO = #{ title: "KAM Torus", name: "kamtorus" };

fn variation(z, p) {
    let x = z.re;
    let s = z.im - x * x;
    let sa = sin(p.angle);
    let ca = cos(p.angle);
    complex(x * ca - s * sa, x * sa + s * ca)
}
