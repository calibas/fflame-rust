// Z-Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Z-Blur", name: "rhai_zblur" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let z_offset = rand() + rand() + rand() + rand() - 2.0;
    vec3(0.0, 0.0, z_offset)
}
