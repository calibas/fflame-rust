// The built-in Novaretti, -6z(z^3 + c) / (2z^3 - c)^2 (Elena Novaretti,
// ZoneXplorer), as a formula of your own (the Escape panel's Edit as
// Formula). Nothing escapes (infinity maps to 0): an orbit ends only as
// it settles. The parameter plane starts at a critical point,
// z^3 = c (-7 + 3 sqrt 5) / 4.

const INFO = #{ title: "Novaretti", center: complex(0.0, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c, prev: pixel }
    } else {
        let z0 = (pixel * -0.0729490168) ** 0.3333333333;
        #{ z: z0, c: pixel, prev: z0 }
    }
}

fn step(s, p) {
    s.prev = s.z;
    let z3 = s.z * s.z * s.z;
    let den_root = 2.0 * z3 - s.c;
    let den = den_root * den_root;
    // The built-in's double pole: a large value the next step maps back
    // toward 0, small enough that the step after stays within f32.
    s.z = if den.norm_sqr() < 1e-24 { complex(1e6, 0.0) } else { -6.0 * (s.z * (z3 + s.c)) / den };
    s
}

fn escaped(s, p) {
    (s.z - s.prev).norm_sqr() < 1e-12
}
