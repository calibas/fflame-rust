// Butterfly, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Butterfly", name: "rhai_butterfly" };

fn variation(z, p) {
    let k = 1.302940031741119;
    let y2 = z.im * 2.0;
    let r = k * sqrt(abs(z.im * z.re) / (1.0e-30 + z.re * z.re + y2 * y2));
    complex(r * z.re, r * y2)
}
