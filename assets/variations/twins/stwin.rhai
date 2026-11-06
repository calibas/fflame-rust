// STwin, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    distort: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distort",
        hint: "Frequency multiplier on the sin term (× 2π internally).",
    },
    offset_xy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset XY",
        hint: "Phase offset added to the sin argument (× 0.1 internally).",
    },
    offset_x2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset X²",
        hint: "Additive offset on x² (× 0.0001 internally). Prevents division by zero at the origin.",
    },
    offset_y2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset Y²",
        hint: "Additive offset on y² (× 0.0001 internally).",
    },
};

const INFO = #{ title: "STwin", name: "rhai_stwin" };

fn variation(z, p, ctx) {
    let distort = p.distort;
    let off_xy = p.offset_xy;
    let off_x2 = p.offset_x2;
    let off_y2 = p.offset_y2;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_pi = 6.28318530717959;
    let x = z.re * w * 0.05;
    let y = z.im * w * 0.05;
    let x2 = x * x + off_x2 * 0.0001;
    let y2 = y * y + off_y2 * 0.0001;
    let divident = x2 + y2;
    if (divident == 0.0) {
        divident = 1.0;
    }
    let result = (x2 - y2) * sin(two_pi * distort * (x + y + off_xy * 0.1)) / divident;
    let fx = w * z.re + result;
    let fy = w * z.im + result;
    complex(fx * inv_w, fy * inv_w)
}
