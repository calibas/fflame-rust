// Horseshoe, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Horseshoe", name: "rhai_horseshoe" };

fn variation(z, p) {
    let r = z.abs() + 1.0e-6;
    let r_inv = 1.0 / r;
    complex((z.re - z.im) * (z.re + z.im) * r_inv, 2.0 * z.re * z.im * r_inv)
}
