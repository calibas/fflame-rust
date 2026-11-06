// Rippled, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Rippled", name: "rhai_rippled" };

fn variation(z, p) {
    let d = z.re * z.re + z.im * z.im + 1.0e-30;
    complex(tanh(d) * z.re, cos(d) * z.im)
}
