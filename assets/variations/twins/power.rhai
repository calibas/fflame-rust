// Power, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Power", name: "rhai_power" };

fn variation(z, p) {
    let r = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let exponent = z.re / r;
    let m = pow(r, exponent);
    complex(m * z.im / r, m * z.re / r)
}
