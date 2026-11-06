/// Wraps the X coordinate around a cylinder (sine), passes Y through
/// unchanged. In 3D, adds a cosine of X as the Z coordinate so the plane
/// really wraps into a cylindrical sheet.
///
/// # Authors
/// - Scott Draves

// Cylinder, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cylinder", name: "rhai_cylinder", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    complex(sin(z.re), z.im)
}

fn variation3(v, p) {
    vec3(sin(v.x), v.y, cos(v.x))
}
