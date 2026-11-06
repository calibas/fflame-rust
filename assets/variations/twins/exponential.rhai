// Exponential, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Exponential", name: "rhai_exponential" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let r = pi * z.im;
    let d = exp(z.re - 1.0);
    complex(cos(r) * d, sin(r) * d)
}
