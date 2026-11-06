/// Joukowski-airfoil-style complex map — computes `k = z + sqrt(z² - a)`
/// (one root of the quadratic `k² - 2k·z + a = 0`), then mirrors small-
/// magnitude outputs back out via a conjugate-and-rotate step (`k → conj(k)
/// · (-a/|a|)` when `|k|² < |a|`) and forces a positive real part. Named
/// for its resemblance to the Joukowski transform `z + 1/z`.
///
/// # Authors
/// - DarkBeam

// Plus Recip, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    ar: #{
        value: 4.0, min: -10.0, max: 10.0, free: true,
        caption: "A Real",
        hint: "Real part of the complex constant `a` in `k = z + sqrt(z² - a)`. Controls the location of the branch cut.",
    },
    ai: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A Imag",
        hint: "Imaginary part of the complex constant `a`.",
    },
};

const INFO = #{ title: "Plus Recip", name: "rhai_plusrecip", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn pr_cabs(z) {
    sqrt(z.re * z.re + z.im * z.im)
}

fn pr_cmul(a, b) {
    complex(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn pr_csqrt(z) {
    let r = pr_cabs(z);
    let real_p = sqrt(max(0.0, (r + z.re) * 0.5));
    let imag_p = sqrt(max(0.0, (r - z.re) * 0.5));
    let s = (if z.im < 0.0 { -1.0 } else { 1.0 });
    complex(real_p, s * imag_p)
}

fn variation(w, p) {
    let ar = p.ar;
    let ai = p.ai;
    let z = complex(w.re, w.im);
    let a = complex(ar, ai);
    let aa = sqrt(a.re * a.re + a.im * a.im + 1.0e-30);
    let z2 = pr_cmul(z, z);
    let k = pr_csqrt(z2 - a) + z;
    let k2 = pr_cmul(k, k);
    if (sqrt(k2.re * k2.re + k2.im * k2.im + 1.0e-30) < aa) {
        let k_conj = complex(k.re, -k.im);
        let a_scaled = -a / aa;
        k = pr_cmul(k_conj, a_scaled);
    }
    if (k.re < 0.0) {
        k = -k;
    }
    k
}

fn variation3(v, p) {
    let ar = p.ar;
    let ai = p.ai;
    let z = complex(v.x, v.y);
    let a = complex(ar, ai);
    let aa = sqrt(a.re * a.re + a.im * a.im + 1.0e-30);
    let z2 = pr_cmul(z, z);
    let k = pr_csqrt(z2 - a) + z;
    let k2 = pr_cmul(k, k);
    if (sqrt(k2.re * k2.re + k2.im * k2.im + 1.0e-30) < aa) {
        let k_conj = complex(k.re, -k.im);
        let a_scaled = -a / aa;
        k = pr_cmul(k_conj, a_scaled);
    }
    if (k.re < 0.0) {
        k = -k;
    }
    vec3(k.re, k.im, v.z)
}
