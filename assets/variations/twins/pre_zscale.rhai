/// Scales the Z coordinate before the rest of the variations run. The
/// variation's weight is the scale factor — weight 2.0 doubles depth, 0.5
/// halves it.

// Pre-ZScale, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre-ZScale", name: "rhai_pre_zscale", category: "depth_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p) {
    z
}

fn variation3(v, p, ctx) {
    let weight = ctx.weight;
    vec3(v.x, v.y, v.z * weight)
}
