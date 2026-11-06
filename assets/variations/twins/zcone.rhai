// ZCone, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ZCone", name: "rhai_zcone" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    vec3(0.0, 0.0, complex(v.x, v.y).abs())
}
