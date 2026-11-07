/// Z-axis inflation #6 — adds `1.5 − acos(sin(atan2(y,x)) · atan2(y,x) ·
/// sin(y − x) · 0.5)` to the Z output. The most complex of the family — an
/// arc-cosine of a triple-product term.
///
/// # Authors
/// - Larry Berlin

// Inflate Z 6, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 6", name: "rhai_inflatez_6", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let ang = atan2(v.y, v.x);
    let adf = v.y - v.x;
    let kik = ang * sin(adf);
    let arg = clamp(sin(ang) * kik * 0.5, -1.0, 1.0);
    vec3(0.0, 0.0, 1.5 - acos(arg))
}
