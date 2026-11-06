/// Z displacement driven by the color register — `zf = factor · (*vc − x0)
/// / (x1 − x0)` (with `x0`/`x1` order-corrected and a zero-divisor guard
/// in init), optionally clamped to `[0, 1]`. XY pass through; Z is either
/// replaced by `w · zf` (`overwrite = 1`, default) or multiplied by `zf`
/// (`overwrite = 0`).
///
/// The "ztransl" name reflects the typical use: place a DC variation
/// upstream that paints a color gradient into `*vc`, then use this
/// variation to translate Z based on the gradient — points in different
/// color ranges land at different Z layers.
///
/// # Authors
/// - Xyrus02

// DC Z Translation, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x0: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "X0",
        hint: "Lower edge of the color-register input range. (Swapped with `x1` in init if `x0 > x1`.)",
    },
    x1: #{
        value: 1.0, min: 0.0, max: 1.0, free: true,
        caption: "X1",
        hint: "Upper edge of the color-register input range. Determines the normalization denominator `x1 − x0`.",
    },
    factor: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Factor",
        hint: "Multiplier on the normalized `zf` value before it drives Z.",
    },
    overwrite: #{
        value: true,
        caption: "Overwrite",
        hint: "When on, Z = `w · zf` (the input Z is discarded). When off, Z = `w · input_z · zf` (multiplicative).",
    },
    clamp: #{
        value: false,
        caption: "Clamp",
        hint: "When on, `zf` is clamped to `[0, 1]` before use.",
    },
};

const INFO = #{ title: "DC Z Translation", name: "rhai_dc_ztransl", category: "full_3d", phase: "any" };

fn prepare(p) {
    let lo = min(p.x0, p.x1);
    let hi = max(p.x0, p.x1);
    let span = hi - lo;
    #{ x1_m_x0: (if span == 0.0 { 1.0e-30 } else { span }) }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    #{ z: z, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let x0_u = p.x0;
    let x1_u = p.x1;
    let factor = p.factor;
    let overwrite = to_int(to_float(p.overwrite));
    let clamp_on = to_int(to_float(p.clamp));
    let x1_m_x0 = p.x1_m_x0;
    let lo = min(x0_u, x1_u);
    let tc = color;
    let zf = factor * (tc - lo) / x1_m_x0;
    if (clamp_on != 0) {
        zf = clamp(zf, 0.0, 1.0);
    }
    let nz = (if overwrite == 0 { v.z * zf } else { zf });
    #{ z: vec3(v.x, v.y, nz), color: color }
}
