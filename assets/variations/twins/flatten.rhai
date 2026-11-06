/// Cancels the Z coordinate, projecting everything back onto the XY
/// plane. Runs in the post phase to undo accumulated depth from earlier
/// 3D variations.

// Flatten, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Flatten", name: "rhai_flatten", category: "depth_3d", phase: "post" };

fn variation(z, p) {
    z
}

fn variation3(v, p) {
    vec3(v.x, v.y, 0.0)
}
