// MandelbrotMix4, Jim Muth's: a mix of two powers of z,
//   z' = k (a z^b + d z^f) + c,   c = fn1(pixel),
// every pixel starting from the same z0 = (-a b / (f d))^(1 / (f - b)),
// where p1 = a + b i, p2 = d + f i, and p3's real part adds to k = 1 and
// its imaginary part to the bailout of 100 (the setup's, which the
// program sets after its table's 4, as far as its call order shows).
// Ported from Iterated Dynamics (the GPL Fractint successor;
// libid/fractals/mandelbrot_mix.cpp, its defaults and view from
// fractalp.cpp). The program's function menu starts at sin (its default
// functions are sin, sqr, sinh, cosh).
//
// Fractint's powers: 0 to any power is 0, and otherwise exp(w ln z); its
// reciprocal of 0 is (1e10, 1e10). z0's base, -a b (1/f) (1/d), is
// usually a negative real, and the program leaves its imaginary part a
// signed zero, whose sign picks the log's branch (+pi or -pi): the sign
// the products' rounding gives it, flipped for four sign patterns of a,
// b, f and d to match Jim Muth's parser formula. A GPU does not keep the
// sign of a zero, so this works the sign out and takes the branch itself
// (at the defaults, -pi).

const PARAMS = #{
    p1: #{ value: complex(0.05, 3.0), caption: "p1 (a, b)", hint: "a, the first term's weight, and b, its power of z" },
    p2: #{ value: complex(-1.5, -2.0), caption: "p2 (d, f)", hint: "d, the second term's weight, and f, its power of z" },
    p3: #{ value: complex(0.0, 0.0), caption: "p3 (k - 1, bailout - 100)", hint: "Added to 1 for k, the mix's scale, and to 100 for the bailout" },
    fn1: #{ function: "sin", caption: "fn1", hint: "The function of the pixel that is c" },
};

const INFO = #{ title: "MandelbrotMix4", center: complex(-0.5, 0.0), magn: 1.3333333333333333, maxiter: 150 };

// Fractint's complex power (cmplx_pwr): 0 for a zero base.
fn mix4_pow(z, w) {
    if z.norm_sqr() == 0.0 { complex(0.0, 0.0) } else { exp(w * ln(z)) }
}

// Fractint's reciprocal of a real x (cmplx_recip): (1e10, 1e10) at 0.
fn mix4_recip(x) {
    if x == 0.0 { complex(1.0e10, 1.0e10) } else { complex(1.0 / x, 0.0) }
}

// Whether the program's zero imaginary part of z0's base is -0: its
// products, (-a)(b), then (1/f), then (1/d), each reciprocal's imaginary
// part -0, under IEEE's signs of zero; then the kludge's flips (a, b,
// 1/f, 1/d negative making 8, 4, 2, 1).
fn mix4_below(a, b, f, d) {
    let after_b = a > 0.0 && b > 0.0;
    let after_f = a * b < 0.0 && f < 0.0;
    let after_d = -a * b / f > 0.0 && (after_f != (d < 0.0));
    let signs = (if a < 0.0 { 8 } else { 0 }) + (if b < 0.0 { 4 } else { 0 }) + (if f < 0.0 { 2 } else { 0 }) + (if d < 0.0 { 1 } else { 0 });
    let flip = signs == 15 || signs == 12 || signs == 10 || signs == 9;
    after_d != flip
}

fn start(pixel, p, ctx) {
    let a = p.p1.re;
    let b = p.p1.im;
    let d = p.p2.re;
    let f = p.p2.im;
    let j = mix4_recip(f - b);
    let base = complex(-a * b, 0.0) * mix4_recip(f) * mix4_recip(d);
    let z0 = if base.im == 0.0 && base.re < 0.0 {
        let turn = if mix4_below(a, b, f, d) { -PI() } else { PI() };
        exp(complex(ln(-base.re), turn) * j)
    } else {
        mix4_pow(base, j)
    };
    #{ z: z0, c: p.fn1(pixel) }
}

fn step(s, p) {
    let k = p.p3.re + 1.0;
    let zb = mix4_pow(s.z, complex(p.p1.im, 0.0));
    let zf = mix4_pow(s.z, complex(p.p2.im, 0.0));
    s.z = zb * (k * p.p1.re) + zf * (k * p.p2.re) + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() >= p.p3.im + 100.0
}
