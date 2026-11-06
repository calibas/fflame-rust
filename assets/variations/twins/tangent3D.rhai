// Tangent 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Tangent 3D", name: "rhai_tangent3d" };

fn variation(z, p) {
    let d = cos(z.im);
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(sin(z.re) * inv, tan(z.im))
}

fn variation3(v, p) {
    let d = cos(v.y);
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    vec3(sin(v.x) * inv, tan(v.y), tan(v.x))
}
