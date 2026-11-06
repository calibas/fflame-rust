/// Log-spiral target — splits the plane into log-spaced spiral arms and
/// rotates each arm by `twist`. `n_of_sp` controls how many spirals
/// interleave; `tightness` controls how rapidly they wind.
///
/// # Authors
/// - Michael Faber
/// - DarkBeam

// Target Sp, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    twist: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Twist",
        hint: "Rotation applied to alternating spiral arms, in half-turns.",
    },
    n_of_sp: #{
        value: 1, min: -20, max: 20,
        caption: "N of Spirals",
        hint: "Number of interleaved spiral arms.",
    },
    size: #{
        value: 1.0, min: 0.01, max: 10.0, free: true,
        caption: "Size",
        hint: "Width of each spiral band in log-radius space.",
    },
    tightness: #{
        value: 0.5, min: -5.0, max: 5.0, free: true,
        caption: "Tightness",
        hint: "Logarithmic winding rate — higher = arms wind tighter.",
    },
};

const INFO = #{ title: "Target Sp", name: "rhai_target_sp", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let twist = p.twist;
    let size = p.size;
    let pi = 3.14159265358979;
    let rota = pi * twist;
    #{ t_size_2: 0.5 * size, rota: rota, rotb: -pi + rota }
}

fn variation(z, p, ctx) {
    let n_of_sp = to_float(p.n_of_sp);
    let size = max(1.0e-6, p.size);
    let tightness = p.tightness;
    let t_size_2 = p.t_size_2;
    let rota = p.rota;
    let rotb = p.rotb;
    let inv_pi = 0.3183098861837907;
    let pi = 3.14159265358979;
    let w_raw = ctx.weight;
    let inv_w = 1.0 / (if abs(w_raw) < 1.0e-30 { 1.0e-30 } else { w_raw });
    let a = atan2(z.im, z.re);
    let r = sqrt(z.re * z.re + z.im * z.im);
    let t = tightness * ln(max(1.0e-30, r)) + n_of_sp * inv_pi * (a + pi);
    if (t < 0.0) {
        t = t - t_size_2;
    }
    t = abs(t) - to_float(floor(abs(t) / size)) * size;
    if (t < t_size_2) {
        a = a + rota;
    } else {
        a = a + rotb;
    }
    complex(r * cos(a) * inv_w, r * sin(a) * inv_w)
}
