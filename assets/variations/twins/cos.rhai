// Cos, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cos", name: "rhai_cos" };

fn variation(z, p) {
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(z.im);
    let ch = cosh(z.im);
    complex(c * ch, -s * sh)
}
