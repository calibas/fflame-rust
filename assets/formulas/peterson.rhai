// Mark Peterson's variations, Fractint's four types, by Form:
// - Marks Mandel: z' = z0^(n-1) z^2 + c, the coefficient taken once from
//   the start z0 = pixel + Z0 (n the Exponent; 1 below 2);
// - Marks Julia: the same with the Constant for c and z0 = pixel, the
//   coefficient Constant^(n-1);
// - Marks Mandel Pwr: z' = t fn1(z) + pixel, t = z0^(z0 - 1);
// - Tim's Error: Marks Mandel Pwr's product written with its slip kept,
//   the imaginary part computed from the real part already replaced.
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/peterson_variations.cpp and the per-pixel setup in
// engine/fractals.cpp, their defaults from fractalp.cpp). The program's
// header comment for Marks Mandel Pwr writes sqr(z), but its code applies
// the function menu: this does as the code does. Fractint counts the
// start of the Mandel forms as their first iteration, so its counts can
// run one above these.
//
// t is exp((z0 - 1) ln z0) here, with no test for z0 = 0, where the
// program's power gives 0: a test on the pixel in the start would keep
// the formula from deep zoom. At that one point t is not a number, and
// the pixel stays inside, as the program's does unless |Z0| >= 2.

const PARAMS = #{
    form: #{ value: 0, caption: "Form", choices: ["Marks Mandel", "Marks Julia", "Marks Mandel Pwr", "Tim's Error"], hint: "Which of the four types" },
    z0: #{ value: complex(0.0, 0.0), caption: "Z0", hint: "Added to the pixel for the Mandel forms' start" },
    constant: #{ value: complex(0.1, 0.9), caption: "Constant", hint: "Marks Julia's c" },
    exponent: #{ value: 1, min: 1, max: 20, caption: "Exponent", hint: "n in the Marks forms' coefficient, start^(n - 1)" },
    fn1: #{ function: "sin", caption: "fn1", hint: "The function of z in Marks Mandel Pwr and Tim's Error (the program's default, sin)" },
};

const INFO = #{ title: "Mark Peterson's Variations", center: complex(-0.5, 0.0), magn: 1.3333333333333333, maxiter: 150 };

// The Marks forms' coefficient is base^(n - 1), 1 below 2 (the program
// raises an Exponent under 1 to 1).
fn start(pixel, p, ctx) {
    if p.form == 1 {
        #{ z: pixel, k: p.constant ** (max(p.exponent, 1) - 1), c: p.constant }
    } else if p.form == 0 {
        let z = pixel + p.z0;
        #{ z: z, k: z ** (max(p.exponent, 1) - 1), c: pixel }
    } else {
        let z = pixel + p.z0;
        #{ z: z, k: exp((z - complex(1.0, 0.0)) * ln(z)), c: pixel }
    }
}

fn step(s, p) {
    if p.form <= 1 {
        s.z = s.k * (s.z * s.z) + s.c;
    } else if p.form == 2 {
        s.z = s.k * p.fn1(s.z) + s.c;
    } else {
        let f = p.fn1(s.z);
        let x = f.re * s.k.re - f.im * s.k.im;
        let y = x * s.k.im - f.im * s.k.re;
        s.z = complex(x, y) + s.c;
    }
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() >= 4.0
}
