/// Square fold-mirror with quadrant routing — folds points around an axis-
/// aligned square of side ±VVAR. Outside the square, points get folded back
/// onto the square's outer edge via a perimeter parameterization with
/// `spin_out`-driven angular offset and a `space` padding that stretches
/// the perpendicular axis. Inside the square, the same parameterization
/// runs with `spin_in` offset and no padding.
///
/// # Authors
/// - Michael Faber

// Lazy Travis, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    spin_in: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Spin In",
        hint: "Inner-square angular offset along the perimeter (multiplied by 4 internally; one unit = one full lap of the square).",
    },
    spin_out: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Spin Out",
        hint: "Outer-square angular offset along the perimeter (same scaling as `spin_in`).",
    },
    space: #{
        value: 1.5707964, min: -10.0, max: 10.0, free: true,
        caption: "Space",
        hint: "Outer-square padding — extends the box edge by `space` and proportionally stretches the perpendicular coordinate.",
    },
};

const INFO = #{ title: "Lazy Travis", name: "rhai_lazytravis", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ spin_in_4: 4.0 * p.spin_in, spin_out_4: 4.0 * p.spin_out }
}

fn variation(z, p, ctx) {
    let space = p.space;
    let spin_in_4 = p.spin_in_4;
    let spin_out_4 = p.spin_out_4;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let ax = abs(z.re);
    let ay = abs(z.im);
    let s = 0.0;
    let pp = 0.0;
    let fx = 0.0;
    let fy = 0.0;
    if (ax > w || ay > w) {
        if (ax > ay) {
            s = ax;
            if (z.re > 0.0) {
                pp = s + z.im + s * spin_out_4;
            } else {
                pp = 5.0 * s - z.im + s * spin_out_4;
            }
        } else {
            s = ay;
            if (z.im > 0.0) {
                pp = 3.0 * s - z.re + s * spin_out_4;
            } else {
                pp = 7.0 * s + z.re + s * spin_out_4;
            }
        }
        let safe_8s = max(1.0e-30, s * 8.0);
        pp = pp - to_float(floor(pp / safe_8s)) * safe_8s;
        let inv_s = 1.0 / max(1.0e-30, s);
        let x2 = 0.0;
        let y2 = 0.0;
        if (pp <= 2.0 * s) {
            x2 = s + space;
            let yt = -(s - pp);
            y2 = yt + yt * inv_s * space;
        } else if (pp <= 4.0 * s) {
            y2 = s + space;
            let xt = 3.0 * s - pp;
            x2 = xt + xt * inv_s * space;
        } else if (pp <= 6.0 * s) {
            x2 = -(s + space);
            let yt = 5.0 * s - pp;
            y2 = yt + yt * inv_s * space;
        } else {
            y2 = -(s + space);
            let xt = -(7.0 * s - pp);
            x2 = xt + xt * inv_s * space;
        }
        fx = w * x2;
        fy = w * y2;
    } else {
        if (ax > ay) {
            s = ax;
            if (z.re > 0.0) {
                pp = s + z.im + s * spin_in_4;
            } else {
                pp = 5.0 * s - z.im + s * spin_in_4;
            }
        } else {
            s = ay;
            if (z.im > 0.0) {
                pp = 3.0 * s - z.re + s * spin_in_4;
            } else {
                pp = 7.0 * s + z.re + s * spin_in_4;
            }
        }
        let safe_8s = max(1.0e-30, s * 8.0);
        pp = pp - to_float(floor(pp / safe_8s)) * safe_8s;
        if (pp <= 2.0 * s) {
            fx = w * s;
            fy = -w * (s - pp);
        } else if (pp <= 4.0 * s) {
            fx = w * (3.0 * s - pp);
            fy = w * s;
        } else if (pp <= 6.0 * s) {
            fx = -w * s;
            fy = w * (5.0 * s - pp);
        } else {
            fx = -w * (7.0 * s - pp);
            fy = -w * s;
        }
    }
    complex(fx * inv_w, fy * inv_w)
}
