/// Gaussian X/Y bumps — adds a gaussian bump `xamp · exp(−y²/xlength²)`
/// to the X output and `yamp · exp(−x²/ylength²)` to the Y output. The
/// bumps decay smoothly away from the X and Y axes respectively, producing
/// a soft cross-shaped distortion.
///
/// # Authors
/// - Apophysis Plugin Pack

// Curve, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xamp: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "X Amp",
        hint: "X-axis gaussian bump amplitude.",
    },
    yamp: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y Amp",
        hint: "Y-axis gaussian bump amplitude.",
    },
    xlength: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Length",
        hint: "X-axis gaussian width — the Y-direction decay scale of the X bump.",
    },
    ylength: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Length",
        hint: "Y-axis gaussian width — the X-direction decay scale of the Y bump.",
    },
};

const INFO = #{ title: "Curve", name: "rhai_curve", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ pc_xlen: max(1.0e-20, p.xlength * p.xlength), pc_ylen: max(1.0e-20, p.ylength * p.ylength) }
}

fn variation(z, p) {
    let xamp = p.xamp;
    let yamp = p.yamp;
    let pc_xlen = p.pc_xlen;
    let pc_ylen = p.pc_ylen;
    let fx = z.re + xamp * exp(-z.im * z.im / pc_xlen);
    let fy = z.im + yamp * exp(-z.re * z.re / pc_ylen);
    complex(fx, fy)
}
