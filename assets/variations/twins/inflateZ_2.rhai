/// Z-axis inflation #2 — adds `0.25 − (2x + 2y)/3` to the Z output. XY pass
/// through unchanged. Z is a tilted plane depending on `x + y`.
///
/// # Authors
/// - Larry Berlin

// Inflate Z 2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 2", name: "rhai_inflatez_2", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let val1 = v.y * 2.0;
    let val2 = v.x * 2.0;
    let aval = (val1 + val2) * 0.3333333333333333;
    vec3(0.0, 0.0, 0.25 - aval)
}
