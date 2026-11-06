/// Power-warp with rmin/rmax clamp — computes a complex radial term `r =
/// (x² + r²·b·y²)^warp − (y² + r²·a·x²)^warp` (with `r² = 1/(x²+y²)`),
/// clamps it to `[rmin, rmax]`, scales it by `effect`, and emits `(x·(1+r),
/// y·(1+r))`. The clamp prevents the power expression from blowing up at
/// large or singular inputs.
///
/// # Authors
/// - DarkBeam

// Devil Warp, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "x² weight in the second power term.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "y² weight in the first power term.",
    },
    effect: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Effect",
        hint: "Scale on the final radial-warp magnitude.",
    },
    warp: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Warp",
        hint: "Power exponent for both radial terms.",
    },
    rmin: #{
        value: -0.24, min: -100.0, max: 100.0, free: true,
        caption: "R min",
        hint: "Lower clamp on the radial-warp magnitude.",
    },
    rmax: #{
        value: 100.0, min: -1000.0, max: 1000.0, free: true,
        caption: "R max",
        hint: "Upper clamp on the radial-warp magnitude.",
    },
};

const INFO = #{ title: "Devil Warp", name: "rhai_devil_warp", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let a = p.a;
    let b = p.b;
    let effect = p.effect;
    let warp = p.warp;
    let rmin = p.rmin;
    let rmax = p.rmax;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let xx = z.re;
    let yy = z.im;
    let denom = xx * xx + yy * yy;
    let safe_denom = max(denom, 1.0e-30);
    let r2 = 1.0 / safe_denom;
    let r = pow(max(xx * xx + r2 * b * yy * yy, 1.0e-30), warp) - pow(max(yy * yy + r2 * a * xx * xx, 1.0e-30), warp);
    r = clamp(r, rmin, rmax);
    r = effect * r;
    complex(xx * (1.0 + r) * inv_w, yy * (1.0 + r) * inv_w)
}
