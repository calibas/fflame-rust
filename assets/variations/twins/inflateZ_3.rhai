/// Z-axis inflation #3 — adds `0.2 · (π − atan2(y,x)) · cos(3·atan2(y,x) +
/// (y − x))` to the Z output. An angular-cosine modulated radial term
/// producing a wavy 3D surface.
///
/// # Authors
/// - Larry Berlin

// Inflate Z 3, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 3", name: "rhai_inflatez_3", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let pi = 3.14159265358979;
    let ang = atan2(v.y, v.x);
    let val1 = 0.2 * (pi - ang) * cos(3.0 * ang + (v.y - v.x));
    vec3(0.0, 0.0, val1)
}
