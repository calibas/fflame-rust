// Fourth, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    spin: #{
        value: 3.1415927, min: -10.0, max: 10.0, free: true,
        caption: "Spin",
        hint: "Lazysusan-quadrant rotation amount, in radians.",
    },
    space: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Space",
        hint: "Lazysusan-quadrant radial nudge for the outside-threshold case.",
    },
    twist: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Twist",
        hint: "Lazysusan-quadrant additional rotation, proportional to distance from the threshold edge.",
    },
    x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "Lazysusan-quadrant X center offset.",
    },
    y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Lazysusan-quadrant Y center offset.",
    },
};

const INFO = #{ title: "Fourth", name: "rhai_fourth" };

fn variation(z, p, ctx) {
    let spin = p.spin;
    let space = p.space;
    let twist = p.twist;
    let x_p = p.x;
    let y_p = p.y;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let sqrvvar = w * w;
    let fx = 0.0;
    let fy = 0.0;
    if (z.re > 0.0 && z.im > 0.0) {
        let r2 = z.re * z.re + z.im * z.im;
        let safe_r2 = max(r2, 1.0e-30);
        let a = atan2(z.im, z.re);
        let inv_r = 1.0 / sqrt(safe_r2);
        fx = w * inv_r * cos(a);
        fy = w * inv_r * sin(a);
    } else if (z.re > 0.0 && z.im < 0.0) {
        let r2 = z.re * z.re + z.im * z.im;
        if (r2 < sqrvvar) {
            let r = w * sqrt(max(sqrvvar / max(r2, 1.0e-30) - 1.0, 0.0));
            fx = r * z.re;
            fy = r * z.im;
        } else {
            fx = w * z.re;
            fy = w * z.im;
        }
    } else if (z.re < 0.0 && z.im > 0.0) {
        let xs = z.re - x_p;
        let ys = z.im + y_p;
        let r = sqrt(xs * xs + ys * ys);
        if (r < w) {
            let a = atan2(ys, xs) + spin + twist * (w - r);
            let r2 = w * r;
            fx = r2 * cos(a) + x_p;
            fy = r2 * sin(a) - y_p;
        } else {
            let r2 = w * (1.0 + space / max(r, 1.0e-30));
            fx = r2 * xs + x_p;
            fy = r2 * ys - y_p;
        }
    } else {
        fx = w * z.re;
        fy = w * z.im;
    }
    complex(fx * inv_w, fy * inv_w)
}
