/// Parametric butterfly curve — emits points on a butterfly curve `r =
/// ½·(exp(cos t) − 2·cos 4t − sin⁵(t/12) + offset)` driven by the input
/// angle `t = cycles · atan2(y, x)`. Based on the Butterfly curve
/// discovered ~1988 by Temple H. Fay. Routes through one of 6 output modes
/// (controlled by `outer_mode`/`inner_mode`) depending on whether the input
/// lies inside or outside the curve, with optional `fill` randomization.
///
/// # Authors
/// - CozyG

// Butterfly Fay, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cycles: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Cycles",
        hint: "Number of butterfly-curve cycles per full input rotation. 0 falls back to π² internally.",
    },
    offset: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset",
        hint: "Additive offset on the curve radius formula.",
    },
    unified_inner_outer: #{
        value: true,
        caption: "Unified",
        hint: "When on, always use the outer mode/spread/ratio. When off, pick based on whether the input is inside or outside the curve.",
    },
    outer_mode: #{
        value: 1, choices: ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Linear Input"],
        caption: "Outer Mode",
        hint: "Output mode for points outside the butterfly curve.",
    },
    inner_mode: #{
        value: 1, choices: ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add", "Half + Offset", "Linear Input"],
        caption: "Inner Mode",
        hint: "Output mode for points inside the butterfly curve.",
    },
    outer_spread: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Outer Spread",
        hint: "Outer-mode spread amount (interpretation depends on `outer_mode`).",
    },
    inner_spread: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Inner Spread",
        hint: "Inner-mode spread amount (interpretation depends on `inner_mode`).",
    },
    outer_spread_ratio: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Outer Ratio",
        hint: "X-vs-Y ratio for outer-mode spread.",
    },
    inner_spread_ratio: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Inner Ratio",
        hint: "X-vs-Y ratio for inner-mode spread.",
    },
    spread_split: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Spread Split",
        hint: "Multiplier on the input radius used to decide inner vs outer (compared against the curve radius).",
    },
    fill: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Fill",
        hint: "Random fill amount added to the curve radius. 0 disables; non-zero triggers an RNG call.",
    },
};

const INFO = #{ title: "Butterfly Fay", name: "rhai_butterfly_fay", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi_sq = 9.869604401089358;
    let cycles = p.cycles;
    #{ n_cycles: (if cycles == 0.0 { pi_sq } else { cycles }) }
}

fn variation(z, p) {
    let offset = p.offset;
    let unified = to_int(to_float(p.unified_inner_outer));
    let outer_mode = to_int(to_float(p.outer_mode));
    let inner_mode = to_int(to_float(p.inner_mode));
    let outer_spread = p.outer_spread;
    let inner_spread = p.inner_spread;
    let outer_ratio = p.outer_spread_ratio;
    let inner_ratio = p.inner_spread_ratio;
    let spread_split = p.spread_split;
    let fill = p.fill;
    let n_cycles = p.n_cycles;
    let theta = atan2(z.im, z.re);
    let t = n_cycles * theta;
    let rin = spread_split * sqrt(z.re * z.re + z.im * z.im);
    let s = sin(t / 12.0);
    let s5 = s * s * s * s * s;
    let r = 0.5 * (exp(cos(t)) - 2.0 * cos(4.0 * t) - s5 + offset);
    if (fill != 0.0) {
        r = r + fill * (rand() - 0.5);
    }
    let x_curve = r * sin(t);
    let y_curve = -r * cos(t);
    let is_outer = (abs(rin) > abs(r)) || (unified == 1);
    let mode = (if is_outer { outer_mode } else { inner_mode });
    let spread = (if is_outer { outer_spread } else { inner_spread });
    let ratio = (if is_outer { outer_ratio } else { inner_ratio });
    let sign = (if is_outer { 1.0 } else { -1.0 });
    let fx = 0.0;
    let fy = 0.0;
    if (mode == 0) {
        fx = x_curve;
        fy = y_curve;
    } else if (mode == 1) {
        let rinx = rin * spread * ratio - spread * ratio + 1.0;
        let riny = rin * spread - spread + 1.0;
        fx = rinx * x_curve;
        fy = riny * y_curve;
    } else if (mode == 2) {
        let xin = abs(z.re);
        let yin = abs(z.im);
        if (x_curve < 0.0) {
            xin = -xin;
        }
        if (y_curve < 0.0) {
            yin = -yin;
        }
        fx = x_curve + sign * spread * ratio * (xin - x_curve);
        fy = y_curve + sign * spread * (yin - y_curve);
    } else if (mode == 3) {
        let xin = abs(z.re);
        let yin = abs(z.im);
        if (x_curve < 0.0) {
            xin = -xin;
        }
        if (y_curve < 0.0) {
            yin = -yin;
        }
        fx = x_curve + sign * spread * ratio * xin;
        fy = y_curve + sign * spread * yin;
    } else if (mode == 4) {
        let rinx = 0.5 * rin + spread * ratio;
        let riny = 0.5 * rin + spread;
        fx = rinx * x_curve;
        fy = riny * y_curve;
    } else if (mode == 5) {
        fx = x_curve + spread * ratio * z.re;
        fy = y_curve + spread * z.im;
    } else {
        fx = x_curve;
        fy = y_curve;
    }
    complex(fx, fy)
}
