// The built-in Newton, a root finder on the dynamical plane: one of six
// functions, by Newton's, Halley's or Chebyshev's scheme, with a complex
// relaxation, as a formula of your own (the Escape panel's Edit as
// Formula). An orbit ends as it settles on a root or as it escapes.

const PARAMS = #{
    power: #{ value: 3.0, min: 2.0, max: 12.0, caption: "Power", hint: "Roots of z^p - 1: p basins of attraction." },
    scheme: #{
        value: 0,
        caption: "Scheme",
        hint: "0: Newton, 1: Halley, 2: Chebyshev.",
        choices: ["Newton", "Halley", "Chebyshev"],
    },
    relax_re: #{ value: 1.0, min: -3.0, max: 3.0, caption: "Relaxation (re)", hint: "Complex relaxation R multiplying the step. 1+0i is the plain scheme; the interesting galleries live away from it." },
    relax_im: #{ value: 0.0, min: -3.0, max: 3.0, caption: "Relaxation (im)", hint: "Imaginary part of the relaxation." },
    func: #{
        value: 0,
        caption: "Function",
        hint: "Which f(z) to find the roots of. 0: z^p - 1 (roots of unity), 1: z^3 - 2z + 2 (Newton's classic failure case), 2: z^8 + 15z^4 - 16, 3: sin z - 1, 4: cosh z - 1, 5: z^p sin z - 1. Only 0 has roots evenly spaced on a circle - for the others set the coloring's Basin Key to General.",
        choices: ["z^p - 1", "z^3 - 2z + 2", "z^8 + 15z^4 - 16", "sin z - 1", "cosh z - 1", "z^p sin z - 1"],
    },
    bailout: #{ value: 10000.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Newton", center: complex(0.0, 0.0), maxiter: 64 };

fn start(pixel, p, ctx) {
    #{ z: pixel, prev: pixel }
}

// The built-in's division: a large value at a pole, which escapes.
fn cdiv(a, b) {
    if b.norm_sqr() < 1e-30 { complex(1e20, 0.0) } else { a / b }
}

fn step(s, p) {
    s.prev = s.z;
    let z = s.z;
    let k = p.power;
    // f, f' and f'' of the chosen function.
    let f = complex(0.0, 0.0);
    let fp = complex(0.0, 0.0);
    let fpp = complex(0.0, 0.0);
    if p.func == 1 {
        let z2 = z * z;
        f = z2 * z - 2.0 * z + 2.0;
        fp = 3.0 * z2 - 2.0;
        fpp = 6.0 * z;
    } else if p.func == 2 {
        let z2 = z * z;
        let z3 = z2 * z;
        let z4 = z2 * z2;
        let z6 = z4 * z2;
        f = z4 * z4 + 15.0 * z4 - 16.0;
        fp = 8.0 * (z6 * z) + 60.0 * z3;
        fpp = 56.0 * z6 + 180.0 * z2;
    } else if p.func == 3 {
        f = sin(z) - 1.0;
        fp = cos(z);
        fpp = -sin(z);
    } else if p.func == 4 {
        f = cosh(z) - 1.0;
        fp = sinh(z);
        fpp = cosh(z);
    } else if p.func == 5 {
        let sn = sin(z);
        let cs = cos(z);
        let zp = z ** k;
        let zp1 = z ** (k - 1.0);
        f = zp * sn - 1.0;
        fp = k * (zp1 * sn) + zp * cs;
        fpp = k * (k - 1.0) * (z ** (k - 2.0) * sn) + 2.0 * k * (zp1 * cs) - zp * sn;
    } else {
        f = z ** k - 1.0;
        fp = k * z ** (k - 1.0);
        fpp = k * (k - 1.0) * z ** (k - 2.0);
    }
    let delta = if p.scheme == 1 {
        // Halley: 2 f f' / (2 f'^2 - f f'')
        cdiv(2.0 * (f * fp), 2.0 * (fp * fp) - f * fpp)
    } else if p.scheme == 2 {
        // Chebyshev: (f / f') (1 + f f'' / (2 f'^2))
        cdiv(f, fp) * (1.0 + cdiv(f * fpp, 2.0 * (fp * fp)))
    } else {
        cdiv(f, fp)
    };
    s.z = z - complex(p.relax_re, p.relax_im) * delta;
    s
}

fn escaped(s, p) {
    (s.z - s.prev).norm_sqr() < 1e-12 || s.z.norm_sqr() > p.bailout
}
