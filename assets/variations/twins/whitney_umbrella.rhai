// Whitney Umbrella, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Whitney Umbrella", name: "rhai_whitney_umbrella" };

fn variation(z, p) {
    complex(z.re * z.im, z.re)
}

fn variation3(v, p) {
    vec3(v.x * v.y, v.x, v.y * v.y)
}
