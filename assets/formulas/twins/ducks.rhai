// The built-in Ducks (Kali-log), log of the iterate with its imaginary
// part folded, as a formula of your own (the Escape panel's Edit as
// Formula). It never escapes: the colourings that read the orbit draw
// it.

const PARAMS = #{
    variant: #{
        value: 0,
        caption: "Variant",
        hint: "0 classic log(f+c); 1 log(sin(f+c)); 2 log(f+c - sec(f+c)); 3 log(sin(f)+c); 4 log((f+c)^2). f is z with |Im z|.",
        choices: ["log(f+c)", "log(sin(f+c))", "log(f+c - sec(f+c))", "log(sin(f)+c)", "log((f+c)^2)"],
    },
};

const INFO = #{ title: "Ducks (Kali-log)", center: complex(0.0, 0.0), maxiter: 80 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

// The built-in's log, to the rounding (half the log of |w|^2, which a
// chaotic map amplifies): a large negative real at zero, not -inf.
// ln(w.norm_sqr()) in one expression deep-zooms as the real part of
// log w, whose rounding near zero is the square root of the modulus'.
fn clog(w) {
    if w.norm_sqr() < 1e-30 { complex(-34.5, 0.0) } else { complex(0.5 * ln(w.norm_sqr()), w.arg()) }
}

// The built-in's division: a large value at a pole, not inf.
fn crecip(w) {
    if w.norm_sqr() < 1e-30 { complex(1e20, 0.0) } else { 1.0 / w }
}

fn step(s, p) {
    let f = complex(s.z.re, s.z.im.abs());
    let t = f + s.c;
    s.z = if p.variant == 3 {
        clog(sin(f) + s.c)
    } else if p.variant == 1 {
        clog(sin(t))
    } else if p.variant == 2 {
        clog(t - crecip(cos(t)))
    } else if p.variant == 4 {
        clog(t * t)
    } else {
        clog(t)
    };
    s
}
