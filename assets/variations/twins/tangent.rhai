// Tangent, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Tangent", name: "rhai_tangent" };

fn variation(z, p) {
    let d = cos(z.im);
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(sin(z.re) * inv, tan(z.im))
}
