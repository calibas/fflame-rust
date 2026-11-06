/// Shifts the Z coordinate during the variation pass. The variation's
/// weight is the offset — set the weight to control how far each point
/// moves up or down.

// ZTranslate, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ZTranslate", name: "rhai_ztranslate", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    vec3(0.0, 0.0, 1.0)
}
