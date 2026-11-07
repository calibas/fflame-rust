/// Maurer-rose curve sampler — a Maurer rose steps around a rhodonea (rose)
/// curve at fixed angular increments and connects consecutive samples with
/// straight lines. This variation picks a random point on the nearest
/// Maurer line, on one of its endpoints, or directly on the underlying rose
/// curve, with the mix between the three modes controlled by relative
/// weights.
///
/// # Authors
/// - CozyG

// Maurer Rose, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    kn: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "K Numerator",
        hint: "K numerator — the rose's petal ratio is `kn/kd`.",
    },
    kd: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "K Denominator",
        hint: "K denominator.",
    },
    c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C Offset",
        hint: "Constant offset added to the rose radius. Shifts the curve outward.",
    },
    line_count: #{
        value: 360.0, min: 1.0, max: 10000.0, free: true,
        caption: "Line Count",
        hint: "Number of Maurer-rose line segments per cycle.",
    },
    line_offset_degrees: #{
        value: 71.0, min: -3600.0, max: 3600.0, free: true,
        caption: "Line Offset (deg)",
        hint: "Angular step between successive samples on the rhodonea, in degrees. Together with line_count, controls how many cycles wrap around the rose.",
    },
    show_lines: #{
        value: 1.0, min: 0.0, max: 10.0,
        caption: "Show Lines",
        hint: "Relative weight of sampling along the Maurer line segments.",
    },
    show_points: #{
        value: 0.0, min: 0.0, max: 10.0,
        caption: "Show Points",
        hint: "Relative weight of sampling at the segment endpoints.",
    },
    show_curve: #{
        value: 0.05, min: 0.0, max: 10.0,
        caption: "Show Curve",
        hint: "Relative weight of sampling directly on the underlying rhodonea curve.",
    },
    line_thickness: #{
        value: 0.5, min: 0.0, max: 100.0, free: true,
        caption: "Line Thick (×100)",
        hint: "Random jitter width around line samples (×100).",
    },
    point_thickness: #{
        value: 3.0, min: 0.0, max: 100.0, free: true,
        caption: "Point Thick (×100)",
        hint: "Random scatter radius around endpoint samples (×100).",
    },
    curve_thickness: #{
        value: 1.0, min: 0.0, max: 100.0, free: true,
        caption: "Curve Thick (×100)",
        hint: "Random jitter width around rose-curve samples (×100).",
    },
};

const INFO = #{ title: "Maurer Rose", name: "rhai_maurer_rose", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let kn = p.kn;
    let kd = p.kd;
    let line_count = p.line_count;
    let line_offset_deg = p.line_offset_degrees;
    let show_lines = p.show_lines;
    let show_points = p.show_points;
    let show_curve = p.show_curve;
    let line_thick_in = p.line_thickness;
    let point_thick_in = p.point_thickness;
    let curve_thick_in = p.curve_thickness;
    let two_pi = 6.28318530717959;
    let safe_kd = (if kd == 0.0 { 1.0e-30 } else { kd });
    let step_size = two_pi * (line_offset_deg / 360.0);
    let show_sum = max(1.0e-30, show_lines + show_points + show_curve);
    let line_frac = show_lines / show_sum;
    let point_frac = show_points / show_sum;
    #{ k: kn / safe_kd, step_size: step_size, safe_step: (if step_size == 0.0 { 1.0e-30 } else { step_size }), cycles: (line_count * step_size) / two_pi, line_thr: line_frac, point_thr: line_frac + point_frac, point_half_thr: line_frac + 0.5 * point_frac, d7: line_thick_in / 100.0, d8: point_thick_in / 100.0, d9: curve_thick_in / 100.0 }
}

fn variation(z, p) {
    let c = p.c;
    let k = p.k;
    let step_size = p.step_size;
    let safe_step = p.safe_step;
    let cycles = p.cycles;
    let line_thr = p.line_thr;
    let point_thr = p.point_thr;
    let point_half_thr = p.point_half_thr;
    let line_thickness = p.d7;
    let point_thickness = p.d8;
    let curve_thickness = p.d9;
    let tin = atan2(z.im, z.re);
    let t = cycles * tin;
    let step = to_float(floor(t / safe_step));
    let theta1 = step * step_size;
    let theta2 = theta1 + step_size;
    let r1 = cos(k * theta1) + c;
    let x1 = r1 * cos(theta1);
    let y1 = r1 * sin(theta1);
    let r2 = cos(k * theta2) + c;
    let x2 = r2 * cos(theta2);
    let y2 = r2 * sin(theta2);
    let xdiff = x2 - x1;
    let ydiff = y2 - y1;
    let m = ydiff / (if abs(xdiff) < 1.0e-30 { 1.0e-30 } else { xdiff });
    let line_length = sqrt(xdiff * xdiff + ydiff * ydiff);
    let rnd = rand();
    let xout = 0.0;
    let yout = 0.0;
    let xoff = 0.0;
    let yoff = 0.0;
    if (rnd < line_thr) {
        let d = rand() * line_length;
        xoff = d / sqrt(1.0 + m * m);
        if (x2 < x1) {
            xoff = -xoff;
        }
        yoff = abs(m * xoff);
        if (y2 < y1) {
            yoff = -yoff;
        }
        if (line_thickness != 0.0) {
            xoff = xoff + (rand() - 0.5) * line_thickness;
            yoff = yoff + (rand() - 0.5) * line_thickness;
        }
        xout = x1 + xoff;
        yout = y1 + yoff;
    } else if (rnd <= point_thr) {
        if (point_thickness != 0.0) {
            let roff = rand() * point_thickness;
            let rang = rand() * 6.28318530717959;
            xoff = roff * cos(rang);
            yoff = roff * sin(rang);
        }
        if (rnd <= point_half_thr) {
            xout = x1 + xoff;
            yout = y1 + yoff;
        } else {
            xout = x2 + xoff;
            yout = y2 + yoff;
        }
    } else {
        let r_curve = cos(k * t) + c;
        let cx = r_curve * cos(t);
        let cy = r_curve * sin(t);
        if (curve_thickness != 0.0) {
            xout = cx + (rand() - 0.5) * curve_thickness;
            yout = cy + (rand() - 0.5) * curve_thickness;
        } else {
            xout = cx;
            yout = cy;
        }
    }
    complex(xout, yout)
}
