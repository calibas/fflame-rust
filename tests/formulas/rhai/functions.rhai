// The function names, as Rhai spells them, as functions and as methods:
// small arguments, so two passes stay finite.

const INFO = #{ title: "Functions" };

fn start(pixel, p) {
    #{ z: pixel, acc: 0.0 }
}

fn step(s, p) {
    let z = s.z;
    let a = sin(z) + cos(z) * 0.5 + z.tan() * 0.1 + sinh(z) * 0.1 + cosh(z) * 0.1 + tanh(z) * 0.1;
    let b = asin(z * 0.1) + acos(z * 0.1) * 0.1 + atan(z) * 0.1 + asinh(z) * 0.1 + atanh(z * 0.1) * 0.1;
    let c = exp(z * 0.1) + ln(z + 2.0) + sqrt(z + 2.0) + recip(z + 3.0) + conj(z) + flip(z) + z.sqr() * 0.1;
    let d = z.abs() + abs(z.re) + z.arg() + atan2(z.im, z.re) + z.norm_sqr() + log(10.0) + round(1.4) + floor(1.6) + ceiling(1.2) + trunc(1.7);
    let e = pow(z, 2) * 0.1 + z ** 3 * 0.01 + fold(z) * 0.1 + ident(z) * 0.1 + zero(z) + cot(z + 1.0) * 0.01 + coth(z + 1.0) * 0.01;
    s.acc += d * 0.001;
    s.z = (a + b + c + e) * 0.01 + complex(s.acc, 0.0);
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > 1e6
}
