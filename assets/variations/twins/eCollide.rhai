/// Elliptic warp with ν-axis branch-and-mod — converts to elliptic
/// coordinates `(μ, ν)`, splits ν into `num` equal wedges, and mirrors
/// alternating wedges with a per-wedge offset. Elliptic analogue of
/// `bcollide`.
///
/// # Authors
/// - Michael Faber

// ECollide, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    num: #{
        value: 1, min: 1, max: 50,
        caption: "N",
        hint: "Number of ν wedges.",
    },
    a: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Per-wedge angular offset.",
    },
};

const INFO = #{ title: "ECollide", name: "rhai_ecollide", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let num = max(1.0, to_float(p.num));
    let a = p.a;
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_n = pi / num;
    let a_n = pi * a / num;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let tmp2 = 2.0 * z.re;
    let xmax = max(1.0, (sqrt(max(0.0, tmp + tmp2)) + sqrt(max(0.0, tmp - tmp2))) * 0.5);
    let t = clamp(z.re / xmax, -1.0, 1.0);
    let nu = acos(t);
    let alt = to_float(floor(nu * num * inv_pi));
    if (to_int(alt) % 2 == 0) {
        nu = alt * pi_n + ((nu + a_n) - to_float(floor((nu + a_n) / pi_n)) * pi_n);
    } else {
        nu = alt * pi_n + ((nu - a_n) - to_float(floor((nu - a_n) / pi_n)) * pi_n);
    }
    if (z.im <= 0.0) {
        nu = -nu;
    }
    complex(xmax * cos(nu), sqrt(max(0.0, xmax - 1.0)) * sqrt(max(0.0, xmax + 1.0)) * sin(nu))
}

fn variation3(v, p) {
    let num = max(1.0, to_float(p.num));
    let a = p.a;
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_n = pi / num;
    let a_n = pi * a / num;
    let tmp = v.y * v.y + v.x * v.x + 1.0;
    let tmp2 = 2.0 * v.x;
    let xmax = max(1.0, (sqrt(max(0.0, tmp + tmp2)) + sqrt(max(0.0, tmp - tmp2))) * 0.5);
    let t = clamp(v.x / xmax, -1.0, 1.0);
    let nu = acos(t);
    let alt = to_float(floor(nu * num * inv_pi));
    if (to_int(alt) % 2 == 0) {
        nu = alt * pi_n + ((nu + a_n) - to_float(floor((nu + a_n) / pi_n)) * pi_n);
    } else {
        nu = alt * pi_n + ((nu - a_n) - to_float(floor((nu - a_n) / pi_n)) * pi_n);
    }
    if (v.y <= 0.0) {
        nu = -nu;
    }
    vec3(xmax * cos(nu), sqrt(max(0.0, xmax - 1.0)) * sqrt(max(0.0, xmax + 1.0)) * sin(nu), v.z)
}
