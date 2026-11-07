/// Sin/cos masked warp — emits `(sin³(xf), cos(xf)·sin²(xf)) · (cosh(yf) +
/// ushift) / (x² + y²)` where `xf = xscale·x + xshift` and `yf = yscale·y +
/// yshift`. The sin-power factors create lobed masking; `cosh(yf)`
/// modulates the overall magnitude based on Y.
///
/// # Authors
/// - Raykoid666
/// - CozyG

// Mask, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xshift: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X shift",
        hint: "Additive offset on X before the sin term.",
    },
    yshift: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y shift",
        hint: "Additive offset on Y before the cosh term.",
    },
    ushift: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "U shift",
        hint: "Constant offset added to `cosh(yf)`.",
    },
    xscale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X scale",
        hint: "Multiplier on X before the sin term.",
    },
    yscale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y scale",
        hint: "Multiplier on Y before the cosh term.",
    },
};

const INFO = #{ title: "Mask", name: "rhai_mask", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let xshift = p.xshift;
    let yshift = p.yshift;
    let ushift = p.ushift;
    let xscale = p.xscale;
    let yscale = p.yscale;
    let sumsq = z.re * z.re + z.im * z.im;
    if (sumsq < 1.0e-30) {
        return complex(0.0, 0.0);
    }
    let xfactor = xscale * z.re + xshift;
    let yfactor = yscale * z.im + yshift;
    let sx = sin(xfactor);
    let body = (cosh(yfactor) + ushift) * sx * sx / sumsq;
    complex(sx * body, cos(xfactor) * body)
}
