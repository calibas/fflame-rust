// The built-in Cactus, z^3 + (c - 1) z - c, as a formula of your own
// (the Escape panel's Edit as Formula). On the parameter plane the
// orbit starts at the pixel.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Cactus", center: complex(0.0, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: pixel, c: pixel }
    }
}

fn step(s, p) {
    let z2 = s.z * s.z;
    let z3 = z2 * s.z;
    s.z = z3 + (s.c - 1.0) * s.z - s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    3.0
}
