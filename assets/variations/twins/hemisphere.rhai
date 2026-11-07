/// Projects each point onto a hemisphere. The XY position picks a spot on
/// the dome and Z becomes the height at that spot — points near the
/// origin sit high, points far away curve down toward the rim.

// Hemisphere, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Hemisphere", name: "rhai_hemisphere", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let r2 = z.norm_sqr();
    let t = 1.0 / sqrt(r2 + 1.0);
    complex(z.re * t, z.im * t)
}

fn variation3(v, p) {
    let r2_xy = complex(v.x, v.y).norm_sqr();
    let t = 1.0 / sqrt(r2_xy + 1.0);
    vec3(v.x * t, v.y * t, t)
}
