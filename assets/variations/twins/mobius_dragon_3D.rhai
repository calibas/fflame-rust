/// 3D Möbius/reciprocal dragon with a Log_tile spread — divides by a complex
/// constant, takes a weighted 3D reciprocal, feeds back for `iterations`,
/// then scatters via `spread·round(log(rand)/log(log_spread))` with a random
/// sign branch. Optional magnitude-based colour tint.
///
/// # Authors
/// - Whittaker Courtney

// Mobius Dragon 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    re: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Re",
        hint: "Real part of the complex divisor `(re, im)` applied each iteration.",
    },
    im: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Im",
        hint: "Imaginary part of the complex divisor.",
    },
    x_spread: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Spread",
        hint: "Log-tile scatter magnitude on X (sign randomized per iteration).",
    },
    y_spread: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Spread",
        hint: "Log-tile scatter magnitude on Y.",
    },
    z_spread: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Spread",
        hint: "Log-tile scatter magnitude on Z.",
    },
    x_add: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X Add",
        hint: "Constant X offset added in the positive log-tile branch.",
    },
    y_add: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Add",
        hint: "Constant Y offset added in the positive log-tile branch.",
    },
    log_spread: #{
        value: 2.71828, min: 1.01, max: 100.0, free: true,
        caption: "Log Spread",
        hint: "Log base for the tile index `round(log(rand)/log(log_spread))`. Default e.",
    },
    line_enable: #{
        value: true,
        caption: "Line Enable",
        hint: "When on, with probability `line_weight` the iteration collapses to a horizontal line segment (`z.re = rand·spreadx`, z.im = 0).",
    },
    line_weight: #{
        value: 0.125, min: 0.0, max: 1.0, free: true,
        caption: "Line Weight",
        hint: "Probability that the line branch fires (compared against the captured per-call random, so it's all-or-nothing across the loop).",
    },
    line_color_shift: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Line Color Shift",
        hint: "Dead in JWildfire — the loop's `color += line_color_shift` is overwritten by the pre-loop colour snapshot. Kept for round-trip; no effect.",
    },
    mag_color: #{
        value: true,
        caption: "Mag Color",
        hint: "When on, adds `tanh(|output|·mag_color_scale/6)` to the colour register, tinting by point magnitude.",
    },
    mag_color_scale: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Mag Color Scale",
        hint: "Scales the magnitude before the `tanh` colour term.",
    },
    iterations: #{
        value: 1, min: 0, max: 24,
        caption: "Iterations",
        hint: "Number of Möbius feedback iterations (0-24). Each divides by `(re, im)`, takes the weighted 3D reciprocal, and feeds back.",
    },
};

