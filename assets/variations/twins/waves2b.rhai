/// Per-axis sine-perturbation displacement (generalization of `waves2`) —
/// adds a wave term to each axis driven by the *other* axis's coordinate.
/// The wave function is chosen by the per-axis power param: `pw > 1e-4` or
/// `pw < -1e-4` → power-mode sine `sin(sgn(c)·|c|^pw·freq)`; `pw ∈ [0,
/// 1e-4)` → Jacobi `sn` (with modulus `jacok`); `pw ∈ (-1e-4, 0)` → Bessel
/// `J1`. Per-axis amplitude is a smooth interpolation between `scale*` (at
/// origin) and `scaleinf*` (at infinity), controlled by `unity`.
///
/// # Authors
/// - DarkBeam

// Waves2 B, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    freqx: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Freq X",
        hint: "Frequency of the wave applied to the X axis (driven by Y).",
    },
    freqy: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Freq Y",
        hint: "Frequency of the wave applied to the Y axis (driven by X).",
    },
    pwx: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Power X",
        hint: "X-axis power exponent — also selects the wave function. `> 1e-4` or `< -1e-4` = power-mode sine of `|c|^pw`; `[0, 1e-4)` = Jacobi `sn`; `(-1e-4, 0)` = Bessel `J1`.",
    },
    pwy: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Power Y",
        hint: "Y-axis power exponent — same threshold-based mode selector as `pwx`.",
    },
    scalex: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X-axis amplitude at the origin (full damping).",
    },
    scaleinfx: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Inf X",
        hint: "X-axis amplitude at infinity (the limiting amplitude as `x → ±∞`).",
    },
    scaley: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y-axis amplitude at the origin.",
    },
    scaleinfy: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Inf Y",
        hint: "Y-axis amplitude at infinity.",
    },
    unity: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Unity",
        hint: "Crossover scale for the amplitude interpolation between `scale*` and `scaleinf*`. Larger values keep the origin-amplitude active over a wider region.",
    },
    jacok: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Jacobi K",
        hint: "Modulus `k` for the Jacobi `sn` mode (only used when `pwx` or `pwy` falls in `[0, 1e-4)`).",
    },
};

const INFO = #{ title: "Waves2 B", name: "rhai_waves2b", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ six: p.scalex - p.scaleinfx, siy: p.scaley - p.scaleinfy }
}

fn w2b_bessel_j1(x) {
    let ax = abs(x);
    let s = (if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 });
    if (ax < 1.0e-30) {
        return 0.0;
    }
    if (ax <= 3.0) {
        let y = (x / 3.0) * (x / 3.0);
        let p = 0.5 - 0.56249985 * y + 0.21093573 * y * y - 0.03954289 * y * y * y + 0.00443319 * y * y * y * y - 0.00031761 * y * y * y * y * y + 0.00001109 * y * y * y * y * y * y;
        return x * p;
    }
    let z = 3.0 / ax;
    let f1 = 0.79788456 + 0.00000156 * z + 0.01659667 * z * z + 0.00017105 * z * z * z - 0.00249511 * z * z * z * z + 0.00113653 * z * z * z * z * z - 0.00020033 * z * z * z * z * z * z;
    let theta1 = ax - 2.35619449 + 0.12499612 * z + 0.00005650 * z * z - 0.00637879 * z * z * z + 0.00074348 * z * z * z * z + 0.00079824 * z * z * z * z * z - 0.00029166 * z * z * z * z * z * z;
    s * f1 / sqrt(ax) * cos(theta1)
}

fn w2b_jac_sn(uu, emmc_in) {
    let ca = 0.0003;
    let emc = emmc_in;
    let u = uu;
    if (abs(emc) < 1.0e-30) {
        return tanh(u);
    }
    let d = 1.0;
    let bo = false;
    if (emc < 0.0) {
        bo = true;
        d = 1.0 - emc;
        emc = -emc / d;
        d = sqrt(d);
        u = d * u;
    }
    let a = 1.0;
    let c = 0.0;
    let em = array(8, 0.0);
    let en = array(8, 0.0);
    let l = 0;
    for i in 0..8 {
        l = i;
        em[i] = a;
        emc = sqrt(max(0.0, emc));
        en[i] = emc;
        c = 0.5 * (a + emc);
        if (abs(a - emc) <= ca * a) {
            break;
        }
        emc = a * emc;
        a = c;
    }
    u = c * u;
    let sn = sin(u);
    let cn_init = cos(u);
    let dn = 1.0;
    if (abs(sn) > 1.0e-30) {
        a = cn_init / sn;
        c = a * c;
        let ii = l;
        while ii >= 0 {
            let b = em[ii];
            a = c * a;
            c = dn * c;
            dn = (en[ii] + a) / (b + a);
            a = c / b;
            ii = ii - 1;
        }
        let inv = 1.0 / sqrt(c * c + 1.0);
        sn = (if sn < 0.0 { -inv } else { inv });
    }
    if (bo) {
        sn = sn / d;
    }
    sn
}

fn w2b_safediv(q, r) {
    if (r < 1.0e-10) {
        return 1.0 / max(1.0e-30, r);
    }
    q / r
}

fn w2b_axis_term(c_in, c_other, freq, pw, jacok, csx) {
    if (pw >= 0.0 && pw < 1.0e-4) {
        return csx * w2b_jac_sn(c_other * freq, jacok);
    }
    if (pw < 0.0 && pw > -1.0e-4) {
        return csx * w2b_bessel_j1(c_other * freq);
    }
    let s = (if c_other < 0.0 { -1.0 } else { 1.0 });
    csx * sin(s * pow(abs(c_other) + 1.0e-10, pw) * freq)
}

fn variation(z, p) {
    let freqx = p.freqx;
    let freqy = p.freqy;
    let pwx = p.pwx;
    let pwy = p.pwy;
    let scaleinfx = p.scaleinfx;
    let scaleinfy = p.scaleinfy;
    let unity = p.unity;
    let jacok = p.jacok;
    let six = p.six;
    let siy = p.siy;
    let csx = w2b_safediv(unity, unity + z.re * z.re) * six + scaleinfx;
    let csy = w2b_safediv(unity, unity + z.im * z.im) * siy + scaleinfy;
    let nx = z.re + w2b_axis_term(z.re, z.im, freqx, pwx, jacok, csx);
    let ny = z.im + w2b_axis_term(z.im, z.re, freqy, pwy, jacok, csy);
    complex(nx, ny)
}
