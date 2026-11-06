/// Elliptic warp with 11 knobs — heavily parameterized elliptic variation.
/// Computes `xmax = c·(sqrt(x²+y²+a1+b1·x) + sqrt(x²+y²+a1−b1·x))`, then
/// emits `(v·atan2(a, b) + ps, ±v·log(xmax + sqrt(xmax − f or g)))` with
/// the Y branch chosen by random vs `e`. The 11 parameters give fine-
/// grained control over the elliptic shape.
///
/// # Authors
/// - Brad Stefanov

// Elliptic 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A1",
        hint: "Constant offset added to `x² + y²` in the elliptic-radius formula.",
    },
    a2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A2",
        hint: "Scale on the `atan2` argument `a = (x/xmax) · a2`.",
    },
    a3: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A3",
        hint: "Phase shift on the X output (scaled by −π/2).",
    },
    b1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "B1",
        hint: "Multiplier on x in the xmax sqrt-difference.",
    },
    b2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B2",
        hint: "Scale on `sqrt(d − a²)` in the b term.",
    },
    c: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Overall scale on xmax.",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Constant in the `sqrt(d − a²)` term.",
    },
    e: #{
        value: 0.5, min: 0.0, max: 1.0, free: true,
        caption: "E",
        hint: "Probability threshold for the Y output sign.",
    },
    f: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Sqrt offset for the positive-Y branch (`sqrt(xmax − f)`).",
    },
    g: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "G",
        hint: "Sqrt offset for the negative-Y branch (`sqrt(xmax − g)`).",
    },
    h: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "H",
        hint: "V-multiplier — output is scaled by `v = w · h / π`.",
    },
};

const INFO = #{ title: "Elliptic 2", name: "rhai_elliptic2", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let inv_pi = 0.31830988618379;
    #{ v_div_w: p.h * inv_pi }
}

fn variation(z, p, ctx) {
    let a1 = p.a1;
    let a2 = p.a2;
    let a3 = p.a3;
    let b1 = p.b1;
    let b2 = p.b2;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let g = p.g;
    let v_div_w = p.v_div_w;
    let pi_2 = 1.5707963267948966;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let v = w * v_div_w;
    let tmp = z.im * z.im + z.re * z.re + a1;
    let x2 = b1 * z.re;
    let safe_xmax_arg_a = max(0.0, tmp + x2);
    let safe_xmax_arg_b = max(0.0, tmp - x2);
    let xmax = c * (sqrt(safe_xmax_arg_a) + sqrt(safe_xmax_arg_b));
    let safe_xmax = (if abs(xmax) < 1.0e-30 { 1.0e-30 } else { xmax });
    let a_val = z.re / safe_xmax * a2;
    let b_val = sqrt(max(0.0, d - a_val * a_val)) * b2;
    let ps = -pi_2 * a3;
    let fx = v * atan2(a_val, b_val) + ps;
    let fy = 0.0;
    if (rand() < e) {
        fy = v * ln(max(1.0e-30, xmax + sqrt(max(0.0, xmax - f))));
    } else {
        fy = -v * ln(max(1.0e-30, xmax + sqrt(max(0.0, xmax - g))));
    }
    complex(fx * inv_w, fy * inv_w)
}
