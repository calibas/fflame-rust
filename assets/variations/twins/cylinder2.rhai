// Cylinder 2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cylinder 2", name: "rhai_cylinder2" };

fn variation(z, p) {
    complex(z.re / sqrt(z.re * z.re + 1.0), z.im)
}
