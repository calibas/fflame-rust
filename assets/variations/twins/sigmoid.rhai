/// Saturating sigmoid in both axes — pushes coordinates through `1/(1 +
/// exp(...))` to compress them toward the [-1, 1] range. `shiftx` /
/// `shifty` control how steep the saturation curve is on each axis.
///
/// # Authors
/// - Xyrus02
/// - Brad Stefanov

// Sigmoid, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shiftx: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift X",
        hint: "X-axis saturation curve. Higher absolute value = steeper transition.",
    },
    shifty: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift Y",
        hint: "Y-axis saturation curve. Higher absolute value = steeper transition.",
    },
};

const INFO = #{ title: "Sigmoid", name: "rhai_sigmoid", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p, ctx) {
    let shiftx_in = p.shiftx;
    let shifty_in = p.shifty;
    let w = ctx.weight;
    let sgn = (if w == 0.0 { 1.0 } else { (if w > 0.0 { 1.0 } else if w < 0.0 { -1.0 } else { 0.0 }) });
    let ax = 1.0;
    let sx = shiftx_in;
    if (sx < 1.0 && sx > -1.0) {
        if (sx == 0.0) {
            sx = 1.0e-6;
        } else {
            ax = (if sx < 0.0 { -1.0 } else { 1.0 });
            sx = 1.0 / sx;
        }
    }
    let ay = 1.0;
    let sy = shifty_in;
    if (sy < 1.0 && sy > -1.0) {
        if (sy == 0.0) {
            sy = 1.0e-6;
        } else {
            ay = (if sy < 0.0 { -1.0 } else { 1.0 });
            sy = 1.0 / sy;
        }
    }
    sx = sx * -5.0;
    sy = sy * -5.0;
    let c0 = ax / (1.0 + exp(sx * z.re));
    let c1 = ay / (1.0 + exp(sy * z.im));
    complex(sgn * 2.0 * (c0 - 0.5), sgn * 2.0 * (c1 - 0.5))
}

fn variation3(v, p, ctx) {
    let shiftx_in = p.shiftx;
    let shifty_in = p.shifty;
    let w = ctx.weight;
    let sgn = (if w == 0.0 { 1.0 } else { (if w > 0.0 { 1.0 } else if w < 0.0 { -1.0 } else { 0.0 }) });
    let ax = 1.0;
    let sx = shiftx_in;
    if (sx < 1.0 && sx > -1.0) {
        if (sx == 0.0) {
            sx = 1.0e-6;
        } else {
            ax = (if sx < 0.0 { -1.0 } else { 1.0 });
            sx = 1.0 / sx;
        }
    }
    let ay = 1.0;
    let sy = shifty_in;
    if (sy < 1.0 && sy > -1.0) {
        if (sy == 0.0) {
            sy = 1.0e-6;
        } else {
            ay = (if sy < 0.0 { -1.0 } else { 1.0 });
            sy = 1.0 / sy;
        }
    }
    sx = sx * -5.0;
    sy = sy * -5.0;
    let c0 = ax / (1.0 + exp(sx * v.x));
    let c1 = ay / (1.0 + exp(sy * v.y));
    vec3(sgn * 2.0 * (c0 - 0.5), sgn * 2.0 * (c1 - 0.5), 0.0)
}
