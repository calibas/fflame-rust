// The built-in Lambda (the logistic map, c z (1 - z)), as a formula of
// your own (the Escape panel's Edit as Formula). On the parameter plane
// the orbit starts at the critical point, 1/2.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Lambda (Logistic)", center: complex(0.5, 0.0), maxiter: 256 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.5, 0.0), c: pixel }
    }
}

fn step(s, p) {
    s.z = s.c * (s.z * (1.0 - s.z));
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    2.0
}
