/// 3D version of Spherical — inverts each point through the unit sphere
/// (`1/r²`). Pulls distant points toward the origin and pushes nearby
/// points outward in all three axes.

// Spherical 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Spherical 3D", name: "rhai_spherical3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let t = max(1.0e-30, z.re * z.re + z.im * z.im);
    let r = 1.0 / t;
    complex(z.re * r, z.im * r)
}

fn variation3(v, p) {
    let t = max(1.0e-30, v.x * v.x + v.y * v.y + v.z * v.z);
    let r = 1.0 / t;
    vec3(v.x * r, v.y * r, v.z * r)
}
