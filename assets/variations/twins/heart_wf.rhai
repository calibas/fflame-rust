/// Polar heart-curve mapping — maps the input radius `r` and angle `a` onto
/// a heart-shaped curve via `nx = ±0.001·(-t² + 40t + 1200)·sin(πt/180)·r`
/// and `ny = -0.001·(-t² + 40t + 400)·cos(πt/180)·r` where `t = |a|/π · 60
/// · scale_r − shift_t` (capped at 60). The sign of `a` selects the left vs
/// right half of the heart, each with its own radial scale.

// Heart WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scale_x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X output scale factor (multiplies the final X output).",
    },
    scale_t: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale T",
        hint: "Unused in body — preserved as a parameter for cpp parity and preset compatibility.",
    },
    shift_t: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift T",
        hint: "T parameter offset subtracted from the `|a|/π · 60 · scale_r` expression.",
    },
    scale_r_left: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale R Left",
        hint: "Radial scale for the left half of the heart (input angle `a < 0`).",
    },
    scale_r_right: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale R Right",
        hint: "Radial scale for the right half (input angle `a ≥ 0`).",
    },
};

const INFO = #{ title: "Heart WF", name: "rhai_heart_wf", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let scale_x = p.scale_x;
    let shift_t = p.shift_t;
    let scale_r_left = p.scale_r_left;
    let scale_r_right = p.scale_r_right;
    let pi = 3.14159265358979;
    let t_max = 60.0;
    let a = atan2(z.re, z.im);
    let r = sqrt(z.re * z.re + z.im * z.im);
    let t = 0.0;
    let nx = 0.0;
    if (a < 0.0) {
        t = -a / pi * t_max * scale_r_left - shift_t;
        if (t > t_max) {
            t = t_max;
        }
        nx = -0.001 * (-t * t + 40.0 * t + 1200.0) * sin(pi * t / 180.0) * r;
    } else {
        t = a / pi * t_max * scale_r_right - shift_t;
        if (t > t_max) {
            t = t_max;
        }
        nx = 0.001 * (-t * t + 40.0 * t + 1200.0) * sin(pi * t / 180.0) * r;
    }
    let ny = -0.001 * (-t * t + 40.0 * t + 400.0) * cos(pi * t / 180.0) * r;
    complex(nx * scale_x, ny)
}
