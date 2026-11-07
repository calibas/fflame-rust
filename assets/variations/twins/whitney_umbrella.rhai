/// Parametric Whitney umbrella surface — output is `(xy, x, y²)`. The
/// classical algebraic surface with the same name.
///
/// # Authors
/// - Don Town

// Whitney Umbrella, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Whitney Umbrella", name: "rhai_whitney_umbrella", category: "full_3d", phase: "any" };

fn variation(z, p) {
    complex(z.re * z.im, z.re)
}

fn variation3(v, p) {
    vec3(v.x * v.y, v.x, v.y * v.y)
}
