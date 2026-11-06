/// 26-mode "synthesizer" variation by slobo777 — combines 6 wave
/// layers (constant `a` + amplitudes `b..f`) into a `theta_factor`
/// scalar that drives one of many position / radius / angle
/// distortions. The mode picks WHICH distortion (e.g. swirl,
/// julia, rings, mirrors, blur variants, raw axis remaps); the
/// 6-layer waveform stack picks the SHAPE of the distortion.
///
/// Each layer b..f has six tunable knobs:
///   - amplitude (the param's bare name)
///   - `*_type`: which waveform (sin / cos / square / saw / triangle
///     / concave / convex / ngon / ingon)
///   - `*_skew`: how asymmetric the wave is (0 = symmetric, ±1 =
///     fully skewed)
///   - `*_frq`: angular frequency
///   - `*_phs`: phase offset
///   - `*_layer`: how this layer combines with the running
///     `theta_factor` (add / multiply / max / min)
///
/// Globals: `a` is the base offset, `mode` picks the distortion,
/// `power` is consumed by power-using modes, `mix` participates in
/// the SINCOS_MIXIN path, `smooth` is dual-purpose — interpolation
/// type for "Smooth YES" modes (0 = linear, 1 = Bezier-quad), or
/// sincos interpretation for "Smooth WAVE" modes (0 = multiply,
/// 1 = mixin).
///
/// # Authors
/// - slobo777

// Synth, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "a",
        hint: "Constant offset added at the start of every synth_value evaluation. Acts as the baseline `theta_factor` that layers b..f modulate.",
    },
    mode: #{
        value: 3, min: 0, max: 1007, free: true,
        caption: "Mode",
        hint: "Which distortion to apply. 0–19 are the original 2D modes (spherical / bubble / blur / raw / shift / mirror variants); 1001–1007 are the 'wave-smoothing' modes (sinusoidal / swirl / hyperbolic / julia / disc / rings / cylinder). The IDs are non-contiguous on purpose — they match JWildfire's file format.",
    },
    power: #{
        value: -2.0, min: -10.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Power exponent consumed by power-using modes (spherical, blur, shifts, etc.). For radial-power modes the exponent applied is `power / 2`; for the rings mode it gates the radial stripe width.",
    },
    mix: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Mix",
        hint: "Mix factor for SINCOS_MIXIN path. Affects how the sin/cos of `theta` is blended with the synth-modified value.",
    },
    smooth: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Smooth",
        hint: "Dual-purpose flag depending on the mode family: for 'Smooth YES' modes it picks the interpolation between the input radius and the synth target (0 = linear, 1 = Bezier-quad); for 'Smooth WAVE' modes it picks the synthsincos interpretation (0 = multiply, 1 = mixin).",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "b",
        hint: "Amplitude of the second synth layer. 0 disables the layer entirely (the whole block short-circuits in the body).",
    },
    b_type: #{
        value: 0, min: 0, max: 8, free: true,
        caption: "b_type",
        hint: "Waveform for layer b. 0=sin, 1=cos, 2=square, 3=saw, 4=triangle, 5=concave, 6=convex, 7=ngon (1/cos-1 style), 8=ingon (inverse ngon).",
    },
    b_skew: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "b_skew",
        hint: "Asymmetry of layer b's waveform. 0 keeps the half-period symmetric; positive squeezes the peak into the second half, negative into the first.",
    },
    b_frq: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "b_frq",
        hint: "Angular frequency of layer b — the input theta gets multiplied by this before the phase shift. Higher = more wiggles per revolution.",
    },
    b_phs: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "b_phs",
        hint: "Phase offset of layer b (radians, added to `theta * b_frq`).",
    },
    b_layer: #{
        value: 0, min: 0, max: 3, free: true,
        caption: "b_layer",
        hint: "How layer b combines with the running theta_factor. 0=add (a + b*x), 1=multiply (factor *= 1 + b*x), 2=max(factor, a + b*x), 3=min(factor, a + b*x).",
    },
    c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "c",
        hint: "Amplitude of layer c. See `b` for shape semantics.",
    },
    c_type: #{
        value: 0, min: 0, max: 8, free: true,
        caption: "c_type",
        hint: "Waveform for layer c. Same encoding as `b_type`.",
    },
    c_skew: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "c_skew",
        hint: "Asymmetry of layer c — see `b_skew`.",
    },
    c_frq: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "c_frq",
        hint: "Angular frequency of layer c — see `b_frq`.",
    },
    c_phs: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "c_phs",
        hint: "Phase offset of layer c — see `b_phs`.",
    },
    c_layer: #{
        value: 0, min: 0, max: 3, free: true,
        caption: "c_layer",
        hint: "Combination operator for layer c — see `b_layer`.",
    },
    d: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "d",
        hint: "Amplitude of layer d. See `b`.",
    },
    d_type: #{
        value: 0, min: 0, max: 8, free: true,
        caption: "d_type",
        hint: "Waveform for layer d. Same encoding as `b_type`.",
    },
    d_skew: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "d_skew",
        hint: "Asymmetry of layer d.",
    },
    d_frq: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "d_frq",
        hint: "Angular frequency of layer d.",
    },
    d_phs: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "d_phs",
        hint: "Phase offset of layer d.",
    },
    d_layer: #{
        value: 0, min: 0, max: 3, free: true,
        caption: "d_layer",
        hint: "Combination operator for layer d.",
    },
    e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "e",
        hint: "Amplitude of layer e.",
    },
    e_type: #{
        value: 0, min: 0, max: 8, free: true,
        caption: "e_type",
        hint: "Waveform for layer e.",
    },
    e_skew: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "e_skew",
        hint: "Asymmetry of layer e.",
    },
    e_frq: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "e_frq",
        hint: "Angular frequency of layer e.",
    },
    e_phs: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "e_phs",
        hint: "Phase offset of layer e.",
    },
    e_layer: #{
        value: 0, min: 0, max: 3, free: true,
        caption: "e_layer",
        hint: "Combination operator for layer e.",
    },
    f: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "f",
        hint: "Amplitude of layer f.",
    },
    f_type: #{
        value: 0, min: 0, max: 8, free: true,
        caption: "f_type",
        hint: "Waveform for layer f.",
    },
    f_skew: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "f_skew",
        hint: "Asymmetry of layer f.",
    },
    f_frq: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "f_frq",
        hint: "Angular frequency of layer f.",
    },
    f_phs: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "f_phs",
        hint: "Phase offset of layer f.",
    },
    f_layer: #{
        value: 0, min: 0, max: 3, free: true,
        caption: "f_layer",
        hint: "Combination operator for layer f.",
    },
};

