/// Rotates the point around the X axis before any other variations run.
/// The rotation angle (in radians) is the variation's own weight slider —
/// e.g. set the weight to 1.57 for a 90° rotation.

// Pre Rotate X, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre Rotate X", name: "rhai_pre_rotate_x", category: "rotation_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p) {
    z
}

fn variation3(v, p, ctx) {
    let weight = ctx.weight;
    let c = cos(weight);
    let s = sin(weight);
    vec3(v.x, s * v.z + c * v.y, c * v.z - s * v.y)
}
