// The built-in Phoenix, z^2 + c + p z_prev, as a formula of your own
// (the Escape panel's Edit as Formula). The previous z is part of the
// state, carried from pass to pass, which deep zoom follows too.

const PARAMS = #{
    p_re: #{ value: -0.5, min: -2.0, max: 2.0, caption: "p (re)", hint: "Real part of the previous-iterate coefficient. The classic Phoenix Julia uses c = 0.5667, p = -0.5." },
    p_im: #{ value: 0.0, min: -2.0, max: 2.0, caption: "p (im)", hint: "Imaginary part of the previous-iterate coefficient." },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Phoenix", center: complex(0.0, 0.0), maxiter: 256 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c, prev: complex(0.0, 0.0) }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel, prev: complex(0.0, 0.0) }
    }
}

fn step(s, p) {
    let k = complex(p.p_re, p.p_im);
    #{ z: s.z * s.z + s.c + k * s.prev, prev: s.z }
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    2.0
}
