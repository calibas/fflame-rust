// Bi-Linear, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Bi-Linear", name: "rhai_bi_linear" };

fn variation(z, p) {
    complex(z.im, z.re)
}
