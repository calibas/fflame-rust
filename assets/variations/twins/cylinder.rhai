/// Wraps the X coordinate around a cylinder (sine), passes Y through
/// unchanged. In 3D, adds a cosine of X as the Z coordinate so the plane
/// really wraps into a cylindrical sheet.
///
/// # Authors
/// - Scott Draves

// Cylinder, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cylinder", name: "rhai_cylinder", category: "advanced_2d", phase: "any", z: "preserve_z", inverse_branches: 10 };

fn variation(z, p) {
    complex(sin(z.re), z.im)
}

fn variation3(v, p) {
    vec3(sin(v.x), v.y, cos(v.x))
}

fn inverse(z, p, branch) {
    if abs(z.re) > 1.0 {
        return no_preimage();
    }
    let turn = to_float(branch / 2) - 2.0;
    let s = asin(z.re);
    let x = if (branch & 1) == 0 { s } else { 3.14159265358979 - s };
    complex(x + 6.28318530717959 * turn, z.im)
}

fn image_gap(z, p) {
    abs(z.re) - 1.0
}
