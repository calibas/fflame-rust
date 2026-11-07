/// Post-phase Z translate — adds the variation weight to the Z coordinate.
/// The XY coordinates pass through unchanged. Useful for offsetting the Z
/// position of a transform's accumulated output.

// Post Z-Translate WF, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Post Z-Translate WF", name: "rhai_post_ztranslate_wf", category: "depth_3d", phase: "post" };

fn variation(z, p) {
    z
}

fn variation3(v, p, ctx) {
    let w = ctx.weight;
    vec3(v.x, v.y, v.z + w)
}