const INFO = #{ title: "Synth", name: "rhai_synth", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn synth_apply_layer(theta, theta_factor_in, base_a, amp, layer_type, skew, frq, phs, layer_op) {
    if (amp == 0.0) {
        return theta_factor_in;
    }
    let TWO_PI = 6.283185307179586;
    let SYNTH_EPS = 1.0e-20;
    let z = phs + theta * frq;
    let y = z / TWO_PI;
    y = y - to_float(floor(y));
    if (skew != 0.0) {
        let z_split = 0.5 + 0.5 * skew;
        if (y > z_split) {
            y = 0.5 + 0.5 * (y - z_split) / (1.0 - z_split + SYNTH_EPS);
        } else {
            y = 0.5 - 0.5 * (z_split - y) / (z_split + SYNTH_EPS);
        }
    }
    let x = 0.0;
    let t1 = (layer_type);
    if t1 == 0 {
        x = sin(y * TWO_PI);
    } else if t1 == 1 {
        x = cos(y * TWO_PI);
    } else if t1 == 2 {
        x = (if y > 0.5 { 1.0 } else { -1.0 });
    } else if t1 == 3 {
        x = 1.0 - 2.0 * y;
    } else if t1 == 4 {
        x = (if y > 0.5 { 3.0 - 4.0 * y } else { 2.0 * y - 1.0 });
    } else if t1 == 5 {
        x = 8.0 * (y - 0.5) * (y - 0.5) - 1.0;
    } else if t1 == 6 {
        x = 2.0 * sqrt(max(0.0, y)) - 1.0;
    } else if t1 == 7 {
        let yy = (y - 0.5) * (TWO_PI / max(abs(frq), SYNTH_EPS));
        x = 1.0 / (cos(yy) + SYNTH_EPS) - 1.0;
    } else if t1 == 8 {
        let yy = (y - 0.5) * (TWO_PI / max(abs(frq), SYNTH_EPS));
        let zz = cos(yy);
        x = zz / (1.0 + SYNTH_EPS - zz);
    } else {
    }
    let t2 = (layer_op);
    if t2 == 0 {
        return theta_factor_in + amp * x;
    } else if t2 == 1 {
        return theta_factor_in * (1.0 + amp * x);
    } else if t2 == 2 {
        let v = base_a + amp * x;
        return (if theta_factor_in > v { theta_factor_in } else { v });
    } else if t2 == 3 {
        let v = base_a + amp * x;
        return (if theta_factor_in < v { theta_factor_in } else { v });
    } else {
    }
    theta_factor_in
}