const INFO = #{ title: "Mobius Dragon 3D", name: "rhai_mobius_dragon_3d", category: "full_3d", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let re = p.re;
    let im = p.im;
    let x_spread = p.x_spread;
    let y_spread = p.y_spread;
    let x_add = p.x_add;
    let y_add = p.y_add;
    let log_spread = p.log_spread;
    let line_enable = to_int(to_float(p.line_enable));
    let line_weight = p.line_weight;
    let mag_color = to_int(to_float(p.mag_color));
    let mag_color_scale = p.mag_color_scale;
    let iterations = clamp(to_int(to_float(p.iterations)), 0, 24);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let random = rand();
    let z1 = 0.0;
    let aff_x = z.re;
    let aff_y = z.im;
    let spreadx = -x_spread;
    let spready = -y_spread;
    let zre = z.re;
    let zim = z.im + 1.0;
    let pz = 0.0;
    let d2 = re * re + im * im;
    let safe_d2 = (if abs(d2) < 1.0e-30 { 1.0e-30 } else { d2 });
    let safe_log_ls = (if abs(ln(log_spread)) < 1.0e-30 { 1.0e-30 } else { ln(log_spread) });
    for i in 0..24 {
        if (i >= iterations) {
            break;
        }
        zre = aff_x;
        zim = aff_y + 1.0;
        let nre = (zre * re + zim * im) / safe_d2;
        let nim = (zim * re - zre * im) / safe_d2;
        let denom = nre * nre + nim * nim + z1 * z1;
        let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
        let r1 = w / safe_denom;
        zre = nre * r1;
        zim = -nim * r1;
        pz = -z1 * r1;
        zre = zre * w;
        zim = zim * w;
        aff_x = zre;
        aff_y = zim + 1.0;
        if (line_enable == 1 && random < line_weight) {
            zre = rand() * spreadx;
            zim = 0.0;
            pz = 0.0;
        }
    }
    if (rand() < 0.5) {
        spreadx = x_spread;
    }
    if (rand() < 0.5) {
        spready = y_spread;
    }
    let _zskip = rand();
    if (random < 0.5) {
        zre = w * x_add + (zre + spreadx * round_even(ln(max(1.0e-30, rand())) / safe_log_ls));
        zim = w * (y_add + (zim + spready * round_even(ln(max(1.0e-30, rand())) / safe_log_ls))) + 1.0;
        let _z = rand();
    } else {
        zre = w * (-zre + spreadx * round_even(ln(max(1.0e-30, rand())) / safe_log_ls));
        zim = w * (-2.0 - (zim + spready * round_even(ln(max(1.0e-30, rand())) / safe_log_ls))) + 1.0;
        let _z = rand();
    }
    if (mag_color == 1) {
        let mag = sqrt(zre * zre + zim * zim + z1 * z1) * (mag_color_scale / 6.0);
        color = color + tanh(mag);
    }
    #{ z: complex(zre * inv_w, zim * inv_w), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let re = p.re;
    let im = p.im;
    let x_spread = p.x_spread;
    let y_spread = p.y_spread;
    let z_spread = p.z_spread;
    let x_add = p.x_add;
    let y_add = p.y_add;
    let log_spread = p.log_spread;
    let line_enable = to_int(to_float(p.line_enable));
    let line_weight = p.line_weight;
    let mag_color = to_int(to_float(p.mag_color));
    let mag_color_scale = p.mag_color_scale;
    let iterations = clamp(to_int(to_float(p.iterations)), 0, 24);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let random = rand();
    let z1 = v.z;
    let aff_x = v.x;
    let aff_y = v.y;
    let spreadx = -x_spread;
    let spready = -y_spread;
    let spreadz = -z_spread;
    let zre = v.x;
    let zim = v.y + 1.0;
    let pz = 0.0;
    let d2 = re * re + im * im;
    let safe_d2 = (if abs(d2) < 1.0e-30 { 1.0e-30 } else { d2 });
    let safe_log_ls = (if abs(ln(log_spread)) < 1.0e-30 { 1.0e-30 } else { ln(log_spread) });
    for i in 0..24 {
        if (i >= iterations) {
            break;
        }
        zre = aff_x;
        zim = aff_y + 1.0;
        let nre = (zre * re + zim * im) / safe_d2;
        let nim = (zim * re - zre * im) / safe_d2;
        let denom = nre * nre + nim * nim + z1 * z1;
        let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
        let r1 = w / safe_denom;
        zre = nre * r1;
        zim = -nim * r1;
        pz = -z1 * r1;
        zre = zre * w;
        zim = zim * w;
        aff_x = zre;
        aff_y = zim + 1.0;
        if (line_enable == 1 && random < line_weight) {
            zre = rand() * spreadx;
            zim = 0.0;
            pz = 0.0;
        }
    }
    if (rand() < 0.5) {
        spreadx = x_spread;
    }
    if (rand() < 0.5) {
        spready = y_spread;
    }
    if (rand() < 0.5) {
        spreadz = z_spread;
    }
    if (random < 0.5) {
        zre = w * x_add + (zre + spreadx * round_even(ln(max(1.0e-30, rand())) / safe_log_ls));
        zim = w * (y_add + (zim + spready * round_even(ln(max(1.0e-30, rand())) / safe_log_ls))) + 1.0;
        pz = w * (pz + spreadz * round_even(ln(max(1.0e-30, rand())) / safe_log_ls));
    } else {
        zre = w * (-zre + spreadx * round_even(ln(max(1.0e-30, rand())) / safe_log_ls));
        zim = w * (-2.0 - (zim + spready * round_even(ln(max(1.0e-30, rand())) / safe_log_ls))) + 1.0;
        pz = w * (-pz + spreadz * round_even(ln(max(1.0e-30, rand())) / safe_log_ls));
    }
    if (mag_color == 1) {
        let mag = sqrt(zre * zre + zim * zim + z1 * z1) * (mag_color_scale / 6.0);
        color = color + tanh(mag);
    }
    #{ z: vec3(zre * inv_w, zim * inv_w, pz * inv_w), color: color }
}
