// Polar, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Polar", name: "rhai_polar" };

fn variation(z, p) {
    let theta = atan2(z.re, z.im);
    let r = z.abs();
    complex(theta / 3.14159265359, r - 1.0)
}