fn synth_value(theta, p) {
    let a = p.a;
    let theta_factor = a;
    theta_factor = synth_apply_layer(theta, theta_factor, a, p.b, to_int(to_float(p.b_type)), p.b_skew, p.b_frq, p.b_phs, to_int(to_float(p.b_layer)));
    theta_factor = synth_apply_layer(theta, theta_factor, a, p.c, to_int(to_float(p.c_type)), p.c_skew, p.c_frq, p.c_phs, to_int(to_float(p.c_layer)));
    theta_factor = synth_apply_layer(theta, theta_factor, a, p.d, to_int(to_float(p.d_type)), p.d_skew, p.d_frq, p.d_phs, to_int(to_float(p.d_layer)));
    theta_factor = synth_apply_layer(theta, theta_factor, a, p.e, to_int(to_float(p.e_type)), p.e_skew, p.e_frq, p.e_phs, to_int(to_float(p.e_layer)));
    theta_factor = synth_apply_layer(theta, theta_factor, a, p.f, to_int(to_float(p.f_type)), p.f_skew, p.f_frq, p.f_phs, to_int(to_float(p.f_layer)));
    theta_factor
}

fn synth_bezier(x_in, m_in) {
    let x = x_in;
    let m = m_in;
    let sign_factor = 1.0;
    if (m < 0.0) {
        m = -m;
        sign_factor = -1.0;
    }
    if (x < 0.0) {
        x = -x;
        sign_factor = -sign_factor;
    }
    let i_m = 1.0e10;
    if (m > 1.0e-10) {
        i_m = 1.0 / m;
    }
    let L = (if (2.0 - m) > 2.0 * m { 2.0 - m } else { 2.0 * m });
    if (x > L || m == 1.0) {
        return sign_factor * x;
    }
    if (m < 1.0 && x <= 1.0) {
        let t = x;
        if ((m - 0.5) * (m - 0.5) > 1.0e-10) {
            t = (-m + sqrt(max(0.0, m * m + (1.0 - 2.0 * m) * x))) / (1.0 - 2.0 * m);
        }
        return sign_factor * (x + (m - 1.0) * t * t);
    }
    if (m > 1.0 && x <= 1.0) {
        let t = x;
        if ((m - 2.0) * (m - 2.0) > 1.0e-10) {
            t = (-i_m + sqrt(max(0.0, i_m * i_m + (1.0 - 2.0 * i_m) * x))) / (1.0 - 2.0 * i_m);
        }
        return sign_factor * (x + (m - 1.0) * t * t);
    }
    if (m < 1.0) {
        let t = sqrt(max(0.0, (x - 1.0) / (L - 1.0)));
        return sign_factor * (x + (m - 1.0) * t * t + 2.0 * (1.0 - m) * t + (m - 1.0));
    }
    let t = (1.0 - m) + sqrt(max(0.0, (m - 1.0) * (m - 1.0) + (x - 1.0)));
    sign_factor * (x + (m - 1.0) * t * t - 2.0 * (m - 1.0) * t + (m - 1.0))
}

fn synth_interp(x, m, lerp_type) {
    if (lerp_type == 1) {
        return synth_bezier(x, m);
    }
    x * m
}

