/// Elliptic-coordinate warp with random Y-sign + constant offsets —
/// converts the input to elliptic coordinates, emits `v · atan2(a, b)` on X
/// (sign-shifted by `±x_p`), and `±v · log(xmax + sqrt(xmax−1))` on Y (sign
/// chosen randomly each iteration, with `±y_p` offset). A Stefanov-tuned
/// elliptic warp.
///
/// # Authors
/// - Brad Stefanov

// Spliptic BS, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.05, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis constant offset. Sign added when input x ≥ 0, subtracted when input x < 0.",
    },
    y: #{
        value: 0.05, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis constant offset. Sign chosen by the same random branch that picks the Y-output sign.",
    },
};

const INFO = #{ title: "Spliptic BS", name: "rhai_spliptic_bs", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ v_div_w: 0.6366197723675814 }
}

fn variation(z, p, ctx) {
    let x_p = p.x;
    let y_p = p.y;
    let v_div_w = p.v_div_w;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let v = w * v_div_w;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let x2 = 2.0 * z.re;
    let xmax = 0.5 * (sqrt(max(0.0, tmp + x2)) + sqrt(max(0.0, tmp - x2)));
    let safe_xmax = max(1.0e-30, xmax);
    let a = z.re / safe_xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));
    let fx = 0.0;
    if (z.re >= 0.0) {
        fx = v * atan2(a, b) + x_p;
    } else {
        fx = v * atan2(a, b) - x_p;
    }
    let log_arg = xmax + sqrt(max(0.0, xmax - 1.0));
    let log_term = ln(max(1.0e-30, log_arg));
    let fy = 0.0;
    if (rand() < 0.5) {
        fy = v * log_term + y_p;
    } else {
        fy = -v * log_term - y_p;
    }
    complex(fx * inv_w, fy * inv_w)
}
