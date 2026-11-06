/// Post-phase `colorscale_wf` — same formula but operates on the
/// accumulator value (`pVarTP.{x,y}`) after normal-phase variations
/// have run. Post phase doesn't apply the outer multiplier; the body
/// directly produces the modified accumulator.
///
/// XY: `nx = p.x · (1 + w · scale_x)`, `ny = p.y · (1 + w · scale_y)` —
/// cpp's `pVarTP.x += w · scale_x · pVarTP.x` desugared.
/// Z: `reset_z > 0` truly overrides Z here (post phase has direct
/// access to `p.z`, unlike the normal-phase variant).
///
/// # Authors
/// - Andreas Maschke

// Post Color Scale WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scale_x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X scale: `p.x · (1 + w · scale_x)`.",
    },
    scale_y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y scale.",
    },
    scale_z: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Scale Z",
        hint: "Color-register → Z coupling: `dz = *vc · scale_z · w + offset_z`.",
    },
    offset_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset Z",
        hint: "Constant Z bump.",
    },
    reset_z: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Reset Z",
        hint: "When > 0, output Z = `dz`; otherwise output Z = `p.z + dz`.",
    },
};

const INFO = #{ title: "Post Color Scale WF", name: "rhai_post_colorscale_wf", category: "full_3d", phase: "post" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let scale_x = p.scale_x;
    let scale_y = p.scale_y;
    let w = ctx.weight;
    #{ z: complex(z.re * (1.0 + w * scale_x), z.im * (1.0 + w * scale_y)), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let scale_x = p.scale_x;
    let scale_y = p.scale_y;
    let scale_z = p.scale_z;
    let offset_z = p.offset_z;
    let reset_z = p.reset_z;
    let w = ctx.weight;
    let tc = color;
    let dz = tc * scale_z * w + offset_z;
    let nz = (if reset_z > 0.0 { dz } else { v.z + dz });
    #{ z: vec3(v.x * (1.0 + w * scale_x), v.y * (1.0 + w * scale_y), nz), color: color }
}
