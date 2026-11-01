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
//
// It colours as a direct-colour variation: each point's palette index
// follows its step or its orbit's iteration (the Color choice), and the
// transform's Direct Color sets how much of that shows (0 gives back
// the transform's colour).
//
// A flame using it starts its orbits afresh every dispatch
// (`resets_orbits` in INFO), each its Orbit Length long: how far an
// orbit goes, saved with the flame in place of Iterations per Thread.
// Max Iterations is how many are drawn.

const PARAMS = #{
    a: #{ value: 2.24, caption: "a", hint: "The frequency of y in x's sine" },
    b: #{ value: 0.43, caption: "b", hint: "The frequency of x in x's cosine" },
    c: #{ value: -0.65, caption: "c", hint: "The frequency of x in y's sine" },
    d: #{ value: -2.43, caption: "d", hint: "The frequency of y in y's cosine" },
    orbit_length: #{ value: 1024, min: 16, max: 10000, caption: "Orbit Length", hint: "How many iterations each orbit runs from a fresh start: how far it goes. Saved with the flame, in place of Iterations per Thread" },
    color: #{ value: 0, caption: "Color", choices: ["Direction", "Step length", "Distance", "Height", "Iteration"], hint: "What the palette index follows: the step's direction (its turn about the z axis), its length, the new point's distance from the origin, its height, or the orbit's iteration since it started" },
    color_cycles: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction), per Color Size (Step length, Distance, Height), or per orbit (Iteration)" },
    color_size: #{ value: 5.0, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length, Distance and Height (the attractor spans about 5 at the defaults)" },
    color_offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" },
};

const INFO = #{ title: "Pickover Attractor", name: "pickover_attractor", resets_orbits: true };

// Each thread's count of its orbit's iterations, for the Iteration colour.
fn init(p) {
    #{ n: 0.0 }
}

// The palette index for the step from v to w, as the Color choice says.
fn pickover_color(v, w, mode, cycles, size, offset, t) {
    let d = w - v;
    let q = if mode == 0 {
        atan2(d.y, d.x) / (2.0 * PI()) + 0.5
    } else if mode == 1 {
        length(d) / size
    } else if mode == 2 {
        length(w) / size
    } else if mode == 3 {
        w.z / size
    } else {
        t
    };
    let k = offset + cycles * q;
    k - floor(k)
}

fn variation3(v, p) {
    this.n = this.n + 1.0;
    let w = vec3(sin(p.a * v.y) - v.z * cos(p.b * v.x), v.z * sin(p.c * v.x) - cos(p.d * v.y), sin(v.x));
    #{ z: w, color: pickover_color(v, w, p.color, p.color_cycles, p.color_size, p.color_offset, this.n / p.orbit_length.to_float()) }
}
