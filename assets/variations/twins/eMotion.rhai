// E-Motion, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    move: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Move",
        hint: "Additive offset on μ (sign chosen by the sign of ν).",
    },
    rotate: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rotate",
        hint: "Additive offset on ν.",
    },
};

const INFO = #{ title: "E-Motion", name: "rhai_emotion" };

fn variation(z, p) {
    let move_p = p.move;
    let rotate = p.rotate;
    let tmp = z.im * z.im + z.re * z.re + 1.0;
    let tmp2 = 2.0 * z.re;
    let xmax_raw = (sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5;
    let xmax = max(xmax_raw, 1.0);
    let mu = acosh(xmax);
    let t = clamp(z.re / xmax, -1.0, 1.0);
    let nu = acos(t);
    if (z.im < 0.0) {
        nu = -nu;
    }
    if (nu < 0.0) {
        mu = mu + move_p;
    } else {
        mu = mu - move_p;
    }
    if (mu <= 0.0) {
        mu = -mu;
        nu = -nu;
    }
    nu = nu + rotate;
    complex(cosh(mu) * cos(nu), sinh(mu) * sin(nu))
}
