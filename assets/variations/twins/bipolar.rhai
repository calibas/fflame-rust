// Bipolar, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shift: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Shift",
        hint: "Vertical offset on the output. Slides the bipolar pattern up or down.",
    },
};

const INFO = #{ title: "Bipolar", name: "rhai_bipolar" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let HALF_PI = 1.57079632679;
    let shift = p.shift;
    let x2y2 = z.norm_sqr();
    let y = 0.5 * atan2(2.0 * z.im, x2y2 - 1.0) + (-HALF_PI * shift);
    if (y > HALF_PI) {
        y = -HALF_PI + (y + HALF_PI) % PI();
    } else if (y < -HALF_PI) {
        y = HALF_PI - (HALF_PI - y) % PI();
    }
    let t = x2y2 + 1.0;
    let x2 = 2.0 * z.re;
    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return complex(0.0, 0.0);
    }
    let new_x = (1.0 / (2.0 * PI())) * ln(f / g);
    let new_y = (2.0 / PI()) * y;
    complex(new_x, new_y)
}
