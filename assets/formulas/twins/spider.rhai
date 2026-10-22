// The built-in Spider (Fractint's), as a formula of your own (the Escape
// panel's Edit as Formula): z^2 + c, then c moves halfway to where z
// went. c is carried from pass to pass, which deep zoom follows too.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Spider", center: complex(-0.4, 0.0), maxiter: 256 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s.c = s.c * 0.5 + s.z;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    2.0
}
