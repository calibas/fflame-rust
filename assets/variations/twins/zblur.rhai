/// Adds gaussian random noise on the Z axis only — XY position is kept,
/// Z is replaced with a bell-curve random value. Useful for softening
/// the depth profile of an otherwise crisp 3D shape.

// Z-Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Z-Blur", name: "rhai_zblur", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let z_offset = rand() + rand() + rand() + rand() - 2.0;
    vec3(0.0, 0.0, z_offset)
}
