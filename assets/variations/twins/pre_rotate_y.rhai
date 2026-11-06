/// Rotates the point around the Y axis before any other variations run.
/// The rotation angle (in radians) is the variation's own weight slider.

// Pre Rotate Y, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre Rotate Y", name: "rhai_pre_rotate_y", category: "rotation_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p) {
    z
}

fn variation3(v, p, ctx) {
    let weight = ctx.weight;
    let c = cos(weight);
    let s = sin(weight);
    vec3(c * v.x - s * v.z, v.y, s * v.x + c * v.z)
}
