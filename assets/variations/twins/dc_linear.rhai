/// Pass-through positioning (linear) with direct-color writes — colors each
/// iteration based on a rotated linear projection of the post-variation
/// point. Output position is unchanged from the input; effect only visible
/// when the transform's Direct Color slider is > 0.
///
/// # Authors
/// - Xyrus02

// DC Linear, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    offset: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset",
        hint: "Offset added to the projected coordinate before computing color.",
    },
    angle: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Rotation angle (degrees) for the projection axis.",
    },
    scale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Scaling factor on the projection — larger compresses the color gradient, smaller stretches it.",
    },
};

const INFO = #{ title: "DC Linear", name: "rhai_dc_linear", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let scale = p.scale;
    let angle_deg = p.angle;
    let angle_rad = angle_deg * 3.14159265358979 / 180.0;
    #{ ldcs: 1.0 / (if scale == 0.0 { 1.0e-5 } else { scale }), cos_a: cos(angle_rad), sin_a: sin(angle_rad) }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let offset = p.offset;
    let ldcs = p.ldcs;
    let cos_a = p.cos_a;
    let sin_a = p.sin_a;
    let weight = ctx.weight;
    let wx = weight * z.re;
    let wy = weight * z.im;
    let proj = cos_a * wx + sin_a * wy + offset;
    color = fract(abs(0.5 * (ldcs * proj + 1.0)));
    #{ z: z, color: color }
}
