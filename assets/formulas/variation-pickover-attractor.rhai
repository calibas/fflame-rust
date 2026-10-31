// Pickover's attractor, a map of space:
//   x' = sin(a y) - z cos(b x),
//   y' = z sin(c x) - cos(d y),
//   z' = sin(x).
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/lorenz.cpp `pickover_orbit`, its defaults and view from
// fractalp.cpp).
//
// A 3D variation: use it in a 3D flame, alone, at weight 1, on one
// transform whose triangle is the identity. The chaos game iterates it
// and the orbits settle on the attractor, about 5 units across
// (Fractint's view is -8/3 to 8/3 by -2 to 2); turn the camera to see
// its depth.

const PARAMS = #{
    a: #{ value: 2.24, caption: "a", hint: "The frequency of y in x's sine" },
    b: #{ value: 0.43, caption: "b", hint: "The frequency of x in x's cosine" },
    c: #{ value: -0.65, caption: "c", hint: "The frequency of x in y's sine" },
    d: #{ value: -2.43, caption: "d", hint: "The frequency of y in y's cosine" },
};

const INFO = #{ title: "Pickover Attractor", name: "pickover_attractor" };

fn variation3(v, p) {
    vec3(sin(p.a * v.y) - v.z * cos(p.b * v.x), v.z * sin(p.c * v.x) - cos(p.d * v.y), sin(v.x))
}
