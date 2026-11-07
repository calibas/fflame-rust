/// Z-axis inflation #1 — adds `sin(atan2(y,x)) − 2y` to the Z output. XY
/// pass through unchanged. The angular sine plus linear-Y term produces a
/// saddle-shaped Z surface.
///
/// # Authors
/// - Larry Berlin

// Inflate Z 1, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 1", name: "rhai_inflatez_1", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let ang = atan2(v.y, v.x);
    let val1 = v.y * 2.0;
    vec3(0.0, 0.0, sin(ang) - val1)
}
