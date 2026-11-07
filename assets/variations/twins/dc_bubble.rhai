/// Apophysis Bubble warp (spherical projection) with direct-color writes —
/// colors each iteration based on the squared distance from a configurable
/// center point. Same XY warp as Bubble, plus per-iteration color
/// modulation. Color effect only visible when the transform's Direct Color
/// slider is > 0.
///
/// # Authors
/// - Xyrus02

// DC Bubble, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    centerx: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Center X",
        hint: "X coordinate of the radial color center.",
    },
    centery: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Center Y",
        hint: "Y coordinate of the radial color center.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Scaling factor on the squared distance — larger compresses the color gradient, smaller stretches it.",
    },
};

const INFO = #{ title: "DC Bubble", name: "rhai_dc_bubble", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let scale = p.scale;
    #{ bdcs: 1.0 / (if scale == 0.0 { 1.0e-5 } else { scale }) }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let cx = p.centerx;
    let cy = p.centery;
    let bdcs = p.bdcs;
    let weight = ctx.weight;
    let r = z.re * z.re + z.im * z.im;
    let r4_1 = 1.0 / (r / 4.0 + 1.0);
    let new_x = r4_1 * z.re;
    let new_y = r4_1 * z.im;
    let wx = weight * new_x + cx;
    let wy = weight * new_y + cy;
    color = fract(abs(bdcs * (wx * wx + wy * wy)));
    #{ z: complex(new_x, new_y), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let cx = p.centerx;
    let cy = p.centery;
    let bdcs = p.bdcs;
    let weight = ctx.weight;
    let r = v.x * v.x + v.y * v.y;
    let r4_1 = 1.0 / (r / 4.0 + 1.0);
    let new_x = r4_1 * v.x;
    let new_y = r4_1 * v.y;
    let wx = weight * new_x + cx;
    let wy = weight * new_y + cy;
    color = fract(abs(bdcs * (wx * wx + wy * wy)));
    #{ z: vec3(new_x, new_y, v.z), color: color }
}
