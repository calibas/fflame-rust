/// Maps the plane onto a sphere - far points shrink toward the equator,
/// near points spread across the surface.
///
/// # Authors
/// - Scott Draves

// Bubble, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Bubble", name: "rhai_bubble", category: "advanced_2d", phase: "any", inverse_branches: 2 };

fn variation(z, p) {
    let r2 = z.norm_sqr();
    let r = r2 / 4.0 + 1.0;
    let scale = 1.0 / r;
    complex(scale * z.re, scale * z.im)
}

fn variation3(v, p) {
    let r2_xy = complex(v.x, v.y).norm_sqr();
    let r = r2_xy / 4.0 + 1.0;
    let scale = 1.0 / r;
    let new_z = 2.0 / r - 1.0;
    vec3(scale * v.x, scale * v.y, new_z)
}

fn inverse(z, p, branch) {
    let x = z.norm_sqr();
    if x > 1.0 {
        return no_preimage();
    }
    let root = sqrt(max(0.0, 1.0 - x));
    if branch == 0 {
        z * (2.0 / (1.0 + root))
    } else {
        z * (2.0 * (1.0 + root) / max(x, 1.0e-30))
    }
}

fn image_gap(z, p) {
    z.abs() - 1.0
}
