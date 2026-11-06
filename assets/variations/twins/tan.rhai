// Tan, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Tan", name: "rhai_tan" };

fn variation(z, p) {
    let s2 = sin(2.0 * z.re);
    let c2 = cos(2.0 * z.re);
    let sh2 = sinh(2.0 * z.im);
    let ch2 = cosh(2.0 * z.im);
    let d = c2 + ch2;
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * s2, inv * sh2)
}
