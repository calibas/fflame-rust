/// Multiplies the Z coordinate by the variation's weight slider. Combine
/// with other 3D variations to amplify or dampen the depth they
/// contribute.

// ZScale, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ZScale", name: "rhai_zscale", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    vec3(0.0, 0.0, v.z)
}
