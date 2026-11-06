/// Same math as Spherical (inverts through the unit circle) but runs before
/// the rest of the variations instead of contributing to the weighted sum.
///
/// # Authors
/// - Scott Draves

// Pre-Spherical, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre-Spherical", name: "rhai_pre_spherical", category: "advanced_2d", phase: "pre", z: "preserve_z" };

fn variation(z, p) {
    let r = 1.0 / (z.norm_sqr() + 1.0e-5);
    z * r
}

fn variation3(v, p) {
    let r = 1.0 / (complex(v.x, v.y).norm_sqr() + 1.0e-5);
    vec3(v.x * r, v.y * r, v.z)
}
