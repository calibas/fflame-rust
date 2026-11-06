/// Z-axis random extrusion — with probability `root_face`, sets Z to the
/// variation weight (clamped at 0 for negative weights); otherwise sets Z
/// to a random fraction of the weight. The XY output is zero. Useful as a
/// Z-only depth contribution to give 2D flames a flat Z extrusion.
///
/// # Authors
/// - Xyrus02

// Extrude, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    root_face: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Root Face",
        hint: "Probability threshold for the `root face` branch — values below it set Z to the (clamped) weight; values above set Z to `weight · rand`.",
    },
};

const INFO = #{ title: "Extrude", name: "rhai_extrude", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    let unused1 = rand();
    complex(0.0, 0.0)
}

fn variation3(v, p, ctx) {
    let root_face = p.root_face;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let fz_cpp = 0.0;
    if (rand() < root_face) {
        fz_cpp = (if w < 0.0 { 0.0 } else { w });
    } else {
        fz_cpp = w * rand();
    }
    vec3(0.0, 0.0, fz_cpp * inv_w)
}
