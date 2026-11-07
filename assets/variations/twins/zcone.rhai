/// Lifts every point along the Z axis by its horizontal distance from
/// the origin. Turns a flat XY input into a cone whose tip sits at the
/// origin and whose base spreads outward.

// ZCone, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "ZCone", name: "rhai_zcone", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    vec3(0.0, 0.0, complex(v.x, v.y).abs())
}
