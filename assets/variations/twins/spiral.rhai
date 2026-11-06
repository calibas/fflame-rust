// Spiral, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Spiral", name: "rhai_spiral" };

fn variation(z, p) {
    let r = z.abs() + 1.0e-6;
    let theta = atan2(z.re, z.im);
    let r_inv = 1.0 / r;
    complex(r_inv * (cos(theta) + sin(r)), r_inv * (sin(theta) - cos(r)))
}
