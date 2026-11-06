/// Rotates the output point around the X axis after all other variations
/// have run. The rotation angle (in radians) is the variation's own weight
/// slider.

// Post Rotate X, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Post Rotate X", name: "rhai_post_rotate_x", category: "rotation_3d", phase: "post" };

fn variation(z, p) {
    z
}

fn variation3(v, p, ctx) {
    let weight = ctx.weight;
    let c = cos(weight);
    let s = sin(weight);
    vec3(v.x, s * v.z + c * v.y, c * v.z - s * v.y)
}
