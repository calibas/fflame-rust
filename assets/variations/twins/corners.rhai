/// Quadrant-based power warp — squares the input per axis, raises each to a
/// power (`xpower + xypower` on X, `ypower + xypower` on Y), and multiplies
/// by `multx`/`multy`. The sign of the input chooses whether the result is
/// added or subtracted, and a constant `xwidth`/`ywidth` is added to the
/// output. With `logmode = 1`, the squared input is first run through a
/// log-base transform before the pow.
///
/// # Authors
/// - Whittaker Courtney

// Corners, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xwidth: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Width",
        hint: "Constant X offset added per quadrant (signed by the input's X sign).",
    },
    ywidth: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Width",
        hint: "Constant Y offset added per quadrant.",
    },
    multx: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Mult X",
        hint: "X-axis multiplier on the squared input before the pow.",
    },
    multy: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Mult Y",
        hint: "Y-axis multiplier on the squared input before the pow.",
    },
    xpower: #{
        value: 0.75, min: -10.0, max: 10.0, free: true,
        caption: "X Power",
        hint: "X-axis power exponent (combined additively with `xypower`).",
    },
    ypower: #{
        value: 0.75, min: -10.0, max: 10.0, free: true,
        caption: "Y Power",
        hint: "Y-axis power exponent (combined additively with `xypower`).",
    },
    xypower: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "XY Power Add",
        hint: "Additional power offset added to both `xpower` and `ypower`.",
    },
    logmode: #{
        value: false,
        caption: "Log Mode",
        hint: "When on, use the log-base formula `pow(log_base(x²·mult + 3), …) − 1.33`. When off, plain `pow(x², …)`.",
    },
    log_base: #{
        value: 2.71828, min: 0.01, max: 100.0, free: true,
        caption: "Log Base",
        hint: "Log base used by the `logmode = 1` formula. Default ≈ e.",
    },
};

const INFO = #{ title: "Corners", name: "rhai_corners", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let xwidth = p.xwidth;
    let ywidth = p.ywidth;
    let multx = p.multx;
    let multy = p.multy;
    let xpower = p.xpower;
    let ypower = p.ypower;
    let xypower = p.xypower;
    let logmode = to_float(p.logmode);
    let log_base = max(1.0e-6, p.log_base);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let xs = z.re * z.re;
    let ys = z.im * z.im;
    let ex = 0.0;
    let ey = 0.0;
    if (logmode < 0.5) {
        ex = pow(max(1.0e-30, xs), xpower + xypower) * multx;
        ey = pow(max(1.0e-30, ys), ypower + xypower) * multy;
    } else {
        let lb = ln(log_base);
        let safe_lb = (if abs(lb) < 1.0e-30 { 1.0e-30 } else { lb });
        ex = pow(max(1.0e-30, ln(xs * multx + 3.0) / safe_lb), xpower + 2.25 + xypower) - 1.33;
        ey = pow(max(1.0e-30, ln(ys * multy + 3.0) / safe_lb), ypower + 2.25 + xypower) - 1.33;
    }
    let fx = (if z.re > 0.0 { w * ex + xwidth } else { w * -ex - xwidth });
    let fy = (if z.im > 0.0 { w * ey + ywidth } else { w * -ey - ywidth });
    complex(fx * inv_w, fy * inv_w)
}
