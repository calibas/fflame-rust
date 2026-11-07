/// 3D pyramid using cubic-distance norm — each coordinate is cubed and
/// divided by the sum of absolute cubes. Produces a pyramid-shaped
/// silhouette.
///
/// # Authors
/// - Zueuk

// Pyramid, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pyramid", name: "rhai_pyramid", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let x3 = z.re * z.re * z.re;
    let y3 = z.im * z.im * z.im;
    let r = 1.0 / (abs(x3) + abs(y3) + 1.0e-9);
    complex(x3 * r, y3 * r)
}

fn variation3(v, p) {
    let x3 = v.x * v.x * v.x;
    let y3 = v.y * v.y * v.y;
    let z3 = abs(v.z * v.z * v.z);
    let r = 1.0 / (abs(x3) + abs(y3) + z3 + 1.0e-9);
    vec3(x3 * r, y3 * r, z3 * r)
}
