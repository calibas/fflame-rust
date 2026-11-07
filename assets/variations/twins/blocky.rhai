/// 2D-block warp — maps points through an ellipse-bounded arctan to produce
/// angular blocky patterns. `mp` controls block size; `x` and `y` set per-
/// axis aspect.
///
/// # Authors
/// - Brad Stefanov

// Blocky, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis scaling on the arctan output.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis scaling on the arctan output.",
    },
    mp: #{
        value: 4.0, min: 0.1, max: 20.0, free: true,
        caption: "MP",
        hint: "Block size — smaller values produce more, finer blocks.",
    },
};

const INFO = #{ title: "Blocky", name: "rhai_blocky", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let px = p.x;
    let py = p.y;
    let mp = p.mp;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_over_pi = 0.6366197723675813;
    let v_in = w * two_over_pi;
    let safe_mp = (if mp == 0.0 { 1.0e-6 } else { mp });
    let T = (cos(z.re) + cos(z.im)) / safe_mp + 1.0;
    let safe_T = (if T == 0.0 { 1.0e-6 } else { T });
    let r = w / safe_T;
    let tmp = z.re * z.re + z.im * z.im + 1.0;
    let x2 = 2.0 * z.re;
    let y2 = 2.0 * z.im;
    let xmax = 0.5 * (sqrt(max(0.0, tmp + x2)) + sqrt(max(0.0, tmp - x2)));
    let ymax = 0.5 * (sqrt(max(0.0, tmp + y2)) + sqrt(max(0.0, tmp - y2)));
    let ax_n = z.re / max(1.0e-30, xmax);
    let bx = sqrt(max(0.0, 1.0 - ax_n * ax_n));
    let out_x = v_in * atan2(ax_n, bx) * r * px;
    let ay_n = z.im / max(1.0e-30, ymax);
    let by = sqrt(max(0.0, 1.0 - ay_n * ay_n));
    let out_y = v_in * atan2(ay_n, by) * r * py;
    complex(out_x * inv_w, out_y * inv_w)
}
