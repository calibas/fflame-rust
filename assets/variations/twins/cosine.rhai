// Cosine, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cosine", name: "rhai_cosine" };

fn variation(z, p) {
    let r = z.re * 3.14159265358979;
    complex(cos(r) * cosh(z.im), -sin(r) * sinh(z.im))
}
