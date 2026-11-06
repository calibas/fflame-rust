/// Radial branch-and-mod (kaleidoscope-style) — converts the input to polar
/// `(r, a)`, splits the angle into `num` equal sectors, and mirrors/shifts
/// alternating sectors based on the parity of the sector index, with a per-
/// sector angular offset `a`. Produces a kaleidoscope-like radial tiling.
///
/// # Authors
/// - Michael Faber

// Collideoscope, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Per-sector angular offset — the parity-based mirror is shifted by ±a.",
    },
    num: #{
        value: 1, min: 1, max: 50,
        caption: "N",
        hint: "Number of angular sectors. Higher = finer kaleidoscope.",
    },
};

const INFO = #{ title: "Collideoscope", name: "rhai_collideoscope", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a_p = p.a;
    let num = max(to_float(p.num), 1.0);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_kn = pi / num;
    let ka_kn = pi * a_p / num;
    let a = atan2(z.im, z.re);
    let r = sqrt(z.re * z.re + z.im * z.im);
    let alt_input = (if a >= 0.0 { a } else { -a });
    let alt = to_float(floor(alt_input * num * inv_pi));
    let alt_i = to_int(alt);
    if (a >= 0.0) {
        if (alt_i % 2 == 0) {
            let v = ka_kn + a;
            a = alt * pi_kn + (v - to_float(floor(v / pi_kn)) * pi_kn);
        } else {
            let v = -ka_kn + a;
            a = alt * pi_kn + (v - to_float(floor(v / pi_kn)) * pi_kn);
        }
    } else if (alt_i % 2 != 0) {
        let v = -ka_kn - a;
        a = -(alt * pi_kn + (v - to_float(floor(v / pi_kn)) * pi_kn));
    } else {
        let v = ka_kn - a;
        a = -(alt * pi_kn + (v - to_float(floor(v / pi_kn)) * pi_kn));
    }
    complex(r * cos(a), r * sin(a))
}
