/// Shifts the Z coordinate up or down before the rest of the variations
/// run. The variation's weight is the offset.

// Pre-ZTranslate, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre-ZTranslate", name: "rhai_pre_ztranslate", category: "depth_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p) {
    z
}

fn variation3(v, p, ctx) {
    let weight = ctx.weight;
    vec3(v.x, v.y, v.z + weight)
}
