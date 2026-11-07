/// Color-scaled XYZ output — `(x, y) = (scale_x · p.x, scale_y · p.y)`
/// and `z = *vc · scale_z + offset_z` (with `reset_z > 0` controlling
/// override vs accumulate in cpp; our model approximates by always
/// accumulating, see file header note). XY use the pre-affine input
/// (the variation's `p`).
///
/// # Authors
/// - Andreas Maschke

// Color Scale WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scale_x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X output scale: contribution to result.x is `w · scale_x · p.x`.",
    },
    scale_y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y output scale.",
    },
    scale_z: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Scale Z",
        hint: "Color-register → Z coupling multiplier in `dz = *vc · scale_z · w + offset_z`.",
    },
    offset_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset Z",
        hint: "Constant Z bump added after the color-derived term.",
    },
    reset_z: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Reset Z",
        hint: "Upstream uses this to override Z when > 0 instead of accumulating; this port always accumulates (see file header).",
    },
};

const INFO = #{ title: "Color Scale WF", name: "rhai_colorscale_wf", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let scale_x = p.scale_x;
    let scale_y = p.scale_y;
    #{ z: complex(scale_x * z.re, scale_y * z.im), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let scale_x = p.scale_x;
    let scale_y = p.scale_y;
    let scale_z = p.scale_z;
    let offset_z = p.offset_z;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let tc = color;
    let dz = tc * scale_z * w + offset_z;
    let nz = dz * inv_w;
    #{ z: vec3(scale_x * v.x, scale_y * v.y, nz), color: color }
}
