/// Z-axis inflation #4 — adds `±(π/2 − atan2(y,x)) · 0.25` to the Z output,
/// with the sign chosen randomly each iteration. Produces a Z surface
/// that's a stochastic mirror of the input angle.
///
/// # Authors
/// - Larry Berlin

// Inflate Z 4, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 4", name: "rhai_inflatez_4", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    let unused1 = rand();
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let pi_2 = 1.5707963267948966;
    let ang1 = atan2(v.y, v.x);
    let val1 = pi_2 - ang1;
    if (rand() < 0.5) {
        val1 = -val1;
    }
    vec3(0.0, 0.0, val1 * 0.25)
}
