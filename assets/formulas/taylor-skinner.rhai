// The Scott Taylor and Lee Skinner variations, Fractint's function-
// combining types, by Form, each from z = pixel:
// - fn(z*z):       z' = fn1(z^2)                      (bailout 4)
// - sqr(fn):       z' = fn1(z)^2                      (bailout 64)
// - sqr(1/fn):     z' = (1 / fn1(z))^2                (bailout 64)
// - fn*fn:         z' = fn1(z) fn2(z)                 (bailout 64)
// - fn+fn:         z' = p1 fn1(z) + p2 fn2(z)         (bailout 64)
// - fn*z+z:        z' = p1 z fn1(z) + p2 z            (bailout 64)
// - fn(z)+fn(pix): z' = fn1(z) + p2 fn2(pixel), from pixel + Z0 (64)
// Scott's are the + forms, Skinner's the - ones: fn+fn with p2 = -1,
// fn*z+z with p2 = -1. Ported from Iterated Dynamics (the GPL Fractint
// successor; libid/fractals/taylor_skinner_variations.cpp, the per-pixel
// setup in engine/fractals.cpp, defaults and bailouts from fractalp.cpp).
// The function menus start at the program's defaults, sin and sqr, and
// its reciprocal of 0 is (1e10, 1e10). Fractint counts fn(z)+fn(pix)'s
// start as its first iteration, so its counts there run one above these.

const PARAMS = #{
    form: #{ value: 0, caption: "Form", choices: ["fn(z*z)", "sqr(fn)", "sqr(1/fn)", "fn*fn", "fn+fn", "fn*z+z", "fn(z)+fn(pix)"], hint: "Which combination of the functions" },
    fn1: #{ function: "sin", caption: "fn1", hint: "The first function (the program's default, sin)" },
    fn2: #{ function: "sqr", caption: "fn2", hint: "The second function (the program's default, sqr)" },
    p1: #{ value: complex(1.0, 0.0), caption: "p1", hint: "fn+fn's and fn*z+z's first coefficient" },
    p2: #{ value: complex(1.0, 0.0), caption: "p2", hint: "The second coefficient: -1 makes Skinner's forms of fn+fn and fn*z+z" },
    z0: #{ value: complex(0.0, 0.0), caption: "Z0", hint: "Added to the pixel for fn(z)+fn(pix)'s start" },
};

const INFO = #{ title: "Taylor / Skinner Variations", center: complex(0.0, 0.0), magn: 1.0, maxiter: 150 };

// Fractint's reciprocal: (1e10, 1e10) at 0.
fn ts_recip(z) {
    if z.norm_sqr() == 0.0 { complex(1.0e10, 1.0e10) } else { recip(z) }
}

fn start(pixel, p, ctx) {
    if p.form == 6 {
        #{ z: pixel + p.z0, k: p.p2 * p.fn2(pixel) }
    } else {
        #{ z: pixel, k: complex(0.0, 0.0) }
    }
}

fn step(s, p) {
    let z = s.z;
    s.z = if p.form == 0 {
        p.fn1(z * z)
    } else if p.form == 1 {
        let f = p.fn1(z);
        f * f
    } else if p.form == 2 {
        let r = ts_recip(p.fn1(z));
        r * r
    } else if p.form == 3 {
        p.fn1(z) * p.fn2(z)
    } else if p.form == 4 {
        p.p1 * p.fn1(z) + p.p2 * p.fn2(z)
    } else if p.form == 5 {
        z * (p.p1 * p.fn1(z)) + p.p2 * z
    } else {
        p.fn1(z) + s.k
    };
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() >= 64.0 || (p.form == 0 && s.z.norm_sqr() >= 4.0)
}
