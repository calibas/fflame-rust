/// Rotates the output point around the Y axis after all other variations
/// have run. The rotation angle (in radians) is the variation's own weight
/// slider.

// Post Rotate Y, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Post Rotate Y", name: "rhai_post_rotate_y", category: "rotation_3d", phase: "post" };

fn variation(z, p) {
    z
}

fn variation3(v, p, ctx) {
    let weight = ctx.weight;
    let c = cos(weight);
    let s = sin(weight);
    vec3(c * v.x - s * v.z, v.y, s * v.x + c * v.z)
}
