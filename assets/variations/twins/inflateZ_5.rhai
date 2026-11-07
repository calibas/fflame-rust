/// Z-axis inflation #5 — adds `cos(π/2 − atan2(y,x)) / 2 = sin(atan2(y,x))
/// / 2` to the Z output. The simplest of the family — a sinusoidal Z
/// surface.
///
/// # Authors
/// - Larry Berlin

// Inflate Z 5, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 5", name: "rhai_inflatez_5", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let pi_2 = 1.5707963267948966;
    let ang1 = atan2(v.y, v.x);
    let val1 = cos(pi_2 - ang1) * 0.5;
    vec3(0.0, 0.0, val1)
}