fn synth_sincos(theta, sincos_type, p) {
    let HALF_PI = 1.5707963267948966;
    let s = sin(theta);
    let c = cos(theta);
    if (sincos_type == 0) {
        s = s * synth_value(theta, p);
        c = c * synth_value(theta + HALF_PI, p);
    } else if (sincos_type == 1) {
        let mix = p.mix;
        s = (1.0 - mix) * s + (synth_value(theta, p) - 1.0);
        c = (1.0 - mix) * c + (synth_value(theta + HALF_PI, p) - 1.0);
    }
    complex(s, c)
}

fn variation(z, p) {
    let PI = 3.14159265359;
    let TWO_PI = 6.283185307179586;
    let SYNTH_EPS = 1.0e-20;
    let mode = to_int(to_float(p.mode));
    let power = p.power;
    let smooth_i = to_int(to_float(p.smooth));
    let vx = z.re;
    let vy = z.im;
    let radius = 0.0;
    let theta = 0.0;
    let theta_factor = 0.0;
    let s = 0.0;
    let c = 0.0;
    let mu = 0.0;
    let ox = 0.0;
    let oy = 0.0;
    let r2 = vx * vx + vy * vy;
    let t1 = (mode);
    if t1 == 0 {
        radius = pow(r2 + SYNTH_EPS, (power + 1.0) * 0.5);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 1 {
        radius = sqrt(r2) / (r2 * 0.25 + 1.0);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 2 {
        radius = (rand() + rand() + 0.002 * rand()) / 2.002;
        theta = TWO_PI * rand() - PI();
        let bx = radius * sin(theta);
        let by = radius * cos(theta);
        radius = pow(radius * radius + SYNTH_EPS, power * 0.5);
        theta_factor = synth_value(theta, p);
        let rr = synth_interp(radius, theta_factor, smooth_i);
        ox = bx * rr;
        oy = by * rr;
    } else if t1 == 3 {
        radius = 0.5 * (rand() + rand());
        theta = TWO_PI * rand() - PI();
        radius = pow(radius * radius + SYNTH_EPS, -power * 0.5);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 4 {
        vy = 1.0 + 0.1 * (rand() + rand() - 1.0) * power;
        theta = 2.0 * asin((rand() - 0.5) * 2.0);
        theta_factor = synth_value(theta, p);
        vy = synth_interp(vy, theta_factor, smooth_i);
        ox = theta / PI();
        oy = vy - 1.0;
    } else if t1 == 5 {
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 6 {
        theta_factor = synth_value(vy, p);
        ox = synth_interp(vx, theta_factor, smooth_i);
        oy = vy;
    } else if t1 == 7 {
        theta_factor = synth_value(vx, p);
        ox = vx;
        oy = synth_interp(vy, theta_factor, smooth_i);
    } else if t1 == 8 {
        let tf_x = synth_value(vy, p);
        ox = synth_interp(vx, tf_x, smooth_i);
        let tf_y = synth_value(vx, p);
        oy = synth_interp(vy, tf_y, smooth_i);
    } else if t1 == 9 {
        ox = vx + synth_value(vy, p) - 1.0;
        oy = vy;
    } else if t1 == 10 {
        ox = vx;
        oy = vy + synth_value(vx, p) - 1.0;
    } else if t1 == 11 {
        ox = vx + synth_value(vy, p) - 1.0;
        oy = vy + synth_value(vx, p) - 1.0;
    } else if t1 == 12 {
        radius = 1.0 + 0.1 * (rand() + rand() - 1.0) * power;
        theta = TWO_PI * rand() - PI();
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 13 {
        theta = TWO_PI * rand() - PI();
        let rr = pow(rand() + SYNTH_EPS, power);
        radius = synth_value(theta, p) + 0.1 * rr;
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 14 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, p);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 15 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        s = sin(theta);
        c = cos(theta);
        mu = synth_value(radius, p) - 1.0;
        vx = vx + mu * c;
        vy = vy - mu * s;
        ox = vx;
        oy = vy;
    } else if t1 == 16 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, p);
        s = sin(theta);
        c = cos(theta);
        radius = sqrt(r2);
        ox = radius * s;
        oy = radius * c;
    } else if t1 == 17 {
        mu = synth_value(vx, p) - 1.0;
        vy = 2.0 * mu - vy;
        ox = vx;
        oy = vy;
    } else if t1 == 18 {
        mu = synth_value(vx, p) - 1.0;
        let mur = synth_value(vy, p) - 1.0;
        vy = 2.0 * mu - vy;
        vx = 2.0 * mur - vx;
        ox = vx;
        oy = vy;
    } else if t1 == 19 {
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        radius = pow(max(0.0, radius), power);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 1001 {
        let mix = p.mix;
        ox = synth_value(vx, p) - 1.0 + (1.0 - mix) * sin(vx);
        oy = synth_value(vy, p) - 1.0 + (1.0 - mix) * sin(vy);
    } else if t1 == 1002 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = s * vx - c * vy;
        oy = c * vx + s * vy;
    } else if t1 == 1003 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        let pair = synth_sincos(theta, smooth_i, p);
        s = pair.re;
        c = pair.im;
        let safe_r = (if abs(radius) < SYNTH_EPS { SYNTH_EPS } else { radius });
        ox = s / safe_r;
        oy = c * radius;
    } else if t1 == 1004 {
        radius = pow(r2 + SYNTH_EPS, power * 0.25);
        theta = atan2(vx, vy) * 0.5;
        if (rand() < 0.5) {
            theta = theta + PI();
        }
        let pair = synth_sincos(theta, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = radius * c;
        oy = radius * s;
    } else if t1 == 1005 {
        theta = atan2(vx, vy) / PI();
        radius = PI() * pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = s * theta;
        oy = c * theta;
    } else if t1 == 1006 {
        let safe_eps = SYNTH_EPS;
        let mu_r = power * power + safe_eps;
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        radius = radius + -2.0 * mu_r * to_float(floor((radius + mu_r) / (2.0 * mu_r))) + radius * (1.0 - mu_r);
        let pair = synth_sincos(theta, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = s * radius;
        oy = c * radius;
    } else if t1 == 1007 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(vx, smooth_i, p);
        s = pair.re;
        ox = radius * s;
        oy = radius * vy;
    } else {
    }
    complex(ox, oy)
}

fn variation3(v, p) {
    let PI = 3.14159265359;
    let TWO_PI = 6.283185307179586;
    let SYNTH_EPS = 1.0e-20;
    let mode = to_int(to_float(p.mode));
    let power = p.power;
    let smooth_i = to_int(to_float(p.smooth));
    let vx = v.x;
    let vy = v.y;
    let radius = 0.0;
    let theta = 0.0;
    let theta_factor = 0.0;
    let s = 0.0;
    let c = 0.0;
    let mu = 0.0;
    let ox = 0.0;
    let oy = 0.0;
    let r2 = vx * vx + vy * vy;
    let t1 = (mode);
    if t1 == 0 {
        radius = pow(r2 + SYNTH_EPS, (power + 1.0) * 0.5);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 1 {
        radius = sqrt(r2) / (r2 * 0.25 + 1.0);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 2 {
        radius = (rand() + rand() + 0.002 * rand()) / 2.002;
        theta = TWO_PI * rand() - PI();
        let bx = radius * sin(theta);
        let by = radius * cos(theta);
        radius = pow(radius * radius + SYNTH_EPS, power * 0.5);
        theta_factor = synth_value(theta, p);
        let rr = synth_interp(radius, theta_factor, smooth_i);
        ox = bx * rr;
        oy = by * rr;
    } else if t1 == 3 {
        radius = 0.5 * (rand() + rand());
        theta = TWO_PI * rand() - PI();
        radius = pow(radius * radius + SYNTH_EPS, -power * 0.5);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 4 {
        vy = 1.0 + 0.1 * (rand() + rand() - 1.0) * power;
        theta = 2.0 * asin((rand() - 0.5) * 2.0);
        theta_factor = synth_value(theta, p);
        vy = synth_interp(vy, theta_factor, smooth_i);
        ox = theta / PI();
        oy = vy - 1.0;
    } else if t1 == 5 {
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 6 {
        theta_factor = synth_value(vy, p);
        ox = synth_interp(vx, theta_factor, smooth_i);
        oy = vy;
    } else if t1 == 7 {
        theta_factor = synth_value(vx, p);
        ox = vx;
        oy = synth_interp(vy, theta_factor, smooth_i);
    } else if t1 == 8 {
        let tf_x = synth_value(vy, p);
        ox = synth_interp(vx, tf_x, smooth_i);
        let tf_y = synth_value(vx, p);
        oy = synth_interp(vy, tf_y, smooth_i);
    } else if t1 == 9 {
        ox = vx + synth_value(vy, p) - 1.0;
        oy = vy;
    } else if t1 == 10 {
        ox = vx;
        oy = vy + synth_value(vx, p) - 1.0;
    } else if t1 == 11 {
        ox = vx + synth_value(vy, p) - 1.0;
        oy = vy + synth_value(vx, p) - 1.0;
    } else if t1 == 12 {
        radius = 1.0 + 0.1 * (rand() + rand() - 1.0) * power;
        theta = TWO_PI * rand() - PI();
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 13 {
        theta = TWO_PI * rand() - PI();
        let rr = pow(rand() + SYNTH_EPS, power);
        radius = synth_value(theta, p) + 0.1 * rr;
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 14 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, p);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 15 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        s = sin(theta);
        c = cos(theta);
        mu = synth_value(radius, p) - 1.0;
        vx = vx + mu * c;
        vy = vy - mu * s;
        ox = vx;
        oy = vy;
    } else if t1 == 16 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, p);
        s = sin(theta);
        c = cos(theta);
        radius = sqrt(r2);
        ox = radius * s;
        oy = radius * c;
    } else if t1 == 17 {
        mu = synth_value(vx, p) - 1.0;
        vy = 2.0 * mu - vy;
        ox = vx;
        oy = vy;
    } else if t1 == 18 {
        mu = synth_value(vx, p) - 1.0;
        let mur = synth_value(vy, p) - 1.0;
        vy = 2.0 * mu - vy;
        vx = 2.0 * mur - vx;
        ox = vx;
        oy = vy;
    } else if t1 == 19 {
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, p);
        radius = synth_interp(radius, theta_factor, smooth_i);
        radius = pow(max(0.0, radius), power);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    } else if t1 == 1001 {
        let mix = p.mix;
        ox = synth_value(vx, p) - 1.0 + (1.0 - mix) * sin(vx);
        oy = synth_value(vy, p) - 1.0 + (1.0 - mix) * sin(vy);
    } else if t1 == 1002 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = s * vx - c * vy;
        oy = c * vx + s * vy;
    } else if t1 == 1003 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        let pair = synth_sincos(theta, smooth_i, p);
        s = pair.re;
        c = pair.im;
        let safe_r = (if abs(radius) < SYNTH_EPS { SYNTH_EPS } else { radius });
        ox = s / safe_r;
        oy = c * radius;
    } else if t1 == 1004 {
        radius = pow(r2 + SYNTH_EPS, power * 0.25);
        theta = atan2(vx, vy) * 0.5;
        if (rand() < 0.5) {
            theta = theta + PI();
        }
        let pair = synth_sincos(theta, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = radius * c;
        oy = radius * s;
    } else if t1 == 1005 {
        theta = atan2(vx, vy) / PI();
        radius = PI() * pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = s * theta;
        oy = c * theta;
    } else if t1 == 1006 {
        let mu_r = power * power + SYNTH_EPS;
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        radius = radius + -2.0 * mu_r * to_float(floor((radius + mu_r) / (2.0 * mu_r))) + radius * (1.0 - mu_r);
        let pair = synth_sincos(theta, smooth_i, p);
        s = pair.re;
        c = pair.im;
        ox = s * radius;
        oy = c * radius;
    } else if t1 == 1007 {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(vx, smooth_i, p);
        s = pair.re;
        ox = radius * s;
        oy = radius * vy;
    } else {
    }
    vec3(ox, oy, v.z)
}
