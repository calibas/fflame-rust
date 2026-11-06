// Sin, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sin", name: "rhai_sin" };

fn variation(z, p) {
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(z.im);
    let ch = cosh(z.im);
    complex(s * ch, c * sh)
}
