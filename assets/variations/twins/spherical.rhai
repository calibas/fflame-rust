/// Inverts every point through the unit circle — a point at distance
/// r is mapped to distance 1/r. Far points fold inward, near points
/// fly outward; an inside-out fisheye on the plane.
///
/// # Authors
/// - Scott Draves

// Spherical, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Spherical", name: "rhai_spherical", category: "basic_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let r2 = z.norm_sqr() + 1.0e-6;
    z / r2
}

fn variation3(v, p) {
    let r2 = complex(v.x, v.y).norm_sqr() + 1.0e-6;
    let t1 = complex(v.x, v.y) / r2;
    vec3(t1.re, t1.im, v.z)
}
